use std::sync::{Mutex, OnceLock};

use gpui::{
    div, AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context, DisplayId, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, WindowHandle,
};

use crate::components::inline_dropdown::{
    inline_dropdown_visible_range_from_start, render_soft_compact_picker_row, InlineDropdown,
    InlineDropdownColors, SOFT_COMPACT_PICKER_ROW_HEIGHT,
};
use crate::components::inline_popup_window::{
    configure_inline_popup_window, inline_popup_height_for_row_height, inline_popup_window_options,
    set_inline_popup_window_bounds, INLINE_POPUP_EDGE_GUTTER, INLINE_POPUP_MAX_VISIBLE_ROWS,
    INLINE_POPUP_VERTICAL_PADDING,
};

use super::{
    apply_device_selection, microphone_display_label, DictationDeviceMenuItem,
    DictationDeviceSelectionAction, DictationOverlay,
};

pub(crate) const DICTATION_MICROPHONE_POPUP_AUTOMATION_ID: &str = "dictation-microphone-popup";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DictationMicrophonePopupRow {
    pub row_id: String,
    pub semantic_id: String,
    pub title: String,
    pub subtitle: String,
    pub action: DictationDeviceSelectionAction,
    pub is_selected: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DictationMicrophonePopupSnapshot {
    pub rows: Vec<DictationMicrophonePopupRow>,
    pub selected_row_id: Option<String>,
    pub visible_start: usize,
    pub visible_row_limit: usize,
    pub width: f32,
}

pub(crate) struct DictationMicrophonePopupRequest {
    pub parent_window_handle: AnyWindowHandle,
    pub parent_bounds: Bounds<Pixels>,
    pub display_bounds: Option<Bounds<Pixels>>,
    pub display_id: Option<DisplayId>,
    pub source_view: WeakEntity<DictationOverlay>,
    pub snapshot: DictationMicrophonePopupSnapshot,
}

struct DictationMicrophonePopupSlot {
    handle: WindowHandle<DictationMicrophonePopupWindow>,
    parent_window_handle: AnyWindowHandle,
}

static DICTATION_MICROPHONE_POPUP_WINDOW: OnceLock<Mutex<Option<DictationMicrophonePopupSlot>>> =
    OnceLock::new();

pub(crate) fn build_dictation_microphone_popup_snapshot(
    items: Vec<DictationDeviceMenuItem>,
    width: f32,
) -> DictationMicrophonePopupSnapshot {
    let mut selected_row_id = None;
    let rows = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let row_id = format!("dictation-mic-row-{idx}");
            let semantic_id = format!("choice:{idx}:{row_id}");
            if item.is_selected {
                selected_row_id = Some(row_id.clone());
            }
            DictationMicrophonePopupRow {
                row_id,
                semantic_id,
                title: microphone_display_label(&item.title),
                subtitle: item.subtitle,
                action: item.action,
                is_selected: item.is_selected,
            }
        })
        .collect();

    DictationMicrophonePopupSnapshot {
        rows,
        selected_row_id,
        visible_start: 0,
        visible_row_limit: INLINE_POPUP_MAX_VISIBLE_ROWS,
        width,
    }
}

fn dictation_microphone_popup_height(snapshot: &DictationMicrophonePopupSnapshot) -> f32 {
    inline_popup_height_for_row_height(
        snapshot.rows.len().min(snapshot.visible_row_limit),
        SOFT_COMPACT_PICKER_ROW_HEIGHT,
    )
}

fn dictation_microphone_popup_visible_row_limit(
    snapshot: &DictationMicrophonePopupSnapshot,
    available_height: f32,
) -> usize {
    let row_count = snapshot.rows.len();
    if row_count == 0 {
        return 0;
    }

    let hard_limit = row_count.min(INLINE_POPUP_MAX_VISIBLE_ROWS);
    (1..=hard_limit)
        .rev()
        .find(|rows| {
            let mut candidate = snapshot.clone();
            candidate.visible_row_limit = *rows;
            dictation_microphone_popup_height(&candidate) <= available_height.max(1.0)
        })
        .unwrap_or(1)
}

