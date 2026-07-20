use gpui::{
    div, list, prelude::*, px, rems, rgb, rgba, Animation, AnimationExt as _, App, Context, Entity,
    FontWeight, ListAlignment, ListOffset, ListSizingBehavior, ListState, Render, Rgba,
    SharedString, StyleRefinement, Window,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::text::{MarkdownLinkLabelPolicy, TextView, TextViewState, TextViewStyle};
use gpui_component::tooltip::Tooltip;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::super::events::AgentChatForkPoint;
use super::super::style_contract::{
    self, resolved_agent_chat_transcript_colors, AgentChatStyleDef,
};
use super::super::thread::{
    AgentChatThread, AgentChatThreadMessage, AgentChatThreadMessageRole, AgentChatThreadStatus,
};
use super::super::tool_card::{
    classify_diff_line, AgentChatToolCardMeta, AgentChatToolStatus, DiffLineKind,
};
use super::super::ui_variant::{AgentChatTranscriptPresentation, AgentChatUiVariant};
use crate::list_item::FONT_MONO;
use crate::theme::{self, PromptColors};
use crate::ui::chrome::AMBIENT_PULSE_CYCLE_MS;

const AGENT_CHAT_TRANSCRIPT_VIEWPORT_FIDELITY_ID: &str = "agent-chat-transcript-viewport";

fn transcript_row_role_slug(role: AgentChatThreadMessageRole) -> &'static str {
    match role {
        AgentChatThreadMessageRole::User => "user",
        AgentChatThreadMessageRole::Assistant => "assistant",
        AgentChatThreadMessageRole::Thought => "thought",
        AgentChatThreadMessageRole::Tool => "tool",
        AgentChatThreadMessageRole::System => "system",
        AgentChatThreadMessageRole::Error => "error",
    }
}

