use super::*;

use std::sync::{Mutex, OnceLock};

use gpui::{
    div, AnyWindowHandle, Bounds, DisplayId, Entity, FocusHandle, Pixels, Point, Render, Size,
    WeakEntity, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

const SHORTCUT_RECORDER_POPUP_HEIGHT: f32 = 196.0;
#[cfg(target_os = "macos")]
const NS_WINDOW_ABOVE: i64 = 1;

static SHORTCUT_RECORDER_WINDOW: OnceLock<
    Mutex<Option<WindowHandle<ShortcutRecorderPopupWindow>>>,
> = OnceLock::new();

fn shortcut_config_script_path(script_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let sdk_path = std::path::PathBuf::from(home_dir)
        .join(".scriptkit")
        .join("sdk")
        .join(script_name);

    if sdk_path.exists() {
        return Ok(sdk_path);
    }

    let dev_path = std::env::current_dir()?.join("scripts").join(script_name);
    if dev_path.exists() {
        return Ok(dev_path);
    }

    Err(anyhow::anyhow!(
        "Could not find {} in .scriptkit/sdk or repo scripts/",
        script_name
    ))
}

fn shortcut_config_bun_path() -> String {
    crate::config::load_config()
        .bun_path
        .as_ref()
        .filter(|path| std::path::Path::new(path.as_str()).exists())
        .cloned()
        .unwrap_or_else(|| "bun".to_string())
}

impl ScriptListApp {
    pub(crate) fn write_config_command_shortcut(
        &self,
        command_id: &str,
        key: &str,
        cmd: bool,
        ctrl: bool,
        alt: bool,
        shift: bool,
    ) -> anyhow::Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
        let script_path = shortcut_config_script_path("update-config-shortcut.ts")?;
        let output = std::process::Command::new(shortcut_config_bun_path())
            .arg(script_path)
            .arg(command_id)
            .arg(key)
            .arg(cmd.to_string())
            .arg(ctrl.to_string())
            .arg(alt.to_string())
            .arg(shift.to_string())
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(anyhow::anyhow!(
                "config shortcut update failed: {}{}",
                stderr.trim(),
                stdout.trim()
            ))
        }
    }

    pub(crate) fn remove_config_command_shortcut(&self, command_id: &str) -> anyhow::Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
        let script_path = shortcut_config_script_path("remove-config-shortcut.ts")?;
        let output = std::process::Command::new(shortcut_config_bun_path())
            .arg(script_path)
            .arg(command_id)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(anyhow::anyhow!(
                "config shortcut removal failed: {}{}",
                stderr.trim(),
                stdout.trim()
            ))
        }
    }
}

struct ShortcutRecorderPopupWindow {
    recorder: Entity<crate::components::shortcut_recorder::ShortcutRecorder>,
    app: WeakEntity<ScriptListApp>,
    focus_handle: FocusHandle,
    generation: u64,
    host_policy: crate::runtime_policy::WindowHostPolicy,
    _recorder_subscription: Subscription,
    _close_subscription: Option<Subscription>,
    data_revision: u64,
    last_semantic_value: Option<serde_json::Value>,
    applied_theme_revision: Option<u64>,
}

impl ShortcutRecorderPopupWindow {
    fn new(
        command_id: String,
        command_name: String,
        theme: std::sync::Arc<theme::Theme>,
        app: WeakEntity<ScriptListApp>,
        host_policy: crate::runtime_policy::WindowHostPolicy,
        cx: &mut Context<Self>,
    ) -> Self {
        let recorder_theme = std::sync::Arc::clone(&theme);
        let recorder = cx.new(move |cx| {
            let conflict_command_id = command_id.clone();
            let recorder = crate::components::shortcut_recorder::ShortcutRecorder::new(cx, recorder_theme)
                .with_detached_window(true)
                .with_command_name(command_name)
                .with_command_description(format!("ID: {}", command_id));
            if host_policy.is_hidden() { recorder } else { recorder.with_conflict_checker(move |recorded| {
                    crate::hotkeys::shortcut_conflict_for_recording(
                        &conflict_command_id,
                        &recorded.to_config_string(),
                    )
                    .map(|conflict| {
                        crate::components::shortcut_recorder::ShortcutConflict {
                            command_name: conflict.command_name,
                            shortcut: conflict.shortcut,
                        }
                    })
                }) }
        });
        let initial_semantic_value = host_policy.is_hidden().then(|| shortcut_recorder_semantic_value(recorder.read(cx)));
        let recorder_subscription = cx.observe(&recorder, |this, recorder, cx| {
            if this.host_policy.is_hidden() {
                let next = shortcut_recorder_semantic_value(recorder.read(cx));
                if this.last_semantic_value.as_ref() != Some(&next) {
                    this.last_semantic_value = Some(next);
                    this.data_revision = this.data_revision.saturating_add(1);
                }
            }
            let action = recorder.update(cx, |recorder, _| recorder.take_pending_action());
            if let Some(action) = action {
                let app = this.app.clone();
                let generation = this.generation;
                cx.spawn(async move |_this, cx| {
                    cx.update(|cx| {
                        if shortcut_fixture_handle("shortcut-recorder-popup", generation, None, cx).is_err() { return; }
                        if let Some(app) = app.upgrade() {
                            app.update(cx, |app, cx| match action {
                                crate::components::shortcut_recorder::RecorderAction::Save(recorded) => app.handle_shortcut_save(&recorded, cx),
                                crate::components::shortcut_recorder::RecorderAction::Cancel => app.close_shortcut_recorder(cx),
                            });
                        } else {
                            let _ = close_shortcut_recorder_instance(generation, cx);
                        }
                    });
                }).detach();
            }
        });

        Self {
            recorder,
            app,
            focus_handle: cx.focus_handle(),
            generation: 0,
            host_policy,
            _recorder_subscription: recorder_subscription,
            _close_subscription: None,
            data_revision: 1,
            last_semantic_value: initial_semantic_value,
            applied_theme_revision: None,
        }
    }
}

