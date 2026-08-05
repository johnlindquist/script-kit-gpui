use super::*;

const ATTACHMENT_PORTAL_WIDTH_RESTORE_EPSILON: f32 = 1.0;

impl ScriptListApp {
    fn open_script_list_attachment_portal(
        &mut self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
        query: &str,
        placeholder: &str,
        _cx: &mut Context<Self>,
    ) {
        self.active_attachment_portal_kind = Some(kind);
        self.filter_text = query.to_string();
        self.computed_filter_text = query.to_string();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
        self.show_script_list_with_main_filter_focus();
        self.hovered_index = None;
        self.selected_index = 0;
        self.opened_from_main_menu = true;
        self.invalidate_grouped_cache();
        self.sync_list_state();
        self.update_window_size();
    }

    fn current_main_window_width() -> Option<f32> {
        crate::platform::get_main_window_bounds()
            .map(|(_, _, width, _)| width as f32)
            .filter(|width| width.is_finite() && *width > 0.0)
    }

    fn capture_attachment_portal_host_snapshot(&self) -> AttachmentPortalHostSnapshot {
        let snapshot = AttachmentPortalHostSnapshot {
            filter_text: self.filter_text.clone(),
            computed_filter_text: self.computed_filter_text.clone(),
            pending_filter_sync: self.pending_filter_sync,
            pending_placeholder: self.pending_placeholder.clone(),
            hovered_index: self.hovered_index,
            selected_index: self.selected_index,
            opened_from_main_menu: self.opened_from_main_menu,
            focused_input: self.focused_input,
            pending_focus: self.pending_focus,
            width_before_portal: self.attachment_portal_return_width,
            width_after_portal_open: None,
        };

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_host_snapshot_captured",
        );

