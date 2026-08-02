use itertools::Itertools;

use super::*;
use gpui::ScrollHandle;

pub(super) fn active_notes_selection_id(
    selected_note_id: Option<NoteId>,
    active_day_date: Option<NaiveDate>,
) -> Option<String> {
    selected_note_id
        .map(|id| id.as_str())
        .or_else(|| active_day_date.map(|date| format!("day:{date}")))
}

impl NotesApp {
    pub(super) fn devtools_text_fingerprint(value: &str) -> String {
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("fnv1a64:{hash:016x}")
    }

    fn automation_shortcut_registry(&self) -> serde_json::Value {
        let active_scope = if self.command_bar.is_open() {
            "actionsPanel"
        } else if self.note_switcher.is_open() {
            "noteSwitcher"
        } else {
            "editor"
        };

        serde_json::json!({
            "schemaVersion": 1,
            "redacted": true,
            "activeScope": active_scope,
            "currentFocusSurface": format!("{:?}", self.current_focus_surface()),
            "pendingFocusSurface": self.pending_focus_surface.map(|surface| format!("{surface:?}")),
            "modalGuard": {
                "activeDialogFirst": true,
                "handles": ["Enter", "Escape", "Tab", "Shift+Tab"],
            },
            "scopes": [
                {
                    "id": "actionsPanel",
                    "open": self.command_bar.is_open(),
                    "owner": "Notes CommandBar",
                    "toggle": "Cmd+K",
                    "handles": ["Escape", "Cmd+K", "Enter", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown", "Backspace", "Delete", "text input"],
                },
                {
                    "id": "noteSwitcher",
                    "open": self.note_switcher.is_open(),
                    "owner": "Notes note switcher",
                    "toggle": "Cmd+P",
                    "handles": ["Escape", "Cmd+P", "Enter", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown", "Backspace", "Delete", "text input"],
                },
                {
                    "id": "editor",
                    "open": true,
                    "owner": "Notes editor",
                    "handles": [
                        "Escape", "Tab", "Shift+Tab", "Alt+Up", "Alt+Down", "Alt+Shift+Up", "Alt+Shift+Down",
                        "Ctrl+Shift+K", "Cmd+Enter", "Cmd+K", "Cmd+Shift+O", "Cmd+P", "Cmd+Shift+P",
                        "Cmd+F", "Cmd+Shift+F", "Cmd+Shift+A", "Cmd+N", "Cmd+Shift+N", "Cmd+Shift+T",
                        "Cmd+W", "Cmd+.", "Cmd+Shift+.", "Cmd+Shift+S", "Cmd+Z", "Cmd+D", "Cmd+Shift+D",
                        "Cmd+Shift+X", "Cmd+L", "Cmd+Shift+L", "Cmd+Shift+-", "Cmd+Shift+H", "Cmd+V",
                        "Cmd+Shift+C", "Cmd+E", "Cmd+J", "Cmd+Shift+U", "Cmd+B", "Cmd+I", "Cmd+Shift+I",
                        "Cmd+Up", "Cmd+Down", "Cmd+Shift+Up", "Cmd+Shift+Down", "Cmd+[", "Cmd+]",
                        "Cmd+Shift+Backspace", "Cmd+Shift+Delete", "Cmd+Shift+7", "Cmd+Shift+8", "Cmd+1..Cmd+9"
                    ],
                },
            ],
        })
    }

    fn automation_focus_transition_timeline(&self) -> serde_json::Value {
        let entries: Vec<serde_json::Value> = self
            .focus_transition_log
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "generation": entry.generation,
                    "phase": entry.phase,
                    "surface": format!("{:?}", entry.surface),
                    "previousSurface": format!("{:?}", entry.previous_surface),
                    "commandBarOpen": entry.command_bar_open,
                    "noteSwitcherOpen": entry.note_switcher_open,
                    "hasActiveDialog": entry.has_active_dialog,
                    "ageMs": entry.recorded_at.elapsed().as_millis() as u64,
                })
            })
            .collect();

        serde_json::json!({
            "schemaVersion": 1,
            "redacted": true,
            "generation": self.focus_transition_generation,
            "entryCount": entries.len(),
            "entries": entries,
        })
    }

    /// Redacted receipt of the most recent notes→main Agent Chat handoff.
    ///
    /// Always present with a stable shape: `active: false` before any
    /// handoff, so probes can assert on the key without existence checks.
    /// Identity/shape only — never note content or selection text.
    fn automation_last_ai_handoff_state(&self) -> serde_json::Value {
        match self.last_ai_handoff.as_ref() {
            None => serde_json::json!({
                "schemaVersion": 1,
                "redacted": true,
                "active": false,
            }),
            Some(receipt) => serde_json::json!({
                "schemaVersion": 1,
                "redacted": true,
                "active": true,
                "generation": receipt.generation,
                "status": receipt.status.as_str(),
                "source": receipt.source,
                "targetSemanticId": receipt.target_semantic_id,
                "targetLabelLength": receipt.target_label_length,
                "targetLabelFingerprint": receipt.target_label_fingerprint,
                "draft": receipt.is_draft,
                "supplementalPartCount": receipt.supplemental_part_count,
                "destinationWindowId": receipt.destination_window_id,
                "destinationSurface": receipt.destination_surface,
                "notesInstanceId": receipt.notes_instance_id,
                "errorCode": receipt.error_code,
                "ageMs": receipt.recorded_at.elapsed().as_millis() as u64,
            }),
        }
    }

    fn automation_ghost_autocomplete_state(&self, cx: &gpui::App) -> serde_json::Value {
        let prediction = self.notes_ghost_prediction.as_ref();
        let last_action = self.notes_ghost_last_action.as_ref().map(|action| {
            serde_json::json!({
                "kind": action.kind,
                "sourceKind": action.source_kind.as_str(),
                "suffixReturned": false,
                "suffixLength": action.suffix_len,
                "insertedLength": action.inserted_len,
                "remainingLength": action.remaining_len,
                "acceptedLength": action.inserted_len,
                "remainingSuffixLength": action.remaining_len,
                "acceptedTextReturned": false,
                "acceptedFingerprint": action.accepted_fingerprint,
                "acceptedLeadingWhitespaceLength": action.accepted_leading_whitespace_len,
                "acceptedNonWhitespaceLength": action.accepted_non_whitespace_len,
                "suffixFingerprint": action.suffix_fingerprint,
                "ageMs": action.recorded_at.elapsed().as_millis() as u64,
            })
        });

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.notes.automationState",
            "redacted": true,
            "visible": prediction.is_some(),
            "kind": prediction.map(|_| "lineSuffix"),
            "sourceKind": prediction.map(|prediction| prediction.source_kind.as_str()),
            "sourceRank": prediction.map(|prediction| prediction.source_rank),
            "acceptsTab": prediction.map(|prediction| prediction.accepts_tab).unwrap_or(false),
            "confidence": prediction.map(|prediction| prediction.confidence),
            "generation": self.notes_ghost_generation,
            "suffixReturned": false,
            "suffixLength": prediction.map(|prediction| prediction.suffix.chars().count()),
            "suffixFingerprint": prediction.map(|prediction| Self::devtools_text_fingerprint(&prediction.suffix)),
            "queryPrefixLength": prediction.map(|prediction| prediction.query_prefix.chars().count()),
            "queryPrefixFingerprint": prediction.map(|prediction| Self::devtools_text_fingerprint(&prediction.query_prefix)),
            // The ghost renders through the editor's native inline completion;
            // this proves the prediction↔render channel stayed in sync.
            "editorInlineCompletionPresent": self.editor_state.read(cx).has_inline_completion(),
            "lastAction": last_action,
        })
    }

    fn automation_line_anchor(
        editor_text: &str,
        selection: &std::ops::Range<usize>,
    ) -> serde_json::Value {
        let cursor = selection.start.min(editor_text.len());
        let line_start = editor_text[..cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = editor_text[cursor..]
            .find('\n')
            .map_or(editor_text.len(), |index| cursor + index);
        let line_text = &editor_text[line_start..line_end];
        let line_index = editor_text[..cursor].matches('\n').count();
        let total_lines = editor_text.lines().count().max(1);

        serde_json::json!({
            "schemaVersion": 1,
            "redacted": true,
            "cursor": cursor,
            "offsetUnit": "utf8ByteOffset",
            "line": {
                "index": line_index,
                "total": total_lines,
                "start": line_start,
                "end": line_end,
                "length": line_text.chars().count(),
                "byteLength": line_text.len(),
                "fingerprint": Self::devtools_text_fingerprint(line_text),
            },
            "selectionRange": [selection.start, selection.end],
            "selectionUnit": "utf8ByteOffset",
            "selectionFingerprint": Self::devtools_text_fingerprint(&format!("{}..{}", selection.start, selection.end)),
        })
    }

    fn automation_draft_snapshot(
        &self,
        editor_text: &str,
        selection: &std::ops::Range<usize>,
        selected_note: Option<&Note>,
        storage_identity: &serde_json::Value,
    ) -> serde_json::Value {
        let note_id = self.selected_note_id.map(|id| id.as_str());
        let storage_generation = storage_identity
            .get("generation")
            .and_then(serde_json::Value::as_u64);

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.notes.automationState",
            "redacted": true,
            "contentReturned": false,
            "titleReturned": false,
            "noteIdFingerprint": note_id.as_deref().map(Self::devtools_text_fingerprint),
            "noteIdLength": note_id.as_deref().map(|id| id.chars().count()),
            "storageGeneration": storage_generation,
            "focusTransitionGeneration": self.focus_transition_generation,
            "dirty": self.has_unsaved_changes,
            "draft": {
                "bodyLength": editor_text.chars().count(),
                "bodyByteLength": editor_text.len(),
                "bodyFingerprint": Self::devtools_text_fingerprint(editor_text),
                "lineCount": editor_text.lines().count().max(1),
                "selectionRange": [selection.start, selection.end],
                "selectionUnit": "utf8ByteOffset",
                "selectionFingerprint": Self::devtools_text_fingerprint(&format!("{}..{}", selection.start, selection.end)),
            },
            "persisted": selected_note.map(|note| serde_json::json!({
                "titleLength": note.title.chars().count(),
                "titleFingerprint": Self::devtools_text_fingerprint(&note.title),
                "contentLength": note.content.chars().count(),
                "contentFingerprint": Self::devtools_text_fingerprint(&note.content),
                "updatedAtMs": note.updated_at.timestamp_millis(),
                "deleted": note.deleted_at.is_some(),
                "isPinned": note.is_pinned,
            })),
        })
    }

    fn automation_editor_anchor(
        &self,
        editor_text: &str,
        selection: &std::ops::Range<usize>,
        editor_scroll_metrics: serde_json::Value,
    ) -> serde_json::Value {
        let anchor = Self::automation_line_anchor(editor_text, selection);
        let scroll_metrics_available = editor_scroll_metrics
            .get("available")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.notes.automationState",
            "redacted": true,
            "available": true,
            "anchor": anchor,
            "scrollMetricsAvailable": scroll_metrics_available,
            "scrollTopAvailable": scroll_metrics_available,
            "scrollHeightAvailable": scroll_metrics_available,
            "clientHeightAvailable": scroll_metrics_available,
            "scroll": editor_scroll_metrics,
            "stopReason": if scroll_metrics_available {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(
                    "Notes editor InputState did not expose runtime scroll offsets".to_string(),
                )
            },
        })
    }

    fn automation_scroll_handle_metrics(
        handle: &ScrollHandle,
        source: &'static str,
    ) -> serde_json::Value {
        let offset = handle.offset();
        let max_offset = handle.max_offset();
        let viewport = handle.bounds().size;
        let max_scroll_top = max_offset.y.as_f32().max(0.0);
        let max_scroll_left = max_offset.x.as_f32().max(0.0);
        let scroll_top = (-offset.y.as_f32()).clamp(0.0, max_scroll_top);
        let scroll_left = (-offset.x.as_f32()).clamp(0.0, max_scroll_left);

        serde_json::json!({
            "schemaVersion": 1,
            "source": source,
            "available": true,
            "offsetUnit": "logicalPx",
            "scrollTop": scroll_top,
            "scrollLeft": scroll_left,
            "rawOffsetX": offset.x.as_f32(),
            "rawOffsetY": offset.y.as_f32(),
            "scrollHeight": viewport.height.as_f32() + max_scroll_top,
            "scrollWidth": viewport.width.as_f32() + max_scroll_left,
            "clientHeight": viewport.height.as_f32(),
            "clientWidth": viewport.width.as_f32(),
            "maxScrollTop": max_scroll_top,
            "maxScrollLeft": max_scroll_left,
            "canScrollY": max_scroll_top > 0.0,
            "canScrollX": max_scroll_left > 0.0,
        })
    }

    fn automation_preview_anchor(
        &self,
        editor_text: &str,
        selection: &std::ops::Range<usize>,
        cx: &gpui::App,
    ) -> serde_json::Value {
        let anchor = Self::automation_line_anchor(editor_text, selection);
        let scroll_metrics = {
            let editor = self.notes_editor.read(cx);
            Self::automation_scroll_handle_metrics(
                editor.preview_scroll_handle(),
                "runtime.components.notes_editor.preview.ScrollHandle",
            )
        };
        let preview_available = self.preview_enabled;

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.notes.automationState",
            "redacted": true,
            "available": preview_available,
            "previewEnabled": self.preview_enabled,
            "owner": crate::components::notes_editor::NOTES_EDITOR_STYLE_OWNER,
            "renderPath": crate::components::notes_editor::NOTES_EDITOR_PREVIEW_RENDER_PATH,
            "scrollSource": "runtime.components.notes_editor.preview.ScrollHandle",
            "anchor": if preview_available { anchor } else { serde_json::Value::Null },
            "scrollMetricsAvailable": preview_available,
            "scrollTopAvailable": preview_available,
            "scrollHeightAvailable": preview_available,
            "clientHeightAvailable": preview_available,
            "scroll": if preview_available { scroll_metrics } else { serde_json::Value::Null },
            "stopReason": if preview_available {
                serde_json::Value::Null
            } else {
                serde_json::Value::String("Notes markdown preview is not mounted".to_string())
            },
        })
    }

    pub(crate) fn automation_state(&self, cx: &gpui::App) -> serde_json::Value {
        let editor = self.editor_state.read(cx);
        let editor_text = editor.value().to_string();
        let selection = editor.selection();
        let selected_note = self
            .selected_note_id
            .and_then(|id| self.get_visible_notes().iter().find(|note| note.id == id));
        let selected_note_metadata = selected_note.map(|note| {
            let tags = storage::get_note_tags(note.id).unwrap_or_default();
            let aliases = storage::get_note_aliases(note.id).unwrap_or_default();
            let outbound_link_count = storage::get_note_outbound_link_count(note.id).unwrap_or(0);
            let backlink_count = storage::get_note_backlink_count(note.id).unwrap_or(0);
            serde_json::json!({
                "tags": tags.iter().take(8).cloned().collect::<Vec<_>>(),
                "aliases": aliases.iter().take(8).cloned().collect::<Vec<_>>(),
                "tagCount": tags.len(),
                "aliasCount": aliases.len(),
                "outboundLinkCount": outbound_link_count,
                "backlinkCount": backlink_count,
            })
        });
        let note_position = self.get_note_position();
        let storage_identity = storage::automation_storage_identity();
        let cursor = selection.start.min(editor_text.len());
        let cursor_line = (!editor_text.is_empty()).then(|| {
            (
                editor_text[..cursor].matches('\n').count() + 1,
                editor_text.lines().count().max(1),
            )
        });
        let search_text = self.search_state.read(cx).value().to_string();
        let metrics = style::adopted_metrics();
        let desired_height = Self::autosize_desired_height(&metrics, self.last_line_count);
        let clamped_height = Self::resolve_auto_resize_height(
            desired_height,
            self.initial_height,
            metrics.auto_resize_max_height,
        );
        let last_autosize_transition = self.last_autosize_transition.as_ref().map(|entry| {
            serde_json::json!({
                "generation": entry.generation,
                "cause": entry.cause,
                "beforeHeight": entry.before_height,
                "afterHeight": entry.after_height,
                "beforeWidth": entry.before_width,
                "afterWidth": entry.after_width,
                "lineCount": entry.line_count,
                "desiredHeight": entry.desired_height,
                "clampedHeight": entry.clamped_height,
                "applied": entry.applied,
                "skippedReason": entry.skipped_reason,
                "ageMs": entry.recorded_at.elapsed().as_millis() as u64,
            })
        });
        let footer_hit_regions = self
            .entry_reveal
            .native_window_number
            .and_then(crate::platform::footer_hit_regions::snapshot_for_native_window_number)
            .map(|snapshot| {
                serde_json::json!({
                    "coordinateSpace": "gpuiWindowLogicalPxYDown",
                    "nativeWindowNumber": snapshot.native_window_number,
                    "windowSize": {
                        "width": snapshot.window_width,
                        "height": snapshot.window_height,
                    },
                    "layoutGeneration": snapshot.layout_generation,
                    "regions": snapshot.regions.iter().map(|region| serde_json::json!({
                        "group": region.group,
                        "index": region.index,
                        "elementId": format!("glass-capsule-{}-{}", region.group, region.index),
                        "bounds": {
                            "x": region.x,
                            "y": region.y,
                            "width": region.width,
                            "height": region.height,
                        },
                    })).collect::<Vec<_>>(),
                })
            });
        let bottom_resize = self.last_bottom_resize_receipt.as_ref().map(|receipt| {
            let protected_region = match &receipt.route {
                resize::NotesBottomResizeRoute::ProtectedFooterButton { group, index } => {
                    Some(serde_json::json!({"group": group, "index": index}))
                }
                _ => None,
            };
            serde_json::json!({
                "route": receipt.route.as_str(),
                "position": {"x": receipt.x, "y": receipt.y},
                "beforeSize": {
                    "width": receipt.before_width,
                    "height": receipt.before_height,
                },
                "afterSize": {
                    "width": receipt.after_width,
                    "height": receipt.after_height,
                },
                "footerLayoutGeneration": receipt.footer_layout_generation,
                "nativeWindowNumber": receipt.native_window_number,
                "protectedRegion": protected_region,
                "ageMs": receipt.recorded_at.elapsed().as_millis() as u64,
            })
        });

        serde_json::json!({
            "schemaVersion": 1,
            "passive": true,
            "redacted": true,
            "activeNoteId": active_notes_selection_id(
                self.selected_note_id,
                self.active_day_binding.as_ref().map(|binding| binding.date),
            ),
            "dirtyState": {
                "hasUnsavedChanges": self.has_unsaved_changes,
                "lastSaveConfirmedMsAgo": self.last_save_confirmed.map(|instant| instant.elapsed().as_millis() as u64),
                "lastSaveAttemptMsAgo": self.last_save_time.map(|instant| instant.elapsed().as_millis() as u64),
            },
            "selectedNote": selected_note.map(|note| serde_json::json!({
                "id": note.id.as_str(),
                "titleLength": note.title.chars().count(),
                "titleFingerprint": Self::devtools_text_fingerprint(&note.title),
                "contentLength": note.content.chars().count(),
                "contentFingerprint": Self::devtools_text_fingerprint(&note.content),
                "isPinned": note.is_pinned,
                "deleted": note.deleted_at.is_some(),
                "metadata": selected_note_metadata,
                "position": note_position.map(|(position, total)| serde_json::json!({
                    "index": position,
                    "total": total,
                })),
            })),
            "editor": {
                "textLength": editor_text.chars().count(),
                "textFingerprint": Self::devtools_text_fingerprint(&editor_text),
                "selectionRange": [selection.start, selection.end],
                "selectionLength": selection.end.saturating_sub(selection.start),
                "hasSelection": selection.start != selection.end,
                "cursor": cursor,
                "cursorLine": cursor_line.map(|(line, total)| serde_json::json!({
                    "line": line,
                    "total": total,
                })),
                "lastLineCount": self.last_line_count,
            },
            "draftSnapshot": self.automation_draft_snapshot(
                &editor_text,
                &selection,
                selected_note,
                &storage_identity,
            ),
            "editorAnchor": self.automation_editor_anchor(
                &editor_text,
                &selection,
                editor.automation_scroll_metrics(),
            ),
            "previewAnchor": self.automation_preview_anchor(&editor_text, &selection, cx),
            "kitResourcePreview": self.automation_kit_resource_preview_state(),
            "deeplinkHoverHint": self.last_deeplink_hover_hint.clone(),
            "view": {
                "viewMode": format!("{:?}", self.view_mode),
                "focusSurface": format!("{:?}", self.current_focus_surface()),
                "focusMode": self.focus_mode,
                "sortMode": format!("{:?}", self.sort_mode),
                "showSearch": self.show_search,
                "showFormatToolbar": self.show_format_toolbar,
                "previewEnabled": self.preview_enabled,
                "showActionsPanel": self.command_bar.is_open(),
                "showBrowsePanel": self.note_switcher.is_open(),
                "autoSizingEnabled": self.auto_sizing_enabled,
                "initialHeight": self.initial_height,
                "lastWindowHeight": self.last_window_height,
            },
            "lastAiHandoff": self.automation_last_ai_handoff_state(),
            "entryReveal": {
                "schemaVersion": 1,
                "instanceId": self.entry_reveal.instance_id,
                "generation": self.entry_reveal.generation,
                "phase": self.entry_reveal.phase.as_str(),
                "bodyVisible": self.entry_reveal.body_visible,
                "nativeWindowNumber": self.entry_reveal.native_window_number,
                "nativeConfigured": self.entry_reveal.native_configured,
                "backdropFoundOrCreated": self.entry_reveal.backdrop_found_or_created,
                "nativeSelectorsSupported": self.entry_reveal.native_selectors_supported,
                "styleApplied": self.entry_reveal.style_applied,
                "fallbackUsed": self.entry_reveal.fallback_used,
                "nativeStyleSignature": self.entry_reveal.native_style_signature,
                "configuredAtMonotonicNs": self.entry_reveal.configured_at_monotonic_ns,
                "nativeConfiguredAtUnixMs": self.entry_reveal.native_configured_at_unix_ms,
                "settleDurationMs": self.entry_reveal.settle_duration_ms,
                "revealDelayMs": self.entry_reveal.reveal_delay_ms,
                "morphStarted": self.entry_reveal.morph_started,
                "morphStartAlphaBits": self.entry_reveal.morph_start_alpha_bits,
                "completedFrameCount": self.entry_reveal.completed_frame_count,
                "firstFrameAtMonotonicNs": self.entry_reveal.first_frame_at_monotonic_ns,
                "revealAnchorAtMonotonicNs": self.entry_reveal.reveal_anchor_at_monotonic_ns,
                "revealRequestedAtMonotonicNs": self.entry_reveal.reveal_requested_at_monotonic_ns,
                "visibleAtMonotonicNs": self.entry_reveal.visible_at_monotonic_ns,
            },
            "footerHitRegions": footer_hit_regions,
            "bottomResize": bottom_resize,
            "windowLifecycle": {
                "schemaVersion": 2,
                "phase": if NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .is_some()
                {
                    "Exiting"
                } else {
                    "Open"
                },
                "hasExitTicket": NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .is_some(),
                "exitGeneration": NOTES_EXIT_TICKET
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .as_ref()
                    .copied()
                    .map(crate::platform::GlassExitTicket::generation),
                "nativeExit": crate::platform::glass_exit_lifecycle_receipt("Notes"),
            },
            "autosize": {
                "schemaVersion": 1,
                "redacted": true,
                "generation": self.autosize_generation,
                "enabled": self.auto_sizing_enabled,
                "lastWindowHeight": self.last_window_height,
                "initialHeight": self.initial_height,
                "minHeight": self.initial_height,
                "maxHeight": metrics.auto_resize_max_height,
                "lineCount": self.last_line_count,
                "desiredHeight": desired_height,
                "clampedHeight": clamped_height,
                "threshold": metrics.auto_resize_threshold,
                "lastCause": last_autosize_transition
                    .as_ref()
                    .and_then(|entry| entry.get("cause"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown"),
                "lastAppliedHeight": last_autosize_transition
                    .as_ref()
                    .and_then(|entry| entry.get("afterHeight"))
                    .and_then(serde_json::Value::as_f64),
                "lastAppliedAt": last_autosize_transition
                    .as_ref()
                    .map(|_| "runtime-relative"),
            },
            "generations": {
                "schemaVersion": 1,
                "state": storage_identity
                    .get("generation")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                "editorText": Self::devtools_text_fingerprint(&editor_text),
                "autosize": self.autosize_generation,
                "storage": storage_identity.get("generation").and_then(serde_json::Value::as_u64),
                "focus": self.focus_transition_generation,
                "target": serde_json::Value::Null,
                "surface": serde_json::Value::Null,
            },
            "lastAutosizeTransition": last_autosize_transition,
            "commandBars": {
                "actions": self.command_bar.automation_state("notes.actions", cx),
                "noteSwitcher": self.note_switcher.automation_state("notes.switcher", cx),
            },
            "spine": self.automation_notes_spine_state(cx),
            "shortcutRegistry": self.automation_shortcut_registry(),
            "focusTransitions": self.automation_focus_transition_timeline(),
            "ghostAutocomplete": self.automation_ghost_autocomplete_state(cx),
            "search": {
                "visible": self.show_search,
                "queryLength": search_text.chars().count(),
                "queryFingerprint": Self::devtools_text_fingerprint(&search_text),
            },
            "storage": storage_identity,
            "counts": {
                "notes": self.notes.len(),
                "deletedNotes": self.deleted_notes.len(),
                "visibleNotes": self.get_visible_notes().len(),
                "historyBack": self.history_back.len(),
                "historyForward": self.history_forward.len(),
            },
        })
    }

    fn automation_kit_resource_preview_state(&self) -> serde_json::Value {
        match self.kit_resource_preview.as_ref() {
            Some(preview) => {
                let mut footer_hints = Vec::new();
                if self.kit_resource_preview_note_source().is_some() {
                    footer_hints.push(serde_json::json!({
                        "actionId": super::render_ui::NOTES_RESOURCE_HINT_OPEN_SOURCE_ID,
                        "interactive": true,
                    }));
                }
                footer_hints.push(serde_json::json!({
                    "actionId": super::render_ui::NOTES_RESOURCE_HINT_COPY_URI_ID,
                    "interactive": true,
                }));
                footer_hints.push(serde_json::json!({
                    "actionId": super::render_ui::NOTES_RESOURCE_HINT_BACK_ID,
                    "interactive": true,
                }));

                serde_json::json!({
                    "schemaVersion": 1,
                    "active": true,
                    "redacted": true,
                    "title": preview.title,
                    "uri": preview.uri,
                    "mimeType": preview.mime_type,
                    "readOnly": true,
                    "truncated": preview.truncated,
                    "textLength": preview.text.chars().count(),
                    "footerHints": footer_hints,
                })
            }
            None => serde_json::json!({
                "schemaVersion": 1,
                "active": false,
                "redacted": true,
                "footerHints": [],
            }),
        }
    }

    fn automation_notes_spine_state(&self, cx: &gpui::App) -> serde_json::Value {
        let contract =
            crate::components::notes_editor::spine::NotesEditorHostSpineContract::notes();
        let current_key = self.notes_spine_input(cx).map(|input| input.key);
        let active = current_key
            .as_deref()
            .is_some_and(|key| self.spine_runtime.dismissed_cache_key.as_deref() != Some(key));
        let cache_matches = current_key.as_deref() == Some(self.spine_runtime.cache_key.as_str());
        let row_ids = if active && cache_matches {
            self.spine_runtime
                .grouped_cache
                .iter()
                .filter_map(|grouped| {
                    let crate::list_item::GroupedListItem::Item(flat_idx) = grouped else {
                        return None;
                    };
                    self.spine_runtime
                        .flat_cache
                        .get(*flat_idx)
                        .map(|row| row.id.to_string())
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let selected_semantic_id = if active && cache_matches {
            crate::list_item::coerce_selection(
                &self.spine_runtime.grouped_cache,
                self.spine_runtime.selected_index,
            )
            .and_then(|selected| {
                match self.spine_runtime.grouped_cache.get(selected) {
                    Some(crate::list_item::GroupedListItem::Item(flat_idx)) => self
                        .spine_runtime
                        .flat_cache
                        .get(*flat_idx)
                        .map(|row| row.id.to_string()),
                    _ => None,
                }
            })
        } else {
            None
        };

        serde_json::json!({
            "schemaVersion": 1,
            "source": "runtime.notes.spine",
            "redacted": true,
            "owner": "components.notes_editor",
            "contractSurface": contract.surface,
            "renderPath": contract.local_overlay_render_path(),
            "overlayElementId": contract.local_overlay_id(),
            "contextMentionsLocalOverlay": false,
            "active": active,
            "cacheKeyPresent": !self.spine_runtime.cache_key.is_empty(),
            "cacheMatchesInput": cache_matches,
            "dismissed": current_key
                .as_deref()
                .is_some_and(|key| self.spine_runtime.dismissed_cache_key.as_deref() == Some(key)),
            "rowCount": row_ids.len(),
            "selectedSemanticId": selected_semantic_id,
            "rowSemanticIds": row_ids,
        })
    }

    pub(crate) fn automation_layout_info(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
    ) -> crate::protocol::LayoutInfo {
        use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
        use crate::ui::chrome as chrome_tokens;

        let (window_width, window_height) = target
            .bounds
            .as_ref()
            .map(|bounds| (bounds.width as f32, bounds.height as f32))
            .unwrap_or((728.0, self.last_window_height.max(self.initial_height)));
        let metrics = style::adopted_metrics();
        let titlebar_height = metrics.titlebar_height;
        // The Notes content stage always fills the Notes window: no footer
        // band, no desktop gutter, no partitioned stage.
        let stage_height = window_height;
        let content_parent = "NotesWindow";
        let search_height = if self.show_search { 40.0 } else { 0.0 };
        let toolbar_height = if self.show_format_toolbar { 36.0 } else { 0.0 };
        let content_top = titlebar_height + search_height + toolbar_height;
        let editor_height = (stage_height - content_top).max(0.0);
        let mut components = Vec::new();

        components.push(
            LayoutComponentInfo::new("NotesWindow", LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, window_width, window_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    Some(chrome_tokens::LIQUID_GLASS_WINDOW_RADIUS_PX),
                )
                .with_visual_token("chrome.notesWindow")
                .with_flex_column()
                .with_depth(0)
                .with_explanation(
                    "Floating Notes window root measured from the resolved target bounds.",
                ),
        );
        components.push(
            LayoutComponentInfo::new("NotesTitlebar", LayoutComponentType::Header)
                .with_bounds(0.0, 0.0, window_width, titlebar_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                )
                .with_visual_token("chrome.notesTitlebar")
                .with_depth(1)
                .with_parent(content_parent)
                .with_explanation(
                    "Titlebar area that hosts note title and hover-revealed controls.",
                ),
        );

        if self.show_search {
            components.push(
                LayoutComponentInfo::new("NotesSearchBar", LayoutComponentType::Input)
                    .with_bounds(0.0, titlebar_height, window_width, search_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_CONTROL_RADIUS_PX),
                    )
                    .with_visual_token("chrome.notesSearch")
                    .with_depth(1)
                    .with_parent(content_parent)
                    .with_explanation("Editor find/search row shown by Cmd+F."),
            );
        }

        if self.show_format_toolbar {
            components.push(
                LayoutComponentInfo::new("NotesFormatToolbar", LayoutComponentType::Panel)
                    .with_bounds(
                        0.0,
                        titlebar_height + search_height,
                        window_width,
                        toolbar_height,
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                    )
                    .with_visual_token("chrome.notesFormatToolbar")
                    .with_depth(1)
                    .with_parent(content_parent)
                    .with_explanation(
                        "Formatting toolbar shown for rich-text and markdown actions.",
                    ),
            );
        }

        let editor_name = if self.preview_enabled {
            "NotesPreview"
        } else {
            "NotesEditor"
        };
        components.push(
            LayoutComponentInfo::new(editor_name, LayoutComponentType::Prompt)
                .with_bounds(0.0, content_top, window_width, editor_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_CONTENT,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    None,
                )
                .with_visual_token("content.notesEditor")
                .with_flex_column()
                .with_flex_grow(1.0)
                .with_depth(1)
                .with_parent(content_parent)
                .with_explanation(
                    "Primary Notes content region after titlebar/search/toolbar reservations.",
                ),
        );

        if self.command_bar.is_open() {
            components.push(
                LayoutComponentInfo::new("NotesActionsPanel", LayoutComponentType::Panel)
                    .with_bounds(
                        16.0,
                        (titlebar_height + super::ACTIONS_PANEL_TOP_OFFSET).min(window_height),
                        (window_width - 32.0).max(0.0),
                        super::ACTIONS_PANEL_WINDOW_MARGIN.min(window_height),
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FLOATING,
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("chrome.notesActionsPanel")
                    .with_depth(2)
                    .with_parent("NotesWindow")
                    .with_explanation("Notes actions command surface anchored under the titlebar."),
            );
        }

        if self.note_switcher.is_open() {
            components.push(
                LayoutComponentInfo::new("NotesBrowsePanel", LayoutComponentType::Panel)
                    .with_bounds(
                        ((window_width - super::BROWSE_PANEL_WIDTH) / 2.0).max(0.0),
                        titlebar_height,
                        super::BROWSE_PANEL_WIDTH.min(window_width),
                        super::BROWSE_PANEL_MAX_HEIGHT
                            .min((window_height - titlebar_height).max(0.0)),
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FLOATING,
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("chrome.notesBrowsePanel")
                    .with_depth(2)
                    .with_parent("NotesWindow")
                    .with_explanation("Notes switcher/browse panel overlay."),
            );
        }

        LayoutInfo {
            window_width,
            window_height,
            prompt_type: "notes".to_string(),
            components,
            fidelity: None,
            handler_form: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn byte_offset_to_char_index(text: &str, byte_offset: usize) -> usize {
        text[..byte_offset.min(text.len())].chars().count()
    }

    fn char_index_to_byte_offset(text: &str, char_index: usize) -> usize {
        text.char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(text.len())
    }

    fn char_range_to_byte_range(
        text: &str,
        range: std::ops::Range<usize>,
    ) -> std::ops::Range<usize> {
        Self::char_index_to_byte_offset(text, range.start)
            ..Self::char_index_to_byte_offset(text, range.end)
    }

    fn note_portal_query_from_token(token: &str) -> Option<String> {
        let (prefix, value) = crate::ai::context_mentions::typed_mention_token_parts(token)?;
        (prefix == "note").then_some(value)
    }

    pub(super) fn focused_note_inline_token_span(
        &self,
        cx: &Context<Self>,
    ) -> Option<crate::ai::context_mentions::InlineTokenSpan> {
        let editor = self.editor_state.read(cx);
        let value = editor.value().to_string();
        let cursor_char = Self::byte_offset_to_char_index(&value, editor.cursor());
        crate::ai::context_mentions::inline_token_at_cursor(&value, cursor_char)
    }

    pub(super) fn focused_note_mention_preview(
        &self,
        cx: &Context<Self>,
    ) -> Option<(String, String)> {
        let span = self.focused_note_inline_token_span(cx)?;
        let detail = if let Some(query) = Self::note_portal_query_from_token(&span.token) {
            if query.trim().is_empty() {
                "notes portal • Cmd+Shift+O replace".to_string()
            } else {
                format!(
                    "notes portal for \"{}\" • Cmd+Shift+O replace",
                    query.trim()
                )
            }
        } else if let Some((prefix, value)) =
            crate::ai::context_mentions::typed_mention_token_parts(&span.token)
        {
            if value.trim().is_empty() {
                format!("@{prefix} token • open in Agent Chat to replace")
            } else {
                format!(
                    "@{prefix} \"{}\" • open in Agent Chat to replace",
                    value.trim()
                )
            }
        } else {
            "Agent token • open in Agent Chat to replace".to_string()
        };

        Some((span.token, detail))
    }

    pub(super) fn open_focused_note_mention_portal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(span) = self.focused_note_inline_token_span(cx) else {
            return false;
        };
        let Some(query) = Self::note_portal_query_from_token(&span.token) else {
            return false;
        };

        let value = self.editor_state.read(cx).value().to_string();
        self.mention_portal_edit = Some(NotesMentionPortalEditSession {
            mention_range: Self::char_range_to_byte_range(&value, span.range),
            original_token: span.token,
        });
        self.open_browse_panel(window, cx);
        if let Some(dialog) = self.note_switcher.dialog().cloned() {
            dialog.update(cx, |d, _cx| {
                d.set_context_title(Some("Replace @note".to_string()));
            });
            crate::actions::set_actions_dialog_search_text(&dialog, query, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn replace_active_note_mention_with_note(
        &mut self,
        id: NoteId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(edit) = self.mention_portal_edit.take() else {
            return false;
        };
        let Some(note) = self.notes.iter().find(|note| note.id == id) else {
            self.show_selected_note_missing_feedback("replace_active_note_mention_with_note", cx);
            self.close_browse_panel(window, cx);
            return true;
        };

        let title = if note.title.trim().is_empty() {
            "Untitled Note"
        } else {
            note.title.trim()
        };
        let token = crate::ai::context_mentions::format_typed_label_mention_token("note", title);
        let current_value = self.editor_state.read(cx).value().to_string();
        let suffix = &current_value[edit.mention_range.end.min(current_value.len())..];
        let needs_space = suffix
            .chars()
            .next()
            .map(|ch| !ch.is_whitespace() && !matches!(ch, ',' | '.' | ';' | ':' | ')' | ']' | '}'))
            .unwrap_or(false);
        let replacement = if needs_space {
            format!("{token} ")
        } else {
            token.clone()
        };
        let next_value = format!(
            "{}{}{}",
            &current_value[..edit.mention_range.start.min(current_value.len())],
            replacement,
            suffix,
        );
        let next_cursor = edit.mention_range.start + replacement.len();

        tracing::info!(
            target: "script_kit::notes",
            event = "notes_mention_portal_replaced",
            old_token = %edit.original_token,
            new_token = %token,
            note_id = %id.as_str(),
        );

        self.editor_state.update(cx, |state, cx| {
            state.set_value_preserving_scroll(next_value, next_cursor, window, cx);
        });
        self.close_browse_panel(window, cx);
        cx.notify();
        true
    }

    /// Get filtered notes based on search query
    pub(super) fn get_visible_notes(&self) -> &[Note] {
        match self.view_mode {
            NotesViewMode::AllNotes => &self.notes,
            NotesViewMode::Trash => &self.deleted_notes,
        }
    }

    /// Get the character count of the current note
    pub(super) fn get_character_count(&self, cx: &Context<Self>) -> usize {
        self.editor_state.read(cx).value().chars().count()
    }

    /// Get the word count of the current note
    pub(super) fn get_word_count(&self, cx: &Context<Self>) -> usize {
        self.editor_state
            .read(cx)
            .value()
            .split_whitespace()
            .count()
    }

    /// Get the 1-based index position of the current note in the visible list
    /// Returns (current_position, total_count) or None if no note selected
    pub(super) fn get_note_position(&self) -> Option<(usize, usize)> {
        let notes = self.get_visible_notes();
        let total = notes.len();
        if total == 0 {
            return None;
        }
        self.selected_note_id.and_then(|id| {
            notes
                .iter()
                .position(|n| n.id == id)
                .map(|idx| (idx + 1, total))
        })
    }

    /// Get the 1-based line number at cursor position, plus total line count
    pub(super) fn get_cursor_line_info(&self, cx: &Context<Self>) -> Option<(usize, usize)> {
        let value = self.editor_state.read(cx).value().to_string();
        if value.is_empty() {
            return None;
        }
        let selection = self.editor_state.read(cx).selection();
        let cursor = selection.start.min(value.len());
        let current_line = value[..cursor].matches('\n').count() + 1;
        // Count lines the same way as `current_line` (newlines + 1) so a cursor
        // on the trailing empty line after a final '\n' can't read as "Ln 22/21".
        // `str::lines()` drops that trailing empty line, causing the off-by-one.
        let total_lines = (value.matches('\n').count() + 1).max(current_line);
        Some((current_line, total_lines))
    }

    /// Check if the currently selected note is pinned
    pub(super) fn is_current_note_pinned(&self) -> bool {
        self.selected_note_id
            .and_then(|id| self.get_visible_notes().iter().find(|n| n.id == id))
            .map(|n| n.is_pinned)
            .unwrap_or(false)
    }

    /// Navigate to the previous note in the list
    pub(super) fn select_prev_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notes = self.get_visible_notes();
        if notes.is_empty() {
            return;
        }
        if let Some(id) = self.selected_note_id {
            if let Some(idx) = notes.iter().position(|n| n.id == id) {
                if idx > 0 {
                    let prev_id = notes[idx - 1].id;
                    self.select_note(prev_id, window, cx);
                }
            }
        }
    }

    /// Navigate to the next note in the list
    pub(super) fn select_next_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notes = self.get_visible_notes();
        if notes.is_empty() {
            return;
        }
        if let Some(id) = self.selected_note_id {
            if let Some(idx) = notes.iter().position(|n| n.id == id) {
                if idx + 1 < notes.len() {
                    let next_id = notes[idx + 1].id;
                    self.select_note(next_id, window, cx);
                }
            }
        }
    }

    /// Jump to the first note in the list (Cmd+Shift+Up)
    pub(super) fn select_first_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notes = self.get_visible_notes();
        if let Some(note) = notes.first() {
            let id = note.id;
            self.select_note(id, window, cx);
        }
    }

    /// Jump to the last note in the list (Cmd+Shift+Down)
    pub(super) fn select_last_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notes = self.get_visible_notes();
        if let Some(note) = notes.last() {
            let id = note.id;
            self.select_note(id, window, cx);
        }
    }

    /// Navigate back in history (Cmd+[)
    pub(super) fn navigate_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prev_id) = self.history_back.pop() {
            // Only navigate if the note still exists
            if self.notes.iter().any(|n| n.id == prev_id) {
                // Push current note onto forward stack
                if let Some(current_id) = self.selected_note_id {
                    self.history_forward.push(current_id);
                }
                self.navigating_history = true;
                self.select_note(prev_id, window, cx);
                self.navigating_history = false;
            }
        }
    }

    /// Navigate forward in history (Cmd+])
    pub(super) fn navigate_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next_id) = self.history_forward.pop() {
            // Only navigate if the note still exists
            if self.notes.iter().any(|n| n.id == next_id) {
                // Push current note onto back stack
                if let Some(current_id) = self.selected_note_id {
                    self.history_back.push(current_id);
                }
                self.navigating_history = true;
                self.select_note(next_id, window, cx);
                self.navigating_history = false;
            }
        }
    }

    /// Toggle pin state of the currently selected note (Cmd+Shift+I)
    pub(super) fn toggle_pin_current_note(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_note_id {
            let mut was_pinned = false;
            if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                note.is_pinned = !note.is_pinned;
                let pinned = note.is_pinned;
                was_pinned = pinned;
                if let Err(e) = storage::save_note(note) {
                    tracing::error!(error = %e, "Failed to toggle pin state");
                    return;
                }
                info!(note_id = %id, pinned = pinned, "Toggled pin state");
            }
            // Re-sort notes: pinned first, then by updated_at descending
            self.notes.sort_by(|a, b| match (a.is_pinned, b.is_pinned) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.updated_at.cmp(&a.updated_at),
            });
            self.show_action_feedback(if was_pinned { "● Pinned" } else { "Unpinned" }, was_pinned);
            cx.notify();
        }
    }

    /// Get relative time description for when a note was last updated
    pub(super) fn get_relative_time(&self) -> Option<String> {
        self.selected_note_id
            .and_then(|id| self.get_visible_notes().iter().find(|n| n.id == id))
            .map(|note| crate::formatting::format_relative_time_short_dt(note.updated_at))
    }

    /// Select a pinned note by its ordinal position (Cmd+1 through Cmd+9)
    pub(super) fn select_pinned_note_by_index(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pinned_notes: Vec<NoteId> = self
            .notes
            .iter()
            .filter(|n| n.is_pinned)
            .map(|n| n.id)
            .collect();

        if let Some(&note_id) = pinned_notes.get(index) {
            self.select_note(note_id, window, cx);
        }
    }

    /// Toggle focus mode (Cmd+.) — hides titlebar icons, footer, toolbar for distraction-free writing
    pub(super) fn toggle_focus_mode(&mut self, cx: &mut Context<Self>) {
        self.focus_mode = !self.focus_mode;
        if self.focus_mode {
            // Also hide search and formatting toolbar in focus mode
            self.show_search = false;
            self.show_format_toolbar = false;
        }
        info!(focus_mode = self.focus_mode, "Toggled focus mode");
        cx.notify();
    }

    /// Get estimated reading time in minutes based on word count (200 wpm average)
    pub(super) fn get_reading_time(&self, cx: &Context<Self>) -> String {
        let words = self.get_word_count(cx);
        if words < 30 {
            return String::new(); // Too short for meaningful estimate
        }
        let minutes = (words as f64 / 200.0).ceil() as usize;
        if minutes <= 1 {
            "~1 min read".to_string()
        } else {
            format!("~{} min read", minutes)
        }
    }

    /// Get the selected text range stats, if any text is selected
    /// Returns (selected_words, selected_chars) or None if no selection
    pub(super) fn get_selection_stats(&self, cx: &Context<Self>) -> Option<(usize, usize)> {
        let selection = self.editor_state.read(cx).selection();
        if selection.start == selection.end {
            return None;
        }
        let value = self.editor_state.read(cx).value().to_string();
        let start = selection.start.min(value.len());
        let end = selection.end.min(value.len());
        let selected_text = &value[start..end];
        let words = selected_text.split_whitespace().count();
        let chars = selected_text.chars().count();
        if chars == 0 {
            return None;
        }
        Some((words, chars))
    }

    /// Format a DateTime as a relative time string for the note switcher
    pub(super) fn format_relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
        crate::formatting::format_relative_time_short_dt(dt)
    }

    pub(super) fn note_switcher_preview(note: &Note) -> String {
        let tags = storage::get_note_tags(note.id).unwrap_or_default();
        let outbound_link_count = storage::get_note_outbound_link_count(note.id).unwrap_or(0);
        let backlink_count = storage::get_note_backlink_count(note.id).unwrap_or(0);
        let preview = Self::strip_markdown_for_preview(&note.preview());
        Self::note_switcher_preview_from_metadata(
            &preview,
            &tags,
            outbound_link_count,
            backlink_count,
        )
    }

    pub(super) fn note_switcher_preview_from_metadata(
        preview: &str,
        tags: &[String],
        outbound_link_count: usize,
        backlink_count: usize,
    ) -> String {
        let mut metadata_parts = Vec::new();
        let tag_count = tags.len();
        metadata_parts.extend(tags.iter().take(3).map(|tag| format!("#{tag}")));
        if tag_count > 3 {
            metadata_parts.push(format!("+{} tags", tag_count - 3));
        }

        if outbound_link_count > 0 {
            metadata_parts.push(format!(
                "{} link{}",
                outbound_link_count,
                if outbound_link_count == 1 { "" } else { "s" }
            ));
        }

        if backlink_count > 0 {
            metadata_parts.push(format!(
                "{} backlink{}",
                backlink_count,
                if backlink_count == 1 { "" } else { "s" }
            ));
        }

        match (metadata_parts.is_empty(), preview.is_empty()) {
            (true, _) => preview.to_string(),
            (false, true) => metadata_parts.join(" · "),
            (false, false) => format!("{} · {}", metadata_parts.join(" · "), preview),
        }
    }

    /// Strip markdown syntax from a preview string for clean display in the note switcher
    pub(super) fn strip_markdown_for_preview(s: &str) -> String {
        let mut result = s.to_string();
        // Strip common markdown inline formatting
        result = result.replace("**", "");
        result = result.replace("__", "");
        result = result.replace("~~", "");
        // Strip heading markers
        while result.starts_with('#') {
            result = result.trim_start_matches('#').to_string();
        }
        // Strip list markers and blockquotes
        result = result
            .lines()
            .map(|line| {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed
                    .strip_prefix("- [ ] ")
                    .or_else(|| trimmed.strip_prefix("- [x] "))
                {
                    rest
                } else if let Some(rest) = trimmed.strip_prefix("- ") {
                    rest
                } else if let Some(rest) = trimmed.strip_prefix("> ") {
                    rest
                } else {
                    trimmed
                }
            })
            .join(" ");
        // Collapse whitespace
        result.split_whitespace().join(" ").trim().to_string()
    }

    /// Welcome note content for first-time users.
    /// Teaches markdown syntax and key shortcuts through the product itself.
    pub(super) fn welcome_note_content() -> String {
        [
            "# Welcome to Notes",
            "",
            "## What Notes is",
            "",
            "Notes is a fast, keyboard-first place for ideas, drafts, and working context. It stores Markdown notes, supports fast switching, and can hand note context to AI.",
            "",
            "> Start with messy thoughts. Use the actions menu as your command center (Cmd+K).",
            "",
            "---",
            "",
            "## Write fast",
            "",
            "Use Markdown for structure without leaving the editor:",
            "",
            "### A tiny example",
            "",
            "- [x] Capture one useful idea",
            "- [ ] Turn it into a script, plan, or checklist",
            "- Use inline `code` for commands, names, and snippets",
            "",
            "Format as you type, then preview the result when you want to read instead of edit (Cmd+Shift+P). Smart paste can clean up copied text into useful Markdown when possible (Cmd+V). Focus mode hides the chrome so the note can take over (Cmd+.).",
            "",
            "## Navigate",
            "",
            "Create a note whenever the thought arrives (Cmd+N). Create one from the clipboard when the clipboard is the thought (Cmd+Shift+N). Switch notes quickly from the note switcher (Cmd+P), jump through history (Cmd+[ and Cmd+]), and pin important notes so they stay at the top (Cmd+Shift+I).",
            "",
            "Jump to pinned notes by number (Cmd+1 through Cmd+9). Cycle sorting between updated, created, and alphabetical when the list needs a different shape (Cmd+Shift+S).",
            "",
            "## Find things",
            "",
            "Find text inside the current note (Cmd+F). Search across notes with the notes search field (Cmd+Shift+F). Open Trash when something should be out of the way but not gone forever (Cmd+Shift+T).",
            "",
            "Delete carefully; Notes uses Trash first, then permanent delete from Trash. Restore a trashed note while viewing Trash (Cmd+Z).",
            "",
            "## Extend",
            "",
            "The actions menu is the source of truth for commands, shortcuts, and anything added later (Cmd+K). Use it when you are unsure what Notes can do.",
            "",
            "Send note context into Agent Chat/AI when you want help rewriting, summarizing, planning, or turning notes into next actions (Cmd+Enter). Reference other notes with `@note` mentions from the notes portal when you are building context across notes (Cmd+Shift+O).",
            "",
            "Make this your own: replace this tour with the first note you actually need.",
        ]
        .join("\n")
    }

    /// Show a brief action feedback message in the footer (auto-clears after 2s)
    /// If `accent` is true, the message renders in accent color; otherwise muted.
    pub(super) fn show_action_feedback(&mut self, msg: impl Into<String>, accent: bool) {
        self.action_feedback = Some((msg.into(), accent, Instant::now()));
    }

    /// Check if action feedback should still be visible (within 2s window)
    pub(super) fn get_action_feedback(&self) -> Option<(&str, bool)> {
        self.action_feedback.as_ref().and_then(|(msg, accent, t)| {
            if t.elapsed() < Duration::from_millis(ACTION_FEEDBACK_MS) {
                Some((msg.as_str(), *accent))
            } else {
                None
            }
        })
    }
}