fn dictation_microphone_popup_bounds_above(
    parent_bounds: Bounds<Pixels>,
    display_bounds: Option<Bounds<Pixels>>,
    snapshot: &mut DictationMicrophonePopupSnapshot,
) -> Bounds<Pixels> {
    let width = snapshot.width;
    let display_top = display_bounds
        .map(|db| db.origin.y.as_f32() + INLINE_POPUP_EDGE_GUTTER)
        .unwrap_or(0.0);
    let available_height = (parent_bounds.origin.y.as_f32() - display_top).max(1.0);
    snapshot.visible_row_limit =
        dictation_microphone_popup_visible_row_limit(snapshot, available_height);
    let height = dictation_microphone_popup_height(snapshot);

    let preferred_left = parent_bounds.origin.x.as_f32();
    let left = display_bounds
        .map(|display_bounds| {
            let display_left = display_bounds.origin.x.as_f32();
            let display_right = display_left + display_bounds.size.width.as_f32();
            preferred_left.clamp(display_left, (display_right - width).max(display_left))
        })
        .unwrap_or(preferred_left);
    let top = parent_bounds.origin.y.as_f32() - height;

    Bounds {
        origin: gpui::point(gpui::px(left), gpui::px(top)),
        size: gpui::size(gpui::px(width), gpui::px(height)),
    }
}

pub(crate) fn sync_dictation_microphone_popup_window(
    cx: &mut App,
    request: DictationMicrophonePopupRequest,
) -> anyhow::Result<()> {
    let DictationMicrophonePopupRequest {
        parent_window_handle,
        parent_bounds,
        display_bounds,
        display_id,
        source_view,
        mut snapshot,
    } = request;
    let bounds =
        dictation_microphone_popup_bounds_above(parent_bounds, display_bounds, &mut snapshot);

    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        if let Some(slot) = guard.as_ref() {
            if slot.parent_window_handle == parent_window_handle {
                let update_result = slot.handle.update(cx, |popup, window, cx| {
                    popup.set_snapshot(snapshot.clone());
                    crate::windows::automation_surface_collector::upsert_dictation_microphone_prompt_popup_snapshot(
                        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                        &snapshot,
                    );
                    set_inline_popup_window_bounds(window, bounds, cx);
                    crate::windows::set_automation_bounds(
                        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                        Some(crate::protocol::AutomationWindowBounds {
                            x: f32::from(bounds.origin.x) as f64,
                            y: f32::from(bounds.origin.y) as f64,
                            width: f32::from(bounds.size.width) as f64,
                            height: f32::from(bounds.size.height) as f64,
                        }),
                    );
                    cx.notify();
                });
                if update_result.is_ok() {
                    return Ok(());
                }
                crate::windows::automation_surface_collector::remove_dictation_microphone_prompt_popup_snapshot(
                    DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                );
                crate::windows::remove_automation_window(DICTATION_MICROPHONE_POPUP_AUTOMATION_ID);
                *guard = None;
            } else {
                let _ = slot.handle.update(cx, |_popup, window, cx| {
                    crate::platform::dematerialize_then_remove_gpui_window(
                        window,
                        cx,
                        "DICTATION",
                        "Microphone popup",
                    );
                });
                crate::windows::automation_surface_collector::remove_dictation_microphone_prompt_popup_snapshot(
                    DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                );
                crate::windows::remove_automation_window(DICTATION_MICROPHONE_POPUP_AUTOMATION_ID);
                *guard = None;
            }
        }
    }

    let window_options = inline_popup_window_options(bounds, display_id);
    let handle = cx.open_window(window_options, |_window, cx| {
        cx.new(|cx| {
            DictationMicrophonePopupWindow::new(
                snapshot.clone(),
                source_view.clone(),
                parent_window_handle,
                cx,
            )
        })
    })?;

    if let Err(error) = configure_inline_popup_window(&handle, cx, parent_window_handle) {
        let _ = handle.update(cx, |_popup, window, _cx| {
            window.remove_window();
        });
        return Err(error.context("failed to configure dictation microphone popup window"));
    }

    let parent_automation_id =
        crate::windows::focused_automation_window_id().unwrap_or_else(|| "main".to_string());
    if let Err(error) = crate::windows::register_attached_popup(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID.to_string(),
        crate::protocol::AutomationWindowKind::PromptPopup,
        Some("Dictation Microphones".to_string()),
        Some("dictationMicrophonePopup".to_string()),
        Some(crate::protocol::AutomationWindowBounds {
            x: f32::from(bounds.origin.x) as f64,
            y: f32::from(bounds.origin.y) as f64,
            width: f32::from(bounds.size.width) as f64,
            height: f32::from(bounds.size.height) as f64,
        }),
        Some(parent_automation_id.as_str()),
    ) {
        let _ = handle.update(cx, |_popup, window, _cx| {
            window.remove_window();
        });
        return Err(error.context("failed to register dictation microphone popup window"));
    }

    crate::windows::automation_surface_collector::upsert_dictation_microphone_prompt_popup_snapshot(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
        &snapshot,
    );

    if let Ok(mut guard) = storage.lock() {
        *guard = Some(DictationMicrophonePopupSlot {
            handle,
            parent_window_handle,
        });
    }

    Ok(())
}

