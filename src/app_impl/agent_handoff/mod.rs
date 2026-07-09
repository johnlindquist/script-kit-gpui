//! Tab-AI mode: Agent Chat entry, context staging, focused-text launch, and AI-surface transitions.

use super::*;

mod agent_chat_context_staging;
mod agent_chat_entry;
mod agent_chat_launch;
mod agent_chat_setup;
mod focused_text_entry;
mod source_classification;
mod types;
use agent_chat_context_staging::materialize_selection_context_parts;
use source_classification::{
    app_view_to_prompt_type_str, build_tab_ai_apply_back_hint, detect_tab_ai_source_type,
    detect_tab_ai_source_type_early,
};
pub(crate) use types::*;

static AGENT_CHAT_OBSERVED_STATE_SYNC_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[allow(dead_code)]
const AGENT_CHAT_ONBOARDING_SOURCE_AUDIT_ANCHORS: &str = r#"
load_agent_chat_agent_catalog_entries
resolve_agent_chat_launch_with_requirements
preferred_agent_id.as_deref()
launch_requirements: requirements
implicit_codex_default_active
retry_request.is_some()
preferred_agent_id.is_none()
preferred_agent_id.as_deref() == selected_agent_id.as_deref()
agent_chat_preferred_agent_post_launch_persist_decision
agent_chat_preferred_agent_preserved_during_fallback_launch
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabAiHarnessCloseDisposition {
    RestoreOrigin,
    CloseMainWindowStateFirst,
}

impl ScriptListApp {
    /// Give the Agent Chat chat one frame to paint before deferred context staging runs.
    const AGENT_CHAT_CONTEXT_FIRST_PAINT_DELAY_MS: u64 = 16;
    /// Upper bound for Ask Anything / ambient desktop capture before bootstrap fails open.
    const TAB_AI_DEFERRED_CAPTURE_TIMEOUT_SEC: u64 = 30;

    fn wire_embedded_agent_chat_footer_callbacks(
        &mut self,
        view_entity: &Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut Context<Self>,
    ) {
        let app_entity = cx.entity().clone();
        view_entity.update(cx, |view, _cx| {
            let actions_app = app_entity.clone();
            view.set_on_toggle_actions(move |window, cx| {
                actions_app.update(cx, |app, cx| {
                    app.toggle_actions(cx, window);
                });
            });

            let close_app = app_entity.clone();
            view.set_on_close_requested(move |window, cx| {
                close_app.update(cx, |app, cx| {
                    app.close_tab_ai_harness_terminal_with_window(window, cx);
                });
            });

            let close_window_app = app_entity.clone();
            view.set_on_close_window_requested(move |window, cx| {
                close_window_app.update(cx, |app, cx| {
                    app.close_tab_ai_harness_terminal_with_window(window, cx);
                    app.close_and_reset_window(cx);
                });
            });

            let history_app = app_entity.clone();
            view.set_on_open_history_command(move |window, cx| {
                history_app.update(cx, |app, cx| {
                    app.open_agent_chat_history_main_list(window, cx);
                });
            });

            let paste_response_app = app_entity.clone();
            view.set_on_paste_response_requested(move |_window, cx| {
                paste_response_app.update(cx, |app, cx| {
                    app.paste_latest_agent_chat_response_to_frontmost(None, cx);
                });
            });

            let focused_text_expand_app = app_entity.clone();
            view.set_on_focused_text_expand_requested(move |cx| {
                focused_text_expand_app.update(cx, |app, cx| {
                    app.set_main_window_mode_state_only(
                        MainWindowMode::Full,
                        cx,
                        "focused_text_expand_agent_chat",
                    );
                    crate::window_resize::resize_to_view_sync(
                        crate::window_resize::ViewType::DivPrompt,
                        0,
                    );
                    cx.notify();
                });
            });

            let focused_text_collapse_app = app_entity.clone();
            view.set_on_focused_text_collapse_requested(move |cx| {
                focused_text_collapse_app.update(cx, |app, cx| {
                    app.set_main_window_mode_state_only(
                        MainWindowMode::Mini,
                        cx,
                        "focused_text_collapse_agent_chat",
                    );
                });
            });

            let portal_app = app_entity.clone();
            view.set_on_open_portal(move |kind, cx| {
                portal_app.update(cx, |app, cx| {
                    app.open_attachment_portal(kind, cx);
                });
            });

            let profile_app = app_entity.clone();
            view.set_on_profile_selected(move |profile_id, cx| {
                profile_app.update(cx, |app, cx| {
                    app.select_agent_chat_profile_and_relaunch(profile_id, cx);
                });
            });
        });

        // Observe the Agent Chat view for ready-script state and footer-status changes
        // owned by the child view. The child notification can fire while the
        // AgentChatView entity is still being updated. The mention/slash picker
        // can also emit a burst of child notifications while rebuilding, so
        // debounce and keep only the latest parent sync before reading from the
        // child entity.
        cx.observe(view_entity, move |this, view_entity, cx| {
            this.schedule_embedded_agent_chat_observed_state_sync(view_entity, cx);
        })
        .detach();
    }

