//! Shared conversation style contract.
//!
//! Single typed owner of the paint values for EVERY conversation surface —
//! Agent Chat and Flow's `ChatPrompt` — so the two can never drift. This
//! module owns the style struct definitions, the production base values, the
//! composer/send constants, and the pure color/geometry resolvers.
//!
//! ## Why this lives under `src/components/**`
//!
//! It began life as `src/ai/agent_chat/ui/style_contract.rs`, where it was
//! already the single typed owner of Agent Chat's values (2026-07-11 Oracle
//! review). That was correct but surface-scoped: Flow rendered its answers
//! through a completely separate bespoke markdown renderer and could not
//! consume it. Promoting the module here — per the repo's shared-component
//! contract, which says reusable UI belongs under `src/components/**` — lets
//! both surfaces read the same values.
//!
//! The former Agent Chat compatibility façade was deleted after all production,
//! test, and design-contract callers migrated here (GOV-002). Persisted design
//! token paths now name this canonical owner directly.
//!
//! ## Contract rules
//!
//! - This module is the single owner of conversation style values.
//! - Checked-in design-contract export artifacts read
//!   [`production_conversation_style`] directly;
//!   nothing else may reach the exporter.
//! - All theme-color + authored-alpha packing shared by the renderers and the
//!   exporter routes through [`pack_rgb_alpha`] / the resolvers below, so
//!   rounding/cast behavior (0x7F borders, 0x14 diff tints, the decimal-50
//!   error background, send-state bytes) has exactly one owner.

use crate::theme::{pack_rgb_alpha, AlphaByte};
// ── Style definition ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationStyleDef {
    pub transcript: ConversationTranscriptStyle,
    pub markdown: ConversationMarkdownStyle,
    pub user_message: ConversationMessageStyle,
    pub assistant_message: ConversationMessageStyle,
    pub collapsible: ConversationCollapsibleStyle,
    pub error: ConversationErrorStyle,
    pub system: ConversationSystemStyle,
    pub actions: ConversationActionStyle,
}

