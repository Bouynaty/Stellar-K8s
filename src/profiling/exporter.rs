//! Profile data export in multiple formats (issue #1416).

use crate::profiling::collector::{AllocationSample, CpuSample};
use crate::profiling::endpoints::ProfilingError;
use serde_json::Value;

/// Supported profile export formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileFormat {
    /// JSON (human-readable, default).
    Json,
    /// Protobuf-encoded pprof profile (compatible with `go tool pprof`).
    /// Not yet implemented; reserved for a future iteration.
    Pprof,
}

/// Exports profile data into the requested format.
#[derive(Default)]
pub struct ProfileExporter;

impl ProfileExporter {
    pub fn new() -> Self {
        Self
    }

    /// Serialise a [`CpuSample`] into the requested format.
    pub fn export_cpu(
        &self,
        sample: &CpuSample,
        format: ProfileFormat,
    ) -> Result<Value, ProfilingError> {
        match format {
            ProfileFormat::Json => {
                serde_json::to_value(sample).map_err(|e| ProfilingError::Export(e.to_string()))
            }
            ProfileFormat::Pprof => {
                // Placeholder — a real implementation would serialize to the
                // pprof proto3 binary format and base64-encode for JSON transport.
                Err(ProfilingError::Export(
                    "pprof binary format not yet implemented; use format=json".to_string(),
                ))
            }
        }
    }

    /// Serialise an [`AllocationSample`] into the requested format.
    pub fn export_heap(
        &self,
        sample: &AllocationSample,
        format: ProfileFormat,
    ) -> Result<Value, ProfilingError> {
        match format {
            ProfileFormat::Json => {
                serde_json::to_value(sample).map_err(|e| ProfilingError::Export(e.to_string()))
            }
            ProfileFormat::Pprof => Err(ProfilingError::Export(
                "pprof binary format not yet implemented; use format=json".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiling::collector::{AllocationSample, CpuSample};
    use chrono::Utc;
    use std::collections::HashMap;

    fn dummy_cpu_sample() -> CpuSample {
        let mut stack_counts = HashMap::new();
        stack_counts.insert("main".to_string(), 42);
        CpuSample {
            captured_at: Utc::now(),
            duration_secs: 10.0,
            stack_counts,
            cpu_time_ms: 500.0,
            wall_time_ms: 10_000.0,
            active_tasks: 5,
        }
    }

    fn dummy_alloc_sample() -> AllocationSample {
        let mut sites = HashMap::new();
        sites.insert("allocator".to_string(), 1024 * 1024);
        AllocationSample {
            captured_at: Utc::now(),
            rss_bytes: 50 * 1024 * 1024,
            vsize_bytes: 100 * 1024 * 1024,
            heap_allocated_bytes: 60 * 1024 * 1024,
            heap_freed_bytes: 10 * 1024 * 1024,
            live_heap_bytes: 50 * 1024 * 1024,
            object_count: 100_000,
            allocation_sites: sites,
        }
    }

    #[test]
    fn export_cpu_json() {
        let exporter = ProfileExporter::new();
        let result = exporter.export_cpu(&dummy_cpu_sample(), ProfileFormat::Json);
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v.get("cpu_time_ms").is_some());
    }

    #[test]
    fn export_heap_json() {
        let exporter = ProfileExporter::new();
        let result = exporter.export_heap(&dummy_alloc_sample(), ProfileFormat::Json);
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v.get("live_heap_bytes").is_some());
    }

    #[test]
    fn export_pprof_returns_not_implemented() {
        let exporter = ProfileExporter::new();
        let result = exporter.export_cpu(&dummy_cpu_sample(), ProfileFormat::Pprof);
        assert!(matches!(result, Err(ProfilingError::Export(_))));
    }
}
