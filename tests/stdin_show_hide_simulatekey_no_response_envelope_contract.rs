//! Source-level contract for stdin response envelopes on
//! `ExternalCommand::Show`, `ExternalCommand::Hide`, and
//! `ExternalCommand::SimulateKey`.
//!
//! Run 9 Pass #5 correctively closes the Pass #4 anomaly
//! `[?] attacker-show-restores-last-view-not-main-menu` (filed Run 9
//! Pass #4, 2026-04-19T05:35Z). Pass #4 observed that a `rpc show`
//! from a "cold" session followed by `rpc getState` reported
//! `{promptType:"agentChatChat", choiceCount:0}` — it claimed `show` in
//! isolation restored a last-used view instead of the canonical
//! ScriptList. Pass #5 clean repro on a FRESH kill+restart session
//! (pid 75585) — `rpc getState` before show → `promptType:"none"`,
//! `rpc show` in isolation (no preceding `triggerBuiltin`), `rpc
//! getState` → `{promptType:"none", windowVisible:true, isFocused:true,
//! choiceCount:476}` (ScriptList). The Pass #4 observation was a
//! transient artifact — likely a stale view left by a prior pass in
//! that long-running pid 30450 session — NOT a show-restore bug. The
//! Show handler in every dispatcher is inspectable at source and does
//! not mutate `view.current_view`; the Hide handler calls
//! `view.reset_to_script_list(ctx)` (see
//! `src/main_entry/runtime_stdin_match_core.rs:138`), so Hide + Show
//! correctly lands back in ScriptList.
//!
//! The current invariant is split:
//! * `show` / `hide` use their dedicated `windowVisibilityAck` envelope.
//! * `simulateKey` uses `externalCommandResult`, but snapshots the
//!   request id + response sender BEFORE dispatch and sends the ack AFTER
//!   dispatch. That ordering matters because the simulated key can mutate
//!   route/session state before a post-dispatch sender lookup.
//!
//! Refactor threat: a well-meaning contributor "improves" the
//! automation DX by adding a `showResult` / `hideResult` /
//! `simulateKeyResult` response variant (analogous to `stateResult`)
//! and wiring `response_tx.send(Message::ShowResult { request_id,
//! .. })` at the tail of each dispatcher arm. Compilation passes,
//! existing tests pass, but every external automation script that
//! reads `responses.ndjson` and matches by `requestId` breaks — those
//! scripts currently treat the absence of a matching envelope as
//! "fire-and-forget; follow with getState", so a new echo would make
//! them incorrectly advance on the echoed receipt before any UI
//! mutation (show/hide) lands on screen.
//!
//! These asserts catch such a refactor before merge by forbidding
//! broad response-emission sinks inside the Show / Hide arm bodies and pin
//! the simulateKey response ordering across dispatcher copies:
//!   - `src/main_entry/runtime_stdin_match_core.rs` (Show + Hide arms)
//!   - `src/main_entry/runtime_stdin_match_simulate_key.rs` (whole file;
//!     the file is a single match arm for `ExternalCommand::SimulateKey`)
//!   - `src/main_entry/runtime_stdin.rs` (live SimulateKey arm)
//!   - `src/main_entry/app_run_setup.rs` (Show + Hide + SimulateKey arms,
//!     scoped via sibling-variant anchors)

const MATCH_CORE: &str = include_str!("../src/main_entry/runtime_stdin_match_core.rs");
const SIMULATE_KEY: &str = include_str!("../src/main_entry/runtime_stdin_match_simulate_key.rs");
const RUNTIME_STDIN: &str = include_str!("../src/main_entry/runtime_stdin.rs");
const APP_RUN_SETUP: &str = include_str!("../src/main_entry/app_run_setup.rs");

const SHOW_ANCHOR: &str = "ExternalCommand::Show { ref request_id } => {";
const HIDE_ANCHOR: &str = "ExternalCommand::Hide { ref request_id } => {";
const SET_FILTER_ANCHOR: &str = "ExternalCommand::SetFilter {";

