use super::types::conversation_turn_pending_indicator_visible;
#[cfg(test)]
use super::types::conversation_turn_streaming_copy_available;
use super::*;
use std::borrow::Cow;

fn truncate_str_chars(s: &str, max_chars: usize) -> &str {
    s.char_indices()
        .nth(max_chars)
        .map_or(s, |(index, _)| &s[..index])
}

fn assistant_response_region_source<'a>(
    script_generation_mode: bool,
    response: &'a str,
    streaming: bool,
) -> Cow<'a, str> {
    match (response.trim().is_empty(), streaming) {
        (true, true) => Cow::Borrowed("_Thinking…_"),
        (true, false) => Cow::Borrowed("_No response returned._"),
        (false, _) => {
            super::types::assistant_response_markdown_source(script_generation_mode, response)
        }
    }
}

impl ChatPrompt {
    pub(super) fn render_turn(
        &self,
        turn: &ConversationTurn,
        turn_index: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let colors = &self.prompt_colors;
        let theme_colors = &self.theme.colors;

        // VIBRANCY: Use theme-aware overlay for subtle lift that lets blur show through
        // Dark mode: white overlay brightens; Light mode: much subtler black overlay
        let container_bg = if self.theme.is_dark_mode() {
            theme::hover_overlay_bg(&self.theme, 0x15) // ~8% white overlay for dark mode
        } else {
            theme::hover_overlay_bg(&self.theme, 0x08) // ~3% black overlay for light mode
        };
        let copy_hover_bg = theme::hover_overlay_bg(&self.theme, 0x28); // ~16% for hover
        let error_color = theme_colors.ui.error;
        let error_bg = rgba((error_color << 8) | 0x40); // Theme error with transparency
        let retry_hover_bg = rgba((theme_colors.accent.selected << 8) | 0x40);
        let has_retry_callback = self.on_retry.is_some();
        let user_fidelity_id = format!("chat-transcript-user-turn-{turn_index}");
        let response_fidelity_id = format!("chat-transcript-response-turn-{turn_index}");
        let pending_fidelity_id = format!("chat-transcript-pending-turn-{turn_index}");
        let copy_fidelity_id = format!("chat-transcript-copy-turn-{turn_index}");

        let mut content = div().flex().flex_col().gap(px(6.0)).w_full().min_w_0();
        // Note: removed overflow_hidden() to allow text to wrap naturally

        // User prompt (small, bold) - only if not empty
        if !turn.user_prompt.is_empty() {
            content = content.child(
                div()
                    .debug_selector(move || user_fidelity_id)
                    .w_full()
                    .min_w_0()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme_colors.text.secondary))
                    .child(turn.user_prompt.clone()),
            );
        }

        // User image thumbnail (if attached)
        if let Some(ref user_image) = turn.user_image {
            let render_img = user_image.clone();
            content = content.child(
                img(move |_window: &mut Window, _cx: &mut App| Some(Ok(render_img.clone())))
                    .w(px(64.))
                    .h(px(64.))
                    .rounded_sm(),
            );
        }

        // Error state - show error message with optional retry button
        if turn.failure.is_some() || turn.error.is_some() {
            let typed_message = turn
                .failure
                .as_ref()
                .map(|failure| crate::ai::reliability::primary_message_for_failure(failure));
            let error_str = typed_message
                .or(turn.error.as_deref())
                .unwrap_or("The AI request did not finish.");
            let error_type = ChatErrorType::from_error_string(error_str);
            let error_message = typed_message.unwrap_or_else(|| error_type.display_message());
            let can_retry = error_type.can_retry() && has_retry_callback;

            let error_fidelity_id = response_fidelity_id.clone();
            let mut error_row = div()
                .debug_selector(move || error_fidelity_id)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(error_color))
                        .child(error_message.to_string()),
                );

            // Add retry button if applicable
            if can_retry {
                let message_id = turn.message_id.clone();
                error_row = error_row.child(
                    div()
                        .id(format!("retry-turn-{}", turn_index))
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(error_bg)
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(retry_hover_bg))
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(theme_colors.text.primary))
                        .child("Retry")
                        .on_click(cx.listener(move |this, _, _window, _cx| {
                            if let Some(msg_id) = &message_id {
                                this.handle_retry(msg_id.clone());
                            }
                        })),
                );
            }

            content = content.child(error_row);

            // Show raw error detail so the actual cause is visible
            let detail = error_str.trim();
            if typed_message.is_none() && !detail.is_empty() && detail != error_message {
                // Unknown errors: full opacity + more chars since raw message is the only info
                let is_unknown = error_type == ChatErrorType::Unknown;
                let max_chars = if is_unknown { 400 } else { 200 };
                let truncated = if detail.chars().count() > max_chars {
                    format!("{}…", truncate_str_chars(detail, max_chars))
                } else {
                    detail.to_string()
                };
                let detail_opacity = if is_unknown { 1.0 } else { 0.5 };
                content = content.child(
                    div()
                        .text_xs()
                        .opacity(detail_opacity)
                        .text_color(rgb(error_color))
                        .child(truncated),
                );
            }
        }
        // AI response (only show if no error, or show partial if stream interrupted)
        else if turn.assistant_response.is_some() || turn.streaming {
            let response = turn.assistant_response.as_deref().unwrap_or("");
            let markdown_response = assistant_response_region_source(
                self.script_generation_mode,
                response,
                turn.streaming,
            );

            // Empty pending, first text, and terminal-empty responses all use
            // the same markdown response region. Streaming activity lives in
            // the card's fixed trailing slot, so it cannot add a row below it.
            content = content.child(
                div()
                    .debug_selector(move || response_fidelity_id)
                    .w_full()
                    .min_w_0()
                    .overflow_x_hidden()
                    .child(render_markdown(markdown_response.as_ref(), colors)),
            );
        }

        let show_streaming_indicator = conversation_turn_pending_indicator_visible(turn);
        let trailing_control = div()
            .id(format!("copy-turn-{}", turn_index))
            .debug_selector(move || copy_fidelity_id)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(24.0))
            .h(px(24.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .opacity(0.7)
            .hover(|s| s.opacity(1.0).bg(copy_hover_bg))
            .child(
                svg()
                    .path(IconName::Copy.asset_path())
                    .size(px(16.))
                    .text_color(rgb(theme_colors.text.secondary)),
            )
            .when(show_streaming_indicator, |slot| {
                slot.child(
                    div()
                        .debug_selector(move || pending_fidelity_id)
                        .absolute()
                        .right(px(1.0))
                        .bottom(px(1.0))
                        .size(px(7.0))
                        .rounded(px(999.0))
                        .bg(rgb(theme_colors.accent.selected))
                        .with_animation(
                            ("chat-turn-streaming-dot-pulse", turn_index),
                            Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                            |style, delta| {
                                let sine = (delta * std::f32::consts::PI * 2.0).sin();
                                style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                            },
                        ),
                )
            })
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.copy_turn_response(turn_index, cx);
            }));

        // The full-width container with copy button
        div()
            .w_full()
            .px(px(CHAT_LAYOUT_CARD_PADDING_X))
            .py(px(CHAT_LAYOUT_CARD_PADDING_Y))
            .bg(container_bg)
            .rounded(px(8.0))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .child(content.flex_1())
            .child(trailing_control)
    }

    /// Handle retry for a failed message
    pub(super) fn handle_retry(&self, message_id: String) {
        logging::log(
            "CHAT",
            &format!("Retry requested for message: {}", message_id),
        );
        if let Some(ref callback) = self.on_retry {
            callback(self.id.clone(), message_id);
        }
    }

    /// Copy the assistant response from a specific turn
    pub(super) fn copy_turn_response(&mut self, turn_index: usize, cx: &mut Context<Self>) {
        self.ensure_conversation_turns_cache();
        if let Some(turn) = self.conversation_turns_cache.get(turn_index) {
            if let Some(ref response) = turn.assistant_response {
                let content = response.clone();
                logging::log(
                    "CHAT",
                    &format!(
                        "Copied turn {} response: {} chars",
                        turn_index,
                        content.len()
                    ),
                );
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            } else if !turn.user_prompt.is_empty() {
                // If no assistant response, copy the user prompt
                let content = turn.user_prompt.clone();
                logging::log(
                    "CHAT",
                    &format!(
                        "Copied turn {} user prompt: {} chars",
                        turn_index,
                        content.len()
                    ),
                );
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assistant_response_region_source, conversation_turn_pending_indicator_visible,
        conversation_turn_streaming_copy_available, truncate_str_chars, ConversationTurn,
    };
    const CHAT_RENDER_TURNS_SOURCE: &str = include_str!("render_turns.rs");

    #[test]
    fn test_truncate_str_chars_returns_original_when_detail_within_limit() {
        assert_eq!(truncate_str_chars("error", 200), "error");
    }

    #[test]
    fn test_truncate_str_chars_truncates_detail_without_breaking_utf8_chars() {
        let input = "🙂🙂🙂abc";
        assert_eq!(truncate_str_chars(input, 2), "🙂🙂");
    }

    #[test]
    fn assistant_response_region_has_stable_honest_empty_states() {
        assert_eq!(
            assistant_response_region_source(false, "", true),
            "_Thinking…_"
        );
        assert_eq!(
            assistant_response_region_source(false, "", false),
            "_No response returned._"
        );
        assert_eq!(
            assistant_response_region_source(false, "First token", true),
            "First token"
        );
    }

    #[test]
    fn pending_indicator_hands_off_to_streaming_copy_after_first_visible_text() {
        let turn = |assistant_response: Option<&str>, streaming: bool, error: Option<&str>| {
            ConversationTurn {
                user_prompt: "user".to_string(),
                assistant_response: assistant_response.map(str::to_string),
                model: None,
                streaming,
                error: error.map(str::to_string),
                failure: None,
                message_id: None,
                user_image: None,
            }
        };

        for pending in [turn(None, true, None), turn(Some(""), true, None)] {
            assert!(conversation_turn_pending_indicator_visible(&pending));
            assert!(!conversation_turn_streaming_copy_available(&pending));
        }
        for visible in [
            turn(Some("First"), true, None),
            turn(Some("First token"), true, None),
        ] {
            assert!(!conversation_turn_pending_indicator_visible(&visible));
            assert!(conversation_turn_streaming_copy_available(&visible));
        }
        for terminal in [
            turn(Some("done"), false, None),
            turn(None, false, Some("error")),
        ] {
            assert!(!conversation_turn_pending_indicator_visible(&terminal));
            assert!(!conversation_turn_streaming_copy_available(&terminal));
        }
    }

    #[test]
    fn test_render_turn_uses_theme_colors_and_keeps_copy_alignment_without_manual_offset() {
        let legacy_text_pattern = ["rgb(colors.", "text_"].concat();
        let legacy_copy_button_margin = ["copy_button", ".mt(px(1.0))"].concat();

        assert!(
            CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.text.secondary")
                && CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.text.primary")
                && CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.accent.selected"),
            "Turn renderer should use theme color scheme entries for turn text and accents"
        );
        assert!(
            !CHAT_RENDER_TURNS_SOURCE.contains(&legacy_text_pattern),
            "Turn renderer should avoid prompt palette text colors for turn chrome"
        );
        assert!(
            !CHAT_RENDER_TURNS_SOURCE.contains(&legacy_copy_button_margin),
            "Copy button should align without a manual margin offset wrapper"
        );
    }
}
