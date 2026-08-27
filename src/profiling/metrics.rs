//! Prometheus metrics for the profiling subsystem (issue #1416).

use serde::{Deserialize, Serialize};

/// Snapshot of profiling-related Prometheus metrics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfilingMetrics {
    /// Total number of CPU profile captures initiated.
    pub cpu_profiles_total: u64,
    /// Total number of heap profile captures initiated.
    pub heap_profiles_total: u64,
    /// Number of authentication failures on profiling endpoints.
    pub auth_failures_total: u64,
    /// Latest observed RSS in bytes.
    pub latest_rss_bytes: u64,
    /// Latest live heap in bytes.
    pub latest_live_heap_bytes: u64,
    /// Latest CPU wall-clock capture time in ms.
    pub latest_cpu_wall_ms: f64,
}

impl ProfilingMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render as a Prometheus text-format scrape payload.
    pub fn render_prometheus(&self) -> String {
        format!(
            "# HELP stellar_profiling_cpu_profiles_total Total CPU profiles captured\n\
             # TYPE stellar_profiling_cpu_profiles_total counter\n\
             stellar_profiling_cpu_profiles_total {cpu}\n\
             # HELP stellar_profiling_heap_profiles_total Total heap profiles captured\n\
             # TYPE stellar_profiling_heap_profiles_total counter\n\
             stellar_profiling_heap_profiles_total {heap}\n\
             # HELP stellar_profiling_auth_failures_total Authentication failures on profiling endpoints\n\
             # TYPE stellar_profiling_auth_failures_total counter\n\
             stellar_profiling_auth_failures_total {auth}\n\
             # HELP stellar_profiling_rss_bytes Latest RSS in bytes\n\
             # TYPE stellar_profiling_rss_bytes gauge\n\
             stellar_profiling_rss_bytes {rss}\n\
             # HELP stellar_profiling_live_heap_bytes Latest live heap in bytes\n\
             # TYPE stellar_profiling_live_heap_bytes gauge\n\
             stellar_profiling_live_heap_bytes {heap_bytes}\n\
             # HELP stellar_profiling_cpu_wall_ms Latest CPU capture wall time ms\n\
             # TYPE stellar_profiling_cpu_wall_ms gauge\n\
             stellar_profiling_cpu_wall_ms {cpu_ms}\n",
            cpu = self.cpu_profiles_total,
            heap = self.heap_profiles_total,
            auth = self.auth_failures_total,
            rss = self.latest_rss_bytes,
            heap_bytes = self.latest_live_heap_bytes,
            cpu_ms = self.latest_cpu_wall_ms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_prometheus_contains_metric_names() {
        let metrics = ProfilingMetrics {
            cpu_profiles_total: 3,
            heap_profiles_total: 5,
            ..Default::default()
        };
        let output = metrics.render_prometheus();
        assert!(output.contains("stellar_profiling_cpu_profiles_total 3"));
        assert!(output.contains("stellar_profiling_heap_profiles_total 5"));
        assert!(output.contains("stellar_profiling_rss_bytes"));
    }
}
