//! Production Agent Chat style contract.
//!
//! Single typed owner of the Agent Chat transcript/composer/send paint
//! values — the style struct definitions, the production base values, the
//! composer and send-button constants, and the pure color/geometry
//! resolvers — consumed by BOTH the renderers
//! (`components/transcript.rs`, `view.rs`) and the `design_contract`
//! exporter so the two can never drift (2026-07-11 Oracle review,
//! agent-chat slice).
//!
//! Contract rules:
//! - This module is the single owner of the Agent Chat style values; the
//!   former Dev Style Tool knob catalog was a consumer, never the owner
//!   (the Dev Style Tool has since been removed).
//! - Checked-in export artifacts read `production_agent_chat_style()`
//!   directly; nothing else may reach the exporter.
//! - All theme-color + authored-alpha packing shared by the renderer and
//!   the exporter routes through [`pack_rgb_alpha`] / the resolvers below,
//!   so rounding/cast behavior (0x7F borders, 0x14 diff tints, the
//!   decimal-50 error background, send-state bytes) has exactly one owner.

// ── Style definition ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatStyleDef {
    pub transcript: AgentChatTranscriptStyle,
    pub markdown: AgentChatMarkdownStyle,
    pub user_message: AgentChatMessageStyle,
    pub assistant_message: AgentChatMessageStyle,
    pub collapsible: AgentChatCollapsibleStyle,
    pub error: AgentChatErrorStyle,
    pub system: AgentChatSystemStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatTranscriptStyle {
    pub row_padding_x: f32,
    pub row_padding_bottom: f32,
    pub dense_row_padding_bottom: f32,
    pub response_start_margin_top: f32,
    pub turn_margin_top: f32,
    pub turn_padding_top: f32,
    pub turn_divider_alpha: f32,
    pub focused_preview_padding_x: f32,
    pub focused_preview_padding_bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatMarkdownStyle {
    pub body_font_size: f32,
    pub paragraph_gap: f32,
    pub heading_1_font_size: f32,
    pub heading_2_font_size: f32,
    pub heading_3_font_size: f32,
    pub code_block_font_size: f32,
    pub code_block_padding_x: f32,
    pub code_block_padding_y: f32,
    pub code_block_radius: f32,
    pub code_block_bg_alpha: f32,
    pub code_block_border_alpha: f32,
    pub blockquote_padding_x: f32,
    pub blockquote_padding_y: f32,
    pub blockquote_radius: f32,
    pub blockquote_bg_alpha: f32,
    pub blockquote_border_alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatMessageStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub dense_padding_y: f32,
    pub radius: f32,
    pub bg_alpha: f32,
    /// Applied ONLY under the RoleSplit transcript presentation; Standard
    /// paints full-width rows (variant-limited source fact, not dead).
    pub max_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatCollapsibleStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub body_padding_top: f32,
    pub max_body_height: f32,
    pub thought_header_opacity: f32,
    pub tool_header_opacity: f32,
    pub status_opacity: f32,
    pub thought_border_alpha: f32,
    pub tool_border_alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatErrorStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    /// NOTE: authored as DECIMAL 50 (= 0x32) while sibling alphas are
    /// hex-authored — recorded as the `agentChat.error.bgAlphaUnits`
    /// contract conflict, deliberately NOT normalized here.
    pub bg_alpha: f32,
    pub border_alpha: f32,
    pub label_opacity: f32,
    pub hint_opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentChatSystemStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub opacity: f32,
    pub border_alpha: f32,
}

/// The production Agent Chat base style. Checked-in design artifacts and
/// live rendering both read this function directly.
pub fn production_agent_chat_style() -> AgentChatStyleDef {
    AgentChatStyleDef {
        transcript: AgentChatTranscriptStyle {
            row_padding_x: 16.0,
            row_padding_bottom: 4.0,
            dense_row_padding_bottom: 1.0,
            response_start_margin_top: 4.0,
            turn_margin_top: 8.0,
            turn_padding_top: 8.0,
            turn_divider_alpha: 0x18 as f32,
            focused_preview_padding_x: 8.0,
            focused_preview_padding_bottom: 4.0,
        },
        markdown: AgentChatMarkdownStyle {
            body_font_size: 14.0,
            paragraph_gap: 0.28,
            heading_1_font_size: 17.0,
            heading_2_font_size: 16.0,
            heading_3_font_size: 15.0,
            code_block_font_size: 13.0,
            code_block_padding_x: 7.0,
            code_block_padding_y: 4.0,
            code_block_radius: 5.0,
            code_block_bg_alpha: 0xA0 as f32,
            code_block_border_alpha: 0x40 as f32,
            blockquote_padding_x: 12.0,
            blockquote_padding_y: 6.0,
            blockquote_radius: 5.0,
            blockquote_bg_alpha: 0x10 as f32,
            blockquote_border_alpha: 0x40 as f32,
        },
        user_message: AgentChatMessageStyle {
            padding_x: 12.0,
            padding_y: 8.0,
            dense_padding_y: 3.0,
            radius: 8.0,
            bg_alpha: 0x06 as f32,
            max_width: 520.0,
        },
        assistant_message: AgentChatMessageStyle {
            padding_x: 12.0,
            padding_y: 4.0,
            dense_padding_y: 2.0,
            radius: 0.0,
            bg_alpha: 0.0,
            max_width: 620.0,
        },
        collapsible: AgentChatCollapsibleStyle {
            padding_x: 12.0,
            padding_y: 2.0,
            body_padding_top: 4.0,
            max_body_height: 200.0,
            thought_header_opacity: 0.75,
            tool_header_opacity: 0.75,
            status_opacity: 0.50,
            thought_border_alpha: 0x7f as f32,
            tool_border_alpha: 0x7f as f32,
        },
        error: AgentChatErrorStyle {
            padding_x: 12.0,
            padding_y: 8.0,
            radius: 8.0,
            bg_alpha: 50.0,
            border_alpha: 0x80 as f32,
            label_opacity: 0.75,
            hint_opacity: 0.40,
        },
        system: AgentChatSystemStyle {
            padding_x: 12.0,
            padding_y: 4.0,
            opacity: 0.60,
            border_alpha: 0x30 as f32,
        },
    }
}

// ── Composer constants (hoisted off `impl AgentChatView`) ─────────────────

/// Horizontal padding used by the Agent Chat composer input row (picker
/// clamping / measurement lanes; the shell's text insets come from the
/// shared main-view input shell).
pub(crate) const AGENT_CHAT_INPUT_PADDING_X: f32 = 12.0;
/// Top padding used by the Agent Chat composer input row (picker lane
/// positioning; the shell height derives from the shared search height +
/// line growth, not from this padding).
pub(crate) const AGENT_CHAT_INPUT_PADDING_Y: f32 = 10.0;
/// Legacy visual line height retained for detached and experimental Agent
/// Chat hosts. The standard embedded main-window composer derives its 26px
/// line height from `MainMenuThemeDef::search`.
pub(crate) const AGENT_CHAT_INPUT_LINE_HEIGHT: f32 = 22.0;
/// Legacy composer text size retained for detached and experimental hosts.
/// The standard embedded main-window composer derives its 20px size and 430
/// weight from `MainMenuThemeDef::search`.
pub(crate) const AGENT_CHAT_INPUT_FONT_SIZE: f32 = 17.0;
/// The composer inherits GPUI's default window font. A SEPARATE authority
/// from `list_item::FONT_SYSTEM_UI` even though both name the system font —
/// do not collapse into `--sk-font-ui` until the renderer shares the const.
pub(crate) const AGENT_CHAT_INPUT_FONT_FAMILY: &str = ".SystemUIFont";

/// Composer placeholder while the transcript is empty.
pub(crate) const AGENT_CHAT_PLACEHOLDER_ASK: &str = "Ask anything\u{2026}";
/// Composer placeholder once the transcript has messages (cleared input +
/// non-empty transcript).
pub(crate) const AGENT_CHAT_PLACEHOLDER_FOLLOW_UP: &str = "Follow up\u{2026}";

// ── Send button constants ──────────────────────────────────────────────────

pub(crate) const AGENT_CHAT_SEND_SIZE: f32 = 24.0;
pub(crate) const AGENT_CHAT_SEND_RADIUS: f32 = 6.0;
/// idle + empty input: `text.primary @ 0x06`, opacity 0.30 (`↑`).
pub(crate) const AGENT_CHAT_SEND_DISABLED_BG_ALPHA: f32 = 0x06 as f32;
pub(crate) const AGENT_CHAT_SEND_DISABLED_OPACITY: f32 = 0.30;
/// idle + text: `accent @ 0x30`, opacity 0.90 (`↑`).
pub(crate) const AGENT_CHAT_SEND_ENABLED_BG_ALPHA: f32 = 0x30 as f32;
pub(crate) const AGENT_CHAT_SEND_ENABLED_OPACITY: f32 = 0.90;
/// streaming + text: `accent @ 0x24`, opacity 0.92 (queue `⇧`).
pub(crate) const AGENT_CHAT_SEND_QUEUE_BG_ALPHA: f32 = 0x24 as f32;
pub(crate) const AGENT_CHAT_SEND_QUEUE_OPACITY: f32 = 0.92;
/// streaming + empty: transparent, opacity 0.40 (activity dot `●`).
pub(crate) const AGENT_CHAT_SEND_STREAMING_OPACITY: f32 = 0.40;

// ── Renderer literals hoisted for the contract ─────────────────────────────

/// Collapsible/tool/system/error left border width (was `.border_l_2()`).
pub(crate) const AGENT_CHAT_BLOCK_BORDER_WIDTH: f32 = 2.0;
/// Collapsible/tool header row gap (was `.gap_1()`).
pub(crate) const AGENT_CHAT_BLOCK_HEADER_GAP: f32 = 4.0;
/// Tool status glyph alpha for pending tools (`text.primary @ 0x80`).
pub(crate) const AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA: f32 = 0x80 as f32;
/// Added/removed diff row background tint alpha (`success/error @ 0x14`).
pub(crate) const AGENT_CHAT_DIFF_TINT_ALPHA: f32 = 0x14 as f32;
/// Context (unchanged) diff row opacity.
pub(crate) const AGENT_CHAT_DIFF_CONTEXT_OPACITY: f32 = 0.55;
/// Synthetic activity tail row: pulsing accent dot diameter.
pub(crate) const AGENT_CHAT_ACTIVITY_DOT_SIZE: f32 = 7.0;
/// Activity row dot ↔ label gap.
pub(crate) const AGENT_CHAT_ACTIVITY_GAP: f32 = 8.0;
/// Activity row "Thinking…" label alpha (`text.primary @ 0xB0`).
pub(crate) const AGENT_CHAT_ACTIVITY_LABEL_ALPHA: f32 = 0xB0 as f32;

// ── Shared alpha packing ───────────────────────────────────────────────────

/// Pack a `0xRRGGBB` theme color with an authored f32 alpha byte exactly the
/// way the transcript renderer does (`(rgb << 8) | alpha.round() as u32`).
/// The ONLY rounding/cast owner for agent-chat alpha bytes — the exporter
/// and every render fn share it.
pub(crate) fn pack_rgb_alpha(rgb: u32, alpha: f32) -> u32 {
    (rgb << 8) | alpha.round() as u32
}

// ── Pure resolvers (theme × authored alphas → painted RGBA bytes) ─────────

/// Every alpha-packed transcript color the renderer paints, resolved from
/// the SAME theme authorities the render fns read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedAgentChatTranscriptColors {
    /// `ui.border @ transcript.turn_divider_alpha` (new-turn hairline).
    pub turn_divider_rgba: u32,
    /// `text.primary @ user_message.bg_alpha` (user bubble surface).
    pub user_bg_rgba: u32,
    /// `background.search_box @ markdown.code_block_bg_alpha`
    /// (code blocks AND the diff body box).
    pub code_bg_rgba: u32,
    /// `ui.border @ markdown.code_block_border_alpha`.
    pub code_border_rgba: u32,
    /// `ui.border @ markdown.blockquote_bg_alpha`.
    pub blockquote_bg_rgba: u32,
    /// `ui.border @ markdown.blockquote_border_alpha`.
    pub blockquote_border_rgba: u32,
    /// `text.primary @ collapsible.thought_border_alpha`.
    pub thought_border_rgba: u32,
    /// `accent.selected @ collapsible.tool_border_alpha`.
    pub tool_border_rgba: u32,
    /// `ui.error @ collapsible.tool_border_alpha` (is_error tools).
    pub tool_border_error_rgba: u32,
    /// `text.primary @ AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA`.
    pub tool_status_pending_rgba: u32,
    /// `ui.success`, opaque (complete glyph + added diff text).
    pub tool_status_complete_rgba: u32,
    /// `ui.error`, opaque (failed glyph + removed diff text).
    pub tool_status_failed_rgba: u32,
    /// `ui.success @ AGENT_CHAT_DIFF_TINT_ALPHA`.
    pub diff_added_bg_rgba: u32,
    /// `ui.error @ AGENT_CHAT_DIFF_TINT_ALPHA`.
    pub diff_removed_bg_rgba: u32,
    /// `ui.border @ system.border_alpha`.
    pub system_border_rgba: u32,
    /// `ui.error @ error.bg_alpha` (bg_alpha authored DECIMAL 50 = 0x32).
    pub error_bg_rgba: u32,
    /// `ui.error @ error.border_alpha`.
    pub error_border_rgba: u32,
    /// `text.primary @ AGENT_CHAT_ACTIVITY_LABEL_ALPHA`.
    pub activity_label_rgba: u32,
}

