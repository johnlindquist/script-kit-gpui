use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Datelike, Local};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, uniform_list, AnyElement, AnyWindowHandle, App, AppContext, Bounds, Context,
    DisplayId, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, WeakEntity, Window, WindowHandle,
};
use gpui_component::scroll::ScrollableElement;

use crate::components::inline_popup_window::{
    InlinePopupAttachResult, InlinePopupCloseGate, InlinePopupFocusReturn, InlinePopupGeneration,
    InlinePopupLifecycle, InlinePopupLifecycleHandle, InlinePopupPhase,
};

use super::history::{
    AgentChatHistoryEntry, AgentChatHistorySearchField, AgentChatHistorySearchHit,
};
use super::view::{AgentChatHistoryMenuState, AgentChatView};

pub(crate) const AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID: &str = "agent_chat-history-popup";

const HISTORY_POPUP_MIN_WIDTH: f32 = crate::actions::constants::POPUP_WIDTH;
const HISTORY_POPUP_MAX_WIDTH: f32 = 420.0;
const HISTORY_POPUP_SIDE_MARGIN: f32 = 8.0;
const HISTORY_POPUP_TOP_INSET: f32 = 45.0;
const HISTORY_POPUP_BOTTOM_INSET: f32 = 12.0;
const HISTORY_POPUP_SEARCH_HEIGHT: f32 = crate::actions::constants::SEARCH_INPUT_HEIGHT;
const HISTORY_POPUP_FOOTER_HEIGHT: f32 = crate::window_resize::main_layout::HINT_STRIP_HEIGHT;
const HISTORY_POPUP_ROW_HEIGHT: f32 = 60.0;
const HISTORY_POPUP_EMPTY_HEIGHT: f32 = 72.0;
const HISTORY_POPUP_VISIBLE_ROWS: usize = 5;
const HISTORY_POPUP_MAX_SECTION_HEADERS: usize = 4;
const HISTORY_POPUP_VERTICAL_PADDING: f32 = 4.0;
pub(super) const HISTORY_POPUP_SEARCH_LIMIT: usize = 24;
pub(super) const HISTORY_POPUP_PAGE_JUMP: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatHistoryDateBucket {
    Today,
    Yesterday,
    ThisWeek,
    Older,
}

impl AgentChatHistoryDateBucket {
    fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::ThisWeek => "This Week",
            Self::Older => "Older",
        }
    }
}

// Variants carry differently-sized payloads; boxing the large one would churn
// this WIP popup for no real memory win at these list sizes.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
enum AgentChatHistoryPopupListItem {
    Header {
        bucket: AgentChatHistoryDateBucket,
        count: usize,
    },
    Entry {
        entry_index: usize,
        entry: AgentChatHistoryPopupEntry,
    },
}

/// A single popup row derived from a ranked search hit.
#[derive(Clone)]
pub(crate) struct AgentChatHistoryPopupEntry {
    pub(crate) hit: AgentChatHistorySearchHit,
    pub(crate) title: SharedString,
    pub(crate) preview: SharedString,
    pub(crate) meta: SharedString,
    pub(crate) match_label: SharedString,
}

impl AgentChatHistoryPopupEntry {
    pub(crate) fn from_hit(hit: AgentChatHistorySearchHit) -> Self {
        let entry = &hit.entry;
        let date = crate::formatting::format_rfc3339_date_for_display(&entry.timestamp);
        let match_label = match hit.matched_field {
            AgentChatHistorySearchField::Title => "title",
            AgentChatHistorySearchField::Preview => "reply",
            AgentChatHistorySearchField::SearchText => "transcript",
            AgentChatHistorySearchField::Timestamp => "date",
        };
        Self {
            title: SharedString::from(entry.title_display().to_string()),
            preview: SharedString::from(entry.preview_display().to_string()),
            meta: SharedString::from(format!("{} msgs \u{00B7} {}", entry.message_count, date)),
            match_label: SharedString::from(match_label),
            hit,
        }
    }
}

pub(crate) fn history_date_bucket(
    timestamp: &str,
    now: DateTime<Local>,
) -> AgentChatHistoryDateBucket {
    let Ok(parsed) = DateTime::parse_from_rfc3339(timestamp) else {
        return AgentChatHistoryDateBucket::Older;
    };
    let local = parsed.with_timezone(&Local);
    let entry_date = local.date_naive();
    let now_date = now.date_naive();

    if entry_date == now_date {
        return AgentChatHistoryDateBucket::Today;
    }
    if entry_date == now_date.pred_opt().unwrap_or(now_date) {
        return AgentChatHistoryDateBucket::Yesterday;
    }
    if local.iso_week() == now.iso_week() {
        return AgentChatHistoryDateBucket::ThisWeek;
    }
    AgentChatHistoryDateBucket::Older
}

pub(crate) fn bucket_history_entries(
    entries: &[AgentChatHistoryPopupEntry],
    now: DateTime<Local>,
) -> Vec<(AgentChatHistoryDateBucket, Vec<AgentChatHistoryPopupEntry>)> {
    let mut buckets = vec![
        (AgentChatHistoryDateBucket::Today, Vec::new()),
        (AgentChatHistoryDateBucket::Yesterday, Vec::new()),
        (AgentChatHistoryDateBucket::ThisWeek, Vec::new()),
        (AgentChatHistoryDateBucket::Older, Vec::new()),
    ];
    for entry in entries {
        let bucket = history_date_bucket(&entry.hit.entry.timestamp, now);
        if let Some((_, bucket_entries)) = buckets
            .iter_mut()
            .find(|(candidate, _)| *candidate == bucket)
        {
            bucket_entries.push(entry.clone());
        }
    }
    buckets
        .into_iter()
        .filter(|(_, bucket_entries)| !bucket_entries.is_empty())
        .collect()
}

fn history_popup_list_items(
    entries: &[AgentChatHistoryPopupEntry],
    now: DateTime<Local>,
) -> Vec<AgentChatHistoryPopupListItem> {
    let mut items = Vec::new();
    for (bucket, bucket_entries) in bucket_history_entries(entries, now) {
        items.push(AgentChatHistoryPopupListItem::Header {
            bucket,
            count: bucket_entries.len(),
        });
        for entry in bucket_entries {
            let entry_index = entries
                .iter()
                .position(|candidate| candidate.hit.entry.session_id == entry.hit.entry.session_id)
                .unwrap_or(0);
            items.push(AgentChatHistoryPopupListItem::Entry { entry_index, entry });
        }
    }
    items
}
#[derive(Clone)]
pub(crate) struct AgentChatHistoryPopupSnapshot {
    pub(crate) title: SharedString,
    pub(crate) query: SharedString,
    pub(crate) selected_index: usize,
    pub(crate) entries: Vec<AgentChatHistoryPopupEntry>,
}