const SIMKEY_ANCHOR: &str =
    "ExternalCommand::SimulateKey { ref key, ref modifiers, ref target, ref request_id } => {";
const OPEN_NOTES_ANCHOR: &str = "ExternalCommand::OpenNotes";

/// Slice the `ExternalCommand::Show` arm body — start at the `Show`
/// anchor, end at the `Hide` anchor (the next sibling variant). Both
/// `runtime_stdin_match_core.rs` and `app_run_setup.rs` place Hide
/// immediately after Show; a contributor reordering sibling variants
/// must update this anchor.
fn show_arm<'a>(source: &'a str, file_name: &str) -> &'a str {
    let start = source.find(SHOW_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must declare `{SHOW_ANCHOR}` — if the arm's pattern \
             was reshaped (e.g. `{{ request_id, .. }}`), update this anchor in \
             the same commit"
        )
    });
    let end_offset = source[start..].find(HIDE_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must continue with `{HIDE_ANCHOR}` after Show — a \
             sibling-variant reorder must update this anchor"
        )
    });
    &source[start..start + end_offset]
}

/// Slice the `ExternalCommand::Hide` arm body — start at the `Hide`
/// anchor, end at the `SetFilter` anchor (the next sibling variant).
fn hide_arm<'a>(source: &'a str, file_name: &str) -> &'a str {
    let start = source.find(HIDE_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must declare `{HIDE_ANCHOR}` — if the arm's pattern \
             was reshaped, update this anchor in the same commit"
        )
    });
    let end_offset = source[start..].find(SET_FILTER_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must continue with `{SET_FILTER_ANCHOR}` after Hide \
             — a sibling-variant reorder must update this anchor"
        )
    });
    &source[start..start + end_offset]
}

/// Slice the `ExternalCommand::SimulateKey` arm body. Sibling-variant anchor
/// is `OpenNotes`, which follows SimulateKey in both dispatcher copies.
fn simulatekey_arm<'a>(source: &'a str, file_name: &str) -> &'a str {
    let start = source.find(SIMKEY_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must declare `{SIMKEY_ANCHOR}` — if the \
             arm's pattern was reshaped, update this anchor in the same commit"
        )
    });
    let end_offset = source[start..].find(OPEN_NOTES_ANCHOR).unwrap_or_else(|| {
        panic!(
            "{file_name} must continue with `{OPEN_NOTES_ANCHOR}` after \
             SimulateKey — a sibling-variant reorder must update this anchor"
        )
    });
    &source[start..start + end_offset]
}

/// Forbidden response-emission sinks. These are the idioms a
/// contributor would reach for when adding an echo envelope:
///
/// * `response_tx.send(` / `response_sender.send(` /
///   `reader_response_tx.send(` — the three names under which the
///   stdout echo `mpsc::SyncSender<crate::protocol::Message>` is
///   passed through the code today (see
///   `src/stdin_commands/mod.rs::create_stdout_response_sender` and
///   `src/execute_script/mod.rs`). Any one of them appearing inside
///   the dispatch arm body means the dispatcher has been wired to
///   emit a response.
///
/// * `Message::StateResult` / `Message::ShowResult` /
///   `Message::HideResult` / `Message::SimulateKeyResult` — explicit
///   constructor references. `StateResult` is the only variant that
///   currently exists (emitted by `GetState`'s handler via the
///   prompt-message pipeline, NOT by these stdin dispatchers); the
///   other three are hypothetical future variants that would signal
///   a "give each stdin command an echo" refactor.
///
/// * `ShowResult {` / `HideResult {` / `SimulateKeyResult {` — the
///   bare-construct form (without the `Message::` prefix) that a
///   `use crate::protocol::message::...` could elide. Belt to the
///   `Message::`-prefixed suspenders above.
///
/// * `"showResult"` / `"hideResult"` / `"simulateKeyResult"` — the
///   wire-format string literals a `responseType` field would carry.
///   Covers a direct `serde_json::json!({"responseType":"showResult",...})`
///   escape hatch that bypasses the typed `Message` enum.
const FORBIDDEN_RESPONSE_SINKS: &[&str] = &[
    "response_tx.send(",
    "response_sender.send(",
    "reader_response_tx.send(",
    "Message::StateResult",
    "Message::ShowResult",
    "Message::HideResult",
    "Message::SimulateKeyResult",
    "ShowResult {",
    "HideResult {",
    "SimulateKeyResult {",
    r#""showResult""#,
    r#""hideResult""#,
    r#""simulateKeyResult""#,
];

