// Shared by binary-only Today wiring first; Notes will consume the same helpers
// as spine parity moves over. The library target does not currently instantiate
// those hosts, so it would otherwise warn on every helper.
#![allow(dead_code)]

use std::{collections::HashMap, ops::Range};

use crate::ai::message_parts::AiContextPart;
use gpui::{
    div, prelude::*, px, rgba, AnyElement, Context, IntoElement, ParentElement, Styled, Window,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesEditorLocalSpineOverlay {
    Disabled,
    Overlay {
        element_id: &'static str,
        render_path: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotesEditorContextMentionBehavior {
    LocalPicker,
    MainMenuRoundTrip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotesEditorHostSpineContract {
    pub(crate) surface: &'static str,
    pub(crate) local_overlay: NotesEditorLocalSpineOverlay,
    pub(crate) context_mentions: NotesEditorContextMentionBehavior,
    pub(crate) project_cwd_local_list: bool,
}

impl NotesEditorHostSpineContract {
    pub(crate) const fn notes() -> Self {
        Self {
            surface: "notes",
            local_overlay: NotesEditorLocalSpineOverlay::Overlay {
                element_id: "notes-spine-list",
                render_path: "components.notes_editor.spine.render_spine_overlay",
            },
            context_mentions: NotesEditorContextMentionBehavior::LocalPicker,
            project_cwd_local_list: false,
        }
    }

    pub(crate) const fn day_page() -> Self {
        Self {
            surface: "day_page",
            local_overlay: NotesEditorLocalSpineOverlay::Disabled,
            context_mentions: NotesEditorContextMentionBehavior::MainMenuRoundTrip,
            project_cwd_local_list: false,
        }
    }

    pub(crate) const fn local_overlay_id(self) -> Option<&'static str> {
        match self.local_overlay {
            NotesEditorLocalSpineOverlay::Disabled => None,
            NotesEditorLocalSpineOverlay::Overlay { element_id, .. } => Some(element_id),
        }
    }

    pub(crate) const fn local_overlay_render_path(self) -> Option<&'static str> {
        match self.local_overlay {
            NotesEditorLocalSpineOverlay::Disabled => None,
            NotesEditorLocalSpineOverlay::Overlay { render_path, .. } => Some(render_path),
        }
    }
}

pub(crate) struct NotesEditorSpineRows {
    pub(crate) grouped: Vec<crate::list_item::GroupedListItem>,
    pub(crate) flat: Vec<crate::spine::SpineListRow>,
    pub(crate) aliases: HashMap<String, (String, AiContextPart)>,
}

impl NotesEditorSpineRows {
    pub(crate) fn new(
        grouped: Vec<crate::list_item::GroupedListItem>,
        flat: Vec<crate::spine::SpineListRow>,
    ) -> Self {
        Self {
            grouped,
            flat,
            aliases: HashMap::new(),
        }
    }
}

pub(crate) struct NotesEditorSpineModel {
    pub(crate) line_range: Range<usize>,
    pub(crate) parse: crate::spine::SpineParse,
    pub(crate) projection: crate::spine::SpineCursorProjection,
    pub(crate) grouped: Vec<crate::list_item::GroupedListItem>,
    pub(crate) flat: Vec<crate::spine::SpineListRow>,
}

#[derive(Debug, Clone)]
pub(crate) struct NotesEditorLineContext {
    pub(crate) line_range: Range<usize>,
    pub(crate) line: String,
    pub(crate) cursor_in_line: usize,
    pub(crate) parse: crate::spine::SpineParse,
    pub(crate) projection: crate::spine::SpineCursorProjection,
}

#[derive(Debug, Clone)]
pub(crate) struct NotesEditorSpineInput {
    pub(crate) key: String,
    pub(crate) line_range: Range<usize>,
    pub(crate) parse: crate::spine::SpineParse,
    pub(crate) projection: crate::spine::SpineCursorProjection,
}

#[derive(Debug, Clone)]
pub(crate) struct NotesEditorContextRoundTripRequest {
    pub(crate) line_range: Range<usize>,
    pub(crate) segment_byte_range: Range<usize>,
    pub(crate) segment_text: String,
}

impl NotesEditorSpineModel {
    pub(crate) fn selected_row(&self, selected_index: usize) -> Option<crate::spine::SpineListRow> {
        let selected_index = crate::list_item::coerce_selection(&self.grouped, selected_index)?;
        let crate::list_item::GroupedListItem::Item(flat_idx) = self.grouped.get(selected_index)?
        else {
            return None;
        };
        let row = self.flat.get(*flat_idx)?;
        row.is_selectable.then(|| row.clone())
    }
}

#[derive(Debug)]
pub(crate) struct NotesEditorSpineRuntime<Flat> {
    pub(crate) selected_index: usize,
    pub(crate) hovered_index: Option<usize>,
    pub(crate) cache_key: String,
    pub(crate) cwd_revision: u64,
    pub(crate) cwd_submit_anchor: bool,
    pub(crate) dismissed_cache_key: Option<String>,
    pub(crate) mention_aliases: HashMap<String, AiContextPart>,
    pub(crate) grouped_cache: Vec<crate::list_item::GroupedListItem>,
    pub(crate) flat_cache: Vec<Flat>,
    pub(crate) alias_cache: HashMap<String, (String, AiContextPart)>,
}

impl<Flat> Default for NotesEditorSpineRuntime<Flat> {
    fn default() -> Self {
        Self {
            selected_index: 0,
            hovered_index: None,
            cache_key: String::new(),
            cwd_revision: 0,
            cwd_submit_anchor: false,
            dismissed_cache_key: None,
            mention_aliases: HashMap::new(),
            grouped_cache: Vec::new(),
            flat_cache: Vec::new(),
            alias_cache: HashMap::new(),
        }
    }
}

impl<Flat> NotesEditorSpineRuntime<Flat> {
    pub(crate) fn reset(&mut self, clear_cwd_anchor: bool, clear_mentions: bool) {
        self.selected_index = 0;
        self.hovered_index = None;
        if clear_cwd_anchor {
            self.cwd_submit_anchor = false;
        }
        self.dismissed_cache_key = None;
        if clear_mentions {
            self.mention_aliases.clear();
        }
        self.cache_key.clear();
        self.grouped_cache.clear();
        self.flat_cache.clear();
        self.alias_cache.clear();
    }

    pub(crate) fn dismiss_current_key(&mut self, key: Option<String>) {
        self.dismissed_cache_key = key;
        self.selected_index = 0;
        self.hovered_index = None;
    }

    pub(crate) fn clear_alias_cache(&mut self) {
        self.alias_cache.clear();
    }

    pub(crate) fn register_mention_alias(&mut self, token: String, part: AiContextPart) {
        self.mention_aliases.insert(token, part);
    }

    pub(crate) fn prune_mention_aliases_for_content(&mut self, content: &str) {
        prune_mention_aliases(&mut self.mention_aliases, content);
    }

    pub(crate) fn clear_transient_cache(&mut self) {
        self.dismissed_cache_key = None;
        self.alias_cache.clear();
    }

    pub(crate) fn coerce_selection_for_cached_rows(&mut self) {
        self.selected_index =
            crate::list_item::coerce_selection(&self.grouped_cache, self.selected_index)
                .unwrap_or(0);
    }

    pub(crate) fn replace_cached_rows(
        &mut self,
        key: String,
        grouped: Vec<crate::list_item::GroupedListItem>,
        flat: Vec<Flat>,
        aliases: HashMap<String, (String, AiContextPart)>,
    ) where
        Flat: Clone,
    {
        self.cache_key = key;
        self.grouped_cache = grouped;
        self.flat_cache = flat;
        self.alias_cache = aliases;
        self.coerce_selection_for_cached_rows();
    }
}

pub(crate) fn current_line_range(content: &str, cursor: usize) -> Range<usize> {
    let cursor = clamp_to_char_boundary(content, cursor.min(content.len()));
    let start = content[..cursor].rfind('\n').map_or(0, |idx| idx + 1);
    let end = content[cursor..]
        .find('\n')
        .map_or(content.len(), |idx| cursor + idx);
    start..end
}

pub(crate) fn notes_editor_line_context(
    content: &str,
    selection: Range<usize>,
) -> Option<NotesEditorLineContext> {
    let cursor = clamp_to_char_boundary(content, selection.end.min(content.len()));
    let line_range = current_line_range(content, cursor);
    let line = content.get(line_range.clone())?.to_string();
    let cursor_in_line = cursor.saturating_sub(line_range.start);
    let parse = crate::spine::parse_spine(&line);
    let projection = crate::spine::project_cursor(&parse, cursor_in_line);
    Some(NotesEditorLineContext {
        line_range,
        line,
        cursor_in_line,
        parse,
        projection,
    })
}

pub(crate) fn local_spine_input_for_contract(
    contract: NotesEditorHostSpineContract,
    content: &str,
    selection: Range<usize>,
) -> Option<NotesEditorSpineInput> {
    if matches!(
        contract.local_overlay,
        NotesEditorLocalSpineOverlay::Disabled
    ) {
        return None;
    }
    let ctx = notes_editor_line_context(content, selection)?;
    if !spine_projection_owns_editor_list(contract, &ctx.parse, &ctx.projection) {
        return None;
    }
    if matches!(
        ctx.projection.active_segment_kind,
        crate::spine::SpineSegmentKind::ProjectCwd { .. }
    ) && !contract.project_cwd_local_list
    {
        return None;
    }
    let key = format!(
        "{}\u{1f}cursor={}\u{1f}active={:?}",
        ctx.line, ctx.cursor_in_line, ctx.projection.active_segment_kind
    );
    Some(NotesEditorSpineInput {
        key,
        line_range: ctx.line_range,
        parse: ctx.parse,
        projection: ctx.projection,
    })
}

pub(crate) fn context_round_trip_request_for_contract(
    contract: NotesEditorHostSpineContract,
    previous_len: usize,
    content: &str,
    selection: Range<usize>,
) -> Option<NotesEditorContextRoundTripRequest> {
    if contract.context_mentions != NotesEditorContextMentionBehavior::MainMenuRoundTrip {
        return None;
    }
    if content.len() <= previous_len {
        return None;
    }
    let ctx = notes_editor_line_context(content, selection)?;
    if ctx.line.trim().is_empty() {
        return None;
    }
    let segment = ctx
        .parse
        .segments
        .get(ctx.projection.active_segment_index)?;
    if !matches!(
        segment.kind,
        crate::spine::SpineSegmentKind::ContextMention { .. }
    ) {
        return None;
    }
    if ctx.cursor_in_line < segment.byte_range.start || ctx.cursor_in_line > segment.byte_range.end
    {
        return None;
    }
    let segment_text = ctx.line.get(segment.byte_range.clone())?.to_string();
    Some(NotesEditorContextRoundTripRequest {
        line_range: ctx.line_range,
        segment_byte_range: segment.byte_range.clone(),
        segment_text,
    })
}

pub(crate) fn context_reference_for_part(
    token: &str,
    part: Option<&AiContextPart>,
) -> Option<(String, AiContextPart)> {
    let part = part.cloned().or_else(|| {
        crate::ai::context_contract::ContextAttachmentKind::from_mention_line(token)
            .map(|kind| kind.part())
    })?;
    let (label, href) = match &part {
        AiContextPart::FilePath { path, label } | AiContextPart::SkillFile { path, label, .. } => (
            label.clone(),
            format!("file://{}", encode_reference_path(path)),
        ),
        AiContextPart::ResourceUri { uri, label } => (label.clone(), uri.clone()),
        AiContextPart::TextBlock { label, source, .. } => {
            if !source.contains(':') || source.contains(char::is_whitespace) {
                return None;
            }
            (label.clone(), source.clone())
        }
        AiContextPart::FocusedTarget { target, label } => {
            if let Some(path) = target
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("path"))
                .and_then(|value| value.as_str())
            {
                (
                    label.clone(),
                    format!("file://{}", encode_reference_path(path)),
                )
            } else {
                (
                    label.clone(),
                    format!("kit://focused-target/{}", target.semantic_id),
                )
            }
        }
        AiContextPart::AmbientContext { label } => (
            label.clone(),
            format!("kit://context?label={}", encode_reference_component(label)),
        ),
    };
    let label = label.trim().replace('[', "\\[").replace(']', "\\]");
    if label.is_empty() || href.trim().is_empty() {
        return None;
    }
    Some((format!("[{label}]({})", href.replace(')', "%29")), part))
}

fn encode_reference_path(path: &str) -> String {
    path.chars()
        .map(|ch| match ch {
            ' ' => "%20".to_string(),
            ')' => "%29".to_string(),
            '(' => "%28".to_string(),
            '%' => "%25".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}

fn encode_reference_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => ch.to_string(),
            ' ' => "%20".to_string(),
            _ => format!("%{:02X}", ch as u32),
        })
        .collect()
}

