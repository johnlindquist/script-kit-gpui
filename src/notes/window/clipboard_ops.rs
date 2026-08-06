use super::*;

impl NotesApp {
    /// Insert dictated text at the current cursor position.
    ///
    /// Replaces the current selection (if any) with the dictated text and
    /// moves the cursor to the end of the insertion.  Called by the dictation
    /// delivery pipeline when the notes window was the active target.
    pub(crate) fn inject_dictation_text(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> serde_json::Value {
        let insertion_range = self.editor_state.update(cx, |state, cx| {
            let selection = state.selection();
            let value = state.value().to_string();
            let start = selection.start.min(value.len());
            let end = selection.end.min(value.len());
            let new_value = format!("{}{}{}", &value[..start], text, &value[end..]);
            let new_cursor = start + text.len();
            state.set_value_preserving_scroll(new_value, new_cursor, window, cx);
            serde_json::json!({
                "available": true,
                "unit": "utf8Bytes",
                "start": start,
                "end": new_cursor,
                "replacedStart": start,
                "replacedEnd": end,
                "insertedLength": text.len(),
                "operation": if start == end { "insertAtCursor" } else { "replaceSelection" },
                "source": "notes.inject_dictation_text",
                "redacted": true,
            })
        });
        self.has_unsaved_changes = true;
        tracing::info!(
            category = "DICTATION",
            text_len = text.len(),
            "Dictated text injected into notes editor"
        );
        cx.notify();
        insertion_range
    }

    pub(crate) fn inject_dictation_text_at_frozen_anchor(
        &mut self,
        text: &str,
        anchor: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<serde_json::Value, String> {
        let insertion_range = self.editor_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            if anchor.start > anchor.end
                || anchor.end > value.len()
                || !value.is_char_boundary(anchor.start)
                || !value.is_char_boundary(anchor.end)
            {
                return Err("Frozen Notes insertion anchor is no longer valid".to_string());
            }
            let new_value = format!("{}{}{}", &value[..anchor.start], text, &value[anchor.end..]);
            let new_cursor = anchor.start + text.len();
            state.set_value_preserving_scroll(new_value, new_cursor, window, cx);
            Ok(serde_json::json!({
                "available": true,
                "unit": "utf8Bytes",
                "start": anchor.start,
                "end": new_cursor,
                "replacedStart": anchor.start,
                "replacedEnd": anchor.end,
                "insertedLength": text.len(),
                "operation": if anchor.start == anchor.end { "insertAtFrozenCursor" } else { "replaceFrozenSelection" },
                "source": "notes.inject_dictation_text_at_frozen_anchor",
                "redacted": true,
            }))
        })?;
        self.has_unsaved_changes = true;
        tracing::info!(
            category = "DICTATION",
            text_len = text.len(),
            "Dictated text injected at frozen Notes anchor"
        );
        cx.notify();
        Ok(insertion_range)
    }

    /// Insert current date/time at cursor position (Cmd+Shift+D)
    pub(super) fn insert_date_time(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let date_str = crate::formatting::format_absolute_datetime(chrono::Utc::now());
        self.editor_state.update(cx, |state, cx| {
            let selection = state.selection();
            let value = state.value().to_string();
            let start = selection.start.min(value.len());
            let end = selection.end.min(value.len());
            let new_value = format!("{}{}{}", &value[..start], date_str, &value[end..]);
            let new_cursor = start + date_str.len();
            state.set_value_preserving_scroll(new_value, new_cursor, window, cx);
        });
        self.has_unsaved_changes = true;
        info!("Inserted date/time at cursor");
        cx.notify();
    }

    /// Copy note content as markdown to clipboard (Cmd+Shift+C)
    pub(super) fn copy_as_markdown(&mut self, cx: &mut Context<Self>) {
        let content = self.editor_state.read(cx).value().to_string();
        self.copy_text_to_clipboard(&content);
        self.show_action_feedback("Copied", false);
        cx.notify();
        info!("Copied note as markdown to clipboard");
    }
}
