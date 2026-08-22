use std::sync::{Mutex, OnceLock};

use anyhow::Context as _;
use gpui::{
    div, AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context, DisplayId, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    WindowHandle,
};

use crate::components::inline_dropdown::{
    inline_dropdown_visible_range_from_start, render_soft_compact_picker_row, InlineDropdown,
    InlineDropdownColors, SOFT_COMPACT_PICKER_ROW_HEIGHT,
};
use crate::components::inline_popup_window::{
    configure_inline_popup_window_lifecycle, inline_popup_height_for_row_height,
    inline_popup_window_options, set_inline_popup_window_bounds, InlinePopupAttachResult,
    InlinePopupCloseGate, InlinePopupFocusReturn, InlinePopupGeneration, InlinePopupLifecycle,
    InlinePopupLifecycleHandle, InlinePopupPhase, INLINE_POPUP_EDGE_GUTTER,
    INLINE_POPUP_MAX_VISIBLE_ROWS, INLINE_POPUP_VERTICAL_PADDING,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DictationMicrophonePopupSelectionMode {
    Production,
    FixtureNoPersistence,
}

impl DictationMicrophonePopupSelectionMode {
    const fn persists_selection(self) -> bool {
        matches!(self, Self::Production)
    }
}

pub(crate) struct DictationMicrophonePopupRequest {
    pub parent_window_handle: AnyWindowHandle,
    pub parent_automation_id: String,
    pub parent_bounds: Bounds<Pixels>,
    pub display_bounds: Option<Bounds<Pixels>>,
    pub display_id: Option<DisplayId>,
    pub source_view: WeakEntity<DictationOverlay>,
    pub snapshot: DictationMicrophonePopupSnapshot,
    pub selection_mode: DictationMicrophonePopupSelectionMode,
    pub lifecycle: InlinePopupLifecycleHandle,
    pub focus_return: InlinePopupFocusReturn,
}

struct DictationMicrophonePopupSlot {
    handle: WindowHandle<DictationMicrophonePopupWindow>,
    parent_window_handle: AnyWindowHandle,
    generation: InlinePopupGeneration,
    lifecycle: InlinePopupLifecycleHandle,
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

fn unregister_dictation_microphone_popup_automation_window(generation: InlinePopupGeneration) {
    crate::windows::automation_surface_collector::remove_dictation_microphone_prompt_popup_snapshot_if_generation(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
        generation.get(),
    );
    crate::windows::remove_runtime_window_handle_if_generation(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
        generation.get(),
    );
    crate::windows::remove_automation_window_if_generation(
        DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
        generation.get(),
    );
}

fn clear_dictation_microphone_popup_window_slot(generation: InlinePopupGeneration) {
    if let Some(storage) = DICTATION_MICROPHONE_POPUP_WINDOW.get() {
        if let Ok(mut guard) = storage.lock() {
            if guard
                .as_ref()
                .is_some_and(|slot| slot.generation == generation)
            {
                *guard = None;
            }
        }
    }
}

fn reconcile_dictation_microphone_popup_native_close(
    generation: InlinePopupGeneration,
    lifecycle: &InlinePopupLifecycleHandle,
    source_view: &WeakEntity<DictationOverlay>,
    focus_return: &InlinePopupFocusReturn,
    cx: &mut App,
) {
    match InlinePopupLifecycle::request_close(lifecycle, generation) {
        InlinePopupCloseGate::StaleGeneration | InlinePopupCloseGate::AlreadyClosed => return,
        InlinePopupCloseGate::Begin | InlinePopupCloseGate::AlreadyClosing => {}
    }
    unregister_dictation_microphone_popup_automation_window(generation);
    if let Some(view) = source_view.upgrade() {
        view.update(cx, |view, cx| {
            view.dismiss_microphone_popup_from_window(generation, "native_close", cx);
        });
    }
    let _ = focus_return.restore(generation, cx);
    let _ = InlinePopupLifecycle::mark_closed(lifecycle, generation);
    clear_dictation_microphone_popup_window_slot(generation);
}

pub(crate) fn sync_dictation_microphone_popup_window(
    cx: &mut App,
    request: DictationMicrophonePopupRequest,
) -> anyhow::Result<()> {
    let DictationMicrophonePopupRequest {
        parent_window_handle,
        parent_automation_id,
        parent_bounds,
        display_bounds,
        display_id,
        source_view,
        mut snapshot,
        selection_mode,
        lifecycle,
        focus_return,
    } = request;
    let generation = InlinePopupLifecycle::generation(&lifecycle);
    let bounds =
        dictation_microphone_popup_bounds_above(parent_bounds, display_bounds, &mut snapshot);
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = storage.lock() {
        if let Some(slot) = guard.as_ref() {
            if slot.parent_window_handle == parent_window_handle && slot.generation == generation {
                let update_result = slot.handle.update(cx, |popup, window, cx| {
                    popup.set_snapshot(snapshot.clone());
                    set_inline_popup_window_bounds(window, bounds, cx);
                    cx.notify();
                });
                if update_result.is_ok() {
                    if InlinePopupLifecycle::snapshot(&slot.lifecycle).1 == InlinePopupPhase::Open {
                        crate::windows::automation_surface_collector::upsert_dictation_microphone_prompt_popup_snapshot(
                            DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                            generation.get(),
                            &snapshot,
                        );
                        crate::windows::set_automation_bounds(
                            DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                            Some(crate::protocol::AutomationWindowBounds {
                                x: f32::from(bounds.origin.x) as f64,
                                y: f32::from(bounds.origin.y) as f64,
                                width: f32::from(bounds.size.width) as f64,
                                height: f32::from(bounds.size.height) as f64,
                            }),
                        );
                    }
                    return Ok(());
                }
            }
            anyhow::bail!(
                "Dictation microphone popup lifetime is still owned by generation {}",
                slot.generation.get()
            );
        }
    }

    let window_options = inline_popup_window_options(bounds, display_id);
    let native_source_view = source_view.clone();
    let native_lifecycle = lifecycle.clone();
    let native_focus_return = focus_return.clone();
    let entity_source_view = source_view.clone();
    let entity_lifecycle = lifecycle.clone();
    let entity_focus_return = focus_return.clone();
    let entity_snapshot = snapshot.clone();
    let automation_snapshot = snapshot.clone();
    let handle = cx.open_window(window_options, move |window, cx| {
        window.on_window_should_close(cx, move |_window, cx| {
            reconcile_dictation_microphone_popup_native_close(
                generation,
                &native_lifecycle,
                &native_source_view,
                &native_focus_return,
                cx,
            );
            true
        });
        cx.new(|cx| {
            DictationMicrophonePopupWindow::new(
                entity_snapshot.clone(),
                entity_source_view.clone(),
                parent_window_handle,
                selection_mode,
                generation,
                entity_lifecycle.clone(),
                entity_focus_return.clone(),
                cx,
            )
        })
    })?;

    if let Ok(mut guard) = storage.lock() {
        *guard = Some(DictationMicrophonePopupSlot {
            handle,
            parent_window_handle,
            generation,
            lifecycle: lifecycle.clone(),
        });
    }

    let any_handle: AnyWindowHandle = handle.into();
    if let Err(error) = configure_inline_popup_window_lifecycle(
        handle,
        parent_window_handle,
        parent_automation_id.clone(),
        lifecycle.clone(),
        cx,
        move |result, cx| match result {
            InlinePopupAttachResult::Ready(receipt) => {
                crate::windows::upsert_runtime_window_handle_instance(
                    DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                    any_handle,
                    Some(receipt.generation.get()),
                );
                if let Err(error) = crate::windows::register_attached_popup_instance(
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
                    Some(receipt.generation.get()),
                ) {
                    tracing::warn!(
                        target: "script_kit::automation",
                        event = "dictation_microphone_popup_registry_failed",
                        generation = receipt.generation.get(),
                        error = %error,
                    );
                    let _ = handle.update(cx, |popup, window, cx| {
                        popup.request_close(window, cx, "registry_failed", true, true);
                    });
                    return;
                }
                crate::windows::automation_surface_collector::upsert_dictation_microphone_prompt_popup_snapshot(
                    DICTATION_MICROPHONE_POPUP_AUTOMATION_ID,
                    receipt.generation.get(),
                    &automation_snapshot,
                );
            }
            InlinePopupAttachResult::Failed {
                generation,
                failure,
            } => {
                tracing::warn!(
                    target: "script_kit::inline_popup",
                    event = "dictation_microphone_popup_attach_failed",
                    generation = generation.get(),
                    failure = ?failure,
                );
                let _ = handle.update(cx, |popup, window, cx| {
                    popup.request_close(window, cx, "attach_failed", true, true);
                });
            }
        },
    ) {
        unregister_dictation_microphone_popup_automation_window(generation);
        clear_dictation_microphone_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::request_close(&lifecycle, generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
        let _ = handle.update(cx, |_popup, window, _cx| window.remove_window());
        return Err(error.context("failed to schedule Dictation microphone popup attach"));
    }

    Ok(())
}

pub(crate) fn close_dictation_microphone_popup_window(cx: &mut App) {
    close_dictation_microphone_popup_window_with_policy("owner_state_closed", true, cx);
}

pub(crate) fn close_dictation_microphone_popup_window_for_owner_loss(cx: &mut App) {
    close_dictation_microphone_popup_window_with_policy("owner_loss", false, cx);
}

fn close_dictation_microphone_popup_window_with_policy(
    reason: &'static str,
    restore_focus: bool,
    cx: &mut App,
) {
    let current = DICTATION_MICROPHONE_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|slot| (slot.handle, slot.generation, slot.lifecycle.clone()))
        });
    let Some((handle, generation, lifecycle)) = current else {
        return;
    };

    if handle
        .update(cx, |popup, window, cx| {
            popup.request_close(window, cx, reason, restore_focus, true);
        })
        .is_err()
    {
        let _ = InlinePopupLifecycle::request_close(&lifecycle, generation);
        unregister_dictation_microphone_popup_automation_window(generation);
        clear_dictation_microphone_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DictationPopupDismissOutcome {
    ClosedCurrentTopLayer,
    ReconciledStaleSlot,
    NotPresent,
}

pub(crate) fn dismiss_dictation_microphone_popup_from_parent(
    expected_generation: Option<InlinePopupGeneration>,
    reason: &'static str,
    cx: &mut App,
) -> DictationPopupDismissOutcome {
    let current = DICTATION_MICROPHONE_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|slot| (slot.handle, slot.generation, slot.lifecycle.clone()))
        });
    let Some((handle, generation, lifecycle)) = current else {
        return DictationPopupDismissOutcome::NotPresent;
    };

    if expected_generation.is_some_and(|expected| expected != generation) {
        return DictationPopupDismissOutcome::ReconciledStaleSlot;
    }

    let phase = InlinePopupLifecycle::snapshot(&lifecycle).1;
    if phase != InlinePopupPhase::Open {
        if matches!(
            phase,
            InlinePopupPhase::CreatedHidden
                | InlinePopupPhase::AttachPending
                | InlinePopupPhase::Closing
        ) {
            let _ = handle.update(cx, |popup, window, cx| {
                popup.request_close(window, cx, reason, true, false);
            });
        } else {
            unregister_dictation_microphone_popup_automation_window(generation);
            clear_dictation_microphone_popup_window_slot(generation);
        }
        return DictationPopupDismissOutcome::ReconciledStaleSlot;
    }

    if handle
        .update(cx, |popup, window, cx| {
            popup.request_close(window, cx, reason, true, false);
        })
        .is_ok()
    {
        if expected_generation == Some(generation) {
            DictationPopupDismissOutcome::ClosedCurrentTopLayer
        } else {
            DictationPopupDismissOutcome::ReconciledStaleSlot
        }
    } else {
        unregister_dictation_microphone_popup_automation_window(generation);
        clear_dictation_microphone_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::request_close(&lifecycle, generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
        DictationPopupDismissOutcome::ReconciledStaleSlot
    }
}

