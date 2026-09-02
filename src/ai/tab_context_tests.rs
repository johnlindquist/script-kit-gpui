#[cfg(test)]
mod execution_record_compat_tests {
    use super::*;

    #[test]
    fn legacy_v1_execution_record_fixture_still_deserializes() {
        let json = std::fs::read_to_string("tests/fixtures/tab_ai_execution_record_v1.json")
            .expect("missing tests/fixtures/tab_ai_execution_record_v1.json");
        let record: TabAiExecutionRecord =
            serde_json::from_str(&json).expect("legacy v1 record should deserialize");
        assert!(!record.intent.is_empty());
        assert!(!record.generated_source.is_empty());
        assert_eq!(record.context_warning_count, 0);
        assert!(
            record.model_id.is_empty(),
            "v1 had no model_id — default should be empty string"
        );
        assert!(
            record.provider_id.is_empty(),
            "v1 had no provider_id — default should be empty string"
        );

        tracing::info!(
            event = "execution_record_compat_test_passed",
            schema_version = record.schema_version,
            intent = %record.intent,
            context_warning_count = record.context_warning_count,
        );
    }

    #[test]
    fn v2_record_with_all_fields_still_deserializes() {
        let json = r#"{
            "schemaVersion": 2,
            "intent": "open browser",
            "generatedSource": "line1\nline2\nline3",
            "tempScriptPath": "/tmp/test.ts",
            "slug": "open-browser",
            "promptType": "ScriptList",
            "modelId": "gpt-4.1",
            "providerId": "vercel",
            "contextWarningCount": 2,
            "executedAt": "2026-03-28T00:00:00Z"
        }"#;
        let record: TabAiExecutionRecord =
            serde_json::from_str(json).expect("v2 record should deserialize");
        assert_eq!(record.schema_version, 2);
        assert_eq!(record.model_id, "gpt-4.1");
        assert_eq!(record.provider_id, "vercel");
        assert_eq!(record.context_warning_count, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_ai_context_blob_default_roundtrip() {
        let blob = TabAiContextBlob {
            schema_version: TAB_AI_CONTEXT_SCHEMA_VERSION,
            timestamp: "2026-03-28T00:00:00Z".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&blob).expect("serialize");
        let parsed: TabAiContextBlob = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(parsed.timestamp, "2026-03-28T00:00:00Z");
    }

    #[test]
    fn tab_ai_ui_snapshot_skips_empty_fields() {
        let snap = TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&snap).expect("serialize");
        // Empty optional fields should be omitted
        assert!(!json.contains("inputText"));
        assert!(!json.contains("focusedSemanticId"));
        assert!(!json.contains("visibleElements"));
    }

