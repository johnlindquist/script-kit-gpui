use serde_json::Value;

const MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-binding-v1/slice.manifest.json"
);
const TARGET_LIFECYCLE_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-target-lifecycle-v1/slice.manifest.json"
);
const TARGET_LIFECYCLE_V2_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-target-lifecycle-v2/slice.manifest.json"
);
const INPUT_FOCUS_RESIZE_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-input-focus-resize-v1/slice.manifest.json"
);
const GLOBAL_BUILTINS_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-global-builtins-v1/slice.manifest.json"
);
const BUILTINS_PREFERENCES_GLOBAL_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtins-preferences-global-v1/slice.manifest.json"
);
const SYSTEM_PERMISSION_AGENT_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-system-permission-agent-copy-v1/slice.manifest.json"
);
const BUILTIN_INTERNAL_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtin-internal-copy-v1/slice.manifest.json"
);
const NONDESTRUCTIVE_BUILTINS_COPY_MANIFEST: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-nondestructive-builtins-copy-v1/slice.manifest.json"
);
const DT_011: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-binding-v1/dt-truth-011-actions-parent-filter-mutates-while-open/scenario.receipt.json"
);
const DT_013: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-binding-v1/dt-truth-013-actions-section-heading-not-action-target/scenario.receipt.json"
);
const DT_015: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-binding-v1/dt-truth-015-actions-dialog-parent-focus-return-truth/scenario.receipt.json"
);
const DT_013_TARGET_LIFECYCLE: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-target-lifecycle-v1/dt-truth-013-actions-section-heading-not-action-target/scenario.receipt.json"
);
const DT_013_TARGET_LIFECYCLE_V2: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-target-lifecycle-v2/dt-truth-013-actions-section-heading-not-action-target/scenario.receipt.json"
);
const DT_017_INPUT_FOCUS_RESIZE: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-input-focus-resize-v1/dt-truth-017-actions-empty-filter-no-submit/scenario.receipt.json"
);
const DT_018_INPUT_FOCUS_RESIZE: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-input-focus-resize-v1/dt-truth-018-actions-filter-resize-shrink-grow/scenario.receipt.json"
);
const DT_019_INPUT_FOCUS_RESIZE: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-input-focus-resize-v1/dt-truth-019-actions-footerless-shortcut-layout-truth/scenario.receipt.json"
);
const DT_020_INPUT_FOCUS_RESIZE: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-input-focus-resize-v1/dt-truth-020-actions-escape-dismiss-parent-focus-return/scenario.receipt.json"
);
const DT_021_GLOBAL_BUILTINS: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-global-builtins-v1/dt-truth-021-actions-global-settings-copy-visible/scenario.receipt.json"
);
const DT_022_GLOBAL_BUILTINS: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-global-builtins-v1/dt-truth-022-actions-global-reload-submit-gated/scenario.receipt.json"
);
const DT_023_GLOBAL_BUILTINS: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-global-builtins-v1/dt-truth-023-actions-builtin-agent-chat-primary-copy/scenario.receipt.json"
);
const DT_024_GLOBAL_BUILTINS: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-global-builtins-v1/dt-truth-024-actions-builtin-clipboard-history-no-favorite/scenario.receipt.json"
);
const DT_025_BUILTINS_PREFERENCES_GLOBAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtins-preferences-global-v1/dt-truth-025-actions-agent-chat-copy-deeplink-truth/scenario.receipt.json"
);
const DT_026_BUILTINS_PREFERENCES_GLOBAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtins-preferences-global-v1/dt-truth-026-actions-agent-chat-preference-plan-truth/scenario.receipt.json"
);
const DT_027_BUILTINS_PREFERENCES_GLOBAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtins-preferences-global-v1/dt-truth-027-actions-clipboard-history-copy-deeplink-truth/scenario.receipt.json"
);
const DT_028_BUILTINS_PREFERENCES_GLOBAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtins-preferences-global-v1/dt-truth-028-actions-global-show-logs-copy-gated/scenario.receipt.json"
);
const DT_029_SYSTEM_PERMISSION_AGENT: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-system-permission-agent-copy-v1/dt-truth-029-actions-agent_chat-history-agent-chat-copy-truth/scenario.receipt.json"
);
const DT_030_SYSTEM_PERMISSION_AGENT: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-system-permission-agent-copy-v1/dt-truth-030-actions-permission-assistant-accessibility-copy-gated/scenario.receipt.json"
);
const DT_031_SYSTEM_PERMISSION_AGENT: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-system-permission-agent-copy-v1/dt-truth-031-actions-system-settings-pane-copy-gated/scenario.receipt.json"
);
const DT_032_SYSTEM_PERMISSION_AGENT: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-system-permission-agent-copy-v1/dt-truth-032-actions-generate-script-agent-chat-handoff-copy-gated/scenario.receipt.json"
);
const DT_033_BUILTIN_INTERNAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtin-internal-copy-v1/dt-truth-033-actions-file-search-main-row-copy-deeplink-truth/scenario.receipt.json"
);
const DT_034_BUILTIN_INTERNAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtin-internal-copy-v1/dt-truth-034-actions-file-search-empty-context-global-fallback-truth/scenario.receipt.json"
);
const DT_035_BUILTIN_INTERNAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtin-internal-copy-v1/dt-truth-035-actions-process-manager-copy-suppresses-stop-kill-truth/scenario.receipt.json"
);
const DT_036_BUILTIN_INTERNAL: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-builtin-internal-copy-v1/dt-truth-036-actions-do-current-app-collapsed-alias-copy-truth/scenario.receipt.json"
);
const DT_037_NONDESTRUCTIVE_BUILTINS_COPY: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-nondestructive-builtins-copy-v1/dt-truth-037-actions-app-launcher-copy-deeplink-truth/scenario.receipt.json"
);
const DT_038_NONDESTRUCTIVE_BUILTINS_COPY: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-nondestructive-builtins-copy-v1/dt-truth-038-actions-emoji-copy-deeplink-truth/scenario.receipt.json"
);
const DT_040_NONDESTRUCTIVE_BUILTINS_COPY: &str = include_str!(
    "../.agents/skills/script-kit-devtools/references/devtools-truth-scenarios/receipts/direct-actions-nondestructive-builtins-copy-v1/dt-truth-040-actions-notes-copy-deeplink-truth/scenario.receipt.json"
);

