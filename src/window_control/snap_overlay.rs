use std::sync::Mutex;

use anyhow::{Context as _, Result};
use gpui::{
    div, point, px, rgba, size, App, AppContext as _, Context, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, Styled, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions,
};

use super::display::get_native_display_descriptors;
use super::snap_mode::SnapMode;
use super::snap_session::{SnapOverlayModel, SnapOverlayScene, SnapOverlayTarget};
use super::types::Bounds;
use crate::runtime_policy::{ExternalEffect, WindowHostPolicy};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SNAP_OVERLAY_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(target_os = "macos")]
unsafe fn configure_snap_overlay_window_native(window: cocoa::base::id) {
    if crate::runtime_policy::check(ExternalEffect::NativeVisibility).is_err() {
        return;
    }
    use objc::{class, msg_send, sel, sel_impl};

    if window.is_null() {
        tracing::warn!(
            target: "script_kit::snap_overlay",
            event = "snap_overlay_window_null",
            "Cannot configure null snap overlay window"
        );
        return;
    }

    // SAFETY: `window` is a valid NSWindow pointer obtained from GPUI on the main thread.
    let _: () = msg_send![window, setMovable: false];
    let _: () = msg_send![window, setMovableByWindowBackground: false];
    let _: () = msg_send![window, setOpaque: false];

    // SAFETY: `clearColor` is a standard NSColor class method returning a valid autoreleased color.
    let clear: cocoa::base::id = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![window, setBackgroundColor: clear];

    let _: () = msg_send![window, setHasShadow: false];
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
    let _: () = msg_send![window, setRestorable: false];
    let _: () = msg_send![window, setAnimationBehavior: 2i64];

    // SAFETY: Empty NSString disables frame autosave for this transient overlay window.
    let empty_string: cocoa::base::id = msg_send![class!(NSString), string];
    let _: () = msg_send![window, setFrameAutosaveName: empty_string];
    let _: () = msg_send![window, orderFrontRegardless];
}

// ---------------------------------------------------------------------------
// Per-display overlay window registry
// ---------------------------------------------------------------------------

struct OverlayDisplayWindow {
    display_bounds: Bounds,
    handle: WindowHandle<SnapOverlayView>,
}

static SNAP_OVERLAY_WINDOWS: Mutex<Vec<OverlayDisplayWindow>> = Mutex::new(Vec::new());

fn pixels_to_i32(value: Pixels) -> i32 {
    let px_value: f64 = value.into();
    px_value.round() as i32
}

fn intersection_area(a: &Bounds, b: &Bounds) -> u64 {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width as i32).min(b.x + b.width as i32);
    let bottom = (a.y + a.height as i32).min(b.y + b.height as i32);

    if right <= left || bottom <= top {
        return 0;
    }

    ((right - left) as u64) * ((bottom - top) as u64)
}

fn gpui_display_slot_for_native_index(
    native_index: usize,
    gpui_display_count: usize,
) -> Option<(usize, &'static str)> {
    if gpui_display_count == 0 {
        return None;
    }

    if gpui_display_count == 1 {
        return Some((0, "single_display"));
    }

    if native_index < gpui_display_count {
        return Some((native_index, "index"));
    }

    None
}

fn matched_gpui_display_for_native_index(
    native_index: usize,
    native_full_bounds: &Bounds,
    gpui_displays: &[(gpui::DisplayId, Bounds)],
) -> Option<(gpui::DisplayId, Bounds, &'static str)> {
    if let Some((gpui_index, strategy)) =
        gpui_display_slot_for_native_index(native_index, gpui_displays.len())
    {
        if let Some((display_id, bounds)) = gpui_displays.get(gpui_index) {
            return Some((*display_id, *bounds, strategy));
        }
    }

    gpui_displays
        .iter()
        .max_by_key(|(_, gpui_bounds)| intersection_area(native_full_bounds, gpui_bounds))
        .map(|(display_id, bounds)| (*display_id, *bounds, "overlap_fallback"))
}

fn same_display_bounds_set(existing: &[OverlayDisplayWindow], next: &[Bounds]) -> bool {
    existing.len() == next.len()
        && existing
            .iter()
            .zip(next.iter())
            .all(|(existing, next)| existing.display_bounds == *next)
}

fn close_overlay_windows(overlay_windows: Vec<OverlayDisplayWindow>, cx: &mut App) {
    for overlay in overlay_windows {
        overlay
            .handle
            .update(cx, |_view, window, _cx| {
                window.remove_window();
            })
            .ok();
    }
}