    #[test]
    fn tab_ai_context_blob_from_parts_deterministic() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ArgPrompt".to_string(),
            input_text: Some("Slack".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            selected_semantic_id: Some("choice:0:slack".to_string()),
            visible_elements: vec![crate::protocol::ElementInfo::product_static_choice(
                0, "Slack", "slack", true,
            )],
        };
        let desktop = crate::context_snapshot::AiContextSnapshot {
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                name: "Slack".to_string(),
                bundle_id: "com.tinyspeck.slackmacgap".to_string(),
                pid: 1234,
            }),
            ..Default::default()
        };
        let recent_inputs = vec!["copy url".to_string(), "open finder".to_string()];
        let ts = "2026-03-28T12:00:00Z".to_string();

        let blob = TabAiContextBlob::from_parts(
            ui,
            desktop,
            recent_inputs,
            None,
            vec![],
            vec![],
            ts.clone(),
        );

        assert_eq!(blob.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(blob.timestamp, ts);
        assert_eq!(blob.ui.prompt_type, "ArgPrompt");
        assert_eq!(blob.ui.input_text.as_deref(), Some("Slack"));
        assert_eq!(blob.ui.visible_elements.len(), 1);
        assert_eq!(
            blob.desktop.frontmost_app.as_ref().map(|a| a.name.as_str()),
            Some("Slack")
        );
        assert_eq!(blob.recent_inputs.len(), 2);
        assert!(blob.clipboard.is_none());
    }

    #[test]
    fn tab_ai_context_blob_camel_case_json_fields() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            input_text: Some("test".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            selected_semantic_id: None,
            visible_elements: vec![],
        };
        let blob = TabAiContextBlob::from_parts(
            ui,
            Default::default(),
            vec!["recent".to_string()],
            Some(TabAiClipboardContext {
                content_type: "text".to_string(),
                preview: "clipboard text".to_string(),
                ocr_text: None,
            }),
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");

        // Verify camelCase field names in JSON output
        assert!(json.contains("schemaVersion"));
        assert!(json.contains("promptType"));
        assert!(json.contains("inputText"));
        assert!(json.contains("focusedSemanticId"));
        assert!(json.contains("recentInputs"));
        assert!(json.contains("contentType"));

        // Verify snake_case is NOT present
        assert!(!json.contains("schema_version"));
        assert!(!json.contains("prompt_type"));
        assert!(!json.contains("input_text"));
        assert!(!json.contains("recent_inputs"));
        assert!(!json.contains("content_type"));
    }

    #[test]
    fn tab_ai_context_blob_json_roundtrip_with_all_fields() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ClipboardHistory".to_string(),
            input_text: Some("search term".to_string()),
            focused_semantic_id: Some("choice:2:item".to_string()),
            selected_semantic_id: Some("choice:2:item".to_string()),
            visible_elements: vec![
                crate::protocol::ElementInfo::input("filter", Some("search term"), true),
                crate::protocol::ElementInfo::product_static_choice(0, "Item A", "a", false),
                crate::protocol::ElementInfo::product_static_choice(1, "Item B", "b", false),
                crate::protocol::ElementInfo::product_static_choice(2, "Item C", "item", true),
            ],
        };
        let desktop = crate::context_snapshot::AiContextSnapshot {
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                name: "Chrome".to_string(),
                bundle_id: "com.google.Chrome".to_string(),
                pid: 5678,
            }),
            selected_text: Some("selected words".to_string()),
            browser: Some(crate::context_snapshot::BrowserContext::from_url(
                "https://example.com".to_string(),
            )),
            ..Default::default()
        };
        let blob = TabAiContextBlob::from_parts(
            ui,
            desktop,
            vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()],
            Some(TabAiClipboardContext {
                content_type: "text".to_string(),
                preview: "clipboard preview".to_string(),
                ocr_text: None,
            }),
            vec![],
            vec![],
            "2026-03-28T18:30:00Z".to_string(),
        );

        let json = serde_json::to_string_pretty(&blob).expect("serialize");
        let parsed: TabAiContextBlob = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(parsed.ui.prompt_type, "ClipboardHistory");
        assert_eq!(parsed.ui.visible_elements.len(), 4);
        assert_eq!(
            parsed.desktop.selected_text.as_deref(),
            Some("selected words")
        );
        assert_eq!(
            parsed.desktop.browser.as_ref().map(|b| b.url.as_str()),
            Some("https://example.com")
        );
        assert_eq!(parsed.recent_inputs.len(), 3);
        assert_eq!(
            parsed.clipboard.as_ref().map(|c| c.preview.as_str()),
            Some("clipboard preview")
        );
    }

    #[test]
    fn tab_ai_context_schema_version_is_three() {
        assert_eq!(TAB_AI_CONTEXT_SCHEMA_VERSION, 3);
    }

    #[test]
    fn tab_ai_context_blob_omits_empty_optional_fields() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");
        assert!(json.contains("\"schemaVersion\":3"));
        assert!(
            !json.contains("recentInputs"),
            "empty Vec should be omitted"
        );
        assert!(!json.contains("clipboard"), "None should be omitted");
        assert!(
            !json.contains("priorAutomations"),
            "empty Vec should be omitted"
        );
    }

    #[test]
    fn tab_ai_user_prompt_preserves_multiline_intent_and_contract() {
        let prompt = build_tab_ai_user_prompt(
            "rename selection\nthen copy it",
            r#"{"ui":{"promptType":"ScriptList"}}"#,
        );
        assert!(prompt.contains("User intent:\nrename selection\nthen copy it"));
        assert!(prompt.contains("Context JSON:"));
        assert!(prompt.contains(r#"{"ui":{"promptType":"ScriptList"}}"#));
        assert!(prompt.contains("Script Kit TypeScript"));
        assert!(prompt.contains("fenced ```ts block"));
    }

    // --- TabAiExecutionRecord tests ---

    fn sample_execution_record() -> TabAiExecutionRecord {
        TabAiExecutionRecord::from_parts(
            "force quit Slack".to_string(),
            "import '@anthropic-ai/sdk';\nawait exec('kill Slack');\nconsole.log('done');"
                .to_string(),
            "/tmp/scriptlet-abc123.ts".to_string(),
            "force-quit-slack".to_string(),
            "AppLauncher".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T12:00:00Z".to_string(),
        )
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_stores_memory_atomically_and_repairs_legacy_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private Tab AI memory fixture");
        let path = directory.path().join("private-tab-ai-memory.json");
        let record = sample_execution_record();
        let written = write_tab_ai_memory_entry_to_path(&record, &path)
            .expect("private owner-only Tab AI memory write");
        assert_eq!(written.intent, record.intent);
        assert_eq!(written.generated_source, record.generated_source);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let entries = read_tab_ai_memory_index_from_path(&path)
            .expect("legacy Tab AI memory permissions repair before read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].generated_source, record.generated_source);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_repairs_receipt_boundary_and_owner_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private Tab AI receipt fixture");
        let path = directory.path().join("private-tab-ai-executions.jsonl");
        let record = sample_execution_record();
        let first = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        std::fs::write(&path, serde_json::to_vec(&first).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let second = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );

        append_tab_ai_execution_receipt_to_path(&second, &path)
            .expect("append safe receipt after a legacy unterminated line");
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<TabAiExecutionReceipt>(lines[1])
                .unwrap()
                .status,
            TabAiExecutionStatus::Succeeded
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_refuses_symlinks_without_exposing_private_paths() {
        let directory = tempfile::tempdir().expect("isolated private Tab AI symlink fixture");
        let external = directory.path().join("unrelated-private-data.json");
        let planted = directory
            .path()
            .join("private-cancer-treatment-memory.json");
        std::fs::write(&external, "preserve unrelated private data").unwrap();
        std::os::unix::fs::symlink(&external, &planted).unwrap();
        let record = sample_execution_record();
        let receipt = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );

        let read_error = read_tab_ai_memory_index_from_path(&planted)
            .expect_err("Tab AI memory must not read a planted symlink");
        assert!(!read_error.contains("private-cancer-treatment"));
        let write_error = write_tab_ai_memory_entry_to_path(&record, &planted)
            .expect_err("Tab AI memory must not write through a planted symlink");
        assert!(!write_error.contains("private-cancer-treatment"));
        let audit_error = append_tab_ai_execution_receipt_to_path(&receipt, &planted)
            .expect_err("Tab AI receipts must not append through a planted symlink");
        assert!(!audit_error.contains("private-cancer-treatment"));
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "preserve unrelated private data"
        );
        assert!(std::fs::symlink_metadata(&planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn tab_ai_execution_record_from_parts_sets_schema_version() {
        let record = sample_execution_record();
        assert_eq!(
            record.schema_version,
            TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION
        );
        assert_eq!(record.intent, "force quit Slack");
        assert_eq!(record.slug, "force-quit-slack");
        assert_eq!(record.prompt_type, "AppLauncher");
    }

    #[test]
    fn tab_ai_execution_record_serde_roundtrip() {
        let record = sample_execution_record();
        let json = serde_json::to_string(&record).expect("serialize");
        let parsed: TabAiExecutionRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.schema_version, record.schema_version);
        assert_eq!(parsed.intent, record.intent);
        assert_eq!(parsed.slug, record.slug);
        assert_eq!(parsed.bundle_id, record.bundle_id);
    }

    #[test]
    fn tab_ai_execution_record_omits_none_bundle_id() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "code".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&record).expect("serialize");
        assert!(!json.contains("bundleId"));
    }

    #[test]
    fn should_offer_save_returns_true_for_three_plus_lines() {
        let record = sample_execution_record();
        // sample has 3 non-empty lines
        assert!(should_offer_save(&record));
    }

    #[test]
    fn should_offer_save_returns_false_for_fewer_than_three_lines() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "one\ntwo".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(!should_offer_save(&record));
    }

    #[test]
    fn should_offer_save_returns_false_for_empty_source() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "   ".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(!should_offer_save(&record));
    }

    // --- TabAiExecutionReceipt tests ---

    #[test]
    fn append_tab_ai_execution_receipt_writes_one_json_line() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join(".tab-ai-executions.jsonl");

        let record = sample_execution_record();
        let receipt = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&receipt, &path).expect("append");

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one line per receipt");

        let parsed: TabAiExecutionReceipt = serde_json::from_str(lines[0]).expect("valid JSON");
        assert_eq!(parsed.status, TabAiExecutionStatus::Dispatched);
        assert_eq!(parsed.slug, "force-quit-slack");
        assert_eq!(parsed.model_id, "gpt-4.1");
        assert_eq!(parsed.provider_id, "vercel");
        assert!(!parsed.save_offer_eligible);
        assert!(!parsed.memory_write_eligible);

        // camelCase check
        assert!(lines[0].contains("modelId"));
        assert!(lines[0].contains("providerId"));
        assert!(lines[0].contains("contextWarningCount"));
        assert!(lines[0].contains("saveOfferEligible"));
        assert!(!lines[0].contains("model_id"));
        assert!(!lines[0].contains("provider_id"));
    }

    #[test]
    fn append_receipt_is_append_only() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join(".tab-ai-executions.jsonl");

        let record = sample_execution_record();

        let r1 = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&r1, &path).expect("append 1");

        let r2 = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&r2, &path).expect("append 2");

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "two receipts = two lines");

        let p1: TabAiExecutionReceipt = serde_json::from_str(lines[0]).expect("parse line 1");
        let p2: TabAiExecutionReceipt = serde_json::from_str(lines[1]).expect("parse line 2");
        assert_eq!(p1.status, TabAiExecutionStatus::Dispatched);
        assert_eq!(p2.status, TabAiExecutionStatus::Succeeded);
        assert!(p2.save_offer_eligible);
        assert!(p2.memory_write_eligible);
    }

    #[test]
    fn build_receipt_sets_eligibility_based_on_status() {
        let record = sample_execution_record();

        let dispatched = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        assert!(!dispatched.memory_write_eligible);
        assert!(!dispatched.save_offer_eligible);

        let succeeded = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );
        assert!(succeeded.memory_write_eligible);
        assert!(succeeded.save_offer_eligible);

        let failed = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Failed,
            true,
            true,
            Some("exit code 1".to_string()),
        );
        assert!(!failed.memory_write_eligible);
        assert!(!failed.save_offer_eligible);
        assert_eq!(failed.error.as_deref(), Some("exit code 1"));
    }

    #[test]
    fn cleanup_tab_ai_temp_script_returns_true_for_absent_file() {
        assert!(cleanup_tab_ai_temp_script(
            "/tmp/nonexistent-tab-ai-test-12345.ts"
        ));
    }

    #[test]
    fn cleanup_tab_ai_temp_script_removes_existing_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("tab-ai-test-cleanup.ts");
        std::fs::write(&path, "console.log('cleanup test')").expect("write test file");
        assert!(path.exists());
        assert!(cleanup_tab_ai_temp_script(path.to_str().expect("utf8")));
        assert!(!path.exists());
    }

    #[test]
    fn tab_ai_memory_entry_serde_roundtrip() {
        let entry = TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "copy url".to_string(),
            generated_source: "await copy(browser.url)".to_string(),
            slug: "copy-url".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.google.Chrome".to_string()),
            written_at: "2026-03-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: TabAiMemoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, entry);
    }

    #[test]
    fn tab_ai_memory_entry_omits_none_bundle_id() {
        let entry = TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "test".to_string(),
            generated_source: "code".to_string(),
            slug: "test".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: None,
            written_at: "2026-03-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(!json.contains("bundleId"));
    }
}