const ALLOWED_PRIMITIVES: &[&str] = &[
    "scripts/devtools/targets.ts",
    "scripts/devtools/elements.ts",
    "scripts/devtools/focus.ts",
    "scripts/devtools/text.ts",
    "scripts/devtools/layout.ts",
    "scripts/devtools/act.ts",
    "scripts/devtools/actions.ts",
    "scripts/devtools/keyboard.ts",
];

const FORBIDDEN_EXECUTORS: &[&str] = &[
    "scripts/agentic/index.ts",
    "scripts/agentic/user-story-audit.ts",
    "scripts/agentic/surface-navigator.ts",
    "tests/smoke/",
    "stress recipe",
    "recipe catalog",
];

const REQUIRED_TRUTH_MODEL_FIELDS: &[&str] = &[
    "routeGeneration",
    "focusOwner",
    "selectedSemanticId",
    "visibleLabel",
    "footerIntent",
    "actionId",
    "handlerId",
    "sideEffectClass",
    "disabledReason",
    "targetSurface",
    "parentSubjectId",
    "layoutGeneration",
];

fn parse(json: &str) -> Value {
    serde_json::from_str(json).expect("receipt JSON must parse")
}

fn command_script(argv: &Value) -> Option<&str> {
    argv.as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|part| {
            part.starts_with("scripts/devtools/")
                || part.starts_with("scripts/agentic/")
                || part.starts_with("tests/smoke/")
        })
}

#[test]
fn direct_actions_binding_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["sliceId"], "direct-actions-binding-v1");
    assert_eq!(manifest["oracleSession"], "devtools-truth-execution");
    assert_eq!(
        manifest["scenarioLedgerOracleSession"],
        "new-devtools-scenarios-plan"
    );
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-011-actions-parent-filter-mutates-while-open",
            "dt-truth-013-actions-section-heading-not-action-target",
            "dt-truth-015-actions-dialog-parent-focus-return-truth",
        ],
        "first direct execution slice must stay small and exact"
    );

    for forbidden in FORBIDDEN_EXECUTORS {
        assert!(
            !MANIFEST.contains(forbidden),
            "manifest must not reference forbidden executor {forbidden}"
        );
    }
}

#[test]
fn direct_actions_binding_receipts_have_required_truth_schema_and_safety() {
    for (expected_id, raw) in [
        (
            "dt-truth-011-actions-parent-filter-mutates-while-open",
            DT_011,
        ),
        (
            "dt-truth-013-actions-section-heading-not-action-target",
            DT_013,
        ),
        (
            "dt-truth-015-actions-dialog-parent-focus-return-truth",
            DT_015,
        ),
    ] {
        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["oracleSession"], "devtools-truth-execution");
        assert_eq!(
            receipt["scenarioLedgerOracleSession"],
            "new-devtools-scenarios-plan"
        );
        assert_eq!(receipt["executor"], "direct-devtools-primitives");

        let result = receipt["result"].as_str().expect("result must be string");
        assert!(
            matches!(
                result,
                "pass"
                    | "fail"
                    | "blocked-by-missing-primitive"
                    | "blocked-by-unsafe-operation"
                    | "needs-oracle-review"
            ),
            "unexpected scenario result {result}"
        );

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        let user_path = receipt["userPath"]
            .as_array()
            .expect("userPath must be array");
        assert!(
            !user_path.is_empty(),
            "{expected_id} must record user path steps"
        );
        for step in user_path {
            assert_eq!(
                step["controlChannel"], "direct-primitive",
                "{expected_id} must use direct primitives only"
            );
            assert!(
                step["receiptRef"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(".json")),
                "{expected_id} userPath step must reference a JSON primitive receipt"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }
    }
}

#[test]
fn direct_actions_binding_receipts_only_reference_allowed_devtools_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-011-actions-parent-filter-mutates-while-open",
            DT_011,
        ),
        (
            "dt-truth-013-actions-section-heading-not-action-target",
            DT_013,
        ),
        (
            "dt-truth-015-actions-dialog-parent-focus-return-truth",
            DT_015,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(
            !commands.is_empty(),
            "{expected_id} must record top-level primitive commands"
        );
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }
    }
}

#[test]
fn direct_actions_binding_slice_has_expected_primitive_coverage_by_scenario() {
    let required = [
        (
            "dt-truth-011-actions-parent-filter-mutates-while-open",
            parse(DT_011),
            vec![
                "targets.inspect",
                "elements.snapshot",
                "focus.inspect",
                "text.measure",
                "act.open-actions",
                "actions.inspect",
                "act.set-input",
                "layout.measure",
            ],
        ),
        (
            "dt-truth-013-actions-section-heading-not-action-target",
            parse(DT_013),
            vec![
                "targets.inspect",
                "act.open-actions",
                "actions.inspect",
                "act.set-input",
                "elements.snapshot",
                "focus.inspect",
                "act.key",
                "layout.measure",
            ],
        ),
        (
            "dt-truth-015-actions-dialog-parent-focus-return-truth",
            parse(DT_015),
            vec![
                "targets.inspect",
                "focus.inspect",
                "text.measure",
                "act.open-actions",
                "actions.inspect",
                "act.key",
                "act.set-input",
                "elements.snapshot",
                "layout.measure",
            ],
        ),
    ];

    for (scenario_id, receipt, expected_commands) in required {
        let primitive_commands = receipt["primitiveReceipts"]
            .as_array()
            .expect("primitiveReceipts must be array")
            .iter()
            .filter_map(|entry| entry["command"].as_str())
            .collect::<Vec<_>>();
        for expected in expected_commands {
            assert!(
                primitive_commands.contains(&expected),
                "{scenario_id} missing primitive command coverage {expected}; got {primitive_commands:?}"
            );
        }
    }
}

