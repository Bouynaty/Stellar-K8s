//! Advanced Security Scanning and Vulnerability Management Module
//!
//! Provides automated scanning, runtime monitoring, and automated remediation.

pub mod cert_manager;
pub mod cert_rotation;
pub mod compliance;
pub mod kms;
pub mod policy;
pub mod remediation;
pub mod runtime;
pub mod secret_audit;
pub mod secret_metrics;
pub mod secret_rotation;
pub mod secret_sync;
pub mod vulnerability;

pub use cert_manager::{CertManager, CertMetrics, CertStatus, CertificateInfo, RenewalConfig};
pub use cert_rotation::{
    CertExpiryAlert, CertIssuanceRequest, CertIssuanceResponse, CertRecord, CertRotationController,
    CertRotationError, ExpiryMonitor, ExpiryMonitorConfig, ExpirySeverity, LocalCaBackend,
    PkiBackend, RotationAuditLog, RotationEvent, RotationTrigger, VaultPkiBackend, VaultPkiConfig,
};

use serde::{Deserialize, Serialize};

/// Security finding summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub component: String,
    pub severity: SecuritySeverity,
    pub description: String,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum SecuritySeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for SecuritySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecuritySeverity::Critical => write!(f, "CRITICAL"),
            SecuritySeverity::High => write!(f, "HIGH"),
            SecuritySeverity::Medium => write!(f, "MEDIUM"),
            SecuritySeverity::Low => write!(f, "LOW"),
        }
    }
}

/// Security posture report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPosture {
    pub overall_score: f32,
    pub findings: Vec<SecurityFinding>,
    pub compliance_status: bool,
}
