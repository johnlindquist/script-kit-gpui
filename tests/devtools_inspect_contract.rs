use std::fs;

const INSPECT: &str = include_str!("../scripts/devtools/inspect.ts");
const TARGETS: &str = include_str!("../scripts/devtools/targets.ts");
const SURFACE: &str = include_str!("../scripts/devtools/surface.ts");
const ELEMENTS: &str = include_str!("../scripts/devtools/elements.ts");
const LAYOUT: &str = include_str!("../scripts/devtools/layout.ts");
const SCROLL: &str = include_str!("../scripts/devtools/scroll.ts");
const FOCUS: &str = include_str!("../scripts/devtools/focus.ts");
const TEXT: &str = include_str!("../scripts/devtools/text.ts");
const KEYBOARD: &str = include_str!("../scripts/devtools/keyboard.ts");
const ACTIONS: &str = include_str!("../scripts/devtools/actions.ts");
const ACTIONS_DIALOG: &str = include_str!("../src/actions/dialog.rs");
const ACTIONS_WINDOW: &str = include_str!("../src/actions/window.rs");
const ACT: &str = include_str!("../scripts/devtools/act.ts");
const COMPARE: &str = include_str!("../scripts/devtools/compare.ts");
const EVENTS: &str = include_str!("../scripts/devtools/events.ts");
const NOTES: &str = include_str!("../scripts/devtools/notes.ts");
const DICTATION: &str = include_str!("../scripts/devtools/dictation.ts");
const SCHEMA: &str = include_str!("../scripts/devtools/schema.ts");
const AUTOMATION_INSPECT: &str = include_str!("../src/protocol/types/automation_inspect.rs");
const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");
const APP_VIEW_STATE: &str = include_str!("../src/main_sections/app_view_state.rs");
const DEVTOOLS_SKILL: &str = include_str!("../.agents/skills/script-kit-devtools/SKILL.md");
const DEVTOOLS_AUDIT: &str =
    include_str!("../.agents/skills/script-kit-devtools/references/devtools-coverage-audit.md");

#[test]
fn inspect_composes_existing_protocol_primitives() {
    for needle in [
        "listAutomationWindows",
        "inspectAutomationWindow",
        "getState",
        "getElements",
        "getLayoutInfo",
        "scripts/agentic/session.sh",
    ] {
        assert!(
            INSPECT.contains(needle),
            "devtools inspect must compose the existing protocol primitive {needle}"
        );
    }

    assert!(
        !INSPECT.contains("scripts/agentic/index.ts"),
        "devtools inspect must not route through the recipe catalog"
    );
}

#[test]
fn automation_inspect_snapshot_exposes_runtime_surface_identity() {
    for needle in [
        "pub surface_kind: Option<String>",
        "pub app_view_variant: Option<String>",
        "pub native_footer_surface: Option<String>",
    ] {
        assert!(
            AUTOMATION_INSPECT.contains(needle),
            "AutomationInspectSnapshot must expose runtime surface identity field: {needle}"
        );
    }

    for needle in [
        "fn app_view_variant(&self) -> &'static str",
        "AppView::ScriptList => \"ScriptList\"",
        "AppView::DivPrompt { .. } => \"DivPrompt\"",
        "AppView::AgentChatView { .. } => \"AgentChatView\"",
    ] {
        assert!(
            APP_VIEW_STATE.contains(needle),
            "AppView must expose stable variant identity for DevTools: {needle}"
        );
    }

    for needle in [
        "let surface_kind = (resolved.kind == protocol::AutomationWindowKind::Main)",
        "let app_view_variant = (resolved.kind == protocol::AutomationWindowKind::Main)",
        "let native_footer_surface = (resolved.kind == protocol::AutomationWindowKind::Main)",
    ] {
        assert!(
            PROMPT_HANDLER.contains(needle),
            "inspectAutomationWindow must populate runtime surface identity: {needle}"
        );
    }
}

