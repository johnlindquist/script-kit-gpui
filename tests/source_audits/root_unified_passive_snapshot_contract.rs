fn braced_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let body = source
        .split_once(signature)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("missing structural signature `{signature}`"));
    let mut depth = 1_usize;
    for (index, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &body[..index];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated structural signature `{signature}`");
}

#[test]
fn root_browser_tabs_foreground_search_is_cache_only() {
    let browser_tabs = include_str!("../../src/browser_tabs.rs");
    let search_fn = browser_tabs
        .split("pub(crate) fn search_root_browser_tabs_meta(")
        .nth(1)
        .and_then(|rest| rest.split("pub(crate) fn focus_root_browser_tab(").next())
        .expect("search_root_browser_tabs_meta should exist");

    assert!(search_fn.contains("search_root_browser_tabs_meta_cached(query, options)"));
    assert!(
        !search_fn.contains("list_open_tabs("),
        "foreground browser-tab root search must not call AppleScript/JXA tab enumeration"
    );
}

#[test]
fn root_browser_history_foreground_search_is_cache_only() {
    let browser_history = include_str!("../../src/browser_history.rs");
    let search_fn = browser_history
        .split("pub(crate) fn search_root_browser_history_meta(")
        .nth(1)
        .and_then(|rest| rest.split("pub(crate) fn open_browser_history_url(").next())
        .expect("search_root_browser_history_meta should exist");

    assert!(search_fn.contains("ensure_root_browser_history_refresh("));
    assert!(search_fn.contains("cached_root_browser_history_snapshot("));
    assert!(
        search_fn.find("root_browser_history_query_is_eligible(")
            < search_fn.find("ensure_root_browser_history_refresh("),
        "disabled or ineligible browser-history root search must not start a refresh"
    );
    assert!(
        !search_fn.contains("copy_sqlite_db_snapshot("),
        "foreground browser-history root search must not copy browser SQLite databases"
    );
    assert!(
        !search_fn.contains("Connection::open"),
        "foreground browser-history root search must not open SQLite databases"
    );
}

#[test]
fn passive_snapshot_refresh_does_not_publish_current_frame() {
    for (source_name, source) in [
        ("browser_tabs", include_str!("../../src/browser_tabs.rs")),
        (
            "browser_history",
            include_str!("../../src/browser_history.rs"),
        ),
    ] {
        assert!(
            !source.contains("invalidate_grouped_cache"),
            "{source_name} passive snapshot refresh must not invalidate grouped root results"
        );
        assert!(
            !source.contains("publish_active_results"),
            "{source_name} passive snapshot refresh must not publish into the active root frame"
        );
        assert!(
            !source.contains("cx.notify"),
            "{source_name} passive snapshot refresh must not notify the main UI frame"
        );
    }
}

#[test]
fn browser_snapshot_sources_expose_status_for_state_first_proof() {
    let browser_tabs = include_str!("../../src/browser_tabs.rs");
    let browser_history = include_str!("../../src/browser_history.rs");

    for (source_name, source, fn_name) in [
        (
            "browser_tabs",
            browser_tabs,
            "root_browser_tabs_snapshot_status",
        ),
        (
            "browser_history",
            browser_history,
            "root_browser_history_snapshot_status",
        ),
    ] {
        assert!(
            source.contains("pub(crate) struct RootPassiveSnapshotStatus")
                && source.contains("pub generation: u64")
                && source.contains("pub refreshing: bool")
                && source.contains("pub cached_count: usize")
                && source.contains(&format!("pub(crate) fn {fn_name}(")),
            "{source_name} must expose content-free snapshot status for runtime proof"
        );
    }
}

#[test]
fn browser_tabs_implicit_warmup_lives_in_app_layer_not_cached_lookup() {
    let filtering = include_str!("../../src/app_impl/filtering_cache.rs");
    let tabs = include_str!("../../src/browser_tabs.rs");
    let cached_section = tabs
        .split("pub(crate) fn search_root_browser_tabs_meta_cached")
        .nth(1)
        .and_then(|rest| {
            rest.split("pub(crate) fn search_root_browser_tabs_meta_direct")
                .next()
        })
        .expect("cached browser tabs helper should exist");

    assert!(
        filtering.contains("implicit_tabs_query"),
        "ordinary browser-tabs warmup should be app-managed"
    );
    assert!(
        filtering.contains("background_executor()")
            && filtering.contains("refresh_root_browser_tabs_snapshot(providers)"),
        "implicit warmup should run browser enumeration off the UI path"
    );
    assert!(
        !cached_section.contains("ensure_root_browser_tabs_refresh")
            && !cached_section.contains("refresh_root_browser_tabs_snapshot")
            && !cached_section.contains("std::thread::spawn"),
        "cached lookup must stay foreground cache-only"
    );
}

