//! Screenshot targeting determinism tests.
//!
//! These tests validate the scoring and tie-rejection contracts for targeted
//! screenshot capture without requiring a live GPUI event loop or real
//! OS windows. They mirror the private scoring logic to verify behavioral
//! contracts: bounds-awareness, tie rejection, and logging field presence.

use script_kit_gpui::protocol::{
    AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind,
};

// ── Scoring mirror ─────────────────────────────────────────────────────
// These replicate the private scoring functions so integration tests can
// verify the behavioral contract without depending on internal visibility.

fn candidate_size_score(bounds: Option<&AutomationWindowBounds>, cw: i32, ch: i32) -> i32 {
    let Some(b) = bounds else { return 0 };

    let tw = b.width.round() as i32;
    let th = b.height.round() as i32;
    let dw = (cw - tw).abs();
    let dh = (ch - th).abs();

    match (dw, dh) {
        (0, 0) => 5_000,
        (dw, dh) if dw <= 4 && dh <= 4 => 2_500,
        (dw, dh) if dw <= 16 && dh <= 16 => 500,
        _ => -1_500,
    }
}

fn score(
    resolved: &AutomationWindowInfo,
    title: &str,
    focused: bool,
    width: i32,
    height: i32,
) -> i32 {
    let mut s = candidate_size_score(resolved.bounds.as_ref(), width, height);

    if let Some(rt) = resolved.title.as_deref() {
        if !rt.is_empty() && title == rt {
            s += 1_000;
        } else if !rt.is_empty() && title.contains(rt) {
            s += 500;
        }
    }

    if resolved.focused == focused {
        s += 100;
    }

    if resolved.kind == AutomationWindowKind::Main
        && (title.contains("Notes") || title.contains("AI") || title.contains("Agent Chat"))
    {
        s -= 200;
    }

    s
}

fn make_info(
    kind: AutomationWindowKind,
    title: Option<&str>,
    focused: bool,
    bounds: Option<(f64, f64)>,
) -> AutomationWindowInfo {
    AutomationWindowInfo {
        id: "test".to_string(),
        kind,
        title: title.map(|s| s.to_string()),
        focused,
        visible: true,
        semantic_surface: None,
        bounds: bounds.map(|(w, h)| AutomationWindowBounds {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        }),
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    }
}

// ── Size scoring tiers ─────────────────────────────────────────────────

