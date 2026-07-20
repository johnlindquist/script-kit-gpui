//! Detached Flow Manager window (protocol §7).
//!
//! One global window following the detached-chat pattern
//! (`src/ai/agent_chat/ui/chat_window.rs`): `OnceLock<Mutex<Option<state>>>`
//! handle slot, reopen focuses the existing window, close never cancels
//! runs. Creation happens only from event/deferred contexts — callers must
//! never invoke `open_flow_manager_window` during draw.
//!
//! Also the home surface for the Mission Control variation: runs dominate,
//! a compact "run a flow" picker sits above them.

use std::sync::{Mutex, OnceLock};

use gpui::{
    div, prelude::*, px, rgb, rgba, AnyWindowHandle, App, FocusHandle, Focusable, Window,
    WindowBounds, WindowKind, WindowOptions,
};

use super::catalog::{flow_catalog, RosterStatus};
use super::model::{EngagementMode, FlowUxVariant, RunPhase};
use super::run_registry::flow_run_registry;
use super::runner::{cancel_run, launch_flow};
use crate::theme;

pub const FLOW_MANAGER_AUTOMATION_ID: &str = "flowManager";

struct ManagerWindowState {
    handle: AnyWindowHandle,
}

static FLOW_MANAGER_WINDOW: OnceLock<Mutex<Option<ManagerWindowState>>> = OnceLock::new();

/// cwd used for the manager's compact picker roster; updated on every launch
/// so the manager follows the user's working context.
static LAST_FLOW_CWD: Mutex<Option<String>> = Mutex::new(None);

pub fn remember_flow_cwd(cwd: &str) {
    if let Ok(mut guard) = LAST_FLOW_CWD.lock() {
        *guard = Some(cwd.to_string());
    }
}

pub fn last_flow_cwd() -> Option<String> {
    LAST_FLOW_CWD.lock().ok().and_then(|guard| guard.clone())
}

pub fn manager_picker_cwd() -> String {
    // Same resolver as the Flow UX surfaces — the manager and the launcher
    // must never disagree about which project's flows they are showing.
    super::resolve_flow_cwd(None)
}

pub fn is_flow_manager_window_open() -> bool {
    let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    guard.is_some()
}

/// True when `window` IS the Flow Manager window. Global keystroke
/// interceptors must treat the manager as a secondary surface that owns its
/// own keys — leasing `ScriptListApp` from inside a manager-window dispatch
/// re-enters the entity and panics when the dispatch originated from the
/// stdin automation path.
pub fn is_flow_manager_window(window: &gpui::Window) -> bool {
    let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = slot.lock() {
        if let Some(state) = guard.as_ref() {
            return window.window_handle() == state.handle;
        }
    }
    false
}

fn clear_manager_window_slot() {
    let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        if guard.take().is_some() {
            crate::windows::remove_runtime_window_handle(FLOW_MANAGER_AUTOMATION_ID);
            crate::windows::remove_automation_window(FLOW_MANAGER_AUTOMATION_ID);
        }
    }
}

fn manager_window_bounds() -> WindowBounds {
    WindowBounds::Windowed(gpui::Bounds {
        origin: gpui::Point {
            x: px(120.0),
            y: px(120.0),
        },
        size: gpui::Size {
            width: px(760.0),
            height: px(520.0),
        },
    })
}

fn manager_automation_bounds() -> Option<crate::protocol::AutomationWindowBounds> {
    let WindowBounds::Windowed(bounds) = manager_window_bounds() else {
        return None;
    };
    Some(crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    })
}