#[test]
fn targets_cli_promotes_strict_target_identity_to_first_class_receipt() {
    for needle in [
        "script-kit-devtools.targets",
        "targets.list",
        "targets.inspect",
        "listAutomationWindows",
        "inspectAutomationWindow",
        "--target-id",
        "--target-kind",
        "--target-index",
        "--target-title",
        "--focused",
        "--main",
        "--surface",
        "--strict",
        "requestedTarget",
        "resolvedTarget",
        "stableTargetId",
        "strictTargetMatch",
        "snapshot.windowKind",
        "snapshot.semanticSurface",
        "blocked-by-target-ambiguity",
        "blocked-by-timeout",
        "screenshotIdentity",
    ] {
        assert!(
            TARGETS.contains(needle),
            "targets CLI must expose strict target identity field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Targets CLI"),
        "DevTools skill should route strict target identity through targets CLI"
    );
}

#[test]
fn targets_cli_normalizes_list_window_kind_to_inspect_identity() {
    for needle in [
        "function stableWindowKind",
        "value === \"actionsDialog\"",
        "\"ActionsDialog\"",
        "windowKind: stableWindowKind(window.kind ?? window.windowKind)",
    ] {
        assert!(
            TARGETS.contains(needle),
            "targets.list must report the same stable window kind spelling as targets.inspect: {needle}"
        );
    }
}

#[test]
fn surface_cli_combines_strict_target_identity_with_surface_contract() {
    for needle in [
        "script-kit-devtools.surface",
        "surface.inspect",
        "scripts/devtools/targets.ts",
        "docs/ai/contracts/surface-contracts.json",
        "--surface",
        "requestedSurfaceKind",
        "targetReceipt",
        "dismissPolicy",
        "getState",
        "stateResult",
        "activeFooterSurface",
        "surfaceContract",
        "activeFooter",
        "focusedSemanticId",
        "selectedSemanticId",
        "rowCountVisible",
        "visibleChoiceCount",
        "choiceCount",
        "blocked-by-unknown-surface",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
        "surfaceContract",
    ] {
        assert!(
            SURFACE.contains(needle),
            "surface CLI must expose target plus contract field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Surface CLI"),
        "DevTools skill should route surface inspection through surface CLI"
    );
}

#[test]
fn elements_cli_promotes_semantic_tree_to_first_class_receipt() {
    for needle in [
        "script-kit-devtools.elements",
        "elements.snapshot",
        "scripts/devtools/targets.ts",
        "getElements",
        "elementsResult",
        "semanticSurface",
        "nodes",
        "semanticId",
        "duplicateSemanticIds",
        "focusedSemanticId",
        "selectedSemanticId",
        "elementBounds",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            ELEMENTS.contains(needle),
            "elements CLI must expose semantic tree field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Elements CLI"),
        "DevTools skill should route semantic inspection through elements CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/elements.ts snapshot"),
        "schema should name the concrete elements implementation"
    );
}

#[test]
fn layout_cli_reports_bounds_overlaps_and_resize_pressure() {
    for needle in [
        "script-kit-devtools.layout",
        "layout.measure",
        "scripts/devtools/targets.ts",
        "getLayoutInfo",
        "layoutInfoResult",
        "viewportRect",
        "windowRect",
        "window: {",
        "viewport: {",
        "pressure: {",
        "clientHeight",
        "scrollHeight",
        "canScrollY",
        "hiddenContentHeight",
        "footerOverlapCount",
        "inputOverlapCount",
        "regions",
        "nodes",
        "clipped",
        "overlaps",
        "resizePressure",
        "overflowY",
        "clippedNodeCount",
        "overlapCount",
        "pressureScore",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            LAYOUT.contains(needle),
            "layout CLI must expose geometry field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Layout CLI"),
        "DevTools skill should route geometry inspection through layout CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/layout.ts measure"),
        "schema should name the concrete layout implementation"
    );
}

