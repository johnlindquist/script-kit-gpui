// Day Page contextual Cmd+K actions ("Today" section) and their executor.
//
// The Today section leads the shared ActionsDialog row list via
// `ActionsDialog::set_host_section`. Every row here must execute through
// `ScriptListApp::execute_day_page_action` with a visible state change —
// rows that cannot execute in the current state are omitted instead of
// rendered disabled.

pub(crate) const DAY_PAGE_ACTIONS_SECTION_TITLE: &str = "Today";
pub(crate) const DAY_PAGE_ASK_AGENT_CHAT_ACTION_ID: &str = "day_page:ask_agent_chat";
pub(crate) const DAY_PAGE_ASK_AGENT_CHAT_CURRENT_LINE_ACTION_ID: &str =
    "day_page:ask_agent_chat_current_line";
pub(crate) const DAY_PAGE_AGENT_CHAT_CONTEXT_SOURCE: &str = "day_page_today";
pub(crate) const DAY_PAGE_CURRENT_LINE_AGENT_CHAT_CONTEXT_SOURCE: &str = "day_page_current_line";
pub(crate) const DAY_PAGE_PREVIEW_AGENT_CHAT_CONTEXT_SOURCE: &str = "day_page_kit_resource_preview";
pub(crate) const DAY_PAGE_PREVIEW_ADD_TO_AGENT_CHAT_ACTION_ID: &str =
    "day_page:kit_preview_add_to_agent_chat";
pub(crate) const DAY_PAGE_PREVIEW_COPY_URI_ACTION_ID: &str = "day_page:kit_preview_copy_uri";
pub(crate) const DAY_PAGE_PREVIEW_OPEN_SOURCE_ACTION_ID: &str = "day_page:kit_preview_open_source";
pub(crate) const DAY_PAGE_PREVIEW_CLOSE_ACTION_ID: &str = "day_page:kit_preview_close";
pub(crate) const DAY_PAGE_TOGGLE_READ_MODE_ACTION_ID: &str = "day_page:toggle_read_mode";

fn day_page_editor_action_id(toolbar_id: &str) -> String {
    format!("day_page:format_{}", toolbar_id.replace('-', "_"))
}

fn day_page_toolbar_id_from_action_id(action_id: &str) -> Option<String> {
    action_id
        .strip_prefix("day_page:format_")
        .map(|id| id.replace('_', "-"))
}

fn day_page_editor_action_shortcut(toolbar_id: &str) -> Option<&'static str> {
    match toolbar_id {
        "bold" => Some("cmd+b"),
        "italic" => Some("cmd+i"),
        "code" => Some("cmd+e"),
        "strikethrough" => Some("cmd+shift+x"),
        _ => None,
    }
}

struct ActiveDayPageLine<'a> {
    text: &'a str,
    line_number: usize,
    byte_range: std::ops::Range<usize>,
}

fn active_day_page_line(content: &str, cursor: usize) -> ActiveDayPageLine<'_> {
    let mut cursor = cursor.min(content.len());
    while cursor > 0 && !content.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let line_start = content[..cursor].rfind('\n').map_or(0, |index| index + 1);
    let line_end = content[cursor..]
        .find('\n')
        .map_or(content.len(), |index| cursor + index);
    let line_number = content[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    ActiveDayPageLine {
        text: &content[line_start..line_end],
        line_number,
        byte_range: line_start..line_end,
    }
}

fn push_unique_day_page_context_part(
    parts: &mut Vec<crate::ai::message_parts::AiContextPart>,
    part: crate::ai::message_parts::AiContextPart,
) {
    if !parts.contains(&part) {
        parts.push(part);
    }
}