/// Per-turn action affordance metrics (the response copy button and its
/// streaming activity dot).
///
/// These values are lifted verbatim from Flow's existing copy control in
/// `src/prompts/chat/render_turns.rs`, which was the only implementation of
/// this affordance. They live here so Agent Chat's port reuses the exact same
/// hit target and hover treatment instead of re-authoring approximations in a
/// second renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationActionStyle {
    /// Square hit target for the trailing action control.
    pub button_size: f32,
    pub button_radius: f32,
    /// Resting opacity; the control lifts to `button_hover_opacity` on hover.
    pub button_opacity: f32,
    pub button_hover_opacity: f32,
    /// Hover surface tint alpha over `text.primary`.
    pub button_hover_bg_alpha: AlphaByte,
    pub icon_size: f32,
    /// Streaming activity dot painted at the control's bottom-right corner.
    pub activity_dot_size: f32,
    pub activity_dot_inset: f32,
    /// Pulse period for that dot, in milliseconds.
    pub activity_pulse_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationTranscriptStyle {
    pub row_padding_x: f32,
    pub row_padding_bottom: f32,
    pub dense_row_padding_bottom: f32,
    pub response_start_margin_top: f32,
    pub turn_margin_top: f32,
    pub turn_padding_top: f32,
    pub turn_divider_alpha: AlphaByte,
    pub focused_preview_padding_x: f32,
    pub focused_preview_padding_bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationMarkdownStyle {
    pub body_font_size: f32,
    pub paragraph_gap: f32,
    pub heading_1_font_size: f32,
    pub heading_2_font_size: f32,
    pub heading_3_font_size: f32,
    pub code_block_font_size: f32,
    pub code_block_padding_x: f32,
    pub code_block_padding_y: f32,
    pub code_block_radius: f32,
    pub code_block_bg_alpha: AlphaByte,
    pub code_block_border_alpha: AlphaByte,
    pub blockquote_padding_x: f32,
    pub blockquote_padding_y: f32,
    pub blockquote_radius: f32,
    pub blockquote_bg_alpha: AlphaByte,
    pub blockquote_border_alpha: AlphaByte,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationMessageStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub dense_padding_y: f32,
    pub radius: f32,
    pub bg_alpha: AlphaByte,
    /// Applied ONLY under the RoleSplit transcript presentation; Standard
    /// paints full-width rows (variant-limited source fact, not dead).
    pub max_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationCollapsibleStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub body_padding_top: f32,
    pub max_body_height: f32,
    pub thought_header_opacity: f32,
    pub tool_header_opacity: f32,
    pub status_opacity: f32,
    pub thought_border_alpha: AlphaByte,
    pub tool_border_alpha: AlphaByte,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationErrorStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    /// NOTE: authored as DECIMAL 50 (= 0x32) while sibling alphas are
    /// hex-authored — recorded as the `agentChat.error.bgAlphaUnits`
    /// contract conflict, deliberately NOT normalized here.
    pub bg_alpha: AlphaByte,
    pub border_alpha: AlphaByte,
    pub label_opacity: f32,
    pub hint_opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversationSystemStyle {
    pub padding_x: f32,
    pub padding_y: f32,
    pub opacity: f32,
    pub border_alpha: AlphaByte,
}

/// The production conversation base style. Checked-in design artifacts and
/// live rendering on BOTH Agent Chat and Flow read this function.
pub fn production_conversation_style() -> ConversationStyleDef {
    ConversationStyleDef {
        transcript: ConversationTranscriptStyle {
            row_padding_x: 16.0,
            row_padding_bottom: 4.0,
            dense_row_padding_bottom: 1.0,
            response_start_margin_top: 4.0,
            turn_margin_top: 8.0,
            turn_padding_top: 8.0,
            turn_divider_alpha: AlphaByte::authored(0x18),
            focused_preview_padding_x: 8.0,
            focused_preview_padding_bottom: 4.0,
        },
        markdown: ConversationMarkdownStyle {
            body_font_size: 14.0,
            paragraph_gap: 0.28,
            heading_1_font_size: 17.0,
            heading_2_font_size: 16.0,
            heading_3_font_size: 15.0,
            code_block_font_size: 13.0,
            code_block_padding_x: 7.0,
            code_block_padding_y: 4.0,
            code_block_radius: 5.0,
            code_block_bg_alpha: AlphaByte::authored(0xA0),
            code_block_border_alpha: AlphaByte::authored(0x40),
            blockquote_padding_x: 12.0,
            blockquote_padding_y: 6.0,
            blockquote_radius: 5.0,
            blockquote_bg_alpha: AlphaByte::authored(0x10),
            blockquote_border_alpha: AlphaByte::authored(0x40),
        },
        user_message: ConversationMessageStyle {
            padding_x: 12.0,
            padding_y: 8.0,
            dense_padding_y: 3.0,
            radius: 8.0,
            bg_alpha: AlphaByte::authored(0x06),
            max_width: 520.0,
        },
        assistant_message: ConversationMessageStyle {
            padding_x: 12.0,
            padding_y: 4.0,
            dense_padding_y: 2.0,
            radius: 0.0,
            bg_alpha: AlphaByte::authored(0x00),
            max_width: 620.0,
        },
        collapsible: ConversationCollapsibleStyle {
            padding_x: 12.0,
            padding_y: 2.0,
            body_padding_top: 4.0,
            max_body_height: 200.0,
            thought_header_opacity: 0.75,
            tool_header_opacity: 0.75,
            status_opacity: 0.50,
            thought_border_alpha: AlphaByte::authored(0x7f),
            tool_border_alpha: AlphaByte::authored(0x7f),
        },
        error: ConversationErrorStyle {
            padding_x: 12.0,
            padding_y: 8.0,
            radius: 8.0,
            bg_alpha: AlphaByte::authored(50),
            border_alpha: AlphaByte::authored(0x80),
            label_opacity: 0.75,
            hint_opacity: 0.40,
        },
        system: ConversationSystemStyle {
            padding_x: 12.0,
            padding_y: 4.0,
            opacity: 0.60,
            border_alpha: AlphaByte::authored(0x30),
        },
        actions: ConversationActionStyle {
            button_size: 24.0,
            button_radius: 4.0,
            button_opacity: 0.7,
            button_hover_opacity: 1.0,
            // Flow resolved its hover surface through
            // `theme::hover_overlay_bg(&theme, 0x28)`; the alpha is carried
            // here so both surfaces tint identically.
            button_hover_bg_alpha: AlphaByte::authored(0x28),
            icon_size: 16.0,
            activity_dot_size: 7.0,
            activity_dot_inset: 1.0,
            activity_pulse_ms: 1200,
        },
    }
}

// ── Composer constants ────────────────────────────────────────────────────

/// Horizontal padding used by the conversation composer input row (picker
/// clamping / measurement lanes; the shell's text insets come from the
/// shared main-view input shell).
pub(crate) const CONVERSATION_INPUT_PADDING_X: f32 = 12.0;
/// Top padding used by the conversation composer input row (picker lane
/// positioning; the shell height derives from the shared search height +
/// line growth, not from this padding).
pub(crate) const CONVERSATION_INPUT_PADDING_Y: f32 = 10.0;
/// Legacy design-contract line height for detached/experimental hosts.
/// Active Agent Chat composers use the main-menu search geometry instead.
pub(crate) const CONVERSATION_INPUT_LINE_HEIGHT: f32 = 22.0;
/// Legacy design-contract font identity. Active composers resolve the
/// theme UI family through `AgentChatComposerTextStyle`.
pub(crate) const CONVERSATION_INPUT_FONT_FAMILY: &str = ".SystemUIFont";

/// Composer placeholder while the transcript is empty.
pub(crate) const CONVERSATION_PLACEHOLDER_ASK: &str = "Ask anything\u{2026}";
/// Composer placeholder once the transcript has messages (cleared input +
/// non-empty transcript).
pub(crate) const CONVERSATION_PLACEHOLDER_FOLLOW_UP: &str = "Follow up\u{2026}";

// ── Send button constants ──────────────────────────────────────────────────

pub(crate) const CONVERSATION_SEND_SIZE: f32 = 24.0;
pub(crate) const CONVERSATION_SEND_RADIUS: f32 = 6.0;
/// idle + empty input: `text.primary @ 0x06`, opacity 0.30 (`↑`).
pub(crate) const CONVERSATION_SEND_DISABLED_BG_ALPHA: AlphaByte = AlphaByte::authored(0x06);
pub(crate) const CONVERSATION_SEND_DISABLED_OPACITY: f32 = 0.30;
/// idle + text: `accent @ 0x30`, opacity 0.90 (`↑`).
pub(crate) const CONVERSATION_SEND_ENABLED_BG_ALPHA: AlphaByte = AlphaByte::authored(0x30);
pub(crate) const CONVERSATION_SEND_ENABLED_OPACITY: f32 = 0.90;
/// streaming + text: `accent @ 0x24`, opacity 0.92 (queue `⇧`).
pub(crate) const CONVERSATION_SEND_QUEUE_BG_ALPHA: AlphaByte = AlphaByte::authored(0x24);
pub(crate) const CONVERSATION_SEND_QUEUE_OPACITY: f32 = 0.92;
/// streaming + empty: transparent, opacity 0.40 (activity dot `●`).
pub(crate) const CONVERSATION_SEND_STREAMING_OPACITY: f32 = 0.40;

// ── Renderer literals hoisted for the contract ─────────────────────────────

/// Collapsible/tool/system/error left border width (was `.border_l_2()`).
pub(crate) const CONVERSATION_BLOCK_BORDER_WIDTH: f32 = 2.0;
/// Collapsible/tool header row gap (was `.gap_1()`).
pub(crate) const CONVERSATION_BLOCK_HEADER_GAP: f32 = 4.0;
/// Tool status glyph alpha for pending tools (`text.primary @ 0x80`).
pub(crate) const CONVERSATION_TOOL_STATUS_PENDING_ALPHA: AlphaByte = AlphaByte::authored(0x80);
/// Added/removed diff row background tint alpha (`success/error @ 0x14`).
pub(crate) const CONVERSATION_DIFF_TINT_ALPHA: AlphaByte = AlphaByte::authored(0x14);
/// Context (unchanged) diff row opacity.
pub(crate) const CONVERSATION_DIFF_CONTEXT_OPACITY: f32 = 0.55;
/// Synthetic activity tail row: pulsing accent dot diameter.
pub(crate) const CONVERSATION_ACTIVITY_DOT_SIZE: f32 = 7.0;
/// Activity row dot ↔ label gap.
pub(crate) const CONVERSATION_ACTIVITY_GAP: f32 = 8.0;
/// Activity row "Thinking…" label alpha (`text.primary @ 0xB0`).
pub(crate) const CONVERSATION_ACTIVITY_LABEL_ALPHA: AlphaByte = AlphaByte::authored(0xB0);

// ── Pure resolvers (theme × authored alphas → painted RGBA bytes) ─────────

/// Every alpha-packed transcript color the renderer paints, resolved from
/// the SAME theme authorities the render fns read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedConversationTranscriptColors {
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
    /// `text.primary @ CONVERSATION_TOOL_STATUS_PENDING_ALPHA`.
    pub tool_status_pending_rgba: u32,
    /// `ui.success`, opaque (complete glyph + added diff text).
    pub tool_status_complete_rgba: u32,
    /// `ui.error`, opaque (failed glyph + removed diff text).
    pub tool_status_failed_rgba: u32,
    /// `ui.success @ CONVERSATION_DIFF_TINT_ALPHA`.
    pub diff_added_bg_rgba: u32,
    /// `ui.error @ CONVERSATION_DIFF_TINT_ALPHA`.
    pub diff_removed_bg_rgba: u32,
    /// `ui.border @ system.border_alpha`.
    pub system_border_rgba: u32,
    /// `ui.error @ error.bg_alpha` (bg_alpha authored DECIMAL 50 = 0x32).
    pub error_bg_rgba: u32,
    /// `ui.error @ error.border_alpha`.
    pub error_border_rgba: u32,
    /// `text.primary @ CONVERSATION_ACTIVITY_LABEL_ALPHA`.
    pub activity_label_rgba: u32,
}

pub(crate) fn resolved_conversation_transcript_colors(
    style: &ConversationStyleDef,
    theme: &crate::theme::Theme,
) -> ResolvedConversationTranscriptColors {
    let colors = &theme.colors;
    ResolvedConversationTranscriptColors {
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
            CONVERSATION_TOOL_STATUS_PENDING_ALPHA,
        ),
        tool_status_complete_rgba: crate::theme::alpha::pack_rgb_alpha(
            colors.ui.success,
            crate::theme::AlphaByte::authored(0xFF),
        ),
        tool_status_failed_rgba: crate::theme::alpha::pack_rgb_alpha(
            colors.ui.error,
            crate::theme::AlphaByte::authored(0xFF),
        ),
        diff_added_bg_rgba: pack_rgb_alpha(colors.ui.success, CONVERSATION_DIFF_TINT_ALPHA),
        diff_removed_bg_rgba: pack_rgb_alpha(colors.ui.error, CONVERSATION_DIFF_TINT_ALPHA),
        system_border_rgba: pack_rgb_alpha(colors.ui.border, style.system.border_alpha),
        error_bg_rgba: pack_rgb_alpha(colors.ui.error, style.error.bg_alpha),
        error_border_rgba: pack_rgb_alpha(colors.ui.error, style.error.border_alpha),
        activity_label_rgba: pack_rgb_alpha(colors.text.primary, CONVERSATION_ACTIVITY_LABEL_ALPHA),
    }
}

