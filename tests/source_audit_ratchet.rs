//! Ratchet for source-audit test brittleness (see CLAUDE.md "Source Audit Test Policy").
//!
//! Occurrence-count assertions over formatted source text (`source.matches(...).count()`)
//! break on rustfmt wrapping, renames, and legitimate additions, training agents to
//! appease the test instead of honoring the invariant. New source audits must enumerate
//! expected sites structurally (e.g. via a `function_body` helper) instead of counting.
//!
//! The allowlist below holds the audited survivors — counts that are genuinely
//! structural (singleton checks on stable signatures/tokens, non-source inputs like
//! markdown or runtime strings). It may only shrink. To fix a failure here, rewrite the
//! new count assertion structurally — do NOT add to the allowlist without recording the
//! justification next to the entry.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Files allowed to keep `.matches(...).count()` / `.match_indices(...).count()`
/// assertions, with the reviewed justification (2026-06-09 audit).
const ALLOWLIST: &[(&str, &str)] = &[
    (
        "tests/actions.rs",
        "violation detector: count>0 reports diagnostics, equivalent to contains",
    ),
    (
        "tests/app_launcher_visible_rows_contract.rs",
        "pre-policy survivor: floor over stable call token (render + keyboard paths)",
    ),
    (
        "tests/clipboard_history_preview_type_filters_contract.rs",
        "singleton on stable call token `entry_text_matches(entry,`",
    ),
    (
        "tests/panel_invariants_contract.rs",
        "pre-policy survivor: exact count over stable `PANEL_CONFIGURED.store(true` token",
    ),
    (
        "tests/panel_invariants_soft_is_key_window_contract.rs",
        "singleton on stable single-line token `r.record_soft(`",
    ),
    (
        "tests/process_manager_visible_rows_contract.rs",
        "pre-policy survivor: floor over stable call token (render + keyboard paths)",
    ),
    (
        "tests/actions_dialog_batch_setinput_resize_parity_contract.rs",
        "exactly-once on a single-line comment anchor; comments survive rustfmt",
    ),
    (
        "tests/actions_popup_parent_preserves_semantic_surface_contract.rs",
        "singleton on function signature",
    ),
    (
        "tests/agent_chat_conversation_export_single_path_contract.rs",
        "singleton on `pub(crate) fn export_conversation` signature",
    ),
    (
        "tests/detached_agent_chat_concurrent_close_safety_contract.rs",
        "singleton on stable single-line token (close-path ownership)",
    ),
    (
        "tests/features_doc_contract.rs",
        "counts markdown headings in FEATURES.md; matrix size is the invariant, not rustfmt-sensitive",
    ),
    (
        "tests/menu_syntax_run12_attacker_pass4_target_examples_and_gate_hud.rs",
        "counts in fixture/example strings, not formatted source",
    ),
    (
        "tests/no_legacy_design_variant.rs",
        "violation detector over remaining legacy tokens; count>0 reports diagnostics",
    ),
    (
        "tests/smoke_main_menu.rs",
        "counts separators in a runtime-built frecency key string, not source text",
    ),
    (
        "tests/source_audits/root_unified_ai_vault_contract.rs",
        "singleton on `search_cmux_vault(` token: defined exactly once, no in-file callers",
    ),
    (
        "tests/source_audits/theme_chooser_single_select_controls.rs",
        "singleton on stable signature token",
    ),
    (
        "tests/stdin_simulatekey_printable_char_noop_contract.rs",
        "statement-count ceiling (<=1) that fails safe under reformatting",
    ),
    (
        "tests/trigger_builtin_filterable_route_state_machine_contract.rs",
        "singleton-assignment invariant on stable token `self.current_view =`",
    ),
    (
        "tests/automation_batch_target_capabilities_contract.rs",
        "match_indices singleton on stable token (pre-policy survivor, reviewed)",
    ),
    // Pre-policy survivors (2026-06-09): counts over stable single-line tokens
    // (call sites / qualified paths), not formatted multi-line code. Convert each to
    // enumerated per-site assertions on its next false failure, then remove the entry.
    (
        "tests/agent_chat_existing_chat_mutation_contract.rs",
        "pre-policy survivor: floor over stable JS error-bridge token",
    ),
    (
        "tests/agent_chat_markdown_blocked_reason_contract.rs",
        "pre-policy survivor: floor over stable call token",
    ),
    (
        "tests/agent_chat_surface_state_contract.rs",
        "pre-policy survivor: counts over stable qualified event paths",
    ),
    (
        "tests/current_app_commands_visible_rows_contract.rs",
        "pre-policy survivor: exact counts over stable call tokens",
    ),
    (
        "tests/design_picker_actions_contract.rs",
        "pre-policy survivor: count over stable enum-variant token",
    ),
    (
        "tests/detached_agent_chat_close_cleanup_contract.rs",
        "pre-policy survivor: exact count over stable call token",
    ),
    (
        "tests/dictation_background_sync_contract.rs",
        "pre-policy survivor: exact count over stable qualified call token",
    ),
    (
        "tests/filter_input_preflight_contract.rs",
        "pre-policy survivor: singleton inside function_body scope",
    ),
    (
        "tests/kit_store_text_state_contract.rs",
        "pre-policy survivor: floors over stable method tokens",
    ),
    (
        "tests/liquid_glass_guideline_assertions_contract.rs",
        "pre-policy survivor: floor over stable token within scoped branch",
    ),
    (
        "tests/main_automation_surface_rekey_owner_contract.rs",
        "pre-policy survivor: exact counts over stable call tokens",
    ),
    (
        "tests/main_window_preflight.rs",
        "pre-policy survivor: floor over stable method token",
    ),
    (
        "tests/screenshot_identity_threading_contract.rs",
        "pre-policy survivor: floors over stable call tokens",
    ),
    (
        "tests/set_filter_routes_to_active_subview_contract.rs",
        "pre-policy survivor: match_indices singleton on stable call token",
    ),
];

