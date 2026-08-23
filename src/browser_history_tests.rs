#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn root_history_hit(title: &str, domain: &str, url: &str) -> RootBrowserHistorySearchHit {
        RootBrowserHistorySearchHit {
            stable_key: "browser-history/chrome/default/test".to_string(),
            provider_label: "Chrome".to_string(),
            profile_label: "Default".to_string(),
            title: title.to_string(),
            url: url.to_string(),
            domain: domain.to_string(),
            last_visit_unix_ms: Utc::now().timestamp_millis(),
            visit_count: 1,
        }
    }

    #[test]
    fn root_browser_history_rejects_unrelated_multi_word_fuzzy_subsequence() {
        let hit = root_history_hit(
            "Pliny the Liberator 🐉 on X: \"what happens when you prompt Fable to use up an entire week&apos;s worth of tokens\"",
            "x.com",
            "https://x.com/pliny/status/1",
        );

        let matches = root_fuzzy_search_browser_history_hits(&[hit], "create a new flow", true);

        assert!(matches.is_empty());
    }

    #[test]
    fn root_browser_history_accepts_multi_word_terms_across_title() {
        let hit = root_history_hit(
            "Pliny the Liberator 🐉 on X: what happens when you prompt Fable",
            "x.com",
            "https://x.com/pliny/status/1",
        );

        let matches = root_fuzzy_search_browser_history_hits(&[hit.clone()], "pliny fable", true);

        assert_eq!(matches, vec![hit]);
    }

    #[test]
    fn root_browser_history_accepts_terms_split_across_title_and_domain() {
        let hit = root_history_hit(
            "Rust reference",
            "docs.example.com",
            "https://docs.example.com/reference",
        );

        let matches = root_fuzzy_search_browser_history_hits(&[hit.clone()], "rust example", false);

        assert_eq!(matches, vec![hit]);
    }

    #[test]
    fn root_browser_history_rejects_single_word_subsequence_only_match() {
        let hit = root_history_hit(
            "Create delightful launcher workflows",
            "example.com",
            "https://example.com/workflows",
        );

        let matches = root_fuzzy_search_browser_history_hits(&[hit], "cdlw", true);

        assert!(matches.is_empty());
    }

    #[test]
    fn fuzzy_search_prefers_title_match_over_browser_name_only() {
        let entries = vec![
            BrowserHistoryEntry {
                browser_name: "Google Chrome".into(),
                browser_bundle_id: "com.google.Chrome".into(),
                title: "Script Kit browser history portal".into(),
                url: "https://example.com/script-kit".into(),
                host: "example.com".into(),
                last_visited_at_ms: Utc::now().timestamp_millis(),
                visit_count: 3,
                profile: "Default".into(),
            },
            BrowserHistoryEntry {
                browser_name: "Chrome".into(),
                browser_bundle_id: "com.google.Chrome".into(),
                title: "Home".into(),
                url: "https://example.com/browser-portal".into(),
                host: "example.com".into(),
                last_visited_at_ms: Utc::now().timestamp_millis(),
                visit_count: 2,
                profile: "Default".into(),
            },
        ];

        let matches = fuzzy_search_browser_history(&entries, "portal");
        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches[0].entry.title.as_ref(),
            "Script Kit browser history portal"
        );
    }

    #[test]
    fn chromium_timestamp_converts_to_unix_ms() {
        let utc = Utc
            .with_ymd_and_hms(2026, 4, 11, 12, 0, 0)
            .single()
            .unwrap_or_else(|| panic!("expected stable timestamp"));
        let visit_time = (utc.timestamp() + CHROMIUM_EPOCH_OFFSET_SECS) * 1_000_000;
        assert_eq!(
            chromium_visit_time_to_unix_ms(visit_time),
            utc.timestamp_millis()
        );
    }

    #[test]
    fn history_db_paths_finds_chromium_profiles() {
        let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let root = temp
            .path()
            .join("Library/Application Support/Google/Chrome");
        std::fs::create_dir_all(root.join("Default"))
            .unwrap_or_else(|error| panic!("create default profile failed: {error}"));
        std::fs::create_dir_all(root.join("Profile 1"))
            .unwrap_or_else(|error| panic!("create second profile failed: {error}"));
        std::fs::write(root.join("Default/History"), "")
            .unwrap_or_else(|error| panic!("write default history failed: {error}"));
        std::fs::write(root.join("Profile 1/History"), "")
            .unwrap_or_else(|error| panic!("write profile history failed: {error}"));

        let paths = history_db_paths_for_browser(&SUPPORTED_BROWSERS[1], temp.path());
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn dedupe_history_entries_collapses_normalized_url_duplicates() {
        let newer = BrowserHistoryEntry {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            title: "Portal".into(),
            url: "https://example.com/docs#intro".into(),
            host: "example.com".into(),
            last_visited_at_ms: Utc::now().timestamp_millis(),
            visit_count: 5,
            profile: "Default".into(),
        };
        let older = BrowserHistoryEntry {
            last_visited_at_ms: newer.last_visited_at_ms - 10_000,
            url: "https://example.com/docs/".into(),
            visit_count: 2,
            ..newer.clone()
        };

        let deduped = dedupe_history_entries(vec![newer.clone(), older], 10);
        assert_eq!(deduped, vec![newer]);
    }

    #[test]
    fn dedupe_history_entries_collapses_same_title_and_host_across_browsers() {
        let newer = BrowserHistoryEntry {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            title: "Inbox (1,626) - johnlindquist@gmail.com - Gmail".into(),
            url: "https://mail.google.com/mail/u/0/#inbox".into(),
            host: "mail.google.com".into(),
            last_visited_at_ms: Utc::now().timestamp_millis(),
            visit_count: 7,
            profile: "Default".into(),
        };
        let older = BrowserHistoryEntry {
            browser_name: "Arc".into(),
            browser_bundle_id: "company.thebrowser.Browser".into(),
            url: "https://mail.google.com/mail/u/1/#inbox".into(),
            profile: "Profile 1".into(),
            last_visited_at_ms: newer.last_visited_at_ms - 5_000,
            ..newer.clone()
        };

        let deduped = dedupe_history_entries(vec![newer.clone(), older], 10);
        assert_eq!(deduped, vec![newer]);
    }

    #[test]
    fn root_browser_history_reads_chromium_url_metadata_only_and_filters_schemes() {
        let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let profile_dir = temp
            .path()
            .join("Library/Application Support/Google/Chrome/Default");
        std::fs::create_dir_all(&profile_dir)
            .unwrap_or_else(|error| panic!("create profile dir failed: {error}"));
        let db_path = profile_dir.join("History");
        let conn = Connection::open(&db_path)
            .unwrap_or_else(|error| panic!("open history db failed: {error}"));
        conn.execute_batch(
            r#"
            CREATE TABLE urls (
                id INTEGER PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT,
                visit_count INTEGER NOT NULL DEFAULT 0,
                typed_count INTEGER NOT NULL DEFAULT 0,
                last_visit_time INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .unwrap_or_else(|error| panic!("create urls table failed: {error}"));

        let now_chromium = (Utc::now().timestamp() + CHROMIUM_EPOCH_OFFSET_SECS) * 1_000_000;
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                1_i64,
                "https://example.com/root-browser-unique",
                "Root Browser Unique Planning Page",
                7_i64,
                2_i64,
                now_chromium,
            ],
        )
        .unwrap_or_else(|error| panic!("insert https row failed: {error}"));
        conn.execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                2_i64,
                "chrome://settings/root-browser-unique",
                "Root Browser Unique Settings",
                3_i64,
                0_i64,
                now_chromium,
            ],
        )
        .unwrap_or_else(|error| panic!("insert chrome row failed: {error}"));
        drop(conn);

        let options = RootBrowserHistorySectionOptions {
            enabled: true,
            max_results: 3,
            min_query_chars: 4,
            max_age_days: 90,
            providers: vec![crate::config::BrowserHistoryProvider::Chrome],
            search_urls: true,
            scan_limit: 500,
            cache_ttl_ms: 30_000,
        };
        let candidates = refresh_root_browser_history_snapshot_from_home(temp.path(), &options)
            .expect("refresh root browser history snapshot");
        let hits = root_fuzzy_search_browser_history_hits(
            &candidates,
            "Root Browser Unique",
            options.search_urls,
        );

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Root Browser Unique Planning Page");
        assert_eq!(hits[0].url, "https://example.com/root-browser-unique");
        assert_eq!(hits[0].domain, "example.com");
        assert_eq!(hits[0].visit_count, 7);
        assert!(hits[0].stable_key.starts_with("browser-history/chrome/"));
    }

    #[test]
    fn root_browser_history_open_rejects_non_http_schemes() {
        assert!(ensure_browser_history_url_is_http_or_https("https://example.com").is_ok());
        assert!(ensure_browser_history_url_is_http_or_https("http://example.com").is_ok());
        assert!(ensure_browser_history_url_is_http_or_https("chrome://settings").is_err());
        assert!(ensure_browser_history_url_is_http_or_https("file:///tmp/a").is_err());
        assert!(ensure_browser_history_url_is_http_or_https("javascript:alert(1)").is_err());
        assert!(ensure_browser_history_url_is_http_or_https("scriptkit://run/test").is_err());
    }

    #[test]
    fn cold_direct_browser_history_lookup_cannot_start_an_unowned_refresh() {
        {
            let mut cache = ROOT_BROWSER_HISTORY_SNAPSHOT
                .lock()
                .expect("browser history cache");
            *cache = RootBrowserHistorySnapshotState {
                generation: 23,
                ..RootBrowserHistorySnapshotState::default()
            };
        }
        let before = root_browser_history_snapshot_status();
        let options = RootBrowserHistorySectionOptions {
            enabled: true,
            min_query_chars: 0,
            ..RootBrowserHistorySectionOptions::default()
        };

        assert!(
            search_root_browser_history_meta_direct("private history query", options).is_empty()
        );
        assert_eq!(root_browser_history_snapshot_status(), before);
        assert!(!root_browser_history_snapshot_status().refreshing);

        if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
            *cache = RootBrowserHistorySnapshotState::default();
        }
    }

    #[test]
    fn direct_browser_history_lookup_preserves_existing_snapshot_and_generation() {
        let previous_rows = Arc::new(vec![root_history_hit(
            "Private cached history",
            "example.invalid",
            "https://example.invalid/private-history-canary",
        )]);
        {
            let mut cache = ROOT_BROWSER_HISTORY_SNAPSHOT
                .lock()
                .expect("browser history cache");
            *cache = RootBrowserHistorySnapshotState {
                snapshot: Some(RootBrowserHistorySnapshot {
                    captured_at: Instant::now() - Duration::from_secs(40),
                    hits: Arc::clone(&previous_rows),
                }),
                generation: 37,
                ..RootBrowserHistorySnapshotState::default()
            };
        }
        let before = root_browser_history_snapshot_status();
        let options = RootBrowserHistorySectionOptions {
            enabled: true,
            min_query_chars: 0,
            ..RootBrowserHistorySectionOptions::default()
        };

        let hits = search_root_browser_history_meta_direct("private cached history", options);
        assert_eq!(hits.len(), 1);
        assert_eq!(root_browser_history_snapshot_status(), before);
        {
            let cache = ROOT_BROWSER_HISTORY_SNAPSHOT
                .lock()
                .expect("browser history cache");
            assert!(Arc::ptr_eq(
                &cache.snapshot.as_ref().expect("snapshot preserved").hits,
                &previous_rows
            ));
        }
        if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
            *cache = RootBrowserHistorySnapshotState::default();
        }
    }

    #[test]
    fn canceled_browser_history_generation_preserves_snapshot_for_current_query() {
        let previous_rows = Arc::new(Vec::new());
        let mut cache = RootBrowserHistorySnapshotState {
            snapshot: Some(RootBrowserHistorySnapshot {
                captured_at: Instant::now(),
                hits: Arc::clone(&previous_rows),
            }),
            refresh_in_flight: true,
            generation: 7,
            last_refresh_error: Some("previous failure".to_owned()),
        };

        assert!(discard_root_browser_history_refresh_from_state(
            &mut cache, 7
        ));
        assert!(!cache.refresh_in_flight);
        assert_eq!(cache.generation, 8);
        assert_eq!(
            cache.last_refresh_error.as_deref(),
            Some("previous failure")
        );
        assert!(Arc::ptr_eq(
            &cache.snapshot.as_ref().expect("preserved snapshot").hits,
            &previous_rows
        ));
    }

    #[test]
    fn older_browser_history_completion_cannot_cancel_newer_in_flight_generation() {
        let mut cache = RootBrowserHistorySnapshotState {
            refresh_in_flight: true,
            generation: 9,
            ..RootBrowserHistorySnapshotState::default()
        };

        assert!(!discard_root_browser_history_refresh_from_state(
            &mut cache, 7
        ));
        assert!(cache.refresh_in_flight);
        assert_eq!(cache.generation, 9);
    }

    #[test]
    fn root_browser_history_refresh_completion_advances_generation_and_stores_rows() {
        if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
            *cache = RootBrowserHistorySnapshotState::default();
        }
        let options = RootBrowserHistorySectionOptions {
            enabled: true,
            max_results: 10,
            min_query_chars: 0,
            max_age_days: 90,
            providers: vec![crate::config::BrowserHistoryProvider::Chrome],
            search_urls: true,
            scan_limit: 10,
            cache_ttl_ms: 30_000,
        };
        let before = root_browser_history_snapshot_status();
        let refresh =
            try_begin_root_browser_history_refresh(&options, "test").expect("refresh should start");
        assert!(root_browser_history_snapshot_status().refreshing);

        let hit = RootBrowserHistorySearchHit {
            stable_key: "browser-history/chrome/default/1".to_string(),
            provider_label: "Chrome".to_string(),
            profile_label: "Default".to_string(),
            title: "Root Completion History".to_string(),
            url: "https://example.com/root-completion-history".to_string(),
            domain: "example.com".to_string(),
            last_visit_unix_ms: Utc::now().timestamp_millis(),
            visit_count: 4,
        };
        assert!(finish_root_browser_history_refresh(
            refresh,
            Ok(vec![hit.clone()])
        ));

        let after = root_browser_history_snapshot_status();
        assert!(after.generation > before.generation);
        assert!(!after.refreshing);
        assert_eq!(after.cached_count, 1);
        assert_eq!(
            cached_root_browser_history_snapshot(30_000).as_ref(),
            &vec![hit]
        );
        if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
            *cache = RootBrowserHistorySnapshotState::default();
        }
    }
}
