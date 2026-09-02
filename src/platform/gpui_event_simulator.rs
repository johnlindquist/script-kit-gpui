//! GPUI Event Simulator
//!
//! Dispatches [`SimulatedGpuiEvent`] through GPUI's real input pipeline,
//! targeting a specific window resolved via the automation registry.
//!
//! This is explicitly separate from the legacy `simulateKey` path in
//! `runtime_stdin_match_simulate_key.rs`, which routes through `AppView`
//! match arms and bypasses GPUI intercepts.
//!
//! Note: these functions are called from the binary crate via `include!()`,
//! not from the library crate directly, so they appear unused to `--lib`.
#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Returns `true` when the given kind represents a surface that is visually
/// attached to a parent window and dispatches mouse events through that parent.
///
/// Attached surfaces need coordinate rebasing: popup-local (x, y) must be
/// translated into the parent window's GPUI dispatch space.
fn is_attached_surface(kind: crate::protocol::AutomationWindowKind) -> bool {
    matches!(
        kind,
        crate::protocol::AutomationWindowKind::ActionsDialog
            | crate::protocol::AutomationWindowKind::PromptPopup
    )
}

/// Translate pointer-event coordinates from target-local space into the parent
/// window's GPUI dispatch space for attached surfaces.
///
/// The parent window is determined from the popup's recorded `parent_window_id`
/// metadata (set at popup registration time via `register_attached_popup`).
/// If an attached popup has no parent metadata, dispatch **fails closed** with
/// an explicit error instead of silently falling back to Main.
///
/// Detached windows and key events pass through unchanged.
/// Returns `Err` with a deterministic message when bounds are unavailable.
fn rebase_mouse_event_to_dispatch_space(
    resolved: &crate::protocol::AutomationWindowInfo,
    event: &crate::protocol::SimulatedGpuiEvent,
) -> Result<crate::protocol::SimulatedGpuiEvent, String> {
    use crate::protocol::SimulatedGpuiEvent;

    if !is_attached_surface(resolved.kind) {
        return Ok(event.clone());
    }

    // Key events don't have coordinates — pass through.
    if matches!(event, SimulatedGpuiEvent::KeyDown { .. }) {
        return Ok(event.clone());
    }

    let target_bounds = resolved.bounds.as_ref().ok_or_else(|| {
        format!(
            "Resolved target {} ({:?}) has no bounds; cannot translate attached-surface coordinates",
            resolved.id, resolved.kind
        )
    })?;

    // Resolve the parent window from the popup's recorded metadata.
    // Fail closed if no parent metadata exists — never silently fall back to Main.
    let parent_id = resolved.parent_window_id.as_ref().ok_or_else(|| {
        format!(
            "Attached surface {} ({:?}) has no parent_window_id metadata; \
             cannot rebase coordinates (fail-closed: will not silently dispatch against Main)",
            resolved.id, resolved.kind
        )
    })?;

    let parent = crate::windows::resolve_automation_window(Some(
        &crate::protocol::AutomationWindowTarget::Id {
            id: parent_id.clone(),
        },
    ))
    .map_err(|err| {
        format!(
            "Failed to resolve parent window '{}' for attached-surface {} dispatch: {err}",
            parent_id, resolved.id
        )
    })?;

    let parent_bounds = parent.bounds.as_ref().ok_or_else(|| {
        format!(
            "Parent window {} ({:?}) has no bounds; cannot translate attached-surface coordinates for {}",
            parent.id, parent.kind, resolved.id
        )
    })?;

    let offset_x = target_bounds.x - parent_bounds.x;
    let offset_y = target_bounds.y - parent_bounds.y;

    // Log the rebased coordinates for observability, including parent identity.
    match event {
        SimulatedGpuiEvent::MouseMove { x, y }
        | SimulatedGpuiEvent::MouseDown { x, y, .. }
        | SimulatedGpuiEvent::MouseUp { x, y, .. }
        | SimulatedGpuiEvent::MouseClick { x, y, .. }
        | SimulatedGpuiEvent::ScrollWheel { x, y, .. } => {
            tracing::info!(
                target: "script_kit::automation",
                window_id = %resolved.id,
                kind = ?resolved.kind,
                parent_window_id = %parent.id,
                parent_kind = ?parent.kind,
                local_x = x,
                local_y = y,
                offset_x = offset_x,
                offset_y = offset_y,
                rebased_x = x + offset_x,
                rebased_y = y + offset_y,
                "gpui_event_simulation.rebased_coordinates"
            );
        }
        SimulatedGpuiEvent::KeyDown { .. } => {}
    }

    let translated = match event {
        SimulatedGpuiEvent::MouseMove { x, y } => SimulatedGpuiEvent::MouseMove {
            x: x + offset_x,
            y: y + offset_y,
        },
        SimulatedGpuiEvent::MouseDown { x, y, button } => SimulatedGpuiEvent::MouseDown {
            x: x + offset_x,
            y: y + offset_y,
            button: button.clone(),
        },
        SimulatedGpuiEvent::MouseUp { x, y, button } => SimulatedGpuiEvent::MouseUp {
            x: x + offset_x,
            y: y + offset_y,
            button: button.clone(),
        },
        SimulatedGpuiEvent::MouseClick { x, y, button } => SimulatedGpuiEvent::MouseClick {
            x: x + offset_x,
            y: y + offset_y,
            button: button.clone(),
        },
        SimulatedGpuiEvent::ScrollWheel {
            x,
            y,
            delta_x,
            delta_y,
            phase,
            direct_phase,
            momentum_phase,
            timestamp_seconds,
        } => SimulatedGpuiEvent::ScrollWheel {
            x: x + offset_x,
            y: y + offset_y,
            delta_x: *delta_x,
            delta_y: *delta_y,
            phase: *phase,
            direct_phase: *direct_phase,
            momentum_phase: *momentum_phase,
            timestamp_seconds: *timestamp_seconds,
        },
        SimulatedGpuiEvent::KeyDown { .. } => event.clone(),
    };

    Ok(translated)
}