fn day_page_agent_chat_part_kind(part: &crate::ai::message_parts::AiContextPart) -> &'static str {
    match part {
        crate::ai::message_parts::AiContextPart::ResourceUri { .. } => "resourceUri",
        crate::ai::message_parts::AiContextPart::FilePath { .. } => "filePath",
        crate::ai::message_parts::AiContextPart::SkillFile { .. } => "skillFile",
        crate::ai::message_parts::AiContextPart::TextBlock { .. } => "textBlock",
        crate::ai::message_parts::AiContextPart::FocusedTarget { .. } => "focusedTarget",
        crate::ai::message_parts::AiContextPart::AmbientContext { .. } => "ambientContext",
    }
}

fn day_page_app_context_part_from_shared(
    part: script_kit_gpui::ai::AiContextPart,
) -> Option<crate::ai::message_parts::AiContextPart> {
    serde_json::from_value(serde_json::to_value(part).ok()?).ok()
}

fn day_page_agent_chat_fingerprint(value: &str) -> String {
    format!(
        "sha256:{}",
        crate::logging::log_private_user_value(value).sha256
    )
}

#[derive(Clone, Copy)]
enum DayPageAgentHandoffMode {
    Selection,
    CurrentLine,
    ExplicitWholeDay,
}

impl DayPageAgentHandoffMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::CurrentLine => "currentLine",
            Self::ExplicitWholeDay => "wholeNote",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Selection | Self::CurrentLine => DAY_PAGE_CURRENT_LINE_AGENT_CHAT_CONTEXT_SOURCE,
            Self::ExplicitWholeDay => DAY_PAGE_AGENT_CHAT_CONTEXT_SOURCE,
        }
    }

    fn includes_whole_day(self) -> bool {
        matches!(self, Self::ExplicitWholeDay)
    }
}

struct DayPageAgentHandoffPacket {
    scope: crate::notes::ai_scope::NotesAiScope,
    parts: Vec<crate::ai::message_parts::AiContextPart>,
    prompt_seed: String,
    receipt: serde_json::Value,
}

fn save_today_action() -> crate::actions::Action {
    crate::actions::Action::new(
        "day_page:save",
        "Save Today",
        Some("Write the day page to disk now".to_string()),
        crate::actions::ActionCategory::ScriptContext,
    )
    .with_shortcut("cmd+s")
    .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE)
}