#[test]
fn scroll_cli_reports_viewport_selected_row_and_overflow_pressure() {
    for needle in [
        "script-kit-devtools.scroll",
        "scroll.inspect",
        "scripts/devtools/targets.ts",
        "getState",
        "summaryOnly",
        "mainListScroll",
        "scrollTop",
        "contentHeight",
        "viewportHeight",
        "safeViewportHeight",
        "maxScrollTop",
        "canScrollY",
        "selectedRowVisible",
        "selectedRowAboveFooter",
        "hiddenContentHeight",
        "selectedRowOutsideSafeViewport",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            SCROLL.contains(needle),
            "scroll CLI must expose scroll/overflow field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Scroll CLI"),
        "DevTools skill should route scroll inspection through scroll CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/scroll.ts inspect"),
        "schema should name the concrete scroll implementation"
    );
}

#[test]
fn focus_cli_reports_focus_selected_nodes_and_keyboard_owner() {
    for needle in [
        "script-kit-devtools.focus",
        "focus.inspect",
        "scripts/devtools/targets.ts",
        "getState",
        "getElements",
        "summaryOnly",
        "windowFocused",
        "windowVisible",
        "focusedSemanticId",
        "selectedSemanticId",
        "focusedNode",
        "selectedNode",
        "activeFooter",
        "keyboardOwner",
        "inputOwnership",
        "keyboardPolicy",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            FOCUS.contains(needle),
            "focus CLI must expose focus/keyboard field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Focus CLI"),
        "DevTools skill should route focus inspection through focus CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/focus.ts inspect"),
        "schema should name the concrete focus implementation"
    );
}

#[test]
fn text_cli_reports_text_fingerprints_and_text_bounds_gap() {
    for needle in [
        "script-kit-devtools.text",
        "text.measure",
        "scripts/devtools/targets.ts",
        "getState",
        "getElements",
        "summaryOnly",
        "textSummary",
        "inputFingerprint",
        "selectedFingerprint",
        "textNodeCount",
        "textLength",
        "fingerprint",
        "footerTexts",
        "textBounds",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            TEXT.contains(needle),
            "text CLI must expose text-measurement field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Text CLI"),
        "DevTools skill should route text inspection through text CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/text.ts measure"),
        "schema should name the concrete text implementation"
    );
}

#[test]
fn keyboard_cli_reports_bindings_policies_and_duplicate_keys() {
    for needle in [
        "script-kit-devtools.keyboard",
        "keyboard.inspect",
        "scripts/devtools/targets.ts",
        "getState",
        "summaryOnly",
        "keyboardPolicy",
        "inputOwnership",
        "focusPolicy",
        "activeFooter",
        "nativeFooterHostInstalled",
        "bindings",
        "duplicateKeys",
        "actionsDialog",
        "activePopup",
        "blocked-by-missing-primitive",
        "strictTargetIdentity",
    ] {
        assert!(
            KEYBOARD.contains(needle),
            "keyboard CLI must expose binding/policy field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Keyboard CLI"),
        "DevTools skill should route keyboard inspection through keyboard CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/keyboard.ts inspect"),
        "schema should name the concrete keyboard implementation"
    );
}

#[test]
fn keyboard_cli_reports_footer_slot_contract_fields() {
    for needle in [
        "actionSlotCount",
        "contextChipCount",
        "duplicateShortcutKeys",
        "slotContractViolation",
    ] {
        assert!(
            KEYBOARD.contains(needle),
            "keyboard CLI must expose activeFooter slot-contract field: {needle}"
        );
    }
}