#[derive(Clone)]
pub(crate) struct AgentChatHistoryPopupRequest {
    pub(crate) parent_window_handle: AnyWindowHandle,
    pub(crate) parent_bounds: Bounds<Pixels>,
    pub(crate) display_id: Option<DisplayId>,
    pub(crate) source_view: WeakEntity<AgentChatView>,
    pub(crate) snapshot: AgentChatHistoryPopupSnapshot,
    pub(crate) lifecycle: InlinePopupLifecycleHandle,
    pub(crate) focus_return: InlinePopupFocusReturn,
}

struct AgentChatHistoryPopupSlot {
    handle: WindowHandle<AgentChatHistoryPopupWindow>,
    parent_window_handle: AnyWindowHandle,
    generation: InlinePopupGeneration,
    automation_generation: Option<u64>,
    lifecycle: InlinePopupLifecycleHandle,
}

static AGENT_CHAT_HISTORY_POPUP_WINDOW: OnceLock<Mutex<Option<AgentChatHistoryPopupSlot>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatHistoryPopupKeyIntent {
    MoveUp,
    MoveDown,
    MoveHome,
    MoveEnd,
    MovePageUp,
    MovePageDown,
    ExecuteSelected,
    Close,
    Backspace,
    TypeChar(char),
}

pub(crate) fn close_history_popup_window(cx: &mut App) {
    close_history_popup_window_with_policy("owner_state_closed", true, cx);
}

pub(crate) fn close_history_popup_window_for_owner_loss(cx: &mut App) {
    close_history_popup_window_with_policy("owner_loss", false, cx);
}

pub(crate) fn close_history_popup_window_from_owner(
    generation: InlinePopupGeneration,
    reason: &'static str,
    restore_focus: bool,
    cx: &mut App,
) {
    let current = AGENT_CHAT_HISTORY_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| {
            guard
                .as_ref()
                .map(|slot| (slot.handle, slot.generation, slot.lifecycle.clone()))
        });
    let Some((handle, current_generation, lifecycle)) = current else {
        return;
    };
    if current_generation != generation {
        return;
    }
    if handle
        .update(cx, |popup, window, cx| {
            popup.request_close(window, cx, reason, restore_focus, false);
        })
        .is_err()
    {
        let _ = InlinePopupLifecycle::request_close(&lifecycle, generation);
        unregister_history_popup_automation_window(generation);
        clear_history_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
    }
}

fn close_history_popup_window_with_policy(reason: &'static str, restore_focus: bool, cx: &mut App) {
    let current = AGENT_CHAT_HISTORY_POPUP_WINDOW
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
        unregister_history_popup_automation_window(generation);
        clear_history_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
    }
}

fn unregister_history_popup_automation_window(generation: InlinePopupGeneration) {
    let registered = AGENT_CHAT_HISTORY_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| {
            guard
                .as_ref()
                .filter(|slot| slot.generation == generation)
                .and_then(|slot| slot.automation_generation)
        });
    if let Some(registered) = registered {
        super::popup_automation::unregister_agent_chat_prompt_popup_automation_window(
            AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID,
            registered,
        );
    }
}

