//! Service create / update / delete for StellarNode.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{Service, ServicePort, ServiceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{Api, Patch, PatchParams};
use kube::{Client, ResourceExt};
use tracing::{instrument, info};

use crate::controller::resource_meta::merge_resource_meta;
use crate::controller::resources::meta::{
    merge_service_annotations, merge_service_metadata_labels, owner_reference, standard_labels,
};
use crate::crd::{NodeType, StellarNode};
use crate::error::Result;

fn patch_params(dry_run: bool) -> PatchParams {
    let mut params = PatchParams::apply("stellar-operator").force();
    if dry_run { params.dry_run = true; }
    params
}

/// Ensure the primary Service exists for a StellarNode.
#[instrument(skip(client, node), fields(name = %node.name_any(), namespace = node.namespace()))]
pub async fn ensure_service(
    client: &Client,
    node: &StellarNode,
    dry_run: bool,
) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let api: Api<Service> = Api::namespaced(client.clone(), &namespace);
    let name = node.name_any();

    let svc = build_service(node);
    info!(service = %name, "Applying Service");
    api.patch(&name, &patch_params(dry_run), &Patch::Apply(&svc))
        .await?;
    Ok(())
}

fn build_service(node: &StellarNode) -> Service {
    let mut labels = standard_labels(node);
    merge_service_metadata_labels(&mut labels, node);

    let mut annotations: BTreeMap<String, String> = BTreeMap::new();
    merge_service_annotations(&mut annotations, node);

    let selector = {
        let mut s = BTreeMap::new();
        s.insert("app.kubernetes.io/instance".to_string(), node.name_any());
        s.insert(
            "app.kubernetes.io/name".to_string(),
            "stellar-node".to_string(),
        );
        s
    };

    let ports = match node.spec.node_type {
        NodeType::Validator => vec![
            ServicePort {
                name: Some("peer".to_string()),
                port: 11625,
                target_port: Some(IntOrString::Int(11625)),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
            ServicePort {
                name: Some("http".to_string()),
                port: 11626,
                target_port: Some(IntOrString::Int(11626)),
                protocol: Some("TCP".to_string()),
                ..Default::default()
            },
        ],
        NodeType::Horizon | NodeType::SorobanRpc => vec![ServicePort {
            name: Some("http".to_string()),
            port: 8000,
            target_port: Some(IntOrString::Int(8000)),
            protocol: Some("TCP".to_string()),
            ..Default::default()
        }],
    };

    Service {
        metadata: merge_resource_meta(
            ObjectMeta {
                name: Some(node.name_any()),
                namespace: node.namespace(),
                labels: Some(labels),
                annotations: if annotations.is_empty() { None } else { Some(annotations) },
                owner_references: Some(vec![owner_reference(node)]),
                ..Default::default()
            },
            &None,
        ),
        spec: Some(ServiceSpec {
            selector: Some(selector),
            ports: Some(ports),
            type_: Some("ClusterIP".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}