pub(crate) fn resolved_agent_chat_transcript_colors(
    style: &AgentChatStyleDef,
    theme: &crate::theme::Theme,
) -> ResolvedAgentChatTranscriptColors {
    let colors = &theme.colors;
    ResolvedAgentChatTranscriptColors {
        turn_divider_rgba: pack_rgb_alpha(colors.ui.border, style.transcript.turn_divider_alpha),
        user_bg_rgba: pack_rgb_alpha(colors.text.primary, style.user_message.bg_alpha),
        code_bg_rgba: pack_rgb_alpha(
            colors.background.search_box,
            style.markdown.code_block_bg_alpha,
        ),
        code_border_rgba: pack_rgb_alpha(colors.ui.border, style.markdown.code_block_border_alpha),
        blockquote_bg_rgba: pack_rgb_alpha(colors.ui.border, style.markdown.blockquote_bg_alpha),
        blockquote_border_rgba: pack_rgb_alpha(
            colors.ui.border,
            style.markdown.blockquote_border_alpha,
        ),
        thought_border_rgba: pack_rgb_alpha(
            colors.text.primary,
            style.collapsible.thought_border_alpha,
        ),
        tool_border_rgba: pack_rgb_alpha(
            colors.accent.selected,
            style.collapsible.tool_border_alpha,
        ),
        tool_border_error_rgba: pack_rgb_alpha(
            colors.ui.error,
            style.collapsible.tool_border_alpha,
        ),
        tool_status_pending_rgba: pack_rgb_alpha(
            colors.text.primary,
            AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA,
        ),
        tool_status_complete_rgba: (colors.ui.success << 8) | 0xFF,
        tool_status_failed_rgba: (colors.ui.error << 8) | 0xFF,
        diff_added_bg_rgba: pack_rgb_alpha(colors.ui.success, AGENT_CHAT_DIFF_TINT_ALPHA),
        diff_removed_bg_rgba: pack_rgb_alpha(colors.ui.error, AGENT_CHAT_DIFF_TINT_ALPHA),
        system_border_rgba: pack_rgb_alpha(colors.ui.border, style.system.border_alpha),
        error_bg_rgba: pack_rgb_alpha(colors.ui.error, style.error.bg_alpha),
        error_border_rgba: pack_rgb_alpha(colors.ui.error, style.error.border_alpha),
        activity_label_rgba: pack_rgb_alpha(colors.text.primary, AGENT_CHAT_ACTIVITY_LABEL_ALPHA),
    }
}

