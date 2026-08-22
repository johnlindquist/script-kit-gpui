//! Source-level contract for `tool-state-image-identity` (Run 5 Pass #1).
//!
//! The `screenshot-identity-threading` user story wanted a live receipt
//! of `getState.imageIdentity="screenshot:<timestamp>"` after a tab-ai
//! capture, so automation could verify identity threading from capture
//! through to Agent Chat context without grepping the filesystem. Pass #14 of
//! Run 1 discovered three structural gaps; the remaining one —
//! "`imageIdentity` is not a `State` field" — is closed here by adding
//! `screenshotIdentity: Option<String>` to the `stateResult` JSON.
//!
//! The field carries the BARE filename of the most recent tab-ai
//! screenshot captured in this process lifetime (`None` if no capture
//! has happened). Identity is already encoded in that filename via
//! `build_tab_ai_screenshot_filename` (timestamp + PID + sequence), so
//! reading `getState.screenshotIdentity` gives automation the same
//! identity the Agent Chat context line `screenshot path: <path>` carries —
//! just without the leading directory and without a filesystem read.
//!
//! Contract pinned:
//!
//! 1. `StateResult` variant declares `screenshot_identity: Option<String>`
//!    with `#[serde(rename = "screenshotIdentity", ...)]`, `default`, and
//!    `skip_serializing_if = "Option::is_none"` — so the key is simply
//!    absent when no capture has occurred (not `null`). Presence of a
//!    `null`-valued key would widen the protocol surface; every parser
//!    would have to handle `undefined | null | string` where `undefined
//!    | string` is enough.
//!
//! 2. `Message::state_result` constructor accepts `screenshot_identity:
//!    Option<String>` as a trailing positional parameter. Keeping it near
//!    the end of the constructor lets future
//!    refactors introduce more state-snapshot fields in the same
//!    trailing slot without reshuffling the whole signature.
//!
//! 3. The `Tab AI` capture paths call `record_last_screenshot_identity`
//!    after a successful file write. The recorded value is the BARE
//!    filename returned by `build_tab_ai_screenshot_filename`, NOT the
//!    absolute path — the automation field is an identity, not a
//!    filesystem handle. Reading through a path would require automation
//!    to know the tmp dir layout.
//!
//! 4. `current_screenshot_identity()` is the single public read
//!    accessor. The prompt-handler state snapshot wires through this
//!    accessor — reshuffling the static's Mutex/OnceLock wrapper
//!    without routing reads through the accessor would break the
//!    snapshot silently.

const QUERY_OPS_VARIANTS: &str = include_str!("../src/protocol/message/variants/query_ops.rs");
const QUERY_OPS_CONSTRUCTORS: &str =
    include_str!("../src/protocol/message/constructors/query_ops.rs");
const SCREENSHOT_FILES_SOURCE: &str = include_str!("../src/ai/harness/screenshot_files.rs");
const PROMPT_HANDLER_SOURCE: &str = include_str!("../src/prompt_handler/mod.rs");

#[test]
fn state_result_variant_declares_screenshot_identity_with_camel_case_rename() {
    assert!(
        QUERY_OPS_VARIANTS.contains("screenshot_identity: Option<String>,"),
        "src/protocol/message/variants/query_ops.rs `StateResult` variant \
         must declare `screenshot_identity: Option<String>,` — the field \
         that carries the most-recent tab-ai screenshot's bare filename \
         for identity-threading verification."
    );
    assert!(
        QUERY_OPS_VARIANTS.contains("rename = \"screenshotIdentity\""),
        "src/protocol/message/variants/query_ops.rs `StateResult.screenshot_identity` \
         must be renamed to JSON field `\"screenshotIdentity\"` (camelCase). \
         Automation clients key on this exact JSON path; a rename would \
         invalidate every fixture without a compile-time signal."
    );
    assert!(
        QUERY_OPS_VARIANTS.contains("skip_serializing_if = \"Option::is_none\""),
        "src/protocol/message/variants/query_ops.rs `StateResult.screenshot_identity` \
         must use `skip_serializing_if = \"Option::is_none\"` so the key is \
         ABSENT when no capture has occurred, not present as `null`. A \
         `null`-valued key widens the protocol surface — clients would \
         have to handle three states (undefined/null/string) instead of \
         two (undefined/string)."
    );
}

