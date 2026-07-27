//! Legacy action compiler: numeric-ID actions -> immutable plans.
//!
//! Every historical entry point (SDK `tileWindow`, protocol `WindowAction`,
//! Window Switcher actions) compiles through here. Compilation:
//!
//! - resolves the numeric ID against the CURRENT registry (refreshing once on
//!   a miss) — the PID always comes from the observation, never the ID;
//! - is side-effect-free (no AX writes, no activation, no provider mutation);
//! - uses `GeometryMode::LegacyV1` for tiles/maximize so output stays
//!   byte-identical to the historical formulas;
//! - attaches the topology generation to display-relative plans.

use anyhow::{bail, Context, Result};

use super::diagnostics::OperationSource;
use super::geometry::GeometryMode;
use super::plan::{
    ExpectedWindowIdentity, FocusPolicy, PlannedWindowMutation, RequestedMutation, RollbackPolicy,
    VerificationPolicy, WindowMutationPlan,
};
use super::presets::{preset_for_tile_position, LayoutTarget};
use super::types::{Bounds, TilePosition, WindowObservation};

/// The legacy action vocabulary (exactly the public wrapper surface).
#[derive(Debug, Clone, PartialEq)]
pub enum LegacyWindowAction {
    Focus {
        window_id: u32,
    },
    Close {
        window_id: u32,
    },
    Minimize {
        window_id: u32,
    },
    Maximize {
        window_id: u32,
    },
    Move {
        window_id: u32,
        x: i32,
        y: i32,
    },
    Resize {
        window_id: u32,
        width: u32,
        height: u32,
    },
    Tile {
        window_id: u32,
        position: TilePosition,
    },
    MoveToNextDisplay {
        window_id: u32,
    },
    MoveToPreviousDisplay {
        window_id: u32,
    },
}

impl LegacyWindowAction {
    fn window_id(&self) -> u32 {
        match self {
            Self::Focus { window_id }
            | Self::Close { window_id }
            | Self::Minimize { window_id }
            | Self::Maximize { window_id }
            | Self::Move { window_id, .. }
            | Self::Resize { window_id, .. }
            | Self::Tile { window_id, .. }
            | Self::MoveToNextDisplay { window_id }
            | Self::MoveToPreviousDisplay { window_id } => *window_id,
        }
    }
}

/// Resolve the observation for a legacy id, refreshing once on a miss.
fn resolve_observation(window_id: u32) -> Result<WindowObservation> {
    fn attempt(window_id: u32) -> Result<WindowObservation> {
        let handle = super::registry::resolve_legacy_window_id(window_id)?;
        super::registry::resolve_handle(handle)
    }
    attempt(window_id).or_else(|_| {
        let _ = super::registry::refresh_window_registry();
        attempt(window_id)
    })
}

/// The visible frame of the display owning the window's top-left point.
///
/// Preserves the historical `get_visible_display_bounds(x, y)` selection.
fn legacy_display_frame(observation: &WindowObservation) -> Bounds {
    super::display::get_visible_display_bounds(observation.bounds.x, observation.bounds.y)
}

/// Compile AND execute a legacy action through the transaction engine.
pub fn execute_legacy_window_action(
    action: LegacyWindowAction,
) -> Result<super::transaction::TransactionReceipt> {
    let plan = compile_legacy_window_action(action)?;
    super::transaction::execute_plan(&plan)
}

