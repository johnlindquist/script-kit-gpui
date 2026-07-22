use super::types::{
    conversation_turn_pending_indicator_visible, conversation_turn_streaming_copy_available,
};
use super::*;

fn normalize_to_png_bytes(raw_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    use anyhow::Context as _;

    let decoded =
        image::load_from_memory(raw_bytes).context("Failed to decode dropped image bytes")?;
    let mut png_cursor = std::io::Cursor::new(Vec::new());
    decoded
        .write_to(&mut png_cursor, image::ImageFormat::Png)
        .context("Failed to encode dropped image bytes as PNG")?;
    Ok(png_cursor.into_inner())
}

impl ChatPrompt {
    /// Install one deterministic transcript phase without invoking a provider.
    /// Used by the Agent Chat/FlowSession geometry timeline proof.
    pub(crate) fn apply_transcript_geometry_fixture(
        &mut self,
        phase: &str,
        user_text: Option<String>,
        assistant_text: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let user_text = user_text.unwrap_or_else(|| "Geometry fixture user row".to_string());
        if user_text.trim().is_empty() {
            return Err("transcript geometry fixture requires non-empty userText".to_string());
        }

        let user = ChatPromptMessage {
            id: Some("geometry-fixture-user".to_string()),
            role: Some(ChatMessageRole::User),
            content: Some(user_text),
            text: String::new(),
            position: ChatMessagePosition::Right,
            name: None,
            model: None,
            streaming: false,
            error: None,
            created_at: None,
            image: None,
        };
        let mut messages = vec![user];
        let default_stream_text = "First token and deterministic follow-up tokens.".to_string();
        let (assistant, streaming) = match phase {
            "waitingNoAssistant" | "waiting-no-assistant" => (None, true),
            "emptyAssistant" | "empty-assistant" => (Some((String::new(), None)), true),
            "firstToken" | "first-token" => (Some(("First".to_string(), None)), true),
            "multiTokenStreaming" | "multi-token-streaming" => (
                Some((
                    assistant_text.unwrap_or_else(|| default_stream_text.clone()),
                    None,
                )),
                true,
            ),
            "completed" => (
                Some((
                    assistant_text.unwrap_or_else(|| default_stream_text.clone()),
                    None,
                )),
                false,
            ),
            "terminalEmpty" | "terminal-empty" => (Some((String::new(), None)), false),
            "error" | "provider-error" => (
                Some((
                    String::new(),
                    Some(assistant_text.unwrap_or_else(|| "Fixture provider error".to_string())),
                )),
                false,
            ),
            other => {
                return Err(format!(
                    "unknown transcript geometry fixture phase {other:?}"
                ));
            }
        };

        if let Some((content, error)) = assistant {
            messages.push(ChatPromptMessage {
                id: Some("geometry-fixture-assistant".to_string()),
                role: Some(ChatMessageRole::Assistant),
                content: Some(content),
                text: String::new(),
                position: ChatMessagePosition::Left,
                name: None,
                model: self.model.clone(),
                streaming,
                error,
                created_at: None,
                image: None,
            });
        }

        self.messages = messages;
        self.streaming_message_id = streaming.then_some("geometry-fixture-assistant".to_string());
        self.builtin_is_streaming = streaming;
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        if streaming
            && self
                .conversation_turns_cache
                .last()
                .is_some_and(|turn| turn.assistant_response.is_none())
        {
            let mut turns = self.conversation_turns_cache.as_ref().clone();
            if let Some(turn) = turns.last_mut() {
                turn.streaming = true;
            }
            self.conversation_turns_cache = Arc::new(turns);
            self.sync_turns_list_state();
        }
        if !self.user_has_scrolled_up {
            self.turns_list_state.set_follow_tail(true);
        }
        cx.notify();
        Ok(())
    }

    pub(crate) fn set_transcript_geometry_scroll(
        &mut self,
        item_ix: usize,
        offset_px: f32,
        cx: &mut Context<Self>,
    ) {
        self.ensure_conversation_turns_cache();
        self.user_has_scrolled_up = true;
        self.turns_list_state.set_follow_tail(false);
        self.turns_list_state.scroll_to(gpui::ListOffset {
            item_ix: item_ix.min(self.conversation_turns_cache.len().saturating_sub(1)),
            offset_in_item: px(offset_px.max(0.0)),
        });
        cx.notify();
    }