    fn select_agent_chat_profile_and_relaunch(
        &mut self,
        profile_id: String,
        cx: &mut Context<Self>,
    ) {
        let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        let mut prefs = crate::config::load_user_preferences();
        let Some(profile) = crate::ai::agent_chat::profiles::persist_agent_chat_profile_selection(
            &mut prefs.ai,
            &profile_id,
            &ctx,
        ) else {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    format!("Agent Chat profile not found: {profile_id}"),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
            return;
        };

        if let Err(error) = crate::config::save_user_preferences(&prefs) {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    format!("Failed to save Agent Chat profile: {error}"),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_profile_persisted",
            profile_id = %profile.id,
            profile_name = %profile.name,
            backend = ?profile.backend,
        );
        self.refresh_agent_model_footer_labels();
        self.prewarm_selected_agent_chat_profile_for_current_cwd(
            &prefs.ai,
            "agent_chat_profile_selection",
        );

        if let AppView::AgentChatView { entity } = self.current_view.clone() {
            if entity.read(cx).is_setup_mode() {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_profile_selection_relaunch_from_setup",
                    profile_id = %profile.id,
                );
                self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                return;
            }

            entity.update(cx, |view, cx| {
                view.set_profile_display(
                    profile.id.clone(),
                    profile.name.clone(),
                    profile.icon_name.clone(),
                    cx,
                );
            });
            cx.notify();
            return;
        }

        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
    }

    fn schedule_embedded_agent_chat_observed_state_sync(
        &mut self,
        view_entity: Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut Context<Self>,
    ) {
        let view_weak = view_entity.downgrade();
        let generation = AGENT_CHAT_OBSERVED_STATE_SYNC_GENERATION
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;

            let _ = cx.update(|cx| {
                if AGENT_CHAT_OBSERVED_STATE_SYNC_GENERATION
                    .load(std::sync::atomic::Ordering::Relaxed)
                    != generation
                {
                    return;
                }
                let Some(app) = this.upgrade() else {
                    return;
                };
                let Some(view_entity) = view_weak.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.sync_embedded_agent_chat_observed_state(&view_entity, cx);
                });
            });
        })
        .detach();
    }

    fn sync_embedded_agent_chat_observed_state(
        &mut self,
        view_entity: &Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut Context<Self>,
    ) {
        let view = view_entity.read(cx);
        let new_path = view.ready_script_path();
        let ready_script_path_changed = self.agent_chat_ready_script_path != new_path;
        let visible_agent_chat_view_changed = matches!(
            &self.current_view,
            AppView::AgentChatView { entity } if entity == view_entity
        );
        let footer_status_changed = if visible_agent_chat_view_changed && !view.is_setup_mode() {
            let snapshot = view.footer_snapshot(cx);
            let changed = self.agent_chat_footer_snapshot.as_ref() != Some(&snapshot);
            if changed {
                self.agent_chat_footer_dot_status = Some(snapshot.dot_status);
                self.agent_chat_footer_model_display = Some(snapshot.model_display.clone());
                self.agent_chat_footer_snapshot = Some(snapshot);
            }
            changed
        } else {
            false
        };
        if ready_script_path_changed {
            self.agent_chat_ready_script_path = new_path;
        }
        if ready_script_path_changed || footer_status_changed {
            cx.notify();
        }
    }

    /// Open the Agent Chat compatibility surface (zero-intent).
    ///
    /// Routes to the harness terminal (`QuickTerminalView`), which connects
    /// to a pre-running CLI harness (Claude Code, Codex, AGY, etc.)
    /// and injects a flat text-native context block via PTY stdin.
    pub(crate) fn open_tab_ai_chat(&mut self, cx: &mut Context<Self>) {
        self.open_tab_ai_chat_with_entry_intent(None, cx);
    }

    /// Compatibility Agent Chat entry point.
    ///
    /// - `None` => open the harness and stage context only (`PasteOnly`)
    /// - `Some(intent)` => open the harness and immediately submit that intent
    pub(crate) fn open_tab_ai_chat_with_entry_intent(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(
            entry_intent,
            crate::ai::TabAiCaptureKind::DefaultContext,
            false,
            cx,
        );
    }

    pub(crate) fn open_tab_ai_chat_with_entry_intent_suppressing_focused_part(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(
            entry_intent,
            crate::ai::TabAiCaptureKind::DefaultContext,
            true,
            cx,
        );
    }

    /// Unified spine Enter dispatch: try the selected row's action, then fall
    /// back to submitting the prompt plan. Returns true if the spine consumed
    /// the Enter, false to let the caller's default action run.
    pub(crate) fn try_handle_spine_enter(
        &mut self,
        window: &mut Window,
        ctx: &mut Context<Self>,
    ) -> bool {
        if !self.spine_projection_owns_main_list() {
            return false;
        }
        // Unarmed empty colon mode (`@clipboard:` showing recents with no
        // selected row): Enter is consumed but attaches nothing — a reflexive
        // double-Enter must never insert an unseen recent item. Down or a
        // click arms the selection first.
        if self.spine_empty_subsearch_selection_suppressed() {
            tracing::info!(
                target: "script_kit::spine",
                event = "empty_context_subsearch_enter_consumed",
            );
            return true;
        }
        // accept_spine_projection_row handles rich subsearch interception
        // (@file:, @clipboard:, etc.) before applying the row's action. If it
        // succeeds, the Spine consumed Enter. Otherwise (e.g. Noop placeholder,
        // no selection), fall through to plan submission.
        if self.accept_spine_projection_row(window, ctx) {
            return true;
        }
        if self.try_submit_spine_prompt_plan_from_enter(ctx) {
            return true;
        }
        // Structural guard: while a `@source:` subsearch segment is active,
        // Enter must never fall through to the launcher's default row
        // execution (opening the note / running the script / pasting the
        // clipboard entry) — that destroys the prompt being built. Consume
        // the Enter even when no row attached.
        if let Some(projection) = self.spine_projection.as_ref() {
            if let crate::spine::SpineSegmentKind::ContextMention {
                context_type,
                sub_query,
            } = &projection.active_segment_kind
            {
                if crate::spine::catalog_subsearch::parse_context_subsearch(
                    context_type,
                    sub_query.as_deref(),
                )
                .is_some()
                {
                    tracing::info!(
                        target: "script_kit::spine",
                        event = "spine_subsearch_enter_consumed_without_attach",
                        context_type = %context_type,
                    );
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn try_submit_spine_prompt_plan_from_enter(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.spine_enabled || !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }

        let raw = self.filter_text.clone();
        if raw.trim().is_empty() {
            return false;
        }

        self.set_spine_parse_from_filter_and_cursor(&raw, raw.len());
        self.try_submit_spine_prompt_plan_from_parse(self.spine_parse.clone(), true, true, true, cx)
    }

    pub(crate) fn try_submit_spine_prompt_plan_from_parse(
        &mut self,
        parse: crate::spine::SpineParse,
        reset_launcher_filter: bool,
        clear_aliases_after_submit: bool,
        use_selection_preview_cache: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let mention_aliases = self.spine_mention_aliases.clone();
        self.try_submit_spine_prompt_plan_from_parse_with_aliases(
            parse,
            &mention_aliases,
            reset_launcher_filter,
            clear_aliases_after_submit,
            use_selection_preview_cache,
            cx,
        )
    }

    pub(crate) fn try_submit_spine_prompt_plan_from_parse_with_aliases(
        &mut self,
        parse: crate::spine::SpineParse,
        mention_aliases: &std::collections::HashMap<
            String,
            crate::ai::message_parts::AiContextPart,
        >,
        reset_launcher_filter: bool,
        clear_aliases_after_submit: bool,
        use_selection_preview_cache: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let plan = Self::spine_prompt_plan_for_aliases(&parse, mention_aliases);
        // Plain prose without sigils normally doesn't submit to chat. But when
        // the user has already established a working directory via the cwd
        // chip (typed `>` then picked a folder), Cmd+Enter on a non-empty
        // prompt should dispatch to Agent Chat with that cwd. The chip is itself a
        // context anchor.
        let cwd_anchor_authorizes_submit =
            self.spine_cwd.is_some() && !plan.normalized_prompt.trim().is_empty();
        if !plan.should_submit_to_chat() && !cwd_anchor_authorizes_submit {
            return false;
        }

        // Style-only rewrites (`.` + pick a style) route into the instant
        // rewrite mini instead of the full Agent Chat transcript: three fast
        // variations, Tab to choose, Enter to paste back — and no `.` sigil
        // surfaced anywhere. Falls back to the legacy Agent Chat handoff when
        // the focused field can't be captured over AX (e.g. web apps that
        // only work through the Cmd+C selection fallback).
        if plan.selected_style.is_some()
            && crate::spine::prompt_plan::spine_parse_is_style_only(&parse)
        {
            if let Some(snapshot) = self.capture_rewrite_snapshot("spine_style_rewrite") {
                let style = plan.selected_style.clone().expect("checked above");
                tracing::info!(
                    target: "script_kit::spine",
                    event = "spine_style_rewrite_routed_to_mini",
                    style = %style.id,
                    chars = snapshot.metrics.chars,
                    app_name = %snapshot.app.name,
                );
                if reset_launcher_filter {
                    self.reset_spine_launcher_filter_after_submit();
                }
                if clear_aliases_after_submit {
                    self.spine_mention_aliases.clear();
                }
                self.open_focused_text_agent_chat_from_snapshot(
                    snapshot,
                    Some(style.instruction),
                    "spine_style_rewrite",
                    cx,
                );
                if let AppView::AgentChatView { entity } = self.current_view.clone() {
                    entity.update(cx, |chat, cx| {
                        if let Err(error) = chat.submit_instant_rewrite(cx) {
                            tracing::warn!(
                                target: "script_kit::focused_text",
                                event = "instant_rewrite_submit_failed",
                                source = "spine_style_rewrite",
                                error = %error,
                            );
                        }
                    });
                }
                cx.notify();
                return true;
            }
        }

        let prompt = plan.normalized_prompt.trim().to_string();
        // Freeze `@selection` into real text NOW, while the launcher panel is
        // still a non-activating overlay and the source app owns focus. The
        // Selection part is otherwise a lazy `kit://context?selectedText=1`
        // resource resolved at thread submit time — by then Agent Chat has
        // taken over app activation and the AX/Cmd+C capture reads from the
        // wrong window ("No selected text was provided").
        let cached_selection = if use_selection_preview_cache {
            let preview = &self.spine_live_preview_cache.current;
            preview
                .selection_text
                .clone()
                .map(|text| crate::selected_text::CapturedText {
                    text,
                    kind: if preview.selection_is_draft {
                        crate::selected_text::CapturedTextKind::FocusedField
                    } else {
                        crate::selected_text::CapturedTextKind::Selection
                    },
                    source_app: preview
                        .selection_source_app
                        .clone()
                        .or_else(|| preview.frontmost_app_name.clone()),
                })
        } else {
            None
        };
        // Selection-or-focused-field: a user who invokes a rewrite while
        // typing (no selection) means "rewrite my draft", so fall back to the
        // focused field's text rather than submitting an empty attachment.
        let parts = materialize_selection_context_parts(
            plan.context_parts.clone(),
            cached_selection,
            || {
                crate::selected_text::capture_selection_or_focused_text()
                    .ok()
                    .flatten()
            },
        );
        let selected_profile_name = if let Some(profile) = plan.selected_profile.as_ref() {
            let mut prefs = crate::config::load_user_preferences();
            let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
            let persisted = crate::ai::agent_chat::profiles::persist_agent_chat_profile_selection(
                &mut prefs.ai,
                &profile.id,
                &ctx,
            )
            .and_then(|entry| {
                if crate::config::save_user_preferences(&prefs).is_ok() {
                    Some(entry.name)
                } else {
                    None
                }
            });
            if persisted.is_none() {
                tracing::warn!(
                    target: "script_kit::spine",
                    event = "spine_prompt_plan_profile_persist_failed",
                    profile_id = %profile.id,
                );
            } else {
                self.prewarm_selected_agent_chat_profile_for_current_cwd(
                    &prefs.ai,
                    "spine_prompt_plan_profile_selection",
                );
            }
            persisted
        } else {
            None
        };

        if prompt.is_empty() && parts.is_empty() {
            if let Some(profile_name) = selected_profile_name {
                self.show_hud(
                    format!("Default Agent Chat profile: {profile_name}"),
                    Some(HUD_SHORT_MS),
                    cx,
                );
                cx.notify();
                return true;
            }
            return false;
        }

        let handoff_source = "spine_prompt_plan";
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_prompt_plan_submit",
            raw_len = plan.raw_input.len(),
            prompt_len = prompt.len(),
            context_count = parts.len(),
            warning_count = plan.unknown_warnings.len(),
            profile = ?plan.selected_profile.as_ref().map(|p| p.id.as_str()),
            style = ?plan.selected_style.as_ref().map(|s| s.id.as_str()),
        );
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_reused_entry_intent_with_host_context_submitted",
            source = handoff_source,
            intent_len = prompt.len(),
            context_part_count = parts.len(),
            alias_count = mention_aliases.len(),
            unknown_warning_count = plan.unknown_warnings.len(),
        );

        if reset_launcher_filter {
            self.reset_spine_launcher_filter_after_submit();
        }
        // Launcher input aliases are tied to a cleared ephemeral prompt. Day
        // Page aliases may still back visible note text, so callers decide
        // when clearing them is appropriate.
        if clear_aliases_after_submit {
            self.spine_mention_aliases.clear();
        }

        let spine_cwd = self.spine_cwd.clone();
        self.embedded_agent_chat = None;
        self.open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(None, cx);

        if let AppView::AgentChatView { entity } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat, cx| {
                if let Some(thread_entity) = chat.thread() {
                    thread_entity.update(cx, |thread, cx| {
                        thread.clear_messages(cx);
                        if let Some(cwd) = &spine_cwd {
                            thread.set_cwd(cwd.clone());
                        }
                    });
                }
                if let Err(e) = chat.submit_reused_entry_intent_with_host_context(
                    prompt,
                    parts,
                    handoff_source,
                    cx,
                ) {
                    tracing::warn!(
                        target: "script_kit::spine",
                        event = "spine_prompt_plan_agent_chat_submit_failed",
                        error = %e,
                    );
                }
            });
        }

        cx.notify();
        true
    }

    fn reset_spine_launcher_filter_after_submit(&mut self) {
        self.filter_text.clear();
        self.computed_filter_text.clear();
        self.pending_filter_sync = true;
        self.spine_parse = Default::default();
        self.spine_projection = None;
        self.spine_empty_subsearch_armed_for = None;
        self.spine_live_preview_cache = Default::default();
        self.invalidate_grouped_cache();
    }

    fn spine_prompt_plan_for_aliases(
        parse: &crate::spine::SpineParse,
        mention_aliases: &std::collections::HashMap<
            String,
            crate::ai::message_parts::AiContextPart,
        >,
    ) -> crate::spine::prompt_plan::SpinePromptPlan {
        crate::spine::prompt_plan::build_spine_prompt_plan_with_aliases(parse, mention_aliases)
    }

    /// Entry point that always routes to Agent Chat chat, bypassing the surface
    /// preference routing that may redirect to the quick terminal.
    ///
    /// Used by the Auto Submit fallback so it always opens the Agent Chat chat
    /// experience regardless of new-script detection heuristics.
    pub(crate) fn open_tab_ai_agent_chat_with_entry_intent(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_agent_chat_with_entry_intent_variant(
            entry_intent,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            cx,
        );
    }

    pub(crate) fn open_tab_ai_agent_chat_with_entry_intent_variant(
        &mut self,
        entry_intent: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) {
        let suppress_focused_part =
            ui_variant != crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard;
        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest::main_launcher_with_variant(
                entry_intent,
                suppress_focused_part,
                ui_variant,
            ),
            cx,
        );
    }

    /// Quick AI: launcher Tab-with-text entry. Sends `query` as the first
    /// turn of a fresh zero-context session (Quick AI profile — spark model,
    /// no tools/skills/memories/context) and clears the launcher filter so
    /// returning from the chat lands on a clean list.
    ///
    /// The `QuickAi` ui variant does the heavy lifting downstream: it forces
    /// `suppress_focused_part`, skips detached/embedded chat reuse, and routes
    /// launch resolution to `resolve_quick_ai_pi_launch`.
    pub(crate) fn open_quick_ai_from_launcher(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "quick_ai_launcher_entry",
            query_chars = query.chars().count(),
        );
        self.clear_filter(window, cx);
        self.open_tab_ai_agent_chat_with_entry_intent_variant(
            Some(query),
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::QuickAi,
            cx,
        );
    }

    /// Entry point for direct prompt handoffs that should not inherit the
    /// currently selected launcher row as Agent Chat context.
    pub(crate) fn open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest::main_launcher(entry_intent, true),
            cx,
        );
    }

    /// Binary-side body for the detached chat window's "reattach into main
    /// panel" action. Registered with
    /// `chat_window::register_reattach_into_main_hook` at startup because the
    /// dual-compiled chat window code cannot name `ScriptListApp`.
    pub(crate) fn reattach_detached_chat_hook(cx: &mut gpui::App) -> bool {
        let Some(handle) = crate::get_main_window_handle() else {
            return false;
        };
        handle
            .update(cx, |any_view, _window, cx| {
                let Ok(root) = any_view.downcast::<gpui_component::Root>() else {
                    return false;
                };
                let inner = root.read(cx).view().clone();
                let Ok(app) = inner.downcast::<ScriptListApp>() else {
                    return false;
                };
                app.update(cx, |app, cx| {
                    app.reattach_embedded_agent_chat_from_detached(cx);
                    app.dispatch_window_event(
                        crate::window_orchestrator::WindowEvent::ShowMain { activate_app: true },
                        cx,
                    );
                });
                true
            })
            .unwrap_or(false)
    }

    /// Reattach flow for the "Return to Panel" action on a detached Agent Chat chat.
    ///
    /// The detached window owns the live [`AgentChatThread`] entity — and may
    /// have switched threads since the detach — so reattach pulls the CURRENT
    /// thread out of the detached view BEFORE closing the window, then builds
    /// a fresh embedded view around that same thread entity. Message history,
    /// pending parts, and thread identity survive the round trip. Only when
    /// no detached view/thread exists do we fall back to the cached embedded
    /// view, then to a fresh launch.
    pub(crate) fn reattach_embedded_agent_chat_from_detached(&mut self, cx: &mut Context<Self>) {
        let detached_thread =
            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
                .and_then(|view| view.read(cx).thread());
        crate::ai::agent_chat::ui::chat_window::close_chat_window(cx);

        if let Some(thread) = detached_thread {
            let view_entity =
                cx.new(|cx| crate::ai::agent_chat::ui::AgentChatView::new(thread, cx));
            self.wire_embedded_agent_chat_footer_callbacks(&view_entity, cx);
            self.embedded_agent_chat = Some(view_entity.clone());
            self.tab_ai_harness_return_view = Some(AppView::ScriptList);
            self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
            self.enter_embedded_agent_chat_surface(view_entity, cx);
            cx.notify();
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_reattach_embedded_from_detached_thread",
                reuse = true,
            );
            return;
        }

        if self.try_reuse_embedded_agent_chat_view(
            None,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            cx,
        ) {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_reattach_embedded_reused",
                reuse = true,
            );
            return;
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_reattach_embedded_cache_miss_fresh_launch",
            reuse = false,
        );
        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
    }

    pub(crate) fn open_tab_ai_agent_chat_with_entry_intent_preserving_return(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_agent_chat_with_entry_intent_preserving_return_and_options(
            entry_intent,
            false,
            cx,
        );
    }

    fn open_tab_ai_agent_chat_with_entry_intent_preserving_return_and_options(
        &mut self,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        cx: &mut Context<Self>,
    ) {
        let previous_return_view = self.tab_ai_harness_return_view.clone();
        let previous_return_focus_target = self.tab_ai_harness_return_focus_target;
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_entry_intent_return_seeded",
            source_view = ?source_view,
            suppress_focused_part,
            auto_submit = entry_intent
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()),
            return_focus_target = ?self.tab_ai_harness_return_focus_target,
        );
        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest {
                origin: agent_chat_entry::AgentChatEntryOrigin::MainLauncher,
                target: agent_chat_entry::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
                seed_text: entry_intent,
                ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
                seed_policy: agent_chat_entry::AgentChatSeedPolicy::AutoSubmitFirstTurn,
                suppress_focused_part,
                context_staging: if suppress_focused_part {
                    agent_chat_entry::AgentChatContextStaging::SuppressFocused
                } else {
                    agent_chat_entry::AgentChatContextStaging::AmbientOrFocused
                },
                return_origin: Some(source_view.clone()),
            },
            cx,
        );
        if !matches!(self.current_view, AppView::AgentChatView { .. }) {
            self.tab_ai_harness_return_view = previous_return_view;
            self.tab_ai_harness_return_focus_target = previous_return_focus_target;
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_entry_intent_return_restored_without_launch",
                source_view = ?source_view,
            );
        }
    }

    fn open_tab_ai_agent_chat_with_options(
        &mut self,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        let normalized_entry_intent = entry_intent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);

        // Quick AI must always launch its own zero-context session: reusing a
        // detached window or the cached embedded chat would submit the query
        // into the user's ongoing thread — wrong profile, wrong model, and
        // with context the mode promises not to load.
        let quick_ai =
            ui_variant == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::QuickAi;

        if !quick_ai && crate::ai::agent_chat::ui::chat_window::is_chat_window_open() {
            match normalized_entry_intent.as_ref().map(|intent| {
                crate::ai::agent_chat::ui::chat_window::submit_reused_entry_intent_in_detached_chat(
                    intent.clone(),
                    cx,
                )
            }) {
                Some(Ok(true)) => return,
                Some(Err(error)) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_reuse_detached_window_entry_intent_failed",
                        error = %error,
                    );
                }
                _ => {}
            }
        }

        let has_cached_retry_request = self
            .embedded_agent_chat
            .as_ref()
            .is_some_and(|entity| entity.read(cx).has_retry_request());
        let cached_agent_chat_is_setup_mode = self
            .embedded_agent_chat
            .as_ref()
            .is_some_and(|entity| entity.read(cx).is_setup_mode());

        if !quick_ai
            && Self::should_reuse_embedded_agent_chat_view_for_open(
                normalized_entry_intent.as_deref(),
                has_cached_retry_request,
                cached_agent_chat_is_setup_mode,
            )
            && self.try_reuse_embedded_agent_chat_view(entry_intent.clone(), ui_variant, cx)
        {
            return;
        }

        self.begin_tab_ai_harness_entry(
            entry_intent,
            suppress_focused_part,
            None,
            crate::ai::TabAiCaptureKind::DefaultContext,
            true,
            ui_variant,
            cx,
        );
    }

    /// Open Agent Chat Chat with an explicit focused target, bypassing view-based
    /// auto-resolution. Used by Cmd+Enter from surfaces that own plain Enter
    /// locally (action menus, Notes) and want to hand off a canonical
    /// `FocusedTarget` chip to Agent Chat without re-resolving from the current view.
    pub(crate) fn open_tab_ai_agent_chat_with_explicit_target(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        let label = Self::format_tab_ai_focused_chip_label(&target);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_explicit_target_agent_chat_open",
            item_source = %target.source,
            item_kind = %target.kind,
            semantic_id = %target.semantic_id,
            label = %label,
        );

        let focused_part =
            Some(crate::ai::message_parts::AiContextPart::FocusedTarget { target, label });

        if let Some(part) = focused_part.clone() {
            if crate::ai::agent_chat::ui::chat_window::is_chat_window_open() {
                match crate::ai::agent_chat::ui::chat_window::submit_reused_entry_intent_with_host_context_in_detached_chat(
                    String::new(),
                    vec![part],
                    "explicit_target",
                    cx,
                ) {
                    Ok(true) => return,
                    Ok(false) => {}
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "tab_ai_reuse_detached_window_explicit_target_failed",
                            error = %error,
                        );
                    }
                }
            }
        }

        // Minimal snapshot for the launch request — we already know the target.
        let (ui_snapshot, invocation_receipt) = self.snapshot_tab_ai_ui(cx);
        self.tab_ai_harness_capture_generation += 1;

        let request = TabAiLaunchRequest {
            source_view: self.current_view.clone(),
            entry_intent: None,
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            suppress_focused_part: false,
            quick_submit_plan: None,
            ui_snapshot,
            invocation_receipt,
            capture_kind: crate::ai::TabAiCaptureKind::DefaultContext,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        // No ambient capture needed — explicit target path skips desktop snapshot.
        let (_tx, rx) = async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);

        self.open_tab_ai_agent_chat_view_from_request_impl(
            request,
            rx,
            focused_part,
            false, // use_ask_anything_fallback
            None,  // explicit_ambient_chip_label
            true,  // force_agent_chat_surface
            cx,
        );
    }

    pub(crate) fn open_tab_ai_agent_chat_with_explicit_target_preserving_return(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        cx: &mut Context<Self>,
    ) {
        let previous_return_view = self.tab_ai_harness_return_view.clone();
        let previous_return_focus_target = self.tab_ai_harness_return_focus_target;
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_explicit_target_return_seeded",
            source_view = ?source_view,
            return_focus_target = ?self.tab_ai_harness_return_focus_target,
            pending_script_list_trigger = ?self.tab_ai_harness_script_list_trigger,
            semantic_id = %target.semantic_id,
        );
        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest {
                origin: agent_chat_entry::AgentChatEntryOrigin::ActionsDialog,
                target: agent_chat_entry::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
                seed_text: None,
                ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
                seed_policy: agent_chat_entry::AgentChatSeedPolicy::ComposerOnly,
                suppress_focused_part: true,
                context_staging: agent_chat_entry::AgentChatContextStaging::ActionsPayload {
                    target,
                },
                return_origin: Some(source_view.clone()),
            },
            cx,
        );
        if !matches!(self.current_view, AppView::AgentChatView { .. }) {
            self.tab_ai_harness_return_view = previous_return_view;
            self.tab_ai_harness_return_focus_target = previous_return_focus_target;
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_explicit_target_return_restored_without_launch",
                source_view = ?source_view,
            );
        }
    }

    pub(crate) fn open_tab_ai_agent_chat_with_context_part(
        &mut self,
        part: crate::ai::message_parts::AiContextPart,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_context_part_agent_chat_open",
            source,
            label = %part.label(),
        );

        let (ui_snapshot, invocation_receipt) = self.snapshot_tab_ai_ui(cx);
        self.tab_ai_harness_capture_generation += 1;

        let request = TabAiLaunchRequest {
            source_view: self.current_view.clone(),
            entry_intent: None,
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            suppress_focused_part: false,
            quick_submit_plan: None,
            ui_snapshot,
            invocation_receipt,
            capture_kind: crate::ai::TabAiCaptureKind::DefaultContext,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        let (_tx, rx) = async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);

        self.open_tab_ai_agent_chat_view_from_request_impl(
            request,
            rx,
            Some(part),
            false,
            None,
            true,
            cx,
        );
    }

    fn quick_terminal_context_part_from_entity(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let capture = entity.update(cx, |term, _cx| term.terminal_context_capture())?;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "quick_terminal_context_part_captured",
            source = %capture.source,
            bytes = capture.text.len(),
            line_count = capture.line_count,
            truncated = capture.truncated,
        );
        crate::terminal_history::record(crate::terminal_history::TerminalHistoryEntry {
            label: capture.label.clone(),
            source: capture.source.clone(),
            text: capture.text.clone(),
            line_count: capture.line_count,
            truncated: capture.truncated,
            captured_at: chrono::Utc::now().to_rfc3339(),
        });
        Some(crate::ai::message_parts::AiContextPart::TextBlock {
            label: capture.label,
            source: capture.source,
            text: capture.text,
            mime_type: Some("text/x-terminal-transcript".to_string()),
        })
    }

    pub(crate) fn open_agent_chat_with_quick_terminal_output(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(part) = self.quick_terminal_context_part_from_entity(entity, cx) else {
            self.show_hud(
                "No terminal output to attach".to_string(),
                Some(HUD_SHORT_MS),
                cx,
            );
            return false;
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "quick_terminal_output_agent_chat_open",
            source = %part.source(),
            label = %part.label(),
        );
        self.open_tab_ai_agent_chat_with_context_part(part, "quick_terminal_output", cx);
        true
    }

    pub(crate) fn attach_quick_terminal_output_to_portal_origin(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.active_attachment_portal_kind()
            != Some(crate::ai::context_selector::types::ContextPortalKind::Terminal)
        {
            return false;
        }

        let Some(part) = self.quick_terminal_context_part_from_entity(entity.clone(), cx) else {
            self.show_hud(
                "No terminal output to attach".to_string(),
                Some(HUD_SHORT_MS),
                cx,
            );
            return true;
        };

        let _ = entity.update(cx, |term, _cx| {
            let _ = term.terminate_session();
        });
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "quick_terminal_portal_output_attached",
            source = %part.source(),
            label = %part.label(),
        );
        self.close_attachment_portal_with_part(part, cx);
        true
    }

    pub(crate) fn dispatch_quick_terminal_cmd_enter(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::QuickTerminalView { .. }) {
            return false;
        }

        if self.attach_quick_terminal_output_to_portal_origin(entity.clone(), cx) {
            return true;
        }

        if self.quick_terminal_can_apply_back() {
            self.apply_tab_ai_result_from_terminal(entity, cx);
            return true;
        }

        self.open_agent_chat_with_quick_terminal_output(entity, cx)
    }

    pub(crate) fn route_large_script_list_paste_to_agent_chat(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }

        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return false;
        };
        if let Ok(image_data) = clipboard.get_image() {
            let Ok(encoded) = crate::clipboard_history::encode_image_as_png(&image_data) else {
                return false;
            };
            let Some(png_bytes) = crate::clipboard_history::content_to_png_bytes(&encoded) else {
                return false;
            };

            if png_bytes.len() > crate::prompts::chat::MAX_IMAGE_BYTES {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "script_list_clipboard_image_rejected_too_large",
                    size_bytes = png_bytes.len(),
                    max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                );
                return false;
            }

            let Ok(path) = crate::pasted_image::write_png_bytes_to_temp_file(&png_bytes) else {
                return false;
            };
            let prepared = crate::pasted_image::prepare_pasted_image(&path, &[]);
            let label = prepared.token.label.clone();
            let part = crate::ai::message_parts::AiContextPart::FilePath {
                path,
                label: label.clone(),
            };

            tracing::info!(
                target: "script_kit::tab_ai",
                event = "script_list_clipboard_image_routed_to_agent_chat",
                label = %label,
                width = image_data.width,
                height = image_data.height,
                size_bytes = png_bytes.len(),
            );

            self.open_tab_ai_agent_chat_with_context_part(part, "script_list_clipboard_image", cx);
            return true;
        }
        let Ok(text) = clipboard.get_text() else {
            return false;
        };

        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        // A5 decision (2026-06-09): the launcher input is single-line, so any
        // multi-line paste routes to Agent Chat instead of being newline-
        // stripped into the filter. Single-line pastes under the large-paste
        // thresholds still paste inline.
        let prepared =
            crate::pasted_text::prepare_pasted_text_for_single_line_surface(&normalized, &[]);
        let Some(token) = prepared.token else {
            return false;
        };

        let part = crate::ai::message_parts::AiContextPart::TextBlock {
            label: token.label.clone(),
            source: format!("clipboard://pasted-text/{}", uuid::Uuid::new_v4()),
            text: normalized,
            mime_type: Some("text/plain".to_string()),
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "script_list_large_paste_routed_to_agent_chat",
            label = %token.label,
            char_count = token.text.chars().count(),
            line_count = token.text.lines().count().max(1),
        );

        self.open_tab_ai_agent_chat_with_context_part(part, "script_list_large_paste", cx);
        true
    }

    /// Open Agent Chat Chat with a selected plugin skill staged like a slash pick.
    ///
    /// Main-menu skill selection should leave `/{skill} ` in the composer
    /// with the skill attached as pending context. It must not become an
    /// entry intent, because entry intents auto-submit.
    pub(crate) fn open_agent_chat_with_selected_skill(
        &mut self,
        skill: &crate::plugins::PluginSkill,
        cx: &mut Context<Self>,
    ) {
        let owner = if skill.plugin_title.is_empty() {
            &skill.plugin_id
        } else {
            &skill.plugin_title
        };
        let command_text =
            crate::ai::agent_chat::ui::build_skill_slash_command_text(&skill.skill_id);
        let part = crate::ai::agent_chat::ui::build_skill_context_part(
            &skill.title,
            owner,
            &skill.skill_id,
            &skill.path,
        );

        tracing::info!(
            event = "agent_chat_skill_slash_selection_requested",
            plugin_id = %skill.plugin_id,
            skill_id = %skill.skill_id,
            skill_title = %skill.title,
            owner,
            path = %skill.path.display(),
            slash_input = %command_text,
            "Opening Agent Chat with plugin skill staged as a slash selection"
        );

        if let Some(entity) =
            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
        {
            let staged = entity.update(cx, |chat, cx| {
                chat.stage_selected_plugin_skill_from_main_menu(skill, cx)
            });
            if staged {
                crate::ai::agent_chat::ui::chat_window::activate_chat_window(cx);
                return;
            }
        }

        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest {
                origin: agent_chat_entry::AgentChatEntryOrigin::PluginSkill {
                    skill_id: skill.skill_id.clone(),
                },
                target: agent_chat_entry::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
                seed_text: None,
                ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
                seed_policy: agent_chat_entry::AgentChatSeedPolicy::ComposerOnly,
                suppress_focused_part: true,
                context_staging: agent_chat_entry::AgentChatContextStaging::SuppressFocused,
                return_origin: Some(self.current_view.clone()),
            },
            cx,
        );

        if let AppView::AgentChatView { entity } = &self.current_view {
            let staged = entity.update(cx, |chat, cx| {
                chat.stage_selected_plugin_skill_from_main_menu(skill, cx)
            });
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_skill_slash_selection_staged",
                plugin_id = %skill.plugin_id,
                skill_id = %skill.skill_id,
                staged,
                slash_input = %command_text,
                attached_part_label = %part.label(),
            );
        }
    }

    pub(crate) fn open_tab_ai_agent_chat_with_slash_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_ai_harness_script_list_trigger = Some('/');
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_open_from_script_list_trigger",
            trigger = "/",
            current_view = ?self.current_view,
        );
        self.open_tab_ai_agent_chat_with_entry_intent(None, cx);

        let detached_opened =
            crate::ai::agent_chat::ui::chat_window::open_detached_slash_picker(cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_trigger_picker_open_attempt",
            trigger = "/",
            detached_opened,
        );
        if detached_opened {
            return;
        }

        if let AppView::AgentChatView { entity } = &self.current_view {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_trigger_picker_open_embedded_deferred",
                trigger = "/",
            );
            self.schedule_embedded_agent_chat_picker_open(
                window.window_handle(),
                entity.clone(),
                '/',
                cx,
            );
        }
    }

    fn schedule_embedded_agent_chat_picker_open(
        &self,
        window_handle: gpui::AnyWindowHandle,
        entity: gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        trigger: char,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    Self::AGENT_CHAT_CONTEXT_FIRST_PAINT_DELAY_MS,
                ))
                .await;

            let _ = window_handle.update(cx, |_root, window, cx| {
                entity.update(cx, |view, cx| match trigger {
                    '/' => view.open_slash_picker_in_window(window, cx),
                    '@' => {
                        view.set_input("@".to_string(), cx);
                        view.refresh_agent_chat_spine_from_composer(cx);
                    }
                    '|' => view.open_profile_trigger_picker_in_window(window, cx),
                    '.' | ';' | '>' => {
                        view.set_input(trigger.to_string(), cx);
                        view.refresh_agent_chat_spine_from_composer(cx);
                    }
                    _ => {}
                });
            });
        })
        .detach();
    }

    fn try_reuse_embedded_agent_chat_view(
        &mut self,
        entry_intent: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(entity) = self.embedded_agent_chat.as_ref().cloned() else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_embedded_agent_chat_cache_miss",
            );
            return false;
        };

        let is_setup_mode = entity.read(cx).is_setup_mode();
        let normalized_intent = entry_intent
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let source_view = self.current_view.clone();

        if normalized_intent.is_some() && is_setup_mode {
            self.embedded_agent_chat = None;
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_embedded_agent_chat_reuse_rejected_setup_mode",
                source_view = ?source_view,
                auto_submit = true,
                agent_chat_ui_variant = ui_variant.state_id(),
            );
            return false;
        }

        let trigger = self.tab_ai_harness_script_list_trigger;
        entity.update(cx, |chat, cx| {
            chat.opened_via_transient_trigger = trigger;
            chat.set_ui_variant(ui_variant, cx);
        });

        self.tab_ai_harness_return_view = Some(source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.enter_embedded_agent_chat_surface(entity.clone(), cx);

        if let Some(intent) = normalized_intent.clone().filter(|_| !is_setup_mode) {
            entity.update(cx, |chat, cx| {
                chat.submit_reused_entry_intent(intent.clone(), cx);
            });
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_embedded_agent_chat_reused",
            source_view = ?source_view,
            auto_submit = normalized_intent.is_some(),
            is_setup_mode,
            agent_chat_ui_variant = ui_variant.state_id(),
        );
        cx.notify();
        true
    }

    fn should_reuse_embedded_agent_chat_view_for_open(
        entry_intent: Option<&str>,
        has_cached_retry_request: bool,
        cached_agent_chat_is_setup_mode: bool,
    ) -> bool {
        entry_intent.is_some() && !has_cached_retry_request && !cached_agent_chat_is_setup_mode
    }

    fn take_prewarmed_agent_chat_for_launch(
        &mut self,
        selected_agent_id: Option<&str>,
        requirements: crate::ai::agent_chat::ui::AgentChatLaunchRequirements,
        retry_request_active: bool,
        cx: &mut Context<Self>,
    ) -> Option<(
        gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        gpui::Entity<crate::ai::agent_chat::ui::AgentChatThread>,
    )> {
        if retry_request_active
            || requirements != crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default()
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_hot_prewarm_skip",
                correlation_id = "agent_chat_hot_prewarm",
                retry_request_active,
                needs_embedded_context = requirements.needs_embedded_context,
                needs_image = requirements.needs_image,
            );
            return None;
        }

        let Some(view) = self.prewarmed_agent_chat.take() else {
            return None;
        };

        let Some(thread) = view.read(cx).thread() else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_hot_prewarm_discarded",
                correlation_id = "agent_chat_hot_prewarm",
                reason = "not_live",
            );
            self.prewarmed_agent_chat = None;
            return None;
        };

        let thread_selected_agent_id = thread.read(cx).selected_agent_id().map(|id| id.to_string());
        if thread_selected_agent_id.as_deref() != selected_agent_id {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_hot_prewarm_discarded",
                correlation_id = "agent_chat_hot_prewarm",
                reason = "agent_mismatch",
                selected_agent_id = ?selected_agent_id,
                thread_selected_agent_id = ?thread_selected_agent_id,
            );
            self.prewarmed_agent_chat = None;
            return None;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_hot_prewarm_consumed",
            correlation_id = "agent_chat_hot_prewarm",
            selected_agent_id = ?selected_agent_id,
        );
        self.prewarmed_agent_chat = None;
        Some((view, thread))
    }

    /// Entry point with explicit capture kind.
    ///
    /// Used by `SendScreenToAi`, `SendFocusedWindowToAi`, etc. so each
    /// command gets the appropriate screenshot/context capture behaviour
    /// instead of always defaulting to focused-window.
    pub(crate) fn open_tab_ai_chat_with_capture_kind(
        &mut self,
        entry_intent: Option<String>,
        capture_kind: crate::ai::TabAiCaptureKind,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(entry_intent, capture_kind, false, cx);
    }

    fn open_tab_ai_chat_with_capture_kind_and_options(
        &mut self,
        entry_intent: Option<String>,
        capture_kind: crate::ai::TabAiCaptureKind,
        suppress_focused_part: bool,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        self.begin_tab_ai_harness_entry(
            entry_intent,
            suppress_focused_part,
            None,
            capture_kind,
            false,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            cx,
        );
    }

    /// Open the harness with a pre-computed quick-submit plan.
    ///
    /// The plan's `submission_intent()` becomes the entry intent and the
    /// plan's `capture_kind` selects the right screenshot/context profile.
    pub(crate) fn open_tab_ai_chat_with_quick_submit_plan(
        &mut self,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }
        let capture_kind = plan.capture_kind_enum();
        let intent = Some(plan.submission_intent().to_string());
        self.begin_tab_ai_harness_entry(
            intent,
            false,
            Some(plan),
            capture_kind,
            false,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            cx,
        );
    }

    /// Route raw text (from Auto Submit fallback or dictation) through the
    /// quick-submit planner and into the harness — either an existing live
    /// session or a fresh one.
    pub(crate) fn submit_to_current_or_new_tab_ai_harness_from_text(
        &mut self,
        raw_text: String,
        source: crate::ai::TabAiQuickSubmitSource,
        cx: &mut Context<Self>,
    ) {
        let Some(plan) = crate::ai::plan_tab_ai_quick_submit(source, &raw_text) else {
            // Empty input — open the harness without intent.
            self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
            return;
        };

        // If the Agent Chat chat view is active, route through the shared
        // verification-input builder so new-script guidance is appended.
        if let AppView::AgentChatView { ref entity } = self.current_view {
            self.submit_live_agent_chat_tab_ai_from_plan(entity.clone(), plan, cx);
            return;
        }

        // If the PTY harness is already the active surface and alive, route through
        // the full structured submission builder so live-session quick submits
        // get fresh context and quick-submit metadata.
        if let Some(session) = self
            .tab_ai_harness
            .as_ref()
            .filter(|_| matches!(self.current_view, AppView::QuickTerminalView { .. }))
            .filter(|session| session.entity.read(cx).is_alive())
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_quick_submit_live_session",
                source = ?plan.source,
                kind = ?plan.kind,
                capture_kind = %plan.capture_kind,
                input_len = plan.raw_query.len(),
            );

            self.submit_live_tab_ai_harness_from_plan(session.entity.clone(), plan, cx);
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_quick_submit_new_session",
            source = ?plan.source,
            kind = ?plan.kind,
            capture_kind = %plan.capture_kind,
            input_len = plan.raw_query.len(),
        );

        self.open_tab_ai_chat_with_quick_submit_plan(plan, cx);
    }

    /// Submit a quick-submit plan into an already-open, live Agent Chat chat session.
    ///
    /// Routes through `build_tab_ai_agent_chat_initial_input_for_prompt` so that
    /// new-script guidance (including mandatory Bun verification) is
    /// appended when the intent matches, keeping live Agent Chat sessions aligned
    /// with the new-session Agent Chat path.
    fn submit_live_agent_chat_tab_ai_from_plan(
        &mut self,
        entity: gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        let source_view = self
            .tab_ai_harness_return_view
            .clone()
            .unwrap_or(AppView::ScriptList);
        let prompt_type = app_view_to_prompt_type_str(&source_view);

        let surface_preference = crate::ai::harness::tab_ai_surface_preference_for_prompt(
            prompt_type,
            Some(plan.submission_intent()),
            crate::ai::TabAiHarnessSubmissionMode::Submit,
        );

        if surface_preference.use_quick_terminal {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_quick_submit_agent_chat_live_rerouted",
                surface = "quick_terminal",
                reason = "script_verification_required",
                prompt_type = prompt_type,
                source = ?plan.source,
                kind = ?plan.kind,
                capture_kind = %plan.capture_kind,
                includes_script_authoring_skill = surface_preference.includes_script_authoring_skill,
                includes_bun_build_verification = surface_preference.includes_bun_build_verification,
                includes_bun_execute_verification = surface_preference.includes_bun_execute_verification,
            );

            let capture_kind = plan.capture_kind_enum();
            let entry_intent = Some(plan.submission_intent().to_string());
            self.begin_tab_ai_harness_entry_from_source_view(
                source_view,
                entry_intent,
                false,
                Some(plan),
                capture_kind,
                false,
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
                cx,
            );
            return;
        }

        let initial_input = crate::ai::harness::build_tab_ai_agent_chat_initial_input_for_prompt(
            prompt_type,
            plan.submission_intent(),
        );

        let submission_text = if initial_input.text.is_empty() {
            plan.submission_intent().to_string()
        } else {
            initial_input.text
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_quick_submit_agent_chat_live",
            prompt_type = prompt_type,
            source = ?plan.source,
            kind = ?plan.kind,
            input_len = plan.raw_query.len(),
            submission_len = submission_text.len(),
            guidance_appended = initial_input.guidance_appended,
            forced_by_script_list_submit = initial_input.forced_by_script_list_submit,
            includes_script_authoring_skill = initial_input.includes_script_authoring_skill,
            includes_bun_build_verification = initial_input.includes_bun_build_verification,
            includes_bun_execute_verification = initial_input.includes_bun_execute_verification,
        );

        entity.update(cx, |chat, cx| {
            chat.live_thread().update(cx, |thread, cx| {
                thread.set_input(submission_text, cx);
                if let Err(error) = thread.submit_input(cx) {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_quick_submit_agent_chat_live_submit_failed",
                        prompt_type = prompt_type,
                        error = %error,
                    );
                }
            });
        });
    }

    /// Submit a structured turn into an already-open, live harness session.
    ///
    /// Captures fresh desktop context, rebuilds the full flat-line context
    /// submission with quick-submit metadata, and injects via Submit mode.
    /// On build failure, shows an error toast instead of falling back to
    /// raw intent-only PTY injection.
    fn submit_live_tab_ai_harness_from_plan(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        let capture_kind = plan.capture_kind_enum();
        let source_view = self
            .tab_ai_harness_return_view
            .clone()
            .unwrap_or(AppView::ScriptList);

        let (ui_snapshot, invocation_receipt) = self.snapshot_tab_ai_ui(cx);
        self.tab_ai_harness_capture_generation += 1;

        let entry_intent = plan.submission_intent().to_string();
        let request = TabAiLaunchRequest {
            source_view,
            entry_intent: Some(entry_intent),
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            suppress_focused_part: false,
            quick_submit_plan: Some(plan),
            ui_snapshot,
            invocation_receipt,
            capture_kind,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        let wait_for_readiness = Self::tab_ai_harness_needs_readiness_wait(&entity, cx);
        let capture_rx = self.spawn_tab_ai_pre_switch_capture(&request);
        let app_weak = cx.entity().downgrade();
        let capture_gen = request.capture_generation;

        cx.spawn(async move |_this, cx| {
            let capture_result = match capture_rx.recv().await {
                Ok(result) => result,
                Err(_) => Err("deferred capture channel closed".to_string()),
            };

            let artifacts = match capture_result {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    tracing::warn!(
                        event = "tab_ai_live_quick_submit_capture_failed",
                        error = %error,
                    );
                    TabAiDeferredCaptureArtifacts::default()
                }
            };

            let _ = cx.update(|cx| {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    if this.tab_ai_harness_capture_generation != capture_gen {
                        tracing::debug!(
                            event = "tab_ai_live_quick_submit_stale",
                            expected = capture_gen,
                            current = this.tab_ai_harness_capture_generation,
                        );
                        return;
                    }

                    let resolved = this.build_tab_ai_context_from(
                        request.entry_intent.clone().unwrap_or_default(),
                        request.source_view.clone(),
                        request.ui_snapshot.clone(),
                        artifacts.desktop,
                        request.invocation_receipt.clone(),
                        cx,
                    );

                    let source_type = detect_tab_ai_source_type(
                        &request.source_view,
                        &resolved.context.desktop,
                        resolved.context.focused_target.as_ref(),
                    );
                    let apply_back_hint = build_tab_ai_apply_back_hint(source_type.as_ref());

                    this.tab_ai_harness_apply_back_route = source_type
                        .clone()
                        .zip(apply_back_hint.clone())
                        .map(|(source_type, hint)| crate::ai::TabAiApplyBackRoute {
                            source_type,
                            hint,
                            focused_target: resolved.context.focused_target.clone(),
                        });

                    let context = resolved.context.with_deferred_capture_fields(
                        source_type,
                        artifacts.screenshot_path,
                        apply_back_hint,
                    );

                    match crate::ai::build_tab_ai_harness_submission(
                        &context,
                        request.entry_intent.as_deref(),
                        crate::ai::TabAiHarnessSubmissionMode::Submit,
                        request.quick_submit_plan.as_ref(),
                        Some(&resolved.invocation_receipt),
                        &resolved.suggested_intents,
                    ) {
                        Ok(submission) => {
                            this.inject_tab_ai_harness_submission(
                                entity.clone(),
                                submission,
                                wait_for_readiness,
                                true,
                                cx,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "tab_ai_live_quick_submit_build_failed",
                                error = %error,
                            );
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to build quick-submit context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }

    /// Map a target kind to its human-readable chip prefix.
    pub(crate) fn tab_ai_chip_prefix_for_kind(kind: &str) -> &'static str {
        match kind {
            "file" => "File",
            "directory" => "Folder",
            "search_query" => "Search",
            "input" => "Input",
            "clipboard_entry" => "Clipboard",
            "script" | "scriptlet" | "builtin" => "Command",
            "window" => "Window",
            "app" => "App",
            "process" => "Process",
            "menu_command" => "Menu Command",
            "action" => "Action",
            "note" => "Note",
            "agent" => "Agent",
            "fallback" => "Suggestion",
            _ => "Selection",
        }
    }

    /// Format a canonical chip label from a resolved target.
    ///
    /// Delegates to the shared `format_explicit_target_chip_label` so Notes,
    /// actions, and the main-window Agent Chat openings all produce the same text.
    fn format_tab_ai_focused_chip_label(target: &crate::ai::TabAiTargetContext) -> String {
        crate::ai::format_explicit_target_chip_label(target)
    }

    /// Resolve targets for a view and emit a structured audit log.
    fn resolve_tab_ai_targets_with_audit_for_view(
        &self,
        view: &AppView,
        ui: &crate::ai::TabAiUiSnapshot,
        phase: &str,
    ) -> (
        Option<crate::ai::TabAiTargetContext>,
        Vec<crate::ai::TabAiTargetContext>,
    ) {
        let (focused_target, visible_targets) =
            self.resolve_tab_ai_surface_targets_for_view(view, ui);
        crate::ai::TabAiTargetAudit::from_targets(
            &ui.prompt_type,
            &focused_target,
            &visible_targets,
        )
        .emit_with_phase(phase);
        (focused_target, visible_targets)
    }

    /// Returns `true` when the current view should treat global `Cmd+Enter`
    /// as launcher-style "send this context to AI".
    fn supports_global_cmd_enter_ai_entry(view: &AppView) -> bool {
        matches!(
            view,
            AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::DesignGalleryView { .. }
                | AppView::FooterGalleryView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::BrowseKitsView { .. }
                | AppView::MigrateV1View { .. }
                | AppView::InstalledKitsView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::CreateAiPresetView { .. }
                | AppView::SettingsView { .. }
                | AppView::FavoritesBrowseView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::FileSearchView { .. }
        )
    }

    /// Route a launcher-style `Cmd+Enter` press from the current non-AI source
    /// surface into Agent Chat.
    ///
    /// Returns `true` when the keypress was consumed and Agent Chat launch began.
    /// Run 12 Pass 11 — Cmd+Enter while composing power syntax generates a
    /// deterministic stub proposal, sets `pending_menu_syntax_ai_proposal`,
    /// and triggers a snapshot rebuild. Returns `true` when the chord was
    /// consumed so the legacy Agent Chat route doesn't also fire.
    pub(crate) fn try_route_cmd_enter_to_menu_syntax_ai(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::menu_syntax::{builtin_schema, MenuSyntaxActionState};
        let raw = self.filter_text().to_string();
        let mode = &self.menu_syntax_mode;
        let pending = if let Some(invocation) = mode.capture_for(&raw) {
            let target = invocation.target.clone();
            let schema = builtin_schema(&target);
            let state = MenuSyntaxActionState::CaptureComposer {
                target: &target,
                payload: invocation,
                schema: schema.as_ref(),
            };
            Some(crate::menu_syntax_ai::PendingMenuSyntaxAiProposal::new(
                raw.clone(),
                Some(target.clone()),
                crate::menu_syntax_ai::stub_proposal_for(&state),
            ))
        } else if let Some(argv) = mode.command_for(&raw) {
            let origin_target = Some(format!("!{}", argv.head));
            let state = MenuSyntaxActionState::CommandComposer {
                head: &argv.head,
                argv: &argv.argv,
            };
            Some(crate::menu_syntax_ai::PendingMenuSyntaxAiProposal::new(
                raw.clone(),
                origin_target,
                crate::menu_syntax_ai::stub_proposal_for(&state),
            ))
        } else if let Some(query) = mode.advanced_query_for(&raw) {
            let state = MenuSyntaxActionState::RefineQuery { query };
            Some(crate::menu_syntax_ai::PendingMenuSyntaxAiProposal::new(
                raw.clone(),
                Some("?".to_string()),
                crate::menu_syntax_ai::stub_proposal_for(&state),
            ))
        } else {
            None
        };
        let Some(pending) = pending else {
            return false;
        };
        tracing::info!(
            target: "script_kit::menu_syntax_ai",
            event = "menu_syntax_ai_proposal_generated",
            title = %pending.proposal.title,
            actionable = pending.proposal.is_actionable(),
            origin_target = ?pending.origin.target,
        );
        self.pending_menu_syntax_ai_proposal = Some(pending);
        cx.notify();
        true
    }

    /// Run 12 Pass 13 — `ai-proposal-accept-dismiss` UI handler. When the
    /// inline AI proposal hint card is up (i.e.
    /// `pending_menu_syntax_ai_proposal.is_some()`) and the user keys
    /// Tab/Enter (Accept) or Esc (Dismiss), thread the keypress through
    /// the pure `apply_proposal` decision layer (Run 11 Pass 35) and
    /// dispatch the resulting `ProposalEffect`:
    ///
    /// - `SetFilterText { new_text }` → write through `set_filter_text_immediate`
    ///   (which also clears `pending_menu_syntax_ai_proposal` as a side
    ///   effect, keeping the surface in sync).
    /// - `Dismiss` → clear `pending_menu_syntax_ai_proposal` and
    ///   `cx.notify()` to drop the card from the next render.
    ///
    /// Returns `true` when the keypress was consumed (caller should
    /// `stop_propagation` so the legacy Tab/Enter/Esc handlers don't also
    /// fire).
    pub(crate) fn try_apply_pending_menu_syntax_ai_proposal(
        &mut self,
        action: crate::menu_syntax_ai_apply::ProposalApplyAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(pending) = self.pending_menu_syntax_ai_proposal.clone() else {
            return false;
        };
        let current_input = self.filter_text().to_string();
        let origin_matches = pending.is_current_for(&current_input);
        let effect =
            crate::menu_syntax_ai_apply::apply_pending_proposal(&current_input, &pending, action);
        tracing::info!(
            target: "script_kit::menu_syntax_ai",
            event = "menu_syntax_ai_proposal_resolved",
            action = ?action,
            origin_matches,
            origin_target = ?pending.origin.target,
            effect_kind = match &effect {
                crate::menu_syntax_ai_apply::ProposalEffect::SetFilterText { .. } => "set_filter_text",
                crate::menu_syntax_ai_apply::ProposalEffect::Dismiss => "dismiss",
            },
        );
        match effect {
            crate::menu_syntax_ai_apply::ProposalEffect::SetFilterText { new_text } => {
                self.set_filter_text_immediate(new_text, window, cx);
            }
            crate::menu_syntax_ai_apply::ProposalEffect::Dismiss => {
                self.pending_menu_syntax_ai_proposal = None;
                cx.notify();
            }
        }
        true
    }

    pub(crate) fn try_route_global_cmd_enter_to_agent_chat_context_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let explicit_spine_prompt_plan =
            self.script_list_cmd_enter_has_explicit_spine_prompt_plan();
        if explicit_spine_prompt_plan {
            if self.try_submit_spine_prompt_plan_from_enter(cx) {
                return true;
            }
        }
        // Run 12 Pass 11 — `cmd-enter-inline-ai-proposal`. When the user is
        // composing power syntax, Cmd+Enter generates an INLINE proposal
        // shown in the hint card (NOT a full Agent Chat chat handoff). Intercept
        // BEFORE the legacy Agent Chat-route guards.
        if self.try_route_cmd_enter_to_menu_syntax_ai(cx) {
            return true;
        }
        if self.show_actions_popup || self.tab_ai_save_offer_state.is_some() {
            return false;
        }
        if !Self::supports_global_cmd_enter_ai_entry(&self.current_view) {
            return false;
        }
        // Same attachment-portal guard as the plain Tab path: the portal
        // temporarily hosts a builtin view, but Cmd+Enter there must not
        // reopen / refocus the Agent Chat chat behind the portal.
        if self.is_in_attachment_portal() {
            tracing::debug!(
                target: "script_kit::tab_ai",
                event = "tab_ai_global_cmd_enter_suppressed_in_attachment_portal",
                source_view = %self.app_view_name(),
            );
            return false;
        }

        if let Some(intent) =
            self.script_list_cmd_enter_plain_prompt_intent(explicit_spine_prompt_plan)
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_global_cmd_enter_plain_prompt_routed_to_agent_chat",
                input_len = intent.len(),
            );
            self.open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(
                Some(intent),
                cx,
            );
            return true;
        }

        let source_view = self.app_view_name();
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_global_cmd_enter_routed_to_agent_chat",
            source_view = %source_view,
        );

        let is_file_search = matches!(self.current_view, AppView::FileSearchView { .. });
        let origin = if is_file_search {
            agent_chat_entry::AgentChatEntryOrigin::FileSearch
        } else {
            agent_chat_entry::AgentChatEntryOrigin::MainLauncher
        };

        // Empty-input guard: when the user opens Agent Chat from the main menu
        // with an empty filter and the selection is merely the default
        // auto-selected top row (i.e. they typed nothing and never arrowed to a
        // specific command), Cmd+Return should land in an EMPTY composer rather
        // than staging that auto-selected row as an `@cmd:` context chip. Staging
        // is still preserved when the filter is non-empty OR the user
        // deliberately moved the selection off the default first-selectable row.
        let suppress_default_script_list_selection =
            matches!(self.current_view, AppView::ScriptList)
                && self.filter_text.trim().is_empty()
                && self
                    .main_menu_result_caches
                    .first_selectable_index()
                    .map_or(self.selected_index == 0, |first| {
                        self.selected_index == first
                    });
        if suppress_default_script_list_selection {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_script_list_empty_default_selection_not_staged",
                selected_index = self.selected_index,
                first_selectable_index = ?self.main_menu_result_caches.first_selectable_index(),
            );
        }
        let suppress_focused_part = suppress_default_script_list_selection;

        self.open_agent_chat_from_entry_request(
            agent_chat_entry::AgentChatEntryRequest {
                origin,
                target: agent_chat_entry::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
                seed_text: None,
                ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
                seed_policy: agent_chat_entry::AgentChatSeedPolicy::ComposerOnly,
                suppress_focused_part,
                context_staging: if is_file_search {
                    agent_chat_entry::AgentChatContextStaging::FileSearchSelection
                } else if suppress_focused_part {
                    agent_chat_entry::AgentChatContextStaging::SuppressFocused
                } else {
                    agent_chat_entry::AgentChatContextStaging::AmbientOrFocused
                },
                return_origin: Some(self.current_view.clone()),
            },
            cx,
        );
        true
    }

    fn script_list_cmd_enter_has_explicit_spine_prompt_plan(&mut self) -> bool {
        if !self.spine_enabled || !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }

        let raw = self.filter_text.clone();
        if raw.trim().is_empty() {
            return false;
        }

        self.set_spine_parse_from_filter_and_cursor(&raw, raw.len());
        crate::spine::parse_has_prompt_builder_segments(&self.spine_parse)
    }

    fn script_list_cmd_enter_plain_prompt_intent(
        &self,
        explicit_spine_prompt_plan: bool,
    ) -> Option<String> {
        if explicit_spine_prompt_plan || !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }

        let intent = self.filter_text.trim();
        if intent.is_empty() {
            return None;
        }

        Some(intent.to_string())
    }

    /// Build a focused-target `AiContextPart` from the current view's
    /// resolved focus, if any. Returns `None` when the active surface has
    /// no resolvable focused item (e.g. empty list, generic prompt).
    fn build_tab_ai_focused_part_for_view(
        &self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let (focused_target, _visible_targets) =
            self.resolve_tab_ai_targets_with_audit_for_view(source_view, ui_snapshot, "pre_open");
        focused_target.map(|target| {
            let label = Self::format_tab_ai_focused_chip_label(&target);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focused_chip_label_built",
                item_source = %target.source,
                item_kind = %target.kind,
                label_chars = label.chars().count(),
            );
            crate::ai::message_parts::AiContextPart::FocusedTarget { target, label }
        })
    }

    /// Returns `true` when the Tab press should route to the Ask Anything
    /// fallback (ambient desktop context) instead of the focused-target chip
    /// path. This happens when no focused item is resolvable from the active
    /// surface.
    fn should_use_tab_ai_ask_anything_fallback(
        &self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
    ) -> bool {
        self.build_tab_ai_focused_part_for_view(source_view, ui_snapshot)
            .is_none()
    }

    /// Deferred-capture entry point: build a launch request from pre-switch
    /// state, start background capture, then immediately open the harness.
    ///
    /// The harness terminal appears within one frame of the Tab keypress.
    /// Context capture (desktop snapshot, screenshot-to-file) runs in the
    /// background and is injected into the live PTY once complete.
    fn begin_tab_ai_harness_entry(
        &mut self,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
        capture_kind: crate::ai::TabAiCaptureKind,
        force_agent_chat_surface: bool,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) {
        self.begin_tab_ai_harness_entry_from_source_view(
            self.current_view.clone(),
            entry_intent,
            suppress_focused_part,
            quick_submit_plan,
            capture_kind,
            force_agent_chat_surface,
            ui_variant,
            cx,
        );
    }

    /// Deferred-capture entry point with an explicit source view.
    ///
    /// This is the inner implementation that `begin_tab_ai_harness_entry`
    /// delegates to. The `source_view` parameter allows callers like the
    /// Agent Chat live-submit reroute to preserve the original source surface
    /// instead of using `self.current_view` (which may already be Agent Chat).
    fn begin_tab_ai_harness_entry_from_source_view(
        &mut self,
        source_view: AppView,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
        capture_kind: crate::ai::TabAiCaptureKind,
        force_agent_chat_surface: bool,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) {
        let snapshot_started_at = std::time::Instant::now();
        let (ui_snapshot, invocation_receipt) = if matches!(source_view, AppView::ScriptList) {
            self.snapshot_tab_ai_ui_fast_for_script_list()
        } else {
            self.snapshot_tab_ai_ui(cx)
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_open_stage",
            stage = "snapshot_tab_ai_ui",
            stage_ms = snapshot_started_at.elapsed().as_millis() as u64,
            source_view = match &source_view {
                AppView::ScriptList => "ScriptList",
                _ => "Other",
            },
        );
        let entry_intent = entry_intent
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Emit the receipt as a standalone structured log line for agent/test consumption
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_invocation_receipt",
            prompt_type = %invocation_receipt.prompt_type,
            input_status = %invocation_receipt.input_status,
            focus_status = %invocation_receipt.focus_status,
            elements_status = %invocation_receipt.elements_status,
            has_input_text = invocation_receipt.has_input_text,
            has_focus_target = invocation_receipt.has_focus_target,
            element_count = invocation_receipt.element_count,
            warning_count = invocation_receipt.warning_count,
            rich = invocation_receipt.rich,
            degradation_reasons = ?invocation_receipt.degradation_reasons,
            receipt_json = %serde_json::to_string(&invocation_receipt).unwrap_or_default(),
        );

        self.tab_ai_harness_capture_generation += 1;

        let request = TabAiLaunchRequest {
            source_view,
            entry_intent,
            ui_variant,
            suppress_focused_part,
            quick_submit_plan,
            ui_snapshot,
            invocation_receipt,
            capture_kind,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        // Compute surface preference from the shared verification markers
        // *before* branching so the decision is logged and deterministic.
        let effective_intent = Self::tab_ai_effective_submission_intent(&request);
        let submit_now = request
            .quick_submit_plan
            .as_ref()
            .map(|plan| plan.submit)
            .unwrap_or(request.entry_intent.is_some());
        let submission_mode = if submit_now {
            crate::ai::TabAiHarnessSubmissionMode::Submit
        } else {
            crate::ai::TabAiHarnessSubmissionMode::PasteOnly
        };
        let surface_preference = crate::ai::harness::tab_ai_surface_preference_for_prompt(
            &request.ui_snapshot.prompt_type,
            effective_intent.as_deref(),
            submission_mode,
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_surface_selected",
            surface = if surface_preference.use_quick_terminal { "quick_terminal" } else { "agent_chat" },
            reason = if surface_preference.use_quick_terminal { "script_verification_required" } else { "default_agent_chat" },
            prompt_type = %request.ui_snapshot.prompt_type,
            has_entry_intent = request.entry_intent.is_some(),
            submit_now,
            includes_script_authoring_skill = surface_preference.includes_script_authoring_skill,
            includes_bun_build_verification = surface_preference.includes_bun_build_verification,
            includes_bun_execute_verification = surface_preference.includes_bun_execute_verification,
        );

        // Resolve whether we have a focused target or need the Ask Anything
        // fallback *before* spawning background capture.
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;
        let should_stage_focused_part = Self::should_stage_focused_part_for_request(
            &request.source_view,
            pending_script_list_trigger,
            request.suppress_focused_part,
        );
        // let use_ask_anything_fallback = self.should_use_tab_ai_ask_anything_fallback(
        let use_ask_anything_fallback = self
            .should_use_tab_ai_ask_anything_fallback(&request.source_view, &request.ui_snapshot);
        let use_ask_anything_fallback = should_stage_focused_part && use_ask_anything_fallback;

        // Explicit AI commands (screen, focused window, selected text, browser tab)
        // must force ambient capture even when the source surface has a focused item.
        let explicit_ambient_chip_label =
            Self::tab_ai_explicit_ambient_chip_label(&request.capture_kind).map(str::to_string);

        if !should_stage_focused_part {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focus_chip_suppressed_for_script_list_trigger",
                trigger = ?pending_script_list_trigger,
            );
        }

        let focused_part = if use_ask_anything_fallback || explicit_ambient_chip_label.is_some() {
            None
        } else if !should_stage_focused_part {
            None
        } else {
            self.build_tab_ai_focused_part_for_view(&request.source_view, &request.ui_snapshot)
        };

        // Only run expensive ambient capture for the Ask Anything fallback path
        // or explicit ambient capture commands.
        // Focused-target Tab flows skip desktop snapshots and screenshots.
        let capture_rx = if use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_ask_anything_fallback",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
            self.spawn_tab_ai_pre_switch_capture(&request)
        } else if let Some(ref label) = explicit_ambient_chip_label {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_explicit_ambient_capture",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
                chip_label = %label,
            );
            self.spawn_tab_ai_pre_switch_capture(&request)
        } else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focus_chip_staged",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
            // No ambient capture needed — return a dummy closed channel.
            let (_tx, rx) =
                async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);
            rx
        };

        if surface_preference.use_quick_terminal && !force_agent_chat_surface {
            self.open_tab_ai_harness_terminal_from_request(request, capture_rx, cx);
        } else {
            self.open_tab_ai_agent_chat_view_from_request_impl(
                request,
                capture_rx,
                focused_part,
                use_ask_anything_fallback,
                explicit_ambient_chip_label,
                force_agent_chat_surface,
                cx,
            );
        }
    }

    /// Start background capture immediately on a dedicated OS thread.
    ///
    /// Captures the desktop context snapshot and (best-effort) focused window
    /// screenshot. Returns a channel receiver that delivers the results.
    ///
    /// Uses `std::thread::spawn` instead of `cx.spawn` + background executor
    /// so the expensive AX/screenshot work begins *immediately* — before the
    /// view switch can steal focus from the frontmost app.
    fn spawn_tab_ai_pre_switch_capture(
        &self,
        request: &TabAiLaunchRequest,
    ) -> TabAiDeferredCaptureRx {
        let capture_kind = request.capture_kind;
        let (tx, rx) = async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);

        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                // Capture desktop context (text-safe, no screenshots in the blob)
                let desktop = crate::context_snapshot::capture_context_snapshot(
                    &crate::context_snapshot::CaptureContextOptions::tab_ai_submit(),
                );

                // Best-effort screenshot-to-file capture, branching on the
                // requested capture kind so explicit AI commands get the right
                // screenshot type instead of always defaulting to focused-window.
                let screenshot_path = match capture_kind {
                    crate::ai::TabAiCaptureKind::DefaultContext
                    | crate::ai::TabAiCaptureKind::FocusedWindow => {
                        match crate::ai::harness::capture_tab_ai_focused_window_screenshot_file() {
                            Ok(Some(file)) => Some(file.path),
                            Ok(None) => None,
                            Err(error) => {
                                tracing::debug!(
                                    event = "tab_ai_deferred_screenshot_failed",
                                    capture_kind = "focused_window",
                                    error = %error,
                                );
                                None
                            }
                        }
                    }
                    crate::ai::TabAiCaptureKind::FullScreen => {
                        match crate::ai::harness::capture_tab_ai_screen_screenshot_file() {
                            Ok(Some(file)) => Some(file.path),
                            Ok(None) => None,
                            Err(error) => {
                                tracing::debug!(
                                    event = "tab_ai_deferred_screenshot_failed",
                                    capture_kind = "full_screen",
                                    error = %error,
                                );
                                None
                            }
                        }
                    }
                    // Text-only captures (selected text, browser tab) skip screenshots.
                    crate::ai::TabAiCaptureKind::SelectedText
                    | crate::ai::TabAiCaptureKind::BrowserTab => None,
                };

                TabAiDeferredCaptureArtifacts {
                    desktop,
                    screenshot_path,
                }
            })
            .map_err(|_| "tab_ai_deferred_capture_panicked".to_string());

            let send_result = match result {
                Ok(artifacts) => tx.send_blocking(Ok(artifacts)),
                Err(error) => tx.send_blocking(Err(error)),
            };
            if send_result.is_err() {
                tracing::debug!(event = "tab_ai_deferred_capture_receiver_dropped");
            }
        });

        rx
    }

    /// Open the Agent Chat chat view immediately, then spawn a task that waits
    /// Compute the canonical submission intent for Agent Chat launches, matching the
    /// PTY path's normalization: prefer `quick_submit_plan.submission_intent()`
    /// over raw `entry_intent`, trim whitespace, and drop empty strings.
    fn tab_ai_effective_submission_intent(request: &TabAiLaunchRequest) -> Option<String> {
        request
            .quick_submit_plan
            .as_ref()
            .map(|plan| plan.submission_intent())
            .or(request.entry_intent.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// Return a chip label for entry modes that must always use ambient capture,
    /// even if the source view has a resolvable focused target.
    fn tab_ai_explicit_ambient_chip_label(
        capture_kind: &crate::ai::TabAiCaptureKind,
    ) -> Option<&'static str> {
        match capture_kind {
            crate::ai::TabAiCaptureKind::DefaultContext => None,
            crate::ai::TabAiCaptureKind::FullScreen => Some("Full Screen"),
            crate::ai::TabAiCaptureKind::FocusedWindow => Some("Focused Window"),
            crate::ai::TabAiCaptureKind::SelectedText => Some("Selected Text"),
            crate::ai::TabAiCaptureKind::BrowserTab => Some("Browser Tab"),
        }
    }

    fn should_stage_focused_part_for_request(
        source_view: &AppView,
        pending_script_list_trigger: Option<char>,
        suppress_focused_part: bool,
    ) -> bool {
        if suppress_focused_part {
            return false;
        }

        if !matches!(source_view, AppView::ScriptList) {
            return true;
        }

        !matches!(pending_script_list_trigger, Some('/' | '@' | '|'))
    }

    /// Extract a `TabAiTargetContext` from an `AiContextPart::FocusedTarget`,
    /// returning `None` for any other variant or `None` input.
    fn tab_ai_focused_target_from_part(
        part: Option<&crate::ai::message_parts::AiContextPart>,
    ) -> Option<crate::ai::TabAiTargetContext> {
        match part {
            Some(crate::ai::message_parts::AiContextPart::FocusedTarget { target, .. }) => {
                Some(target.clone())
            }
            _ => None,
        }
    }

    /// Seed `tab_ai_harness_apply_back_route` synchronously from the source
    /// view, UI snapshot, and an optional focused part.  When `focused_part`
    /// is `AiContextPart::FocusedTarget`, the concrete target metadata is
    /// preserved in the route so downstream apply-back consumers can identify
    /// what the user's chip refers to.
    fn seed_tab_ai_apply_back_route(
        &mut self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
        focused_part: Option<&crate::ai::message_parts::AiContextPart>,
    ) {
        let early_source_type = detect_tab_ai_source_type_early(source_view, ui_snapshot);
        let focused_target = Self::tab_ai_focused_target_from_part(focused_part);

        self.tab_ai_harness_apply_back_route = early_source_type.clone().and_then(|source_type| {
            build_tab_ai_apply_back_hint(Some(&source_type)).map(|hint| {
                crate::ai::TabAiApplyBackRoute {
                    source_type,
                    hint,
                    focused_target: focused_target.clone(),
                }
            })
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_apply_back_route_seeded",
            source_view = match source_view {
                AppView::ScriptList => "ScriptList",
                _ => "Other",
            },
            has_early_source_type = early_source_type.is_some(),
            has_focused_target = focused_target.is_some(),
            focused_target_kind = ?focused_target.as_ref().map(|t| t.kind.as_str()),
            focused_target_source = ?focused_target.as_ref().map(|t| t.source.as_str()),
        );
    }

    /// Open the Agent Chat chat view and stage context.
    ///
    pub(crate) fn open_agent_chat_history_main_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_builtin_filterable_view(
            AppView::AgentChatHistoryView {
                filter: String::new(),
                selected_index: 0,
            },
            "Search conversation history...",
            true,
            cx,
        );
    }

    /// Open the harness terminal immediately, then spawn a task that waits
    /// for the deferred capture result and injects the enriched context.
    ///
    /// **Contract:** `AppView::QuickTerminalView` and `cx.notify()` happen
    /// *before* any deferred-capture await. The user sees the terminal cursor
    /// within one frame.
    fn open_tab_ai_harness_terminal_from_request(
        &mut self,
        request: TabAiLaunchRequest,
        capture_rx: TabAiDeferredCaptureRx,
        cx: &mut Context<Self>,
    ) {
        // Reuse a silently prewarmed session exactly once; otherwise force fresh.
        let reuse_fresh_prewarm = self
            .tab_ai_harness
            .as_ref()
            .map(|session| session.is_fresh_prewarm() && session.entity.read(cx).is_alive())
            .unwrap_or(false);

        if reuse_fresh_prewarm {
            if let Some(session) = self.tab_ai_harness.as_mut() {
                session.mark_consumed();
            }
        }

        let (entity, _was_cold_start) = match self
            .ensure_tab_ai_harness_terminal(!reuse_fresh_prewarm, cx)
        {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(
                    event = "tab_ai_harness_start_failed",
                    error = %error,
                );
                self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            format!("Failed to start harness: {error}. Install the configured CLI or update claudeCode.path in ~/.scriptkit/config.ts"),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                cx.notify();
                return;
            }
        };

        // Determine readiness based on actual PTY output, not cold-start flag.
        let wait_for_readiness = Self::tab_ai_harness_needs_readiness_wait(&entity, cx);

        tracing::debug!(
            event = "tab_ai_harness_submission_planned",
            wait_for_readiness,
            has_entry_intent = request.entry_intent.is_some(),
        );

        // Save the originating surface so Escape and re-entry can use it
        self.tab_ai_harness_return_view = Some(request.source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());

        // Seed apply-back route synchronously so the footer shows the correct
        // Cmd+Enter label on first render and the first ⌘↩ press works without
        // waiting for the deferred capture.  PTY path has no focused part.
        self.seed_tab_ai_apply_back_route(&request.source_view, &request.ui_snapshot, None);

        // --- View switch FIRST: user sees the terminal immediately ---
        self.current_view = AppView::QuickTerminalView {
            entity: entity.clone(),
        };
        self.focused_input = FocusedInput::None;
        self.clear_actions_popup_state();
        self.pending_focus = Some(FocusTarget::TermPrompt);

        // Deferred resize to avoid RefCell borrow error
        cx.spawn(async move |_this, _cx| {
            resize_to_view_sync(ViewType::TermPrompt, 0);
        })
        .detach();
        cx.notify();

        // --- Spawn deferred context injection task ---
        // This waits for the background capture, builds the full context blob
        // with source type / screenshot / apply-back hint, then injects.
        let app_weak = cx.entity().downgrade();
        let capture_gen = request.capture_generation;

        cx.spawn(async move |_this, cx| {
            // Wait for deferred capture to complete
            let capture_result = match capture_rx.recv().await {
                Ok(result) => result,
                Err(_) => Err("deferred capture channel closed".to_string()),
            };

            let artifacts = match capture_result {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(
                        event = "tab_ai_deferred_capture_failed",
                        error = %e,
                    );
                    TabAiDeferredCaptureArtifacts::default()
                }
            };

            // Apply the captured context
            let _ = cx.update(|cx| {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    // Stale generation check
                    if this.tab_ai_harness_capture_generation != capture_gen {
                        tracing::debug!(
                            event = "tab_ai_deferred_capture_stale",
                            expected = capture_gen,
                            current = this.tab_ai_harness_capture_generation,
                        );
                        return;
                    }

                    let resolved = this.build_tab_ai_context_from(
                        request.entry_intent.clone().unwrap_or_default(),
                        request.source_view.clone(),
                        request.ui_snapshot.clone(),
                        artifacts.desktop,
                        request.invocation_receipt.clone(),
                        cx,
                    );

                    let source_type = detect_tab_ai_source_type(
                        &request.source_view,
                        &resolved.context.desktop,
                        resolved.context.focused_target.as_ref(),
                    );
                    let apply_back_hint = build_tab_ai_apply_back_hint(source_type.as_ref());

                    // Persist the apply-back route so ⌘⏎ can execute it later.
                    // Carry the focused target metadata so apply-back can route
                    // results without rediscovering UI state after the harness closes.
                    this.tab_ai_harness_apply_back_route = source_type
                        .clone()
                        .zip(apply_back_hint.clone())
                        .map(|(source_type, hint)| crate::ai::TabAiApplyBackRoute {
                            source_type,
                            hint,
                            focused_target: resolved.context.focused_target.clone(),
                        });

                    let context = resolved.context.with_deferred_capture_fields(
                        source_type,
                        artifacts.screenshot_path,
                        apply_back_hint,
                    );

                    let effective_intent = request
                        .quick_submit_plan
                        .as_ref()
                        .map(|plan| plan.submission_intent())
                        .or(request.entry_intent.as_deref());

                    let submit_now = request
                        .quick_submit_plan
                        .as_ref()
                        .map(|plan| plan.submit)
                        .unwrap_or(request.entry_intent.is_some());

                    let submission_mode = if submit_now {
                        crate::ai::TabAiHarnessSubmissionMode::Submit
                    } else {
                        crate::ai::TabAiHarnessSubmissionMode::PasteOnly
                    };

                    match crate::ai::build_tab_ai_harness_submission(
                        &context,
                        effective_intent,
                        submission_mode,
                        request.quick_submit_plan.as_ref(),
                        Some(&resolved.invocation_receipt),
                        &resolved.suggested_intents,
                    ) {
                        Ok(submission) => {
                            this.inject_tab_ai_harness_submission(
                                entity.clone(),
                                submission,
                                wait_for_readiness,
                                submit_now,
                                cx,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "tab_ai_harness_context_build_failed",
                                error = %error,
                            );
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to build harness context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }

    /// Prewarm the configured harness at app startup so the first Tab press
    /// reuses a live PTY instead of paying spawn cost.
    ///
    /// This must be silent: no view switch, no focus change, no toast.
    /// User-facing errors still belong to the explicit Tab path.
    ///
    /// When `respect_startup_opt_out` is `true`, the `warmOnStartup` config
    /// flag is honoured — set for startup prewarm only.  The post-close
    /// reseed path passes `false` so the next Tab always feels instant even
    /// when startup warming is disabled.
    fn warm_tab_ai_harness_silently(
        &mut self,
        respect_startup_opt_out: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(existing) = &self.tab_ai_harness {
            if existing.entity.read(cx).is_alive() {
                tracing::debug!(
                    event = "tab_ai_harness_prewarm_skipped",
                    reason = "already_alive",
                );
                return;
            }
        }

        let config = match crate::ai::read_tab_ai_harness_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::debug!(
                    event = "tab_ai_harness_prewarm_skipped",
                    reason = "config_read_failed",
                    error = %error,
                );
                return;
            }
        };

        if respect_startup_opt_out && !config.warm_on_startup {
            tracing::debug!(
                event = "tab_ai_harness_prewarm_skipped",
                reason = "disabled",
            );
            return;
        }

        if let Err(error) = crate::ai::validate_tab_ai_harness_config(&config) {
            tracing::debug!(
                event = "tab_ai_harness_prewarm_skipped",
                reason = "invalid_config",
                error = %error,
            );
            return;
        }

        match self.ensure_tab_ai_harness_terminal(false, cx) {
            Ok((_entity, was_cold_start)) => {
                // Tag newly created sessions as FreshPrewarm so the next Tab
                // reuses them exactly once instead of immediately killing them.
                if was_cold_start {
                    if let Some(session) = self.tab_ai_harness.as_mut() {
                        session.mark_fresh_prewarm();
                    }
                }
                tracing::info!(
                    event = "tab_ai_harness_prewarmed",
                    backend = ?config.backend,
                    command = %config.command,
                    was_cold_start,
                    source = if respect_startup_opt_out { "startup" } else { "post_close" },
                );
            }
            Err(error) => {
                tracing::warn!(
                    event = "tab_ai_harness_prewarm_failed",
                    error = %error,
                );
            }
        }
    }

    /// Startup prewarm: respects `warmOnStartup=false`.
    pub(crate) fn warm_agent_chat_on_startup(&mut self, _cx: &mut Context<Self>) {
        if std::env::var("SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_hot_prewarm_skipped",
                correlation_id = "agent_chat_hot_prewarm",
                reason = "disabled_by_env",
            );
            return;
        }

        let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        let ai_prefs = crate::config::load_user_preferences().ai;
        let cwd_override = self.spine_cwd_for_agent_chat_launch();
        match crate::ai::agent_chat::launch::resolve_selected_pi_launch_with_cwd_override(
            &ai_prefs,
            &profile_ctx,
            cwd_override.clone(),
        ) {
            Ok(pi_launch) => {
                match crate::ai::agent_chat::launch::warm_session_manager()
                    .prepare_warm_background(pi_launch.warm_spec())
                {
                    Ok(snapshot) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_started",
                            profile_id = %pi_launch.profile.id,
                            warm_key = %pi_launch.warm_key,
                            cwd = %pi_launch.cwd.display(),
                            cwd_override = ?cwd_override,
                            generation = snapshot.generation,
                            state = ?snapshot.state,
                        );
                    }
                    Err(error) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_unavailable",
                            profile_id = %pi_launch.profile.id,
                            warm_key = %pi_launch.warm_key,
                            cwd = %pi_launch.cwd.display(),
                            cwd_override = ?cwd_override,
                            error = %error,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "pi_agent_chat_warm_resolution_failed",
                    error = %error,
                );
            }
        }

        match crate::ai::agent_chat::launch::resolve_focused_text_pi_launch(&ai_prefs, &profile_ctx)
        {
            Ok(text_launch) => {
                match crate::ai::agent_chat::launch::warm_session_manager()
                    .prepare_warm_background(text_launch.warm_spec())
                {
                    Ok(snapshot) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_started",
                            profile_id = %text_launch.profile.id,
                            warm_key = %text_launch.warm_key,
                            generation = snapshot.generation,
                            state = ?snapshot.state,
                        );
                    }
                    Err(error) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_unavailable",
                            profile_id = %text_launch.profile.id,
                            warm_key = %text_launch.warm_key,
                            error = %error,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "pi_agent_chat_text_warm_resolution_skipped",
                    error = %error,
                );
            }
        }

        // Quick AI (launcher Tab-with-text) must feel instant: prewarm its
        // session alongside the selected and Text profiles. Resolves through
        // `resolve_quick_ai_pi_launch`, so a profile picked via Shift+Tab
        // ("Use for Quick AI") is the one that gets warmed.
        match crate::ai::agent_chat::launch::resolve_quick_ai_pi_launch(&ai_prefs, &profile_ctx) {
            Ok(quick_ai_launch) => {
                match crate::ai::agent_chat::launch::warm_session_manager()
                    .prepare_warm_background(quick_ai_launch.warm_spec())
                {
                    Ok(snapshot) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_started",
                            profile_id = %quick_ai_launch.profile.id,
                            warm_key = %quick_ai_launch.warm_key,
                            generation = snapshot.generation,
                            state = ?snapshot.state,
                        );
                    }
                    Err(error) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "pi_agent_chat_warm_unavailable",
                            profile_id = %quick_ai_launch.profile.id,
                            warm_key = %quick_ai_launch.warm_key,
                            error = %error,
                        );
                    }
                }
            }
            Err(error) => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "pi_agent_chat_quick_ai_warm_resolution_skipped",
                    error = %error,
                );
            }
        }
    }

    /// Startup prewarm: respects `warmOnStartup=false`.
    pub(crate) fn warm_tab_ai_harness_on_startup(&mut self, cx: &mut Context<Self>) {
        self.warm_tab_ai_harness_silently(true, cx);
    }

    /// Post-close prewarm: bypasses the startup opt-out so the next Tab
    /// always gets an instant fresh session after a close cycle.
    fn warm_tab_ai_harness_after_close(&mut self, cx: &mut Context<Self>) {
        self.warm_tab_ai_harness_silently(false, cx);
    }

    /// Ensure a harness terminal session exists and is alive.
    /// Returns the entity and whether this was a cold start (newly created).
    ///
    /// When `force_fresh` is `true`, any existing alive session is terminated
    /// first so the caller gets a brand-new PTY.  The startup and post-close
    /// prewarm paths pass `false` to seed a reusable session; the first
    /// explicit Tab press may reuse that `FreshPrewarm` once, and later
    /// explicit opens force a clean PTY again.
    fn ensure_tab_ai_harness_terminal(
        &mut self,
        force_fresh: bool,
        cx: &mut Context<Self>,
    ) -> Result<(gpui::Entity<crate::term_prompt::TermPrompt>, bool), String> {
        if force_fresh {
            // Kill the existing session so the user gets a clean slate.
            // Terminate FIRST, then clear the handle — if termination fails
            // the handle stays in app state so we don't lose track of a live PTY.
            if let Some(existing) = self.tab_ai_harness.as_ref() {
                existing.entity.update(cx, |term, _cx| {
                    term.terminate_session().map_err(|e| e.to_string())
                })?;
            }
            if self.tab_ai_harness.is_some() {
                self.tab_ai_harness = None;
                tracing::info!(event = "tab_ai_harness_old_session_terminated");
            }
        } else {
            // Reuse existing session if alive (prewarm path)
            if let Some(existing) = &self.tab_ai_harness {
                if existing.entity.read(cx).is_alive() {
                    return Ok((existing.entity.clone(), false));
                }
            }
        }

        let config = crate::ai::read_tab_ai_harness_config()?;
        crate::ai::validate_tab_ai_harness_config(&config)?;

        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(move |_id: String, _value: Option<String>| {});

        let term_height = crate::window_resize::layout::MAX_HEIGHT
            - gpui::px(crate::window_resize::layout::FOOTER_HEIGHT);

        let mut prompt = crate::term_prompt::TermPrompt::with_height(
            "tab-ai-harness".to_string(),
            Some(config.command_line()),
            self.focus_handle.clone(),
            submit_callback,
            std::sync::Arc::clone(&self.theme),
            std::sync::Arc::new(self.config.clone()),
            Some(term_height),
        )
        .map_err(|e| format!("tab_ai_harness_terminal_create_failed: {e}"))?;

        // Let the smart routing in scroll_to_pty() decide: when the TUI
        // enables mouse mode (Claude Code, etc.), scroll events are forwarded
        // as escape sequences so the TUI can handle scrolling internally.
        // When mouse mode is off, scroll falls back to local display buffer.
        prompt.prefer_buffer_scroll_on_wheel = false;

        let entity = cx.new(|_| prompt);

        tracing::info!(
            event = "tab_ai_harness_terminal_created",
            backend = ?config.backend,
            command = %config.command,
        );

        self.tab_ai_harness = Some(crate::ai::TabAiHarnessSessionState::new(
            config,
            entity.clone(),
            "tab-ai-harness",
        ));

        Ok((entity, true))
    }

    /// Maximum time (ms) to wait for a cold-started harness to produce its
    /// first output before injecting context anyway.
    const HARNESS_READINESS_TIMEOUT_MS: u64 = 2000;

    /// Interval (ms) between readiness polls during cold-start wait.
    const HARNESS_READINESS_POLL_MS: u64 = 20;

    /// Check whether a harness session needs to wait for prompt readiness
    /// before context paste.  Returns `true` when the PTY has not yet
    /// produced any output — regardless of whether it was cold-started or
    /// prewarmed.
    fn tab_ai_harness_needs_readiness_wait(
        entity: &gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &Context<Self>,
    ) -> bool {
        !entity.read(cx).has_received_output
    }

    /// Inject the context submission into the harness PTY, with a readiness
    /// gate that fires whenever the PTY has not yet produced output.
    ///
    /// Polls `has_received_output` on the `TermPrompt` entity up to
    /// [`HARNESS_READINESS_TIMEOUT_MS`] before injecting.  Falls back
    /// deterministically if the harness does not produce output in time.
    /// This applies to both cold-started and prewarmed sessions that have
    /// not yet rendered their first prompt.
    ///
    /// When `submit` is true, the payload is sent as a full line (appends CR).
    /// When false, the payload is pasted without a trailing CR so the user
    /// can type their intent before pressing Enter.
    fn inject_tab_ai_harness_submission(
        &self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        submission: String,
        wait_for_readiness: bool,
        submit: bool,
        cx: &mut Context<Self>,
    ) {
        let app = cx.entity().downgrade();
        let entity_weak = entity.downgrade();

        cx.spawn(async move |_this, cx| {
            // Wait until the harness has produced output (its prompt/banner),
            // with a bounded timeout as fallback.  This fires for both
            // cold-started and prewarmed sessions that are not yet ready.
            if wait_for_readiness {
                let poll_interval =
                    std::time::Duration::from_millis(Self::HARNESS_READINESS_POLL_MS);
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_millis(Self::HARNESS_READINESS_TIMEOUT_MS);

                loop {
                    let is_ready = cx.update(|cx| {
                        entity_weak
                            .upgrade()
                            .map(|e| e.read(cx).has_received_output)
                            .unwrap_or(true) // entity gone → skip waiting
                    });

                    if is_ready {
                        tracing::debug!(
                            event = "tab_ai_harness_readiness_detected",
                            elapsed_ms = %std::time::Instant::now()
                                .duration_since(deadline - std::time::Duration::from_millis(Self::HARNESS_READINESS_TIMEOUT_MS))
                                .as_millis(),
                        );
                        break;
                    }
                    if std::time::Instant::now() >= deadline {
                        tracing::warn!(
                            event = "tab_ai_harness_readiness_timeout",
                            timeout_ms = Self::HARNESS_READINESS_TIMEOUT_MS,
                        );
                        break;
                    }
                    cx.background_executor().timer(poll_interval).await;
                }
            }

            let _ = cx.update(|cx| {
                let Some(entity) = entity_weak.upgrade() else {
                    return;
                };
                let result = entity.update(cx, |term, _cx| {
                    if submit {
                        term.send_line(&submission).map_err(|e| e.to_string())
                    } else {
                        term.send_text_as_paste(&submission)
                            .map_err(|e| e.to_string())
                    }
                });
                if let Err(error) = result {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| {
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to inject Tab AI context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        });
                    }
                }
            });
        })
        .detach();
    }

    /// Terminate any active PTY harness session without restoring views.
    ///
    /// Used at Agent Chat open to kill a stale prewarm session, and by close to
    /// tear down the PTY for `QuickTerminalView`.
    fn terminate_tab_ai_harness_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.tab_ai_harness.as_ref() {
            let result = session.entity.update(cx, |term, _cx| {
                term.terminate_session().map_err(|e| e.to_string())
            });
            match result {
                Ok(()) => {
                    self.tab_ai_harness = None;
                }
                Err(error) => {
                    tracing::warn!(
                        event = "tab_ai_harness_terminal_kill_failed",
                        error = %error,
                    );
                }
            }
        }
    }

    /// Close the Tab AI harness surface.
    ///
    /// **Close semantics contract:**
    /// - `Cmd+W` closes the Quick Terminal main window state-first (handled in `render_prompts/term.rs`).
    /// - Restore-origin callers still use the saved return view/focus for apply-back and embedded Agent Chat close.
    /// - Plain `Escape` is forwarded to the PTY so the harness TUI can handle it.
    /// - The footer advertises the same Close affordance as Cmd+W for Quick Terminal.
    ///
    /// **Lifecycle contract:**
    /// - For `QuickTerminalView`: tears down PTY, clears harness, schedules prewarm.
    /// - For `AgentChatView`: restores view/focus without touching PTY lifecycle.
    fn clear_transient_script_list_trigger_on_return(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            self.tab_ai_harness_script_list_trigger = None;
            return;
        }

        let Some(trigger) = self.tab_ai_harness_script_list_trigger.take() else {
            return;
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "script_list_transient_trigger_cleared",
            trigger = %trigger,
            via_window = window.is_some(),
        );

        if let Some(window) = window {
            self.clear_filter(window, cx);
        } else {
            self.reset_script_list_filter_and_selection_state(cx);
        }
    }

    fn close_tab_ai_harness_terminal_impl(
        &mut self,
        window: Option<&mut Window>,
        disposition: TabAiHarnessCloseDisposition,
        cx: &mut Context<Self>,
    ) {
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_close_with_window_decision",
            return_is_script_list = self
                .tab_ai_harness_return_view
                .as_ref()
                .map_or(true, |v| matches!(v, AppView::ScriptList)),
            has_pending_script_list_trigger = pending_script_list_trigger.is_some(),
            pending_script_list_trigger = ?pending_script_list_trigger,
            filter_text = %self.filter_text,
            current_view = ?self.current_view,
        );
        let closing_quick_terminal = matches!(self.current_view, AppView::QuickTerminalView { .. });
        let closing_agent_chat = matches!(self.current_view, AppView::AgentChatView { .. });
        let closing_pi_agent_chat =
            closing_agent_chat && self.active_agent_chat_warm_lease.is_some();

        if !closing_quick_terminal && !closing_agent_chat {
            return;
        }

        // Invalidate any in-flight deferred capture so late results cannot
        // inject stale context after the surface has been closed.
        self.tab_ai_harness_capture_generation += 1;

        // Clear the apply-back route so stale targets cannot leak across sessions.
        self.tab_ai_harness_apply_back_route = None;

        // Only the legacy PTY-backed quick terminal owns `self.tab_ai_harness`.
        if closing_quick_terminal {
            self.terminate_tab_ai_harness_session(cx);
        }
        let mut pending_warm_lease = None;
        if closing_agent_chat {
            if let AppView::AgentChatView { entity } = &self.current_view {
                self.embedded_agent_chat = Some(entity.clone());
                entity.update(cx, |view, cx| {
                    view.prepare_for_host_hide(cx);
                });
            }
            pending_warm_lease = self.active_agent_chat_warm_lease.take();
            if closing_pi_agent_chat {
                self.embedded_agent_chat = None;
            }
        }

        let mut return_view = self
            .tab_ai_harness_return_view
            .take()
            .unwrap_or(AppView::ScriptList);
        if closing_agent_chat && matches!(return_view, AppView::AgentChatView { .. }) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "embedded_agent_chat_return_origin_self_guarded",
                "Embedded Agent Chat close had Agent Chat as its own return origin; falling back to ScriptList"
            );
            return_view = AppView::ScriptList;
        }
        let return_is_script_list = matches!(return_view, AppView::ScriptList);
        let return_focus_target = self
            .tab_ai_harness_return_focus_target
            .take()
            .unwrap_or(FocusTarget::MainFilter);

        tracing::info!(
            event = "tab_ai_harness_terminal_close",
            focus_target = %format!("{return_focus_target:?}"),
            session_cleared = closing_quick_terminal,
            capture_generation = self.tab_ai_harness_capture_generation,
            disposition = ?disposition,
        );

        if disposition == TabAiHarnessCloseDisposition::CloseMainWindowStateFirst {
            self.tab_ai_harness_script_list_trigger = None;

            // Keep prewarm only for the actual PTY-backed quick terminal path.
            if closing_quick_terminal {
                self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);
            }

            self.close_and_reset_window(cx);
            if let Some(lease) = pending_warm_lease {
                self.dismiss_agent_chat_warm_lease_background(lease);
            }
            cx.notify();
            return;
        }

        if closing_agent_chat {
            self.agent_chat_ready_script_path = None;
            self.agent_chat_footer_dot_status = None;
            self.agent_chat_footer_model_display = None;
            self.agent_chat_footer_snapshot = None;
            let _receipt = self.exit_embedded_agent_chat_surface(
                return_view,
                return_focus_target,
                "close_tab_ai_harness_terminal_impl",
                cx,
            );
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "embedded_agent_chat_lifecycle_close_to_origin",
                return_view = ?self.current_view,
                focus_target = ?return_focus_target,
            );
        } else {
            self.restore_current_view_with_focus(return_view, return_focus_target);
        }

        // Returning to the launcher root consumes the "opened from main menu"
        // origin. Leaving it set makes the NEXT Escape on the empty ScriptList
        // run a no-op go_back_or_close instead of hiding the window (the
        // "brain item → Agent Chat → Escape → Escape doesn't close" bug).
        if matches!(self.current_view, AppView::ScriptList) && !self.is_in_attachment_portal() {
            self.opened_from_main_menu = false;
        }

        if return_is_script_list {
            self.clear_transient_script_list_trigger_on_return(window, cx);
        } else {
            self.tab_ai_harness_script_list_trigger = None;
        }

        // Keep prewarm only for the actual PTY-backed quick terminal path.
        if closing_quick_terminal {
            self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);
        }
        if let Some(lease) = pending_warm_lease {
            self.dismiss_agent_chat_warm_lease_background(lease);
        }
        cx.notify();
    }

    pub(crate) fn close_tab_ai_harness_terminal_with_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tab_ai_harness_terminal_impl(
            Some(window),
            TabAiHarnessCloseDisposition::RestoreOrigin,
            cx,
        );
    }

    pub(crate) fn close_tab_ai_harness_terminal(&mut self, cx: &mut Context<Self>) {
        // close_tab_ai_harness_terminal_impl(None, cx) owns the real contract:
        // - it guards the restore path to AppView::QuickTerminalView / AppView::AgentChatView
        // - tab_ai_harness_capture_generation += 1
        // - self.tab_ai_harness_apply_back_route = None;
        // - self.terminate_tab_ai_harness_session(cx);
        // - self.restore_current_view_with_focus(return_view, return_focus_target);
        // - the close log records session_cleared = closing_quick_terminal
        // - it requeues schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx)
        // - it ends with cx.notify()
        self.close_tab_ai_harness_terminal_impl(
            None,
            TabAiHarnessCloseDisposition::RestoreOrigin,
            cx,
        );
    }

    pub(crate) fn close_quick_terminal_main_window_state_first(&mut self, cx: &mut Context<Self>) {
        // State-first Quick Terminal close intentionally does not restore the
        // saved return view while the main panel is visible. `close_and_reset_window`
        // sets windowVisible=false before prompt cancellation/reset can produce
        // a hidden ScriptList state.
        self.close_tab_ai_harness_terminal_impl(
            None,
            TabAiHarnessCloseDisposition::CloseMainWindowStateFirst,
            cx,
        );
    }

    pub(crate) fn close_agent_chat_main_window_state_first(&mut self, cx: &mut Context<Self>) {
        self.close_tab_ai_harness_terminal_impl(
            None,
            TabAiHarnessCloseDisposition::CloseMainWindowStateFirst,
            cx,
        );
    }

    /// Close Agent Chat chat and force the main panel back to ScriptList.
    ///
    /// Used by the hotkey detach path where the intent is always to return
    /// to the launcher, regardless of which view opened the Agent Chat chat.
    /// This avoids the correctness bug where `close_tab_ai_harness_terminal`
    /// would restore an unrelated originating view (e.g. ClipboardHistory).
    ///
    /// When `focus_main_filter` is `false`, the main panel switches to
    /// ScriptList but does not reclaim keyboard focus — this keeps the
    /// newly-detached chat window as the active key target.
    pub(crate) fn close_agent_chat_to_script_list(
        &mut self,
        focus_main_filter: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::AgentChatView { .. }) {
            return;
        }

        // Invalidate any in-flight deferred capture so late results cannot
        // target a chat surface that has already been detached.
        self.tab_ai_harness_capture_generation += 1;
        self.tab_ai_harness_apply_back_route = None;
        if let AppView::AgentChatView { entity } = &self.current_view {
            // Detach hand-off: when the detached window now owns this thread,
            // leave the embedded cache empty so the next main-window AI entry
            // starts a fresh chat instead of aliasing the detached
            // conversation. Reattach pulls the live thread back from the
            // detached view itself (see reattach_embedded_agent_chat_from_detached).
            let detached_owns_thread =
                crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
                    .and_then(|view| view.read(cx).thread())
                    .zip(entity.read(cx).thread())
                    .is_some_and(|(detached, ours)| detached == ours);
            if detached_owns_thread {
                self.embedded_agent_chat = None;
            } else {
                self.embedded_agent_chat = Some(entity.clone());
            }
            entity.update(cx, |view, cx| {
                view.prepare_for_host_hide(cx);
            });
            if detached_owns_thread {
                // The detached window keeps streaming on the warm lease's
                // connection: regenerate the warm slot WITHOUT cancelling the
                // in-flight turn (dismiss would kill the detached response).
                if let Some(lease) = self.active_agent_chat_warm_lease.take() {
                    self.release_agent_chat_warm_lease_for_detach_handoff(lease);
                }
            }
        }
        let pending_warm_lease = self.active_agent_chat_warm_lease.take();

        // A hotkey detach is a deliberate mode switch back to the launcher,
        // not a normal "return to the originating surface" close.
        self.tab_ai_harness_return_view = None;
        self.tab_ai_harness_return_focus_target = None;

        self.current_view = AppView::ScriptList;
        // Same launcher-root landing rule as the close-to-origin path above:
        // clear the origin flag so the next Escape hides the window instead of
        // burning a press on a no-op go_back_or_close.
        self.opened_from_main_menu = false;
        self.agent_chat_ready_script_path = None;
        self.agent_chat_footer_dot_status = None;
        self.agent_chat_footer_model_display = None;
        self.agent_chat_footer_snapshot = None;
        self.pending_focus = if focus_main_filter {
            Some(FocusTarget::MainFilter)
        } else {
            None
        };
        self.focused_input = if focus_main_filter {
            FocusedInput::MainFilter
        } else {
            FocusedInput::None
        };
        self.clear_transient_script_list_trigger_on_return(None, cx);

        // Re-key main's automation surface tag in lockstep with the view flip.
        // Without this, `listAutomationWindows` reports `semanticSurface:"agentChatChat"`
        // on main even though the view is back on ScriptList, until the next
        // hide/show re-keys it. Mirrors the hide path in window_visibility.rs
        // which calls the same helper after `reset_to_script_list`.
        crate::windows::update_automation_semantic_surface("main", Some("scriptList".to_string()));

        // Pair with the entry upsert in `enter_embedded_agent_chat_surface`:
        // tear the AI entry back out of the automation registry so
        // `listAutomationWindows` stops reporting a kind=ai window once the
        // user is back on ScriptList.
        crate::windows::ensure_embedded_ai_window(false);
        self.transition_agent_chat_surface(
            crate::ai::agent_chat::ui::surface_state::AgentChatSurfaceEvent::EmbeddedClosed,
        );

        tracing::info!(
            event = "agent_chat_restored_to_script_list",
            capture_generation = self.tab_ai_harness_capture_generation,
            focus_main_filter,
        );
        if let Some(lease) = pending_warm_lease {
            self.dismiss_agent_chat_warm_lease_background(lease);
        }
        cx.notify();
    }

    fn dismiss_agent_chat_warm_lease_background(
        &mut self,
        lease: crate::ai::agent_chat::warm_session::AgentChatWarmSessionLease,
    ) {
        let key = lease.key.clone();
        let generation = lease.generation;
        let snapshot =
            crate::ai::agent_chat::launch::warm_session_manager().dismiss_reset_background(lease);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "pi_agent_chat_warm_dismiss_reset_background",
            warm_key = %key,
            generation,
            replacement_generation = snapshot.generation,
            replacement_state = ?snapshot.state,
        );
    }

    /// Release a warm lease whose connection now belongs to the detached chat
    /// window. Regenerates the slot (next launch gets a fresh warm session)
    /// without cancelling the detached conversation's in-flight turn.
    fn release_agent_chat_warm_lease_for_detach_handoff(
        &mut self,
        lease: crate::ai::agent_chat::warm_session::AgentChatWarmSessionLease,
    ) {
        let key = lease.key.clone();
        let generation = lease.generation;
        let snapshot =
            crate::ai::agent_chat::launch::warm_session_manager().release_for_detach_handoff(lease);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "pi_agent_chat_warm_release_for_detach_handoff",
            warm_key = %key,
            generation,
            replacement_generation = snapshot.generation,
            replacement_state = ?snapshot.state,
        );
    }

    fn dismiss_active_agent_chat_warm_lease(&mut self) {
        if let Some(lease) = self.active_agent_chat_warm_lease.take() {
            self.dismiss_agent_chat_warm_lease_background(lease);
        }
    }

    /// Schedule a deferred prewarm of the Tab AI harness so the next Tab
    /// press gets an instant fresh session after a close cycle.
    fn schedule_tab_ai_harness_prewarm(
        &mut self,
        delay: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        let app_weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = cx.update(|cx| {
                if let Some(app) = app_weak.upgrade() {
                    app.update(cx, |this, cx| {
                        this.warm_tab_ai_harness_after_close(cx);
                    });
                }
            });
        })
        .detach();
    }

    /// Build context from explicit inputs, resolving targets and clipboard
    /// against the provided `source_view` (the view that was active when Tab
    /// was pressed) rather than `self.current_view` (which may now differ).
    fn build_tab_ai_context_from(
        &self,
        intent_for_lookup: String,
        source_view: AppView,
        ui: crate::ai::TabAiUiSnapshot,
        desktop: crate::context_snapshot::AiContextSnapshot,
        invocation_receipt: crate::ai::TabAiInvocationReceipt,
        _cx: &Context<Self>,
    ) -> TabAiResolvedContext {
        let bundle_id = desktop
            .frontmost_app
            .as_ref()
            .map(|app| app.bundle_id.clone());
        let recent_inputs = self.input_history.recent_entries(5);
        let clipboard_selected_index = match &source_view {
            AppView::ClipboardHistoryView { selected_index, .. } => Some(*selected_index),
            _ => None,
        };
        let clipboard_history = self.resolve_tab_ai_clipboard_history(
            clipboard_selected_index,
            TAB_AI_CLIPBOARD_HISTORY_LIMIT,
        );
        let clipboard = clipboard_history
            .first()
            .map(|entry| crate::ai::TabAiClipboardContext {
                content_type: entry.content_type.clone(),
                preview: entry.preview.clone(),
                ocr_text: entry.ocr_text.clone(),
            });
        let prior_automations = match crate::ai::resolve_tab_ai_prior_automations_for_entry(
            &intent_for_lookup,
            bundle_id.as_deref(),
            3,
        ) {
            Ok(items) => items,
            Err(error) => {
                tracing::warn!(event = "tab_ai_prior_automation_lookup_failed", error = %error);
                Vec::new()
            }
        };
        let (focused_target, visible_targets) =
            self.resolve_tab_ai_targets_with_audit_for_view(&source_view, &ui, "submit_context");

        let suggested_intents = crate::ai::build_tab_ai_suggested_intents(
            focused_target.as_ref(),
            clipboard.as_ref(),
            &prior_automations,
        );

        let context = crate::ai::TabAiContextBlob::from_parts_with_targets(
            ui,
            focused_target,
            visible_targets,
            desktop,
            recent_inputs,
            clipboard,
            clipboard_history,
            prior_automations,
            chrono::Utc::now().to_rfc3339(),
        );

        TabAiResolvedContext {
            context,
            invocation_receipt,
            suggested_intents,
        }
    }

    fn seed_agent_chat_return_origin_for_view(&mut self, source_view: &AppView) {
        self.tab_ai_harness_return_view = Some(source_view.clone());
        self.tab_ai_harness_return_focus_target =
            Some(Self::tab_ai_return_focus_target_for_view(source_view));
    }

    pub(crate) fn seed_agent_chat_dictation_return_origin(&mut self) {
        self.tab_ai_harness_return_view = Some(AppView::ScriptList);
        self.tab_ai_harness_return_focus_target = Some(FocusTarget::MainFilter);
        self.tab_ai_harness_script_list_trigger = None;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_dictation_return_origin_seeded",
            return_view = "ScriptList",
            return_focus_target = "MainFilter",
        );
    }

    fn tab_ai_return_focus_target_for_view(source_view: &AppView) -> FocusTarget {
        match source_view {
            AppView::ScriptList
            | AppView::ClipboardHistoryView { .. }
            | AppView::AppLauncherView { .. }
            | AppView::WindowSwitcherView { .. }
            | AppView::BrowserTabsView { .. }
            | AppView::FileSearchView { .. }
            | AppView::ProfileSearchView { .. }
            | AppView::ThemeChooserView { .. }
            | AppView::EmojiPickerView { .. }
            | AppView::BrowseKitsView { .. }
            | AppView::MigrateV1View { .. }
            | AppView::InstalledKitsView { .. }
            | AppView::ProcessManagerView { .. }
            | AppView::FlowUxView { .. }
            | AppView::SearchAiPresetsView { .. }
            | AppView::CreateAiPresetView { .. }
            | AppView::SettingsView { .. }
            | AppView::FavoritesBrowseView { .. }
            | AppView::AgentChatHistoryView { .. }
            | AppView::BrowserHistoryView { .. }
            | AppView::DictationHistoryView { .. }
            | AppView::NotesBrowseView { .. }
            | AppView::CurrentAppCommandsView { .. }
            | AppView::DesignGalleryView { .. }
            | AppView::FooterGalleryView { .. }
            | AppView::CreationFeedback { .. }
            | AppView::ScriptIssuesView { .. }
            | AppView::SdkReferenceView { .. }
            | AppView::ScriptTemplateCatalogView { .. }
            | AppView::ActionsDialog => FocusTarget::MainFilter,

            AppView::About { .. } => FocusTarget::AppRoot,

            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. }
            | AppView::DivPrompt { .. }
            | AppView::WebcamView { .. } => FocusTarget::AppRoot,

            AppView::FormPrompt { .. } => FocusTarget::FormPrompt,

            AppView::EditorPrompt { .. } | AppView::ScratchPadView { .. } => {
                FocusTarget::EditorPrompt
            }

            AppView::SelectPrompt { .. } => FocusTarget::SelectPrompt,
            AppView::PathPrompt { .. } => FocusTarget::PathPrompt,
            AppView::EnvPrompt { .. } => FocusTarget::EnvPrompt,
            AppView::DropPrompt { .. } => FocusTarget::DropPrompt,
            AppView::TemplatePrompt { .. } => FocusTarget::TemplatePrompt,
            AppView::HotkeyPrompt { .. } => FocusTarget::AppRoot,

            AppView::TermPrompt { .. } | AppView::QuickTerminalView { .. } => {
                FocusTarget::TermPrompt
            }

            AppView::ChatPrompt { .. } | AppView::AgentChatView { .. } => FocusTarget::ChatPrompt,
            AppView::DayPage { .. } => FocusTarget::EditorPrompt,
            AppView::NamingPrompt { .. } => FocusTarget::NamingPrompt,

            AppView::ConfirmPrompt { .. } => FocusTarget::AppRoot,
            AppView::NonListStatesView { .. } => FocusTarget::AppRoot,
            AppView::PermissionsWizardView { .. } => FocusTarget::AppRoot,

            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => FocusTarget::AppRoot,
        }
    }

    /// Return the correct `FocusTarget` for the originating surface so that
    /// closing the Tab AI overlay restores focus to the right place.
    fn tab_ai_return_focus_target(&self) -> FocusTarget {
        Self::tab_ai_return_focus_target_for_view(&self.current_view)
    }

    /// Resolve a bounded clipboard history with hydrated content from the
    /// clipboard database. The selected entry (if any) is always first,
    /// followed by other recent entries in order, deduplicated by content.
    fn resolve_tab_ai_clipboard_history(
        &self,
        selected_index: Option<usize>,
        limit: usize,
    ) -> Vec<crate::ai::TabAiClipboardHistoryEntry> {
        let mut ordered: Vec<&crate::clipboard_history::ClipboardEntryMeta> = Vec::new();

        // Selected entry goes first
        if let Some(selected) =
            selected_index.and_then(|index| self.cached_clipboard_entries.get(index))
        {
            ordered.push(selected);
        }

        for entry in &self.cached_clipboard_entries {
            if ordered.iter().any(|candidate| candidate.id == entry.id) {
                continue;
            }
            ordered.push(entry);
            if ordered.len() >= limit {
                break;
            }
        }

        let mut last_dedupe_key: Option<String> = None;
        let mut result = Vec::new();

        for entry in ordered {
            // Hydrate text content from the database for text-like entries
            let full_text = match entry.content_type {
                crate::clipboard_history::ContentType::Text
                | crate::clipboard_history::ContentType::Link
                | crate::clipboard_history::ContentType::File
                | crate::clipboard_history::ContentType::Color => {
                    crate::clipboard_history::get_entry_content(&entry.id)
                        .filter(|content| !content.trim().is_empty())
                        .map(|content| {
                            crate::ai::truncate_tab_ai_text(&content, TAB_AI_CLIPBOARD_TEXT_LIMIT)
                        })
                }
                crate::clipboard_history::ContentType::Image => None,
            };

            let preview_source = full_text
                .clone()
                .or_else(|| entry.ocr_text.clone())
                .unwrap_or_else(|| entry.display_preview());

            // Deduplicate consecutive identical entries
            let dedupe_key = format!("{}::{}", entry.content_type.as_str(), preview_source);
            if last_dedupe_key.as_deref() == Some(dedupe_key.as_str()) {
                continue;
            }
            last_dedupe_key = Some(dedupe_key);

            result.push(crate::ai::TabAiClipboardHistoryEntry {
                id: entry.id.clone(),
                content_type: entry.content_type.as_str().to_string(),
                timestamp: entry.timestamp,
                preview: crate::ai::truncate_tab_ai_text(
                    &preview_source,
                    TAB_AI_CLIPBOARD_TEXT_LIMIT,
                ),
                full_text,
                ocr_text: entry
                    .ocr_text
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| {
                        crate::ai::truncate_tab_ai_text(&text, TAB_AI_CLIPBOARD_TEXT_LIMIT)
                    }),
                image_width: entry.image_width,
                image_height: entry.image_height,
            });
        }

        result
    }

    fn tab_ai_target_from_element(
        prompt_type: &str,
        element: &crate::protocol::ElementInfo,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: prompt_type.to_string(),
            kind: format!("{:?}", element.element_type).to_lowercase(),
            semantic_id: element.semantic_id.clone(),
            label: element
                .text
                .clone()
                .or_else(|| element.value.clone())
                .unwrap_or_else(|| element.semantic_id.clone()),
            metadata: Some(serde_json::json!({
                "text": element.text.clone(),
                "value": element.value.clone(),
                "selected": element.selected,
                "focused": element.focused,
                "index": element.index,
            })),
        }
    }

    /// Convert a `SearchResult` from the Script List into a `TabAiTargetContext`
    /// with script-native metadata (name, path, description, type).
    pub(crate) fn tab_ai_target_from_search_result(
        index: usize,
        result: &scripts::SearchResult,
    ) -> crate::ai::TabAiTargetContext {
        let name = result.name().to_string();
        let kind = match result {
            scripts::SearchResult::Script(_) => "script",
            scripts::SearchResult::Scriptlet(_) => "scriptlet",
            scripts::SearchResult::BuiltIn(_) => "builtin",
            scripts::SearchResult::App(_) => "app",
            scripts::SearchResult::Window(_) => "window",
            scripts::SearchResult::File(_) => "file",
            scripts::SearchResult::Note(_) => "note",
            scripts::SearchResult::BrainHit(_) => "brain",
            scripts::SearchResult::BrainInboxItem(_) => "brainInbox",
            scripts::SearchResult::Todo(_) => "todo",
            scripts::SearchResult::AgentChatHistory(_) => "agent_chatHistory",
            scripts::SearchResult::AiVault(_) => "aiVault",
            scripts::SearchResult::ClipboardHistory(_) => "clipboardHistory",
            scripts::SearchResult::DictationHistory(_) => "dictationHistory",
            scripts::SearchResult::BrowserTab(_) => "browserTab",
            scripts::SearchResult::BrowserHistory(_) => "browserHistory",
            scripts::SearchResult::Agent(_) => "agent",
            scripts::SearchResult::Skill(_) => "skill",
            scripts::SearchResult::Fallback(_) => "fallback",
            scripts::SearchResult::ScriptIssue(_) => "scriptIssue",
            scripts::SearchResult::SpineProjection(_) => "spineProjection",
        };
        let metadata = match result {
            scripts::SearchResult::Script(m) => serde_json::json!({
                "name": m.script.name,
                "path": m.script.path.to_string_lossy(),
                "description": m.script.description,
                "shortcut": m.script.shortcut,
                "alias": m.script.alias,
            }),
            scripts::SearchResult::Scriptlet(m) => serde_json::json!({
                "name": m.scriptlet.name,
                "description": m.scriptlet.description,
                "filePath": m.scriptlet.file_path,
            }),
            scripts::SearchResult::BuiltIn(m) => serde_json::json!({
                "id": m.entry.id,
                "name": m.entry.name,
                "description": m.entry.description,
            }),
            scripts::SearchResult::App(m) => serde_json::json!({
                "name": m.app.name,
                "path": m.app.path.to_string_lossy(),
                "bundleId": m.app.bundle_id,
            }),
            scripts::SearchResult::Window(m) => serde_json::json!({
                "app": m.window.app,
                "title": m.window.title,
            }),
            scripts::SearchResult::File(m) => serde_json::json!({
                "name": m.file.name,
                "path": m.file.path,
                "fileType": format!("{:?}", m.file.file_type),
            }),
            scripts::SearchResult::Note(m) => serde_json::json!({
                "noteId": m.hit.id.as_str(),
                "title": m.title,
                "updatedAt": m.hit.updated_at.to_rfc3339(),
                "isPinned": m.hit.is_pinned,
                "charCount": m.hit.char_count,
            }),
            scripts::SearchResult::BrainHit(m) => serde_json::json!({
                "source": m.hit.source.as_str(),
                "sourceId": m.hit.source_id,
                "title": m.hit.title,
                "excerpt": m.hit.excerpt,
            }),
            scripts::SearchResult::BrainInboxItem(m) => serde_json::json!({
                "id": m.item.id,
                "kind": m.item.kind.as_str(),
                "title": m.item.title,
                "detail": m.item.detail,
                "source": m.item.source,
                "sourceId": m.item.source_id,
            }),
            scripts::SearchResult::Todo(m) => serde_json::json!({
                "stableKey": m.hit.stable_key,
                "title": m.hit.title,
                "body": m.hit.body,
                "subtitle": m.hit.subtitle,
                "tags": m.hit.tags,
                "priority": m.hit.priority,
                "due": m.hit.due,
                "createdAt": m.hit.created_at,
            }),
            scripts::SearchResult::AgentChatHistory(m) => serde_json::json!({
                "sessionId": m.entry.session_id,
                "title": m.entry.title_display(),
                "preview": m.entry.preview_display(),
                "messageCount": m.entry.message_count,
                "matchedField": format!("{:?}", m.matched_field),
            }),
            scripts::SearchResult::AiVault(m) => serde_json::json!({
                "stableKey": m.hit.stable_key,
                "provider": m.hit.provider,
                "providerDisplayName": m.hit.provider_display_name,
                "sessionId": m.hit.session_id,
                "sourceKind": m.hit.source_kind,
                "safeTitle": m.hit.safe_title,
                "workspacePath": m.hit.workspace_path,
                "model": m.hit.model,
                "modifiedAt": m.hit.modified_at,
                "matchedField": format!("{:?}", m.hit.matched_field),
            }),
            scripts::SearchResult::ClipboardHistory(m) => serde_json::json!({
                "id": m.entry.id,
                "contentType": m.entry.content_type.as_str(),
                "preview": m.title,
                "pinned": m.entry.pinned,
                "timestamp": m.entry.timestamp,
            }),
            scripts::SearchResult::DictationHistory(m) => serde_json::json!({
                "id": m.id,
                "preview": m.preview,
                "target": m.target,
                "timestamp": m.timestamp,
                "audioDurationMs": m.audio_duration_ms,
                "matchedField": format!("{:?}", m.matched_field),
            }),
            scripts::SearchResult::BrowserTab(m) => serde_json::json!({
                "stableKey": m.hit.stable_key,
                "title": m.hit.title,
                "url": m.hit.url,
                "domain": m.hit.domain,
                "provider": m.hit.provider_label,
                "windowIndex": m.hit.tab.window_index,
                "tabIndex": m.hit.tab.tab_index,
            }),
            scripts::SearchResult::BrowserHistory(m) => serde_json::json!({
                "stableKey": m.hit.stable_key,
                "title": m.hit.title,
                "url": m.hit.url,
                "domain": m.hit.domain,
                "provider": m.hit.provider_label,
                "profile": m.hit.profile_label,
                "lastVisitUnixMs": m.hit.last_visit_unix_ms,
                "visitCount": m.hit.visit_count,
            }),
            scripts::SearchResult::Agent(m) => serde_json::json!({
                "name": m.agent.name,
                "path": m.agent.path.to_string_lossy(),
                "description": m.agent.description,
            }),
            scripts::SearchResult::Skill(m) => serde_json::json!({
                "pluginId": m.skill.plugin_id,
                "skillId": m.skill.skill_id,
                "title": m.skill.title,
                "description": m.skill.description,
                "path": m.skill.path.to_string_lossy(),
            }),
            scripts::SearchResult::Fallback(m) => serde_json::json!({
                "name": m.display_label(),
                "description": m.display_description(),
            }),
            scripts::SearchResult::ScriptIssue(m) => serde_json::json!({
                "title": m.title,
                "description": m.description,
                "failedCount": m.failed_count,
                "fatalCount": m.fatal_count,
                "warningCount": m.warning_count,
            }),
            scripts::SearchResult::SpineProjection(_) => serde_json::json!({}),
        };
        crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: kind.to_string(),
            semantic_id: crate::protocol::generate_semantic_id("choice", index, &name),
            label: name,
            metadata: Some(metadata),
        }
    }

    fn tab_ai_target_from_app_launcher_row(
        display_index: usize,
        source_index: usize,
        app: &app_launcher::AppInfo,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "AppLauncher".to_string(),
            kind: "app".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("choice", display_index, &app.name),
            label: app.name.clone(),
            metadata: Some(serde_json::json!({
                "name": app.name.clone(),
                "path": app.path.to_string_lossy(),
                "bundleId": app.bundle_id.clone(),
                "sourceIndex": source_index,
            })),
        }
    }

    fn tab_ai_target_from_process_manager_row(
        display_index: usize,
        source_index: usize,
        process: &crate::process_manager::ProcessInfo,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "ProcessManager".to_string(),
            kind: "process".to_string(),
            semantic_id: crate::protocol::generate_semantic_id(
                "choice",
                display_index,
                &process.script_path,
            ),
            label: process.script_path.clone(),
            metadata: Some(serde_json::json!({
                "pid": process.pid,
                "scriptPath": process.script_path.clone(),
                "startedAt": process.started_at.to_rfc3339(),
                "sourceIndex": source_index,
            })),
        }
    }

    fn tab_ai_target_from_sdk_reference_row(
        row: crate::mcp_resources::SdkReferenceVisibleRow<'_>,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "SdkReference".to_string(),
            kind: "sdk_function".to_string(),
            semantic_id: crate::protocol::generate_semantic_id(
                "choice",
                row.display_index,
                &row.entry.name,
            ),
            label: row.entry.name.clone(),
            metadata: Some(serde_json::json!({
                "name": row.entry.name.clone(),
                "signature": row.entry.signature.clone(),
                "description": row.entry.description.clone(),
                "category": row.entry.category.clone(),
                "support": row.entry.support,
                "unsupportedNote": row.entry.unsupported_note.clone(),
                "sourceIndex": row.source_index,
            })),
        }
    }

    fn tab_ai_target_from_script_template_catalog_row(
        row: crate::mcp_resources::ScriptTemplateCatalogVisibleRow<'_>,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "ScriptTemplateCatalog".to_string(),
            kind: "script_template".to_string(),
            semantic_id: crate::protocol::generate_semantic_id(
                "choice",
                row.display_index,
                &row.template.title,
            ),
            label: row.template.title.clone(),
            metadata: Some(serde_json::json!({
                "id": row.template.id.clone(),
                "title": row.template.title.clone(),
                "description": row.template.description.clone(),
                "category": row.template.category.clone(),
                "filenameHint": row.template.filename_hint.clone(),
                "sourceIndex": row.source_index,
            })),
        }
    }

    /// Return the first `limit` file search results in display order.
    fn visible_file_search_results(
        &self,
        limit: usize,
    ) -> Vec<(usize, &crate::file_search::FileResult)> {
        (0..self.file_search_display_indices.len())
            .take(limit)
            .filter_map(|display_index| {
                self.file_search_result_at_display_index(display_index)
                    .map(|result| (display_index, result))
            })
            .collect()
    }

    /// Determine the query mode label for file search AI context.
    fn file_search_query_mode(query: &str) -> &'static str {
        if crate::file_search::parse_directory_path(query).is_some() {
            "path-browse"
        } else if crate::file_search::looks_like_advanced_mdquery(query) {
            "spotlight-advanced"
        } else {
            "spotlight-basic"
        }
    }

    /// Build surface-level metadata for the file search view, shared across
    /// all targets so the AI can reason about the query and visible result set.
    fn file_search_surface_metadata(
        &self,
        query: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut metadata = serde_json::Map::new();

        let visible_results: Vec<serde_json::Value> = self
            .visible_file_search_results(TAB_AI_VISIBLE_TARGET_LIMIT)
            .into_iter()
            .map(|(display_index, entry)| {
                serde_json::json!({
                    "displayIndex": display_index,
                    "name": entry.name.clone(),
                    "path": entry.path.clone(),
                    "fileType": format!("{:?}", entry.file_type),
                })
            })
            .collect();

        metadata.insert(
            "query".to_string(),
            serde_json::Value::String(query.to_string()),
        );
        metadata.insert(
            "queryMode".to_string(),
            serde_json::Value::String(Self::file_search_query_mode(query).to_string()),
        );
        metadata.insert(
            "visibleResultCount".to_string(),
            serde_json::json!(self.file_search_display_indices.len()),
        );
        metadata.insert(
            "visibleResults".to_string(),
            serde_json::Value::Array(visible_results),
        );

        if let Some(parsed) = crate::file_search::parse_directory_path(query) {
            metadata.insert(
                "directory".to_string(),
                serde_json::Value::String(parsed.directory),
            );
            metadata.insert(
                "directoryFilter".to_string(),
                match parsed.filter {
                    Some(filter) => serde_json::Value::String(filter),
                    None => serde_json::Value::Null,
                },
            );
        }

        metadata
    }

    /// Convert a file search result into a `TabAiTargetContext`, enriched
    /// with surface-level metadata about the query mode and visible results.
    fn tab_ai_target_from_file_search_result(
        display_index: usize,
        entry: &crate::file_search::FileResult,
        surface_metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::ai::TabAiTargetContext {
        let mut metadata = surface_metadata.clone();
        metadata.insert("displayIndex".to_string(), serde_json::json!(display_index));
        metadata.insert(
            "path".to_string(),
            serde_json::Value::String(entry.path.clone()),
        );
        metadata.insert(
            "fileType".to_string(),
            serde_json::Value::String(format!("{:?}", entry.file_type)),
        );
        metadata.insert("size".to_string(), serde_json::json!(entry.size));
        metadata.insert("modified".to_string(), serde_json::json!(entry.modified));

        crate::ai::TabAiTargetContext {
            source: "FileSearch".to_string(),
            kind: if entry.file_type == crate::file_search::FileType::Directory {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            semantic_id: crate::protocol::generate_semantic_id(
                "choice",
                display_index,
                &entry.name,
            ),
            label: entry.name.clone(),
            metadata: Some(serde_json::Value::Object(metadata)),
        }
    }

    /// Convert a search query into a `TabAiTargetContext`.
    fn tab_ai_target_from_search_query(
        source: &str,
        query: &str,
        metadata: serde_json::Value,
    ) -> crate::ai::TabAiTargetContext {
        let trimmed = query.trim();
        let label = crate::ai::truncate_tab_ai_text(trimmed, 96);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_search_query_target_resolved",
            item_source = %source,
            label_chars = label.chars().count(),
        );
        crate::ai::TabAiTargetContext {
            source: source.to_string(),
            kind: "search_query".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("query", 0, trimmed),
            label,
            metadata: Some(metadata),
        }
    }

    /// Convert raw input text into a lightweight `TabAiTargetContext`.
    fn tab_ai_target_from_input_text(
        source: &str,
        input_text: &str,
    ) -> crate::ai::TabAiTargetContext {
        let trimmed = input_text.trim();
        let label = crate::ai::truncate_tab_ai_text(trimmed, 96);
        let preview = crate::ai::truncate_tab_ai_text(trimmed, 400);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_input_target_resolved",
            item_source = %source,
            label_chars = label.chars().count(),
        );
        crate::ai::TabAiTargetContext {
            source: source.to_string(),
            kind: "input".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("input", 0, trimmed),
            label,
            metadata: Some(serde_json::json!({
                "inputPreview": preview,
                "inputLength": trimmed.chars().count(),
            })),
        }
    }

    /// Source-view-aware target resolution: resolves targets against an explicit view
    /// instead of `self.current_view`. Used at submit time when `current_view`
    /// has already switched to the harness terminal.
    fn resolve_tab_ai_surface_targets_for_view(
        &self,
        view: &AppView,
        ui: &crate::ai::TabAiUiSnapshot,
    ) -> (
        Option<crate::ai::TabAiTargetContext>,
        Vec<crate::ai::TabAiTargetContext>,
    ) {
        match view {
            AppView::ClipboardHistoryView { selected_index, .. } => {
                let focused_target =
                    self.cached_clipboard_entries
                        .get(*selected_index)
                        .map(|entry| {
                            let preview = if entry.content_type.as_str() == "image" {
                                entry
                                    .ocr_text
                                    .clone()
                                    .filter(|text| !text.trim().is_empty())
                                    .unwrap_or_else(|| entry.display_preview())
                            } else {
                                entry.display_preview()
                            };
                            crate::ai::TabAiTargetContext {
                                source: "ClipboardHistory".to_string(),
                                kind: "clipboard_entry".to_string(),
                                semantic_id: crate::protocol::generate_semantic_id(
                                    "choice",
                                    *selected_index,
                                    &entry.text_preview,
                                ),
                                label: preview.clone(),
                                metadata: Some(serde_json::json!({
                                    "id": entry.id.clone(),
                                    "timestamp": entry.timestamp,
                                    "contentType": entry.content_type.as_str(),
                                    "preview": preview,
                                    "ocrText": entry.ocr_text.clone(),
                                    "imageWidth": entry.image_width,
                                    "imageHeight": entry.image_height,
                                })),
                            }
                        });
                let visible_targets = self
                    .cached_clipboard_entries
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| {
                        let preview = if entry.content_type.as_str() == "image" {
                            entry
                                .ocr_text
                                .clone()
                                .filter(|text| !text.trim().is_empty())
                                .unwrap_or_else(|| entry.display_preview())
                        } else {
                            entry.display_preview()
                        };
                        crate::ai::TabAiTargetContext {
                            source: "ClipboardHistory".to_string(),
                            kind: "clipboard_entry".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice",
                                index,
                                &entry.text_preview,
                            ),
                            label: preview.clone(),
                            metadata: Some(serde_json::json!({
                                "id": entry.id.clone(),
                                "timestamp": entry.timestamp,
                                "contentType": entry.content_type.as_str(),
                                "preview": preview,
                                "ocrText": entry.ocr_text.clone(),
                                "imageWidth": entry.image_width,
                                "imageHeight": entry.image_height,
                            })),
                        }
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            } => {
                let surface_metadata = self.file_search_surface_metadata(query);
                let focused_target = self
                    .selected_file_search_result(*selected_index)
                    .map(|(display_index, entry)| {
                        Self::tab_ai_target_from_file_search_result(
                            display_index,
                            entry,
                            &surface_metadata,
                        )
                    })
                    .or_else(|| {
                        let trimmed = query.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(Self::tab_ai_target_from_search_query(
                                "FileSearch",
                                trimmed,
                                serde_json::Value::Object(surface_metadata.clone()),
                            ))
                        }
                    });
                let visible_targets = self
                    .visible_file_search_results(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .into_iter()
                    .map(|(display_index, entry)| {
                        Self::tab_ai_target_from_file_search_result(
                            display_index,
                            entry,
                            &surface_metadata,
                        )
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::WindowSwitcherView { selected_index, .. } => {
                let focused_target = self.cached_windows.get(*selected_index).map(|entry| {
                    let label = format!("{} — {}", entry.app, entry.title);
                    crate::ai::TabAiTargetContext {
                        source: "WindowSwitcher".to_string(),
                        kind: "window".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            *selected_index,
                            &label,
                        ),
                        label,
                        metadata: Some(serde_json::json!({
                            "app": entry.app.clone(),
                            "title": entry.title.clone(),
                        })),
                    }
                });
                let visible_targets = self
                    .cached_windows
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| {
                        let label = format!("{} — {}", entry.app, entry.title);
                        crate::ai::TabAiTargetContext {
                            source: "WindowSwitcher".to_string(),
                            kind: "window".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice", index, &label,
                            ),
                            label,
                            metadata: Some(serde_json::json!({
                                "app": entry.app.clone(),
                                "title": entry.title.clone(),
                            })),
                        }
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                let focused_target = self
                    .app_launcher_selected_visible_entry(filter, *selected_index)
                    .map(|(source_index, app)| {
                        Self::tab_ai_target_from_app_launcher_row(
                            *selected_index,
                            source_index,
                            app,
                        )
                    });
                let visible_targets = self
                    .app_launcher_visible_target_rows(filter, TAB_AI_VISIBLE_TARGET_LIMIT)
                    .into_iter()
                    .map(|(display_index, source_index, app)| {
                        Self::tab_ai_target_from_app_launcher_row(display_index, source_index, app)
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                let focused_target = self
                    .process_manager_selected_visible_entry(filter, *selected_index)
                    .map(|(source_index, process)| {
                        Self::tab_ai_target_from_process_manager_row(
                            *selected_index,
                            source_index,
                            process,
                        )
                    });
                let visible_targets = self
                    .process_manager_visible_target_rows(filter, TAB_AI_VISIBLE_TARGET_LIMIT)
                    .into_iter()
                    .map(|(display_index, source_index, process)| {
                        Self::tab_ai_target_from_process_manager_row(
                            display_index,
                            source_index,
                            process,
                        )
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::CurrentAppCommandsView { selected_index, .. } => {
                let focused_target =
                    self.cached_current_app_entries
                        .get(*selected_index)
                        .map(|entry| crate::ai::TabAiTargetContext {
                            source: "CurrentAppCommands".to_string(),
                            kind: "menu_command".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice",
                                *selected_index,
                                &entry.name,
                            ),
                            label: entry.name.clone(),
                            metadata: Some(serde_json::json!({
                                "name": entry.name.clone(),
                            })),
                        });
                let visible_targets = self
                    .cached_current_app_entries
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| crate::ai::TabAiTargetContext {
                        source: "CurrentAppCommands".to_string(),
                        kind: "menu_command".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            index,
                            &entry.name,
                        ),
                        label: entry.name.clone(),
                        metadata: Some(serde_json::json!({
                            "name": entry.name.clone(),
                        })),
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::SdkReferenceView {
                filter,
                selected_index,
                entries,
            } => {
                let focused_target = crate::mcp_resources::sdk_reference_selected_visible_entry(
                    entries,
                    filter,
                    *selected_index,
                )
                .map(Self::tab_ai_target_from_sdk_reference_row);
                let visible_targets = crate::mcp_resources::sdk_reference_visible_target_rows(
                    entries,
                    filter,
                    TAB_AI_VISIBLE_TARGET_LIMIT,
                )
                .into_iter()
                .map(Self::tab_ai_target_from_sdk_reference_row)
                .collect();
                (focused_target, visible_targets)
            }
            AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                templates,
            } => {
                let focused_target =
                    crate::mcp_resources::script_template_catalog_selected_visible_template(
                        templates,
                        filter,
                        *selected_index,
                    )
                    .map(Self::tab_ai_target_from_script_template_catalog_row);
                let visible_targets =
                    crate::mcp_resources::script_template_catalog_visible_target_rows(
                        templates,
                        filter,
                        TAB_AI_VISIBLE_TARGET_LIMIT,
                    )
                    .into_iter()
                    .map(Self::tab_ai_target_from_script_template_catalog_row)
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::ScriptList => {
                // Resolve the focused script list item through the result-cache owner,
                // which maps selected_index -> flat result index -> SearchResult.
                let focused_target = self
                    .main_menu_result_caches
                    .search_result_for_grouped_item(self.selected_index)
                    .map(|result| {
                        Self::tab_ai_target_from_search_result(self.selected_index, result)
                    })
                    .or_else(|| {
                        let trimmed = self.filter_text.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(Self::tab_ai_target_from_search_query(
                                "ScriptList",
                                trimmed,
                                serde_json::json!({
                                    "query": trimmed,
                                    "visibleResultCount": self.main_menu_result_caches.grouped_flat_result_count(),
                                }),
                            ))
                        }
                    });

                let visible_targets: Vec<crate::ai::TabAiTargetContext> = self
                    .main_menu_result_caches
                    .grouped_search_results()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(display_index, result)| {
                        Self::tab_ai_target_from_search_result(display_index, result)
                    })
                    .collect();

                (focused_target, visible_targets)
            }
            _ => {
                let visible_targets: Vec<crate::ai::TabAiTargetContext> = ui
                    .visible_elements
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .map(|element| Self::tab_ai_target_from_element(&ui.prompt_type, element))
                    .collect();

                let focused_target = ui
                    .selected_semantic_id
                    .as_deref()
                    .or(ui.focused_semantic_id.as_deref())
                    .and_then(|semantic_id| {
                        visible_targets
                            .iter()
                            .find(|target| target.semantic_id == semantic_id)
                            .cloned()
                            .or_else(|| {
                                ui.visible_elements
                                    .iter()
                                    .find(|element| element.semantic_id == semantic_id)
                                    .map(|element| {
                                        Self::tab_ai_target_from_element(&ui.prompt_type, element)
                                    })
                            })
                    })
                    .or_else(|| {
                        ui.input_text
                            .as_deref()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                            .map(|text| Self::tab_ai_target_from_input_text(&ui.prompt_type, text))
                    });

                (focused_target, visible_targets)
            }
        }
    }

    /// Format visible file search results for AI context injection.
    fn format_file_search_ai_visible_results(
        &self,
        selected_display_index: Option<usize>,
        limit: usize,
    ) -> String {
        self.visible_file_search_results(limit)
            .into_iter()
            .map(|(display_index, entry)| {
                let marker = if Some(display_index) == selected_display_index {
                    "*"
                } else {
                    "-"
                };
                let kind = if entry.file_type == crate::file_search::FileType::Directory {
                    "directory"
                } else {
                    "file"
                };
                format!("{marker} [{}] {} ({kind})", display_index, entry.path)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build a query-level AI intent when no valid row is selected.
    /// Falls back to the current query and visible result set.
    pub(crate) fn build_file_search_ai_query_intent(
        &self,
        query: &str,
        plan_mode: bool,
    ) -> Option<String> {
        let query = query.trim();
        if query.is_empty() && self.file_search_display_indices.is_empty() {
            return None;
        }

        let nearby = self.format_file_search_ai_visible_results(None, 6);
        let nearby = if nearby.is_empty() {
            "- (no visible results yet)".to_string()
        } else {
            nearby
        };

        let presentation = match &self.current_view {
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Mini,
                ..
            } => "mini",
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Full,
                ..
            } => "full",
            _ => "unknown",
        };

        let query_mode = if query.is_empty() {
            "empty"
        } else {
            Self::file_search_query_mode(query)
        };

        Some(if plan_mode {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Query mode: {query_mode}\n\
                 Visible results:\n\
                 {nearby}\n\n\
                 Use the current search as the primary context.\n\
                 Propose a concrete next-step plan, including which files or directories to inspect next,\n\
                 how to refine the query, and how to verify the result."
            )
        } else {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Query mode: {query_mode}\n\
                 Visible results:\n\
                 {nearby}\n\n\
                 Use the current search as the primary context.\n\
                 Summarize what this search is likely showing, point out the most important pattern,\n\
                 and tell me the highest-leverage next search or file to inspect."
            )
        })
    }

    /// Build an entry intent string for launching the AI harness from file
    /// search.  Returns `None` when no file is selected.
    ///
    /// `plan_mode`:
    /// - `false` (⌘↵) — "explain this file/directory"
    /// - `true`  (⌘⇧↵) — "propose a plan using this selection + query"
    pub(crate) fn build_file_search_ai_entry_intent(
        &self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
    ) -> Option<String> {
        let (display_index, selected) = self.selected_file_search_result(selected_index)?;

        let subject = if selected.file_type == crate::file_search::FileType::Directory {
            "directory"
        } else {
            "file"
        };

        let nearby = self.format_file_search_ai_visible_results(Some(display_index), 6);

        let presentation = match &self.current_view {
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Mini,
                ..
            } => "mini",
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Full,
                ..
            } => "full",
            _ => "unknown",
        };

        Some(if plan_mode {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Selected {subject}: {}\n\
                 Nearby visible results:\n\
                 {nearby}\n\n\
                 Use the selected item as the primary target. Use nearby results only as supporting context.\n\
                 Propose a concrete plan, related files to inspect, and verification steps.",
                selected.path
            )
        } else {
            format!(
                "I selected this {subject} in Script Kit file search.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Selected {subject}: {}\n\
                 Nearby visible results:\n\
                 {nearby}\n\n\
                 Explain what it is, summarize why it matters, and tell me the highest-leverage next thing to inspect or change.",
                selected.path
            )
        })
    }

    /// Open the AI harness with the currently selected file-search result
    /// routed through the quick-submit plan path (richer harness hints).
    ///
    /// Returns `false` when there is no valid selection or intent, so the
    /// caller can fall through to default key handling.
    pub(crate) fn open_file_search_selection_in_tab_ai(
        &mut self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_display_index, selected)) = self.selected_file_search_result(selected_index)
        else {
            return false;
        };

        let Some(intent) = self.build_file_search_ai_entry_intent(query, selected_index, plan_mode)
        else {
            return false;
        };

        let plan = crate::ai::TabAiQuickSubmitPlan {
            source: crate::ai::TabAiQuickSubmitSource::FileSearch,
            kind: crate::ai::TabAiQuickSubmitKind::FileDrop,
            raw_query: selected.path.clone(),
            normalized_query: selected.path.clone(),
            synthesized_intent: intent,
            capture_kind: "defaultContext".to_string(),
            submit: true,
        };

        self.open_tab_ai_chat_with_quick_submit_plan(plan, cx);
        true
    }

    /// Open the AI harness from file search, falling back to a query-level
    /// intent when no valid row is selected.
    ///
    /// Returns `false` only when no useful context exists at all (empty query
    /// and no visible results), so `⌘↵` / `⌘⇧↵` is never a dead keypress.
    pub(crate) fn open_file_search_selection_or_query_in_tab_ai(
        &mut self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.open_file_search_selection_in_tab_ai(query, selected_index, plan_mode, cx) {
            return true;
        }

        let Some(intent) = self.build_file_search_ai_query_intent(query, plan_mode) else {
            return false;
        };

        self.open_tab_ai_chat_with_entry_intent(Some(intent), cx);
        true
    }

    /// Fast-path UI snapshot for `AppView::ScriptList`.
    ///
    /// Skips the expensive `collect_visible_elements()` tree walk because the
    /// downstream target resolution in `resolve_tab_ai_surface_targets_for_view`
    /// already uses `cached_grouped_items` / `cached_grouped_flat_results`.
    fn snapshot_tab_ai_ui_fast_for_script_list(
        &self,
    ) -> (
        crate::ai::TabAiUiSnapshot,
        crate::ai::TabAiInvocationReceipt,
    ) {
        let prompt_type = "ScriptList".to_string();
        let input_text = if self.filter_text.is_empty() {
            None
        } else {
            Some(self.filter_text.clone())
        };

        let focused_semantic_id = self
            .main_menu_result_caches
            .search_result_for_grouped_item(self.selected_index)
            .map(|result| {
                Self::tab_ai_target_from_search_result(self.selected_index, result).semantic_id
            });
        let selected_semantic_id = focused_semantic_id.clone();

        let snapshot = crate::ai::TabAiUiSnapshot {
            prompt_type: prompt_type.clone(),
            input_text: input_text.clone(),
            focused_semantic_id: focused_semantic_id.clone(),
            selected_semantic_id: selected_semantic_id.clone(),
            visible_elements: Vec::new(),
        };

        let receipt = crate::ai::TabAiInvocationReceipt::from_snapshot(
            &prompt_type,
            &input_text,
            &focused_semantic_id,
            &selected_semantic_id,
            0,
            &[],
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_snapshot_fast_script_list",
            has_input_text = snapshot.input_text.is_some(),
            has_focus_target = snapshot.focused_semantic_id.is_some(),
            selected_index = self.selected_index,
        );

        (snapshot, receipt)
    }

    /// Capture a snapshot of the current UI state for context assembly.
    ///
    /// Returns the snapshot and a machine-readable invocation receipt that
    /// identifies whether UI context was rich or degraded with explicit
    /// reason codes.
    #[allow(dead_code)]
    fn snapshot_tab_ai_ui(
        &self,
        cx: &Context<Self>,
    ) -> (
        crate::ai::TabAiUiSnapshot,
        crate::ai::TabAiInvocationReceipt,
    ) {
        let prompt_type = self.app_view_name();

        // Collect visible elements
        let outcome = self.collect_visible_elements(TAB_AI_VISIBLE_ELEMENT_LIMIT, cx);

        let input_text = self.current_input_text(cx);
        let focused_id = outcome.focused_semantic_id();
        let selected_id = outcome.selected_semantic_id();

        // Build the machine-readable invocation receipt
        let receipt = crate::ai::TabAiInvocationReceipt::from_snapshot(
            &prompt_type,
            &input_text,
            &focused_id,
            &selected_id,
            outcome.elements.len(),
            &outcome.warnings,
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_snapshot_captured",
            prompt_type = %prompt_type,
            input_status = %receipt.input_status,
            focus_status = %receipt.focus_status,
            elements_status = %receipt.elements_status,
            has_input_text = receipt.has_input_text,
            has_focus_target = receipt.has_focus_target,
            element_count = receipt.element_count,
            warning_count = receipt.warning_count,
            rich = receipt.rich,
            degradation_reasons = ?receipt.degradation_reasons,
            "tab ai snapshot captured"
        );

        let snapshot = crate::ai::TabAiUiSnapshot {
            prompt_type,
            input_text,
            focused_semantic_id: focused_id,
            selected_semantic_id: selected_id,
            visible_elements: outcome.elements,
        };

        (snapshot, receipt)
    }

    /// Return a human-readable name for the current `AppView` variant.
    pub(crate) fn app_view_name(&self) -> String {
        match &self.current_view {
            AppView::ScriptList => "ScriptList".to_string(),
            AppView::About { .. } => "About".to_string(),
            AppView::ArgPrompt { .. } => "ArgPrompt".to_string(),
            AppView::MiniPrompt { .. } => "MiniPrompt".to_string(),
            AppView::MicroPrompt { .. } => "MicroPrompt".to_string(),
            AppView::DivPrompt { .. } => "DivPrompt".to_string(),
            AppView::FormPrompt { .. } => "FormPrompt".to_string(),
            AppView::TermPrompt { .. } => "TermPrompt".to_string(),
            AppView::EditorPrompt { .. } => "EditorPrompt".to_string(),
            AppView::SelectPrompt { .. } => "SelectPrompt".to_string(),
            AppView::PathPrompt { .. } => "PathPrompt".to_string(),
            AppView::EnvPrompt { .. } => "EnvPrompt".to_string(),
            AppView::DropPrompt { .. } => "DropPrompt".to_string(),
            AppView::TemplatePrompt { .. } => "TemplatePrompt".to_string(),
            AppView::HotkeyPrompt { .. } => "HotkeyPrompt".to_string(),
            AppView::ChatPrompt { .. } => "ChatPrompt".to_string(),
            AppView::ClipboardHistoryView { .. } => "ClipboardHistory".to_string(),
            AppView::AppLauncherView { .. } => "AppLauncher".to_string(),
            AppView::WindowSwitcherView { .. } => "WindowSwitcher".to_string(),
            AppView::BrowserTabsView { .. } => "BrowserTabs".to_string(),
            AppView::FileSearchView { .. } => "FileSearch".to_string(),
            AppView::ProfileSearchView { .. } => "ProfileSearch".to_string(),
            AppView::ThemeChooserView { .. } => "ThemeChooser".to_string(),
            AppView::EmojiPickerView { .. } => "EmojiPicker".to_string(),
            AppView::WebcamView { .. } => "Webcam".to_string(),
            AppView::ScratchPadView { .. } => "ScratchPad".to_string(),
            AppView::QuickTerminalView { .. } => "QuickTerminal".to_string(),
            AppView::NamingPrompt { .. } => "NamingPrompt".to_string(),
            AppView::CreationFeedback { .. } => "CreationFeedback".to_string(),
            AppView::DesignGalleryView { .. } => "DesignGallery".to_string(),
            AppView::FooterGalleryView { .. } => "FooterGallery".to_string(),
            AppView::NonListStatesView { .. } => "NonListStates".to_string(),
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => "DesignExplorer".to_string(),
            AppView::ActionsDialog => "ActionsDialog".to_string(),
            AppView::BrowseKitsView { .. } => "BrowseKits".to_string(),
            AppView::MigrateV1View { .. } => "MigrateV1".to_string(),
            AppView::InstalledKitsView { .. } => "InstalledKits".to_string(),
            AppView::ProcessManagerView { .. } => "ProcessManager".to_string(),
            AppView::FlowUxView { .. } => "FlowUx".to_string(),
            AppView::SearchAiPresetsView { .. } => "SearchAiPresets".to_string(),
            AppView::CreateAiPresetView { .. } => "CreateAiPreset".to_string(),
            AppView::SettingsView { .. } => "Settings".to_string(),
            AppView::PermissionsWizardView { .. } => "PermissionsWizard".to_string(),
            AppView::FavoritesBrowseView { .. } => "FavoritesBrowse".to_string(),
            AppView::CurrentAppCommandsView { .. } => "CurrentAppCommands".to_string(),
            AppView::AgentChatHistoryView { .. } => "AgentChatHistoryView".to_string(),
            AppView::BrowserHistoryView { .. } => "BrowserHistoryView".to_string(),
            AppView::DictationHistoryView { .. } => "DictationHistoryView".to_string(),
            AppView::NotesBrowseView { .. } => "NotesBrowse".to_string(),
            AppView::AgentChatView { .. } => "AgentChatView".to_string(),
            AppView::DayPage { .. } => "DayPage".to_string(),
            AppView::ScriptIssuesView { .. } => "ScriptIssuesView".to_string(),
            AppView::SdkReferenceView { .. } => "SdkReferenceView".to_string(),
            AppView::ScriptTemplateCatalogView { .. } => "ScriptTemplateCatalogView".to_string(),
            AppView::ConfirmPrompt { .. } => "ConfirmPrompt".to_string(),
        }
    }

    /// Return the current input text from whichever view is active.
    ///
    /// Returns `Some(text)` when the view has user-editable text that is
    /// non-empty, `None` otherwise.  Entity-based prompts are read via
    /// `entity.read(cx)` so this method requires a context reference.
    #[allow(dead_code)]
    fn current_input_text(&self, cx: &Context<Self>) -> Option<String> {
        let non_empty = |s: String| if s.is_empty() { None } else { Some(s) };

        match &self.current_view {
            AppView::ScriptList => non_empty(self.filter_text.clone()),

            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => non_empty(self.arg_input.text().to_string()),

            AppView::ClipboardHistoryView { filter, .. }
            | AppView::AppLauncherView { filter, .. }
            | AppView::WindowSwitcherView { filter, .. }
            | AppView::BrowserTabsView { filter, .. }
            | AppView::ThemeChooserView { filter, .. }
            | AppView::ProfileSearchView { filter, .. }
            | AppView::EmojiPickerView { filter, .. }
            | AppView::ProcessManagerView { filter, .. }
            | AppView::FlowUxView { filter, .. }
            | AppView::SettingsView { filter, .. }
            | AppView::SearchAiPresetsView { filter, .. }
            | AppView::FavoritesBrowseView { filter, .. }
            | AppView::CurrentAppCommandsView { filter, .. }
            | AppView::DesignGalleryView { filter, .. }
            | AppView::FooterGalleryView { filter, .. }
            | AppView::AgentChatHistoryView { filter, .. }
            | AppView::BrowserHistoryView { filter, .. }
            | AppView::DictationHistoryView { filter, .. }
            | AppView::NotesBrowseView { filter, .. }
            | AppView::SdkReferenceView { filter, .. }
            | AppView::ScriptTemplateCatalogView { filter, .. }
            | AppView::MigrateV1View { filter, .. } => non_empty(filter.clone()),

            AppView::FileSearchView { query, .. } => non_empty(query.clone()),

            AppView::BrowseKitsView { query, .. } => non_empty(query.clone()),

            // --- Entity-based prompts ---
            AppView::EditorPrompt { entity, .. } => {
                entity.read_with(cx, |editor, app| non_empty(editor.content_from_app(app)))
            }
            AppView::ScratchPadView { entity, .. } => {
                entity.read_with(cx, |editor, app| non_empty(editor.content_from_app(app)))
            }
            AppView::DayPage { entity } => {
                non_empty(entity.read(cx).notes_editor.read(cx).content(cx))
            }
            AppView::ChatPrompt { entity, .. } => {
                non_empty(entity.read(cx).input.text().to_string())
            }
            AppView::PathPrompt { entity, .. } => {
                let p = entity.read(cx);
                // Prefer active filter text; fall back to current directory path
                non_empty(p.filter_text.clone()).or_else(|| non_empty(p.current_path.clone()))
            }
            AppView::EnvPrompt { entity, .. } => {
                let p = entity.read(cx);
                // Return the user-entered value (masked text is still useful
                // for "is something typed?" without revealing secrets)
                if p.secret {
                    // For secret fields, report presence but not content
                    let text = p.input_text();
                    if text.is_empty() {
                        None
                    } else {
                        Some("[secret]".to_string())
                    }
                } else {
                    non_empty(p.input_text().to_string())
                }
            }
            AppView::SelectPrompt { entity, .. } => non_empty(entity.read(cx).filter_text.clone()),
            AppView::NamingPrompt { entity, .. } => {
                non_empty(entity.read(cx).friendly_name.clone())
            }
            AppView::TemplatePrompt { entity, .. } => {
                let p = entity.read(cx);
                // Return the value of the currently focused template input
                p.values
                    .get(p.current_input)
                    .and_then(|v| non_empty(v.clone()))
            }
            AppView::CreateAiPresetView {
                name,
                system_prompt,
                model,
                active_field,
            } => {
                // Return whichever field is active
                match active_field {
                    0 => non_empty(name.clone()),
                    1 => non_empty(system_prompt.clone()),
                    2 => non_empty(model.clone()),
                    _ => non_empty(name.clone()),
                }
            }

            // --- Views with no meaningful user-editable text ---
            // DivPrompt: script-rendered HTML, no user input
            // FormPrompt: multi-field form — field values are in elements,
            //   not a single "input text" (use visible_elements instead)
            // TermPrompt/QuickTerminal: terminal content, not user text input
            // DropPrompt: file drop zone, no typed text
            // WebcamView: camera feed, no text
            // CreationFeedback: read-only confirmation
            // ActionsDialog: transient overlay, not a primary surface
            // InstalledKitsView: navigation-only, no free text
            AppView::DivPrompt { .. }
            | AppView::About { .. }
            | AppView::FormPrompt { .. }
            | AppView::TermPrompt { .. }
            | AppView::QuickTerminalView { .. }
            | AppView::AgentChatView { .. }
            | AppView::DropPrompt { .. }
            | AppView::HotkeyPrompt { .. }
            | AppView::WebcamView { .. }
            | AppView::CreationFeedback { .. }
            | AppView::ScriptIssuesView { .. }
            | AppView::ActionsDialog
            | AppView::InstalledKitsView { .. }
            | AppView::NonListStatesView { .. }
            | AppView::PermissionsWizardView { .. }
            | AppView::ConfirmPrompt { .. } => None,

            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => None,
        }
    }

    /// Complete the pending Tab AI execution after the script actually exits.
    ///
    /// Gates memory write-back, save-offer, and temp-file cleanup on real
    /// completion status — never at dispatch time.
    ///
    /// Called from the prompt-handler `ScriptExit` / `ScriptError` paths
    /// once the ephemeral process terminates. Uses `take()` on the pending
    /// record so only the first caller does work — subsequent calls are no-ops.
    pub(crate) fn complete_tab_ai_execution(
        &mut self,
        success: bool,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.pending_tab_ai_execution.take() else {
            return;
        };

        let cleanup_attempted = true;
        let cleanup_succeeded = crate::ai::cleanup_tab_ai_temp_script(&record.temp_script_path);

        let status = if success {
            crate::ai::TabAiExecutionStatus::Succeeded
        } else {
            crate::ai::TabAiExecutionStatus::Failed
        };

        let receipt = crate::ai::build_tab_ai_execution_receipt(
            &record,
            status,
            cleanup_attempted,
            cleanup_succeeded,
            error.clone(),
        );

        if let Err(audit_error) = crate::ai::append_tab_ai_execution_receipt(&receipt) {
            tracing::warn!(
                event = "tab_ai_execution_audit_write_failed",
                error = %audit_error,
            );
        }

        if success {
            if let Err(memory_error) = crate::ai::write_tab_ai_memory_entry(&record) {
                tracing::warn!(
                    event = "tab_ai_memory_writeback_failed",
                    error = %memory_error,
                );
            }

            if crate::ai::should_offer_save(&record) {
                tracing::info!(
                    event = "tab_ai_save_offer_open",
                    slug = %record.slug,
                    prompt_type = %record.prompt_type,
                );
                self.open_tab_ai_save_offer(record, cx);
            }
        } else {
            let message = error.unwrap_or_else(|| "Tab AI script failed".to_string());
            self.toast_manager.push(
                components::toast::Toast::error(message, &self.theme)
                    .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
        }
    }
    // ── Tab AI save-offer overlay ──────────────────────────────────────

    fn tab_ai_default_save_name(record: &crate::ai::TabAiExecutionRecord) -> String {
        let derived = super::prompt_ai::derive_script_name_from_description(&record.intent);
        if derived == "ai-generated-script" || derived.is_empty() {
            record.slug.clone()
        } else {
            derived
        }
    }

    fn open_tab_ai_save_offer(
        &mut self,
        record: crate::ai::TabAiExecutionRecord,
        cx: &mut Context<Self>,
    ) {
        let filename_stem = Self::tab_ai_default_save_name(&record);
        tracing::info!(
            event = "tab_ai_save_offer_state_set",
            filename_stem = %filename_stem,
        );
        self.tab_ai_save_offer_state = Some(TabAiSaveOfferState {
            record,
            filename_stem,
            error: None,
        });
        cx.notify();
    }

    fn close_tab_ai_save_offer(&mut self, cx: &mut Context<Self>) {
        if self.tab_ai_save_offer_state.take().is_some() {
            tracing::info!(event = "tab_ai_save_offer_dismissed");
            self.pending_focus = Some(self.tab_ai_return_focus_target());
            cx.notify();
        }
    }

    fn save_tab_ai_script(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.tab_ai_save_offer_state.clone() else {
            return;
        };

        // Route through the contract pipeline (extraction, convention
        // enforcement, concurrent-prompt-API rejection, receipt, background
        // Bun verification) instead of a bare create + write — a generated
        // script that the tested generator would reject must not land on
        // disk unflagged. The user-chosen filename stem still wins.
        let prompt = if state.record.intent.trim().is_empty() {
            state.filename_stem.as_str()
        } else {
            state.record.intent.as_str()
        };
        let created_path =
            match crate::ai::script_generation::save_generated_script_from_response_with_slug(
                prompt,
                &state.record.generated_source,
                Some(&state.filename_stem),
            ) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        event = "tab_ai_save_create_failed",
                        error = %error,
                        filename_stem = %state.filename_stem,
                    );
                    if let Some(save_state) = &mut self.tab_ai_save_offer_state {
                        save_state.error = Some(format!("Failed to save script: {error}").into());
                    }
                    cx.notify();
                    return;
                }
            };

        tracing::info!(
            event = "tab_ai_script_saved",
            filename_stem = %state.filename_stem,
            path = %created_path.display(),
        );

        let created_file_path = if created_path.is_absolute() {
            created_path.clone()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(&created_path),
                Err(_) => created_path.clone(),
            }
        };

        let editor_error =
            crate::script_creation::open_in_editor(&created_path, &self.config).err();

        self.tab_ai_save_offer_state = None;

        match editor_error {
            Some(error) => {
                tracing::warn!(
                    event = "tab_ai_save_editor_open_failed",
                    error = %error,
                );
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Saved script but failed to open editor: {error}"),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
            }
            None => {
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Saved '{}' and opened in editor", state.filename_stem),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_SUCCESS_MS)),
                );
            }
        }

        self.open_creation_feedback_payload(
            prompts::CreationFeedbackPayload::generated_script(created_file_path),
            cx,
        );
    }

    fn handle_tab_ai_save_offer_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        if crate::ui_foundation::is_key_escape(key) {
            self.close_tab_ai_save_offer(cx);
            cx.stop_propagation();
            return;
        }

        if crate::ui_foundation::is_key_enter(key) {
            self.save_tab_ai_script(cx);
            cx.stop_propagation();
            return;
        }

        cx.propagate();
    }

    pub(crate) fn render_tab_ai_save_offer_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let state = self.tab_ai_save_offer_state.as_ref()?;
        let theme = crate::theme::get_cached_theme();

        // Ensure the main focus handle is focused so key events route here
        if !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }

        // Whisper chrome colors — same tokens as the main Tab AI overlay
        let bg_scrim = gpui::rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            theme.colors.background.main,
            crate::theme::opacity::OPACITY_NEAR_FULL,
        ));
        let text_primary = gpui::rgb(theme.colors.text.primary);
        let error_color = gpui::rgb(theme.colors.ui.error);
        let divider_rgba = gpui::rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            theme.colors.text.primary,
            crate::theme::opacity::OPACITY_GHOST,
        ));

        let hint_px: f32 = crate::window_resize::main_layout::HINT_STRIP_PADDING_X;

        let message: SharedString = format!("Save as {}.ts?", state.filename_stem).into();

        // Full-width inline panel — matches main Tab AI overlay chrome,
        // not a floating card. Footer uses HintStrip with save-specific
        // hints (justified exception: this is a confirmation dialog, not
        // the primary input surface).
        let overlay = div()
            .id("tab-ai-save-offer")
            .absolute()
            .inset_0()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(bg_scrim)
            // Message row — bare text, no card, no accent bar
            .child(
                div().w_full().px(px(hint_px)).py(px(10.)).child(
                    div()
                        .text_sm()
                        .font_family(crate::list_item::FONT_MONO)
                        .text_color(text_primary)
                        .child(message),
                ),
            )
            // Hairline divider — ghost opacity
            .child(div().w_full().h(px(1.)).bg(divider_rgba))
            // Error message if present — below divider, minimal
            .when_some(state.error.clone(), |d, msg| {
                d.child(
                    div()
                        .w_full()
                        .px(px(hint_px))
                        .py(px(4.))
                        .text_xs()
                        .text_color(error_color)
                        .child(msg),
                )
            })
            // Spacer pushes footer to bottom
            .child(div().flex_1())
            // Footer — save-specific hint strip via shared component
            // (justified exception: confirmation dialog uses ↵ Save / Esc Dismiss
            // instead of the canonical three-key strip)
            .child(components::HintStrip::new(vec![
                "\u{21B5} Save".into(),
                "Esc Dismiss".into(),
            ]))
            .on_key_down(cx.listener(Self::handle_tab_ai_save_offer_key_down));

        Some(overlay.into_any_element())
    }
}