/// Open the Flow Manager, or focus it when it already exists. Never
/// duplicates the window; closing it leaves every run alive.
pub fn open_flow_manager_window(cx: &mut App) -> anyhow::Result<()> {
    let existing = {
        let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|g| g.as_ref().map(|s| s.handle))
    };
    if let Some(handle) = existing {
        let _ = handle.update(cx, |_root, window, _cx| {
            window.activate_window();
        });
        crate::windows::upsert_automation_window(manager_automation_info(true));
        return Ok(());
    }

    let window_background = if theme::get_cached_theme().is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };
    let options = WindowOptions {
        window_bounds: Some(manager_window_bounds()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Flow Manager".into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui::Point {
                x: px(8.0),
                y: px(7.0),
            }),
        }),
        is_movable: true,
        window_background,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        ..Default::default()
    };

    let handle = cx.open_window(options, |window, cx| {
        window.on_window_should_close(cx, |_window, _cx| {
            // Closing the manager must never cancel runs — it only drops the
            // window and its automation registrations (protocol §7).
            clear_manager_window_slot();
            true
        });
        let view = cx.new(|cx| FlowManagerApp::new(window, cx));
        let focus_handle = view.read(cx).focus_handle.clone();
        window.focus(&focus_handle, cx);
        view
    })?;

    let _ = handle.update(cx, |_root, window, _cx| {
        crate::platform::configure_overlay_window_glass(window, "Flow Manager");
    });

    let any_handle: AnyWindowHandle = handle.into();
    {
        let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(ManagerWindowState { handle: any_handle });
        }
    }
    crate::windows::upsert_runtime_window_handle(FLOW_MANAGER_AUTOMATION_ID, any_handle);
    crate::windows::upsert_automation_window(manager_automation_info(true));
    Ok(())
}

fn manager_automation_info(focused: bool) -> crate::protocol::AutomationWindowInfo {
    crate::protocol::AutomationWindowInfo {
        id: FLOW_MANAGER_AUTOMATION_ID.to_string(),
        kind: crate::protocol::AutomationWindowKind::FlowManager,
        title: Some("Flow Manager".to_string()),
        focused,
        visible: true,
        semantic_surface: Some("flowManager".to_string()),
        bounds: manager_automation_bounds(),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(std::process::id()),
    }
}

pub fn close_flow_manager_window(cx: &mut App) {
    let handle = {
        let slot = FLOW_MANAGER_WINDOW.get_or_init(|| Mutex::new(None));
        slot.lock().ok().and_then(|g| g.as_ref().map(|s| s.handle))
    };
    if let Some(handle) = handle {
        let _ = handle.update(cx, |_root, window, _cx| {
            window.remove_window();
        });
    }
    clear_manager_window_slot();
}

/// Manager state surfaced into the `flowUx` automation snapshot.
pub fn manager_automation_state() -> (bool, Option<u64>) {
    let visible = is_flow_manager_window_open();
    let focused = if visible {
        flow_run_registry().selected_id()
    } else {
        None
    };
    (visible, focused)
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Which pane arrow keys drive: the compact picker (launch new work) or the
/// run list (supervise existing work).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ManagerZone {
    Picker,
    Runs,
}

pub struct FlowManagerApp {
    pub focus_handle: FocusHandle,
    zone: ManagerZone,
    picker_index: usize,
    seen_generation: u64,
}