#[allow(
    dead_code,
    reason = "the separately compiled application binary consumes this popup state in its prompt handler"
)]
pub(crate) fn is_dictation_microphone_popup_window_open() -> bool {
    DICTATION_MICROPHONE_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| guard.as_ref().map(|slot| slot.lifecycle.clone()))
        .is_some_and(|lifecycle| {
            InlinePopupLifecycle::snapshot(&lifecycle).1 == InlinePopupPhase::Open
        })
}

// Called from the binary crate's prompt_handler automation path; the
// library build compiles this module without that caller.
#[allow(dead_code)]
pub(crate) fn batch_select_dictation_microphone_popup_row_by_value(
    generation: u64,
    value: &str,
    cx: &mut App,
) -> Option<String> {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get()?;
    let slot = storage
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())?;
    if slot.generation.get() != generation
        || InlinePopupLifecycle::snapshot(&slot.lifecycle).1 != InlinePopupPhase::Open
    {
        return None;
    }
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
    generation: u64,
    semantic_id: &str,
    cx: &mut App,
) -> Option<String> {
    let storage = DICTATION_MICROPHONE_POPUP_WINDOW.get()?;
    let slot = storage
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())?;
    if slot.generation.get() != generation
        || InlinePopupLifecycle::snapshot(&slot.lifecycle).1 != InlinePopupPhase::Open
    {
        return None;
    }
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
            generation: self.generation,
            lifecycle: self.lifecycle.clone(),
        }
    }
}