/// True when, in whitespace-collapsed source, a `.matches(` / `.match_indices(` call is
/// immediately followed by `.count()` in the same chain.
fn has_count_over_matches(source: &str) -> bool {
    let collapsed: String = source.split_whitespace().collect::<Vec<_>>().join("");
    for needle in [".matches(", ".match_indices("] {
        let mut from = 0;
        while let Some(pos) = collapsed[from..].find(needle) {
            let open = from + pos + needle.len() - 1;
            let bytes = collapsed.as_bytes();
            let mut depth = 0usize;
            let mut in_string = false;
            let mut idx = open;
            // Parens inside string-literal needles (e.g. `.matches("foo(")`) must not
            // affect nesting, so track double-quote state and skip escapes.
            while idx < bytes.len() {
                match bytes[idx] {
                    b'\\' if in_string => idx += 1,
                    b'"' => in_string = !in_string,
                    b'(' if !in_string => depth += 1,
                    b')' if !in_string => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                idx += 1;
            }
            if idx < bytes.len() && collapsed[idx + 1..].starts_with(".count()") {
                return true;
            }
            from = open + 1;
        }
    }
    false
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("read tests dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn count_assertions_over_source_text_only_shrink() {
    let mut files = Vec::new();
    rust_files(Path::new("tests"), &mut files);

    let allow: BTreeSet<&str> = ALLOWLIST.iter().map(|(f, _)| *f).collect();
    let mut offenders = Vec::new();
    let mut hits = BTreeSet::new();

    for path in files {
        let rel = path.to_string_lossy().replace('\\', "/");
        if rel == "tests/source_audit_ratchet.rs" {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read test source");
        if has_count_over_matches(&source) {
            hits.insert(rel.clone());
            if !allow.contains(rel.as_str()) {
                offenders.push(rel);
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "new occurrence-count assertions over source text in: {offenders:?}\n\
         Enumerate the expected sites structurally instead (see CLAUDE.md \"Source Audit \
         Test Policy\"); do not extend the allowlist without a reviewed justification."
    );

    let stale: Vec<&str> = allow
        .iter()
        .filter(|f| !hits.contains(**f))
        .copied()
        .collect();
    assert!(
        stale.is_empty(),
        "allowlist entries no longer needed (remove them so the ratchet only shrinks): {stale:?}"
    );
}
