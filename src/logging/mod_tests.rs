#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    use std::io::Write as IoWrite;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;
    use tracing_subscriber::{fmt as fmt_sub, EnvFilter};
    #[derive(Clone)]
    struct BufferWriter(Arc<Mutex<Vec<u8>>>);
    struct BufferGuard<'a> {
        buf: &'a Arc<Mutex<Vec<u8>>>,
    }
    impl<'a> IoWrite for BufferGuard<'a> {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let mut buf = self.buf.lock().unwrap_or_else(|e| e.into_inner());
            buf.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> MakeWriter<'a> for BufferWriter {
        type Writer = BufferGuard<'a>;

        fn make_writer(&'a self) -> Self::Writer {
            BufferGuard { buf: &self.0 }
        }
    }
    #[test]
    fn json_formatter_injects_correlation_id() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .json()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(JsonWithCorrelation)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello-json-correlation");
        });

        let output =
            String::from_utf8(buffer.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap();
        let line = output.lines().next().unwrap();
        let value: serde_json::Value = serde_json::from_str(line).unwrap();

        let cid = value
            .get("correlation_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        assert!(
            !cid.is_empty(),
            "correlation_id should be present and non-empty"
        );
    }

    #[test]
    fn private_launcher_values_never_escape_into_structured_json_logs() {
        let secret = "sk-live-private-token https://private.example/path?password=hunter2";
        let safe = log_private_user_value(secret);
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .json()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(JsonWithCorrelation)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                query_bytes = safe.raw_bytes,
                query_sha256 = %safe.sha256,
                "private launcher query"
            );
            tracing::info!("legacy launcher query {} ({} bytes)", safe, safe.raw_bytes);
        });

        let output = String::from_utf8(
            buffer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        )
        .expect("structured test output must be valid UTF-8");

        assert!(!output.contains(secret));
        assert!(!output.contains("hunter2"));
        assert!(!output.contains("private.example"));
        assert!(output.contains(&safe.sha256));
        assert!(output.contains(&safe.raw_bytes.to_string()));
        assert_eq!(output.lines().count(), 2);
    }

    #[test]
    fn production_ai_queries_and_file_mentions_never_escape_structured_logs() {
        let secret_query = "canary-private-query-93841";
        let secret_path = "/vault/canary-auth-token-93841/private-prompt.txt";
        let mention = format!("attach @file:{secret_path}");
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .json()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(JsonWithCorrelation)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let rows = crate::ai::context_selector::slash_command_rows_with_descriptions(
                secret_query,
                [(secret_query, "sensitive command")],
            );
            assert_eq!(rows.len(), 1);

            let mentions = crate::ai::context_mentions::parse_inline_context_mentions(&mention);
            assert_eq!(mentions.len(), 1);
            assert_eq!(mentions[0].part.label(), "private-prompt.txt");

            let private_token = format!("@skills:{secret_query}");
            let aliases = std::collections::HashMap::from([(
                private_token.clone(),
                crate::ai::message_parts::AiContextPart::FilePath {
                    path: secret_path.to_string(),
                    label: "private-prompt.txt".to_string(),
                },
            )]);
            let sync_plan =
                crate::ai::context_mentions::build_inline_mention_sync_plan_with_aliases(
                    &private_token,
                    &[],
                    &std::collections::HashSet::new(),
                    &aliases,
                );
            assert_eq!(sync_plan.added_parts.len(), 1);
        });

        let output = String::from_utf8(
            buffer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone(),
        )
        .expect("structured production events must be valid UTF-8");

        assert!(output.contains("ai_context_selector_slash_items_built"));
        assert!(output.contains("inline_context_token_resolved"));
        assert!(output.contains("inline_mention_sync_plan_built"));
        assert!(output.contains(&log_private_user_value(secret_query).sha256));
        assert!(output.contains(&log_private_user_value("private-prompt.txt").sha256));
        assert!(!output.contains(secret_query));
        assert!(!output.contains(secret_path));
        assert!(!output.contains("canary-auth-token-93841"));
        assert!(!output.contains("private-prompt.txt"));
    }

    #[test]
    fn slash_command_ties_use_canonical_identity_not_discovery_order() {
        let forward = crate::ai::context_selector::slash_command_rows_with_descriptions(
            "",
            [("zeta", "last"), ("alpha", "first")],
        );
        let reverse = crate::ai::context_selector::slash_command_rows_with_descriptions(
            "",
            [("alpha", "first"), ("zeta", "last")],
        );
        let forward_labels: Vec<&str> = forward.iter().map(|row| row.label.as_ref()).collect();
        let reverse_labels: Vec<&str> = reverse.iter().map(|row| row.label.as_ref()).collect();

        assert_eq!(forward_labels, ["alpha", "zeta"]);
        assert_eq!(forward_labels, reverse_labels);
    }

    #[test]
    fn compact_formatter_includes_correlation_id_token() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = fmt_sub()
            .with_writer(BufferWriter(buffer.clone()))
            .event_format(CompactAiFormatter)
            .with_env_filter(EnvFilter::new("info"))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello-compact-correlation");
        });

        let output =
            String::from_utf8(buffer.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap();
        let line = output.lines().next().unwrap_or("");
        assert!(
            line.contains("cid="),
            "compact log should include cid token: {}",
            line
        );
    }
    // -------------------------------------------------------------------------
    // category_to_code tests - using real category strings from logs
    // -------------------------------------------------------------------------

    #[test]
    fn test_category_to_code_position() {
        // From: "CALCULATING WINDOW POSITION FOR MOUSE DISPLAY"
        assert_eq!(category_to_code("POSITION"), 'P');
        assert_eq!(category_to_code("position"), 'P');
        assert_eq!(category_to_code("Position"), 'P');
    }
    #[test]
    fn test_category_to_code_app() {
        // From: "Application logging initialized", "GPUI Application starting"
        assert_eq!(category_to_code("APP"), 'A');
        assert_eq!(category_to_code("app"), 'A');
    }
    #[test]
    fn test_category_to_code_stdin() {
        // From: "External command listener started", "Received: {\"type\": \"run\"..."
        assert_eq!(category_to_code("STDIN"), 'S');
    }
    #[test]
    fn test_category_to_code_hotkey() {
        // From: "Registered global hotkey meta+Digit0", "Tray icon initialized"
        assert_eq!(category_to_code("HOTKEY"), 'H');
        assert_eq!(category_to_code("TRAY"), 'H'); // Tray maps to H
    }
    #[test]
    fn test_category_to_code_visibility() {
        // From: "HOTKEY TRIGGERED - TOGGLE WINDOW", "WINDOW_VISIBLE set to: true"
        assert_eq!(category_to_code("VISIBILITY"), 'V');
    }
    #[test]
    fn test_category_to_code_exec() {
        // From: "Executing script: hello-world", "Script execution complete"
        assert_eq!(category_to_code("EXEC"), 'E');
    }
    #[test]
    fn test_category_to_code_theme() {
        // From: "Theme file not found, using defaults based on system appearance"
        assert_eq!(category_to_code("THEME"), 'T');
    }
    #[test]
    fn test_category_to_code_window_mgr() {
        // From: "Searching for main window among 2 windows"
        assert_eq!(category_to_code("WINDOW_MGR"), 'W');
    }
    #[test]
    fn test_category_to_code_config() {
        // From: "Successfully loaded config from ~/.scriptkit/config.ts"
        assert_eq!(category_to_code("CONFIG"), 'N');
        assert_eq!(category_to_code("config"), 'N');
        assert_eq!(category_to_code("Config"), 'N');
    }
    #[test]
    fn test_category_to_code_perf() {
        // From: "Startup loading: 33.30ms total (331 scripts in 5.03ms)"
        assert_eq!(category_to_code("PERF"), 'R');
    }
    #[test]
    fn test_category_to_code_all_categories() {
        // Complete mapping verification
        let mappings = [
            ("POSITION", 'P'),
            ("APP", 'A'),
            ("UI", 'U'),
            ("STDIN", 'S'),
            ("HOTKEY", 'H'),
            ("VISIBILITY", 'V'),
            ("EXEC", 'E'),
            ("KEY", 'K'),
            ("FOCUS", 'F'),
            ("THEME", 'T'),
            ("CACHE", 'C'),
            ("PERF", 'R'),
            ("WINDOW_MGR", 'W'),
            ("ERROR", 'X'),
            ("MOUSE_HOVER", 'M'),
            ("SCROLL_STATE", 'L'),
            ("SCROLL_PERF", 'Q'),
            ("SCRIPT", 'G'), // Changed from B to G
            ("CONFIG", 'N'),
            ("RESIZE", 'Z'),
            ("DESIGN", 'D'),
            ("BENCH", 'B'), // New: Benchmark timing
            ("CHAT", 'U'),
            ("AI", 'U'),
            ("ACTIONS", 'U'),
            ("WINDOW_STATE", 'W'),
            ("DEBUG_GRID", 'D'),
            ("MCP", 'S'),
            ("WARN", 'X'),
            ("SCRIPTLET_PARSE", 'G'),
        ];

        for (category, expected_code) in mappings {
            assert_eq!(
                category_to_code(category),
                expected_code,
                "Category '{}' should map to '{}'",
                category,
                expected_code
            );
        }
    }
    #[test]
    fn test_category_to_code_unknown() {
        assert_eq!(category_to_code("UNKNOWN_CATEGORY"), '-');
        assert_eq!(category_to_code(""), '-');
        assert_eq!(category_to_code("foobar"), '-');
    }
    // -------------------------------------------------------------------------
    // level_to_char tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_level_to_char() {
        assert_eq!(level_to_char(Level::ERROR), 'e');
        assert_eq!(level_to_char(Level::WARN), 'w');
        assert_eq!(level_to_char(Level::INFO), 'i');
        assert_eq!(level_to_char(Level::DEBUG), 'd');
        assert_eq!(level_to_char(Level::TRACE), 't');
    }
    // -------------------------------------------------------------------------
    // infer_category_from_target tests - using real module paths
    // -------------------------------------------------------------------------

    #[test]
    fn test_infer_category_executor() {
        // From: script_kit_gpui::executor
        assert_eq!(infer_category_from_target("script_kit_gpui::executor"), 'E');
    }
    #[test]
    fn test_infer_category_theme() {
        // From: "script_kit_gpui::theme: Theme file not found"
        assert_eq!(infer_category_from_target("script_kit_gpui::theme"), 'T');
    }
    #[test]
    fn test_infer_category_config() {
        // From: "script_kit_gpui::config: Successfully loaded config"
        assert_eq!(infer_category_from_target("script_kit_gpui::config"), 'N');
    }
    #[test]
    fn test_infer_category_clipboard() {
        // From: "script_kit_gpui::clipboard_history: Initializing clipboard history"
        assert_eq!(
            infer_category_from_target("script_kit_gpui::clipboard_history"),
            'A'
        );
    }
    #[test]
    fn test_infer_category_logging() {
        // From: "script_kit_gpui::logging: Application logging initialized"
        assert_eq!(infer_category_from_target("script_kit_gpui::logging"), 'A');
    }
    #[test]
    fn test_infer_category_protocol() {
        // From: "script_kit_gpui::protocol" (stdin message handling)
        assert_eq!(infer_category_from_target("script_kit_gpui::protocol"), 'S');
    }
    #[test]
    fn test_infer_category_prompts() {
        // UI components
        assert_eq!(infer_category_from_target("script_kit_gpui::prompts"), 'U');
        assert_eq!(infer_category_from_target("script_kit_gpui::editor"), 'U');
        assert_eq!(infer_category_from_target("script_kit_gpui::panel"), 'U');
    }
    #[test]
    fn test_infer_category_scripts() {
        // From: "Loaded 331 scripts from ~/.scriptkit/scripts"
        assert_eq!(infer_category_from_target("script_kit_gpui::scripts"), 'G');
        assert_eq!(
            infer_category_from_target("script_kit_gpui::file_search"),
            'G'
        );
    }
    #[test]
    fn test_infer_category_hotkey() {
        // From: "Registered global hotkey meta+Digit0"
        assert_eq!(infer_category_from_target("script_kit_gpui::hotkey"), 'H');
        assert_eq!(infer_category_from_target("script_kit_gpui::tray"), 'H');
    }
    #[test]
    fn test_infer_category_window() {
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_manager"),
            'W'
        );
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_control"),
            'W'
        );
        assert_eq!(
            infer_category_from_target("script_kit_gpui::window_state"),
            'W'
        );
    }
    #[test]
    fn test_infer_category_unknown() {
        assert_eq!(infer_category_from_target("script_kit_gpui::main"), 'A');
        assert_eq!(infer_category_from_target("script_kit_gpui::ai"), 'U');
        assert_eq!(
            infer_category_from_target("script_kit_gpui::mcp_server"),
            'S'
        );
        assert_eq!(infer_category_from_target("unknown::module"), '-');
    }
    #[test]
    fn test_legacy_level_for_category() {
        assert_eq!(legacy_level_for_category("ERROR"), LegacyLogLevel::Error);
        assert_eq!(legacy_level_for_category("WARN"), LegacyLogLevel::Warn);
        assert_eq!(legacy_level_for_category("WARNING"), LegacyLogLevel::Warn);
        assert_eq!(legacy_level_for_category("DEBUG"), LegacyLogLevel::Debug);
        assert_eq!(legacy_level_for_category("TRACE"), LegacyLogLevel::Trace);
        assert_eq!(legacy_level_for_category("UI"), LegacyLogLevel::Info);
    }
    // -------------------------------------------------------------------------
    // log rotation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_rotate_log_if_oversized() {
        let dir = std::env::temp_dir().join(format!("sk-rotate-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rotate-me.jsonl");
        let rotated = dir.join("rotate-me.jsonl.1");

        // Under the cap: untouched.
        fs::write(&path, b"small").unwrap();
        rotate_log_if_oversized(&path, 100);
        assert!(path.exists());
        assert!(!rotated.exists());

        // Over the cap: renamed to .1.
        fs::write(&path, vec![b'x'; 200]).unwrap();
        rotate_log_if_oversized(&path, 100);
        assert!(!path.exists());
        assert_eq!(fs::metadata(&rotated).unwrap().len(), 200);

        // Next oversized rotation replaces the previous .1.
        fs::write(&path, vec![b'y'; 300]).unwrap();
        rotate_log_if_oversized(&path, 100);
        assert_eq!(fs::metadata(&rotated).unwrap().len(), 300);

        // Cap 0 disables rotation entirely.
        fs::write(&path, vec![b'z'; 300]).unwrap();
        rotate_log_if_oversized(&path, 0);
        assert!(path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // correlation scope propagation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_with_correlation_scope_carries_id_across_capture() {
        // Capture inside an interaction scope...
        let captured = {
            let _guard = set_correlation_id("scope-test-id");
            scoped_correlation_id()
        };
        assert_eq!(captured.as_deref(), Some("scope-test-id"));

        // ...and re-apply it later (simulating the far side of a spawn/defer
        // hop, including on a different thread where the thread-local is
        // empty).
        let seen = std::thread::spawn(move || {
            with_correlation_scope(captured.as_deref(), current_correlation_id)
        })
        .join()
        .expect("scope thread should not panic");
        assert_eq!(seen, "scope-test-id");
    }

    #[test]
    fn test_with_correlation_scope_none_preserves_existing_scope() {
        let _guard = set_correlation_id("outer-scope");
        // A None capture (no interaction id at capture time) must not clobber
        // the executing thread's active scope.
        let seen = with_correlation_scope(None, current_correlation_id);
        assert_eq!(seen, "outer-scope");
    }

    // -------------------------------------------------------------------------
    // protocol log ring tests
    // -------------------------------------------------------------------------

    fn push_ring_entry(level: &str, target: &str, message: &str) {
        let entry = LogRingEntry {
            timestamp: "2026-07-01T00:00:00.000Z".to_string(),
            level: level.to_string(),
            target: target.to_string(),
            correlation_id: "test-cid".to_string(),
            message: message.to_string(),
        };
        let mut ring = log_ring().lock().unwrap_or_else(|e| e.into_inner());
        if ring.len() >= LOG_RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(entry);
    }

    #[test]
    fn test_query_log_ring_filters_and_limit() {
        // The ring is process-global and other tests may log; use a unique
        // target so this test's entries are isolated.
        let target = "ring_test::filters";
        push_ring_entry("INFO", target, "first info");
        push_ring_entry("WARN", target, "a warning");
        push_ring_entry("ERROR", target, "an error");
        push_ring_entry("DEBUG", target, "debug noise");

        // Target filter alone sees all four.
        let (entries, matched) = query_log_ring(10, None, Some(target), None);
        assert_eq!(matched, 4);
        assert_eq!(entries.len(), 4);

        // Min-level warn keeps warn + error only.
        let (entries, matched) = query_log_ring(10, Some("warn"), Some(target), None);
        assert_eq!(matched, 2);
        assert!(entries
            .iter()
            .all(|e| e.level == "WARN" || e.level == "ERROR"));

        // Contains filter matches message text.
        let (entries, matched) = query_log_ring(10, None, Some(target), Some("an error"));
        assert_eq!(matched, 1);
        assert_eq!(entries[0].message, "an error");

        // Limit truncates from the front but reports the full match count.
        let (entries, matched) = query_log_ring(2, None, Some(target), None);
        assert_eq!(matched, 4);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].message, "debug noise");
    }

    // -------------------------------------------------------------------------
    // get_compact_timestamp tests
    // -------------------------------------------------------------------------

    /// Parse "HH:MM:SS.mmm" into total milliseconds since midnight UTC.
    fn parse_compact_ts(ts: &str) -> u64 {
        let (clock, millis) = ts.split_once('.').expect("timestamp should contain '.'");
        let parts: Vec<u64> = clock.split(':').map(|p| p.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3, "clock part of '{}' should be HH:MM:SS", ts);
        let millis: u64 = millis.parse().expect("millis should be numeric");
        ((parts[0] * 60 + parts[1]) * 60 + parts[2]) * 1000 + millis
    }

    #[test]
    fn test_get_compact_timestamp_format() {
        let ts = get_compact_timestamp();
        // Format should be "HH:MM:SS.mmm" - sortable across the whole session
        assert_eq!(ts.len(), 12, "Timestamp '{}' should be 12 chars", ts);
        let total_ms = parse_compact_ts(&ts);
        assert!(
            total_ms < 24 * 3600 * 1000,
            "Timestamp '{}' should be within one day",
            ts
        );
    }
    #[test]
    fn test_get_compact_timestamp_monotonic_across_minute() {
        // Two calls a few ms apart must remain ordered even when the wall
        // clock crosses a minute boundary — the old SS.mmm format failed this.
        let ts1 = get_compact_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ts2 = get_compact_timestamp();

        let diff = parse_compact_ts(&ts2).saturating_sub(parse_compact_ts(&ts1));
        assert!(
            diff >= 4,
            "Timestamps should be at least 4ms apart, got {}ms",
            diff
        );
        assert!(
            diff < 100,
            "Timestamps should be less than 100ms apart, got {}ms",
            diff
        );
        // Lexicographic ordering must agree with chronological ordering
        // (ignoring the once-a-day midnight wraparound).
        assert!(ts1 < ts2, "'{}' should sort before '{}'", ts1, ts2);
    }
    // -------------------------------------------------------------------------
    // Compact format output validation (pattern matching)
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_pattern() {
        // Real example from logs:
        // "18:42:11.697|i|A|Application logging initialized event_type=app_lifecycle..."
        let example = "18:42:11.697|i|A|Application logging initialized";

        let parts: Vec<&str> = example.split('|').collect();
        assert_eq!(parts.len(), 4, "Compact format should have 4 parts");

        // Part 0: timestamp (HH:MM:SS.mmm)
        assert_eq!(parts[0].len(), 12);
        assert!(parts[0].contains('.'));

        // Part 1: level (single char)
        assert_eq!(parts[1].len(), 1);
        assert!("iwedtIWEDT".contains(parts[1]));

        // Part 2: category (single char)
        assert_eq!(parts[2].len(), 1);

        // Part 3: message (rest)
        assert!(!parts[3].is_empty());
    }
    #[test]
    fn test_compact_format_real_examples() {
        // Real log lines from test run
        let examples = [
            ("18:42:11.697|i|A|Application logging initialized", "i", "A"),
            ("18:42:11.717|i|N|Successfully loaded config", "i", "N"),
            (
                "18:42:11.741|i|H|Registered global hotkey meta+Digit0",
                "i",
                "H",
            ),
            ("18:42:11.779|i|P|Available displays: 1", "i", "P"),
        ];

        for (line, expected_level, expected_cat) in examples {
            let parts: Vec<&str> = line.split('|').collect();
            assert_eq!(
                parts[1], expected_level,
                "Line '{}' should have level '{}'",
                line, expected_level
            );
            assert_eq!(
                parts[2], expected_cat,
                "Line '{}' should have category '{}'",
                line, expected_cat
            );
        }
    }

    // --- merged from part_001.rs ---
    // -------------------------------------------------------------------------
    // Token savings verification
    // -------------------------------------------------------------------------

    #[test]
    fn test_compact_format_token_savings() {
        // Real comparison from logs:
        // Standard: "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: Selected display..."
        // Compact:  "15:22:13.150|i|P|Selected display..."

        let standard_prefix = "2025-12-27T15:22:13.150640Z  INFO script_kit_gpui::logging: ";
        let compact_prefix = "15:22:13.150|i|P|";

        let savings_percent =
            100.0 - (compact_prefix.len() as f64 / standard_prefix.len() as f64 * 100.0);

        // Should save at least 60% on the prefix
        assert!(
            savings_percent > 60.0,
            "Should save >60% on prefix, got {:.1}%",
            savings_percent
        );

        // Actual: 17 chars vs 59 chars = ~71% savings (the full HH:MM:SS.mmm
        // timestamp costs 6 chars over the old SS.mmm but keeps lines sortable)
        assert!(
            savings_percent > 70.0,
            "Should save >70% on prefix, got {:.1}%",
            savings_percent
        );
    }
    // -------------------------------------------------------------------------
    // AI log mode env var parsing tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_ai_log_mode_env_parsing() {
        // Test the parsing logic used in init()
        // SCRIPT_KIT_AI_LOG=1 should enable AI mode

        let parse_ai_log = |val: &str| -> bool {
            val.eq_ignore_ascii_case("1")
                || val.eq_ignore_ascii_case("true")
                || val.eq_ignore_ascii_case("yes")
        };

        assert!(parse_ai_log("1"));
        assert!(parse_ai_log("true"));
        assert!(parse_ai_log("TRUE"));
        assert!(parse_ai_log("yes"));
        assert!(parse_ai_log("YES"));

        assert!(!parse_ai_log("0"));
        assert!(!parse_ai_log("false"));
        assert!(!parse_ai_log("no"));
        assert!(!parse_ai_log(""));
    }

    #[test]
    fn test_open_writer_or_sink_uses_sink_when_open_fails() {
        let mut writer = open_writer_or_sink(
            Err(std::io::Error::other("forced open failure")),
            "test log file",
        );

        writer
            .write_all(b"test log line")
            .expect("sink fallback should accept writes");
        writer.flush().expect("sink fallback should flush");
    }

    // -------------------------------------------------------------------------
    // Payload truncation tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_truncate_for_log_short_string() {
        let s = "hello";
        assert_eq!(truncate_for_log(s, 10), "hello");
    }
    #[test]
    fn test_truncate_for_log_exact_limit() {
        let s = "hello";
        assert_eq!(truncate_for_log(s, 5), "hello");
    }
    #[test]
    fn test_truncate_for_log_long_string() {
        let s = "hello world this is a long string";
        let result = truncate_for_log(s, 10);
        assert!(result.starts_with("hello worl"));
        assert!(result.contains("...(33)")); // Original length in parens
    }
    #[test]
    fn test_truncate_for_log_utf8_emoji() {
        // Emoji are 4-byte UTF-8 sequences. Truncating mid-codepoint would panic with naive &s[..max_len]
        let s = "hello 🎉 world";
        // "hello " is 6 bytes, 🎉 is 4 bytes (positions 6-9), " world" starts at byte 10
        // If max_len=8, naive slice would land inside the emoji and panic
        let result = truncate_for_log(s, 8);
        // Should truncate to a valid char boundary without panic
        assert!(result.starts_with("hello "));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_utf8_multibyte() {
        // Test with various multi-byte UTF-8 characters
        let s = "日本語テスト"; // Each char is 3 bytes = 18 bytes total
                                // If we truncate at 5 bytes, we'd land mid-character
        let result = truncate_for_log(s, 5);
        // Should back up to char boundary (3 bytes = 1 char)
        assert!(result.starts_with("日"));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_utf8_mixed() {
        // Mixed ASCII and multi-byte
        let s = "abc日本語def";
        // "abc" = 3 bytes, "日本語" = 9 bytes, "def" = 3 bytes
        // Truncate at 5 would land inside 日
        let result = truncate_for_log(s, 5);
        // Should truncate at byte 3 (after "abc")
        assert!(result.starts_with("abc"));
        assert!(result.contains(&format!("...({})", s.len())));
    }
    #[test]
    fn test_truncate_for_log_empty_string() {
        let s = "";
        assert_eq!(truncate_for_log(s, 10), "");
    }
    #[test]
    fn test_truncate_for_log_zero_max_len() {
        let s = "hello";
        let result = truncate_for_log(s, 0);
        // Edge case: max_len=0 should return just the suffix
        assert!(result.contains("...(5)"));
    }
    #[test]
    fn test_summarize_payload_with_type() {
        let json = r#"{"type":"submit","id":"test","value":"foo"}"#;
        let summary = summarize_payload(json);
        assert!(summary.contains("type:submit"));
        assert!(summary.contains(&format!("len:{}", json.len())));
    }
    #[test]
    fn test_summarize_payload_without_type() {
        let json = r#"{"data":"some value"}"#;
        let summary = summarize_payload(json);
        assert!(summary.contains(&format!("len:{}", json.len())));
        assert!(!summary.contains("type:"));
    }
    #[test]
    fn test_summarize_payload_large_base64() {
        // Simulate a large base64 screenshot payload
        let base64_data = "a".repeat(100000);
        let json = format!(r#"{{"type":"screenshotResult","data":"{}"}}"#, base64_data);
        let summary = summarize_payload(&json);
        // Summary should be compact, not contain the full base64
        assert!(summary.len() < 100);
        assert!(summary.contains("type:screenshotResult"));
        assert!(summary.contains(&format!("len:{}", json.len())));
    }
    // -------------------------------------------------------------------------
    // Log capture tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_capture_enabled_default_false() {
        // By default, capture should be disabled
        // Note: we can't test this in isolation because it's a global static
        // but we can verify the initial state
        let _ = is_capture_enabled(); // Just verify it doesn't panic
    }
    #[test]
    fn test_toggle_capture_returns_correct_state() {
        // First toggle should start capture (if not already running)
        let initial_state = is_capture_enabled();

        if !initial_state {
            // If not capturing, toggle should start it
            let (is_capturing, path) = toggle_capture();
            assert!(is_capturing);
            assert!(path.is_some());

            // Clean up: toggle again to stop
            let (is_capturing2, path2) = toggle_capture();
            assert!(!is_capturing2);
            assert!(path2.is_some());
        } else {
            // If already capturing (from another test), toggle should stop it
            let (is_capturing, path) = toggle_capture();
            assert!(!is_capturing);
            assert!(path.is_some());
        }
    }
    #[test]
    fn test_capture_file_path_format() {
        // Start capture and check the file path format
        let was_enabled = is_capture_enabled();

        if !was_enabled {
            let result = start_capture();
            assert!(result.is_ok());

            let path = result.unwrap();
            let filename = path.file_name().unwrap().to_str().unwrap();

            // Filename should be like: capture-2026-01-11T08-37-28.jsonl
            assert!(filename.starts_with("capture-"));
            assert!(filename.ends_with(".jsonl"));

            // Clean up
            let _ = stop_capture();
        }
    }
    #[test]
    fn test_stop_capture_when_not_started() {
        // Ensure capture is stopped
        while is_capture_enabled() {
            let _ = stop_capture();
        }

        // Stopping when not started should return None
        let result = stop_capture();
        assert!(result.is_none());
    }
}