/// Build the Today section rows for the current Day Page state.
pub(crate) fn day_page_host_actions_section(
    view: &DayPageView,
    cx: &App,
) -> Vec<crate::actions::Action> {
    use crate::actions::{Action, ActionCategory};

    let mut actions = Vec::new();
    let viewing_fragment = view.session.is_viewing_fragment();

    if let Some(preview) = view.kit_resource_preview.as_ref() {
        let availability = view
            .kit_resource_preview_action_availability()
            .expect("preview action availability exists when preview is open");
        if availability.can_add_to_agent_chat {
            actions.push(
                Action::new(
                    DAY_PAGE_PREVIEW_ADD_TO_AGENT_CHAT_ACTION_ID,
                    "Add Resource to Agent Chat",
                    Some("Attach only this kit:// preview URI to Agent Chat".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
            );
        }
        actions.push(
            Action::new(
                DAY_PAGE_PREVIEW_COPY_URI_ACTION_ID,
                "Copy URI",
                Some("Copy this resource URI".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
        if availability.open_source_target.is_some() {
            actions.push(
                Action::new(
                    DAY_PAGE_PREVIEW_OPEN_SOURCE_ACTION_ID,
                    "Open Source",
                    Some("Open the source represented by this resource in Today".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
            );
        }
        actions.push(
            Action::new(
                DAY_PAGE_PREVIEW_CLOSE_ACTION_ID,
                format!(
                    "Close Preview / {}",
                    view.kit_resource_preview_return_label()
                ),
                Some(format!("Return from {}", preview.uri)),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("escape")
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
        if view.is_dirty() {
            actions.push(save_today_action());
        }
        return actions;
    }

    if viewing_fragment {
        actions.push(
            Action::new(
                "day_page:back_to_today",
                "Back to Today",
                Some("Return from the fragment to the day page".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("escape")
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
    }

    if view.session.is_viewing_note()
        || (!view.session.is_viewing_fragment() && view.session.bound_date().is_some())
    {
        actions.push(
            Action::new(
                "day_page:open_in_notes_window",
                "Open in Notes Window",
                Some("Move this note from the Day Page surface to the Notes window".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
    }

    if view.is_dirty() {
        actions.push(save_today_action());
    }

    actions.push(
        Action::new(
            DAY_PAGE_TOGGLE_READ_MODE_ACTION_ID,
            if view.read_mode {
                "Edit Markdown"
            } else {
                "Preview Markdown"
            },
            Some(if view.read_mode {
                "Return to the editable Day Page notes editor".to_string()
            } else {
                "Read Today's markdown with the shared Notes preview renderer".to_string()
            }),
            ActionCategory::ScriptContext,
        )
        .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
    );

    let viewing_today = view
        .session
        .bound_date()
        .is_some_and(|date| date == view.local_today());
    if !view.session.is_viewing_fragment() && viewing_today {
        let selection = view.notes_editor.read(cx).selection(cx);
        let has_selection = selection.start < selection.end;
        actions.push(
            Action::new(
                DAY_PAGE_ASK_AGENT_CHAT_CURRENT_LINE_ACTION_ID,
                if has_selection {
                    "Ask AI About Selection"
                } else {
                    "Ask AI About Current Line"
                },
                Some(if has_selection {
                    "Attach only the selected Today text and references inside that selection"
                        .to_string()
                } else {
                    "Attach only the active Today line and references on that line".to_string()
                }),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+enter")
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
        actions.push(
            Action::new(
                DAY_PAGE_ASK_AGENT_CHAT_ACTION_ID,
                "Ask Agent Chat About Today",
                Some("Attach Today's brain to Agent Chat and start a question".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
    }

    let clipboard_text = cx
        .read_from_clipboard()
        .and_then(|item| item.text().map(|text| text.to_string()))
        .filter(|text| !text.trim().is_empty());
    if clipboard_text.is_some() && !view.read_mode {
        actions.push(
            Action::new(
                "day_page:insert_clipboard",
                "Insert Clipboard Text",
                Some("Paste the current clipboard text at the cursor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
        );
    }

    if !view.read_mode {
        for item in crate::components::notes_editor::NOTES_EDITOR_TOOLBAR_ACTIONS {
            let toolbar_id = item.spec.id;
            actions.push(
                Action::new(
                    day_page_editor_action_id(toolbar_id),
                    crate::components::notes_editor::notes_editor_toolbar_action_title(toolbar_id),
                    Some("Apply shared Notes editor Markdown formatting".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut_opt(day_page_editor_action_shortcut(toolbar_id).map(str::to_string))
                .with_section(DAY_PAGE_ACTIONS_SECTION_TITLE),
            );
        }
    }

    actions
}

impl DayPageView {
    pub(crate) fn kit_resource_preview_context_part(
        &self,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let preview = self.kit_resource_preview.as_ref()?;
        if !preview.allow_agent_chat_action {
            return None;
        }
        Some(crate::ai::message_parts::AiContextPart::ResourceUri {
            uri: preview.uri.clone(),
            label: preview.title.clone(),
        })
    }

    pub(crate) fn open_agent_chat_about_kit_resource_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(part) = self.kit_resource_preview_context_part() else {
            if let Some(preview) = self.kit_resource_preview.as_ref() {
                tracing::warn!(
                    target: "script_kit::day_page",
                    event = "day_page_kit_preview_agent_chat_blocked",
                    uri_fingerprint = %day_page_agent_chat_fingerprint(&preview.uri),
                );
            }
            return false;
        };
        let scope = crate::notes::ai_scope::NotesAiScope::Resource {
            uri_identity: part.source().to_string(),
        };
        self.last_agent_chat_handoff_receipt = Some(serde_json::json!({
            "schemaVersion": 1,
            "receiptKind": "dayPage.agentChatHandoff",
            "redacted": true,
            "mode": scope.kind(),
            "scope": scope.kind(),
            "documentSemanticIdLength": scope.document_semantic_id().chars().count(),
            "documentSemanticIdFingerprint": day_page_agent_chat_fingerprint(scope.document_semantic_id()),
            "rangeLength": scope.range_length(),
            "destination": "agentChat",
            "stagingOutcome": "composerOnly",
            "contextPartCount": 1,
        }));
        let prompt = String::new();
        let Some(app) = self.app.upgrade() else {
            return false;
        };

        window.defer(cx, move |_window, cx| {
            app.update(cx, |app, cx| {
                app.clear_actions_popup_state();
                if crate::actions::is_actions_window_open() {
                    crate::actions::close_actions_window(cx);
                }
                app.open_tab_ai_agent_chat_with_context_part(
                    part,
                    DAY_PAGE_PREVIEW_AGENT_CHAT_CONTEXT_SOURCE,
                    cx,
                );
                if let AppView::AgentChatView { entity } = &app.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.set_input(prompt.clone(), cx);
                    });
                }
            });
        });
        true
    }

    fn today_agent_chat_context_part(&self, cx: &App) -> crate::ai::message_parts::AiContextPart {
        let content = self.notes_editor.read(cx).content(cx);
        let date_label = self
            .session
            .bound_date()
            .map(|date| date.to_string())
            .unwrap_or_else(|| "Today".to_string());
        let source = self
            .session
            .path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("brain://days/{date_label}"));

        crate::ai::message_parts::AiContextPart::TextBlock {
            label: format!("Today's brain - {date_label}"),
            source,
            text: if content.trim().is_empty() {
                "(Today's brain is empty.)".to_string()
            } else {
                content
            },
            mime_type: Some("text/markdown".to_string()),
        }
    }

    fn build_whole_day_agent_chat_packet(&self, cx: &App) -> DayPageAgentHandoffPacket {
        let date_label = self.day_page_agent_chat_date_label();
        let canonical_path = self.day_page_agent_chat_canonical_path(&date_label);
        let scope = crate::notes::ai_scope::NotesAiScope::WholeNote {
            document_id: format!("day:{date_label}"),
        };
        let parts = vec![self.today_agent_chat_context_part(cx)];
        let receipt = self.build_agent_chat_handoff_receipt(
            DayPageAgentHandoffMode::ExplicitWholeDay,
            &scope,
            &date_label,
            canonical_path.as_deref(),
            None,
            &parts,
            cx,
        );

        DayPageAgentHandoffPacket {
            scope,
            parts,
            prompt_seed: String::new(),
            receipt,
        }
    }

    fn build_current_line_agent_chat_packet(&self, cx: &App) -> DayPageAgentHandoffPacket {
        let content = self.notes_editor.read(cx).content(cx);
        let selection = self.notes_editor.read(cx).selection(cx);
        let date_label = self.day_page_agent_chat_date_label();
        let canonical_path = self.day_page_agent_chat_canonical_path(&date_label);
        let document_id = format!("day:{date_label}");
        let (scope, scoped_text) = crate::notes::ai_scope::selected_or_current_line_scope(
            document_id,
            &content,
            selection,
        );
        let mode = if matches!(scope, crate::notes::ai_scope::NotesAiScope::Selection { .. }) {
            DayPageAgentHandoffMode::Selection
        } else {
            DayPageAgentHandoffMode::CurrentLine
        };
        let active_line = matches!(mode, DayPageAgentHandoffMode::CurrentLine).then(|| {
            active_day_page_line(
                &content,
                scope.range().map(|range| range.end).unwrap_or_default(),
            )
        });
        let source = format!(
            "{}#scope={}",
            canonical_path.as_deref().unwrap_or("brain://days/Today"),
            scope.kind(),
        );
        let label = match mode {
            DayPageAgentHandoffMode::Selection => format!("Today selection - {date_label}"),
            DayPageAgentHandoffMode::CurrentLine => format!("Today current line - {date_label}"),
            DayPageAgentHandoffMode::ExplicitWholeDay => unreachable!(),
        };

        let mut parts = vec![crate::ai::message_parts::AiContextPart::TextBlock {
            label,
            source,
            text: if scoped_text.trim().is_empty() {
                format!("({} is empty.)", scope.kind())
            } else {
                scoped_text.clone()
            },
            mime_type: Some("text/markdown".to_string()),
        }];

        for part in
            script_kit_gpui::day_page::context_parts_from_day_page_markdown_links(&scoped_text)
        {
            if let Some(part) = day_page_app_context_part_from_shared(part) {
                push_unique_day_page_context_part(&mut parts, part);
            }
        }

        for (token, part) in &self.spine_handoff.mention_aliases {
            if scoped_text.contains(token) {
                push_unique_day_page_context_part(&mut parts, part.clone());
            }
        }

        let receipt = self.build_agent_chat_handoff_receipt(
            mode,
            &scope,
            &date_label,
            canonical_path.as_deref(),
            active_line.as_ref(),
            &parts,
            cx,
        );

        DayPageAgentHandoffPacket {
            scope,
            parts,
            prompt_seed: String::new(),
            receipt,
        }
    }

    fn day_page_agent_chat_date_label(&self) -> String {
        self.session
            .bound_date()
            .map(|date| date.to_string())
            .unwrap_or_else(|| "Today".to_string())
    }

    fn day_page_agent_chat_canonical_path(&self, date_label: &str) -> Option<String> {
        self.session
            .path()
            .map(|path| path.display().to_string())
            .or_else(|| Some(format!("brain://days/{date_label}")))
    }

    fn build_agent_chat_handoff_receipt(
        &self,
        mode: DayPageAgentHandoffMode,
        scope: &crate::notes::ai_scope::NotesAiScope,
        date_label: &str,
        canonical_path: Option<&str>,
        active_line: Option<&ActiveDayPageLine<'_>>,
        parts: &[crate::ai::message_parts::AiContextPart],
        cx: &App,
    ) -> serde_json::Value {
        let content = self.notes_editor.read(cx).content(cx);
        let total_lines = content.lines().count();
        let omitted_line_count = active_line
            .map(|_| total_lines.saturating_sub(1))
            .unwrap_or(0);
        let packet_chars = parts
            .iter()
            .map(|part| match part {
                crate::ai::message_parts::AiContextPart::TextBlock { text, .. } => {
                    text.chars().count()
                }
                _ => 0,
            })
            .sum::<usize>();
        let part_receipts = parts
            .iter()
            .map(|part| {
                let source = part.source();
                serde_json::json!({
                    "kind": day_page_agent_chat_part_kind(part),
                    "labelChars": part.label().chars().count(),
                    "sourceChars": source.chars().count(),
                    "sourceHash": day_page_agent_chat_fingerprint(source),
                })
            })
            .collect::<Vec<_>>();

        serde_json::json!({
            "schemaVersion": 1,
            "receiptKind": "dayPage.agentChatHandoff",
            "redacted": true,
            "mode": mode.as_str(),
            "scope": scope.kind(),
            "documentSemanticIdLength": scope.document_semantic_id().chars().count(),
            "documentSemanticIdFingerprint": day_page_agent_chat_fingerprint(scope.document_semantic_id()),
            "rangeLength": scope.range_length(),
            "destination": "agentChat",
            "stagingOutcome": "composerOnly",
            "source": mode.source(),
            "date": date_label,
            "canonicalPathChars": canonical_path.map(|path| path.chars().count()),
            "canonicalPathHash": canonical_path.map(day_page_agent_chat_fingerprint),
            "lineRange": active_line.map(|line| serde_json::json!({
                "lineNumber": line.line_number,
                "start": line.byte_range.start,
                "end": line.byte_range.end,
                "unit": "utf8ByteOffset",
                "lineChars": line.text.chars().count(),
            })),
            "wholeDayIncluded": mode.includes_whole_day(),
            "excludedContent": {
                "omittedLineCount": omitted_line_count,
                "omittedContentIncluded": false,
                "contentFingerprint": day_page_agent_chat_fingerprint(&content),
            },
            "packetChars": packet_chars,
            "contextPartCount": parts.len(),
            "contextParts": part_receipts,
        })
    }

    pub(crate) fn open_agent_chat_about_today(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let packet = self.build_whole_day_agent_chat_packet(cx);
        self.last_agent_chat_handoff_receipt = Some(packet.receipt.clone());
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_agent_chat_handoff_receipt",
            receipt_json = %packet.receipt,
        );
        let parts = packet.parts.clone();
        let prompt_seed = packet.prompt_seed.clone();
        let part = parts[0].clone();
        let Some(app) = self.app.upgrade() else {
            return;
        };

        window.defer(cx, move |_window, cx| {
            app.update(cx, |app, cx| {
                if app.has_day_page_context_round_trip_pending() {
                    tracing::warn!(
                        target: "script_kit::day_page",
                        event = "day_page_agent_chat_open_blocked",
                        reason = "context_round_trip_pending",
                    );
                    return;
                }
                app.clear_actions_popup_state();
                if crate::actions::is_actions_window_open() {
                    crate::actions::close_actions_window(cx);
                }
                app.open_tab_ai_agent_chat_with_context_part(
                    part,
                    DAY_PAGE_AGENT_CHAT_CONTEXT_SOURCE,
                    cx,
                );
                if let AppView::AgentChatView { entity } = &app.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        let _ = chat.stage_inline_context_parts_from_host(
                            parts.clone(),
                            DAY_PAGE_AGENT_CHAT_CONTEXT_SOURCE,
                            cx,
                        );
                        chat.set_input(prompt_seed.clone(), cx);
                    });
                }
            });
        });
    }

    pub(crate) fn open_agent_chat_about_current_line(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let packet = self.build_current_line_agent_chat_packet(cx);
        self.last_agent_chat_handoff_receipt = Some(packet.receipt.clone());
        tracing::info!(
            target: "script_kit::day_page",
            event = "day_page_agent_chat_handoff_receipt",
            receipt_json = %packet.receipt,
        );
        let parts = packet.parts.clone();
        let prompt_seed = packet.prompt_seed.clone();
        let Some(part) = parts.first().cloned() else {
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };

        window.defer(cx, move |_window, cx| {
            app.update(cx, |app, cx| {
                if app.has_day_page_context_round_trip_pending() {
                    tracing::warn!(
                        target: "script_kit::day_page",
                        event = "day_page_agent_chat_open_blocked",
                        reason = "context_round_trip_pending",
                    );
                    return;
                }
                app.clear_actions_popup_state();
                if crate::actions::is_actions_window_open() {
                    crate::actions::close_actions_window(cx);
                }
                app.open_tab_ai_agent_chat_with_context_part(
                    part,
                    DAY_PAGE_CURRENT_LINE_AGENT_CHAT_CONTEXT_SOURCE,
                    cx,
                );
                if let AppView::AgentChatView { entity } = &app.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        let _ = chat.stage_inline_context_parts_from_host(
                            parts.clone(),
                            DAY_PAGE_CURRENT_LINE_AGENT_CHAT_CONTEXT_SOURCE,
                            cx,
                        );
                        chat.set_input(prompt_seed.clone(), cx);
                    });
                }
            });
        });
    }

    pub(crate) fn append_agent_chat_response_to_today_file(
        &mut self,
        response: &str,
    ) -> anyhow::Result<()> {
        let response = response.trim();
        if response.is_empty() {
            anyhow::bail!("empty Agent Chat response");
        }
        let local = Utc::now().with_timezone(&self.session.substrate().timezone());
        let timestamp = local.format("%H:%M").to_string();
        self.session
            .append_external_line_to_bound_file(&format!("{timestamp} Agent Chat\n\n{response}"))?;
        script_kit_gpui::brain::indexer::wake_indexer();
        Ok(())
    }

    pub(crate) fn insert_clipboard_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx
            .read_from_clipboard()
            .and_then(|item| item.text().map(|text| text.to_string()))
        else {
            return;
        };
        if text.is_empty() {
            return;
        }
        self.notes_editor.update(cx, |editor, cx| {
            let input = editor.input_state();
            input.update(cx, |state, cx| {
                state.replace(&text, window, cx);
            });
        });
        cx.notify();
    }

    pub(crate) fn run_shared_markdown_toolbar_action(
        &mut self,
        toolbar_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(action) =
            crate::components::notes_editor::notes_editor_toolbar_action_by_id(toolbar_id)
        else {
            return false;
        };
        self.notes_editor.update(cx, |editor, cx| {
            (action.run)(editor, window, cx);
        });
        self.sync_footer(window, cx);
        cx.notify();
        true
    }
}

impl ScriptListApp {
    /// Execute a `day_page:*` actions-dialog row. Returns true when handled.
    pub(crate) fn execute_day_page_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::DayPage { entity } = &self.current_view else {
            return false;
        };
        let entity = entity.clone();
        let handled = match action_id {
            "day_page:save" => {
                entity.update(cx, |view, cx| {
                    view.save_and_sync_footer(window, cx);
                });
                true
            }
            "day_page:open_past_day" => {
                let window_handle = window.window_handle();
                window.defer(cx, move |_window, cx| {
                    cx.defer(move |cx| {
                        let _ = window_handle.update(cx, |_root, window, cx| {
                            entity.update(cx, |view, cx| view.open_note_switcher(window, cx));
                        });
                    });
                });
                true
            }
            "day_page:open_in_notes_window" => {
                let (note_id, day_date) = {
                    let view = entity.read(cx);
                    (
                        view.session.viewing_note_id().map(str::to_string),
                        view.session.bound_date(),
                    )
                };
                if let Some(note_id) = note_id.and_then(|id| crate::notes::NoteId::parse(&id)) {
                    if let Err(error) = crate::notes::open_note_in_notes_window(cx, note_id) {
                        tracing::warn!(
                            target: "script_kit::day_page",
                            error = %error,
                            "day_page_open_in_notes_window_failed"
                        );
                    }
                    true
                } else if let Some(date) = day_date {
                    if let Err(error) = crate::notes::open_day_note_in_notes_window(cx, date) {
                        tracing::warn!(
                            target: "script_kit::day_page",
                            error = %error,
                            "day_page_open_day_in_notes_window_failed"
                        );
                    }
                    true
                } else {
                    false
                }
            }
            DAY_PAGE_ASK_AGENT_CHAT_ACTION_ID => {
                entity.update(cx, |view, cx| view.open_agent_chat_about_today(window, cx));
                true
            }
            DAY_PAGE_ASK_AGENT_CHAT_CURRENT_LINE_ACTION_ID => {
                entity.update(cx, |view, cx| {
                    view.open_agent_chat_about_current_line(window, cx)
                });
                true
            }
            DAY_PAGE_PREVIEW_ADD_TO_AGENT_CHAT_ACTION_ID => entity.update(cx, |view, cx| {
                view.open_agent_chat_about_kit_resource_preview(window, cx)
            }),
            DAY_PAGE_PREVIEW_COPY_URI_ACTION_ID => {
                let uri = {
                    let view = entity.read(cx);
                    view.kit_resource_preview_action_availability()
                        .filter(|availability| availability.can_copy_uri)
                        .and_then(|_| {
                            view.kit_resource_preview
                                .as_ref()
                                .map(|preview| preview.uri.clone())
                        })
                };
                if let Some(uri) = uri {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(uri));
                    true
                } else {
                    false
                }
            }
            DAY_PAGE_PREVIEW_OPEN_SOURCE_ACTION_ID => entity.update(cx, |view, cx| {
                view.open_kit_resource_preview_source(window, cx)
            }),
            DAY_PAGE_PREVIEW_CLOSE_ACTION_ID => entity.update(cx, |view, cx| {
                let can_close = view
                    .kit_resource_preview_action_availability()
                    .is_some_and(|availability| availability.can_close);
                if can_close {
                    view.close_kit_resource_preview(window, cx);
                }
                can_close
            }),
            DAY_PAGE_TOGGLE_READ_MODE_ACTION_ID => {
                entity.update(cx, |view, cx| view.toggle_read_mode(window, cx));
                true
            }
            "day_page:back_to_today" => {
                entity.update(cx, |view, cx| view.return_to_day_page(window, cx));
                true
            }
            "day_page:insert_clipboard" => {
                entity.update(cx, |view, cx| view.insert_clipboard_text(window, cx));
                true
            }
            _ => day_page_toolbar_id_from_action_id(action_id)
                .map(|toolbar_id| {
                    entity.update(cx, |view, cx| {
                        view.run_shared_markdown_toolbar_action(&toolbar_id, window, cx)
                    })
                })
                .unwrap_or(false),
        };
        if handled {
            tracing::info!(
                target: "script_kit::day_page",
                event = "day_page_action_executed",
                action_id = %action_id,
            );
        }
        handled
    }
}

#[cfg(test)]
mod day_page_markdown_action_tests {
    use super::*;
    use crate::components::notes_editor::NOTES_EDITOR_TOOLBAR_ACTIONS;

    #[test]
    fn day_page_markdown_action_catalog_covers_notes_toolbar_actions() {
        for item in NOTES_EDITOR_TOOLBAR_ACTIONS {
            let action_id = day_page_editor_action_id(item.spec.id);
            let round_trip = day_page_toolbar_id_from_action_id(&action_id)
                .expect("day page action id should map back to toolbar id");
            assert_eq!(round_trip, item.spec.id);
        }
    }

    #[test]
    fn day_page_legacy_format_action_ids_are_preserved() {
        assert_eq!(day_page_editor_action_id("bold"), "day_page:format_bold");
        assert_eq!(
            day_page_editor_action_id("italic"),
            "day_page:format_italic"
        );
        assert_eq!(day_page_editor_action_id("code"), "day_page:format_code");
        assert_eq!(
            day_page_editor_action_id("strikethrough"),
            "day_page:format_strikethrough"
        );
    }

    #[test]
    fn day_page_hyphenated_toolbar_ids_round_trip() {
        assert_eq!(
            day_page_toolbar_id_from_action_id("day_page:format_numbered_list").as_deref(),
            Some("numbered-list")
        );
    }

    #[test]
    fn active_day_page_line_scopes_to_cursor_line() {
        let content = "first [README](file:///tmp/readme.md)\nsecond active\nthird";
        let cursor = content.find("active").expect("active line exists");
        let line = active_day_page_line(content, cursor);
        assert_eq!(line.text, "second active");
        assert_eq!(line.line_number, 2);
    }

    #[test]
    fn day_page_agent_chat_actions_distinguish_line_and_whole_day() {
        assert_eq!(
            DAY_PAGE_ASK_AGENT_CHAT_CURRENT_LINE_ACTION_ID,
            "day_page:ask_agent_chat_current_line"
        );
        assert_eq!(DAY_PAGE_ASK_AGENT_CHAT_ACTION_ID, "day_page:ask_agent_chat");
        assert_ne!(
            DAY_PAGE_CURRENT_LINE_AGENT_CHAT_CONTEXT_SOURCE, DAY_PAGE_AGENT_CHAT_CONTEXT_SOURCE,
            "current-line and whole-day handoffs must be separately receiptable"
        );
    }

    #[test]
    fn day_page_agent_chat_receipt_hashes_sources() {
        let hash = day_page_agent_chat_fingerprint("/private/day-page.md#line=2");
        assert_eq!(hash.len(), "sha256:".len() + 64);
        assert!(hash.starts_with("sha256:"));
        assert!(!hash.contains("day-page"));
    }
}