#[test]
fn actions_cli_reports_popup_route_geometry_shortcuts_and_gaps() {
    for needle in [
        "script-kit-devtools.actions",
        "actions.inspect",
        "scripts/devtools/act.ts",
        "scripts/devtools/targets.ts",
        "scripts/devtools/elements.ts",
        "scripts/devtools/layout.ts",
        "scripts/devtools/keyboard.ts",
        "--open",
        "--start",
        "--keep-open",
        "--open-target-kind",
        "SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN",
        "openNotes",
        "parentOpenReceipt",
        "open-actions",
        "--show",
        "actionsDialog",
        "state.actionsDialog",
        "attachedPopup",
        "rowGeometry",
        "sectionBoundsAvailable",
        "runtimeBoundsAvailable",
        "visibleSample",
        "parentTarget",
        "parentWindowId",
        "popupState",
        "routeStateAvailable",
        "routeStack",
        "popupRect",
        "parentRect",
        "anchorRect",
        "pinnedEdge",
        "generation",
        "stale",
        "clippingEdges",
        "section bounds",
        "hover row",
        "runtime shortcut layout bounds",
        "getLayoutInfo(actionsDialog)",
        "disabledReasonBoundsRequired",
        "disabled reason bounds",
        "proofMode",
        "blocked-by-missing-primitive",
    ] {
        assert!(
            ACTIONS.contains(needle),
            "actions CLI must expose popup proof field or missing primitive: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Actions CLI"),
        "DevTools skill should route popup/action-menu inspection through actions CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/actions.ts inspect"),
        "schema should name the concrete actions implementation"
    );
    for needle in [
        "GetStateTargetResolution::ActionsDialog",
        "get_actions_dialog_entity(cx)",
        "automation_state(\"actionsDialog\", cx)",
    ] {
        assert!(
            PROMPT_HANDLER.contains(needle),
            "getState must expose ActionsDialog state for popup DevTools inspection: {needle}"
        );
    }
    assert!(
        ACTIONS_DIALOG.contains("actions_popup_automation_snapshot()"),
        "ActionsDialog state must include the runtime-owned attached popup snapshot"
    );
    for needle in [
        "pub(crate) fn automation_layout_info",
        "ActionsDialog",
        "ActionsSearchInput",
        "ActionsContextHeader",
        "ActionsList",
        "ActionsRow[",
        "ActionsShortcut[",
        "LayoutComponentType::ListItem",
    ] {
        assert!(
            ACTIONS_DIALOG.contains(needle),
            "ActionsDialog must expose target-scoped layout info field: {needle}"
        );
    }
    for needle in [
        "ACTIONS_POPUP_AUTOMATION_SNAPSHOT",
        "record_actions_popup_automation_snapshot",
        "update_actions_popup_automation_snapshot_for_resize",
        "clear_actions_popup_automation_snapshot",
        "anchorRect",
        "pinnedEdge",
        "generation",
        "stale",
    ] {
        assert!(
            ACTIONS_WINDOW.contains(needle),
            "actions window runtime must own attached popup proof field: {needle}"
        );
    }
    for needle in [
        "devtools_row_geometry",
        "runtime.actionsDialog.render",
        "popupLogicalPx",
        "attachedPopupGeneration",
        "disabledReasonLayout",
        "shortcutLayout",
        "footer_shortcut_keycap_layout_model",
        "FOOTER_SHORTCUT_LAYOUT_MEASUREMENT_SOURCE",
        "sectionBoundsAvailable",
        "selectedRowBoundsAvailable",
        "shortcutBoundsAvailable",
        "disabledReasonBoundsAvailable",
        "hoverRowAvailable",
        "hoveredRow",
        ".on_hover",
    ] {
        assert!(
            ACTIONS_DIALOG.contains(needle),
            "ActionsDialog state must expose runtime row geometry field: {needle}"
        );
    }
}

#[test]
fn actions_cli_session_arg_does_not_suppress_actions_dialog_default_target() {
    for needle in [
        "hasExplicitInspectTarget",
        "inspectTargetForwarded",
        "DEFAULT_INSPECT_TARGET",
        "\"--target-kind\", \"actionsDialog\"",
        "\"--surface\", \"ActionsDialog\"",
        "inspectForwarded(args)",
    ] {
        assert!(
            ACTIONS.contains(needle),
            "actions.inspect must distinguish transport args from explicit inspect targets: {needle}"
        );
    }

    assert!(
        !ACTIONS.contains("args.forwarded.length > 0"),
        "session/timeout forwarding must not suppress the ActionsDialog default target"
    );
}

#[test]
fn actions_cli_waits_for_actions_dialog_target_after_protocol_open() {
    for needle in [
        "waitForActionsDialogTarget",
        "targets.inspect.actionsDialog.ready",
        "automationId === \"actions-dialog\"",
        "targetKind === \"ActionsDialog\"",
        "targetReadiness",
    ] {
        assert!(
            ACTIONS.contains(needle),
            "actions.inspect must prove ActionsDialog target readiness after openActions: {needle}"
        );
    }
}