/// Send button surface + opacity for the four (busy, can_send) states —
/// shared byte owner for the renderer and the exporter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AgentChatSendStateChrome {
    pub bg_rgba: u32,
    pub opacity: f32,
}

pub(crate) fn resolved_agent_chat_send_state_chrome(
    busy: bool,
    can_send: bool,
    accent: u32,
    text_primary: u32,
) -> AgentChatSendStateChrome {
    match (busy, can_send) {
        (true, true) => AgentChatSendStateChrome {
            bg_rgba: pack_rgb_alpha(accent, AGENT_CHAT_SEND_QUEUE_BG_ALPHA),
            opacity: AGENT_CHAT_SEND_QUEUE_OPACITY,
        },
        (true, false) => AgentChatSendStateChrome {
            bg_rgba: 0x0000_0000,
            opacity: AGENT_CHAT_SEND_STREAMING_OPACITY,
        },
        (false, true) => AgentChatSendStateChrome {
            bg_rgba: pack_rgb_alpha(accent, AGENT_CHAT_SEND_ENABLED_BG_ALPHA),
            opacity: AGENT_CHAT_SEND_ENABLED_OPACITY,
        },
        (false, false) => AgentChatSendStateChrome {
            bg_rgba: pack_rgb_alpha(text_primary, AGENT_CHAT_SEND_DISABLED_BG_ALPHA),
            opacity: AGENT_CHAT_SEND_DISABLED_OPACITY,
        },
    }
}

