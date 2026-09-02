//! Source-level contract for the generated surface contract matrix.
//!
//! The JSON artifact is for agents; the Rust registry is the source of truth.
//! This test keeps the artifact tied to `AppView::surface_kind()` and
//! `SurfaceKind::surface_contract()` instead of becoming another hand-written
//! map that can drift.

use std::process::Command;

const APP_VIEW_STATE: &str = include_str!("../src/main_sections/app_view_state.rs");
const GENERATOR: &str = include_str!("../scripts/generate-surface-contracts.ts");
const MATRIX_JSON: &str = include_str!("../docs/ai/contracts/surface-contracts.json");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing start marker: {start}"));
    let after_start = &source[start_index..];
    let end_index = after_start
        .find(end)
        .unwrap_or_else(|| panic!("missing end marker after {start}: {end}"));
    &after_start[..end_index]
}

fn surface_kind_names() -> Vec<String> {
    let enum_body = source_between(
        APP_VIEW_STATE,
        "pub(crate) enum SurfaceKind {",
        "}\n\n/// First-pass vocabulary",
    );
    enum_body
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(','))
        .filter(|line| !line.starts_with("#["))
        .map(|line| line.trim_end_matches(',').to_string())
        .filter(|line| line.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .collect()
}

fn matrix_entries() -> Vec<serde_json::Value> {
    let parsed: serde_json::Value =
        serde_json::from_str(MATRIX_JSON).expect("surface contract matrix must be valid JSON");
    assert_eq!(parsed["schemaVersion"], 2);
    assert_eq!(
        parsed["generatedFrom"],
        "src/main_sections/app_view_state.rs"
    );
    assert_eq!(
        parsed["registry"],
        "AppView::surface_kind -> SurfaceKind::surface_contract"
    );
    parsed["entries"]
        .as_array()
        .expect("matrix entries must be an array")
        .clone()
}

#[test]
fn generated_surface_contract_matrix_is_not_stale() {
    let output = Command::new("bun")
        .arg("scripts/generate-surface-contracts.ts")
        .arg("--check")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("bun must run surface contract generator");
    assert!(
        output.status.success(),
        "surface contract matrix is stale:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn matrix_contains_every_surface_kind_once() {
    let mut expected = surface_kind_names();
    expected.sort();

    let mut actual: Vec<String> = matrix_entries()
        .into_iter()
        .map(|entry| {
            entry["surfaceKind"]
                .as_str()
                .expect("entry.surfaceKind must be a string")
                .to_string()
        })
        .collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "agent-readable matrix must contain every SurfaceKind exactly once"
    );
}

#[test]
fn matrix_entries_expose_behavior_fields_agents_need() {
    for entry in matrix_entries() {
        let surface_kind = entry["surfaceKind"]
            .as_str()
            .expect("surfaceKind must be a string");
        assert!(
            entry["appViewVariants"]
                .as_array()
                .expect("appViewVariants must be an array")
                .iter()
                .all(|variant| variant.as_str().is_some())
                && !entry["appViewVariants"].as_array().unwrap().is_empty(),
            "{surface_kind} must list the AppView variants that map to it"
        );
        assert!(
            entry["appViewFooters"]
                .as_array()
                .expect("appViewFooters must be an array")
                .iter()
                .all(|footer| footer["variant"].as_str().is_some()
                    && (footer["nativeFooterSurface"].as_str().is_some()
                        || footer["nativeFooterSurface"].is_null())),
            "{surface_kind} must expose each AppView variant's native footer surface"
        );
        for path in [
            ["vocabulary", "family"],
            ["vocabulary", "inputOwnership"],
            ["vocabulary", "previewRole"],
            ["dismissPolicy", "policy"],
            ["dismissPolicy", "windowBlur"],
            ["dismissPolicy", "backdropClick"],
            ["dismissPolicy", "escape"],
            ["dismissPolicy", "cmdW"],
        ] {
            assert!(
                entry[path[0]][path[1]].as_str().is_some(),
                "{surface_kind} must expose {}.{} as a string",
                path[0],
                path[1]
            );
        }
        assert!(
            entry["automationSemanticSurface"].as_str().is_some(),
            "{surface_kind} must expose automationSemanticSurface"
        );
        assert!(
            entry["focusPolicy"].as_str().is_some(),
            "{surface_kind} must expose focusPolicy"
        );
        assert!(
            entry["keyboardPolicy"].as_str().is_some(),
            "{surface_kind} must expose keyboardPolicy"
        );
        assert!(
            entry["actionsPolicy"].as_str().is_some(),
            "{surface_kind} must expose actionsPolicy"
        );
        assert!(
            entry["proofPolicy"].as_str().is_some(),
            "{surface_kind} must expose proofPolicy"
        );
        assert!(
            entry["visualPolicy"].as_str().is_some(),
            "{surface_kind} must expose visualPolicy"
        );
    }
}

#[test]
fn matrix_exposes_known_native_footer_surfaces() {
    let entries = matrix_entries();
    for (variant, expected_footer) in [
        ("QuickTerminalView", Some("quick_terminal")),
        ("TermPrompt", None),
        ("MicroPrompt", None),
        ("ConfirmPrompt", Some("confirm_prompt")),
        ("BrowseKitsView", Some("kit_store_browse")),
        ("InstalledKitsView", Some("kit_store_installed")),
    ] {
        let footer = entries
            .iter()
            .flat_map(|entry| entry["appViewFooters"].as_array().unwrap())
            .find(|footer| footer["variant"] == variant)
            .unwrap_or_else(|| panic!("missing appViewFooters entry for {variant}"));
        match expected_footer {
            Some(expected) => assert_eq!(footer["nativeFooterSurface"], expected),
            None => assert!(
                footer["nativeFooterSurface"].is_null(),
                "{variant} must explicitly expose null nativeFooterSurface"
            ),
        }
    }
}

#[test]
fn generator_reads_the_typed_registry_not_a_parallel_map() {
    for expected in [
        "pub(crate) fn surface_kind(&self) -> SurfaceKind",
        "pub(crate) fn surface_contract(self) -> LauncherSurfaceContract",
        "pub(crate) enum SurfaceKind {",
        "SurfaceKind::",
        "AppView::",
        "--check",
        "--write",
    ] {
        assert!(
            GENERATOR.contains(expected),
            "generator must include source-backed contract marker `{expected}`"
        );
    }
}
