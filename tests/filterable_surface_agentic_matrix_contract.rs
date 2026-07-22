//! Source-level contract for the AURP-06 filterable surface proof matrix.
//!
//! The runtime matrix is the repeatable agent-facing version of the manual
//! `getState`/`getElements` proof: migrated filterable surfaces must declare
//! their entry command, prompt type, SurfaceKind, list id, and filter text in one place.

const MATRIX: &str = include_str!("../scripts/agentic/filterable-surface-matrix.ts");

fn compact(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn function_body<'a>(source: &'a str, name: &str, next_name: &str) -> &'a str {
    let start_pat = format!("function {name}(");
    let export_start_pat = format!("export function {name}(");
    let start = source
        .find(&start_pat)
        .or_else(|| source.find(&export_start_pat))
        .unwrap_or_else(|| panic!("missing function start: {start_pat}"));
    let end_pat = format!("\nfunction {next_name}(");
    let async_end_pat = format!("\nasync function {next_name}(");
    let export_end_pat = format!("\nexport function {next_name}(");
    let export_async_end_pat = format!("\nexport async function {next_name}(");
    let end_rel = source[start..]
        .find(&end_pat)
        .or_else(|| source[start..].find(&async_end_pat))
        .or_else(|| source[start..].find(&export_end_pat))
        .or_else(|| source[start..].find(&export_async_end_pat))
        .unwrap_or_else(|| panic!("missing next function start: {end_pat}"));
    &source[start..start + end_rel]
}

#[test]
fn matrix_declares_current_app_commands_visible_row_case() {
    assert!(
        MATRIX.contains("export const FILTERABLE_SURFACE_MATRIX"),
        "The matrix must be data-first and importable."
    );
    assert!(
        MATRIX.contains("id: \"current-app-commands-visible-rows\""),
        "AURP-06 starts with the migrated Current App Commands surface."
    );
    assert!(
        MATRIX.contains("promptType: \"currentAppCommands\""),
        "The matrix must pin the state promptType expected after entry."
    );
    assert!(
        MATRIX.contains("surfaceKind: \"CurrentAppCommands\""),
        "The matrix must pin the state surfaceContract.surfaceKind expected after entry."
    );
    assert!(
        MATRIX.contains("listSemanticId: \"list:menu-commands\""),
        "The matrix must identify the getElements list whose item count must match state."
    );
    assert!(
        MATRIX
            .contains("entryCommand: { type: \"triggerBuiltin\", name: \"current-app-commands\" }"),
        "The matrix must use the real runtime entry path."
    );
    assert!(
        MATRIX.contains("filterText: \"workspace\""),
        "The matrix must include a narrowing filter, not only the empty-filter case."
    );
    assert!(
        MATRIX.contains("expectedElementChromeCount: 2"),
        "Input plus list chrome must be explicit so totalCount parity is testable."
    );
    assert!(
        MATRIX.contains("viewName: \"current-app-commands\"")
            && MATRIX.contains("imageLibraryName: \"current-app-commands.png\""),
        "The matrix must declare stable image-library names for navigator captures."
    );
    assert!(
        MATRIX.contains("target: MAIN_TARGET")
            && compact(MATRIX).contains(
                "constMAIN_TARGET:MatrixAutomationTarget={type:\"kind\",kind:\"main\",index:0,};"
            ),
        "Stable matrix surfaces must declare exact main-window automation targets."
    );
    assert!(
        MATRIX.contains("safeInteractions: SAFE_NON_SUBMITTING_INTERACTIONS")
            && MATRIX.contains("selectFirstVisibleChoice: false")
            && MATRIX.contains("submit: false"),
        "Navigator-safe interactions must be explicit, filter-only for currently supported matrix surfaces, and non-submitting."
    );
}

