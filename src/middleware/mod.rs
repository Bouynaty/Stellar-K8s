//! HTTP middleware: correlation IDs + structured logging
//!
//! - `correlation_middleware`: extracts `X-Correlation-ID` / `X-Request-ID` or generates UUID,
//!   stores in request extensions, echoes in response headers, and injects into tracing span.
//! - `graceful_degradation`: helper to build degraded responses for partial failures.

pub mod correlation;
pub mod degradation;

pub use correlation::{correlation_middleware, CorrelationId};
pub use degradation::{degraded_response, DegradationContext};