fn assert_no_response_sink(arm: &str, label: &str) {
    for forbidden in FORBIDDEN_RESPONSE_SINKS {
        assert!(
            !arm.contains(forbidden),
            "{label} contains forbidden response-emission sink `{forbidden}`. \
             The Show/Hide/SimulateKey arms are fire-and-forget by contract \
             (Run 9 Pass #5 closing Pass #4's anomaly `attacker-show-\
             restores-last-view-not-main-menu`). Adding an echo envelope \
             would break every automation script that matches by \
             `requestId` and currently treats absence-of-envelope as \
             'fire-and-forget; follow with getState'. See \
             `removed-docs` §\"Prompt and control messages\" for the \
             rationale."
        );
    }
}

#[test]
fn show_arm_emits_no_response_envelope() {
    // Both source-audit snippets AND the mega-dispatcher must keep
    // the Show arm body free of response-emission sinks.
    assert_no_response_sink(
        show_arm(MATCH_CORE, "runtime_stdin_match_core.rs"),
        "runtime_stdin_match_core.rs (Show arm)",
    );
    assert_no_response_sink(
        show_arm(APP_RUN_SETUP, "app_run_setup.rs"),
        "app_run_setup.rs (Show arm)",
    );
}

/// OF-43: a hidden reset can consume the launcher focus request before
/// AppKit finishes ordering the panel out. Every explicit Show dispatcher
/// must therefore re-arm the main filter after the native reveal/root-focus
/// phase; otherwise the panel becomes key while the launcher input remains
/// without a GPUI first responder.
#[test]
fn show_arm_rearms_main_filter_after_native_reveal() {
    for (label, arm) in [
        (
            "runtime_stdin_match_core.rs (Show arm)",
            show_arm(MATCH_CORE, "runtime_stdin_match_core.rs"),
        ),
        (
            "runtime_stdin.rs (Show arm)",
            show_arm(RUNTIME_STDIN, "runtime_stdin.rs"),
        ),
        (
            "app_run_setup.rs (Show arm)",
            show_arm(APP_RUN_SETUP, "app_run_setup.rs"),
        ),
    ] {
        let native_show = arm
            .find("platform::show_main_window_without_activation();")
            .unwrap_or_else(|| panic!("{label} must reveal the native main panel"));
        let root_focus = arm
            .find("window.focus(&focus_handle, ctx);")
            .unwrap_or_else(|| panic!("{label} must restore the GPUI root focus context"));
        let registry_sync = arm
            .find("sync_main_automation_window(None, true, true);")
            .unwrap_or_else(|| panic!("{label} must publish the visible main window"));
        let focused_input = arm
            .find("view.focused_input = FocusedInput::MainFilter;")
            .unwrap_or_else(|| {
                panic!("{label} must re-arm launcher input focus after native reveal")
            });
        let pending_focus = arm
            .find("view.pending_focus = Some(FocusTarget::MainFilter);")
            .unwrap_or_else(|| {
                panic!("{label} must request a post-reveal MainFilter focus application")
            });

        assert!(
            native_show < root_focus
                && root_focus < registry_sync
                && registry_sync < focused_input
                && focused_input < pending_focus,
            "{label} must re-arm focused_input/pending_focus after native reveal and root focus;              pre-reveal focus is consumed while hidden and then cleared by AppKit blur"
        );
    }
}