#[test]
fn matrix_declares_expanded_filterable_surface_cases() {
    assert!(
        MATRIX.contains("id: \"clipboard-history-visible-rows\""),
        "AURP-11 must add Clipboard History to the state-first proof matrix."
    );
    assert!(
        MATRIX.contains("promptType: \"clipboardHistory\"")
            && MATRIX.contains("listSemanticId: \"list:clipboard-history\"")
            && MATRIX.contains("entryCommand: { type: \"triggerBuiltin\", name: \"clipboard-history\" }")
            && MATRIX.contains("filterText: \"__aurp11_no_clipboard_match__\""),
        "Clipboard History must declare promptType, list id, real entry path, and stable zero-match filter."
    );
    assert!(
        MATRIX.contains("id: \"emoji-picker-visible-rows\""),
        "AURP-11 must add Emoji Picker to the state-first proof matrix."
    );
    assert!(
        MATRIX.contains("promptType: \"emojiPicker\"")
            && MATRIX.contains("listSemanticId: \"list:emoji-results\"")
            && MATRIX.contains("entryCommand: { type: \"triggerBuiltin\", name: \"emoji\" }")
            && MATRIX.contains("filterText: \"heart\""),
        "Emoji Picker must declare promptType, list id, real entry path, and deterministic narrowing filter."
    );
}

#[test]
fn matrix_declares_stable_sibling_filterable_surface_cases() {
    for (id, prompt_type, surface_kind, list_id, entry_name, filter_text) in [
        (
            "app-launcher-visible-rows",
            "appLauncher",
            "AppLauncher",
            "list:apps",
            "apps",
            "__aurp16_no_app_match__",
        ),
        (
            "window-switcher-visible-rows",
            "windowSwitcher",
            "WindowSwitcher",
            "list:windows",
            "window-switcher",
            "__aurp16_no_window_match__",
        ),
        (
            "browser-tabs-visible-rows",
            "browserTabs",
            "BrowserTabs",
            "list:browser-tabs",
            "browser-tabs",
            "__aurp16_no_browser_tab_match__",
        ),
        (
            "process-manager-visible-rows",
            "processManager",
            "ProcessManager",
            "list:processes",
            "process-manager",
            "__aurp16_no_process_match__",
        ),
    ] {
        assert!(
            MATRIX.contains(&format!("id: \"{id}\""))
                && MATRIX.contains(&format!("promptType: \"{prompt_type}\""))
                && MATRIX.contains(&format!("surfaceKind: \"{surface_kind}\""))
                && MATRIX.contains(&format!("listSemanticId: \"{list_id}\""))
                && MATRIX.contains(&format!(
                    "entryCommand: {{ type: \"triggerBuiltin\", name: \"{entry_name}\" }}"
                ))
                && MATRIX.contains(&format!("filterText: \"{filter_text}\"")),
            "AURP-16 case {id} must declare promptType, list id, real entry path, and stable filter text."
        );
    }
}

#[test]
fn matrix_declares_generic_filterable_variant_cases() {
    for (id, prompt_type, list_id, builtin_id, filter_text) in [
        (
            "favorites-visible-rows",
            "favorites",
            "list:favorites",
            "builtin/favorites",
            "__liquid_no_favorite_match__",
        ),
        (
            "search-ai-presets-visible-rows",
            "searchAiPresets",
            "list:ai-presets",
            "builtin/search-ai-presets",
            "coder",
        ),
    ] {
        assert!(
            MATRIX.contains(&format!("id: \"{id}\""))
                && MATRIX.contains(&format!("promptType: \"{prompt_type}\""))
                && MATRIX.contains("surfaceKind: \"GenericFilterableList\"")
                && MATRIX.contains(&format!("listSemanticId: \"{list_id}\""))
                && compact(MATRIX).contains("entryCommand:{type:\"triggerBuiltin\",")
                && MATRIX.contains(&format!("builtinId: \"{builtin_id}\""))
                && MATRIX.contains(&format!("filterText: \"{filter_text}\"")),
            "GenericFilterable case {id} must declare promptType, list id, triggerBuiltin entry, and stable filter text."
        );
    }
}