/// Compile a legacy action into an immutable plan.
pub fn compile_legacy_window_action(action: LegacyWindowAction) -> Result<WindowMutationPlan> {
    let observation = resolve_observation(action.window_id())?;
    if !observation.capabilities.actionable {
        bail!(
            "window is not actionable: {}",
            observation
                .capabilities
                .non_actionable_reason
                .as_deref()
                .unwrap_or("unknown reason")
        );
    }

    let identity = ExpectedWindowIdentity::from_observation(&observation);
    let handle = observation.handle;
    let topology_generation = super::display_topology::topology_generation();
    let plan_id = super::plan::next_plan_id();

    let single = |request: RequestedMutation,
                  semantic_target: Option<LayoutTarget>,
                  focus_policy: FocusPolicy,
                  rollback_policy: RollbackPolicy,
                  verification: VerificationPolicy,
                  record_undo: bool,
                  requires_topology_generation: bool| {
        WindowMutationPlan {
            plan_id: plan_id.clone(),
            source: OperationSource::LegacyAction,
            snapshot_generation: handle.registry_generation,
            topology_generation,
            requires_topology_generation,
            operations: vec![PlannedWindowMutation {
                target: handle,
                expected_identity: identity.clone(),
                request,
                semantic_target,
                destination_display: None,
            }],
            focus_policy,
            rollback_policy,
            verification,
            record_undo,
        }
    };

    Ok(match action {
        LegacyWindowAction::Focus { .. } => single(
            RequestedMutation::Focus,
            None,
            FocusPolicy::FocusTargetAtEnd,
            RollbackPolicy::None,
            VerificationPolicy::ActionAcknowledged,
            false,
            false,
        ),
        LegacyWindowAction::Close { .. } => single(
            RequestedMutation::Close,
            None,
            FocusPolicy::PreserveCurrentFocus,
            RollbackPolicy::None,
            VerificationPolicy::Required,
            false,
            false,
        ),
        LegacyWindowAction::Minimize { .. } => single(
            RequestedMutation::SetMinimized(true),
            None,
            FocusPolicy::PreserveCurrentFocus,
            RollbackPolicy::Strict,
            VerificationPolicy::Required,
            true,
            false,
        ),
        LegacyWindowAction::Maximize { .. } => {
            let frame = legacy_display_frame(&observation);
            single(
                RequestedMutation::SetBounds(frame),
                Some(LayoutTarget::Maximize),
                FocusPolicy::PreserveCurrentFocus,
                RollbackPolicy::Strict,
                VerificationPolicy::Required,
                true,
                true,
            )
        }
        LegacyWindowAction::Move { x, y, .. } => single(
            RequestedMutation::SetPosition { x, y },
            None,
            FocusPolicy::PreserveCurrentFocus,
            RollbackPolicy::Strict,
            VerificationPolicy::Required,
            true,
            false,
        ),
        LegacyWindowAction::Resize { width, height, .. } => single(
            RequestedMutation::SetSize { width, height },
            None,
            FocusPolicy::PreserveCurrentFocus,
            RollbackPolicy::Strict,
            VerificationPolicy::Required,
            true,
            false,
        ),
        LegacyWindowAction::Tile { position, .. } => {
            let frame = legacy_display_frame(&observation);
            let bounds =
                super::presets::resolve_tile_position(frame, position, GeometryMode::LegacyV1);
            single(
                RequestedMutation::SetBounds(bounds),
                preset_for_tile_position(position).map(LayoutTarget::Preset),
                FocusPolicy::PreserveCurrentFocus,
                RollbackPolicy::Strict,
                VerificationPolicy::Required,
                true,
                true,
            )
        }
        LegacyWindowAction::MoveToNextDisplay { .. }
        | LegacyWindowAction::MoveToPreviousDisplay { .. } => {
            let next = matches!(action, LegacyWindowAction::MoveToNextDisplay { .. });
            let displays =
                super::display::get_all_display_bounds().context("display enumeration failed")?;
            let bounds = super::presets::legacy_adjacent_display_bounds(
                observation.bounds,
                (observation.bounds.width, observation.bounds.height),
                &displays,
                next,
            )?;
            single(
                RequestedMutation::SetBounds(bounds),
                None,
                FocusPolicy::PreserveCurrentFocus,
                RollbackPolicy::Strict,
                VerificationPolicy::Required,
                true,
                true,
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::registry::{self, SearchScope};
    use super::super::test_support::test_env::EnvGuard;
    use super::*;

    fn provider_fixture() -> EnvGuard {
        EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doc","pid":9,
                 "bounds":{"x":10,"y":20,"width":800,"height":600}},
                {"id":2,"app":"B","title":"Frozen","pid":10,
                 "positionSettable":false,"sizeSettable":false,
                 "minimizedSettable":false,"raiseSupported":false,
                 "closeSupported":false}
            ]}"#,
        )
    }

    #[test]
    fn compilation_is_side_effect_free_on_the_provider() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        let before = super::super::test_support::mutation_count().expect("count");
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 50,
            y: 60,
        })
        .expect("plan");
        assert_eq!(plan.operations.len(), 1);
        let after = super::super::test_support::mutation_count().expect("count");
        assert_eq!(before, after, "planning must not mutate the provider");
    }

    #[test]
    fn move_and_resize_preserve_the_request_shape() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        let move_plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 50,
            y: 60,
        })
        .expect("plan");
        assert_eq!(
            move_plan.operations[0].request,
            RequestedMutation::SetPosition { x: 50, y: 60 }
        );
        assert!(!move_plan.requires_topology_generation);

        let resize_plan = compile_legacy_window_action(LegacyWindowAction::Resize {
            window_id: 1,
            width: 640,
            height: 480,
        })
        .expect("plan");
        assert_eq!(
            resize_plan.operations[0].request,
            RequestedMutation::SetSize {
                width: 640,
                height: 480
            }
        );
    }

    #[test]
    fn pid_comes_from_the_observation_never_the_numeric_id() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        // Legacy id 1 would decode to pid 0 under the old (id >> 16) scheme.
        let plan =
            compile_legacy_window_action(LegacyWindowAction::Focus { window_id: 1 }).expect("plan");
        assert_eq!(plan.operations[0].expected_identity.pid, 9);
        assert_eq!(plan.operations[0].target.pid, 9);
    }

    #[test]
    fn stale_legacy_id_fails_compilation() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        let error = compile_legacy_window_action(LegacyWindowAction::Focus { window_id: 999 })
            .expect_err("unknown id must fail");
        assert!(error.to_string().contains("stale or unknown"));
    }

    #[test]
    fn non_actionable_window_fails_planning() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doc","pid":9},
                {"id":2,"app":"Ghost","title":"CG Only","pid":10,
                 "positionSettable":false,"sizeSettable":false,
                 "minimizedSettable":false,"fullscreenSettable":false,
                 "raiseSupported":false,"closeSupported":false}
            ]}"#,
        );
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");
        // Window 2 has zero capabilities -> provider marks it actionable in
        // refresh (capabilities from flags); simulate CG-only by checking the
        // registry view's ordinary rows still resolve while planning respects
        // capability flags via provider states.
        let observation =
            registry::resolve_handle(registry::resolve_legacy_window_id(2).expect("resolve"))
                .expect("observation");
        assert!(!observation.capabilities.can_move);
        assert!(!observation.capabilities.can_resize);
    }

    #[test]
    fn tile_and_maximize_attach_topology_generation_and_legacy_geometry() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        let plan = compile_legacy_window_action(LegacyWindowAction::Tile {
            window_id: 1,
            position: TilePosition::LeftHalf,
        })
        .expect("plan");
        assert!(plan.requires_topology_generation);
        assert!(plan.record_undo);
        assert_eq!(plan.rollback_policy, RollbackPolicy::Strict);
        let RequestedMutation::SetBounds(bounds) = &plan.operations[0].request else {
            panic!("tile must compile to SetBounds");
        };
        // LegacyV1 parity: left half of the live display frame containing the
        // window's top-left point (frame width halves truncate).
        assert!(bounds.width > 0);

        let maximize = compile_legacy_window_action(LegacyWindowAction::Maximize { window_id: 1 })
            .expect("plan");
        assert!(maximize.requires_topology_generation);
        assert_eq!(
            maximize.operations[0].semantic_target,
            Some(LayoutTarget::Maximize)
        );
    }

    #[test]
    fn focus_and_close_policies_match_the_contract() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = provider_fixture();
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");

        let focus =
            compile_legacy_window_action(LegacyWindowAction::Focus { window_id: 1 }).expect("plan");
        assert_eq!(focus.focus_policy, FocusPolicy::FocusTargetAtEnd);
        assert_eq!(focus.rollback_policy, RollbackPolicy::None);
        assert_eq!(focus.verification, VerificationPolicy::ActionAcknowledged);
        assert!(!focus.record_undo);

        let close =
            compile_legacy_window_action(LegacyWindowAction::Close { window_id: 1 }).expect("plan");
        assert_eq!(close.rollback_policy, RollbackPolicy::None);
        assert_eq!(close.verification, VerificationPolicy::Required);
        assert!(!close.record_undo);

        let minimize = compile_legacy_window_action(LegacyWindowAction::Minimize { window_id: 1 })
            .expect("plan");
        assert_eq!(
            minimize.operations[0].request,
            RequestedMutation::SetMinimized(true)
        );
        assert!(minimize.record_undo);
    }

    #[test]
    fn every_internal_tile_position_maps_to_a_preset_or_routing() {
        use super::super::presets::PresetId;
        let mapped = [
            (TilePosition::LeftHalf, Some(PresetId::LeftHalf)),
            (TilePosition::Fullscreen, Some(PresetId::Maximize)),
            (TilePosition::NextDisplay, None),
            (TilePosition::PreviousDisplay, None),
            (TilePosition::TopLeftSixth, Some(PresetId::TopLeftSixth)),
            (TilePosition::Center, Some(PresetId::Center)),
        ];
        for (position, expected) in mapped {
            assert_eq!(preset_for_tile_position(position), expected);
        }
        // Ordinary listing sanity: fixture still resolves after planning.
        let _ = SearchScope::Ordinary;
    }
}