#[cfg(test)]
mod tab_ai_memory_suggestion_tests {
    use super::*;

    fn memory_entry(
        intent: &str,
        bundle_id: Option<&str>,
        slug: &str,
        written_at: &str,
    ) -> TabAiMemoryEntry {
        TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: intent.to_string(),
            generated_source: "import \"@scriptkit/sdk\";\nawait hide();\n".to_string(),
            slug: slug.to_string(),
            prompt_type: "AppLauncher".to_string(),
            bundle_id: bundle_id.map(str::to_string),
            written_at: written_at.to_string(),
        }
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_returns_similar_non_exact_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![memory_entry(
            "force quit current app",
            Some("com.apple.Safari"),
            "force-quit-current-app",
            "2026-03-28T00:00:00Z",
        )];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "force quit app",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].slug, "force-quit-current-app");
        assert_eq!(results[0].effective_query, "force quit current app");
        assert!(results[0].score >= TAB_AI_MEMORY_SUGGESTION_MIN_SCORE);
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_filters_by_bundle_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![
            memory_entry(
                "copy browser url",
                Some("com.apple.Safari"),
                "copy-browser-url",
                "2026-03-28T00:00:00Z",
            ),
            memory_entry(
                "copy browser url",
                Some("com.tinyspeck.slackmacgap"),
                "copy-browser-url-slack",
                "2026-03-28T00:00:01Z",
            ),
        ];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "copy url",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].slug, "copy-browser-url");
        assert_eq!(results[0].bundle_id, "com.apple.Safari");
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_prefers_exact_match_then_recency() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![
            memory_entry(
                "force quit current app",
                Some("com.apple.Safari"),
                "older-similar",
                "2026-03-28T00:00:00Z",
            ),
            memory_entry(
                "force quit app",
                Some("com.apple.Safari"),
                "exact-match",
                "2026-03-28T00:00:01Z",
            ),
        ];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "force quit app",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].slug, "exact-match");
        assert!(results[0].score >= results[1].score);
    }
}