impl Render for ShortcutRecorderPopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let published = crate::theme::get_theme_snapshot();
        if self.applied_theme_revision != Some(published.revision) {
            self.recorder.update(cx, |recorder, _| {
                recorder.theme = published.theme.clone();
                recorder.colors = crate::components::shortcut_recorder::ShortcutRecorderColors::from_theme(&published.theme);
            });
            self.applied_theme_revision = Some(published.revision);
        }
        let recorder_fh = self.recorder.read(cx).focus_handle.clone();
        if !recorder_fh.is_focused(window) {
            window.focus(&recorder_fh, cx);
        }


        div()
            .id("shortcut-recorder-window")
            .debug_selector(|| "shortcut-recorder-window".into())
            .relative()
            .w_full()
            .h_full()
            .track_focus(&self.focus_handle)
            .child(self.recorder.clone())
    }
}

fn shortcut_recorder_window_bounds(parent_bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let width =
        px(crate::components::confirm_modal_shell::MODAL_WIDTH_PX).min(parent_bounds.size.width);
    let height = px(SHORTCUT_RECORDER_POPUP_HEIGHT).min(parent_bounds.size.height);
    let x = parent_bounds.origin.x + ((parent_bounds.size.width - width) / 2.0);
    let y = parent_bounds.origin.y + ((parent_bounds.size.height - height) / 2.0);

    Bounds {
        origin: Point { x, y },
        size: Size { width, height },
    }
}

fn close_shortcut_recorder_window(cx: &mut App) {
    if let Some(generation) = crate::windows::automation_window_by_id("shortcut-recorder-popup").and_then(|info| info.generation) {
        let _ = close_shortcut_recorder_instance(generation, cx);
    }
}

pub(crate) fn close_shortcut_recorder_instance(generation: u64, cx: &mut App) -> anyhow::Result<()> {
    let expected = crate::windows::get_runtime_window_handle_for_generation("shortcut-recorder-popup", generation)
        .ok_or_else(|| anyhow::anyhow!("shortcut_recorder_stale"))?;
    let storage = SHORTCUT_RECORDER_WINDOW.get_or_init(|| Mutex::new(None));
    let handle = {
        let mut guard = storage.lock().map_err(|_| anyhow::anyhow!("shortcut_recorder_lock_poisoned"))?;
        match *guard {
            Some(handle) if AnyWindowHandle::from(handle) == expected => {
                *guard = None;
                handle
            }
            _ => anyhow::bail!("shortcut_recorder_stale"),
        }
    };
    crate::windows::remove_runtime_window_instance("shortcut-recorder-popup", generation);
    let app = handle.update(cx, |popup, window, cx| {
        if popup.host_policy.is_hidden() { window.remove_window(); }
        else { crate::platform::dematerialize_then_remove_gpui_window(window, cx, "SHORTCUT", "Shortcut recorder popup"); }
        popup.app.clone()
    })?;
    if let Some(app) = app.upgrade() {
        app.update(cx, |app, cx| {
            app.shortcut_recorder_state = None;
            app.shortcut_recorder_entity = None;
            app.pending_focus = Some(FocusTarget::MainFilter);
            app.focused_input = FocusedInput::MainFilter;
            app.mark_main_presentation_changed();
            cx.notify();
        });
    }
    Ok(())
}