#[test]
fn direct_actions_binding_records_first_slice_as_blocked_not_green() {
    let manifest = parse(MANIFEST);
    assert_eq!(manifest["summary"]["pass"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 3);

    for (expected_id, raw) in [
        (
            "dt-truth-011-actions-parent-filter-mutates-while-open",
            DT_011,
        ),
        (
            "dt-truth-013-actions-section-heading-not-action-target",
            DT_013,
        ),
        (
            "dt-truth-015-actions-dialog-parent-focus-return-truth",
            DT_015,
        ),
    ] {
        let receipt = parse(raw);
        assert_eq!(
            receipt["result"], "blocked-by-missing-primitive",
            "{expected_id} should preserve the direct DevTools blocker instead of claiming green"
        );
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be an array");
        assert!(
            checks.iter().any(|check| check["status"] == "blocked"),
            "{expected_id} must name the blocked truth check"
        );
    }
}

#[test]
fn direct_actions_target_lifecycle_slice_records_heading_truth_and_submit_failure() {
    let manifest = parse(TARGET_LIFECYCLE_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["sliceId"], "direct-actions-target-lifecycle-v1");
    assert_eq!(manifest["oracleSession"], "actions-target-session-fix");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["summary"]["fail"], 1);
    assert_eq!(manifest["summary"]["pass"], 0);
    assert_eq!(
        manifest["scenarioIds"]
            .as_array()
            .expect("scenarioIds must be an array")
            .iter()
            .map(|value| value.as_str().expect("scenario id must be string"))
            .collect::<Vec<_>>(),
        vec!["dt-truth-013-actions-section-heading-not-action-target"]
    );

    let receipt = parse(DT_013_TARGET_LIFECYCLE);
    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(
        receipt["scenarioId"],
        "dt-truth-013-actions-section-heading-not-action-target"
    );
    assert_eq!(receipt["result"], "fail");
    assert_eq!(
        receipt["truthModel"]["selectedSemanticId"],
        "choice:2:toggle_info"
    );
    assert_eq!(receipt["truthModel"]["actionId"], "toggle_info");
    assert_eq!(receipt["safety"]["submitAttempted"], false);

    for field in REQUIRED_TRUTH_MODEL_FIELDS {
        assert!(
            receipt["truthModel"]
                .as_object()
                .expect("truthModel must be object")
                .contains_key(*field),
            "target lifecycle receipt missing truthModel.{field}"
        );
    }

    let checks = receipt["truthChecks"]
        .as_array()
        .expect("truthChecks must be array");
    for expected_pass in [
        "actionsDialogTargetIsExplicit",
        "sectionRowsAreNonExecutable",
        "selectedSemanticIdExcludesHeadings",
        "keyboardNavigationSkipsHeadings",
    ] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == expected_pass && check["status"] == "pass"),
            "expected pass check {expected_pass}"
        );
    }
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "safeSubmitRequiresLiveTarget"
                && check["status"] == "fail"
                && check["actual"]
                    .as_str()
                    .is_some_and(|actual| actual.contains("session_dead"))),
        "receipt must preserve the live-target submit failure"
    );
}

#[test]
fn direct_actions_target_lifecycle_uses_only_direct_devtools_primitives() {
    for forbidden in FORBIDDEN_EXECUTORS {
        assert!(
            !TARGET_LIFECYCLE_MANIFEST.contains(forbidden),
            "manifest must not reference forbidden executor {forbidden}"
        );
        assert!(
            !DT_013_TARGET_LIFECYCLE.contains(forbidden),
            "receipt must not reference forbidden executor {forbidden}"
        );
    }

    let receipt = parse(DT_013_TARGET_LIFECYCLE);
    let commands = receipt["executorProvenance"]["topLevelCommands"]
        .as_array()
        .expect("executorProvenance.topLevelCommands must be array");
    assert_eq!(commands.len(), 16);
    for command in commands {
        let script =
            command_script(&command["argv"]).expect("each command argv must include a script path");
        assert!(
            ALLOWED_PRIMITIVES.contains(&script),
            "target lifecycle slice used non-allowed command path {script}"
        );
    }

    let primitive_commands = receipt["primitiveReceipts"]
        .as_array()
        .expect("primitiveReceipts must be array")
        .iter()
        .filter_map(|entry| entry["command"].as_str())
        .collect::<Vec<_>>();
    for expected in [
        "actions.inspect",
        "targets.list",
        "act.set-input",
        "elements.snapshot",
        "focus.inspect",
        "act.key",
        "layout.measure",
        "targets.inspect",
    ] {
        assert!(
            primitive_commands.contains(&expected),
            "target lifecycle slice missing primitive command coverage {expected}; got {primitive_commands:?}"
        );
    }
}

#[test]
fn direct_actions_target_lifecycle_v2_records_submit_lifecycle_green() {
    let manifest = parse(TARGET_LIFECYCLE_V2_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["sliceId"], "direct-actions-target-lifecycle-v2");
    assert_eq!(manifest["oracleSession"], "actions-submit-lifecycle");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["summary"]["pass"], 1);
    assert_eq!(manifest["summary"]["fail"], 0);

    let receipt = parse(DT_013_TARGET_LIFECYCLE_V2);
    assert_eq!(receipt["result"], "pass");
    assert_eq!(
        receipt["truthModel"]["selectedSemanticId"],
        "choice:2:toggle_info"
    );
    assert_eq!(receipt["truthModel"]["actionId"], "toggle_info");
    assert_eq!(receipt["safety"]["submitAttempted"], true);
    assert_eq!(
        receipt["safety"]["submitPreflightSelectedSemanticId"],
        "choice:2:toggle_info"
    );
    assert_eq!(
        receipt["safety"]["submitLifecycleState"],
        "source-closed-parent-live"
    );

    for field in REQUIRED_TRUTH_MODEL_FIELDS {
        assert!(
            receipt["truthModel"]
                .as_object()
                .expect("truthModel must be object")
                .contains_key(*field),
            "target lifecycle v2 receipt missing truthModel.{field}"
        );
    }

    let checks = receipt["truthChecks"]
        .as_array()
        .expect("truthChecks must be array");
    for expected_pass in [
        "selectedSemanticIdExcludesHeadings",
        "safeSubmitLeavesParentInspectable",
        "postSubmitMainTargetLive",
    ] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == expected_pass && check["status"] == "pass"),
            "expected v2 pass check {expected_pass}"
        );
    }

    let commands = receipt["executorProvenance"]["topLevelCommands"]
        .as_array()
        .expect("executorProvenance.topLevelCommands must be array");
    assert_eq!(commands.len(), 16);
    for command in commands {
        let script =
            command_script(&command["argv"]).expect("each command argv must include a script path");
        assert!(
            ALLOWED_PRIMITIVES.contains(&script),
            "target lifecycle v2 used non-allowed command path {script}"
        );
    }
}

#[test]
fn direct_actions_input_focus_resize_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(INPUT_FOCUS_RESIZE_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["sliceId"], "direct-actions-input-focus-resize-v1");
    assert_eq!(manifest["oracleSession"], "actions-input-focus-resize");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 4);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-017-actions-empty-filter-no-submit",
            "dt-truth-018-actions-filter-resize-shrink-grow",
            "dt-truth-019-actions-footerless-shortcut-layout-truth",
            "dt-truth-020-actions-escape-dismiss-parent-focus-return",
        ]
    );
}