pub(crate) fn close_dictation_microphone_popup_window(cx: &mut App) {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get_or_init(|| Mutex::new(None));
    let handle = storage.lock().ok().and_then(|mut guard| guard.take());
    if let Some(slot) = handle {
        let _ = slot.handle.update(cx, |_popup, window, cx| {
            crate::platform::dematerialize_then_remove_gpui_window(
                window,
                cx,
                "DICTATION",
                "Microphone popup",
            );
        });
    }
    clear_dictation_microphone_popup_registration();
}

fn close_dictation_microphone_popup_from_entity(window: &mut Window, cx: &mut App) {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        guard.take();
    }
    clear_dictation_microphone_popup_registration();
    crate::platform::dematerialize_then_remove_gpui_window_from_app(
        window,
        cx,
        "DICTATION",
        "Microphone popup",
    );
}

fn clear_dictation_microphone_popup_registration() {
    crate::windows::automation_surface_collector::remove_dictation_microphone_prompt_popup_snapshot(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
    );
    crate::windows::remove_automation_window(DICTATION_MICROPHONE_POPUP_AUTOMATION_ID);
}

pub(crate) fn is_dictation_microphone_popup_window_open() -> bool {
    DICTATION_MICROPHONE_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok().map(|guard| guard.is_some()))
        .unwrap_or(false)
}

// Called from the binary crate's prompt_handler automation path; the
// library build compiles this module without that caller.
#[allow(dead_code)]
pub(crate) fn batch_select_dictation_microphone_popup_row_by_value(
    value: &str,
    cx: &mut App,
) -> Option<String> {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get()?;
    let slot = storage
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())?;
    slot.handle
        .update(cx, |popup, window, cx| {
            popup.accept_value(value, window, cx)
        })
        .ok()
        .flatten()
}

// See batch_select_dictation_microphone_popup_row_by_value.
#[allow(dead_code)]
pub(crate) fn batch_select_dictation_microphone_popup_row_by_semantic_id(
    semantic_id: &str,
    cx: &mut App,
) -> Option<String> {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get()?;
    let slot = storage
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())?;
    slot.handle
        .update(cx, |popup, window, cx| {
            popup.accept_semantic_id(semantic_id, window, cx)
        })
        .ok()
        .flatten()
}

impl Clone for DictationMicrophonePopupSlot {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            parent_window_handle: self.parent_window_handle,
        }
    }
}

pub(crate) struct DictationMicrophonePopupWindow {
    snapshot: DictationMicrophonePopupSnapshot,
    source_view: WeakEntity<DictationOverlay>,
    parent_window_handle: AnyWindowHandle,
    focus_handle: FocusHandle,
}