#[cfg(test)]
mod tab_ai_memory_resolution_tests {
    use super::*;

    #[test]
    fn tab_ai_memory_resolution_reports_missing_bundle_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit slack",
            None,
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::MissingBundleId
        );
        assert_eq!(resolution.outcome.candidate_count, 0);
        assert_eq!(resolution.outcome.match_count, 0);
        assert!(resolution.outcome.top_score.is_none());
        assert!(resolution.outcome.matched_slugs.is_empty());
    }

    #[test]
    fn tab_ai_memory_resolution_reports_empty_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "   ",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::EmptyQuery
        );
    }

    #[test]
    fn tab_ai_memory_resolution_reports_zero_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit",
            Some("com.tinyspeck.slackmacgap"),
            0,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::ZeroLimit
        );
    }

    #[test]
    fn tab_ai_memory_resolution_reports_index_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit slack",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::IndexMissing
        );
        assert!(resolution.outcome.index_path.contains("missing.json"));
    }

    #[test]
    fn tab_ai_memory_resolution_reports_no_candidates_for_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");
        let entries = vec![TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "force quit".to_string(),
            generated_source: "import \"@scriptkit/sdk\";\n".to_string(),
            slug: "force-quit".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.apple.Safari".to_string()),
            written_at: "2026-03-28T00:00:00Z".to_string(),
        }];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::NoCandidatesForBundle
        );
        assert_eq!(resolution.outcome.candidate_count, 0);
    }

    #[test]
    fn tab_ai_memory_resolution_prefers_recent_high_score_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        // Both intents share enough tokens with the query "force quit app"
        // to score above the 0.35 threshold.
        let older = TabAiExecutionRecord::from_parts(
            "force quit current app".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"old\");\n".to_string(),
            "/tmp/old.ts".to_string(),
            "force-quit-old".to_string(),
            "ScriptList".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let newer = TabAiExecutionRecord::from_parts(
            "force quit app".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"new\");\n".to_string(),
            "/tmp/new.ts".to_string(),
            "force-quit-new".to_string(),
            "ScriptList".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T01:00:00Z".to_string(),
        );

        write_tab_ai_memory_entry_to_path(&older, &path).expect("write older");
        write_tab_ai_memory_entry_to_path(&newer, &path).expect("write newer");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit app",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::Matched
        );
        assert_eq!(resolution.outcome.match_count, 2);
        assert_eq!(resolution.outcome.top_score, Some(1.0));
        // Exact match "force quit app" scores 1.0, should be first
        assert_eq!(
            resolution.suggestions.first().map(|s| s.slug.as_str()),
            Some("force-quit-new")
        );
        assert_eq!(resolution.outcome.candidate_count, 2);
        assert!(!resolution.outcome.matched_slugs.is_empty());
    }

    #[test]
    fn tab_ai_memory_write_dedupes_same_intent_and_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let first = TabAiExecutionRecord::from_parts(
            "copy url".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"a\");\n".to_string(),
            "/tmp/one.ts".to_string(),
            "copy-url-one".to_string(),
            "ScriptList".to_string(),
            Some("com.google.Chrome".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let second = TabAiExecutionRecord::from_parts(
            "copy url".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"b\");\n".to_string(),
            "/tmp/two.ts".to_string(),
            "copy-url-two".to_string(),
            "ScriptList".to_string(),
            Some("com.google.Chrome".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T01:00:00Z".to_string(),
        );

        write_tab_ai_memory_entry_to_path(&first, &path).expect("write first");
        write_tab_ai_memory_entry_to_path(&second, &path).expect("write second");

        let entries = read_tab_ai_memory_index_from_path(&path).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].slug, "copy-url-two");
    }
}

#[cfg(test)]
mod tab_ai_entry_resolution_tests {
    use super::*;

