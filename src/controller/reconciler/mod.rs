//! Reconciler module — split from the monolithic `reconciler.rs`.
//!
//! # Sub-modules
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`state`] | [`ControllerState`] definition and accessors |
//! | [`runner`] | [`run_controller`] entry-point and watch setup |
//! | [`core`] | Core `reconcile()` state machine |
//! | [`batch`] | [`BatchSummaryReport`] for fold accumulation |
//! | [`events`] | Kubernetes Event helpers (`emit_event!`, `publish_stellar_event!`) |
//! | [`dry_run`] | `apply_or_emit` dry-run gate helpers |

pub mod batch;
pub mod core;
pub mod dry_run;
pub mod events;
pub mod runner;
pub mod state;

// Re-export the public surface so callers don't need to know the sub-module layout.
pub use batch::BatchSummaryReport;
pub use core::reconcile;
pub use runner::run_controller;
pub use state::ControllerState;