impl FlowManagerApp {
    pub fn new(_window: &mut Window, cx: &mut gpui::Context<Self>) -> Self {
        // Poll the registry generation so streaming output repaints without
        // cross-thread waker plumbing; 150ms is imperceptible for a
        // supervision surface and costs nothing when nothing changed.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let alive = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    let generation = flow_run_registry().generation();
                    if generation != view.seen_generation {
                        view.seen_generation = generation;
                        cx.notify();
                    }
                })
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            zone: ManagerZone::Runs,
            picker_index: 0,
            seen_generation: 0,
        }
    }

    fn handle_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        use crate::ui_foundation::{is_key_down, is_key_enter, is_key_escape, is_key_up};
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform;
        let registry = flow_run_registry();

        if is_key_escape(key) {
            window.remove_window();
            clear_manager_window_slot();
            return;
        }
        if key == "tab" {
            self.zone = match self.zone {
                ManagerZone::Picker => ManagerZone::Runs,
                ManagerZone::Runs => ManagerZone::Picker,
            };
            cx.notify();
            return;
        }
        if cmd && key == "k" {
            registry.clear_finished();
            cx.notify();
            return;
        }
        if cmd && key == "backspace" {
            if let Some(selected) = registry.selected_id() {
                cancel_run(selected);
                cx.notify();
            }
            return;
        }
        if cmd && key == "c" {
            if let Some(run) = registry.selected_id().and_then(|id| registry.get(id)) {
                // Interleaved tail: a late stderr diagnostic must land in the
                // copied output, not vanish behind stdout.
                let text: Vec<String> = run.merged_tail.lines().map(str::to_string).collect();
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.join("\n")));
            }
            return;
        }

        match self.zone {
            ManagerZone::Picker => {
                let roster = flow_catalog().roster_for(&manager_picker_cwd());
                let count = roster.flows.len();
                if is_key_up(key) {
                    self.picker_index = self.picker_index.saturating_sub(1);
                    cx.notify();
                } else if is_key_down(key) {
                    if count > 0 && self.picker_index < count - 1 {
                        self.picker_index += 1;
                    }
                    cx.notify();
                } else if is_key_enter(key) {
                    if let Some(flow) = roster.flows.get(self.picker_index) {
                        let cwd = manager_picker_cwd();
                        remember_flow_cwd(&cwd);
                        launch_flow(
                            &flow.id,
                            &flow.name,
                            &flow.path,
                            &cwd,
                            FlowUxVariant::MissionControl,
                            EngagementMode::ManagerFocused,
                            Vec::new(),
                            std::time::Instant::now(),
                            false,
                        );
                        self.zone = ManagerZone::Runs;
                        cx.notify();
                    }
                }
            }
            ManagerZone::Runs => {
                // Arrow movement must match the DISPLAYED order (render_runs
                // shows newest first via .rev()); walking raw snapshot order
                // makes Up move visually down.
                let runs: Vec<_> = registry.snapshot().into_iter().rev().collect();
                let selected_pos = registry
                    .selected_id()
                    .and_then(|id| runs.iter().position(|r| r.local_id == id))
                    .unwrap_or(0);
                if is_key_up(key) {
                    if selected_pos > 0 {
                        registry.select(runs[selected_pos - 1].local_id);
                        cx.notify();
                    }
                } else if is_key_down(key) {
                    if selected_pos + 1 < runs.len() {
                        registry.select(runs[selected_pos + 1].local_id);
                        cx.notify();
                    }
                } else if is_key_enter(key) {
                    if let Some(run) = runs.get(selected_pos) {
                        registry.set_engagement(run.local_id, EngagementMode::ManagerFocused);
                        cx.notify();
                    }
                }
            }
        }
    }

    fn render_run_row(
        &self,
        run: &super::run_registry::FlowRun,
        selected: bool,
        colors: crate::list_item::ListItemColors,
    ) -> gpui::AnyElement {
        let phase_icon = match run.phase {
            RunPhase::Starting => "◌",
            RunPhase::Running => "●",
            RunPhase::Cancelling => "◍",
            RunPhase::Succeeded => "✓",
            RunPhase::Failed => "✕",
            RunPhase::Cancelled => "⊘",
        };
        let subtitle = format!(
            "{} · {} · {}",
            run.display_status(),
            format_elapsed(run.elapsed_ms()),
            run.last_output_line().unwrap_or("—"),
        );
        crate::list_item::ListItem::new(run.flow_name.clone(), colors)
            .description_opt(Some(subtitle))
            .icon(phase_icon)
            .selected(selected)
            .into_any_element()
    }
}

fn format_elapsed(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}m{:02}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

impl Focusable for FlowManagerApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FlowManagerApp {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = theme::get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let list_colors = crate::list_item::ListItemColors::from_theme(&theme);
        let registry = flow_run_registry();
        let runs = registry.snapshot();
        let selected_id = registry.selected_id();
        let cwd = manager_picker_cwd();
        let roster = flow_catalog().roster_for(&cwd);
        self.picker_index = self.picker_index.min(roster.flows.len().saturating_sub(1));

        let selected_run = selected_id.and_then(|id| registry.get(id));