#[test]
fn hide_arm_emits_no_response_envelope() {
    assert_no_response_sink(
        hide_arm(MATCH_CORE, "runtime_stdin_match_core.rs"),
        "runtime_stdin_match_core.rs (Hide arm)",
    );
    assert_no_response_sink(
        hide_arm(APP_RUN_SETUP, "app_run_setup.rs"),
        "app_run_setup.rs (Hide arm)",
    );
}

#[test]
fn simulatekey_arm_snapshots_sender_before_dispatch_and_acks_after() {
    for (label, arm) in [
        (
            "runtime_stdin_match_simulate_key.rs (whole file = SimulateKey arm)",
            SIMULATE_KEY,
        ),
        (
            "runtime_stdin.rs (SimulateKey arm)",
            simulatekey_arm(RUNTIME_STDIN, "runtime_stdin.rs"),
        ),
        (
            "app_run_setup.rs (SimulateKey arm)",
            simulatekey_arm(APP_RUN_SETUP, "app_run_setup.rs"),
        ),
    ] {
        let capture_idx = arm
            .find("let simulate_key_response = request_id")
            .unwrap_or_else(|| panic!("{label} must snapshot request_id before dispatch"));
        let sender_capture_idx = arm
            .find("view.response_sender")
            .unwrap_or_else(|| panic!("{label} must clone response_sender before dispatch"));
        let sender_clone_idx = arm[sender_capture_idx..]
            .find(".clone()")
            .map(|offset| sender_capture_idx + offset)
            .unwrap_or_else(|| panic!("{label} must clone response_sender before dispatch"));
        let dispatch_idx = arm
            .find("view.dispatch_simulate_key(")
            .unwrap_or_else(|| panic!("{label} must dispatch exactly one simulateKey action"));
        let ack_idx = arm
            .find("crate::protocol::Message::external_command_result(")
            .unwrap_or_else(|| panic!("{label} must emit externalCommandResult"));
        let command_idx = arm
            .find("\"simulateKey\".to_string()")
            .unwrap_or_else(|| panic!("{label} must identify the command as simulateKey"));

        assert!(
            capture_idx < dispatch_idx,
            "{label} must snapshot response routing before simulateKey dispatch"
        );
        assert!(
            sender_capture_idx < dispatch_idx && sender_clone_idx < dispatch_idx,
            "{label} must clone response_sender before simulateKey dispatch"
        );
        assert!(
            dispatch_idx < ack_idx,
            "{label} must ack after dispatch so the response means the mutator ran"
        );
        assert!(
            ack_idx < command_idx,
            "{label} must populate the simulateKey command in the externalCommandResult"
        );
        assert!(
            arm.contains("if let Some((rid, sender)) = simulate_key_response"),
            "{label} must send from the pre-dispatch response snapshot"
        );
        assert!(
            !arm[dispatch_idx..ack_idx].contains("view.response_sender"),
            "{label} must not look up view.response_sender after dispatch"
        );
    }
}

#[test]
fn dispatcher_snippets_do_not_import_response_sender() {
    // Show/Hide source-audit snippets remain response-channel-free.
    // SimulateKey is intentionally excluded: it owns a narrow
    // `externalCommandResult` envelope and has its own ordering contract above.
    //
    // `app_run_setup.rs` is NOT covered here — as the mega-dispatcher
    // it legitimately hosts other arms (e.g. GetConfigFingerprint)
    // that emit structured `config_fingerprint_result` tracing, and
    // transitively imports plenty of `mpsc`/`Message` names. The Show
    // / Hide / SimulateKey arms within it are pinned above at body
    // granularity.
    for (name, source) in [("runtime_stdin_match_core.rs", MATCH_CORE)] {
        for binding in ["response_tx", "response_sender", "reader_response_tx"] {
            assert!(
                !source.contains(binding),
                "{name} must not reference `{binding}` — the source-audit \
                 snippet dispatchers are response-channel-free by design. \
                 Introducing a binding is the first step of the echo-\
                 envelope refactor; Run 9 Pass #5 pins this structural \
                 property to force that conversation at review time."
            );
        }
    }
}