pub(crate) fn is_shortcut_recorder_window(window: &gpui::Window) -> bool {
    if let Some(storage) = SHORTCUT_RECORDER_WINDOW.get() {
        if let Ok(guard) = storage.lock() {
            if let Some(handle) = guard.as_ref() {
                let recorder_any: AnyWindowHandle = (*handle).into();
                return window.window_handle() == recorder_any;
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn shortcut_recorder_ns_window(window: &mut Window) -> Option<cocoa::base::id> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use cocoa::base::nil;
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
            unsafe {
                let ns_window: cocoa::base::id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some(ns_window);
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn attach_shortcut_recorder_to_parent_window(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    child_ns_window: cocoa::base::id,
) {
    let _ = cx.update_window(parent_window_handle, move |_, parent_window, _cx| {
        let Some(parent_ns_window) = shortcut_recorder_ns_window(parent_window) else {
            return;
        };

        unsafe {
            use cocoa::base::nil;
            use objc::{msg_send, sel, sel_impl};

            if parent_ns_window == nil
                || child_ns_window == nil
                || parent_ns_window == child_ns_window
            {
                return;
            }

            let _: () =
                msg_send![parent_ns_window, addChildWindow:child_ns_window ordered:NS_WINDOW_ABOVE];
            let _: () = msg_send![child_ns_window, orderFrontRegardless];
            let _: () = msg_send![child_ns_window, makeKeyWindow];
        }
    });
}

fn shortcut_recorder_semantic_value(recorder: &crate::components::shortcut_recorder::ShortcutRecorder) -> serde_json::Value {
    serde_json::json!({
        "shortcut": recorder.shortcut.to_config_string(), "recording": recorder.is_recording,
        "canSave": recorder.can_save(), "conflict": recorder.conflict.as_ref().map(|conflict| &conflict.command_name),
        "heldModifiers": [recorder.current_modifiers.platform, recorder.current_modifiers.control, recorder.current_modifiers.alt, recorder.current_modifiers.shift],
        "focusedAction": format!("{:?}", recorder.focused_action),
    })
}

pub(crate) struct ShortcutFixtureObservation {
    pub data_revision: u64,
    pub presentation_revision: u64,
    pub applied_theme_revision: Option<u64>,
    pub value: serde_json::Value,
}

fn shortcut_fixture_handle(id: &str, generation: u64, window: Option<&Window>, cx: &App) -> anyhow::Result<WindowHandle<ShortcutRecorderPopupWindow>> {
    anyhow::ensure!(id == "shortcut-recorder-popup" && generation > 0, "not_shortcut_recorder");
    let handle = SHORTCUT_RECORDER_WINDOW.get().and_then(|storage| storage.lock().ok().and_then(|guard| *guard))
        .ok_or_else(|| anyhow::anyhow!("shortcut_recorder_missing"))?;
    anyhow::ensure!(crate::windows::get_runtime_window_handle_for_generation(id, generation) == Some(handle.into()), "shortcut_recorder_stale");
    let info = crate::windows::automation_window_by_id(id).ok_or_else(|| anyhow::anyhow!("shortcut_recorder_missing"))?;
    anyhow::ensure!(info.generation == Some(generation) && crate::windows::automation_surface_collector::read_window_root(handle, window, cx, |popup, _| popup.generation)? == generation, "shortcut_recorder_stale");
    let parent_id = info.parent_window_id.as_deref().ok_or_else(|| anyhow::anyhow!("shortcut_parent_missing"))?;
    let parent_generation = info.parent_window_generation.ok_or_else(|| anyhow::anyhow!("shortcut_parent_generation_missing"))?;
    anyhow::ensure!(crate::windows::get_runtime_window_handle_for_generation(parent_id, parent_generation).is_some(), "shortcut_parent_stale");
    Ok(handle)
}

pub(crate) fn shortcut_fixture_observation(id: &str, generation: u64, window: Option<&Window>, cx: &App) -> anyhow::Result<ShortcutFixtureObservation> {
    let handle = shortcut_fixture_handle(id, generation, window, cx)?;
    crate::windows::automation_surface_collector::read_window_root(handle, window, cx, |popup, cx| {
        ShortcutFixtureObservation { data_revision: popup.data_revision, presentation_revision: 1, applied_theme_revision: popup.applied_theme_revision, value: shortcut_recorder_semantic_value(popup.recorder.read(cx)) }
    })
}

pub(crate) fn shortcut_fixture_elements(id: &str, generation: u64, cx: &App) -> anyhow::Result<Vec<crate::protocol::ElementInfo>> {
    let handle = shortcut_fixture_handle(id, generation, None, cx)?;
    Ok(handle.read(cx)?.recorder.read(cx).automation_elements())
}

pub(crate) fn shortcut_fixture_select(id: &str, generation: u64, semantic_id: &str, submit: bool, cx: &mut App) -> anyhow::Result<bool> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    if !submit {
        return Err(crate::protocol::TransactionError {
            code: crate::protocol::TransactionErrorCode::UnsupportedCommand,
            message: "selection_only_unsupported".into(),
            suggestion: Some("Use explicit submit:true to activate a recorder action".into()),
        }.into());
    }
    let handle = shortcut_fixture_handle(id, generation, None, cx)?;
    anyhow::ensure!(crate::windows::runtime_window_host_policy(id, generation)? == crate::runtime_policy::WindowHostPolicy::OwnedHidden
        && handle.read(cx)?.host_policy == crate::runtime_policy::WindowHostPolicy::OwnedHidden, "shortcut_owned_host_required");
    handle.update(cx, |popup, _, cx| {
        popup.recorder.update(cx, |recorder, cx| recorder.activate_semantic_action(semantic_id, cx))
            .map_err(anyhow::Error::msg)
    })?
}

pub(crate) fn shortcut_fixture_layout(id: &str, generation: u64, cx: &mut App) -> anyhow::Result<crate::protocol::LayoutInfo> {
    use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
    let handle = shortcut_fixture_handle(id, generation, None, cx)?;
    handle.update(cx, |popup, window, cx| {
        let frame = window.rendered_frame_generation();
        anyhow::ensure!(frame > 0, "shortcut_layout_unavailable:unpainted");
        let recorder = popup.recorder.read(cx);
        let mut selectors = vec![
            ("shortcut-recorder-window", LayoutComponentType::Panel, None),
            ("shortcut-modal-content", LayoutComponentType::Container, Some("shortcut-recorder-window")),
            ("shortcut-recorder-header", LayoutComponentType::Header, Some("shortcut-modal-content")),
            ("shortcut-key-display", LayoutComponentType::Input, Some("shortcut-modal-content")),
        ];
        if recorder.conflict.is_some() {
            selectors.push(("shortcut-conflict-warning", LayoutComponentType::Other, Some("shortcut-modal-content")));
        }
        if !recorder.capture_only {
            selectors.extend([
                ("shortcut-modal-action-row", LayoutComponentType::Container, Some("shortcut-modal-content")),
                ("shortcut-save-button", LayoutComponentType::Button, Some("shortcut-modal-action-row")),
                ("shortcut-clear-button", LayoutComponentType::Button, Some("shortcut-modal-action-row")),
                ("shortcut-cancel-button", LayoutComponentType::Button, Some("shortcut-modal-action-row")),
            ]);
        }
        let mut components = Vec::with_capacity(selectors.len());
        for (selector, component_type, parent) in selectors {
            let entry = window.debug_bounds_entries().iter().rev().find(|entry| entry.selector == selector)
                .ok_or_else(|| anyhow::anyhow!("shortcut_layout_unavailable:{selector}"))?;
            let bounds = entry.bounds;
            let visible = entry.visible_bounds;
            let clip = entry.clip_bounds;
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            anyhow::ensure!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0, "shortcut_layout_unavailable:{selector}");
            let mut component = LayoutComponentInfo::new(selector, component_type)
                .with_bounds(f32::from(bounds.origin.x), f32::from(bounds.origin.y), width, height)
                .with_measurement("paint-time", "window")
                .with_measurement_frame(frame)
                .with_paint_visibility(f32::from(visible.origin.x), f32::from(visible.origin.y), f32::from(visible.size.width), f32::from(visible.size.height), f32::from(clip.origin.x), f32::from(clip.origin.y), f32::from(clip.size.width), f32::from(clip.size.height));
            if let Some(parent) = parent { component = component.with_parent(parent); }
            components.push(component);
        }
        let viewport = window.viewport_size();
        Ok(LayoutInfo { window_width: f32::from(viewport.width), window_height: f32::from(viewport.height), prompt_type: "shortcutRecorder".into(), components, timestamp: chrono::Utc::now().to_rfc3339(), ..Default::default() })
    })?
}

pub(crate) fn mount_owned_shortcut_recorder(app: Entity<ScriptListApp>, parent: &crate::protocol::AutomationWindowInfo, parent_handle: AnyWindowHandle, cx: &mut App) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    let policy = crate::runtime_policy::WindowHostPolicy::OwnedHidden;
    policy.validate()?;
    let generation = parent.generation.ok_or_else(|| anyhow::anyhow!("shortcut_parent_generation_missing"))?;
    anyhow::ensure!(parent.id == "main" && crate::windows::get_runtime_window_handle_for_generation(&parent.id, generation) == Some(parent_handle), "shortcut_parent_stale");
    anyhow::ensure!(crate::windows::runtime_window_host_policy(&parent.id, generation)? == policy, "shortcut_parent_not_owned");
    let (bounds, display_id) = parent_handle.update(cx, |_, window, cx| (window.bounds(), window.display(cx).map(|display| display.id())))?;
    let command_id = "builtin/clipboard-history".to_string();
    let command_name = "Clipboard History".to_string();
    app.update(cx, |app, cx| {
        app.shortcut_recorder_state = Some(ShortcutRecorderState { command_id: command_id.clone(), command_name: command_name.clone() });
        app.shortcut_recorder_entity = None;
        app.mark_main_presentation_changed();
        cx.notify();
    });
    let theme = app.read(cx).theme.clone();
    let opened = open_shortcut_recorder_window(cx, app.downgrade(), command_id, command_name, theme, ShortcutRecorderParentWindow { handle: parent_handle, bounds, display_id, info: parent.clone() }, policy);
    if let Err(error) = opened {
        app.update(cx, |app, cx| { app.shortcut_recorder_state = None; cx.notify(); });
        return Err(error);
    }
    crate::windows::automation_window_by_id("shortcut-recorder-popup").ok_or_else(|| anyhow::anyhow!("shortcut_registration_missing"))
}

struct ShortcutRecorderParentWindow {
    handle: AnyWindowHandle,
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    info: crate::protocol::AutomationWindowInfo,
}

/// Opens the shortcut recorder as a key popup because raw shortcut capture must
/// receive Tab/modifier keystrokes instead of routing them through the main
/// window. This is the explicit exception to confirm's `focus:false` model: the
/// AppKit setup still follows the actions popup recipe
/// (`WindowKind::PopUp` + `configure_actions_popup_window`) so the child panel
/// stays non-activating while owning keyboard input for Escape/capture.
fn open_shortcut_recorder_window(
    cx: &mut App,
    app: WeakEntity<ScriptListApp>,
    command_id: String,
    command_name: String,
    theme: std::sync::Arc<theme::Theme>,
    parent: ShortcutRecorderParentWindow,
    host_policy: crate::runtime_policy::WindowHostPolicy,
) -> anyhow::Result<WindowHandle<ShortcutRecorderPopupWindow>> {
    host_policy.validate()?;
    let parent_generation = parent.info.generation.ok_or_else(|| anyhow::anyhow!("shortcut_parent_generation_missing"))?;
    anyhow::ensure!(crate::windows::get_runtime_window_handle_for_generation(&parent.info.id, parent_generation) == Some(parent.handle), "shortcut_parent_stale");
    close_shortcut_recorder_window(cx);

    let window_background = if theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        WindowBackgroundAppearance::Opaque
    };
    let is_dark_vibrancy = theme.should_use_dark_vibrancy();
    let bounds = shortcut_recorder_window_bounds(parent.bounds);

    let window_theme = std::sync::Arc::clone(&theme);
    let app_for_close = app.clone();
    // Intentionally not Root-wrapped: this popup is fixed compact capture chrome.
    // Keep focus/root behavior unchanged unless capture dismissal is retested.
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            window_background,
            focus: !host_policy.is_hidden(),
            show: !host_policy.is_hidden(),
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: parent.display_id,
            ..Default::default()
        },
        move |_window, cx| {
            cx.new(|cx| {
                ShortcutRecorderPopupWindow::new(command_id, command_name, window_theme, app, host_policy, cx)
            })
        },
    )?;

    #[cfg(target_os = "macos")]
    if !host_policy.is_hidden() {
        let parent_id = parent.info.id.clone();
        let _ = handle.update(cx, move |_popup, window, cx| {
            window.defer(cx, move |window, cx| {
                if crate::windows::get_runtime_window_handle_for_generation(&parent_id, parent_generation) != Some(parent.handle)
                    || crate::windows::get_runtime_window_handle("shortcut-recorder-popup") != Some(window.window_handle()) { return; }
                if let Some(ns_window) = shortcut_recorder_ns_window(window) {
                    unsafe {
                        crate::platform::configure_shortcut_recorder_popup_window(
                            ns_window,
                            is_dark_vibrancy,
                        );
                    }
                    attach_shortcut_recorder_to_parent_window(cx, parent.handle, ns_window);
                }
            });
        });
    }

    let storage = SHORTCUT_RECORDER_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        *guard = Some(handle);
    }

    let registered = crate::windows::register_runtime_window_instance(crate::protocol::AutomationWindowInfo {
        id: "shortcut-recorder-popup".into(), kind: crate::protocol::AutomationWindowKind::PromptPopup,
        title: Some("Shortcut Recorder".into()), semantic_surface: Some("shortcutRecorder".into()),
        focused: false, visible: !host_policy.is_hidden(),
        bounds: Some(crate::protocol::AutomationWindowBounds { x: f32::from(bounds.origin.x) as f64, y: f32::from(bounds.origin.y) as f64, width: f32::from(bounds.size.width) as f64, height: f32::from(bounds.size.height) as f64 }),
        parent_window_id: Some(parent.info.id), parent_kind: Some(parent.info.kind), parent_window_generation: Some(parent_generation),
        pid: Some(std::process::id()), generation: None,
    }, handle.into(), cx);
    let info = match registered {
        Ok(info) => info,
        Err(error) => {
            if let Ok(mut guard) = storage.lock() { *guard = None; }
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            return Err(error);
        }
    };
    let generation = info.generation.ok_or_else(|| anyhow::anyhow!("shortcut_generation_missing"))?;
    let subscription = cx.on_window_closed(move |cx, id| {
        if id == handle.window_id() {
            if crate::windows::remove_runtime_window_instance("shortcut-recorder-popup", generation) {
                if let Some(app) = app_for_close.upgrade() {
                    app.update(cx, |app, cx| { app.shortcut_recorder_state = None; app.shortcut_recorder_entity = None; app.pending_focus = Some(FocusTarget::MainFilter); app.mark_main_presentation_changed(); cx.notify(); });
                }
            }
            if let Some(storage) = SHORTCUT_RECORDER_WINDOW.get() {
                if let Ok(mut guard) = storage.lock() {
                    if *guard == Some(handle) { *guard = None; }
                }
            }
        }
    });
    handle.update(cx, |popup, _, _| { popup.generation = generation; popup._close_subscription = Some(subscription); })?;

    logging::log(
        "SHORTCUT",
        "Shortcut recorder popup window opened with vibrancy",
    );

    Ok(handle)
}