#[test]
fn direct_actions_input_focus_resize_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-017-actions-empty-filter-no-submit",
            DT_017_INPUT_FOCUS_RESIZE,
        ),
        (
            "dt-truth-018-actions-filter-resize-shrink-grow",
            DT_018_INPUT_FOCUS_RESIZE,
        ),
        (
            "dt-truth-019-actions-footerless-shortcut-layout-truth",
            DT_019_INPUT_FOCUS_RESIZE,
        ),
        (
            "dt-truth-020-actions-escape-dismiss-parent-focus-return",
            DT_020_INPUT_FOCUS_RESIZE,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }
    }
}

#[test]
fn direct_actions_input_focus_resize_records_expected_truth_checks() {
    let expected = [
        (
            DT_017_INPUT_FOCUS_RESIZE,
            vec![
                "emptyFilterHasNoVisibleActions",
                "emptyFilterHasNoSelectedAction",
                "emptySubmitBlockedBeforeDispatch",
                "blockedEnterLeavesActionsTargetLive",
                "filterRecoveryRestoresVisibleActions",
            ],
        ),
        (
            DT_018_INPUT_FOCUS_RESIZE,
            vec![
                "filterToEmptyUpdatesLayoutTruth",
                "filterRecoveryRestoresRows",
                "popupParentIdentityStableAcrossFilter",
                "popupDoesNotClipAfterShrinkOrGrow",
                "layoutReceiptsHaveNoOverlapPressure",
            ],
        ),
        (
            DT_019_INPUT_FOCUS_RESIZE,
            vec![
                "actionsDialogFooterlessAtRuntime",
                "shortcutRowsHaveRuntimeBounds",
                "shortcutLayoutRowsMatchVisibleShortcutActions",
                "layoutContainsShortcutComponents",
                "focusReceiptHasNoActiveFooter",
            ],
        ),
        (
            DT_020_INPUT_FOCUS_RESIZE,
            vec![
                "escapeDismissLeavesParentInspectable",
                "escapeDoesNotSubmitAction",
                "actionsDialogTargetGoneAfterDismiss",
                "parentFocusInspectableAfterDismiss",
                "dismissLifecycleSourceClosedParentLive",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_global_builtins_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(GLOBAL_BUILTINS_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["sliceId"], "direct-actions-global-builtins-v1");
    assert_eq!(manifest["oracleSession"], "actions-next-builtins-batch");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 4);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-021-actions-global-settings-copy-visible",
            "dt-truth-022-actions-global-reload-submit-gated",
            "dt-truth-023-actions-builtin-agent-chat-primary-copy",
            "dt-truth-024-actions-builtin-clipboard-history-no-favorite",
        ]
    );

    for forbidden in FORBIDDEN_EXECUTORS {
        assert!(
            !GLOBAL_BUILTINS_MANIFEST.contains(forbidden),
            "manifest must not reference forbidden executor {forbidden}"
        );
    }
}

#[test]
fn direct_actions_global_builtins_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-021-actions-global-settings-copy-visible",
            DT_021_GLOBAL_BUILTINS,
        ),
        (
            "dt-truth-022-actions-global-reload-submit-gated",
            DT_022_GLOBAL_BUILTINS,
        ),
        (
            "dt-truth-023-actions-builtin-agent-chat-primary-copy",
            DT_023_GLOBAL_BUILTINS,
        ),
        (
            "dt-truth-024-actions-builtin-clipboard-history-no-favorite",
            DT_024_GLOBAL_BUILTINS,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }
    }
}

#[test]
fn direct_actions_global_builtins_records_expected_truth_checks() {
    let expected = [
        (
            DT_021_GLOBAL_BUILTINS,
            vec![
                "settingsGlobalActionVisible",
                "settingsActionIdMatchesHandler",
                "settingsInspectionDidNotLaunchEditor",
                "settingsDialogFooterless",
            ],
        ),
        (
            DT_022_GLOBAL_BUILTINS,
            vec![
                "reloadGlobalActionVisible",
                "reloadSubmitWithoutAllowSubmitBlocked",
                "reloadSubmitDidNotDispatch",
                "blockedEnterLeavesActionsTargetLive",
            ],
        ),
        (
            DT_023_GLOBAL_BUILTINS,
            vec![
                "mainSelectionIsAgentChatBuiltin",
                "agentChatPrimaryCopyPreserved",
                "agentChatPrimaryActionIdMatchesRunScript",
                "agentChatPrimaryCopyIsNotGenericOpenOrRun",
            ],
        ),
        (
            DT_024_GLOBAL_BUILTINS,
            vec![
                "mainSelectionIsClipboardHistoryBuiltin",
                "clipboardHistoryUnfilteredHasNoFavoriteAction",
                "favoriteFilterHasNoVisibleActions",
                "favoriteFilterHasNoSelectedAction",
                "favoriteFilterReportsEmptyList",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_global_builtins_records_submit_gate_and_selection_copy() {
    let reload = parse(DT_022_GLOBAL_BUILTINS);
    assert_eq!(reload["truthModel"]["actionId"], "reload_scripts");
    assert_eq!(
        reload["safety"]["blockedClassification"],
        "blocked-by-unsafe-operation"
    );
    assert_eq!(reload["safety"]["submitAttempted"], false);
    assert_eq!(reload["safety"]["nativeEscalation"], false);

    let agent_chat = parse(DT_023_GLOBAL_BUILTINS);
    assert_eq!(agent_chat["truthModel"]["visibleLabel"], "Open Agent Chat");
    assert_eq!(agent_chat["truthModel"]["actionId"], "run_script");
    assert_eq!(
        agent_chat["truthModel"]["parentSubjectId"],
        "choice:22:agent-chat"
    );

    let clipboard = parse(DT_024_GLOBAL_BUILTINS);
    assert_eq!(clipboard["truthModel"]["actionId"], Value::Null);
    assert_eq!(clipboard["truthModel"]["visibleLabel"], "0 actions");
    assert_eq!(
        clipboard["truthModel"]["sideEffectClass"],
        "copy-suppression"
    );
}

#[test]
fn direct_actions_builtins_preferences_global_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(BUILTINS_PREFERENCES_GLOBAL_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(
        manifest["sliceId"],
        "direct-actions-builtins-preferences-global-v1"
    );
    assert_eq!(manifest["oracleSession"], "actions-next-builtins-batch-2");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 4);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-025-actions-agent-chat-copy-deeplink-truth",
            "dt-truth-026-actions-agent-chat-preference-plan-truth",
            "dt-truth-027-actions-clipboard-history-copy-deeplink-truth",
            "dt-truth-028-actions-global-show-logs-copy-gated",
        ]
    );

    for forbidden in FORBIDDEN_EXECUTORS {
        assert!(
            !BUILTINS_PREFERENCES_GLOBAL_MANIFEST.contains(forbidden),
            "manifest must not reference forbidden executor {forbidden}"
        );
    }
}

#[test]
fn direct_actions_builtins_preferences_global_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-025-actions-agent-chat-copy-deeplink-truth",
            DT_025_BUILTINS_PREFERENCES_GLOBAL,
        ),
        (
            "dt-truth-026-actions-agent-chat-preference-plan-truth",
            DT_026_BUILTINS_PREFERENCES_GLOBAL,
        ),
        (
            "dt-truth-027-actions-clipboard-history-copy-deeplink-truth",
            DT_027_BUILTINS_PREFERENCES_GLOBAL,
        ),
        (
            "dt-truth-028-actions-global-show-logs-copy-gated",
            DT_028_BUILTINS_PREFERENCES_GLOBAL,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }
    }
}

