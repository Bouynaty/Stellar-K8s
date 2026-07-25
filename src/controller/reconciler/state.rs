//! [`ControllerState`] — shared state for the StellarNode controller.
//!
//! Extracted from the monolithic `reconciler.rs` so that the type definition
//! lives in its own focused module.  All fields are public so that the runner,
//! reconciler core, and helper modules can access them without indirection.

use std::sync::Arc;

use kube::client::Client;
use kube::runtime::events::Reporter;
use tracing_subscriber::{reload::Handle, EnvFilter, Registry};

/// Shared state for the controller.
///
/// Holds the Kubernetes client and every piece of shared context needed by the
/// reconciler.  The value is wrapped in `Arc<ControllerState>` and cloned into
/// each reconcile call.
pub struct ControllerState {
    /// Kubernetes client for API interactions.
    pub client: Client,
    pub enable_mtls: bool,
    pub operator_namespace: String,
    /// Restrict the operator to only watch StellarNode resources in this namespace.
    /// `None` means cluster-scoped (all namespaces).
    pub watch_namespace: Option<String>,
    pub mtls_config: Option<crate::MtlsConfig>,
    pub dry_run: bool,
    /// Requeue interval (seconds) for retriable errors.
    pub retry_budget_retriable_secs: u64,
    /// Requeue interval (seconds) for non-retriable errors.
    pub retry_budget_nonretriable_secs: u64,
    /// Maximum HTTP retry attempts for SCP / quorum queries.
    pub retry_budget_max_attempts: u32,
    pub is_leader: Arc<std::sync::atomic::AtomicBool>,
    /// Identifies this operator instance in Kubernetes Events.
    pub event_reporter: Reporter,
    /// Operator-level config loaded from the Helm-rendered ConfigMap.
    pub operator_config: Arc<crate::controller::operator_config::OperatorConfig>,
    /// Monotonically-increasing counter for unique reconcile IDs.
    pub reconcile_id_counter: std::sync::atomic::AtomicU64,
    /// Unix-epoch timestamp of the last successful reconcile.
    pub last_reconcile_success: Arc<std::sync::atomic::AtomicU64>,
    /// Handle to reload the tracing `EnvFilter` at runtime.
    pub log_reload_handle: Handle<EnvFilter, Registry>,
    /// Optional expiry for a temporary log-level override.
    pub log_level_expires_at:
        Arc<tokio::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>>,
    /// Unix-epoch timestamp of the last event received from the K8s watch stream.
    pub last_event_received: Arc<std::sync::atomic::AtomicU64>,
    /// Background job registry for the monitoring dashboard.
    pub job_registry: Arc<crate::controller::background_jobs::JobRegistry>,
    /// In-memory audit log for admin activity.
    pub audit_log: Arc<crate::controller::audit_log::AuditLog>,
    /// Unified audit recorder (in-memory log + optional sink).
    pub audit_recorder: Arc<crate::controller::audit_recorder::AuditRecorder>,
    /// ML-based anomaly detector for operator behaviour.
    pub anomaly_detector: Arc<crate::controller::anomaly_detection::AnomalyDetector>,
    /// Plugin registry for custom reconciliation hooks and sidecar injectors.
    pub plugin_registry: Arc<crate::plugin_sdk::PluginRegistry>,
    /// Log analytics engine for pattern detection and anomaly reporting.
    pub analytics_engine: Arc<crate::logging::analytics::AnalyticsEngine>,
    /// Optional OIDC configuration for the REST API.
    #[cfg(feature = "rest-api")]
    pub oidc_config: Option<crate::rest_api::OidcConfig>,
    /// Thread-safe cache of Stellar metrics shared between the background
    /// [`HorizonMetricsCollector`] and the custom-metrics API handlers.
    #[cfg(feature = "rest-api")]
    pub metrics_store: Arc<crate::rest_api::metrics_store::StellarMetricsStore>,
}

impl ControllerState {
    /// Generate a unique reconcile ID (monotonically increasing).
    pub fn next_reconcile_id(&self) -> u64 {
        self.reconcile_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}
