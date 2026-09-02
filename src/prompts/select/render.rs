use super::*;
use crate::components::{FocusablePrompt, FocusablePromptInterceptedKey};
use crate::theme::AppChromeColors;
use crate::ui_foundation::{
    is_key_backspace, is_key_down, is_key_enter, is_key_space, is_key_up, printable_char,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SelectRowState {
    pub is_focused: bool,
    pub is_selected: bool,
    pub is_hovered: bool,
}

pub(super) fn compute_row_state(
    display_idx: usize,
    focused_index: usize,
    choice_idx: usize,
    selected: &HashSet<usize>,
    hovered_index: Option<usize>,
) -> SelectRowState {
    SelectRowState {
        is_focused: display_idx == focused_index,
        is_selected: selected.contains(&choice_idx),
        is_hovered: hovered_index == Some(display_idx),
    }
}

pub(super) fn visual_row_state_for_input_modality(
    row_state: SelectRowState,
    last_input_was_keyboard: bool,
) -> SelectRowState {
    if last_input_was_keyboard {
        SelectRowState {
            is_hovered: false,
            ..row_state
        }
    } else {
        row_state
    }
}

pub(super) fn visual_row_state_for_selection_mode(
    row_state: SelectRowState,
    is_multiple: bool,
) -> SelectRowState {
    if is_multiple {
        row_state
    } else {
        SelectRowState {
            is_selected: false,
            ..row_state
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelectKeyIntent {
    MoveUp,
    MoveDown,
    Submit,
    Backspace,
    Append(char),
    ToggleFocusedSelection,
    ToggleAllFiltered,
    LetGlobalHandle,
    Ignore,
}

pub(super) fn classify_select_key(
    key: &str,
    key_char: Option<&str>,
    has_platform_modifier: bool,
    is_multiple: bool,
) -> SelectKeyIntent {
    if has_platform_modifier {
        if key.eq_ignore_ascii_case("a") && is_multiple {
            return SelectKeyIntent::ToggleAllFiltered;
        }
        if is_key_space(key) && is_multiple {
            return SelectKeyIntent::ToggleFocusedSelection;
        }

        return SelectKeyIntent::LetGlobalHandle;
    }

    if is_key_up(key) {
        SelectKeyIntent::MoveUp
    } else if is_key_down(key) {
        SelectKeyIntent::MoveDown
    } else if is_key_space(key) {
        SelectKeyIntent::Append(' ')
    } else if is_key_enter(key) {
        SelectKeyIntent::Submit
    } else if is_key_backspace(key) {
        SelectKeyIntent::Backspace
    } else if let Some(ch) = printable_char(key_char) {
        if should_append_to_filter(ch) {
            SelectKeyIntent::Append(ch)
        } else {
            SelectKeyIntent::Ignore
        }
    } else {
        SelectKeyIntent::Ignore
    }
}

pub(super) fn extract_choice_icon_hint(description: Option<&str>) -> Option<&str> {
    description.and_then(|raw| {
        raw.split(['•', '|', '\n'])
            .map(str::trim)
            .find_map(|token| {
                let token_lower = token.to_ascii_lowercase();
                if token_lower == "icon"
                    || token_lower.starts_with("icon:")
                    || token_lower.starts_with("icon=")
                    || token_lower.starts_with("icon ")
                {
                    token
                        .split_once(':')
                        .or_else(|| token.split_once('='))
                        .map(|(_, value)| value.trim())
                        .or_else(|| token.split_whitespace().nth(1))
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn icon_kind_from_choice(choice: &Choice) -> IconKind {
    let metadata_icon =
        extract_choice_icon_hint(choice.description.as_deref()).and_then(IconKind::from_icon_hint);
    let name_prefix_icon = choice
        .name
        .split_whitespace()
        .next()
        .and_then(IconKind::from_icon_hint);

    metadata_icon
        .or(name_prefix_icon)
        .unwrap_or_else(|| IconKind::Svg("Code".to_string()))
}

fn leading_content_from_icon_kind(icon_kind: IconKind) -> LeadingContent {
    match icon_kind {
        IconKind::Emoji(emoji) => LeadingContent::Emoji(emoji.into()),
        IconKind::Image(render_image) => LeadingContent::AppIcon(render_image),
        IconKind::Svg(name) => LeadingContent::Icon {
            name: SharedString::from(name),
            color: None,
        },
    }
}

fn render_select_search_header(
    filter_text: &str,
    placeholder: &str,
    multiple: bool,
    selected_count: usize,
    submission_hint: Option<&str>,
    theme: &crate::theme::Theme,
    def: crate::designs::MainMenuThemeDef,
) -> AnyElement {
    let mut input = crate::components::main_view_chrome::PromptSearchInputChrome::controller_owned(
        "input:select-filter",
        filter_text.to_string(),
        placeholder.to_string(),
    );

    if multiple {
        let chrome = AppChromeColors::from_theme(theme);
        input = input.with_trailing(
            div()
                .flex_none()
                .whitespace_nowrap()
                .pr(px(def.search.text_inset_x))
                .text_size(px(def.typography.desc_font_size))
                .line_height(px(def.typography.desc_line_height))
                .font_weight(def.typography.desc_weight)
                .text_color(rgba(chrome.text_hint_rgba))
                .child(
                    submission_hint
                        .map_or_else(|| format!("{} selected", selected_count), str::to_string),
                )
                .into_any_element(),
        );
    }

    crate::components::main_view_chrome::render_prompt_search_input(theme, def, input)
}

impl Focusable for SelectPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chrome = AppChromeColors::from_theme(&self.theme);
        let menu_def = crate::designs::current_main_menu_theme().def();
        let tokens = get_tokens(self.design_variant);
        let spacing = tokens.spacing();

        let text_color = rgb(chrome.text_primary_hex);
        let hint_color = rgba(chrome.text_hint_rgba);

        let placeholder = self
            .placeholder
            .clone()
            .unwrap_or_else(|| "Search...".to_string());

        let header = render_select_search_header(
            &self.filter_text,
            &placeholder,
            self.multiple,
            self.selected.len(),
            self.submission_hint.as_deref(),
            &self.theme,
            menu_def,
        );

        // Choices list
        let filtered_len = self.filtered_choices.len();
        let choices_content: AnyElement = if filtered_len == 0 {
            let empty_message = if self.filter_text.trim().is_empty() {
                "No choices available"
            } else {
                "No choices match your filter"
            };
            div()
                .w_full()
                .py(px(spacing.padding_xl))
                .px(px(spacing.item_padding_x))
                .text_color(hint_color)
                .child(empty_message)
                .into_any_element()
        } else {
            uniform_list(
                "select-choices",
                filtered_len,
                cx.processor(
                    move |this: &mut SelectPrompt,
                          visible_range: std::ops::Range<usize>,
                          window,
                          cx| {
                        let item_colors = UnifiedListItemColors::from_theme(&this.theme);
                        let last_input_was_keyboard = window.last_input_was_keyboard();
                        let mut rows = Vec::with_capacity(visible_range.len());

                        for display_idx in visible_range {
                            if let Some(&choice_idx) = this.filtered_choices.get(display_idx) {
                                if let Some(choice) = this.choices.get(choice_idx) {
                                    if let Some(indexed_choice) = this.choice_index.get(choice_idx)
                                    {
                                        let row_state = compute_row_state(
                                            display_idx,
                                            this.focused_index,
                                            choice_idx,
                                            &this.selected,
                                            this.hovered_index,
                                        );
                                        let visual_row_state = visual_row_state_for_selection_mode(
                                            visual_row_state_for_input_modality(
                                                row_state,
                                                last_input_was_keyboard,
                                            ),
                                            this.multiple,
                                        );
                                        let is_focused = visual_row_state.is_focused;
                                        let is_selected = visual_row_state.is_selected;
                                        let is_hovered = visual_row_state.is_hovered;
                                        let is_disabled = this.disabled_choices.contains(&choice_idx);
                                        let semantic_id =
                                            select_choice_semantic_id(choice, choice_idx);
                                        let leading = if this.multiple {
                                            Some(LeadingContent::Emoji(
                                                choice_selection_indicator(true, is_selected)
                                                    .into(),
                                            ))
                                        } else {
                                            Some(leading_content_from_icon_kind(
                                                icon_kind_from_choice(choice),
                                            ))
                                        };
                                        // Keep row anatomy stable as focus moves; the main-menu
                                        // row language does not add/remove its metadata line on
                                        // selection changes.
                                        let subtitle = indexed_choice
                                            .metadata
                                            .subtitle_text()
                                            .map(TextContent::plain);
                                        let title = highlighted_choice_title(
                                            &choice.name,
                                            &this.filter_text,
                                        );
                                        let trailing = indexed_choice
                                            .metadata
                                            .shortcut
                                            .clone()
                                            .map(TrailingContent::shortcut);
                                        let hover_handler = cx.listener(
                                            move |this: &mut SelectPrompt,
                                                  hovered: &bool,
                                                  _window,
                                                  cx| {
                                                if *hovered {
                                                    if this.hovered_index != Some(display_idx) {
                                                        this.hovered_index = Some(display_idx);
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_index == Some(display_idx) {
                                                    this.hovered_index = None;
                                                    cx.notify();
                                                }
                                            },
                                        );

                                        // UnifiedListItem owns row chrome. Focused or selected
                                        // rows share the neutral selected background; multi-select
                                        // does not imply active focus everywhere.
                                        let row = div()
                                            .id(display_idx)
                                            .w_full()
                                            .cursor_pointer()
                                            .on_hover(hover_handler)
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(
                                                    move |this: &mut SelectPrompt,
                                                          _event,
                                                          _window,
                                                          cx| {
                                                        if this.disabled_choices.contains(&choice_idx) {
                                                            return;
                                                        }
                                                        if this
                                                            .filtered_choices
                                                            .get(display_idx)
                                                            .is_none()
                                                        {
                                                            return;
                                                        }

                                                        this.set_focused_index(display_idx);
                                                        this.hovered_index = Some(display_idx);
                                                        if this.multiple {
                                                            this.toggle_selection(cx);
                                                        } else {
                                                            this.submit(cx);
                                                        }
                                                    },
                                                ),
                                            )
                                            .child(
                                                UnifiedListItem::new(
                                                    gpui::ElementId::Name(semantic_id.into()),
                                                    title,
                                                )
                                                .subtitle_opt(subtitle)
                                                .leading_opt(leading)
                                                .trailing_opt(trailing)
                                                .state(ItemState {
                                                    is_selected: is_focused || is_selected,
                                                    is_hovered,
                                                    is_disabled,
                                                })
                                                .density(Density::Comfortable)
                                                .with_direct_hover(false)
                                                .colors(item_colors),
                                            );

                                        rows.push(row);
                                    }
                                }
                            }
                        }

                        rows
                    },
                ),
            )
            .h_full()
            .w_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };
        let content = div()
            .id(gpui::ElementId::Name("list:select-choices".into()))
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .child(choices_content);

        let hints = crate::components::universal_prompt_hints();
        crate::components::emit_prompt_hint_audit("prompts::select", &hints);

        let footer =
            crate::components::prompt_layout_shell::main_window_footer_slot_for_prompt_surface(
                "select_prompt",
                || crate::components::render_simple_hint_strip(hints, None),
            );

        let container = if let Some(context) = &self.header_context {
            let header = crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
                menu_def, context.render(&self.theme, menu_def), header,
            );
            div().id(gpui::ElementId::Name("window:select".into()))
                .flex().flex_col().size_full().min_h(px(0.0))
                .child(crate::components::main_view_chrome::render_main_view_header_with_context_outset(
                    header, menu_def.header_info_bar.context_edge_outset_x,
                ))
                .child(content)
                .when_some(footer, |root, footer| root.child(footer))
        } else {
            crate::components::render_minimal_list_prompt_shell_with_footer(0.0, None, header, content, footer)
                .id(gpui::ElementId::Name("window:select".into()))
        }.text_color(text_color);

        FocusablePrompt::new(container)
            .key_context("select_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, _event, _window, _cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        this.submit_cancel();
                        true
                    }
                    _ => false,
                },
                |this, event, _window, cx| {
                    let key_str = event.keystroke.key.as_str();
                    match classify_select_key(
                        key_str,
                        event.keystroke.key_char.as_deref(),
                        event.keystroke.modifiers.platform,
                        this.multiple,
                    ) {
                        SelectKeyIntent::MoveUp => this.move_up(cx),
                        SelectKeyIntent::MoveDown => this.move_down(cx),
                        SelectKeyIntent::Submit => {
                            this.submit(cx);
                        }
                        SelectKeyIntent::Backspace => this.handle_backspace(cx),
                        SelectKeyIntent::Append(ch) => this.handle_char(ch, cx),
                        SelectKeyIntent::ToggleFocusedSelection => this.toggle_selection(cx),
                        SelectKeyIntent::ToggleAllFiltered => this.toggle_select_all_filtered(cx),
                        SelectKeyIntent::LetGlobalHandle | SelectKeyIntent::Ignore => {}
                    }
                },
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_select_key, visual_row_state_for_input_modality,
        visual_row_state_for_selection_mode, SelectKeyIntent, SelectRowState,
    };

    /// Row background resolution — test-only helper retained for unit test coverage.
    fn resolve_row_bg_rgba(
        row_state: SelectRowState,
        focused_bg_rgba: u32,
        hovered_bg_rgba: u32,
    ) -> u32 {
        if row_state.is_focused || row_state.is_selected {
            focused_bg_rgba
        } else if row_state.is_hovered {
            hovered_bg_rgba
        } else {
            0x00000000
        }
    }

    #[test]
    fn test_resolve_row_bg_rgba_uses_focused_accent_for_selected_row() {
        let focused_bg_hex = 0x55AAFF3A;
        let hovered_bg_hex = 0x55AAFF26;

        let selected_row = SelectRowState {
            is_focused: false,
            is_selected: true,
            is_hovered: false,
        };

        assert_eq!(
            resolve_row_bg_rgba(selected_row, focused_bg_hex, hovered_bg_hex),
            focused_bg_hex
        );
    }

    #[test]
    fn test_resolve_row_bg_rgba_uses_hover_color_for_unselected_hovered_row() {
        let focused_bg_hex = 0x55AAFF3A;
        let hovered_bg_hex = 0x55AAFF26;

        let hovered_row = SelectRowState {
            is_focused: false,
            is_selected: false,
            is_hovered: true,
        };

        assert_eq!(
            resolve_row_bg_rgba(hovered_row, focused_bg_hex, hovered_bg_hex),
            hovered_bg_hex
        );
    }

    #[test]
    fn select_prompt_uses_footer_aware_minimal_list_prompt_shell() {
        let source = include_str!("render.rs");
        assert!(
            source.contains("render_minimal_list_prompt_shell_with_footer("),
            "select prompt should use the footer-aware minimal list prompt shell"
        );
        assert!(
            source.contains("main_window_footer_slot_for_prompt_surface(")
                && source.contains("\"select_prompt\""),
            "select prompt should route footer ownership through the prompt surface slot helper"
        );
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];
        assert!(
            !render_code.contains("active_main_window_footer_surface()"),
            "select prompt render code should not call the global native footer state directly"
        );
    }

    #[test]
    fn select_prompt_search_header_is_prompt_owned_shared_chrome() {
        let source = include_str!("render.rs");
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];
        assert!(
            render_code.contains("fn render_select_search_header("),
            "select prompt search header should live behind the prompt-owned shared-header helper"
        );
        assert!(
            render_code.contains("render_select_search_header("),
            "select prompt render path should call the prompt-owned shared-header helper"
        );
        assert!(
            render_code.contains("PromptSearchInputChrome::controller_owned(")
                && render_code.contains("\"input:select-filter\""),
            "select prompt must hand its automation input semantic id to shared search chrome"
        );
        assert!(
            !render_code.contains("render_search_input(")
                && !render_code.contains("gpui_input_state")
                && !render_code.contains("TextInputState"),
            "select prompt must not borrow launcher or text-field input ownership"
        );
    }

    #[test]
    fn select_prompt_does_not_use_prompt_footer() {
        let source = include_str!("render.rs");
        let needle = ["PromptFooter", "::new("].concat();
        let render_fn_end = source.find("#[cfg(test)]").unwrap_or(source.len());
        let render_code = &source[..render_fn_end];
        assert!(
            !render_code.contains(&needle),
            "select prompt render code should not use PromptFooter"
        );
    }

    #[test]
    fn visual_row_state_suppresses_hover_in_keyboard_modality() {
        let row_state = SelectRowState {
            is_focused: false,
            is_selected: false,
            is_hovered: true,
        };
        let visual = visual_row_state_for_input_modality(row_state, true);
        assert_eq!(
            visual,
            SelectRowState {
                is_focused: false,
                is_selected: false,
                is_hovered: false,
            }
        );
    }

    #[test]
    fn visual_row_state_preserves_focus_and_selection_when_hover_is_suppressed() {
        let row_state = SelectRowState {
            is_focused: true,
            is_selected: true,
            is_hovered: true,
        };
        let visual = visual_row_state_for_input_modality(row_state, true);
        assert_eq!(
            visual,
            SelectRowState {
                is_focused: true,
                is_selected: true,
                is_hovered: false,
            }
        );
    }

    #[test]
    fn visual_row_state_keeps_hover_in_mouse_modality() {
        let row_state = SelectRowState {
            is_focused: false,
            is_selected: false,
            is_hovered: true,
        };
        let visual = visual_row_state_for_input_modality(row_state, false);
        assert_eq!(visual, row_state);
    }

    #[test]
    fn visual_row_state_drops_single_select_stale_selection() {
        let row_state = SelectRowState {
            is_focused: false,
            is_selected: true,
            is_hovered: false,
        };
        assert_eq!(
            visual_row_state_for_selection_mode(row_state, false),
            SelectRowState {
                is_focused: false,
                is_selected: false,
                is_hovered: false,
            }
        );
    }

    #[test]
    fn select_key_classifier_yields_global_platform_shortcuts() {
        assert_eq!(
            classify_select_key("enter", Some("\n"), true, true),
            SelectKeyIntent::LetGlobalHandle
        );
        assert_eq!(
            classify_select_key("k", Some("k"), true, true),
            SelectKeyIntent::LetGlobalHandle
        );
        assert_eq!(
            classify_select_key("a", Some("a"), true, false),
            SelectKeyIntent::LetGlobalHandle
        );
    }

    #[test]
    fn select_key_classifier_keeps_multi_select_shortcuts_local() {
        assert_eq!(
            classify_select_key("a", Some("a"), true, true),
            SelectKeyIntent::ToggleAllFiltered
        );
        assert_eq!(
            classify_select_key("space", Some(" "), true, true),
            SelectKeyIntent::ToggleFocusedSelection
        );
    }

    #[test]
    fn select_key_classifier_keeps_plain_input_local() {
        assert_eq!(
            classify_select_key("enter", Some("\n"), false, true),
            SelectKeyIntent::Submit
        );
        assert_eq!(
            classify_select_key("space", Some(" "), false, true),
            SelectKeyIntent::Append(' ')
        );
        assert_eq!(
            classify_select_key("x", Some("x"), false, true),
            SelectKeyIntent::Append('x')
        );
    }

    #[test]
    fn select_rows_pair_pointer_chrome_with_mouse_activation() {
        const SOURCE: &str = include_str!("render.rs");
        let render_fn_end = SOURCE.find("#[cfg(test)]").unwrap_or(SOURCE.len());
        let render_code = &SOURCE[..render_fn_end];
        assert!(
            render_code.contains(".cursor_pointer()") && render_code.contains(".on_mouse_down("),
            "select rows should only use pointer chrome when they handle mouse activation"
        );
        assert!(
            render_code.contains("this.toggle_selection(cx)")
                && render_code.contains("this.submit(cx)"),
            "select row mouse activation should toggle multi-select rows and submit single-select rows"
        );
    }

    #[test]
    fn select_render_drops_local_row_alpha_constants() {
        const SOURCE: &str = include_str!("render.rs");
        let render_fn_end = SOURCE.find("#[cfg(test)]").unwrap_or(SOURCE.len());
        let render_code = &SOURCE[..render_fn_end];
        assert!(
            !render_code.contains("ROW_FOCUSED_BG_ALPHA"),
            "select render should not define ROW_FOCUSED_BG_ALPHA"
        );
        assert!(
            !render_code.contains("ROW_HOVER_BG_ALPHA"),
            "select render should not define ROW_HOVER_BG_ALPHA"
        );
    }

    #[test]
    fn select_render_disables_unified_direct_hover() {
        const SOURCE: &str = include_str!("render.rs");
        assert!(
            SOURCE.contains(".with_direct_hover(false)"),
            "select render should let modality-adjusted row state own hover paint"
        );
    }

    #[test]
    fn select_prompt_render_uses_shared_semantic_id_helper() {
        const SOURCE: &str = include_str!("render.rs");
        assert!(
            SOURCE.contains("select_choice_semantic_id(choice, choice_idx)"),
            "select rows should use the same semantic ID helper as automation elements"
        );
    }
}