fn transcript_row_fidelity_id(message: &AgentChatThreadMessage) -> String {
    format!(
        "agent-chat-transcript-row-{}-{}",
        transcript_row_role_slug(message.role),
        message.id
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingActivityPlacement {
    Hidden,
    TailSentinel,
    EmptyAssistantRow(usize),
}

fn pending_activity_placement(
    messages: &[AgentChatThreadMessage],
    pending: bool,
) -> PendingActivityPlacement {
    if !pending {
        return PendingActivityPlacement::Hidden;
    }

    let Some(user_index) = messages
        .iter()
        .rposition(|message| matches!(message.role, AgentChatThreadMessageRole::User))
    else {
        return PendingActivityPlacement::TailSentinel;
    };

    let response = messages[user_index + 1..]
        .iter()
        .enumerate()
        .find(|(_, message)| {
            matches!(
                message.role,
                AgentChatThreadMessageRole::Assistant | AgentChatThreadMessageRole::Error
            )
        })
        .map(|(relative_index, message)| (user_index + 1 + relative_index, message));

    match response {
        None => PendingActivityPlacement::TailSentinel,
        Some((message_index, message))
            if matches!(message.role, AgentChatThreadMessageRole::Assistant)
                && message.body.trim().is_empty() =>
        {
            PendingActivityPlacement::EmptyAssistantRow(message_index)
        }
        Some(_) => PendingActivityPlacement::Hidden,
    }
}

/// Callback invoked when the user forks the thread by editing message `u64`.
type ForkEditMessageHandler = Arc<dyn Fn(u64, &mut Window, &mut App) + 'static>;

pub enum AgentChatTranscriptEvent {
    ToggleMessage(u64),
    ForkEditMessage(u64),
}

impl gpui::EventEmitter<AgentChatTranscriptEvent> for AgentChatTranscript {}

#[derive(Clone, Copy, Debug, Default)]
struct HeavyMarkdownStats {
    bytes: usize,
    chars: usize,
    lines: usize,
    fence_markers: usize,
    table_like_lines: usize,
    blockquote_lines: usize,
    list_like_lines: usize,
    link_like_spans: usize,
}

impl HeavyMarkdownStats {
    fn from_text(text: &str) -> Self {
        let mut stats = Self {
            bytes: text.len(),
            chars: text.chars().count(),
            ..Self::default()
        };

        for line in text.lines() {
            stats.lines += 1;
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                stats.fence_markers += 1;
            }
            if trimmed.starts_with('|') && trimmed.contains('|') {
                stats.table_like_lines += 1;
            }
            if trimmed.starts_with('>') {
                stats.blockquote_lines += 1;
            }
            if trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || trimmed.starts_with("- [")
                || trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                    && trimmed.contains(". ")
            {
                stats.list_like_lines += 1;
            }
            stats.link_like_spans += count_link_like_spans(trimmed);
        }

        stats
    }

    fn is_scroll_heavy(self) -> bool {
        self.bytes > 5_000
            || self.lines > 60
            || (self.bytes > 2_500
                && (self.lines > 36
                    || self.fence_markers >= 4
                    || self.table_like_lines >= 3
                    || self.blockquote_lines >= 6
                    || self.list_like_lines >= 14
                    || self.link_like_spans >= 8))
            || self.link_like_spans >= 14
    }
}

fn count_link_like_spans(line: &str) -> usize {
    let markdown_targets = markdown_link_target_ranges(line);
    markdown_targets.len() + count_bare_link_spans(line, &markdown_targets)
}

fn markdown_link_target_ranges(line: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;

    while let Some(close_label_rel) = line[index..].find("](") {
        let close_label = index + close_label_rel;
        let Some(open_label_rel) = line[..close_label].rfind('[') else {
            index = close_label + 2;
            continue;
        };
        if open_label_rel + 1 >= close_label {
            index = close_label + 2;
            continue;
        }

        let target_start = close_label + 2;
        let Some(close_target_rel) = line[target_start..].find(')') else {
            break;
        };
        let close_target = target_start + close_target_rel;
        if bytes[target_start..close_target]
            .iter()
            .any(|byte| !byte.is_ascii_whitespace())
        {
            ranges.push(target_start..close_target);
        }
        index = close_target + 1;
    }

    ranges
}

fn count_bare_link_spans(line: &str, markdown_targets: &[std::ops::Range<usize>]) -> usize {
    ["http://", "https://", "kit://", "scriptkit://"]
        .iter()
        .map(|needle| {
            line.match_indices(needle)
                .filter(|(index, _)| {
                    !markdown_targets
                        .iter()
                        .any(|range| range.start <= *index && *index < range.end)
                })
                .count()
        })
        .sum()
}

pub struct AgentChatTranscript {
    list_state: ListState,
    messages: Vec<AgentChatThreadMessage>,
    collapsed_ids: HashSet<u64>,
    expanded_heavy_markdown_ids: HashSet<u64>,
    message_views: HashMap<u64, gpui::Entity<TextViewState>>,
    message_texts: HashMap<u64, String>,
    message_stats: HashMap<u64, HeavyMarkdownStats>,
    message_previews: HashMap<u64, String>,
    on_fork_edit_message: Option<ForkEditMessageHandler>,
    fork_points: Vec<AgentChatForkPoint>,
    thread_status: AgentChatThreadStatus,
    ui_variant: AgentChatUiVariant,
    /// While a turn is streaming with no assistant text yet, render content in
    /// the permanent synthetic tail row so visibility changes do not reset the
    /// whole measured list.
    show_activity_row: bool,
    follow_tail: Cell<bool>,
}

impl AgentChatTranscript {
    pub fn new(messages: Vec<AgentChatThreadMessage>, cx: &mut Context<Self>) -> Self {
        let total = messages.len() + 1;
        let list_state = ListState::new(total, ListAlignment::Bottom, px(200.0)).measure_all();
        list_state.set_follow_tail(true);

        let mut transcript = Self {
            list_state,
            messages,
            collapsed_ids: HashSet::new(),
            expanded_heavy_markdown_ids: HashSet::new(),
            message_views: HashMap::new(),
            message_texts: HashMap::new(),
            message_stats: HashMap::new(),
            message_previews: HashMap::new(),
            on_fork_edit_message: None,
            fork_points: Vec::new(),
            thread_status: AgentChatThreadStatus::Idle,
            ui_variant: AgentChatUiVariant::Standard,
            show_activity_row: false,
            follow_tail: Cell::new(true),
        };
        transcript.reconcile_message_views(cx);
        transcript
    }

    fn row_count(&self) -> usize {
        self.messages.len() + 1
    }

    pub fn with_ui_variant(
        mut self,
        ui_variant: AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) -> Self {
        self.ui_variant = ui_variant;
        self.reconcile_message_views(cx);
        self
    }

    pub fn set_ui_variant(&mut self, ui_variant: AgentChatUiVariant, cx: &mut Context<Self>) {
        if self.ui_variant != ui_variant {
            self.ui_variant = ui_variant;
            self.reconcile_message_views(cx);
            cx.notify();
        }
    }

    pub fn list_state(&self) -> ListState {
        self.list_state.clone()
    }

    fn messages_match_current(&self, messages: &[AgentChatThreadMessage]) -> bool {
        self.messages.len() == messages.len()
            && self
                .messages
                .iter()
                .zip(messages.iter())
                .all(|(current, incoming)| {
                    current.id == incoming.id
                        && current.role == incoming.role
                        && current.body == incoming.body
                        && current.tool_call_id == incoming.tool_call_id
                        && current.tool_meta == incoming.tool_meta
                })
    }

    fn message_prefix_matches_current(&self, messages: &[AgentChatThreadMessage]) -> bool {
        self.messages
            .iter()
            .zip(messages.iter())
            .take(self.messages.len().min(messages.len()))
            .all(|(current, incoming)| current.id == incoming.id && current.role == incoming.role)
    }

    /// Markdown text shown in the message body view.
    ///
    /// Tool messages embed `title\nstatus\noutput` for history compatibility;
    /// when structured card meta is present the card header already renders
    /// title and status, so the body view shows only the output lines.
    fn display_body(msg: &AgentChatThreadMessage) -> String {
        if msg.tool_meta.is_some() {
            let mut lines = msg.body.lines();
            let _title = lines.next();
            let _status = lines.next();
            lines.collect::<Vec<_>>().join("\n")
        } else {
            msg.body.to_string()
        }
    }

    fn markdown_link_policy_for(
        ui_variant: AgentChatUiVariant,
        msg: &AgentChatThreadMessage,
    ) -> MarkdownLinkLabelPolicy {
        if ui_variant == AgentChatUiVariant::QuickAi
            && matches!(msg.role, AgentChatThreadMessageRole::Assistant)
            && msg.tool_meta.is_none()
        {
            MarkdownLinkLabelPolicy::CompactLongBareHttp
        } else {
            MarkdownLinkLabelPolicy::Preserve
        }
    }

    fn should_use_heavy_markdown_preview(
        msg: &AgentChatThreadMessage,
        stats: HeavyMarkdownStats,
    ) -> bool {
        matches!(
            msg.role,
            AgentChatThreadMessageRole::User | AgentChatThreadMessageRole::Assistant
        ) && msg.tool_meta.is_none()
            && stats.is_scroll_heavy()
    }

    fn should_use_heavy_markdown_preview_for(
        ui_variant: AgentChatUiVariant,
        msg: &AgentChatThreadMessage,
        stats: HeavyMarkdownStats,
    ) -> bool {
        Self::should_use_heavy_markdown_preview(msg, stats)
            && Self::markdown_link_policy_for(ui_variant, msg) == MarkdownLinkLabelPolicy::Preserve
    }

    fn should_parse_message_immediately(
        msg: &AgentChatThreadMessage,
        previous_text: Option<&str>,
        display_text: &str,
        is_latest_message: bool,
    ) -> bool {
        matches!(msg.role, AgentChatThreadMessageRole::Assistant)
            && !display_text.trim().is_empty()
            && previous_text
                .map(|text| text.trim().is_empty())
                .unwrap_or(is_latest_message)
    }

    fn heavy_markdown_preview_text(text: &str) -> String {
        const MAX_LINES: usize = 28;
        const MAX_CHARS: usize = 1_800;

        let mut out = String::new();
        for line in text.lines().take(MAX_LINES) {
            if out.len() + line.len() + 1 > MAX_CHARS {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }

        out.trim_end().to_string()
    }

    fn reconcile_message_views(&mut self, cx: &mut Context<Self>) {
        let message_count = self.messages.len();
        for (message_index, msg) in self.messages.iter().enumerate() {
            let display_text = Self::display_body(msg);
            let stats = HeavyMarkdownStats::from_text(&display_text);
            let use_preview =
                Self::should_use_heavy_markdown_preview_for(self.ui_variant, msg, stats);
            let expanded = self.expanded_heavy_markdown_ids.contains(&msg.id);

            self.message_stats.insert(msg.id, stats);
            if use_preview {
                self.message_previews
                    .insert(msg.id, Self::heavy_markdown_preview_text(&display_text));
            } else {
                self.message_previews.remove(&msg.id);
            }

            if use_preview && !expanded {
                self.message_views.remove(&msg.id);
                self.message_texts.remove(&msg.id);
                continue;
            }

            let previous_text = self.message_texts.get(&msg.id).map(String::as_str);
            let parse_immediately = Self::should_parse_message_immediately(
                msg,
                previous_text,
                &display_text,
                message_index + 1 == message_count,
            );

            match self.message_views.entry(msg.id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let parse_initially = parse_immediately
                        || (matches!(msg.role, AgentChatThreadMessageRole::Assistant)
                            && display_text.trim().is_empty());
                    entry.insert(cx.new(|cx| {
                        if parse_initially {
                            TextViewState::markdown_immediate(&display_text, cx)
                        } else {
                            TextViewState::markdown(&display_text, cx)
                        }
                    }));
                    self.message_texts.insert(msg.id, display_text);
                }
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let text_changed =
                        previous_text.is_none_or(|last_text| last_text != display_text);
                    if text_changed {
                        entry.get().update(cx, |state, cx| {
                            if parse_immediately {
                                state.set_markdown_text_immediate(&display_text, cx);
                            } else {
                                state.set_text(&display_text, cx);
                            }
                        });
                        self.message_texts.insert(msg.id, display_text);
                    }
                }
            }
        }
    }

    pub fn set_messages(&mut self, messages: Vec<AgentChatThreadMessage>, cx: &mut Context<Self>) {
        if self.messages_match_current(&messages) {
            return;
        }

        let old_message_count = self.messages.len();
        let tail_only_count_change = self.message_prefix_matches_current(&messages);
        self.messages = messages;
        let new_message_count = self.messages.len();

        if new_message_count != old_message_count {
            if tail_only_count_change {
                self.list_state.splice(
                    old_message_count.min(new_message_count)..old_message_count,
                    new_message_count.saturating_sub(old_message_count),
                );
            } else {
                self.list_state.reset(self.row_count());
            }
        }

        // Clean up message inputs for deleted messages
        let active_ids: HashSet<u64> = self.messages.iter().map(|m| m.id).collect();
        self.expanded_heavy_markdown_ids
            .retain(|id| active_ids.contains(id));
        self.message_views.retain(|id, _| active_ids.contains(id));
        self.message_texts.retain(|id, _| active_ids.contains(id));
        self.message_stats.retain(|id, _| active_ids.contains(id));
        self.message_previews
            .retain(|id, _| active_ids.contains(id));
        self.reconcile_message_views(cx);

        cx.notify();
    }

    pub fn set_show_activity_row(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_activity_row == show {
            return;
        }
        self.show_activity_row = show;
        cx.notify();
    }

    pub(crate) fn set_on_fork_edit_message(&mut self, handler: ForkEditMessageHandler) {
        self.on_fork_edit_message = Some(handler);
    }

    pub(crate) fn set_fork_points(
        &mut self,
        fork_points: Vec<AgentChatForkPoint>,
        cx: &mut Context<Self>,
    ) {
        if self.fork_points == fork_points {
            return;
        }
        self.fork_points = fork_points;
        cx.notify();
    }

    pub(crate) fn set_thread_status(
        &mut self,
        thread_status: AgentChatThreadStatus,
        cx: &mut Context<Self>,
    ) {
        if self.thread_status == thread_status {
            return;
        }
        self.thread_status = thread_status;
        cx.notify();
    }

    pub fn toggle_collapsed(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.collapsed_ids.contains(&id) {
            self.collapsed_ids.remove(&id);
        } else {
            self.collapsed_ids.insert(id);
        }
        cx.notify();
    }

    fn expand_heavy_markdown(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.expanded_heavy_markdown_ids.insert(id) {
            self.reconcile_message_views(cx);
            cx.notify();
        }
    }

    /// Whether a collapsible message renders expanded by default, before any
    /// user toggle. Edit diffs and failed tools surface their body immediately;
    /// everything else starts collapsed.
    fn default_expanded(msg: &AgentChatThreadMessage) -> bool {
        msg.tool_meta
            .as_ref()
            .is_some_and(|meta| meta.diff.is_some() || meta.is_error)
    }

    /// `collapsed_ids` records user toggles, so the effective state is the
    /// default expansion XOR a recorded toggle.
    fn is_collapsed_for(msg: &AgentChatThreadMessage, toggled: &HashSet<u64>) -> bool {
        let is_collapsible = matches!(
            msg.role,
            AgentChatThreadMessageRole::Thought | AgentChatThreadMessageRole::Tool
        );
        if !is_collapsible {
            return false;
        }
        let expanded = Self::default_expanded(msg) ^ toggled.contains(&msg.id);
        !expanded
    }

    pub fn clear_collapsed_ids(&mut self, cx: &mut Context<Self>) {
        self.collapsed_ids.clear();
        cx.notify();
    }

    pub fn expand_ids(&mut self, ids: Vec<u64>, cx: &mut Context<Self>) {
        self.collapsed_ids.extend(ids);
        cx.notify();
    }

    pub fn scroll_to_reveal_item(&self, index: usize) {
        self.follow_tail.set(false);
        self.list_state.set_follow_tail(false);
        self.list_state.scroll_to_reveal_item(index);
    }

    pub fn logical_scroll_top(&self) -> ListOffset {
        self.list_state.logical_scroll_top()
    }

    pub fn scroll_to(&self, offset: ListOffset) {
        self.follow_tail.set(false);
        self.list_state.set_follow_tail(false);
        self.list_state.scroll_to(offset);
    }

    pub fn scroll_to_end(&self) {
        self.follow_tail.set(true);
        self.list_state.set_follow_tail(true);
    }

    pub(crate) fn scroll_metrics(&self) -> crate::protocol::AgentChatTranscriptScrollMetrics {
        const GPUI_SCROLLBAR_MIN_THUMB_SIZE_PX: f32 = 48.0;

        let logical = self.list_state.logical_scroll_top();
        let viewport_height = self
            .list_state
            .viewport_bounds()
            .size
            .height
            .as_f32()
            .max(0.0);
        let max_scroll_top = self
            .list_state
            .max_offset_for_scrollbar()
            .y
            .as_f32()
            .max(0.0);
        let scroll_offset = self.list_state.scroll_px_offset_for_scrollbar();
        let scroll_top = (-scroll_offset.y.as_f32()).clamp(0.0, max_scroll_top);
        let content_height = viewport_height + max_scroll_top;
        let can_scroll_y = content_height > viewport_height && viewport_height > 0.0;

        let (thumb_height, thumb_top, thumb_bottom, thumb_position_ratio) = if can_scroll_y {
            let thumb_track_height = viewport_height;
            let thumb_height = ((viewport_height / content_height) * thumb_track_height)
                .max(GPUI_SCROLLBAR_MIN_THUMB_SIZE_PX)
                .min(thumb_track_height);
            let thumb_position_ratio = if max_scroll_top > 0.0 {
                (scroll_top / max_scroll_top).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let thumb_top = thumb_position_ratio * (thumb_track_height - thumb_height).max(0.0);
            (
                thumb_height,
                thumb_top,
                thumb_top + thumb_height,
                thumb_position_ratio,
            )
        } else {
            (viewport_height, 0.0, viewport_height, 0.0)
        };

        crate::protocol::AgentChatTranscriptScrollMetrics {
            row_count: self.row_count(),
            scroll_top_item: logical.item_ix,
            scroll_top_offset_px: logical.offset_in_item.as_f32(),
            viewport_height_px: viewport_height,
            content_height_px: content_height,
            scroll_top_px: scroll_top,
            max_scroll_top_px: max_scroll_top,
            can_scroll_y,
            thumb_track_height_px: viewport_height,
            thumb_height_px: thumb_height,
            thumb_top_px: thumb_top,
            thumb_bottom_px: thumb_bottom,
            thumb_position_ratio,
            measurement_source: "listState".to_string(),
            follow_tail: self.follow_tail.get(),
            manual_scroll: !self.follow_tail.get(),
            pending_indicator_count: usize::from(!matches!(
                pending_activity_placement(
                    &self.messages,
                    self.show_activity_row
                        || matches!(self.thread_status, AgentChatThreadStatus::Streaming),
                ),
                PendingActivityPlacement::Hidden
            )),
            streaming_copy_available: matches!(
                self.thread_status,
                AgentChatThreadStatus::Streaming
            ) && self.messages.iter().rev().any(|message| {
                matches!(message.role, AgentChatThreadMessageRole::Assistant)
                    && !message.body.trim().is_empty()
            }),
            row_semantic_ids: self
                .messages
                .iter()
                .map(transcript_row_fidelity_id)
                .collect(),
        }
    }

    pub(crate) fn viewport_bounds_px(&self) -> (f32, f32, f32, f32) {
        let bounds = self.list_state.viewport_bounds();
        (
            bounds.origin.x.as_f32(),
            bounds.origin.y.as_f32(),
            bounds.size.width.as_f32(),
            bounds.size.height.as_f32(),
        )
    }

    /// Build (or reuse) the markdown `TextViewStyle` for transcript rows.
    ///
    /// This runs once per visible row per frame during scrolling, and building
    /// a `HighlightTheme` clones a full syntax style table. Memoize on the
    /// exact inputs so repeated frames return the same `Arc`ed theme, which
    /// also lets `TextViewStyle::eq` inside `TextView` hit its pointer
    /// fast path instead of deep-comparing the syntax table per row per frame.
    fn transcript_text_style(
        theme: &crate::theme::Theme,
        colors: &PromptColors,
        style_def: &AgentChatStyleDef,
    ) -> TextViewStyle {
        #[derive(PartialEq)]
        struct StyleCacheKey {
            style_def: AgentChatStyleDef,
            is_dark: bool,
            accent: u32,
            secondary: u32,
            muted: u32,
            code_bg: u32,
            quote_border: u32,
        }
        thread_local! {
            static STYLE_CACHE: std::cell::RefCell<Option<(StyleCacheKey, TextViewStyle)>> =
                const { std::cell::RefCell::new(None) };
        }

        let key = StyleCacheKey {
            style_def: *style_def,
            is_dark: theme.is_dark_mode(),
            accent: theme.colors.accent.selected,
            secondary: theme.colors.text.secondary,
            muted: theme.colors.text.muted,
            code_bg: colors.code_bg,
            quote_border: colors.quote_border,
        };

        if let Some(style) = STYLE_CACHE.with(|cache| {
            cache
                .borrow()
                .as_ref()
                .filter(|(cached_key, _)| *cached_key == key)
                .map(|(_, style)| style.clone())
        }) {
            return style;
        }

        let style = Self::build_transcript_text_style(theme, colors, style_def);
        STYLE_CACHE.with(|cache| {
            *cache.borrow_mut() = Some((key, style.clone()));
        });
        style
    }

    fn build_transcript_text_style(
        _theme: &crate::theme::Theme,
        colors: &PromptColors,
        style_def: &AgentChatStyleDef,
    ) -> TextViewStyle {
        // PromptColors.code_bg/quote_border are background.search_box and
        // ui.border — the same theme authorities the shared resolver reads.
        let _ = colors;
        let resolved = resolved_agent_chat_transcript_colors(style_def, _theme);
        let code_bg = rgba(resolved.code_bg_rgba);
        let code_border = rgba(resolved.code_border_rgba);
        let blockquote_bg = rgba(resolved.blockquote_bg_rgba);
        let blockquote_border = rgba(resolved.blockquote_border_rgba);
        let heading_1_font_size = style_def.markdown.heading_1_font_size;
        let heading_2_font_size = style_def.markdown.heading_2_font_size;
        let heading_3_font_size = style_def.markdown.heading_3_font_size;
        let body_font_size = style_def.markdown.body_font_size;
        let mut style = TextViewStyle::default()
            .paragraph_gap(rems(style_def.markdown.paragraph_gap))
            .heading_font_size(move |level, _base_size| match level {
                1 => px(heading_1_font_size),
                2 => px(heading_2_font_size),
                3 => px(heading_3_font_size),
                _ => px(body_font_size),
            })
            .code_block(
                StyleRefinement::default()
                    .bg(code_bg)
                    .border_1()
                    .border_color(code_border)
                    .rounded(px(style_def.markdown.code_block_radius))
                    .px(px(style_def.markdown.code_block_padding_x))
                    .py(px(style_def.markdown.code_block_padding_y))
                    .text_size(px(style_def.markdown.code_block_font_size)),
            )
            .code_block_copy_button(true)
            .blockquote(
                StyleRefinement::default()
                    .bg(blockquote_bg)
                    .border_color(blockquote_border)
                    .rounded(px(style_def.markdown.blockquote_radius))
                    .px(px(style_def.markdown.blockquote_padding_x))
                    .py(px(style_def.markdown.blockquote_padding_y)),
            );

        style.highlight_theme = Arc::new(
            crate::theme::gpui_integration::build_markdown_highlight_theme(
                _theme,
                _theme.is_dark_mode(),
            ),
        );
        style.is_dark = _theme.is_dark_mode();
        style
    }

    fn selectable_markdown_view(
        text_view_state: &gpui::Entity<TextViewState>,
        theme: &crate::theme::Theme,
        colors: &PromptColors,
        text_color: Rgba,
        style_def: &AgentChatStyleDef,
        fidelity_scope: SharedString,
    ) -> TextView {
        Self::selectable_markdown_view_with_policy(
            text_view_state,
            theme,
            colors,
            text_color,
            style_def,
            fidelity_scope,
            MarkdownLinkLabelPolicy::Preserve,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn selectable_markdown_view_with_policy(
        text_view_state: &gpui::Entity<TextViewState>,
        theme: &crate::theme::Theme,
        colors: &PromptColors,
        text_color: Rgba,
        style_def: &AgentChatStyleDef,
        fidelity_scope: SharedString,
        link_label_policy: MarkdownLinkLabelPolicy,
    ) -> TextView {
        let style = Self::transcript_text_style(theme, colors, style_def)
            .markdown_link_label_policy(link_label_policy);
        TextView::new(text_view_state)
            .style(style)
            .selectable(crate::logging::agent_chat_markdown_selectable_enabled())
            .fidelity_scope(fidelity_scope)
            .w_full()
            .text_size(px(style_def.markdown.body_font_size))
            .text_color(text_color)
    }

    fn message_text_fidelity_scope(msg: &AgentChatThreadMessage) -> SharedString {
        format!("{}/text", transcript_row_fidelity_id(msg)).into()
    }

    fn render_heavy_markdown_preview(
        msg: &AgentChatThreadMessage,
        preview: &str,
        stats: HeavyMarkdownStats,
        colors: &PromptColors,
        theme: &crate::theme::Theme,
        style_def: &AgentChatStyleDef,
        entity: &gpui::WeakEntity<AgentChatTranscript>,
    ) -> gpui::AnyElement {
        let message_id = msg.id;
        let entity = entity.clone();

        let mut preview_body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .text_size(px(style_def.markdown.body_font_size))
            .text_color(rgb(colors.text_primary));

        for line in preview.lines() {
            preview_body = preview_body.child(div().w_full().child(line.to_string()));
        }

        div()
            .id(SharedString::from(format!(
                "agent_chat-heavy-markdown-preview-{message_id}"
            )))
            .w_full()
            .px(px(style_def.assistant_message.padding_x))
            .py(px(style_def.assistant_message.padding_y))
            .rounded(px(style_def.assistant_message.radius))
            .bg(rgba((theme.colors.text.primary << 8) | 0x08))
            .border_l_2()
            .border_color(rgba((theme.colors.accent.selected << 8) | 0x55))
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                if let Some(transcript) = entity.upgrade() {
                    transcript.update(cx, |this, cx| {
                        this.expand_heavy_markdown(message_id, cx);
                    });
                }
            })
            .child(preview_body)
            .child(
                div()
                    .pt(px(6.0))
                    .text_size(px((style_def.markdown.body_font_size - 1.0).max(10.0)))
                    .opacity(0.62)
                    .text_color(rgb(colors.accent_color))
                    .child(format!(
                        "Heavy markdown preview - {} lines, {} chars - show full markdown",
                        stats.lines, stats.chars
                    )),
            )
            .into_any_element()
    }

    /// Attach the expand/collapse click handler to a collapsible header.
    /// Routed through the transcript entity so toggling re-renders the row.
    fn with_toggle_click(
        header: gpui::Stateful<gpui::Div>,
        entity: &gpui::WeakEntity<AgentChatTranscript>,
        message_id: u64,
    ) -> gpui::Stateful<gpui::Div> {
        let entity = entity.clone();
        header.on_click(move |_event, _window, cx| {
            if let Some(transcript) = entity.upgrade() {
                transcript.update(cx, |this, cx| this.toggle_collapsed(message_id, cx));
            }
        })
    }

    // Render entry point threaded straight from the transcript's own state; the
    // parameter list mirrors that state rather than a synthetic params struct.
    #[allow(clippy::too_many_arguments)]
    fn render_message(
        ui_variant: AgentChatUiVariant,
        msg: &AgentChatThreadMessage,
        colors: &PromptColors,
        is_collapsed: bool,
        text_view_state: &gpui::Entity<TextViewState>,
        style_def: &AgentChatStyleDef,
        entity: &gpui::WeakEntity<AgentChatTranscript>,
        messages: &[AgentChatThreadMessage],
        fork_points: &[AgentChatForkPoint],
        thread_status: AgentChatThreadStatus,
        on_fork_edit_message: Option<ForkEditMessageHandler>,
        show_pending_activity: bool,
    ) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();
        let presentation = ui_variant.config().transcript;

        match msg.role {
            AgentChatThreadMessageRole::User => Self::render_user_message(
                msg,
                colors,
                &theme,
                text_view_state,
                presentation,
                style_def,
                entity,
                messages,
                fork_points,
                thread_status,
                on_fork_edit_message,
            ),
            AgentChatThreadMessageRole::Assistant => Self::render_assistant_message(
                ui_variant,
                msg,
                colors,
                &theme,
                text_view_state,
                presentation,
                style_def,
                show_pending_activity,
            ),
            AgentChatThreadMessageRole::Thought => Self::render_collapsible_block(
                msg,
                colors,
                &theme,
                is_collapsed,
                false,
                text_view_state,
                style_def,
                entity,
            ),
            AgentChatThreadMessageRole::Tool => Self::render_collapsible_block(
                msg,
                colors,
                &theme,
                is_collapsed,
                true,
                text_view_state,
                style_def,
                entity,
            ),
            AgentChatThreadMessageRole::Error => {
                Self::render_error_message(msg, colors, text_view_state, style_def)
            }
            AgentChatThreadMessageRole::System => {
                Self::render_system_message(msg, colors, &theme, text_view_state, style_def)
            }
        }
    }

    fn render_pending_activity(
        theme: &crate::theme::Theme,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        let resolved = resolved_agent_chat_transcript_colors(style_def, theme);
        let dot = div()
            .size(px(style_contract::AGENT_CHAT_ACTIVITY_DOT_SIZE))
            .rounded(px(999.0))
            .bg(rgb(theme.colors.accent.selected))
            .with_animation(
                "agent-chat-transcript-thinking-pulse",
                Animation::new(std::time::Duration::from_millis(AMBIENT_PULSE_CYCLE_MS)).repeat(),
                |style, delta| {
                    let sine = (delta * std::f32::consts::PI * 2.0).sin();
                    style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                },
            );

        div()
            .debug_selector(|| "agent-chat-transcript-pending".to_string())
            .flex()
            .items_center()
            .gap(px(style_contract::AGENT_CHAT_ACTIVITY_GAP))
            .child(dot)
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .text_color(rgba(resolved.activity_label_rgba))
                    .child("Thinking\u{2026}"),
            )
            .into_any()
    }

    fn render_activity_row(
        theme: &crate::theme::Theme,
        colors: &PromptColors,
        presentation: AgentChatTranscriptPresentation,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        div()
            .debug_selector(|| "agent-chat-transcript-row-assistant-pending".to_string())
            .w_full()
            .px(px(style_def.transcript.row_padding_x))
            .pb(px(style_def.transcript.row_padding_bottom))
            .mt(px(style_def.transcript.response_start_margin_top))
            .child(Self::render_assistant_message_shell(
                Self::render_pending_activity(theme, style_def),
                colors,
                theme,
                presentation,
                style_def,
            ))
            .into_any()
    }

    fn render_empty_activity_row() -> gpui::AnyElement {
        div().w_full().h(px(0.0)).overflow_hidden().into_any()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_user_message(
        msg: &AgentChatThreadMessage,
        _colors: &PromptColors,
        theme: &crate::theme::Theme,
        text_view_state: &gpui::Entity<TextViewState>,
        presentation: AgentChatTranscriptPresentation,
        style_def: &AgentChatStyleDef,
        _entity: &gpui::WeakEntity<AgentChatTranscript>,
        messages: &[AgentChatThreadMessage],
        fork_points: &[AgentChatForkPoint],
        thread_status: AgentChatThreadStatus,
        on_fork_edit_message: Option<ForkEditMessageHandler>,
    ) -> gpui::AnyElement {
        let user_style = style_def.user_message;
        let forkable =
            matches!(
                thread_status,
                AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error
            ) && AgentChatThread::fork_point_for_message_id(messages, fork_points, msg.id)
                .is_some();
        let message_id = msg.id;
        let edit_button = (forkable && on_fork_edit_message.is_some()).then(|| {
            let on_fork_edit_message = on_fork_edit_message.clone();
            div()
                .id(SharedString::from(format!(
                    "agent_chat-fork-edit-{message_id}"
                )))
                .absolute()
                .top(px(4.0))
                .right(px(4.0))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.0))
                .rounded(px(6.0))
                .bg(rgba((theme.colors.text.primary << 8) | 0x08))
                .text_color(rgb(_colors.text_primary))
                .text_size(px((style_def.markdown.body_font_size - 1.0).max(10.0)))
                .opacity(0.0)
                .group_hover("agent-chat-user-message-row", |d| d.opacity(0.82))
                .hover(|d| d.opacity(1.0))
                .cursor_pointer()
                .tooltip(|window, cx| {
                    Tooltip::new("Edit & branch from here — removes later messages")
                        .build(window, cx)
                })
                .on_click(move |_event, window, cx| {
                    if let Some(handler) = on_fork_edit_message.as_ref() {
                        handler(message_id, window, cx);
                    }
                })
                .child("✎")
        });

        // Visible receipt of what was attached with this message: label with
        // provenance ("Selection — Safari") plus a short excerpt of the text,
        // so the user can see WHAT is being rewritten and where it came from.
        let attachment_receipts = (!msg.attachments.is_empty()).then(|| {
            let label_color = rgba((theme.colors.text.primary << 8) | 0x99);
            let snippet_color = rgba((theme.colors.text.primary << 8) | 0xc2);
            let quote_bar = rgba((theme.colors.text.primary << 8) | 0x2e);
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .pb(px(8.0))
                .children(msg.attachments.iter().map(|attachment| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .pl(px(8.0))
                        .border_l_2()
                        .border_color(quote_bar)
                        .child(
                            div()
                                .text_size(px((style_def.markdown.body_font_size - 2.0).max(10.0)))
                                .text_color(label_color)
                                .child(attachment.label.clone()),
                        )
                        .when_some(attachment.snippet.clone(), |d, snippet| {
                            d.child(
                                div()
                                    .text_size(px(
                                        (style_def.markdown.body_font_size - 1.0).max(10.0)
                                    ))
                                    .text_color(snippet_color)
                                    .italic()
                                    .child(snippet),
                            )
                        })
                }))
        });

        let bubble = div()
            .relative()
            .group("agent-chat-user-message-row")
            .w_full()
            .px(px(user_style.padding_x))
            .py(
                if matches!(presentation, AgentChatTranscriptPresentation::DenseLog) {
                    px(user_style.dense_padding_y)
                } else {
                    px(user_style.padding_y)
                },
            )
            .rounded(px(user_style.radius))
            .bg(rgba(
                resolved_agent_chat_transcript_colors(style_def, theme).user_bg_rgba,
            ))
            .when(
                matches!(presentation, AgentChatTranscriptPresentation::UserBold),
                |d| d.font_weight(FontWeight::BOLD),
            )
            .when_some(attachment_receipts, |d, receipts| d.child(receipts))
            .child(Self::selectable_markdown_view(
                text_view_state,
                theme,
                _colors,
                rgb(_colors.text_primary),
                style_def,
                Self::message_text_fidelity_scope(msg),
            ))
            .when_some(edit_button, |d, button| d.child(button));

        if matches!(presentation, AgentChatTranscriptPresentation::RoleSplit) {
            div()
                .w_full()
                .flex()
                .justify_end()
                .child(div().max_w(px(user_style.max_width)).child(bubble))
                .into_any_element()
        } else {
            bubble.into_any_element()
        }
    }

    fn render_assistant_message(
        ui_variant: AgentChatUiVariant,
        msg: &AgentChatThreadMessage,
        _colors: &PromptColors,
        _theme: &crate::theme::Theme,
        text_view_state: &gpui::Entity<TextViewState>,
        presentation: AgentChatTranscriptPresentation,
        style_def: &AgentChatStyleDef,
        show_pending_activity: bool,
    ) -> gpui::AnyElement {
        let content = if show_pending_activity {
            Self::render_pending_activity(_theme, style_def)
        } else {
            Self::selectable_markdown_view_with_policy(
                text_view_state,
                _theme,
                _colors,
                rgb(_colors.text_primary),
                style_def,
                Self::message_text_fidelity_scope(msg),
                Self::markdown_link_policy_for(ui_variant, msg),
            )
            .into_any_element()
        };

        Self::render_assistant_message_shell(content, _colors, _theme, presentation, style_def)
    }

    fn render_assistant_message_shell(
        content: gpui::AnyElement,
        _colors: &PromptColors,
        theme: &crate::theme::Theme,
        presentation: AgentChatTranscriptPresentation,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        let assistant_style = style_def.assistant_message;
        let message = div()
            .w_full()
            .px(px(assistant_style.padding_x))
            .py(
                if matches!(presentation, AgentChatTranscriptPresentation::DenseLog) {
                    px(assistant_style.dense_padding_y)
                } else {
                    px(assistant_style.padding_y)
                },
            )
            .rounded(px(assistant_style.radius))
            .when(assistant_style.bg_alpha > 0.0, |d| {
                d.bg(rgba(style_contract::pack_rgb_alpha(
                    theme.colors.text.primary,
                    assistant_style.bg_alpha,
                )))
            })
            .child(content);

        if matches!(presentation, AgentChatTranscriptPresentation::RoleSplit) {
            div()
                .w_full()
                .flex()
                .justify_start()
                .child(div().max_w(px(assistant_style.max_width)).child(message))
                .into_any_element()
        } else {
            message.into_any_element()
        }
    }

    /// Maximum diff rows rendered inline before truncating with a marker.
    /// Bounds element count for very large edits; the full diff remains in
    /// the thread state.
    const MAX_DIFF_ROWS: usize = 200;

    /// Render the colored, line-numbered diff Pi emits for edit/write tools.
    /// Lines are classified by their `+`/`-`/space marker prefix.
    fn render_diff_body(
        diff: &str,
        theme: &crate::theme::Theme,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        let block_style = style_def.collapsible;
        let code_font_size = style_def.markdown.code_block_font_size;
        let total_rows = diff.lines().count();
        let resolved = resolved_agent_chat_transcript_colors(style_def, theme);

        let mut rows = div()
            .flex()
            .flex_col()
            .w_full()
            .mt(px(block_style.body_padding_top))
            .rounded(px(style_def.markdown.code_block_radius))
            .bg(rgba(resolved.code_bg_rgba))
            .px(px(style_def.markdown.code_block_padding_x))
            .py(px(style_def.markdown.code_block_padding_y))
            .font_family(FONT_MONO)
            .text_size(px(code_font_size));

        for line in diff.lines().take(Self::MAX_DIFF_ROWS) {
            let row = div().w_full().whitespace_nowrap().child(line.to_string());
            let row = match classify_diff_line(line) {
                DiffLineKind::Added => row
                    .text_color(rgb(theme.colors.ui.success))
                    .bg(rgba(resolved.diff_added_bg_rgba)),
                DiffLineKind::Removed => row
                    .text_color(rgb(theme.colors.ui.error))
                    .bg(rgba(resolved.diff_removed_bg_rgba)),
                DiffLineKind::Context => row
                    .text_color(rgb(theme.colors.text.primary))
                    .opacity(style_contract::AGENT_CHAT_DIFF_CONTEXT_OPACITY),
            };
            rows = rows.child(row);
        }

        if total_rows > Self::MAX_DIFF_ROWS {
            rows = rows.child(
                div()
                    .text_color(rgb(theme.colors.text.primary))
                    .opacity(0.45)
                    .child(format!(
                        "\u{2026} {} more lines",
                        total_rows - Self::MAX_DIFF_ROWS
                    )),
            );
        }

        rows.into_any_element()
    }

    /// Render a tool call as a structured card: status badge, kind glyph,
    /// tool name, args subject, and (expanded) diff or output body.
    #[allow(clippy::too_many_arguments)]
    fn render_tool_card(
        msg: &AgentChatThreadMessage,
        meta: &AgentChatToolCardMeta,
        _colors: &PromptColors,
        theme: &crate::theme::Theme,
        is_collapsed: bool,
        text_view_state: &gpui::Entity<TextViewState>,
        style_def: &AgentChatStyleDef,
        entity: &gpui::WeakEntity<AgentChatTranscript>,
    ) -> gpui::AnyElement {
        let block_style = style_def.collapsible;
        let resolved = resolved_agent_chat_transcript_colors(style_def, theme);
        let status_color = match meta.status {
            AgentChatToolStatus::Pending => rgba(resolved.tool_status_pending_rgba),
            AgentChatToolStatus::Running => rgb(_colors.accent_color),
            AgentChatToolStatus::Complete => rgb(theme.colors.ui.success),
            AgentChatToolStatus::Failed => rgb(theme.colors.ui.error),
        };
        let left_border_color = if meta.is_error {
            rgba(resolved.tool_border_error_rgba)
        } else {
            rgba(resolved.tool_border_rgba)
        };
        let chevron = if is_collapsed {
            "\u{25B8}" // ▸
        } else {
            "\u{25BE}" // ▾
        };

        let display_body = Self::display_body(msg);
        let has_body = !display_body.trim().is_empty();
        let collapsed_line_count = meta
            .diff
            .as_deref()
            .map(|diff| diff.lines().count())
            .unwrap_or_else(|| display_body.lines().count());

        let header = div()
            .id(SharedString::from(format!("agent_chat-toggle-{}", msg.id)))
            .flex()
            .items_center()
            .gap(px(style_contract::AGENT_CHAT_BLOCK_HEADER_GAP))
            .cursor_pointer()
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .opacity(block_style.tool_header_opacity)
                    .text_color(rgb(_colors.accent_color))
                    .child(chevron.to_string()),
            )
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .text_color(status_color)
                    .child(format!("{} ", meta.status.glyph())),
            )
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .opacity(block_style.tool_header_opacity)
                    .text_color(rgb(_colors.accent_color))
                    .child(format!("{} {}", meta.kind.glyph(), meta.tool_name)),
            )
            .when_some(meta.subject.clone(), |d, subject| {
                d.child(
                    div()
                        // Mono renders optically larger than the UI font at equal px,
                        // so the subject tracks the code-block size, one step under body.
                        .text_size(px(style_def.markdown.code_block_font_size))
                        .min_w(px(0.0))
                        .flex_shrink()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .font_family(FONT_MONO)
                        .opacity(block_style.status_opacity)
                        .text_color(rgb(_colors.text_primary))
                        .child(subject),
                )
            })
            .when(matches!(meta.status, AgentChatToolStatus::Failed), |d| {
                d.child(
                    div()
                        .text_size(px(style_def.markdown.body_font_size))
                        .text_color(rgb(theme.colors.ui.error))
                        .child(meta.status.label().to_string()),
                )
            })
            // Collapsed-size hint; single-line bodies get no badge (the
            // chevron already says "expandable" and "1 lines" reads broken).
            .when(is_collapsed && collapsed_line_count > 1, |d| {
                d.child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(px(style_def.markdown.body_font_size))
                        .opacity(block_style.status_opacity)
                        .text_color(rgb(_colors.accent_color))
                        .child(format!("{collapsed_line_count} lines")),
                )
            });
        let header = Self::with_toggle_click(header, entity, msg.id);

        let mut container = div()
            .w_full()
            .pl(px(block_style.padding_x))
            .pr(px(block_style.padding_x))
            .py(px(block_style.padding_y))
            .border_l(px(style_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH))
            .border_color(left_border_color)
            .child(header);

        if !is_collapsed {
            if let Some(diff) = meta.diff.as_deref() {
                container = container.child(
                    div()
                        .max_h(px(block_style.max_body_height))
                        .overflow_y_hidden()
                        .child(Self::render_diff_body(diff, theme, style_def)),
                );
            } else if has_body {
                container = container.child(
                    div()
                        .pt(px(block_style.body_padding_top))
                        .max_h(px(block_style.max_body_height))
                        .overflow_y_hidden()
                        .child(Self::selectable_markdown_view(
                            text_view_state,
                            theme,
                            _colors,
                            rgb(_colors.accent_color),
                            style_def,
                            Self::message_text_fidelity_scope(msg),
                        )),
                );
            }
        }

        container.into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_collapsible_block(
        msg: &AgentChatThreadMessage,
        _colors: &PromptColors,
        theme: &crate::theme::Theme,
        is_collapsed: bool,
        is_tool: bool,
        text_view_state: &gpui::Entity<TextViewState>,
        style_def: &AgentChatStyleDef,
        entity: &gpui::WeakEntity<AgentChatTranscript>,
    ) -> gpui::AnyElement {
        if is_tool {
            if let Some(meta) = msg.tool_meta.as_ref() {
                return Self::render_tool_card(
                    msg,
                    meta,
                    _colors,
                    theme,
                    is_collapsed,
                    text_view_state,
                    style_def,
                    entity,
                );
            }
        }

        let (label, status_hint) = if is_tool {
            let mut lines = msg.body.lines();
            let title = lines
                .next()
                .map(|l| l.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() < 80)
                .unwrap_or_else(|| "Tool".to_string());
            let status = lines
                .next()
                .map(|l| l.trim().to_string())
                .filter(|s| !s.is_empty() && s.len() < 40);
            (title, status)
        } else {
            ("Thinking".to_string(), None)
        };

        let chevron = if is_collapsed {
            "\u{25B8}" // ▸
        } else {
            "\u{25BE}" // ▾
        };

        let line_count = msg.body.lines().count();
        let block_style = style_def.collapsible;
        let header_opacity = if is_tool {
            block_style.tool_header_opacity
        } else {
            block_style.thought_header_opacity
        };
        let resolved = resolved_agent_chat_transcript_colors(style_def, theme);
        let left_border_color = if is_tool {
            rgba(resolved.tool_border_rgba)
        } else {
            rgba(resolved.thought_border_rgba)
        };

        let mut container = div()
            .w_full()
            .pl(px(block_style.padding_x))
            .pr(px(block_style.padding_x))
            .py(px(block_style.padding_y))
            .border_l(px(style_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH))
            .border_color(left_border_color);

        // Header row — clickable toggle uses element ID only (no cx.listener in static context).
        let header = div()
            .id(SharedString::from(format!("agent_chat-toggle-{}", msg.id)))
            .flex()
            .items_center()
            .gap(px(style_contract::AGENT_CHAT_BLOCK_HEADER_GAP))
            .cursor_pointer()
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .opacity(header_opacity)
                    .when(is_tool, |d| d.text_color(rgb(_colors.accent_color)))
                    .child(chevron.to_string()),
            )
            .child(
                div()
                    .text_size(px(style_def.markdown.body_font_size))
                    .opacity(header_opacity)
                    .when(is_tool, |d| d.text_color(rgb(_colors.accent_color)))
                    .child(label),
            )
            .when_some(status_hint.clone(), |d, status| {
                d.child(
                    div()
                        .text_size(px(style_def.markdown.body_font_size))
                        .opacity(block_style.status_opacity)
                        .when(is_tool, |d| d.text_color(rgb(_colors.accent_color)))
                        .child(status),
                )
            })
            .when(
                is_collapsed && line_count > 1 && status_hint.is_none(),
                |d| {
                    d.child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .text_size(px(style_def.markdown.body_font_size))
                            .opacity(block_style.status_opacity)
                            .when(is_tool, |d| d.text_color(rgb(_colors.accent_color)))
                            .child(format!("{line_count} lines")),
                    )
                },
            );
        let header = Self::with_toggle_click(header, entity, msg.id);

        container = container.child(header);

        if !is_collapsed {
            let body_color = if is_tool {
                rgb(_colors.accent_color)
            } else {
                rgb(_colors.text_primary)
            };

            let body = div()
                .pt(px(block_style.body_padding_top))
                .max_h(px(block_style.max_body_height))
                .overflow_y_hidden()
                .child(Self::selectable_markdown_view(
                    text_view_state,
                    theme,
                    _colors,
                    body_color,
                    style_def,
                    Self::message_text_fidelity_scope(msg),
                ));

            container = container.child(body);
        }

        container.into_any_element()
    }

    fn render_error_message(
        msg: &AgentChatThreadMessage,
        _colors: &PromptColors,
        text_view_state: &gpui::Entity<TextViewState>,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        let error_style = style_def.error;
        let resolved = resolved_agent_chat_transcript_colors(style_def, &theme::get_cached_theme());
        div()
            .w_full()
            .px(px(error_style.padding_x))
            .py(px(error_style.padding_y))
            .rounded(px(error_style.radius))
            .bg(rgba(resolved.error_bg_rgba))
            .border_l(px(style_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH))
            .border_color(rgba(resolved.error_border_rgba))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .pb(px(4.0))
                    .child(
                        div()
                            .text_size(px(style_def.markdown.body_font_size))
                            .opacity(error_style.label_opacity)
                            .child("\u{26A0}"),
                    )
                    .child(
                        div()
                            .text_size(px(style_def.markdown.body_font_size))
                            .font_weight(FontWeight::SEMIBOLD)
                            .opacity(error_style.label_opacity)
                            .child("Error"),
                    ),
            )
            .child(Self::selectable_markdown_view(
                text_view_state,
                &theme::get_cached_theme(),
                _colors,
                rgb(_colors.text_primary),
                style_def,
                Self::message_text_fidelity_scope(msg),
            ))
            .child(
                div()
                    .pt(px(4.0))
                    .text_size(px(style_def.markdown.body_font_size))
                    .opacity(error_style.hint_opacity)
                    .child("This failure is saved in the transcript"),
            )
            .into_any_element()
    }

    fn render_system_message(
        msg: &AgentChatThreadMessage,
        _colors: &PromptColors,
        theme: &crate::theme::Theme,
        text_view_state: &gpui::Entity<TextViewState>,
        style_def: &AgentChatStyleDef,
    ) -> gpui::AnyElement {
        let system_style = style_def.system;
        div()
            .w_full()
            .px(px(system_style.padding_x))
            .py(px(system_style.padding_y))
            .opacity(system_style.opacity)
            .border_l(px(style_contract::AGENT_CHAT_BLOCK_BORDER_WIDTH))
            .border_color(rgba(
                resolved_agent_chat_transcript_colors(style_def, theme).system_border_rgba,
            ))
            .child(Self::selectable_markdown_view(
                text_view_state,
                theme,
                _colors,
                rgb(_colors.text_primary),
                style_def,
                Self::message_text_fidelity_scope(msg),
            ))
            .into_any_element()
    }
}

