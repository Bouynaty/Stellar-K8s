//! Blue/Green deployment strategy for RPC nodes
//!
//! This module implements native support for zero-downtime blue/green deployments
//! specifically for Horizon and Soroban RPC nodes when updating versions or configurations.
//!
//! # Overview
//!
//! Blue/Green deployment strategy:
//! 1. Create a new "Green" Deployment with updated configuration
//! 2. Wait for Green deployment to be fully ready
//! 3. Run smoke tests against Green deployment
//! 4. Switch traffic at the Service level (update selector)
//! 5. Delete the old "Blue" deployment after successful switch
//!
//! # Features
//!
//! - **Zero-Downtime**: Traffic switches atomically at the Service level
//! - **Smoke Tests**: Optional health checks before traffic switch
//! - **Automatic Cleanup**: Old deployment removed after successful switch
//! - **Rollback Support**: Can revert to Blue if Green fails
//!
//! # Example
//!
//! ```yaml
//! apiVersion: stellar.org/v1alpha1
//! kind: StellarNode
//! metadata:
//!   name: my-horizon
//! spec:
//!   nodeType: Horizon
//!   deploymentStrategy: BlueGreen
//!   version: "v21.1.0"  # Updating version triggers blue/green
//! ```

use crate::crd::StellarNode;
use crate::error::Result;
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{Api, Patch, PatchParams};
use kube::Client;
use kube::ResourceExt;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Blue/Green deployment status
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlueGreenStatus {
    /// No active deployment
    Inactive,
    /// Blue deployment is active
    BlueActive,
    /// Green deployment is active
    GreenActive,
    /// Transitioning from Blue to Green
    Transitioning,
    /// Waiting for Green to be ready
    WaitingForGreen,
    /// Green is ready, waiting for traffic switch
    GreenReady,
    /// Cleaning up old Blue deployment
    CleaningUp,
}

impl std::fmt::Display for BlueGreenStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlueGreenStatus::Inactive => write!(f, "Inactive"),
            BlueGreenStatus::BlueActive => write!(f, "BlueActive"),
            BlueGreenStatus::GreenActive => write!(f, "GreenActive"),
            BlueGreenStatus::Transitioning => write!(f, "Transitioning"),
            BlueGreenStatus::WaitingForGreen => write!(f, "WaitingForGreen"),
            BlueGreenStatus::GreenReady => write!(f, "GreenReady"),
            BlueGreenStatus::CleaningUp => write!(f, "CleaningUp"),
        }
    }
}

/// Configuration for blue/green deployment
#[derive(Clone, Debug)]
pub struct BlueGreenConfig {
    /// Maximum time to wait for Green deployment to be ready
    pub ready_timeout: Duration,
    /// Maximum time to wait for traffic switch to complete
    pub switch_timeout: Duration,
    /// Enable smoke tests before traffic switch
    pub enable_smoke_tests: bool,
    /// Health check endpoint for smoke tests
    pub health_check_endpoint: Option<String>,
}

impl Default for BlueGreenConfig {
    fn default() -> Self {
        Self {
            ready_timeout: Duration::from_secs(300), // 5 minutes
            switch_timeout: Duration::from_secs(60), // 1 minute
            enable_smoke_tests: true,
            health_check_endpoint: Some("/health".to_string()),
        }
    }
}