// ---------------------------------------------------------------------------
// Overlay view
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SnapOverlayView {
    model: Option<SnapOverlayModel>,
    registration: Option<(String, u64)>,
    data_revision: u64,
    presentation_revision: u64,
    applied_theme_revision: u64,
}

impl SnapOverlayView {
    pub fn new() -> Self {
        Self {
            model: None,
            registration: None,
            data_revision: 1,
            presentation_revision: 1,
            applied_theme_revision: 0,
        }
    }

    pub(crate) fn model(&self) -> Option<&SnapOverlayModel> {
        self.model.as_ref()
    }
    pub(crate) fn revision_facts(&self) -> (u64, u64, u64, u64) {
        (
            self.data_revision,
            self.data_revision,
            self.presentation_revision,
            self.applied_theme_revision,
        )
    }
    pub(crate) fn close_owned(&mut self, window: &mut Window) -> Result<()> {
        anyhow::ensure!(window.is_owned_hidden(), "snap_owned_host_required");
        let (id, generation) = self
            .registration
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("snap_registration_missing"))?;
        anyhow::ensure!(
            crate::windows::get_runtime_window_handle_for_generation(id, *generation)
                == Some(window.window_handle()),
            "snap_target_stale"
        );
        crate::windows::remove_runtime_window_instance(id, *generation);
        self.registration = None;
        self.model = None;
        window.remove_window();
        Ok(())
    }

    pub fn set_model(&mut self, model: Option<SnapOverlayModel>, cx: &mut Context<Self>) {
        let active_tile = model.as_ref().and_then(|m| {
            m.targets
                .iter()
                .find(|t| t.active)
                .map(|t| format!("{:?}", t.tile))
        });

        tracing::info!(
            target: "script_kit::snap_overlay",
            event = "snap_overlay_model_updated",
            has_model = model.is_some(),
            mode = model.as_ref().map(|m| format!("{:?}", m.mode)),
            dominant = model.as_ref().map(|m| m.is_dominant),
            target_count = model.as_ref().map(|m| m.targets.len()),
            ?active_tile,
            "updated snap overlay model"
        );

        self.model = model;
        self.data_revision = self.data_revision.saturating_add(1);
        cx.notify();
    }
}

impl Drop for SnapOverlayView {
    fn drop(&mut self) {
        if let Some((id, generation)) = &self.registration {
            crate::windows::remove_runtime_window_instance(id, *generation);
        }
    }
}

/// Open the real Snap presentation, independently of desktop discovery and snapping effects.
pub(crate) fn open_snap_overlay_window(
    bounds: gpui::Bounds<Pixels>,
    display_id: Option<gpui::DisplayId>,
    host_policy: WindowHostPolicy,
    cx: &mut App,
) -> Result<WindowHandle<SnapOverlayView>> {
    host_policy.validate()?;
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id,
            titlebar: None,
            window_background: WindowBackgroundAppearance::Transparent,
            focus: false,
            show: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        },
        |_, cx| cx.new(|_| SnapOverlayView::new()),
    )?;
    let mut configure = || -> Result<()> {
        #[cfg(target_os = "macos")]
        if !host_policy.is_hidden() {
            crate::runtime_policy::check(ExternalEffect::NativeVisibility)?;
            handle.update(cx, |_, window, _| -> Result<()> {
                let native = crate::components::inline_popup_window::inline_popup_ns_window(window)
                    .ok_or_else(|| anyhow::anyhow!("snap_native_window_missing"))?;
                // SAFETY: native is the exact newly-created live GPUI window.
                unsafe {
                    configure_snap_overlay_window_native(native);
                }
                Ok(())
            })??;
        }
        let id = format!(
            "snap-overlay:{}",
            NEXT_SNAP_OVERLAY_ID.fetch_add(1, Ordering::Relaxed)
        );
        let info = crate::windows::register_runtime_window_instance(
            crate::protocol::AutomationWindowInfo {
                id: id.clone(),
                kind: crate::protocol::AutomationWindowKind::SnapOverlay,
                title: Some("Snap overlay".into()),
                focused: false,
                visible: !host_policy.is_hidden(),
                semantic_surface: Some("snapOverlay".into()),
                bounds: Some(crate::protocol::AutomationWindowBounds {
                    x: f32::from(bounds.origin.x) as f64,
                    y: f32::from(bounds.origin.y) as f64,
                    width: f32::from(bounds.size.width) as f64,
                    height: f32::from(bounds.size.height) as f64,
                }),
                parent_window_id: None,
                parent_kind: None,
                parent_window_generation: None,
                pid: Some(std::process::id()),
                generation: None,
            },
            handle.into(),
            cx,
        )?;
        let generation = info
            .generation
            .ok_or_else(|| anyhow::anyhow!("snap_generation_missing"))?;
        handle.update(cx, |view, _, _| view.registration = Some((id, generation)))?;
        Ok(())
    };
    if let Err(error) = configure() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        return Err(error);
    }
    Ok(handle)
}

