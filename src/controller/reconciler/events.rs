//! Kubernetes Event helpers for the StellarNode controller.
//!
//! Provides the `emit_event!` and `publish_stellar_event!` macros together with
//! their underlying async free-functions.  Extracted from `reconciler.rs` so
//! that event-publishing logic is easy to read and test in isolation.

use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use kube::client::Client;
use kube::runtime::events::{Event as K8sRecorderEvent, EventType, Recorder, Reporter};
use kube::Resource;

use crate::crd::StellarNode;
use crate::error::{Error, Result};

// ── Trait helpers so macros accept Arc<T>, &Arc<T> and T by value ─────────────

pub(crate) trait ToStellarNodeArc {
    fn to_arc(&self) -> Arc<StellarNode>;
}
impl ToStellarNodeArc for Arc<StellarNode> {
    fn to_arc(&self) -> Arc<StellarNode> { self.clone() }
}
impl ToStellarNodeArc for &Arc<StellarNode> {
    fn to_arc(&self) -> Arc<StellarNode> { (*self).clone() }
}
impl ToStellarNodeArc for StellarNode {
    fn to_arc(&self) -> Arc<StellarNode> { Arc::new(self.clone()) }
}
impl ToStellarNodeArc for &StellarNode {
    fn to_arc(&self) -> Arc<StellarNode> { Arc::new((*self).clone()) }
}

// ── Public helpers ─────────────────────────────────────────────────────────────

/// Build a [`Recorder`] for the given node.
pub fn recorder_for(client: &Client, reporter: &Reporter, node: &StellarNode) -> Recorder {
    Recorder::new(client.clone(), reporter.clone(), node.object_ref(&()))
}

/// Publish a structured Kubernetes Event on a StellarNode.
pub async fn publish_object_event(
    recorder: &Recorder,
    type_: EventType,
    reason: &str,
    action: &str,
    note: &str,
) -> Result<()> {
    recorder
        .publish(K8sRecorderEvent {
            type_,
            reason: reason.to_string(),
            action: action.to_string(),
            note: Some(note.to_string()),
            secondary: None,
        })
        .await
        .map_err(Error::KubeError)
}

/// Owned variant used by macros (all `String` args, no lifetimes).
pub fn emit_event_owned(
    client: Client,
    reporter: Reporter,
    node: Arc<StellarNode>,
    type_: EventType,
    reason: String,
    action: String,
    note: String,
) -> BoxFuture<'static, Result<()>> {
    async move {
        let recorder = recorder_for(&client, &reporter, &node);
        publish_object_event(&recorder, type_, &reason, &action, &note).await
    }
    .boxed()
}

/// Alias used by `publish_stellar_event!` — identical implementation.
pub fn publish_stellar_event_owned(
    client: Client,
    reporter: Reporter,
    node: Arc<StellarNode>,
    type_: EventType,
    reason: String,
    action: String,
    note: String,
) -> BoxFuture<'static, Result<()>> {
    emit_event_owned(client, reporter, node, type_, reason, action, note)
}

// ── Macros ─────────────────────────────────────────────────────────────────────

/// Emit a Kubernetes Event on a StellarNode.
///
/// ```ignore
/// emit_event!(client, reporter, node, EventType::Normal, "Reason", "Action", "note").await?;
/// ```
#[macro_export]
macro_rules! emit_event {
    ($client:expr, $reporter:expr, $node:expr, $type:expr,
     $reason:expr, $action:expr, $note:expr $(,)?) => {
        $crate::controller::reconciler::events::emit_event_owned(
            $client.clone(),
            $reporter.clone(),
            $crate::controller::reconciler::events::ToStellarNodeArc::to_arc(&$node),
            $type,
            $reason.to_string(),
            $action.to_string(),
            $note.to_string(),
        )
    };
}

/// Publish a Stellar-specific Kubernetes Event on a StellarNode.
#[macro_export]
macro_rules! publish_stellar_event {
    ($client:expr, $reporter:expr, $node:expr, $type:expr,
     $reason:expr, $action:expr, $note:expr $(,)?) => {
        $crate::controller::reconciler::events::publish_stellar_event_owned(
            $client.clone(),
            $reporter.clone(),
            $crate::controller::reconciler::events::ToStellarNodeArc::to_arc(&$node),
            $type,
            $reason.to_string(),
            $action.to_string(),
            $note.to_string(),
        )
    };
}
