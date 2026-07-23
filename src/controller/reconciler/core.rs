//! Core reconcile state-machine for StellarNode resources.
//!
//! This module contains only the `reconcile()` function and its direct
//! helpers.  Infrastructure concerns (state, events, dry-run, batch reporting,
//! runner setup) live in their own sibling modules.
//!
//! # Design
//!
//! The reconciler follows "declarative convergence":
//! 1. **Observe** current cluster state.
//! 2. **Compute** the delta between `spec` and `status`.
//! 3. **Apply** patches to drive the cluster toward desired state.
//!
//! # Error handling
//!
//! Retriable errors return `Action::requeue`; non-retriable errors return
//! `Action::requeue` with a longer backoff so the controller does not storm.

use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use futures::FutureExt;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::{
        controller::Action,
        events::{EventType, Reporter},
    },
    Resource, ResourceExt,
};
use tracing::{debug, error, info, warn};

use crate::controller::reconciler::{events::ToStellarNodeArc, state::ControllerState};
use crate::crd::{Condition, StellarNode, StellarNodeStatus};
use crate::error::{Error, Result};

/// Format structured spec-validation errors into a user-friendly message.
fn format_spec_validation_errors(errors: &[crate::crd::SpecValidationError]) -> String {
    let mut msg = String::from("Spec validation failed with the following issues:\n");
    for e in errors {
        msg.push_str(&format!(
            "- Field `{}`: {}\n  How to fix: {}\n",
            e.field, e.message, e.how_to_fix
        ));
    }
    msg.trim_end().to_string()
}

/// Emit a single grouped Kubernetes Event for all spec-validation errors.
async fn emit_spec_validation_event(
    client: &kube::client::Client,
    reporter: &Reporter,
    node: &StellarNode,
    errors: &[crate::crd::SpecValidationError],
) -> Result<()> {
    let message = format_spec_validation_errors(errors);
    crate::controller::reconciler::events::emit_event_owned(
        client.clone(),
        reporter.clone(),
        node.to_arc(),
        EventType::Warning,
        "SpecValidationFailed".to_string(),
        "ValidationFailed".to_string(),
        message,
    )
    .await
}

/// The core reconcile function wired into the kube-rs controller loop.
///
/// Returns `Ok(Action)` on both success and handled errors.  Only truly
/// unrecoverable conditions should bubble up as `Err`.
pub fn reconcile(
    obj: Arc<StellarNode>,
    ctx: Arc<ControllerState>,
) -> BoxFuture<'static, Result<Action>> {
    async move {
        let node_name = obj.name_any();
        let namespace = obj.namespace().unwrap_or_else(|| "default".to_string());

        if !ctx.is_leader.load(std::sync::atomic::Ordering::Relaxed) {
            debug!("Not the leader — skipping reconciliation");
            return Ok(Action::requeue(Duration::from_secs(5)));
        }

        let client = ctx.client.clone();
        let api: Api<StellarNode> = Api::namespaced(client.clone(), &namespace);

        info!(
            node = %node_name,
            namespace = %namespace,
            node_type = ?obj.spec.node_type,
            reconcile_id = ctx.next_reconcile_id(),
            "Reconciling StellarNode",
        );

        // --- Deletion / finalizer handling -----------------------------------
        if obj.metadata.deletion_timestamp.is_some() {
            info!(node = %node_name, "Node is being deleted — running cleanup");
            if let Err(e) = handle_deletion(&client, &ctx, &obj).await {
                error!(node = %node_name, error = %e, "Deletion handler failed");
                return Ok(Action::requeue(Duration::from_secs(
                    ctx.retry_budget_retriable_secs,
                )));
            }
            return Ok(Action::await_change());
        }

        // --- Ensure finalizer present ----------------------------------------
        if let Err(e) = crate::controller::finalizers::ensure_finalizer(&client, &obj).await {
            warn!(node = %node_name, error = %e, "Failed to add finalizer — requeuing");
            return Ok(Action::requeue(Duration::from_secs(
                ctx.retry_budget_retriable_secs,
            )));
        }

        // --- Spec validation --------------------------------------------------
        match obj.spec.validate() {
            Ok(()) => {}
            Err(errors) => {
                warn!(
                    node = %node_name,
                    error_count = errors.len(),
                    "Spec validation failed",
                );
                if let Err(e) =
                    emit_spec_validation_event(&client, &ctx.event_reporter, &obj, &errors).await
                {
                    warn!(node = %node_name, error = %e, "Failed to emit spec-validation event");
                }
                let status_patch = build_invalid_status(&obj, &errors);
                let _ = api
                    .patch_status(
                        &node_name,
                        &PatchParams::apply("stellar-operator").force(),
                        &Patch::Apply(&status_patch),
                    )
                    .await;
                return Ok(Action::requeue(Duration::from_secs(
                    ctx.retry_budget_nonretriable_secs,
                )));
            }
        }

        // --- Delegate to the resource reconciler -----------------------------
        match crate::controller::resources::reconcile_node_resources(
            &client,
            &ctx,
            &obj,
        )
        .await
        {
            Ok(()) => {
                // Record audit annotation (best-effort).
                crate::controller::audit::patch_audit_annotations(
                    &client,
                    &obj,
                    crate::controller::audit::actions::RECONCILED,
                )
                .await;

                ctx.last_reconcile_success.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    std::sync::atomic::Ordering::Relaxed,
                );

                info!(node = %node_name, "Reconcile complete");
                Ok(Action::requeue(Duration::from_secs(30)))
            }
            Err(Error::KubeError(ref e))
                if e.to_string().contains("Conflict") || e.to_string().contains("timeout") =>
            {
                warn!(node = %node_name, error = %e, "Retriable error — requeuing");
                Ok(Action::requeue(Duration::from_secs(
                    ctx.retry_budget_retriable_secs,
                )))
            }
            Err(e) => {
                error!(node = %node_name, error = %e, "Non-retriable reconcile error");
                Ok(Action::requeue(Duration::from_secs(
                    ctx.retry_budget_nonretriable_secs,
                )))
            }
        }
    }
    .boxed()
}

// ── Private helpers ───────────────────────────────────────────────────────────

async fn handle_deletion(
    client: &kube::client::Client,
    ctx: &Arc<ControllerState>,
    node: &Arc<StellarNode>,
) -> Result<()> {
    crate::controller::finalizers::handle_finalizer(client, ctx, node).await
}

fn build_invalid_status(
    node: &StellarNode,
    errors: &[crate::crd::SpecValidationError],
) -> serde_json::Value {
    let conditions = vec![Condition {
        type_: "Valid".to_string(),
        status: "False".to_string(),
        reason: Some("SpecValidationFailed".to_string()),
        message: Some(format_spec_validation_errors(errors)),
        last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    }];

    serde_json::json!({
        "apiVersion": "stellar.org/v1alpha1",
        "kind": "StellarNode",
        "metadata": {
            "name": node.name_any(),
            "namespace": node.namespace().unwrap_or_else(|| "default".to_string()),
        },
        "status": StellarNodeStatus {
            conditions: Some(conditions),
            ..Default::default()
        }
    })
}
