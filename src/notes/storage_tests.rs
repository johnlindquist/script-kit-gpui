#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root_owned_notes_options() -> RootNotesSectionOptions {
        RootNotesSectionOptions {
            enabled: true,
            max_results: 5,
            min_query_chars: 0,
            search_content: true,
        }
    }

    fn root_owned_note_hit(title: &str) -> RootNoteSearchHit {
        RootNoteSearchHit {
            id: NoteId::new(),
            title: title.to_owned(),
            updated_at: Utc::now(),
            is_pinned: false,
            char_count: title.chars().count(),
            score: 100,
        }
    }

    #[test]
    fn root_notes_owned_result_limit_honors_explicit_sources_without_unbounded_reads() {
        let mut options = root_owned_notes_options();
        options.max_results = 3;
        assert_eq!(root_notes_search_result_limit(options), 3);
        options.max_results = 8;
        assert_eq!(root_notes_search_result_limit(options), 8);
        options.max_results = 12;
        assert_eq!(root_notes_search_result_limit(options), 12);
        options.max_results = usize::MAX;
        assert_eq!(root_notes_search_result_limit(options), 24);
        options.max_results = 0;
        assert_eq!(root_notes_search_result_limit(options), 1);
    }

    #[test]
    fn root_notes_owned_refresh_accepts_empty_snapshot_without_restarting_forever() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let refresh =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "private query", options, 41)
                .expect("one exact Notes owner for a cold query");
        assert!(try_begin_root_notes_search_refresh_in_cache(
            &mut cache,
            "private query",
            options,
            41,
        )
        .is_none());

        let flight = refresh.flight.clone();
        assert!(finish_root_notes_search_refresh_in_cache(
            &mut cache,
            refresh,
            RootNotesSearchSnapshot {
                flight: flight.clone(),
                hits: Ok(Vec::new()),
            },
            41,
        ));
        assert!(cache
            .hits_by_query
            .get(&flight.search)
            .is_some_and(|cached| cached.hits.is_empty()));
        assert!(cache.in_flight.is_empty());
        assert!(try_begin_root_notes_search_refresh_in_cache(
            &mut cache,
            "private query",
            options,
            41,
        )
        .is_none());
    }

    #[test]
    fn fresh_notes_cache_proof_requires_exact_eligible_key_epoch_and_no_worker() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        assert!(fresh_root_notes_search_cache_status(&cache, "query", options, 7).is_none());
        let refresh =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 7).unwrap();
        let flight = refresh.flight.clone();
        assert!(finish_root_notes_search_refresh_in_cache(
            &mut cache,
            refresh,
            RootNotesSearchSnapshot {
                flight,
                hits: Ok(Vec::new())
            },
            7,
        ));
        assert_eq!(
            fresh_root_notes_search_cache_status(&cache, "query", options, 7),
            Some((7, 0))
        );
        assert!(fresh_root_notes_search_cache_status(&cache, "query", options, 8).is_none());
        assert!(fresh_root_notes_search_cache_status(&cache, "other", options, 7).is_none());
        assert!(fresh_root_notes_search_cache_status(
            &cache,
            "query",
            RootNotesSectionOptions {
                enabled: false,
                ..options
            },
            7
        )
        .is_none());
        assert!(fresh_root_notes_search_cache_status(
            &cache,
            "query",
            RootNotesSectionOptions {
                search_content: false,
                ..options
            },
            7
        )
        .is_none());
        let _worker =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "other", options, 7).unwrap();
        assert!(fresh_root_notes_search_cache_status(&cache, "query", options, 7).is_none());
    }

    #[test]
    fn root_notes_owned_refresh_rejects_stale_epoch_and_releases_current_worker() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let refresh = try_begin_root_notes_search_refresh_in_cache(
            &mut cache,
            "changed private note",
            options,
            7,
        )
        .expect("old Notes cache epoch owns one worker");
        let flight = refresh.flight.clone();

        assert!(!finish_root_notes_search_refresh_in_cache(
            &mut cache,
            refresh,
            RootNotesSearchSnapshot {
                flight,
                hits: Ok(vec![root_owned_note_hit("stale private note")]),
            },
            8,
        ));
        assert!(cache.hits_by_query.is_empty());
        assert!(cache.in_flight.is_empty());
        let replacement = try_begin_root_notes_search_refresh_in_cache(
            &mut cache,
            "changed private note",
            options,
            8,
        )
        .expect("new cache epoch can immediately recover");
        assert_eq!(replacement.flight.generation, 8);
    }

    #[test]
    fn root_notes_owned_refresh_rejects_foreign_owner_and_wrong_query_before_publication() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let refresh = try_begin_root_notes_search_refresh_in_cache(
            &mut cache,
            "real private query",
            options,
            12,
        )
        .expect("Notes owns the exact query");
        let mut foreign = refresh.clone();
        foreign.owner.source = sk_protocol::command_contract::CommandSource::Todo;
        assert!(!finish_root_notes_search_refresh_in_cache(
            &mut cache,
            foreign.clone(),
            RootNotesSearchSnapshot {
                flight: foreign.flight.clone(),
                hits: Ok(vec![root_owned_note_hit("foreign private note")]),
            },
            12,
        ));
        assert!(!discard_root_notes_search_refresh_in_cache(
            &mut cache, foreign,
        ));

        let mut wrong_query = refresh.flight.clone();
        wrong_query.search.query = "another private query".to_owned();
        assert!(!finish_root_notes_search_refresh_in_cache(
            &mut cache,
            refresh.clone(),
            RootNotesSearchSnapshot {
                flight: wrong_query,
                hits: Ok(vec![root_owned_note_hit("wrong private note")]),
            },
            12,
        ));
        assert!(cache.hits_by_query.is_empty());
        assert_eq!(cache.refresh_lifecycle.in_flight, Some(refresh.owner));
        assert!(discard_root_notes_search_refresh_in_cache(
            &mut cache, refresh,
        ));
    }

    #[test]
    fn root_notes_failed_read_is_not_empty_success_and_preserves_last_good_rows() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let refresh =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 1).unwrap();
        let key = refresh.flight.search.clone();
        cache.hits_by_query.insert(
            key.clone(),
            RootNotesCachedHits {
                generation: 1,
                hits: vec![root_owned_note_hit("last good")],
            },
        );
        let snapshot = RootNotesSearchSnapshot {
            flight: refresh.flight.clone(),
            hits: Err(anyhow::anyhow!("read failed")),
        };
        assert!(snapshot.read_outcome().is_err());
        assert!(!finish_root_notes_search_refresh_in_cache(
            &mut cache, refresh, snapshot, 1
        ));
        assert_eq!(cache.hits_by_query[&key].hits[0].title, "last good");
        assert!(cache.refresh_lifecycle.in_flight.is_none());
        assert!(cache.in_flight.is_empty());
    }

    #[test]
    fn root_notes_stale_discard_cannot_release_replacement_worker() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let stale =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 1).unwrap();
        assert!(discard_root_notes_search_refresh_in_cache(
            &mut cache,
            stale.clone()
        ));
        let current =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 1).unwrap();
        assert!(!discard_root_notes_search_refresh_in_cache(
            &mut cache, stale
        ));
        assert_eq!(cache.refresh_lifecycle.in_flight, Some(current.owner));
        assert!(cache.in_flight.contains(&current.flight));
    }

    #[test]
    fn root_notes_freshness_generation_preserves_rows_until_replacement() {
        let mut cache = RootNotesSearchCache::default();
        let options = root_owned_notes_options();
        let key = root_notes_search_cache_key("query", options);
        cache.hits_by_query.insert(
            key.clone(),
            RootNotesCachedHits {
                generation: 1,
                hits: vec![root_owned_note_hit("old title")],
            },
        );
        assert!(
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 1).is_none()
        );
        let refresh =
            try_begin_root_notes_search_refresh_in_cache(&mut cache, "query", options, 2).unwrap();
        assert_eq!(cache.hits_by_query[&key].hits[0].title, "old title");
        let snapshot = RootNotesSearchSnapshot {
            flight: refresh.flight.clone(),
            hits: Ok(Vec::new()),
        };
        assert!(finish_root_notes_search_refresh_in_cache(
            &mut cache, refresh, snapshot, 2
        ));
        assert!(cache.hits_by_query[&key].hits.is_empty());
        assert_eq!(cache.hits_by_query[&key].generation, 2);
    }

    #[test]
    fn root_notes_owned_cache_only_lookup_never_creates_an_unowned_worker() {
        let query = format!("isolated-private-cache-{}", uuid::Uuid::new_v4());
        let options = root_owned_notes_options();
        assert!(search_root_notes_meta_cached(&query, options).is_empty());

        let key = root_notes_search_cache_key(&query, options);
        let cache = root_notes_search_cache()
            .lock()
            .expect("inspect cache-only Notes lookup");
        assert!(!cache.hits_by_query.contains_key(&key));
        assert!(!cache.in_flight.iter().any(|flight| flight.search == key));
    }

    #[test]
    fn root_notes_owned_failure_event_never_emits_raw_private_query_or_database_error() {
        #[derive(Clone)]
        struct EventWriter(Arc<Mutex<Vec<u8>>>);

        impl std::io::Write for EventWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let query = "my private cancer journal and sk-live-secret";
        let error_text = "/Users/private/medical/notes.sqlite: provider password hunter2";
        let error = anyhow::anyhow!(error_text);
        let expected_query = crate::logging::log_private_user_value(query);
        let expected_error = crate::logging::log_private_user_value(error_text);
        let captured = Arc::new(Mutex::new(Vec::new()));
        let writer = Arc::clone(&captured);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || EventWriter(Arc::clone(&writer)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            log_notes_search_completed(query, 2, "metadata_only");
            log_notes_search_fts_fallback(query, &error);
            log_root_notes_search_failure(query, &error);
        });

        let output = String::from_utf8(
            captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        )
        .expect("structured Notes failure event");
        assert!(!output.contains(query));
        assert!(!output.contains(error_text));
        assert!(!output.contains("cancer"));
        assert!(!output.contains("hunter2"));
        let events: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("actual JSON tracing event"))
            .collect();
        assert_eq!(events.len(), 3);
        for event in &events {
            assert_eq!(event["fields"]["query_bytes"], query.len());
            assert_eq!(event["fields"]["query_sha256"], expected_query.sha256);
        }
        assert_eq!(events[0]["fields"]["message"], "Note search completed");
        assert_eq!(events[0]["fields"]["method"], "metadata_only");
        assert_eq!(events[0]["fields"]["count"], 2);
        for event in &events[1..] {
            assert_eq!(event["fields"]["error_bytes"], error_text.len());
            assert_eq!(event["fields"]["error_sha256"], expected_error.sha256);
        }
        assert_eq!(
            events[1]["fields"]["message"],
            "FTS search failed, using LIKE fallback"
        );
        assert_eq!(events[2]["fields"]["message"], "root_notes_search_failed");
    }

    #[cfg(unix)]
    #[test]
    fn canonical_note_private_reader_repairs_legacy_permissions_before_parsing() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = tempfile::tempdir().expect("isolated canonical note fixture");
        let substrate = BrainSubstrate::new(fixture.path().join("brain"));
        let path = substrate.paths().note_file("legacy-private-note");
        let note_id = NoteId::new();
        let now = Utc::now();
        substrate
            .write_document(
                &path,
                &BrainFrontmatter::new(note_id, now, now),
                "# Private note\nSensitive body",
            )
            .expect("seed canonical note");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        let (note, slug, _) = load_note_from_file(&substrate, &path, None, 0)
            .expect("repair private note before reading");
        assert_eq!(note.id, note_id);
        assert_eq!(slug, "legacy-private-note");
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonical_note_private_owners_reject_hostile_symlinks_without_foreign_mutation() {
        use std::os::unix::fs::symlink;

        let fixture = tempfile::tempdir().expect("isolated canonical note symlink fixture");
        let substrate = BrainSubstrate::new(fixture.path().join("brain"));
        fs::create_dir_all(substrate.paths().notes_dir()).unwrap();
        let foreign = fixture.path().join("foreign-private-note.md");
        fs::write(&foreign, "foreign note must never be parsed or replaced").unwrap();
        let planted = substrate.paths().note_file("planted");
        symlink(&foreign, &planted).expect("plant hostile canonical note symlink");

        assert!(load_note_from_file(&substrate, &planted, None, 0).is_err());
        assert!(guard_external_edit_before_write(&planted, NoteId::new()).is_err());
        let replacement = Note::with_content("# Never replace\nPrivate body");
        assert!(write_canonical_note_file(&substrate, &replacement, "planted").is_err());
        assert_eq!(
            fs::read_to_string(&foreign).unwrap(),
            "foreign note must never be parsed or replaced"
        );
        assert!(fs::symlink_metadata(planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn conflict_copy_preserves_every_same_second_recovery_artifact() {
        let root = tempfile::tempdir().expect("isolated note conflict fixture");
        let original = root.path().join("private-note.md");
        let timestamp = "20260822123456";
        let first = write_conflict_copy_at(&original, "first private version", timestamp)
            .unwrap()
            .unwrap();
        let second = write_conflict_copy_at(&original, "second private version", timestamp)
            .unwrap()
            .unwrap();
        let third = write_conflict_copy_at(&original, "third private version", timestamp)
            .unwrap()
            .unwrap();

        assert_eq!(
            first.file_name().unwrap().to_str().unwrap(),
            "private-note.conflict-20260822123456.md"
        );
        assert_eq!(
            second.file_name().unwrap().to_str().unwrap(),
            "private-note.conflict-20260822123456-2.md"
        );
        assert_eq!(
            third.file_name().unwrap().to_str().unwrap(),
            "private-note.conflict-20260822123456-3.md"
        );
        assert_eq!(fs::read_to_string(&first).unwrap(), "first private version");
        assert_eq!(
            fs::read_to_string(&second).unwrap(),
            "second private version"
        );
        assert_eq!(fs::read_to_string(&third).unwrap(), "third private version");
        assert!([first, second, third]
            .iter()
            .all(|path| is_conflict_copy_path(path)));
    }

    #[cfg(unix)]
    #[test]
    fn conflict_copy_refuses_symlink_redirection_without_destroying_private_target() {
        let root = tempfile::tempdir().expect("isolated note conflict fixture");
        let original = root.path().join("private-note.md");
        let target = root.path().join("unrelated-private.txt");
        fs::write(&target, "preserve unrelated private data").unwrap();
        let hostile = root.path().join("private-note.conflict-20260822123456.md");
        std::os::unix::fs::symlink(&target, &hostile).expect("hostile conflict symlink");

        let safe = write_conflict_copy_at(&original, "new private note", "20260822123456")
            .unwrap()
            .unwrap();
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "preserve unrelated private data"
        );
        assert_eq!(
            safe.file_name().unwrap().to_str().unwrap(),
            "private-note.conflict-20260822123456-2.md"
        );
        assert_eq!(fs::read_to_string(safe).unwrap(), "new private note");
        assert!(fs::symlink_metadata(hostile)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn conflict_copy_is_owner_only_from_its_first_creation() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("isolated note conflict fixture");
        let path = write_conflict_copy_at(
            &root.path().join("private-note.md"),
            "sensitive private note",
            "20260822123456",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(path).unwrap(), "sensitive private note");
    }

    /// A corrupt notes.sqlite must be moved aside and replaced with a fresh,
    /// working database — never surfaced as an error (which breaks deeplink
    /// open, search, and MCP note tools) or as a silently empty Notes list.
    /// Markdown is canonical, so the caller rebuilds the index when the
    /// recovery flag is true.
    #[test]
    fn open_or_recover_notes_db_moves_corrupt_db_aside_and_starts_fresh() {
        let dir = std::env::temp_dir().join(format!(
            "sk_notes_recovery_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let db_path = dir.join("notes.sqlite");
        fs::write(&db_path, b"this is not a sqlite database").expect("write garbage db");

        let (conn, recovered) =
            open_or_recover_notes_db(&db_path).expect("recovery must succeed on corrupt db");
        assert!(recovered, "corrupt db must be reported as recovered");

        // The damaged file was preserved aside, not deleted.
        let corrupt_siblings = fs::read_dir(&dir)
            .expect("read temp dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(corrupt_siblings, 1, "damaged db must be moved aside");

        // The fresh connection is usable with the notes schema in place.
        let note_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .expect("fresh db must have the notes schema");
        assert_eq!(note_count, 0);

        // A healthy db must open without triggering recovery.
        drop(conn);
        let (_conn, recovered_again) =
            open_or_recover_notes_db(&db_path).expect("healthy reopen must succeed");
        assert!(
            !recovered_again,
            "healthy db must not be treated as corrupt"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn open_or_recover_notes_db_rejects_hostile_symlink_without_replacing_foreign_notes() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("isolated notes recovery fixture");
        let path = directory.path().join("notes.sqlite");
        let foreign = directory.path().join("foreign.sqlite");
        fs::write(&foreign, b"foreign private note titles and bodies")
            .expect("seed foreign Notes owner");
        symlink(&foreign, &path).expect("plant Notes recovery symlink");

        assert!(open_or_recover_notes_db(&path).is_err());
        assert_eq!(
            fs::read(&foreign).expect("foreign Notes owner remains untouched"),
            b"foreign private note titles and bodies"
        );
        assert!(fs::symlink_metadata(&path)
            .expect("hostile Notes link remains available for repair")
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_dir(directory.path())
                .expect("inspect isolated Notes directory")
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
                .count(),
            0
        );
    }

    fn unique_test_token(prefix: &str) -> String {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        format!(
            "{prefix}_{millis}_{}",
            NoteId::new().as_str().replace('-', "")
        )
    }

    fn text_cart_item(
        note_id: NoteId,
        id: String,
        label: String,
        text: String,
        sort_order: i32,
    ) -> crate::notes::model::NoteCartItem {
        let now = Utc::now();
        crate::notes::model::NoteCartItem {
            id,
            note_id,
            label,
            payload: crate::notes::model::NoteCartItemPayload::Text {
                text,
                source: "agentic://notes-cart-rebuild-test".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
            created_at: now,
            updated_at: now,
            sort_order,
        }
    }

    #[test]
    fn test_db_path() {
        let path = get_notes_db_path();
        assert!(path.to_string_lossy().contains("notes.sqlite"));
    }

    #[test]
    fn test_search_notes_handles_special_characters() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize for special-character search");

        // Search with special characters should not error (even if no results)
        // These are FTS5 special characters that can break MATCH queries
        let special_queries = [
            "test@example.com", // @ symbol
            "foo*bar",          // wildcard
            "hello\"world",     // quote
            "foo:bar",          // colon (FTS column prefix syntax)
            "(test)",           // parentheses
            "test^2",           // caret (boost syntax)
            "test-query",       // hyphen (can be operator)
            "'test'",           // single quotes
            "test AND OR NOT",  // operators
        ];

        for query in special_queries {
            let result = search_notes(query);
            assert!(
                result.is_ok(),
                "Search with '{}' should not error: {:?}",
                query,
                result.err()
            );
        }
    }

    #[test]
    fn test_notes_au_trigger_has_when_guard_for_real_content_changes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trigger inspection");

        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        let trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'notes_au'",
                [],
                |row| row.get(0),
            )
            .expect("notes_au trigger should exist");

        assert!(
            trigger_sql.contains("WHEN OLD.title <> NEW.title OR OLD.content <> NEW.content"),
            "notes_au trigger should only fire when title/content differ: {trigger_sql}"
        );
    }

    #[test]
    fn test_init_notes_db_recreates_triggers_for_existing_connection() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trigger recreation");

        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        // Install a legacy unguarded trigger to simulate stale schema
        conn.execute_batch(
            r#"
            DROP TRIGGER IF EXISTS notes_au;
            CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
                INSERT INTO notes_fts(notes_fts, rowid, title, content)
                VALUES('delete', OLD.rowid, OLD.title, OLD.content);
                INSERT INTO notes_fts(rowid, title, content)
                VALUES (NEW.rowid, NEW.title, NEW.content);
            END;
            "#,
        )
        .expect("should install legacy notes_au trigger");
        drop(conn);

        // Re-init should verify schema and recreate triggers
        init_notes_db().expect("re-init should verify schema and recreate triggers");

        let db = get_db().expect("notes db should still be initialized");
        let conn = db.lock().expect("notes db lock should still succeed");

        let trigger_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'notes_au'",
                [],
                |row| row.get(0),
            )
            .expect("notes_au trigger should exist after re-init");

        assert!(
            trigger_sql.contains("WHEN OLD.title <> NEW.title OR OLD.content <> NEW.content"),
            "re-init should restore the guarded notes_au trigger: {trigger_sql}"
        );
    }

    #[test]
    fn test_search_notes_limits_fts_results_to_200() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before search limit test");
        let token = unique_test_token("search_limit");
        let now = Utc::now();
        let mut note_ids = Vec::new();

        for index in 0..220 {
            let note = Note {
                id: NoteId::new(),
                title: format!("{token} title {index}"),
                content: format!("{token} content {index}"),
                created_at: now,
                updated_at: now,
                deleted_at: None,
                is_pinned: false,
                sort_order: index,
            };

            save_note(&note).expect("failed to save note for search limit test");
            note_ids.push(note.id);
        }

        let results = search_notes(&token).expect("search should succeed");

        for id in note_ids {
            delete_note_permanently(id).expect("cleanup failed for search limit test");
        }

        assert!(
            results.len() <= 200,
            "search should cap FTS results at 200, got {}",
            results.len()
        );
    }

    #[test]
    fn test_root_notes_query_eligibility_respects_config() {
        let options = RootNotesSectionOptions {
            enabled: true,
            min_query_chars: 3,
            ..Default::default()
        };

        assert!(root_notes_query_is_eligible("fix", options));
        assert!(!root_notes_query_is_eligible("fi", options));
        assert!(!root_notes_query_is_eligible("fix\nnote", options));
        assert!(!root_notes_query_is_eligible(
            "fix",
            RootNotesSectionOptions {
                enabled: false,
                ..options
            }
        ));
    }

    #[test]
    fn test_search_root_notes_meta_is_bounded_active_only_and_metadata_only() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before root notes search test");
        let token = unique_test_token("root_notes");
        let now = Utc::now();
        let active = Note {
            id: NoteId::new(),
            title: format!("{token} active"),
            content: format!("{token} body that must not be returned"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: true,
            sort_order: 0,
        };
        let deleted = Note {
            id: NoteId::new(),
            title: format!("{token} deleted"),
            content: format!("{token} deleted body"),
            created_at: now,
            updated_at: now,
            deleted_at: Some(now),
            is_pinned: false,
            sort_order: 1,
        };

        save_note(&active).expect("failed to save active note");
        save_note(&deleted).expect("failed to save deleted note");

        let hits = search_root_notes_meta(
            &token,
            RootNotesSectionOptions {
                enabled: true,
                max_results: 1,
                min_query_chars: 3,
                search_content: true,
            },
        );

        delete_note_permanently(active.id).expect("cleanup failed for active note");
        delete_note_permanently(deleted.id).expect("cleanup failed for deleted note");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, active.id);
        assert_eq!(hits[0].title, active.title);
        assert!(hits[0].is_pinned);
        assert_eq!(hits[0].char_count, active.content.chars().count());
    }

    #[test]
    fn test_search_root_notes_meta_matches_title_substrings_when_fts_has_no_hit() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before root notes substring test");
        let now = Utc::now();
        let note = Note {
            id: NoteId::new(),
            title: "Welcome to Notes".to_string(),
            content: "Starter content for source-filter search.".to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: true,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save welcome note");

        let hits = search_root_notes_meta(
            "not",
            RootNotesSectionOptions {
                enabled: true,
                max_results: 5,
                min_query_chars: 0,
                search_content: true,
            },
        );

        delete_note_permanently(note.id).expect("cleanup failed for welcome note");

        assert!(
            hits.iter()
                .any(|candidate| candidate.id == note.id && candidate.title == "Welcome to Notes"),
            "root note search should treat `not` as a substring/prefix match for `Notes`"
        );
    }

    #[test]
    fn test_delete_all_deleted_notes_removes_soft_deleted_notes_in_batch() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before batch delete test");
        let token = unique_test_token("batch_delete");
        let now = Utc::now();

        let deleted_note = Note {
            id: NoteId::new(),
            title: format!("{token} deleted"),
            content: format!("{token} deleted content"),
            created_at: now,
            updated_at: now,
            deleted_at: Some(now),
            is_pinned: false,
            sort_order: 0,
        };
        save_note(&deleted_note).expect("failed to save soft-deleted note");

        let active_note = Note {
            id: NoteId::new(),
            title: format!("{token} active"),
            content: format!("{token} active content"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 1,
        };
        save_note(&active_note).expect("failed to save active note");

        delete_all_deleted_notes().expect("batch delete should succeed");

        let deleted_result = get_note(deleted_note.id).expect("query deleted note should succeed");
        let active_result = get_note(active_note.id).expect("query active note should succeed");

        delete_note_permanently(active_note.id).expect("cleanup failed for active note");

        assert!(
            deleted_result.is_none(),
            "soft-deleted note should be permanently removed by batch delete"
        );
        assert!(
            active_result.is_some(),
            "active note should not be removed by batch delete"
        );
    }

    #[test]
    fn test_rebuild_notes_search_index_recovers_desynced_rows() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before FTS rebuild test");
        let token = unique_test_token("fts_rebuild");
        let now = Utc::now();

        let note = Note {
            id: NoteId::new(),
            title: format!("{token} title"),
            content: format!("{token} content"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save note for fts rebuild test");

        // Manually remove the FTS row to simulate a desynced index
        let db = get_db().expect("notes db should be initialized");
        let conn = db.lock().expect("notes db lock should succeed");

        conn.execute(
            r#"
            INSERT INTO notes_fts(notes_fts, rowid, title, content)
            VALUES(
                'delete',
                (SELECT rowid FROM notes WHERE id = ?1),
                ?2,
                ?3
            )
            "#,
            params![note.id.as_str(), note.title.clone(), note.content.clone()],
        )
        .expect("failed to desync notes_fts row");
        drop(conn);

        // The note should NOT be searchable while desynced
        let missing = search_notes(&token).expect("search before rebuild should succeed");
        assert!(
            missing.iter().all(|candidate| candidate.id != note.id),
            "desynced note should not be searchable before rebuild"
        );

        // Rebuild should restore the index
        rebuild_notes_search_index().expect("fts rebuild should succeed");

        let rebuilt = search_notes(&token).expect("search after rebuild should succeed");
        delete_note_permanently(note.id).expect("cleanup failed for fts rebuild test");

        assert!(
            rebuilt.iter().any(|candidate| candidate.id == note.id),
            "fts rebuild should restore existing rows into notes_fts"
        );
    }

    #[test]
    fn test_search_notes_returns_matching_note_for_special_character_content() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before special-character match test");
        let token = unique_test_token("search_special_match");
        let query = format!("{token}@example.com");
        let now = Utc::now();

        let note = Note {
            id: NoteId::new(),
            title: format!("Contact {query}"),
            content: format!("Reach me at {query}"),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            is_pinned: false,
            sort_order: 0,
        };

        save_note(&note).expect("failed to save note for special character search test");

        // FTS5 index updates may lag under concurrent writes (nextest parallelism).
        // Retry briefly so the test is not flaky.
        let mut results = Vec::new();
        for _ in 0..5 {
            results = search_notes(&query).expect("search should succeed");
            if results.iter().any(|c| c.id == note.id) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        delete_note_permanently(note.id).expect("cleanup failed for special character search test");

        assert!(
            results.iter().any(|candidate| candidate.id == note.id),
            "search should return the note that contains the special-character query"
        );
    }

    #[test]
    fn test_note_metadata_tables_roundtrip() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata roundtrip test");
        let token = unique_test_token("metadata_roundtrip");
        let note = Note::with_content(format!(
            "---\ntags: [{token}, notes/metadata]\naliases: [{token} Alias]\n---\n# Metadata Roundtrip\nBody #{token} [[Missing Target]]"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save note with metadata");
        let tags = get_note_tags(id).expect("metadata tags should be readable");
        let aliases = get_note_aliases(id).expect("metadata aliases should be readable");
        let outbound_count =
            get_note_outbound_link_count(id).expect("outbound links should be countable");

        delete_note_permanently(id).expect("cleanup failed for metadata note");

        assert!(
            tags.iter().any(|tag| tag == &token),
            "frontmatter/hash tag should be indexed"
        );
        assert!(
            aliases
                .iter()
                .any(|alias| alias == &format!("{token} Alias")),
            "frontmatter alias should be indexed"
        );
        assert_eq!(outbound_count, 1);
    }

    #[test]
    fn test_search_notes_matches_tags_and_aliases() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata search test");
        let token = unique_test_token("metadata_search");
        let note = Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Searchable Metadata\nBody"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save searchable metadata note");
        let tag_results = search_notes(&format!("tag:{token}")).expect("tag search should succeed");
        let alias_results =
            search_notes(&format!("alias:{token}-alias")).expect("alias search should succeed");

        delete_note_permanently(id).expect("cleanup failed for metadata search note");

        assert!(tag_results.iter().any(|candidate| candidate.id == id));
        assert!(alias_results.iter().any(|candidate| candidate.id == id));
    }

    #[test]
    fn test_count_active_notes_with_tag_ignores_soft_deleted_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before tag count test");
        let token = unique_test_token("instr_count");
        let mut note = Note::with_content(format!("---\ntags: [{token}]\n---\n# Instruction"));
        let id = note.id;

        save_note(&note).expect("failed to save instruction note");
        let active_count = count_active_notes_with_tag(&token).expect("tag count should succeed");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete instruction note");
        let deleted_count =
            count_active_notes_with_tag(&token).expect("tag count after delete should succeed");

        delete_note_permanently(id).expect("cleanup failed for tag count note");

        assert_eq!(active_count, 1);
        assert_eq!(deleted_count, 0);
    }

    #[test]
    fn test_backlinks_resolve_after_target_note_is_created() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before backlink test");
        let token = unique_test_token("backlink_target");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;

        save_note(&source).expect("failed to save unresolved source link");
        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");

        let backlink_count =
            get_note_backlink_count(target_id).expect("backlinks should be countable");
        let backlinks = get_note_backlinks(target_id).expect("backlinks should be readable");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");

        assert_eq!(backlink_count, 1);
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].id, source_id);
        assert_eq!(backlinks[0].title, "Source");
    }

    #[test]
    fn test_backlink_count_matches_distinct_active_backlink_sources() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before backlink count test");
        let token = unique_test_token("backlink_distinct");
        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");
        let source = Note::with_content(format!(
            "# Source\n[[{token} Target]] and again [[{token} Target]]"
        ));
        let source_id = source.id;
        save_note(&source).expect("failed to save source note");

        assert_eq!(
            get_note_backlink_count(target_id).expect("backlink count should work"),
            1
        );
        assert_eq!(
            get_note_backlinks(target_id)
                .expect("backlinks should work")
                .len(),
            1
        );

        let mut deleted_source = get_note(source_id)
            .expect("source note lookup should work")
            .expect("source note should exist");
        deleted_source.soft_delete();
        save_note(&deleted_source).expect("failed to soft-delete source note");

        assert_eq!(
            get_note_backlink_count(target_id)
                .expect("backlink count should ignore deleted sources"),
            0
        );
        assert_eq!(
            get_note_backlinks(target_id)
                .expect("backlinks should ignore deleted sources")
                .len(),
            0
        );

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");
    }

    #[test]
    fn test_metadata_backfills_existing_notes_after_schema_creation() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before metadata backfill test");
        let token = unique_test_token("metadata_backfill");
        let note = Note::with_content(format!("# Backfill\nBody #{token}"));
        let id = note.id;

        save_note(&note).expect("failed to save note before simulated metadata loss");
        {
            let db = get_db().expect("db should be initialized");
            let conn = db.lock().expect("db lock");
            conn.execute(
                "DELETE FROM note_tags WHERE note_id = ?1",
                params![id.as_str()],
            )
            .expect("failed to clear note tags");
        }

        init_notes_db().expect("init should backfill missing metadata");
        let tags = get_note_tags(id).expect("tags should be backfilled");

        delete_note_permanently(id).expect("cleanup failed for metadata backfill note");

        assert!(tags.iter().any(|tag| tag == &token));
    }

    #[test]
    fn test_save_note_persists_canonical_brain_markdown_file() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before canonical file test");
        let token = unique_test_token("canonical_file");
        let note = Note::with_content(format!("# {token}\nBody with #{token}"));
        let id = note.id;

        save_note(&note).expect("failed to save note");

        let db = get_db().expect("db");
        let conn = db.lock().expect("lock");
        let slug = lookup_note_slug(&conn, id)
            .expect("slug lookup")
            .expect("slug should exist after save");
        drop(conn);

        let substrate = notes_substrate().expect("substrate");
        let path = substrate.paths().note_file(&slug);
        assert!(
            path.exists(),
            "save_note should write canonical markdown at {}",
            path.display()
        );

        let raw = fs::read_to_string(&path).expect("read canonical note file");
        assert!(
            raw.contains(&id.as_str()),
            "file frontmatter should preserve note id"
        );
        assert!(
            raw.contains(&token),
            "file body should preserve note content"
        );

        delete_note_permanently(id).expect("cleanup");
    }

    #[test]
    fn test_save_note_preserves_source_frontmatter_in_canonical_file() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before source frontmatter test");
        let token = unique_test_token("canonical_source");
        let source = format!("scriptkit://agent-chat/{token}");
        let note = Note::with_content(format!(
            "---\nsource: {source}\n---\n# Agent Chat Conversation\nBody with {token}"
        ));
        let id = note.id;

        save_note(&note).expect("failed to save note with source frontmatter");

        let db = get_db().expect("db");
        let conn = db.lock().expect("lock");
        let slug = lookup_note_slug(&conn, id)
            .expect("slug lookup")
            .expect("slug should exist after save");
        drop(conn);

        let substrate = notes_substrate().expect("substrate");
        let path = substrate.paths().note_file(&slug);
        let raw = fs::read_to_string(&path).expect("read canonical note file");

        delete_note_permanently(id).expect("cleanup");

        assert!(
            raw.contains(&format!("source: {source}")),
            "canonical file should preserve source provenance in frontmatter: {raw}"
        );
    }

    #[test]
    fn test_save_note_preserves_custom_frontmatter_through_reload() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before custom frontmatter test");
        let token = unique_test_token("canonical_custom_frontmatter");
        let custom = "owner: Alice\ncustom_nested:\n  tags: [one, two]\n  id: nested-user-id\n  enabled: true\nsummary: |+\n  first line\n  ---\n\n  last line\n\n\n";
        let expected: serde_yaml::Mapping = serde_yaml::from_str(custom).expect("custom fields");
        let note = Note::with_content(format!(
            "---\nid: not-the-note-id\ncreated: not-a-timestamp\nupdated: not-a-timestamp\n{custom}---\n# {token}\nBody sentinel"
        ));
        let id = note.id;
        save_note(&note).expect("save custom frontmatter");

        let substrate = notes_substrate().expect("substrate");
        let path = note_file_path(id)
            .expect("canonical path lookup")
            .expect("canonical path");
        let raw = brain_io::read_private_document(&path).expect("read canonical document");
        let (frontmatter, body) = substrate
            .parse_document(&raw)
            .expect("parse canonical document");
        assert_eq!(frontmatter.id, id);
        assert_eq!(frontmatter.created, note.created_at);
        assert_eq!(frontmatter.updated, note.updated_at);
        assert_eq!(frontmatter.extra, expected);
        assert!(body.contains("Body sentinel"));
        verify_saved_note_content(id, &note.content).expect("verify saved document");

        let (mut loaded, _, _) =
            load_note_from_file(&substrate, &path, None, 0).expect("reload note");
        let mut visible = BrainFrontmatter::new(id, note.created_at, note.updated_at);
        visible
            .merge_extra_from_content(&loaded.content)
            .expect("read visible custom fields");
        assert_eq!(visible.extra, expected);
        loaded.content.push_str("\nEdited after reload");
        save_note(&loaded).expect("save reloaded note");

        let raw = brain_io::read_private_document(&path).expect("read rewritten document");
        let (frontmatter, body) = substrate
            .parse_document(&raw)
            .expect("parse rewritten document");
        assert_eq!(frontmatter.extra, expected);
        assert!(body.ends_with("Edited after reload"));

        // Older indexes may contain only the body; a routine edit must not
        // erase canonical metadata the old reader did not expose.
        loaded.content = format!("# {token}\nEdited from a body-only index");
        save_note(&loaded).expect("save body-only indexed note");
        rebuild_index_from_files().expect("rebuild from canonical documents");
        let rebuilt = get_note(id)
            .expect("read rebuilt note")
            .expect("rebuilt note");
        let mut visible = BrainFrontmatter::new(id, note.created_at, note.updated_at);
        visible
            .merge_extra_from_content(&rebuilt.content)
            .expect("read rebuilt custom fields");
        assert_eq!(visible.extra, expected);
        assert!(rebuilt.content.ends_with("Edited from a body-only index"));

        delete_note_permanently(id).expect("cleanup custom frontmatter note");
    }

    #[test]
    fn test_rebuild_index_from_files_preserves_cart_items_for_rebuilt_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before cart rebuild preserve test");
        let token = unique_test_token("cart_rebuild_preserve");
        let note = Note::with_content(format!("# {token}\nBody"));
        let note_id = note.id;
        save_note(&note).expect("failed to save canonical note");
        let item = text_cart_item(
            note_id,
            format!("{token}-cart"),
            "Cart Preserve".to_string(),
            format!("payload {token}"),
            7,
        );
        save_note_cart_item(&item).expect("failed to save cart item before rebuild");

        rebuild_index_from_files().expect("notes rebuild should succeed");
        let rebuilt_items =
            list_note_cart_items(note_id).expect("cart items should be readable after rebuild");

        delete_note_permanently(note_id).expect("cleanup failed for preserved cart note");

        assert_eq!(rebuilt_items.len(), 1);
        assert_eq!(rebuilt_items[0].id, item.id);
        assert_eq!(rebuilt_items[0].label, item.label);
        assert_eq!(rebuilt_items[0].sort_order, item.sort_order);
        assert_eq!(rebuilt_items[0].payload, item.payload);
    }

    #[test]
    fn test_rebuild_index_from_files_prunes_cart_items_for_missing_canonical_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before cart prune test");
        let token = unique_test_token("cart_rebuild_prune");
        let stale_note = Note::with_content(format!("# {token}\nDB only"));
        let stale_note_id = stale_note.id;
        {
            let db = get_db().expect("db should be initialized");
            let conn = db.lock().expect("db lock should succeed");
            upsert_note_index_with_conn(
                &conn,
                &stale_note,
                &format!("{token}-missing-file"),
                "synthetic-hash",
            )
            .expect("failed to insert stale db-only note row");
        }
        let item = text_cart_item(
            stale_note_id,
            format!("{token}-orphan-cart"),
            "Orphan Cart".to_string(),
            format!("orphan payload {token}"),
            0,
        );
        save_note_cart_item(&item).expect("failed to save cart item for stale note");
        assert_eq!(
            list_note_cart_items(stale_note_id)
                .expect("cart item should exist before rebuild")
                .len(),
            1
        );

        rebuild_index_from_files().expect("notes rebuild should succeed");

        assert!(
            get_note(stale_note_id)
                .expect("stale note lookup should succeed")
                .is_none(),
            "db-only note should disappear after markdown-source rebuild"
        );
        assert!(
            list_note_cart_items(stale_note_id)
                .expect("cart lookup after rebuild should succeed")
                .is_empty(),
            "cart rows for notes absent from canonical files should be pruned"
        );
    }

    #[test]
    fn test_rebuild_index_from_files_preserves_cart_items_for_trashed_notes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trashed cart rebuild test");
        let token = unique_test_token("cart_rebuild_trash");
        let mut note = Note::with_content(format!("# {token}\nBody"));
        let note_id = note.id;
        save_note(&note).expect("failed to save active note");
        let item = text_cart_item(
            note_id,
            format!("{token}-trash-cart"),
            "Trash Cart".to_string(),
            format!("trash payload {token}"),
            4,
        );
        save_note_cart_item(&item).expect("failed to save cart item before soft delete");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete note");
        rebuild_index_from_files().expect("notes rebuild should succeed");
        let rebuilt_items = list_note_cart_items(note_id)
            .expect("cart items should be readable after trash rebuild");

        delete_note_permanently(note_id).expect("cleanup failed for trashed note");

        assert_eq!(rebuilt_items.len(), 1);
        assert_eq!(rebuilt_items[0].id, item.id);
        assert_eq!(rebuilt_items[0].payload, item.payload);
    }

    #[test]
    fn test_rebuild_index_from_files_restores_search_tags_pins_and_backlinks() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before rebuild contract test");
        let token = unique_test_token("rebuild_contract");

        let target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;
        save_note(&target).expect("failed to save target note");

        let mut source = Note::with_content(format!(
            "---\ntags: [{token}, instructions]\naliases: [{token} Alias]\n---\n# Source\n[[{token} Target]]"
        ));
        source.is_pinned = true;
        let source_id = source.id;
        save_note(&source).expect("failed to save source note");

        let golden_search = search_notes(&token).expect("search should work");
        let golden_tags = get_note_tags(source_id).expect("tags should work");
        let golden_aliases = get_note_aliases(source_id).expect("aliases should work");
        let golden_backlinks = get_note_backlinks(target_id).expect("backlinks should work");
        let golden_backlink_count =
            get_note_backlink_count(target_id).expect("backlink count should work");
        let golden_pin = get_note(source_id)
            .expect("get note should work")
            .expect("source note should exist")
            .is_pinned;

        clear_index_tables(&get_db().expect("db").lock().expect("lock"))
            .expect("failed to clear index for rebuild test");
        rebuild_index_from_files().expect("rebuild should succeed");

        let rebuilt_search = search_notes(&token).expect("search after rebuild should work");
        let rebuilt_tags = get_note_tags(source_id).expect("tags after rebuild should work");
        let rebuilt_aliases =
            get_note_aliases(source_id).expect("aliases after rebuild should work");
        let rebuilt_backlinks =
            get_note_backlinks(target_id).expect("backlinks after rebuild should work");
        let rebuilt_backlink_count =
            get_note_backlink_count(target_id).expect("backlink count after rebuild should work");
        let rebuilt_pin = get_note(source_id)
            .expect("get note after rebuild should work")
            .expect("source note should exist after rebuild")
            .is_pinned;

        delete_note_permanently(source_id).expect("cleanup source");
        delete_note_permanently(target_id).expect("cleanup target");

        assert_eq!(
            golden_search.iter().map(|note| note.id).collect::<Vec<_>>(),
            rebuilt_search
                .iter()
                .map(|note| note.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(golden_tags, rebuilt_tags);
        assert_eq!(golden_aliases, rebuilt_aliases);
        assert_eq!(golden_backlinks.len(), rebuilt_backlinks.len());
        assert_eq!(
            golden_backlinks.first().map(|hit| hit.id),
            rebuilt_backlinks.first().map(|hit| hit.id)
        );
        assert_eq!(golden_backlink_count, rebuilt_backlink_count);
        assert_eq!(golden_pin, rebuilt_pin);
        assert!(golden_pin, "fixture note should be pinned");
    }

    #[test]
    fn test_soft_delete_moves_file_to_trash_and_restore_returns_it() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trash roundtrip test");
        let token = unique_test_token("trash_roundtrip");
        let mut note = Note::with_content(format!("# {token}\nBody"));
        let id = note.id;
        save_note(&note).expect("failed to save active note");

        let substrate = notes_substrate().expect("substrate");
        let slug = lookup_note_slug(&get_db().expect("db").lock().expect("lock"), id)
            .expect("slug lookup")
            .expect("slug should exist");
        let active_path = substrate.paths().note_file(&slug);
        assert!(active_path.exists(), "canonical note file should exist");

        note.soft_delete();
        save_note(&note).expect("failed to soft-delete note");
        assert!(!active_path.exists(), "active note file should be trashed");

        let deleted = get_deleted_notes().expect("deleted notes");
        assert!(deleted.iter().any(|candidate| candidate.id == id));

        note.restore();
        save_note(&note).expect("failed to restore note");
        assert!(active_path.exists(), "restored note file should exist");

        delete_note_permanently(id).expect("cleanup");
    }

    #[test]
    fn test_same_title_note_preserves_trash_and_restores_without_overwriting() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before trash collision test");
        let token = unique_test_token("trash_collision");
        let mut original = Note::with_content(format!("# {token}\nOriginal deleted body"));
        save_note(&original).expect("save original");
        let original_path = note_file_path(original.id)
            .expect("original path lookup")
            .expect("original path");
        let substrate = notes_substrate().expect("substrate");
        let trash_path = substrate
            .paths()
            .trash_dir()
            .join(original_path.file_name().expect("original filename"));
        original.soft_delete();
        save_note(&original).expect("trash original");
        let trash_before = brain_io::read_private_document(&trash_path).expect("trashed document");
        let unrelated_path = substrate
            .paths()
            .trash_dir()
            .join(format!("{token}-day.md"));
        let unrelated_body = "# Unrelated day page\nKeep this capture.\n";
        brain_io::atomic_write(&unrelated_path, unrelated_body).expect("unrelated trash document");

        let replacement = Note::with_content(format!("# {token}\nDifferent active body"));
        save_note(&replacement).expect("save different note with the same title");
        assert!(
            trash_path.exists(),
            "saving a new note must not consume another note's trash document"
        );
        save_note(&replacement).expect("ordinary active save must not restore another note");
        assert_eq!(
            brain_io::read_private_document(&trash_path).expect("original still in trash"),
            trash_before
        );
        let replacement_path = note_file_path(replacement.id)
            .expect("replacement path lookup")
            .expect("replacement path");
        let replacement_before = brain_io::read_private_document(&replacement_path)
            .expect("replacement canonical document");

        original.restore();
        save_note(&original).expect("restore original without replacing the active note");
        let restored_path = note_file_path(original.id)
            .expect("restored path lookup")
            .expect("restored path");
        assert_ne!(restored_path, replacement_path);
        assert!(
            !trash_path.exists(),
            "the exact original should be restored"
        );
        let restored = brain_io::read_private_document(&restored_path).expect("restored document");
        assert_eq!(
            substrate
                .parse_document(&restored)
                .expect("parse restored")
                .0
                .id,
            original.id
        );
        assert!(restored.contains("Original deleted body"));
        assert_eq!(
            brain_io::read_private_document(&replacement_path).expect("active note unchanged"),
            replacement_before
        );
        assert_eq!(
            brain_io::read_private_document(&unrelated_path).expect("unrelated trash unchanged"),
            unrelated_body
        );
        fs::remove_file(&unrelated_path).expect("cleanup unrelated fixture");
        delete_note_permanently(original.id).expect("cleanup original");
        delete_note_permanently(replacement.id).expect("cleanup replacement");
    }

    #[test]
    fn test_backlinks_recompute_when_target_alias_changes() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before stale backlink test");
        let token = unique_test_token("stale_backlink");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;
        let mut target = Note::with_content(format!("# {token} Target\nBody"));
        let target_id = target.id;

        save_note(&target).expect("failed to save target note");
        save_note(&source).expect("failed to save source note");
        assert_eq!(
            get_note_backlink_count(target_id).expect("backlinks should resolve"),
            1
        );

        target.title = format!("{token} Renamed");
        target.content = format!("# {token} Renamed\nBody");
        save_note(&target).expect("failed to save renamed target note");
        let backlink_count =
            get_note_backlink_count(target_id).expect("backlinks should recompute");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_id).expect("cleanup failed for target note");

        assert_eq!(backlink_count, 0);
    }

    #[test]
    fn test_backlinks_do_not_resolve_ambiguous_aliases() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before ambiguous backlink test");
        let token = unique_test_token("ambiguous_backlink");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;
        let target_a = Note::with_content(format!("# {token} Target\nA"));
        let target_a_id = target_a.id;
        let target_b = Note::with_content(format!("# {token} Target\nB"));
        let target_b_id = target_b.id;

        save_note(&target_a).expect("failed to save first target note");
        save_note(&target_b).expect("failed to save second target note");
        save_note(&source).expect("failed to save source note");

        let backlinks_a = get_note_backlink_count(target_a_id).expect("backlinks should count");
        let backlinks_b = get_note_backlink_count(target_b_id).expect("backlinks should count");

        delete_note_permanently(source_id).expect("cleanup failed for source note");
        delete_note_permanently(target_a_id).expect("cleanup failed for first target note");
        delete_note_permanently(target_b_id).expect("cleanup failed for second target note");

        assert_eq!(backlinks_a + backlinks_b, 0);
    }

    #[test]
    fn test_search_notes_matches_link_metadata() {
        let _guard = notes_db_test_guard();
        init_notes_db().expect("notes db should initialize before link metadata search test");
        let token = unique_test_token("link_search");
        let source = Note::with_content(format!("# Source\n[[{token} Target]]"));
        let source_id = source.id;

        save_note(&source).expect("failed to save link source note");
        let results = search_notes(&format!("link:{token}-target"))
            .expect("link metadata search should succeed");

        delete_note_permanently(source_id).expect("cleanup failed for link source note");

        assert!(results.iter().any(|note| note.id == source_id));
    }
}