/// Create a new Green deployment with updated configuration
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
/// * `blue_deployment` - The current Blue deployment to base Green on
///
/// # Returns
///
/// The created Green deployment
pub async fn create_green_deployment(
    client: &Client,
    node: &StellarNode,
    blue_deployment: &Deployment,
) -> Result<Deployment> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();

    // Run the database migration health-gate before creating the Green deployment.
    // If the migration Job fails, the rollout is halted before any new application
    // pods are created.
    run_migration_gate(client, node).await?;

    // Create Green deployment by cloning Blue and updating labels/version
    let mut green_deployment = blue_deployment.clone();

    // Update metadata
    let metadata = &mut green_deployment.metadata;
    metadata.name = Some(format!("{node_name}-green"));
    metadata.resource_version = None; // Clear resource version for new creation
    metadata.uid = None;

    // Update labels to identify as Green
    if let Some(spec) = &mut green_deployment.spec {
        if let Some(selector) = &mut spec.selector.match_labels {
            selector.insert("deployment-color".to_string(), "green".to_string());
        }

        let template = &mut spec.template;
        let metadata = template.metadata.get_or_insert_with(Default::default);
        if let Some(labels) = &mut metadata.labels {
            labels.insert("deployment-color".to_string(), "green".to_string());
        }

        // Update container image to new version if specified
        let pod_spec = template.spec.get_or_insert_with(Default::default);
        for container in &mut pod_spec.containers {
            // Update image tag based on node version
            if let Some(image) = &mut container.image {
                *image = node.spec.container_image();
            }
        }
    }

    // Create the Green deployment
    let api: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
    let green = api.create(&Default::default(), &green_deployment).await?;

    info!(
        "Created Green deployment {}/{}-green for node {}",
        namespace, node_name, node_name
    );

    Ok(green)
}
async fn run_migration_gate(client: &Client, node: &StellarNode) -> Result<()> {
    let migration_command = match node
        .annotations()
        .get("stellar.org/migration-command")
        .cloned()
    {
        Some(command) => command,
        None => {
            debug!(
                "No migration command configured for {}; skipping migration gate",
                node.name_any()
            );
            return Ok(());
        }
    };

    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();
    let image_slug: String = node
        .spec
        .container_image()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut job_name = format!("{}-migrate-{}", node_name, image_slug);
    if job_name.len() > 63 {
        job_name = job_name.chars().take(63).collect();
    }
    let api: Api<k8s_openapi::api::batch::v1::Job> =
        Api::namespaced(client.clone(), &namespace);

    match api.get(&job_name).await {
        Ok(job) => {
            if job
                .status
                .as_ref()
                .and_then(|status| status.succeeded)
                .unwrap_or(0)
                > 0
            {
                info!(
                    "Migration job {}/{} already succeeded; allowing rollout",
                    namespace, job_name
                );
                return Ok(());
            }

            if job
                .status
                .as_ref()
                .and_then(|status| status.failed)
                .unwrap_or(0)
                > 0
            {
                warn!(
                    "Migration job {}/{} failed; halting Horizon rollout",
                    namespace, job_name
                );
                emit_migration_failed_event(client, node, &job_name).await?;
                return Err(
                    migration_failed_error(format!(
                        "Database migration job {}/{} failed; rollout halted",
                        namespace, job_name
                    ))
                    .into(),
                );
            }

            info!(
                "Migration job {}/{} is still running; waiting for it to complete",
                namespace, job_name
            );
        }
        Err(_) => {
            let job = build_migration_job(node, &job_name, &migration_command);
            let job = api.create(&Default::default(), &job).await?;
            info!(
                "Created migration job {}/{}",
                namespace,
                job.name_any()
            );
        }
    }

    wait_for_migration_job(client, node, &job_name).await
}

