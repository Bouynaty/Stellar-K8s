//! Performance Profiling Integration for Rust Services (issue #1416)
//!
//! Provides CPU and memory profiling endpoints gated behind authentication
//! for production use. Profiles can be captured and exported in pprof-compatible
//! format for analysis with standard tooling (pprof, flamegraph, etc.).
//!
//! # Architecture
//!
//! ```text
//! HTTP request (authenticated)
//!   → /debug/pprof/profile     — CPU profile (duration configurable)
//!   → /debug/pprof/heap        — Heap/memory allocation profile
//!   → /debug/pprof/goroutine   — Active async task trace
//!   → /debug/pprof/cmdline     — Process command line
//!   → /debug/pprof/symbol      — Symbol lookup
//! ```
//!
//! All endpoints require a valid `X-Profiling-Token` header matching the
//! configured secret. In production this token should be a high-entropy
//! random value managed via Kubernetes Secrets.

pub mod collector;
pub mod endpoints;
pub mod exporter;
pub mod metrics;
pub mod reporter;

pub use collector::{AllocationSample, CpuSample, ProfileCollector};
pub use endpoints::{ProfilingEndpoints, ProfilingConfig, ProfilingAuth};
pub use exporter::{ProfileExporter, ProfileFormat};
pub use metrics::ProfilingMetrics;
pub use reporter::{BottleneckReport, ProfileReporter};
