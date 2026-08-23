#[cfg(test)]
mod tests {
    use super::*;

    /// Per-process temp directory for test DB isolation.
    /// All tests in this binary share one temp DB (serialized by the global Mutex),
    /// but it is separate from the production DB and from other test binaries.
    static TEST_DB_INIT: std::sync::Once = std::sync::Once::new();
    static TEST_DB_DIR: OnceLock<PathBuf> = OnceLock::new();

    fn init_test_db() {
        TEST_DB_INIT.call_once(|| {
            let dir =
                std::env::temp_dir().join(format!("script-kit-ai-test-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("Should create test DB directory");
            let db_path = dir.join("ai-chats-test.sqlite");
            let _ = TEST_DB_DIR.set(dir);
            init_ai_db_at(db_path).expect("Should initialize test DB");
        });
        // For threads that lost the race, just wait for init to complete (Once handles this).
    }

    #[test]
    fn test_db_path() {
        let path = get_ai_db_path();
        let expected = crate::setup::get_kit_path()
            .join("db")
            .join("ai-chats.sqlite");

        assert_eq!(path, expected);
    }

    #[test]
    fn test_init_ai_db_is_idempotent() {
        // First call via test helper sets up the temp DB
        init_test_db();

        // Subsequent calls to init_ai_db should succeed (OnceLock already set)
        let result2 = init_ai_db();
        assert!(
            result2.is_ok(),
            "init_ai_db() should be idempotent, second call failed: {:?}",
            result2.err()
        );

        // Third call for good measure
        let result3 = init_ai_db();
        assert!(
            result3.is_ok(),
            "init_ai_db() should be idempotent, third call failed: {:?}",
            result3.err()
        );
    }

    #[test]
    fn test_search_chats_does_not_error() {
        // Ensure DB is initialized
        init_test_db();

        // Empty search should return all chats (not error)
        let result = search_chats("");
        assert!(
            result.is_ok(),
            "Empty search should not error: {:?}",
            result.err()
        );

        // Simple text search should not error (even if no results)
        let result = search_chats("test");
        assert!(
            result.is_ok(),
            "Simple text search should not error: {:?}",
            result.err()
        );

        // Search with special characters should not crash
        // (FTS MATCH is fragile with special characters - should fall back gracefully)
        let result = search_chats("test@example.com");
        assert!(
            result.is_ok(),
            "Search with @ should not error: {:?}",
            result.err()
        );

        let result = search_chats("foo*bar");
        assert!(
            result.is_ok(),
            "Search with * should not error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_fts_triggers_use_update_of_column() {
        // Ensure DB is initialized
        init_test_db();

        let db = get_db().expect("Should get db connection");
        let conn = db.lock().expect("Should lock connection");

        // Query the trigger SQL to verify it uses "UPDATE OF" syntax
        let chat_trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='chats_au'",
                [],
                |row| row.get(0),
            )
            .expect("Should find chats_au trigger");

        // The trigger should only fire on UPDATE OF title, not on all updates
        assert!(
            chat_trigger_sql.to_lowercase().contains("update of title"),
            "chats_au trigger should use 'UPDATE OF title' to avoid FTS churn on updated_at changes. Got: {}",
            chat_trigger_sql
        );

        let message_trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='messages_au'",
                [],
                |row| row.get(0),
            )
            .expect("Should find messages_au trigger");

        // The trigger should only fire on UPDATE OF content, not on all updates
        assert!(
            message_trigger_sql
                .to_lowercase()
                .contains("update of content"),
            "messages_au trigger should use 'UPDATE OF content' to avoid FTS churn. Got: {}",
            message_trigger_sql
        );
    }

    #[test]
    fn test_ai_db_has_required_pragmas() {
        // Ensure DB is initialized
        init_test_db();

        let db = get_db().expect("Should get db connection");
        let conn = db.lock().expect("Should lock connection");

        // Verify WAL mode is enabled
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("Should query journal_mode");
        assert_eq!(
            journal_mode.to_lowercase(),
            "wal",
            "AI DB should use WAL mode for better concurrency"
        );

        // Verify foreign keys are enabled
        let foreign_keys: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("Should query foreign_keys");
        assert_eq!(
            foreign_keys, 1,
            "AI DB should have foreign_keys=ON for CASCADE to work"
        );

        // Verify busy_timeout is set (should be > 0)
        let busy_timeout: i32 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("Should query busy_timeout");
        assert!(
            busy_timeout >= 1000,
            "AI DB should have busy_timeout >= 1000ms, got {}",
            busy_timeout
        );
    }

    #[test]
    fn test_save_message_persists_images_and_getters_populate_them() {
        init_test_db();

        let chat = Chat::new("test-model-images", "test-provider-images");
        create_chat(&chat).expect("Should create chat");

        let mut message = Message::user(chat.id, "user message with image attachments");
        message.images = vec![
            ImageAttachment::png("base64-image-1".to_string()),
            ImageAttachment::jpeg("base64-image-2".to_string()),
        ];

        save_message(&message).expect("Should save message with images");

        let all_messages = get_chat_messages(&chat.id).expect("Should fetch chat messages");
        let stored_message = all_messages
            .iter()
            .find(|m| m.id == message.id)
            .expect("Saved message should exist in full chat query");

        assert_eq!(stored_message.images.len(), 2);
        assert_eq!(stored_message.images[0].data, "base64-image-1");
        assert_eq!(stored_message.images[0].media_type, "image/png");
        assert_eq!(stored_message.images[1].data, "base64-image-2");
        assert_eq!(stored_message.images[1].media_type, "image/jpeg");

        let recent_messages =
            get_recent_messages(&chat.id, 1).expect("Should fetch recent message");
        assert_eq!(recent_messages.len(), 1);
        assert_eq!(recent_messages[0].id, message.id);
        assert_eq!(recent_messages[0].images.len(), 2);

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_save_message_replaces_existing_images_on_upsert() {
        init_test_db();

        let chat = Chat::new("test-model-image-upsert", "test-provider-image-upsert");
        create_chat(&chat).expect("Should create chat");

        let mut message = Message::user(chat.id, "first revision");
        message.images = vec![ImageAttachment::png("stale-base64".to_string())];
        save_message(&message).expect("Should save initial message image");

        message.content = "second revision".to_string();
        message.images = vec![
            ImageAttachment::jpeg("fresh-base64-1".to_string()),
            ImageAttachment::png("fresh-base64-2".to_string()),
        ];
        save_message(&message).expect("Should replace message image attachments");

        let stored_messages = get_chat_messages(&chat.id).expect("Should read back chat messages");
        let stored = stored_messages
            .iter()
            .find(|m| m.id == message.id)
            .expect("Updated message should exist");

        assert_eq!(stored.images.len(), 2);
        assert_eq!(stored.images[0].data, "fresh-base64-1");
        assert_eq!(stored.images[0].media_type, "image/jpeg");
        assert_eq!(stored.images[1].data, "fresh-base64-2");
        assert_eq!(stored.images[1].media_type, "image/png");

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_delete_messages_batch_rolls_back_when_any_message_missing() {
        init_test_db();

        let chat = Chat::new("test-model-batch-delete", "test-provider-batch-delete");
        create_chat(&chat).expect("Should create chat");

        let mut first = Message::user(chat.id, "first");
        first.images = vec![ImageAttachment::png("rollback-image".to_string())];
        let second = Message::assistant(chat.id, "second");

        save_message(&first).expect("Should save first message");
        save_message(&second).expect("Should save second message");

        let missing_id = format!("missing-{}", ChatId::new());
        let failed_delete =
            delete_messages_batch(&[first.id.clone(), missing_id.clone(), second.id.clone()]);
        assert!(
            failed_delete.is_err(),
            "Batch delete should fail when any message id is missing"
        );

        let still_present = get_chat_messages(&chat.id).expect("Should read chat after rollback");
        assert!(
            still_present.iter().any(|m| m.id == first.id),
            "Rollback should preserve first message when batch delete mismatches"
        );
        assert!(
            still_present.iter().any(|m| m.id == second.id),
            "Rollback should preserve second message when batch delete mismatches"
        );
        let first_after_rollback = still_present
            .iter()
            .find(|m| m.id == first.id)
            .expect("First message should be present after rollback");
        assert_eq!(
            first_after_rollback.images.len(),
            1,
            "Rollback should also preserve image attachments"
        );

        delete_messages_batch(&[first.id.clone(), second.id.clone()])
            .expect("Batch delete should succeed when all ids exist");
        let after_success =
            get_chat_messages(&chat.id).expect("Should read chat after successful delete");
        assert!(
            after_success.is_empty(),
            "All messages should be removed after successful batch delete"
        );

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_create_chat_with_messages_bulk_persists_chat_and_messages() {
        init_test_db();

        let mut chat = Chat::new("test-model-bulk-create", "test-provider-bulk-create");
        chat.title = "Bulk create test chat".to_string();

        let mut first = Message::user(chat.id, "bulk user message").with_tokens(11);
        first.images = vec![ImageAttachment::png("bulk-image-1".to_string())];
        let second = Message::assistant(chat.id, "bulk assistant message").with_tokens(17);

        create_chat_with_messages_bulk(&chat, &[first.clone(), second.clone()])
            .expect("Should create chat and messages in a single transaction");

        let stored_chat = get_chat(&chat.id)
            .expect("Should query stored chat")
            .expect("Bulk-created chat should exist");
        assert_eq!(stored_chat.id, chat.id);
        assert_eq!(stored_chat.title, chat.title);
        assert_eq!(stored_chat.model_id, chat.model_id);
        assert_eq!(stored_chat.provider, chat.provider);

        let stored_messages = get_chat_messages(&chat.id).expect("Should fetch stored messages");
        assert_eq!(
            stored_messages.len(),
            2,
            "Should persist both bulk messages"
        );

        let stored_first = stored_messages
            .iter()
            .find(|message| message.id == first.id)
            .expect("First bulk message should exist");
        assert_eq!(stored_first.content, first.content);
        assert_eq!(stored_first.tokens_used, first.tokens_used);
        assert_eq!(stored_first.images.len(), 1);
        assert_eq!(stored_first.images[0].data, "bulk-image-1");
        assert_eq!(stored_first.images[0].media_type, "image/png");

        let stored_second = stored_messages
            .iter()
            .find(|message| message.id == second.id)
            .expect("Second bulk message should exist");
        assert_eq!(stored_second.content, second.content);
        assert_eq!(stored_second.tokens_used, second.tokens_used);
        assert!(
            stored_second.images.is_empty(),
            "Second bulk message should not have images"
        );

        delete_chat_permanently(&chat.id).expect("Should cleanup test chat");
    }

    #[test]
    fn test_create_chat_with_messages_bulk_rolls_back_when_message_insert_fails() {
        init_test_db();

        let chat = Chat::new("test-model-bulk-rollback", "test-provider-bulk-rollback");
        let first = Message::user(chat.id, "first bulk message");
        let failing = Message::assistant(ChatId::new(), "message with missing parent chat");

        let result = create_chat_with_messages_bulk(&chat, &[first, failing]);
        assert!(
            result.is_err(),
            "Bulk create should fail when one message insert violates FK constraints"
        );

        let stored_chat = get_chat(&chat.id).expect("Should query bulk-created chat");
        assert!(
            stored_chat.is_none(),
            "Bulk transaction should rollback the chat insert when a message insert fails"
        );

        let stored_messages =
            get_chat_messages(&chat.id).expect("Should query messages after rollback");
        assert!(
            stored_messages.is_empty(),
            "Bulk transaction should rollback prior message inserts on failure"
        );
    }

    #[test]
    fn test_sanitize_fts_query_supports_prefix_matching() {
        // Single word gets quoted + wildcard
        let result = sanitize_fts_query("hel");
        assert_eq!(result, "\"hel\"*");

        // Multiple words each get prefix wildcards
        let result = sanitize_fts_query("hello wor");
        assert_eq!(result, "\"hello\"* \"wor\"*");

        // Special chars are stripped
        let result = sanitize_fts_query("test:query");
        assert!(result.contains("test"));
        assert!(result.contains("query"));
        assert!(!result.contains(':'));
    }

    #[test]
    fn test_extract_match_snippet_centers_on_match() {
        let content =
            "The quick brown fox jumps over the lazy dog and then keeps running across the meadow";
        let snippet = extract_match_snippet(content, "fox");
        assert!(
            snippet.contains("fox"),
            "Snippet should contain the match: {}",
            snippet
        );
        assert!(
            snippet.len() <= 90,
            "Snippet should be bounded: len={}",
            snippet.len()
        );
    }

    #[test]
    fn test_extract_match_snippet_adds_ellipsis_when_truncated() {
        let content = "A".repeat(20) + " MATCH " + &"B".repeat(200);
        let snippet = extract_match_snippet(&content, "match");
        assert!(snippet.contains("MATCH"), "Snippet should contain match");
        assert!(
            snippet.ends_with("..."),
            "Should have trailing ellipsis when truncated"
        );
    }

    #[test]
    fn test_search_chats_with_snippets_does_not_error() {
        init_test_db();

        // Empty search should return all chats
        let result = search_chats_with_snippets("");
        assert!(
            result.is_ok(),
            "Empty search should not error: {:?}",
            result.err()
        );

        // Simple text search should not error
        let result = search_chats_with_snippets("test");
        assert!(
            result.is_ok(),
            "Text search should not error: {:?}",
            result.err()
        );

        // Special characters should fall back gracefully
        let result = search_chats_with_snippets("test@example.com");
        assert!(
            result.is_ok(),
            "Special char search should not error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_search_chats_with_snippets_returns_match_context() {
        init_test_db();

        // Create a test chat with a message containing a unique keyword
        let chat = Chat::new("test-model-snippet", "test-provider-snippet");
        let chat_id = chat.id;
        create_chat(&chat).expect("Should create chat");

        let unique_keyword = "xyzzyplugh42";
        save_message(&Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id,
            role: MessageRole::User,
            content: format!("Tell me about {}", unique_keyword),
            created_at: chrono::Utc::now(),
            tokens_used: None,
            images: Vec::new(),
        })
        .expect("Should save message");

        let results = search_chats_with_snippets(unique_keyword).expect("Search should succeed");

        // Should find the chat
        assert!(!results.is_empty(), "Should find chat with unique keyword");
        let found = results.iter().find(|r| r.chat.id == chat_id);
        assert!(found.is_some(), "Should find our specific chat");

        let result = found.expect("already checked");
        // Should have a snippet containing the keyword
        assert!(
            result.match_snippet.is_some(),
            "Should have a match snippet for message content match"
        );
        let snippet = result.match_snippet.as_deref().unwrap_or("");
        assert!(
            snippet.to_lowercase().contains(unique_keyword),
            "Snippet should contain the keyword: got '{}'",
            snippet
        );

        // Cleanup
        delete_chat_permanently(&chat_id).expect("Should cleanup");
    }

    #[test]
    fn test_message_preparation_audit_round_trip() {
        use crate::ai::message_parts::{
            prepare_user_message, AiContextPart, ContextPreparationItem, PreparedMessageDecision,
        };
        use crate::ai::preflight_audit::AiPreflightAudit;

        init_test_db();

        // Create a parent chat so the FK is satisfied
        let chat_id = ChatId::new();
        let chat = Chat {
            id: chat_id,
            title: "Audit round-trip test".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
            model_id: "test-model".to_string(),
            provider: "test".to_string(),
            source: ChatSource::AiWindow,
        };
        create_chat(&chat).expect("Should create parent chat");

        let prepared = prepare_user_message(
            "Summarize this page",
            &[ContextPreparationItem::primary(AiContextPart::FilePath {
                path: "/missing/PATH_CANARY.txt".to_string(),
                label: "Missing attachment".to_string(),
            })],
            &[],
            &[],
        );
        assert_eq!(prepared.decision, PreparedMessageDecision::Blocked);
        let mut audit = AiPreflightAudit::new(
            &chat_id,
            "Summarize this page",
            "Summarize this page",
            false,
            true,
            prepared.receipt,
        );
        audit.correlation_id = format!("test-roundtrip-{}", std::process::id());
        audit.preflight_generation = 1;

        // Save
        save_message_preparation_audit(&audit).expect("Should save audit");
        {
            let db = get_db().expect("database");
            let conn = db.lock().expect("database lock");
            let persisted: (String, String) = conn
                .query_row(
                    "SELECT raw_content, authored_content FROM message_preparation_audits WHERE correlation_id = ?1",
                    params![audit.correlation_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("read redacted compatibility columns");
            assert_eq!(persisted, ("19".to_string(), "19".to_string()));
        }

        // Load back
        let loaded = get_last_message_preparation_audit(&chat_id)
            .expect("Should query audit")
            .expect("Should find saved audit");

        assert_eq!(loaded.schema_version, audit.schema_version);
        assert_eq!(loaded.correlation_id, audit.correlation_id);
        assert_eq!(loaded.chat_id, audit.chat_id);
        assert_eq!(loaded.decision, audit.decision);
        assert_eq!(loaded.receipt.context.attempted, 1);
        assert_eq!(loaded.receipt.context.resolved, 0);
        assert_eq!(loaded.receipt.context.failed, 1);
        assert_eq!(loaded.actionable_failures.len(), 1);
        assert_eq!(loaded.actionable_failures[0].code, "attachment_unavailable");
        let serialized = serde_json::to_string(&loaded).expect("serialize loaded receipt");
        assert!(!serialized.contains("PATH_CANARY"));
        assert!(!serialized.contains("Summarize this page"));
        assert_eq!(loaded.message_id, None);

        // Create a real message so the FK constraint is satisfied on upsert
        let msg = Message {
            id: "msg-audit-rt".to_string(),
            chat_id,
            role: MessageRole::User,
            content: "Summarize this page".to_string(),
            created_at: chrono::Utc::now(),
            tokens_used: None,
            images: Vec::new(),
        };
        save_message(&msg).expect("Should save message");

        // Upsert with message_id
        audit.attach_message_id(&msg.id);
        save_message_preparation_audit(&audit).expect("Should upsert audit");

        let reloaded = get_last_message_preparation_audit(&chat_id)
            .expect("Should re-query audit")
            .expect("Should find upserted audit");
        assert_eq!(reloaded.message_id, Some("msg-audit-rt".to_string()));

        // Cleanup
        delete_chat_permanently(&chat_id).expect("Should cleanup");
    }
}