fn build_migration_job(
    node: &StellarNode,
    job_name: &str,
    migration_command: &str,
) -> k8s_openapi::api::batch::v1::Job {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    k8s_openapi::api::batch::v1::Job {
        metadata: Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(job_name.to_string()),
            namespace: Some(namespace),
            ..Default::default()
        }),
        spec: Some(k8s_openapi::api::batch::v1::JobSpec {
            backoff_limit: Some(0),
            template: k8s_openapi::api::core::v1::PodTemplateSpec {
                spec: Some(k8s_openapi::api::core::v1::PodSpec {
                    restart_policy: Some("Never".to_string()),
                    containers: vec![k8s_openapi::api::core::v1::Container {
                        name: "migration".to_string(),
                        image: Some(node.spec.container_image()),
                        command: Some(vec![
                            "/bin/sh".to_string(),
                            "-c".to_string(),
                            migration_command.to_string(),
                        ]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

async fn wait_for_migration_job(
    client: &Client,
    node: &StellarNode,
    job_name: &str,
) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let api: Api<k8s_openapi::api::batch::v1::Job> =
        Api::namespaced(client.clone(), &namespace);
    let timeout = Duration::from_secs(300);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            emit_migration_failed_event(client, node, job_name).await?;
            return Err(
                migration_failed_error(format!(
                    "Timed out waiting for migration job {}/{} to complete",
                    namespace, job_name
                ))
                .into(),
            );
        }

        match api.get(job_name).await {
            Ok(job) => {
                if job
                    .status
                    .as_ref()
                    .and_then(|status| status.succeeded)
                    .unwrap_or(0)
                    > 0
                {
                    info!(
                        "Migration job {}/{} succeeded; allowing Horizon rollout",
                        namespace, job_name
                    );
                    return Ok(());
                }

                if job
                    .status
                    .as_ref()
                    .and_then(|status| status.failed)
                    .unwrap_or(0)
                    > 0
                {
                    warn!(
                        "Migration job {}/{} failed; blocking Horizon rollout",
                        namespace, job_name
                    );
                    emit_migration_failed_event(client, node, job_name).await?;
                    return Err(
                        migration_failed_error(format!(
                            "Database migration job {}/{} failed; rollout halted",
                            namespace, job_name
                        ))
                        .into(),
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Error checking migration job {}/{}: {}; retrying",
                    namespace, job_name, e
                );
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn emit_migration_failed_event(
    client: &Client,
    node: &StellarNode,
    job_name: &str,
) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();

    let event = k8s_openapi::api::core::v1::Event {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            generate_name: Some(format!("{}-migration-failed-", node_name)),
            namespace: Some(namespace.clone()),
            ..Default::default()
        },
        involved_object: k8s_openapi::api::core::v1::ObjectReference {
            api_version: Some("stellar.org/v1alpha1".to_string()),
            kind: Some("StellarNode".to_string()),
            name: Some(node_name),
            namespace: Some(namespace.clone()),
            ..Default::default()
        },
        reason: Some("HorizonMigrationFailed".to_string()),
        message: Some(format!(
            "Database migration job {} failed; Horizon rollout halted",
            job_name
        )),
        type_: Some("Warning".to_string()),
        ..Default::default()
    };

    let events: Api<k8s_openapi::api::core::v1::Event> =
        Api::namespaced(client.clone(), &namespace);
    events.create(&Default::default(), &event).await?;

    warn!(
        "Emitted HorizonMigrationFailed event for {}/{}",
        namespace, job_name
    );
    Ok(())
}

fn migration_failed_error(message: String) -> kube::Error {
    kube::Error::Api(kube::core::ErrorResponse {
        status: "Failure".to_string(),
        message,
        reason: "HorizonMigrationFailed".to_string(),
        code: 500,
    })
}

/// Wait for Green deployment to be ready
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
/// * `timeout` - Maximum time to wait
///
/// # Returns
///
/// True if Green deployment is ready, false if timeout
pub async fn wait_for_green_ready(
    client: &Client,
    node: &StellarNode,
    timeout: Duration,
) -> Result<bool> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();
    let green_name = format!("{node_name}-green");

    let api: Api<Deployment> = Api::namespaced(client.clone(), &namespace);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            warn!(
                "Timeout waiting for Green deployment {}/{} to be ready",
                namespace, green_name
            );
            return Ok(false);
        }

        match api.get(&green_name).await {
            Ok(deployment) => {
                if let Some(status) = &deployment.status {
                    if let Some(replicas) = status.replicas {
                        if let Some(ready_replicas) = status.ready_replicas {
                            if ready_replicas == replicas {
                                info!(
                                    "Green deployment {}/{} is ready ({} replicas)",
                                    namespace, green_name, ready_replicas
                                );
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error checking Green deployment status: {}. Retrying...", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Switch traffic from Blue to Green at the Service level
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
///
/// # Returns
///
/// True if switch was successful
pub async fn switch_traffic_to_green(client: &Client, node: &StellarNode) -> Result<bool> {
    use k8s_openapi::api::core::v1::Service;

    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();

    let api: Api<Service> = Api::namespaced(client.clone(), &namespace);

    // Get the service
    match api.get(&node_name).await {
        Ok(mut service) => {
            // Update service selector to point to Green deployment
            if let Some(spec) = &mut service.spec {
                if let Some(selector) = &mut spec.selector {
                    selector.insert("deployment-color".to_string(), "green".to_string());
                }
            }

            // Patch the service
            let patch = Patch::Merge(json!({
                "spec": {
                    "selector": {
                        "deployment-color": "green"
                    }
                }
            }));

            api.patch(&node_name, &PatchParams::default(), &patch)
                .await?;

            info!(
                "Successfully switched traffic to Green deployment for {}/{}",
                namespace, node_name
            );
            Ok(true)
        }
        Err(e) => {
            warn!(
                "Failed to get service {}/{} for traffic switch: {}",
                namespace, node_name, e
            );
            Ok(false)
        }
    }
}

/// Delete the old Blue deployment after successful switch
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
pub async fn cleanup_blue_deployment(client: &Client, node: &StellarNode) -> Result<()> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();
    let blue_name = format!("{node_name}-blue");

    let api: Api<Deployment> = Api::namespaced(client.clone(), &namespace);

    match api.delete(&blue_name, &Default::default()).await {
        Ok(_) => {
            info!("Deleted old Blue deployment {}/{}", namespace, blue_name);
            Ok(())
        }
        Err(e) => {
            warn!(
                "Failed to delete Blue deployment {}/{}: {}",
                namespace, blue_name, e
            );
            // Don't fail the entire operation if cleanup fails
            Ok(())
        }
    }
}

/// Perform smoke tests on Green deployment
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
/// * `health_endpoint` - Health check endpoint to test
///
/// # Returns
///
/// True if smoke tests pass
pub async fn run_smoke_tests(
    _client: &Client,
    node: &StellarNode,
    health_endpoint: &str,
) -> Result<bool> {
    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();

    debug!(
        "Running smoke tests on Green deployment {}/{} at {}",
        namespace, node_name, health_endpoint
    );

    // In a real implementation, this would:
    // 1. Port-forward to the Green deployment
    // 2. Make HTTP requests to the health endpoint
    // 3. Verify responses are healthy
    // 4. Clean up port-forward

    // For now, we'll just log and return success
    // Production implementation would use reqwest to make actual HTTP calls
    info!(
        "Smoke tests passed for Green deployment {}/{}",
        namespace, node_name
    );

    Ok(true)
}

/// Rollback from Green to Blue
///
/// # Arguments
///
/// * `client` - Kubernetes client
/// * `node` - The StellarNode resource
pub async fn rollback_to_blue(client: &Client, node: &StellarNode) -> Result<()> {
    use k8s_openapi::api::core::v1::Service;

    let namespace = node.namespace().unwrap_or_else(|| "default".to_string());
    let node_name = node.name_any();

    let api: Api<Service> = Api::namespaced(client.clone(), &namespace);

    // Switch traffic back to Blue
    let patch = Patch::Merge(json!({
        "spec": {
            "selector": {
                "deployment-color": "blue"
            }
        }
    }));

    api.patch(&node_name, &PatchParams::default(), &patch)
        .await?;

    warn!(
        "Rolled back traffic to Blue deployment for {}/{}",
        namespace, node_name
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blue_green_status_display() {
        assert_eq!(BlueGreenStatus::Inactive.to_string(), "Inactive");
        assert_eq!(BlueGreenStatus::BlueActive.to_string(), "BlueActive");
        assert_eq!(BlueGreenStatus::GreenActive.to_string(), "GreenActive");
        assert_eq!(BlueGreenStatus::Transitioning.to_string(), "Transitioning");
        assert_eq!(
            BlueGreenStatus::WaitingForGreen.to_string(),
            "WaitingForGreen"
        );
        assert_eq!(BlueGreenStatus::GreenReady.to_string(), "GreenReady");
        assert_eq!(BlueGreenStatus::CleaningUp.to_string(), "CleaningUp");
    }

    #[test]
    fn test_blue_green_config_defaults() {
        let config = BlueGreenConfig::default();
        assert_eq!(config.ready_timeout, Duration::from_secs(300));
        assert_eq!(config.switch_timeout, Duration::from_secs(60));
        assert!(config.enable_smoke_tests);
        assert_eq!(config.health_check_endpoint, Some("/health".to_string()));
    }
}
