impl AgentChatView {
    fn focused_text_mini_automation_layout_info(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
        cx: &App,
    ) -> crate::protocol::LayoutInfo {
        use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
        use crate::ui::chrome as chrome_tokens;

        let (window_width, window_height) = target
            .bounds
            .as_ref()
            .map(|bounds| (bounds.width as f32, bounds.height as f32))
            .unwrap_or((
                750.0,
                crate::window_resize::focused_text_mini_input_height(),
            ));
        let footer_visible = target.kind == crate::protocol::AutomationWindowKind::Main
            && self.main_window_footer_visible(cx);
        let footer_height = if footer_visible {
            crate::components::footer_chrome::current_main_menu_footer_height()
        } else {
            0.0
        };
        let compact_inner_height =
            crate::window_resize::focused_text_mini_inner_height(window_height);
        let budget = focused_text_mini_layout_budget(
            compact_inner_height,
            self.scope_visible,
            footer_height,
        );
        let theme = theme::get_cached_theme();
        let text_style = AgentChatComposerTextStyle::current(&theme);
        let input_shell = focused_text_mini_input_shell_geometry(
            window_width,
            FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX,
            budget.input_height,
            &text_style,
        );
        let scope_shell = focused_text_mini_input_shell_geometry(
            window_width,
            FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX + budget.input_height,
            budget.scope_height,
            &text_style,
        );
        let shows_result_area = matches!(
            self.focused_text_mini_phase_for_thread(self.live_thread().read(cx)),
            Some(
                FocusedTextMiniPhase::Streaming
                    | FocusedTextMiniPhase::Result
                    | FocusedTextMiniPhase::Error
            )
        );
        let root_name = "FocusedTextMiniRoot";
        let mut components = vec![
            LayoutComponentInfo::new(root_name, LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, window_width, window_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                )
                .with_visual_token("chrome.focusedTextMini")
                .with_flex_column()
                .with_depth(0)
                .with_explanation(
"Intentional compact outer-slot exception: the 44px row is retained for the focused-text variation-card window contract, while its nested composer is the canonical MainViewInput shell.",
                ),
            LayoutComponentInfo::new("FocusedTextMiniInputRow", LayoutComponentType::Input)
                .with_bounds(
                    FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX,
                    FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX,
                    (window_width - FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX * 2.0).max(0.0),
                    budget.input_height,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                )
                .with_visual_token("chrome.focusedTextMiniInputSlot")
                .with_depth(1)
                .with_parent(root_name)
                .with_explanation(
"Compact focused-text slot retained for variation cards; it centers one canonical-height MainViewInput shell.",
),
LayoutComponentInfo::new("FocusedTextMiniInputShell", LayoutComponentType::Input)
.with_bounds(
input_shell.x,
input_shell.y,
input_shell.width,
input_shell.height,
)
.with_padding(
0.0,
text_style.text_inset_right,
0.0,
text_style.text_inset_left,
)
.with_visual_style(
chrome_tokens::CHROME_LAYER_FUNCTIONAL,
chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
)
.with_visual_token("chrome.mainViewInput")
.with_depth(2)
.with_parent("FocusedTextMiniInputRow")
.with_explanation(
"Canonical MainViewInput shell using active main-menu height and insets inside the compact focused-text slot.",
                ),
        ];

        if budget.scope_height > 0.0 {
            components.push(
                LayoutComponentInfo::new("FocusedTextMiniScopeRow", LayoutComponentType::Input)
                    .with_bounds(0.0, budget.input_height, window_width, budget.scope_height)
                    .with_depth(1)
                    .with_parent(root_name)
                    .with_explanation("Optional compact scope slot below the instruction row."),
            );
            components.push(
LayoutComponentInfo::new("FocusedTextMiniScopeShell", LayoutComponentType::Input)
.with_bounds(
scope_shell.x,
scope_shell.y,
scope_shell.width,
scope_shell.height,
)
.with_padding(
0.0,
text_style.text_inset_right,
0.0,
text_style.text_inset_left,
)
.with_visual_style(
chrome_tokens::CHROME_LAYER_FUNCTIONAL,
chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
)
.with_visual_token("chrome.mainViewInput")
.with_depth(2)
.with_parent("FocusedTextMiniScopeRow")
.with_explanation(
"Scope input reuses the same canonical MainViewInput shell and one-line geometry.",
),
            );
        }
        if shows_result_area && budget.result_height > 0.0 {
            components.push(
                LayoutComponentInfo::new("FocusedTextMiniResult", LayoutComponentType::Container)
                    .with_bounds(0.0, budget.result_y, window_width, budget.result_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_CONTENT,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                    )
                    .with_visual_token("content.focusedTextMiniResult")
                    .with_depth(1)
                    .with_parent(root_name)
                    .with_explanation(
                        "Result or variation area after subtracting the native footer safe area.",
                    ),
            );
        }
        if footer_visible {
            components.push(
                LayoutComponentInfo::new("MainViewFooter", LayoutComponentType::Panel)
                    .with_bounds(
                        0.0,
                        (window_height - budget.footer_height).max(0.0),
                        window_width,
                        budget.footer_height,
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                    )
                    .with_visual_token("chrome.mainViewFooter")
                    .with_visual_exception("floatingFooterOverlay")
                    .with_depth(1)
                    .with_parent(root_name)
                    .with_explanation(
                        "Native main-window footer with an equal GPUI safe-area spacer; it never overlays the compact result.",
                    ),
            );
        }

        LayoutInfo {
            window_width,
            window_height,
            prompt_type: "focusedTextMini".to_string(),
            components,
            fidelity: None,
            handler_form: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn composer_picker_width_for_window(window_width: f32) -> f32 {
        let max_width = (window_width - (Self::AGENT_CHAT_COMPOSER_PICKER_EDGE_GUTTER * 2.0))
            .min(Self::AGENT_CHAT_COMPOSER_PICKER_WIDTH);
        max_width.max(Self::AGENT_CHAT_COMPOSER_PICKER_MIN_WIDTH)
    }

    fn clamp_composer_picker_left(anchor_left: f32, picker_width: f32, window_width: f32) -> f32 {
        let def = crate::designs::current_main_menu_theme().def();
        let min_left = crate::components::main_view_chrome::main_view_input_horizontal_metrics(
            def,
            window_width,
        )
        .shell_x;
        let max_left = (window_width - picker_width - Self::AGENT_CHAT_COMPOSER_PICKER_EDGE_GUTTER)
            .max(min_left);
        anchor_left.clamp(min_left, max_left)
    }

    /// Measured width of `prefix` at the composer's real font and size.
    /// Per-glyph advances from the text system replace the old flat
    /// 8.5px-per-char estimate that drifted on wide or narrow glyph runs.
    fn measure_agent_chat_input_prefix_width(
        prefix: &str,
        cx: &App,
        text_style: &AgentChatComposerTextStyle,
    ) -> f32 {
        if prefix.is_empty() {
            return 0.0;
        }

        let text_system = cx.text_system();
        let font_id = text_system.resolve_font(&text_style.font());
        let font_size = gpui::px(text_style.font_size);
        prefix
            .chars()
            .map(|ch| f32::from(text_system.layout_width(font_id, font_size, ch)))
            .sum()
    }

    /// Returns the maximum text wrapping width for the Agent Chat composer.
    fn composer_wrap_width_for_window(
        window_width: f32,
        text_style: &AgentChatComposerTextStyle,
    ) -> f32 {
        text_style.wrap_width(
            window_width,
            crate::components::conversation_style::CONVERSATION_SEND_SIZE,
        )
    }

    /// Returns the Agent Chat composer cursor position `(x, y)` after rendering `text`,
    /// accounting for explicit newlines and real word-wrap boundaries from the
    /// text system's line wrapper (the previous char-count modulo ignored
    /// word breaks, so anchors drifted on wrapped lines).
    fn measure_agent_chat_input_cursor_position(
        text: &str,
        window_width: f32,
        cx: &App,
        text_style: &AgentChatComposerTextStyle,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, 0.0);
        }
        let wrap_width = Self::composer_wrap_width_for_window(window_width, text_style);
        let mut wrapper = cx
            .text_system()
            .line_wrapper(text_style.font(), gpui::px(text_style.font_size));
        let logical_lines: Vec<&str> = text.split('\n').collect();
        let mut visual_row = 0usize;
        let mut cursor_x = 0.0f32;
        for (ix, logical_line) in logical_lines.iter().enumerate() {
            let boundaries: Vec<usize> = wrapper
                .wrap_line(
                    &[gpui::LineFragment::text(logical_line)],
                    gpui::px(wrap_width),
                )
                .map(|boundary| boundary.ix)
                .collect();
            if ix + 1 == logical_lines.len() {
                visual_row += boundaries.len();
                let tail_start = boundaries.last().copied().unwrap_or(0);
                cursor_x = Self::measure_agent_chat_input_prefix_width(
                    &logical_line[tail_start..],
                    cx,
                    text_style,
                );
            } else {
                visual_row += boundaries.len() + 1;
            }
        }
        (cursor_x, visual_row as f32 * text_style.line_height)
    }