// ---------------------------------------------------------------------------
// Apply-back: clipboard helpers
// ---------------------------------------------------------------------------

fn read_tab_ai_apply_back_clipboard_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    let text = clipboard
        .get_text()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_read_failed: {error}"))?;
    if text.trim().is_empty() {
        return Err("tab_ai_apply_back_clipboard_empty".to_string());
    }
    Ok(text)
}

fn write_tab_ai_apply_back_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| format!("tab_ai_apply_back_clipboard_write_failed: {error}"))
}

// ---------------------------------------------------------------------------
// Apply-back: entry point (⌘⏎ in QuickTerminalView)
// ---------------------------------------------------------------------------

/// Route-aware success message for the apply-back toast.
fn tab_ai_apply_back_success_message(source_type: &crate::ai::TabAiSourceType) -> &'static str {
    match source_type {
        crate::ai::TabAiSourceType::RunningCommand => "Applied result to the active prompt",
        crate::ai::TabAiSourceType::ClipboardEntry => "Copied result to the clipboard",
        crate::ai::TabAiSourceType::ScriptListItem => "Saved and ran the generated script",
        crate::ai::TabAiSourceType::DesktopSelection => "Replaced the frontmost selection",
        crate::ai::TabAiSourceType::Desktop => "Pasted into the frontmost app",
    }
}

