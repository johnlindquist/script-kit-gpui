fn append_prompt_design_tokens(
    b: &mut BundleBuilder,
    theme: &Theme,
    def: MainMenuThemeDef,
    opacity: &crate::theme::types::BackgroundOpacity,
) {
    // ── Chat prompt (SDK chat surface, AppView::ChatPrompt) ─────────────
    // Geometry/colors the HTML mockup previously staged under
    // --sk-emulator-staged-chat-*. Rows of constants live next to the
    // renderer (prompts/chat/mod.rs); resolved colors pack the same theme
    // bytes production paints.
    {
        use crate::prompts::chat::{
            CHAT_LAYOUT_BORDER_ALPHA, CHAT_LAYOUT_CARD_PADDING_X, CHAT_LAYOUT_CARD_PADDING_Y,
            CHAT_LAYOUT_PADDING_X, CHAT_LAYOUT_SECTION_PADDING_Y,
        };

        b.source_len(
            "chatPrompt.layout.paddingX",
            "--sk-chat-layout-padding-x",
            CHAT_LAYOUT_PADDING_X,
            "prompts::chat::CHAT_LAYOUT_PADDING_X",
        );
        b.source_len(
            "chatPrompt.layout.sectionPaddingY",
            "--sk-chat-section-padding-y",
            CHAT_LAYOUT_SECTION_PADDING_Y,
            "prompts::chat::CHAT_LAYOUT_SECTION_PADDING_Y",
        );
        b.source_len(
            "chatPrompt.header.gap",
            "--sk-chat-header-gap",
            8.0,
            "prompts::chat::render_input::render_header gap",
        );
        b.source_len(
            "chatPrompt.title.fontSize",
            "--sk-chat-title-font-size",
            14.0,
            "gpui text_sm (0.875rem)",
        );
        b.add(
            "resolved.chatPrompt.title.lineHeight",
            TokenStage::Resolved,
            Some("--sk-chat-title-line-height"),
            TokenValue::Length { value: 23.0 },
            None,
            false,
            &[
                "chatPrompt.title.fontSize",
                "gpui phi() rounded line height",
            ],
        );
        b.add(
            "chatPrompt.title.fontWeight",
            TokenStage::Source,
            Some("--sk-chat-title-font-weight"),
            TokenValue::FontWeight { value: 500.0 },
            Some("FontWeight::MEDIUM"),
            true,
            &[],
        );
        b.resolved_color(
            "resolved.chatPrompt.divider",
            "--sk-chat-divider",
            (theme.colors.ui.border << 8) | CHAT_LAYOUT_BORDER_ALPHA,
            &[
                "theme.colors.ui.border",
                "prompts::chat::CHAT_LAYOUT_BORDER_ALPHA",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.text.secondary",
            "--sk-chat-text-secondary",
            (theme.colors.text.secondary << 8) | 0xFF,
            &["theme.colors.text.secondary"],
        );
        b.source_len(
            "chatPrompt.input.minHeight",
            "--sk-chat-input-min-height",
            28.0,
            "prompts::chat::render_input field min_h",
        );
        b.source_len(
            "chatPrompt.input.fontSize",
            "--sk-chat-input-font-size",
            14.0,
            "prompts::chat::render_input input_font_size (full mode)",
        );
        // search_box fill at search_box opacity — same ladder the confirm input uses.
        let search_box = theme.colors.background.search_box;
        let search_alpha = (opacity.search_box.clamp(0.0, 1.0) * 255.0).round() as u32;
        b.resolved_color(
            "resolved.chatPrompt.input.surface",
            "--sk-chat-input-surface",
            (search_box << 8) | search_alpha,
            &[
                "theme.colors.background.search_box",
                "theme.opacity.search_box",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.input.placeholder",
            "--sk-chat-placeholder-text",
            (theme.colors.text.muted << 8) | 0xFF,
            &["theme.colors.text.muted"],
        );
        b.source_len(
            "chatPrompt.turn.gap",
            "--sk-chat-turn-gap",
            8.0,
            "prompts::chat::render_core turns pb",
        );
        b.source_len(
            "chatPrompt.card.paddingX",
            "--sk-chat-card-padding-x",
            CHAT_LAYOUT_CARD_PADDING_X,
            "prompts::chat::CHAT_LAYOUT_CARD_PADDING_X",
        );
        b.source_len(
            "chatPrompt.card.paddingY",
            "--sk-chat-card-padding-y",
            CHAT_LAYOUT_CARD_PADDING_Y,
            "prompts::chat::CHAT_LAYOUT_CARD_PADDING_Y",
        );
        b.source_len(
            "chatPrompt.card.radius",
            "--sk-chat-card-radius",
            8.0,
            "prompts::chat::render_turns card rounded",
        );
        // Dark-mode turn card: text.primary @ 0x15 (≈8.2%).
        b.resolved_color(
            "resolved.chatPrompt.card.background",
            "--sk-chat-card-background",
            (theme.colors.text.primary << 8) | 0x15,
            &[
                "theme.colors.text.primary",
                "prompts::chat::TURN_CARD_OVERLAY_ALPHA_DARK (0x15)",
            ],
        );
        b.source_len(
            "chatPrompt.card.rowGap",
            "--sk-chat-card-row-gap",
            8.0,
            "prompts::chat::render_turns card gap",
        );
        b.source_len(
            "chatPrompt.card.contentGap",
            "--sk-chat-card-content-gap",
            6.0,
            "prompts::chat::render_turns content gap",
        );
        b.add(
            "chatPrompt.user.fontWeight",
            TokenStage::Source,
            Some("--sk-chat-user-font-weight"),
            TokenValue::FontWeight { value: 600.0 },
            Some("FontWeight::SEMIBOLD"),
            true,
            &[],
        );
        b.source_len(
            "chatPrompt.copy.buttonSize",
            "--sk-chat-copy-button-size",
            24.0,
            "prompts::chat::render_turns copy button w/h",
        );
        b.source_len(
            "chatPrompt.copy.buttonRadius",
            "--sk-chat-copy-button-radius",
            4.0,
            "prompts::chat::render_turns copy rounded",
        );
        b.source_len(
            "chatPrompt.copy.iconSize",
            "--sk-chat-copy-icon-size",
            16.0,
            "prompts::chat::render_turns copy svg size",
        );
        b.add(
            "chatPrompt.copy.opacity",
            TokenStage::Source,
            Some("--sk-chat-copy-opacity"),
            TokenValue::Number { value: 0.7 },
            Some("prompts::chat::render_turns copy opacity"),
            true,
            &[],
        );
        b.source_len(
            "chatPrompt.md.blockGap",
            "--sk-chat-md-block-gap",
            6.0,
            "prompts::markdown::api root gap",
        );
        b.source_len(
            "chatPrompt.md.fontSize",
            "--sk-chat-md-font-size",
            14.0,
            "prompts::markdown text_sm",
        );
        b.add(
            "resolved.chatPrompt.md.lineHeight",
            TokenStage::Resolved,
            Some("--sk-chat-md-line-height"),
            TokenValue::Length { value: 23.0 },
            None,
            false,
            &["chatPrompt.md.fontSize", "gpui phi() rounded line height"],
        );
        b.source_len(
            "chatPrompt.code.marginY",
            "--sk-chat-code-margin-y",
            4.0,
            "prompts::markdown::code_table container mt/mb",
        );
        b.source_len(
            "chatPrompt.code.radius",
            "--sk-chat-code-radius",
            6.0,
            "prompts::markdown::code_table container rounded",
        );
        b.resolved_color(
            "resolved.chatPrompt.code.background",
            "--sk-chat-code-background",
            (search_box << 8) | 0xE0,
            &[
                "theme.colors.background.search_box",
                "markdown code alpha 0xE0",
            ],
        );
        b.resolved_color(
            "resolved.chatPrompt.code.headerBorder",
            "--sk-chat-code-header-border",
            (theme.colors.ui.border << 8) | 0x30,
            &["theme.colors.ui.border", "markdown code header alpha 0x30"],
        );
        b.source_len(
            "chatPrompt.code.headerPaddingX",
            "--sk-chat-code-header-padding-x",
            10.0,
            "prompts::markdown::code_table header px",
        );
        b.source_len(
            "chatPrompt.code.headerPaddingY",
            "--sk-chat-code-header-padding-y",
            4.0,
            "prompts::markdown::code_table header py",
        );
        b.source_len(
            "chatPrompt.code.labelFontSize",
            "--sk-chat-code-label-font-size",
            12.0,
            "gpui text_xs",
        );
        b.source_len(
            "chatPrompt.code.bodyPaddingX",
            "--sk-chat-code-body-padding-x",
            10.0,
            "prompts::markdown::code_table body px",
        );
        b.source_len(
            "chatPrompt.code.bodyPaddingY",
            "--sk-chat-code-body-padding-y",
            8.0,
            "prompts::markdown::code_table body py",
        );
        b.source_len(
            "chatPrompt.code.lineGap",
            "--sk-chat-code-line-gap",
            2.0,
            "prompts::markdown::code_table line gap",
        );
        b.source_len(
            "chatPrompt.code.lineMinHeight",
            "--sk-chat-code-line-min-height",
            16.0,
            "prompts::markdown::code_table line min-h",
        );
        b.source_len(
            "chatPrompt.inlineCode.paddingX",
            "--sk-chat-inline-code-padding-x",
            4.0,
            "prompts::markdown inline code px",
        );
        b.source_len(
            "chatPrompt.inlineCode.paddingY",
            "--sk-chat-inline-code-padding-y",
            1.0,
            "prompts::markdown inline code py",
        );
        b.source_len(
            "chatPrompt.inlineCode.radius",
            "--sk-chat-inline-code-radius",
            3.0,
            "prompts::markdown inline code radius",
        );
        b.resolved_color(
            "resolved.chatPrompt.inlineCode.background",
            "--sk-chat-inline-code-background",
            (search_box << 8) | search_alpha,
            &[
                "theme.colors.background.search_box",
                "theme.opacity.search_box",
            ],
        );
        b.add(
            "resolved.chatPrompt.shell.fixtureHeight",
            TokenStage::Resolved,
            Some("--sk-chat-window-height"),
            TokenValue::Length {
                value: f32::from(crate::window_resize::layout::STANDARD_HEIGHT) as f64,
            },
            None,
            false,
            &["window_resize::layout::STANDARD_HEIGHT (DivPrompt/chat full)"],
        );
        b.source_len(
            "chatPrompt.inputArea.gap",
            "--sk-chat-input-area-gap",
            8.0,
            "prompts::chat::render_input input area gap",
        );
    }

    // ── Terminal prompt (SDK term surface, AppView::TermPrompt) ─────────
    {
        use crate::config::defaults::{DEFAULT_LAYOUT_MAX_HEIGHT, DEFAULT_TERMINAL_FONT_SIZE};
        use crate::term_prompt::{LINE_HEIGHT_MULTIPLIER, MEASURED_CELL_WIDTH_AT_14};
        use crate::window_resize::layout::FOOTER_HEIGHT;

        let term_font = DEFAULT_TERMINAL_FONT_SIZE; // 14
        let cell_height = term_font * LINE_HEIGHT_MULTIPLIER; // 18.2
        let entity_height = DEFAULT_LAYOUT_MAX_HEIGHT - FOOTER_HEIGHT; // 670
        let pad_top = crate::config::defaults::DEFAULT_PADDING_TOP;
        let pad_left = crate::config::defaults::DEFAULT_PADDING_LEFT;
        let pad_right = crate::config::defaults::DEFAULT_PADDING_RIGHT;

        b.source_len(
            "terminalPrompt.shell.height",
            "--sk-terminal-window-height",
            DEFAULT_LAYOUT_MAX_HEIGHT,
            "config::defaults::DEFAULT_LAYOUT_MAX_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.entity.height",
            "--sk-terminal-entity-height",
            entity_height,
            "MAX_HEIGHT − window_resize::layout::FOOTER_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.font.size",
            "--sk-terminal-font-size",
            term_font,
            "config::defaults::DEFAULT_TERMINAL_FONT_SIZE",
        );
        b.source_len(
            "terminalPrompt.cell.width",
            "--sk-terminal-cell-width",
            MEASURED_CELL_WIDTH_AT_14,
            "term_prompt::MEASURED_CELL_WIDTH_AT_14 (conservative_cell_width(8.4287))",
        );
        b.source_len(
            "terminalPrompt.cell.height",
            "--sk-terminal-cell-height",
            cell_height,
            "DEFAULT_TERMINAL_FONT_SIZE × term_prompt::LINE_HEIGHT_MULTIPLIER",
        );
        b.source_len(
            "terminalPrompt.padding.top",
            "--sk-terminal-padding-top",
            pad_top,
            "config::defaults::DEFAULT_PADDING_TOP",
        );
        b.source_len(
            "terminalPrompt.padding.bottom",
            "--sk-terminal-padding-bottom",
            pad_top, // effective_padding_bottom = top for SDK terminals
            "term_prompt::effective_padding bottom = top",
        );
        b.source_len(
            "terminalPrompt.padding.left",
            "--sk-terminal-padding-left",
            pad_left,
            "config::defaults::DEFAULT_PADDING_LEFT",
        );
        b.source_len(
            "terminalPrompt.padding.right",
            "--sk-terminal-padding-right",
            pad_right,
            "config::defaults::DEFAULT_PADDING_RIGHT",
        );
        b.resolved_color(
            "resolved.terminalPrompt.cursor.background",
            "--sk-terminal-cursor-background",
            (theme.colors.accent.selected << 8) | 0xFF,
            &["theme.colors.accent.selected"],
        );
        b.resolved_color(
            "resolved.terminalPrompt.selection.background",
            "--sk-terminal-selection-background",
            (theme.colors.accent.selected_subtle << 8) | 0x0F,
            &[
                "theme.colors.accent.selected_subtle",
                "selection alpha 0x0f",
            ],
        );
        b.resolved_color(
            "resolved.terminalPrompt.ansi.green",
            "--sk-terminal-ansi-green",
            (theme.colors.terminal.green << 8) | 0xFF,
            &["theme.colors.terminal.green"],
        );
        // Hint strip geometry reuses the universal chrome constants.
        b.source_len(
            "terminalPrompt.hint.height",
            "--sk-terminal-hint-height",
            crate::window_resize::main_layout::HINT_STRIP_HEIGHT,
            "window_resize::main_layout::HINT_STRIP_HEIGHT",
        );
        b.source_len(
            "terminalPrompt.hint.paddingX",
            "--sk-terminal-hint-padding-x",
            14.0,
            "HINT_STRIP_PADDING_X",
        );
        b.source_len(
            "terminalPrompt.hint.paddingY",
            "--sk-terminal-hint-padding-y",
            8.0,
            "HINT_STRIP_PADDING_Y",
        );
        b.source_len(
            "terminalPrompt.hint.gap",
            "--sk-terminal-hint-gap",
            8.0,
            "HINT_STRIP_CONTENT_GAP",
        );
        b.source_len(
            "terminalPrompt.hint.textSize",
            "--sk-terminal-hint-text-size",
            12.5,
            "FOOTER_HINT_TEXT_SIZE",
        );
        b.source_len(
            "terminalPrompt.hint.iconSize",
            "--sk-terminal-hint-icon-size",
            14.0,
            "KEY_ICON_SIZE",
        );
        b.source_len(
            "terminalPrompt.hint.iconGap",
            "--sk-terminal-hint-icon-gap",
            3.0,
            "KEY_ICON_LABEL_GAP",
        );
        b.source_len(
            "terminalPrompt.hint.keycapPaddingX",
            "--sk-terminal-hint-keycap-padding-x",
            6.0,
            "KEYCAP_PADDING_X",
        );
        b.source_len(
            "terminalPrompt.hint.keycapPaddingY",
            "--sk-terminal-hint-keycap-padding-y",
            1.0,
            "KEYCAP_PADDING_Y",
        );
        b.source_len(
            "terminalPrompt.hint.keycapRadius",
            "--sk-terminal-hint-keycap-radius",
            5.0,
            "KEYCAP_RADIUS",
        );
        b.resolved_color(
            "resolved.terminalPrompt.hint.keycapBackground",
            "--sk-terminal-hint-keycap-background",
            (theme.colors.text.primary << 8) | 0x1F, // ≈0.12
            &["theme.colors.text.primary", "KEYCAP_BG_OPACITY ~0.12"],
        );
    }

    // ── Arg prompt (script-driven choices/input, AppView::ArgPrompt) ────
    // Rows intentionally emit NO new row tokens — they are the shared
    // ListItem with no metrics override, so --sk-main-menu-row-* /
    // --sk-main-menu-name-* are the row contract. Only shell/input/footer
    // slot values are arg-specific (briefs/arg-prompt-brief.md).
    {
        use crate::panel::{
            CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y, HEADER_DIVIDER_MARGIN, HEADER_PADDING_X,
            HEADER_PADDING_Y,
        };
        use crate::window_resize::{height_for_view, ViewType};

        let arg_input_height = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0); // 22
                                                                           // Painted input line box tracks the gpui-component Input font
                                                                           // (Size::Size(font_size_xl) → line box = 1.25 × font), NOT the
                                                                           // CURSOR_HEIGHT_LG model. Runtime capture 2026-07-12: input band
                                                                           // spans 41px (= 8 + 25 + 8) between context zone and divider.
        let arg_input_line_height =
            crate::theme::TypographyResolver::new(theme, crate::designs::DesignVariant::Default)
                .font_size_xl()
                * 1.25; // 25
        let arg_header_height = (HEADER_PADDING_Y * 2.0) + arg_input_line_height; // 41
        let arg_header_model_height = (HEADER_PADDING_Y * 2.0) + arg_input_height; // 38
                                                                                   // Vendor gpui-component caret: width 2px, height 0.85 × line height.
                                                                                   // For Size::Size(px(20)) font the line box tracks the font size.
        let arg_caret_height = 0.85 * 20.0; // 17
                                            // The launcher opens in MINI mode by default, so a script-driven arg
                                            // prompt sizes through ViewType::MiniPrompt (5-visible-row clamp),
                                            // NOT ArgPromptWithChoices. Runtime window capture 2026-07-12: 279.
        let arg_fixture_height = f32::from(height_for_view(ViewType::MiniPrompt, 6)); // 279
        let arg_full_mode_height = f32::from(height_for_view(ViewType::ArgPromptWithChoices, 6)); // 319
        let arg_footer_spacer = crate::components::footer_chrome::current_main_menu_footer_height(); // 32

        b.source_len(
            "argPrompt.header.paddingX",
            "--sk-arg-header-padding-x",
            HEADER_PADDING_X,
            "crate::panel::HEADER_PADDING_X",
        );
        b.add(
            "resolved.argPrompt.header.height",
            TokenStage::Resolved,
            Some("--sk-arg-header-height"),
            TokenValue::Length {
                value: arg_header_height as f64,
            },
            None,
            false,
            &["argPrompt.input.lineHeight", "panel.HEADER_PADDING_Y"],
        );
        b.add(
            "resolved.argPrompt.input.lineHeight",
            TokenStage::Resolved,
            Some("--sk-arg-input-line-height"),
            TokenValue::Length {
                value: arg_input_line_height as f64,
            },
            None,
            false,
            &[
                "argPrompt.input.fontSize",
                "gpui-component 1.25×font line box",
            ],
        );
        b.add(
            "resolved.argPrompt.input.height",
            TokenStage::Resolved,
            Some("--sk-arg-input-height"),
            TokenValue::Length {
                value: arg_input_height as f64,
            },
            None,
            false,
            &["panel.CURSOR_HEIGHT_LG", "panel.CURSOR_MARGIN_Y"],
        );
        b.add(
            "resolved.argPrompt.input.fontSize",
            TokenStage::Resolved,
            Some("--sk-arg-input-font-size"),
            TokenValue::Length {
                value: crate::theme::TypographyResolver::new(
                    theme,
                    crate::designs::DesignVariant::Default,
                )
                .font_size_xl() as f64,
            },
            None,
            false,
            &["theme.fonts.uiSize"],
        );
        b.source_len(
            "argPrompt.input.caretWidth",
            "--sk-arg-input-caret-width",
            2.0,
            "gpui_component::input::blink_cursor::CURSOR_WIDTH",
        );
        b.add(
            "resolved.argPrompt.input.caretHeight",
            TokenStage::Resolved,
            Some("--sk-arg-input-caret-height"),
            TokenValue::Length {
                value: arg_caret_height,
            },
            None,
            false,
            &[
                "argPrompt.input.fontSize",
                "gpui-component 0.85×line_height",
            ],
        );
        b.source_len(
            "argPrompt.divider.marginX",
            "--sk-arg-divider-margin-x",
            HEADER_DIVIDER_MARGIN,
            "crate::panel::HEADER_DIVIDER_MARGIN",
        );
        b.add(
            "resolved.argPrompt.footer.spacerHeight",
            TokenStage::Resolved,
            Some("--sk-arg-footer-spacer-height"),
            TokenValue::Length {
                value: arg_footer_spacer as f64,
            },
            None,
            false,
            &["footer.railHeight"],
        );
        b.add(
            "resolved.argPrompt.shell.fixtureHeight",
            TokenStage::Resolved,
            Some("--sk-arg-window-height"),
            TokenValue::Length {
                value: arg_fixture_height as f64,
            },
            None,
            false,
            &[
                "window_resize::height_for_view(MiniPrompt, 6) — launcher opens mini",
                "argPrompt.header.height",
            ],
        );

        b.conflict(
            "argPrompt.windowHeight.miniVsFull",
            &[
                (
                    "height_for_view(MiniPrompt, 6) — default mini launcher",
                    format!("{arg_fixture_height}"),
                ),
                (
                    "height_for_view(ArgPromptWithChoices, 6) — full-mode model",
                    format!("{arg_full_mode_height}"),
                ),
            ],
            "info",
            "Script-driven arg prompts open from the MINI launcher and clamp to 5 model \
             rows; the full-mode ArgPromptWithChoices model reserves its own derived height.",
        );
        b.conflict(
            "argPrompt.headerHeight.modelVsPaint",
            &[
                (
                    "CURSOR_HEIGHT_LG input model (py*2 + 22)",
                    format!("{arg_header_model_height}"),
                ),
                (
                    "painted gpui-component line box (py*2 + 25)",
                    format!("{arg_header_height}"),
                ),
            ],
            "info",
            "The input host paints a 1.25×font line box (25px @ 20px font), not the \
             CURSOR_HEIGHT_LG 22px model; the painted header band is 41px.",
        );
        b.conflict(
            "argPrompt.rowHeight.modelVsPaint",
            &[
                (
                    "window_resize::LIST_ITEM_HEIGHT (model)",
                    format!("{}", crate::list_item::LIST_ITEM_HEIGHT),
                ),
                (
                    "InfoBarBase item_height (paint)",
                    format!("{}", def.list.item_height),
                ),
            ],
            "high",
            "Resize model reserves 40px/row but InfoBarBase paints 44px — the 6th \
             choice clips under a full list. Recorded, not smoothed.",
        );
        b.conflict(
            "argPrompt.listPadding.deadModelField",
            &[
                (
                    "ARG_LIST_PADDING_Y (counted in height model)",
                    format!("{}", crate::window_resize::layout::ARG_LIST_PADDING_Y),
                ),
                (
                    "arg renderer list padding",
                    "0 (no list pad painted)".into(),
                ),
            ],
            "info",
            "ARG_LIST_PADDING_Y is counted in height_for_view but the arg list paints no pad.",
        );
        b.conflict(
            "argPrompt.footerHeight.modelVsPaint",
            &[
                (
                    "window_resize::layout::FOOTER_HEIGHT",
                    format!("{}", crate::window_resize::layout::FOOTER_HEIGHT),
                ),
                (
                    "current_main_menu_footer_height (spacer)",
                    format!("{arg_footer_spacer}"),
                ),
            ],
            "info",
            "Model FOOTER_HEIGHT 30 vs painted native spacer 32 vs main-menu host 36.",
        );
    }
}