#[test]
fn direct_actions_builtins_preferences_global_records_expected_truth_checks() {
    let expected = [
        (
            DT_025_BUILTINS_PREFERENCES_GLOBAL,
            vec![
                "agentChatParentSelectionStable",
                "copyDeeplinkActionVisible",
                "copyDeeplinkDescriptionMatchesBuiltInName",
                "copyDeeplinkSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_026_BUILTINS_PREFERENCES_GLOBAL,
            vec![
                "aliasPlanIsMutuallyExclusive",
                "shortcutPlanIsMutuallyExclusive",
                "aliasCopyMatchesPlan",
                "shortcutCopyMatchesPlan",
                "preferenceSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_027_BUILTINS_PREFERENCES_GLOBAL,
            vec![
                "clipboardHistoryParentSelectionStable",
                "clipboardHistoryDeepLinkVisible",
                "clipboardHistoryDeepLinkDescriptionMatchesName",
                "deepLinkSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_028_BUILTINS_PREFERENCES_GLOBAL,
            vec![
                "showLogsGlobalActionVisible",
                "showLogsDescriptionMatchesGlobalCopy",
                "showLogsSubmitBlockedWithoutAllowSubmit",
                "escapeDismissLeavesParentInspectable",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_builtins_preferences_global_records_copy_and_submit_truth() {
    let agent_chat_deep = parse(DT_025_BUILTINS_PREFERENCES_GLOBAL);
    assert_eq!(
        agent_chat_deep["truthModel"]["visibleLabel"],
        "Copy Deep Link"
    );
    assert_eq!(agent_chat_deep["truthModel"]["actionId"], "copy_deeplink");
    assert_eq!(
        agent_chat_deep["truthModel"]["parentSubjectId"],
        "choice:22:agent-chat"
    );
    assert_eq!(agent_chat_deep["safety"]["submitAttempted"], false);

    let preference = parse(DT_026_BUILTINS_PREFERENCES_GLOBAL);
    assert_eq!(preference["truthModel"]["actionId"], "add_shortcut");
    assert_eq!(preference["truthModel"]["handlerId"], "shortcut_alias");
    assert_eq!(
        preference["truthModel"]["sideEffectClass"],
        "preference-mutation"
    );

    let clipboard = parse(DT_027_BUILTINS_PREFERENCES_GLOBAL);
    assert_eq!(clipboard["truthModel"]["visibleLabel"], "Copy Deep Link");
    assert_eq!(
        clipboard["truthModel"]["parentSubjectId"],
        "choice:6:clipboard-history"
    );

    let logs = parse(DT_028_BUILTINS_PREFERENCES_GLOBAL);
    assert_eq!(logs["truthModel"]["visibleLabel"], "Show Logs");
    assert_eq!(logs["truthModel"]["actionId"], "view_logs");
    assert_eq!(logs["truthModel"]["sideEffectClass"], "ui-toggle");
    assert_eq!(logs["safety"]["submitAttempted"], false);
}

#[test]
fn direct_actions_system_permission_agent_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(SYSTEM_PERMISSION_AGENT_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(
        manifest["sliceId"],
        "direct-actions-system-permission-agent-copy-v1"
    );
    assert_eq!(manifest["oracleSession"], "actions-batch-three-plan");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 4);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-029-actions-agent_chat-history-agent-chat-copy-truth",
            "dt-truth-030-actions-permission-assistant-accessibility-copy-gated",
            "dt-truth-031-actions-system-settings-pane-copy-gated",
            "dt-truth-032-actions-generate-script-agent-chat-handoff-copy-gated",
        ]
    );

    for forbidden in FORBIDDEN_EXECUTORS {
        assert!(
            !SYSTEM_PERMISSION_AGENT_MANIFEST.contains(forbidden),
            "manifest must not reference forbidden executor {forbidden}"
        );
    }
}

#[test]
fn direct_actions_system_permission_agent_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-029-actions-agent_chat-history-agent-chat-copy-truth",
            DT_029_SYSTEM_PERMISSION_AGENT,
        ),
        (
            "dt-truth-030-actions-permission-assistant-accessibility-copy-gated",
            DT_030_SYSTEM_PERMISSION_AGENT,
        ),
        (
            "dt-truth-031-actions-system-settings-pane-copy-gated",
            DT_031_SYSTEM_PERMISSION_AGENT,
        ),
        (
            "dt-truth-032-actions-generate-script-agent-chat-handoff-copy-gated",
            DT_032_SYSTEM_PERMISSION_AGENT,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["oracleSession"], "actions-batch-three-plan");
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }

        let primitive_commands = receipt["primitiveReceipts"]
            .as_array()
            .expect("primitiveReceipts must be array")
            .iter()
            .filter_map(|entry| entry["command"].as_str())
            .collect::<Vec<_>>();
        for expected in [
            "targets.inspect",
            "act.set-input",
            "act.select",
            "act.open-actions",
            "actions.inspect",
        ] {
            assert!(
                primitive_commands.contains(&expected),
                "{expected_id} missing primitive command coverage {expected}; got {primitive_commands:?}"
            );
        }
    }
}