impl ScriptListApp {
    const TAB_AI_APPLY_BACK_FOCUS_SETTLE_MS: u64 = 250;
    const TAB_AI_APPLY_BACK_CLIPBOARD_PRIME_MS: u64 = 25;
    const TAB_AI_APPLY_BACK_ROUTE_POLL_MS: u64 = 20;
    const TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS: u64 = 750;

    /// Show a route-aware error toast when ⌘↩ is pressed but there is
    /// neither a terminal selection nor harness output available yet.
    fn toast_tab_ai_apply_back_unavailable(&mut self, cx: &mut Context<Self>) {
        let apply_label = crate::ai::tab_ai_apply_back_footer_label(
            self.tab_ai_harness_apply_back_route
                .as_ref()
                .map(|route| &route.source_type),
        );
        self.toast_manager.push(
            crate::components::toast::Toast::error(
                format!("{apply_label} failed: select terminal text or wait for output."),
                &self.theme,
            )
            .duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
    }

    /// Show a route-aware error toast when the apply-back route is still
    /// unavailable after the bounded wait expires.
    fn toast_tab_ai_apply_back_pending(&mut self, cx: &mut Context<Self>) {
        let message = match self.tab_ai_harness_apply_back_route.as_ref() {
            Some(route) => format!(
                "{} is still preparing. Try again in a moment.",
                crate::ai::tab_ai_apply_back_footer_label(Some(&route.source_type)),
            ),
            None => "Paste Back target is still preparing. Try again in a moment.".to_string(),
        };
        self.toast_manager.push(
            crate::components::toast::Toast::error(message, &self.theme)
                .duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
    }

    /// Unified apply handler — routes `text` to the correct destination
    /// based on `route.source_type`.  Called by both the terminal-selection
    /// fast path and the clipboard fallback.
    fn apply_tab_ai_result_text(
        &mut self,
        route: crate::ai::TabAiApplyBackRoute,
        text: String,
        cx: &mut Context<Self>,
    ) {
        if text.trim().is_empty() {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "No terminal selection or harness output was available".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
            return;
        }

        match route.source_type.clone() {
            crate::ai::TabAiSourceType::RunningCommand => {
                self.close_tab_ai_harness_terminal(cx);
                if self.try_set_prompt_input(text.clone(), cx) {
                    self.toast_manager.push(
                        crate::components::toast::Toast::success(
                            tab_ai_apply_back_success_message(&route.source_type).to_string(),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                } else {
                    self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            "The original prompt is no longer active".to_string(),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                }
                cx.notify();
            }
            crate::ai::TabAiSourceType::ClipboardEntry => {
                self.close_tab_ai_harness_terminal(cx);
                match write_tab_ai_apply_back_clipboard_text(&text) {
                    Ok(()) => {
                        self.toast_manager.push(
                            crate::components::toast::Toast::success(
                                tab_ai_apply_back_success_message(&route.source_type).to_string(),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_SUCCESS_MS)),
                        );
                    }
                    Err(error) => {
                        self.toast_manager.push(
                            crate::components::toast::Toast::error(
                                format!("Failed to update clipboard: {error}"),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                    }
                }
                cx.notify();
            }
            crate::ai::TabAiSourceType::ScriptListItem => {
                self.close_tab_ai_harness_terminal(cx);

                // Use the focused target label as the prompt for slug derivation.
                let prompt_label = route
                    .focused_target
                    .as_ref()
                    .map(|t| t.label.clone())
                    .unwrap_or_else(|| "ai generated script".to_string());

                match crate::ai::script_generation::save_generated_script_from_response(
                    &prompt_label,
                    &text,
                ) {
                    Ok(script_path) => {
                        let path_str = script_path.to_string_lossy().to_string();
                        tracing::info!(
                            target: "tab_ai",
                            source_type = "ScriptListItem",
                            script_path = %path_str,
                            "tab_ai_apply_back.script_saved"
                        );
                        self.toast_manager.push(
                            crate::components::toast::Toast::success(
                                format!(
                                    "Saved and running generated script: {}",
                                    script_path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("script"),
                                ),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_SUCCESS_MS)),
                        );
                        self.execute_script_by_path(&path_str, cx);
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "tab_ai",
                            error = %error,
                            "tab_ai_apply_back.script_save_failed"
                        );
                        self.toast_manager.push(
                            crate::components::toast::Toast::error(
                                format!("Failed to save generated script: {error}"),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                    }
                }
                cx.notify();
            }
            /* crate::ai::TabAiSourceType::DesktopSelection
            | crate::ai::TabAiSourceType::Desktop => */
            crate::ai::TabAiSourceType::DesktopSelection | crate::ai::TabAiSourceType::Desktop => {
                // Desktop selection / generic desktop: hide the main window first,
                // wait for focus to settle back to the previous frontmost app,
                // then apply via set_selected_text or TextInjector::paste_text.
                self.close_tab_ai_harness_terminal(cx);
                crate::platform::defer_hide_main_window(cx);

                let app_weak = cx.entity().downgrade();
                cx.spawn(async move |_this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(
                            Self::TAB_AI_APPLY_BACK_FOCUS_SETTLE_MS,
                        ))
                        .await;

                    let route_for_apply = route.clone();
                    let route_for_toast = route.clone();
                    let text_for_apply = text.clone();

                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            match route_for_apply.source_type {
                                crate::ai::TabAiSourceType::DesktopSelection => {
                                    selected_text::set_selected_text(&text_for_apply)
                                        .map_err(|error| error.to_string())
                                }
                                crate::ai::TabAiSourceType::Desktop => {
                                    let injector = text_injector::TextInjector::new();
                                    injector
                                        .paste_text(&text_for_apply)
                                        .map_err(|error| error.to_string())
                                }
                                _ => Ok(()),
                            }
                        })
                        .await;

                    let _ = cx.update(|cx| {
                        let Some(app) = app_weak.upgrade() else {
                            return;
                        };
                        app.update(cx, |this, cx| {
                            match result {
                                Ok(()) => {
                                    this.toast_manager.push(
                                        crate::components::toast::Toast::success(
                                            tab_ai_apply_back_success_message(
                                                &route_for_toast.source_type,
                                            )
                                            .to_string(),
                                            &this.theme,
                                        )
                                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                                    );
                                }
                                Err(error) => {
                                    this.toast_manager.push(
                                        crate::components::toast::Toast::error(
                                            format!("Failed to apply result: {error}"),
                                            &this.theme,
                                        )
                                        .duration_ms(Some(TOAST_ERROR_MS)),
                                    );
                                }
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            }
        }
    }

    /// Apply `text` immediately when the route is known; otherwise poll
    /// for up to `TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS` ms.  If the route
    /// is still unavailable after the deadline, show a route-aware error
    /// toast instead of waiting forever.  Cancels silently if the harness
    /// closes (view leaves `QuickTerminalView`) or the entity is dropped.
    fn apply_tab_ai_result_text_or_wait_for_route(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(route) = self.tab_ai_harness_apply_back_route.clone() {
            self.apply_tab_ai_result_text(route, text, cx);
            return;
        }

        let app_weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(
                    ScriptListApp::TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS,
                );

            loop {
                enum WaitState {
                    Ready(crate::ai::TabAiApplyBackRoute),
                    Pending,
                    TimedOut,
                    Cancelled,
                }

                let state = cx.update(|cx| {
                    let Some(app) = app_weak.upgrade() else {
                        return WaitState::Cancelled;
                    };
                    app.update(cx, |this, _cx| {
                        if !matches!(this.current_view, AppView::QuickTerminalView { .. }) {
                            return WaitState::Cancelled;
                        }
                        if let Some(route) = this.tab_ai_harness_apply_back_route.clone() {
                            return WaitState::Ready(route);
                        }
                        if std::time::Instant::now() >= deadline {
                            return WaitState::TimedOut;
                        }
                        WaitState::Pending
                    })
                });

                match state {
                    WaitState::Ready(route) => {
                        let _ = cx.update(|cx| {
                            let Some(app) = app_weak.upgrade() else {
                                return;
                            };
                            app.update(cx, |this, cx| {
                                this.apply_tab_ai_result_text(route, text.clone(), cx);
                            });
                        });
                        break;
                    }
                    WaitState::TimedOut => {
                        let _ = cx.update(|cx| {
                            let Some(app) = app_weak.upgrade() else {
                                return;
                            };
                            app.update(cx, |this, cx| {
                                this.toast_tab_ai_apply_back_pending(cx);
                            });
                        });
                        break;
                    }
                    WaitState::Cancelled => break,
                    WaitState::Pending => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(
                                ScriptListApp::TAB_AI_APPLY_BACK_ROUTE_POLL_MS,
                            ))
                            .await;
                    }
                }
            }
        })
        .detach();
    }

    /// Apply harness output from the terminal.  Prefers the terminal selection
    /// directly (no clipboard round-trip); falls back to clipboard priming
    /// only when no selection exists.
    #[allow(dead_code)] // Called from include!() binary code (render_prompts/term.rs)
    pub(crate) fn apply_tab_ai_result_from_terminal(
        &mut self,
        entity: Entity<term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) {
        // Try to read the terminal selection directly — avoids the
        // clipboard prime → timer → read race entirely.
        let selected_text =
            entity.update(cx, |term_prompt, _cx| term_prompt.selected_text_for_apply());

        if let Some(text) = selected_text {
            self.apply_tab_ai_result_text_or_wait_for_route(text, cx);
            return;
        }

        // No selection — fall back to clipboard priming (copies last output).
        entity.update(cx, |term_prompt, cx| {
            term_prompt.prime_apply_clipboard(cx);
        });

        let app = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    Self::TAB_AI_APPLY_BACK_CLIPBOARD_PRIME_MS,
                ))
                .await;
            let _ = cx.update(|cx| {
                let Some(app) = app.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.apply_tab_ai_result_from_clipboard(cx);
                });
            });
        })
        .detach();
    }

    pub(crate) fn apply_tab_ai_result_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let text = match read_tab_ai_apply_back_clipboard_text() {
            Ok(text) => text,
            Err(_error) => {
                self.toast_tab_ai_apply_back_unavailable(cx);
                return;
            }
        };

        self.apply_tab_ai_result_text_or_wait_for_route(text, cx);
    }
}