/// Send button surface + opacity for the four (busy, can_send) states —
/// shared byte owner for the renderer and the exporter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ConversationSendStateChrome {
    pub bg_rgba: u32,
    pub opacity: f32,
}

pub(crate) fn resolved_conversation_send_state_chrome(
    busy: bool,
    can_send: bool,
    accent: u32,
    text_primary: u32,
) -> ConversationSendStateChrome {
    match (busy, can_send) {
        (true, true) => ConversationSendStateChrome {
            bg_rgba: pack_rgb_alpha(accent, CONVERSATION_SEND_QUEUE_BG_ALPHA),
            opacity: CONVERSATION_SEND_QUEUE_OPACITY,
        },
        (true, false) => ConversationSendStateChrome {
            bg_rgba: 0x0000_0000,
            opacity: CONVERSATION_SEND_STREAMING_OPACITY,
        },
        (false, true) => ConversationSendStateChrome {
            bg_rgba: pack_rgb_alpha(accent, CONVERSATION_SEND_ENABLED_BG_ALPHA),
            opacity: CONVERSATION_SEND_ENABLED_OPACITY,
        },
        (false, false) => ConversationSendStateChrome {
            bg_rgba: pack_rgb_alpha(text_primary, CONVERSATION_SEND_DISABLED_BG_ALPHA),
            opacity: CONVERSATION_SEND_DISABLED_OPACITY,
        },
    }
}

