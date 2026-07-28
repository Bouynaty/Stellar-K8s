# CI Pipeline Architecture & Reliability Guide

## Overview

This document describes the hardened CI/CD pipeline architecture with reliability
improvements addressing stability, security, and production correctness issues.

**Recent Updates (2026-07-28):** Major stability and security hardening wave
focused on eliminating CI failures, standardizing configurations, and improving
error handling.

---

## Pipeline Reliability Improvements (2026-07-28)

### Critical Fixes Applied

#### ✅ Docker Base Image Security
- **Fixed invalid digest**: Updated `debian:bookworm-slim` with valid SHA256 digest
- **Supply chain security**: Ensures reproducible, verified base images
- **Impact**: Eliminates "invalid digest format" build failures

#### ✅ Security Audit Hardening  
- **Centralized configuration**: Uses `.cargo/audit.toml` instead of inline CLI ignores
- **Documented ignores**: Each CVE ignore includes justification and review date
- **Removed stale entries**: Eliminated non-existent year-2026 RUSTSEC IDs
- **Audit trail**: Clear tracking of security decisions

#### ✅ Action Version Consistency
- **Standardized versions**: All workflows use consistent action versions
- **Updated versions**:
  - `actions/setup-python`: `@v6` (was inconsistent `@v5`/`@v6`)
  - `aquasecurity/trivy-action`: `@v0.36.0` (was mixed `@v0.35.0`/`@v0.36.0`)
- **Prevents**: Breaking changes from automatic minor version updates

#### ✅ Rust Cache Optimization
- **Removed deprecated setting**: `cache-all-crates: true` → `cache-directories`
- **Explicit cache paths**: Prevents cache thrashing between jobs  
- **Save optimization**: Only saves cache on main branch pushes
- **Performance**: Faster cache restoration, reduced cache size

#### ✅ Enhanced Error Handling
- **Retry logic**: Robust handling of network failures for tool installation
- **Exponential backoff**: Built-in retry patterns for transient failures
- **Timeout management**: Proper timeouts prevent hanging jobs

---

## Shared Composite Actions

All reusable logic lives under `.github/actions/`:

| Action | Purpose | Reliability Features |
|--------|---------|---------------------|
| `setup-rust` | Install Rust toolchain + system deps + cache | ✅ Optimized cache config, retry logic |
| `setup-kind-cluster` | Provision kind cluster, load image, install CRDs | ✅ Timeout handling, error recovery |
| `collect-e2e-logs` | Dump logs → artifact | ✅ Guaranteed artifact collection |
| `setup-perf-env` | Install k6/kind/kubectl, deploy operator | ✅ Dependency verification |
| `security-scan` | Run Trivy security scanning | ✅ Updated to v0.36.0 |

---

## Core CI Workflows (#700)

### `ci.yml`
- **Change detection** gates expensive jobs (helm-lint, api-docs, examples-smoke-test,
  security-audit) so they only run when relevant files change.
- **Unified Rust cache** via `setup-rust` composite action with per-job `shared-key`.
- **Removed duplicate** system-dependency install blocks (now in `setup-rust`).
- **Removed duplicate** `actions/checkout@v6` references (standardised on `@v4`).
- `lint` and `security-audit` run in **parallel** (both depend only on `changes`).
- `test` runs on every PR; `coverage` runs on **main pushes only** (tarpaulin is slow).
- Removed standalone `pre-commit.yml` and `commit-lint.yml` workflows — lint/format
  is covered by the main `ci.yml` `lint` job.

### Estimated time reduction
Parallel lint + audit + test/coverage, combined with shared caching, reduces
the critical path by ~35–40% compared to the previous sequential layout.

---

## Heavy Validation Workflows (#703)

### `chaos-tests.yml`
- **Extracted** cluster provisioning into `setup-kind-cluster` composite action.
- **Parallel execution**: experiments 01–02 (pod-kill, network partition) run in
  `chaos-kill-network` job; experiments 03–05 (latency, peer-partition, disk-fill)
  run in `chaos-latency-disk` job simultaneously.
- **Consolidated logging** via `collect-e2e-logs` composite action.
- Binary built once in a `build` job and downloaded as an artifact by both
  parallel jobs — no duplicate Rust compilation.

### `soak-test.yml`
- Uses `setup-kind-cluster` for cluster provisioning.
- Uses `collect-e2e-logs` for failure-time log collection.
- Removed duplicated Rust toolchain + apt-get blocks.

### `verify-operator-boot.yml`
- Uses `setup-rust` composite action.
- Runs on **main pushes** and `workflow_dispatch` only (kind-cluster boot check is
  too heavy for every contributor PR).
- Artifact name includes `github.run_id` to avoid collisions.

---

## Performance & Benchmark Workflows (#701)

### `performance.yml` (unified pipeline)
- **Replaces** the former `benchmark.yml`, `performance-regression.yml`, and
  `webhook-benchmark.yml` with a single matrix-driven workflow.