/// Returns `true` when GPUI dispatch still collapses all windows of this kind
/// to a single `WindowRole`, meaning it cannot distinguish between multiple
/// visible windows of the same kind.
fn kind_collapses_to_single_window_role(kind: crate::protocol::AutomationWindowKind) -> bool {
    automation_kind_to_window_role(kind).is_some()
}

/// Count how many visible windows share the given [`AutomationWindowKind`].
fn visible_window_count_for_kind(kind: crate::protocol::AutomationWindowKind) -> usize {
    crate::windows::list_automation_windows()
        .into_iter()
        .filter(|w| w.kind == kind && w.visible)
        .count()
}

/// Map an [`AutomationWindowKind`] to the corresponding [`WindowRole`]
/// used by the unified window registry.
///
/// Returns `None` for kinds that don't map to a single `WindowRole`
/// (e.g. `ActionsDialog` and `PromptPopup` are attached to their parent
/// window, not registered as independent window handles).
fn automation_kind_to_window_role(
    kind: crate::protocol::AutomationWindowKind,
) -> Option<crate::windows::WindowRole> {
    use crate::protocol::AutomationWindowKind;
    use crate::windows::WindowRole;

    match kind {
        AutomationWindowKind::Main => Some(WindowRole::Main),
        AutomationWindowKind::Notes => Some(WindowRole::Notes),
        AutomationWindowKind::AgentChatDetached => Some(WindowRole::AgentChat),
        // Attached surfaces and popup-only windows use exact runtime handles
        // when available and do not map to the shared role registry.
        AutomationWindowKind::ActionsDialog
        | AutomationWindowKind::Dictation
        | AutomationWindowKind::PromptPopup
        | AutomationWindowKind::Hud => None,
        // Each display overlay has its own exact runtime handle, not a shared role.
        AutomationWindowKind::SnapOverlay => None,
    }
}

/// Build a GPUI [`Keystroke`] from a `SimulatedGpuiEvent::KeyDown`.
fn build_keystroke(
    key: &str,
    modifiers: &[crate::stdin_commands::KeyModifier],
    text: Option<&str>,
) -> gpui::Keystroke {
    use crate::stdin_commands::KeyModifier;

    let mut mods = gpui::Modifiers::default();
    for m in modifiers {
        match m {
            KeyModifier::Cmd => mods.platform = true,
            KeyModifier::Shift => mods.shift = true,
            KeyModifier::Alt => mods.alt = true,
            KeyModifier::Ctrl => mods.control = true,
        }
    }

    gpui::Keystroke {
        modifiers: mods,
        key: key.to_string(),
        key_char: text.map(String::from),
    }
}

/// Convert the wire-level touch phase into GPUI's platform-input phase.
fn simulated_touch_phase_to_gpui(phase: crate::protocol::SimulatedTouchPhase) -> gpui::TouchPhase {
    match phase {
        crate::protocol::SimulatedTouchPhase::Started => gpui::TouchPhase::Started,
        crate::protocol::SimulatedTouchPhase::Moved => gpui::TouchPhase::Moved,
        crate::protocol::SimulatedTouchPhase::Ended => gpui::TouchPhase::Ended,
    }
}

/// Result of a GPUI event simulation dispatch.
pub(crate) struct GpuiEventDispatchResult {
    /// Whether the event was dispatched (even if not consumed by any handler).
    pub success: bool,
    /// Machine-readable error category: `target_not_found`, `target_ambiguous`,
    /// `handle_unavailable`, or `dispatch_failed`.
    pub error_code: Option<String>,
    /// Human-readable error message if dispatch could not be attempted.
    pub error: Option<String>,
    /// The dispatch path that was used: `"exact_handle"` when the resolved
    /// automation target had a registered runtime handle, `"window_role_fallback"`
    /// when we fell back to `WindowRole`-based dispatch, or `None` on error.
    pub dispatch_path: Option<String>,
    /// The resolved automation window ID, when available.
    pub resolved_window_id: Option<String>,
    /// True when the event was applied before the protocol response returned.
    pub dispatch_completed: bool,
    /// True when the event was scheduled after the protocol response returned.
    pub dispatch_scheduled: bool,
    pub was_deferred: bool,
    /// Raw event dispatch is not handler activation proof.
    pub activation_proof: Option<String>,
}

/// Apply a [`SimulatedGpuiEvent`] to a live GPUI [`gpui::Window`] via the real input pipeline.
pub(crate) fn apply_simulated_event(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    event: &crate::protocol::SimulatedGpuiEvent,
) {
    use crate::protocol::SimulatedGpuiEvent;

    match event {
        SimulatedGpuiEvent::KeyDown {
            key,
            modifiers,
            text,
        } => {
            let keystroke = build_keystroke(key, modifiers, text.as_deref());
            window.dispatch_keystroke(keystroke, cx);
        }
        SimulatedGpuiEvent::MouseMove { x, y } => {
            let position = gpui::point(gpui::px(*x as f32), gpui::px(*y as f32));
            window.dispatch_event(
                gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers: gpui::Modifiers::default(),
                }),
                cx,
            );
        }
        SimulatedGpuiEvent::MouseDown { x, y, button } => {
            let position = gpui::point(gpui::px(*x as f32), gpui::px(*y as f32));
            window.dispatch_event(
                gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                    button: parse_mouse_button(button.as_deref()),
                    position,
                    modifiers: gpui::Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
        }
        SimulatedGpuiEvent::MouseUp { x, y, button } => {
            let position = gpui::point(gpui::px(*x as f32), gpui::px(*y as f32));
            window.dispatch_event(
                gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                    button: parse_mouse_button(button.as_deref()),
                    position,
                    modifiers: gpui::Modifiers::default(),
                    click_count: 1,
                }),
                cx,
            );
        }
        SimulatedGpuiEvent::MouseClick { x, y, button } => {
            let position = gpui::point(gpui::px(*x as f32), gpui::px(*y as f32));
            let button = parse_mouse_button(button.as_deref());
            window.dispatch_event(
                gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                    button,
                    position,
                    modifiers: gpui::Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                    button,
                    position,
                    modifiers: gpui::Modifiers::default(),
                    click_count: 1,
                }),
                cx,
            );
        }
        SimulatedGpuiEvent::ScrollWheel {
            x,
            y,
            delta_x,
            delta_y,
            phase,
            direct_phase,
            momentum_phase,
            timestamp_seconds,
        } => {
            window.dispatch_event(
                gpui::PlatformInput::ScrollWheel(simulated_scroll_wheel_to_gpui(
                    (*x, *y),
                    (*delta_x, *delta_y),
                    *phase,
                    *direct_phase,
                    *momentum_phase,
                    *timestamp_seconds,
                )),
                cx,
            );
        }
    }
}