/// Markdown body line box: the renderer never sets a line height, so GPUI's
/// implicit phi() default applies. Resolved through the SAME app-side
/// framework helper the confirm contract uses — never a fresh 1.618034
/// literal in the exporter.
pub(crate) fn resolved_conversation_markdown_body_line_height(style: &ConversationStyleDef) -> f32 {
    crate::confirm::confirm_prompt_line_height_px(style.markdown.body_font_size)
}

/// Legacy design-contract projection of the one-line composer shell. Active
/// Agent Chat rendering reads the canonical main-menu search geometry.
pub(crate) fn resolved_conversation_composer_single_line_height(search_height: f32) -> f32 {
    crate::components::main_view_chrome::main_view_multiline_input_height(
        search_height,
        CONVERSATION_INPUT_LINE_HEIGHT,
        1,
    )
}

#[cfg(test)]
mod conversation_style_contract_tests {
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
        let style = production_conversation_style();
        assert_eq!(style.transcript.row_padding_x, 16.0);
        assert_eq!(style.transcript.row_padding_bottom, 4.0);
        assert_eq!(style.transcript.turn_divider_alpha.get(), 0x18);
        assert_eq!(style.markdown.body_font_size, 14.0);
        assert_eq!(style.markdown.paragraph_gap, 0.28);
        assert_eq!(style.markdown.code_block_bg_alpha.get(), 0xA0);
        assert_eq!(style.user_message.bg_alpha.get(), 0x06);
        assert_eq!(style.assistant_message.bg_alpha.get(), 0);
        assert_eq!(style.assistant_message.radius, 0.0);
        // Separate thought/tool header opacities stay independently
        // addressable even while both equal 0.75.
        assert_eq!(style.collapsible.thought_header_opacity, 0.75);
        assert_eq!(style.collapsible.tool_header_opacity, 0.75);
        assert_eq!(style.collapsible.thought_border_alpha.get(), 0x7f);
        // The decimal-50 error bg alpha is a foot-gun, recorded as the
        // agentChat.error.bgAlphaUnits conflict — do not "fix" to hex here.
        assert_eq!(style.error.bg_alpha.get(), 50);
    }

    /// GOV-003: the authored decimal-50 error byte stays EXACTLY 0x32
    /// through the typed authored-byte boundary — and would clamp to 0xFF
    /// through the normalized constructor, which is why the two paths are
    /// separate types/constructors instead of one f32.
    #[test]
    fn error_bg_alpha_is_the_authored_byte_0x32_never_a_normalized_opacity() {
        use crate::theme::AlphaByte;
        let style = production_conversation_style();
        assert_eq!(style.error.bg_alpha.get(), 0x32);
        assert_eq!(AlphaByte::authored(0x32).get(), 0x32);
        // The negative control: feeding the authored decimal through the
        // normalized quantizer is NOT the same byte.
        assert_ne!(AlphaByte::from_normalized(50.0).get(), 0x32);
    }

    #[test]
    fn resolved_transcript_bytes_match_renderer_packing() {
        let theme = stock_theme();
        let style = production_conversation_style();
        let resolved = resolved_conversation_transcript_colors(&style, &theme);
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
        let disabled = resolved_conversation_send_state_chrome(false, false, accent, text);
        assert_eq!(disabled.bg_rgba, 0xFFFFFF_06);
        assert_eq!(disabled.opacity, 0.30);
        let enabled = resolved_conversation_send_state_chrome(false, true, accent, text);
        assert_eq!(enabled.bg_rgba, 0xFBBF24_30);
        assert_eq!(enabled.opacity, 0.90);
        let queue = resolved_conversation_send_state_chrome(true, true, accent, text);
        assert_eq!(queue.bg_rgba, 0xFBBF24_24);
        assert_eq!(queue.opacity, 0.92);
        let streaming = resolved_conversation_send_state_chrome(true, false, accent, text);
        assert_eq!(streaming.bg_rgba, 0x0000_0000);
        assert_eq!(streaming.opacity, 0.40);
    }

    #[test]
    fn markdown_body_line_height_uses_the_shared_phi_helper() {
        let style = production_conversation_style();
        // 14px body → GPUI's rounded phi line box (same helper as confirm).
        assert_eq!(
            resolved_conversation_markdown_body_line_height(&style),
            23.0
        );
    }

    #[test]
    fn composer_single_line_height_tracks_the_shared_search_height() {
        assert_eq!(
            resolved_conversation_composer_single_line_height(26.0),
            26.0
        );
        assert_eq!(
            crate::components::main_view_chrome::main_view_multiline_input_height(
                26.0,
                CONVERSATION_INPUT_LINE_HEIGHT,
                3
            ),
            70.0
        );
    }

    #[test]
    fn canonical_owner_is_deterministic() {
        assert_eq!(
            production_conversation_style(),
            production_conversation_style()
        );
    }
}