    #[test]
    fn empty_entry_query_uses_recent_bundle_automations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "rename this file".to_string(),
                generated_source: "code".to_string(),
                slug: "older".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T10:00:00Z".to_string(),
            },
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "summarize this file".to_string(),
                generated_source: "code".to_string(),
                slug: "newer".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T11:00:00Z".to_string(),
            },
        ];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "",
            Some("com.apple.finder"),
            2,
            &path,
        )
        .expect("resolve");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].slug, "newer");
        assert_eq!(items[1].slug, "older");
    }

    #[test]
    fn non_empty_entry_query_uses_fuzzy_matching() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "rename this file".to_string(),
                generated_source: "code".to_string(),
                slug: "rename-entry".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T10:00:00Z".to_string(),
            },
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "summarize this file".to_string(),
                generated_source: "code".to_string(),
                slug: "summarize-entry".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T11:00:00Z".to_string(),
            },
        ];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        // Non-empty query should use fuzzy matching, not recent-bundle fallback
        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "rename",
            Some("com.apple.finder"),
            2,
            &path,
        )
        .expect("resolve");

        // Should match the rename entry via query matching
        assert!(
            items.iter().any(|item| item.slug == "rename-entry"),
            "expected rename-entry in results: {items:?}"
        );
    }

    #[test]
    fn whitespace_only_query_treated_as_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "open terminal".to_string(),
            generated_source: "code".to_string(),
            slug: "open-term".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.apple.Terminal".to_string()),
            written_at: "2026-03-29T12:00:00Z".to_string(),
        }];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "   ",
            Some("com.apple.Terminal"),
            5,
            &path,
        )
        .expect("resolve");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "open-term");
    }
}

#[cfg(test)]
mod tab_ai_invocation_receipt_tests {
    use super::*;

    fn receipt(
        prompt_type: &str,
        input_text: Option<&str>,
        focused_id: Option<&str>,
        selected_id: Option<&str>,
        element_count: usize,
        warnings: &[&str],
    ) -> TabAiInvocationReceipt {
        let input_text = input_text.map(ToString::to_string);
        let focused_id = focused_id.map(ToString::to_string);
        let selected_id = selected_id.map(ToString::to_string);
        let warnings = warnings
            .iter()
            .map(|w| (*w).to_string())
            .collect::<Vec<_>>();
        TabAiInvocationReceipt::from_snapshot(
            prompt_type,
            &input_text,
            &focused_id,
            &selected_id,
            element_count,
            &warnings,
        )
    }

    #[test]
    fn script_list_with_empty_filter_is_still_rich_when_focus_and_elements_exist() {
        let r = receipt(
            "ScriptList",
            None,
            Some("input:filter"),
            Some("choice:0:slack"),
            3,
            &[],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert_eq!(r.focus_status, TabAiFieldStatus::Captured);
        assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
        assert!(!r.has_input_text);
        assert!(r.has_focus_target);
        assert!(r.degradation_reasons.is_empty());
        assert!(r.rich);
    }

    #[test]
    fn term_prompt_without_linear_text_is_degraded() {
        let r = receipt(
            "TermPrompt",
            None,
            None,
            None,
            1,
            &["panel_only_term_prompt"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotExtractable));
        assert!(!r.rich);
    }

    #[test]
    fn current_view_fallback_is_never_reported_as_captured_elements() {
        let r = receipt(
            "SearchAiPresets",
            Some("claude"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.focus_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::MissingFocusTarget));
        assert!(!r.rich);
    }

    #[test]
    fn settings_surface_reports_input_not_applicable() {
        let r = receipt(
            "Settings",
            None,
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotApplicable));
        assert!(!r.rich);
    }

    #[test]
    fn receipt_serializes_machine_readable_statuses() {
        let r = receipt(
            "SearchAiPresets",
            Some("claude"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        let json = serde_json::to_string(&r).expect("receipt should serialize");
        assert!(json.contains("\"inputStatus\":\"captured\""));
        assert!(json.contains("\"elementsStatus\":\"degraded\""));
        assert!(json
            .contains("\"degradationReasons\":[\"collector_fallback\",\"missing_focus_target\"]"));
    }

    #[test]
    fn receipt_marks_empty_script_list_input_as_captured() {
        let r = receipt(
            "ScriptList",
            None,
            Some("input:filter"),
            Some("choice:0:slack"),
            3,
            &[],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert!(!r.has_input_text);
        assert!(r.rich);
        assert!(r.degradation_reasons.is_empty());
    }

    #[test]
    fn receipt_marks_quick_terminal_missing_input_as_degraded() {
        let r = receipt(
            "QuickTerminal",
            None,
            None,
            None,
            1,
            &["panel_only_quick_terminal"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.focus_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotExtractable));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::PanelOnlyElements));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::MissingFocusTarget));
        assert!(!r.rich);
    }

    #[test]
    fn receipt_marks_actions_dialog_input_as_unavailable() {
        let r = receipt(
            "ActionsDialog",
            None,
            None,
            None,
            1,
            &["panel_only_actions_dialog"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotApplicable));
    }

    #[test]
    fn receipt_marks_collector_fallback_explicitly() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
    }

    #[test]
    fn receipt_marks_collector_fallback_degraded_even_with_zero_elements() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            0,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(
            r.elements_status,
            TabAiFieldStatus::Degraded,
            "warnings should win over element_count==0"
        );
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::NoSemanticElements));
    }

    #[test]
    fn receipt_emits_both_panel_only_and_collector_fallback_independently() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            1,
            &[
                "panel_only_future_prompt",
                "collector_used_current_view_fallback",
            ],
        );
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::PanelOnlyElements));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
    }
}

#[cfg(test)]
mod target_context_tests {
    use super::*;

