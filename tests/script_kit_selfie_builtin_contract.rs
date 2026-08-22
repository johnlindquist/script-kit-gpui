const BUILTINS: &str = include_str!("../src/builtins/mod.rs");
const TRIGGER_REGISTRY: &str = include_str!("../src/builtins/trigger_registry.rs");
const TRIGGER_RESOLVE: &str = include_str!("../src/builtins/trigger_resolve.rs");
const ROUTES: &str = include_str!("../src/app_impl/routes.rs");
const EXECUTION: &str = include_str!("../src/app_execute/builtin_execution.rs");
const HOTKEYS: &str = include_str!("../src/hotkeys/mod.rs");
const SELFIE_CAPTURE: &str = include_str!("../src/platform/selfie_capture.rs");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source start: {start}"));
    let tail = &source[start_idx..];
    let end_idx = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing source end after {start}: {end}"));
    &tail[..end_idx]
}

#[test]
fn script_kit_selfie_builtin_is_registered_and_routable() {
    for required in [
        "ScriptKitSelfie",
        "\"builtin/script-kit-selfie\"",
        "\"script-kit-selfie\"",
        "\"scriptkitselfie\"",
        "\"selfie\"",
        "\"screenshot-selfie\"",
    ] {
        assert!(
            TRIGGER_REGISTRY.contains(required),
            "trigger registry should include Script Kit Selfie field: {required}"
        );
    }
    assert!(
        TRIGGER_RESOLVE.contains("TriggerBuiltin::ScriptKitSelfie => \"ScriptKitSelfie\""),
        "trigger resolver should render ScriptKitSelfie golden outcomes"
    );
    assert!(
        ROUTES.contains(
            "TriggerBuiltin::ScriptKitSelfie => AppRoute::ExecuteBuiltin(\"builtin/script-kit-selfie\")"
        ),
        "trigger route planner should execute the Script Kit Selfie builtin"
    );
}

#[test]
fn script_kit_selfie_builtin_text_matches_capture_receipt_behavior() {
    for required in [
        "UtilityCommandType::ScriptKitSelfie",
        "\"Script Kit Selfie\"",
        "\"Capture Script Kit with the current desktop background and save a receipt\"",
        "UtilityCommandType::ScriptKitSelfie => \"Capture Selfie\"",
        "UtilityCommandType::ScriptKitSelfie => \"Selfie\"",
    ] {
        assert!(
            BUILTINS.contains(required),
            "builtin command text should describe Script Kit Selfie behavior: {required}"
        );
    }
    assert!(
        SELFIE_CAPTURE.contains("pub fn capture_script_kit_selfie(state: &str)")
            && SELFIE_CAPTURE.contains("capture_method: format!(")
            && SELFIE_CAPTURE.contains("\"xcap.monitor.capture_region.composited_desktop.{}\""),
        "Script Kit Selfie platform implementation should capture the composited desktop region"
    );
}

#[test]
fn script_kit_selfie_execution_uses_named_action_copy() {
    let execute_body = source_between(
        EXECUTION,
        "fn execute_utility_selfie_builtin(",
        "    fn execute_utility_turn_this_into_command_builtin(",
    );

    for required in [
        "enum UtilitySelfieBuiltinAction",
        "ScriptKitSelfie => Some(Self::Selfie)",
        "fn starting_hud(self, state: &str) -> String",
        "fn saved_hud(self, receipt: &crate::platform::ScriptKitSelfieReceipt) -> String",
        "fn failure_message(self, error: &dyn std::fmt::Display) -> String",
        "execute_utility_selfie_builtin",
        "crate::platform::capture_script_kit_selfie(&state)",
    ] {
        assert!(
            EXECUTION.contains(required),
            "Script Kit Selfie execution should use named action behavior: {required}"
        );
    }
    assert!(
        !execute_body.contains("format!(\"Selfie saved:")
            && !execute_body.contains("format!(\"Script Kit Selfie failed:"),
        "Script Kit Selfie execution copy should stay on UtilitySelfieBuiltinAction"
    );
}

#[test]
fn script_kit_selfie_has_default_hotkey() {
    assert!(
        HOTKEYS.contains("SCRIPT_KIT_SELFIE_COMMAND_ID")
            && HOTKEYS.contains("\"builtin/script-kit-selfie\"")
            && HOTKEYS.contains("\"cmd+alt+1\"")
            && HOTKEYS.contains("\"Script Kit Selfie\""),
        "Script Kit Selfie should register its default cmd+alt+1 launcher shortcut"
    );
}