#[test]
fn act_cli_performs_safe_protocol_first_user_actions_with_pre_post_receipts() {
    for needle in [
        "script-kit-devtools.act",
        "set-input",
        "select",
        "key",
        "open-actions",
        "scripts/devtools/targets.ts",
        "scripts/devtools/focus.ts",
        "scripts/devtools/scroll.ts",
        "batch",
        "setInput",
        "openActions",
        "selectBySemanticId",
        "simulateKey",
        "command: `act.${args.actionKind}`",
        "targetBefore",
        "targetAfter",
        "visibleResult",
        "blocked-by-unsafe-operation",
        "actionFailed",
        "actionReceipt.success === false",
        "(result as JsonObject).success === false",
        "--text",
        "--value",
        "--allow-submit",
        "menu-syntax-trigger-accept",
        "isScopedMenuSyntaxTriggerAccept",
        "menuSyntaxTriggerPicker",
        "menu-syntax-object-selector-accept",
        "isScopedMenuSyntaxObjectSelectorAccept",
        "menuSyntaxObjectSelector",
        "nativeEscalation: false",
    ] {
        assert!(
            ACT.contains(needle),
            "act CLI must expose safe user-action field or guard: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Act CLI"),
        "DevTools skill should route user-like actions through act CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/act.ts set-input|select|key|open-actions"),
        "schema should name the concrete act implementation"
    );
}

#[test]
fn compare_cli_pins_red_green_receipts_to_same_primitive_stack_and_target() {
    for needle in [
        "script-kit-devtools.compare",
        "compare.redgreen",
        "redReceiptIds",
        "greenReceiptIds",
        "samePrimitiveStack",
        "sameUserPath",
        "sameTargetSelector",
        "targetIdentityComparable",
        "metricNamesComparable",
        "classificationDelta",
        "blocked-by-missing-primitive",
        "--require-fixed",
    ] {
        assert!(
            COMPARE.contains(needle),
            "compare CLI must expose red/green proof field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Compare CLI"),
        "DevTools skill should route before/after proof through compare CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/compare.ts redgreen"),
        "schema should name the concrete compare implementation"
    );
}

#[test]
fn events_cli_records_action_correlated_app_and_response_logs() {
    for needle in [
        "script-kit-devtools.events",
        "events.record",
        "events.tail",
        "scripts/agentic/session.sh",
        "session-status",
        "appLog",
        "responses",
        "recordedCommand",
        "actionReceipt",
        "eventSummary",
        "correlationId",
        "commandType",
        "blocked-by-missing-primitive",
    ] {
        assert!(
            EVENTS.contains(needle),
            "events CLI must expose action-correlated event field: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Events CLI"),
        "DevTools skill should route protocol/log correlation through events CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/events.ts record"),
        "schema should name the concrete events implementation"
    );
}

#[test]
fn events_cli_compacts_child_output_and_records_artifact_path() {
    for needle in [
        "--output",
        "--preview-bytes",
        "--inline-full-output",
        "outputPolicy",
        "actionOutput",
        "stdoutSummary",
        "stderrSummary",
        "omittedBytes",
        "fingerprint",
        "stdoutJson",
        "artifactPath",
    ] {
        assert!(
            EVENTS.contains(needle),
            "events.ts must expose compact command output field {needle}"
        );
    }
}

#[test]
fn events_cli_reports_openai_pi_and_external_wrapper_signals() {
    for needle in [
        "--external-session",
        "--external-output-log",
        "--wrapper-timeout-ms",
        "failureSignals",
        "openAi",
        "statusNotAgentChatSeen",
        "piWarm",
        "likelyOwners",
        "wrapperTimeoutIsNotBrowserSessionFailure",
        "reattachInstruction",
    ] {
        assert!(
            EVENTS.contains(needle),
            "events.ts must expose proof-failure signal {needle}"
        );
    }
}