#[test]
fn direct_actions_system_permission_agent_records_expected_truth_checks() {
    let expected = [
        (
            DT_029_SYSTEM_PERMISSION_AGENT,
            vec![
                "agent_chatHistoryParentSelectionStable",
                "agent_chatHistoryVisibleCopyNamesAgentChat",
                "agent_chatHistoryCopyRejectsAiConversations",
                "genericAssistantFilterDoesNotExposeStaleAction",
                "historyFilterRecoveryRestoresAction",
            ],
        ),
        (
            DT_030_SYSTEM_PERMISSION_AGENT,
            vec![
                "accessibilityPermissionAssistantParentSelectionStable",
                "accessibilityAssistantCopyDoesNotClaimGrant",
                "grantFilterHasNoExecutableAction",
                "openFilterRecoveryRestoresAssistantAction",
                "accessibilityAssistantSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_031_SYSTEM_PERMISSION_AGENT,
            vec![
                "privacySettingsParentSelectionStable",
                "privacySettingsCopyNamesPane",
                "systemPreferencesTextAbsent",
                "privacyFilterRecoveryRestoresPaneAction",
                "privacySettingsSubmitBlockedWithoutExternalActivation",
            ],
        ),
        (
            DT_032_SYSTEM_PERMISSION_AGENT,
            vec![
                "generateScriptParentSelectionStable",
                "generateScriptCopyNamesAgentChatHandoff",
                "currentAppGenerateScriptCopyNamesAgentChatHandoff",
                "aiAssistantFilterDoesNotExposeStaleCopy",
                "agentChatFilterRecoveryRestoresHandoffAction",
                "generateScriptSubmitBlockedWithoutAllowSubmit",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_system_permission_agent_records_copy_and_submit_truth() {
    let history = parse(DT_029_SYSTEM_PERMISSION_AGENT);
    assert_eq!(
        history["truthModel"]["visibleLabel"],
        "Open Agent Chat History"
    );
    assert_eq!(
        history["truthModel"]["parentSubjectId"],
        "choice:23:agent-chat-history"
    );
    assert_eq!(history["safety"]["submitAttempted"], false);

    let accessibility = parse(DT_030_SYSTEM_PERMISSION_AGENT);
    assert_eq!(
        accessibility["truthModel"]["visibleLabel"],
        "Open Accessibility Assistant"
    );
    assert_eq!(
        accessibility["truthModel"]["sideEffectClass"],
        "permission-assistant-open"
    );
    assert_eq!(
        accessibility["safety"]["blockedClassification"],
        "blocked-by-unsafe-operation"
    );

    let privacy = parse(DT_031_SYSTEM_PERMISSION_AGENT);
    assert_eq!(
        privacy["truthModel"]["visibleLabel"],
        "Open Privacy & Security Settings"
    );
    assert_eq!(
        privacy["truthModel"]["sideEffectClass"],
        "system-settings-pane-open"
    );
    assert_eq!(privacy["safety"]["externalActivation"], false);

    let generate = parse(DT_032_SYSTEM_PERMISSION_AGENT);
    assert_eq!(
        generate["truthModel"]["visibleLabel"],
        "Open Agent Chat to Generate Script"
    );
    assert_eq!(
        generate["truthModel"]["sideEffectClass"],
        "agent-chat-handoff"
    );
    assert_eq!(
        generate["truthModel"]["parentSubjectId"],
        "choice:39:generate-script-with"
    );
    assert_eq!(generate["safety"]["submitAttempted"], false);
}

#[test]
fn direct_devtools_script_list_selection_uses_named_state_machine() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    for expected in [
        "enum DevtoolsSelectionState",
        "DevtoolsSelectionState::MainMenuScriptList",
        "DevtoolsSelectionState::ChoiceBackedPrompt",
        "fn devtools_selection_state",
        "fn select_main_menu_choice_by_semantic_id",
        "fn select_prompt_choice_by_semantic_id",
        "fn apply_main_menu_selection",
    ] {
        assert!(
            source.contains(expected),
            "direct DevTools selection should keep ScriptList support behind named state/action {expected}"
        );
    }
}

#[test]
fn direct_actions_builtin_internal_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(BUILTIN_INTERNAL_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(
        manifest["sliceId"],
        "direct-actions-builtin-internal-copy-v1"
    );
    assert_eq!(manifest["oracleSession"], "actions-batch-four-plan");
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 4);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-033-actions-file-search-main-row-copy-deeplink-truth",
            "dt-truth-034-actions-file-search-empty-context-global-fallback-truth",
            "dt-truth-035-actions-process-manager-copy-suppresses-stop-kill-truth",
            "dt-truth-036-actions-do-current-app-collapsed-alias-copy-truth",
        ]
    );
}

#[test]
fn direct_actions_builtin_internal_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-033-actions-file-search-main-row-copy-deeplink-truth",
            DT_033_BUILTIN_INTERNAL,
        ),
        (
            "dt-truth-034-actions-file-search-empty-context-global-fallback-truth",
            DT_034_BUILTIN_INTERNAL,
        ),
        (
            "dt-truth-035-actions-process-manager-copy-suppresses-stop-kill-truth",
            DT_035_BUILTIN_INTERNAL,
        ),
        (
            "dt-truth-036-actions-do-current-app-collapsed-alias-copy-truth",
            DT_036_BUILTIN_INTERNAL,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["oracleSession"], "actions-batch-four-plan");
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "systemPasteboardChanged",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }

        let primitive_commands = receipt["primitiveReceipts"]
            .as_array()
            .expect("primitiveReceipts must be array")
            .iter()
            .filter_map(|entry| entry["command"].as_str())
            .collect::<Vec<_>>();
        for expected in [
            "targets.inspect",
            "act.select",
            "act.open-actions",
            "actions.inspect",
            "act.set-input",
            "act.key",
        ] {
            assert!(
                primitive_commands.contains(&expected),
                "{expected_id} missing primitive command coverage {expected}; got {primitive_commands:?}"
            );
        }
    }
}