fn clear_history_popup_window_slot(generation: InlinePopupGeneration) {
    if let Some(storage) = AGENT_CHAT_HISTORY_POPUP_WINDOW.get() {
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

pub(crate) fn get_history_popup_snapshot(cx: &gpui::App) -> Option<AgentChatHistoryPopupSnapshot> {
    let storage = AGENT_CHAT_HISTORY_POPUP_WINDOW.get()?;
    let guard = storage.lock().ok()?;
    let slot = guard.as_ref()?;
    slot.handle
        .read_with(cx, |popup, _cx| popup.snapshot.clone())
        .ok()
}

pub(crate) fn get_history_popup_snapshot_for_generation(
    generation: u64,
    cx: &gpui::App,
) -> Option<AgentChatHistoryPopupSnapshot> {
    let storage = AGENT_CHAT_HISTORY_POPUP_WINDOW.get()?;
    let guard = storage.lock().ok()?;
    let slot = guard.as_ref()?;
    if slot.automation_generation != Some(generation)
        || InlinePopupLifecycle::snapshot(&slot.lifecycle).1 != InlinePopupPhase::Open
        || crate::windows::get_runtime_window_handle_for_generation(
            AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID,
            generation,
        ) != Some(slot.handle.into())
    {
        return None;
    }
    slot.handle
        .read_with(cx, |popup, _| popup.snapshot.clone())
        .ok()
}

fn with_history_popup<T>(
    generation: u64,
    cx: &mut gpui::App,
    action: impl FnOnce(
        &mut AgentChatHistoryPopupWindow,
        &mut Window,
        &mut Context<AgentChatHistoryPopupWindow>,
    ) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let handle = {
        let guard = AGENT_CHAT_HISTORY_POPUP_WINDOW
            .get()
            .ok_or_else(|| anyhow::anyhow!("history_popup_missing"))?
            .lock()
            .map_err(|_| anyhow::anyhow!("history_popup_poisoned"))?;
        let slot = guard
            .as_ref()
            .filter(|slot| {
                slot.automation_generation == Some(generation)
                    && InlinePopupLifecycle::snapshot(&slot.lifecycle).1 == InlinePopupPhase::Open
            })
            .ok_or_else(|| anyhow::anyhow!("history_popup_stale"))?;
        slot.handle
    };
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(
            AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID,
            generation
        ) == Some(handle.into()),
        "history_popup_stale"
    );
    handle.update(cx, |popup, window, cx| {
        anyhow::ensure!(
            crate::windows::automation_window_by_id(&popup.focus_return.parent_automation_id)
                .is_some_and(
                    |parent| parent.generation == Some(popup.focus_return.parent_generation)
                ),
            "history_popup_parent_stale"
        );
        action(popup, window, cx)
    })?
}

pub(crate) fn batch_set_history_popup_input(
    generation: u64,
    text: &str,
    cx: &mut gpui::App,
) -> anyhow::Result<()> {
    anyhow::ensure!(text.len() <= 64 * 1024, "history_query_limit");
    with_history_popup(generation, cx, |popup, _, cx| {
        popup.set_query(text.to_owned(), cx);
        Ok(())
    })
}

pub(crate) fn batch_select_history_popup_by_value(
    generation: u64,
    value: &str,
    submit: bool,
    cx: &mut gpui::App,
) -> anyhow::Result<Option<String>> {
    with_history_popup(generation, cx, |popup, _, cx| {
        let Some(index) = popup
            .snapshot
            .entries
            .iter()
            .position(|entry| entry.hit.entry.session_id == value)
        else {
            return Ok(None);
        };
        popup.navigate(index as i32 - popup.snapshot.selected_index as i32, cx);
        let entry = popup.snapshot.entries[index].clone();
        if submit {
            popup.resume_session(&entry, cx);
        }
        Ok(Some(entry.hit.entry.session_id))
    })
}

pub(crate) fn batch_select_history_popup_by_semantic_id(
    generation: u64,
    semantic_id: &str,
    submit: bool,
    cx: &mut gpui::App,
) -> anyhow::Result<Option<String>> {
    with_history_popup(generation, cx, |popup, _, cx| {
        let Some(index) = popup
            .snapshot
            .entries
            .iter()
            .enumerate()
            .position(|(index, entry)| {
                format!("choice:{index}:{}", entry.hit.entry.session_id) == semantic_id
            })
        else {
            return Ok(None);
        };
        popup.navigate(index as i32 - popup.snapshot.selected_index as i32, cx);
        let entry = popup.snapshot.entries[index].clone();
        if submit {
            popup.resume_session(&entry, cx);
        }
        Ok(Some(entry.hit.entry.session_id))
    })
}

pub(crate) fn history_popup_revision_facts(
    generation: u64,
    window: Option<&Window>,
    cx: &gpui::App,
) -> Option<(u64, u64, u64, u64)> {
    let guard = AGENT_CHAT_HISTORY_POPUP_WINDOW.get()?.lock().ok()?;
    let slot = guard.as_ref()?;
    if slot.automation_generation != Some(generation)
        || InlinePopupLifecycle::snapshot(&slot.lifecycle).1 != InlinePopupPhase::Open
    {
        return None;
    }
    crate::windows::automation_surface_collector::read_window_root(
        slot.handle,
        window,
        cx,
        |view, _| {
            (
                generation,
                view.semantic_revision,
                view.semantic_revision,
                view.applied_theme_revision,
            )
        },
    )
    .ok()
}

pub(crate) fn history_popup_generation() -> Option<u64> {
    AGENT_CHAT_HISTORY_POPUP_WINDOW
        .get()
        .and_then(|storage| storage.lock().ok())
        .and_then(|guard| {
            guard.as_ref().and_then(|slot| {
                (InlinePopupLifecycle::snapshot(&slot.lifecycle).1 == InlinePopupPhase::Open)
                    .then_some(slot.automation_generation)
                    .flatten()
            })
        })
}

pub(crate) fn is_history_popup_window_open() -> bool {
    history_popup_generation().is_some()
}

fn register_history_popup_automation_window(
    handle: AnyWindowHandle,
    popup_bounds: Bounds<Pixels>,
    focus_return: &InlinePopupFocusReturn,
    cx: &mut App,
) -> anyhow::Result<u64> {
    super::popup_automation::register_agent_chat_prompt_popup_automation_window(
        AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID,
        "Agent Chat History Popup",
        handle,
        popup_bounds,
        focus_return,
        cx,
    )
}

fn popup_height(_snapshot: &AgentChatHistoryPopupSnapshot) -> f32 {
    let body_height = (HISTORY_POPUP_VISIBLE_ROWS as f32 * HISTORY_POPUP_ROW_HEIGHT)
        + (HISTORY_POPUP_MAX_SECTION_HEADERS as f32 * crate::list_item::SECTION_HEADER_HEIGHT);
    HISTORY_POPUP_SEARCH_HEIGHT
        + HISTORY_POPUP_FOOTER_HEIGHT
        + body_height.max(HISTORY_POPUP_EMPTY_HEIGHT)
        + (HISTORY_POPUP_VERTICAL_PADDING * 2.0)
}

fn popup_bounds(
    parent_bounds: Bounds<Pixels>,
    snapshot: &AgentChatHistoryPopupSnapshot,
) -> Bounds<Pixels> {
    let parent_width = parent_bounds.size.width.as_f32();
    let parent_height = parent_bounds.size.height.as_f32();
    let width = (parent_width - (HISTORY_POPUP_SIDE_MARGIN * 2.0))
        .clamp(HISTORY_POPUP_MIN_WIDTH, HISTORY_POPUP_MAX_WIDTH);
    let height = popup_height(snapshot)
        .min((parent_height - HISTORY_POPUP_TOP_INSET - HISTORY_POPUP_BOTTOM_INSET).max(140.0));
    let left = ((parent_width - width) / 2.0).max(HISTORY_POPUP_SIDE_MARGIN);
    let max_top =
        (parent_height - height - HISTORY_POPUP_BOTTOM_INSET).max(HISTORY_POPUP_TOP_INSET);
    let centered_top = (parent_height - height) / 2.0;
    let top = centered_top.clamp(HISTORY_POPUP_TOP_INSET, max_top);

    Bounds {
        origin: gpui::point(
            parent_bounds.origin.x + px(left),
            parent_bounds.origin.y + px(top),
        ),
        size: gpui::size(px(width), px(height)),
    }
}

fn reconcile_history_popup_native_close(
    generation: InlinePopupGeneration,
    lifecycle: &InlinePopupLifecycleHandle,
    source_view: &WeakEntity<AgentChatView>,
    focus_return: &InlinePopupFocusReturn,
    cx: &mut App,
) {
    match InlinePopupLifecycle::request_close(lifecycle, generation) {
        InlinePopupCloseGate::StaleGeneration | InlinePopupCloseGate::AlreadyClosed => return,
        InlinePopupCloseGate::Begin | InlinePopupCloseGate::AlreadyClosing => {}
    }
    unregister_history_popup_automation_window(generation);
    if let Some(view) = source_view.upgrade() {
        view.update(cx, |view, cx| {
            view.dismiss_history_popup_from_window(generation, "native_close", cx);
        });
    }
    let _ = focus_return.restore(generation, cx);
    let _ = InlinePopupLifecycle::mark_closed(lifecycle, generation);
    clear_history_popup_window_slot(generation);
}

pub(crate) fn sync_history_popup_window(
    cx: &mut App,
    request: AgentChatHistoryPopupRequest,
) -> anyhow::Result<()> {
    let AgentChatHistoryPopupRequest {
        parent_window_handle,
        parent_bounds,
        display_id,
        source_view,
        snapshot,
        lifecycle,
        focus_return,
    } = request;
    let generation = InlinePopupLifecycle::generation(&lifecycle);
    let bounds = popup_bounds(parent_bounds, &snapshot);
    let storage = AGENT_CHAT_HISTORY_POPUP_WINDOW.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = storage.lock() {
        if let Some(slot) = guard.as_ref() {
            if slot.parent_window_handle == parent_window_handle && slot.generation == generation {
                let update_result = slot.handle.update(cx, |popup, _window, cx| {
                    popup.set_snapshot(snapshot.clone());
                    cx.notify();
                });
                if update_result.is_ok() {
                    if let Some(registered) = slot.automation_generation {
                        crate::windows::set_automation_bounds_if_generation(
                            AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID,
                            registered,
                            Some(super::popup_automation::automation_bounds(bounds)),
                        );
                    }
                    return Ok(());
                }
            }
            anyhow::bail!(
                "Agent Chat history popup lifetime is still owned by generation {}",
                slot.generation.get()
            );
        }
    }

    let window_options = crate::components::inline_popup_window::inline_popup_window_options(
        bounds,
        display_id,
        focus_return.host_policy,
    );
    let native_source_view = source_view.clone();
    let native_lifecycle = lifecycle.clone();
    let native_focus_return = focus_return.clone();
    let entity_source_view = source_view.clone();
    let entity_lifecycle = lifecycle.clone();
    let entity_focus_return = focus_return.clone();
    let handle = cx.open_window(window_options, move |window, cx| {
        window.on_window_should_close(cx, move |_window, cx| {
            reconcile_history_popup_native_close(
                generation,
                &native_lifecycle,
                &native_source_view,
                &native_focus_return,
                cx,
            );
            true
        });
        cx.new(|cx| {
            AgentChatHistoryPopupWindow::new(
                snapshot.clone(),
                entity_source_view.clone(),
                parent_window_handle,
                generation,
                entity_lifecycle.clone(),
                entity_focus_return.clone(),
                cx,
            )
        })
    })?;

    if let Ok(mut guard) = storage.lock() {
        *guard = Some(AgentChatHistoryPopupSlot {
            handle,
            parent_window_handle,
            generation,
            automation_generation: None,
            lifecycle: lifecycle.clone(),
        });
    }

    let any_handle: AnyWindowHandle = handle.into();
    if let Err(error) =
        crate::components::inline_popup_window::configure_inline_popup_window_lifecycle(
            handle,
            crate::components::inline_popup_window::InlinePopupParent {
                window_handle: parent_window_handle,
                automation_id: focus_return.parent_automation_id.clone(),
                generation: focus_return.parent_generation,
                host_policy: focus_return.host_policy,
            },
            lifecycle.clone(),
            cx,
            move |result, cx| match result {
                InlinePopupAttachResult::Ready(receipt) => {
                    let registration = register_history_popup_automation_window(
                        any_handle,
                        bounds,
                        &focus_return,
                        cx,
                    );
                    if let Ok(registered) = registration.as_ref() {
                        if let Ok(mut guard) = storage.lock() {
                            if let Some(slot) = guard
                                .as_mut()
                                .filter(|slot| slot.generation == receipt.generation)
                            {
                                slot.automation_generation = Some(*registered);
                            }
                        }
                        // Establish the local key route without activating the native window.
                        let _ = handle.update(cx, |popup, window, cx| {
                            window.focus(&popup.focus_handle, cx);
                        });
                    }
                    if let Err(error) = registration {
                        tracing::warn!(
                            target: "script_kit::automation",
                            event = "agent_chat_history_popup_registry_failed",
                            generation = receipt.generation.get(),
                            error = %error,
                        );
                        let _ = handle.update(cx, |popup, window, cx| {
                            popup.request_close(window, cx, "registry_failed", true, true);
                        });
                    }
                }
                InlinePopupAttachResult::Failed {
                    generation,
                    failure,
                } => {
                    tracing::warn!(
                        target: "script_kit::inline_popup",
                        event = "agent_chat_history_popup_attach_failed",
                        generation = generation.get(),
                        failure = ?failure,
                    );
                    let _ = handle.update(cx, |popup, window, cx| {
                        popup.request_close(window, cx, "attach_failed", true, true);
                    });
                }
            },
        )
    {
        unregister_history_popup_automation_window(generation);
        clear_history_popup_window_slot(generation);
        let _ = InlinePopupLifecycle::request_close(&lifecycle, generation);
        let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
        let _ = handle.update(cx, |_popup, window, _cx| window.remove_window());
        return Err(error.context("failed to schedule Agent Chat history popup attach"));
    }

    Ok(())
}

pub(crate) struct AgentChatHistoryPopupWindow {
    snapshot: AgentChatHistoryPopupSnapshot,
    source_view: WeakEntity<AgentChatView>,
    parent_window_handle: AnyWindowHandle,
    generation: InlinePopupGeneration,
    lifecycle: InlinePopupLifecycleHandle,
    focus_return: InlinePopupFocusReturn,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    activation_subscription: Option<Subscription>,
    focus_pair_was_active: bool,
    semantic_revision: u64,
    last_semantic_token: Option<u64>,
    applied_theme_revision: u64,
}

impl AgentChatHistoryPopupWindow {
    fn new(
        snapshot: AgentChatHistoryPopupSnapshot,
        source_view: WeakEntity<AgentChatView>,
        parent_window_handle: AnyWindowHandle,
        generation: InlinePopupGeneration,
        lifecycle: InlinePopupLifecycleHandle,
        focus_return: InlinePopupFocusReturn,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            snapshot,
            source_view,
            parent_window_handle,
            generation,
            lifecycle,
            focus_return,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
            activation_subscription: None,
            focus_pair_was_active: false,
            semantic_revision: 1,
            last_semantic_token: None,
            applied_theme_revision: 0,
        }
    }

    fn set_snapshot(&mut self, snapshot: AgentChatHistoryPopupSnapshot) {
        self.snapshot = snapshot;
    }

    fn sync_selection_to_source_view(&self, selected_index: usize, cx: &mut App) {
        if let Some(view) = self.source_view.upgrade() {
            view.update(cx, |view, cx| {
                view.sync_history_popup_selection_from_window(self.generation, selected_index, cx);
            });
        }
    }

    fn sync_state_to_source_view(
        &self,
        query: String,
        hits: Vec<AgentChatHistorySearchHit>,
        selected_index: usize,
        cx: &mut App,
    ) {
        if let Some(view) = self.source_view.upgrade() {
            view.update(cx, |view, cx| {
                view.sync_history_popup_state_from_window(
                    self.generation,
                    query,
                    hits,
                    selected_index,
                    cx,
                );
            });
        }
    }

    /// Default action (Enter / click): attach a summary as a context chip.
    fn attach_summary(&self, entry: &AgentChatHistoryPopupEntry, cx: &mut App) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_popup_action",
            action = "attach_summary",
            session_id = %entry.hit.entry.session_id,
            score = entry.hit.score,
            matched_field = ?entry.hit.matched_field,
        );
        if let Some(view) = self.source_view.upgrade() {
            let session_id = entry.hit.entry.session_id.clone();
            let generation = self.generation;
            view.update(cx, |view, cx| {
                view.dismiss_history_popup_from_window(
                    generation,
                    "committed_summary_selection",
                    cx,
                );
                if let Err(error) = view.attach_history_session(
                    &session_id,
                    super::history_attachment::AgentChatHistoryAttachMode::Summary,
                    cx,
                ) {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_history_popup_attach_failed",
                        session_id = %session_id,
                        mode = "summary",
                        error = %error,
                    );
                }
                cx.notify();
            });
        }
        let generation = self.generation;
        cx.defer(move |cx| {
            close_history_popup_window_from_owner(generation, "selection_committed", true, cx)
        });
    }

    /// Shift+Enter: attach full transcript as a context chip.
    fn attach_transcript(&self, entry: &AgentChatHistoryPopupEntry, cx: &mut App) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_popup_action",
            action = "attach_transcript",
            session_id = %entry.hit.entry.session_id,
            score = entry.hit.score,
            matched_field = ?entry.hit.matched_field,
        );
        if let Some(view) = self.source_view.upgrade() {
            let session_id = entry.hit.entry.session_id.clone();
            let generation = self.generation;
            view.update(cx, |view, cx| {
                view.dismiss_history_popup_from_window(
                    generation,
                    "committed_transcript_selection",
                    cx,
                );
                if let Err(error) = view.attach_history_session(
                    &session_id,
                    super::history_attachment::AgentChatHistoryAttachMode::Transcript,
                    cx,
                ) {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_history_popup_attach_failed",
                        session_id = %session_id,
                        mode = "transcript",
                        error = %error,
                    );
                }
                cx.notify();
            });
        }
        let generation = self.generation;
        cx.defer(move |cx| {
            close_history_popup_window_from_owner(generation, "selection_committed", true, cx)
        });
    }

    /// Enter: resume (load) the session into the Agent Chat thread.
    fn resume_session(&self, entry: &AgentChatHistoryPopupEntry, cx: &mut App) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_popup_action",
            action = "resume",
            session_id = %entry.hit.entry.session_id,
            score = entry.hit.score,
            matched_field = ?entry.hit.matched_field,
        );
        if let Some(view) = self.source_view.upgrade() {
            let history_entry = entry.hit.entry.clone();
            let generation = self.generation;
            view.update(cx, |view, cx| {
                view.dismiss_history_popup_from_window(
                    generation,
                    "committed_resume_selection",
                    cx,
                );
                view.apply_selected_history_entry(&history_entry, cx);
            });
        }
        let generation = self.generation;
        cx.defer(move |cx| {
            close_history_popup_window_from_owner(generation, "selection_committed", true, cx)
        });
    }

    fn navigate(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.snapshot.entries.is_empty() {
            return;
        }
        let len = self.snapshot.entries.len();
        let idx = self.snapshot.selected_index;
        self.snapshot.selected_index = if delta < 0 {
            idx.saturating_sub((-delta) as usize)
        } else {
            (idx + delta as usize).min(len.saturating_sub(1))
        };
        self.scroll_handle
            .scroll_to_item(self.snapshot.selected_index, ScrollStrategy::Nearest);
        self.sync_selection_to_source_view(self.snapshot.selected_index, cx);
        cx.notify();
    }

    fn jump_to_boundary(&mut self, end: bool, cx: &mut Context<Self>) {
        if self.snapshot.entries.is_empty() {
            return;
        }

        self.snapshot.selected_index = if end {
            self.snapshot.entries.len().saturating_sub(1)
        } else {
            0
        };
        self.scroll_handle
            .scroll_to_item(self.snapshot.selected_index, ScrollStrategy::Nearest);
        self.sync_selection_to_source_view(self.snapshot.selected_index, cx);
        cx.notify();
    }

    fn page_navigate(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.snapshot.entries.is_empty() {
            return;
        }

        let len = self.snapshot.entries.len();
        let target = if delta < 0 {
            self.snapshot
                .selected_index
                .saturating_sub(HISTORY_POPUP_PAGE_JUMP)
        } else {
            (self.snapshot.selected_index + HISTORY_POPUP_PAGE_JUMP).min(len.saturating_sub(1))
        };
        self.snapshot.selected_index = target;
        self.scroll_handle
            .scroll_to_item(self.snapshot.selected_index, ScrollStrategy::Nearest);
        self.sync_selection_to_source_view(self.snapshot.selected_index, cx);
        cx.notify();
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        let hits = super::history::search_history(&query, HISTORY_POPUP_SEARCH_LIMIT);
        let entries = hits
            .iter()
            .cloned()
            .map(AgentChatHistoryPopupEntry::from_hit)
            .collect::<Vec<_>>();

        self.snapshot.query = SharedString::from(query.clone());
        self.snapshot.entries = entries;
        self.snapshot.selected_index = 0;
        self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        self.sync_state_to_source_view(query, hits, 0, cx);
        cx.notify();
    }

    fn handle_char_input(&mut self, ch: char, cx: &mut Context<Self>) {
        let mut query = self.snapshot.query.to_string();
        query.push(ch);
        self.set_query(query, cx);
    }

    fn handle_backspace_input(&mut self, cx: &mut Context<Self>) {
        let mut query = self.snapshot.query.to_string();
        if query.is_empty() {
            return;
        }
        query.pop();
        self.set_query(query, cx);
    }

    fn request_close(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        reason: &'static str,
        restore_focus: bool,
        reconcile_owner: bool,
    ) {
        if InlinePopupLifecycle::request_close(&self.lifecycle, self.generation)
            != InlinePopupCloseGate::Begin
        {
            return;
        }

        unregister_history_popup_automation_window(self.generation);
        if reconcile_owner {
            if let Some(view) = self.source_view.upgrade() {
                view.update(cx, |view, cx| {
                    view.dismiss_history_popup_from_window(self.generation, reason, cx);
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
                    event = "agent_chat_history_popup_focus_restore",
                    generation = generation.get(),
                    semantic_id = focus_return.semantic_id,
                    restored,
                );
            });
        }

        let generation = self.generation;
        let lifecycle = self.lifecycle.clone();
        let hidden = self.focus_return.host_policy.is_hidden();
        window.defer(cx, move |window, cx| {
            if hidden {
                window.remove_window();
                let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
                clear_history_popup_window_slot(generation);
                return;
            }
            crate::platform::dematerialize_then_remove_gpui_window_from_app(
                window,
                cx,
                "AGENT_CHAT",
                "Agent Chat history popup",
            );
            cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                cx.background_executor()
                    .timer(crate::platform::glass_exit_remove_delay())
                    .await;
                cx.update(|_cx| {
                    let _ = InlinePopupLifecycle::mark_closed(&lifecycle, generation);
                    clear_history_popup_window_slot(generation);
                });
            })
            .detach();
        });
    }

    fn ensure_activation_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_return.host_policy.is_hidden() {
            return;
        }
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
}

impl Focusable for AgentChatHistoryPopupWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AgentChatHistoryPopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.snapshot.query.hash(&mut hash);
        self.snapshot.selected_index.hash(&mut hash);
        for entry in &self.snapshot.entries {
            entry.title.hash(&mut hash);
            entry.preview.hash(&mut hash);
        }
        let token = hash.finish();
        if self.last_semantic_token != Some(token) {
            self.semantic_revision = self.semantic_revision.wrapping_add(1).max(1);
            self.last_semantic_token = Some(token);
        }
        self.applied_theme_revision = crate::theme::get_theme_snapshot().revision;
        self.ensure_activation_subscription(window, cx);

        let theme = crate::theme::get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let title_color = gpui::rgb(theme.colors.text.primary);
        let dimmed_text = gpui::rgb(theme.colors.text.dimmed);
        let secondary_text = gpui::rgb(theme.colors.text.secondary);
        let hover_bg = gpui::rgba(chrome.hover_rgba);
        let selected_bg = gpui::rgba(chrome.selection_rgba);
        let container_bg = gpui::rgba(chrome.popup_surface_rgba);
        let container_border = gpui::rgba(chrome.border_rgba);
        let accent_color = gpui::rgb(theme.colors.accent.selected);
        let search_query = self.snapshot.query.to_string();
        let placeholder_text = self.snapshot.title.clone();
        let search_display = if search_query.is_empty() {
            SharedString::from(placeholder_text.to_string())
        } else {
            SharedString::from(search_query.clone())
        };
        div()
            .track_focus(&self.focus_handle)
            .id("agent_chat-history-popup")
            .debug_selector(|| "agent_chat-history-popup".to_string())
            .on_mouse_down_out(cx.listener(|this, _event: &gpui::MouseDownEvent, window, cx| {
                this.request_close(window, cx, "mouse_down_out", true, true);
            }))
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    let modifiers = &event.keystroke.modifiers;

                    match history_popup_key_intent(key, modifiers) {
                        Some(AgentChatHistoryPopupKeyIntent::MoveUp) => {
                            this.navigate(-1, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::MoveDown) => {
                            this.navigate(1, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::MoveHome) => {
                            this.jump_to_boundary(false, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::MoveEnd) => {
                            this.jump_to_boundary(true, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::MovePageUp) => {
                            this.page_navigate(-1, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::MovePageDown) => {
                            this.page_navigate(1, cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::ExecuteSelected) => {
                            let has_cmd = modifiers.platform;
                            let has_shift = modifiers.shift;
                            if let Some(entry) = this
                                .snapshot
                                .entries
                                .get(this.snapshot.selected_index)
                                .cloned()
                            {
                                if has_shift {
                                    this.attach_transcript(&entry, cx);
                                } else if has_cmd {
                                    this.attach_summary(&entry, cx);
                                } else {
                                    this.resume_session(&entry, cx);
                                }
                            }
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::Close) => {
                            this.request_close(window, cx, "escape", true, true);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::Backspace) => {
                            this.handle_backspace_input(cx);
                            cx.stop_propagation();
                        }
                        Some(AgentChatHistoryPopupKeyIntent::TypeChar(ch)) => {
                            this.handle_char_input(ch, cx);
                            cx.stop_propagation();
                        }
                        None => {
                            cx.propagate();
                        }
                    }
                }),
            )
            .w_full()
            .h_full()
            .px(px(HISTORY_POPUP_SIDE_MARGIN))
            .py(px(HISTORY_POPUP_VERTICAL_PADDING))
            .child(
                div()
                    .w_full()
                    .h_full()
                    .bg(container_bg)
                    .border_1()
                    .border_color(container_border)
                    .overflow_hidden()
                    .child(
                        div()
                            .debug_selector(|| "agent_chat-history-popup-header".to_string())
                            .w_full()
                            .h(px(HISTORY_POPUP_SEARCH_HEIGHT))
                            .px(px(crate::actions::constants::ACTION_PADDING_X))
                            .py(px(6.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(28.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .text_sm()
                                    .text_color(if search_query.is_empty() {
                                        dimmed_text
                                    } else {
                                        title_color
                                    })
                                    .when(search_query.is_empty(), |d| {
                                        d.child(
                                            div()
                                                .w(px(2.0))
                                                .h(px(16.0))
                                                .mr(px(2.0))
                                                .rounded(px(1.0))
                                                .bg(accent_color),
                                        )
                                    })
                                    .child(search_display)
                                    .when(!search_query.is_empty(), |d| {
                                        d.child(
                                            div()
                                                .w(px(2.0))
                                                .h(px(16.0))
                                                .mr(px(2.0))
                                                .rounded(px(1.0))
                                                .bg(accent_color),
                                        )
                                    }),
                            ),
                    )
                    .child(if self.snapshot.entries.is_empty() {
                        div()
                            .w_full()
                            .h(px(HISTORY_POPUP_EMPTY_HEIGHT))
                            .px(px(crate::actions::constants::ACTION_PADDING_X))
                            .flex()
                            .items_center()
                            .text_sm()
                            .text_color(dimmed_text)
                            .child(if search_query.is_empty() {
                                "No conversation history yet"
                            } else {
                                "No matches yet \u{2014} try words from the prompt, reply, or date"
                            })
                            .into_any_element()
                    } else {
                        let entries = self.snapshot.entries.clone();
                        let list_items = history_popup_list_items(&entries, Local::now());
                        let selected_index = self.snapshot.selected_index;
                        let source_view = self.source_view.clone();

                        uniform_list(
                            "agent_chat-history-popup-list",
                            list_items.len(),
                            cx.processor(
                                move |_this,
                                      visible_range: std::ops::Range<usize>,
                                      _window,
                                      cx| {
                                visible_range
                                    .map(|idx| {
                                        let item = list_items[idx].clone();
                                        let (entry_index, entry) = match item {
                                            AgentChatHistoryPopupListItem::Header {
                                                bucket,
                                                count,
                                            } => {
                                                return crate::components::unified_list_item::SectionHeader::new(
                                                    bucket.label(),
                                                )
                                                .count(count)
                                                .into_any_element();
                                            }
                                            AgentChatHistoryPopupListItem::Entry {
                                                entry_index,
                                                entry,
                                            } => (entry_index, entry),
                                        };
                                        let is_selected = entry_index == selected_index;
                                        let row_entry = entry.clone();
                                        let row_view = source_view.clone();
                                        let click_view = source_view.clone();

                                        div()
                                            .id(SharedString::from(format!(
                                                "agent_chat-history-popup-row-{entry_index}"
                                            )))
                                            .debug_selector(move || format!("agent_chat-history-popup-row-{entry_index}"))
                                            .h(px(HISTORY_POPUP_ROW_HEIGHT))
                                            .w_full()
                                            .px(px(crate::actions::constants::ACTION_ROW_INSET))
                                            .py(px(4.0))
                                            .flex()
                                            .flex_col()
                                            .justify_center()
                                            .cursor_pointer()
                                            .when(is_selected, |d| d.bg(selected_bg))
                                            .when(!is_selected, |d| d.hover(|d| d.bg(hover_bg)))
                                            .on_mouse_move(cx.listener(move |this, _event, _window, cx| {
                                                if this.snapshot.selected_index != entry_index {
                                                    this.snapshot.selected_index = entry_index;
                                                    this.sync_selection_to_source_view(entry_index, cx);
                                                    cx.notify();
                                                } else if let Some(view) = row_view.upgrade() {
                                                    let generation = this.generation;
                                                    view.update(cx, |view, cx| {
                                                        view.sync_history_popup_selection_from_window(
                                                            generation,
                                                            entry_index,
                                                            cx,
                                                        );
                                                    });
                                                }
                                            }))
                                            .on_click(
                                                cx.listener(move |this, _event, _window, cx| {
                                                    this.snapshot.selected_index = entry_index;
                                                    if let Some(view) = click_view.upgrade() {
                                                        let generation = this.generation;
                                                        view.update(cx, |view, cx| {
                                                            view.sync_history_popup_selection_from_window(
                                                                generation,
                                                                entry_index,
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                    this.attach_summary(&row_entry, cx);
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .w_full()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .px(px(
                                                        crate::actions::constants::ACTION_PADDING_X
                                                            - crate::actions::constants::ACTION_ROW_INSET,
                                                    ))
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap(px(8.0))
                                                            .child(
                                                                div()
                                                                    .flex_1()
                                                                    .min_w(px(0.0))
                                                                    .text_sm()
                                                                    .font_weight(if is_selected {
                                                                        FontWeight::MEDIUM
                                                                    } else {
                                                                        FontWeight::NORMAL
                                                                    })
                                                                    .text_color(title_color)
                                                                    .overflow_hidden()
                                                                    .text_ellipsis()
                                                                    .whitespace_nowrap()
                                                                    .child(entry.title.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex_shrink_0()
                                                                    .px(px(6.0))
                                                                    .py(px(2.0))
                                                                    .rounded(px(999.0))
                                                                    .bg(if is_selected {
                                                                        gpui::rgba(
                                                                            (theme.colors.accent.selected
                                                                                << 8)
                                                                                | 0x18,
                                                                        )
                                                                    } else {
                                                                        gpui::rgba(
                                                                            (theme.colors.ui.border << 8)
                                                                                | 0x10,
                                                                        )
                                                                    })
                                                                    .border_1()
                                                                    .border_color(if is_selected {
                                                                        gpui::rgba(
                                                                            (theme.colors.accent.selected
                                                                                << 8)
                                                                                | 0x30,
                                                                        )
                                                                    } else {
                                                                        gpui::rgba(
                                                                            (theme.colors.ui.border << 8)
                                                                                | 0x20,
                                                                        )
                                                                    })
                                                                    .text_xs()
                                                                    .text_color(if is_selected {
                                                                        gpui::rgb(theme.colors.accent.selected)
                                                                    } else {
                                                                        dimmed_text
                                                                    })
                                                                    .child(entry.match_label.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .flex_shrink_0()
                                                                    .text_xs()
                                                                    .text_color(if is_selected {
                                                                        secondary_text
                                                                    } else {
                                                                        dimmed_text
                                                                    })
                                                                    .whitespace_nowrap()
                                                                    .child(entry.meta.clone()),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .text_xs()
                                                            .text_color(if is_selected {
                                                                secondary_text
                                                            } else {
                                                                dimmed_text
                                                            })
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .whitespace_nowrap()
                                                            .child(entry.preview.clone()),
                                                    ),
                                            )
                                            .into_any_element()
                                    })
                                    .collect::<Vec<AnyElement>>()
                                },
                            ),
                        )
                        .debug_selector(|| "agent_chat-history-popup-list".to_string())
                        .h_full()
                        .w_full()
                        .track_scroll(&self.scroll_handle)
                        .into_any_element()
                    })
                    .child(div().w_full().child(crate::components::HintStrip::new(vec![
                        "Type to Search".into(),
                        "\u{2191}\u{2193} Navigate".into(),
                        "\u{21B5} Resume".into(),
                        "\u{21E7}\u{21B5} Attach Transcript".into(),
                        "\u{2318}\u{21B5} Attach Summary".into(),
                        "Esc Close".into(),
                    ]))),
            )
    }
}

#[inline]
pub(super) fn history_popup_key_intent(
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<AgentChatHistoryPopupKeyIntent> {
    if key.eq_ignore_ascii_case("space") {
        return Some(AgentChatHistoryPopupKeyIntent::TypeChar(' '));
    }
    if crate::ui_foundation::is_key_up(key) {
        return Some(AgentChatHistoryPopupKeyIntent::MoveUp);
    }
    if crate::ui_foundation::is_key_down(key) {
        return Some(AgentChatHistoryPopupKeyIntent::MoveDown);
    }
    if key.eq_ignore_ascii_case("home") {
        return Some(AgentChatHistoryPopupKeyIntent::MoveHome);
    }
    if key.eq_ignore_ascii_case("end") {
        return Some(AgentChatHistoryPopupKeyIntent::MoveEnd);
    }
    if key.eq_ignore_ascii_case("pageup") {
        return Some(AgentChatHistoryPopupKeyIntent::MovePageUp);
    }
    if key.eq_ignore_ascii_case("pagedown") {
        return Some(AgentChatHistoryPopupKeyIntent::MovePageDown);
    }
    if crate::ui_foundation::is_key_enter(key) {
        return Some(AgentChatHistoryPopupKeyIntent::ExecuteSelected);
    }
    if crate::ui_foundation::is_key_escape(key) {
        return Some(AgentChatHistoryPopupKeyIntent::Close);
    }
    if crate::ui_foundation::is_key_backspace(key) || key.eq_ignore_ascii_case("delete") {
        return Some(AgentChatHistoryPopupKeyIntent::Backspace);
    }
    if !modifiers.platform && !modifiers.control && !modifiers.alt {
        if let Some(ch) = key.chars().next() {
            if key.len() == 1
                && (ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_')
            {
                return Some(AgentChatHistoryPopupKeyIntent::TypeChar(ch));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        bucket_history_entries, history_date_bucket, history_popup_key_intent, popup_bounds,
        popup_height, AgentChatHistoryDateBucket, AgentChatHistoryPopupEntry,
        AgentChatHistoryPopupKeyIntent, AgentChatHistoryPopupSnapshot, HISTORY_POPUP_BOTTOM_INSET,
        HISTORY_POPUP_MAX_WIDTH, HISTORY_POPUP_TOP_INSET,
    };
    use crate::ai::agent_chat::ui::history::{
        AgentChatHistoryEntry, AgentChatHistorySearchField, AgentChatHistorySearchHit,
    };
    use chrono::TimeZone;
    use gpui::SharedString;

    fn make_entry(
        session_id: &str,
        first_message: &str,
        message_count: usize,
    ) -> AgentChatHistoryPopupEntry {
        make_entry_with_timestamp(
            session_id,
            first_message,
            message_count,
            "2026-04-05T12:00:00Z",
        )
    }

    fn make_entry_with_timestamp(
        session_id: &str,
        first_message: &str,
        message_count: usize,
        timestamp: &str,
    ) -> AgentChatHistoryPopupEntry {
        AgentChatHistoryPopupEntry::from_hit(AgentChatHistorySearchHit {
            entry: AgentChatHistoryEntry {
                timestamp: timestamp.to_string(),
                first_message: first_message.to_string(),
                message_count,
                session_id: session_id.to_string(),
                ..Default::default()
            },
            score: 0,
            matched_field: AgentChatHistorySearchField::Title,
            evidence: None,
        })
    }

    #[test]
    fn popup_height_accounts_for_rows_and_chrome() {
        let snapshot = AgentChatHistoryPopupSnapshot {
            title: SharedString::from("Recent Conversations"),
            query: SharedString::from(""),
            selected_index: 0,
            entries: vec![
                make_entry("one", "First", 3),
                make_entry("two", "Second", 5),
            ],
        };

        assert!(popup_height(&snapshot) > 100.0);
    }

    #[test]
    fn popup_bounds_center_within_parent() {
        let snapshot = AgentChatHistoryPopupSnapshot {
            title: SharedString::from("Recent Conversations"),
            query: SharedString::from(""),
            selected_index: 0,
            entries: vec![make_entry("one", "First", 3)],
        };
        let parent = gpui::Bounds {
            origin: gpui::point(gpui::px(100.0), gpui::px(40.0)),
            size: gpui::size(gpui::px(480.0), gpui::px(440.0)),
        };

        let bounds = popup_bounds(parent, &snapshot);
        assert_eq!(f32::from(bounds.size.width), HISTORY_POPUP_MAX_WIDTH);
        assert!(f32::from(bounds.origin.x) > 100.0);
        assert!(f32::from(bounds.origin.y) >= f32::from(parent.origin.y) + HISTORY_POPUP_TOP_INSET);
        assert!(
            f32::from(bounds.origin.y + bounds.size.height)
                <= f32::from(parent.origin.y + parent.size.height) - HISTORY_POPUP_BOTTOM_INSET
        );
    }

    #[test]
    fn from_hit_preserves_match_metadata() {
        let hit = AgentChatHistorySearchHit {
            entry: AgentChatHistoryEntry {
                timestamp: "2026-04-08T08:10:00Z".to_string(),
                first_message: "continue".to_string(),
                title: "Continue the deployment cleanup".to_string(),
                custom_title: None,
                preview: "I found the stale kubernetes secret".to_string(),
                search_text: "kubernetes secret deployment".to_string(),
                message_count: 14,
                session_id: "test-session".to_string(),
            },
            score: 42,
            matched_field: AgentChatHistorySearchField::SearchText,
            evidence: None,
        };

        let entry = AgentChatHistoryPopupEntry::from_hit(hit);
        assert_eq!(entry.title.as_ref(), "Continue the deployment cleanup");
        assert_eq!(
            entry.preview.as_ref(),
            "I found the stale kubernetes secret"
        );
        assert_eq!(entry.match_label.as_ref(), "transcript");
        assert!(entry.meta.as_ref().contains("14 msgs"));
        assert_eq!(entry.hit.score, 42);
    }

    #[test]
    fn history_date_bucket_groups_today_yesterday_this_week_and_older() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 4, 8, 12, 0, 0)
            .single()
            .expect("local datetime");
        assert_eq!(
            history_date_bucket(&now.to_rfc3339(), now),
            AgentChatHistoryDateBucket::Today
        );
        assert_eq!(
            history_date_bucket(&(now - chrono::Duration::days(1)).to_rfc3339(), now),
            AgentChatHistoryDateBucket::Yesterday
        );
        assert_eq!(
            history_date_bucket(&(now - chrono::Duration::days(2)).to_rfc3339(), now),
            AgentChatHistoryDateBucket::ThisWeek
        );
        assert_eq!(
            history_date_bucket(&(now - chrono::Duration::days(10)).to_rfc3339(), now),
            AgentChatHistoryDateBucket::Older
        );
    }

    #[test]
    fn bucket_history_entries_preserves_bucket_order_and_counts() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 4, 8, 12, 0, 0)
            .single()
            .expect("local datetime");
        let entries = vec![
            make_entry_with_timestamp(
                "old",
                "Old",
                1,
                &(now - chrono::Duration::days(10)).to_rfc3339(),
            ),
            make_entry_with_timestamp("today", "Today", 1, &now.to_rfc3339()),
            make_entry_with_timestamp(
                "week",
                "Week",
                1,
                &(now - chrono::Duration::days(2)).to_rfc3339(),
            ),
            make_entry_with_timestamp(
                "yesterday",
                "Yesterday",
                1,
                &(now - chrono::Duration::days(1)).to_rfc3339(),
            ),
        ];

        let buckets = bucket_history_entries(&entries, now);
        assert_eq!(
            buckets
                .iter()
                .map(|(bucket, entries)| (*bucket, entries.len()))
                .collect::<Vec<_>>(),
            vec![
                (AgentChatHistoryDateBucket::Today, 1),
                (AgentChatHistoryDateBucket::Yesterday, 1),
                (AgentChatHistoryDateBucket::ThisWeek, 1),
                (AgentChatHistoryDateBucket::Older, 1),
            ]
        );
    }

    #[test]
    fn history_popup_key_intent_matches_actions_style_navigation_and_search() {
        let no_mods = gpui::Modifiers::default();

        assert_eq!(
            history_popup_key_intent("up", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MoveUp)
        );
        assert_eq!(
            history_popup_key_intent("down", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MoveDown)
        );
        assert_eq!(
            history_popup_key_intent("home", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MoveHome)
        );
        assert_eq!(
            history_popup_key_intent("end", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MoveEnd)
        );
        assert_eq!(
            history_popup_key_intent("pageup", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MovePageUp)
        );
        assert_eq!(
            history_popup_key_intent("pagedown", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::MovePageDown)
        );
        assert_eq!(
            history_popup_key_intent("backspace", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::Backspace)
        );
        assert_eq!(
            history_popup_key_intent("space", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::TypeChar(' '))
        );
        assert_eq!(
            history_popup_key_intent("a", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::TypeChar('a'))
        );
        assert_eq!(
            history_popup_key_intent("-", &no_mods),
            Some(AgentChatHistoryPopupKeyIntent::TypeChar('-'))
        );
        assert_eq!(history_popup_key_intent("tab", &no_mods), None);
        assert_eq!(history_popup_key_intent("arrowleft", &no_mods), None);
    }
}
