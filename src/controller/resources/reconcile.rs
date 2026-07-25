//! Top-level resource reconcile entry-point.
//!
//! `reconcile_node_resources` is called by `controller::reconciler::core` after
//! spec validation.  It fans out to the individual resource builders (PVC,
//! ConfigMap, Service, Deployment / StatefulSet).

use std::collections::BTreeMap;

use kube::Client;

use crate::controller::label_propagation::LabelPropagator;
use crate::controller::reconciler::state::ControllerState;
use crate::controller::resources::{
    config_map::ensure_config_map,
    deployment::ensure_deployment,
    pvc::ensure_pvc,
    service::ensure_service,
    statefulset::ensure_statefulset,
};
use crate::crd::{NodeType, StellarNode};
use crate::error::Result;

/// Reconcile all Kubernetes resources owned by a StellarNode.
///
/// Called by the core reconciler after spec validation succeeds.
pub async fn reconcile_node_resources(
    client: &Client,
    ctx: &std::sync::Arc<ControllerState>,
    node: &StellarNode,
) -> Result<()> {
    let dry_run = ctx.dry_run;
    let enable_mtls = ctx.enable_mtls;

    // Build propagated labels from the LabelPropagator.
    let propagated_labels: BTreeMap<String, String> = LabelPropagator::new(
        ctx.operator_config
            .label_propagation
            .propagate_labels
            .clone()
            .unwrap_or_default(),
        ctx.operator_config
            .label_propagation
            .exclude_labels
            .clone()
            .unwrap_or_default(),
        ctx.operator_config
            .label_propagation
            .label_filters
            .clone()
            .unwrap_or_default(),
    )
    .propagate(node);

    // 1. PVC
    ensure_pvc(client, node, &propagated_labels, dry_run).await?;

    // 2. ConfigMap (quorum override resolved inside if needed)
    ensure_config_map(client, node, None, enable_mtls, dry_run).await?;

    // 3. Service
    ensure_service(client, node, dry_run).await?;

    // 4. Workload — StatefulSet for Validators, Deployment for the rest
    match node.spec.node_type {
        NodeType::Validator => {
            ensure_statefulset(client, node, enable_mtls, &propagated_labels, dry_run).await?;
        }
        NodeType::Horizon | NodeType::SorobanRpc => {
            ensure_deployment(client, node, enable_mtls, &propagated_labels, dry_run).await?;
        }
    }

    Ok(())
}
