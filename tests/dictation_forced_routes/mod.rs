//! Contract tests for forced-route dictation builtins and overlay route badges.

const BUILTIN_EXECUTION_SOURCE: &str = include_str!("../../src/app_execute/builtin_execution.rs");
const BUILTINS_SOURCE: &str = include_str!("../../src/builtins/mod.rs");
const OVERLAY_SOURCE: &str = include_str!("../../src/dictation/window.rs");
const TYPES_SOURCE: &str = include_str!("../../src/dictation/types.rs");
const EXECUTION_SCRIPTS_SOURCE: &str = include_str!("../../src/app_impl/execution_scripts.rs");

fn production_source(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

#[test]
fn dictation_forced_route_variants_remain_internal() {
    let builtins_source = production_source(BUILTINS_SOURCE);
    let top_level_entries = builtins_source
        .split("fn hidden_builtin_entry")
        .next()
        .unwrap_or(builtins_source);

    assert!(builtins_source.contains("BuiltInFeature::DictationToFrontmostApp"));
    assert!(builtins_source.contains("BuiltInFeature::DictationToNotes"));
    assert!(
        !top_level_entries.contains("builtin/dictation-to-app"),
        "forced app dictation should no longer be advertised as a top-level launcher entry"
    );
    assert!(
        !top_level_entries.contains("builtin/dictation-to-notes"),
        "forced notes dictation should no longer be advertised as a top-level launcher entry"
    );
}

#[test]
fn forced_route_builtins_are_no_main_window() {
    assert!(EXECUTION_SCRIPTS_SOURCE.contains("builtin/dictation-to-app"));
    assert!(EXECUTION_SCRIPTS_SOURCE.contains("builtin/dictation-to-notes"));
}

#[test]
fn overlay_state_carries_target() {
    assert!(
        OVERLAY_SOURCE.contains("pub target: crate::dictation::DictationTarget")
            || OVERLAY_SOURCE.contains("pub target: DictationTarget")
    );
}

#[test]
fn dictation_target_exposes_overlay_labels() {
    assert!(TYPES_SOURCE.contains("pub fn overlay_label(self) -> &'static str"));
    assert!(TYPES_SOURCE.contains(r#""Script Kit""#));
    assert!(TYPES_SOURCE.contains(r#""Prompt""#));
    assert!(TYPES_SOURCE.contains(r#""Notes""#));
    assert!(TYPES_SOURCE.contains(r#""Agent""#));
    assert!(TYPES_SOURCE.contains(r#""App""#));
}

#[test]
fn overlay_renders_target_badge() {
    assert!(OVERLAY_SOURCE.contains("render_target_badge"));
    assert!(OVERLAY_SOURCE.contains("TARGET_BADGE_SLOT_WIDTH_PX"));
}

#[test]
fn overlay_target_badge_preserves_full_label_for_accessibility() {
    assert!(
        OVERLAY_SOURCE.contains("let target_label = target_badge_label(self.state.target);")
            && OVERLAY_SOURCE.contains("Tooltip::new(target_label.clone())"),
        "fixed-width dictation target badge must expose the full label even when the visible text is clipped"
    );
    assert!(
        OVERLAY_SOURCE.contains(".max_w(px(TARGET_BADGE_SLOT_WIDTH_PX - 18.0))")
            && OVERLAY_SOURCE.contains(".text_ellipsis()")
            && OVERLAY_SOURCE.contains(".whitespace_nowrap()"),
        "dictation target badge text must clip predictably inside the fixed overlay width"
    );
}

#[test]
fn hidden_dictation_to_app_route_uses_frontmost_app_target() {
    let hidden_start = BUILTINS_SOURCE
        .find(r#""builtin/dictation-to-app""#)
        .expect("hidden dictation-to-app route must exist");
    let hidden_body = &BUILTINS_SOURCE[hidden_start..];
    let next_entry = hidden_body[1..]
        .find(r#""builtin/dictation-to-notes""#)
        .unwrap_or(hidden_body.len() - 1);
    let hidden_body = &hidden_body[..next_entry + 1];

    assert!(hidden_body.contains("Start Dictation to App"));
    assert!(hidden_body.contains("BuiltInFeature::DictationToFrontmostApp"));
}

#[test]
fn dictation_to_notes_handler_forces_notes_target() {
    let handler_start = BUILTIN_EXECUTION_SOURCE
        .find("BuiltInFeature::DictationToNotes")
        .expect("DictationToNotes match arm must exist");
    let handler_body = &BUILTIN_EXECUTION_SOURCE[handler_start..];
    let next_arm = handler_body[1..]
        .find("builtins::BuiltInFeature::")
        .unwrap_or(handler_body.len());
    let handler_body = &handler_body[..next_arm];
    assert!(handler_body.contains("DictationBuiltinAction::Notes"));
    assert!(BUILTIN_EXECUTION_SOURCE
        .contains("Self::Notes => Some(crate::dictation::DictationTarget::NotesEditor)"));
    assert!(BUILTIN_EXECUTION_SOURCE.contains("Self::Notes => \"dictation_to_notes_toggle\""));
    assert!(BUILTIN_EXECUTION_SOURCE.contains("Starting forced-route dictation"));
}
