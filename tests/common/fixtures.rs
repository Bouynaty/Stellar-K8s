/// tests/common/fixtures.rs
///
/// Isolated, deterministic test fixtures for integration and unit test suites.
///
/// # Design (issue #1140, consolidated from tests/fixtures/mod.rs per #1196)
///
/// Every fixture function returns a fully-constructed value with sensible
/// defaults. Tests can customise via builder-style overrides. No fixture
/// function allocates cluster resources — that is the responsibility of the
/// test guards in `common/mod.rs`.
///
/// Fixture categories:
/// - `stellarnode_*`  — `StellarNodeSpec` and related CRD types
/// - `backup_*`       — `BackupVerificationConfig` and `BackupSource`
/// - `rotation_*`     — `SecretRotationConfig`
/// - `manifest_*`     — raw YAML strings for `kubectl apply` tests
/// - `k8s_*`          — Kubernetes API objects (Pods, Containers, VolumeMounts)
/// - `deterministic`  — SeededRng, fixed timestamps, deterministic name helpers
use k8s_openapi::api::core::v1::{Container, VolumeMount};
use rand::SeedableRng;

// ---------------------------------------------------------------------------
// Deterministic test utilities (consolidated from tests/fixtures/mod.rs)
// ---------------------------------------------------------------------------

/// Deterministic RNG for tests. Use `SeededRng::seeded(seed)` for reproducible
/// results.
pub struct SeededRng(rand::rngs::SmallRng);

impl SeededRng {
    pub fn seeded(seed: u64) -> Self {
        Self(rand::rngs::SmallRng::seed_from_u64(seed))
    }

    pub fn inner(&mut self) -> &mut rand::rngs::SmallRng {
        &mut self.0
    }
}

/// Fixed "now" timestamp for deterministic time-sensitive tests.
///
/// Returns 2026-01-15T12:00:00Z. Use this instead of `Utc::now()` in tests
/// to avoid flaky time-dependent assertions.
pub fn fixed_now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-15T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

/// Generate a deterministic test namespace name from a seed.
pub fn test_namespace(seed: &str) -> String {
    format!("test-{}", seed)
}

/// Generate a deterministic test StellarNode name from a seed.
pub fn test_node_name(seed: &str) -> String {
    format!("node-{}", seed)
}

// ---------------------------------------------------------------------------
// StellarNode fixtures
// ---------------------------------------------------------------------------

/// Unique namespace name for an integration test.
///
/// Includes a short random suffix so parallel tests do not collide even when
/// the same test binary runs more than once against the same cluster.
///
/// # Example
/// ```
/// use tests::common::fixtures::unique_namespace;
/// let ns = unique_namespace("backup-test");
/// // "stellar-it-backup-test-a1b2c3d4"
/// ```
pub fn unique_namespace(label: &str) -> String {
    // Use thread ID + timestamp for a lightweight unique suffix that works
    // without pulling in uuid or rand at the test level.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("stellar-it-{label}-{ts:08x}")
}

/// Minimal valid `StellarNode` YAML for a Testnet Validator.
///
/// Uses `retentionPolicy: Delete` so PVCs are cleaned up automatically and
/// tests do not leave orphaned storage in the cluster.
pub fn testnet_validator_manifest(name: &str, namespace: &str) -> String {
    format!(
        r#"apiVersion: stellar.org/v1alpha1
kind: StellarNode
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    app.kubernetes.io/managed-by: stellar-k8s-integration-test
spec:
  nodeType: Validator
  network: Testnet
  version: "v21.0.0"
  storage:
    storageClass: standard
    size: 10Gi
    retentionPolicy: Delete
"#
    )
}

/// Minimal valid `StellarNode` YAML for a Testnet Horizon node.
pub fn testnet_horizon_manifest(name: &str, namespace: &str) -> String {
    format!(
        r#"apiVersion: stellar.org/v1alpha1
kind: StellarNode
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    app.kubernetes.io/managed-by: stellar-k8s-integration-test
spec:
  nodeType: Horizon
  network: Testnet
  version: "v2.28.0"
  storage:
    storageClass: standard
    size: 50Gi
    retentionPolicy: Delete
"#
    )
}

/// Minimal valid `StellarNode` YAML for a Testnet Soroban RPC node.
pub fn testnet_soroban_manifest(name: &str, namespace: &str) -> String {
    format!(
        r#"apiVersion: stellar.org/v1alpha1
kind: StellarNode
metadata:
  name: {name}
  namespace: {namespace}
  labels:
    app.kubernetes.io/managed-by: stellar-k8s-integration-test
spec:
  nodeType: SorobanRpc
  network: Testnet
  version: "v0.0.5"
  storage:
    storageClass: standard
    size: 20Gi
    retentionPolicy: Delete
"#
    )
}

// ---------------------------------------------------------------------------
// Kubernetes API object fixtures
// ---------------------------------------------------------------------------

