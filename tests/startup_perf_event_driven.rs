use std::fs;

fn read_source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
}

fn read_script_list_startup_source() -> String {
    read_source("src/app_impl/startup.rs")
}

fn read_prompt_ai_source() -> String {
    read_source("src/app_impl/prompt_ai.rs")
}

fn read_main_source() -> String {
    read_source("src/main.rs")
}

fn read_main_startup_bootstrap_source() -> String {
    read_source("src/main_entry/app_run_setup.rs")
}

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let tail = &source[start..];

    let next_fn = tail
        .match_indices("\n    fn ")
        .find_map(|(idx, _)| (idx > 0).then_some(idx));
    let next_pub_fn = tail
        .match_indices("\n    pub fn ")
        .find_map(|(idx, _)| (idx > 0).then_some(idx));

    let end = match (next_fn, next_pub_fn) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => tail.len(),
    };

    &tail[..end]
}

#[test]
fn test_script_list_app_new_uses_event_driven_receive_when_loading_startup_data() {
    let source = read_script_list_startup_source();
    let body = function_body(&source, "pub(crate) fn new(");

    assert!(
        body.contains("rx.recv().await"),
        "ScriptListApp::new should await channel receive instead of polling"
    );
    assert!(
        !body.contains("try_recv("),
        "ScriptListApp::new should not poll with try_recv()"
    );
    assert!(
        !body.contains("Duration::from_millis(50)"),
        "ScriptListApp::new should not use 50ms polling timers for startup bridges"
    );
}

#[test]
fn test_script_list_up_history_navigation_uses_top_of_grouped_items_boundary() {
    let source = read_script_list_startup_source();

    assert!(
        source.contains("const HISTORY: &str = \"HISTORY\";"),
        "ScriptList arrow handler should use a HISTORY log target constant"
    );
    assert!(
        source.contains("let (grouped_items, _) = this.get_grouped_results_cached();"),
        "Up arrow handler should compute grouped results for top-of-list detection"
    );
    assert!(
        source.contains("crate::list_item::GroupedListItem::Item(_)"),
        "Up arrow handler should locate the first item row in grouped results"
    );
    assert!(
        source.contains(".map(|position| this.selected_index <= position)")
            && source.contains(".unwrap_or(true);"),
        "Up arrow handler should treat missing items as top-of-list"
    );
    assert!(
        source.contains("let filter_has_text = !this.filter_text.is_empty()")
            && source.contains("filter_has_text,")
            && source.contains("\"filter_text_up_noop\"")
            && source.contains("if !source_filter_mode")
            && source.contains("&& filter_has_text")
            && source.contains("&& at_top_of_list")
            && source.contains("cx.stop_propagation();")
            && source.contains("return;"),
        "Up arrow handler should consume and noop when the main menu filter has text"
    );
    assert!(
        source.contains("if !source_filter_mode && (in_history || at_top_of_list) {")
            && source.contains("if let Some(text) = this.input_history.navigate_up() {")
            && source.contains("cx.stop_propagation();")
            && source.contains("return;"),
        "Up arrow handler should route to history recall and consume the event only when the filter is empty and in history or at top"
    );
    assert!(
        source.contains("history_filter_render_pending")
            && source.contains("history_key_repeat_coalesced_until_render"),
        "History key repeat should wait for the previous recalled filter to render before advancing again"
    );
}