fn simulated_scroll_wheel_to_gpui(
    position: (f64, f64),
    delta: (f64, f64),
    touch_phase: crate::protocol::SimulatedTouchPhase,
    direct_phase: Option<crate::protocol::SimulatedScrollPhase>,
    momentum_phase: Option<crate::protocol::SimulatedScrollPhase>,
    timestamp_seconds: Option<f64>,
) -> gpui::ScrollWheelEvent {
    let (x, y) = position;
    let (delta_x, delta_y) = delta;
    let touch_phase = simulated_touch_phase_to_gpui(touch_phase);
    gpui::ScrollWheelEvent {
        position: gpui::point(gpui::px(x as f32), gpui::px(y as f32)),
        delta: gpui::ScrollDelta::Pixels(gpui::point(
            gpui::px(delta_x as f32),
            gpui::px(delta_y as f32),
        )),
        modifiers: gpui::Modifiers::default(),
        touch_phase,
        phase: direct_phase
            .map(simulated_scroll_phase_to_gpui)
            .unwrap_or(match touch_phase {
                gpui::TouchPhase::Started => gpui::ScrollPhase::Began,
                gpui::TouchPhase::Moved => gpui::ScrollPhase::Changed,
                gpui::TouchPhase::Ended => gpui::ScrollPhase::Ended,
            }),
        momentum_phase: momentum_phase
            .map(simulated_scroll_phase_to_gpui)
            .unwrap_or(gpui::ScrollPhase::None),
        timestamp_seconds: timestamp_seconds.filter(|value| value.is_finite()),
    }
}

fn simulated_scroll_phase_to_gpui(
    phase: crate::protocol::SimulatedScrollPhase,
) -> gpui::ScrollPhase {
    match phase {
        crate::protocol::SimulatedScrollPhase::None => gpui::ScrollPhase::None,
        crate::protocol::SimulatedScrollPhase::MayBegin => gpui::ScrollPhase::MayBegin,
        crate::protocol::SimulatedScrollPhase::Began => gpui::ScrollPhase::Began,
        crate::protocol::SimulatedScrollPhase::Changed => gpui::ScrollPhase::Changed,
        crate::protocol::SimulatedScrollPhase::Stationary => gpui::ScrollPhase::Stationary,
        crate::protocol::SimulatedScrollPhase::Ended => gpui::ScrollPhase::Ended,
        crate::protocol::SimulatedScrollPhase::Cancelled => gpui::ScrollPhase::Cancelled,
    }
}

#[cfg(test)]
mod simulated_scroll_wheel_tests {
    use super::*;
    use crate::protocol::{SimulatedScrollPhase, SimulatedTouchPhase};

    #[test]
    fn preserves_direct_and_momentum_phases_pixel_delta_and_timestamp() {
        let phases = [
            SimulatedScrollPhase::Began,
            SimulatedScrollPhase::Changed,
            SimulatedScrollPhase::Ended,
        ];
        for phase in phases {
            let event = simulated_scroll_wheel_to_gpui(
                (11.0, 22.0),
                (-1.25, 3.5),
                SimulatedTouchPhase::Moved,
                Some(phase),
                Some(phase),
                Some(42.25),
            );
            assert_eq!(event.phase, simulated_scroll_phase_to_gpui(phase));
            assert_eq!(event.momentum_phase, simulated_scroll_phase_to_gpui(phase));
            assert_eq!(event.timestamp_seconds, Some(42.25));
            match event.delta {
                gpui::ScrollDelta::Pixels(delta) => {
                    assert_eq!(delta.x, gpui::px(-1.25));
                    assert_eq!(delta.y, gpui::px(3.5));
                }
                gpui::ScrollDelta::Lines(_) => {
                    panic!("simulated native wheel must stay pixel-only")
                }
            }
        }
    }

    #[test]
    fn derives_direct_phase_from_touch_phase_and_rejects_non_finite_timestamp() {
        let event = simulated_scroll_wheel_to_gpui(
            (0.0, 0.0),
            (0.0, 0.0),
            SimulatedTouchPhase::Started,
            None,
            None,
            Some(f64::NAN),
        );
        assert_eq!(event.phase, gpui::ScrollPhase::Began);
        assert_eq!(event.momentum_phase, gpui::ScrollPhase::None);
        assert_eq!(event.timestamp_seconds, None);
    }
}

/// A ticket owns cancellation until its one terminal result is consumed.
pub(crate) struct GpuiEventDispatchTicket {
    state: Rc<DispatchCompletion>,
    receiver: async_channel::Receiver<GpuiEventDispatchResult>,
}

struct DispatchCompletion {
    sender: RefCell<Option<async_channel::Sender<GpuiEventDispatchResult>>>,
    resolved_id: Option<String>,
    path: Option<String>,
    was_deferred: std::cell::Cell<bool>,
}

pub(crate) type GpuiDispatchPrecondition = Box<
    dyn FnOnce(
        &crate::protocol::AutomationWindowInfo,
        &mut gpui::Window,
        &mut gpui::App,
    ) -> Result<(), String>,
>;

impl DispatchCompletion {
    fn finish(&self, result: GpuiEventDispatchResult) {
        if let Some(sender) = self.sender.borrow_mut().take() {
            let _ = sender.try_send(result);
        }
    }