pub(crate) fn clamp_to_char_boundary(text: &str, mut pos: usize) -> usize {
    pos = pos.min(text.len());
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

pub(crate) fn replace_segment_content(
    content: &str,
    line_range: Range<usize>,
    segment_byte_range: Range<usize>,
    replacement: &str,
    trailing_space: bool,
) -> Option<(String, usize)> {
    let start = line_range.start.checked_add(segment_byte_range.start)?;
    let end = line_range.start.checked_add(segment_byte_range.end)?;
    if start > end
        || end > content.len()
        || !content.is_char_boundary(start)
        || !content.is_char_boundary(end)
    {
        return None;
    }

    let suffix = &content[end..];
    let add_space = trailing_space
        && !replacement.ends_with(char::is_whitespace)
        && !suffix.starts_with(char::is_whitespace);
    let space = if add_space { " " } else { "" };
    let new_content = format!("{}{}{}{}", &content[..start], replacement, space, suffix);
    let cursor = start + replacement.len() + space.len();
    Some((new_content, cursor))
}

pub(crate) fn spine_prompt_plan_can_submit(
    parse: &crate::spine::SpineParse,
    cwd_anchor: bool,
    mention_aliases: &HashMap<String, AiContextPart>,
) -> bool {
    let plan =
        crate::spine::prompt_plan::build_spine_prompt_plan_with_aliases(parse, mention_aliases);
    plan.should_submit_to_chat()
        || (cwd_anchor
            && matches!(
                plan.blocked_reason,
                Some(
                    crate::spine::prompt_plan::SpinePromptPlanBlockReason::NoPromptBuilderSegments
                )
            )
            && plan.unknown_warnings.is_empty()
            && !plan.normalized_prompt.trim().is_empty())
}

pub(crate) fn spine_projection_owns_editor_list(
    contract: NotesEditorHostSpineContract,
    parse: &crate::spine::SpineParse,
    projection: &crate::spine::SpineCursorProjection,
) -> bool {
    if matches!(
        projection.active_segment_kind,
        crate::spine::SpineSegmentKind::ContextMention { .. }
    ) {
        return contract.context_mentions == NotesEditorContextMentionBehavior::LocalPicker;
    }

    if matches!(
        projection.active_segment_kind,
        crate::spine::SpineSegmentKind::Capture { .. }
    ) {
        let raw = parse
            .segments
            .get(projection.active_segment_index)
            .map(|segment| segment.raw.as_str())
            .unwrap_or("");
        if !raw.starts_with(';') {
            return false;
        }
    }

    // `-` (flow search) is an Agent Chat composer feature. In a note, a
    // leading dash is a markdown bullet — never a flow trigger, so the
    // editor overlay must not claim it.
    if matches!(
        projection.active_segment_kind,
        crate::spine::SpineSegmentKind::Flow { .. }
    ) {
        return false;
    }

    !matches!(
        projection.active_segment_kind,
        crate::spine::SpineSegmentKind::FreeText
    )
}

pub(crate) fn push_spine_sections_as_grouped(
    sections: Vec<crate::spine::SpineListSection>,
    supports_action: impl Fn(&crate::spine::SpineListAction) -> bool,
) -> NotesEditorSpineRows {
    let mut grouped = Vec::new();
    let mut flat = Vec::new();
    for section in sections {
        grouped.push(crate::list_item::GroupedListItem::SectionHeader(
            section.title.to_string(),
            section.icon.as_ref().map(|icon| icon.as_ref().to_string()),
        ));
        for row in section.rows {
            if row.is_selectable && !supports_action(&row.action) {
                continue;
            }
            if !row.is_selectable {
                let mut label = row.title.to_string();
                if let Some(subtitle) = row.subtitle.as_ref() {
                    if !subtitle.is_empty() {
                        label.push_str(" \u{b7} ");
                        label.push_str(subtitle.as_ref());
                    }
                }
                grouped.push(crate::list_item::GroupedListItem::SectionHeader(
                    label,
                    row.icon.as_ref().map(|icon| icon.as_ref().to_string()),
                ));
                continue;
            }
            let flat_idx = flat.len();
            flat.push(row);
            grouped.push(crate::list_item::GroupedListItem::Item(flat_idx));
        }
    }
    NotesEditorSpineRows::new(grouped, flat)
}

pub(crate) fn notes_editor_supports_insert_resolve_action(
    action: &crate::spine::SpineListAction,
) -> bool {
    matches!(
        action,
        crate::spine::SpineListAction::InsertSegmentText { .. }
            | crate::spine::SpineListAction::ResolveSegment { .. }
    )
}

pub(crate) fn spine_model_for_runtime(
    runtime: &mut NotesEditorSpineRuntime<crate::spine::SpineListRow>,
    input: NotesEditorSpineInput,
    build_rows: impl FnOnce(
        &crate::spine::SpineParse,
        &crate::spine::SpineCursorProjection,
    ) -> Option<NotesEditorSpineRows>,
) -> Option<NotesEditorSpineModel> {
    if runtime.dismissed_cache_key.as_deref() == Some(input.key.as_str()) {
        return None;
    }

    if runtime.cache_key == input.key {
        runtime.coerce_selection_for_cached_rows();
        return Some(NotesEditorSpineModel {
            line_range: input.line_range,
            parse: input.parse,
            projection: input.projection,
            grouped: runtime.grouped_cache.clone(),
            flat: runtime.flat_cache.clone(),
        });
    }

    let rows = build_rows(&input.parse, &input.projection)?;
    if rows.grouped.is_empty() {
        return None;
    }
    let grouped = rows.grouped;
    let flat = rows.flat;
    runtime.replace_cached_rows(input.key, grouped.clone(), flat.clone(), rows.aliases);

    Some(NotesEditorSpineModel {
        line_range: input.line_range,
        parse: input.parse,
        projection: input.projection,
        grouped,
        flat,
    })
}

pub(crate) fn render_spine_overlay<T, F>(
    contract: NotesEditorHostSpineContract,
    model: &NotesEditorSpineModel,
    selected_index: usize,
    cx: &mut Context<T>,
    on_select_index: F,
) -> Option<AnyElement>
where
    T: 'static,
    F: Fn(&mut T, usize, &mut Window, &mut Context<T>) + Clone + 'static,
{
    let NotesEditorLocalSpineOverlay::Overlay { element_id, .. } = contract.local_overlay else {
        return None;
    };

    let theme = crate::theme::get_cached_theme();
    let item_colors = crate::list_item::ListItemColors::from_theme(&theme);
    let main_menu_theme = crate::designs::current_main_menu_theme();
    let editor_surface =
        crate::components::notes_editor::NotesEditorSurfaceStyle::from_theme(&theme);
    let selected = crate::list_item::coerce_selection(&model.grouped, selected_index);

    let mut rows = div().flex().flex_col().w_full();
    for (ix, grouped_item) in model.grouped.iter().enumerate() {
        match grouped_item {
            crate::list_item::GroupedListItem::SectionHeader(label, icon) => {
                rows = rows.child(
                    div()
                        .h(px(
                            crate::list_item::effective_section_header_height_for_theme(
                                main_menu_theme,
                            ),
                        ))
                        .child(crate::list_item::render_section_header(
                            label,
                            icon.as_deref(),
                            item_colors,
                            ix == 0,
                        )),
                );
            }
            crate::list_item::GroupedListItem::ReservedSectionSlot => {
                rows = rows.child(div().h(px(
                    crate::list_item::effective_first_section_header_height_for_theme(
                        main_menu_theme,
                    ),
                )));
            }
            crate::list_item::GroupedListItem::Status(status) => {
                rows = rows.child(
                    div()
                        .h(px(
                            crate::list_item::effective_source_status_row_height_for_theme(
                                main_menu_theme,
                            ),
                        ))
                        .px_4()
                        .flex()
                        .items_center()
                        .text_sm()
                        .text_color(gpui::rgb(item_colors.text_secondary))
                        .child(status.label.clone()),
                );
            }
            crate::list_item::GroupedListItem::Item(flat_idx) => {
                let Some(row) = model.flat.get(*flat_idx) else {
                    continue;
                };
                let is_selected = selected == Some(ix);
                let on_select_index = on_select_index.clone();
                let click_handler = cx.listener(
                    move |this: &mut T, _event: &gpui::MouseDownEvent, window, cx| {
                        on_select_index(this, ix, window, cx);
                    },
                );
                rows = rows.child(
                    div()
                        .h(px(crate::list_item::effective_list_item_height_for_theme(
                            main_menu_theme,
                        )))
                        .on_mouse_down(gpui::MouseButton::Left, click_handler)
                        .child(
                            crate::list_item::ListItem::new(row.title.to_string(), item_colors)
                                .index(ix)
                                .selected(is_selected)
                                .hovered(false)
                                .main_menu_theme(main_menu_theme)
                                .semantic_id(row.id.to_string())
                                .description_opt(row.subtitle.as_ref().map(|s| s.to_string()))
                                .icon_kind_opt(None)
                                .type_accessory(crate::list_item::TypeAccessory {
                                    label: row.kind.type_accessory_info().0,
                                    icon_name: row.kind.type_accessory_info().1,
                                })
                                .source_hint_opt(row.meta.as_ref().map(|m| m.to_string())),
                        ),
                );
            }
        }
    }

    Some(
        div()
            .id(element_id)
            .absolute()
            .inset_0()
            .bg(rgba(editor_surface.occlusion_rgba))
            .occlude()
            .overflow_y_scroll()
            .child(rows)
            .into_any_element(),
    )
}

fn single_char_deletion_index(previous: &str, next: &str) -> Option<usize> {
    let previous_chars: Vec<char> = previous.chars().collect();
    let next_chars: Vec<char> = next.chars().collect();
    if previous_chars.len() != next_chars.len() + 1 {
        return None;
    }
    let mut index = 0;
    while index < next_chars.len() && previous_chars[index] == next_chars[index] {
        index += 1;
    }
    (previous_chars[index + 1..] == next_chars[index..]).then_some(index)
}

fn byte_index_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == text.chars().count() {
        return text.len();
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len())
}

