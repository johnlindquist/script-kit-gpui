//! Immutable window mutation plans.
//!
//! A plan is compiled from intent + the CURRENT registry/topology snapshots
//! and is entirely side-effect-free to construct: no AX writes, no app
//! activation, no provider mutations. Execution (S10) re-validates identity
//! and generations before any write.

use super::diagnostics::OperationSource;
use super::presets::LayoutTarget;
use super::types::{Bounds, DisplayId, NativeWindowId, WindowHandle, WindowObservation};

/// What execution does with focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    PreserveCurrentFocus,
    FocusTargetAtEnd,
    DoNotActivateApp,
}

/// How execution treats a mid-plan failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackPolicy {
    /// Any failure restores already-mutated windows in reverse order.
    Strict,
    /// Apply what can be applied; report partial.
    BestEffort,
    /// No rollback (single irreversible ops like focus/close).
    None,
}

/// How execution decides success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationPolicy {
    /// Readback must satisfy the request within tolerance.
    Required,
    /// The action's own acknowledgment suffices (focus).
    ActionAcknowledged,
}

/// One requested mutation.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestedMutation {
    Focus,
    Close,
    SetPosition { x: i32, y: i32 },
    SetSize { width: u32, height: u32 },
    SetBounds(Bounds),
    SetMinimized(bool),
}

/// Identity facts execution must re-confirm before mutating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedWindowIdentity {
    pub pid: i32,
    pub nonce: u64,
    pub native_window_id: Option<NativeWindowId>,
    pub bundle_id: Option<String>,
    pub registry_generation: u64,
}

impl ExpectedWindowIdentity {
    pub(super) fn from_observation(observation: &WindowObservation) -> Self {
        Self {
            pid: observation.handle.pid,
            nonce: observation.handle.nonce,
            native_window_id: observation.handle.native_window_id,
            bundle_id: observation.app.bundle_id.clone(),
            registry_generation: observation.handle.registry_generation,
        }
    }
}

/// One planned mutation against one window.
#[derive(Debug, Clone)]
pub struct PlannedWindowMutation {
    pub target: WindowHandle,
    pub expected_identity: ExpectedWindowIdentity,
    pub request: RequestedMutation,
    /// The semantic target that produced the request, when one exists.
    pub semantic_target: Option<LayoutTarget>,
    pub destination_display: Option<DisplayId>,
}

/// An immutable, executable plan.
#[derive(Debug, Clone)]
pub struct WindowMutationPlan {
    pub plan_id: String,
    pub source: OperationSource,
    pub snapshot_generation: u64,
    pub topology_generation: u64,
    /// When true, execution rejects the plan if topology changed since
    /// compilation (display-relative geometry would be stale).
    pub requires_topology_generation: bool,
    pub operations: Vec<PlannedWindowMutation>,
    pub focus_policy: FocusPolicy,
    pub rollback_policy: RollbackPolicy,
    pub verification: VerificationPolicy,
    pub record_undo: bool,
}

/// Allocate a process-unique plan id.
pub(super) fn next_plan_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("plan-{sequence}")
}

/// Build a single-operation explicit-bounds plan (snap commits, layouts).
pub(super) fn build_explicit_bounds_plan(
    source: OperationSource,
    observation: &WindowObservation,
    bounds: Bounds,
    rollback_policy: RollbackPolicy,
    record_undo: bool,
) -> WindowMutationPlan {
    WindowMutationPlan {
        plan_id: next_plan_id(),
        source,
        snapshot_generation: observation.handle.registry_generation,
        topology_generation: super::display_topology::topology_generation(),
        requires_topology_generation: false,
        operations: vec![PlannedWindowMutation {
            target: observation.handle,
            expected_identity: ExpectedWindowIdentity::from_observation(observation),
            request: RequestedMutation::SetBounds(bounds),
            semantic_target: None,
            destination_display: None,
        }],
        focus_policy: FocusPolicy::PreserveCurrentFocus,
        rollback_policy,
        verification: VerificationPolicy::Required,
        record_undo,
    }
}