impl ScriptListApp {
    pub(crate) fn main_window_modal_owns_keyboard(&self) -> bool {
        self.shortcut_recorder_state.is_some()
            || self.alias_input_state.is_some()
            || crate::confirm::is_confirm_window_open()
    }

    pub(crate) fn edit_script(&mut self, path: &std::path::Path) {
        let editor = self.config.get_editor();
        logging::log(
            "UI",
            &format!("Opening script in editor '{}': {}", editor, path.display()),
        );
        let path_str = path.to_string_lossy().to_string();

        std::thread::spawn(move || {
            use std::process::Command;
            match Command::new(&editor).arg(&path_str).spawn() {
                Ok(_) => logging::log("UI", &format!("Successfully spawned editor: {}", editor)),
                Err(e) => logging::log(
                    "ERROR",
                    &format!("Failed to spawn editor '{}': {}", editor, e),
                ),
            }
        });
    }

    /// Open config.ts for configuring a keyboard shortcut
    /// Creates the file with documentation if it doesn't exist
    ///
    /// NOTE: This is the legacy approach. For new code, use `show_shortcut_recorder()` instead
    /// which opens the detached shortcut recorder popup.
    #[allow(dead_code)]
    pub(crate) fn open_config_for_shortcut(&mut self, command_id: &str) {
        let config_path = shellexpand::tilde("~/.scriptkit/config.ts").to_string();
        let editor = self.config.get_editor();

        logging::log(
            "UI",
            &format!(
                "Opening config.ts for shortcut configuration: {} (command: {})",
                config_path, command_id
            ),
        );

        // Ensure config.ts exists with documentation
        let config_path_buf = std::path::PathBuf::from(&config_path);
        if !config_path_buf.exists() {
            if let Err(e) = Self::create_config_template(&config_path_buf) {
                tracing::error!(error = %e, "Failed to create config.ts");
            }
        }

        // Copy command_id to clipboard as a hint
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = self.pbcopy(command_id) {
                tracing::error!(error = %e, "Failed to copy command ID to clipboard");
            } else {
                self.last_output = Some(gpui::SharedString::from(format!(
                    "Copied '{}' to clipboard - paste in config.ts commands section",
                    command_id
                )));
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            use arboard::Clipboard;
            if let Ok(mut clipboard) = Clipboard::new() {
                if clipboard.set_text(command_id).is_ok() {
                    self.last_output = Some(gpui::SharedString::from(format!(
                        "Copied '{}' to clipboard - paste in config.ts commands section",
                        command_id
                    )));
                }
            }
        }

        let config_path_clone = config_path.clone();
        std::thread::spawn(move || {
            use std::process::Command;
            match Command::new(&editor).arg(&config_path_clone).spawn() {
                Ok(_) => logging::log("UI", &format!("Opened config.ts in {}", editor)),
                Err(e) => tracing::error!(error = %e, "Failed to open config.ts in editor"),
            }
        });
    }

