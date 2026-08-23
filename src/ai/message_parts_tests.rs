#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_context_fingerprint_uses_ephemeral_key_not_predictable_public_hash() {
        use sha2::Digest as _;

        let secret = "private attached document and hidden model context";
        let actual = run_scoped_fingerprint(secret);

        assert_eq!(
            actual,
            crate::logging::log_private_user_value(secret).sha256
        );
        assert_eq!(actual, run_scoped_fingerprint(secret));
        assert_ne!(
            actual,
            run_scoped_fingerprint("a different private context")
        );
        assert_ne!(
            actual,
            format!("{:x}", sha2::Sha256::digest(secret.as_bytes()))
        );
    }

    #[test]
    fn private_context_receipt_fingerprints_actual_prepared_prompt_with_shared_key() {
        use sha2::Digest as _;

        let secret = "sensitive attached document content";
        let items = [ContextPreparationItem::primary(AiContextPart::TextBlock {
            label: "Private notes".to_string(),
            source: "synthetic://private".to_string(),
            text: secret.to_string(),
            mime_type: Some("text/plain".to_string()),
        })];
        let prepared = prepare_user_message("", &items, &[], &[]);
        let actual = prepared.receipt.outcomes[0]
            .content_fingerprint
            .as_deref()
            .expect("successful real context preparation has a private fingerprint");

        assert_eq!(
            actual,
            crate::logging::log_private_user_value(&prepared.final_user_content).sha256
        );
        assert_ne!(
            actual,
            format!(
                "{:x}",
                sha2::Sha256::digest(prepared.final_user_content.as_bytes())
            )
        );
        assert!(!serde_json::to_string(&prepared.receipt)
            .unwrap()
            .contains(secret));
    }

    #[test]
    fn semantic_chip_projection_is_stable_redacted_and_capability_neutral() {
        let a = AiContextPart::TextBlock {
            label: "Terminal output".to_string(),
            source: "terminal://session/42".to_string(),
            text: "secret first body".to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        let b = AiContextPart::TextBlock {
            label: "Terminal output".to_string(),
            source: "terminal://session/42".to_string(),
            text: "different secret body".to_string(),
            mime_type: Some("application/json".to_string()),
        };
        let removable = a.semantic_chip_projection(true);
        let retained = b.semantic_chip_projection(false);

        assert_eq!(removable.semantic_id, retained.semantic_id);
        assert_eq!(removable.label, "Terminal output");
        assert!(removable.removable);
        assert!(!retained.removable);
        assert!(!removable.semantic_id.contains("secret"));
        assert!(!removable.semantic_id.contains("Terminal output"));
        assert!(removable
            .semantic_id
            .starts_with("agent-chat-context-text-"));
    }

    #[test]
    fn semantic_chip_projection_changes_with_part_identity_not_content() {
        let first = AiContextPart::FilePath {
            path: "/tmp/one.txt".to_string(),
            label: "Notes".to_string(),
        };
        let same = first.clone();
        let other = AiContextPart::FilePath {
            path: "/tmp/two.txt".to_string(),
            label: "Notes".to_string(),
        };
        assert_eq!(
            first.semantic_chip_projection(true).semantic_id,
            same.semantic_chip_projection(true).semantic_id
        );
        assert_ne!(
            first.semantic_chip_projection(true).semantic_id,
            other.semantic_chip_projection(true).semantic_id
        );
    }

    #[test]
    fn test_serde_roundtrip_resource_uri() {
        let part = AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Current Context".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"resourceUri\""));
    }

    #[test]
    fn test_serde_roundtrip_file_path() {
        let part = AiContextPart::FilePath {
            path: "/tmp/test.rs".to_string(),
            label: "test.rs".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"filePath\""));
    }

    #[test]
    fn test_label_accessor() {
        let uri_part = AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        };
        assert_eq!(uri_part.label(), "Context");

        let file_part = AiContextPart::FilePath {
            path: "/tmp/foo.rs".to_string(),
            label: "foo.rs".to_string(),
        };
        assert_eq!(file_part.label(), "foo.rs");
    }

    #[test]
    fn test_resolve_readable_file_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "Hello, world!").expect("write temp file");

        let part = AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "hello.txt".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("<attachment path=\""));
        assert!(block.contains("Hello, world!"));
        assert!(block.contains("</attachment>"));
        assert!(!block.contains("unreadable"));
    }

    #[test]
    fn test_resolve_skill_file_path_builds_staged_skill_prompt() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("SKILL.md");
        std::fs::write(&file_path, "# Review\nReview the current diff.").expect("write temp file");

        let part = AiContextPart::SkillFile {
            path: file_path.to_string_lossy().to_string(),
            label: "/review".to_string(),
            skill_name: "Review".to_string(),
            owner_label: "Script Kit".to_string(),
            slash_name: "review".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("Use the attached skill \"Review\""));
        assert!(block.contains("from plugin \"Script Kit\""));
        assert!(block.contains("<skill path=\""));
        assert!(block.contains("Review the current diff."));
        assert!(block.contains("</skill>"));
    }

    /// Regression: a `kit://context` resource once carried a 758KB base64
    /// screenshot into the prompt text and overflowed the model's context.
    #[test]
    fn test_sanitize_resource_text_strips_base64_payloads_from_json() {
        let big = "A".repeat(200_000);
        let json = format!(
            "{{\"focusedWindowImage\":{{\"mimeType\":\"image/png\",\"base64Data\":\"{big}\"}},\"selectedText\":\"hello\"}}"
        );
        let sanitized =
            sanitize_resource_text_for_prompt(&json, "application/json", "kit://context?test");
        assert!(!sanitized.contains(&big), "base64 payload must be stripped");
        assert!(sanitized.contains("[binary omitted: 200000 base64 chars]"));
        assert!(sanitized.contains("hello"), "non-binary fields survive");
        assert!(sanitized.chars().count() <= MAX_RESOURCE_PROMPT_CHARS + 100);
    }

    #[test]
    fn test_sanitize_resource_text_truncates_oversized_content() {
        let huge = "x".repeat(200_000);
        let sanitized = sanitize_resource_text_for_prompt(&huge, "text/plain", "kit://big");
        assert!(sanitized.chars().count() < huge.chars().count());
        assert!(sanitized.contains("[truncated: context attachment exceeded"));

        let small = "small content";
        assert_eq!(
            sanitize_resource_text_for_prompt(small, "application/json", "kit://small"),
            small,
            "content without base64 and under the ceiling passes through unchanged"
        );
    }

    #[test]
    fn test_resolve_unreadable_file_path_does_not_panic() {
        // Create a file, make it exist but unreadable by removing read permissions
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("binary.dat");
        std::fs::write(&file_path, vec![0u8; 64]).expect("write temp file");

        // On Unix, remove read permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000))
                .expect("set permissions");
        }

        let part = AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "binary.dat".to_string(),
        };

        // On unix, this should produce an unreadable fallback (metadata-only)
        #[cfg(unix)]
        {
            let block = resolve_context_part_to_prompt_block(&part, &[], &[])
                .expect("resolve should not panic");
            assert!(block.contains("unreadable=\"true\""));
            assert!(block.contains("bytes=\"64\""));
        }

        // On non-unix, file is readable, so just verify no panic
        #[cfg(not(unix))]
        {
            let _ = resolve_context_part_to_prompt_block(&part, &[], &[]);
        }

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644));
        }
    }

    #[test]
    fn test_resolve_nonexistent_file_returns_error() {
        let part = AiContextPart::FilePath {
            path: "/nonexistent/path/that/does/not/exist.txt".to_string(),
            label: "ghost.txt".to_string(),
        };

        let result = resolve_context_part_to_prompt_block(&part, &[], &[]);
        assert!(result.is_err(), "nonexistent file should error");
    }

    #[test]
    fn test_resolve_multiple_parts() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file1 = dir.path().join("a.txt");
        let file2 = dir.path().join("b.txt");
        std::fs::write(&file1, "content A").expect("write");
        std::fs::write(&file2, "content B").expect("write");

        let parts = vec![
            AiContextPart::FilePath {
                path: file1.to_string_lossy().to_string(),
                label: "a.txt".to_string(),
            },
            AiContextPart::FilePath {
                path: file2.to_string_lossy().to_string(),
                label: "b.txt".to_string(),
            },
        ];

        let prefix =
            resolve_context_parts_to_prompt_prefix(&parts, &[], &[]).expect("resolve prefix");
        assert!(prefix.contains("content A"));
        assert!(prefix.contains("content B"));
        // Two blocks separated by double newline
        assert!(prefix.contains("</attachment>\n\n<attachment"));
    }

    #[test]
    fn test_resolve_empty_parts_returns_empty_string() {
        let prefix = resolve_context_parts_to_prompt_prefix(&[], &[], &[]).expect("resolve empty");
        assert!(prefix.is_empty());
    }

    // --- PreparedMessageReceipt tests ---

    #[test]
    fn test_prepare_user_message_no_parts_is_ready() {
        let receipt = prepare_user_message_with_receipt("hello", &[], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(
            receipt.schema_version,
            AI_MESSAGE_PREPARATION_SCHEMA_VERSION
        );
        assert_eq!(receipt.authored_content_chars, 5);
        assert_eq!(receipt.final_user_content, "hello");
        assert!(receipt.outcomes.is_empty());
        assert!(receipt.unresolved_parts().is_empty());
        assert!(receipt.user_error.is_none());
        assert!(receipt.can_send_message());
    }

    #[test]
    fn test_prepare_user_message_blocks_when_all_parts_fail() {
        let parts = vec![AiContextPart::FilePath {
            path: "/definitely/missing/file.txt".to_string(),
            label: "missing.txt".to_string(),
        }];

        let items = parts
            .iter()
            .cloned()
            .map(ContextPreparationItem::primary)
            .collect::<Vec<_>>();
        let receipt = prepare_user_message("hello", &items, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Blocked);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 0);
        assert_eq!(receipt.unresolved_parts(), parts);
        assert!(receipt.user_error.is_some());
        assert_eq!(receipt.outcomes.len(), 1);
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::Failed
        );
        assert!(!receipt.can_send_message());
    }

    #[test]
    fn test_prepare_user_message_marks_unreadable_file_as_metadata_only() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("binary.dat");
        std::fs::write(&file_path, vec![0u8; 64]).expect("write temp file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000))
                .expect("set permissions");
        }

        let parts = vec![AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "binary.dat".to_string(),
        }];

        let receipt = prepare_user_message_with_receipt("", &parts, &[], &[]);

        #[cfg(unix)]
        {
            assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
            assert_eq!(receipt.context.resolved, 1);
            assert!(receipt.context.failed == 0);
            assert_eq!(receipt.outcomes.len(), 1);
            assert_eq!(
                receipt.outcomes[0].kind,
                ContextPartPreparationOutcomeKind::MetadataOnly
            );
            assert!(receipt.final_user_content.contains("unreadable=\"true\""));
            assert!(receipt.can_send_message());
        }

        #[cfg(not(unix))]
        {
            assert_eq!(receipt.context.resolved, 1);
        }

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644));
        }
    }

    #[test]
    fn test_prepare_user_message_appends_prompt_prefix_before_raw_content() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("note.txt");
        std::fs::write(&file_path, "attached text").expect("write temp file");

        let parts = vec![AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "note.txt".to_string(),
        }];

        let receipt = prepare_user_message_with_receipt("user text", &parts, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert!(receipt.final_user_content.contains("attached text"));
        assert!(receipt.final_user_content.ends_with("user text"));
        assert_eq!(receipt.outcomes.len(), 1);
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::FullContent
        );
    }

    #[test]
    fn test_prepare_user_message_partial_when_mixed_success_failure() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let good_file = dir.path().join("good.txt");
        std::fs::write(&good_file, "good content").expect("write temp file");

        let parts = vec![
            AiContextPart::FilePath {
                path: good_file.to_string_lossy().to_string(),
                label: "good.txt".to_string(),
            },
            AiContextPart::FilePath {
                path: "/definitely/missing/bad.txt".to_string(),
                label: "bad.txt".to_string(),
            },
        ];

        let receipt = prepare_user_message_with_receipt("query", &parts, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Partial);
        assert_eq!(receipt.context.attempted, 2);
        assert_eq!(receipt.context.resolved, 1);
        assert_eq!(receipt.context.failed, 1);
        assert_eq!(receipt.unresolved_parts().len(), 1);
        assert!(receipt.final_user_content.contains("good content"));
        assert!(receipt.final_user_content.ends_with("query"));
        assert!(receipt.user_error.is_some());
        assert!(receipt.can_send_message());
    }

    #[test]
    fn merge_context_parts_deduplicates_and_preserves_order() {
        let selection = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0"
                    .to_string(),
            label: "Selection".to_string(),
        };
        let browser = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        };

        let merged = merge_context_parts(
            &[selection.clone(), browser.clone()],
            std::slice::from_ref(&selection),
        );

        assert_eq!(merged, vec![selection, browser]);
    }

    #[test]
    fn merge_context_parts_empty_inputs() {
        let merged = merge_context_parts(&[], &[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_context_parts_preserves_left_then_right_order() {
        let a = AiContextPart::FilePath {
            path: "/a.rs".to_string(),
            label: "a.rs".to_string(),
        };
        let b = AiContextPart::FilePath {
            path: "/b.rs".to_string(),
            label: "b.rs".to_string(),
        };
        let c = AiContextPart::FilePath {
            path: "/c.rs".to_string(),
            label: "c.rs".to_string(),
        };

        let merged = merge_context_parts(&[a.clone(), b.clone()], &[c.clone(), a.clone()]);
        assert_eq!(merged, vec![a, b, c]);
    }

    #[test]
    fn test_prepare_user_message_receipt_serde_roundtrip() {
        let receipt = PreparedMessageReceipt {
            schema_version: AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
            decision: PreparedMessageDecision::Ready,
            authored_content_chars: 5,
            final_content_chars: 13,
            context: ContextPreparationSummary {
                attempted: 1,
                resolved: 1,
                failed: 0,
                primary_failed: 0,
                supplemental_failed: 0,
            },
            assembly: None,
            outcomes: vec![ContextPartPreparationOutcome {
                part_id: "context-0000".to_string(),
                source_kind: ContextSourceKind::File,
                role: ContextPreparationRole::Supplemental,
                kind: ContextPartPreparationOutcomeKind::FullContent,
                content_chars: 6,
                content_fingerprint: Some("run-scoped-fingerprint".to_string()),
                failure_code: None,
                diagnostic_fingerprint: None,
            }],
            user_error: None,
        };

        let json = serde_json::to_string(&receipt).expect("serialize");
        let deserialized: PreparedMessageReceipt =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(receipt, deserialized);

        // Verify camelCase serde
        assert!(json.contains("\"schemaVersion\""));
        assert!(json.contains("\"authoredContentChars\""));
        assert!(json.contains("\"fullContent\""));
        assert!(!json.contains("rawContent"));
        assert!(!json.contains("finalUserContent"));
        assert!(!json.contains("promptPrefix"));
        assert!(!json.contains("/tmp/note.txt"));
    }

    #[test]
    fn merge_context_parts_with_receipt_reports_duplicate_provenance() {
        let selection = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0"
                    .to_string(),
            label: "Selection".to_string(),
        };
        let browser = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        };

        let receipt = merge_context_parts_with_receipt(
            &[selection.clone(), browser.clone()],
            std::slice::from_ref(&selection),
        );

        assert_eq!(receipt.merged_parts, vec![selection.clone(), browser]);
        assert_eq!(receipt.duplicates_removed, 1);
        assert_eq!(receipt.duplicates.len(), 1);
        assert_eq!(
            receipt.duplicates[0].kept_from,
            ContextAssemblyOrigin::Mention
        );
        assert_eq!(
            receipt.duplicates[0].dropped_from,
            ContextAssemblyOrigin::Pending
        );
        assert_eq!(receipt.duplicates[0].label, "Selection");
    }

    #[test]
    fn prepare_user_message_from_sources_with_receipt_attaches_assembly_receipt() {
        crate::context_snapshot::enable_deterministic_context_capture();
        let prepared = prepare_user_message_from_sources_with_receipt(
            "ship it",
            &[AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Current Context".to_string(),
            }],
            &[AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Current Context".to_string(),
            }],
            &[],
            &[],
        );

        assert!(prepared.can_send_message());
        let assembly = prepared
            .receipt
            .assembly
            .as_ref()
            .expect("assembly receipt must be present");
        assert_eq!(assembly.mention_count, 1);
        assert_eq!(assembly.pending_count, 1);
        assert_eq!(assembly.merged_count, 1);
        assert_eq!(assembly.duplicates_removed, 1);
    }

    #[test]
    fn current_context_selector_part_is_not_treated_as_ambient_bootstrap() {
        let part = AiContextPart::ResourceUri {
            uri: ASK_ANYTHING_RESOURCE_URI.to_string(),
            label: "Current Context".to_string(),
        };

        assert!(
            !part.is_ambient_bootstrap_resource(),
            "@context should resolve directly on submit instead of waiting on deferred capture"
        );
    }

    #[test]
    fn ask_anything_and_explicit_capture_labels_still_use_ambient_bootstrap() {
        for label in [
            ASK_ANYTHING_LABEL,
            "Full Screen",
            "Focused Window",
            "Selected Text",
            "Browser Tab",
        ] {
            let part = AiContextPart::ResourceUri {
                uri: ASK_ANYTHING_RESOURCE_URI.to_string(),
                label: label.to_string(),
            };
            assert!(
                part.is_ambient_bootstrap_resource(),
                "{label} should keep using deferred ambient capture"
            );
        }
    }

    #[test]
    fn test_serde_roundtrip_focused_target() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:main.rs".to_string(),
                label: "main.rs".to_string(),
                metadata: Some(serde_json::json!({ "path": "/tmp/main.rs" })),
            },
            label: "File: main.rs".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"focusedTarget\""));
        assert!(json.contains("\"semanticId\""));
    }

    #[test]
    fn test_focused_target_label_and_source() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:2:my-script".to_string(),
                label: "My Script".to_string(),
                metadata: None,
            },
            label: "Command: My Script".to_string(),
        };
        assert_eq!(part.label(), "Command: My Script");
        assert_eq!(part.source(), "choice:2:my-script");
    }

    #[test]
    fn test_resolve_focused_target_produces_context_block() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:agent_handoff.rs".to_string(),
                label: "agent_handoff.rs".to_string(),
                metadata: Some(serde_json::json!({ "path": "/tmp/agent_handoff.rs" })),
            },
            label: "File: agent_handoff.rs".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("source=\"focusedTarget\""));
        assert!(block.contains("itemSource=\"FileSearch\""));
        assert!(block.contains("itemKind=\"file\""));
        assert!(block.contains("semanticId=\"choice:0:agent_handoff.rs\""));
        assert!(block.contains("Label: File: agent_handoff.rs"));
        assert!(block.contains("/tmp/agent_handoff.rs"));
    }

    #[test]
    fn test_resolve_focused_target_no_metadata() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:hello".to_string(),
                label: "hello".to_string(),
                metadata: None,
            },
            label: "Command: hello".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("source=\"focusedTarget\""));
        assert!(block.contains("{}"), "empty metadata should be '{{}}'");
    }

    #[test]
    fn test_prepare_user_message_with_focused_target() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ClipboardHistory".to_string(),
                kind: "clipboard_entry".to_string(),
                semantic_id: "choice:0:clip".to_string(),
                label: "clip".to_string(),
                metadata: Some(serde_json::json!({ "contentType": "text/plain" })),
            },
            label: "Clipboard: clip".to_string(),
        };

        let receipt = prepare_user_message_with_receipt("explain this", &[part], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 1);
        assert!(receipt.final_user_content.contains("focusedTarget"));
        assert!(receipt.final_user_content.ends_with("explain this"));
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::FullContent
        );
    }

    #[test]
    fn test_prepare_user_message_with_ambient_context_is_display_only() {
        let part = AiContextPart::AmbientContext {
            label: ASK_ANYTHING_LABEL.to_string(),
        };

        let receipt = prepare_user_message_with_receipt("answer this", &[part], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 0);
        assert!(receipt.context.failed == 0);
        assert!(receipt.unresolved_parts.is_empty());
        assert_eq!(receipt.final_user_content, "answer this");
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::DisplayOnly
        );
    }

    #[test]
    fn context_assembly_receipt_serde_roundtrip() {
        let receipt = ContextAssemblyReceipt {
            mention_count: 2,
            pending_count: 1,
            merged_count: 2,
            duplicates_removed: 1,
            duplicates: vec![ContextAssemblyDuplicate {
                kept_from: ContextAssemblyOrigin::Mention,
                dropped_from: ContextAssemblyOrigin::Pending,
                label: "Selection".to_string(),
                source: "kit://context?selectedText=1".to_string(),
            }],
            merged_parts: vec![AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Context".to_string(),
            }],
        };

        let json = serde_json::to_string(&receipt).expect("serialize");
        let deserialized: ContextAssemblyReceipt =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(receipt, deserialized);
        assert!(json.contains("\"mentionCount\""));
        assert!(json.contains("\"pendingCount\""));
        assert!(json.contains("\"keptFrom\""));
        assert!(json.contains("\"droppedFrom\""));
    }

    #[test]
    fn canonical_sanitizer_strips_nested_base64_from_wrong_mime_text_block() {
        let canary = "BASE64_CANARY".repeat(20_000);
        let part = AiContextPart::TextBlock {
            label: "Logs".to_string(),
            source: "text://synthetic".to_string(),
            text: serde_json::json!({
                "outer": [{"inner": {"base64Data": canary, "keep": "nonbinary-survives"}}]
            })
            .to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared.final_user_content.contains("BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("nonbinary-survives"));
        assert!(prepared
            .final_user_content
            .contains("[binary omitted: 260000 base64 chars]"));
        assert!(prepared.final_user_content.chars().count() < 101_000);
    }

    #[test]
    fn canonical_sanitizer_strips_base64_when_text_block_mime_is_absent() {
        let part = AiContextPart::TextBlock {
            label: "Synthetic JSON".to_string(),
            source: "text://synthetic".to_string(),
            text: serde_json::json!({
                "base64Data": "MISSING_MIME_BASE64_CANARY",
                "keep": "still-here"
            })
            .to_string(),
            mime_type: None,
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared
            .final_user_content
            .contains("MISSING_MIME_BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("still-here"));
    }

    #[test]
    fn canonical_sanitizer_strips_base64_from_focused_metadata() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "File<Search".to_string(),
                kind: "file&row".to_string(),
                semantic_id: "choice:\"unsafe\"".to_string(),
                label: "Focused".to_string(),
                metadata: Some(serde_json::json!({
                    "nested": {"base64Data": "FOCUSED_BASE64_CANARY", "keep": 42}
                })),
            },
            label: "Focused item".to_string(),
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared
            .final_user_content
            .contains("FOCUSED_BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("\"keep\": 42"));
        assert!(prepared.final_user_content.contains("File&lt;Search"));
        assert!(prepared.final_user_content.contains("file&amp;row"));
        assert!(prepared
            .final_user_content
            .contains("choice:&quot;unsafe&quot;"));
    }

    #[test]
    fn wrapper_attributes_escape_xml_metacharacters() {
        let part = AiContextPart::TextBlock {
            label: "label<&\"'>".to_string(),
            source: "source<&\"'>".to_string(),
            text: "safe body".to_string(),
            mime_type: Some("text/<&\"'>".to_string()),
        };
        let block = resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve");
        assert!(block.contains("source=\"source&lt;&amp;&quot;&apos;&gt;\""));
        assert!(block.contains("label=\"label&lt;&amp;&quot;&apos;&gt;\""));
        assert!(block.contains("mimeType=\"text/&lt;&amp;&quot;&apos;&gt;\""));
    }

    #[test]
    fn primary_failure_blocks_while_supplemental_failure_can_be_partial() {
        let missing_primary = ContextPreparationItem::primary(AiContextPart::FilePath {
            path: "/missing/PRIMARY_PATH_CANARY".to_string(),
            label: "Primary".to_string(),
        });
        let blocked = prepare_user_message("authored", &[missing_primary], &[], &[]);
        assert_eq!(blocked.decision, PreparedMessageDecision::Blocked);
        assert_eq!(blocked.context.primary_failed, 1);
        assert!(!blocked.can_send_message());

        let missing_supplemental = ContextPreparationItem::supplemental(AiContextPart::FilePath {
            path: "/missing/SUPPLEMENTAL_PATH_CANARY".to_string(),
            label: "Supplemental".to_string(),
        });
        let partial = prepare_user_message("authored", &[missing_supplemental], &[], &[]);
        assert_eq!(partial.decision, PreparedMessageDecision::Partial);
        assert_eq!(partial.context.supplemental_failed, 1);
        assert!(partial.can_send_message());
        assert_eq!(partial.final_user_content, "authored");
    }

    #[test]
    fn valid_primary_plus_missing_supplemental_preserves_private_payload() {
        let good = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(good.path(), "PRIMARY_CONTENT_CANARY").expect("write");
        let items = vec![
            ContextPreparationItem::primary(AiContextPart::FilePath {
                path: good.path().to_string_lossy().to_string(),
                label: "Primary".to_string(),
            }),
            ContextPreparationItem::supplemental(AiContextPart::FilePath {
                path: "/missing/RAW_PATH_CANARY".to_string(),
                label: "Supplemental".to_string(),
            }),
        ];
        let prepared = prepare_user_message("", &items, &[], &[]);
        assert_eq!(prepared.decision, PreparedMessageDecision::Partial);
        assert!(prepared
            .final_user_content
            .contains("PRIMARY_CONTENT_CANARY"));
        let serialized = serde_json::to_string(&prepared.receipt).expect("serialize receipt");
        for canary in [
            "PRIMARY_CONTENT_CANARY",
            "RAW_PATH_CANARY",
            "rawContent",
            "finalUserContent",
            "promptPrefix",
            "metadata failed",
        ] {
            assert!(!serialized.contains(canary), "receipt leaked {canary}");
        }
        assert!(!format!("{prepared:?}").contains("PRIMARY_CONTENT_CANARY"));
    }

    #[test]
    fn legacy_v1_receipt_discards_content_bearing_fields() {
        let legacy = serde_json::json!({
            "schemaVersion": 1,
            "decision": "partial",
            "rawContent": "LEGACY_RAW_CANARY",
            "finalUserContent": "LEGACY_FINAL_CANARY",
            "context": {
                "attempted": 2,
                "resolved": 1,
                "failures": [{
                    "label": "LEGACY_LABEL_CANARY",
                    "source": "kit://URI_CANARY",
                    "error": "OS_ERROR_CANARY"
                }],
                "promptPrefix": "PROMPT_PREFIX_CANARY"
            },
            "outcomes": []
        });
        let loaded: PreparedMessageReceipt =
            serde_json::from_value(legacy).expect("load legacy receipt");
        let serialized = serde_json::to_string(&loaded).expect("serialize redacted receipt");
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.context.attempted, 2);
        assert_eq!(loaded.context.resolved, 1);
        for canary in [
            "LEGACY_RAW_CANARY",
            "LEGACY_FINAL_CANARY",
            "LEGACY_LABEL_CANARY",
            "URI_CANARY",
            "OS_ERROR_CANARY",
            "PROMPT_PREFIX_CANARY",
        ] {
            assert!(!serialized.contains(canary), "legacy load leaked {canary}");
        }
    }
}