/// Markdown body line box: the renderer never sets a line height, so GPUI's
/// implicit phi() default applies. Resolved through the SAME app-side
/// framework helper the confirm contract uses — never a fresh 1.618034
/// literal in the exporter.
pub(crate) fn resolved_agent_chat_markdown_body_line_height(style: &AgentChatStyleDef) -> f32 {
    crate::confirm::confirm_prompt_line_height_px(style.markdown.body_font_size)
}

/// Single-line composer shell height through the SAME shared formula owner
/// the renderer calls (`main_view_multiline_input_height`): the main-menu
/// search height grows by one composer line per extra visible line, so one
/// line == the shared search height. Fixture-resolved — NOT a universal
/// "composer height" (multiline/expanded composers are taller).
pub(crate) fn resolved_agent_chat_composer_single_line_height(search_height: f32) -> f32 {
    crate::components::main_view_chrome::main_view_multiline_input_height(
        search_height,
        AGENT_CHAT_INPUT_LINE_HEIGHT,
        1,
    )
}

#[cfg(test)]
mod agent_chat_style_contract_tests {
    use super::*;

    fn stock_theme() -> crate::theme::Theme {
        crate::theme::presets::all_presets()
            .into_iter()
            .find(|preset| preset.id == "script-kit-dark")
            .expect("script-kit-dark preset")
            .create_theme()
    }