impl Render for SnapOverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let revision = crate::theme::service::theme_revision();
        if self.applied_theme_revision != revision {
            self.applied_theme_revision = revision;
            self.presentation_revision = self.presentation_revision.saturating_add(1);
        }
        let Some(ref model) = self.model else {
            return div()
                .debug_selector(|| "snap-overlay-root".to_string())
                .size_full();
        };

        let display = model.display_bounds;
        let targets: Vec<SnapOverlayTarget> = model.targets.clone();

        // Base color for each snap mode (RGB values with alpha placeholder 00)
        let base_color = match model.mode {
            SnapMode::Simple => 0x66D9E800,
            SnapMode::Expanded => 0xFFFFFF00,
            SnapMode::Precision => 0xF6AD5500,
            SnapMode::Off => 0x00000000,
        };

        let scrim = match model.mode {
            SnapMode::Simple | SnapMode::Expanded => 0x00000010,
            SnapMode::Precision => 0x00000012,
            SnapMode::Off => 0x00000000,
        };
        let scrim = if model.is_dominant { scrim } else { scrim / 2 };

        div()
            .debug_selector(|| "snap-overlay-root".to_string())
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .w(px(display.width as f32))
            .h(px(display.height as f32))
            .bg(rgba(scrim))
            .children(targets.into_iter().filter(|t| t.active).map(move |target| {
                let rel_x = (target.bounds.x - display.x) as f32;
                let rel_y = (target.bounds.y - display.y) as f32;

                // Border: 75% opacity (0xC0) when highlighted/active
                let border = base_color | 0xC0;

                // The area itself is not highlighted/filled anymore, only the border is highlighted.
                let fill = 0x00000000;

                div()
                    .debug_selector(move || format!("snap:target:{:?}", target.tile))
                    .absolute()
                    .left(px(rel_x))
                    .top(px(rel_y))
                    .w(px(target.bounds.width as f32))
                    .h(px(target.bounds.height as f32))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgba(border))
                    .bg(rgba(fill))
            }))
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Ensure one overlay window exists per connected display.
///
/// Idempotent — returns immediately if windows are already open.
pub fn ensure_snap_overlay_windows(cx: &mut App) -> Result<()> {
    crate::runtime_policy::check(ExternalEffect::SystemDiscovery)?;
    let mut guard = super::snap_lock(&SNAP_OVERLAY_WINDOWS, "overlay")?;

    let native_displays = get_native_display_descriptors()
        .context("failed to query native display descriptors for snap overlay")?;
    let expected_visible_bounds: Vec<Bounds> = native_displays
        .iter()
        .map(|display| display.visible_bounds)
        .collect();

    if !guard.is_empty() && same_display_bounds_set(&guard, &expected_visible_bounds) {
        return Ok(());
    }

    let old_windows: Vec<_> = guard.drain(..).collect();
    drop(guard);
    close_overlay_windows(old_windows, cx);

    let gpui_displays: Vec<_> = cx
        .displays()
        .iter()
        .map(|display| {
            let full = display.bounds();
            (
                display.id(),
                Bounds {
                    x: pixels_to_i32(full.origin.x),
                    y: pixels_to_i32(full.origin.y),
                    width: pixels_to_i32(full.size.width) as u32,
                    height: pixels_to_i32(full.size.height) as u32,
                },
            )
        })
        .collect();

    let mut overlay_windows = Vec::new();

    for (native_index, native_display) in native_displays.into_iter().enumerate() {
        let Some((display_id, gpui_full_bounds, mapping_strategy)) =
            matched_gpui_display_for_native_index(
                native_index,
                &native_display.full_bounds,
                &gpui_displays,
            )
        else {
            continue;
        };

        let local_x = native_display.visible_bounds.x - native_display.full_bounds.x;
        let local_y = native_display.visible_bounds.y - native_display.full_bounds.y;

        let overlay_bounds = gpui::Bounds {
            origin: point(px(local_x as f32), px(local_y as f32)),
            size: size(
                px(native_display.visible_bounds.width as f32),
                px(native_display.visible_bounds.height as f32),
            ),
        };

        let handle = open_snap_overlay_window(
            overlay_bounds,
            Some(display_id),
            WindowHostPolicy::Interactive,
            cx,
        )
        .context("failed to open snap overlay window")?;

        tracing::info!(
            target: "script_kit::snap_overlay",
            event = "snap_overlay_window_mapped",
            native_index,
            mapping_strategy,
            native_display_count = expected_visible_bounds.len(),
            gpui_display_count = gpui_displays.len(),
            visible_x = native_display.visible_bounds.x,
            visible_y = native_display.visible_bounds.y,
            visible_w = native_display.visible_bounds.width,
            visible_h = native_display.visible_bounds.height,
            native_full_x = native_display.full_bounds.x,
            native_full_y = native_display.full_bounds.y,
            gpui_full_x = gpui_full_bounds.x,
            gpui_full_y = gpui_full_bounds.y,
            gpui_full_w = gpui_full_bounds.width,
            gpui_full_h = gpui_full_bounds.height,
            "mapped native display descriptor to GPUI display for snap overlay"
        );

        overlay_windows.push(OverlayDisplayWindow {
            display_bounds: native_display.visible_bounds,
            handle,
        });
    }

    let mut guard = super::snap_lock(&SNAP_OVERLAY_WINDOWS, "overlay")?;
    *guard = overlay_windows;

    tracing::info!(
        target: "script_kit::snap_overlay",
        event = "snap_overlay_windows_opened",
        window_count = guard.len(),
        "opened snap overlay windows"
    );

    Ok(())
}