#[test]
fn focus_and_keyboard_report_native_footer_activation_gap() {
    for needle in [
        "nativeFooter",
        "nativeFooterHostInstalled",
        "activationPrimitiveAvailable",
        "nativeFooterActivationReceipt",
        "missingPrimitive",
    ] {
        assert!(
            FOCUS.contains(needle) || KEYBOARD.contains(needle),
            "focus/keyboard must expose native footer gap field {needle}"
        );
    }
}

#[test]
fn notes_cli_exposes_notes_window_state_and_missing_runtime_primitives() {
    for needle in [
        "script-kit-devtools.notes",
        "notes.inspect",
        "openNotes",
        "scripts/devtools/targets.ts",
        "scripts/devtools/elements.ts",
        "scripts/devtools/focus.ts",
        "scripts/devtools/text.ts",
        "scripts/devtools/layout.ts",
        "scripts/devtools/coverage.ts",
        "input:notes-editor",
        "panel:notes-window",
        "activeNoteId",
        "dirtyState",
        "selectionRange",
        "draftSnapshot",
        "editorAnchor",
        "previewAnchor",
        "scrollMetricsAvailable",
        "scrollTopAvailable",
        "scrollHeightAvailable",
        "clientHeightAvailable",
        "scrollTop",
        "scrollHeight",
        "clientHeight",
        "togglePreview",
        "bodyFingerprint",
        "bodyByteLength",
        "selectionUnit",
        "contentReturned",
        "layout.measure",
        "editorRegion",
        "storage",
        "commandBars",
        "shortcutRegistry",
        "focusTransitions",
        "autosize",
        "lastAutosizeTransition",
        "shortcutActivation",
        "runtimeState",
        "stateResult",
        "redacted",
        "getState",
        "editorFingerprint",
        "missingRuntimePrimitives",
        "blocked-by-missing-primitive",
        "resize-compare",
        "SCRIPT_KIT_TEST_NOTES_DB_PATH",
        "--sandbox-db",
        "--confirm-real-notes-mutation",
        "blocked-by-real-data-risk",
        "devtools.notes.resizeCompare",
        "height grew after tall content",
        "height shrank after short content",
        "autosize generation advanced",
        "raw note content redacted",
    ] {
        assert!(
            NOTES.contains(needle),
            "notes CLI must expose Notes-specific inspect field or gap: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Notes CLI"),
        "DevTools skill should route Notes window debugging through notes CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/notes.ts inspect"),
        "schema should name the concrete notes implementation"
    );
    assert!(
        SCHEMA.contains("devtools.notes.resizeCompare")
            && SCHEMA.contains("bun scripts/devtools/notes.ts resize-compare"),
        "schema should name the concrete Notes resize-compare primitive"
    );
}

#[test]
fn dictation_cli_passively_reports_media_state_targets_and_delivery_gaps() {
    for needle in [
        "script-kit-devtools.dictation",
        "dictation.inspect",
        "deliver-fixture",
        "dictation.deliverFixture",
        "open-overlay-fixture",
        "openDictationOverlayFixture",
        "targets.inspect.dictation",
        "visualOverlayFixtureOpened",
        "scripts/devtools/coverage.ts",
        "scripts/devtools/media.ts",
        "SCRIPT_KIT_DICTATION_JSON",
        "DictationSessionPhase",
        "DictationTarget",
        "PushDictationResult",
        "noMicrophoneCaptureRequired",
        "noTccMutationRequired",
        "noSyntheticTranscriptInjected",
        "syntheticTranscriptInjected",
        "transcriptContentReturned",
        "runtimeState",
        "stateResult",
        "lastDelivery",
        "deliveryReceiptAvailable",
        "deliveryAdvanced",
        "insertionRangeAvailable",
        "redactedFingerprint",
        "passive current phase RPC",
        "target delivery generation receipt",
        "cursor insertion range",
        "cursor insertion range for Notes/Agent Chat/frontmost destinations",
        "wrong-target refusal receipt",
        "blocked-by-missing-primitive",
    ] {
        assert!(
            DICTATION.contains(needle),
            "dictation CLI must expose passive media field or gap: {needle}"
        );
    }

    assert!(
        DEVTOOLS_SKILL.contains("Dictation CLI"),
        "DevTools skill should route Dictation debugging through dictation CLI"
    );
    assert!(
        SCHEMA.contains("bun scripts/devtools/dictation.ts inspect"),
        "schema should name the concrete dictation implementation"
    );
    assert!(
        SCHEMA.contains("devtools.dictation.deliverFixture")
            && SCHEMA.contains("bun scripts/devtools/dictation.ts deliver-fixture"),
        "schema should name the concrete dictation delivery fixture primitive"
    );
}

