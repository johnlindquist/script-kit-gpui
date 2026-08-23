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
        let day_page_plan = ScriptListApp::spine_prompt_plan_for_aliases(&parse, &day_page_aliases);

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
            prompt.contains("fenced ```ts block"),
            "Prompt must ask for a fenced TypeScript block so extract_generated_script_source works"
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
        use super::AgentChatContextPolicy::{AmbientOrFocused, SuppressFocused};
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('@'),
            &AmbientOrFocused,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('/'),
            &AmbientOrFocused,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            &AmbientOrFocused,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: 0,
            },
            Some('@'),
            &AmbientOrFocused,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            &SuppressFocused,
        ));
    }

    #[test]
    fn clean_context_policy_does_not_prime_script_authoring_slash() {
        use super::AgentChatContextPolicy::{AmbientOrFocused, SuppressFocused};
        assert!(ScriptListApp::should_prime_script_authoring_slash(
            false,
            false,
            false,
            None,
            &AmbientOrFocused,
        ));
        assert!(!ScriptListApp::should_prime_script_authoring_slash(
            false,
            false,
            false,
            None,
            &SuppressFocused,
        ));
    }

    #[test]
    fn empty_script_list_cmd_enter_suppresses_default_auto_selected_row() {
        use super::AgentChatContextPolicy::{AmbientOrFocused, SuppressFocused};
        assert_eq!(
            ScriptListApp::cmd_enter_context_policy_for_launcher_selection(true, true, 0, Some(0),),
            SuppressFocused,
        );
        assert_eq!(
            ScriptListApp::cmd_enter_context_policy_for_launcher_selection(true, true, 0, None,),
            SuppressFocused,
            "uncached first-selectable fallback preserves selected-index-zero behavior",
        );
        assert_eq!(
            ScriptListApp::cmd_enter_context_policy_for_launcher_selection(true, true, 1, Some(0),),
            AmbientOrFocused,
            "a user-moved non-default row remains meaningful context",
        );
        assert_eq!(
            ScriptListApp::cmd_enter_context_policy_for_launcher_selection(true, false, 0, Some(0),),
            AmbientOrFocused,
        );
    }

    #[test]
    fn file_search_cmd_enter_keeps_selected_row_context() {
        assert_eq!(
            ScriptListApp::cmd_enter_context_policy_for_launcher_selection(false, true, 0, Some(0),),
            super::AgentChatContextPolicy::AmbientOrFocused,
            "File Search must keep its selected file or directory as context",
        );
    }

    #[test]
    fn embedded_cache_rejects_cross_policy_reuse() {
        use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::{Full, QuickAi};
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;

        // A cached Quick AI view must NOT be reused for a Standard relaunch —
        // set_ui_variant is tighten-only, so the reused view would stay
        // capability-restricted while presenting as a full chat. Reject → fresh.
        assert!(
            !ScriptListApp::embedded_reuse_policy_matches(QuickAi, AgentChatUiVariant::Standard),
            "cached Quick AI view must not be reused by a Standard launch",
        );
        // Reverse mode-laundering: a cached full view must not be reused by a
        // Quick AI launch (would leak retained context into a zero-context surface).
        assert!(
            !ScriptListApp::embedded_reuse_policy_matches(Full, AgentChatUiVariant::QuickAi),
            "cached full view must not be reused by a Quick AI launch",
        );

        // Matching policies may reuse the cached view.
        assert!(ScriptListApp::embedded_reuse_policy_matches(
            Full,
            AgentChatUiVariant::Standard
        ));
        assert!(ScriptListApp::embedded_reuse_policy_matches(
            QuickAi,
            AgentChatUiVariant::QuickAi
        ));
        // Full policy is shared across every nonstandard non-Quick-AI variant,
        // so a cached full view may be reused across those restyles.
        assert!(ScriptListApp::embedded_reuse_policy_matches(
            Full,
            AgentChatUiVariant::UserBold
        ));
    }

    /// WP-B1: Quick AI NEVER reuses a retained embedded thread — not even for
    /// another Quick AI launch (same-policy). Combined with the cross-policy
    /// mismatch ban this closes every embedded-cache resurrection path.
    #[test]
    fn quick_ai_never_reuses_same_policy_embedded_cache() {
        use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::{Full, QuickAi};
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;

        // The banned same-policy case: a closed Quick AI view must NOT be
        // reused by another Quick AI launch, even though the policies match.
        assert!(
            ScriptListApp::embedded_reuse_policy_matches(QuickAi, AgentChatUiVariant::QuickAi),
            "sanity: QuickAi↔QuickAi policies do match",
        );
        assert!(
            !ScriptListApp::embedded_cache_reuse_allowed(QuickAi, AgentChatUiVariant::QuickAi),
            "a Quick AI launch must never reuse a cached view — start fresh",
        );
        // Any incoming Quick AI launch is non-reusable regardless of the cache.
        assert!(!ScriptListApp::embedded_cache_reuse_allowed(
            Full,
            AgentChatUiVariant::QuickAi
        ));
        // Full reuse still works for matching policies.
        assert!(ScriptListApp::embedded_cache_reuse_allowed(
            Full,
            AgentChatUiVariant::Standard
        ));
        // Cross-policy is still refused.
        assert!(!ScriptListApp::embedded_cache_reuse_allowed(
            QuickAi,
            AgentChatUiVariant::Standard
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