#[test]
fn filtering_cache_freezes_passive_snapshot_hits_per_query_frame() {
    let app_state = include_str!("../../src/main_sections/app_state.rs");
    let filtering_cache = include_str!("../../src/app_impl/filtering_cache.rs");
    let preflight_types = include_str!("../../src/main_window_preflight/types.rs");
    let preflight_build = include_str!("../../src/main_window_preflight/build.rs");

    let app_body = braced_body(app_state, "pub(crate) struct ScriptListApp {");
    let frame_fn = filtering_cache
        .split("fn root_passive_frame_for_current_query(")
        .nth(1)
        .and_then(|rest| rest.split("fn root_file_frame_for_current_query(").next())
        .expect("root_passive_frame_for_current_query should exist");
    let preflight_fn = preflight_build
        .split("fn build_root_passive_frame_receipt(")
        .nth(1)
        .and_then(|rest| rest.split("fn build_root_file_frame_receipt(").next())
        .expect("build_root_passive_frame_receipt should exist");

    assert!(app_state.contains("pub(crate) struct RootPassiveFrameKey"));
    assert!(app_state.contains("pub(crate) struct RootPassiveFrame"));
    assert!(app_body.contains("root_search: RootSearchStore"));
    assert!(!app_body.contains("root_passive_frame:"));
    assert!(filtering_cache.contains("fn root_passive_frame_for_current_query("));
    assert!(
        frame_fn.contains("self.root_search.cached_root_passive_frame(&key)")
            && frame_fn.contains("self.root_search.cache_root_passive_frame(frame)"),
        "same-query passive frames should reuse the frozen hit vectors"
    );
    assert!(
        filtering_cache.contains("search_root_notes_meta_cached(")
            && filtering_cache.contains("search_root_clipboard_history_meta_cached(")
            && filtering_cache.contains("search_root_dictation_history_cached(")
            && filtering_cache.contains("search_history_cached(")
            && filtering_cache.contains("search_root_browser_tabs_meta_cached(")
            && filtering_cache.contains("search_root_browser_history_meta_direct(")
            && filtering_cache.contains("&root_passive_frame.note_hits")
            && filtering_cache.contains("&root_passive_frame.clipboard_history_hits")
            && filtering_cache.contains("&root_passive_frame.dictation_history_hits")
            && filtering_cache.contains("&root_passive_frame.agent_chat_history_hits")
            && filtering_cache.contains("&root_passive_frame.browser_tab_hits")
            && filtering_cache.contains("&root_passive_frame.browser_history_hits"),
        "all passive source hits should flow through the frozen passive frame"
    );
    assert!(
        preflight_types.contains("RootPassiveFrameReceipt")
            && preflight_types.contains("RootPassiveSourceReceipt")
            && preflight_types.contains("root_passive_frame: Option<RootPassiveFrameReceipt>"),
        "preflight state receipts should expose content-free passive frame status"
    );
    assert!(
        preflight_fn.contains("app.root_search.root_passive_frame()")
            && preflight_fn.contains("root_browser_tabs_snapshot_status()")
            && preflight_fn.contains("root_browser_history_snapshot_status()"),
        "runtime proof should be able to wait for browser passive refresh completion"
    );
}

/// Root grouping runs synchronously on every launcher keystroke, so blocking
/// SQLite/day-page readers must remain outside this exact structural owner.
#[test]
fn foreground_grouping_uses_cache_only_passive_sources() {
    let filtering_cache = include_str!("../../src/app_impl/filtering_cache.rs");
    let frame_fn = filtering_cache
        .split("fn root_passive_frame_for_current_query(")
        .nth(1)
        .and_then(|rest| rest.split("fn root_file_frame_for_current_query(").next())
        .expect("root_passive_frame_for_current_query should exist");

    for required in [
        "search_root_notes_meta_cached(",
        "search_root_todos_cached(",
        "search_root_clipboard_history_meta_cached(",
        "search_root_dictation_history_cached(",
        "search_history_cached(",
    ] {
        assert!(
            frame_fn.contains(required),
            "root passive frame should use cache-only foreground source `{required}`"
        );
    }

    for forbidden in [
        "search_root_notes_meta(",
        "search_root_notes_meta_direct(",
        "search_root_todos_direct(",
        "search_root_clipboard_history_meta(",
        "search_root_dictation_history(",
        "crate::ai::agent_chat::ui::history::search_history(",
    ] {
        assert!(
            !frame_fn.contains(forbidden),
            "foreground root passive frame must not call blocking source `{forbidden}`"
        );
    }
}