- Runs on **main pushes** (path-filtered) and `workflow_dispatch` — not on PRs.
- **Shared build job** produces the operator binary and Docker image once; all
  three suites (operator, regression, webhook) download the same artifact.
- **Matrix execution** runs operator and regression suites via `setup-perf-env`,
  and the webhook suite directly (no kind cluster required).
- **Shared baseline comparison** via `.github/actions/compare-benchmarks`
  composite action wrapping `compare_benchmarks.py`.

---

## Release & Multi-Arch Workflows (#665)

### `multiarch-build.yml`
- Runs on **main pushes** (path-filtered) and `workflow_dispatch` — not on PRs.
- Per-platform GHA cache scopes (`multiarch-amd64`, `multiarch-arm64`) prevent
  cross-arch cache pollution and improve cache hit rates.
- `arch-benchmark` jobs use `setup-rust` composite action.
- Combined manifest build pulls from both per-platform caches.

### `release.yml`
- **Eliminated duplicate Docker build**: `container` job first attempts to
  re-tag the `sha-<sha>` image already published by `multiarch-build.yml`.
  A fresh build only runs as a fallback when the sha image is unavailable.
- **Fail-safe**: `validate` job enforces semver format AND Cargo.toml version
  match before any build or publish step runs. A mismatch is now a hard error
  (previously a warning).
- `release` job depends on ALL of: `build-artifacts`, `container`, `security`,
  `helm` — broken builds can never be tagged for release.
- Standardised on `actions/upload-artifact@v4` / `actions/download-artifact@v4`.

---

## Action Version Standardisation & Security

All workflows now use consistent, security-hardened action versions:

| Action | Version | Security Notes |
|--------|---------|----------------|
| `actions/checkout` | `@v7` | Latest with security patches |
| `actions/setup-node` | `@v4` | Stable, consistent |
| `actions/setup-python` | `@v6` | **Fixed inconsistency** (was mixed v5/v6) |
| `actions/upload-artifact` | `@v4` | Consistent across all workflows |
| `actions/download-artifact` | `@v4` | Consistent across all workflows |
| `actions/cache` | `@v4` | Stable caching |
| `helm/kind-action` | `v1.14.0` | Pinned for stability |
| `docker/build-push-action` | `@v7` | Latest with security improvements |
| `aquasecurity/trivy-action` | `@v0.36.0` | **Fixed inconsistency** (was mixed v0.35.0/v0.36.0) |
| `Swatinem/rust-cache` | `@v2` | **Optimized configuration** |

### Security Hardening Applied

#### Docker Image Security
- **Valid base image digest**: Fixed dummy SHA256 → actual `debian:bookworm-slim` digest
- **Supply chain verification**: Ensures reproducible, verified builds
- **SBOM generation**: Enabled for all release artifacts
- **Provenance attestation**: Cryptographic build provenance for containers

#### Dependency Security
- **Centralized audit config**: Moved from inline CLI ignores to documented `.cargo/audit.toml`
- **Justified ignores**: Each security advisory ignore includes:
  - Technical rationale for why it's safe to ignore
  - Conditions for removal
  - Review date for re-evaluation
- **Eliminated phantom entries**: Removed non-existent future-year RUSTSEC IDs

---

## Reliability Testing & Monitoring

### New CI Reliability Test (`ci-reliability-test.yml`)
Validates pipeline stability and hardening:

- ✅ **Docker config validation**: Verifies base image digests are valid
- ✅ **Security audit testing**: Confirms audit configuration is functional  
- ✅ **Action version consistency**: Detects version drift across workflows
- ✅ **Cache configuration**: Validates deprecated settings are removed
- ✅ **Retry logic testing**: Confirms error handling patterns exist
- ✅ **Documentation completeness**: Ensures troubleshooting guides exist

### Troubleshooting Documentation
New comprehensive guide: `.github/CI_TROUBLESHOOTING.md`

**Covers common failure scenarios:**
- Docker build failures and digest issues
- Security audit failures and ignore management  
- Test timeouts and performance regressions
- Cache restoration problems
- Action version conflicts

**Includes local reproduction steps:**
```bash
# Reproduce CI failures locally
docker build --target runtime --platform linux/amd64 .
cargo test --all-features --workspace
cargo audit  # Uses .cargo/audit.toml config
```

---

## Monitoring & Success Metrics

### Target Reliability Metrics
- **Success rate**: >95% on main branch
- **Build duration**: <45 minutes end-to-end
- **Cache hit rate**: >80% for Rust builds
- **Security audit**: 0 unaddressed critical/high CVEs

### Alert Conditions
- 3+ consecutive main branch failures
- Individual job runtime >60 minutes  
- Cache hit rate <60% (indicates configuration issues)
- New high/critical CVEs not addressed within 7 days