#[test]
fn test_history_recall_suppresses_programmatic_filter_echo_and_waits_for_render_ack() {
    let startup = read_script_list_startup_source();
    let filter_change = read_source("src/app_impl/filter_input_change.rs");
    let filter_updates = read_source("src/app_impl/filter_input_updates.rs");
    let render_impl = read_source("src/main_sections/render_impl.rs");

    assert!(
        filter_updates.contains("self.pending_programmatic_filter_echo = Some(text.clone());"),
        "Programmatic filter writes should mark the next matching input change as an echo"
    );

    let echo_pos = filter_change
        .find("programmatic_filter_echo_suppressed")
        .expect("missing programmatic echo suppression log");
    let normal_entry_pos = filter_change
        .find("DO_IN_TRACE filter_change.entry")
        .expect("missing normal filter-change entry log");
    assert!(
        echo_pos < normal_entry_pos,
        "The delayed programmatic input echo should return before normal filter-change work"
    );

    assert!(
        startup.contains("this.history_filter_render_pending =")
            && startup.contains("history_key_repeat_coalesced_until_render"),
        "History recalls should mark render pending and coalesce repeat arrows while it is set"
    );
    assert!(
        startup.contains("history_recalled")
            && startup.contains("logging::log_user_value(&text)")
            && !startup.contains("format!(\"Recalled: {}\", text)"),
        "History recall logging should use safe previews instead of raw user-entered history text"
    );
    assert!(
        filter_updates.contains("cancel_history_filter_render_pending_if_obsolete")
            && filter_change
                .contains("self.cancel_history_filter_render_pending_if_obsolete(&new_text);"),
        "Real user filter changes should cancel obsolete pending history render gates"
    );
    assert!(
        render_impl.contains("history_filter_render_ack")
            && render_impl.contains("this.history_filter_render_pending = None;"),
        "Render should acknowledge the recalled filter before another history repeat is accepted"
    );
}

#[test]
fn test_rebuild_provider_registry_async_uses_event_driven_receive_when_refreshing_registry() {
    let source = read_prompt_ai_source();
    let body = function_body(&source, "pub fn rebuild_provider_registry_async");

    assert!(
        body.contains("rx.recv().await"),
        "rebuild_provider_registry_async should await channel receive"
    );
    assert!(
        !body.contains("try_recv"),
        "rebuild_provider_registry_async should not poll with try_recv()"
    );
    assert!(
        !body.contains("Duration::from_millis(50)"),
        "rebuild_provider_registry_async should not use 50ms polling timers"
    );
}

#[test]
fn test_main_defers_tray_initialization_until_after_window_creation_for_first_render() {
    let main_source = read_main_source();
    assert!(
        main_source.contains("include!(\"main_entry/app_run_setup.rs\");"),
        "main.rs should include main_entry/app_run_setup.rs for startup wiring"
    );

    let source = read_main_startup_bootstrap_source();

    let window_open_log_idx = source
        .find("Window opened, creating ScriptListApp wrapped in Root")
        .expect("missing window-open log in app_run_setup.rs");
    let tray_init_idx = source
        .find("Tray icon initialized successfully (deferred)")
        .expect("missing deferred tray init log in app_run_setup.rs");
    let deferred_timer_idx = source
        .find("Duration::from_millis(1)")
        .expect("missing deferred tray init timer in app_run_setup.rs");

    assert!(
        tray_init_idx > window_open_log_idx,
        "tray init should be deferred until after window creation"
    );
    assert!(
        deferred_timer_idx < tray_init_idx,
        "tray init path should include a deferred timer before initialization"
    );
}

#[test]
fn test_main_tray_handler_uses_event_driven_receive_when_processing_menu_events() {
    let main_source = read_main_source();
    assert!(
        main_source.contains("include!(\"main_entry/app_run_setup.rs\");"),
        "main.rs should include main_entry/app_run_setup.rs for startup wiring"
    );

    let source = read_main_startup_bootstrap_source();

    let tray_block_start = source
        .find("Tray menu event handler started (event-driven)")
        .expect("missing tray event-driven handler block");
    let tray_block = &source[tray_block_start..];

    assert!(
        tray_block.contains("while let Ok(event) = tray_event_rx.recv().await"),
        "tray handler should await async-channel receive"
    );
    assert!(
        !tray_block.contains("menu_event_receiver().try_recv()"),
        "tray handler should not poll menu events with try_recv()"
    );
    assert!(
        !tray_block.contains("Duration::from_millis(250)"),
        "tray handler should not use 250ms polling timers"
    );
}