        // --- compact picker strip ---------------------------------------
        let picker_active = self.zone == ManagerZone::Picker;
        let picker_hint = match roster.status {
            RosterStatus::Ready if roster.flows.is_empty() => {
                "No flows found in this project".to_string()
            }
            RosterStatus::Ready => {
                let flow = roster
                    .flows
                    .get(self.picker_index)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                format!("↑↓ pick · ↵ run — {flow}")
            }
            RosterStatus::Loading => "Loading flows…".to_string(),
            RosterStatus::Legacy => {
                "mdflow is pre-protocol — upgrade to use the picker".to_string()
            }
            RosterStatus::Error => roster
                .warnings
                .first()
                .cloned()
                .unwrap_or_else(|| "Roster unavailable".to_string()),
        };
        let picker = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgba(if picker_active {
                chrome.selection_rgba
            } else {
                chrome.input_surface_rgba
            }))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(chrome.accent_hex))
                    .child("New flow"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(chrome.text_secondary_hex))
                    .child(picker_hint),
            );

        // --- runs list ----------------------------------------------------
        let runs_active = self.zone == ManagerZone::Runs;
        let run_rows: Vec<gpui::AnyElement> = if runs.is_empty() {
            vec![div()
                .p_4()
                .text_sm()
                .text_color(rgb(chrome.text_muted_hex))
                .child("No runs yet — launch a flow from the picker or any Flow UX surface.")
                .into_any_element()]
        } else {
            runs.iter()
                .rev() // newest first
                .map(|run| {
                    let selected = runs_active && Some(run.local_id) == selected_id;
                    self.render_run_row(run, selected, list_colors)
                })
                .collect()
        };

        // --- detail pane ----------------------------------------------------
        let detail: gpui::AnyElement = match selected_run {
            Some(run) => {
                // Interleaved stdout+stderr so a late error is always
                // visible in the pane the user is actually looking at.
                let lines: Vec<String> = run.merged_tail.lines().map(str::to_string).collect();
                let shown: Vec<String> = lines.iter().rev().take(24).rev().cloned().collect();
                let steps: Vec<gpui::AnyElement> = run
                    .steps
                    .iter()
                    .map(|(id, step)| {
                        let mark = if step.completed {
                            if step.cached {
                                "▣"
                            } else {
                                "✓"
                            }
                        } else {
                            "…"
                        };
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_secondary_hex))
                            .child(format!("{mark} {id}"))
                            .into_any_element()
                    })
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(chrome.text_primary_hex))
                            .child(format!(
                                "{} — {} · {}",
                                run.flow_name,
                                run.display_status(),
                                format_elapsed(run.elapsed_ms())
                            )),
                    )
                    .children(steps)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .mt_2()
                            .p_2()
                            .rounded_md()
                            .bg(rgba(chrome.preview_surface_rgba))
                            .overflow_hidden()
                            .children(shown.into_iter().map(|line| {
                                div()
                                    .text_xs()
                                    .text_color(rgb(chrome.text_secondary_hex))
                                    .child(line)
                            })),
                    )
                    .into_any_element()
            }
            None => div()
                .p_4()
                .text_sm()
                .text_color(rgb(chrome.text_muted_hex))
                .child("Select a run to see its output.")
                .into_any_element(),
        };

        let footer = div()
            .flex()
            .gap_3()
            .px_3()
            .py_2()
            .text_xs()
            .text_color(rgb(chrome.text_muted_hex))
            .child("Tab Switch zone")
            .child("↵ Engage / Run")
            .child("⌘⌫ Cancel")
            .child("⌘K Clear finished")
            .child("⌘C Copy output")
            .child("Esc Close");

        div()
            .id("flow-manager-root")
            .key_context("FlowManager")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key(event, window, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgba(chrome.surface_rgba))
            .text_color(rgb(chrome.text_primary_hex))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .child(picker)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .gap_2()
                            .min_h(px(0.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .overflow_hidden()
                                    .children(run_rows),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
                                    .rounded_md()
                                    .bg(rgba(chrome.panel_surface_rgba))
                                    .child(detail),
                            ),
                    )
                    .flex_1()
                    .min_h(px(0.0)),
            )
            .child(footer)
    }
}