#[test]
fn matrix_runner_checks_state_and_elements_count_parity() {
    let body = function_body(MATRIX, "observeCounts", "runEntry");
    assert!(
        body.contains("objectField(state, \"surfaceContract\")")
            && body.contains("surfaceKind !== entry.surfaceKind")
            && body.contains("automationSemanticSurface !== entry.surface"),
        "The matrix must compare live getState.surfaceContract against the declared surface contract identity."
    );
    assert!(
        body.contains("visibleChoiceCount > choiceCount"),
        "The matrix must preserve the state subset invariant."
    );
    assert!(
        body.contains("listCount !== visibleChoiceCount"),
        "The matrix must compare getElements list count against state visibleChoiceCount."
    );
    assert!(
        compact(body)
            .contains("elementsTotalCount<visibleChoiceCount+entry.expectedElementChromeCount"),
        "The matrix must verify totalCount contains at least visible rows plus required chrome."
    );
}

#[test]
fn matrix_runner_uses_parse_receipts_state_accepted_filters_and_typed_rpcs() {
    assert!(
        MATRIX.contains("\"--await-parse\""),
        "Surface-entry commands must still wait for parse receipts when the runtime emits them."
    );
    assert!(
        MATRIX.contains("export async function getStateAndElements("),
        "State and element proof must be exported for the surface navigator."
    );
    assert!(
        MATRIX.contains("emptyElementsCommand"),
        "Empty elements proof must be an explicit matrix step."
    );
    assert!(
        MATRIX.contains("\"elementsResult\""),
        "Element proof must use typed elementsResult RPC."
    );
    assert!(
        MATRIX.contains("type: \"setFilter\",")
            && MATRIX.contains("text,")
            && MATRIX.contains("entry.filterText,")
            && MATRIX.contains("`${entry.id}-set-filter`")
            && MATRIX.contains("setFilterAndWaitForState("),
        "Filtered proof must drive the real setFilter path and require the requested filter text to appear in getState."
    );
    assert!(
        MATRIX.contains("entry,\n      \"\",")
            && MATRIX.contains("`${entry.id}-reset-filter`")
            && MATRIX.contains("stateInputText(state) === text"),
        "Each case must reset the active filter after entry and prove the reset through state so multi-case runs do not inherit the prior case's filter."
    );
}

#[test]
fn matrix_exports_reusable_surface_navigation_helpers() {
    for helper in [
        "export async function enterFilterableSurface(",
        "export async function waitForPromptType(",
        "export async function getStateAndElements(",
        "export function observeCounts(",
        "export async function sendAndAwaitParse(",
        "export async function sendCommand(",
        "export async function setFilterAndWaitForState(",
        "export async function rpc(",
    ] {
        assert!(
            MATRIX.contains(helper),
            "filterable matrix must export reusable helper: {helper}"
        );
    }
}

#[test]
fn matrix_get_state_and_elements_accepts_target_override() {
    assert!(
        MATRIX.contains("targetOverride") && MATRIX.contains("target: targetOverride"),
        "navigator must be able to promote kind targets to exact id targets without forking getState/getElements logic"
    );
}

#[test]
fn matrix_entries_declare_image_library_metadata() {
    for (view_name, file_name) in [
        ("current-app-commands", "current-app-commands.png"),
        ("clipboard-history", "clipboard-history.png"),
        ("emoji-picker", "emoji-picker.png"),
        ("app-launcher", "app-launcher.png"),
        ("window-switcher", "window-switcher.png"),
        ("browser-tabs", "browser-tabs.png"),
        ("process-manager", "process-manager.png"),
    ] {
        assert!(
            MATRIX.contains(&format!("viewName: \"{view_name}\""))
                && MATRIX.contains(&format!("imageLibraryName: \"{file_name}\"")),
            "matrix entry must declare viewName={view_name} and imageLibraryName={file_name}"
        );
    }
}

#[test]
fn matrix_runner_exposes_list_mode_for_agents() {
    assert!(
        MATRIX.contains("hasFlag(\"--list\")"),
        "Agents must be able to inspect the matrix without launching the app."
    );
    assert!(
        MATRIX.contains("matrix: FILTERABLE_SURFACE_MATRIX"),
        "--list must return the same data the runner executes."
    );
}
