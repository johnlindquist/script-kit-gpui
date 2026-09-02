//! Source-level contract for `getState.surfaceContract`.
//!
//! The generated surface matrix is useful to agents only if runtime receipts
//! expose the same contract identity. `stateResult.surfaceContract` is the
//! main-window bridge from live state to `SurfaceKind::surface_contract()`.

const QUERY_OPS_VARIANTS: &str = include_str!("../src/protocol/message/variants/query_ops.rs");
const QUERY_OPS_CONSTRUCTORS: &str =
    include_str!("../src/protocol/message/constructors/query_ops.rs");
const PROTOCOL_TYPES: &str = include_str!("../src/protocol/types/automation_surface.rs");
const PROMPT_HANDLER_SOURCE: &str = include_str!("../src/prompt_handler/mod.rs");
const KIT_SDK_SOURCE: &str = include_str!("../scripts/kit-sdk.ts");

fn source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end: {end_pat}"));
    &source[start..start + end_rel]
}

#[test]
fn state_result_declares_surface_contract_snapshot_field() {
    assert!(
        QUERY_OPS_VARIANTS
            .contains("surface_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>"),
        "StateResult must expose an optional LauncherSurfaceContractSnapshot"
    );
    assert!(
        QUERY_OPS_VARIANTS.contains("rename = \"surfaceContract\"")
            && QUERY_OPS_VARIANTS.contains("skip_serializing_if = \"Option::is_none\""),
        "surfaceContract must be camelCase and omitted when not available"
    );
    assert!(
        QUERY_OPS_CONSTRUCTORS
            .contains("surface_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>")
            && QUERY_OPS_CONSTRUCTORS.contains("            surface_contract,"),
        "Message::state_result must accept and forward the surface contract snapshot"
    );
    assert!(
        QUERY_OPS_VARIANTS.contains(
            "active_popup_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>"
        ) && QUERY_OPS_VARIANTS.contains("rename = \"activePopupContract\"")
            && QUERY_OPS_CONSTRUCTORS.contains(
                "active_popup_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>"
            )
            && QUERY_OPS_CONSTRUCTORS.contains("            active_popup_contract,"),
        "StateResult must expose and forward an optional activePopupContract snapshot for overlays"
    );
}

#[test]
fn surface_contract_snapshot_schema_contains_contract_matrix_fields() {
    for field in [
        "pub schema_version: u32",
        "pub surface_kind: String",
        "pub family: String",
        "pub input_ownership: String",
        "pub preview_role: String",
        "pub focus_policy: String",
        "pub keyboard_policy: String",
        "pub actions_policy: String",
        "pub proof_policy: String",
        "pub visual_policy: String",
        "pub automation_semantic_surface: String",
        "pub native_footer_surface: Option<String>",
    ] {
        assert!(
            PROTOCOL_TYPES.contains(field),
            "LauncherSurfaceContractSnapshot must expose `{field}`"
        );
    }
}

#[test]
fn prompt_handler_builds_snapshot_from_active_app_view_contract() {
    let snapshot = source_between(
        PROMPT_HANDLER_SOURCE,
        "fn current_surface_contract_snapshot(",
        "\n    /// Get the active popup surface contract",
    );
    assert!(
        snapshot.contains("let contract = self.current_view.surface_contract();")
            && snapshot
                .contains("surface_kind: format!(\"{:?}\", self.current_view.surface_kind())")
            && snapshot.contains("focus_policy: format!(\"{:?}\", contract.focus_policy)")
            && snapshot.contains("keyboard_policy: format!(\"{:?}\", contract.keyboard_policy)")
            && snapshot.contains("actions_policy: format!(\"{:?}\", contract.actions_policy)")
            && snapshot.contains("proof_policy: format!(\"{:?}\", contract.proof_policy)")
            && snapshot.contains("visual_policy: format!(\"{:?}\", contract.visual_policy)")
            && snapshot.contains(
                "automation_semantic_surface: contract.automation_semantic_surface.to_string()"
            )
            && snapshot.contains(".native_footer_surface()")
            && snapshot.contains(".map(str::to_string)"),
        "getState surfaceContract must be derived from the active AppView surface contract"
    );
}

#[test]
fn prompt_handler_exposes_actions_popup_overlay_contract() {
    assert!(
        PROMPT_HANDLER_SOURCE.contains("fn active_popup_contract_snapshot(")
            && PROMPT_HANDLER_SOURCE
                .contains("if !(self.show_actions_popup || self.actions_dialog.is_some())")
            && PROMPT_HANDLER_SOURCE
                .contains("let contract = AppView::ActionsDialog.surface_contract();")
            && PROMPT_HANDLER_SOURCE.contains("surface_kind: \"ActionsDialog\".to_string()")
            && PROMPT_HANDLER_SOURCE.contains("self.active_popup_contract_snapshot()"),
        "main-window getState must expose activePopupContract for attached ActionsDialog overlays"
    );
}

#[test]
fn main_get_state_includes_surface_contract_but_target_diagnostics_do_not() {
    let get_state = source_between(
        PROMPT_HANDLER_SOURCE,
        "PromptMessage::GetState {",
        "// Collect current UI state",
    );
    assert!(
        PROMPT_HANDLER_SOURCE
            .contains("Some(self.current_surface_contract_snapshot(&main_target))"),
        "main-window getState must include a surface contract snapshot"
    );
    assert!(
        get_state.contains("\"unsupported\".to_string()")
            && get_state.contains("target_unsupported:{:?}")
            && get_state.contains("\"target_resolution_failed\".to_string()")
            && get_state.contains("target_error:{}"),
        "secondary-window diagnostics should keep the unsupported/target_resolution_failed paths"
    );
    for field in [
        "surface_kind: None,",
        "app_view_variant: None,",
        "native_footer_surface: None,",
        "target_generation: None,",
        "surface_generation: None,",
        "data_generation: None,",
        "title: None,",
        "resolved_bounds: None,",
    ] {
        assert!(
            get_state.contains(field),
            "secondary-window diagnostics should omit surfaceContract because they are not main AppView receipts (missing `{field}`)"
        );
    }
}

#[test]
fn kit_sdk_prompt_state_exposes_surface_contract_snapshot() {
    assert!(
        KIT_SDK_SOURCE.contains("export interface LauncherSurfaceContractSnapshot")
            && KIT_SDK_SOURCE.contains("surfaceContract?: LauncherSurfaceContractSnapshot;")
            && KIT_SDK_SOURCE.contains("activePopupContract?: LauncherSurfaceContractSnapshot;")
            && KIT_SDK_SOURCE.contains("activeFooter?: ActiveFooterSnapshot;")
            && KIT_SDK_SOURCE.contains("nativeFooterSurface?: string | null;")
            && KIT_SDK_SOURCE.contains("surfaceContract: state.surfaceContract,"),
        "Kit SDK getState typing must expose and forward stateResult.surfaceContract"
    );
}
