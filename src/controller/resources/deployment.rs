//! Deployment builder for Horizon and SorobanRpc nodes.

use std::collections::BTreeMap;

use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::api::{Api, Patch, PatchParams};
use kube::{Client, ResourceExt};
use tracing::{info, instrument};

use crate::controller::label_propagation::LabelPropagator;
use crate::controller::resource_meta::merge_resource_meta;
use crate::controller::resources::meta::{owner_reference, standard_labels};
use crate::controller::resources::probes::{
    apply_probe_override, default_liveness_probe, default_readiness_probe, default_startup_probe,
};
use crate::crd::StellarNode;
use crate::error::{Error, Result};

fn patch_params(dry_run: bool) -> PatchParams {
    let mut params = PatchParams::apply("stellar-operator").force();
    if dry_run { params.dry_run = true; }
    params
}

/// Ensure a Deployment exists for Horizon or SorobanRpc nodes.
#[instrument(skip(client, node, propagated_labels),
    fields(name = %node.name_any(), namespace = node.namespace()))]
pub async fn ensure_deployment(
    client: &Client,
    node: &StellarNode,
    enable_mtls: bool,
    propagated_labels: &BTreeMap<String, String>,
    dry_run: bool,
) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let api: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
    let name = node.name_any();

    let existing_labels = match api.get(&name).await {
        Ok(existing) => existing.metadata.labels.clone().unwrap_or_default(),
        Err(kube::Error::Api(e)) if e.code == 404 => BTreeMap::new(),
        Err(e) => return Err(Error::KubeError(e)),
    };

    let mut deploy = build_deployment(node, enable_mtls);

    // Label propagation.
    let base_labels = deploy.metadata.labels.clone().unwrap_or_default();
    let merged = LabelPropagator::merge_onto(&base_labels, propagated_labels);
    let final_labels =
        LabelPropagator::remove_stale_labels(&merged, propagated_labels, &existing_labels);
    deploy.metadata.labels = Some(final_labels);

    info!(deployment = %name, "Applying Deployment");
    api.patch(&name, &patch_params(dry_run), &Patch::Apply(&deploy))
        .await?;
    Ok(())
}

fn build_deployment(node: &StellarNode, _enable_mtls: bool) -> Deployment {
    let labels = standard_labels(node);
    let name = node.name_any();
    let image = node.spec.image.clone();

    let liveness = apply_probe_override(
        Some(default_liveness_probe(&node.spec.node_type)),
        node.spec.liveness_probe_override.as_ref(),
    );
    let readiness = apply_probe_override(
        Some(default_readiness_probe(&node.spec.node_type)),
        node.spec.readiness_probe_override.as_ref(),
    );
    let startup = apply_probe_override(
        Some(default_startup_probe(&node.spec.node_type)),
        node.spec.startup_probe_override.as_ref(),
    );

    let container = Container {
        name: "stellar-node".to_string(),
        image: Some(image),
        liveness_probe: liveness,
        readiness_probe: readiness,
        startup_probe: startup,
        ..Default::default()
    };

    let pod_template = PodTemplateSpec {
        metadata: Some(ObjectMeta {
            labels: Some(labels.clone()),
            ..Default::default()
        }),
        spec: Some(PodSpec {
            containers: vec![container],
            ..Default::default()
        }),
    };

    Deployment {
        metadata: merge_resource_meta(
            ObjectMeta {
                name: Some(name.clone()),
                namespace: node.namespace(),
                labels: Some(labels.clone()),
                owner_references: Some(vec![owner_reference(node)]),
                ..Default::default()
            },
            &None,
        ),
        spec: Some(DeploymentSpec {
            replicas: node.spec.replicas.map(|r| r as i32).or(Some(1)),
            selector: LabelSelector {
                match_labels: Some({
                    let mut s = BTreeMap::new();
                    s.insert("app.kubernetes.io/instance".to_string(), name);
                    s.insert(
                        "app.kubernetes.io/name".to_string(),
                        "stellar-node".to_string(),
                    );
                    s
                }),
                ..Default::default()
            },
            template: pod_template,
            ..Default::default()
        }),
        ..Default::default()
    }
}