    #[test]
    fn production_base_source_values_hold() {
        let style = production_agent_chat_style();
        assert_eq!(style.transcript.row_padding_x, 16.0);
        assert_eq!(style.transcript.row_padding_bottom, 4.0);
        assert_eq!(style.transcript.turn_divider_alpha, 0x18 as f32);
        assert_eq!(style.markdown.body_font_size, 14.0);
        assert_eq!(style.markdown.paragraph_gap, 0.28);
        assert_eq!(style.markdown.code_block_bg_alpha, 0xA0 as f32);
        assert_eq!(style.user_message.bg_alpha, 0x06 as f32);
        assert_eq!(style.assistant_message.bg_alpha, 0.0);
        assert_eq!(style.assistant_message.radius, 0.0);
        // Separate thought/tool header opacities stay independently
        // addressable even while both equal 0.75.
        assert_eq!(style.collapsible.thought_header_opacity, 0.75);
        assert_eq!(style.collapsible.tool_header_opacity, 0.75);
        assert_eq!(style.collapsible.thought_border_alpha, 0x7f as f32);
        // The decimal-50 error bg alpha is a foot-gun, recorded as the
        // agentChat.error.bgAlphaUnits conflict — do not "fix" to hex here.
        assert_eq!(style.error.bg_alpha, 50.0);
    }