    #[test]
    fn tab_ai_context_blob_serializes_focused_target() {
        let blob = TabAiContextBlob::from_parts_with_targets(
            TabAiUiSnapshot {
                prompt_type: "FileSearch".to_string(),
                selected_semantic_id: Some("choice:0:report.md".to_string()),
                ..Default::default()
            },
            Some(TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:report.md".to_string(),
                label: "report.md".to_string(),
                metadata: Some(serde_json::json!({
                    "path": "/tmp/report.md",
                    "fileType": "File"
                })),
            }),
            vec![],
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T13:37:35Z".to_string(),
        );
        let json = serde_json::to_value(&blob).expect("serialize context blob");
        assert_eq!(json["focusedTarget"]["kind"], "file");
        assert_eq!(json["focusedTarget"]["metadata"]["path"], "/tmp/report.md");
    }

    #[test]
    fn tab_ai_user_prompt_teaches_model_not_to_guess_target() {
        let prompt = build_tab_ai_user_prompt(
            "rename to kebab-case",
            r#"{"focusedTarget":null,"ui":{"promptType":"ScriptList"}}"#,
        );
        assert!(prompt.contains("focusedTarget is the default subject"));
        assert!(prompt.contains("do not invent an implicit subject"));
    }

    #[test]
    fn implicit_target_detection_avoids_false_positive_on_split() {
        assert!(tab_ai_intent_uses_implicit_target(
            "rename this to kebab-case"
        ));
        assert!(tab_ai_intent_uses_implicit_target("rename to kebab-case"));
        assert!(!tab_ai_intent_uses_implicit_target(
            "rename report.md to kebab-case"
        ));
        assert!(!tab_ai_intent_uses_implicit_target("split lines"));
        assert!(!tab_ai_intent_uses_implicit_target("copy url"));
    }

    #[test]
    fn from_parts_without_targets_leaves_focused_target_none() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot::default(),
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(blob.focused_target.is_none());
        assert!(blob.visible_targets.is_empty());
    }

    #[test]
    fn focused_target_serializes_camel_case() {
        let target = TabAiTargetContext {
            source: "ClipboardHistory".to_string(),
            kind: "clipboard_entry".to_string(),
            semantic_id: "choice:2:link".to_string(),
            label: "https://example.com".to_string(),
            metadata: Some(serde_json::json!({"contentType": "text"})),
        };
        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["semanticId"], "choice:2:link");
        assert_eq!(json["metadata"]["contentType"], "text");
    }

    #[test]
    fn implicit_target_detection_recognizes_all_pronouns() {
        assert!(tab_ai_intent_uses_implicit_target("open it"));
        assert!(tab_ai_intent_uses_implicit_target("paste that here"));
        assert!(tab_ai_intent_uses_implicit_target("copy selected text"));
        assert!(tab_ai_intent_uses_implicit_target("show current status"));
        assert!(tab_ai_intent_uses_implicit_target("close focused window"));
        assert!(tab_ai_intent_uses_implicit_target("force quit"));
    }

    #[test]
    fn visible_targets_omitted_when_empty() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot::default(),
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");
        assert!(!json.contains("visibleTargets"));
        assert!(!json.contains("focusedTarget"));
    }
}

#[cfg(test)]
mod target_audit_tests {
    use super::*;

    fn sample_focused_target() -> TabAiTargetContext {
        TabAiTargetContext {
            source: "FileSearch".to_string(),
            kind: "file".to_string(),
            semantic_id: "choice:0:report.md".to_string(),
            label: "report.md".to_string(),
            metadata: Some(serde_json::json!({"path": "/tmp/report.md"})),
        }
    }

    fn sample_visible_targets() -> Vec<TabAiTargetContext> {
        vec![
            TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:report.md".to_string(),
                label: "report.md".to_string(),
                metadata: None,
            },
            TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "directory".to_string(),
                semantic_id: "choice:1:src".to_string(),
                label: "src".to_string(),
                metadata: None,
            },
        ]
    }

    #[test]
    fn target_audit_schema_version_is_one() {
        assert_eq!(TAB_AI_TARGET_AUDIT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn target_audit_from_focused_target_captures_fields() {
        let focused = sample_focused_target();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &[]);
        assert!(audit.has_focused_target);
        assert_eq!(audit.visible_target_count, 0);
        assert_eq!(audit.focused_source.as_deref(), Some("FileSearch"));
        assert_eq!(audit.focused_kind.as_deref(), Some("file"));
        assert_eq!(
            audit.focused_semantic_id.as_deref(),
            Some("choice:0:report.md")
        );
        assert!(audit.visible_kinds.is_empty());
    }

    #[test]
    fn target_audit_from_visible_targets_deduplicates_kinds() {
        let visible = sample_visible_targets();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &None, &visible);
        assert!(!audit.has_focused_target);
        assert_eq!(audit.visible_target_count, 2);
        assert!(audit.focused_source.is_none());
        assert!(audit.focused_kind.is_none());
        assert!(audit.focused_semantic_id.is_none());
        assert_eq!(audit.visible_kinds, vec!["directory", "file"]);
    }

    #[test]
    fn target_audit_serializes_camel_case() {
        let focused = sample_focused_target();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &[]);
        let json = serde_json::to_string(&audit).expect("serialize");

        assert!(json.contains("schemaVersion"));
        assert!(json.contains("promptType"));
        assert!(json.contains("hasFocusedTarget"));
        assert!(json.contains("visibleTargetCount"));
        assert!(json.contains("focusedSource"));
        assert!(json.contains("focusedKind"));
        assert!(json.contains("focusedSemanticId"));

        // Snake case must not appear
        assert!(!json.contains("schema_version"));
        assert!(!json.contains("prompt_type"));
        assert!(!json.contains("has_focused_target"));
        assert!(!json.contains("visible_target_count"));
        assert!(!json.contains("focused_source"));
        assert!(!json.contains("focused_kind"));
        assert!(!json.contains("focused_semantic_id"));
    }

    #[test]
    fn target_audit_roundtrip_preserves_all_fields() {
        let focused = sample_focused_target();
        let visible = sample_visible_targets();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &visible);
        let json = serde_json::to_string(&audit).expect("serialize");
        let parsed: TabAiTargetAudit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, audit);
    }

    #[test]
    fn target_audit_omits_empty_optional_fields() {
        let audit = TabAiTargetAudit::from_targets("ScriptList", &None, &[]);
        let json = serde_json::to_string(&audit).expect("serialize");
        assert!(!json.contains("focusedSource"));
        assert!(!json.contains("focusedKind"));
        assert!(!json.contains("focusedSemanticId"));
        assert!(!json.contains("visibleKinds"));
    }

    #[test]
    fn target_audit_fails_deserialization_without_required_fields() {
        // Missing hasFocusedTarget — required field
        let json = r#"{"schemaVersion":1,"promptType":"X","visibleTargetCount":0}"#;
        let result = serde_json::from_str::<TabAiTargetAudit>(json);
        assert!(result.is_err(), "should fail without hasFocusedTarget");
    }
}

