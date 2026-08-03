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
    /// Raw event dispatch is not handler activation proof.
    pub activation_proof: Option<String>,
}

/// Apply a [`SimulatedGpuiEvent`] to a live GPUI [`gpui::Window`] via the real input pipeline.
fn apply_simulated_event(
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
                    *x,
                    *y,
                    *delta_x,
                    *delta_y,
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
    x: f64,
    y: f64,
    delta_x: f64,
    delta_y: f64,
    touch_phase: crate::protocol::SimulatedTouchPhase,
    direct_phase: Option<crate::protocol::SimulatedScrollPhase>,
    momentum_phase: Option<crate::protocol::SimulatedScrollPhase>,
    timestamp_seconds: Option<f64>,
) -> gpui::ScrollWheelEvent {
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
                11.0,
                22.0,
                -1.25,
                3.5,
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
            0.0,
            0.0,
            0.0,
            0.0,
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

/// Dispatch a simulated event through an [`AnyWindowHandle`] via the real GPUI
/// input pipeline and return the result.
fn dispatch_with_any_handle(
    handle: gpui::AnyWindowHandle,
    request_id: &str,
    resolved_id: &str,
    event_type: &str,
    event: &crate::protocol::SimulatedGpuiEvent,
    cx: &mut gpui::App,
    dispatch_path: &str,
) -> GpuiEventDispatchResult {
    let dispatch_result = handle.update(cx, |_root, window, cx| {
        apply_simulated_event(window, cx, event);
    });

    match dispatch_result {
        Ok(()) => {
            tracing::info!(
                target: "script_kit::automation",
                request_id = %request_id,
                window_id = %resolved_id,
                event_type = %event_type,
                "gpui_event_simulation.dispatch_exact_complete"
            );
            GpuiEventDispatchResult {
                success: true,
                error_code: None,
                error: None,
                dispatch_path: Some(dispatch_path.to_string()),
                resolved_window_id: Some(resolved_id.to_string()),
                dispatch_completed: true,
                dispatch_scheduled: false,
                activation_proof: Some("not_observed".to_string()),
            }
        }
        Err(err) => {
            let msg = format!("GPUI dispatch failed: {err}");
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                window_id = %resolved_id,
                error = %msg,
                "gpui_event_simulation.failed"
            );
            GpuiEventDispatchResult {
                success: false,
                error_code: Some("dispatch_failed".to_string()),
                error: Some(msg),
                dispatch_path: Some(dispatch_path.to_string()),
                resolved_window_id: Some(resolved_id.to_string()),
                dispatch_completed: false,
                dispatch_scheduled: false,
                activation_proof: None,
            }
        }
    }
}

/// Defer exact-handle dispatch until the current GPUI update stack unwinds.
///
/// Stdin protocol messages are processed while the main app entity is being
/// updated. Detached surfaces can still route back through shared app state
/// during key dispatch, so exact-handle dispatch must be able to hop a tick
/// instead of synchronously re-entering `ScriptListApp`.
fn dispatch_with_any_handle_deferred(
    handle: gpui::AnyWindowHandle,
    request_id: &str,
    resolved_id: &str,
    event_type: &str,
    event: &crate::protocol::SimulatedGpuiEvent,
    cx: &mut gpui::App,
    dispatch_path: &str,
) -> GpuiEventDispatchResult {
    let request_id_owned = request_id.to_string();
    let resolved_id_owned = resolved_id.to_string();
    let event_type_owned = event_type.to_string();
    let event_for_spawn = event.clone();
    let deferred_path = format!("{dispatch_path}_deferred");
    let deferred_path_for_spawn = deferred_path.clone();

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        use gpui::AppContext;
        cx.background_executor()
            .timer(std::time::Duration::from_millis(1))
            .await;
        let update_result = cx.update_window(handle, |_root, window, cx| {
            apply_simulated_event(window, cx, &event_for_spawn);
        });
        match update_result {
            Ok(()) => {
                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id_owned,
                    window_id = %resolved_id_owned,
                    event_type = %event_type_owned,
                    dispatch_path = %deferred_path_for_spawn,
                    "gpui_event_simulation.exact_deferred_complete"
                );
            }
            Err(err) => {
                tracing::warn!(
                    target: "script_kit::automation",
                    request_id = %request_id_owned,
                    window_id = %resolved_id_owned,
                    error = %err,
                    dispatch_path = %deferred_path_for_spawn,
                    "gpui_event_simulation.exact_deferred_failed"
                );
            }
        }
    })
    .detach();

    tracing::info!(
        target: "script_kit::automation",
        request_id = %request_id,
        window_id = %resolved_id,
        dispatch_path = %deferred_path,
        "gpui_event_simulation.exact_deferred_scheduled"
    );

    GpuiEventDispatchResult {
        success: true,
        error_code: None,
        error: None,
        dispatch_path: Some(deferred_path),
        resolved_window_id: Some(resolved_id.to_string()),
        dispatch_completed: false,
        dispatch_scheduled: true,
        activation_proof: Some("not_observed".to_string()),
    }
}

