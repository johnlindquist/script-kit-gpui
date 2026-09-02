use super::*;

impl NotesApp {
    pub(crate) fn capture_dictation_destination(
        &self,
        cx: &App,
    ) -> NotesDictationDestinationSnapshot {
        let editor = self.editor_state.read(cx);
        let content = editor.value().to_string();
        let document_id = if let Some(day) = self.active_day_binding.as_ref() {
            format!("day:{}", day.date)
        } else if let Some(note_id) = self.selected_note_id {
            format!("note:{}", note_id.as_str())
        } else {
            format!("draft:{}", self.instance_id)
        };
        NotesDictationDestinationSnapshot {
            notes_instance_id: self.instance_id,
            document_id,
            editor_generation: format!(
                "notes-v2:{}:{}:{}:{}",
                self.automation_generation.unwrap_or(0),
                self.document_revision,
                self.notes_editor.read(cx).semantic_revision(cx),
                super::ai_handoff::fnv1a64_fingerprint(&content)
            ),
            insertion_anchor: editor.selection(),
        }
    }

    pub(crate) fn inject_dictation_text_into_snapshot(
        &mut self,
        expected: &NotesDictationDestinationSnapshot,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<serde_json::Value, String> {
        if &self.capture_dictation_destination(cx) != expected {
            return Err(
                "stale_destination: Notes instance, document, editor or selection changed"
                    .to_string(),
            );
        }
        if let Some(generation) = self.automation_generation {
            if crate::windows::get_runtime_window_handle_for_generation("notes", generation)
                != Some(window.window_handle())
            {
                return Err("stale_destination: Notes host generation changed".to_string());
            }
        }
        if crate::runtime_policy::is_owned_evaluation()
            && (!self.host_policy.is_hidden()
                || self.automation_generation.is_none()
                || (self.selected_note_id.is_none() && self.active_day_binding.is_none()))
        {
            return Err("stale_destination: owned Notes document is not bound".to_string());
        }
        if text.is_empty() {
            return Err("stale_destination: empty dictation text".to_string());
        }
        if self.view_mode == NotesViewMode::Trash
            || self
                .selected_note_id
                .is_some_and(|id| !self.notes.iter().any(|note| note.id == id))
        {
            return Err("stale_destination: Notes document is not editable".to_string());
        }
        let before = self.editor_text(cx);
        let mut receipt = self
            .inject_dictation_text_at_frozen_anchor(
                text,
                expected.insertion_anchor.clone(),
                window,
                cx,
            )
            .map_err(|error| format!("stale_destination: {error}"))?;
        self.on_editor_change(window, cx);
        let after = self.editor_text(cx);
        let observed = crate::components::notes_editor::observed_replacement_range(
            &before,
            &after,
            expected.insertion_anchor.clone(),
            text,
            self.editor_state.read(cx).selection(),
        )
        .ok_or_else(|| {
            "mutation_failed: Notes observed insertion differs from requested replacement"
                .to_string()
        })?;
        if crate::runtime_policy::is_owned_evaluation() && !self.save_current_note() {
            return Err(
                "mutation_failed: Notes draft changed but canonical save failed".to_string(),
            );
        }
        if crate::runtime_policy::is_owned_evaluation() {
            if let Some(day) = self.active_day_binding.as_ref() {
                let actual = crate::brain::substrate::io::read_private_document(&day.path)
                    .map_err(|error| {
                        format!("mutation_failed: Notes Day readback failed: {error}")
                    })?;
                if actual != after {
                    return Err("mutation_failed: Notes Day canonical content differs".to_string());
                }
            } else if let Some(id) = self.selected_note_id {
                crate::notes::verify_saved_note_content(id, &after).map_err(|error| {
                    format!("mutation_failed: Notes canonical readback failed: {error}")
                })?;
            }
        }
        receipt["start"] = serde_json::json!(observed.start);
        receipt["end"] = serde_json::json!(observed.end);
        receipt["insertedLength"] = serde_json::json!(observed.end - observed.start);
        receipt["observed"] = serde_json::json!(true);
        receipt["saved"] = serde_json::json!(crate::runtime_policy::is_owned_evaluation());
        Ok(receipt)
    }

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
        let date_str = crate::formatting::format_absolute_datetime(self.now());
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

    /// Copy note content as markdown (Cmd+Shift+C).
    pub(super) fn copy_as_markdown(&mut self, cx: &mut Context<Self>) {
        let content = self.editor_state.read(cx).value().to_string();
        self.copy_text_to_clipboard(&content, "Copied", cx);
    }
}