impl Render for AgentChatTranscript {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render_started =
            crate::logging::agent_chat_render_trace_enabled().then(std::time::Instant::now);
        let theme = theme::get_cached_theme();
        let colors = PromptColors::from_theme(&theme);
        let style_def = crate::dev_style_tool::runtime_overrides::effective_agent_chat_style();

        let focused_text_preview = matches!(
            self.ui_variant.config().transcript,
            AgentChatTranscriptPresentation::FocusedTextPreview
        );
        let messages_snapshot = self.messages.clone();
        let on_fork_edit_message = self.on_fork_edit_message.clone();
        let fork_points_snapshot = self.fork_points.clone();
        let thread_status = self.thread_status;
        let collapsed_ids = self.collapsed_ids.clone();
        let visible_indices: Vec<usize> = if focused_text_preview {
            messages_snapshot
                .iter()
                .enumerate()
                .filter_map(|(ix, msg)| {
                    (matches!(msg.role, AgentChatThreadMessageRole::Assistant)
                        && !msg.body.trim().is_empty())
                    .then_some(ix)
                })
                .collect()
        } else {
            (0..messages_snapshot.len()).collect()
        };

        let message_views_snapshot = self.message_views.clone();
        let message_stats_snapshot = self.message_stats.clone();
        let message_previews_snapshot = self.message_previews.clone();
        let expanded_heavy_markdown_ids = self.expanded_heavy_markdown_ids.clone();
        let ui_variant = self.ui_variant;
        let entity = cx.entity().downgrade();
        let scroll_top_item = self.list_state.logical_scroll_top().item_ix;
        let row_count = self.row_count();
        let message_count = self.messages.len();
        let visible_index_count = visible_indices.len();
        let markdown_view_count = message_views_snapshot.len();
        let heavy_preview_count = self.message_previews.len();
        let expanded_heavy_markdown_count = self.expanded_heavy_markdown_ids.len();
        let pending_activity_placement = pending_activity_placement(
            &messages_snapshot,
            self.show_activity_row || matches!(thread_status, AgentChatThreadStatus::Streaming),
        );
        let transcript_content = if focused_text_preview {
            let mut preview = div().size_full().flex().flex_col().overflow_hidden();

            for message_ix in visible_indices.iter().copied() {
                let Some(msg) = messages_snapshot.get(message_ix) else {
                    continue;
                };

                let is_collapsed = Self::is_collapsed_for(msg, &collapsed_ids);
                let stats = message_stats_snapshot
                    .get(&msg.id)
                    .copied()
                    .unwrap_or_default();
                let use_heavy_preview =
                    Self::should_use_heavy_markdown_preview_for(ui_variant, msg, stats)
                        && !expanded_heavy_markdown_ids.contains(&msg.id);
                let row_fidelity_id = transcript_row_fidelity_id(msg);
                let mut row = div()
                    .debug_selector(move || row_fidelity_id)
                    .w_full()
                    .px(px(style_def.transcript.focused_preview_padding_x))
                    .pb(px(style_def.transcript.focused_preview_padding_bottom));

                if use_heavy_preview {
                    let preview_text = message_previews_snapshot
                        .get(&msg.id)
                        .map(String::as_str)
                        .unwrap_or("");
                    row = row.child(Self::render_heavy_markdown_preview(
                        msg,
                        preview_text,
                        stats,
                        &colors,
                        &theme,
                        &style_def,
                        &entity,
                    ));
                } else if let Some(text_view_state) = message_views_snapshot.get(&msg.id) {
                    row = row.child(Self::render_message(
                        ui_variant,
                        msg,
                        &colors,
                        is_collapsed,
                        text_view_state,
                        &style_def,
                        &entity,
                        &messages_snapshot,
                        &fork_points_snapshot,
                        thread_status,
                        on_fork_edit_message.clone(),
                        false,
                    ));
                } else {
                    continue;
                }

                preview = preview.child(row);
            }

            preview.into_any_element()
        } else {
            list(self.list_state.clone(), move |ix, _window, _cx| {
                if ix == visible_indices.len() {
                    return match pending_activity_placement {
                        PendingActivityPlacement::TailSentinel => Self::render_activity_row(
                            &theme,
                            &colors,
                            ui_variant.config().transcript,
                            &style_def,
                        ),
                        PendingActivityPlacement::Hidden
                        | PendingActivityPlacement::EmptyAssistantRow(_) => {
                            Self::render_empty_activity_row()
                        }
                    };
                }
                let Some(&message_ix) = visible_indices.get(ix) else {
                    return div().into_any();
                };
                let msg = &messages_snapshot[message_ix];

                let is_collapsed = Self::is_collapsed_for(msg, &collapsed_ids);

                let prev_was_user = message_ix > 0
                    && matches!(
                        messages_snapshot[message_ix - 1].role,
                        AgentChatThreadMessageRole::User
                    );
                let is_response_start =
                    prev_was_user && !matches!(msg.role, AgentChatThreadMessageRole::User);
                let is_new_turn = message_ix > 0
                    && matches!(msg.role, AgentChatThreadMessageRole::User)
                    && !matches!(
                        messages_snapshot[message_ix - 1].role,
                        AgentChatThreadMessageRole::User
                    );

                let stats = message_stats_snapshot
                    .get(&msg.id)
                    .copied()
                    .unwrap_or_default();
                let use_heavy_preview =
                    Self::should_use_heavy_markdown_preview_for(ui_variant, msg, stats)
                        && !expanded_heavy_markdown_ids.contains(&msg.id);

                let row_fidelity_id = transcript_row_fidelity_id(msg);
                let row = div()
                    .debug_selector(move || row_fidelity_id)
                    .w_full()
                    .px(px(style_def.transcript.row_padding_x))
                    .pb(px(style_def.transcript.row_padding_bottom))
                    .when(is_response_start, |d| {
                        d.mt(px(style_def.transcript.response_start_margin_top))
                    })
                    .when(is_new_turn, |d| {
                        d.mt(px(style_def.transcript.turn_margin_top))
                            .pt(px(style_def.transcript.turn_padding_top))
                            .border_t_1()
                            .border_color(rgba(
                                resolved_agent_chat_transcript_colors(&style_def, &theme)
                                    .turn_divider_rgba,
                            ))
                    })
                    .when(
                        matches!(
                            ui_variant.config().transcript,
                            AgentChatTranscriptPresentation::DenseLog
                        ),
                        |d| d.pb(px(style_def.transcript.dense_row_padding_bottom)),
                    );
                if use_heavy_preview {
                    let preview_text = message_previews_snapshot
                        .get(&msg.id)
                        .map(String::as_str)
                        .unwrap_or("");
                    return row
                        .child(Self::render_heavy_markdown_preview(
                            msg,
                            preview_text,
                            stats,
                            &colors,
                            &theme,
                            &style_def,
                            &entity,
                        ))
                        .into_any();
                }

                let Some(text_view_state) = message_views_snapshot.get(&msg.id) else {
                    return div().into_any();
                };

                row.child(Self::render_message(
                    ui_variant,
                    msg,
                    &colors,
                    is_collapsed,
                    text_view_state,
                    &style_def,
                    &entity,
                    &messages_snapshot,
                    &fork_points_snapshot,
                    thread_status,
                    on_fork_edit_message.clone(),
                    matches!(
                        pending_activity_placement,
                        PendingActivityPlacement::EmptyAssistantRow(index)
                            if index == message_ix
                    ),
                ))
                .into_any()
            })
            .with_sizing_behavior(ListSizingBehavior::Auto)
            .size_full()
            .into_any_element()
        };