impl DictationMicrophonePopupWindow {
    fn new(
        snapshot: DictationMicrophonePopupSnapshot,
        source_view: WeakEntity<DictationOverlay>,
        parent_window_handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            snapshot,
            source_view,
            parent_window_handle,
            focus_handle: cx.focus_handle(),
        }
    }

    fn set_snapshot(&mut self, mut snapshot: DictationMicrophonePopupSnapshot) {
        snapshot.visible_start = self.visible_range().start;
        self.snapshot = snapshot;
    }

    fn selected_index(&self) -> Option<usize> {
        let selected_id = self.snapshot.selected_row_id.as_deref()?;
        self.snapshot
            .rows
            .iter()
            .position(|row| row.row_id == selected_id)
    }

    fn visible_range(&self) -> std::ops::Range<usize> {
        let row_count = self.snapshot.rows.len();
        if row_count == 0 {
            return 0..0;
        }
        let selected_index = self
            .selected_index()
            .unwrap_or_else(|| self.snapshot.visible_start.min(row_count.saturating_sub(1)));
        inline_dropdown_visible_range_from_start(
            self.snapshot.visible_start,
            selected_index,
            row_count,
            self.snapshot
                .visible_row_limit
                .clamp(1, INLINE_POPUP_MAX_VISIBLE_ROWS),
        )
    }

    fn select_row(&mut self, row_index: usize, cx: &mut Context<Self>) {
        let Some(row) = self.snapshot.rows.get(row_index) else {
            return;
        };
        self.snapshot.selected_row_id = Some(row.row_id.clone());
        cx.notify();
    }

    fn accept_row(
        &mut self,
        row_index: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        let row = self.snapshot.rows.get(row_index)?.clone();
        if let Err(error) = apply_device_selection(&row.action) {
            tracing::warn!(
                category = "DICTATION",
                error = %error,
                "Failed to persist microphone selection from dictation popup"
            );
            return None;
        }
        tracing::info!(
            category = "DICTATION",
            microphone = %row.title,
            row_id = %row.row_id,
            "Dictation microphone popup updated preference"
        );
        // The live AVCaptureSession keeps the mic it opened with — surface the
        // pending switch in the overlay footer instead of silently applying
        // the change one session late.
        if crate::dictation::is_dictation_recording() {
            crate::dictation::set_pending_dictation_device_label(Some(row.title.to_string()));
        }
        if let Some(view) = self.source_view.upgrade() {
            let _ = cx.update_window(self.parent_window_handle, |_entity, _window, cx| {
                view.update(cx, |_overlay, cx| {
                    cx.notify();
                });
            });
        }
        close_dictation_microphone_popup_from_entity(window, cx);
        Some(row.row_id)
    }

    fn accept_value(&mut self, value: &str, window: &mut Window, cx: &mut App) -> Option<String> {
        let row_index = self
            .snapshot
            .rows
            .iter()
            .position(|row| row.row_id == value)?;
        self.accept_row(row_index, window, cx)
    }

    fn accept_semantic_id(
        &mut self,
        semantic_id: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        let row_index = self
            .snapshot
            .rows
            .iter()
            .position(|row| row.semantic_id == semantic_id)?;
        self.accept_row(row_index, window, cx)
    }

    fn handle_row_click(
        &mut self,
        index: usize,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.accept_row(index, window, cx);
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let row_count = self.snapshot.rows.len();
        if row_count == 0 {
            cx.propagate();
            return;
        }

        let current = self.selected_index().unwrap_or(0);
        if crate::ui_foundation::is_key_down(key) {
            self.select_row((current + 1) % row_count, cx);
            cx.stop_propagation();
            return;
        }
        if crate::ui_foundation::is_key_up(key) {
            let next = if current == 0 {
                row_count - 1
            } else {
                current - 1
            };
            self.select_row(next, cx);
            cx.stop_propagation();
            return;
        }
        if crate::ui_foundation::is_key_enter(key) {
            let _ = self.accept_row(current, window, cx);
            cx.stop_propagation();
            return;
        }
        if crate::ui_foundation::is_key_escape(key) {
            close_dictation_microphone_popup_from_entity(window, cx);
            cx.stop_propagation();
            return;
        }
        cx.propagate();
    }

    fn render_picker_row(
        &self,
        idx: usize,
        row: &DictationMicrophonePopupRow,
        is_selected: bool,
        colors: InlineDropdownColors,
    ) -> gpui::Stateful<gpui::Div> {
        render_soft_compact_picker_row(
            SharedString::from(format!("dictation-microphone-popup-row-{idx}")),
            row.title.clone().into(),
            Some(row.subtitle.clone().into()),
            &[],
            &[],
            is_selected,
            colors,
        )
    }

    fn render_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = crate::theme::get_cached_theme();
        let colors = InlineDropdownColors::popup_from_theme(&theme);
        let visible = self.visible_range();
        let selected_index = self.selected_index();
        let visible_rows: Vec<_> = self
            .snapshot
            .rows
            .iter()
            .enumerate()
            .skip(visible.start)
            .take(visible.len())
            .collect();

        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .children(visible_rows.into_iter().map(|(idx, row)| {
                let is_selected = selected_index == Some(idx);
                self.render_picker_row(idx, row, is_selected, colors)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, event, window, cx| {
                        this.handle_row_click(idx, event, window, cx);
                    }))
                    .into_any_element()
            }))
            .into_any_element();

        InlineDropdown::new(
            SharedString::from("dictation-microphone-popup"),
            body,
            colors,
        )
        .vertical_padding(INLINE_POPUP_VERTICAL_PADDING / 2.0)
        .into_any_element()
    }
}

impl Focusable for DictationMicrophonePopupWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DictationMicrophonePopupWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.render_picker(cx))
    }
}