    fn pending(&self) -> bool {
        self.sender.borrow().is_some()
    }

    fn failure(&self, code: &str, error: impl Into<String>) -> GpuiEventDispatchResult {
        GpuiEventDispatchResult {
            success: false,
            error_code: Some(code.into()),
            error: Some(error.into()),
            dispatch_path: self.path.clone(),
            resolved_window_id: self.resolved_id.clone(),
            dispatch_completed: false,
            dispatch_scheduled: false,
            was_deferred: self.was_deferred.get(),
            activation_proof: None,
        }
    }
}

impl GpuiEventDispatchTicket {
    async fn completed(self) -> GpuiEventDispatchResult {
        match self.receiver.recv().await {
            Ok(result) => result,
            Err(_) => self
                .state
                .failure("dispatch_cancelled", "Dispatch producer was dropped"),
        }
    }
}

impl Drop for GpuiEventDispatchTicket {
    fn drop(&mut self) {
        self.state.finish(
            self.state
                .failure("dispatch_cancelled", "Dispatch receiver was dropped"),
        );
    }
}

#[derive(Default)]
struct GpuiDispatchRequests {
    issued: HashSet<String>,
    pending: HashMap<String, Weak<DispatchCompletion>>,
}
impl gpui::Global for GpuiDispatchRequests {}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Retain exact handles without probing a window that is on GPUI's update stack.
/// Metadata, parent lifetime and the same handle are checked again at execution.
pub(crate) struct DispatchTarget {
    pub(crate) info: crate::protocol::AutomationWindowInfo,
    handle: gpui::AnyWindowHandle,
    parent: Option<(
        crate::protocol::AutomationWindowInfo,
        Option<gpui::AnyWindowHandle>,
    )>,
    exact: bool,
    role: Option<crate::windows::WindowRole>,
}

pub(crate) struct DispatchTargetError {
    pub code: &'static str,
    pub message: String,
}

impl DispatchTargetError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn runtime_handle(info: &crate::protocol::AutomationWindowInfo) -> Option<gpui::AnyWindowHandle> {
    match info.generation {
        Some(generation) => {
            crate::windows::get_runtime_window_handle_for_generation(&info.id, generation)
        }
        None => crate::windows::get_runtime_window_handle(&info.id),
    }
}

fn same_lifetime(expected: &crate::protocol::AutomationWindowInfo) -> Result<(), String> {
    use crate::protocol::AutomationWindowTarget;
    let target = match expected.generation {
        Some(generation) => AutomationWindowTarget::Instance {
            id: expected.id.clone(),
            generation,
        },
        None => AutomationWindowTarget::Id {
            id: expected.id.clone(),
        },
    };
    let current = crate::windows::resolve_automation_window(Some(&target))
        .map_err(|error| error.to_string())?;
    if current.kind != expected.kind
        || current.generation != expected.generation
        || current.parent_window_id != expected.parent_window_id
        || current.parent_window_generation != expected.parent_window_generation
        || current.parent_kind != expected.parent_kind
        || current.semantic_surface != expected.semantic_surface
    {
        return Err("Target owner or parent changed while dispatch was queued".into());
    }
    Ok(())
}

impl DispatchTarget {
    pub(crate) fn resolve(
        target: Option<&crate::protocol::AutomationWindowTarget>,
    ) -> Result<Self, DispatchTargetError> {
        use crate::protocol::{AutomationWindowKind, AutomationWindowTarget};
        let info = crate::windows::resolve_automation_window(target)
            .map_err(|error| DispatchTargetError::new("target_not_found", error.to_string()))?;
        let parent = if let Some(id) = &info.parent_window_id {
            let target = match info.parent_window_generation {
                Some(generation) => AutomationWindowTarget::Instance {
                    id: id.clone(),
                    generation,
                },
                None => AutomationWindowTarget::Id { id: id.clone() },
            };
            let parent = crate::windows::resolve_automation_window(Some(&target))
                .map_err(|error| DispatchTargetError::new("stale_target", error.to_string()))?;
            if info.parent_kind != Some(parent.kind) {
                return Err(DispatchTargetError::new(
                    "stale_target",
                    "Target parent kind mismatch",
                ));
            }
            let handle = runtime_handle(&parent);
            if parent.generation.is_some() && handle.is_none() {
                return Err(DispatchTargetError::new(
                    "handle_unavailable",
                    "Parent runtime handle missing",
                ));
            }
            Some((parent, handle))
        } else {
            None
        };
        if let Some(handle) = runtime_handle(&info) {
            return Ok(Self {
                info,
                handle,
                parent,
                exact: true,
                role: None,
            });
        }
        if info.generation.is_some() {
            return Err(DispatchTargetError::new(
                "stale_or_missing_instance_handle",
                "Exact instance has no matching runtime handle",
            ));
        }
        let dispatch_info = if is_attached_surface(info.kind) {
            &parent
                .as_ref()
                .ok_or_else(|| {
                    DispatchTargetError::new(
                        "handle_unavailable",
                        "Attached target has no parent identity",
                    )
                })?
                .0
        } else {
            &info
        };
        if kind_collapses_to_single_window_role(dispatch_info.kind)
            && visible_window_count_for_kind(dispatch_info.kind) > 1
        {
            return Err(DispatchTargetError::new(
                "target_ambiguous",
                "Target is ambiguous without an exact runtime handle",
            ));
        }
        let role = automation_kind_to_window_role(dispatch_info.kind).ok_or_else(|| {
            DispatchTargetError::new("handle_unavailable", "Target has no runtime handle")
        })?;
        let handle = runtime_handle(dispatch_info)
            .or_else(|| crate::windows::get_window(role).map(Into::into))
            .or_else(|| {
                (dispatch_info.kind == AutomationWindowKind::Main)
                    .then(crate::get_main_window_handle)
                    .flatten()
            })
            .ok_or_else(|| {
                DispatchTargetError::new("handle_unavailable", "Target window handle unavailable")
            })?;
        Ok(Self {
            info,
            handle,
            parent,
            exact: false,
            role: Some(role),
        })
    }