#[test]
fn cold_passive_sources_warm_future_frames_without_publishing() {
    let notes = include_str!("../../src/notes/storage.rs");
    let agent_chat = include_str!("../../src/ai/agent_chat/ui/history.rs");
    let clipboard = include_str!("../../src/clipboard_history/cache.rs");
    let dictation = include_str!("../../src/dictation/history.rs");

    for (source_name, source, cached_fn, warmer) in [
        (
            "notes",
            notes,
            "search_root_notes_meta_cached(",
            "root-notes-search-cache",
        ),
        (
            "agent_chat_history",
            agent_chat,
            "search_history_cached(",
            "root-agent_chat-history-cache",
        ),
        (
            "clipboard_history",
            clipboard,
            "search_root_clipboard_history_meta_cached(",
            "root-clipboard-history-cache",
        ),
        (
            "dictation_history",
            dictation,
            "search_root_dictation_history_cached(",
            "root-dictation-history-cache",
        ),
    ] {
        assert!(
            source.contains(cached_fn) && source.contains(warmer),
            "{source_name} should expose a cache-only root search and background warmer"
        );
        assert!(
            !source.contains("invalidate_grouped_cache")
                && !source.contains("publish_active_results")
                && !source.contains("cx.notify"),
            "{source_name} cache warmers must not publish into the active root frame"
        );
    }
}

#[test]
fn agentic_root_passive_frame_stability_proof_uses_state_receipts() {
    let proof = include_str!("../../scripts/agentic/root-passive-frame-stability.ts");

    for required in [
        "setFilter",
        "waitFor",
        "stateMatch",
        "getState",
        "mainWindowPreflight",
        "rootPassiveFrame",
        "selectedResultKey",
        "visibleResultKeyFingerprint",
        "enterAction",
        "browserTabs",
        "browserHistory",
        "refreshing",
    ] {
        assert!(
            proof.contains(required),
            "passive-frame runtime proof should contain `{required}`"
        );
    }

    assert!(
        !proof.contains("captureScreenshot") && !proof.contains("simulateClick"),
        "passive-frame proof should stay state-first, not screenshot or mouse based"
    );
}

#[test]
fn root_defaults_benchmark_enforces_input_echo_and_staleness() {
    let proof = include_str!("../../scripts/agentic/root-all-passive-defaults-benchmark.ts");
    for required in [
        "setFilterAndMeasureEcho",
        "measureRapidTypingCase",
        "measureInFlightCancellation",
        "inputEchoMs",
        "staleResultCount",
        "finalInputStillEchoed",
        "typingInputEchoP95Ms",
        "overlappedInputEchoP95Ms",
        "computedSearchTextMismatchCount",
    ] {
        assert!(
            proof.contains(required),
            "root passive benchmark should enforce `{required}`"
        );
    }
    assert!(
        proof.contains("waitForSourceResult(")
            && proof.contains("expectedComputedSearchText"),
        "source settle waits must be keyed by the expected final query, not the global fixture query"
    );
    assert!(
        !proof.contains("captureScreenshot") && !proof.contains("simulateClick"),
        "input responsiveness proof should stay state/protocol-first"
    );
}

#[test]
fn root_delete_key_benchmark_enforces_key_repeat_responsiveness() {
    let proof = include_str!("../../scripts/agentic/root-delete-key-benchmark.ts");
    for required in [
        "simulateGpuiEvent",
        "key: \"backspace\"",
        "measureDeleteEcho",
        "measureDeleteBurst",
        "cadenceOverrunMaxMs",
        "computedSearchText",
        "handlerSlowCount",
        "deleteInputEchoP95Ms",
        "deleteBurstTotalP95Ms",
    ] {
        assert!(
            proof.contains(required),
            "delete-key benchmark should enforce `{required}`"
        );
    }
    assert!(
        proof.contains("waitForInput(expected)") && proof.contains("computedMatchesInput"),
        "delete-key proof must wait for the real input echo and computed query after backspace"
    );
    assert!(
        !proof.contains("captureScreenshot") && !proof.contains("simulateClick"),
        "delete-key responsiveness proof should stay state/protocol-first"
    );
}

