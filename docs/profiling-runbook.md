# Performance Profiling Runbook

This runbook guides operators through capturing and analysing CPU and memory
profiles from a live Stellar-K8s deployment.

## Overview

Stellar-K8s exposes pprof-compatible profiling endpoints on a dedicated
localhost-only server (`127.0.0.1:6060` by default). All endpoints are gated
behind a `X-Profiling-Token` header that must match a pre-configured secret.

## Prerequisites

- `kubectl` configured for your cluster
- `jq` for JSON pretty-printing
- `flamegraph` (optional, for flamegraph visualisation)
- Operator pod name (e.g. `stellar-operator-abc123`)

## Enabling Profiling

Profiling is **disabled by default**. Enable it by setting the following in
your `values.yaml` before deploying:

```yaml
profiling:
  enabled: true
  # SHA-256 hex digest of your chosen token. Generate with:
  # echo -n "YOUR-SECRET" | sha256sum
  tokenSha256: "4a3d0c7f6e9c6d6c2c2bc5ad97e9dbcf1e1b5e0e6c8e8d4e3a3c9b7f2d1e0a5f"
  bindAddr: "127.0.0.1:6060"
```

Store the raw token in a Kubernetes Secret:

```bash
kubectl create secret generic stellar-profiling-token \
  --from-literal=token=YOUR-SECRET \
  -n stellar-system
```

## Endpoints Reference

| Endpoint | Method | Description |
|---|---|---|
| `/debug/pprof/profile?duration=30` | GET | CPU profile (seconds) |
| `/debug/pprof/heap` | GET | Heap/memory snapshot |
| `/debug/pprof/goroutine` | GET | Active async task trace |
| `/debug/pprof/cmdline` | GET | Process command line |
| `/debug/pprof/analysis` | GET | Automated bottleneck report |
| `/metrics` | GET | Prometheus profiling metrics |

All endpoints accept `?format=json` (default).

## Capturing a CPU Profile

### Via kubectl port-forward

```bash
# Step 1: Forward the profiling port
kubectl port-forward -n stellar-system pod/stellar-operator-abc123 6060:6060 &

# Step 2: Retrieve token from Secret
TOKEN=$(kubectl get secret stellar-profiling-token -n stellar-system \
  -o jsonpath='{.data.token}' | base64 -d)

# Step 3: Capture a 30-second CPU profile
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/profile?duration=30" | jq .

# Step 4: Examine top frames
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/profile?duration=30" | \
  jq '.top_frames[] | "\(.pct | floor)% \(.symbol)"'
```

Expected output:
```
"42% tokio::runtime::park"
"18% stellar_k8s::controller::reconciler"
"8% stellar_k8s::rest_api::handlers"
```

## Capturing a Heap Profile

```bash
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/heap" | jq .

# Examine top allocation sites
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/heap" | \
  jq '.top_allocations[] | "\(.pct | floor)% \(.site) (\(.bytes / 1048576 | floor) MB)"'
```

## Automated Bottleneck Analysis

After capturing several profiles, run the automated analyser:

```bash
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/analysis" | jq .
```

The response includes:
- **bottlenecks**: Identified issues ordered by severity (critical → warning → info)
- **top_symbols**: CPU hotspots ranked by sample count
- **rss_growing**: Boolean indicating a potential memory leak
- **summary**: Human-readable summary suitable for incident reports

## Generating a Flamegraph

```bash
# Capture 60-second profile in JSON
curl -s -H "X-Profiling-Token: $TOKEN" \
  "http://localhost:6060/debug/pprof/profile?duration=60" > profile.json

# Convert to folded stack format and render with flamegraph
cat profile.json | jq -r '.payload.stack_counts | to_entries[] | "\(.key) \(.value)"' \
  | flamegraph.pl --title "Stellar Operator CPU Profile" > flamegraph.svg

# Open the SVG
open flamegraph.svg
```

## Interpreting Results

### High CPU Wall Time

**Symptom**: `high-cpu-wall-time` bottleneck; wall_time_ms > 1000.

**Likely causes**:
- Controller reconciliation loops blocked on slow Kubernetes API calls
- Expensive JSON serialisation under load
- Lock contention in shared state (RwLock write-heavy paths)

**Actions**:
1. Increase Kubernetes API client timeout and retry budget
2. Profile reconciler phases with `stellar-operator diff` to isolate the slow phase
3. Consider caching expensive list operations with an informer cache

### Memory Leak

**Symptom**: `rss-growth` critical; RSS increasing monotonically across samples.

**Likely causes**:
- Unbounded caches (LRU cache missing eviction policy)
- Retained async task handles that never resolve
- Accumulated Kubernetes watch event backlog

**Actions**:
1. Capture heap profiles at T+0 and T+30m and compare `allocation_sites`
2. Search for `Arc::clone` sites that outlive their expected scope
3. Add bounds to any unbounded `Vec` or `HashMap` accumulating over time

### Hot Function

**Symptom**: `hot-function` critical; single symbol >50% of CPU samples.

**Actions**:
1. Identify the function from `top_symbols[0].symbol`
2. Add instrumentation (`tracing::instrument`) to sub-calls
3. Consider memoisation if the function is pure and called frequently

## Prometheus Alerts for Profiling Health

The profiling subsystem exports these metrics to Prometheus:

```promql
# Auth failure spike (potential brute-force)
increase(stellar_profiling_auth_failures_total[5m]) > 10

# RSS growing faster than 50 MB/min
deriv(stellar_profiling_rss_bytes[5m]) > 50 * 1024 * 1024
```

## Security Considerations

- The profiling server **must never** be exposed outside the pod/localhost
  without network-level controls (e.g. Kubernetes NetworkPolicy)
- Rotate the profiling token quarterly or after any suspected credential exposure
- Profiling adds ~1-2% CPU overhead; disable between investigations
- Profile data may contain sensitive information (memory addresses, internal
  state strings) — treat captures as confidential artefacts

## Disabling Profiling

Set `profiling.enabled: false` in `values.yaml` and roll out. The
`127.0.0.1:6060` port will stop listening immediately on pod restart.