/// Dispatch a [`SimulatedGpuiEvent`] to the resolved target window
/// through GPUI's real input pipeline.
///
/// **Exact-handle dispatch**: If the resolved automation target has a
/// registered runtime handle (via [`upsert_runtime_window_handle`]),
/// events are dispatched directly to that exact window.  This allows
/// two visible detached Agent Chat windows to be targeted independently by
/// exact ID.
///
/// **Fallback dispatch**: When no exact runtime handle exists, the
/// function maps the resolved kind to a [`WindowRole`] and dispatches
/// through the unified window registry.  If more than one visible
/// window shares the same kind and no exact handle is available,
/// dispatch fails closed with `target_ambiguous`.
///
/// # Tracing
///
/// Emits structured logs at `script_kit::automation` with:
/// - `request_id`, `window_id`, `kind`, `event_type` on entry
/// - `dispatch_exact_handle` when using the runtime handle registry
/// - `runtime_handle_missing` when falling back to WindowRole
/// - `dispatch_exact_complete` / `failed` on completion
pub(crate) fn dispatch_gpui_event(
    request_id: &str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
    event: &crate::protocol::SimulatedGpuiEvent,
    cx: &mut gpui::App,
) -> GpuiEventDispatchResult {
    use crate::protocol::SimulatedGpuiEvent;

    // 1. Resolve the target window via the automation metadata registry.
    let resolved = match crate::windows::resolve_automation_window(target) {
        Ok(info) => info,
        Err(err) => {
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                error = %err,
                "gpui_event_simulation.target_failed"
            );
            return GpuiEventDispatchResult {
                success: false,
                error_code: Some("target_not_found".to_string()),
                error: Some(err.to_string()),
                dispatch_path: None,
                resolved_window_id: None,
                dispatch_completed: false,
                dispatch_scheduled: false,
                activation_proof: None,
            };
        }
    };

    let event_type = match event {
        SimulatedGpuiEvent::KeyDown { .. } => "keyDown",
        SimulatedGpuiEvent::MouseMove { .. } => "mouseMove",
        SimulatedGpuiEvent::MouseDown { .. } => "mouseDown",
        SimulatedGpuiEvent::MouseUp { .. } => "mouseUp",
        SimulatedGpuiEvent::MouseClick { .. } => "mouseClick",
        SimulatedGpuiEvent::ScrollWheel { .. } => "scrollWheel",
    };

    tracing::info!(
        target: "script_kit::automation",
        request_id = %request_id,
        window_id = %resolved.id,
        kind = ?resolved.kind,
        event_type = %event_type,
        "gpui_event_simulation.dispatch"
    );

    // 2. Try exact runtime handle first — this preserves the resolved
    //    automation window identity all the way through GPUI dispatch,
    //    so two detached Agent Chat windows can be targeted independently. Attached
    //    popup exact handles also receive popup-local mouse coordinates; only
    //    the parent-window fallback below rebases into parent dispatch space.
    let exact_runtime_handle = match resolved.generation {
        Some(generation) => crate::windows::get_valid_runtime_window_handle_for_generation(
            &resolved.id,
            generation,
            cx,
        ),
        None => crate::windows::get_valid_runtime_window_handle(&resolved.id, cx),
    };
    if let Some(handle) = exact_runtime_handle {
        tracing::info!(
            target: "script_kit::automation",
            request_id = %request_id,
            window_id = %resolved.id,
            "gpui_event_simulation.dispatch_exact_handle"
        );
        if matches!(
            resolved.kind,
            crate::protocol::AutomationWindowKind::AgentChatDetached
                | crate::protocol::AutomationWindowKind::Dictation
                | crate::protocol::AutomationWindowKind::PromptPopup
        ) {
            return dispatch_with_any_handle_deferred(
                handle,
                request_id,
                &resolved.id,
                event_type,
                event,
                cx,
                "exact_handle",
            );
        }

        return dispatch_with_any_handle(
            handle,
            request_id,
            &resolved.id,
            event_type,
            event,
            cx,
            "exact_handle",
        );
    }

    if resolved.generation.is_some() {
        return GpuiEventDispatchResult {
            success: false,
            error_code: Some("stale_or_missing_instance_handle".to_string()),
            error: Some(format!(
                "Exact automation instance {} generation {:?} has no matching live runtime handle",
                resolved.id, resolved.generation
            )),
            dispatch_path: None,
            resolved_window_id: Some(resolved.id.clone()),
            dispatch_completed: false,
            dispatch_scheduled: false,
            activation_proof: None,
        };
    }

    // Rebase mouse coordinates for attached surfaces (ActionsDialog, PromptPopup)
    // only when falling back to the parent window. Exact popup handles above use
    // popup-local coordinates directly.
    let event = match rebase_mouse_event_to_dispatch_space(&resolved, event) {
        Ok(rebased) => rebased,
        Err(msg) => {
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                window_id = %resolved.id,
                error = %msg,
                "gpui_event_simulation.coordinate_translation_failed"
            );
            return GpuiEventDispatchResult {
                success: false,
                error_code: Some("coordinate_translation_failed".to_string()),
                error: Some(msg),
                dispatch_path: None,
                resolved_window_id: Some(resolved.id.clone()),
                dispatch_completed: false,
                dispatch_scheduled: false,
                activation_proof: None,
            };
        }
    };

    // 2b. Main-window re-acquisition: the runtime handle registry only gets
    //     upserted on show/hide transitions, and `handle.update()` returns
    //     `Err` when the target is already on GPUI's update stack (reentrancy
    //     when stdin commands run inside `window_for_stdin.update`). For the
    //     Main window we always have a durable handle in the `MAIN_WINDOW_HANDLE`
    //     global, so re-acquire it and dispatch — deferring via `cx.spawn`
    //     when we detect reentrancy so the outer update can finish first.
    if matches!(resolved.kind, crate::protocol::AutomationWindowKind::Main) {
        if let Some(handle) = crate::get_main_window_handle() {
            crate::windows::upsert_runtime_window_handle(resolved.id.clone(), handle);
            match handle.update(cx, |_, _, _| {}) {
                Ok(_) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        window_id = %resolved.id,
                        "gpui_event_simulation.main_reacquired_global"
                    );
                    return dispatch_with_any_handle(
                        handle,
                        request_id,
                        &resolved.id,
                        event_type,
                        &event,
                        cx,
                        "main_reacquire_global",
                    );
                }
                Err(_) => {
                    // Reentrancy: we're inside a nested window update for Main
                    // (stdin dispatcher holds `window_for_stdin.update`). Defer
                    // so the outer update finishes, then dispatch against the
                    // live handle. Report synchronous success with path
                    // `main_deferred` so the caller can proceed.
                    let request_id_owned = request_id.to_string();
                    let resolved_id_owned = resolved.id.clone();
                    let event_type_owned = event_type.to_string();
                    let event_for_spawn = event.clone();
                    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                        use gpui::AppContext;
                        let update_result = cx.update_window(handle, |_root, window, cx| {
                            apply_simulated_event(window, cx, &event_for_spawn);
                        });
                        match update_result {
                            Ok(()) => {
                                tracing::info!(
                                    target: "script_kit::automation",
                                    request_id = %request_id_owned,
                                    window_id = %resolved_id_owned,
                                    event_type = %event_type_owned,
                                    "gpui_event_simulation.main_deferred_complete"
                                );
                            }
                            Err(err) => {
                                tracing::warn!(
                                    target: "script_kit::automation",
                                    request_id = %request_id_owned,
                                    window_id = %resolved_id_owned,
                                    error = %err,
                                    "gpui_event_simulation.main_deferred_failed"
                                );
                            }
                        }
                    })
                    .detach();

                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        window_id = %resolved.id,
                        "gpui_event_simulation.main_deferred_scheduled"
                    );
                    return GpuiEventDispatchResult {
                        success: true,
                        error_code: None,
                        error: None,
                        dispatch_path: Some("main_deferred".to_string()),
                        resolved_window_id: Some(resolved.id.clone()),
                        dispatch_completed: false,
                        dispatch_scheduled: true,
                        activation_proof: Some("not_observed".to_string()),
                    };
                }
            }
        }
    }

    // 3. No exact handle — fall back to WindowRole-based dispatch.
    tracing::warn!(
        target: "script_kit::automation",
        request_id = %request_id,
        window_id = %resolved.id,
        kind = ?resolved.kind,
        "gpui_event_simulation.runtime_handle_missing"
    );

    // 3a. Ambiguity guard — fail closed when multiple visible windows
    //     share the same kind and no exact runtime handle is registered.
    let visible_count = visible_window_count_for_kind(resolved.kind);
    if kind_collapses_to_single_window_role(resolved.kind) && visible_count > 1 {
        let msg = format!(
            "Resolved target {} ({:?}) is ambiguous: {} visible windows share this kind \
             and no exact runtime handle is registered",
            resolved.id, resolved.kind, visible_count
        );
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            window_id = %resolved.id,
            kind = ?resolved.kind,
            visible_count = visible_count,
            "gpui_event_simulation.ambiguous_role"
        );
        return GpuiEventDispatchResult {
            success: false,
            error_code: Some("target_ambiguous".to_string()),
            error: Some(msg),
            dispatch_path: None,
            resolved_window_id: Some(resolved.id.clone()),
            dispatch_completed: false,
            dispatch_scheduled: false,
            activation_proof: None,
        };
    }

    // 3b. Map to WindowRole and get the handle.
    let role = match automation_kind_to_window_role(resolved.kind) {
        Some(r) => r,
        None => {
            // For attached surfaces (ActionsDialog, PromptPopup), dispatch
            // to the parent window since they share its GPUI context.
            // Resolve the parent from the popup's recorded metadata.
            let parent_kind = resolved
                .parent_kind
                .unwrap_or(crate::protocol::AutomationWindowKind::Main);
            match automation_kind_to_window_role(parent_kind) {
                Some(r) => r,
                None => {
                    let msg = format!(
                        "Attached surface {} ({:?}) parent kind {:?} has no WindowRole mapping",
                        resolved.id, resolved.kind, parent_kind
                    );
                    tracing::warn!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        error = %msg,
                        "gpui_event_simulation.parent_role_unmapped"
                    );
                    return GpuiEventDispatchResult {
                        success: false,
                        error_code: Some("handle_unavailable".to_string()),
                        error: Some(msg),
                        dispatch_path: None,
                        resolved_window_id: Some(resolved.id.clone()),
                        dispatch_completed: false,
                        dispatch_scheduled: false,
                        activation_proof: None,
                    };
                }
            }
        }
    };

    let handle = match crate::windows::get_valid_window(role, cx) {
        Some(h) => {
            let any: gpui::AnyWindowHandle = h.into();
            any
        }
        None => {
            if role == crate::windows::WindowRole::Main
                && is_attached_surface(resolved.kind)
                && resolved.parent_kind == Some(crate::protocol::AutomationWindowKind::Main)
            {
                if let Some(handle) = crate::get_main_window_handle() {
                    if let Some(parent_id) = resolved.parent_window_id.as_ref() {
                        crate::windows::upsert_runtime_window_handle(parent_id.clone(), handle);
                    }
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        window_id = %resolved.id,
                        parent_window_id = ?resolved.parent_window_id,
                        "gpui_event_simulation.attached_main_reacquired_global"
                    );
                    return dispatch_with_any_handle_deferred(
                        handle,
                        request_id,
                        &resolved.id,
                        event_type,
                        &event,
                        cx,
                        "attached_main_reacquire",
                    );
                }
            }

            let msg = format!(
                "Window handle not available for role {:?} (kind {:?})",
                role, resolved.kind
            );
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                error = %msg,
                "gpui_event_simulation.no_handle"
            );
            return GpuiEventDispatchResult {
                success: false,
                error_code: Some("handle_unavailable".to_string()),
                error: Some(msg),
                dispatch_path: None,
                resolved_window_id: Some(resolved.id.clone()),
                dispatch_completed: false,
                dispatch_scheduled: false,
                activation_proof: None,
            };
        }
    };

    dispatch_with_any_handle(
        handle,
        request_id,
        &resolved.id,
        event_type,
        &event,
        cx,
        "window_role_fallback",
    )
}

fn parse_mouse_button(button: Option<&str>) -> gpui::MouseButton {
    match button {
        Some("right") => gpui::MouseButton::Right,
        Some("middle") => gpui::MouseButton::Middle,
        _ => gpui::MouseButton::Left,
    }
}