#[test]
fn root_file_test_provider_supports_multi_query_cancellation_fixture() {
    let root_file = include_str!("../../src/app_impl/root_file_search.rs");
    for required in [
        "enum RootFileSearchTestProvider",
        "fixtures: Vec<RootFileSearchTestFixture>",
        "passthrough_unmatched",
        "SearchEvent::Done",
        "fixture.query == query",
    ] {
        assert!(
            root_file.contains(required),
            "root file test provider should support deterministic multi-query fixture field `{required}`"
        );
    }
}

/// JSONL history must reuse a signature-keyed index and open private files
/// through their no-follow owner; Result-returning loaders are valid owners.
#[test]
fn jsonl_history_sources_use_mtime_backed_foreground_indexes() {
    for (source_name, source) in [
        (
            "agent_chat_history",
            include_str!("../../src/ai/agent_chat/ui/history.rs"),
        ),
        (
            "dictation_history",
            include_str!("../../src/dictation/history.rs"),
        ),
    ] {
        assert!(
            source.contains("OnceLock<Mutex<Option<")
                && source.contains("IndexCache")
                && source.contains("history_file_signature(&path)")
                && source.contains("parse_history_entries(&content)")
                && source.contains("cache.entries.clone()")
                && (source.contains("read_private_history_file(&path)")
                    || source.contains("crate::atomic_file::read_private_file(path)")),
            "{source_name} root-search history reads should reuse a private mtime-backed index cache"
        );
        assert!(
            source.contains("invalidate_history_cache();"),
            "{source_name} history writes/deletes must invalidate the root-search index cache"
        );
    }
}

/// History search must borrow its already-bounded transcript and use the
/// shared field/evidence matcher instead of renormalizing it in the row owner.
#[test]
fn agent_chat_history_root_cache_bounds_search_text_before_foreground_ranking() {
    let source = include_str!("../../src/ai/agent_chat/ui/history.rs");
    let rank_fn = source
        .split("fn rank_history_entries(")
        .nth(1)
        .and_then(|rest| rest.split("/// Search loaded history entries").next())
        .expect("rank_history_entries should exist");

    assert!(
        source.contains("const HISTORY_SEARCH_TEXT_MAX_CHARS")
            && source.contains("fn bounded_search_text(")
            && source.contains("entry.search_text = bounded_search_text(&entry.search_text);"),
        "Agent Chat history cache should clamp legacy multi-megabyte search_text fields before root ranking"
    );
    assert!(
        rank_fn.contains("text: entry.search_text.as_str(),")
            && rank_fn.contains("match_long_text_query(&compiled, &fields)")
            && !rank_fn.contains("normalize_search_text(&entry.search_text)"),
        "foreground Agent Chat history ranking should borrow bounded cached search_text for the shared evidence matcher"
    );
}

#[test]
fn passive_root_sections_share_score_cap_and_order() {
    let grouping = include_str!("../../src/scripts/grouping.rs");

    assert!(grouping.contains("pub const ROOT_PASSIVE_RESULT_SCORE_BASE: i32 = 100_000"));
    for section_fn in [
        "append_root_browser_tabs_section",
        "append_root_notes_section",
        "append_root_clipboard_history_section",
        "append_root_dictation_history_section",
        "append_root_agent_chat_history_section",
        "append_root_browser_history_section",
    ] {
        let section = grouping
            .split(&format!("fn {section_fn}("))
            .nth(1)
            .and_then(|rest| rest.split("\nfn ").next())
            .unwrap_or_else(|| panic!("{section_fn} should exist"));
        assert!(
            section.contains("root_passive_result_score(rank)"),
            "{section_fn} should use the shared passive score cap"
        );
    }

    assert!(
        grouping.find("append_recent_root_file_section(")
            < grouping.find("append_root_browser_tabs_section("),
        "Browser Tabs should remain after Files/Recent Files"
    );
    assert!(
        grouping.find("append_root_browser_tabs_section(")
            < grouping.find("append_root_notes_section("),
        "Browser Tabs should remain before Notes"
    );
    assert!(
        grouping.find("append_root_clipboard_history_section(")
            < grouping.find("append_root_dictation_history_section("),
        "Dictation History should remain after Clipboard History"
    );
    assert!(
        grouping.find("append_root_dictation_history_section(")
            < grouping.find("append_root_agent_chat_history_section("),
        "Dictation History should remain before Agent Chat Conversations"
    );
    assert!(
        grouping.find("append_root_agent_chat_history_section(")
            < grouping.find("append_root_browser_history_section("),
        "Browser History should remain after Agent Chat Conversations"
    );
}
