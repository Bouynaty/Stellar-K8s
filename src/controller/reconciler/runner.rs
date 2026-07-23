//! Controller runner — sets up the kube-rs [`Controller`] watch loop.
//!
//! Extracted from the monolithic `reconciler.rs` so that the startup /
//! watch-binding code is easy to read independently from reconciliation logic.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Service};
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use kube::{
    api::Api,
    client::Client,
    runtime::{controller::Controller, watcher::Config},
};
use tracing::{error, info};

use crate::controller::audit_worker::AuditWorker;
use crate::controller::reconciler::{
    batch::BatchSummaryReport, core::reconcile, state::ControllerState,
};
use crate::crd::StellarNode;
use crate::error::{Error, Result};

/// Error policy — requeue after a short backoff.
pub(crate) fn error_policy(
    _obj: Arc<StellarNode>,
    _err: &crate::error::Error,
    ctx: Arc<ControllerState>,
) -> kube::runtime::controller::Action {
    kube::runtime::controller::Action::requeue(Duration::from_secs(
        ctx.retry_budget_retriable_secs,
    ))
}

/// Helper: return a namespaced or cluster-scoped API depending on watch config.
fn api_for<K>(client: Client, ns: &Option<String>) -> Api<K>
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + std::fmt::Debug
        + 'static,
    K::DynamicType: Default,
{
    match ns {
        Some(n) => Api::namespaced(client, n),
        None => Api::all(client),
    }
}

/// Main entry point to start the controller.
///
/// Initialises the watch loop, background workers (Spot Drain, Quorum
/// Optimizer, Horizon Metrics Collector, Audit Worker), and the controller
/// fold accumulator.
pub async fn run_controller(state: Arc<ControllerState>) -> Result<()> {
    let client = state.client.clone();
    let ns = &state.watch_namespace;
    let stellar_nodes: Api<StellarNode> = api_for(client.clone(), ns);

    info!(
        mode = if let Some(n) = ns { n.as_str() } else { "cluster-scoped" },
        "Starting StellarNode controller",
    );

    // Verify CRD is installed.
    match stellar_nodes.list(&Default::default()).await {
        Ok(_) => info!("StellarNode CRD is available"),
        Err(e) => {
            error!(error = %e, "StellarNode CRD not found — install the CRD first");
            return Err(Error::ConfigError(
                "StellarNode CRD not installed".to_string(),
            ));
        }
    }

    // Background: Node Drain Orchestrator.
    {
        let drain = Arc::new(crate::controller::maintenance::NodeDrainOrchestrator::new(
            client.clone(),
            state.event_reporter.clone(),
        ));
        tokio::spawn(async move {
            if let Err(e) = drain.run().await {
                error!(error = %e, "Node Drain Orchestrator stopped with error");
            }
        });
    }

    // Background: Spot/Preemptible Drain Handler.
    if let Ok(node_name) = std::env::var("NODE_NAME") {
        let spot = Arc::new(crate::controller::spot_drain::SpotDrainHandler::new(
            client.clone(),
            state.event_reporter.clone(),
            node_name,
        ));
        tokio::spawn(async move {
            if let Err(e) = spot.run().await {
                error!(error = %e, "Spot Drain Handler stopped with error");
            }
        });
    } else {
        info!("NODE_NAME env var not set — Spot Drain Handler disabled");
    }

    // Background: Horizon Metrics Collector (rest-api feature).
    #[cfg(feature = "rest-api")]
    {
        use crate::controller::horizon_metrics_collector::spawn_horizon_metrics_collector;
        let store = state.metrics_store.clone();
        let coll_client = client.clone();
        let coll_ns = state.watch_namespace.clone();
        tokio::spawn(async move {
            let handle = spawn_horizon_metrics_collector(store, 30, coll_client, coll_ns);
            if let Err(e) = handle.await {
                error!(error = ?e, "Horizon Metrics Collector stopped with error");
            }
        });
    }

    // Background: Quorum Optimizer.
    {
        let qo = Arc::new(crate::controller::quorum::QuorumOptimizer::new(
            client.clone(),
            state.event_reporter.clone(),
        ));
        tokio::spawn(async move {
            if let Err(e) = qo.run().await {
                error!(error = %e, "Quorum Optimizer stopped with error");
            }
        });
    }

    // Background: Audit Worker.
    if state.operator_config.audit.enabled {
        let aw = AuditWorker::new(client.clone(), state.audit_recorder.clone());
        tokio::spawn(async move {
            if let Err(e) = aw.run().await {
                error!(error = %e, "Audit Worker stopped with error");
            }
        });
    }

    // Build the controller, wiring up owned-resource watches.
    Controller::new(stellar_nodes, Config::default())
        .owns::<Deployment>(api_for(client.clone(), ns), Config::default())
        .owns::<StatefulSet>(api_for(client.clone(), ns), Config::default())
        .owns::<Service>(api_for(client.clone(), ns), Config::default())
        .owns::<PersistentVolumeClaim>(api_for(client.clone(), ns), Config::default())
        .owns::<PodDisruptionBudget>(api_for(client.clone(), ns), Config::default())
        .watches::<k8s_openapi::api::core::v1::Secret, _>(
            api_for(client.clone(), ns),
            Config::default(),
            |_secret| vec![],
        )
        .shutdown_on_signal()
        .run(
            |obj, ctx| reconcile(obj, ctx),
            error_policy,
            state.clone(),
        )
        .fold(BatchSummaryReport::new(50), {
            let state = state.clone();
            move |mut report, res| {
                let state = state.clone();
                async move {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    state
                        .last_event_received
                        .store(now, std::sync::atomic::Ordering::Relaxed);

                    match res {
                        Ok(obj) => {
                            let name = format!("{:?}", obj);
                            info!(object = %name, "Reconciled");
                            report.record_success(name);
                        }
                        Err(e) => {
                            let err_str = format!("{:?}", e);
                            error!(error = %err_str, "Reconcile error");
                            report.record_failure("unknown".to_string(), err_str);
                        }
                    }
                    report
                }
            }
        })
        .await
        .emit_final_summary();

    Ok(())
}