#[test]
fn state_result_constructor_accepts_screenshot_identity_as_trailing_param() {
    assert!(
        QUERY_OPS_CONSTRUCTORS.contains("screenshot_identity: Option<String>,"),
        "src/protocol/message/constructors/query_ops.rs `Message::state_result` \
         must accept `screenshot_identity: Option<String>` as a distinct \
         parameter. Merging it into an existing field would hide identity \
         information from every automation caller."
    );
    assert!(
        QUERY_OPS_CONSTRUCTORS.contains("            screenshot_identity,"),
        "src/protocol/message/constructors/query_ops.rs `Message::state_result` \
         must forward the `screenshot_identity` parameter into the \
         `Message::StateResult` struct literal. A regression that drops \
         this line would silently omit the field for every automation query."
    );
}

#[test]
fn screenshot_files_exposes_identity_accessor_and_capture_paths_record_it() {
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("pub fn current_screenshot_identity()"),
        "src/ai/harness/screenshot_files.rs must expose a public \
         `current_screenshot_identity()` accessor — the single read \
         choke-point the state-snapshot wires into. Reading the static \
         directly (without the accessor) would couple the prompt handler \
         to the Mutex/OnceLock wrapping and let a refactor of the storage \
         shape silently break the snapshot."
    );
    assert!(
        SCREENSHOT_FILES_SOURCE.contains("pub fn record_last_screenshot_identity("),
        "src/ai/harness/screenshot_files.rs must expose a public \
         `record_last_screenshot_identity()` setter so the capture paths \
         can register the bare filename after a successful write. Any \
         alternative mechanism (direct static access, thread-local cache) \
         would bypass the contract."
    );
    let focused_write_idx = SCREENSHOT_FILES_SOURCE
        .find("\"tab_ai_screenshot_file_written\",")
        .expect("focused-window capture path trace event must remain");
    let after_focused = &SCREENSHOT_FILES_SOURCE[focused_write_idx..];
    let focused_end = after_focused
        .find("Ok(Some(TabAiScreenshotFile {")
        .expect("focused-window Ok(Some(...)) return must remain");
    let focused_block = &after_focused[..focused_end];
    assert!(
        focused_block.contains("record_last_screenshot_identity(filename.clone())"),
        "The focused-window capture path MUST call \
         `record_last_screenshot_identity(filename.clone())` between the \
         `tab_ai_screenshot_file_written` trace and the `Ok(Some(...))` \
         return. Recording elsewhere (e.g. only after cleanup) would \
         leave the identity stale if cleanup failed."
    );

    let screen_write_idx = SCREENSHOT_FILES_SOURCE
        .find("\"tab_ai_screen_screenshot_file_written\",")
        .expect("full-screen capture path trace event must remain");
    let after_screen = &SCREENSHOT_FILES_SOURCE[screen_write_idx..];
    let screen_end = after_screen
        .find("Ok(Some(TabAiScreenshotFile {")
        .expect("full-screen Ok(Some(...)) return must remain");
    let screen_block = &after_screen[..screen_end];
    assert!(
        screen_block.contains("record_last_screenshot_identity(filename.clone())"),
        "The full-screen capture path MUST call \
         `record_last_screenshot_identity(filename.clone())` between the \
         `tab_ai_screen_screenshot_file_written` trace and the \
         `Ok(Some(...))` return. Both capture paths share the same \
         identity generator — omitting the call in either path would \
         leave half the captures invisible to automation."
    );
}

#[test]
fn prompt_handler_main_state_snapshot_reads_through_screenshot_identity_accessor() {
    assert!(
        PROMPT_HANDLER_SOURCE
            .contains("crate::ai::harness::screenshot_files::current_screenshot_identity()"),
        "src/prompt_handler/mod.rs main `Message::state_result(...)` \
         construction must pass \
         `crate::ai::harness::screenshot_files::current_screenshot_identity()` \
         as the `screenshot_identity` argument. Passing `None` here \
         (the secondary-window / target-error paths correctly pass \
         `None` since those don't reflect main's capture state) would \
         silently erase the field for every `getState` on main."
    );
}