    /// The retained handle, not a freshly resolved target. Callers validate at
    /// the actual mutation boundary before using it to apply an effect.
    pub(crate) fn handle(&self) -> gpui::AnyWindowHandle {
        self.handle
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        same_lifetime(&self.info)?;
        if let Some((parent, handle)) = &self.parent {
            same_lifetime(parent)?;
            if runtime_handle(parent) != *handle {
                return Err("Parent runtime handle changed".into());
            }
        }
        if self.exact {
            if runtime_handle(&self.info) != Some(self.handle) {
                return Err("Target runtime handle changed".into());
            }
        } else {
            let dispatch_info = self
                .parent
                .as_ref()
                .map(|(info, _)| info)
                .unwrap_or(&self.info);
            let current = runtime_handle(dispatch_info)
                .or_else(|| {
                    self.role
                        .and_then(crate::windows::get_window)
                        .map(Into::into)
                })
                .or_else(|| {
                    (dispatch_info.kind == crate::protocol::AutomationWindowKind::Main)
                        .then(crate::get_main_window_handle)
                        .flatten()
                });
            if current != Some(self.handle) {
                return Err("Fallback window handle changed".into());
            }
            if visible_window_count_for_kind(dispatch_info.kind) > 1 {
                return Err("Fallback became ambiguous".into());
            }
        }
        Ok(())
    }
}

/// Ordinary stdin/script dispatch: scheduling is internal, never a successful reply.
/// The callback performs the final authority/deadline check without yielding before mutation.
pub(crate) fn dispatch_gpui_event(
    request_id: &str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
    event: crate::protocol::SimulatedGpuiEvent,
    deadline_unix_ms: Option<u64>,
    precondition: Option<GpuiDispatchPrecondition>,
    cx: &mut gpui::App,
) -> Option<GpuiEventDispatchTicket> {
    cx.default_global::<GpuiDispatchRequests>();
    let requests = cx.global_mut::<GpuiDispatchRequests>();
    if requests.issued.contains(request_id) {
        return None;
    }
    // Keep uniqueness for the entire connection/app lifetime, not only pending work.
    if requests.issued.len() >= 100_000 {
        return None;
    }
    requests.issued.insert(request_id.into());
    let resolved = DispatchTarget::resolve(target);
    let (sender, receiver) = async_channel::bounded(1);
    let state = Rc::new(DispatchCompletion {
        sender: RefCell::new(Some(sender)),
        resolved_id: resolved.as_ref().ok().map(|target| target.info.id.clone()),
        path: resolved.as_ref().ok().map(|target| {
            if target.exact {
                "exact_handle_deferred"
            } else {
                "window_role_fallback_deferred"
            }
            .into()
        }),
        was_deferred: std::cell::Cell::new(false),
    });
    let ticket = GpuiEventDispatchTicket {
        state: state.clone(),
        receiver,
    };
    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) => {
            state.finish(state.failure(error.code, error.message));
            return Some(ticket);
        }
    };
    let now = unix_ms();
    let deadline_unix_ms = deadline_unix_ms.unwrap_or_else(|| now.saturating_add(5_000));
    if deadline_unix_ms <= now || deadline_unix_ms.saturating_sub(now) > 600_000 {
        state.finish(state.failure(
            "dispatch_deadline_exceeded",
            "Dispatch deadline expired or invalid",
        ));
        return Some(ticket);
    }
    let deadline = cx.background_executor().now() + Duration::from_millis(deadline_unix_ms - now);
    state.was_deferred.set(true);
    cx.global_mut::<GpuiDispatchRequests>()
        .pending
        .insert(request_id.into(), Rc::downgrade(&state));
    let request_id = request_id.to_owned();
    // Foreground deferral returns the stdin owner's entity/window borrows
    // before dispatch without making input delivery depend on timer progress.
    cx.defer(move |cx| {
        if state.pending() {
            let result =
                if unix_ms() >= deadline_unix_ms || cx.background_executor().now() >= deadline {
                    state.failure(
                        "dispatch_deadline_exceeded",
                        "Dispatch expired before execution",
                    )
                } else if let Err(error) = resolved.validate() {
                    state.failure("stale_target", error)
                } else {
                    let event = if resolved.exact {
                        Ok(event)
                    } else {
                        rebase_mouse_event_to_dispatch_space(&resolved.info, &event)
                    };
                    match event
                        .map_err(|error| {
                            DispatchTargetError::new("coordinate_translation_failed", error)
                        })
                        .and_then(|event| {
                            crate::windows::with_runtime_window_dispatch(resolved.handle, || {
                                resolved.handle.update(cx, |_, window, cx| {
                                    if window.is_owned_hidden()
                                        && !matches!(
                                            event,
                                            crate::protocol::SimulatedGpuiEvent::KeyDown { .. }
                                        )
                                        && precondition.is_none()
                                    {
                                        return Err(DispatchTargetError::new(
                                        "expected_frame_required",
                                        "Owned coordinate input requires completed-frame authority",
                                    ));
                                    }
                                    if let Some(validate) = precondition {
                                        validate(&resolved.info, window, cx).map_err(|error| {
                                            let code = if error == "expected_frame_required" {
                                                "expected_frame_required"
                                            } else {
                                                "stale_target_identity"
                                            };
                                            DispatchTargetError::new(code, error)
                                        })?;
                                    }
                                    apply_simulated_event(window, cx, &event);
                                    Ok(())
                                })
                            })
                            .map_err(|error| {
                                DispatchTargetError::new("dispatch_failed", error.to_string())
                            })?
                        }) {
                        Ok(()) => GpuiEventDispatchResult {
                            success: true,
                            error_code: None,
                            error: None,
                            dispatch_path: state.path.clone(),
                            resolved_window_id: state.resolved_id.clone(),
                            dispatch_completed: true,
                            dispatch_scheduled: false,
                            was_deferred: true,
                            activation_proof: Some("not_observed".into()),
                        },
                        Err(error) => state.failure(error.code, error.message),
                    }
                };
            state.finish(result);
        }
        cx.global_mut::<GpuiDispatchRequests>()
            .pending
            .remove(&request_id);
    });
    Some(ticket)
}

