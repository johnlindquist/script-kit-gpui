#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_empty() -> TriggerPickerContext {
        TriggerPickerContext::default()
    }

    #[test]
    fn legacy_triggers_return_no_snapshot() {
        let ctx = ctx_empty();
        for input in ["", "git deploy", "~", "~/Desktop", "/", "@", ">", "?"] {
            assert!(
                build_trigger_picker_snapshot(input, &ctx).is_none(),
                "input '{input}' must not produce a trigger picker snapshot"
            );
        }
    }

    #[test]
    fn unknown_plus_head_returns_no_snapshot() {
        let ctx = ctx_empty();
        for input in ["+github", "+1", "+react component"] {
            assert!(
                build_trigger_picker_snapshot(input, &ctx).is_none(),
                "input '{input}' must fall back to fuzzy search"
            );
        }
    }

    #[test]
    fn trigger_picker_does_not_build_object_selector_snapshot() {
        let ctx = TriggerPickerContext::default();
        assert!(
            build_trigger_picker_snapshot(";snippet update @fetch", &ctx).is_none(),
            "object refs are owned by menu_syntax::object_selector, not TriggerPickerSnapshot"
        );
    }

    #[test]
    fn snippet_capture_field_popup_lists_metadata_fields() {
        let ctx = TriggerPickerContext::default();
        let snap =
            build_trigger_picker_snapshot(";snippet Hello there! :", &ctx).expect("snapshot");

        assert_eq!(snap.mode, TriggerPickerMode::Capture);
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "name" && row.token.as_deref() == Some("name:")));
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "keyword" && row.token.as_deref() == Some("keyword:")));
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "description" && row.token.as_deref() == Some("description:")));
    }

    #[test]
    fn snippet_capture_field_popup_filters_and_replaces_field_token() {
        let ctx = TriggerPickerContext::default();
        let snap =
            build_trigger_picker_snapshot(";snippet Hello there! de:", &ctx).expect("snapshot");

        assert_eq!(snap.rows[0].title, "description");
        match &snap.rows[0].action {
            TriggerPickerAction::ReplaceInput { text } => {
                assert_eq!(text, ";snippet Hello there! description: ");
            }
            other => panic!("expected ReplaceInput, got {other:?}"),
        }
    }

    #[test]
    fn link_capture_field_popup_lists_metadata_fields() {
        let ctx = TriggerPickerContext::default();
        let snap = build_trigger_picker_snapshot(";link https://example.com Example :", &ctx)
            .expect("snapshot");

        assert_eq!(snap.mode, TriggerPickerMode::Capture);
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "url" && row.token.as_deref() == Some("url:")));
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "title" && row.token.as_deref() == Some("title:")));
        assert!(snap
            .rows
            .iter()
            .any(|row| row.title == "description" && row.token.as_deref() == Some("description:")));
    }

    #[test]
    fn link_capture_field_popup_filters_and_replaces_field_token() {
        let ctx = TriggerPickerContext::default();
        let snap = build_trigger_picker_snapshot(";link https://example.com Example ti:", &ctx)
            .expect("snapshot");

        assert_eq!(snap.rows[0].title, "title");
        match &snap.rows[0].action {
            TriggerPickerAction::ReplaceInput { text } => {
                assert_eq!(text, ";link https://example.com Example title: ");
            }
            other => panic!("expected ReplaceInput, got {other:?}"),
        }
    }

    #[test]
    fn notes_alias_never_renders_as_capture_target_row() {
        let ctx = ctx_empty();
        assert!(capture_target_catalog(&ctx, true)
            .iter()
            .all(|entry| !entry.slug.eq_ignore_ascii_case("notes")));

        let snapshot = build_capture_snapshot(Some("notes"), &ctx);
        assert!(snapshot.rows.iter().all(|row| {
            row.token.as_deref() != Some("notes;")
                && !row.title.contains(";notes")
                && !row.detail.as_deref().unwrap_or("").contains(";notes")
                && !row.example.as_deref().unwrap_or("").contains(";notes")
        }));
        assert!(
            snapshot
                .rows
                .iter()
                .any(|row| row.token.as_deref() == Some("note;")),
            "hidden alias should point at the public note; row"
        );
        assert!(snapshot.rows.iter().all(|row| {
            !matches!(
                &row.action,
                TriggerPickerAction::CreateHandler { target: Some(target) }
                    if target.eq_ignore_ascii_case("notes")
            )
        }));
    }

    #[test]
    fn todo_aliases_never_render_as_capture_target_rows() {
        let ctx = ctx_empty();
        for alias in ["reminder", "snooze", "defer"] {
            assert!(capture_target_catalog(&ctx, true)
                .iter()
                .all(|entry| !entry.slug.eq_ignore_ascii_case(alias)));

            let snapshot = build_capture_snapshot(Some(alias), &ctx);
            assert!(snapshot.rows.iter().all(|row| {
                row.token.as_deref() != Some(&format!(";{alias}"))
                    && !row.title.contains(&format!(";{alias}"))
                    && !row
                        .detail
                        .as_deref()
                        .unwrap_or("")
                        .contains(&format!(";{alias}"))
                    && !row
                        .example
                        .as_deref()
                        .unwrap_or("")
                        .contains(&format!(";{alias}"))
            }));
            assert!(
                snapshot
                    .rows
                    .iter()
                    .any(|row| row.token.as_deref() == Some("todo;")),
                "hidden {alias} alias should point at the public todo; row"
            );
            assert!(snapshot.rows.iter().all(|row| {
                !matches!(
                    &row.action,
                    TriggerPickerAction::CreateHandler { target: Some(target) }
                        if target.eq_ignore_ascii_case(alias)
                )
            }));
        }
    }

    #[test]
    fn unknown_keyword_head_returns_no_snapshot() {
        let ctx = ctx_empty();
        assert!(build_trigger_picker_snapshot("localhost:3000", &ctx).is_none());
        assert!(build_trigger_picker_snapshot("not-a-target: stuff", &ctx).is_none());
    }

    #[test]
    fn exact_bare_colon_lists_source_and_property_heads_not_terminal_examples() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":", &ctx).expect("snapshot");
        assert_eq!(snap.mode, TriggerPickerMode::AdvancedQuery);
        assert!(snap.target.is_none());

        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        for expected in crate::menu_syntax::SOURCE_HEAD_SPECS
            .iter()
            .map(|spec| spec.canonical)
            .chain([
                "has:",
                "type:",
                "shortcut:",
                "source:",
                "plugin:",
                "name:",
                "desc:",
                "alias:",
                "tag:",
                "meta.<path>:",
            ])
        {
            assert!(
                tokens.contains(&expected),
                "bare ':' should list filter head {expected}, got {tokens:?}"
            );
        }

        for forbidden in [
            "type:script",
            "type:scriptlet",
            "shortcut:any",
            "shortcut:none",
            "shortcut:cmd+k",
            "has:menuSyntax",
            "has:shortcut",
            "-type:app",
            "#",
        ] {
            assert!(
                !tokens.contains(&forbidden),
                "bare ':' should not show concrete qualifier {forbidden}"
            );
        }

        assert!(snap.rows.iter().all(|row| row.example.is_none()));
        assert_eq!(
            snap.rows.first().and_then(|row| row.token.as_deref()),
            Some("files:"),
            "files: should be the first source head"
        );
    }

    #[test]
    fn partial_colon_narrows_qualifier_rows() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":t", &ctx).expect("snapshot");
        let qualifier_tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter(|row| row.kind == TriggerPickerRowKind::Qualifier)
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert_eq!(qualifier_tokens, vec!["todo:", "tabs:", "type:", "tag:"]);
        assert!(!qualifier_tokens.contains(&"type:script"));
        assert!(!qualifier_tokens.contains(&"shortcut:any"));
    }

    #[test]
    fn partial_colon_ty_shows_type_head_only() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":ty", &ctx).expect("snapshot");
        let qualifier_tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter(|row| row.kind == TriggerPickerRowKind::Qualifier)
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert_eq!(qualifier_tokens, vec!["type:"]);
    }

    #[test]
    fn type_value_partial_narrows_to_type_rows_only() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":type:s", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert!(
            tokens.iter().all(|token| token.starts_with("type:s")),
            "partial :type:s should only show matching type values, got {tokens:?}"
        );
        assert!(tokens.contains(&"type:script"));
        assert!(tokens.contains(&"type:scriptlet"));
        assert!(tokens.contains(&"type:skill"));
        assert!(!tokens.iter().any(|token| token.starts_with("files:")));
        assert!(!tokens.iter().any(|token| token.starts_with("todo:")));
        assert!(!tokens.iter().any(|token| token.starts_with("-type:")));
    }

    #[test]
    fn bare_type_value_partial_narrows_to_type_rows_only() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("type:s", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert!(
            tokens.iter().all(|token| token.starts_with("type:s")),
            "partial type:s should only show matching type values, got {tokens:?}"
        );
        assert!(tokens.contains(&"type:script"));
        assert!(tokens.contains(&"type:scriptlet"));
        assert!(tokens.contains(&"type:skill"));
        assert!(!tokens.iter().any(|token| token.starts_with("files:")));
        assert!(!tokens.iter().any(|token| token.starts_with("todo:")));
        assert!(!tokens.iter().any(|token| token.starts_with("-type:")));
    }

    #[test]
    fn type_value_partial_sc_narrows_further() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":type:sc", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert!(
            tokens.iter().all(|token| token.starts_with("type:sc")),
            "partial :type:sc should only show matching type values, got {tokens:?}"
        );
        assert!(tokens.contains(&"type:script"));
        assert!(tokens.contains(&"type:scriptlet"));
        assert!(!tokens.contains(&"type:skill"));
    }

    #[test]
    fn bare_type_value_partial_sc_narrows_further() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("type:sc", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert!(
            tokens.iter().all(|token| token.starts_with("type:sc")),
            "partial type:sc should only show matching type values, got {tokens:?}"
        );
        assert!(tokens.contains(&"type:script"));
        assert!(tokens.contains(&"type:scriptlet"));
        assert!(!tokens.contains(&"type:skill"));
    }

    #[test]
    fn bare_type_value_partial_scr_shows_script_and_scriptlet() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("type:scr", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert_eq!(tokens, vec!["type:script", "type:scriptlet"]);
    }

    #[test]
    fn colon_type_open_value_lists_type_rows_only() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":type:", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert_eq!(
            tokens
                .iter()
                .filter(|token| token.starts_with("type:"))
                .count(),
            8
        );
        assert!(
            tokens.iter().all(|token| token.starts_with("type:")),
            "open :type: should stay in type values, got {tokens:?}"
        );
    }

    #[test]
    fn bare_type_open_value_lists_type_rows_only() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("type:", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        assert_eq!(
            tokens,
            vec![
                "type:script",
                "type:scriptlet",
                "type:skill",
                "type:builtin",
                "type:app",
                "type:window",
                "type:agent",
                "type:issue",
            ]
        );
    }

    #[test]
    fn exact_type_value_without_boundary_keeps_value_picker() {
        let ctx = ctx_empty();
        for input in [":type:script", "type:script", ":kind:script", "kind:script"] {
            let snap = build_trigger_picker_snapshot(input, &ctx)
                .unwrap_or_else(|| panic!("{input:?} should keep picker rows"));
            let tokens: Vec<&str> = snap
                .rows
                .iter()
                .filter_map(|row| row.token.as_deref())
                .collect();
            assert!(
                tokens.contains(&"type:script"),
                "{input:?} should include script row, got {tokens:?}"
            );
            assert!(
                tokens.contains(&"type:scriptlet"),
                "{input:?} should include scriptlet row, got {tokens:?}"
            );
        }
    }

    #[test]
    fn complete_type_value_after_boundary_is_terminal() {
        let ctx = ctx_empty();
        for input in [
            ":type:script ",
            "type:script ",
            ":kind:script ",
            "kind:script ",
            ":type:script deploy",
            "type:script deploy",
        ] {
            assert!(
                build_trigger_picker_snapshot(input, &ctx).is_none(),
                "complete type value {input:?} with boundary should be terminal"
            );
        }
    }

    #[test]
    fn unknown_type_value_closes_picker() {
        let ctx = ctx_empty();

        assert!(build_trigger_picker_snapshot(":type:zzz", &ctx).is_none());
    }

    #[test]
    fn bare_unknown_type_value_closes_picker() {
        let ctx = ctx_empty();

        assert!(build_trigger_picker_snapshot("type:zzz", &ctx).is_none());
    }

    #[test]
    fn trigger_picker_includes_hash_tag_filter_row() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":#", &ctx).expect("snapshot");
        let row = snap
            .rows
            .iter()
            .find(|r| r.id == "qualifier:#")
            .expect("hash tag row");

        assert_eq!(row.title, "Filter by tag");
        assert_eq!(row.token.as_deref(), Some("#"));
        assert!(row.subtitle.as_deref().unwrap().contains("#work"));
        assert!(row.example.as_deref().unwrap().contains("#work"));
    }

    #[test]
    fn hash_tag_filter_row_keeps_popup_open() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":#", &ctx).expect("snapshot");
        let row = snap
            .rows
            .iter()
            .find(|r| r.id == "qualifier:#")
            .expect("hash tag row");

        match &row.action {
            TriggerPickerAction::InsertToken { token, keep_open } => {
                assert_eq!(token, "#");
                assert!(*keep_open, "tag filter row should stay open for a tag name");
            }
            other => panic!("expected InsertToken, got {other:?}"),
        }
    }

    #[test]
    fn canonical_tag_filter_row_keeps_popup_open() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":tag:", &ctx).expect("snapshot");
        let row = snap
            .rows
            .iter()
            .find(|r| r.id == "qualifier:tag:")
            .expect("tag row");

        match &row.action {
            TriggerPickerAction::InsertToken { token, keep_open } => {
                assert_eq!(token, "tag:");
                assert!(
                    *keep_open,
                    "canonical tag row should stay open for a tag name"
                );
            }
            other => panic!("expected InsertToken, got {other:?}"),
        }
    }

    #[test]
    fn advanced_query_popup_has_no_help_footer_by_default() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":", &ctx).expect("snapshot");
        assert!(
            snap.rows
                .iter()
                .all(|r| r.id != "footer:help" && r.action != TriggerPickerAction::OpenHelp),
            "advanced-query popup must not emit a generic help footer; main-hint owns context copy",
        );

        // `has:` also must not show the help footer — main-hint shows
        // catalog rows instead.
        let snap = build_trigger_picker_snapshot("has:", &ctx).expect("snapshot");
        assert!(
            snap.rows.iter().all(|r| r.id != "footer:help"),
            "advanced-query `has:` popup must not emit a help footer",
        );
    }

    #[test]
    fn complete_has_shortcut_does_not_open_completion_popup() {
        let ctx = ctx_empty();
        assert!(
            build_trigger_picker_snapshot("has:shortcut", &ctx).is_none(),
            "complete has:shortcut is a search predicate, not a completion state"
        );
        assert!(
            build_trigger_picker_snapshot("has:shortcut ", &ctx).is_none(),
            "trailing space after complete has:shortcut must not reopen completion"
        );
        assert!(
            build_trigger_picker_snapshot("has:shortc", &ctx).is_some(),
            "partial has:shortc should still offer has:shortcut"
        );
    }

    #[test]
    fn complete_non_completable_predicates_do_not_open_catalog_popup() {
        let ctx = ctx_empty();
        for input in [
            "shortcut:any",
            "name:deploy",
            "plugin:main",
            "tag:work",
            "type:script deploy",
        ] {
            assert!(
                build_trigger_picker_snapshot(input, &ctx).is_none(),
                "completed filter predicate {input:?} must not reopen the broad qualifier catalog"
            );
        }
    }

    #[test]
    fn colon_qualifier_with_open_value_keeps_popup_open() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":", &ctx).expect("snapshot");
        let type_row = snap
            .rows
            .iter()
            .find(|r| r.token.as_deref() == Some("type:"))
            .expect("type head row");
        match &type_row.action {
            TriggerPickerAction::InsertToken { token, keep_open } => {
                assert_eq!(token, "type:");
                assert!(*keep_open, "open-value qualifier must keep popup open");
            }
            other => panic!("expected InsertToken, got {other:?}"),
        }
    }

    #[test]
    fn colon_qualifier_concrete_row_closes_popup() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":type:", &ctx).expect("snapshot");
        let row = snap
            .rows
            .iter()
            .find(|r| r.id == "qualifier:type:script")
            .expect("type:script row");
        match &row.action {
            TriggerPickerAction::InsertToken { token, keep_open } => {
                assert_eq!(token, "type:script");
                assert!(!*keep_open, "concrete qualifier must close the popup");
            }
            other => panic!("expected InsertToken, got {other:?}"),
        }
    }

    #[test]
    fn typo_head_produces_fix_row() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":typ:script", &ctx).expect("snapshot");
        let fix = snap
            .rows
            .iter()
            .find(|r| r.kind == TriggerPickerRowKind::UnknownQualifierFix)
            .expect("fix row");
        match &fix.action {
            TriggerPickerAction::FixQualifier { bad, good } => {
                assert_eq!(bad, "typ:script");
                assert_eq!(good, "type:script");
            }
            other => panic!("expected FixQualifier, got {other:?}"),
        }
    }

    #[test]
    fn transposed_head_produces_fix_row() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":tpye:script", &ctx).expect("snapshot");
        let fix = snap
            .rows
            .iter()
            .find(|r| r.kind == TriggerPickerRowKind::UnknownQualifierFix)
            .expect("fix row from transposition");
        match &fix.action {
            TriggerPickerAction::FixQualifier { good, .. } => {
                assert_eq!(good, "type:script");
            }
            other => panic!("expected FixQualifier, got {other:?}"),
        }
    }

    #[test]
    fn meta_path_is_not_flagged_as_typo() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":meta.category:", &ctx).expect("snapshot");
        assert!(
            !snap
                .rows
                .iter()
                .any(|r| r.kind == TriggerPickerRowKind::UnknownQualifierFix),
            "meta.<path> qualifiers must not fire typo suggestions"
        );
        assert!(
            build_trigger_picker_snapshot(":meta.category:inbox", &ctx).is_none(),
            "completed meta.<path>:value predicates must not reopen the broad qualifier catalog"
        );
    }

    #[test]
    fn correct_qualifier_does_not_produce_fix_row() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(":type:s", &ctx).expect("snapshot");
        assert!(
            !snap
                .rows
                .iter()
                .any(|r| r.kind == TriggerPickerRowKind::UnknownQualifierFix),
            "correct qualifier must not produce fix row"
        );
    }

    #[test]
    fn exact_bare_colon_does_not_append_recent_queries() {
        let ctx = TriggerPickerContext {
            recent_queries: vec![
                ":type:script deploy".to_string(),
                ":shortcut:any".to_string(),
                "plain fuzzy text".to_string(),
                ";todo already captured".to_string(),
            ],
            ..Default::default()
        };
        let snap = build_trigger_picker_snapshot(":", &ctx).expect("snapshot");
        let recent: Vec<&TriggerPickerRow> = snap
            .rows
            .iter()
            .filter(|r| r.kind == TriggerPickerRowKind::RecentQuery)
            .collect();
        assert_eq!(
            recent.len(),
            0,
            "exact ':' should stay a filter-head catalog, not a recent-query list"
        );
    }

    #[test]
    fn bare_plus_builds_all_target_rows() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("+", &ctx).expect("snapshot");
        assert_eq!(snap.mode, TriggerPickerMode::Capture);
        assert!(snap.target.is_none());

        let targets: Vec<&str> = snap
            .rows
            .iter()
            .filter(|r| r.kind == TriggerPickerRowKind::CaptureTarget)
            .filter_map(|r| r.token.as_deref())
            .collect();
        assert_eq!(
            targets,
            vec!["todo;", "note;", "link;", "snippet;", "cal;", "social;"]
        );
    }

    #[test]
    fn registered_capture_targets_extend_plus_picker() {
        let github = make_script(
            "Capture GitHub Issue",
            "custom",
            r#"[{ "family": "capture.v1", "targets": ["github"] }]"#,
        );
        let ctx = TriggerPickerContext {
            scripts: vec![github],
            ..Default::default()
        };

        let bare = build_trigger_picker_snapshot("+", &ctx).expect("snapshot");
        assert!(bare
            .rows
            .iter()
            .any(|row| row.token.as_deref() == Some("github;")));

        let focused = build_trigger_picker_snapshot("+github", &ctx).expect("snapshot");
        assert_eq!(focused.target.as_deref(), Some("github"));
        assert_eq!(
            focused
                .rows
                .iter()
                .filter(|row| row.kind == TriggerPickerRowKind::CaptureTarget)
                .count(),
            1
        );
        assert!(
            build_trigger_picker_snapshot("+github issue", &ctx).is_none(),
            "registered target body composition should close the target picker"
        );
    }

    #[test]
    fn plus_with_target_focuses_single_target() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(";todo", &ctx).expect("snapshot");
        assert_eq!(snap.mode, TriggerPickerMode::Capture);
        assert_eq!(snap.target.as_deref(), Some("todo"));

        let target_rows: Vec<&TriggerPickerRow> = snap
            .rows
            .iter()
            .filter(|r| r.kind == TriggerPickerRowKind::CaptureTarget)
            .collect();
        assert_eq!(target_rows.len(), 1);
        assert_eq!(target_rows[0].token.as_deref(), Some("todo;"));
    }

    #[test]
    fn plus_target_with_body_is_composer_not_picker() {
        let ctx = ctx_empty();
        assert!(
            build_trigger_picker_snapshot(";todo buy milk", &ctx).is_none(),
            "body composition owns input after the target boundary; the target picker must close"
        );
    }

    #[test]
    fn keyword_alias_with_body_is_composer_not_picker() {
        let ctx = ctx_empty();
        assert!(
            build_trigger_picker_snapshot("note: decision", &ctx).is_none(),
            "keyword capture aliases compose text after the colon instead of opening the target picker"
        );
    }

    #[test]
    fn plus_target_incomplete_body_still_focuses_target() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(";todo", &ctx).expect("snapshot");
        assert_eq!(snap.target.as_deref(), Some("todo"));
        assert_eq!(
            snap.rows
                .iter()
                .filter(|r| r.kind == TriggerPickerRowKind::CaptureTarget)
                .count(),
            1,
        );
    }

    #[test]
    fn plus_footer_action_routes_to_create_handler() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot(";unknown_target", &ctx).expect("snapshot");
        let footer = snap
            .rows
            .iter()
            .find(|r| r.id == "footer:create-handler")
            .expect("create handler footer");
        match &footer.action {
            TriggerPickerAction::CreateHandler { target } => {
                assert_eq!(target.as_deref(), Some("unknown_target"));
            }
            other => panic!("expected CreateHandler, got {other:?}"),
        }
    }

    #[test]
    fn bare_plus_has_no_create_handler_footer() {
        let ctx = ctx_empty();
        let snap = build_trigger_picker_snapshot("+", &ctx).expect("snapshot");
        let has_footer = snap.rows.iter().any(|r| r.id == "footer:create-handler");
        assert!(!has_footer, "bare + must not show create handler footer");
    }

    #[test]
    fn row_ids_are_unique_within_snapshot() {
        let ctx = TriggerPickerContext {
            recent_queries: vec![
                ":type:script deploy".to_string(),
                ":has:menuSyntax".to_string(),
            ],
            ..Default::default()
        };
        let colon = build_trigger_picker_snapshot(":", &ctx).expect("colon snapshot");
        assert_ids_unique(&colon.rows);

        let plus = build_trigger_picker_snapshot("+", &ctx).expect("plus snapshot");
        assert_ids_unique(&plus.rows);
    }

    fn assert_ids_unique(rows: &[TriggerPickerRow]) {
        let mut seen: Vec<&str> = Vec::new();
        for row in rows {
            assert!(
                !seen.contains(&row.id.as_str()),
                "duplicate row id: {}",
                row.id
            );
            seen.push(row.id.as_str());
        }
    }

    #[test]
    fn capture_picker_never_renders_handler_rows_even_with_scripts() {
        let todo_script = make_script(
            "Add Todo",
            "my-plugin",
            r#"[{ "family": "capture.v1", "targets": ["todo"] }]"#,
        );
        let ctx = TriggerPickerContext {
            recent_queries: Vec::new(),
            scripts: vec![todo_script],
            scriptlets: Vec::new(),
        };
        let snap = build_trigger_picker_snapshot(";todo", &ctx).expect("snapshot");
        assert_eq!(snap.target.as_deref(), Some("todo"));
        assert_eq!(
            snap.rows
                .iter()
                .filter(|r| r.kind == TriggerPickerRowKind::CaptureHandler)
                .count(),
            0,
            "capture handlers execute after composer submit; they do not render in the target picker"
        );
    }

    #[test]
    fn bare_bang_builds_command_rows_from_scripts_and_scriptlets() {
        let deploy = make_script("Deploy Prod", "main", "[]");
        let scriptlet = Arc::new(Scriptlet {
            icon: None,
            name: "Open PR".to_string(),
            description: Some("Open a pull request".to_string()),
            code: String::new(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: Some("GitHub".to_string()),
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: Some("/tmp/scriptlets.md#open-pr".to_string()),
            command: Some("open-pr".to_string()),
            alias: None,
        });
        let ctx = TriggerPickerContext {
            scripts: vec![deploy],
            scriptlets: vec![scriptlet],
            ..Default::default()
        };

        let snap = build_trigger_picker_snapshot("!", &ctx).expect("snapshot");
        assert_eq!(snap.mode, TriggerPickerMode::Command);
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();
        assert!(tokens.contains(&">deploy-prod"));
        assert!(tokens.contains(&"!open-pr"));
    }

    #[test]
    fn duplicate_command_heads_are_visible_but_not_selectable() {
        let script = make_script("Deploy Prod", "main", "[]");
        let scriptlet = Arc::new(Scriptlet {
            icon: None,
            name: "Deploy Prod".to_string(),
            description: Some("Duplicate command".to_string()),
            code: String::new(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: Some("Ops".to_string()),
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: Some("/tmp/scriptlets.md#deploy-prod".to_string()),
            command: Some("deploy-prod".to_string()),
            alias: None,
        });
        let ctx = TriggerPickerContext {
            scripts: vec![script],
            scriptlets: vec![scriptlet],
            ..Default::default()
        };

        let snap = build_trigger_picker_snapshot("!dep", &ctx).expect("snapshot");
        assert_eq!(snap.rows.len(), 2);
        assert!(
            snap.rows
                .iter()
                .all(|row| !row.enabled && row.badges.iter().any(|badge| badge == "duplicate")),
            "duplicate ! heads should render as disabled ambiguity rows"
        );
    }

    #[test]
    fn demo_command_pack_surfaces_script_scriptlet_and_duplicate_rows() {
        let env_script = {
            let mut script = make_script("Power Syntax Command Env Dump", "main", "[]");
            Arc::make_mut(&mut script).alias = Some("ps-env".to_string());
            script
        };
        let dupe_script = {
            let mut script = make_script("Power Syntax Duplicate Command Script", "main", "[]");
            Arc::make_mut(&mut script).alias = Some("ps-dupe".to_string());
            script
        };
        let stamp_scriptlet = Arc::new(Scriptlet {
            icon: None,
            name: "PS Stamp".to_string(),
            description: Some("Append local stamp".to_string()),
            code: String::new(),
            tool: "bash".to_string(),
            shortcut: None,
            keyword: None,
            group: Some("menu-syntax-demo".to_string()),
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: Some("/tmp/power-syntax.md#ps-stamp".to_string()),
            command: Some("ps-stamp".to_string()),
            alias: Some("power-stamp".to_string()),
        });
        let dupe_scriptlet = Arc::new(Scriptlet {
            icon: None,
            name: "PS Dupe".to_string(),
            description: Some("Duplicate command".to_string()),
            code: String::new(),
            tool: "bash".to_string(),
            shortcut: None,
            keyword: None,
            group: Some("menu-syntax-demo".to_string()),
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: Some("/tmp/power-syntax.md#ps-dupe".to_string()),
            command: Some("ps-dupe".to_string()),
            alias: Some("power-dupe".to_string()),
        });
        let ctx = TriggerPickerContext {
            scripts: vec![env_script, dupe_script],
            scriptlets: vec![stamp_scriptlet, dupe_scriptlet],
            ..Default::default()
        };

        let snap = build_trigger_picker_snapshot("!ps", &ctx).expect("snapshot");
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();
        assert!(tokens.contains(&">ps-env"));
        assert!(tokens.contains(&"!ps-stamp"));
        assert!(tokens.contains(&"!ps-dupe"));

        let dupe_rows: Vec<&TriggerPickerRow> = snap
            .rows
            .iter()
            .filter(|row| row.token.as_deref() == Some("!ps-dupe"))
            .collect();
        assert_eq!(dupe_rows.len(), 2);
        assert!(dupe_rows
            .iter()
            .all(|row| { !row.enabled && row.badges.iter().any(|badge| badge == "duplicate") }));
    }

    #[test]
    fn partial_bang_filters_command_rows_and_accept_commits_command_head() {
        let deploy = make_script("Deploy Prod", "main", "[]");
        let docs = make_script("Generate Docs", "main", "[]");
        let ctx = TriggerPickerContext {
            scripts: vec![deploy, docs],
            ..Default::default()
        };

        let snap = build_trigger_picker_snapshot("!dep", &ctx).expect("snapshot");
        assert_eq!(snap.mode, TriggerPickerMode::Command);
        assert_eq!(snap.rows.len(), 1);
        assert_eq!(snap.rows[0].token.as_deref(), Some(">deploy-prod"));
        assert_eq!(
            snap.rows[0].action,
            TriggerPickerAction::InsertToken {
                token: ">deploy-prod ".to_string(),
                keep_open: false,
            }
        );
    }

    #[test]
    fn command_with_arguments_is_composer_not_picker() {
        let deploy = make_script("Deploy Prod", "main", "[]");
        let ctx = TriggerPickerContext {
            scripts: vec![deploy],
            ..Default::default()
        };
        assert!(build_trigger_picker_snapshot(">deploy-prod -- staging", &ctx).is_none());
    }

    fn make_script(name: &str, plugin_id: &str, menu_syntax_json: &str) -> Arc<Script> {
        use crate::metadata_parser::TypedMetadata;
        use std::collections::HashMap;
        use std::path::PathBuf;

        let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
        extra.insert(
            "menuSyntax".to_string(),
            serde_json::from_str(menu_syntax_json).expect("valid JSON"),
        );
        let mut meta = TypedMetadata::default();
        meta.extra = extra;
        Arc::new(Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{}.ts", name.to_lowercase().replace(' ', "-"))),
            extension: "ts".to_string(),
            description: Some(format!("{name} description")),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: Some(meta),
            schema: None,
            plugin_id: plugin_id.to_string(),
            plugin_title: None,
            kit_name: None,
            body: None,
        })
    }

    #[test]
    fn within_one_edit_detects_typos() {
        assert!(within_one_edit("typ", "type"));
        assert!(within_one_edit("tyep", "type"));
        assert!(within_one_edit("tpye", "type"));
        assert!(within_one_edit("typee", "type"));
        assert!(within_one_edit("tyme", "type"));
        assert!(!within_one_edit("", "type"));
        assert!(!within_one_edit("foo", "type"));
        assert!(!within_one_edit("typeabc", "type"));
    }

    #[test]
    fn has_filter_completion_shows_first_class_fields_only() {
        let ctx = TriggerPickerContext::default();
        let snap = build_trigger_picker_snapshot(":has", &ctx).expect("snapshot");

        assert_eq!(snap.mode, TriggerPickerMode::AdvancedQuery);
        let tokens: Vec<&str> = snap
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();

        // Should contain first-class fields from HAS_FIELD_SPECS
        assert!(tokens.contains(&"has:shortcut"));
        assert!(tokens.contains(&"has:alias"));
        assert!(tokens.contains(&"has:menuSyntax"));

        // Should NOT contain the old static examples that just happened to match "Has"
        assert!(!tokens.contains(&"shortcut:any"));
        assert!(!tokens.contains(&"shortcut:none"));
    }

    #[test]
    fn has_filter_completion_progressively_filters_fields() {
        let ctx = TriggerPickerContext::default();
        let all_rows = build_trigger_picker_snapshot(":has:", &ctx).expect("snapshot");
        let all_tokens: Vec<&str> = all_rows
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();
        assert!(all_tokens.contains(&"has:shortcut"));
        assert!(all_tokens.contains(&"has:alias"));
        assert!(all_tokens.contains(&"has:menuSyntax"));

        let menu_rows = build_trigger_picker_snapshot(":has:men", &ctx).expect("snapshot");
        let menu_tokens: Vec<&str> = menu_rows
            .rows
            .iter()
            .filter_map(|row| row.token.as_deref())
            .collect();
        assert!(menu_tokens.contains(&"has:menuSyntax"));
        assert!(!menu_tokens.contains(&"has:shortcut"));
        assert!(!menu_tokens.contains(&"has:alias"));

        assert!(
            build_trigger_picker_snapshot(":has:shortcut", &ctx).is_none(),
            "completed has:shortcut should be terminal search input, not a completion state"
        );
    }
}
