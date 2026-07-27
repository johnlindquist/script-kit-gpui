#[cfg(target_os = "macos")]
use std::sync::Arc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    LazyLock, Mutex,
};
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;

use anyhow::{anyhow, Result};
use gpui::{App, AsyncApp};

use super::ax::{get_window_position, get_window_size};
use super::query::{get_frontmost_window_of_previous_app, has_accessibility_permission};
use super::snap_mode::{current_snap_mode, SnapMode};
use super::snap_runtime::{finish_snap_runtime, is_snap_runtime_active, start_snap_runtime};
use super::types::{Bounds, WindowInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapMonitorEvent {
    Pressed,
    Dragged,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DragArmState {
    window_id: u32,
    bounds: Bounds,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
/// At most one drag-time AX poll in flight.
static DRAG_POLL_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static DRAG_ARM_STATE: LazyLock<Mutex<Option<DragArmState>>> = LazyLock::new(|| Mutex::new(None));
static SNAP_MONITOR_CHANNEL: LazyLock<(
    async_channel::Sender<SnapMonitorEvent>,
    async_channel::Receiver<SnapMonitorEvent>,
)> = LazyLock::new(|| async_channel::bounded(128));

fn origin_changed(previous: Bounds, current: Bounds) -> bool {
    previous.x != current.x || previous.y != current.y
}

fn arm_state_for_window(window: &WindowInfo) -> DragArmState {
    DragArmState {
        window_id: window.id,
        bounds: window.bounds,
    }
}

fn poll_armed_window_bounds(window_id: u32) -> Option<Bounds> {
    // Poll path: registry lookup only, no refresh — this runs at 16 ms cadence.
    let handle = super::registry::resolve_legacy_window_id(window_id).ok()?;
    let window = super::registry::retained_window(handle).ok()?;
    let (x, y) = get_window_position(window.as_ptr()).ok()?;
    let (width, height) = get_window_size(window.as_ptr()).ok()?;
    Some(Bounds::new(x, y, width, height))
}

fn should_start_runtime(armed: DragArmState, current_bounds: Option<Bounds>) -> bool {
    matches!(
        current_bounds,
        Some(current_bounds) if origin_changed(armed.bounds, current_bounds)
    )
}

fn handle_snap_monitor_event(event: SnapMonitorEvent, cx: &mut App) -> Result<()> {
    if !has_accessibility_permission() {
        return Ok(());
    }

    // Gate: when snap mode is Off, ignore press/drag (but still handle release
    // to clean up a runtime that was started before the mode changed).
    if current_snap_mode() == SnapMode::Off && !matches!(event, SnapMonitorEvent::Released) {
        return Ok(());
    }

    match event {
        SnapMonitorEvent::Pressed => {
            if is_snap_runtime_active() {
                return Ok(());
            }

            // Previous-app resolution performs AX reads: run it OFF the GPUI
            // thread. The arm generation invalidates a late resolution after
            // release.
            *super::snap_lock(&DRAG_ARM_STATE, "monitor arm")? = None;
            let arm_generation = super::snap_runtime::snap_arm_generation();
            cx.spawn(async move |cx: &mut AsyncApp| {
                let armed = cx
                    .background_executor()
                    .spawn(async move {
                        get_frontmost_window_of_previous_app()
                            .ok()
                            .flatten()
                            .as_ref()
                            .map(arm_state_for_window)
                    })
                    .await;
                if super::snap_runtime::snap_arm_generation() != arm_generation {
                    return; // released before resolution completed
                }
                if let (Some(armed), Ok(mut guard)) =
                    (armed, super::snap_lock(&DRAG_ARM_STATE, "monitor arm"))
                {
                    *guard = Some(armed);
                }
            })
            .detach();
        }
        SnapMonitorEvent::Dragged => {
            if is_snap_runtime_active() {
                return Ok(());
            }

            let armed = *super::snap_lock(&DRAG_ARM_STATE, "monitor arm")?;
            let Some(armed) = armed else {
                return Ok(());
            };

            // One in-flight poll; AX read happens on the background executor.
            if DRAG_POLL_IN_FLIGHT.swap(true, Ordering::SeqCst) {
                return Ok(());
            }
            let arm_generation = super::snap_runtime::snap_arm_generation();
            cx.spawn(async move |cx: &mut AsyncApp| {
                let current_bounds = cx
                    .background_executor()
                    .spawn(async move { poll_armed_window_bounds(armed.window_id) })
                    .await;
                DRAG_POLL_IN_FLIGHT.store(false, Ordering::SeqCst);
                cx.update(|cx| {
                    if super::snap_runtime::snap_arm_generation() != arm_generation
                        || is_snap_runtime_active()
                    {
                        return;
                    }
                    if should_start_runtime(armed, current_bounds) {
                        if let Ok(mut guard) = super::snap_lock(&DRAG_ARM_STATE, "monitor arm") {
                            *guard = None;
                        }
                        tracing::info!(
                            target: "script_kit::snap_monitor",
                            event = "snap_drag_started",
                            window_id = armed.window_id,
                            "detected external window drag"
                        );
                        let _ = start_snap_runtime(cx);
                    } else if current_bounds.is_none() {
                        if let Ok(mut guard) = super::snap_lock(&DRAG_ARM_STATE, "monitor arm") {
                            *guard = None;
                        }
                    }
                });
            })
            .detach();
        }
        SnapMonitorEvent::Released => {
            // Invalidate any in-flight arm resolution or poll decision.
            super::snap_runtime::bump_snap_arm_generation();
            *super::snap_lock(&DRAG_ARM_STATE, "monitor arm")? = None;

            if is_snap_runtime_active() {
                tracing::info!(
                    target: "script_kit::snap_monitor",
                    event = "snap_drag_released",
                    "detected external drag release"
                );
                finish_snap_runtime(cx)?;
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
struct SendableMachPortRef(Option<core_foundation::mach_port::CFMachPortRef>);

#[cfg(target_os = "macos")]
// SAFETY: The mach port ref is only accessed on the monitor thread or through a mutex
// when re-enabling the event tap after timeout/user-input disablement.
unsafe impl Send for SendableMachPortRef {}

#[cfg(target_os = "macos")]
// SAFETY: CFMachPortRef is a Core Foundation reference type guarded by a mutex here.
unsafe impl Sync for SendableMachPortRef {}

#[cfg(target_os = "macos")]
fn install_global_mouse_monitor() -> Result<()> {
    if !has_accessibility_permission() {
        return Ok(());
    }

    thread::Builder::new()
        .name("snap-mouse-monitor".to_string())
        .spawn(run_global_mouse_monitor)
        .map(|_| ())
        .map_err(|error| anyhow!("failed to spawn snap mouse monitor thread: {error}"))
}

#[cfg(not(target_os = "macos"))]
fn install_global_mouse_monitor() -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_global_mouse_monitor() {
    use core_foundation::base::TCFType;
    use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
    use core_graphics::event::{
        CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventType,
    };

    let current_run_loop = CFRunLoop::get_current();
    let mach_port_ref: Arc<std::sync::Mutex<SendableMachPortRef>> =
        Arc::new(std::sync::Mutex::new(SendableMachPortRef(None)));
    let mach_port_for_callback = Arc::clone(&mach_port_ref);
    let sender = SNAP_MONITOR_CHANNEL.0.clone();

    let event_tap = match CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::LeftMouseDragged,
        ],
        move |_proxy, event_type, _event: &CGEvent| {
            match event_type {
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    reenable_mouse_tap(&mach_port_for_callback);
                    return None;
                }
                _ => {}
            }

            if let Some(event) = snap_event_from_cg_event_type(event_type) {
                let _ = sender.try_send(event);
            }

            None
        },
    ) {
        Ok(tap) => tap,
        Err(()) => {
            tracing::warn!(
                target: "script_kit::snap_monitor",
                event = "snap_drag_monitor_tap_create_failed",
                "failed to create snap mouse event tap"
            );
            return;
        }
    };

    if let Ok(mut guard) = mach_port_ref.lock() {
        guard.0 = Some(event_tap.mach_port.as_concrete_TypeRef());
    } else {
        tracing::warn!(
            target: "script_kit::snap_monitor",
            event = "snap_drag_monitor_tap_store_failed",
            "failed to store snap mouse event tap handle"
        );
        return;
    }

    let run_loop_source = match event_tap.mach_port.create_runloop_source(0) {
        Ok(source) => source,
        Err(()) => {
            tracing::warn!(
                target: "script_kit::snap_monitor",
                event = "snap_drag_monitor_runloop_source_failed",
                "failed to create snap mouse event tap run loop source"
            );
            return;
        }
    };

    // SAFETY: The run loop source was created from a valid event tap and is added
    // to the current thread's run loop in a common mode for event delivery.
    unsafe {
        current_run_loop.add_source(&run_loop_source, kCFRunLoopCommonModes);
    }
    event_tap.enable();

    loop {
        let _ = CFRunLoop::run_in_mode(
            // SAFETY: kCFRunLoopDefaultMode is a valid Core Foundation constant.
            unsafe { kCFRunLoopDefaultMode },
            Duration::from_millis(250),
            true,
        );
    }
}

#[cfg(target_os = "macos")]
fn snap_event_from_cg_event_type(
    event_type: core_graphics::event::CGEventType,
) -> Option<SnapMonitorEvent> {
    use core_graphics::event::CGEventType;

    match event_type {
        CGEventType::LeftMouseDown => Some(SnapMonitorEvent::Pressed),
        CGEventType::LeftMouseDragged => Some(SnapMonitorEvent::Dragged),
        CGEventType::LeftMouseUp => Some(SnapMonitorEvent::Released),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn reenable_mouse_tap(mach_port_ref: &Arc<std::sync::Mutex<SendableMachPortRef>>) {
    extern "C" {
        fn CGEventTapEnable(tap: core_foundation::mach_port::CFMachPortRef, enable: bool);
    }

    if let Ok(guard) = mach_port_ref.lock() {
        if let Some(port) = guard.0 {
            // SAFETY: `port` is the valid mach port backing the CGEventTap.
            unsafe {
                CGEventTapEnable(port, true);
            }
        }
    }
}

/// Install the snap drag monitor that detects external window drags and
/// drives the desktop snap overlay lifecycle (start on drag, finish on release).
pub fn install_snap_drag_monitor(cx: &mut App) -> Result<()> {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        tracing::info!(
            target: "script_kit::snap_monitor",
            event = "snap_drag_monitor_already_installed",
            "snap drag monitor already installed"
        );
        return Ok(());
    }

    install_global_mouse_monitor()?;

    tracing::info!(
        target: "script_kit::snap_monitor",
        event = "snap_drag_monitor_installed",
        mode = "global_mouse_monitor",
        "installed snap drag monitor"
    );

    let rx = SNAP_MONITOR_CHANNEL.1.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(event) = rx.recv().await {
            cx.update(|cx| {
                if let Err(error) = handle_snap_monitor_event(event, cx) {
                    tracing::warn!(
                        target: "script_kit::snap_monitor",
                        event = "snap_drag_monitor_event_failed",
                        ?event,
                        %error,
                        "snap drag monitor event failed"
                    );
                }
            });
        }
    })
    .detach();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_when_armed_window_origin_changes() {
        let armed = DragArmState {
            window_id: 7,
            bounds: Bounds::new(900, 100, 1200, 800),
        };
        assert!(should_start_runtime(
            armed,
            Some(Bounds::new(750, 100, 1200, 800))
        ));
    }

    #[test]
    fn does_not_start_on_size_only_change() {
        let armed = DragArmState {
            window_id: 7,
            bounds: Bounds::new(900, 100, 1200, 800),
        };
        assert!(!should_start_runtime(
            armed,
            Some(Bounds::new(900, 100, 1400, 800))
        ));
    }

    #[test]
    fn does_not_start_without_origin_change() {
        let armed = DragArmState {
            window_id: 7,
            bounds: Bounds::new(900, 100, 1200, 800),
        };
        assert!(!should_start_runtime(
            armed,
            Some(Bounds::new(900, 100, 1200, 800))
        ));
    }

    #[test]
    fn does_not_start_without_window() {
        let armed = DragArmState {
            window_id: 7,
            bounds: Bounds::new(900, 100, 1200, 800),
        };
        assert!(!should_start_runtime(armed, None));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn maps_mouse_event_types() {
        use core_graphics::event::CGEventType;

        assert_eq!(
            snap_event_from_cg_event_type(CGEventType::LeftMouseDown),
            Some(SnapMonitorEvent::Pressed)
        );
        assert_eq!(
            snap_event_from_cg_event_type(CGEventType::LeftMouseDragged),
            Some(SnapMonitorEvent::Dragged)
        );
        assert_eq!(
            snap_event_from_cg_event_type(CGEventType::LeftMouseUp),
            Some(SnapMonitorEvent::Released)
        );
    }
}
