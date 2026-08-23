#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_syntax::{
        build_trigger_picker_snapshot, parse_advanced_query, TriggerPickerContext,
    };
    use std::path::PathBuf;

    fn script(name: &str, alias: Option<&str>) -> Arc<Script> {
        Arc::new(Script {
            name: name.to_string(),
            alias: alias.map(str::to_string),
            path: PathBuf::from(format!("/tmp/{}.ts", name.to_ascii_lowercase())),
            extension: "ts".to_string(),
            ..Default::default()
        })
    }

    fn mcal_script() -> Arc<Script> {
        use crate::metadata_parser::TypedMetadata;
        use serde_json::json;
        use std::collections::HashMap;

        let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
        extra.insert(
            "menuSyntax".to_string(),
            json!([{
                "family": "capture.v1",
                "targets": ["mcal"],
                "accepts": ["tags", "date", "dateRange", "duration", "recurrence", "kv"],
                "required": ["body", "date"],
                "label": "Add event to macOS Calendar",
                "payloadSchema": "kit://schema/menu-syntax/payload-v1",
                "defaultHandler": true
            }]),
        );
        Arc::new(Script {
            name: "Create macOS Calendar Event".to_string(),
            alias: None,
            path: PathBuf::from("/tmp/create-mac-calendar-event.ts"),
            extension: "ts".to_string(),
            typed_metadata: Some(TypedMetadata {
                extra,
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn capture_hint_for(raw: &str, scripts: &[Arc<Script>]) -> MenuSyntaxMainHintSnapshot {
        let targets = crate::menu_syntax::registered_capture_targets_from_scripts(scripts);
        let mode = MenuSyntaxMode::from_input_with_capture_targets(raw, &targets);
        build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts,
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("capture hint")
    }

    fn scriptlet(name: &str, command: Option<&str>) -> Arc<Scriptlet> {
        Arc::new(Scriptlet {
            icon: None,
            name: name.to_string(),
            description: None,
            code: String::new(),
            tool: "ts".to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            file_path: None,
            command: command.map(str::to_string),
            alias: None,
        })
    }

    #[test]
    fn unknown_slug_no_match_hint_is_setup_focused() {
        let mode = MenuSyntaxMode::from_input(";gcal");
        let snapshot = build_trigger_picker_snapshot(";gcal", &TriggerPickerContext::default())
            .expect("gcal snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: ";gcal",
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: Some("footer:create-handler"),
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CapturePickerCompanion);
        assert_eq!(hint.title, "No capture target named ;gcal");
        assert_eq!(hint.subtitle, None);
        assert!(hint.examples.is_empty(), "no examples in no-match state");
        assert_eq!(hint.example, None);
        assert_eq!(
            hint.status_chip.as_ref().map(|c| c.label.as_str()),
            Some("new target")
        );
        assert!(hint
            .primary_hint
            .as_deref()
            .unwrap()
            .contains("Press Enter to create the handler scaffold"));
        assert!(hint
            .secondary_hint
            .as_deref()
            .unwrap()
            .contains("Cmd+Enter"));
        let row_labels: Vec<&str> = hint.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(row_labels.contains(&"Action"));
        assert!(row_labels.contains(&"File"));
        assert!(row_labels.contains(&"Registers"));
        for row in &hint.rows {
            assert_ne!(row.label, "Selected");
        }
        // Near-miss "Similar" line should fire for ;gcal -> ;cal (one edit away).
        let similar = hint
            .rows
            .iter()
            .find(|r| r.label == "Similar")
            .expect("similar row for ;gcal -> ;cal");
        assert!(similar.value.contains(";cal"));
        assert!(similar.value.contains("Calendar event"));
    }

    #[test]
    fn unknown_slug_no_match_hint_drops_choose_target_copy() {
        let mode = MenuSyntaxMode::from_input(";zzzz");
        let snapshot = build_trigger_picker_snapshot(";zzzz", &TriggerPickerContext::default())
            .expect("zzzz snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: ";zzzz",
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: Some("footer:create-handler"),
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        let primary = hint.primary_hint.unwrap_or_default();
        let secondary = hint.secondary_hint.unwrap_or_default();
        assert!(!primary.contains("Choose a capture target"));
        assert!(!secondary.contains("After choosing"));
        // No near-miss for ;zzzz against built-ins (todo/cal/note/social/link).
        assert!(hint.rows.iter().all(|r| r.label != "Similar"));
    }

    #[test]
    fn known_slug_picker_companion_keeps_examples() {
        // Sanity check: the no-match branch must not steal the existing
        // ;todo/;cal/;note/;social/;link behavior. A committed known target
        // (`;todo`) is owned by the capture composer (A4 pivot), so the
        // hint is the composer card — examples must survive regardless.
        let mode = MenuSyntaxMode::from_input(";todo");
        let snapshot = build_trigger_picker_snapshot(";todo", &TriggerPickerContext::default())
            .expect("todo snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: ";todo",
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: Some("target:todo"),
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.title, "Capture Todo inbox");
        assert!(!hint.examples.is_empty());
        assert!(hint.examples.iter().all(|e| e.starts_with(";todo")));
    }

    #[test]
    fn semicolon_picker_companion_describes_selected_target() {
        let mode = MenuSyntaxMode::from_input(";");
        let snapshot = build_trigger_picker_snapshot("+", &TriggerPickerContext::default())
            .expect("plus snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: "+",
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: Some("target:todo"),
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CapturePickerCompanion);
        assert_eq!(hint.title, "Todo inbox");
        assert!(hint
            .primary_hint
            .as_deref()
            .unwrap()
            .contains("accept ;todo"));
    }

    #[test]
    fn capture_composer_previews_payload() {
        let raw = ";todo Renew passport #errands p1 due:tomorrow";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CaptureComposer);
        // Composer titles use the resolved target display title
        // ("Todo inbox"), matching `reminder_hint_labels_todo_operation`.
        assert_eq!(hint.title, "Capture Todo inbox");
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Body" && row.value == "Renew passport"));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Tags" && row.value == "#errands"));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Priority" && row.value == "P1"));
    }

    #[test]
    fn capture_composer_explains_tags_as_labels() {
        let raw = ";todo Buy milk";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CaptureComposer);
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Tags" && row.value.contains("#errands")));
        assert!(hint
            .secondary_hint
            .as_deref()
            .unwrap()
            .contains("Tags group the saved item"));
        assert!(hint
            .examples
            .iter()
            .any(|example| example.contains("#errands")));
    }

    #[test]
    fn unregistered_semicolon_head_gets_no_hint() {
        let raw = ";github issue #bug";
        let mode = MenuSyntaxMode::from_input(raw);
        assert!(build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .is_none());
    }

    #[test]
    fn registered_semicolon_head_gets_capture_hint() {
        let raw = ";github issue #bug";
        let targets = vec!["github".to_string()];
        let mode = MenuSyntaxMode::from_input_with_capture_targets(raw, &targets);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("registered target hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CaptureComposer);
        assert_eq!(hint.title, "Capture github");
    }

    #[test]
    fn command_composer_previews_fields_tags_and_argv() {
        let raw = ">ps-env env:dev project:launcher #demo -- --dry-run alpha";
        let mode = MenuSyntaxMode::from_input(raw);
        let scripts = vec![script("Power Syntax Env", Some("ps-env"))];
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &scripts,
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CommandComposer);
        assert_eq!(hint.title, "Run ps-env");
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Fields" && row.value.contains("env=dev")));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Tags" && row.value == "#demo"));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Argv" && row.value.contains("--dry-run")));
    }

    #[test]
    fn unknown_command_warns_without_shell_semantics() {
        // `>` is the command sigil since the grammar pivot dropped `!`.
        let raw = ">important";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CommandComposer);
        assert!(hint.title.contains("No registered command"));
        assert!(hint
            .primary_hint
            .as_deref()
            .unwrap()
            .contains("not run a shell"));
    }

    /// The grammar pivot removed `!` as a command sigil, so a bang-prefixed
    /// fuzzy query must expose the same hint classification as plain text.
    #[test]
    fn removed_bang_sigil_matches_plain_text_hint_classification() {
        fn hint_kind(raw: &str) -> Option<MenuSyntaxMainHintKind> {
            let mode = MenuSyntaxMode::from_input(raw);
            build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
                raw_filter_text: raw,
                mode: &mode,
                picker_snapshot: None,
                picker_selected_row_id: None,
                scripts: &[],
                scriptlets: &[],
                advanced_query_results_empty: has_active_head(raw),
                menu_syntax_ai_proposal: None,
            })
            .map(|hint| hint.kind)
        }

        let plain = hint_kind("x");
        let bang_prefixed = hint_kind("!x");

        assert_eq!(bang_prefixed, plain);
        assert_eq!(plain, None, "plain fuzzy text currently has no main hint");
    }

    #[test]
    fn duplicate_command_warns() {
        let raw = ">ps-dupe";
        let mode = MenuSyntaxMode::from_input(raw);
        let scripts = vec![script("Duplicate Script", Some("ps-dupe"))];
        let scriptlets = vec![scriptlet("Duplicate Scriptlet", Some("ps-dupe"))];
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &scripts,
            scriptlets: &scriptlets,
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CommandComposer);
        assert_eq!(hint.title, "Ambiguous command");
        assert!(hint.warning.as_deref().unwrap().contains("2 registered"));
    }

    #[test]
    fn bare_colon_main_hint_explains_refine() {
        let raw = ":";
        let mode = MenuSyntaxMode::from_input(raw);
        let snapshot =
            build_trigger_picker_snapshot(raw, &TriggerPickerContext::default()).expect("snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryGuide);
        assert_eq!(hint.title, "Refine launcher search");
        assert!(hint.subtitle.as_deref().unwrap().contains("add filters"));
        assert!(hint
            .examples
            .iter()
            .any(|example| example == ":#work type:script"));
    }

    #[test]
    fn colon_hash_main_hint_explains_tag_filter_boundary() {
        let raw = ":#";
        let mode = MenuSyntaxMode::from_input(raw);
        let snapshot =
            build_trigger_picker_snapshot(raw, &TriggerPickerContext::default()).expect("snapshot");
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: Some("qualifier:#"),
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryGuide);
        assert_eq!(hint.title, "Filter by tag");
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "#work" && row.value.contains("Plain")));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == ":#work" && row.value.contains("Filter")));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == ";... #work" && row.value.contains("Label")));
    }

    #[test]
    fn advanced_query_empty_summarizes_predicates() {
        let raw = ":#work type:script nohit";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: true,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryEmpty);
        // Head-aware empty copy: a `type:` predicate wins the title, so the
        // zero-result state names the kind and the search words.
        assert_eq!(hint.title, "No scripts match `nohit`.");
        assert!(hint.rows.iter().any(|row| {
            row.label == "Filters"
                && row.value.contains("#work")
                && row.value.contains("scripts only")
        }));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Search words" && row.value == "nohit"));
        assert!(hint
            .primary_hint
            .as_deref()
            .unwrap()
            .contains("Remove `type:script`"));
    }

    #[test]
    fn plain_top_level_tag_gets_tag_empty_hint() {
        // Since 57c7696df bare `#work` claims an AdvancedQuery (tag
        // predicate) at the top level, so a zero-result state renders the
        // tag-specific empty hint instead of staying silent.
        let raw = "#work";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: true,
            menu_syntax_ai_proposal: None,
        })
        .expect("tag empty hint");
        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryEmpty);
        assert_eq!(hint.title, "No launcher items tagged #work");
    }

    #[test]
    fn command_composer_renders_schema_rows_for_registered_head() {
        // sdk-command-schema: a script that registers a `command.v1`
        // handler with `head: deploy`, args `[env]`, flags `[--dry-run]`
        // makes `setFilter ">deploy"` getState surface "env" and
        // "--dry-run" as labels in `menuSyntaxMainHint.rows`.
        use crate::metadata_parser::TypedMetadata;
        use serde_json::json;
        use std::collections::HashMap;

        let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
        extra.insert(
            "menuSyntax".to_string(),
            json!([{
                "family": "command.v1",
                "head": "deploy",
                "label": "Deploy a service",
                "args": [
                    {"name": "env", "required": true,
                     "values": ["prod", "staging", "dev"]}
                ],
                "flags": [
                    {"name": "--dry-run", "alias": "-n",
                     "description": "Print the plan without applying"}
                ],
                "usage": ">deploy -- <env> [--dry-run]"
            }]),
        );
        let typed = TypedMetadata {
            extra,
            ..Default::default()
        };
        let s = Arc::new(Script {
            name: "Deploy".to_string(),
            alias: None,
            path: PathBuf::from("/tmp/deploy.ts"),
            extension: "ts".to_string(),
            typed_metadata: Some(typed),
            ..Default::default()
        });

        let raw = ">deploy";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: std::slice::from_ref(&s),
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CommandComposer);
        let labels: Vec<&str> = hint.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(
            labels.contains(&"env"),
            "expected `env` arg row, got rows: {labels:?}"
        );
        assert!(
            labels.contains(&"--dry-run"),
            "expected `--dry-run` flag row, got rows: {labels:?}"
        );
        // The arg's `required: true` becomes a "required" chip on the env row.
        let env_row = hint
            .rows
            .iter()
            .find(|r| r.label == "env")
            .expect("env row");
        assert!(
            env_row.chips.iter().any(|c| c.label == "required"),
            "expected `required` chip on env row, got: {:?}",
            env_row.chips
        );
        // The arg's `values` list becomes the row value text so authors see
        // accepted choices in the hint card.
        assert_eq!(env_row.value, "prod | staging | dev");
        let dry_row = hint
            .rows
            .iter()
            .find(|r| r.label == "--dry-run")
            .expect("--dry-run row");
        assert!(
            dry_row.value.contains("Print the plan"),
            "expected description in flag value, got: {}",
            dry_row.value
        );
    }

    #[test]
    fn command_composer_without_schema_omits_schema_rows() {
        // Negative pin: command_composer_hint must not invent schema rows
        // when no script registers a matching command.v1 handler. This pins
        // the `script_command_schema_for` dependency — a regression that
        // returned a stub spec by default would surface ghost rows.
        let raw = ">unknown";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");

        assert_eq!(hint.kind, MenuSyntaxMainHintKind::CommandComposer);
        // The default `Command >unknown` row remains, but no `env` /
        // `--dry-run` schema rows should exist.
        let labels: Vec<&str> = hint.rows.iter().map(|r| r.label.as_str()).collect();
        assert!(
            !labels.contains(&"env") && !labels.contains(&"--dry-run"),
            "schema rows leaked through without a registered handler: {labels:?}"
        );
    }

    #[test]
    fn predicate_label_handles_negation() {
        let query = parse_advanced_query(":-type:app has:menuSyntax");
        let labels = query
            .predicates
            .iter()
            .map(predicate_label)
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["-type:app", "has:menuSyntax"]);
    }

    // ========================================================================
    // capture_validation_snapshot (Pass 22)
    // ========================================================================

    #[test]
    fn capture_validation_cal_with_no_invocation_yields_missing_snapshot() {
        // Receipt from story: setFilter ";cal" reports captureValidation.status
        // = incomplete while capture-form header chips stay empty.
        let validation = capture_validation_snapshot("cal", None, None, &[], &[]);
        let v = validation.expect("cal has a builtin schema");
        assert_eq!(v.status, MenuSyntaxCaptureValidationStatus::Incomplete);
        assert!(!v.can_submit);
        assert_eq!(v.target, "cal");
        assert_eq!(
            v.missing_field_labels,
            vec!["body".to_string(), "date".to_string()]
        );
    }

    #[test]
    fn capture_validation_cal_with_body_and_date_yields_ready() {
        let mut inv = CaptureInvocation {
            target: "cal".to_string(),
            alias_form: CaptureAlias::CapturePrefix,
            body: "Design review".to_string(),
            tags: vec![],
            priority: None,
            url: None,
            duration: None,
            kv: vec![],
            date_phrases: vec![],
            raw: ";cal Design review start:friday".to_string(),
        };
        inv.date_phrases
            .push(crate::menu_syntax::payload::DatePhrase {
                role: DateRole::Start,
                source: "friday".to_string(),
                source_span: (0, 6),
            });
        let validation = capture_validation_snapshot("cal", Some(&inv), None, &[], &[]);
        let v = validation.unwrap();
        assert_eq!(v.status, MenuSyntaxCaptureValidationStatus::Ready);
        assert!(v.can_submit);
        assert!(v.missing_field_labels.is_empty());
    }

    #[test]
    fn capture_validation_unknown_target_returns_no_snapshot() {
        let validation = capture_validation_snapshot("github", None, None, &[], &[]);
        assert!(
            validation.is_none(),
            "no builtin schema for github → no snapshot; doctor flags this elsewhere"
        );
    }

    #[test]
    fn capture_validation_link_with_bad_url_yields_malformed() {
        let inv = CaptureInvocation {
            target: "link".to_string(),
            alias_form: CaptureAlias::CapturePrefix,
            body: String::new(),
            tags: vec![],
            priority: None,
            url: Some("ftp://nope".to_string()),
            duration: None,
            kv: vec![],
            date_phrases: vec![],
            raw: ";link ftp://nope".to_string(),
        };
        let validation = capture_validation_snapshot("link", Some(&inv), None, &[], &[]);
        let v = validation.unwrap();
        assert_eq!(v.status, MenuSyntaxCaptureValidationStatus::Malformed);
        assert!(!v.can_submit);
        assert_eq!(v.malformed_field_label.as_deref(), Some("url"));
        assert!(v.malformed_reason.as_deref().unwrap().contains("http"));
    }

    #[test]
    fn capture_validation_uses_resolved_nl_state_for_mcal() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal Lunch with Ryan tomorrow at 12pm til 1pm", &scripts);
        assert!(hint.status_chips.is_empty());
        assert!(hint.mode_chip.is_none());
        assert!(hint.status_chip.is_none());
        let validation = hint.capture_validation.expect("validation");
        assert_eq!(validation.status, MenuSyntaxCaptureValidationStatus::Ready);
        assert!(validation.can_submit);
        assert!(validation.missing_field_labels.is_empty());
    }

    #[test]
    fn capture_validation_mcal_date_only_needs_body_not_date() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal tomorrow at 12pm til 1pm", &scripts);
        assert!(hint.status_chips.is_empty());
        assert!(hint.mode_chip.is_none());
        assert!(hint.status_chip.is_none());
        let validation = hint.capture_validation.expect("validation");
        assert_eq!(validation.missing_field_labels, vec!["body".to_string()]);
    }

    #[test]
    fn capture_validation_snapshot_serializes_to_camel_case() {
        let snapshot = MenuSyntaxCaptureValidationSnapshot {
            target: "cal".to_string(),
            status: MenuSyntaxCaptureValidationStatus::Incomplete,
            can_submit: false,
            missing_field_labels: vec!["body".to_string(), "date".to_string()],
            malformed_field_label: None,
            malformed_reason: None,
            hud_message: Some(";cal needs body and date".to_string()),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"canSubmit\":false"), "got {json}");
        assert!(json.contains("\"missingFieldLabels\""), "got {json}");
        assert!(json.contains("\"status\":\"incomplete\""), "got {json}");
        // Empty optional fields are skipped
        assert!(!json.contains("malformedFieldLabel"), "got {json}");
    }

    // -------- target_examples (Run 12 Pass 2 — hint-examples-target-relevant) --------

    #[test]
    fn target_examples_for_cal_all_start_with_semicolon_cal() {
        let examples = target_examples("cal");
        assert!(!examples.is_empty(), ";cal must have ≥1 example");
        for ex in &examples {
            assert!(
                ex.starts_with(";cal "),
                "all ;cal examples must start with `;cal `, got: {ex}"
            );
        }
    }

    #[test]
    fn target_examples_for_cal_have_no_todo_leakage() {
        // Falsifier: this is the exact bug the user reported in screenshot
        // /Users/johnlindquist/screenshots/CleanShot 2026-04-25 at 09.27.22@2x.png
        // — `;cal` previously showed a `;todo Send proposal …` example mixed
        // in. After this story ships, a `;cal` hint must NEVER contain `;todo`.
        let examples = target_examples("cal");
        for ex in &examples {
            assert!(
                !ex.contains(";todo"),
                "`;cal` example MUST NOT contain `;todo`, got: {ex}"
            );
        }
    }

    #[test]
    fn target_examples_for_cal_include_a_date_slot() {
        // ;cal requires a date — the example should double as a fix-it
        // template, so at least one example must show a date key.
        let examples = target_examples("cal");
        let has_date = examples.iter().any(|ex| {
            ex.contains("start:")
                || ex.contains("at:")
                || ex.contains("due:")
                || ex.contains("end:")
        });
        assert!(
            has_date,
            ";cal examples must include at least one date slot (start:/at:/due:/end:), got: {examples:?}"
        );
    }

    #[test]
    fn target_examples_for_todo_all_start_with_semicolon_todo() {
        let examples = target_examples("todo");
        assert!(!examples.is_empty());
        for ex in &examples {
            assert!(ex.starts_with(";todo "), "got: {ex}");
        }
    }

    #[test]
    fn target_examples_for_notes_alias_return_public_note_examples() {
        let examples = target_examples("notes");
        assert!(!examples.is_empty());
        assert!(examples.iter().all(|example| example.starts_with(";note ")));
        assert!(examples
            .iter()
            .all(|example| !example.starts_with(";notes ")));
    }

    #[test]
    fn target_examples_for_todo_aliases_return_public_todo_examples() {
        for alias in ["reminder", "snooze", "defer"] {
            let examples = target_examples(alias);
            assert!(!examples.is_empty());
            assert!(examples.iter().all(|example| example.starts_with(";todo ")));
            assert!(examples
                .iter()
                .all(|example| !example.starts_with(&format!(";{alias} "))));
        }
    }

    #[test]
    fn target_examples_for_unknown_target_falls_back_with_correct_verb() {
        // Custom user-defined targets get the generic example list, but each
        // example MUST still start with the user's actual verb — no `;todo`
        // leakage even on the fallback path.
        let examples = target_examples("custom");
        assert!(!examples.is_empty());
        for ex in &examples {
            assert!(
                ex.starts_with(";custom "),
                "fallback example must use the actual target verb, got: {ex}"
            );
            assert!(
                !ex.contains(";todo"),
                "fallback must not leak ;todo, got: {ex}"
            );
        }
    }

    #[test]
    fn target_examples_for_shipped_dynamic_targets_match_their_handlers() {
        let cases = [
            ("github", ["johnlindquist/kit", "repo=", "url:"]),
            ("expense", ["amount=", "vendor=", "reimbursable="]),
            ("snippet", ["trigger:", "lang:", "--"]),
            ("fixture", ["env=", "kind=", "state="]),
            ("gcal", ["calendarId=", "start:", "guests="]),
            ("mcal", ["calendar=", "alarm=", "start:"]),
        ];

        for (target, expected_fragments) in cases {
            let examples = target_examples(target);
            assert_eq!(examples.len(), 3, "{target} should ship three examples");
            for example in &examples {
                assert!(
                    example.starts_with(&format!(";{target} ")),
                    "{target} example must use its own target, got: {example}"
                );
                assert!(
                    !example.contains("Buy milk") && !example.contains("Send proposal"),
                    "{target} example leaked generic todo copy: {example}"
                );
            }
            for fragment in expected_fragments {
                assert!(
                    examples.iter().any(|example| example.contains(fragment)),
                    "{target} examples should include `{fragment}`, got: {examples:?}"
                );
            }
        }
    }

    #[test]
    fn reminder_hint_labels_todo_operation() {
        let hint = capture_hint_for(";reminder Walk dog every day at 8am", &[]);
        assert_eq!(hint.title, "Capture Todo reminder");
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Target" && row.value == "todo"));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Operation" && row.value == "remind"));
    }

    #[test]
    fn snippet_hint_mentions_body_separator() {
        let hint = capture_hint_for(
            ";snippet add fetch-json trigger:fj lang:ts -- const res = await fetch(url)",
            &[],
        );
        assert_eq!(hint.title, "Capture Snippet");
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Body separator" && row.value.contains("--")));
    }

    #[test]
    fn snippet_hint_labels_update_operation_from_body() {
        let hint = capture_hint_for(";snippet update @snippet:fj -- const value = 1", &[]);
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Operation" && row.value == "update"));
    }

    #[test]
    fn main_hint_snapshot_omits_fragment_preview_when_none() {
        let snapshot = MenuSyntaxMainHintSnapshot {
            kind: MenuSyntaxMainHintKind::CaptureComposer,
            raw_filter_text: ";mcal Lunch".to_string(),
            title: "Capture mcal".to_string(),
            subtitle: None,
            mode_chip: None,
            status_chip: None,
            status_chips: Vec::new(),
            capture_validation: None,
            form: None,
            unresolved_dates: Vec::new(),
            menu_syntax_ai_proposal: None,
            rows: Vec::new(),
            fragment_preview: None,
            primary_hint: None,
            secondary_hint: None,
            example: None,
            examples: Vec::new(),
            warning: None,
            active_head: None,
            active_head_value_partial: None,
            accessibility_label: String::new(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("fragmentPreview"), "{json}");
    }

    #[test]
    fn fragment_preview_snapshot_serializes_camel_case() {
        let preview = MenuSyntaxFragmentPreviewSnapshot {
            rows: vec![MenuSyntaxFragmentPreviewRow {
                role: crate::menu_syntax::fragments::MenuSyntaxFragmentRole::DateRange,
                label: "When".to_string(),
                value: "tomorrow 12-1".to_string(),
                source: "tomorrow 12pm til 1pm".to_string(),
                source_span: (5, 27),
                status: crate::menu_syntax::fragments::MenuSyntaxFragmentStatus::Resolved,
                tone: MenuSyntaxMainHintTone::Info,
                chips: vec![MenuSyntaxMainHintChip {
                    label: "range".to_string(),
                    tone: MenuSyntaxMainHintTone::Accent,
                }],
            }],
        };
        let json = serde_json::to_string(&preview).unwrap();
        assert!(json.contains("\"sourceSpan\":[5,27]"), "{json}");
        assert!(json.contains("\"dateRange\""), "{json}");
        assert!(json.contains("\"tone\":\"Info\""), "{json}");
    }

    #[test]
    fn capture_composer_fragment_preview_for_mcal_range() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal Lunch with Ryan tomorrow at 12pm til 1pm", &scripts);
        let preview = hint.fragment_preview.expect("fragment preview");
        assert!(preview.rows.iter().any(
            |row| row.role == MenuSyntaxFragmentRole::Subject && row.value == "Lunch with Ryan"
        ));
        assert!(preview.rows.iter().any(|row| {
            row.role == MenuSyntaxFragmentRole::DateRange
                && row.label == "Date range"
                && row.value.contains("resolved")
        }));
        let range = preview
            .rows
            .iter()
            .find(|row| row.role == MenuSyntaxFragmentRole::DateRange)
            .expect("range row");
        assert_eq!(range.source, "tomorrow at 12pm til 1pm");
        assert_eq!(range.source_span, (22, 46));
    }

    #[test]
    fn capture_composer_fragment_preview_for_mcal_duration() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal Lunch with Ryan tom 12pm for 30mins", &scripts);
        let preview = hint.fragment_preview.expect("fragment preview");
        assert!(preview.rows.iter().any(|row| {
            row.role == MenuSyntaxFragmentRole::Duration && row.value.contains("30 minutes")
        }));
    }

    #[test]
    fn capture_composer_fragment_preview_for_mcal_recurrence() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal Lunch w/ Ryan every mon from 1 til 2", &scripts);
        let preview = hint.fragment_preview.expect("fragment preview");
        assert!(preview.rows.iter().any(|row| {
            row.role == MenuSyntaxFragmentRole::Recurrence
                && row.value.contains("FREQ=WEEKLY;BYDAY=MO")
        }));
    }

    #[test]
    fn capture_composer_fragment_preview_marks_unresolved_muted() {
        let scripts = vec![mcal_script()];
        let hint = capture_hint_for(";mcal Lunch start:asdf", &scripts);
        let preview = hint.fragment_preview.expect("fragment preview");
        assert!(preview.rows.iter().any(|row| {
            row.role == MenuSyntaxFragmentRole::Unresolved
                && row.status == MenuSyntaxFragmentStatus::Unresolved
                && row.tone == MenuSyntaxMainHintTone::Muted
        }));
    }

    #[test]
    fn main_hint_snapshot_omits_fragment_preview_when_capture_empty() {
        let scripts = vec![mcal_script()];
        let targets = crate::menu_syntax::registered_capture_targets_from_scripts(&scripts);
        let raw = ";mcal ";
        let mode = MenuSyntaxMode::from_input_with_capture_targets(raw, &targets);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &scripts,
            scriptlets: &[],
            advanced_query_results_empty: false,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");
        let json = serde_json::to_string(&hint).unwrap();
        assert!(!json.contains("fragmentPreview"), "{json}");
    }

    #[test]
    fn existing_non_capture_hint_json_unchanged_with_fragment_preview_field() {
        let raw = ":type:script nope";
        let mode = MenuSyntaxMode::from_input(raw);
        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: true,
            menu_syntax_ai_proposal: None,
        })
        .expect("hint");
        let json = serde_json::to_string(&hint).unwrap();
        assert!(!json.contains("fragmentPreview"), "{json}");
    }

    // ----- Run 12: head-aware empty hint regression coverage -----

    fn empty_hint_for(raw: &str) -> MenuSyntaxMainHintSnapshot {
        let mode = MenuSyntaxMode::from_input(raw);
        build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: None,
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: true,
            menu_syntax_ai_proposal: None,
        })
        .expect("empty hint")
    }

    #[test]
    fn type_filter_zero_result_hint_has_rows_for_skill() {
        let hint = empty_hint_for(":type:skill review");

        assert!(
            !hint.rows.is_empty(),
            "zero-result :type:skill hint must have body rows"
        );
        assert_eq!(hint.active_head.as_deref(), Some(":type:"));
        assert_eq!(hint.active_head_value_partial.as_deref(), Some("skill"));
        // Head-aware zero-result body: a Filters summary plus the search
        // words, with the recovery copy carried by primary_hint.
        assert!(hint.rows.iter().any(|row| row.label == "Filters"));
        assert!(hint
            .rows
            .iter()
            .any(|row| row.label == "Search words" && row.value == "review"));
        assert!(hint
            .primary_hint
            .as_deref()
            .unwrap()
            .contains("Remove `type:skill`"));
    }

    #[test]
    fn has_bare_head_lists_catalog_examples() {
        let hint = empty_hint_for("has:");
        assert!(
            !hint.examples.is_empty(),
            "bare has: must list catalog examples"
        );
        for token in &hint.examples {
            assert!(
                token.starts_with("has:"),
                "examples must be has:<field> tokens, got {token}"
            );
        }
        assert!(hint.examples.iter().any(|e| e == "has:shortcut"));
    }

    #[test]
    fn has_partial_sh_lists_only_shortcut() {
        let hint = empty_hint_for("has:sh");
        assert!(
            hint.examples.iter().all(|e| e == "has:shortcut"),
            "has:sh examples must be exactly [has:shortcut], got {:?}",
            hint.examples
        );
        assert!(
            !hint.examples.iter().any(|e| e.contains("#work")
                || e.contains("tag:work")
                || e.contains("type:script deploy")),
            "has:sh must not leak generic tag/type examples: {:?}",
            hint.examples
        );
        assert_eq!(hint.active_head.as_deref(), Some("has:"));
        assert_eq!(hint.active_head_value_partial.as_deref(), Some("sh"));
    }

    #[test]
    fn has_partial_examples_do_not_leak_tag_or_type_examples() {
        let hint = empty_hint_for("has:shortcut");
        for token in &hint.examples {
            assert!(!token.contains("#work"), "{token}");
            assert!(!token.contains("tag:work"), "{token}");
            assert!(!token.contains("type:script deploy"), "{token}");
        }
    }

    #[test]
    fn has_field_rows_match_popup_tokens() {
        let rows = has_field_rows_for_partial("sh");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "has:shortcut");
    }

    #[test]
    fn has_unknown_field_empty_copy_suggests_known_has_fields() {
        let hint = empty_hint_for("has:notAField");
        assert!(
            hint.title.contains("notAField"),
            "title must name the typed field, got: {}",
            hint.title
        );
        let primary = hint.primary_hint.as_deref().unwrap_or_default();
        assert!(primary.contains("has:shortcut"));
        assert!(primary.contains("has:alias"));
        assert!(primary.contains("has:menuSyntax"));
        assert_eq!(
            hint.examples,
            vec![
                "has:shortcut".to_string(),
                "has:alias".to_string(),
                "has:menuSyntax".to_string(),
            ]
        );
        assert!(
            !primary.contains("Remove a filter"),
            "must not fall back to generic copy"
        );
        assert_eq!(hint.active_head.as_deref(), Some("has:"));
        assert_eq!(hint.active_head_value_partial.as_deref(), Some("notAField"));
    }

    #[test]
    fn has_unknown_field_empty_copy_is_field_specific() {
        let hint = empty_hint_for("has:weird");
        assert_eq!(hint.title, "No scripts or scriptlets have a `weird` field.");
    }

    #[test]
    fn clipboard_source_zero_copy_names_clipboard_entries() {
        let hint = empty_hint_for("c:zzz");
        assert_eq!(hint.title, "No clipboard entries match `zzz`.");
        assert_eq!(
            hint.primary_hint.as_deref(),
            Some("Press Esc to clear the filter.")
        );
        assert_eq!(hint.active_head.as_deref(), Some("c:"));
        assert_eq!(hint.active_head_value_partial.as_deref(), Some("zzz"));
    }

    #[test]
    fn source_attached_clipboard_zero_copy_is_contextual() {
        let hint = empty_hint_for("clipboard:zzz");
        assert_eq!(hint.title, "No clipboard entries match `zzz`.");
        assert_eq!(
            hint.primary_hint.as_deref(),
            Some("Press Esc to clear the filter.")
        );
        for token in &hint.examples {
            assert!(!token.contains("#work"));
            assert!(!token.contains("type:"));
        }
    }

    #[test]
    fn type_scriptlet_zero_copy_removes_type_filter() {
        let hint = empty_hint_for(":type:scriptlet zzz");
        assert_eq!(hint.title, "No scriptlets match `zzz`.");
        assert_eq!(
            hint.primary_hint.as_deref(),
            Some("Remove `type:scriptlet` to widen.")
        );
        for token in &hint.examples {
            assert!(
                !token.contains("#work"),
                "type:scriptlet examples must not leak tag copy: {token}"
            );
            assert!(
                !token.contains("tag:work"),
                "type:scriptlet examples must not leak tag copy: {token}"
            );
        }
    }

    #[test]
    fn snapshot_serializes_active_head_camel_case() {
        let hint = empty_hint_for("has:sh");
        let json = serde_json::to_value(&hint).unwrap();
        assert_eq!(
            json.get("activeHead").and_then(|v| v.as_str()),
            Some("has:")
        );
        assert_eq!(
            json.get("activeHeadValuePartial").and_then(|v| v.as_str()),
            Some("sh"),
        );
    }

    #[test]
    fn has_context_never_serializes_tag_examples() {
        for raw in ["has:", "has:s", "has:sh", "has:shortcut"] {
            let hint = empty_hint_for(raw);
            let json = serde_json::to_string(&hint).unwrap();
            assert!(
                !json.contains(":#work"),
                "{raw} hint leaked :#work — payload: {json}"
            );
            assert!(
                !json.contains(":tag:work"),
                "{raw} hint leaked :tag:work — payload: {json}"
            );
            assert!(
                !json.contains(":type:script deploy"),
                "{raw} hint leaked :type:script deploy — payload: {json}"
            );
        }
    }

    #[test]
    fn advanced_empty_primary_copy_is_single_sentence() {
        // Compact recovery sentence: empty states omit the legacy
        // multi-sentence secondary hint.
        let hint = empty_hint_for("has:sh");
        assert!(hint.secondary_hint.is_none());
    }

    #[test]
    fn active_head_detector_classifies_known_heads() {
        let ctx = active_head_context_for_filter("has:sh").expect("has:");
        assert_eq!(ctx.head, "has:");
        assert_eq!(ctx.value_partial, "sh");

        let ctx = active_head_context_for_filter("c:zzz").expect("c:");
        assert_eq!(ctx.head, "c:");
        assert_eq!(ctx.value_partial, "zzz");

        let ctx = active_head_context_for_filter("clipboard:zzz").expect("clipboard:");
        assert!(ctx.head == "c:" || ctx.head == "clipboard:");

        let ctx = active_head_context_for_filter(":type:scriptlet").expect(":type:");
        assert_eq!(ctx.head, ":type:");
        assert_eq!(ctx.value_partial, "scriptlet");

        let ctx = active_head_context_for_filter("type:scriptlet").expect("type:");
        assert_eq!(ctx.head, "type:");
        assert_eq!(ctx.value_partial, "scriptlet");

        let ctx = active_head_context_for_filter(":tag:work").expect(":tag:");
        assert_eq!(ctx.head, ":tag:");
        assert_eq!(ctx.value_partial, "work");

        let ctx = active_head_context_for_filter("meta.x:").expect("meta.x:");
        assert_eq!(ctx.head, "meta.x:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter(":meta.x:value").expect(":meta.x:");
        assert_eq!(ctx.head, ":meta.x:");
        assert_eq!(ctx.value_partial, "value");

        let ctx = active_head_context_for_filter("name:").expect("name:");
        assert_eq!(ctx.head, "name:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter("desc:").expect("desc:");
        assert_eq!(ctx.head, "desc:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter("description:").expect("description:");
        assert_eq!(ctx.head, "description:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter("alias:").expect("alias:");
        assert_eq!(ctx.head, "alias:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter("plugin:").expect("plugin:");
        assert_eq!(ctx.head, "plugin:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter("source:").expect("source:");
        assert_eq!(ctx.head, "source:");
        assert_eq!(ctx.value_partial, "");

        let ctx = active_head_context_for_filter(";daily").expect(";");
        assert_eq!(ctx.head, ";");
        assert_eq!(ctx.value_partial, "daily");

        let ctx = active_head_context_for_filter("!ps").expect("!");
        assert_eq!(ctx.head, "!");
        assert_eq!(ctx.value_partial, "ps");
    }

    #[test]
    fn active_filter_head_owns_unresolved_filter_heads() {
        for raw in [
            "type:",
            "type:t",
            "type:to",
            "type:zzz",
            ":type:s",
            "has:",
            "has:x",
            "meta.",
            "meta.x",
            "meta.x:",
            "name:",
            "desc:",
            "description:",
            "alias:",
            "plugin:",
            "source:",
        ] {
            assert!(active_filter_head_owns_main_list(raw), "{raw}");
        }
    }

    #[test]
    fn active_filter_head_does_not_own_terminal_queries_or_source_heads() {
        for raw in [
            "type:script",
            ":type:script",
            "has:shortcut",
            "shortcut:any",
            "name:deploy",
            "plugin:main",
            "meta.x:value",
            "c:",
            "c:zzz",
            "clipboard:zzz",
            "files:report",
            "f:",
            ";todo",
            ">deploy",
            "png :f",
            ":bro",
            "plain search",
        ] {
            assert!(!active_filter_head_owns_main_list(raw), "{raw}");
        }
    }

    #[test]
    fn meta_path_open_value_gets_filter_owned_empty_hint() {
        let hint = empty_hint_for("meta.x:");
        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryEmpty);
        assert_eq!(hint.active_head.as_deref(), Some("meta.x:"));
        assert!(hint.rows.iter().any(|row| row.label == "Filter"));
    }

    #[test]
    fn name_open_value_gets_filter_owned_empty_hint() {
        let hint = empty_hint_for("name:");
        assert_eq!(hint.kind, MenuSyntaxMainHintKind::AdvancedQueryEmpty);
        assert_eq!(hint.active_head.as_deref(), Some("name:"));
        assert!(hint.rows.iter().any(|row| row.label == "Filter"));
    }

    #[test]
    fn type_value_picker_rows_suppress_empty_hint() {
        let raw = "type:s";
        let mode = MenuSyntaxMode::from_input(raw);
        let snapshot =
            build_trigger_picker_snapshot(raw, &TriggerPickerContext::default()).expect("snapshot");

        let hint = build_menu_syntax_main_hint(MenuSyntaxMainHintContext {
            raw_filter_text: raw,
            mode: &mode,
            picker_snapshot: Some(&snapshot),
            picker_selected_row_id: None,
            scripts: &[],
            scriptlets: &[],
            advanced_query_results_empty: true,
            menu_syntax_ai_proposal: None,
        });

        assert!(
            hint.is_none(),
            "picker rows should own the main list instead of reporting an empty hint: {hint:?}"
        );
    }

    /// Regression: multibyte first tokens used to abort the whole app with
    /// "byte index N is not a char boundary" when [[qualifier_value_partial]]
    /// sliced the token at a qualifier head's byte length (e.g. "ミーティング"
    /// at the length of `todo:`). Every entry here must simply not panic.
    #[test]
    fn active_head_detector_survives_multibyte_filters() {
        for raw in [
            "ミーティング",
            "ミーティング 予算",
            "予算",
            "🚀",
            "🚀 launch checklist",
            "résumé café",
            "héllo: wörld",
            "ñañ:value",
            "日本語のクエリをここに入力",
            "한국어 검색어",
            "e\u{301}le\u{301}phant", // combining accents
            ";ミーティング",
            "type:ミーティング",
        ] {
            let _ = active_head_context_for_filter(raw);
            let _ = active_head_is_source_filter(raw);
        }
    }
}
