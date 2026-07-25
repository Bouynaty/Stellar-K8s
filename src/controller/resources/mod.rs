//! Kubernetes resource builders for StellarNode — split into focused sub-modules.
//!
//! # Sub-modules
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`meta`] | Labels, annotations, owner-references, common helpers |
//! | [`pvc`] | PersistentVolumeClaim create / update / delete |
//! | [`config_map`] | ConfigMap create / update / delete |
//! | [`deployment`] | Deployment (Horizon, SorobanRpc) create / update |
//! | [`statefulset`] | StatefulSet (Validator) create / update |
//! | [`service`] | Service create / update |
//! | [`probes`] | Default liveness / readiness / startup probes |
//! | [`reconcile`] | Top-level `reconcile_node_resources` entry-point |
//!
//! All public symbols from the old monolithic `resources.rs` are re-exported
//! here so call-sites need only `use crate::controller::resources::*`.

pub mod config_map;
pub mod deployment;
pub mod meta;
pub mod probes;
pub mod pvc;
pub mod reconcile;
pub mod service;
pub mod statefulset;

// Re-export the most-used public API surface.
pub use config_map::{build_config_map, delete_config_map, ensure_config_map};
pub use deployment::ensure_deployment;
pub use meta::{
    merge_service_annotations, merge_service_metadata_labels, owner_reference, resource_name,
    standard_labels,
};
pub use probes::{apply_probe_override_pub, default_liveness_probe, default_readiness_probe};
pub use pvc::{build_pvc, delete_pvc, ensure_pvc};
pub use reconcile::reconcile_node_resources;
pub use service::ensure_service;
pub use statefulset::ensure_statefulset;