#[test]
fn inspect_reports_capabilities_gaps_and_next_steps() {
    for needle in [
        "script-kit-devtools.inspect",
        "schemaVersion",
        "capabilities",
        "capabilityDetails",
        "command: \"inspect.orchestrate\"",
        "bug: {",
        "visibleWindowProof",
        "primitiveStack",
        "summaryOnly",
        "getState(summaryOnly)",
        "missingFields",
        "missingFieldDetails",
        "recommendedNext",
        "recommendedNextPrimitives",
        "likelyOwners",
        "doNotUseRecipeReason",
        "cleanup",
        "blocked-by-missing-primitive",
        "errors",
        "status: errors.length === 0 ? (missing.length === 0 ? \"ok\" : \"partial\") : \"blocked\"",
        "target-scoped layout missing",
        "inspect is read-only; use act/batch for mutation proof",
    ] {
        assert!(
            INSPECT.contains(needle),
            "devtools inspect report is missing contract field or fail-closed behavior: {needle}"
        );
    }

    for gap in [
        "target_state",
        "semantic_elements",
        "full_semantic_elements",
        "target_layout_info",
        "screenshot_metadata",
    ] {
        assert!(
            INSPECT.contains(gap),
            "devtools inspect must name missing coverage gap {gap}"
        );
    }
}

#[test]
fn inspect_supports_agent_target_selection() {
    for needle in [
        "--target-id",
        "--target-kind",
        "--target-index",
        "--target-title",
        "--focused",
        "--main",
        "--bug",
        "--surface",
        "--hi-dpi",
        "--start",
        "--show",
    ] {
        assert!(
            INSPECT.contains(needle),
            "devtools inspect must support target selector {needle}"
        );
    }
}

#[test]
fn devtools_skill_keeps_recipes_as_regression_wrappers() {
    for needle in [
        "Think Chrome DevTools for Script Kit, not a script catalog.",
        "Use recipes only when they match the bug directly or as regression proof",
        "Produce at least one direct primitive receipt",
        "devtools.inspect",
    ] {
        assert!(
            DEVTOOLS_SKILL.contains(needle),
            "script-kit-devtools skill must preserve the DevTools-first boundary: {needle}"
        );
    }

    assert!(
        DEVTOOLS_AUDIT.contains("Recipes should be rebuilt as thin smoke/regression wrappers"),
        "coverage audit must keep recipe usage bounded to smoke/regression wrappers"
    );
}

#[test]
fn devtools_docs_are_checked_in() {
    for path in [
        ".agents/skills/script-kit-devtools/SKILL.md",
        ".agents/skills/script-kit-devtools/references/devtools-coverage-audit.md",
        "scripts/devtools/inspect.ts",
        "scripts/devtools/targets.ts",
        "scripts/devtools/surface.ts",
        "scripts/devtools/elements.ts",
        "scripts/devtools/layout.ts",
        "scripts/devtools/scroll.ts",
        "scripts/devtools/focus.ts",
        "scripts/devtools/text.ts",
        "scripts/devtools/keyboard.ts",
        "scripts/devtools/actions.ts",
        "scripts/devtools/act.ts",
        "scripts/devtools/compare.ts",
        "scripts/devtools/events.ts",
        "scripts/devtools/notes.ts",
        "scripts/devtools/dictation.ts",
    ] {
        assert!(
            fs::metadata(path).is_ok(),
            "expected checked-in DevTools artifact at {path}"
        );
    }
}
