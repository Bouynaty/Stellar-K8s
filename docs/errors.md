# Stellar-K8s Error Codes

This document provides details on all error variants encountered in the Stellar-K8s operator, their causes, structured fields, and resolution steps.

| Error Code | Name | Description | Resolution Steps |
| --- | --- | --- | --- |
| **SK8S-001** | `KubeError(kube::Error)` | Kubernetes API error returned from `kube-rs`. | Check the Kubernetes cluster status and accessibility of the API server. Review RBAC permissions for the operator. |
| **SK8S-002** | `SerializationError(serde_json::Error)` | JSON serialization/deserialization failed. | Ensure custom resource definitions (CRDs) match operator schema and specs contain valid JSON/YAML syntax. |
| **SK8S-003** | `FinalizerError(String)` | A finalizer failed to execute during resource cleanup. | Examine operator deployment logs to identify the failing cleanup task (e.g., non-deletable associated resources). |
| **SK8S-004** | `ConfigError(String)` | Operator or resource configuration is invalid. | Review configuration for typos and validate fields against supported schema constraints and environment settings. |
| **SK8S-005** | `ValidationError(String)` | Node specification validation failed. | Inspect `StellarNode` CR fields against validation rules. Verify parameter compatibility and resource bounds. |
| **SK8S-006** | `NotFound { kind, name, namespace }` | The requested Kubernetes resource (`kind/name` in `namespace`) was not found. | Ensure the target resource exists in the specified namespace and the resource name is spelled correctly. |
| **SK8S-007** | `InvalidNodeType(String)` | An invalid or unrecognized node type was requested. | Validate `nodeType` in the spec. Allowed types must be recognized by this operator version (e.g., Validator, Horizon, SorobanRpc). |
| **SK8S-008** | `MissingRequiredField { field, node_type }` | Mandatory `field` for the specified `node_type` is missing. | Complete the node spec by providing all required parameters for the specified `nodeType` (e.g., `seedSecretRef` for Validators). |
| **SK8S-009** | `ArchiveHealthCheckError(String)` | History archive health check failed. | Verify history archive URL reachability, network connectivity, and storage endpoint status. |
| **SK8S-010** | `HttpError(reqwest::Error)` | HTTP request error during external/internal API calls. | Check network connectivity, DNS resolution, and NetworkPolicies for outbound traffic. |
| **SK8S-011** | `RemediationError(String)` | Automated remediation task failed during execution. | Inspect operator logs for the failed remediation sequence. Check RBAC permissions and target pod/node stability. |
| **SK8S-012** | `PluginError(String)` | Error during WASM admission plugin execution. | Verify WASM plugin compilation integrity, runtime configuration, and dependency availability. |
| **SK8S-013** | `WebhookError(String)` | Admission webhook server operational error. | Verify webhook TLS certificates, service endpoint routing, and pod readiness. |
| **SK8S-014** | `NetworkError(String)` | General network connectivity failure encountered. | Check cluster CNI plugin health, pod routing, and inter-node network stability. |
| **SK8S-015** | `CertificateError(rcgen::Error)` | Generating or parsing TLS certificate failed. | Inspect certificate configuration and CA key pairs. Verify cert-manager integration if applicable. |
| **SK8S-016** | `IoError(std::io::Error)` | File system input/output failure. | Check filesystem permissions, mount availability, and disk capacity for local caching paths. |
| **SK8S-017** | `MaintenanceError(String)` | Database maintenance or pruning task failed. | Check PostgreSQL status, disk space, and process locks on node database tables. |
| **SK8S-018** | `SqlxError(sqlx::Error)` | SQL database interaction error from SQLx driver. | Verify database connectivity, active connections, credentials, and schema migration state. |
| **SK8S-019** | `KubeconfigError(kube::config::KubeconfigError)` | Failed to load or parse local Kubeconfig file. | Verify `KUBECONFIG` environment variable path, file existence, and file permissions. |
| **SK8S-020** | `ZipError(zip::result::ZipError)` | Failure during compression or extraction of snapshots. | Verify snapshot archive integrity and ensure adequate disk space is available for extraction. |
| **SK8S-021** | `NetworkSafetyViolation(NetworkSafetyViolation)` | Cross-network safety policy violation (e.g. Mainnet and Testnet in same namespace). | Deploy nodes from different network types into separate Kubernetes namespaces to prevent ledger contamination. |
| **SK8S-022** | `InternalError(String)` | Unexpected internal state error. | Check operator logs for `[SK8S-022]` details and report unrecoverable internal errors. |

## Error Helper Functions & Behavior Semantics

The operator provides built-in helper functions and methods for structured diagnostic formatting, error construction, retry management, and status reporting:

### Diagnostic Formatting: `diagnostic(step, detail)`
Formats a user-facing diagnostic string by pairing an explicit pipeline execution step with error details:
`diagnostic("load kubeconfig", "file not found")` → `"[load kubeconfig] file not found"`

### Step-Aware Constructors
- `Error::config_step(step, detail)` — Constructs `Error::ConfigError` formatted via `diagnostic(step, detail)`.
- `Error::internal_step(step, detail)` — Constructs `Error::InternalError` formatted via `diagnostic(step, detail)`.
- `Error::validation_step(step, detail)` — Constructs `Error::ValidationError` formatted via `diagnostic(step, detail)`.

### Retry Semantics: `Error::is_retriable()`
Determines whether an error variant should trigger an automatic reconciliation retry. The following variants are classified as retriable:
- `Error::KubeError` — Transient cluster API server communication issues.
- `Error::FinalizerError` — Temporary resource cleanup impediments.
- `Error::RemediationError` — Transient auto-remediation failures.

Non-retriable variants (such as `ConfigError` or `ValidationError`) require manual user intervention or spec modifications.

### Status Reporting: `Error::status_message()`
Delegates directly to the `Display` implementation (`self.to_string()`), serving as a single source of truth for updating `StellarNode` custom resource status conditions.

## General Troubleshooting
When encountering these errors, the primary source of detailed insight will be the operator logs. You can fetch them with:
```bash
kubectl logs -n stellar-system deploy/stellar-operator
```
Look for the `[SK8S-XXX]` prefix in the logging output for rapid filtering.

---

*Last verified: 2026-07-29 (pipeline log redaction + rustfmt CI wave).*