#[test]
fn direct_actions_builtin_internal_records_expected_truth_checks() {
    let expected = [
        (
            DT_033_BUILTIN_INTERNAL,
            vec![
                "fileSearchParentSelectionStable",
                "fileSearchDeepLinkCopyNamesSearchFiles",
                "fileSearchRejectsRevealAndAgentChatStaleCopy",
                "copyDeepFilterRecoveryRestoresDeepLinkAction",
                "fileSearchDeepLinkSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_034_BUILTIN_INTERNAL,
            vec![
                "fileSearchEmptyContextShowsGlobalFallback",
                "fileSearchEmptyContextSuppressesFileActions",
                "copyPathFilterHasNoExecutableAction",
                "revealFilterHasNoExecutableAction",
                "fileSearchEmptyFallbackSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_035_BUILTIN_INTERNAL,
            vec![
                "processManagerParentSelectionStable",
                "processManagerCopyNamesInspectionOnly",
                "processManagerKillFilterHasNoExecutableAction",
                "processManagerStopFilterHasNoExecutableAction",
                "processManagerSubmitBlockedWithoutAllowSubmit",
            ],
        ),
        (
            DT_036_BUILTIN_INTERNAL,
            vec![
                "doInCurrentAppCollapsedAliasVisible",
                "currentAppCommandsAndTurnIntoCommandNotShownAsSeparateActions",
                "appCommandsFilterHasNoExecutableAction",
                "turnThisFilterHasNoExecutableAction",
                "doInCurrentAppSubmitBlockedWithoutAllowSubmit",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_builtin_internal_records_copy_submit_and_allowlist_truth() {
    let file_search = parse(DT_033_BUILTIN_INTERNAL);
    assert_eq!(file_search["truthModel"]["visibleLabel"], "Copy Deep Link");
    assert_eq!(file_search["truthModel"]["actionId"], "copy_deeplink");
    assert_eq!(
        file_search["truthModel"]["parentSubjectId"],
        "choice:8:search-files"
    );

    let empty_file_search = parse(DT_034_BUILTIN_INTERNAL);
    assert_eq!(empty_file_search["truthModel"]["visibleLabel"], "Show Logs");
    assert_eq!(empty_file_search["truthModel"]["actionId"], "view_logs");
    assert_eq!(
        empty_file_search["truthModel"]["sideEffectClass"],
        "global-safe-fallback"
    );

    let process = parse(DT_035_BUILTIN_INTERNAL);
    assert_eq!(
        process["truthModel"]["visibleLabel"],
        "Open Process Manager"
    );
    assert_eq!(
        process["truthModel"]["sideEffectClass"],
        "inspection-only-open"
    );

    let current_app = parse(DT_036_BUILTIN_INTERNAL);
    assert_eq!(
        current_app["truthModel"]["visibleLabel"],
        "Do in Current App"
    );
    assert_eq!(
        current_app["truthModel"]["handlerId"],
        "builtin-do-in-current-app"
    );
}

#[test]
fn direct_devtools_non_destructive_launcher_submit_uses_named_allowlist() {
    let source = include_str!("../scripts/devtools/act.ts");
    for expected in [
        "nonDestructiveLauncherSubmitIds",
        "\"search-files\"",
        "isNonDestructiveLauncherSubmit",
        "isScriptListTargetReceipt",
        "submit requires ActionsDialog target or non-destructive launcher allowlist",
    ] {
        assert!(
            source.contains(expected),
            "direct DevTools submit safety should expose named launcher allowlist state {expected}"
        );
    }
}

#[test]
fn direct_actions_nondestructive_builtins_copy_slice_has_exact_scenarios_and_no_runner() {
    let manifest = parse(NONDESTRUCTIVE_BUILTINS_COPY_MANIFEST);
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(
        manifest["sliceId"],
        "direct-actions-nondestructive-builtins-copy-v1"
    );
    assert_eq!(manifest["oracleSession"], "actions-slice-retry-plan");
    assert_eq!(
        manifest["scenarioLedgerOracleSession"],
        "actions-slice-retry-plan"
    );
    assert_eq!(manifest["executor"], "direct-devtools-primitives");
    assert_eq!(manifest["hasRunner"], false);
    assert_eq!(manifest["forbiddenExecutorsUsed"], false);
    assert_eq!(manifest["summary"]["pass"], 3);
    assert_eq!(manifest["summary"]["fail"], 0);
    assert_eq!(manifest["summary"]["blockedByMissingPrimitive"], 0);
    assert_eq!(manifest["summary"]["blockedByUnsafeOperation"], 0);

    let scenario_ids = manifest["scenarioIds"]
        .as_array()
        .expect("scenarioIds must be an array")
        .iter()
        .map(|value| value.as_str().expect("scenario id must be string"))
        .collect::<Vec<_>>();
    assert_eq!(
        scenario_ids,
        vec![
            "dt-truth-037-actions-app-launcher-copy-deeplink-truth",
            "dt-truth-038-actions-emoji-copy-deeplink-truth",
            "dt-truth-040-actions-notes-copy-deeplink-truth",
        ]
    );
}

#[test]
fn direct_actions_nondestructive_builtins_copy_receipts_have_truth_schema_safety_and_primitives() {
    for (expected_id, raw) in [
        (
            "dt-truth-037-actions-app-launcher-copy-deeplink-truth",
            DT_037_NONDESTRUCTIVE_BUILTINS_COPY,
        ),
        (
            "dt-truth-038-actions-emoji-copy-deeplink-truth",
            DT_038_NONDESTRUCTIVE_BUILTINS_COPY,
        ),
        (
            "dt-truth-040-actions-notes-copy-deeplink-truth",
            DT_040_NONDESTRUCTIVE_BUILTINS_COPY,
        ),
    ] {
        for forbidden in FORBIDDEN_EXECUTORS {
            assert!(
                !raw.contains(forbidden),
                "{expected_id} must not reference forbidden executor {forbidden}"
            );
        }

        let receipt = parse(raw);
        assert_eq!(receipt["schemaVersion"], 1);
        assert_eq!(receipt["scenarioId"], expected_id);
        assert_eq!(receipt["oracleSession"], "actions-slice-retry-plan");
        assert_eq!(receipt["result"], "pass");
        assert_eq!(receipt["executor"], "direct-devtools-primitives");
        assert_eq!(receipt["executorProvenance"]["hasRunner"], false);

        let truth_model = receipt["truthModel"]
            .as_object()
            .expect("truthModel must be object");
        for field in REQUIRED_TRUTH_MODEL_FIELDS {
            assert!(
                truth_model.contains_key(*field),
                "{expected_id} missing truthModel.{field}"
            );
        }

        for safety_field in [
            "destructiveOperationObserved",
            "filesystemMutationOutsideSandbox",
            "externalActivation",
        ] {
            assert_eq!(
                receipt["safety"][safety_field], false,
                "{expected_id} must preserve non-destructive safety field {safety_field}"
            );
        }
        assert_eq!(receipt["safety"]["submitAllowed"], true);
        assert_eq!(receipt["safety"]["submitAttempted"], true);
        assert_eq!(receipt["safety"]["nativeEscalation"], false);
        assert_eq!(receipt["safety"]["allowedClassification"], "ok");
        assert_eq!(
            receipt["safety"]["submitLifecycleState"],
            "source-closed-parent-live"
        );

        let commands = receipt["executorProvenance"]["topLevelCommands"]
            .as_array()
            .expect("executorProvenance.topLevelCommands must be array");
        assert!(!commands.is_empty(), "{expected_id} must record commands");
        for command in commands {
            let script = command_script(&command["argv"])
                .expect("each command argv must include a script path");
            assert!(
                ALLOWED_PRIMITIVES.contains(&script),
                "{expected_id} used non-allowed command path {script}"
            );
        }

        let primitive_commands = receipt["primitiveReceipts"]
            .as_array()
            .expect("primitiveReceipts must be array")
            .iter()
            .filter_map(|entry| entry["command"].as_str())
            .collect::<Vec<_>>();
        for expected in [
            "targets.inspect",
            "act.set-input",
            "act.select",
            "act.open-actions",
            "actions.inspect",
            "act.key",
        ] {
            assert!(
                primitive_commands.contains(&expected),
                "{expected_id} missing primitive command coverage {expected}; got {primitive_commands:?}"
            );
        }
    }
}

#[test]
fn direct_actions_nondestructive_builtins_copy_records_expected_truth_checks() {
    let expected = [
        (
            DT_037_NONDESTRUCTIVE_BUILTINS_COPY,
            vec![
                "launchpadParentSelectionStable",
                "launchpadCopyDeepLinkVisible",
                "launchpadRejectsKillStaleCopy",
                "launchpadCopySubmitAllowedByNamedPair",
                "launchpadCopySubmitKeepsParentInspectable",
            ],
        ),
        (
            DT_038_NONDESTRUCTIVE_BUILTINS_COPY,
            vec![
                "emojiParentSelectionStable",
                "emojiCopyDeepLinkVisible",
                "emojiRejectsPasteStaleCopy",
                "emojiKeyboardPrimitiveDoesNotChangeActionTruth",
                "emojiCopySubmitAllowedByNamedPair",
            ],
        ),
        (
            DT_040_NONDESTRUCTIVE_BUILTINS_COPY,
            vec![
                "openNotesParentSelectionStable",
                "openNotesCopyDeepLinkVisible",
                "openNotesRejectsSendStaleCopy",
                "openNotesKeyboardPrimitiveDoesNotChangeActionTruth",
                "openNotesCopySubmitAllowedByNamedPair",
            ],
        ),
    ];

    for (raw, names) in expected {
        let receipt = parse(raw);
        let checks = receipt["truthChecks"]
            .as_array()
            .expect("truthChecks must be array");
        for name in names {
            assert!(
                checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "pass"),
                "{} missing passing truth check {name}",
                receipt["scenarioId"]
            );
        }
    }
}

#[test]
fn direct_actions_nondestructive_builtins_copy_records_exact_parent_action_pairs() {
    let expected = [
        (
            DT_037_NONDESTRUCTIVE_BUILTINS_COPY,
            "Launchpad",
            "choice:44:launchpad",
            "builtin-launchpad",
        ),
        (
            DT_038_NONDESTRUCTIVE_BUILTINS_COPY,
            "Emoji Picker",
            "choice:35:emoji-picker",
            "builtin-emoji-picker",
        ),
        (
            DT_040_NONDESTRUCTIVE_BUILTINS_COPY,
            "Open Notes",
            "choice:2:open-notes",
            "builtin-open-notes",
        ),
    ];

    for (raw, parent_text, parent_id, handler_id) in expected {
        let receipt = parse(raw);
        assert_eq!(receipt["truthModel"]["visibleLabel"], "Copy Deep Link");
        assert_eq!(receipt["truthModel"]["actionId"], "copy_deeplink");
        assert_eq!(receipt["truthModel"]["parentSubjectText"], parent_text);
        assert_eq!(receipt["truthModel"]["parentSubjectId"], parent_id);
        assert_eq!(receipt["truthModel"]["handlerId"], handler_id);
        assert_eq!(receipt["truthModel"]["sideEffectClass"], "deeplink-copy");
        assert_eq!(receipt["safety"]["systemPasteboardChanged"], true);
    }
}

#[test]
fn direct_devtools_actions_dialog_copy_submit_uses_parent_action_allowlist() {
    let source = include_str!("../scripts/devtools/act.ts");
    for expected in [
        "nonDestructiveActionsDialogSubmitPairs",
        "parentFocusForTarget",
        "selectedSubjectFromFocus",
        "isNonDestructiveActionsDialogSubmit",
        "submit requires named non-destructive ActionsDialog parent/action allowlist",
        "{ parentText: \"Launchpad\", actionId: \"copy_deeplink\" }",
        "{ parentText: \"Emoji Picker\", actionId: \"copy_deeplink\" }",
        "{ parentText: \"Design Gallery\", actionId: \"copy_deeplink\" }",
        "{ parentText: \"Open Notes\", actionId: \"copy_deeplink\" }",
        "pair.actionId === actionId && pair.parentText === parentSubjectText",
    ] {
        assert!(
            source.contains(expected),
            "direct DevTools ActionsDialog submit safety should expose exact parent/action allowlist state {expected}"
        );
    }
}

#[test]
fn direct_devtools_agent_handoff_submit_requires_scoped_dry_run_receipt_gate() {
    let source = include_str!("../scripts/devtools/act.ts");
    for expected in [
        "isScopedAgentHandoffDryRun",
        "agent-handoff-dry-run",
        "prompt-target/cmux-codex",
        "agent_chat:handoff:cmux_codex",
        "prompt-action/export-file",
        "prompt-action/export-gist",
        "prompt-action/copy-prompt",
        "SCRIPT_KIT_AGENT_HANDOFF_DRY_RUN",
        "SCRIPT_KIT_AGENT_HANDOFF_RECEIPT_PATH",
        "submitIntent:agent-handoff-dry-run",
        "parentTarget?.targetKind === \"AgentChat\"",
        "parentTarget?.targetKind === \"ScriptList\"",
        "parentTarget?.targetKind === \"ActionsDialog\"",
    ] {
        assert!(
            source.contains(expected),
            "direct DevTools agent handoff submit safety should require the scoped dry-run receipt gate {expected}"
        );
    }
}
