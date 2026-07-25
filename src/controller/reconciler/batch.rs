//! Batch summary reporting for the controller reconciliation loop.
//!
//! [`BatchSummaryReport`] is used as the accumulator in the `fold` at the end of
//! [`super::runner::run_controller`].  It emits a structured summary log every
//! `batch_size` events so operators can see throughput at a glance without
//! drowning in per-object noise.

use tracing::{info, warn};

/// Summary report for a batch of reconciliation results.
///
/// Tracks the number of successful and failed reconciliations
/// within a reporting window and provides a formatted summary log.
#[derive(Debug, Default)]
pub struct BatchSummaryReport {
    /// Number of successful reconciliations in this batch.
    pub successes: u64,
    /// Number of failed reconciliations in this batch.
    pub failures: u64,
    /// Names of successfully reconciled objects in this batch.
    pub reconciled_objects: Vec<String>,
    /// Failure details: (object name, error description).
    pub failure_details: Vec<(String, String)>,
    /// Total events seen (successes + failures).
    pub total: u64,
    /// Emit a summary every N events (batch window size).
    batch_size: u64,
}

impl BatchSummaryReport {
    /// Create a new report that emits a summary every `batch_size` events.
    pub fn new(batch_size: u64) -> Self {
        Self {
            batch_size: batch_size.max(1),
            ..Default::default()
        }
    }

    /// Record a successful reconciliation.
    pub fn record_success(&mut self, object_name: String) {
        self.successes += 1;
        self.total += 1;
        self.reconciled_objects.push(object_name);
        if self.total.is_multiple_of(self.batch_size) {
            self.emit_summary();
        }
    }

    /// Record a failed reconciliation.
    pub fn record_failure(&mut self, object_name: String, error: String) {
        self.failures += 1;
        self.total += 1;
        self.failure_details.push((object_name, error));
        if self.total.is_multiple_of(self.batch_size) {
            self.emit_summary();
        }
    }

    /// Emit the end-of-batch summary log.
    pub fn emit_summary(&self) {
        info!(
            total = self.total,
            successes = self.successes,
            failures = self.failures,
            "=== Reconciliation batch summary ==="
        );
        if !self.reconciled_objects.is_empty() {
            info!(
                objects = ?self.reconciled_objects,
                "Reconciled objects in this batch"
            );
        }
        if !self.failure_details.is_empty() {
            for (name, err) in &self.failure_details {
                warn!(object = %name, error = %err, "Reconciliation failure in batch");
            }
        }
    }

    /// Emit a final summary regardless of batch window position.
    ///
    /// Call this when the controller shuts down.
    pub fn emit_final_summary(&self) {
        if self.total == 0 {
            info!("=== End-of-run summary: no reconciliation events processed ===");
            return;
        }
        let success_rate = (self.successes as f64 / self.total as f64) * 100.0;
        info!(
            total = self.total,
            successes = self.successes,
            failures = self.failures,
            success_rate_pct = %format!("{:.1}", success_rate),
            "=== End-of-run reconciliation summary ==="
        );
        if !self.failure_details.is_empty() {
            warn!(
                failure_count = self.failures,
                "Failures encountered during this run:"
            );
            for (name, err) in &self.failure_details {
                warn!(object = %name, error = %err, "  Failed reconciliation");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_report_starts_empty() {
        let r = BatchSummaryReport::new(10);
        assert_eq!(r.total, 0);
        assert_eq!(r.successes, 0);
        assert_eq!(r.failures, 0);
    }

    #[test]
    fn record_success_increments_counts() {
        let mut r = BatchSummaryReport::new(100);
        r.record_success("node-a".to_string());
        assert_eq!(r.total, 1);
        assert_eq!(r.successes, 1);
        assert_eq!(r.failures, 0);
    }

    #[test]
    fn record_failure_increments_counts() {
        let mut r = BatchSummaryReport::new(100);
        r.record_failure("node-b".to_string(), "some error".to_string());
        assert_eq!(r.total, 1);
        assert_eq!(r.failures, 1);
        assert_eq!(r.successes, 0);
    }

    #[test]
    fn emit_final_summary_does_not_panic_on_empty() {
        let r = BatchSummaryReport::new(10);
        r.emit_final_summary(); // must not panic
    }

    #[test]
    fn emit_final_summary_does_not_panic_with_data() {
        let mut r = BatchSummaryReport::new(100);
        r.record_success("a".to_string());
        r.record_failure("b".to_string(), "err".to_string());
        r.emit_final_summary(); // must not panic
    }
}