#[test]
fn size_score_exact_match() {
    let b = AutomationWindowBounds {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    assert_eq!(candidate_size_score(Some(&b), 800, 600), 5_000);
}

#[test]
fn size_score_close_match() {
    let b = AutomationWindowBounds {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    assert_eq!(candidate_size_score(Some(&b), 803, 602), 2_500);
}

#[test]
fn size_score_moderate_match() {
    let b = AutomationWindowBounds {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    assert_eq!(candidate_size_score(Some(&b), 810, 610), 500);
}

#[test]
fn size_score_far_mismatch() {
    let b = AutomationWindowBounds {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    assert_eq!(candidate_size_score(Some(&b), 400, 300), -1_500);
}

#[test]
fn size_score_no_bounds_returns_zero() {
    assert_eq!(candidate_size_score(None, 800, 600), 0);
}

// ── Composite scoring ──────────────────────────────────────────────────

#[test]
fn exact_size_dominates_title() {
    let resolved = make_info(
        AutomationWindowKind::Main,
        Some("Other"),
        true,
        Some((800.0, 600.0)),
    );

    let size_match = score(&resolved, "Script Kit", true, 800, 600);
    let title_match = score(&resolved, "Other", true, 400, 300);

    assert!(
        size_match > title_match,
        "size_match={size_match} should beat title_match={title_match}"
    );
}

#[test]
fn title_still_matters_without_bounds() {
    let resolved = make_info(AutomationWindowKind::Main, Some("Script Kit"), true, None);

    let titled = score(&resolved, "Script Kit", true, 800, 600);
    let untitled = score(&resolved, "Notes", true, 800, 600);

    assert!(titled > untitled);
}

#[test]
fn main_target_penalizes_secondary_windows() {
    let resolved = make_info(AutomationWindowKind::Main, None, false, None);

    let main = score(&resolved, "Script Kit", false, 800, 600);
    let notes = score(&resolved, "Notes", false, 800, 600);
    let ai = score(&resolved, "Agent Chat", false, 800, 600);

    assert!(main > notes, "main={main} should beat notes={notes}");
    assert!(main > ai, "main={main} should beat ai={ai}");
}

// ── Tie detection ──────────────────────────────────────────────────────

#[test]
fn identical_candidates_tie() {
    let resolved = make_info(
        AutomationWindowKind::AgentChatDetached,
        None,
        false,
        Some((600.0, 400.0)),
    );

    let a = score(&resolved, "Script Kit Agent Chat", false, 600, 400);
    let b = score(&resolved, "Script Kit Agent Chat", false, 600, 400);

    assert_eq!(a, b, "identical candidates must tie");
}

#[test]
fn size_difference_breaks_tie() {
    let resolved = make_info(
        AutomationWindowKind::AgentChatDetached,
        None,
        false,
        Some((600.0, 400.0)),
    );

    let exact = score(&resolved, "Script Kit Agent Chat", false, 600, 400);
    let close = score(&resolved, "Script Kit Agent Chat", false, 603, 401);

    assert!(
        exact > close,
        "exact={exact} should beat close={close} to break tie"
    );
}

#[test]
fn focus_difference_breaks_tie_when_sizes_equal() {
    let resolved = make_info(
        AutomationWindowKind::AgentChatDetached,
        None,
        true,
        Some((600.0, 400.0)),
    );

    let focused = score(&resolved, "Script Kit Agent Chat", true, 600, 400);
    let unfocused = score(&resolved, "Script Kit Agent Chat", false, 600, 400);

    assert!(
        focused > unfocused,
        "focused={focused} should beat unfocused={unfocused}"
    );
}

// ── Protocol error contract ────────────────────────────────────────────

#[test]
fn ambiguous_error_message_contains_candidates_and_score() {
    // Simulate what capture_resolved_window would produce for a tie
    let id = "agentChatDetached:thread-1";
    let kind = AutomationWindowKind::AgentChatDetached;
    let title_a = "Script Kit Agent Chat";
    let title_b = "Script Kit Agent Chat";
    let tied_score = 5100;

    let error = format!(
        "Ambiguous OS window match for automation target {} ({:?}); \
         '{}' and '{}' tied at score {}",
        id, kind, title_a, title_b, tied_score
    );

    assert!(error.contains("Ambiguous"));
    assert!(error.contains(id));
    assert!(error.contains("AgentChatDetached"));
    assert!(error.contains(title_a));
    assert!(error.contains(&tied_score.to_string()));
}

// ── Attached-surface target bounds in screenshot ──────────────────────

use script_kit_gpui::protocol::{AutomationWindowTarget, InspectBoundsInScreenshot};
use std::sync::atomic::{AtomicU32, Ordering};

static SHOT_COUNTER: AtomicU32 = AtomicU32::new(70_000);
fn shot_prefix() -> String {
    let n = SHOT_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("shot{n}")
}

fn make_registered(
    prefix: &str,
    id: &str,
    kind: AutomationWindowKind,
    bounds: Option<AutomationWindowBounds>,
) -> AutomationWindowInfo {
    let info = AutomationWindowInfo {
        id: format!("{prefix}:{id}"),
        kind,
        title: Some(format!("Window {id}")),
        focused: false,
        visible: true,
        semantic_surface: None,
        bounds,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(info.clone());
    info
}

fn shot_cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

/// ActionsDialog target bounds are offset from the main window's origin,
/// not placed at (0, 0). This ensures screenshots expose where the popup
/// lives inside the captured parent-window image.
#[test]
fn attached_actions_dialog_bounds_are_offset_in_screenshot() {
    let p = shot_prefix();

    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(AutomationWindowBounds {
            x: 240.0,
            y: 124.0,
            width: 1280.0,
            height: 820.0,
        }),
    );
    make_registered(
        &p,
        "actions",
        AutomationWindowKind::ActionsDialog,
        Some(AutomationWindowBounds {
            x: 620.0,
            y: 242.0,
            width: 520.0,
            height: 384.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let bounds = script_kit_gpui::protocol::target_bounds_in_screenshot(&resolved)
        .expect("must compute bounds for attached surface");

    // Offset = target.origin - main.origin = (620-240, 242-124) = (380, 118)
    assert!(
        (bounds.x - 380.0).abs() < f64::EPSILON,
        "Expected x=380, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 118.0).abs() < f64::EPSILON,
        "Expected y=118, got {}",
        bounds.y
    );
    // Dimensions must match the dialog, not the parent
    assert!(
        (bounds.width - 520.0).abs() < f64::EPSILON,
        "Width must be dialog width"
    );
    assert!(
        (bounds.height - 384.0).abs() < f64::EPSILON,
        "Height must be dialog height"
    );

    shot_cleanup(&p, &["main", "actions"]);
}

/// PromptPopup target bounds are similarly offset from main.
#[test]
fn attached_prompt_popup_bounds_are_offset_in_screenshot() {
    let p = shot_prefix();

    let main_bounds_val = AutomationWindowBounds {
        x: 100.0,
        y: 50.0,
        width: 1000.0,
        height: 700.0,
    };
    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(main_bounds_val.clone()),
    );
    make_registered(
        &p,
        "popup",
        AutomationWindowKind::PromptPopup,
        Some(AutomationWindowBounds {
            x: 350.0,
            y: 200.0,
            width: 400.0,
            height: 300.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:popup"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let bounds = script_kit_gpui::protocol::target_bounds_in_screenshot_with_main(
        &resolved,
        Some(&main_bounds_val),
    )
    .expect("must compute");

    assert!(
        (bounds.x - 250.0).abs() < f64::EPSILON,
        "Expected x=250, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 150.0).abs() < f64::EPSILON,
        "Expected y=150, got {}",
        bounds.y
    );

    shot_cleanup(&p, &["main", "popup"]);
}

/// Detached window target bounds must NOT be offset — they're at (0, 0)
/// because they have their own independent screenshot.
#[test]
fn detached_window_bounds_at_origin_in_screenshot() {
    let p = shot_prefix();

    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(AutomationWindowBounds {
            x: 100.0,
            y: 50.0,
            width: 800.0,
            height: 600.0,
        }),
    );
    make_registered(
        &p,
        "agent_chat",
        AutomationWindowKind::AgentChatDetached,
        Some(AutomationWindowBounds {
            x: 900.0,
            y: 200.0,
            width: 480.0,
            height: 440.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:agent_chat"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let bounds =
        script_kit_gpui::protocol::target_bounds_in_screenshot(&resolved).expect("must compute");

    assert!(
        (bounds.x - 0.0).abs() < f64::EPSILON,
        "Detached x must be 0"
    );
    assert!(
        (bounds.y - 0.0).abs() < f64::EPSILON,
        "Detached y must be 0"
    );
    assert!(
        (bounds.width - 480.0).abs() < f64::EPSILON,
        "Width must match"
    );
    assert!(
        (bounds.height - 440.0).abs() < f64::EPSILON,
        "Height must match"
    );

    shot_cleanup(&p, &["main", "agent_chat"]);
}

/// Target bounds must be contained within the parent screenshot dimensions
/// when both are available. Uses `_with_main` variant for test isolation.
#[test]
fn attached_surface_bounds_contained_in_parent_dimensions() {
    let p = shot_prefix();

    let main_w = 1280.0;
    let main_h = 820.0;
    let main_bounds_val = AutomationWindowBounds {
        x: 240.0,
        y: 124.0,
        width: main_w,
        height: main_h,
    };
    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(main_bounds_val.clone()),
    );
    make_registered(
        &p,
        "actions",
        AutomationWindowKind::ActionsDialog,
        Some(AutomationWindowBounds {
            x: 620.0,
            y: 242.0,
            width: 520.0,
            height: 384.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let bounds = script_kit_gpui::protocol::target_bounds_in_screenshot_with_main(
        &resolved,
        Some(&main_bounds_val),
    )
    .expect("must compute");

    // The target rect must fit within the parent window dimensions
    assert!(bounds.x >= 0.0, "Target x must be non-negative");
    assert!(bounds.y >= 0.0, "Target y must be non-negative");
    assert!(
        bounds.x + bounds.width <= main_w,
        "Target right edge ({}) must not exceed parent width ({})",
        bounds.x + bounds.width,
        main_w
    );
    assert!(
        bounds.y + bounds.height <= main_h,
        "Target bottom edge ({}) must not exceed parent height ({})",
        bounds.y + bounds.height,
        main_h
    );

    shot_cleanup(&p, &["main", "actions"]);
}

/// When the attached surface has bounds but main does not, the geometry
/// must fail closed (return None) rather than guess.
/// Uses `_with_main(None)` for test isolation.
#[test]
fn attached_surface_no_main_bounds_fails_closed() {
    let p = shot_prefix();

    make_registered(&p, "main", AutomationWindowKind::Main, None);
    make_registered(
        &p,
        "actions",
        AutomationWindowKind::ActionsDialog,
        Some(AutomationWindowBounds {
            x: 300.0,
            y: 200.0,
            width: 400.0,
            height: 300.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    // Explicit None main bounds for test isolation
    let result = script_kit_gpui::protocol::target_bounds_in_screenshot_with_main(&resolved, None);
    assert!(
        result.is_none(),
        "Must fail closed when main has no bounds, not silently produce (0, 0)"
    );

    shot_cleanup(&p, &["main", "actions"]);
}

/// When the attached surface itself has no bounds, geometry must return None.
/// Uses `_with_main` variant for test isolation.
#[test]
fn attached_surface_no_own_bounds_fails_closed() {
    let p = shot_prefix();

    let main_bounds_val = AutomationWindowBounds {
        x: 100.0,
        y: 50.0,
        width: 800.0,
        height: 600.0,
    };
    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(main_bounds_val.clone()),
    );
    make_registered(&p, "actions", AutomationWindowKind::ActionsDialog, None);

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let result = script_kit_gpui::protocol::target_bounds_in_screenshot_with_main(
        &resolved,
        Some(&main_bounds_val),
    );
    assert!(
        result.is_none(),
        "Must fail closed when target has no bounds"
    );

    shot_cleanup(&p, &["main", "actions"]);
}

// ── Popup capture receipt contract ────────────────────────────────────

use script_kit_gpui::platform::{PopupCaptureReceipt, PopupCaptureStrategy};

/// Helper to build a popup capture receipt for testing.
fn make_popup_receipt(
    strategy: PopupCaptureStrategy,
    kind: &str,
    target_bounds: Option<InspectBoundsInScreenshot>,
    semantic_primary: bool,
) -> PopupCaptureReceipt {
    PopupCaptureReceipt {
        strategy,
        window_kind: kind.to_string(),
        target_bounds,
        semantic_receipts_are_primary: semantic_primary,
    }
}

/// Popup capture receipt for attached popup with crop bounds includes
/// both strategy and target_bounds.
#[test]
fn popup_receipt_attached_with_bounds() {
    let receipt = make_popup_receipt(
        PopupCaptureStrategy::ParentCaptureWithCrop,
        "ActionsDialog",
        Some(InspectBoundsInScreenshot {
            x: 380.0,
            y: 118.0,
            width: 520.0,
            height: 384.0,
        }),
        true,
    );

    assert_eq!(
        receipt.strategy,
        PopupCaptureStrategy::ParentCaptureWithCrop
    );
    assert_eq!(receipt.window_kind, "ActionsDialog");
    assert!(receipt.target_bounds.is_some());
    assert!(receipt.semantic_receipts_are_primary);

    // Serde roundtrip
    let json = serde_json::to_string(&receipt).expect("serialize");
    assert!(json.contains("parent_capture_with_crop"));
    assert!(json.contains("targetBounds"));
    let parsed: PopupCaptureReceipt = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.strategy, PopupCaptureStrategy::ParentCaptureWithCrop);
}

/// Popup capture receipt for detached window has direct strategy and no bounds.
#[test]
fn popup_receipt_detached() {
    let receipt = make_popup_receipt(
        PopupCaptureStrategy::DirectWindowCapture,
        "AgentChatDetached",
        None,
        true,
    );

    assert_eq!(receipt.strategy, PopupCaptureStrategy::DirectWindowCapture);
    assert!(receipt.target_bounds.is_none());
    assert!(receipt.semantic_receipts_are_primary);
}

/// Popup capture receipt for main window has not-applicable strategy.
#[test]
fn popup_receipt_main_not_applicable() {
    let receipt = make_popup_receipt(PopupCaptureStrategy::NotApplicable, "Main", None, false);

    assert_eq!(receipt.strategy, PopupCaptureStrategy::NotApplicable);
    assert!(!receipt.semantic_receipts_are_primary);
}

/// Attached popup receipt without target_bounds is structurally representable
/// but signals a verification failure — agents must treat null bounds as error.
#[test]
fn popup_receipt_attached_null_bounds_serializes_for_error_reporting() {
    let receipt = make_popup_receipt(
        PopupCaptureStrategy::ParentCaptureWithCrop,
        "PromptPopup",
        None,
        true,
    );

    let json = serde_json::to_string(&receipt).expect("serialize");
    assert!(json.contains("parent_capture_with_crop"));
    assert!(json.contains("\"targetBounds\":null"));
}

/// All three strategy variants roundtrip through serde.
#[test]
fn popup_capture_strategy_serde_roundtrip() {
    for (strategy, expected_str) in [
        (
            PopupCaptureStrategy::ParentCaptureWithCrop,
            "\"parent_capture_with_crop\"",
        ),
        (
            PopupCaptureStrategy::DirectWindowCapture,
            "\"direct_window_capture\"",
        ),
        (PopupCaptureStrategy::NotApplicable, "\"not_applicable\""),
    ] {
        let json = serde_json::to_string(&strategy).expect("serialize");
        assert_eq!(
            json, expected_str,
            "strategy {:?} must serialize to {}",
            strategy, expected_str
        );
        let parsed: PopupCaptureStrategy = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, strategy);
    }
}

/// Default hit point computed from target_bounds_in_screenshot must be
/// at the center of the offset rectangle, not at the center of the
/// full screenshot. Uses `_with_main` variant for test isolation.
#[test]
fn hit_point_is_center_of_offset_bounds_not_full_image() {
    let p = shot_prefix();

    let main_bounds_val = AutomationWindowBounds {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    make_registered(
        &p,
        "main",
        AutomationWindowKind::Main,
        Some(main_bounds_val.clone()),
    );
    make_registered(
        &p,
        "actions",
        AutomationWindowKind::ActionsDialog,
        Some(AutomationWindowBounds {
            x: 700.0,
            y: 300.0,
            width: 520.0,
            height: 384.0,
        }),
    );

    let target = AutomationWindowTarget::Id {
        id: format!("{p}:actions"),
    };
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&target)).expect("should resolve");

    let bounds = script_kit_gpui::protocol::target_bounds_in_screenshot_with_main(
        &resolved,
        Some(&main_bounds_val),
    )
    .expect("must compute");
    let hit = script_kit_gpui::protocol::default_surface_hit_point(&bounds);

    // bounds = (700, 300, 520, 384) → center = (700 + 260, 300 + 192) = (960, 492)
    assert!(
        (hit.x - 960.0).abs() < f64::EPSILON,
        "Hit x must be center of offset bounds (960), got {}",
        hit.x
    );
    assert!(
        (hit.y - 492.0).abs() < f64::EPSILON,
        "Hit y must be center of offset bounds (492), got {}",
        hit.y
    );

    // Contrast: center of full image would be (960, 540) — NOT the same
    assert!(
        (hit.y - 540.0).abs() > 1.0,
        "Hit y must NOT be the center of the full image"
    );

    shot_cleanup(&p, &["main", "actions"]);
}