#[cfg(test)]
mod tests {
    use crate::{AppView, ScriptListApp};

    fn selection_resource_part() -> crate::ai::message_parts::AiContextPart {
        crate::ai::context_contract::ContextAttachmentKind::Selection.part()
    }

    fn text_block_part(label: &str) -> crate::ai::message_parts::AiContextPart {
        crate::ai::message_parts::AiContextPart::TextBlock {
            label: label.to_string(),
            source: format!("test:{label}"),
            text: format!("{label} body"),
            mime_type: None,
        }
    }

    #[test]
    fn day_page_submit_plan_uses_explicit_aliases_not_stale_app_aliases() {
        let parse = crate::spine::parse_spine("@clipboard:Latest summarize");
        let mut app_aliases = std::collections::HashMap::new();
        app_aliases.insert(
            "@clipboard:Latest".to_string(),
            text_block_part("stale app clipboard"),
        );
        let day_page_aliases = std::collections::HashMap::new();

        let stale_app_plan = ScriptListApp::spine_prompt_plan_for_aliases(&parse, &app_aliases);
        let day_page_plan =
            ScriptListApp::spine_prompt_plan_for_aliases(&parse, &day_page_aliases);

        assert_eq!(
            stale_app_plan.context_parts.len(),
            1,
            "control: app alias map would resolve the compact token"
        );
        assert!(
            day_page_plan.context_parts.is_empty(),
            "Day Page submission must not attach stale app aliases after local alias reset"
        );
        assert_eq!(
            day_page_plan.unknown_warnings.len(),
            1,
            "unbacked compact Day Page token should remain an explicit warning"
        );
    }