    pub(crate) fn transcript_geometry_snapshot(&self) -> serde_json::Value {
        let mut turns = build_conversation_turns(&self.messages, &self.image_render_cache);
        if self.is_streaming()
            && turns
                .last()
                .is_some_and(|turn| turn.assistant_response.is_none())
        {
            if let Some(turn) = turns.last_mut() {
                turn.streaming = true;
            }
        }
        let logical = self.turns_list_state.logical_scroll_top();
        let viewport_height = self
            .turns_list_state
            .viewport_bounds()
            .size
            .height
            .as_f32()
            .max(0.0);
        let max_scroll_top = self
            .turns_list_state
            .max_offset_for_scrollbar()
            .y
            .as_f32()
            .max(0.0);
        let scroll_top = (-self
            .turns_list_state
            .scroll_px_offset_for_scrollbar()
            .y
            .as_f32())
        .clamp(0.0, max_scroll_top);
        let pending_indicator_count = turns
            .iter()
            .filter(|turn| conversation_turn_pending_indicator_visible(turn))
            .count();
        let streaming_copy_available = turns.iter().any(conversation_turn_streaming_copy_available);
        let row_semantic_ids = turns
            .iter()
            .enumerate()
            .flat_map(|(index, turn)| {
                let mut ids = vec![format!("chat-transcript-user-turn-{index}")];
                if turn.assistant_response.is_some() || turn.streaming || turn.error.is_some() {
                    ids.push(format!("chat-transcript-response-turn-{index}"));
                }
                ids
            })
            .collect::<Vec<_>>();

        serde_json::json!({
            "rowSemanticIds": row_semantic_ids,
            "viewportHeightPx": viewport_height,
            "contentHeightPx": viewport_height + max_scroll_top,
            "scrollAnchor": {
                "itemIx": logical.item_ix,
                "offsetPx": logical.offset_in_item.as_f32(),
                "scrollTopPx": scroll_top,
            },
            "followTail": !self.user_has_scrolled_up,
            "manualScroll": self.user_has_scrolled_up,
            "selectedText": {
                "composerHasSelection": !self.input.selection().is_empty(),
                "transcriptSelectionObserved": false,
            },
            "pendingIndicatorCount": pending_indicator_count,
            "streamingCopyAvailable": streaming_copy_available,
            "measurementSource": "chatPromptListState",
        })
    }

    pub(super) fn mark_conversation_turns_dirty(&mut self) {
        self.conversation_turns_dirty = true;
    }

    pub(super) fn sync_turns_list_state(&mut self) {
        let item_count = self.conversation_turns_cache.len();
        let old_count = self.turns_list_state.item_count();
        if old_count != item_count {
            self.turns_list_state.splice(0..old_count, item_count);
        } else if self.streaming_message_id.is_some() && item_count > 0 {
            // Content within the streaming turn changed — invalidate its cached
            // height so the list re-measures it on the next layout pass.
            let last = item_count - 1;
            self.turns_list_state.splice(last..item_count, 1);
        }
    }

    pub(super) fn ensure_conversation_turns_cache(&mut self) {
        if !self.conversation_turns_dirty {
            return;
        }
        // WP-B3: bytes scanned by the rebuild = the message content it walks.
        let bytes_scanned: usize = self.messages.iter().map(|m| m.get_content().len()).sum();
        let input_messages = self.messages.len();
        self.conversation_turns_cache = Arc::new(build_conversation_turns(
            &self.messages,
            &self.image_render_cache,
        ));
        self.conversation_turns_dirty = false;
        // WP-B3: one rebuild tagged with input messages consumed, rows produced,
        // and bytes scanned — the amplification WP8/WP9 target.
        crate::chat_hot_counters::record_chat_turn_cache_rebuild(
            input_messages,
            self.conversation_turns_cache.len(),
            bytes_scanned,
        );
        self.sync_turns_list_state();
    }

