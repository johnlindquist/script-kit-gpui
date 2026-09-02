#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // ── Helpers ──────────────────────────────────────────────────────

    fn make_conversation(
        session_id: &str,
        timestamp: &str,
        messages: Vec<(&str, &str)>,
    ) -> SavedConversation {
        SavedConversation {
            session_id: session_id.to_string(),
            timestamp: timestamp.to_string(),
            custom_title: None,
            messages: messages
                .into_iter()
                .map(|(role, body)| SavedMessage {
                    role: role.to_string(),
                    body: body.to_string(),
                })
                .collect(),
        }
    }

    // SK_PATH is process-global, so these tests must share the repo-wide
    // lock; a module-local mutex races against every other test suite that
    // repoints SK_PATH (dictation history, config, scriptlets, ...).
    fn history_env_lock() -> &'static Mutex<()> {
        crate::test_utils::SK_PATH_TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn fresh_conversation_cache_proof_tracks_publication_freshness_and_worker_ownership() {
        let _guard = history_env_lock().lock().expect("history env lock");
        let previous_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
        let temp = tempfile::tempdir().expect("temp dir");
        std::env::set_var(crate::setup::SK_PATH_ENV, temp.path());
        invalidate_history_cache();
        assert!(root_agent_chat_history_fresh_cache_status().is_none());
        let refresh = try_begin_root_agent_chat_history_refresh().unwrap();
        assert!(root_agent_chat_history_fresh_cache_status().is_none());
        assert!(finish_root_agent_chat_history_refresh(
            refresh,
            read_root_agent_chat_history_snapshot()
        ));
        let (revision, count) = root_agent_chat_history_fresh_cache_status().unwrap();
        assert!(revision > 0);
        assert_eq!(count, 0);
        {
            let _cache = agent_chat_history_index_cache().lock().unwrap();
            assert!(root_agent_chat_history_fresh_cache_status().is_none());
        }
        let worker = agent_chat_history_refresh_lifecycle()
            .lock()
            .unwrap()
            .begin(
                sk_protocol::command_contract::CommandSource::Conversation,
                false,
            )
            .unwrap();
        assert!(root_agent_chat_history_fresh_cache_status().is_none());
        assert!(discard_root_agent_chat_history_refresh(worker));
        assert_eq!(
            root_agent_chat_history_fresh_cache_status(),
            Some((revision, 0))
        );
        crate::atomic_file::write_private_atomic(&history_path(), b"").unwrap();
        assert!(root_agent_chat_history_fresh_cache_status().is_none());
        let refresh = try_begin_root_agent_chat_history_refresh().unwrap();
        assert!(finish_root_agent_chat_history_refresh(
            refresh,
            read_root_agent_chat_history_snapshot()
        ));
        assert!(root_agent_chat_history_fresh_cache_status().unwrap().0 > revision);
        match previous_sk_path {
            Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
            None => std::env::remove_var(crate::setup::SK_PATH_ENV),
        }
        invalidate_history_cache();
        assert!(root_agent_chat_history_fresh_cache_status().is_none());
    }

    #[test]
    fn agent_chat_history_integrity_repairs_legacy_private_jsonl_boundaries() {
        let root = tempfile::tempdir().expect("isolated Agent Chat history root");
        let first = make_conversation(
            "legacy-private-session",
            "2026-08-22T12:00:00Z",
            vec![("user", "first private Agent Chat request")],
        );
        let second = make_conversation(
            "next-private-session",
            "2026-08-22T12:00:01Z",
            vec![("user", "second private Agent Chat request")],
        );
        let first_entry = build_history_entry(&first).expect("legacy index entry");
        let second_entry = build_history_entry(&second).expect("next index entry");
        let path = root.path().join("agent_chat-history.jsonl");
        let legacy = serde_json::to_string(&first_entry).expect("serialize legacy private entry");
        crate::atomic_file::write_private_atomic(&path, legacy.as_bytes())
            .expect("seed private JSONL without a terminal newline");

        save_history_entry_at(root.path(), &second_entry)
            .expect("repair legacy Agent Chat index record boundary");

        let content = read_private_history_file(&path).expect("read private history index");
        let entries = parse_history_entries(&content).expect("parse both complete conversations");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].session_id, second.session_id);
        assert_eq!(entries[1].session_id, first.session_id);
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn agent_chat_history_integrity_repairs_legacy_private_prompt_boundaries() {
        let root = tempfile::tempdir().expect("isolated Agent Chat prompt root");
        let path = root.path().join("agent_chat-prompt-history.jsonl");
        let first = PromptHistoryLine {
            timestamp: "2026-08-22T12:00:00Z".to_string(),
            prompt: "first private composer prompt".to_string(),
        };
        let legacy = serde_json::to_string(&first).expect("serialize legacy prompt");
        crate::atomic_file::write_private_atomic(&path, legacy.as_bytes())
            .expect("seed legacy prompt without a terminal newline");

        append_prompt_history_at(root.path(), "second private composer prompt")
            .expect("repair private prompt JSONL boundary");

        let prompts = load_prompt_history_at(root.path(), 10)
            .expect("both private composer prompts remain recoverable");
        assert_eq!(
            prompts,
            [
                "first private composer prompt",
                "second private composer prompt"
            ]
        );
    }

    #[test]
    fn agent_chat_history_integrity_never_overwrites_or_exposes_malformed_prompt_history() {
        let root = tempfile::tempdir().expect("isolated Agent Chat prompt root");
        let path = root.path().join("agent_chat-prompt-history.jsonl");
        append_prompt_history_at(root.path(), "valid private composer prompt")
            .expect("seed valid private composer history");
        let valid = read_private_history_file(&path).expect("read valid private prompt history");
        let canary = "never-expose-malformed-private-composer-prompt";
        let corrupted = format!("{valid}{{\"prompt\":\"{canary}\"");
        crate::atomic_file::write_private_atomic(&path, corrupted.as_bytes())
            .expect("seed malformed private composer history");

        let error = append_prompt_history_at(root.path(), "new private prompt")
            .expect_err("malformed composer history refuses destructive append");

        assert_eq!(
            error,
            AgentChatConversationPersistenceError::InvalidPromptHistoryPayload
        );
        assert!(!error.to_string().contains(canary));
        assert_eq!(read_private_history_file(&path).unwrap(), corrupted);
        assert!(load_prompt_history_at(root.path(), 10).is_err());
    }

    #[test]
    fn agent_chat_history_integrity_never_deletes_a_conversation_for_a_malformed_index() {
        let root = tempfile::tempdir().expect("isolated Agent Chat conversation root");
        let conversation = make_conversation(
            "preserve-private-conversation",
            "2026-08-22T12:00:00Z",
            vec![("user", "private conversation must remain recoverable")],
        );
        let entry = build_history_entry(&conversation).expect("private index entry");
        save_completed_conversation_at(root.path(), &conversation, &entry)
            .expect("persist valid private conversation");
        let path = root.path().join("agent_chat-history.jsonl");
        let valid = read_private_history_file(&path).expect("read valid private index");
        let canary = "never-expose-malformed-private-agent-chat-content";
        let corrupted = format!("{valid}{{\"private\":\"{canary}\"");
        crate::atomic_file::write_private_atomic(&path, corrupted.as_bytes())
            .expect("seed malformed private index");

        let error = delete_conversation_at(root.path(), &conversation.session_id)
            .expect_err("malformed private index must stop deletion before any mutation");

        assert!(!error.to_string().contains(canary));
        assert_eq!(read_private_history_file(&path).unwrap(), corrupted);
        let retained = load_conversation_at(root.path(), &conversation.session_id)
            .expect("original conversation remains readable")
            .expect("original conversation remains on disk");
        assert_eq!(retained.messages[0].body, conversation.messages[0].body);
    }

    #[test]
    fn agent_chat_history_integrity_refuses_malformed_index_before_saving_or_renaming() {
        let root = tempfile::tempdir().expect("isolated Agent Chat conversation root");
        let original = make_conversation(
            "original-private-session",
            "2026-08-22T12:00:00Z",
            vec![("user", "original private Agent Chat request")],
        );
        let original_entry = build_history_entry(&original).expect("original private index");
        save_completed_conversation_at(root.path(), &original, &original_entry)
            .expect("persist original private conversation");
        let path = root.path().join("agent_chat-history.jsonl");
        let valid = read_private_history_file(&path).expect("read valid index");
        let corrupted = format!("{valid}{{malformed private index");
        crate::atomic_file::write_private_atomic(&path, corrupted.as_bytes())
            .expect("seed malformed index");
        let incoming = make_conversation(
            "incoming-private-session",
            "2026-08-22T12:00:01Z",
            vec![("user", "incoming private Agent Chat request")],
        );
        let incoming_entry = build_history_entry(&incoming).expect("incoming private index");

        assert_eq!(
            save_completed_conversation_at(root.path(), &incoming, &incoming_entry),
            Err(AgentChatConversationPersistenceError::InvalidHistoryIndexPayload)
        );
        assert!(
            rename_conversation_at(root.path(), &original.session_id, "Unexpected rename").is_err()
        );
        assert_eq!(read_private_history_file(&path).unwrap(), corrupted);
        assert!(load_conversation_at(root.path(), &incoming.session_id)
            .expect("incoming identity remains safe")
            .is_none());
        let unchanged = load_conversation_at(root.path(), &original.session_id)
            .expect("original conversation remains readable")
            .expect("original conversation remains on disk");
        assert!(unchanged.custom_title.is_none());
    }

    #[test]
    fn agent_chat_history_integrity_rejects_malformed_private_root_snapshots() {
        let root = tempfile::tempdir().expect("isolated root Agent Chat history snapshot");
        let path = root.path().join("agent_chat-history.jsonl");
        crate::atomic_file::write_private_atomic(&path, b"private malformed conversation\n")
            .expect("seed malformed private Agent Chat index");

        let snapshot = read_root_agent_chat_history_snapshot_at(&path);

        assert!(snapshot.read_outcome().is_err());
        assert!(!root_agent_chat_history_snapshot_is_current_at(
            &snapshot, &path
        ));
    }

    #[test]
    fn conversation_read_outcome_distinguishes_failure_from_successful_empty() {
        let failed = RootAgentChatHistorySnapshot {
            cache: Err(AgentChatConversationPersistenceError::InvalidHistoryIndexPayload.into()),
        };
        assert_eq!(
            failed
                .read_outcome()
                .unwrap_err()
                .downcast_ref::<AgentChatConversationPersistenceError>(),
            Some(&AgentChatConversationPersistenceError::InvalidHistoryIndexPayload)
        );
        let empty = RootAgentChatHistorySnapshot {
            cache: Ok(AgentChatHistoryIndexCache {
                signature: None,
                owned: true,
                owned_fresh: true,
                entries: Vec::new(),
            }),
        };
        assert_eq!(empty.read_outcome().unwrap(), 0);
    }

    #[test]
    fn agent_chat_history_integrity_serializes_completed_saves_and_deletion() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().expect("isolated concurrent Agent Chat root");
        let root = Arc::new(directory.path().to_path_buf());
        let removed = make_conversation(
            "remove-only-this-session",
            "2026-08-22T12:00:00Z",
            vec![("user", "only this private session should disappear")],
        );
        let removed_entry = build_history_entry(&removed).expect("removed private index");
        save_completed_conversation_at(&root, &removed, &removed_entry)
            .expect("persist initial private session");
        let start = Arc::new(Barrier::new(6));
        let workers = (0..5)
            .map(|index| {
                let root = Arc::clone(&root);
                let start = Arc::clone(&start);
                std::thread::spawn(move || {
                    let session = format!("concurrent-private-session-{index}");
                    let message = format!("private concurrent conversation {index}");
                    let conversation = make_conversation(
                        &session,
                        "2026-08-22T12:00:01Z",
                        vec![("user", &message)],
                    );
                    let entry = build_history_entry(&conversation).expect("concurrent index");
                    start.wait();
                    save_completed_conversation_at(&root, &conversation, &entry)
                        .expect("persist complete concurrent private conversation");
                    session
                })
            })
            .collect::<Vec<_>>();

        start.wait();
        delete_conversation_at(&root, &removed.session_id)
            .expect("delete only the requested private conversation");
        let recorded = workers
            .into_iter()
            .map(|worker| worker.join().expect("join isolated Agent Chat worker"))
            .collect::<Vec<_>>();
        let content = read_private_history_file(&root.join("agent_chat-history.jsonl"))
            .expect("read complete private Agent Chat index");
        let entries = parse_history_entries(&content).expect("parse complete private index");

        assert_eq!(entries.len(), recorded.len());
        assert!(!entries
            .iter()
            .any(|entry| entry.session_id == removed.session_id));
        assert!(recorded.iter().all(|session| {
            entries.iter().any(|entry| &entry.session_id == session)
                && conversation_exists_at(&root, session) == Ok(true)
        }));
        assert_eq!(
            conversation_exists_at(&root, &removed.session_id),
            Ok(false)
        );
    }

    #[test]
    fn root_history_refresh_never_starts_for_fresh_cache_or_duplicates_active_worker() {
        let source = sk_protocol::command_contract::CommandSource::Conversation;
        let mut lifecycle = AgentChatHistoryRefreshLifecycle::default();
        assert!(lifecycle.begin(source, true).is_none());

        let first = lifecycle
            .begin(source, false)
            .expect("cold cache starts one worker");
        assert_eq!(first.generation, 1);
        assert!(lifecycle.begin(source, false).is_none());
        assert!(lifecycle.finish(first));
        assert!(lifecycle.begin(source, false).is_some());
    }

    #[test]
    fn root_history_refresh_stale_completion_cannot_release_a_newer_owned_worker() {
        let source = sk_protocol::command_contract::CommandSource::Conversation;
        let mut lifecycle = AgentChatHistoryRefreshLifecycle::default();
        let stale = lifecycle.begin(source, false).expect("first owned worker");
        assert!(lifecycle.finish(stale));

        let current = lifecycle
            .begin(source, false)
            .expect("replacement owned worker");
        assert!(current.generation > stale.generation);
        assert!(!lifecycle.finish(stale));
        assert!(lifecycle.begin(source, false).is_none());
        assert!(lifecycle.finish(current));
    }

    #[test]
    fn root_history_refresh_generation_exhaustion_refuses_ticket_reuse() {
        let mut lifecycle = AgentChatHistoryRefreshLifecycle {
            next_generation: u64::MAX,
            in_flight: None,
        };
        assert!(lifecycle
            .begin(
                sk_protocol::command_contract::CommandSource::Conversation,
                false,
            )
            .is_none());
        assert_eq!(lifecycle.next_generation, u64::MAX);
        assert!(lifecycle.in_flight.is_none());
    }

    #[test]
    fn root_history_refresh_reads_private_snapshot_without_publishing_changed_file() {
        let temp = tempfile::tempdir().expect("isolated history refresh fixture");
        let path = temp.path().join("history.jsonl");
        let entry = AgentChatHistoryEntry {
            session_id: "owned-history-session".to_owned(),
            first_message: "private conversation".to_owned(),
            timestamp: "2026-08-22T10:00:00Z".to_owned(),
            ..Default::default()
        };
        std::fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();

        let snapshot = read_root_agent_chat_history_snapshot_at(&path);
        assert_eq!(snapshot.read_outcome().unwrap(), 1);
        assert_eq!(
            snapshot.cache.as_ref().unwrap().entries[0].session_id,
            entry.session_id
        );
        assert!(root_agent_chat_history_snapshot_is_current_at(
            &snapshot, &path
        ));

        std::fs::write(&path, "a newer and deliberately different private snapshot")
            .expect("replace history after worker read");
        assert!(!root_agent_chat_history_snapshot_is_current_at(
            &snapshot, &path
        ));
    }

    // ── Serde roundtrip ─────────────────────────────────────────────

    #[test]
    fn history_entry_serializes_with_new_fields() {
        let entry = AgentChatHistoryEntry {
            timestamp: "2026-04-01T18:00:00Z".to_string(),
            first_message: "hello world".to_string(),
            message_count: 5,
            session_id: "test-123".to_string(),
            title: "hello world".to_string(),
            custom_title: Some("Real title".to_string()),
            preview: "The answer is 42".to_string(),
            search_text: "hello world the answer is 42".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: AgentChatHistoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.title, "hello world");
        assert_eq!(parsed.custom_title.as_deref(), Some("Real title"));
        assert_eq!(parsed.preview, "The answer is 42");
        assert!(!parsed.search_text.is_empty());
    }

    #[test]
    fn legacy_entry_without_new_fields_deserializes() {
        // Simulates an old JSONL line that has no title/preview/search_text.
        let legacy_json = r#"{"timestamp":"2026-03-01T12:00:00Z","first_message":"fix the login","message_count":3,"session_id":"legacy-1"}"#;
        let entry: AgentChatHistoryEntry =
            serde_json::from_str(legacy_json).expect("legacy entry should deserialize");
        assert_eq!(entry.first_message, "fix the login");
        // New fields default to empty strings.
        assert!(entry.title.is_empty());
        assert!(entry.custom_title.is_none());
        assert!(entry.preview.is_empty());
        assert!(entry.search_text.is_empty());
    }

    #[test]
    fn legacy_saved_conversation_without_custom_title_deserializes() {
        let legacy_json = r#"{"session_id":"legacy-conv","timestamp":"2026-03-01T12:00:00Z","messages":[{"role":"user","body":"hello"}]}"#;
        let conversation: SavedConversation =
            serde_json::from_str(legacy_json).expect("legacy conversation should deserialize");
        assert_eq!(conversation.session_id, "legacy-conv");
        assert!(conversation.custom_title.is_none());
    }

    #[test]
    fn saved_conversation_serializes() {
        let conv = make_conversation(
            "test-456",
            "2026-04-01T18:00:00Z",
            vec![("user", "hello"), ("assistant", "hi there!")],
        );
        let json = serde_json::to_string_pretty(&conv).expect("serialize");
        assert!(json.contains("hello"));
        assert!(json.contains("hi there!"));
    }

    #[test]
    fn prompt_history_preserves_trimmed_private_values_order_and_consecutive_deduplication() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        assert_eq!(load_prompt_history_at(temp.path(), 10), Ok(Vec::new()));
        assert_eq!(append_prompt_history_at(temp.path(), "   \n"), Ok(()));
        assert!(!temp.path().join("agent_chat-prompt-history.jsonl").exists());

        append_prompt_history_at(temp.path(), "  first private prompt  ")
            .expect("first private prompt saves");
        append_prompt_history_at(temp.path(), "first private prompt")
            .expect("consecutive duplicate is suppressed");
        append_prompt_history_at(temp.path(), "second private prompt")
            .expect("second private prompt saves");
        append_prompt_history_at(temp.path(), "first private prompt")
            .expect("non-consecutive repeated prompt remains legitimate");

        assert_eq!(
            load_prompt_history_at(temp.path(), 10),
            Ok(vec![
                "first private prompt".to_string(),
                "second private prompt".to_string(),
                "first private prompt".to_string(),
            ]),
        );
        assert_eq!(
            load_prompt_history_at(temp.path(), 2),
            Ok(vec![
                "second private prompt".to_string(),
                "first private prompt".to_string(),
            ]),
        );
        assert_eq!(load_prompt_history_at(temp.path(), 0), Ok(Vec::new()));
    }

    #[test]
    fn prompt_history_compaction_preserves_only_newest_entries_in_chronological_order() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let path = temp.path().join("agent_chat-prompt-history.jsonl");
        let mut seeded = String::new();
        for index in 0..PROMPT_HISTORY_MAX_LINES * 2 {
            let line = PromptHistoryLine {
                timestamp: "2026-04-01T18:00:00Z".to_string(),
                prompt: format!("private-prompt-{index}"),
            };
            seeded.push_str(&serde_json::to_string(&line).expect("synthetic prompt line"));
            seeded.push('\n');
        }
        write_private_history_file_atomically(&path, &seeded)
            .expect("private bounded prompt fixture");

        append_prompt_history_at(temp.path(), "  newest private prompt  ")
            .expect("one extra prompt triggers private atomic compaction");
        let loaded = load_prompt_history_at(temp.path(), usize::MAX)
            .expect("compacted private prompts remain readable");
        assert_eq!(loaded.len(), PROMPT_HISTORY_MAX_LINES);
        assert_eq!(
            loaded.first().map(String::as_str),
            Some("private-prompt-201")
        );
        assert_eq!(
            loaded.last().map(String::as_str),
            Some("newest private prompt")
        );
        assert_eq!(
            std::fs::read_to_string(path)
                .expect("compacted prompt file remains regular")
                .lines()
                .count(),
            PROMPT_HISTORY_MAX_LINES,
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_history_never_follows_symlinked_private_prompt_store() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let external = temp.path().join("unrelated-sensitive-file.txt");
        std::fs::write(&external, "external private content")
            .expect("external private file fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-prompt-history.jsonl"))
            .expect("malicious private-prompt symlink fixture");

        assert_eq!(
            append_prompt_history_at(&root, "never append this secret"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            load_prompt_history_at(&root, 10),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            std::fs::read_to_string(&external).expect("external private file stays untouched"),
            "external private content",
        );
        assert!(
            std::fs::symlink_metadata(root.join("agent_chat-prompt-history.jsonl"))
                .expect("malicious symlink remains untouched")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn prompt_history_rejects_non_file_targets_before_writing() {
        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        std::fs::create_dir(temp.path().join("agent_chat-prompt-history.jsonl"))
            .expect("wrong-type prompt fixture");

        assert_eq!(
            append_prompt_history_at(temp.path(), "never write this private prompt"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            load_prompt_history_at(temp.path(), 5),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_history_creates_and_compacts_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated prompt fixture");
        let path = temp.path().join("agent_chat-prompt-history.jsonl");
        append_prompt_history_at(temp.path(), "private prompt")
            .expect("private prompt store initializes safely");
        let created_mode = std::fs::metadata(&path)
            .expect("private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(created_mode, 0o600);

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("legacy over-permissive prompt fixture");
        append_prompt_history_at(temp.path(), "repair legacy private permissions")
            .expect("legacy prompt store permissions become private before append");
        let repaired_mode = std::fs::metadata(&path)
            .expect("repaired private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(repaired_mode, 0o600);

        let mut oversized = String::new();
        for index in 0..PROMPT_HISTORY_MAX_LINES * 2 {
            let line = PromptHistoryLine {
                timestamp: "2026-04-01T18:00:00Z".to_string(),
                prompt: format!("private-{index}"),
            };
            oversized.push_str(&serde_json::to_string(&line).expect("synthetic prompt line"));
            oversized.push('\n');
        }
        write_private_history_file_atomically(&path, &oversized)
            .expect("private oversized fixture");
        append_prompt_history_at(temp.path(), "final private prompt")
            .expect("atomic prompt compaction");
        let compacted_mode = std::fs::metadata(&path)
            .expect("compacted private prompt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(compacted_mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn history_index_repairs_legacy_permissions_before_appending_private_transcript() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated conversation index fixture");
        let path = temp.path().join("agent_chat-history.jsonl");
        std::fs::write(&path, "").expect("legacy conversation index fixture");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("over-permissive legacy conversation index fixture");
        let conversation = make_conversation(
            "private-history-session",
            "2026-08-22T10:00:00Z",
            vec![("user", "private medical transcript")],
        );
        let entry = build_history_entry(&conversation).expect("real conversation index entry");

        save_history_entry_at(temp.path(), &entry)
            .expect("private transcript index repairs legacy permissions before append");

        let mode = std::fs::metadata(&path)
            .expect("repaired conversation index metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let persisted = std::fs::read_to_string(path).expect("private index remains readable");
        assert!(persisted.contains("private medical transcript"));
    }

    #[cfg(unix)]
    #[test]
    fn private_history_reads_repair_legacy_permissions_before_exposing_contents() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("isolated private-history fixture");
        for filename in [
            "agent_chat-history.jsonl",
            "agent_chat-prompt-history.jsonl",
            "saved-conversation.json",
        ] {
            let path = temp.path().join(filename);
            std::fs::write(&path, "legacy private user content")
                .expect("legacy private-history fixture");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
                .expect("over-permissive legacy private-history fixture");

            assert_eq!(
                read_private_history_file(&path)
                    .expect("private-history migration succeeds before content is returned"),
                "legacy private user content",
            );
            let mode = std::fs::metadata(&path)
                .expect("migrated private-history metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{filename} stayed world-readable");
        }
    }

    #[test]
    fn conversation_session_ids_preserve_real_formats_and_reject_traversal() {
        for valid in [
            "warm:8ecf16f4-c02a-4a2b-a4d2-a64c76d69303",
            "standard-agent-chat-mock-fixture",
            "legacy.session_42",
            "東京-session",
        ] {
            assert_eq!(validate_agent_chat_session_id(valid), Ok(()));
        }

        for invalid in [
            "",
            " ",
            ".",
            "..",
            "../escaped",
            "../../outside",
            "/absolute/session",
            "nested/session",
            "nested\\session",
            "C:outside",
            "C:\\outside",
            "line\nbreak",
            "nul\0value",
        ] {
            assert_eq!(
                validate_agent_chat_session_id(invalid),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
                "session ID must fail closed: {invalid:?}",
            );
        }
    }

    #[test]
    fn conversation_persistence_rejects_traversal_before_any_filesystem_mutation() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let root_sibling = root.join("escaped.json");
        let external_sibling = temp.path().join("outside.json");
        std::fs::write(&root_sibling, "preserve kit sibling").expect("kit sibling fixture");
        std::fs::write(&external_sibling, "preserve external sibling")
            .expect("external sibling fixture");

        for session_id in [
            "../escaped",
            "../../outside",
            "/absolute/session",
            "nested\\escape",
            ".",
            "..",
            "nul\0escape",
        ] {
            let conversation = make_conversation(
                session_id,
                "2026-04-01T18:00:00Z",
                vec![("user", "safe fixture")],
            );
            assert_eq!(
                save_conversation_at(&root, &conversation),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
            assert_eq!(
                conversation_exists_at(&root, session_id),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
            assert!(matches!(
                load_conversation_at(&root, session_id),
                Err(AgentChatConversationPersistenceError::InvalidSessionId)
            ));
            assert!(rename_conversation_at(&root, session_id, "ignored").is_err());
            assert!(delete_conversation_at(&root, session_id).is_err());

            let entry = build_history_entry(&conversation).expect("synthetic history entry");
            assert_eq!(
                save_history_entry_at(&root, &entry),
                Err(AgentChatConversationPersistenceError::InvalidSessionId),
            );
        }

        assert!(!root.join("agent_chat-conversations").exists());
        assert!(!root.join("agent_chat-history.jsonl").exists());
        assert_eq!(
            std::fs::read_to_string(root_sibling).expect("kit sibling stays intact"),
            "preserve kit sibling",
        );
        assert_eq!(
            std::fs::read_to_string(external_sibling).expect("external sibling stays intact"),
            "preserve external sibling",
        );
    }

    #[test]
    fn conversation_persistence_round_trips_warm_ids_and_preserves_private_permissions() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let session_id = "warm:8ecf16f4-c02a-4a2b-a4d2-a64c76d69303";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![
                ("user", "private question"),
                ("assistant", "private answer"),
            ],
        );

        save_conversation_at(temp.path(), &conversation).expect("safe session saves");
        assert_eq!(conversation_exists_at(temp.path(), session_id), Ok(true));
        let loaded = load_conversation_at(temp.path(), session_id)
            .expect("safe session loads")
            .expect("saved session exists");
        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.messages.len(), 2);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let path = conversation_path_at(temp.path(), session_id).expect("safe path");
            let mode = std::fs::metadata(path)
                .expect("private conversation metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        rename_conversation_at(temp.path(), session_id, "Private Title")
            .expect("safe session renames");
        let renamed = load_conversation_at(temp.path(), session_id)
            .expect("renamed session loads")
            .expect("renamed session exists");
        assert_eq!(renamed.custom_title.as_deref(), Some("Private Title"));
        delete_conversation_at(temp.path(), session_id).expect("safe session deletes");
        assert_eq!(conversation_exists_at(temp.path(), session_id), Ok(false));
        assert_eq!(
            std::fs::read_to_string(temp.path().join("agent_chat-history.jsonl"))
                .expect("safe index remains readable"),
            "",
        );
    }

    #[test]
    fn conversation_deletion_removes_only_the_selected_sessions_private_attachments() {
        let root = tempfile::tempdir().expect("isolated conversation fixture");
        let session_id = "warm:owned-session";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![
                ("user", "private question"),
                ("assistant", "private answer"),
            ],
        );
        save_conversation_at(root.path(), &conversation).expect("save owned conversation");
        let directory = root.path().join("agent_chat-history-attachments");
        std::fs::create_dir(&directory).expect("attachment directory");
        let owned_summary = directory.join(format!("{session_id}-summary.md"));
        let owned_transcript = directory.join(format!("{session_id}-transcript.md"));
        let unrelated = directory.join("another-session-transcript.md");
        std::fs::write(&owned_summary, "private summary").unwrap();
        std::fs::write(&owned_transcript, "private complete transcript").unwrap();
        std::fs::write(&unrelated, "another owner's private transcript").unwrap();

        delete_conversation_at(root.path(), session_id)
            .expect("delete only the selected conversation and its attachments");

        assert!(!owned_summary.exists());
        assert!(!owned_transcript.exists());
        assert_eq!(
            std::fs::read_to_string(unrelated).unwrap(),
            "another owner's private transcript"
        );
    }

    #[cfg(unix)]
    #[test]
    fn conversation_deletion_rejects_attachment_symlink_before_any_private_store_changes() {
        let root = tempfile::tempdir().expect("isolated conversation fixture");
        let session_id = "owned-session";
        let conversation = make_conversation(
            session_id,
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        save_conversation_at(root.path(), &conversation).expect("save owned conversation");
        save_history_entry_at(
            root.path(),
            &build_history_entry(&conversation).expect("private index entry"),
        )
        .expect("save owned index entry");
        let directory = root.path().join("agent_chat-history-attachments");
        std::fs::create_dir(&directory).expect("attachment directory");
        let external = root.path().join("external-private.md");
        std::fs::write(&external, "never follow or delete me").unwrap();
        std::os::unix::fs::symlink(&external, directory.join("owned-session-transcript.md"))
            .expect("hostile attachment symlink");

        assert!(delete_conversation_at(root.path(), session_id).is_err());
        assert_eq!(conversation_exists_at(root.path(), session_id), Ok(true));
        assert!(
            std::fs::read_to_string(root.path().join("agent_chat-history.jsonl"))
                .unwrap()
                .contains(session_id)
        );
        assert_eq!(
            std::fs::read_to_string(external).unwrap(),
            "never follow or delete me"
        );
    }

    #[test]
    fn conversation_load_rename_and_delete_reject_spoofed_payload_identity() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let directory = temp.path().join("agent_chat-conversations");
        std::fs::create_dir(&directory).expect("conversation directory fixture");
        let spoofed = make_conversation(
            "other-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "another user's conversation")],
        );
        let requested_path = directory.join("requested-session.json");
        let payload = serde_json::to_string(&spoofed).expect("spoofed payload fixture");
        std::fs::write(&requested_path, &payload).expect("spoofed session fixture");

        assert!(matches!(
            load_conversation_at(temp.path(), "requested-session"),
            Err(AgentChatConversationPersistenceError::SessionIdMismatch)
        ));
        assert!(rename_conversation_at(temp.path(), "requested-session", "Wrong Title").is_err());
        assert!(delete_conversation_at(temp.path(), "requested-session").is_err());
        assert_eq!(
            std::fs::read_to_string(requested_path).expect("spoofed payload remains untouched"),
            payload,
        );
    }

    #[cfg(unix)]
    #[test]
    fn conversation_persistence_never_follows_symlinked_session_targets() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        let directory = root.join("agent_chat-conversations");
        std::fs::create_dir_all(&directory).expect("conversation directory fixture");
        let external = temp.path().join("private-sibling.json");
        std::fs::write(&external, "untouched sibling secrets").expect("sibling fixture");
        let session_path = directory.join("safe-session.json");
        std::os::unix::fs::symlink(&external, &session_path)
            .expect("malicious session symlink fixture");

        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        assert_eq!(
            save_conversation_at(&root, &conversation),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert_eq!(
            conversation_exists_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        assert!(matches!(
            load_conversation_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget)
        ));
        assert!(rename_conversation_at(&root, "safe-session", "Wrong Title").is_err());
        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert_eq!(
            std::fs::read_to_string(external).expect("symlink destination remains untouched"),
            "untouched sibling secrets",
        );
        assert!(std::fs::symlink_metadata(session_path)
            .expect("session link remains untouched")
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn conversation_persistence_rejects_symlinked_conversation_directory() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        let external = temp.path().join("private-external-directory");
        std::fs::create_dir(&root).expect("isolated kit root");
        std::fs::create_dir(&external).expect("external directory fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-conversations"))
            .expect("malicious directory symlink fixture");
        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );

        assert_eq!(
            save_conversation_at(&root, &conversation),
            Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory),
        );
        assert_eq!(
            conversation_exists_at(&root, "safe-session"),
            Err(AgentChatConversationPersistenceError::UnsafeConversationDirectory),
        );
        assert!(load_conversation_at(&root, "safe-session").is_err());
        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert!(!external.join("safe-session.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn conversation_delete_preflights_symlinked_index_before_touching_saved_session() {
        let temp = tempfile::tempdir().expect("isolated session fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        let conversation = make_conversation(
            "safe-session",
            "2026-04-01T18:00:00Z",
            vec![("user", "private question")],
        );
        save_conversation_at(&root, &conversation).expect("safe saved conversation fixture");
        let external = temp.path().join("unrelated-private-index.jsonl");
        std::fs::write(&external, "external history secrets").expect("external index fixture");
        std::os::unix::fs::symlink(&external, root.join("agent_chat-history.jsonl"))
            .expect("malicious index symlink fixture");

        assert!(delete_conversation_at(&root, "safe-session").is_err());
        assert!(rename_conversation_at(&root, "safe-session", "Wrong Title").is_err());
        let entry = build_history_entry(&conversation).expect("synthetic index entry");
        assert_eq!(
            save_history_entry_at(&root, &entry),
            Err(AgentChatConversationPersistenceError::UnsafeFileTarget),
        );
        let original = load_conversation_at(&root, "safe-session")
            .expect("original session remains readable")
            .expect("original session remains on disk");
        assert!(original.custom_title.is_none());
        assert_eq!(
            std::fs::read_to_string(external).expect("external index remains untouched"),
            "external history secrets",
        );
    }

    #[test]
    fn rename_conversation_updates_saved_conversation_and_index() {
        let _guard = history_env_lock().lock().expect("history env lock");
        let previous_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
        let temp = tempfile::tempdir().expect("temp dir");
        std::env::set_var(crate::setup::SK_PATH_ENV, temp.path());

        let conv = make_conversation(
            "rename-1",
            "2026-04-01T18:00:00Z",
            vec![("user", "please debug auth"), ("assistant", "I found it")],
        );
        save_completed_conversation(&conv, &build_history_entry(&conv).expect("entry"))
            .expect("persist complete conversation and private index");

        rename_conversation("rename-1", r#"" Auth Debugging Plan! ""#).expect("rename");
        let saved = load_conversation("rename-1").expect("saved conversation");
        assert_eq!(saved.custom_title.as_deref(), Some("Auth Debugging Plan"));

        let entries = load_history();
        let entry = entries
            .iter()
            .find(|entry| entry.session_id == "rename-1")
            .expect("history entry");
        assert_eq!(entry.title_display(), "Auth Debugging Plan");

        match previous_sk_path {
            Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
            None => std::env::remove_var(crate::setup::SK_PATH_ENV),
        }
        invalidate_history_cache();
    }

    // ── build_history_entry ─────────────────────────────────────────

    #[test]
    fn build_entry_populates_title_preview_search_text() {
        let conv = make_conversation(
            "build-1",
            "2026-04-01T10:00:00Z",
            vec![
                ("user", "help me fix login"),
                (
                    "assistant",
                    "The root cause is an expired OAuth redirect URI",
                ),
            ],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert_eq!(entry.title, "help me fix login");
        assert!(entry.custom_title.is_none());
        assert!(entry.preview.contains("expired OAuth redirect URI"));
        assert!(entry.search_text.contains("oauth"));
        assert!(entry.search_text.contains("redirect"));
        assert_eq!(entry.message_count, 2);
    }

    #[test]
    fn build_entry_returns_none_without_user_message() {
        let conv = make_conversation(
            "no-user",
            "2026-04-01T10:00:00Z",
            vec![("assistant", "hello")],
        );
        assert!(build_history_entry(&conv).is_none());
    }

    #[test]
    fn build_entry_uses_first_user_for_preview_when_no_assistant() {
        let conv = make_conversation(
            "user-only",
            "2026-04-01T10:00:00Z",
            vec![("user", "just a question")],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert_eq!(entry.preview, "just a question");
    }

    #[test]
    fn build_entry_truncates_title_at_100_chars() {
        let long_msg = "a".repeat(200);
        let conv = make_conversation(
            "long-title",
            "2026-04-01T10:00:00Z",
            vec![("user", &long_msg)],
        );
        let entry = build_history_entry(&conv).expect("should build");
        // 100 chars + ellipsis
        assert!(entry.title.chars().count() <= 101);
        assert!(entry.title.ends_with('\u{2026}'));
    }

    #[test]
    fn build_entry_truncates_preview_at_160_chars() {
        let long_reply = "b".repeat(300);
        let conv = make_conversation(
            "long-preview",
            "2026-04-01T10:00:00Z",
            vec![("user", "question"), ("assistant", &long_reply)],
        );
        let entry = build_history_entry(&conv).expect("should build");
        assert!(entry.preview.chars().count() <= 161);
    }

    // ── title_display / preview_display ─────────────────────────────

    #[test]
    fn title_display_falls_back_to_first_message() {
        let entry = AgentChatHistoryEntry {
            first_message: "fallback title".to_string(),
            ..Default::default()
        };
        assert_eq!(entry.title_display(), "fallback title");

        let custom = AgentChatHistoryEntry {
            first_message: "ignored".to_string(),
            title: "heuristic title".to_string(),
            custom_title: Some("Custom Title".to_string()),
            ..Default::default()
        };
        assert_eq!(custom.title_display(), "Custom Title");

        let entry2 = AgentChatHistoryEntry {
            first_message: "ignored".to_string(),
            title: "real title".to_string(),
            ..Default::default()
        };
        assert_eq!(entry2.title_display(), "real title");
    }

    #[test]
    fn preview_display_falls_back_to_first_message() {
        let entry = AgentChatHistoryEntry {
            first_message: "fallback preview".to_string(),
            ..Default::default()
        };
        assert_eq!(entry.preview_display(), "fallback preview");
    }

    // ── Text helpers ────────────────────────────────────────────────

    #[test]
    fn collapse_whitespace_normalizes() {
        assert_eq!(collapse_whitespace("  a  b  c  "), "a b c");
        assert_eq!(collapse_whitespace("hello\n\nworld"), "hello world");
    }

    #[test]
    fn truncate_chars_adds_ellipsis() {
        assert_eq!(truncate_chars("abcde", 3), "abc\u{2026}");
        assert_eq!(truncate_chars("ab", 5), "ab");
    }

    // ── rank_history_entries / search ────────────────────────────────

    fn sample_entries() -> Vec<AgentChatHistoryEntry> {
        vec![
            AgentChatHistoryEntry {
                timestamp: "2026-04-01T10:00:00Z".to_string(),
                first_message: "help me fix login".to_string(),
                message_count: 4,
                session_id: "s1".to_string(),
                title: "help me fix login".to_string(),
                custom_title: None,
                preview: "The root cause is an expired OAuth redirect URI".to_string(),
                search_text: normalize_search_text(
                    "help me fix login\nThe root cause is an expired OAuth redirect URI\nuser: help me fix login\nassistant: The root cause is an expired OAuth redirect URI",
                ),
            },
            AgentChatHistoryEntry {
                timestamp: "2026-04-02T10:00:00Z".to_string(),
                first_message: "add dark mode".to_string(),
                message_count: 3,
                session_id: "s2".to_string(),
                title: "add dark mode".to_string(),
                custom_title: None,
                preview: "I added CSS variables for theming".to_string(),
                search_text: normalize_search_text(
                    "add dark mode\nI added CSS variables for theming\nuser: add dark mode\nassistant: I added CSS variables for theming",
                ),
            },
            AgentChatHistoryEntry {
                timestamp: "2026-04-03T10:00:00Z".to_string(),
                first_message: "review PR 42".to_string(),
                message_count: 6,
                session_id: "s3".to_string(),
                title: "review PR 42".to_string(),
                custom_title: None,
                preview: "The PR looks good but the OAuth scope is too broad".to_string(),
                search_text: normalize_search_text(
                    "review PR 42\nThe PR looks good but the OAuth scope is too broad\nuser: review PR 42\nassistant: The PR looks good but the OAuth scope is too broad",
                ),
            },
        ]
    }

    #[test]
    fn empty_query_returns_all_up_to_limit() {
        let hits = rank_history_entries(sample_entries(), "", 100);
        assert_eq!(hits.len(), 3);
        // All scores should be 0 for empty query.
        assert!(hits.iter().all(|h| h.score == 0));
    }

    #[test]
    fn search_matches_later_transcript_content() {
        let hits = rank_history_entries(sample_entries(), "oauth redirect", 10);
        // "oauth redirect" appears in s1's preview and s3's preview.
        assert!(!hits.is_empty());
        // s1 has "redirect" in preview AND search_text → higher score.
        assert_eq!(hits[0].entry.session_id, "s1");
    }

    #[test]
    fn search_excludes_non_matching_entries() {
        let hits = rank_history_entries(sample_entries(), "nonexistent xyz", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn search_is_case_insensitive() {
        let hits = rank_history_entries(sample_entries(), "OAUTH", 10);
        assert!(!hits.is_empty());
    }

    #[test]
    fn search_multi_token_requires_all_tokens() {
        // "dark" matches s2, "oauth" matches s1/s3 → no entry has both.
        let hits = rank_history_entries(sample_entries(), "dark oauth", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn search_title_prefix_scores_highest() {
        let hits = rank_history_entries(sample_entries(), "help", 10);
        assert_eq!(hits[0].entry.session_id, "s1");
        assert_eq!(hits[0].matched_field, AgentChatHistorySearchField::Title);
    }

    #[test]
    fn search_respects_limit() {
        let hits = rank_history_entries(sample_entries(), "oauth", 1);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn search_recency_breaks_ties() {
        // Both s1 and s3 match "oauth", but with different scores.
        // If scores tied, s3 (later timestamp) would come first.
        let mut entries = sample_entries();
        // Make s1 and s3 have identical search_text so score is equal.
        let shared_text = normalize_search_text("oauth common content");
        entries[0].search_text = shared_text.clone();
        entries[0].title = "oauth common content".to_string();
        entries[0].preview = "oauth common content".to_string();
        entries[2].search_text = shared_text;
        entries[2].title = "oauth common content".to_string();
        entries[2].preview = "oauth common content".to_string();

        let hits = rank_history_entries(entries, "oauth", 10);
        assert!(hits.len() >= 2);
        // s3 has later timestamp → should come first when scores tie.
        assert_eq!(hits[0].entry.session_id, "s3");
        assert_eq!(hits[1].entry.session_id, "s1");
    }

    #[test]
    fn search_whitespace_only_query_returns_all() {
        let hits = rank_history_entries(sample_entries(), "   ", 100);
        assert_eq!(hits.len(), 3);
    }

    /// Screenshot regression (2026-07-11): "what are the" must not surface
    /// conversations whose only hits are stopwords scattered mid-word or
    /// across distant transcript turns.
    #[test]
    fn sentence_query_rejects_scattered_stopword_noise() {
        let noise = AgentChatHistoryEntry {
            timestamp: "2026-07-11T10:00:00Z".to_string(),
            first_message: "Explain keyboard-first macOS launchers".to_string(),
            message_count: 5,
            session_id: "noise".to_string(),
            title: "Explain keyboard-first macOS launchers".to_string(),
            custom_title: None,
            preview: "Somewhat shared themes and other reports are generated".to_string(),
            search_text: normalize_search_text(
                "Explain keyboard-first macOS launchers\nuser: what happened\nassistant: many unrelated words separate everything here from anything useful and more filler keeps going until eventually are appears and then much later after so much more filler text the final token shows up: the",
            ),
        };
        let phrase = AgentChatHistoryEntry {
            timestamp: "2026-07-01T10:00:00Z".to_string(),
            first_message: "What are the release criteria?".to_string(),
            message_count: 2,
            session_id: "phrase".to_string(),
            title: "What are the release criteria?".to_string(),
            custom_title: None,
            preview: "Ship gates are green".to_string(),
            search_text: normalize_search_text("What are the release criteria?"),
        };

        let hits = rank_history_entries(vec![noise, phrase], "what are the", 10);
        assert_eq!(hits.len(), 1, "only the visible phrase row qualifies");
        assert_eq!(hits[0].entry.session_id, "phrase");
        let evidence = hits[0].evidence.as_ref().expect("evidence present");
        assert!(
            !evidence.title_indices.is_empty(),
            "phrase match highlights the title words"
        );
    }

    /// Hidden transcript matches still qualify, rank below visible phrase
    /// rows, and carry an excerpt explaining why they matched.
    #[test]
    fn hidden_transcript_match_ranks_below_visible_and_carries_excerpt() {
        let hidden = AgentChatHistoryEntry {
            timestamp: "2026-07-11T10:00:00Z".to_string(),
            first_message: "Planning session".to_string(),
            message_count: 8,
            session_id: "hidden".to_string(),
            title: "Planning session".to_string(),
            custom_title: None,
            preview: "Sounds good, next steps agreed".to_string(),
            search_text: normalize_search_text(
                "Planning session\nuser: so what are the migration constraints for launch\nassistant: mostly disk budget",
            ),
        };
        let visible = AgentChatHistoryEntry {
            timestamp: "2026-06-01T10:00:00Z".to_string(),
            first_message: "What are the migration constraints?".to_string(),
            message_count: 2,
            session_id: "visible".to_string(),
            title: "What are the migration constraints?".to_string(),
            custom_title: None,
            preview: "Disk budget mostly".to_string(),
            search_text: normalize_search_text("What are the migration constraints?"),
        };

        let hits = rank_history_entries(vec![hidden, visible], "what are the migration", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].entry.session_id, "visible",
            "visible phrase must outrank hidden transcript despite being older"
        );
        let hidden_evidence = hits[1].evidence.as_ref().expect("evidence present");
        assert!(hidden_evidence.title_indices.is_empty());
        let excerpt = hidden_evidence
            .hidden_excerpt
            .as_ref()
            .expect("hidden match explains itself");
        assert!(excerpt.text.contains("migration"));
    }
}
