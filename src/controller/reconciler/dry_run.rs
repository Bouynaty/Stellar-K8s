//! Dry-run gate for reconcile actions.
//!
//! The `apply_or_emit_owned` helper (and the `apply_or_emit!` macro) intercept
//! every mutating Kubernetes call.  In dry-run mode the real mutation is skipped
//! and a "WouldCreate / WouldUpdate / WouldDelete" Kubernetes Event is published
//! instead.  This lets operators preview changes without touching the cluster.

use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use kube::runtime::events::EventType;
use tracing::info;

use crate::controller::reconciler::state::ControllerState;
use crate::crd::StellarNode;
use crate::error::Result;

// ── Action type ───────────────────────────────────────────────────────────────

/// The type of mutating action being gated by the dry-run check.
#[derive(Debug, Clone, Copy)]
pub enum ActionType {
    Create,
    Update,
    Delete,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Create => write!(f, "create"),
            ActionType::Update => write!(f, "update"),
            ActionType::Delete => write!(f, "delete"),
        }
    }
}

// ── Core helper ───────────────────────────────────────────────────────────────

/// Execute `fut` or — when in dry-run mode — emit a "Would…" Kubernetes Event.
pub fn apply_or_emit_owned<Fut>(
    ctx: Arc<ControllerState>,
    node: Arc<StellarNode>,
    action: ActionType,
    resource_info: String,
    fut: Fut,
) -> BoxFuture<'static, Result<()>>
where
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    async move {
        if ctx.dry_run {
            let reason = match action {
                ActionType::Create => "WouldCreate",
                ActionType::Update => "WouldUpdate",
                ActionType::Delete => "WouldDelete",
            };
            let message = format!("Dry Run: Would {action} {resource_info}");
            info!("{}", message);
            publish_stellar_event!(
                ctx.client,
                ctx.event_reporter,
                node,
                EventType::Normal,
                reason,
                "DryRun",
                message
            )
            .await?;
        } else {
            fut.await?;
        }
        Ok(())
    }
    .boxed()
}

// ── Trait helpers for the apply_or_emit! macro ────────────────────────────────

pub(crate) trait ToControllerStateArc {
    fn to_arc_controller(&self) -> Arc<ControllerState>;
}
impl ToControllerStateArc for Arc<ControllerState> {
    fn to_arc_controller(&self) -> Arc<ControllerState> { self.clone() }
}
impl ToControllerStateArc for &Arc<ControllerState> {
    fn to_arc_controller(&self) -> Arc<ControllerState> { (*self).clone() }
}

// ── Macro ─────────────────────────────────────────────────────────────────────

/// Execute an async closure or emit a dry-run event.
///
/// ```ignore
/// apply_or_emit!(ctx, node, ActionType::Create, "Deployment/my-node", |client, ctx, node| async move {
///     ensure_deployment(&client, &node, …).await
/// });
/// ```
#[macro_export]
macro_rules! apply_or_emit {
    ($ctx:expr, $node:expr, $action:expr, $info:expr,
     clones: [$($clone:ident),*], $closure:expr $(,)?) => {
        {
            $( let $clone = $clone.clone(); )*
            let _ctx_internal =
                $crate::controller::reconciler::dry_run::ToControllerStateArc::to_arc_controller(&$ctx);
            let _node_internal =
                $crate::controller::reconciler::events::ToStellarNodeArc::to_arc(&$node);
            let _client_clone = _ctx_internal.client.clone();
            let _ctx_clone = _ctx_internal.clone();
            let _node_clone = _node_internal.clone();
            let _fut = $closure(_client_clone, _ctx_clone, _node_clone);
            $crate::controller::reconciler::dry_run::apply_or_emit_owned(
                _ctx_internal, _node_internal, $action, $info.to_string(), _fut,
            )
        }
    };
    ($ctx:expr, $node:expr, $action:expr, $info:expr, $closure:expr $(,)?) => {
        apply_or_emit!($ctx, $node, $action, $info, clones: [], $closure)
    };
}