    pub(super) fn turns_list_is_at_bottom(&self) -> bool {
        let item_count = self.conversation_turns_cache.len();
        if item_count == 0 {
            return true;
        }

        // For bottom-aligned lists, GPUI reports `item_ix == item_count` when the
        // viewport is at the real bottom (logical_scroll_top == None internally).
        let scroll_top = self.turns_list_state.logical_scroll_top();
        if scroll_top.item_ix >= item_count {
            return true;
        }

        // A wheel/momentum scroll can stop a fraction of a pixel short of the
        // exact bottom, which never resets `logical_scroll_top` to None. Treat
        // "within a line of the max offset" as bottom so the jump pill hides
        // and auto-follow resumes where the user visually is at the bottom.
        let current = -f32::from(self.turns_list_state.scroll_px_offset_for_scrollbar().y);
        let max = f32::from(self.turns_list_state.max_offset_for_scrollbar().y);
        scroll_offset_is_at_bottom(current, max, CHAT_SCROLL_BOTTOM_TOLERANCE_PX)
    }

    pub(super) fn apply_scroll_follow_decision(
        &mut self,
        reason: &'static str,
        direction: ChatScrollDirection,
        at_bottom_before: bool,
        at_bottom_after: bool,
        cx: &mut Context<Self>,
    ) {
        let previous_manual_mode = self.user_has_scrolled_up;
        let decision = resolve_chat_scroll_follow_after_scroll(
            previous_manual_mode,
            direction,
            at_bottom_before,
            at_bottom_after,
        );

        tracing::debug!(
            target: "script_kit::chat_scroll",
            event = "follow_state",
            reason,
            direction = ?direction,
            previous_manual_mode,
            next_manual_mode = decision.next_manual_mode,
            at_bottom_before,
            at_bottom_after,
            turn_count = self.conversation_turns_cache.len(),
            scroll_top_item_ix = self.turns_list_state.logical_scroll_top().item_ix,
        );

        if decision.next_manual_mode != previous_manual_mode {
            self.user_has_scrolled_up = decision.next_manual_mode;
            cx.notify();
        }
    }

    pub(super) fn scroll_turns_to_bottom(&mut self) {
        self.ensure_conversation_turns_cache();
        let item_count = self.conversation_turns_cache.len();
        if item_count == 0 {
            self.turns_list_state.set_follow_tail(false);
            return;
        }

        if self.user_has_scrolled_up && self.turns_list_is_at_bottom() {
            tracing::debug!(
                target: "script_kit::chat_scroll",
                event = "resume_auto_follow",
                reason = "already_at_bottom",
                item_count,
            );
            self.user_has_scrolled_up = false;
        }

        self.turns_list_state
            .set_follow_tail(!self.user_has_scrolled_up);
    }

    pub(super) fn force_scroll_turns_to_bottom(&mut self) {
        self.user_has_scrolled_up = false;
        self.ensure_conversation_turns_cache();
        let item_count = self.conversation_turns_cache.len();
        tracing::debug!(
            target: "script_kit::chat_scroll",
            event = "scroll_to_bottom",
            reason = "force",
            item_count,
        );
        self.turns_list_state.set_follow_tail(item_count > 0);
    }

    pub fn add_message(&mut self, message: ChatPromptMessage, cx: &mut Context<Self>) {
        logging::log(
            "CHAT",
            &format!("Adding message: {:?}", message.get_position()),
        );
        self.messages.push(message);
        self.mark_conversation_turns_dirty();
        self.force_scroll_turns_to_bottom();
        cx.notify();
    }

    /// WP-B3: bulk-restore a whole conversation (e.g. Flow history replay) in a
    /// single pass. Extends the message vector, marks the turn cache dirty ONCE,
    /// and rebuilds ONCE — versus calling [`Self::add_message`] per message,
    /// which (through render-time `ensure_conversation_turns_cache`) can rebuild
    /// the whole cache once per restored message.
    pub fn restore_messages(
        &mut self,
        messages: impl IntoIterator<Item = ChatPromptMessage>,
        cx: &mut Context<Self>,
    ) {
        let before = self.messages.len();
        self.messages.extend(messages);
        if self.messages.len() == before {
            return;
        }
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        self.force_scroll_turns_to_bottom();
        cx.notify();
    }