pub(crate) struct DictationMicrophonePopupWindow {
    snapshot: DictationMicrophonePopupSnapshot,
    source_view: WeakEntity<DictationOverlay>,
    parent_window_handle: AnyWindowHandle,
    selection_mode: DictationMicrophonePopupSelectionMode,
    generation: InlinePopupGeneration,
    lifecycle: InlinePopupLifecycleHandle,
    focus_return: InlinePopupFocusReturn,
    focus_handle: FocusHandle,
    activation_subscription: Option<Subscription>,
    focus_pair_was_active: bool,
}

impl DictationMicrophonePopupWindow {
    #[expect(
        clippy::too_many_arguments,
        reason = "each independently owned parent, lifecycle, generation, and focus fact must remain explicit"
    )]
    fn new(
        snapshot: DictationMicrophonePopupSnapshot,
        source_view: WeakEntity<DictationOverlay>,
        parent_window_handle: AnyWindowHandle,
        selection_mode: DictationMicrophonePopupSelectionMode,
        generation: InlinePopupGeneration,
        lifecycle: InlinePopupLifecycleHandle,
        focus_return: InlinePopupFocusReturn,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            snapshot,
            source_view,
            parent_window_handle,
            selection_mode,
            generation,
            lifecycle,
            focus_return,
            focus_handle: cx.focus_handle(),
            activation_subscription: None,
            focus_pair_was_active: false,
        }
    }

    fn request_close(
        &self,
        window: &mut Window,
        cx: &mut App,
        reason: &'static str,
        restore_focus: bool,
        reconcile_owner: bool,
    ) {
        if InlinePopupLifecycle::request_close(&self.lifecycle, self.generation)
            != InlinePopupCloseGate::Begin
        {
            return;
        }

        unregister_dictation_microphone_popup_automation_window(self.generation);
        if reconcile_owner {
            if let Some(view) = self.source_view.upgrade() {
                view.update(cx, |view, cx| {
                    view.dismiss_microphone_popup_from_window(self.generation, reason, cx);
                });
            }
        }

        if restore_focus {
            let focus_return = self.focus_return.clone();
            let generation = self.generation;
            cx.defer(move |cx| {
                let restored = focus_return.restore(generation, cx);
                tracing::info!(
                    target: "script_kit::inline_popup",
                    event = "dictation_microphone_popup_focus_restore",
                    generation = generation.get(),
                    semantic_id = focus_return.semantic_id,
                    restored,
                );
            });
        }

        let generation = self.generation;
        let lifecycle = self.lifecycle.clone();
        window.defer(cx, move |window, cx| {
            crate::platform::dematerialize_then_remove_gpui_window_from_app(
                window,
                cx,
                "DICTATION",
                "Microphone popup",
            );
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                cx.background_executor()
                    .timer(crate::platform::glass_exit_remove_delay())
                    .await;
                cx.update(|_cx| {
                    let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
                    clear_dictation_microphone_popup_window_slot(generation);
                });
            })
            .detach();
        });
    }

    fn ensure_activation_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.activation_subscription.is_some() {
            return;
        }

        let parent_window_handle = self.parent_window_handle;
        self.focus_pair_was_active |=
            crate::components::inline_popup_window::inline_popup_focus_pair_is_active(
                window,
                parent_window_handle,
                cx,
            );
        self.activation_subscription = Some(cx.observe_window_activation(
            window,
            move |this, window, cx| {
                if crate::components::inline_popup_window::inline_popup_focus_pair_is_active(
                    window,
                    parent_window_handle,
                    cx,
                ) {
                    this.focus_pair_was_active = true;
                    return;
                }
                if !this.focus_pair_was_active {
                    return;
                }

                this.request_close(window, cx, "focus_lost", true, true);
            },
        ));
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
        if self.selection_mode.persists_selection() {
            if let Err(error) = apply_device_selection(&row.action) {
                tracing::warn!(
                    category = "DICTATION",
                    error = %error,
                    "Failed to persist microphone selection from dictation popup"
                );
                return None;
            }
        } else {
            tracing::info!(
                category = "DICTATION",
                row_id = %row.row_id,
                "Fixture microphone selection accepted without persistence"
            );
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
        if self.selection_mode.persists_selection() && crate::dictation::is_dictation_recording() {
            crate::dictation::set_pending_dictation_device_label(Some(row.title.to_string()));
        }
        if let Some(view) = self.source_view.upgrade() {
            let _ = cx.update_window(self.parent_window_handle, |_entity, _window, cx| {
                view.update(cx, |_overlay, cx| {
                    cx.notify();
                });
            });
        }
        self.request_close(window, cx, "accepted", true, true);
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
        if crate::ui_foundation::is_key_escape(key) {
            self.request_close(window, cx, "escape", true, true);
            cx.stop_propagation();
            return;
        }

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_activation_subscription(window, cx);

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(
                cx.listener(|this, _event: &gpui::MouseDownEvent, window, cx| {
                    this.request_close(window, cx, "mouse_down_out", true, true);
                }),
            )
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.render_picker(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::DictationMicrophonePopupSelectionMode;

    #[test]
    fn dictation_microphone_fixture_selection_does_not_persist() {
        assert!(DictationMicrophonePopupSelectionMode::Production.persists_selection());
        assert!(!DictationMicrophonePopupSelectionMode::FixtureNoPersistence.persists_selection());
    }
}
