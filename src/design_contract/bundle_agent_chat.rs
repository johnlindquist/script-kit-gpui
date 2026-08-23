fn append_agent_chat_design_tokens(
    b: &mut BundleBuilder,
    theme: &Theme,
    def: MainMenuThemeDef,
    colors: &crate::theme::ColorScheme,
) {
    // ── Agent Chat (embedded Pi chat surface) ────────────────────────────
    // Production contract only: `conversation_style::production_conversation_style()`.
    // Every theme-color × authored-alpha byte routes through the SAME
    // `conversation_style` resolvers `components/transcript.rs` and `view.rs`
    // paint with, so exporter and renderer literally share bytes.
    use crate::components::conversation_style as agent_chat_contract;
    let chat = agent_chat_contract::production_conversation_style();
    let chat_resolved = agent_chat_contract::resolved_conversation_transcript_colors(&chat, &theme);
    let chat_send_disabled = agent_chat_contract::resolved_conversation_send_state_chrome(
        false,
        false,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_enabled = agent_chat_contract::resolved_conversation_send_state_chrome(
        false,
        true,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_queue = agent_chat_contract::resolved_conversation_send_state_chrome(
        true,
        true,
        colors.accent.selected,
        colors.text.primary,
    );
    let chat_send_streaming = agent_chat_contract::resolved_conversation_send_state_chrome(
        true,
        false,
        colors.accent.selected,
        colors.text.primary,
    );

    // Source geometry (writable leaves; CSS-consumable).
    for (id, var, value, path) in [
        (
            "agentChat.transcript.rowPaddingX",
            "--sk-agent-chat-row-padding-x",
            chat.transcript.row_padding_x,
            "AgentChatTranscriptStyle.row_padding_x",
        ),
        (
            "agentChat.transcript.rowPaddingBottom",
            "--sk-agent-chat-row-padding-bottom",
            chat.transcript.row_padding_bottom,
            "AgentChatTranscriptStyle.row_padding_bottom",
        ),
        (
            "agentChat.transcript.responseStartMarginTop",
            "--sk-agent-chat-response-start-margin-top",
            chat.transcript.response_start_margin_top,
            "AgentChatTranscriptStyle.response_start_margin_top",
        ),
        (
            "agentChat.transcript.turnMarginTop",
            "--sk-agent-chat-turn-margin-top",
            chat.transcript.turn_margin_top,
            "AgentChatTranscriptStyle.turn_margin_top",
        ),
        (
            "agentChat.transcript.turnPaddingTop",
            "--sk-agent-chat-turn-padding-top",
            chat.transcript.turn_padding_top,
            "AgentChatTranscriptStyle.turn_padding_top",
        ),
        (
            "agentChat.markdown.bodyFontSize",
            "--sk-agent-chat-md-body-font-size",
            chat.markdown.body_font_size,
            "AgentChatMarkdownStyle.body_font_size",
        ),
        (
            "agentChat.markdown.h1FontSize",
            "--sk-agent-chat-md-h1-font-size",
            chat.markdown.heading_1_font_size,
            "AgentChatMarkdownStyle.heading_1_font_size",
        ),
        (
            "agentChat.markdown.h2FontSize",
            "--sk-agent-chat-md-h2-font-size",
            chat.markdown.heading_2_font_size,
            "AgentChatMarkdownStyle.heading_2_font_size",
        ),
        (
            "agentChat.markdown.h3FontSize",
            "--sk-agent-chat-md-h3-font-size",
            chat.markdown.heading_3_font_size,
            "AgentChatMarkdownStyle.heading_3_font_size",
        ),
        (
            "agentChat.markdown.codeFontSize",
            "--sk-agent-chat-md-code-font-size",
            chat.markdown.code_block_font_size,
            "AgentChatMarkdownStyle.code_block_font_size",
        ),
        (
            "agentChat.markdown.codePaddingX",
            "--sk-agent-chat-md-code-padding-x",
            chat.markdown.code_block_padding_x,
            "AgentChatMarkdownStyle.code_block_padding_x",
        ),
        (
            "agentChat.markdown.codePaddingY",
            "--sk-agent-chat-md-code-padding-y",
            chat.markdown.code_block_padding_y,
            "AgentChatMarkdownStyle.code_block_padding_y",
        ),
        (
            "agentChat.markdown.codeRadius",
            "--sk-agent-chat-md-code-radius",
            chat.markdown.code_block_radius,
            "AgentChatMarkdownStyle.code_block_radius",
        ),
        (
            "agentChat.markdown.blockquotePaddingX",
            "--sk-agent-chat-md-blockquote-padding-x",
            chat.markdown.blockquote_padding_x,
            "AgentChatMarkdownStyle.blockquote_padding_x",
        ),
        (
            "agentChat.markdown.blockquotePaddingY",
            "--sk-agent-chat-md-blockquote-padding-y",
            chat.markdown.blockquote_padding_y,
            "AgentChatMarkdownStyle.blockquote_padding_y",
        ),
        (
            "agentChat.markdown.blockquoteRadius",
            "--sk-agent-chat-md-blockquote-radius",
            chat.markdown.blockquote_radius,
            "AgentChatMarkdownStyle.blockquote_radius",
        ),
        (
            "agentChat.user.paddingX",
            "--sk-agent-chat-user-padding-x",
            chat.user_message.padding_x,
            "AgentChatMessageStyle(user).padding_x",
        ),
        (
            "agentChat.user.paddingY",
            "--sk-agent-chat-user-padding-y",
            chat.user_message.padding_y,
            "AgentChatMessageStyle(user).padding_y",
        ),
        (
            "agentChat.user.radius",
            "--sk-agent-chat-user-radius",
            chat.user_message.radius,
            "AgentChatMessageStyle(user).radius",
        ),
        (
            "agentChat.assistant.paddingX",
            "--sk-agent-chat-assistant-padding-x",
            chat.assistant_message.padding_x,
            "AgentChatMessageStyle(assistant).padding_x",
        ),
        (
            "agentChat.assistant.paddingY",
            "--sk-agent-chat-assistant-padding-y",
            chat.assistant_message.padding_y,
            "AgentChatMessageStyle(assistant).padding_y",
        ),
        (
            "agentChat.block.paddingX",
            "--sk-agent-chat-block-padding-x",
            chat.collapsible.padding_x,
            "AgentChatCollapsibleStyle.padding_x",
        ),
        (
            "agentChat.block.paddingY",
            "--sk-agent-chat-block-padding-y",
            chat.collapsible.padding_y,
            "AgentChatCollapsibleStyle.padding_y",
        ),
        (
            "agentChat.block.bodyPaddingTop",
            "--sk-agent-chat-block-body-padding-top",
            chat.collapsible.body_padding_top,
            "AgentChatCollapsibleStyle.body_padding_top",
        ),
        (
            "agentChat.block.maxBodyHeight",
            "--sk-agent-chat-block-max-body-height",
            chat.collapsible.max_body_height,
            "AgentChatCollapsibleStyle.max_body_height",
        ),
        (
            "agentChat.block.borderWidth",
            "--sk-agent-chat-block-border-width",
            agent_chat_contract::CONVERSATION_BLOCK_BORDER_WIDTH,
            "conversation_style::CONVERSATION_BLOCK_BORDER_WIDTH",
        ),
        (
            "agentChat.block.headerGap",
            "--sk-agent-chat-block-header-gap",
            agent_chat_contract::CONVERSATION_BLOCK_HEADER_GAP,
            "conversation_style::CONVERSATION_BLOCK_HEADER_GAP",
        ),
        (
            "agentChat.system.paddingX",
            "--sk-agent-chat-system-padding-x",
            chat.system.padding_x,
            "AgentChatSystemStyle.padding_x",
        ),
        (
            "agentChat.system.paddingY",
            "--sk-agent-chat-system-padding-y",
            chat.system.padding_y,
            "AgentChatSystemStyle.padding_y",
        ),
        (
            "agentChat.error.paddingX",
            "--sk-agent-chat-error-padding-x",
            chat.error.padding_x,
            "AgentChatErrorStyle.padding_x",
        ),
        (
            "agentChat.error.paddingY",
            "--sk-agent-chat-error-padding-y",
            chat.error.padding_y,
            "AgentChatErrorStyle.padding_y",
        ),
        (
            "agentChat.error.radius",
            "--sk-agent-chat-error-radius",
            chat.error.radius,
            "AgentChatErrorStyle.radius",
        ),
        (
            "agentChat.send.size",
            "--sk-agent-chat-send-size",
            agent_chat_contract::CONVERSATION_SEND_SIZE,
            "conversation_style::CONVERSATION_SEND_SIZE",
        ),
        (
            "agentChat.send.radius",
            "--sk-agent-chat-send-radius",
            agent_chat_contract::CONVERSATION_SEND_RADIUS,
            "conversation_style::CONVERSATION_SEND_RADIUS",
        ),
    ] {
        b.source_len(id, var, value, path);
    }

    // Embedded Agent Chat aliases the canonical main-menu search typography;
    // these records are resolved/non-writable so there is still one owner.
    b.add(
        "agentChat.composer.fontFamily",
        TokenStage::Resolved,
        None,
        TokenValue::Text {
            value: crate::list_item::FONT_SYSTEM_UI.to_string(),
        },
        None,
        false,
        &["mainMenu.type.uiFontFamily"],
    );
    b.add(
        "agentChat.composer.fontSize",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-font-size"),
        TokenValue::Length {
            value: def.search.font_size as f64,
        },
        None,
        false,
        &["mainMenu.search.fontSize"],
    );
    b.add(
        "agentChat.composer.fontWeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-font-weight"),
        TokenValue::FontWeight {
            value: def.search.font_weight.0 as f64,
        },
        None,
        false,
        &["mainMenu.search.fontWeight"],
    );
    b.add(
        "agentChat.composer.lineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-line-height"),
        TokenValue::Length {
            value: def.search.height as f64,
        },
        None,
        false,
        &["mainMenu.search.height"],
    );

    // Source opacities (writable Numbers; CSS-consumable). Thought and tool
    // header opacities stay SEPARATE tokens even while both equal 0.75.
    for (id, var, value, path) in [
        (
            "agentChat.block.thoughtHeaderOpacity",
            "--sk-agent-chat-thought-header-opacity",
            chat.collapsible.thought_header_opacity,
            "AgentChatCollapsibleStyle.thought_header_opacity",
        ),
        (
            "agentChat.block.toolHeaderOpacity",
            "--sk-agent-chat-tool-header-opacity",
            chat.collapsible.tool_header_opacity,
            "AgentChatCollapsibleStyle.tool_header_opacity",
        ),
        (
            "agentChat.block.statusOpacity",
            "--sk-agent-chat-block-status-opacity",
            chat.collapsible.status_opacity,
            "AgentChatCollapsibleStyle.status_opacity",
        ),
        (
            "agentChat.diff.contextOpacity",
            "--sk-agent-chat-diff-context-opacity",
            agent_chat_contract::CONVERSATION_DIFF_CONTEXT_OPACITY,
            "conversation_style::CONVERSATION_DIFF_CONTEXT_OPACITY",
        ),
        (
            "agentChat.system.opacity",
            "--sk-agent-chat-system-opacity",
            chat.system.opacity,
            "AgentChatSystemStyle.opacity",
        ),
        (
            "agentChat.error.labelOpacity",
            "--sk-agent-chat-error-label-opacity",
            chat.error.label_opacity,
            "AgentChatErrorStyle.label_opacity",
        ),
        (
            "agentChat.error.hintOpacity",
            "--sk-agent-chat-error-hint-opacity",
            chat.error.hint_opacity,
            "AgentChatErrorStyle.hint_opacity",
        ),
        (
            "agentChat.send.disabledOpacity",
            "--sk-agent-chat-send-disabled-opacity",
            agent_chat_contract::CONVERSATION_SEND_DISABLED_OPACITY,
            "conversation_style::CONVERSATION_SEND_DISABLED_OPACITY",
        ),
        (
            "agentChat.send.enabledOpacity",
            "--sk-agent-chat-send-enabled-opacity",
            agent_chat_contract::CONVERSATION_SEND_ENABLED_OPACITY,
            "conversation_style::CONVERSATION_SEND_ENABLED_OPACITY",
        ),
        (
            "agentChat.send.queueOpacity",
            "--sk-agent-chat-send-queue-opacity",
            agent_chat_contract::CONVERSATION_SEND_QUEUE_OPACITY,
            "conversation_style::CONVERSATION_SEND_QUEUE_OPACITY",
        ),
        (
            "agentChat.send.streamingOpacity",
            "--sk-agent-chat-send-streaming-opacity",
            agent_chat_contract::CONVERSATION_SEND_STREAMING_OPACITY,
            "conversation_style::CONVERSATION_SEND_STREAMING_OPACITY",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            Some(var),
            TokenValue::Number {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }

    // Authored alpha leaves (writable source records; JSON-only — the HTML
    // consumes the resolved final colors, but the app-authored byte must not
    // disappear inside derived_from).
    for (id, value, path) in [
        (
            "agentChat.transcript.turnDividerAlpha",
            chat.transcript.turn_divider_alpha,
            "AgentChatTranscriptStyle.turn_divider_alpha (0x18)",
        ),
        (
            "agentChat.markdown.codeBgAlpha",
            chat.markdown.code_block_bg_alpha,
            "AgentChatMarkdownStyle.code_block_bg_alpha (0xA0)",
        ),
        (
            "agentChat.markdown.codeBorderAlpha",
            chat.markdown.code_block_border_alpha,
            "AgentChatMarkdownStyle.code_block_border_alpha (0x40)",
        ),
        (
            "agentChat.markdown.blockquoteBgAlpha",
            chat.markdown.blockquote_bg_alpha,
            "AgentChatMarkdownStyle.blockquote_bg_alpha (0x10)",
        ),
        (
            "agentChat.markdown.blockquoteBorderAlpha",
            chat.markdown.blockquote_border_alpha,
            "AgentChatMarkdownStyle.blockquote_border_alpha (0x40)",
        ),
        (
            "agentChat.user.bgAlpha",
            chat.user_message.bg_alpha,
            "AgentChatMessageStyle(user).bg_alpha (0x06)",
        ),
        (
            "agentChat.block.thoughtBorderAlpha",
            chat.collapsible.thought_border_alpha,
            "AgentChatCollapsibleStyle.thought_border_alpha (0x7F)",
        ),
        (
            "agentChat.block.toolBorderAlpha",
            chat.collapsible.tool_border_alpha,
            "AgentChatCollapsibleStyle.tool_border_alpha (0x7F)",
        ),
        (
            "agentChat.tool.statusPendingAlpha",
            agent_chat_contract::CONVERSATION_TOOL_STATUS_PENDING_ALPHA,
            "conversation_style::CONVERSATION_TOOL_STATUS_PENDING_ALPHA (0x80)",
        ),
        (
            "agentChat.diff.tintAlpha",
            agent_chat_contract::CONVERSATION_DIFF_TINT_ALPHA,
            "conversation_style::CONVERSATION_DIFF_TINT_ALPHA (0x14)",
        ),
        (
            "agentChat.system.borderAlpha",
            chat.system.border_alpha,
            "AgentChatSystemStyle.border_alpha (0x30)",
        ),
        (
            "agentChat.error.bgAlpha",
            chat.error.bg_alpha,
            "AgentChatErrorStyle.bg_alpha — authored DECIMAL 50 (= 0x32); see agentChat.error.bgAlphaUnits",
        ),
        (
            "agentChat.error.borderAlpha",
            chat.error.border_alpha,
            "AgentChatErrorStyle.border_alpha (0x80)",
        ),
        (
            "agentChat.send.disabledBgAlpha",
            agent_chat_contract::CONVERSATION_SEND_DISABLED_BG_ALPHA,
            "conversation_style::CONVERSATION_SEND_DISABLED_BG_ALPHA (0x06)",
        ),
        (
            "agentChat.send.enabledBgAlpha",
            agent_chat_contract::CONVERSATION_SEND_ENABLED_BG_ALPHA,
            "conversation_style::CONVERSATION_SEND_ENABLED_BG_ALPHA (0x30)",
        ),
        (
            "agentChat.send.queueBgAlpha",
            agent_chat_contract::CONVERSATION_SEND_QUEUE_BG_ALPHA,
            "conversation_style::CONVERSATION_SEND_QUEUE_BG_ALPHA (0x24)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }
    // paragraph_gap is authored in REMS (framework-relative), not px — a
    // Number source record with no CSS variable; the mockup's rem
    // conversion is emulator calibration.
    b.add(
        "agentChat.markdown.paragraphGapRems",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: chat.markdown.paragraph_gap as f64,
        },
        Some("AgentChatMarkdownStyle.paragraph_gap (rems scalar)"),
        true,
        &[],
    );
    // Composer paddings: app-authored, but the shell height derives from
    // the shared search height + line growth, so these are JSON-only (the
    // Y padding feeds picker-lane math, the X padding measurement lanes).
    for (id, value, path) in [
        (
            "agentChat.composer.paddingX",
            agent_chat_contract::CONVERSATION_INPUT_PADDING_X,
            "conversation_style::CONVERSATION_INPUT_PADDING_X (picker clamping/measurement; not shell geometry)",
        ),
        (
            "agentChat.composer.paddingY",
            agent_chat_contract::CONVERSATION_INPUT_PADDING_Y,
            "conversation_style::CONVERSATION_INPUT_PADDING_Y (picker lane positioning; not shell height)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Length {
                value: value as f64,
            },
            Some(path),
            true,
            &[],
        );
    }

    // Resolved paint (never writable) — the SAME resolver bytes the
    // transcript renderer paints.
    b.resolved_color(
        "resolved.agentChat.transcript.turnDivider",
        "--sk-agent-chat-turn-divider",
        chat_resolved.turn_divider_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.transcript.turnDividerAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.codeBg",
        "--sk-agent-chat-md-code-bg",
        chat_resolved.code_bg_rgba,
        &[
            "theme.colors.background.searchBox",
            "agentChat.markdown.codeBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.codeBorder",
        "--sk-agent-chat-md-code-border",
        chat_resolved.code_border_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.codeBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.blockquoteBg",
        "--sk-agent-chat-md-blockquote-bg",
        chat_resolved.blockquote_bg_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.blockquoteBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.markdown.blockquoteBorder",
        "--sk-agent-chat-md-blockquote-border",
        chat_resolved.blockquote_border_rgba,
        &[
            "theme.colors.ui.border",
            "agentChat.markdown.blockquoteBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.user.bg",
        "--sk-agent-chat-user-bg",
        chat_resolved.user_bg_rgba,
        &["theme.colors.text.primary", "agentChat.user.bgAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.thought.border",
        "--sk-agent-chat-thought-border",
        chat_resolved.thought_border_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.block.thoughtBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.border",
        "--sk-agent-chat-tool-border",
        chat_resolved.tool_border_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.block.toolBorderAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.borderError",
        "--sk-agent-chat-tool-border-error",
        chat_resolved.tool_border_error_rgba,
        &["theme.colors.ui.error", "agentChat.block.toolBorderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusPending",
        "--sk-agent-chat-tool-status-pending",
        chat_resolved.tool_status_pending_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.tool.statusPendingAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusComplete",
        "--sk-agent-chat-tool-status-complete",
        chat_resolved.tool_status_complete_rgba,
        &["theme.colors.ui.success"],
    );
    b.resolved_color(
        "resolved.agentChat.tool.statusFailed",
        "--sk-agent-chat-tool-status-failed",
        chat_resolved.tool_status_failed_rgba,
        &["theme.colors.ui.error"],
    );
    b.resolved_color(
        "resolved.agentChat.diff.addedBg",
        "--sk-agent-chat-diff-added-bg",
        chat_resolved.diff_added_bg_rgba,
        &["theme.colors.ui.success", "agentChat.diff.tintAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.diff.removedBg",
        "--sk-agent-chat-diff-removed-bg",
        chat_resolved.diff_removed_bg_rgba,
        &["theme.colors.ui.error", "agentChat.diff.tintAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.system.border",
        "--sk-agent-chat-system-border",
        chat_resolved.system_border_rgba,
        &["theme.colors.ui.border", "agentChat.system.borderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.error.bg",
        "--sk-agent-chat-error-bg",
        chat_resolved.error_bg_rgba,
        &["theme.colors.ui.error", "agentChat.error.bgAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.error.border",
        "--sk-agent-chat-error-border",
        chat_resolved.error_border_rgba,
        &["theme.colors.ui.error", "agentChat.error.borderAlpha"],
    );
    b.resolved_color(
        "resolved.agentChat.send.disabledBg",
        "--sk-agent-chat-send-disabled-bg",
        chat_send_disabled.bg_rgba,
        &[
            "theme.colors.text.primary",
            "agentChat.send.disabledBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.send.enabledBg",
        "--sk-agent-chat-send-enabled-bg",
        chat_send_enabled.bg_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.send.enabledBgAlpha",
        ],
    );
    b.resolved_color(
        "resolved.agentChat.send.queueBg",
        "--sk-agent-chat-send-queue-bg",
        chat_send_queue.bg_rgba,
        &[
            "theme.colors.accent.selected",
            "agentChat.send.queueBgAlpha",
        ],
    );
    debug_assert_eq!(chat_send_streaming.bg_rgba, 0x0000_0000);

    // Markdown body line box: renderer never sets a line height; GPUI's
    // implicit phi() default applies — resolved through the shared app-side
    // helper (confirm_prompt_line_height_px), never a fresh 1.618034
    // literal here.
    b.add(
        "resolved.agentChat.markdown.bodyLineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-md-body-line-height"),
        TokenValue::Length {
            value: agent_chat_contract::resolved_conversation_markdown_body_line_height(&chat)
                as f64,
        },
        None,
        false,
        &[
            "agentChat.markdown.bodyFontSize",
            "gpui TextStyle default phi() line height, rounded",
        ],
    );
    // Single-line composer shell height: the shared main-menu search height
    // grows by one composer line per extra visible line (shared formula
    // owner `main_view_multiline_input_height`). Fixture-resolved — NOT a
    // universal composer height (multiline/expanded composers are taller).
    b.add(
        "resolved.agentChat.composer.singleLineHeight",
        TokenStage::Resolved,
        Some("--sk-agent-chat-composer-single-line-height"),
        TokenValue::Length {
            value: agent_chat_contract::resolved_conversation_composer_single_line_height(
                def.search.height,
            ) as f64,
        },
        None,
        false,
        &["mainMenu.search.height", "agentChat.composer.lineHeight"],
    );
    // Send glyph typography: production uses gpui `text_sm` (a framework
    // authority — NOT the markdown body size, which merely coincides at 14).
    b.add(
        "resolved.framework.textSmFontSize",
        TokenStage::Resolved,
        Some("--sk-framework-text-sm-font-size"),
        TokenValue::Length { value: 14.0 },
        None,
        false,
        &["gpui Styled::text_sm (0.875rem × 16px rem) — send glyph typography"],
    );

    // JSON-only Agent Chat facts (no CSS role, never writable).
    for (id, value, path) in [
        (
            "agentChat.composer.placeholderEmpty",
            agent_chat_contract::CONVERSATION_PLACEHOLDER_ASK.to_string(),
            "conversation_style::CONVERSATION_PLACEHOLDER_ASK",
        ),
        (
            "agentChat.composer.placeholderFollowUp",
            agent_chat_contract::CONVERSATION_PLACEHOLDER_FOLLOW_UP.to_string(),
            "conversation_style::CONVERSATION_PLACEHOLDER_FOLLOW_UP (non-empty transcript state)",
        ),
        (
            "agentChat.legacyComposer.fontFamily",
            agent_chat_contract::CONVERSATION_INPUT_FONT_FAMILY.to_string(),
            "conversation_style::CONVERSATION_INPUT_FONT_FAMILY — detached/experimental Agent Chat and Focused Text Mini only",
        ),
        (
            "agentChat.transcript.alignment",
            "bottomFollowTailWithSyntheticActivityTail".to_string(),
            "AgentChatTranscript::new — ListState::new(len+1, ListAlignment::Bottom).measure_all() + follow_tail(true)",
        ),
        (
            "agentChat.footer.presentation",
            "gpuiSpacerPlusNativeOverlay".to_string(),
            "render_native_main_window_footer_spacer for surface \"agent_chat\" — the GPUI band is EMPTY in captures; button truth needs an activeFooter probe",
        ),
        (
            "agentChat.tool.defaultExpansion",
            "collapsedExceptDiffOrError".to_string(),
            "AgentChatTranscript::default_expanded — tools with a diff or is_error start expanded",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }
    // Variant-limited / ineffective-in-Standard numbers (JSON-only, not
    // writable through the token reverse path; see the
    // agentChat.standard.roleSplitOnlyFields conflict).
    for (id, value, path) in [
        (
            "agentChat.user.maxWidthRoleSplitOnly",
            chat.user_message.max_width as f64,
            "AgentChatMessageStyle(user).max_width — applied ONLY under RoleSplit presentation",
        ),
        (
            "agentChat.assistant.maxWidthRoleSplitOnly",
            chat.assistant_message.max_width as f64,
            "AgentChatMessageStyle(assistant).max_width — applied ONLY under RoleSplit presentation",
        ),
        (
            "agentChat.assistant.radius",
            chat.assistant_message.radius as f64,
            "AgentChatMessageStyle(assistant).radius — 0; assistant bg only paints when bg_alpha > 0",
        ),
        (
            "agentChat.assistant.bgAlpha",
            chat.assistant_message.bg_alpha as f64,
            "AgentChatMessageStyle(assistant).bg_alpha — 0: no assistant surface painted in Standard",
        ),
        (
            "agentChat.activity.dotSize",
            agent_chat_contract::CONVERSATION_ACTIVITY_DOT_SIZE as f64,
            "conversation_style::CONVERSATION_ACTIVITY_DOT_SIZE — hidden (0px row) in the idle fixture",
        ),
        (
            "agentChat.activity.gap",
            agent_chat_contract::CONVERSATION_ACTIVITY_GAP as f64,
            "conversation_style::CONVERSATION_ACTIVITY_GAP",
        ),
        (
            "agentChat.activity.labelAlpha",
            agent_chat_contract::CONVERSATION_ACTIVITY_LABEL_ALPHA as f64,
            "conversation_style::CONVERSATION_ACTIVITY_LABEL_ALPHA (0xB0)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number { value },
            Some(path),
            false,
            &[],
        );
    }

    // ── Agent Chat conflicts (recorded, not collapsed) ──────────────────
    b.conflict(
        "agentChat.error.bgAlphaUnits",
        &[
            (
                "AgentChatErrorStyle.bg_alpha",
                format!("{} (DECIMAL — 0x32)", chat.error.bg_alpha),
            ),
            (
                "sibling alphas",
                "hex-authored bytes (0x18, 0xA0, 0x7F, 0x80, …)".to_string(),
            ),
        ],
        "info",
        "The error background alpha is authored as decimal 50 while every sibling \
         alpha is hex-authored — a real edit foot-gun. Recorded, not normalized; the \
         shared pack_rgb_alpha resolver rounds it to 0x32 either way.",
    );
    b.conflict(
        "agentChat.standard.roleSplitOnlyFields",
        &[
            (
                "declared",
                format!(
                    "user.max_width {} / assistant.max_width {} / assistant.radius {} / assistant.bg_alpha {}",
                    chat.user_message.max_width,
                    chat.assistant_message.max_width,
                    chat.assistant_message.radius,
                    chat.assistant_message.bg_alpha
                ),
            ),
            (
                "Standard presentation",
                "full-width rows; max_width applies only under RoleSplit, assistant bg only \
                 when bg_alpha > 0"
                    .to_string(),
            ),
        ],
        "info",
        "Real source controls that are variant-limited: exported as JSON-only facts with \
         no CSS variable so the Standard mockup cannot consume phantom geometry, and the \
         workbench cannot advertise edits that paint nothing on this screen.",
    );
}