    fn captured(text: &str, source_app: Option<&str>) -> crate::selected_text::CapturedText {
        crate::selected_text::CapturedText {
            text: text.to_string(),
            kind: crate::selected_text::CapturedTextKind::Selection,
            source_app: source_app.map(str::to_string),
        }
    }

    #[test]
    fn materialize_selection_swaps_resource_for_captured_text() {
        let parts = super::materialize_selection_context_parts(
            vec![selection_resource_part()],
            None,
            || Some(captured("captured live", Some("Safari"))),
        );
        match &parts[0] {
            crate::ai::message_parts::AiContextPart::TextBlock {
                text,
                source,
                label,
                ..
            } => {
                assert_eq!(text, "captured live");
                assert!(
                    source.contains("#selection="),
                    "source must key the @selected inline token, got {source}"
                );
                assert_eq!(
                    label, "Selection \u{2014} Safari",
                    "label must carry provenance into the transcript receipt"
                );
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
    }

    #[test]
    fn materialize_selection_falls_back_to_live_preview_cache() {
        let parts = super::materialize_selection_context_parts(
            vec![selection_resource_part()],
            Some(captured("cached preview", None)),
            || None,
        );
        match &parts[0] {
            crate::ai::message_parts::AiContextPart::TextBlock { text, .. } => {
                assert_eq!(text, "cached preview");
            }
            other => panic!("expected TextBlock, got {other:?}"),
        }
    }

    #[test]
    fn materialize_selection_keeps_lazy_part_when_nothing_captured() {
        let parts = super::materialize_selection_context_parts(
            vec![selection_resource_part()],
            Some(captured("   ", None)),
            || Some(captured("  ", None)),
        );
        assert!(matches!(
            &parts[0],
            crate::ai::message_parts::AiContextPart::ResourceUri { .. }
        ));
    }

    #[test]
    fn materialize_selection_never_captures_without_selection_part() {
        let clipboard_part = crate::ai::context_contract::ContextAttachmentKind::Clipboard.part();
        let parts =
            super::materialize_selection_context_parts(vec![clipboard_part.clone()], None, || {
                panic!("capture must not run when no @selection part is present")
            });
        assert_eq!(parts, vec![clipboard_part]);
    }

    #[test]
    fn tab_ai_user_prompt_contains_intent_and_context() {
        let prompt = crate::ai::build_tab_ai_user_prompt("force quit", r#"{"ui":{}}"#);
        assert!(prompt.contains("force quit"));
        assert!(prompt.contains(r#"{"ui":{}}"#));
        assert!(prompt.contains("Script Kit TypeScript"));
    }

    #[test]
    fn tab_ai_user_prompt_contains_code_block_instruction() {
        let prompt = crate::ai::build_tab_ai_user_prompt("test intent", "{}");
        assert!(
            prompt.contains("fenced code block"),
            "Prompt must ask for a fenced code block so extract_generated_script_source works"
        );
    }

    #[test]
    fn tab_ai_user_prompt_separates_intent_from_context() {
        let prompt = crate::ai::build_tab_ai_user_prompt("copy url", r#"{"schemaVersion":1}"#);
        // The intent appears before the context
        let intent_pos = prompt.find("copy url").expect("intent present");
        let context_pos = prompt.find("schemaVersion").expect("context present");
        assert!(
            intent_pos < context_pos,
            "Intent should appear before context JSON"
        );
    }

    #[test]
    fn tab_ai_user_prompt_with_rich_context_json() {
        let context = serde_json::to_string_pretty(&crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("slack".to_string()),
                focused_semantic_id: Some("input:filter".to_string()),
                selected_semantic_id: Some("choice:0:slack".to_string()),
                visible_elements: vec![],
            },
            Default::default(),
            vec!["recent1".to_string()],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        ))
        .expect("serialize");

        let prompt = crate::ai::build_tab_ai_user_prompt("force quit this app", &context);

        assert!(prompt.contains("force quit this app"));
        assert!(prompt.contains("ScriptList"));
        assert!(prompt.contains("slack"));
        assert!(prompt.contains("choice:0:slack"));
        assert!(prompt.contains("recent1"));
    }

    #[test]
    fn tab_ai_chat_uses_three_key_footer_contract() {
        const TAB_AI_SOURCE: &str = include_str!("mod.rs");
        assert!(
            TAB_AI_SOURCE.contains(r#""\u{21B5} Send"#),
            "tab ai chat should expose the Send hint"
        );
        assert!(
            TAB_AI_SOURCE.contains(r#""\u{2318}K Actions"#),
            "tab ai chat should expose the Actions hint"
        );
        assert!(
            TAB_AI_SOURCE.contains(r#""Esc Back"#),
            "tab ai chat should expose the Esc Back hint"
        );
    }

    #[test]
    fn tab_ai_overlay_preserves_memory_hint_rendering() {
        const TAB_AI_SOURCE: &str = include_str!("mod.rs");
        assert!(
            TAB_AI_SOURCE.contains("Similar prior automation:"),
            "visual cleanup must not silently remove memory-hint behavior"
        );
    }

    #[test]
    fn tab_ai_overlay_uses_named_opacity_constants() {
        const TAB_AI_SOURCE: &str = include_str!("mod.rs");
        // The render function should reference OPACITY_GHOST, not raw 0.06
        assert!(
            TAB_AI_SOURCE.contains("OPACITY_GHOST"),
            "tab ai overlay should use named ghost opacity constant"
        );
    }

    #[test]
    fn tab_ai_overlay_uses_shared_hint_strip_component() {
        const TAB_AI_SOURCE: &str = include_str!("mod.rs");
        assert!(
            TAB_AI_SOURCE.contains("HintStrip::new"),
            "tab ai overlay should use the shared HintStrip component"
        );
    }

    // ── Source-type detection tests ──────────────────────────────────

    #[test]
    fn desktop_selection_beats_internal_surface_classification() {
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        // Even when the source view is ScriptList with a focused target,
        // desktop selected text takes precedence.
        let focused_target = crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, Some(&focused_target),),
            Some(crate::ai::TabAiSourceType::DesktopSelection),
            "Desktop selected text must take precedence over ScriptList classification"
        );
    }

    #[test]
    fn script_list_requires_real_focused_target() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();

        // ScriptList without a focused target falls back to Desktop
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, None),
            Some(crate::ai::TabAiSourceType::Desktop),
            "ScriptList without focused target must fall back to Desktop"
        );

        // ScriptList WITH a focused target resolves to ScriptListItem
        let focused_target = crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, Some(&focused_target),),
            Some(crate::ai::TabAiSourceType::ScriptListItem),
            "ScriptList with focused target must resolve to ScriptListItem"
        );
    }