#[cfg(test)]
mod tab_ai_source_type_tests {
    use super::*;

    #[test]
    fn script_list_origin_beats_desktop_selection() {
        // When the user is on ScriptList with a focused target AND desktop
        // happens to have selected text, the Script Kit origin surface wins.
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        let target = TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, Some(&target)),
            Some(TabAiSourceType::ScriptListItem),
            "ScriptList with focused target should beat incidental desktop selection"
        );
    }

    #[test]
    fn desktop_selection_wins_when_no_stronger_origin() {
        // Without a Script Kit-specific origin surface, desktop selection applies.
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
            Some(TabAiSourceType::DesktopSelection)
        );
    }

    #[test]
    fn script_list_requires_focused_target() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
            Some(TabAiSourceType::Desktop),
            "ScriptList without a focused target should fall through to Desktop"
        );

        let target = TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, Some(&target)),
            Some(TabAiSourceType::ScriptListItem)
        );
    }

    #[test]
    fn clipboard_history_maps_to_clipboard_entry() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ClipboardHistory", &desktop, None),
            Some(TabAiSourceType::ClipboardEntry)
        );
    }

    #[test]
    fn prompt_surfaces_map_to_running_command() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        for prompt_type in &[
            "ArgPrompt",
            "MiniPrompt",
            "MicroPrompt",
            "DivPrompt",
            "FormPrompt",
            "EditorPrompt",
            "SelectPrompt",
            "PathPrompt",
            "DropPrompt",
            "TemplatePrompt",
            "HotkeyPrompt",
            "TermPrompt",
            "EnvPrompt",
            "ChatPrompt",
            "NamingPrompt",
        ] {
            assert_eq!(
                detect_tab_ai_source_type_from_prompt(prompt_type, &desktop, None),
                Some(TabAiSourceType::RunningCommand),
                "{prompt_type} should map to RunningCommand"
            );
        }
    }

    #[test]
    fn unknown_surface_falls_through_to_desktop() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("SomeOtherView", &desktop, None),
            Some(TabAiSourceType::Desktop)
        );
    }

    #[test]
    fn empty_or_whitespace_selected_text_does_not_trigger_desktop_selection() {
        for text in &["", "   ", "\n\t  "] {
            let desktop = crate::context_snapshot::AiContextSnapshot {
                selected_text: Some(text.to_string()),
                ..Default::default()
            };
            assert_ne!(
                detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
                Some(TabAiSourceType::DesktopSelection),
                "whitespace-only selected_text should not trigger DesktopSelection"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Source-text contract tests: verify structural invariants of agent_handoff.rs
    // -----------------------------------------------------------------------

    const TAB_AI_MODE_SRC: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/app_impl/agent_handoff/mod.rs"
    ));

    #[test]
    fn quick_terminal_switch_and_notify_happen_before_capture_await() {
        let open_idx = TAB_AI_MODE_SRC
            .find("self.current_view = AppView::QuickTerminalView")
            .expect("quick terminal view switch");
        let notify_idx = TAB_AI_MODE_SRC[open_idx..]
            .find("cx.notify();")
            .map(|idx| open_idx + idx)
            .expect("notify after view switch");
        // Search from the view switch onwards so we find the await in the
        // new-session path, not the live-session helper above it.
        let await_idx = TAB_AI_MODE_SRC[open_idx..]
            .find("capture_rx.recv().await")
            .map(|idx| open_idx + idx)
            .expect("deferred capture await after view switch");
        assert!(
            open_idx < await_idx,
            "QuickTerminalView must be visible before deferred capture is awaited"
        );
        assert!(
            notify_idx < await_idx,
            "cx.notify() must happen before deferred capture is awaited"
        );
    }

    #[test]
    fn deferred_capture_is_started_before_harness_open_call() {
        let scoped_start = TAB_AI_MODE_SRC
            .find("let capture_rx = if use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {")
            .expect("capture staging block");
        let scoped = &TAB_AI_MODE_SRC[scoped_start..];
        let spawn_idx = scoped
            .find("self.spawn_tab_ai_pre_switch_capture(&request)")
            .expect("capture spawn");
        let open_idx = scoped
            .find("self.open_tab_ai_agent_chat_view_from_request_impl(")
            .expect("harness open");
        assert!(
            spawn_idx < open_idx,
            "capture must be started before the harness open call"
        );
    }

    #[test]
    fn pre_switch_capture_uses_immediate_thread_spawn() {
        let fn_start = TAB_AI_MODE_SRC
            .find("fn spawn_tab_ai_pre_switch_capture(")
            .expect("function start");
        let fn_end = TAB_AI_MODE_SRC[fn_start..]
            .find("fn open_tab_ai_harness_terminal_from_request(")
            .map(|idx| fn_start + idx)
            .expect("next function");
        let body = &TAB_AI_MODE_SRC[fn_start..fn_end];
        assert!(
            body.contains("std::thread::spawn(move ||"),
            "capture must start immediately on its own thread"
        );
        assert!(
            !body.contains("cx.background_executor().spawn(async move {"),
            "do not add an extra scheduler hop before desktop capture begins"
        );
    }

    #[test]
    fn apply_back_hint_matches_source_type() {
        let cases = [
            (TabAiSourceType::DesktopSelection, "replaceSelectedText"),
            (TabAiSourceType::ScriptListItem, "runGeneratedScript"),
            (TabAiSourceType::RunningCommand, "pasteToPrompt"),
            (TabAiSourceType::ClipboardEntry, "copyToClipboard"),
            (TabAiSourceType::Desktop, "pasteToFrontmostApp"),
        ];
        for (source_type, expected_action) in &cases {
            let hint = build_tab_ai_apply_back_hint_from_source(Some(source_type))
                .expect("should produce a hint");
            assert_eq!(
                hint.action, *expected_action,
                "wrong action for {source_type:?}"
            );
            assert!(
                hint.target_label.is_some(),
                "target_label should be set for {source_type:?}"
            );
        }
        assert!(build_tab_ai_apply_back_hint_from_source(None).is_none());
    }

    #[test]
    fn apply_back_footer_label_matches_source_type() {
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::RunningCommand)),
            "Paste Back to Prompt"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::ClipboardEntry)),
            "Copy Result"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::ScriptListItem)),
            "Save as Script & Run"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::DesktopSelection)),
            "Replace Selection"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::Desktop)),
            "Paste Back to App"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(None),
            "Preparing Paste Back\u{2026}"
        );
    }
}