/// Production protocol response owner, shared by stdin and running scripts.
pub(crate) fn handle_gpui_event_message(
    message: crate::protocol::Message,
    sender: std::sync::mpsc::SyncSender<crate::protocol::Message>,
    precondition: Option<GpuiDispatchPrecondition>,
    cx: &mut gpui::App,
) {
    use crate::protocol::Message;
    let (request_id, ticket) = match message {
        Message::CancelGpuiEvent { request_id } => {
            if let Some(requests) = cx.try_global::<GpuiDispatchRequests>() {
                if let Some(state) = requests.pending.get(&request_id).and_then(Weak::upgrade) {
                    state.finish(
                        state.failure("dispatch_cancelled", "Dispatch cancelled before execution"),
                    );
                }
            }
            return;
        }
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
            deadline_unix_ms,
            expected,
            expected_frame,
        } => {
            let precondition = precondition.or_else(|| {
                (expected.is_some() || expected_frame.is_some()).then(|| {
                    Box::new(
                        |_: &crate::protocol::AutomationWindowInfo,
                         _: &mut gpui::Window,
                         _: &mut gpui::App| {
                            Err("expected_identity_validator_missing".into())
                        },
                    ) as GpuiDispatchPrecondition
                })
            });
            let Some(ticket) = dispatch_gpui_event(
                &request_id,
                target.as_ref(),
                event,
                deadline_unix_ms,
                precondition,
                cx,
            ) else {
                return;
            };
            (request_id, ticket)
        }
        _ => unreachable!("GPUI event handler accepts only simulation and cancellation"),
    };
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        let result = ticket.completed().await;
        let mut response = Message::SimulateGpuiEventResult {
            request_id,
            success: result.success,
            error_code: result.error_code,
            error: result.error,
            dispatch_path: result.dispatch_path,
            resolved_window_id: result.resolved_window_id,
            dispatch_completed: result.dispatch_completed,
            dispatch_scheduled: result.dispatch_scheduled,
            was_deferred: result.was_deferred,
            activation_proof: result.activation_proof,
        };
        // Backpressure retains the terminal reply; never drop it or block the GPUI thread.
        loop {
            match sender.try_send(response) {
                Ok(()) => break,
                Err(std::sync::mpsc::TrySendError::Full(returned)) => {
                    response = returned;
                    cx.background_executor()
                        .timer(Duration::from_millis(1))
                        .await;
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
            }
        }
    })
    .detach();
}

#[cfg(test)]
mod deferred_dispatch_tests {
    use super::*;
    use crate::protocol::{
        AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget, Message,
    };
    use gpui::{prelude::*, AppContext};
    use std::cell::Cell;