pub(crate) fn mention_atomic_delete_fixup(
    previous: &str,
    next: &str,
    mention_aliases: &HashMap<String, AiContextPart>,
) -> Option<(String, usize)> {
    if mention_aliases.is_empty() {
        return None;
    }
    let deleted_char_index = single_char_deletion_index(previous, next)?;
    let deleted_inline_token = crate::ai::context_mentions::inline_token_spans(previous)
        .into_iter()
        .any(|span| {
            deleted_char_index >= span.range.start
                && deleted_char_index < span.range.end
                && mention_aliases.contains_key(&span.token)
        });
    if deleted_inline_token {
        let (fixed, cursor_char) =
            crate::ai::context_mentions::remove_inline_mention_at_cursor_with_aliases(
                previous,
                deleted_char_index + 1,
                false,
                mention_aliases,
            )?;
        let cursor = byte_index_for_char_index(&fixed, cursor_char);
        return Some((fixed, cursor));
    }

    let (token_start, token_end) = mention_aliases.keys().find_map(|token| {
        previous.match_indices(token).find_map(|(start, _)| {
            let start_char = previous[..start].chars().count();
            let end = start + token.len();
            let end_char = start_char + token.chars().count();
            (deleted_char_index >= start_char && deleted_char_index < end_char)
                .then_some((start, end))
        })
    })?;
    let mut remove_end = token_end;
    if previous[token_end..].starts_with(' ') {
        remove_end += 1;
    }
    let fixed = format!("{}{}", &previous[..token_start], &previous[remove_end..]);
    Some((fixed, token_start))
}

