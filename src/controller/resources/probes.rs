//! Default Kubernetes liveness, readiness, and startup probes for each node type.
//!
//! Probe definitions live here so they can be shared between Deployment and
//! StatefulSet builders without duplication.

use k8s_openapi::api::core::v1::{ExecAction, HTTPGetAction, Probe, TCPSocketAction};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;

use crate::crd::{types::ProbeOverride, NodeType};

// ── Public entry-points ───────────────────────────────────────────────────────

/// Default liveness probe per node type.
///
/// - **Validator**: TCP socket on port 11625 (Stellar Core peer port).
/// - **Horizon / SorobanRpc**: HTTP GET `/health` on port 8000.
pub fn default_liveness_probe(node_type: &NodeType) -> Probe {
    match node_type {
        NodeType::Validator => Probe {
            tcp_socket: Some(TCPSocketAction {
                port: IntOrString::Int(11625),
                ..Default::default()
            }),
            initial_delay_seconds: Some(30),
            period_seconds: Some(15),
            timeout_seconds: Some(5),
            failure_threshold: Some(3),
            success_threshold: Some(1),
            ..Default::default()
        },
        _ => Probe {
            http_get: Some(HTTPGetAction {
                path: Some("/health".to_string()),
                port: IntOrString::Int(8000),
                ..Default::default()
            }),
            initial_delay_seconds: Some(20),
            period_seconds: Some(15),
            timeout_seconds: Some(5),
            failure_threshold: Some(3),
            success_threshold: Some(1),
            ..Default::default()
        },
    }
}

/// Default readiness probe per node type.
///
/// - **Validator**: exec probe that queries `/info` — marks the pod Not Ready
///   when the node is in `CATCHING_UP` or `SYNCING` state.  The pod remains
///   Not Ready until fully synced, preventing traffic routing to an unsynced
///   node.  The liveness probe (TCP socket) is intentionally separate so that
///   a syncing node is never restarted — only removed from the ready set.
/// - **Horizon / SorobanRpc**: HTTP GET `/health` on port 8000.
pub fn default_readiness_probe(node_type: &NodeType) -> Probe {
    match node_type {
        NodeType::Validator => {
            let script = concat!(
                "RESP=$(wget -qO- http://localhost:11626/info 2>/dev/null) && ",
                "echo \"$RESP\" | grep -qv '\"state\".*\"CATCHING_UP\"' && ",
                "echo \"$RESP\" | grep -qv '\"state\".*\"SYNCING\"'"
            );
            Probe {
                exec: Some(ExecAction {
                    command: Some(vec![
                        "/bin/sh".to_string(),
                        "-c".to_string(),
                        script.to_string(),
                    ]),
                }),
                initial_delay_seconds: Some(15),
                period_seconds: Some(10),
                timeout_seconds: Some(5),
                failure_threshold: Some(3),
                success_threshold: Some(1),
                ..Default::default()
            }
        }
        _ => Probe {
            http_get: Some(HTTPGetAction {
                path: Some("/health".to_string()),
                port: IntOrString::Int(8000),
                ..Default::default()
            }),
            initial_delay_seconds: Some(10),
            period_seconds: Some(10),
            timeout_seconds: Some(5),
            failure_threshold: Some(3),
            success_threshold: Some(1),
            ..Default::default()
        },
    }
}

/// Default startup probe — allows extra time for initial ledger sync.
///
/// 30 × 10s = 5 minutes max startup time for all node types.
pub fn default_startup_probe(node_type: &NodeType) -> Probe {
    match node_type {
        NodeType::Validator => Probe {
            tcp_socket: Some(TCPSocketAction {
                port: IntOrString::Int(11625),
                ..Default::default()
            }),
            initial_delay_seconds: Some(10),
            period_seconds: Some(10),
            timeout_seconds: Some(5),
            failure_threshold: Some(30),
            success_threshold: Some(1),
            ..Default::default()
        },
        _ => Probe {
            http_get: Some(HTTPGetAction {
                path: Some("/health".to_string()),
                port: IntOrString::Int(8000),
                ..Default::default()
            }),
            initial_delay_seconds: Some(10),
            period_seconds: Some(10),
            timeout_seconds: Some(5),
            failure_threshold: Some(30),
            success_threshold: Some(1),
            ..Default::default()
        },
    }
}

// ── Probe override application ─────────────────────────────────────────────────

/// Apply a [`ProbeOverride`] on top of an optional base probe (public wrapper).
pub fn apply_probe_override_pub(
    base: Option<Probe>,
    override_cfg: Option<&ProbeOverride>,
) -> Option<Probe> {
    apply_probe_override(base, override_cfg)
}

pub(crate) fn apply_probe_override(
    base: Option<Probe>,
    override_cfg: Option<&ProbeOverride>,
) -> Option<Probe> {
    let cfg = match override_cfg {
        Some(c) => c,
        None => return base,
    };
    let mut probe = base.unwrap_or_default();
    if let Some(v) = cfg.initial_delay_seconds {
        probe.initial_delay_seconds = Some(v);
    }
    if let Some(v) = cfg.period_seconds {
        probe.period_seconds = Some(v);
    }
    if let Some(v) = cfg.timeout_seconds {
        probe.timeout_seconds = Some(v);
    }
    if let Some(v) = cfg.success_threshold {
        probe.success_threshold = Some(v);
    }
    if let Some(v) = cfg.failure_threshold {
        probe.failure_threshold = Some(v);
    }
    Some(probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_liveness_uses_tcp() {
        let p = default_liveness_probe(&NodeType::Validator);
        assert!(p.tcp_socket.is_some(), "validator must use TCP socket probe");
        assert!(p.http_get.is_none());
    }

    #[test]
    fn horizon_liveness_uses_http() {
        let p = default_liveness_probe(&NodeType::Horizon);
        assert!(p.http_get.is_some(), "horizon must use HTTP probe");
        assert!(p.tcp_socket.is_none());
    }

    #[test]
    fn validator_readiness_uses_exec() {
        let p = default_readiness_probe(&NodeType::Validator);
        assert!(p.exec.is_some(), "validator readiness must use exec probe");
    }

    #[test]
    fn apply_probe_override_none_returns_base() {
        let base = Some(default_liveness_probe(&NodeType::Horizon));
        let result = apply_probe_override(base.clone(), None);
        assert_eq!(
            result.as_ref().unwrap().period_seconds,
            base.as_ref().unwrap().period_seconds
        );
    }

    #[test]
    fn apply_probe_override_applies_period() {
        let base = Some(default_liveness_probe(&NodeType::Horizon));
        let override_cfg = ProbeOverride {
            period_seconds: Some(99),
            ..Default::default()
        };
        let result = apply_probe_override(base, Some(&override_cfg));
        assert_eq!(result.unwrap().period_seconds, Some(99));
    }
}