        if let Some(render_started) = render_started {
            let elapsed = render_started.elapsed();
            tracing::info!(
                target: "script_kit::agent_chat_render",
                event = "agent_chat_transcript_render",
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                message_count,
                row_count,
                visible_index_count,
                markdown_view_count,
                heavy_preview_count,
                expanded_heavy_markdown_count,
                focused_text_preview,
                scroll_top_item,
                "Agent Chat transcript render"
            );
        }

        div()
            .debug_selector(|| AGENT_CHAT_TRANSCRIPT_VIEWPORT_FIDELITY_ID.to_string())
            .relative()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(transcript_content)
            .vertical_scrollbar_with_fidelity_scope(
                &self.list_state,
                "agent-chat-transcript-scrollbar",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(
        role: AgentChatThreadMessageRole,
        body: impl Into<SharedString>,
    ) -> AgentChatThreadMessage {
        AgentChatThreadMessage {
            id: 1,
            role,
            body: body.into(),
            tool_call_id: None,
            tool_meta: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn transcript_fidelity_ids_are_unique_per_rendered_message() {
        let mut assistant = message(AgentChatThreadMessageRole::Assistant, "assistant");
        assistant.id = 17;
        let mut system = message(AgentChatThreadMessageRole::System, "system");
        system.id = 18;
        let mut tool = message(AgentChatThreadMessageRole::Tool, "tool");
        tool.id = 20;

        assert_eq!(
            transcript_row_fidelity_id(&assistant),
            "agent-chat-transcript-row-assistant-17"
        );
        assert_eq!(
            transcript_row_fidelity_id(&system),
            "agent-chat-transcript-row-system-18"
        );
        assert_eq!(
            transcript_row_fidelity_id(&tool),
            "agent-chat-transcript-row-tool-20"
        );
    }

    #[test]
    fn pending_activity_transfers_with_the_latest_user_response() {
        let prior_user = message(AgentChatThreadMessageRole::User, "Earlier");
        let prior_empty_assistant = message(AgentChatThreadMessageRole::Assistant, "");
        let latest_user = message(AgentChatThreadMessageRole::User, "Latest");
        let mut messages = vec![prior_user, prior_empty_assistant, latest_user];

        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::TailSentinel,
            "historical empty assistant rows must not capture current activity"
        );

        messages.push(message(AgentChatThreadMessageRole::Assistant, ""));
        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::EmptyAssistantRow(3)
        );

        messages[3].body = "First token".into();
        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::Hidden,
            "first visible assistant text must replace pending activity"
        );

        assert_eq!(
            pending_activity_placement(&messages, false),
            PendingActivityPlacement::Hidden
        );
    }