        snapshot
    }

    fn record_attachment_portal_open_width(&mut self) {
        let open_width = Self::current_main_window_width();
        if let Some(snapshot) = self.attachment_portal_host_snapshot.as_mut() {
            snapshot.width_after_portal_open = open_width;
        }
    }

    fn restore_attachment_portal_host_snapshot(
        &mut self,
        snapshot: AttachmentPortalHostSnapshot,
    ) -> (Option<f32>, Option<f32>) {
        let AttachmentPortalHostSnapshot {
            filter_text,
            computed_filter_text,
            pending_filter_sync,
            pending_placeholder,
            hovered_index,
            selected_index,
            opened_from_main_menu,
            focused_input,
            pending_focus,
            width_before_portal,
            width_after_portal_open,
        } = snapshot;

        let filter_text_changed = self.filter_text != filter_text;

        self.filter_text = filter_text;
        self.computed_filter_text = computed_filter_text;
        self.pending_filter_sync = pending_filter_sync || filter_text_changed;
        self.pending_placeholder = pending_placeholder;
        self.hovered_index = hovered_index;
        self.selected_index = selected_index;
        self.opened_from_main_menu = opened_from_main_menu;
        self.focused_input = focused_input;
        self.pending_focus = pending_focus;
        self.invalidate_grouped_cache();
        self.sync_list_state();

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_host_snapshot_restored",
        );

        (width_before_portal, width_after_portal_open)
    }

    fn restore_attachment_portal_return_view(
        &mut self,
        return_view: AppView,
        return_focus_target: FocusTarget,
    ) {
        let return_width = self.attachment_portal_return_width.take();
        let current_width = Self::current_main_window_width();
        let (width_before_portal, width_after_portal_open) = self
            .attachment_portal_host_snapshot
            .take()
            .map(|snapshot| self.restore_attachment_portal_host_snapshot(snapshot))
            .unwrap_or((return_width, None));

        self.active_attachment_portal_kind = None;
        self.restore_current_view_with_focus(return_view, return_focus_target);

        // Portal views can temporarily expand the window; restore the
        // originating surface sizing before control returns to the user unless
        // the user manually resized after the portal opened.
        match (width_before_portal, current_width) {
            (Some(return_width), Some(current_width))
                if width_after_portal_open.is_some_and(|open_width| {
                    (current_width - open_width).abs() > ATTACHMENT_PORTAL_WIDTH_RESTORE_EPSILON
                }) =>
            {
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "attachment_portal_width_restore_skipped_user_resize",
                    width_before_portal = return_width,
                    width_after_portal_open = ?width_after_portal_open,
                    current_width,
                );
                self.resize_current_view_to_width(current_width);
            }
            (Some(return_width), _) => {
                self.resize_current_view_to_width(return_width);
            }
            (None, Some(current_width)) => {
                self.resize_current_view_to_width(current_width);
            }
            (None, None) => {
                self.update_window_size();
            }
        }
    }

    /// Whether the app is currently in an attachment portal (file search or
    /// clipboard history opened from the Agent Chat chat context selector, or
    /// file search opened from the main-menu `@file` spine flow).
    ///
    /// Agent-chat-hosted portals read from the app-owned
    /// `agent_chat_surface_state` machine rather than probing
    /// `attachment_portal_return_view.is_some()`. The Cmd+Enter launcher-entry
    /// guard calls this, so a single source of truth prevents it from drifting
    /// against the portal snapshot fields. ScriptList-hosted spine portals are
    /// tracked by `spine_mention_portal_segment` because the agent-chat
    /// machine deliberately rejects `PortalOpened` while `Hidden`.
    pub(crate) fn is_in_attachment_portal(&self) -> bool {
        self.agent_chat_surface_state.is_attachment_portal()
            || self.spine_mention_portal_segment.is_some()
    }

    /// Consume Script List Escape while an attachment portal owns the main surface.
    ///
    /// Both physical GPUI dispatch and the automation mirror route through this
    /// helper so the portal returns to its host before the normal launcher Escape
    /// ladder can clear the query, navigate elsewhere, or hide the window.
    pub(crate) fn try_cancel_script_list_attachment_portal_escape(
        &mut self,
        routing_path: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_in_attachment_portal() {
            return false;
        }

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "script_list_attachment_portal_escape_consumed",
            routing_path,
        );
        self.close_attachment_portal_cancel(cx);
        true
    }

    /// Open the full built-in File Search surface as a ScriptList-hosted
    /// attachment portal for the main-menu `@file` spine flow. Enter resolves
    /// the originating segment into a compact `@file:basename` token; Escape
    /// restores the pre-portal filter text.
    pub(crate) fn open_spine_file_search_attachment_portal(
        &mut self,
        segment_byte_range: std::ops::Range<usize>,
        query: String,
        cx: &mut Context<Self>,
    ) {
        if self.is_in_attachment_portal() {
            tracing::warn!(
                target: "script_kit::spine",
                event = "spine_attachment_portal_nested_prevented",
            );
            return;
        }
        if !matches!(self.current_view, AppView::ScriptList) {
            tracing::warn!(
                target: "script_kit::spine",
                event = "spine_attachment_portal_requires_script_list",
                current_view = ?self.current_view,
            );
            return;
        }

        self.attachment_portal_return_view = Some(AppView::ScriptList);
        self.attachment_portal_return_focus_target = Some(FocusTarget::MainFilter);
        self.attachment_portal_return_width = Self::current_main_window_width();
        self.attachment_portal_host_snapshot = Some(self.capture_attachment_portal_host_snapshot());
        self.active_attachment_portal_kind =
            Some(crate::ai::context_selector::types::ContextPortalKind::FileSearch);
        self.spine_mention_portal_segment = Some(segment_byte_range);

        tracing::info!(
            target: "script_kit::spine",
            event = "spine_attachment_portal_opened",
            query = %query,
            return_width = ?self.attachment_portal_return_width,
        );

        self.open_file_search(query, cx);
        self.record_attachment_portal_open_width();
        cx.notify();
    }

    /// Resolve the `@file` spine segment that opened a ScriptList-hosted
    /// portal: replace the segment text with the compact token, register the
    /// full-path alias, and re-run the spine/filter pipeline. Runs after the
    /// host snapshot restore, so `filter_text` is the pre-portal text the
    /// stored byte range was captured against.
    fn resolve_spine_mention_from_portal_part(
        &mut self,
        segment_byte_range: std::ops::Range<usize>,
        part: &crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        let crate::ai::message_parts::AiContextPart::FilePath { path, .. } = part else {
            tracing::warn!(
                target: "script_kit::spine",
                event = "spine_attachment_portal_part_not_file",
            );
            return;
        };

        let token = self.unique_spine_file_mention_token(path);
        self.register_spine_file_mention_alias(token.clone(), path.clone());

        // Today → main-menu round trip: route the resolved token back into
        // the Day Page line instead of the launcher filter. Deferred via the
        // main window handle because this path has no `window`.
        if self.day_page_context_return.is_some() {
            let app_entity = cx.entity();
            let token_for_day_page = token.clone();
            cx.defer(move |cx| {
                let Some(handle) = crate::get_main_window_handle() else {
                    return;
                };
                let _ = handle.update(cx, |_, window, cx| {
                    app_entity.update(cx, |app, cx| {
                        app.try_complete_day_page_context_round_trip(
                            &token_for_day_page,
                            window,
                            cx,
                        );
                    });
                });
            });
            return;
        }

        let current = self.filter_text.clone();
        let new_text = if segment_byte_range.end <= current.len()
            && current.is_char_boundary(segment_byte_range.start)
            && current.is_char_boundary(segment_byte_range.end)
        {
            let prefix = &current[..segment_byte_range.start];
            let suffix = current[segment_byte_range.end..].trim_start();
            if suffix.is_empty() {
                format!("{prefix}{token} ")
            } else {
                format!("{prefix}{token} {suffix}")
            }
        } else {
            // The stored range no longer fits the restored filter; append
            // rather than dropping the accepted file.
            let trimmed = current.trim_end();
            if trimmed.is_empty() {
                format!("{token} ")
            } else {
                format!("{trimmed} {token} ")
            }
        };

        tracing::info!(
            target: "script_kit::spine",
            event = "spine_attachment_portal_segment_resolved",
            token = %token,
        );

        self.filter_text = new_text.clone();
        self.computed_filter_text = new_text.clone();
        self.pending_filter_sync = true;
        self.set_spine_parse_from_filter_and_cursor(&new_text, new_text.len());
        self.maybe_start_spine_file_subsearch_for_current_projection(cx);
        self.invalidate_grouped_cache();
        self.sync_list_state();
    }

    pub(crate) fn active_attachment_portal_kind(
        &self,
    ) -> Option<crate::ai::context_selector::types::ContextPortalKind> {
        self.active_attachment_portal_kind
    }

    pub(crate) fn build_attachment_portal_part_for_selected_script_list_result(
        &mut self,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        use crate::ai::message_parts::AiContextPart;

        let result = self.get_selected_result()?;
        match &result {
            scripts::SearchResult::Script(script_match) => Some(AiContextPart::FilePath {
                path: script_match.script.path.to_string_lossy().to_string(),
                label: script_match.script.name.clone(),
            }),
            scripts::SearchResult::Scriptlet(scriptlet_match) => {
                let target = Self::tab_ai_target_from_search_result(self.selected_index, &result);
                let target = crate::ai::TabAiTargetContext {
                    metadata: Some(serde_json::json!({
                        "name": scriptlet_match.scriptlet.name,
                        "description": scriptlet_match.scriptlet.description,
                        "tool": scriptlet_match.scriptlet.tool,
                        "code": scriptlet_match.scriptlet.code,
                        "filePath": scriptlet_match.scriptlet.file_path,
                        "pluginId": scriptlet_match.scriptlet.plugin_id,
                        "pluginTitle": scriptlet_match.scriptlet.plugin_title,
                    })),
                    ..target
                };
                let label = crate::ai::format_explicit_target_chip_label(&target);
                Some(AiContextPart::FocusedTarget { target, label })
            }
            scripts::SearchResult::Skill(skill_match) => {
                let owner = if skill_match.skill.plugin_title.is_empty() {
                    skill_match.skill.plugin_id.clone()
                } else {
                    skill_match.skill.plugin_title.clone()
                };
                Some(AiContextPart::SkillFile {
                    path: skill_match.skill.path.to_string_lossy().to_string(),
                    label: skill_match.skill.title.clone(),
                    skill_name: skill_match.skill.title.clone(),
                    owner_label: owner,
                    slash_name: skill_match.skill.skill_id.clone(),
                })
            }
            _ => None,
        }
    }

    /// Open a full built-in view as an attachment portal. The user browses
    /// files or clipboard entries; Enter attaches the selection back to the
    /// Agent Chat chat, Escape cancels and returns.
    pub(crate) fn open_attachment_portal(
        &mut self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
        cx: &mut Context<Self>,
    ) {
        use crate::ai::context_selector::types::ContextPortalKind;

        // Prevent nesting — only one portal at a time.
        if self.is_in_attachment_portal() {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "attachment_portal_nested_prevented",
            );
            return;
        }

        // Save the current view and focus target for restoration on return.
        // The portal is always opened from AgentChatView, so ChatPrompt is correct.
        self.attachment_portal_return_view = Some(self.current_view.clone());
        self.attachment_portal_return_focus_target = Some(FocusTarget::ChatPrompt);
        self.attachment_portal_return_width = crate::platform::get_main_window_bounds()
            .map(|(_, _, width, _)| width as f32)
            .filter(|width| width.is_finite() && *width > 0.0);
        self.attachment_portal_host_snapshot = Some(self.capture_attachment_portal_host_snapshot());
        self.active_attachment_portal_kind = Some(kind);
        self.transition_agent_chat_surface(
            crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::PortalOpened { kind },
        );

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_opened",
            kind = ?kind,
            return_width = ?self.attachment_portal_return_width,
        );

        let portal_query = if let Some(AppView::AgentChatView { entity }) =
            self.attachment_portal_return_view.as_ref()
        {
            entity.update(cx, |view, _cx| {
                view.portal_query_for(kind).unwrap_or_default()
            })
        } else {
            String::new()
        };

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_query_seeded_from_contract",
            kind = ?kind,
            query = %portal_query,
        );

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_picker_query_resolved",
            kind = ?kind,
            query = %portal_query,
        );

        if let Some(AppView::AgentChatView { entity }) = self.attachment_portal_return_view.as_ref()
        {
            let entity = entity.clone();
            entity.update(cx, |view, cx| {
                view.prepare_for_attachment_portal_open(cx);
            });
        }

        match kind {
            ContextPortalKind::FileSearch => {
                self.open_file_search(portal_query, cx);
            }
            ContextPortalKind::BrowserHistory => {
                cx.spawn(async move |this, cx| {
                    let result = crate::browser_history::list_recent_history(500);
                    this.update(cx, |this, cx| {
                        if let Ok(entries) = result {
                            this.cached_browser_history = entries;
                            cx.notify();
                        }
                    })
                    .ok();
                })
                .detach();

                self.open_builtin_filterable_view_with_filter(
                    AppView::BrowserHistoryView {
                        filter: portal_query.clone(),
                        selected_index: 0,
                    },
                    &portal_query,
                    "Search browser history...",
                    true,
                    cx,
                );
            }
            ContextPortalKind::BrowserTabs => {
                cx.spawn(async move |this, cx| {
                    let result = crate::browser_tabs::list_open_tabs();
                    this.update(cx, |this, cx| {
                        match result {
                            Ok(tabs) => {
                                this.cached_browser_tabs = tabs;
                            }
                            Err(error) => {
                                tracing::warn!(
                                    target: "script_kit::agent_chat",
                                    event = "browser_tabs_portal_load_failed",
                                    error = %error,
                                );
                                this.cached_browser_tabs.clear();
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();

                self.open_builtin_filterable_view_with_filter(
                    AppView::BrowserTabsView {
                        filter: portal_query.clone(),
                        selected_index: 0,
                    },
                    &portal_query,
                    "Search open browser tabs...",
                    true,
                    cx,
                );
            }
            ContextPortalKind::ClipboardHistory => {
                self.open_clipboard_history_surface_with_filter(portal_query.clone(), cx);
            }
            ContextPortalKind::DictationHistory => {
                self.open_builtin_filterable_view_with_filter(
                    AppView::DictationHistoryView {
                        filter: portal_query.clone(),
                        selected_index: 0,
                    },
                    &portal_query,
                    "Search dictation history...",
                    true,
                    cx,
                );
            }
            ContextPortalKind::ScriptSearch => {
                self.open_script_list_attachment_portal(
                    kind,
                    &portal_query,
                    "Search scripts...",
                    cx,
                );
            }
            ContextPortalKind::ScriptletSearch => {
                self.open_script_list_attachment_portal(
                    kind,
                    &portal_query,
                    "Search scriptlets...",
                    cx,
                );
            }
            ContextPortalKind::SkillSearch => {
                self.open_script_list_attachment_portal(
                    kind,
                    &portal_query,
                    "Search skills...",
                    cx,
                );
            }
            ContextPortalKind::NotesBrowse => {
                self.open_builtin_filterable_view_with_filter(
                    AppView::NotesBrowseView {
                        search: crate::notes::search_model::NoteSearchHostState::load(
                            portal_query.clone(),
                            crate::notes::search_model::NoteSearchDestination::AttachNote,
                            &crate::notes::notes_brain_days_dir(),
                        ),
                    },
                    &portal_query,
                    "Search notes...",
                    true,
                    cx,
                );
            }
            ContextPortalKind::AgentChatHistory => {
                self.open_builtin_filterable_view_with_filter(
                    AppView::AgentChatHistoryView {
                        filter: portal_query.clone(),
                        selected_index: 0,
                    },
                    &portal_query,
                    "Search conversation history...",
                    true,
                    cx,
                );
            }
            ContextPortalKind::Terminal => {
                self.open_quick_terminal(None, cx);
            }
        }

        self.record_attachment_portal_open_width();
        cx.notify();
    }

    /// Close the attachment portal and attach the selected part: to the Agent
    /// Chat chat for agent-chat-hosted portals, or as a resolved compact
    /// `@file:` token for ScriptList-hosted spine portals.
    pub(crate) fn close_attachment_portal_with_part(
        &mut self,
        part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        let spine_segment = self.spine_mention_portal_segment.take();
        let return_view = self
            .attachment_portal_return_view
            .take()
            .unwrap_or(AppView::ScriptList);
        let return_focus_target = self
            .attachment_portal_return_focus_target
            .take()
            .unwrap_or(FocusTarget::MainFilter);

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_closed_with_part",
            focus_target = ?return_focus_target,
        );

        self.restore_attachment_portal_return_view(return_view.clone(), return_focus_target);

        // ScriptList-hosted spine portal: resolve the `@file` segment in the
        // restored filter and skip the agent-chat surface transitions — the
        // agent-chat machine never entered its portal state for this host.
        if let Some(segment_byte_range) = spine_segment {
            self.resolve_spine_mention_from_portal_part(segment_byte_range, &part, cx);
            cx.notify();
            return;
        }
        // Drive the placement machine back to Embedded when the portal
        // is returning to the chat view, otherwise Hidden — the portal
        // can only have been opened from an embedded Agent Chat host, so
        // EmbeddedOpened from a non-chat return means host state has
        // drifted; downgrade to Hidden rather than a silent no-op.
        self.transition_agent_chat_surface(
            crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::PortalClosed,
        );
        if !matches!(return_view, AppView::AgentChatView { .. }) {
            self.transition_agent_chat_surface(
                crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::EmbeddedClosed,
            );
        }

        // Stage the context part with an inline @mention token.
        // Uses the canonical token format so the mention sync system can track
        // it — deleting characters from the mention removes the part.
        if let AppView::AgentChatView { entity } = &return_view {
            let entity = entity.clone();
            entity.update(cx, |view, cx| {
                view.resume_after_attachment_portal_close(cx);
                view.attach_portal_part(part.clone(), cx);
                cx.notify();
            });
        }

        cx.notify();
    }

    /// Close the attachment portal without attaching anything (Escape).
    pub(crate) fn close_attachment_portal_cancel(&mut self, cx: &mut Context<Self>) {
        let spine_segment = self.spine_mention_portal_segment.take();
        let portal_kind = self.active_attachment_portal_kind;
        let return_view = self
            .attachment_portal_return_view
            .take()
            .unwrap_or(AppView::ScriptList);
        let return_focus_target = self
            .attachment_portal_return_focus_target
            .take()
            .unwrap_or(FocusTarget::MainFilter);

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "attachment_portal_cancelled",
            kind = ?portal_kind,
            focus_target = ?return_focus_target,
        );

        self.restore_attachment_portal_return_view(return_view.clone(), return_focus_target);

        // ScriptList-hosted spine portal: the snapshot restore already put the
        // pre-portal `@file` filter text back; no agent-chat surface
        // transitions or entity hooks apply to this host.
        if spine_segment.is_some() {
            cx.notify();
            return;
        }
        // Same split as the accept path: PortalClosed first (the
        // default return lands back in Embedded), then EmbeddedClosed
        // if the restored view is not the Agent Chat chat.
        self.transition_agent_chat_surface(
            crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::PortalClosed,
        );
        if !matches!(return_view, AppView::AgentChatView { .. }) {
            self.transition_agent_chat_surface(
                crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::EmbeddedClosed,
            );
        }

        if let (Some(kind), AppView::AgentChatView { entity }) = (portal_kind, &return_view) {
            let entity = entity.clone();
            entity.update(cx, |view, cx| {
                view.resume_after_attachment_portal_close(cx);
                view.cancel_pending_portal_session(kind, cx);
            });
        }

        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn attachment_portal_source() -> String {
        fs::read_to_string("src/app_impl/attachment_portal.rs")
            .expect("Failed to read src/app_impl/attachment_portal.rs")
    }

    #[test]
    fn attachment_portal_restore_helper_reapplies_window_size_contract() {
        let source = attachment_portal_source();
        let helper_start = source
            .find("fn restore_attachment_portal_return_view(")
            .expect("restore helper must exist");
        let helper_body = &source[helper_start..];

        assert!(
            helper_body.contains("self.resize_current_view_to_width(return_width);")
                || helper_body.contains("self.update_window_size();"),
            "restore helper must reapply the originating surface window size"
        );
    }

    #[test]
    fn attachment_portal_exit_paths_use_shared_restore_helper() {
        let source = attachment_portal_source();
        let helper_calls = source
            .matches("self.restore_attachment_portal_return_view(")
            .count();

        assert!(
            helper_calls >= 2,
            "attach + cancel portal exits must share the same restore helper"
        );
    }

    #[test]
    fn attachment_portal_open_captures_return_width() {
        let source = attachment_portal_source();

        assert!(
            source.contains(
                "self.attachment_portal_return_width = crate::platform::get_main_window_bounds()"
            ),
            "portal open should capture the pre-portal main-window width"
        );
    }

    #[test]
    fn attachment_portal_cancel_clears_agent_chat_portal_session() {
        let source = attachment_portal_source();

        assert!(
            source.contains("view.cancel_pending_portal_session(kind, cx);"),
            "cancel should clear any staged Agent Chat portal session after restoring the host view"
        );
    }

    #[test]
    fn attachment_portal_open_captures_host_snapshot() {
        let source = attachment_portal_source();

        assert!(
            source.contains(
                "self.attachment_portal_host_snapshot = Some(self.capture_attachment_portal_host_snapshot());"
            ),
            "portal open should capture shared launcher host state before mutating it"
        );
    }

    #[test]
    fn attachment_portal_restore_respects_user_resized_width() {
        let source = attachment_portal_source();

        assert!(
            source.contains("attachment_portal_width_restore_skipped_user_resize"),
            "restore should log when it preserves a manual resize instead of snapping back"
        );
    }

    #[test]
    fn attachment_portal_calls_agent_chat_prepare_and_resume_hooks() {
        let source = attachment_portal_source();

        assert!(
            source.contains("view.prepare_for_attachment_portal_open(cx);"),
            "portal open should let Agent Chat dismiss popup/setup surfaces before the host switch"
        );
        assert!(
            source.contains("view.resume_after_attachment_portal_close(cx);"),
            "portal close should notify Agent Chat after the host view is restored"
        );
    }
}