    /// Create config.ts template with keyboard shortcut documentation
    #[allow(dead_code)]
    pub(crate) fn create_config_template(path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let template = r#"// Script Kit Configuration
// https://scriptkit.com/docs/config

import type { Config } from "@scriptkit/sdk";

export default {
  // ============================================
  // MAIN HOTKEY
  // ============================================
  // The keyboard shortcut to open Script Kit
  hotkey: {
    modifiers: ["meta"],
    key: "Semicolon",
  },

  // ============================================
  // KEYBOARD SHORTCUTS
  // ============================================
  // Configure shortcuts for any command (scripts, built-ins, apps, snippets)
  //
  // Command ID formats:
  //   - "script/my-script"           - User scripts (by filename without extension)
  //   - "builtin/clipboard-history"  - Built-in features
  //   - "app/com.apple.Safari"       - Apps (by bundle ID)
  //   - "scriptlet/my-snippet"       - Scriptlets/snippets
  //
  // Modifier keys: "meta" (⌘), "ctrl", "alt" (⌥), "shift"
  // Key names: "KeyA"-"KeyZ", "Digit0"-"Digit9", "Space", "Enter", etc.
  //
  // Example:
  //   commands: {
  //     "builtin/clipboard-history": {
  //       shortcut: { modifiers: ["meta", "shift"], key: "KeyV" }
  //     },
  //     "app/com.apple.Safari": {
  //       shortcut: { modifiers: ["meta", "alt"], key: "KeyS" }
  //     }
  //   }
  commands: {
    // Add your shortcuts here
  },

  // ============================================
  // WINDOW HOTKEYS
  // ============================================
  // notesHotkey: { modifiers: ["meta", "shift"], key: "KeyN" },
  // aiHotkey: { modifiers: ["meta", "shift"], key: "Space" },

  // ============================================
  // APPEARANCE
  // ============================================
  // editorFontSize: 14,
  // terminalFontSize: 14,
  // uiScale: 1.0,

  // ============================================
  // PATHS
  // ============================================
  // bun_path: "/opt/homebrew/bin/bun",
  // editor: "code",
} satisfies Config;
"#;

        let mut file = std::fs::File::create(path)?;
        file.write_all(template.as_bytes())?;
        logging::log(
            "UI",
            &format!("Created config.ts template: {}", path.display()),
        );
        Ok(())
    }

