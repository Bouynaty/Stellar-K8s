//! ConfigMap create / update / delete for StellarNode.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::ConfigMap;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, Patch, PatchParams};
use kube::{Client, ResourceExt};
use tracing::{info, instrument, warn};

use crate::controller::resource_meta::merge_resource_meta;
use crate::controller::resources::meta::{owner_reference, resource_name, standard_labels};
use crate::crd::{HistoryMode, NodeType, StellarNode};
use crate::error::{Error, Result};

fn patch_params(dry_run: bool) -> PatchParams {
    let mut params = PatchParams::apply("stellar-operator").force();
    if dry_run { params.dry_run = true; }
    params
}

fn delete_params(dry_run: bool) -> DeleteParams {
    if dry_run {
        DeleteParams { dry_run: true, ..Default::default() }
    } else {
        DeleteParams::default()
    }
}

/// Build the desired ConfigMap for a StellarNode.
pub fn build_config_map(
    node: &StellarNode,
    quorum_override: Option<crate::controller::vsl::QuorumSet>,
    enable_mtls: bool,
) -> ConfigMap {
    let labels = standard_labels(node);
    let name = resource_name(node, "config");
    let mut data = BTreeMap::new();

    data.insert(
        "NETWORK_PASSPHRASE".to_string(),
        node.spec.network_passphrase().to_string(),
    );

    if enable_mtls {
        data.insert("MTLS_ENABLED".to_string(), "true".to_string());
    }

    match &node.spec.node_type {
        NodeType::Validator => {
            let mut core_cfg = String::new();
            if let Some(config) = &node.spec.validator_config {
                if let Some(qs) = quorum_override {
                    core_cfg.push_str(&qs.to_stellar_core_toml());
                } else if let Some(q) = &config.quorum_set {
                    core_cfg.push_str(q);
                }
            }
            if enable_mtls {
                core_cfg.push_str("\n# mTLS Configuration\n");
                core_cfg.push_str("HTTP_PORT_SECURE=true\n");
                core_cfg.push_str("TLS_CERT_FILE=\"/etc/stellar/tls/tls.crt\"\n");
                core_cfg.push_str("TLS_KEY_FILE=\"/etc/stellar/tls/tls.key\"\n");
            }
            match node.spec.history_mode {
                HistoryMode::Full => {
                    core_cfg.push_str("\n# Full History Mode\nCATCHUP_COMPLETE=true\n");
                }
                HistoryMode::Recent => {
                    core_cfg.push_str(
                        "\n# Recent History Mode\nCATCHUP_COMPLETE=false\nCATCHUP_RECENT=60480\n",
                    );
                }
            }
            if !core_cfg.is_empty() {
                data.insert("stellar-core.cfg".to_string(), core_cfg);
            }
        }
        NodeType::Horizon => {
            if let Some(config) = &node.spec.horizon_config {
                data.insert("STELLAR_CORE_URL".to_string(), config.stellar_core_url.clone());
                data.insert("INGEST".to_string(), config.enable_ingest.to_string());
            }
        }
        NodeType::SorobanRpc => {
            if let Some(config) = &node.spec.soroban_config {
                data.insert("STELLAR_CORE_URL".to_string(), config.stellar_core_url.clone());

                if config.captive_core_structured_config.is_some() {
                    match crate::controller::captive_core::CaptiveCoreConfigBuilder::from_node_config(node) {
                        Ok(builder) => match builder.build_toml() {
                            Ok(toml) => { data.insert("captive-core.cfg".to_string(), toml); }
                            Err(e) => tracing::warn!("Failed to build Captive Core TOML: {}", e),
                        },
                        Err(e) => tracing::warn!("Failed to create Captive Core builder: {}", e),
                    }
                } else {
                    #[allow(deprecated)]
                    if let Some(captive_config) = &config.captive_core_config {
                        data.insert("captive-core.cfg".to_string(), captive_config.clone());
                    }
                }
            }
        }
    }

    let annotations = node.spec.storage.annotations.clone().unwrap_or_default();

    ConfigMap {
        metadata: merge_resource_meta(
            ObjectMeta {
                name: Some(name),
                namespace: node.namespace(),
                labels: Some(labels),
                annotations: if annotations.is_empty() { None } else { Some(annotations) },
                owner_references: Some(vec![owner_reference(node)]),
                ..Default::default()
            },
            &None,
        ),
        data: Some(data),
        ..Default::default()
    }
}

/// Ensure a ConfigMap exists for the given node.
#[instrument(skip(client, node), fields(name = %node.name_any(), namespace = node.namespace()))]
pub async fn ensure_config_map(
    client: &Client,
    node: &StellarNode,
    quorum_override: Option<crate::controller::vsl::QuorumSet>,
    enable_mtls: bool,
    dry_run: bool,
) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), &namespace);
    let name = resource_name(node, "config");

    let cm = build_config_map(node, quorum_override, enable_mtls);
    api.patch(&name, &patch_params(dry_run), &Patch::Apply(&cm))
        .await?;
    Ok(())
}

/// Delete the ConfigMap for a node.
#[instrument(skip(client, node), fields(name = %node.name_any(), namespace = node.namespace()))]
pub async fn delete_config_map(client: &Client, node: &StellarNode, dry_run: bool) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), &namespace);
    let name = resource_name(node, "config");

    match api.delete(&name, &delete_params(dry_run)).await {
        Ok(_) => info!(configmap = %name, "Deleted ConfigMap"),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            warn!(configmap = %name, "ConfigMap not found — already deleted");
        }
        Err(e) => return Err(Error::KubeError(e)),
    }
    Ok(())
}