/// Show the snap overlay scene, distributing models to matching display windows.
///
/// Each overlay window receives the model whose `display_bounds` matches its
/// own, or `None` if no model in the scene covers that display.
pub fn show_snap_overlay(scene: SnapOverlayScene, cx: &mut App) -> Result<()> {
    ensure_snap_overlay_windows(cx)?;

    let guard = super::snap_lock(&SNAP_OVERLAY_WINDOWS, "overlay")?;

    for overlay in guard.iter() {
        let model_for_window = scene
            .displays
            .iter()
            .find(|m| m.display_bounds == overlay.display_bounds)
            .cloned();

        let _ = overlay.handle.update(cx, |view, _window, cx| {
            view.set_model(model_for_window, cx);
        });
    }

    tracing::info!(
        target: "script_kit::snap_overlay",
        event = "snap_overlay_scene_updated",
        display_count = scene.displays.len(),
        ?scene.mode,
        "updated snap overlay scene"
    );

    Ok(())
}

/// Hide the snap overlay on all displays.
pub fn hide_snap_overlay(cx: &mut App) -> Result<()> {
    let guard = super::snap_lock(&SNAP_OVERLAY_WINDOWS, "overlay")?;

    for overlay in guard.iter() {
        let _ = overlay.handle.update(cx, |view, _window, cx| {
            view.set_model(None, cx);
        });
    }

    tracing::info!(
        target: "script_kit::snap_overlay",
        event = "snap_overlay_hidden",
        "hid snap overlay"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::gpui_display_slot_for_native_index;

    #[test]
    fn native_and_gpui_display_slots_align_by_index_when_counts_match() {
        assert_eq!(gpui_display_slot_for_native_index(0, 3), Some((0, "index")));
        assert_eq!(gpui_display_slot_for_native_index(1, 3), Some((1, "index")));
        assert_eq!(gpui_display_slot_for_native_index(2, 3), Some((2, "index")));
    }

    #[test]
    fn single_gpui_display_maps_everything_to_that_display() {
        assert_eq!(
            gpui_display_slot_for_native_index(0, 1),
            Some((0, "single_display"))
        );
        assert_eq!(
            gpui_display_slot_for_native_index(2, 1),
            Some((0, "single_display"))
        );
    }

    #[test]
    fn out_of_range_native_index_uses_fallback_path() {
        assert_eq!(gpui_display_slot_for_native_index(3, 3), None);
        assert_eq!(gpui_display_slot_for_native_index(0, 0), None);
    }
}