    #[test]
    fn desktop_selection_whitespace_only_does_not_count() {
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("   \n\t  ".to_string()),
            ..Default::default()
        };
        // Whitespace-only selection should NOT trigger DesktopSelection
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, None),
            Some(crate::ai::TabAiSourceType::Desktop),
            "Whitespace-only selected text must not trigger DesktopSelection"
        );
    }

    #[test]
    fn source_type_computed_after_context_resolution() {
        // Structural contract: sourceType is computed after build_tab_ai_context_from
        // so it can inspect the resolved focused_target.
        const SRC: &str = include_str!("mod.rs");

        let build_idx = SRC
            .find("let resolved = this.build_tab_ai_context_from(")
            .expect("build_tab_ai_context_from call");
        let detect_idx = SRC
            .find("let source_type = detect_tab_ai_source_type(")
            .expect("detect_tab_ai_source_type call");

        assert!(
            build_idx < detect_idx,
            "sourceType must be computed AFTER build_tab_ai_context_from so it can inspect resolved targets"
        );
    }

    #[test]
    fn detect_source_type_passes_resolved_focused_target() {
        // Structural contract: detect_tab_ai_source_type receives focused_target from resolved context
        const SRC: &str = include_str!("mod.rs");
        assert!(
            SRC.contains("resolved.context.focused_target.as_ref()"),
            "detect_tab_ai_source_type must receive focused_target from the resolved context"
        );
    }

    fn tab_ai_contract_compact(input: &str) -> String {
        input.split_whitespace().collect::<String>()
    }

    fn tab_ai_extract_fn_body(source: &str, signature: &str) -> String {
        let start = source.find(signature).expect("signature must exist");
        let rest = &source[start..];
        let open = rest.find('{').expect("function body must open");
        let mut depth = 0usize;
        let mut end = None;
        for (idx, ch) in rest[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + idx + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        rest[..end.expect("function body must close")].to_string()
    }

    #[test]
    fn tab_ai_open_path_switches_view_before_waiting_for_capture_contract() {
        let source = include_str!("mod.rs");
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn open_tab_ai_harness_terminal_from_request(",
        ));

        let view_switch = body
            .find(&tab_ai_contract_compact(
                "self.current_view = AppView::QuickTerminalView",
            ))
            .expect("QuickTerminalView switch must exist");
        let notify = body
            .find(&tab_ai_contract_compact("cx.notify();"))
            .expect("cx.notify must exist");
        let capture_wait = body
            .find(&tab_ai_contract_compact("capture_rx.recv().await"))
            .expect("deferred capture await must exist");

        assert!(
            view_switch < notify,
            "the harness view must be selected before notifying the UI"
        );
        assert!(
            notify < capture_wait,
            "the terminal must become visible before waiting for deferred capture"
        );
    }

    #[test]
    fn tab_ai_startup_prewarm_is_marked_fresh_on_cold_start_contract() {
        let source = include_str!("mod.rs");
        // The shared silent helper is where cold-start tagging lives.
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn warm_tab_ai_harness_silently(",
        ));

        assert!(
            body.contains(&tab_ai_contract_compact("if was_cold_start {")),
            "silent prewarm helper must gate FreshPrewarm tagging on a newly created session"
        );
        assert!(
            body.contains(&tab_ai_contract_compact("session.mark_fresh_prewarm();")),
            "cold-started prewarm must be marked reusable once"
        );
    }

    #[test]
    fn tab_ai_close_path_reseeds_future_prewarm_contract() {
        let source = include_str!("mod.rs");
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn close_tab_ai_harness_terminal_impl(",
        ));

        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.terminate_tab_ai_harness_session(cx);"
            )),
            "close path must delegate PTY session teardown"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
            )),
            "close path must schedule a fresh prewarm for the next Tab press"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.clear_transient_script_list_trigger_on_return(window, cx);"
            )),
            "close path must clear transient ScriptList trigger filters when returning to the main menu"
        );
    }

    #[test]
    fn script_list_explicit_triggers_do_not_stage_focused_parts() {
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('@'),
            false,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('/'),
            false,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            false,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: 0,
            },
            Some('@'),
            false,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            true,
        ));
    }

    #[test]
    fn agent_chat_initial_input_prefills_script_list_triggers_without_intent() {
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ScriptList",
                None,
                Some('@'),
                false,
            )
            .as_deref(),
            Some("@")
        );
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ScriptList",
                None,
                Some('/'),
                false,
            )
            .as_deref(),
            Some("/")
        );
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ScriptList",
                None,
                Some('|'),
                false,
            )
            .as_deref(),
            Some("|")
        );
    }

    #[test]
    fn agent_chat_initial_input_does_not_prefill_non_script_list_triggers() {
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ThemeChooser",
                None,
                Some('@'),
                false,
            ),
            None
        );
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ScriptList",
                None,
                Some('>'),
                false,
            ),
            None
        );
    }

    #[test]
    fn agent_chat_initial_input_prefers_effective_intent_over_script_list_trigger() {
        assert_eq!(
            ScriptListApp::tab_ai_agent_chat_initial_input_for_launch(
                "ScriptList",
                Some("explain this code"),
                Some('@'),
                true,
            )
            .as_deref(),
            Some("explain this code")
        );
    }

    #[test]
    fn embedded_agent_chat_reuse_requires_entry_intent_no_retry_and_non_setup_cache() {
        assert!(
            !ScriptListApp::should_reuse_embedded_agent_chat_view_for_open(None, false, false,)
        );
        assert!(
            ScriptListApp::should_reuse_embedded_agent_chat_view_for_open(
                Some("explain this"),
                false,
                false,
            )
        );
        assert!(
            !ScriptListApp::should_reuse_embedded_agent_chat_view_for_open(
                Some("switch agent"),
                true,
                false,
            )
        );
        assert!(
            !ScriptListApp::should_reuse_embedded_agent_chat_view_for_open(
                Some("explain this"),
                false,
                true,
            )
        );
    }

    #[test]
    fn embedded_agent_chat_reuse_submits_entry_intent_via_reuse_reset_helper() {
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            include_str!("mod.rs"),
            "fn try_reuse_embedded_agent_chat_view(",
        ));
        assert!(
            body.contains(&tab_ai_contract_compact(
                "chat.submit_reused_entry_intent(intent.clone(), cx);",
            )),
            "reused Agent Chat entry intents must clear stale composer state before submit"
        );
    }

    #[test]
    fn entry_intent_does_not_reuse_cached_setup_mode_agent_chat_view() {
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            include_str!("mod.rs"),
            "fn try_reuse_embedded_agent_chat_view(",
        ));
        assert!(
            body.contains(&tab_ai_contract_compact(
                "if normalized_intent.is_some() && is_setup_mode {"
            )),
            "non-empty entry intents must reject setup-mode Agent Chat cache reuse"
        );
        assert!(
            body.contains(&tab_ai_contract_compact("self.embedded_agent_chat = None;")),
            "setup-mode cache rejection must clear the stale embedded Agent Chat view"
        );
        assert!(
            body.contains("tab_ai_embedded_agent_chat_reuse_rejected_setup_mode"),
            "setup-mode cache rejection must leave a positive audit log"
        );
        assert!(
            body.contains(&tab_ai_contract_compact("return false;")),
            "setup-mode cache rejection must fall through to fresh launch resolution"
        );
    }

    #[test]
    fn main_menu_skill_launch_stages_slash_pick_without_entry_intent_submit_contract() {
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            include_str!("mod.rs"),
            "pub(crate) fn open_agent_chat_with_selected_skill(",
        ));

        assert!(
            body.contains(&tab_ai_contract_compact(
                "crate::ai::agent_chat::ui::build_skill_slash_command_text(&skill.skill_id)",
            )),
            "main-menu skill launch must use the same slash text as Agent Chat slash acceptance"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "crate::ai::agent_chat::ui::build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path)",
            )),
            "main-menu skill launch must attach the same skill context part as Agent Chat slash acceptance"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(None, cx);",
            )),
            "main-menu skill launch must open Agent Chat without an auto-submit entry intent"
        );
        assert!(
            body.contains("stage_selected_plugin_skill_from_main_menu"),
            "main-menu skill launch must stage the slash-style skill selection after Agent Chat opens"
        );
        assert!(
            !body.contains("build_staged_skill_prompt"),
            "main-menu skill launch must not build an entry-intent prompt because entry intents auto-submit"
        );
        assert!(
            !body.contains(&tab_ai_contract_compact(
                "open_tab_ai_agent_chat_with_entry_intent(Some",
            )),
            "main-menu skill launch must not pass selected skills as auto-submit entry intents"
        );
    }

    #[test]
    fn script_list_trigger_routes_stage_trigger_before_agent_chat_open_contract() {
        let source = include_str!("mod.rs");
        for (signature, trigger) in [(
            "pub(crate) fn open_tab_ai_agent_chat_with_slash_picker(",
            "Some('/')",
        )] {
            let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(source, signature));
            let trigger_idx = body
                .find(&tab_ai_contract_compact(&format!(
                    "self.tab_ai_harness_script_list_trigger = {trigger};"
                )))
                .expect("route must stage the trigger first");
            let open_idx = body
                .find(&tab_ai_contract_compact(
                    "self.open_tab_ai_agent_chat_with_entry_intent(None, cx);",
                ))
                .expect("route must open Agent Chat");
            assert!(
                trigger_idx < open_idx,
                "route must stage the trigger before opening Agent Chat"
            );
        }
    }

    #[test]
    fn script_list_trigger_routes_defer_embedded_picker_contract() {
        let source = include_str!("mod.rs");
        for signature in ["pub(crate) fn open_tab_ai_agent_chat_with_slash_picker("] {
            let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(source, signature));
            assert!(
                body.contains(&tab_ai_contract_compact(
                    "self.schedule_embedded_agent_chat_picker_open("
                )),
                "trigger route must defer embedded picker opening"
            );
        }
    }

    #[test]
    fn explicit_target_return_seeding_restores_previous_origin_without_agent_chat_launch() {
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            include_str!("mod.rs"),
            "pub(crate) fn open_tab_ai_agent_chat_with_explicit_target_preserving_return(",
        ));
        assert!(
            body.contains(&tab_ai_contract_compact(
                "let previous_return_view = self.tab_ai_harness_return_view.clone();"
            )) && body.contains(&tab_ai_contract_compact(
                "let previous_return_focus_target = self.tab_ai_harness_return_focus_target;"
            )) && body.contains(&tab_ai_contract_compact(
                "if !matches!(self.current_view, AppView::AgentChatView { .. }) {"
            )) && body.contains(&tab_ai_contract_compact(
                "self.tab_ai_harness_return_view = previous_return_view;"
            )) && body.contains(&tab_ai_contract_compact(
                "self.tab_ai_harness_return_focus_target = previous_return_focus_target;"
            )),
            "explicit target return seeding must restore the previous Agent Chat return origin when the handoff does not actually launch Agent Chat"
        );
    }

    // ── Existing save-name tests ──────────────────────────────────

    #[test]
    fn tab_ai_default_save_name_falls_back_to_slug_when_intent_is_generic() {
        let record = crate::ai::TabAiExecutionRecord::from_parts(
            "".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"ok\");\n".to_string(),
            "/tmp/tab-ai.ts".to_string(),
            "tab-ai-script".to_string(),
            "ScriptList".to_string(),
            None,
            "vercel/test-model".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert_eq!(
            ScriptListApp::tab_ai_default_save_name(&record),
            "tab-ai-script"
        );
    }

    #[test]
    fn tab_ai_default_save_name_derives_from_intent_when_meaningful() {
        let record = crate::ai::TabAiExecutionRecord::from_parts(
            "force quit this app".to_string(),
            "code".to_string(),
            "/tmp/tab-ai.ts".to_string(),
            "force-quit-this-app".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let name = ScriptListApp::tab_ai_default_save_name(&record);
        assert!(
            name.contains("force") && name.contains("quit"),
            "Should derive from intent, got: {name}"
        );
    }
}