/// A minimal init container with a name, image, and command.
pub fn init_container(name: &str, image: &str, command: Vec<&str>) -> Container {
    Container {
        name: name.to_string(),
        image: Some(image.to_string()),
        command: Some(command.into_iter().map(String::from).collect()),
        ..Default::default()
    }
}

/// An init container that mounts a named volume at the given path.
pub fn init_container_with_volume(
    name: &str,
    image: &str,
    volume_name: &str,
    mount_path: &str,
) -> Container {
    Container {
        name: name.to_string(),
        image: Some(image.to_string()),
        volume_mounts: Some(vec![VolumeMount {
            name: volume_name.to_string(),
            mount_path: mount_path.to_string(),
            ..Default::default()
        }]),
        ..Default::default()
    }
}

/// A volume mount referencing the given volume at a path.
pub fn volume_mount(volume_name: &str, mount_path: &str) -> VolumeMount {
    VolumeMount {
        name: volume_name.to_string(),
        mount_path: mount_path.to_string(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Backup / rotation fixtures
// ---------------------------------------------------------------------------

/// Returns a `BackupVerificationConfig` with all fields at documented defaults.
///
/// Use this instead of `BackupVerificationConfig::default()` directly so tests
/// are isolated from any future changes to the `Default` impl.
pub fn backup_verification_defaults() -> stellar_k8s::backup::BackupVerificationConfig {
    stellar_k8s::backup::BackupVerificationConfig {
        enabled: false,
        schedule: "0 2 * * 0".to_string(),
        timeout_minutes: 60,
        benchmark_enabled: false,
        strategy: stellar_k8s::backup::VerificationStrategy::Standard,
        ..Default::default()
    }
}

/// Returns a `BackupVerificationConfig` configured for a quick CI run.
pub fn backup_verification_quick() -> stellar_k8s::backup::BackupVerificationConfig {
    stellar_k8s::backup::BackupVerificationConfig {
        enabled: true,
        schedule: "*/5 * * * *".to_string(),
        timeout_minutes: 5,
        benchmark_enabled: false,
        strategy: stellar_k8s::backup::VerificationStrategy::Quick,
        ..Default::default()
    }
}

/// A `BackupSource::S3` pointing at a test bucket.
pub fn s3_backup_source() -> stellar_k8s::backup::BackupSource {
    stellar_k8s::backup::BackupSource::S3 {
        bucket: "stellar-it-test-bucket".to_string(),
        region: "us-east-1".to_string(),
        prefix: "integration-tests/".to_string(),
        credentials_secret: "aws-test-creds".to_string(),
    }
}

/// A `BackupSource::VolumeSnapshot` referencing a test snapshot.
pub fn volume_snapshot_backup_source() -> stellar_k8s::backup::BackupSource {
    stellar_k8s::backup::BackupSource::VolumeSnapshot {
        snapshot_name: "stellar-it-snapshot".to_string(),
        storage_class: "standard".to_string(),
    }
}

/// Returns a `SecretRotationConfig` with all fields at documented defaults.
pub fn secret_rotation_defaults() -> stellar_k8s::backup::SecretRotationConfig {
    stellar_k8s::backup::SecretRotationConfig {
        enabled: false,
        schedule: "0 0 1 * *".to_string(),
        password_length: 32,
        db_timeout_seconds: 30,
        max_retries: 3,
        audit_logging_enabled: false,
        audit_log_destination: None,
        notification_webhook: None,
    }
}

/// Returns a `SecretRotationConfig` with all features enabled, suitable for
/// testing the serialisation round-trip.
pub fn secret_rotation_full() -> stellar_k8s::backup::SecretRotationConfig {
    stellar_k8s::backup::SecretRotationConfig {
        enabled: true,
        schedule: "0 0 1 * *".to_string(),
        password_length: 40,
        db_timeout_seconds: 60,
        max_retries: 5,
        audit_logging_enabled: true,
        audit_log_destination: Some("https://audit.example.com".to_string()),
        notification_webhook: Some("https://webhook.example.com".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_rng_deterministic() {
        let mut a = SeededRng::seeded(42);
        let mut b = SeededRng::seeded(42);
        let va: u64 = rand::Rng::gen(a.inner());
        let vb: u64 = rand::Rng::gen(b.inner());
        assert_eq!(va, vb);
    }

    #[test]
    fn seeded_rng_different_seeds() {
        let mut a = SeededRng::seeded(1);
        let mut b = SeededRng::seeded(2);
        let va: u64 = rand::Rng::gen(a.inner());
        let vb: u64 = rand::Rng::gen(b.inner());
        assert_ne!(va, vb);
    }

    #[test]
    fn fixed_now_is_deterministic() {
        let t1 = fixed_now();
        let t2 = fixed_now();
        assert_eq!(t1, t2);
        assert_eq!(t1.to_rfc3339(), "2026-01-15T12:00:00+00:00");
    }

    #[test]
    fn test_namespace_format() {
        assert_eq!(test_namespace("mytest"), "test-mytest");
    }

    #[test]
    fn test_node_name_format() {
        assert_eq!(test_node_name("alpha"), "node-alpha");
    }
}