    pub fn start_streaming(
        &mut self,
        message_id: String,
        position: ChatMessagePosition,
        cx: &mut Context<Self>,
    ) {
        let role = match position {
            ChatMessagePosition::Right => Some(ChatMessageRole::User),
            ChatMessagePosition::Left => Some(ChatMessageRole::Assistant),
        };

        let message = ChatPromptMessage {
            id: Some(message_id.clone()),
            role,
            content: Some(String::new()),
            text: String::new(),
            position,
            name: None,
            model: self.model.clone(),
            streaming: true,
            error: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        };
        self.messages.push(message);
        self.streaming_message_id = Some(message_id);
        self.mark_conversation_turns_dirty();
        self.force_scroll_turns_to_bottom();
        cx.notify();
    }

    pub fn append_chunk(&mut self, message_id: &str, chunk: &str, cx: &mut Context<Self>) {
        if self.streaming_message_id.as_deref() == Some(message_id) {
            if let Some(msg) = self
                .messages
                .iter_mut()
                .rev()
                .find(|m| m.id.as_deref() == Some(message_id))
            {
                msg.append_content(chunk);
                self.schedule_stream_flush(cx);
            }
        }
    }

    /// Coalesce external streaming deltas to frame cadence. Rebuilding the
    /// conversation-turns cache costs O(total transcript) per call; codex
    /// threads deliver token-level deltas fast enough that per-delta rebuilds
    /// visibly hitch wheel scrolling. Text accumulates immediately (see
    /// `append_chunk`); the visible flush runs at most once per interval, and
    /// `complete_streaming`/`stop_streaming` flush the tail synchronously.
    pub(super) fn schedule_stream_flush(&mut self, cx: &mut Context<Self>) {
        const STREAM_FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);