    fn measure_agent_chat_input_visual_line_count(
        text: &str,
        window_width: f32,
        cx: &App,
        text_style: &AgentChatComposerTextStyle,
    ) -> usize {
        let (_, cursor_y) =
            Self::measure_agent_chat_input_cursor_position(text, window_width, cx, text_style);
        (cursor_y / text_style.line_height).round() as usize + 1
    }

    /// Returns `(left, top, width)` for the composer picker, anchored to the
    /// trigger character position in the Agent Chat composer, including wrapping.
    fn composer_picker_anchor_for_session(
        &self,
        session: &AgentChatComposerPickerSession,
        input_text: &str,
        window_width: f32,
        cx: &App,
    ) -> (f32, f32, f32) {
        let picker_width = Self::composer_picker_width_for_window(window_width);
        let trigger_start_byte = Self::char_to_byte_offset(input_text, session.trigger_range.start);
        let prefix = &input_text[..trigger_start_byte];
        let trigger_text = match session.trigger {
            AgentChatComposerPickerTrigger::Slash => "/",
            AgentChatComposerPickerTrigger::Profile => PROFILE_TRIGGER_STR,
        };
        let theme = theme::get_cached_theme();
        let text_style = AgentChatComposerTextStyle::current(&theme);
        let trigger_width =
            Self::measure_agent_chat_input_prefix_width(trigger_text, cx, &text_style);
        let (after_trigger_x, after_trigger_y) = Self::measure_agent_chat_input_cursor_position(
            &format!("{prefix}{trigger_text}"),
            window_width,
            cx,
            &text_style,
        );
        let unclamped_left = text_style.shell_inset_x
            + text_style.text_inset_left
            + (after_trigger_x - trigger_width).max(0.0);
        let left = Self::clamp_composer_picker_left(unclamped_left, picker_width, window_width);
        let top =
            after_trigger_y + text_style.line_height + Self::AGENT_CHAT_COMPOSER_PICKER_OFFSET_Y;
        (left, top, picker_width)
    }

    /// Compute the visible range of items for a selected index.
    pub(super) fn composer_picker_visible_range_for(
        selected_index: usize,
        item_count: usize,
    ) -> std::ops::Range<usize> {
        crate::components::inline_dropdown::inline_dropdown_visible_range(
            selected_index,
            item_count,
            Self::COMPOSER_PICKER_MAX_VISIBLE,
        )
    }

    /// Compute the visible range of items for the selected index.
    fn composer_picker_visible_range_from_start(
        visible_start: usize,
        selected_index: usize,
        item_count: usize,
    ) -> std::ops::Range<usize> {
        crate::components::inline_dropdown::inline_dropdown_visible_range_from_start(
            visible_start,
            selected_index,
            item_count,
            Self::COMPOSER_PICKER_MAX_VISIBLE,
        )
    }

    /// Compute the visible range of items for the selected index.
    fn composer_picker_visible_range(
        session: &AgentChatComposerPickerSession,
    ) -> std::ops::Range<usize> {
        Self::composer_picker_visible_range_from_start(
            session.visible_start,
            session.selected_index,
            session.items.len(),
        )
    }
}