    /// Show the detached shortcut recorder popup for a command.
    ///
    /// This replaces `open_config_for_shortcut` for non-script commands.
    /// For scripts, we still open the script file directly to edit the // Shortcut: comment.
    ///
    /// # Arguments
    /// * `command_id` - The unique identifier for the command (e.g., "builtin/clipboard-history")
    /// * `command_name` - Human-readable name of the command
    /// * `cx` - The context for UI updates
    pub(crate) fn show_shortcut_recorder(
        &mut self,
        command_id: String,
        command_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "SHORTCUT",
            &format!(
                "Showing shortcut recorder for '{}' (id: {})",
                command_name, command_id
            ),
        );

        // Store state so parent key handlers treat the recorder as modal while
        // the native popup owns actual input.
        self.shortcut_recorder_state = Some(ShortcutRecorderState {
            command_id: command_id.clone(),
            command_name: command_name.clone(),
        });
        self.shortcut_recorder_entity = None;

        // Close actions popup if open
        self.clear_actions_popup_state();

        let app = cx.entity().downgrade();
        let theme = std::sync::Arc::clone(&self.theme);
        let parent_window_handle = window.window_handle();
        let parent_bounds = window.bounds();
        let display_id = window.display(cx).map(|display| display.id());
        let Some(parent_info) = crate::windows::automation_window_by_id("main") else {
            self.shortcut_recorder_state = None;
            self.show_error_toast("Shortcut recorder requires a registered main parent".to_string(), cx);
            return;
        };
        let host_policy = self.main_services.host_policy();