        let elapsed = self.last_stream_flush_at.map(|t| t.elapsed());
        if elapsed.is_none_or(|e| e >= STREAM_FLUSH_INTERVAL) {
            self.flush_stream_updates(cx);
            return;
        }
        if self.stream_flush_pending {
            return;
        }
        self.stream_flush_pending = true;
        let delay = STREAM_FLUSH_INTERVAL.saturating_sub(elapsed.unwrap_or_default());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |this, cx| {
                this.stream_flush_pending = false;
                // A completed/stopped stream already flushed its final state.
                if this.streaming_message_id.is_some() {
                    this.flush_stream_updates(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn flush_stream_updates(&mut self, cx: &mut Context<Self>) {
        // WP-B3: one scheduled visible stream flush (the throttled 33 ms clock).
        crate::chat_hot_counters::record_chat_scheduled_flush();
        crate::chat_hot_counters::maybe_log_snapshot("chat_stream_flush");
        self.last_stream_flush_at = Some(std::time::Instant::now());
        self.mark_conversation_turns_dirty();
        self.scroll_turns_to_bottom();
        cx.notify();
    }

    pub fn complete_streaming(&mut self, message_id: &str, cx: &mut Context<Self>) {
        if let Some(msg) = self
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.id.as_deref() == Some(message_id))
        {
            msg.streaming = false;
        }
        if self.streaming_message_id.as_deref() == Some(message_id) {
            self.streaming_message_id = None;
        }
        // WP-B3: terminal flush — forces a final rebuild off the scheduled clock.
        crate::chat_hot_counters::record_chat_terminal_flush();
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        crate::chat_hot_counters::log_snapshot("chat_complete_streaming");
        cx.notify();
    }

    pub fn clear_messages(&mut self, cx: &mut Context<Self>) {
        self.messages.clear();
        self.streaming_message_id = None;
        self.user_has_scrolled_up = false;
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        cx.notify();
    }

    /// Check if currently streaming a response
    pub fn is_streaming(&self) -> bool {
        self.builtin_is_streaming || self.streaming_message_id.is_some()
    }

    /// Stop streaming the current response (preserves partial content)
    /// Triggered by Cmd+. or Escape
    pub fn stop_streaming(&mut self, cx: &mut Context<Self>) {
        logging::log("CHAT", "Stop streaming requested (Cmd+. or Escape)");

        // Flush all accumulated content so the user sees everything received so far
        if let Some(msg_id) = self.streaming_message_id.take() {
            if let Some(msg) = self
                .messages
                .iter_mut()
                .find(|m| m.id.as_deref() == Some(&msg_id))
            {
                if !self.builtin_accumulated_content.is_empty() {
                    msg.set_content(&self.builtin_accumulated_content);
                }
                msg.streaming = false;
            }
        }

        self.builtin_is_streaming = false;
        self.builtin_streaming_content.clear();
        self.builtin_accumulated_content.clear();
        self.builtin_reveal_offset = 0;
        // WP-B3: terminal flush — forces a final rebuild off the scheduled clock.
        crate::chat_hot_counters::record_chat_terminal_flush();
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        crate::chat_hot_counters::log_snapshot("chat_stop_streaming");

        cx.notify();
    }

    /// Get context-aware conversation starters
    /// Shows different suggestions based on clipboard content
    pub(super) fn get_conversation_starters(&self, cx: &Context<Self>) -> Vec<ConversationStarter> {
        let mut starters = default_conversation_starters();

        // Check if clipboard has content - add "Summarize clipboard" if so
        if let Some(clipboard) = cx.read_from_clipboard() {
            if let Some(text) = clipboard.text() {
                if !text.is_empty() && text.len() < 50000 {
                    // Insert clipboard-aware suggestion at position 1
                    starters.insert(
                        1,
                        ConversationStarter::new(
                            "clipboard",
                            "Summarize clipboard",
                            format!("Summarize the following:\n\n{}", text),
                        ),
                    );
                }
            }
        }

        // Limit to 5 suggestions max
        starters.truncate(5);
        starters
    }

    /// Handle clicking a conversation starter
    pub(super) fn select_conversation_starter(
        &mut self,
        starter: &ConversationStarter,
        cx: &mut Context<Self>,
    ) {
        logging::log("CHAT", &format!("Selected starter: {}", starter.id));

        // Insert the prompt into the input
        self.input.clear();
        for ch in starter.prompt.chars() {
            self.input.insert_char(ch);
        }
        self.reset_cursor_blink();
        cx.notify();
    }

    /// Render conversation starters for empty state
    pub(super) fn render_conversation_starters(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let colors = &self.prompt_colors;

        // Hosted surfaces (flow sessions) replace the stock starter chips —
        // canned "Explain this code" suggestions are wrong for an agent
        // identity. Show the agent's own purpose, quietly.
        if let Some(note) = &self.empty_state_note {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .flex_1()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(colors.text_secondary))
                        .max_w(px(480.))
                        .text_center()
                        .child(note.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(colors.text_tertiary))
                        .child("Type a message to start the conversation"),
                )
                .into_any_element();
        }

        let starters = self.get_conversation_starters(cx);

        // Chip styling - use theme-aware overlays
        let chip_bg = theme::hover_overlay_bg(&self.theme, 0x20);
        let chip_hover_bg = theme::hover_overlay_bg(&self.theme, 0x35);

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .gap(px(16.))
            .child(
                div()
                    .text_color(rgb(colors.text_secondary))
                    .text_sm()
                    .child("What can I help you with?"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(8.))
                    .max_w(px(400.))
                    .children(starters.into_iter().enumerate().map(|(i, starter)| {
                        let starter_clone = starter.clone();
                        div()
                            .id(format!("starter-{}", i))
                            .px(px(12.))
                            .py(px(8.))
                            .bg(chip_bg)
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(chip_hover_bg))
                            .text_sm()
                            .text_color(rgb(colors.text_primary))
                            .child(starter.label.clone())
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                this.select_conversation_starter(&starter_clone, cx);
                            }))
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text_tertiary))
                    .mt(px(8.))
                    .child("or type your own question..."),
            )
            .into_any_element()
    }

    /// Set an error on a message (typically on streaming failure)
    pub fn set_message_error(&mut self, message_id: &str, error: String, cx: &mut Context<Self>) {
        if let Some(msg) = self
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.id.as_deref() == Some(message_id))
        {
            msg.error = Some(error);
            msg.streaming = false; // Stop streaming indicator
        }
        if self.streaming_message_id.as_deref() == Some(message_id) {
            self.streaming_message_id = None;
        }
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        cx.notify();
    }

    /// Clear error from a message (before retry)
    pub fn clear_message_error(&mut self, message_id: &str, cx: &mut Context<Self>) {
        if let Some(msg) = self
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.id.as_deref() == Some(message_id))
        {
            msg.error = None;
        }
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        cx.notify();
    }

    /// Handle paste event - check for clipboard images.
    /// Returns true if an image was pasted (caller should not process text).
    pub(super) fn handle_paste_for_image(&mut self, cx: &mut Context<Self>) -> bool {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if let Ok(image_data) = clipboard.get_image() {
                    match crate::clipboard_history::encode_image_as_png(&image_data) {
                        Ok(encoded) => {
                            let base64_data =
                                encoded.strip_prefix("png:").unwrap_or(&encoded).to_string();

                            // Decode to check raw size and build preview
                            use base64::Engine;
                            if let Ok(png_bytes) =
                                base64::engine::general_purpose::STANDARD.decode(&base64_data)
                            {
                                if png_bytes.len() > MAX_IMAGE_BYTES {
                                    tracing::warn!(
                                        size_bytes = png_bytes.len(),
                                        max_bytes = MAX_IMAGE_BYTES,
                                        "Rejecting pasted image larger than 10 MB in ChatPrompt"
                                    );
                                    return false;
                                }
                                if let Ok(render_img) =
                                    crate::list_item::decode_png_to_render_image_with_bgra_conversion(
                                        &png_bytes,
                                    )
                                {
                                    self.pending_image_render = Some(render_img);
                                }
                            }

                            self.pending_image = Some(base64_data);
                            cx.notify();
                            return true;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to encode pasted image");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to access clipboard");
            }
        }
        false
    }

    /// Handle file drop - if it's an image, set it as pending image
    pub(super) fn handle_file_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        use anyhow::Context as _;

        let paths = paths.paths();
        if paths.is_empty() {
            return;
        }

        let path = &paths[0];
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let is_image = matches!(
            extension.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        );

        if !is_image {
            return;
        }

        let process_result = (|| -> anyhow::Result<()> {
            let data = std::fs::read(path)
                .with_context(|| format!("Failed to read dropped image: {}", path.display()))?;

            if data.len() > MAX_IMAGE_BYTES {
                tracing::warn!(
                    path = %path.display(),
                    size_bytes = data.len(),
                    max_bytes = MAX_IMAGE_BYTES,
                    "Skipping dropped image larger than 10MB"
                );
                return Ok(());
            }

            let png_bytes = if extension == "png" {
                data
            } else {
                normalize_to_png_bytes(&data).with_context(|| {
                    format!(
                        "Failed to normalize dropped {} image to PNG: {}",
                        extension,
                        path.display()
                    )
                })?
            };

            use base64::Engine;
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

            if let Ok(render_img) =
                crate::list_item::decode_png_to_render_image_with_bgra_conversion(&png_bytes)
            {
                self.pending_image_render = Some(render_img);
            }

            self.pending_image = Some(base64_data);
            cx.notify();
            Ok(())
        })();

        if let Err(error) = process_result {
            tracing::warn!(
                path = %path.display(),
                extension = %extension,
                error = %error,
                "Failed to process dropped image file"
            );
        }
    }

    /// Launch interactive screen area capture and attach the result as a pending image.
    ///
    /// Runs macOS `screencapture -i` on a background thread. On completion the captured
    /// image is base64-encoded and set as the pending image attachment. Escape cancels
    /// the capture silently.
    pub fn capture_screen_area_attachment(&mut self, cx: &mut Context<Self>) {
        tracing::info!(
            action = "capture_screen_area_start",
            "Starting screen area capture for ChatPrompt attachment"
        );

        cx.spawn(async move |this, cx| {
            let capture_result = cx
                .background_executor()
                .spawn(async { crate::platform::capture_screen_area() })
                .await;

            match capture_result {
                Ok(Some(capture)) => {
                    if capture.png_data.len() > MAX_IMAGE_BYTES {
                        tracing::warn!(
                            size_bytes = capture.png_data.len(),
                            max_bytes = MAX_IMAGE_BYTES,
                            "Rejecting screen capture larger than 10 MB in ChatPrompt"
                        );
                        return;
                    }

                    use base64::Engine;
                    let base64_data =
                        base64::engine::general_purpose::STANDARD.encode(&capture.png_data);
                    let size_kb = capture.png_data.len() / 1024;

                    // Decode to RenderImage for preview
                    let render_img =
                        crate::list_item::decode_png_to_render_image_with_bgra_conversion(
                            &capture.png_data,
                        )
                        .ok();

                    this.update(cx, |this, cx| {
                        if let Some(img) = render_img {
                            this.pending_image_render = Some(img);
                        }
                        this.pending_image = Some(base64_data);
                        tracing::info!(
                            action = "capture_screen_area_attached",
                            width = capture.width,
                            height = capture.height,
                            size_kb = size_kb,
                            "Screen area captured and attached to ChatPrompt"
                        );
                        cx.notify();
                    })
                    .ok();
                }
                Ok(None) => {
                    tracing::info!(
                        action = "capture_screen_area_cancelled",
                        "User cancelled screen area capture"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        action = "capture_screen_area_error",
                        error = %e,
                        "Screen area capture failed"
                    );
                }
            }
        })
        .detach();
    }

    /// Remove the pending image attachment
    pub(super) fn remove_pending_image(&mut self, cx: &mut Context<Self>) {
        self.pending_image = None;
        self.pending_image_render = None;
        cx.notify();
    }

    /// Render the pending image preview badge
    pub(super) fn render_pending_image_preview(&self, cx: &Context<Self>) -> impl IntoElement {
        let render_img = self.pending_image_render.clone();
        let colors = &self.theme.colors;
        let preview_bg = if self.theme.is_dark_mode() {
            theme::hover_overlay_bg(&self.theme, 0x24)
        } else {
            theme::hover_overlay_bg(&self.theme, 0x12)
        };
        let preview_border = rgba((colors.accent.selected << 8) | 0x55);
        let remove_hover_bg = rgba((colors.ui.error << 8) | 0x2d);

        div()
            .id("pending-image-preview")
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(8.0))
            .bg(preview_bg)
            .border_1()
            .border_color(preview_border)
            // Thumbnail
            .when_some(render_img, |d, render_img| {
                d.child(
                    img(move |_window: &mut Window, _cx: &mut App| Some(Ok(render_img.clone())))
                        .w(px(48.))
                        .h(px(48.))
                        .rounded_sm(),
                )
            })
            // Label
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text.primary))
                    .child("Image attached"),
            )
            // Remove button
            .child(
                div()
                    .id("remove-image-btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(move |s| s.bg(remove_hover_bg))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.remove_pending_image(cx);
                        }),
                    )
                    .child(
                        svg()
                            .path(IconName::Close.asset_path())
                            .size(px(10.))
                            .text_color(rgb(colors.text.muted)),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_to_png_bytes;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

    #[test]
    fn test_normalize_to_png_bytes_converts_jpeg_to_png_when_input_is_jpeg() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255])));
        let mut jpeg_cursor = std::io::Cursor::new(Vec::new());
        source
            .write_to(&mut jpeg_cursor, ImageFormat::Jpeg)
            .expect("jpeg encode should succeed");
        let jpeg_bytes = jpeg_cursor.into_inner();

        let png_bytes =
            normalize_to_png_bytes(&jpeg_bytes).expect("jpeg bytes should normalize to png bytes");
        assert!(png_bytes.starts_with(&PNG_SIGNATURE));
        let decoded = image::load_from_memory(&png_bytes).expect("normalized bytes should decode");
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);
    }

    #[test]
    fn test_normalize_to_png_bytes_returns_error_when_input_is_invalid() {
        let result = normalize_to_png_bytes(b"not-an-image");
        assert!(result.is_err());
    }
}
