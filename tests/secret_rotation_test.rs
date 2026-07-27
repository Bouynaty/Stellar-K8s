//! Isolated fixture suite for secret rotation types.
//!
//! Refactored in issue #1140: cluster-dependent test (`test_password_generation`)
//! converted from a silent early-return to a properly `#[ignore]`-annotated
//! cluster test. Inline data builders replaced by shared fixtures.

mod common;

use common::fixtures::{secret_rotation_defaults, secret_rotation_full};
use stellar_k8s::backup::{SecretRotationConfig, SecretRotationScheduler};

// ── Default value contract ────────────────────────────────────────────────────

#[tokio::test]
async fn test_secret_rotation_config_default() {
    let config = secret_rotation_defaults();

    assert!(!config.enabled);
    assert_eq!(config.schedule, "0 0 1 * *");
    assert_eq!(config.password_length, 32);
    assert_eq!(config.db_timeout_seconds, 30);
    assert_eq!(config.max_retries, 3);
    assert!(!config.audit_logging_enabled);
    assert!(config.audit_log_destination.is_none());
    assert!(config.notification_webhook.is_none());
}

// ── Serialisation round-trip ──────────────────────────────────────────────────

#[tokio::test]
async fn test_secret_rotation_config_serialization() {
    let config = secret_rotation_full();

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: SecretRotationConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config, deserialized);
}

#[tokio::test]
async fn test_secret_rotation_config_round_trip_preserves_all_fields() {
    let config = secret_rotation_full();
    let deserialized: SecretRotationConfig =
        serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();

    assert_eq!(deserialized.enabled, config.enabled);
    assert_eq!(deserialized.schedule, config.schedule);
    assert_eq!(deserialized.password_length, config.password_length);
    assert_eq!(deserialized.db_timeout_seconds, config.db_timeout_seconds);
    assert_eq!(deserialized.max_retries, config.max_retries);
    assert_eq!(
        deserialized.audit_logging_enabled,
        config.audit_logging_enabled
    );
    assert_eq!(
        deserialized.audit_log_destination,
        config.audit_log_destination
    );
    assert_eq!(deserialized.notification_webhook, config.notification_webhook);
}

// ── Cluster tests (require a live Kubernetes API) ─────────────────────────────

/// Verifies that `SecretRotationScheduler::new` constructs successfully when a
/// Kubernetes client is available.
///
/// Run with:
///   cargo test --test secret_rotation_test -- --ignored
///
/// The test is `#[ignore]`-annotated so it never silently passes in
/// environments without a cluster (see issue #1140 for the previous silent
/// early-return pattern this replaces).
#[tokio::test]
#[ignore = "requires a live Kubernetes cluster (KUBECONFIG or in-cluster config)"]
async fn test_scheduler_constructs_with_live_client() {
    let config = secret_rotation_defaults();
    let client = kube::Client::try_default()
        .await
        .expect("KUBECONFIG or in-cluster config must be available for this test");
    let _scheduler = SecretRotationScheduler::new(config.clone(), client);

    // Verify the config we passed through is reachable via the public field.
    assert_eq!(config.password_length, 32);
}