        cx.spawn(async move |this, cx| {
            cx.update(|cx| {
                if let Err(error) = open_shortcut_recorder_window(
                    cx,
                    app,
                    command_id,
                    command_name,
                    theme,
                    ShortcutRecorderParentWindow {
                        handle: parent_window_handle,
                        bounds: parent_bounds,
                        display_id,
                        info: parent_info,
                    },
                    host_policy,
                ) {
                    tracing::error!(
                        target: "script_kit::shortcut",
                        error = %error,
                        "Failed to open shortcut recorder popup"
                    );
                    let _ = this.update(cx, |app, cx| {
                        app.shortcut_recorder_state = None;
                        app.shortcut_recorder_entity = None;
                        app.show_error_toast(
                            format!("Failed to open shortcut recorder: {}", error),
                            cx,
                        );
                    });
                }
            });
        })
        .detach();

        cx.notify();
    }

    /// Close the shortcut recorder and clear state.
    /// Returns focus to the main filter input.
    pub fn close_shortcut_recorder(&mut self, cx: &mut Context<Self>) {
        if self.shortcut_recorder_state.is_some() || self.shortcut_recorder_entity.is_some() {
            logging::log(
                "SHORTCUT",
                "Closing shortcut recorder, returning focus to main filter",
            );
            self.shortcut_recorder_state = None;
            self.shortcut_recorder_entity = None;
            self.pending_focus = Some(FocusTarget::MainFilter);
            self.focused_input = FocusedInput::MainFilter;
            let generation = crate::windows::automation_window_by_id("shortcut-recorder-popup").and_then(|info| info.generation);
            let owned = self.main_services.host_policy().is_hidden();
            cx.spawn(async move |this, cx| {
                cx.update(|cx| {
                    if let Some(generation) = generation { let _ = close_shortcut_recorder_instance(generation, cx); }
                    if !owned { crate::platform::show_main_window_without_activation(); }
                });
                let _ = this.update(cx, |app, cx| {
                    app.pending_focus = Some(FocusTarget::MainFilter);
                    app.focused_input = FocusedInput::MainFilter;
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
        }
    }

    /// Handle saving a shortcut from the recorder.
    ///
    /// This saves the shortcut to config.ts and updates the live hotkey registry.
    pub(crate) fn handle_shortcut_save(
        &mut self,
        recorded: &crate::components::shortcut_recorder::RecordedShortcut,
        cx: &mut Context<Self>,
    ) {
        let Some(ref state) = self.shortcut_recorder_state else {
            logging::log("SHORTCUT", "No recorder state when trying to save");
            return;
        };

        let command_id = state.command_id.clone();
        let command_name = state.command_name.clone();

        // Convert RecordedShortcut to the persistence Shortcut type
        let shortcut = crate::shortcuts::Shortcut {
            key: recorded.key.clone().unwrap_or_default().to_lowercase(),
            modifiers: crate::shortcuts::Modifiers {
                cmd: recorded.cmd,
                ctrl: recorded.ctrl,
                alt: recorded.alt,
                shift: recorded.shift,
            },
        };

        logging::log(
            "SHORTCUT",
            &format!(
                "Saving shortcut for '{}' ({}): {}",
                command_name,
                command_id,
                shortcut.to_canonical_string()
            ),
        );

        let recorded_key = recorded.key.clone().unwrap_or_default();
        let shortcut_str = shortcut.to_canonical_string();
        if let MainServices::OwnedFixtures(sources) = &mut self.main_services {
            Arc::make_mut(sources).shortcut_overrides.insert(command_id, shortcut_str);
            self.mark_main_data_changed();
            crate::runtime_policy::record_completed_fixture_effect();
            self.close_shortcut_recorder(cx);
            return;
        }

        if let Some(conflict) =
            crate::hotkeys::shortcut_conflict_for_recording(&command_id, &shortcut_str)
        {
            tracing::warn!(
                command_id = %command_id,
                conflicting_command_id = %conflict.command_id,
                shortcut = %conflict.shortcut,
                "Shortcut save blocked by live route conflict"
            );
            self.show_error_toast(
                format!("Shortcut already used by {}", conflict.command_name),
                cx,
            );
            return;
        }

        // Save to config.ts via the same CLI path used by script-side tooling.
        match self.write_config_command_shortcut(
            &command_id,
            &recorded_key,
            recorded.cmd,
            recorded.ctrl,
            recorded.alt,
            recorded.shift,
        ) {
            Ok(()) => {
                logging::log("SHORTCUT", "Shortcut saved to config.ts commands");

                // Register the hotkey immediately so it works without restart
                match crate::hotkeys::update_script_hotkey(&command_id, None, Some(&shortcut_str)) {
                    Ok(()) => {
                        logging::log("SHORTCUT", "Registered config shortcut immediately");
                        self.show_hud(
                            format!("Shortcut set: {} (active now)", shortcut.display()),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                    Err(e) => {
                        // Shortcut saved but couldn't register, so the config remains durable while
                        // the live binding stays inactive until the user resolves the OS/global conflict.
                        logging::log(
                            "SHORTCUT",
                            &format!(
                                "Shortcut saved but registration failed: {} - not active now",
                                e
                            ),
                        );
                        self.show_hud(
                            format!("Shortcut saved: {} (not active now)", shortcut.display()),
                            Some(HUD_LONG_MS),
                            cx,
                        );
                    }
                }
                self.refresh_scripts(cx);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to save shortcut");
                self.show_error_toast(format!("Failed to save shortcut: {}", e), cx);
            }
        }

        // Close the recorder and restore focus
        self.close_shortcut_recorder(cx);
    }
}