pub(crate) fn prune_mention_aliases(
    mention_aliases: &mut HashMap<String, AiContextPart>,
    content: &str,
) {
    if mention_aliases.is_empty() {
        return;
    }
    let visible_tokens = crate::ai::context_mentions::inline_token_spans(content)
        .into_iter()
        .map(|span| span.token)
        .collect::<std::collections::HashSet<_>>();
    mention_aliases.retain(|token, _| {
        visible_tokens.contains(token)
            || (token.starts_with('[')
                && token.contains("](")
                && token.ends_with(')')
                && content.contains(token))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_line_parser_ignores_prior_captured_mentions() {
        let content = "captured text\nnew /rewrite";
        let cursor = content.len();
        let range = current_line_range(content, cursor);
        assert_eq!(&content[range], "new /rewrite");
    }

    #[test]
    fn current_line_parser_targets_non_final_active_line() {
        let content = "first /rewrite\nmiddle .professional\nlast ;todo";
        let cursor = content.find("professional").expect("active query exists") + "pro".len();
        let range = current_line_range(content, cursor);
        assert_eq!(&content[range], "middle .professional");
    }

    #[test]
    fn current_line_parser_clamps_unicode_cursor_to_char_boundary() {
        let content = "emoji é /rewrite\nnext";
        let cursor_inside_e_acute = "emoji ".len() + 1;
        let range = current_line_range(content, cursor_inside_e_acute);
        assert_eq!(&content[range], "emoji é /rewrite");
    }

    #[test]
    fn current_line_parser_handles_blank_active_line() {
        let content = "above\n\nbelow /rewrite";
        let cursor = "above\n".len();
        let range = current_line_range(content, cursor);
        assert_eq!(&content[range], "");
    }

    #[test]
    fn notes_contract_allows_local_insert_resolve_overlay_for_capture_segments() {
        let content = "make ;todo";
        let selection = content.len()..content.len();

        let input = local_spine_input_for_contract(
            NotesEditorHostSpineContract::notes(),
            content,
            selection,
        );

        assert!(
            input.is_some(),
            "Notes should keep local insert/resolve spine suggestions"
        );
    }

    #[test]
    fn notes_contract_opens_local_picker_for_context_mentions() {
        let content = "ask @fi";
        let selection = content.len()..content.len();

        let input = local_spine_input_for_contract(
            NotesEditorHostSpineContract::notes(),
            content,
            selection,
        )
        .expect("Notes context mentions must use the local picker");
        let sections = crate::spine::list::build_spine_list_sections_full_with_resolved_tokens(
            &input.parse,
            &input.projection,
            None,
            &|_| false,
        );
        let row_ids = sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .map(|row| row.id.as_ref())
            .collect::<Vec<_>>();

        assert!(row_ids.contains(&"spine:@:subsearch:file"));
        assert_eq!(
            NotesEditorHostSpineContract::notes().context_mentions,
            NotesEditorContextMentionBehavior::LocalPicker
        );
    }

    #[test]
    fn built_in_context_reference_uses_shared_today_token_and_atomic_alias() {
        let (reference, part) =
            context_reference_for_part("@here", None).expect("built-in context token resolves");
        assert_eq!(
            reference,
            "[What I’m Looking At](kit://context?profile=minimal)"
        );
        let mut aliases = HashMap::from([(reference.clone(), part)]);
        prune_mention_aliases(&mut aliases, &format!("ask {reference} now"));
        assert!(
            aliases.contains_key(&reference),
            "shared Markdown-reference aliases survive ordinary adjacent edits",
        );
        let previous = reference.clone();
        let next = reference
            .strip_suffix(')')
            .expect("reference ends with a closing parenthesis")
            .to_string();
        let (deleted, cursor) = mention_atomic_delete_fixup(&previous, &next, &aliases)
            .expect("editing a shared reference deletes it atomically");
        assert_eq!(deleted, "");
        assert_eq!(cursor, 0);
        prune_mention_aliases(&mut aliases, &deleted);
        assert!(aliases.is_empty());
    }

    #[test]
    fn notes_context_subsearch_keeps_loading_state_visible() {
        let content = "ask @file:readme";
        let selection = content.len()..content.len();
        let input = local_spine_input_for_contract(
            NotesEditorHostSpineContract::notes(),
            content,
            selection,
        )
        .expect("Notes context subsearch must stay in the local picker");
        let mut runtime = NotesEditorSpineRuntime::default();
        let model = spine_model_for_runtime(&mut runtime, input, |parse, projection| {
            let sections = crate::spine::list::build_spine_list_sections_full_with_resolved_tokens(
                parse,
                projection,
                None,
                &|_| false,
            );
            Some(push_spine_sections_as_grouped(
                sections,
                notes_editor_supports_insert_resolve_action,
            ))
        })
        .expect("non-selectable loading rows still render");

        assert!(model.flat.is_empty());
        assert!(model.grouped.iter().any(|row| matches!(
            row,
            crate::list_item::GroupedListItem::SectionHeader(label, _)
                if label.contains("Loading") || label.contains("Searching files")
        )));
    }

    #[test]
    fn day_contract_disables_all_local_spine_overlay_inputs() {
        let content = "make ;todo";
        let selection = content.len()..content.len();

        let input = local_spine_input_for_contract(
            NotesEditorHostSpineContract::day_page(),
            content,
            selection,
        );

        assert!(
            input.is_none(),
            "Day must never render a local spine overlay"
        );
    }

    #[test]
    fn day_contract_starts_round_trip_only_for_new_context_mention_edits() {
        let content = "ask @file:readme";
        let selection = content.len()..content.len();

        let request = context_round_trip_request_for_contract(
            NotesEditorHostSpineContract::day_page(),
            content.len() - 1,
            content,
            selection.clone(),
        );
        assert!(
            request.is_some(),
            "Day should round-trip context mentions through the main menu"
        );

        let no_growth = context_round_trip_request_for_contract(
            NotesEditorHostSpineContract::day_page(),
            content.len(),
            content,
            selection,
        );
        assert!(
            no_growth.is_none(),
            "Round trip should not retrigger without editor growth"
        );
    }

    #[test]
    fn notes_contract_never_uses_main_menu_round_trip() {
        let content = "ask @file:readme";
        let selection = content.len()..content.len();

        let request = context_round_trip_request_for_contract(
            NotesEditorHostSpineContract::notes(),
            content.len() - 1,
            content,
            selection,
        );

        assert!(
            request.is_none(),
            "Notes local editor contract must not hijack Day's round-trip"
        );
    }

    #[test]
    fn replace_segment_content_preserves_surrounding_lines() {
        let content = "captured old\nnew /rew\nnext line";
        let line_start = content.find("new ").expect("line exists");
        let line_range = line_start.."captured old\nnew /rew".len();
        let segment_start = "new ".len();
        let segment_end = segment_start + "/rew".len();
        let (new_content, cursor) = replace_segment_content(
            content,
            line_range,
            segment_start..segment_end,
            "/rewrite",
            false,
        )
        .expect("replacement should fit current line");

        assert_eq!(new_content, "captured old\nnew /rewrite\nnext line");
        assert_eq!(cursor, "captured old\nnew /rewrite".len());
    }

    #[test]
    fn replace_segment_content_adds_trailing_space_when_needed() {
        let content = "ask /rew";
        let line_range = 0..content.len();
        let segment_start = "ask ".len();
        let segment_end = segment_start + "/rew".len();
        let (new_content, cursor) = replace_segment_content(
            content,
            line_range,
            segment_start..segment_end,
            "/rewrite",
            true,
        )
        .expect("replacement should fit");

        assert_eq!(new_content, "ask /rewrite ");
        assert_eq!(cursor, "ask /rewrite ".len());
    }

    fn test_text_block_part(label: &str) -> AiContextPart {
        AiContextPart::TextBlock {
            label: label.to_string(),
            source: format!("test:{label}"),
            text: format!("{label} body"),
            mime_type: None,
        }
    }

    #[test]
    fn alias_backed_token_deletes_atomically_and_consumes_space() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );

        let fixed = mention_atomic_delete_fixup(
            "ask @clipboard:Latest now",
            "ask @clipboard:Lates now",
            &aliases,
        )
        .expect("registered token should delete atomically");

        assert_eq!(fixed, ("ask now".to_string(), "ask ".len()));
    }

    #[test]
    fn unresolved_subsearch_token_keeps_normal_character_delete() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );

        assert_eq!(
            mention_atomic_delete_fixup("ask @file:readme now", "ask @file:readm now", &aliases),
            None
        );
    }

    #[test]
    fn prune_aliases_drops_tokens_no_longer_visible() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );
        aliases.insert("@file:demo.rs".to_string(), test_text_block_part("demo.rs"));

        prune_mention_aliases(&mut aliases, "ask @file:demo.rs");

        assert!(!aliases.contains_key("@clipboard:Latest"));
        assert!(aliases.contains_key("@file:demo.rs"));
    }

    #[test]
    fn set_input_prune_boundary_uses_inline_token_spans_not_substrings() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );

        prune_mention_aliases(&mut aliases, "literal @clipboard:Latest-ish");

        assert!(aliases.is_empty());
    }

    #[test]
    fn runtime_reset_clears_transient_state_and_optionally_mentions() {
        let mut runtime = NotesEditorSpineRuntime::<usize>::default();
        runtime.selected_index = 3;
        runtime.hovered_index = Some(2);
        runtime.cache_key = "key".to_string();
        runtime.cwd_submit_anchor = true;
        runtime.dismissed_cache_key = Some("dismissed".to_string());
        runtime.grouped_cache = vec![crate::list_item::GroupedListItem::Item(0)];
        runtime.flat_cache = vec![1];
        runtime.alias_cache.insert(
            "row".to_string(),
            (
                "@clipboard:Latest".to_string(),
                test_text_block_part("Latest"),
            ),
        );
        runtime.register_mention_alias(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );

        runtime.reset(false, false);

        assert_eq!(runtime.selected_index, 0);
        assert_eq!(runtime.hovered_index, None);
        assert_eq!(runtime.cache_key, "");
        assert!(runtime.cwd_submit_anchor);
        assert!(runtime.dismissed_cache_key.is_none());
        assert!(runtime.grouped_cache.is_empty());
        assert!(runtime.flat_cache.is_empty());
        assert!(runtime.alias_cache.is_empty());
        assert!(runtime.mention_aliases.contains_key("@clipboard:Latest"));

        runtime.reset(true, true);

        assert!(!runtime.cwd_submit_anchor);
        assert!(runtime.mention_aliases.is_empty());
    }

    #[test]
    fn runtime_prunes_mentions_using_inline_token_spans() {
        let mut runtime = NotesEditorSpineRuntime::<usize>::default();
        runtime.register_mention_alias(
            "@clipboard:Latest".to_string(),
            test_text_block_part("Latest"),
        );

        runtime.prune_mention_aliases_for_content("literal @clipboard:Latest-ish");

        assert!(runtime.mention_aliases.is_empty());
    }

    #[test]
    fn runtime_dismisses_only_the_current_cache_key() {
        let mut runtime = NotesEditorSpineRuntime::<usize>::default();

        runtime.dismiss_current_key(Some("alpha".to_string()));

        assert_eq!(runtime.dismissed_cache_key.as_deref(), Some("alpha"));
        assert_ne!(runtime.dismissed_cache_key.as_deref(), Some("beta"));
        assert_eq!(runtime.selected_index, 0);
        assert_eq!(runtime.hovered_index, None);
    }

    #[test]
    fn runtime_cache_replacement_coerces_selected_index_to_item() {
        let mut runtime = NotesEditorSpineRuntime::<usize>::default();
        runtime.selected_index = 0;

        runtime.replace_cached_rows(
            "key".to_string(),
            vec![
                crate::list_item::GroupedListItem::SectionHeader("Suggested".to_string(), None),
                crate::list_item::GroupedListItem::Item(0),
            ],
            vec![42],
            HashMap::new(),
        );

        assert_eq!(runtime.cache_key, "key");
        assert_eq!(runtime.selected_index, 1);
        assert_eq!(runtime.flat_cache, vec![42]);
    }

    #[test]
    fn cmd_enter_preflight_rejects_plain_text_without_cwd_anchor() {
        let parse = crate::spine::parse_spine("summarize this folder");
        let aliases = HashMap::new();

        assert!(!spine_prompt_plan_can_submit(&parse, false, &aliases));
    }

    #[test]
    fn cmd_enter_preflight_allows_plain_text_with_cwd_anchor() {
        let parse = crate::spine::parse_spine("summarize this folder");
        let aliases = HashMap::new();

        assert!(spine_prompt_plan_can_submit(&parse, true, &aliases));
    }
}