    #[test]
    fn first_visible_assistant_text_parses_immediately() {
        let assistant = message(AgentChatThreadMessageRole::Assistant, "First token");
        assert!(AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some(""),
            "First token",
            true
        ));
        assert!(AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            None,
            "First token",
            true
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            None,
            "Historical answer",
            false
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some("First"),
            "First token",
            true
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some(""),
            "   ",
            true
        ));

        let user = message(AgentChatThreadMessageRole::User, "First token");
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &user,
            Some(""),
            "First token",
            true
        ));
    }

    #[test]
    fn heavy_markdown_stats_count_markdown_and_bare_links() {
        let body = [
            "[Calendar](scriptkit://run/add-to-google-calendar)",
            "[Docs](https://example.com/docs) and https://example.com/raw",
            "[empty]() [not a link]",
        ]
        .join("\n");

        let stats = HeavyMarkdownStats::from_text(&body);

        // Calendar: 1 markdown link (its scriptkit:// target is excluded from
        // bare counting). Docs: 1 markdown link + 1 bare URL outside the
        // target. Empty target and bracket-only text count as 0.
        assert_eq!(stats.link_like_spans, 3);
    }

    #[test]
    fn link_dense_user_messages_use_heavy_preview_path() {
        let body = (0..14)
            .map(|ix| format!("[Brain source {ix}](scriptkit://agent-chat/thread-{ix})"))
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::User, body);

        assert!(stats.is_scroll_heavy());
        assert!(AgentChatTranscript::should_use_heavy_markdown_preview(
            &msg, stats
        ));
    }

    #[test]
    fn heavy_markdown_preview_still_skips_tool_rows() {
        let body = (0..20)
            .map(|ix| format!("[Tool source {ix}](https://example.com/{ix})"))
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::Tool, body);

        assert!(stats.is_scroll_heavy());
        assert!(!AgentChatTranscript::should_use_heavy_markdown_preview(
            &msg, stats
        ));
    }

    #[test]
    fn quick_ai_link_policy_is_assistant_only() {
        let assistant = message(AgentChatThreadMessageRole::Assistant, "answer");
        assert_eq!(
            AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::QuickAi, &assistant),
            MarkdownLinkLabelPolicy::CompactLongBareHttp
        );
        assert_eq!(
            AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::Standard, &assistant),
            MarkdownLinkLabelPolicy::Preserve
        );

        for role in [
            AgentChatThreadMessageRole::User,
            AgentChatThreadMessageRole::Thought,
            AgentChatThreadMessageRole::Tool,
            AgentChatThreadMessageRole::System,
            AgentChatThreadMessageRole::Error,
        ] {
            let msg = message(role, "unchanged");
            assert_eq!(
                AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::QuickAi, &msg),
                MarkdownLinkLabelPolicy::Preserve
            );
        }
    }

    #[test]
    fn quick_ai_compact_links_bypass_heavy_plain_preview() {
        let body = (0..14)
            .map(|ix| {
                format!(
                    "https://news.google.com/rss/articles/very-long-redirect-{ix}-{}",
                    "x".repeat(96)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::Assistant, body);

        assert!(stats.is_scroll_heavy());
        assert!(AgentChatTranscript::should_use_heavy_markdown_preview_for(
            AgentChatUiVariant::Standard,
            &msg,
            stats
        ));
        assert!(!AgentChatTranscript::should_use_heavy_markdown_preview_for(
            AgentChatUiVariant::QuickAi,
            &msg,
            stats
        ));
    }
}