#[cfg(test)]
mod tab_ai_apply_back_route_tests {
    use super::*;

    #[test]
    fn apply_back_route_serde_roundtrip() {
        let route = TabAiApplyBackRoute {
            source_type: TabAiSourceType::DesktopSelection,
            hint: TabAiApplyBackHint {
                action: "replaceSelectedText".to_string(),
                target_label: Some("Frontmost selection".to_string()),
            },
            focused_target: None,
        };
        let json = serde_json::to_string(&route).expect("serialize");
        let back: TabAiApplyBackRoute = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(route, back);

        // Route with focused_target round-trips correctly
        let route_with_target = TabAiApplyBackRoute {
            source_type: TabAiSourceType::ScriptListItem,
            hint: TabAiApplyBackHint {
                action: "runGeneratedScript".to_string(),
                target_label: Some("Focused script".to_string()),
            },
            focused_target: Some(TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:my-script".to_string(),
                label: "My Script".to_string(),
                metadata: Some(serde_json::json!({"path": "/scripts/my-script.ts"})),
            }),
        };
        let json2 = serde_json::to_string(&route_with_target).expect("serialize with target");
        let back2: TabAiApplyBackRoute =
            serde_json::from_str(&json2).expect("deserialize with target");
        assert_eq!(route_with_target, back2);
        assert!(
            json2.contains("focusedTarget"),
            "focusedTarget must appear when Some"
        );
    }

    #[test]
    fn tab_ai_harness_tracks_apply_back_route_state() {
        let source =
            std::fs::read_to_string("src/main_sections/app_state.rs").expect("read app_state.rs");
        assert!(
            source.contains("tab_ai_harness_apply_back_route"),
            "ScriptListApp must persist apply-back routing state for the active harness session"
        );
    }

    #[test]
    fn quick_terminal_cmd_enter_routes_to_apply_back() {
        // Cmd+Enter now goes through the shared dispatcher, which routes to
        // the de-raced apply-back helper (after the portal-attach check).
        let source = std::fs::read_to_string("src/render_prompts/term.rs").expect("read term.rs");
        assert!(
            source.contains("this.dispatch_quick_terminal_cmd_enter(entity.clone(), cx);"),
            "QuickTerminalView must route Cmd+Enter through the shared dispatcher"
        );
        let dispatcher = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff/mod.rs");
        let start = dispatcher
            .find("fn dispatch_quick_terminal_cmd_enter")
            .expect("dispatch_quick_terminal_cmd_enter should exist");
        let body = &dispatcher[start..(start + 2000).min(dispatcher.len())];
        assert!(
            body.contains("self.apply_tab_ai_result_from_terminal(entity, cx);"),
            "dispatcher must route Cmd+Enter through the de-raced apply-back helper"
        );
    }

    #[test]
    fn tab_ai_apply_back_uses_running_command_prompt_reinjection() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/apply_back.rs")
            .expect("read apply_back.rs");
        assert!(
            source.contains("self.try_set_prompt_input(text.clone(), cx)"),
            "RunningCommand apply-back must reuse try_set_prompt_input"
        );
    }

    #[test]
    fn tab_ai_frontmost_apply_back_hides_before_paste() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/apply_back.rs")
            .expect("read apply_back.rs");
        let hide_pos = source
            .find("crate::platform::defer_hide_main_window(cx)")
            .expect("apply-back must defer-hide the main window");
        let replace_pos = source
            .find("selected_text::set_selected_text(&text_for_apply)")
            .expect("apply-back must support selected-text replacement");
        let paste_pos = source
            .find(".paste_text(&text_for_apply)")
            .expect("apply-back must support frontmost-app paste");
        assert!(
            hide_pos < replace_pos,
            "main window must hide before set_selected_text fires"
        );
        assert!(
            hide_pos < paste_pos,
            "main window must hide before TextInjector::paste_text fires"
        );
    }

    #[test]
    fn tab_ai_apply_back_route_cleared_on_close() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff.rs");
        let close_fn_pos = source
            .find("fn close_tab_ai_harness_terminal")
            .expect("close_tab_ai_harness_terminal must exist");
        let clear_pos = source[close_fn_pos..]
            .find("self.tab_ai_harness_apply_back_route = None")
            .expect("close must clear apply-back route");
        let slice = &source[close_fn_pos..close_fn_pos + clear_pos];
        let lines_between = slice.lines().count();
        assert!(
            lines_between < 60,
            "route clear should be near the top of close_tab_ai_harness_terminal, found at line offset {lines_between}"
        );
    }
}