    #[test]
    fn resolved_transcript_bytes_match_renderer_packing() {
        let theme = stock_theme();
        let style = production_agent_chat_style();
        let resolved = resolved_agent_chat_transcript_colors(&style, &theme);
        // Stock theme: border #343434, text #FFFFFF, search_box #2A2A2A,
        // accent #FBBF24, success #00FF00, error #EF4444.
        assert_eq!(resolved.turn_divider_rgba, 0x343434_18);
        assert_eq!(resolved.user_bg_rgba, 0xFFFFFF_06);
        assert_eq!(resolved.code_bg_rgba, 0x2A2A2A_A0);
        assert_eq!(resolved.code_border_rgba, 0x343434_40);
        assert_eq!(resolved.blockquote_bg_rgba, 0x343434_10);
        assert_eq!(resolved.blockquote_border_rgba, 0x343434_40);
        assert_eq!(resolved.thought_border_rgba, 0xFFFFFF_7F);
        assert_eq!(resolved.tool_border_rgba, 0xFBBF24_7F);
        assert_eq!(resolved.tool_border_error_rgba, 0xEF4444_7F);
        assert_eq!(resolved.tool_status_pending_rgba, 0xFFFFFF_80);
        assert_eq!(resolved.tool_status_complete_rgba, 0x00FF00_FF);
        assert_eq!(resolved.tool_status_failed_rgba, 0xEF4444_FF);
        assert_eq!(resolved.diff_added_bg_rgba, 0x00FF00_14);
        assert_eq!(resolved.diff_removed_bg_rgba, 0xEF4444_14);
        assert_eq!(resolved.system_border_rgba, 0x343434_30);
        // Decimal 50.0 rounds to 0x32 through the shared packer.
        assert_eq!(resolved.error_bg_rgba, 0xEF4444_32);
        assert_eq!(resolved.error_border_rgba, 0xEF4444_80);
        assert_eq!(resolved.activity_label_rgba, 0xFFFFFF_B0);
    }

    #[test]
    fn send_state_chrome_covers_all_four_states() {
        let accent = 0xFBBF24;
        let text = 0xFFFFFF;
        let disabled = resolved_agent_chat_send_state_chrome(false, false, accent, text);
        assert_eq!(disabled.bg_rgba, 0xFFFFFF_06);
        assert_eq!(disabled.opacity, 0.30);
        let enabled = resolved_agent_chat_send_state_chrome(false, true, accent, text);
        assert_eq!(enabled.bg_rgba, 0xFBBF24_30);
        assert_eq!(enabled.opacity, 0.90);
        let queue = resolved_agent_chat_send_state_chrome(true, true, accent, text);
        assert_eq!(queue.bg_rgba, 0xFBBF24_24);
        assert_eq!(queue.opacity, 0.92);
        let streaming = resolved_agent_chat_send_state_chrome(true, false, accent, text);
        assert_eq!(streaming.bg_rgba, 0x0000_0000);
        assert_eq!(streaming.opacity, 0.40);
    }

    #[test]
    fn markdown_body_line_height_uses_the_shared_phi_helper() {
        let style = production_agent_chat_style();
        // 14px body → GPUI's rounded phi line box (same helper as confirm).
        assert_eq!(resolved_agent_chat_markdown_body_line_height(&style), 23.0);
    }

    #[test]
    fn composer_single_line_height_tracks_the_shared_search_height() {
        // One visible line preserves the main menu's exact input geometry
        // (InfoBarBase search height 26); each extra line adds 22.
        assert_eq!(resolved_agent_chat_composer_single_line_height(26.0), 26.0);
        assert_eq!(
            crate::components::main_view_chrome::main_view_multiline_input_height(
                26.0,
                AGENT_CHAT_INPUT_LINE_HEIGHT,
                3
            ),
            70.0
        );
    }
}