    struct InputOwner(Rc<Cell<usize>>);
    impl gpui::Render for InputOwner {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let mutations = self.0.clone();
            gpui::div()
                .size_full()
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    mutations.set(mutations.get() + 1);
                })
        }
    }

    #[derive(Default)]
    struct TestWindowRegistrations(Vec<(String, u64)>);

    impl Drop for TestWindowRegistrations {
        fn drop(&mut self) {
            // TestAppContext teardown does not guarantee window-close callbacks on panic.
            // Remove only lifetimes owned by this scope, never a reopened replacement.
            for (id, generation) in self.0.iter().rev() {
                crate::windows::remove_runtime_window_instance(id, *generation);
            }
        }
    }

    fn mount(
        cx: &mut gpui::App,
        registrations: &mut TestWindowRegistrations,
        id: &str,
        kind: AutomationWindowKind,
        parent: Option<&AutomationWindowInfo>,
    ) -> (gpui::AnyWindowHandle, AutomationWindowInfo, Rc<Cell<usize>>) {
        let count = Rc::new(Cell::new(0));
        let handle = cx
            .open_window(
                gpui::WindowOptions {
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| InputOwner(count.clone())),
            )
            .unwrap();
        let info = crate::windows::register_runtime_window_instance(
            AutomationWindowInfo {
                id: id.into(),
                kind,
                title: None,
                focused: false,
                visible: false,
                semantic_surface: None,
                bounds: None,
                parent_window_id: parent.map(|info| info.id.clone()),
                parent_window_generation: parent.and_then(|info| info.generation),
                parent_kind: parent.map(|info| info.kind),
                generation: None,
                pid: None,
            },
            handle.into(),
            cx,
        )
        .unwrap();
        registrations
            .0
            .push((info.id.clone(), info.generation.unwrap()));
        (handle.into(), info, count)
    }

    fn input(request_id: &str, info: &AutomationWindowInfo) -> Message {
        // Same serde request contract consumed by stdin; no injected response or mock dispatcher.
        serde_json::from_value(serde_json::json!({
            "type":"simulateGpuiEvent", "requestId":request_id,
            "target":{"type":"instance","id":info.id,"generation":info.generation},
            "event":{"type":"mouseDown","x":10,"y":10},
            "deadlineUnixMs":unix_ms() + 5_000,
        }))
        .unwrap()
    }

    fn settle(cx: &gpui::TestAppContext) {
        cx.run_until_parked();
        cx.background_executor
            .advance_clock(Duration::from_millis(2));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn test_registration_teardown_preserves_replacements_and_survives_panic(
        cx: &mut gpui::TestAppContext,
    ) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut original = TestWindowRegistrations::default();
        let (handle, info, _) = cx.update(|cx| {
            mount(
                cx,
                &mut original,
                "cleanup-reopened",
                AutomationWindowKind::Main,
                None,
            )
        });
        cx.update(|cx| {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        let mut retained = TestWindowRegistrations::default();
        let (replacement, replacement_info, _) =
            cx.update(|cx| mount(cx, &mut retained, &info.id, info.kind, None));
        drop(original);
        assert_eq!(
            crate::windows::get_runtime_window_handle_for_generation(
                &replacement_info.id,
                replacement_info.generation.unwrap(),
            ),
            Some(replacement)
        );

        let mut unwound = TestWindowRegistrations::default();
        let (_, unwound_info, _) = cx.update(|cx| {
            mount(
                cx,
                &mut unwound,
                "cleanup-unwound",
                AutomationWindowKind::Dictation,
                None,
            )
        });
        let result = std::panic::catch_unwind(move || {
            let _registrations = unwound;
            panic!("exercise registration cleanup during a failed assertion");
        });
        assert!(result.is_err());
        assert!(crate::windows::get_runtime_window_handle(&unwound_info.id).is_none());
        assert!(
            crate::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
                id: unwound_info.id,
            }))
            .is_err()
        );
        assert_eq!(
            crate::windows::get_runtime_window_handle_for_generation(
                &replacement_info.id,
                replacement_info.generation.unwrap(),
            ),
            Some(replacement),
            "another scope's registration must survive panic cleanup"
        );
        assert!(crate::windows::resolve_automation_window(Some(
            &AutomationWindowTarget::Instance {
                id: replacement_info.id.clone(),
                generation: replacement_info.generation.unwrap(),
            }
        ))
        .is_ok());

        drop(retained);
        assert!(crate::windows::get_runtime_window_handle(&replacement_info.id).is_none());
        assert!(
            crate::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
                id: replacement_info.id,
            }))
            .is_err()
        );
    }

    #[gpui::test]
    fn production_response_owner_completes_main_chat_dictation_and_popups_once(
        cx: &mut gpui::TestAppContext,
    ) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut registrations = TestWindowRegistrations::default();
        let main = cx.update(|cx| {
            mount(
                cx,
                &mut registrations,
                "deferred-main",
                AutomationWindowKind::Main,
                None,
            )
        });
        let mut windows = vec![main];
        for kind in [
            AutomationWindowKind::AgentChatDetached,
            AutomationWindowKind::Dictation,
            AutomationWindowKind::PromptPopup,
            AutomationWindowKind::ActionsDialog,
        ] {
            let parent = is_attached_surface(kind).then(|| windows[0].1.clone());
            windows.push(cx.update(|cx| {
                mount(
                    cx,
                    &mut registrations,
                    &format!("deferred-{}", kind.as_camel_case()),
                    kind,
                    parent.as_ref(),
                )
            }));
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(32);
        for (index, (handle, info, count)) in windows.iter().enumerate() {
            let id = format!("complete-{index}");
            // Enter through the response owner while that target window is borrowed,
            // exactly the reentrancy condition in the ordinary stdin handler.
            cx.update(|cx| {
                handle
                    .update(cx, |_, _, cx| {
                        handle_gpui_event_message(input(&id, info), tx.clone(), None, cx);
                        assert_eq!(
                            count.get(),
                            0,
                            "dispatch must wait for the current window borrow"
                        );
                        assert!(rx.try_recv().is_err(), "scheduling must not emit a reply");
                    })
                    .unwrap()
            });
            settle(cx);
            let response = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
            assert_eq!(response["requestId"], id);
            assert_eq!(response["success"], true);
            assert_eq!(response["dispatchCompleted"], true);
            assert_eq!(response["dispatchScheduled"], false);
            assert_eq!(response["wasDeferred"], true);
            assert_eq!(response["activationProof"], "not_observed");
            assert_eq!(count.get(), 1, "real GPUI handler must run once");
            cx.update(|cx| {
                handle_gpui_event_message(input(&id, info), tx.clone(), None, cx);
                handle_gpui_event_message(
                    Message::CancelGpuiEvent {
                        request_id: id.clone(),
                    },
                    tx.clone(),
                    None,
                    cx,
                );
            });
            settle(cx);
            assert_eq!(count.get(), 1);
            assert!(
                rx.try_recv().is_err(),
                "duplicates/cancel-after-completion cannot reply again"
            );
        }
        for (handle, _, _) in windows.into_iter().rev() {
            cx.update(|cx| {
                handle
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap()
            });
        }
    }

    #[gpui::test]
    fn cancellation_and_deadline_prevent_late_mutation_on_each_dispatch_kind(
        cx: &mut gpui::TestAppContext,
    ) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut registrations = TestWindowRegistrations::default();
        for kind in [
            AutomationWindowKind::Main,
            AutomationWindowKind::AgentChatDetached,
            AutomationWindowKind::Dictation,
            AutomationWindowKind::PromptPopup,
            AutomationWindowKind::ActionsDialog,
        ] {
            let (handle, info, count) = cx.update(|cx| {
                mount(
                    cx,
                    &mut registrations,
                    &format!("cancel-{}", kind.as_camel_case()),
                    kind,
                    None,
                )
            });
            let (tx, rx) = std::sync::mpsc::sync_channel(8);
            let id = format!("cancel-{}", info.id);
            cx.update(|cx| {
                handle_gpui_event_message(input(&id, &info), tx.clone(), None, cx);
                handle_gpui_event_message(
                    Message::CancelGpuiEvent {
                        request_id: id.clone(),
                    },
                    tx.clone(),
                    None,
                    cx,
                );
                handle_gpui_event_message(
                    Message::CancelGpuiEvent {
                        request_id: id.clone(),
                    },
                    tx.clone(),
                    None,
                    cx,
                );
            });
            settle(cx);
            let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
            assert_eq!(reply["requestId"], id);
            assert_eq!(reply["errorCode"], "dispatch_cancelled");
            assert_eq!(reply["success"], false);
            assert_eq!(reply["dispatchCompleted"], false);
            assert_eq!(reply["dispatchScheduled"], false);
            assert_eq!(reply["wasDeferred"], true);
            assert_eq!(count.get(), 0);
            assert!(rx.try_recv().is_err());
            let id = format!("expired-{}", info.id);
            let mut request = input(&id, &info);
            if let Message::SimulateGpuiEvent {
                deadline_unix_ms, ..
            } = &mut request
            {
                *deadline_unix_ms = Some(unix_ms() + 100);
            }
            let dispatcher = cx.dispatcher.clone();
            cx.update(|cx| {
                handle_gpui_event_message(request, tx.clone(), None, cx);
                // Hold the foreground callback until its delivery deadline has
                // elapsed, without executing it in an intermediate update.
                dispatcher.advance_clock_without_running(Duration::from_millis(200));
            });
            settle(cx);
            let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
            assert_eq!(reply["requestId"], id);
            assert_eq!(reply["errorCode"], "dispatch_deadline_exceeded");
            assert_eq!(reply["success"], false);
            assert_eq!(reply["dispatchCompleted"], false);
            assert_eq!(reply["dispatchScheduled"], false);
            assert_eq!(reply["wasDeferred"], true);
            assert_eq!(count.get(), 0);
            assert!(rx.try_recv().is_err());
            cx.update(|cx| {
                handle
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap()
            });
        }
    }

    #[gpui::test]
    fn queued_input_rejects_reopened_target_and_dropped_completion_receiver(
        cx: &mut gpui::TestAppContext,
    ) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut registrations = TestWindowRegistrations::default();
        let (handle, info, first_count) = cx.update(|cx| {
            mount(
                cx,
                &mut registrations,
                "queued-reopen",
                AutomationWindowKind::Main,
                None,
            )
        });
        let (tx, rx) = std::sync::mpsc::sync_channel(8);
        let (replacement, replacement_info, replacement_count) = cx.update(|cx| {
            handle_gpui_event_message(input("reopened", &info), tx.clone(), None, cx);
            handle
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            mount(cx, &mut registrations, &info.id, info.kind, None)
        });
        settle(cx);
        let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
        assert_eq!(reply["errorCode"], "stale_target");
        assert_eq!(first_count.get(), 0);
        assert_eq!(replacement_count.get(), 0);
        cx.update(|cx| {
            let target = AutomationWindowTarget::Instance {
                id: replacement_info.id,
                generation: replacement_info.generation.unwrap(),
            };
            let event = crate::protocol::SimulatedGpuiEvent::MouseDown {
                x: 10.0,
                y: 10.0,
                button: None,
            };
            drop(dispatch_gpui_event(
                "dropped-receiver",
                Some(&target),
                event,
                None,
                None,
                cx,
            ));
        });
        settle(cx);
        assert_eq!(replacement_count.get(), 0);
        assert!(rx.try_recv().is_err());
        cx.update(|cx| {
            replacement
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
    }

    #[gpui::test]
    fn owner_preconditions_run_at_delivery_and_parent_reopen_cannot_retarget(
        cx: &mut gpui::TestAppContext,
    ) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut registrations = TestWindowRegistrations::default();
        let (parent, parent_info, _) = cx.update(|cx| {
            mount(
                cx,
                &mut registrations,
                "queued-parent",
                AutomationWindowKind::Main,
                None,
            )
        });
        let (child, info, count) = cx.update(|cx| {
            mount(
                cx,
                &mut registrations,
                "queued-child",
                AutomationWindowKind::PromptPopup,
                Some(&parent_info),
            )
        });
        let (tx, rx) = std::sync::mpsc::sync_channel(8);
        let revision = Rc::new(Cell::new(1));
        let at_delivery = revision.clone();
        let precondition: GpuiDispatchPrecondition = Box::new(move |_, _, _| {
            if at_delivery.get() == 1 {
                Ok(())
            } else {
                Err("stale_target_identity".into())
            }
        });
        cx.update(|cx| {
            handle_gpui_event_message(
                input("revision-changed", &info),
                tx.clone(),
                Some(precondition),
                cx,
            );
            revision.set(2);
        });
        settle(cx);
        let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
        assert_eq!(reply["errorCode"], "stale_target_identity");
        assert_eq!(count.get(), 0);
        let (new_parent, _, _) = cx.update(|cx| {
            handle_gpui_event_message(input("parent-reopened", &info), tx.clone(), None, cx);
            parent
                .update(cx, |_, window, _| window.remove_window())
                .unwrap();
            mount(
                cx,
                &mut registrations,
                &parent_info.id,
                parent_info.kind,
                None,
            )
        });
        settle(cx);
        let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
        assert_eq!(reply["errorCode"], "stale_target");
        assert_eq!(count.get(), 0);
        assert!(rx.try_recv().is_err());
        cx.update(|cx| {
            child
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
        cx.update(|cx| {
            new_parent
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
    }

    #[gpui::test]
    fn full_response_channel_retains_the_single_terminal_result(cx: &mut gpui::TestAppContext) {
        let _guard = crate::windows::automation_registry::tests::registry_guard();
        let mut registrations = TestWindowRegistrations::default();
        let (handle, info, count) = cx.update(|cx| {
            mount(
                cx,
                &mut registrations,
                "deferred-backpressure",
                AutomationWindowKind::Main,
                None,
            )
        });
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        tx.send(Message::ListAutomationWindows {
            request_id: "occupied-channel".into(),
        })
        .unwrap();
        cx.update(|cx| {
            handle_gpui_event_message(input("backpressure", &info), tx.clone(), None, cx)
        });
        settle(cx);
        assert_eq!(count.get(), 1);
        assert_eq!(
            rx.try_recv().unwrap().request_id(),
            Some("occupied-channel")
        );
        settle(cx);
        let reply = serde_json::to_value(rx.try_recv().unwrap()).unwrap();
        assert_eq!(reply["requestId"], "backpressure");
        assert_eq!(reply["dispatchCompleted"], true);
        assert_eq!(count.get(), 1);
        assert!(rx.try_recv().is_err());
        cx.update(|cx| {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
    }
}

fn parse_mouse_button(button: Option<&str>) -> gpui::MouseButton {
    match button {
        Some("right") => gpui::MouseButton::Right,
        Some("middle") => gpui::MouseButton::Middle,
        _ => gpui::MouseButton::Left,
    }
}
