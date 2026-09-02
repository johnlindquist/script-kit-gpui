//! Fixture data and controls for the actual secondary production factories.
use super::fixture_ids::SECONDARY_FIXTURE_IDS;
use crate::{
    actions::{Action, ActionCategory, ActionsDialog, ActionsDialogRoute},
    confirm::window::{
        open_confirm_popup_window, ConfirmPopupParentWindow, ConfirmWindowOptions,
        ParentDialogResult,
    },
    runtime_policy::WindowHostPolicy,
    window_control::{
        snap_overlay::open_snap_overlay_window,
        snap_session::{SnapOverlayModel, SnapOverlayTarget},
    },
};
use anyhow::{ensure, Result};
use gpui::{point, px, size, AnyView, AnyWindowHandle, App, AppContext, Entity};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub(crate) struct SecondaryFixtureParent {
    pub id: String,
    pub generation: u64,
    pub handle: AnyWindowHandle,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecondaryActionReceipt {
    pub delivered_action_ids: Vec<String>,
    pub failure: Option<String>,
}

pub(crate) enum SecondaryFixtureControls {
    Actions {
        dialog: Entity<ActionsDialog>,
        receipt: Arc<Mutex<SecondaryActionReceipt>>,
    },
    Confirm {
        result: async_channel::Receiver<ParentDialogResult>,
        observed: RefCell<Option<ParentDialogResult>>,
    },
    Hud {
        id: u64,
        view: Entity<crate::hud_manager::HudView>,
    },
    Snap {
        view: Entity<crate::window_control::SnapOverlayView>,
    },
}

pub(crate) struct MountedSecondaryFixture {
    pub handle: AnyWindowHandle,
    pub root: AnyView,
    pub automation_id: String,
    pub generation: u64,
    pub controls: SecondaryFixtureControls,
}

impl SecondaryFixtureControls {
    pub(crate) fn action_receipt(&self) -> Result<SecondaryActionReceipt> {
        let Self::Actions { receipt, .. } = self else {
            anyhow::bail!("not_an_actions_fixture");
        };
        receipt
            .lock()
            .map(|receipt| receipt.clone())
            .map_err(|_| anyhow::anyhow!("fixture_action_sink_poisoned"))
    }

    /// Retain the actual delivered completion so repeated inspection is stable.
    pub(crate) fn confirm_result(&self) -> Result<Option<ParentDialogResult>> {
        let Self::Confirm { result, observed } = self else {
            anyhow::bail!("not_a_confirm_fixture");
        };
        let mut observed = observed.borrow_mut();
        if observed.is_some() {
            return Ok(*observed);
        }
        match result.try_recv() {
            Ok(value) => {
                *observed = Some(value);
                crate::runtime_policy::record_completed_fixture_effect();
                Ok(Some(value))
            }
            Err(async_channel::TryRecvError::Empty) => Ok(None),
            Err(async_channel::TryRecvError::Closed) => {
                anyhow::bail!("confirm_completion_disconnected")
            }
        }
    }

    pub(crate) fn observation(&self, cx: &App) -> Result<serde_json::Value> {
        Ok(match self {
            Self::Actions { .. } => serde_json::to_value(self.action_receipt()?)?,
            Self::Confirm { .. } => {
                let result = self.confirm_result()?.map(|result| match result {
                    ParentDialogResult::Primary => "primary",
                    ParentDialogResult::Secondary => "secondary",
                    ParentDialogResult::Dismiss => "dismiss",
                    ParentDialogResult::ProgrammaticClose => "programmaticClose",
                });
                serde_json::json!({"result": result, "completed": result.is_some()})
            }
            Self::Hud { id, view } => {
                let view = view.read(cx);
                let (text, label, error, completed) = view.semantic_state();
                serde_json::json!({"hudId": id, "text": text, "actionLabel": label, "actionError": error, "actionCompleted": completed})
            }
            Self::Snap { view } => {
                let view = view.read(cx);
                match view.model() {
                    Some(model) => {
                        serde_json::json!({"mode": format!("{:?}", model.mode), "targets": model.targets.iter().map(|target| serde_json::json!({"tile": format!("{:?}", target.tile), "active": target.active, "bounds": {"x": target.bounds.x, "y": target.bounds.y, "width": target.bounds.width, "height": target.bounds.height}})).collect::<Vec<_>>()})
                    }
                    None => serde_json::json!({"mode": null, "targets": []}),
                }
            }
        })
    }
}

impl MountedSecondaryFixture {
    /// Invoke the real owner teardown, with an exact generation check before singleton access.
    pub(crate) fn close(&self, cx: &mut App) -> Result<()> {
        ensure!(
            crate::windows::get_runtime_window_handle_for_generation(
                &self.automation_id,
                self.generation
            ) == Some(self.handle),
            "secondary_fixture_stale"
        );
        let info = crate::windows::automation_window_by_id(&self.automation_id)
            .ok_or_else(|| anyhow::anyhow!("secondary_fixture_stale"))?;
        crate::windows::automation_surface_collector::close_owned_registered_surface(&info, cx)
    }
}

fn parent_geometry(
    parent: &SecondaryFixtureParent,
    cx: &mut App,
) -> Result<(gpui::Bounds<gpui::Pixels>, Option<gpui::DisplayId>)> {
    ensure!(
        crate::windows::get_runtime_window_handle_for_generation(&parent.id, parent.generation)
            == Some(parent.handle),
        "secondary_parent_stale"
    );
    ensure!(
        crate::windows::runtime_window_host_policy(&parent.id, parent.generation)?
            == WindowHostPolicy::OwnedHidden,
        "secondary_parent_not_owned"
    );
    parent.handle.update(cx, |_, window, cx| {
        (
            window.bounds(),
            window.display(cx).map(|display| display.id()),
        )
    })
}

pub(crate) fn mount_secondary_fixture(
    fixture_id: &str,
    parent: Option<SecondaryFixtureParent>,
    cx: &mut App,
) -> Result<MountedSecondaryFixture> {
    WindowHostPolicy::OwnedHidden.validate()?;
    ensure!(
        SECONDARY_FIXTURE_IDS.contains(&fixture_id),
        "unknown_secondary_fixture"
    );
    let (handle, root, controls) = match fixture_id {
        "secondary.actions" => {
            let parent = parent.ok_or_else(|| anyhow::anyhow!("secondary_parent_required"))?;
            let (bounds, display_id) = parent_geometry(&parent, cx)?;
            let receipt = Arc::new(Mutex::new(SecondaryActionReceipt::default()));
            let sink = receipt.clone();
            let on_select: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |action_id| {
                if let Ok(mut receipt) = sink.lock() {
                    if receipt.delivered_action_ids.len() >= 16 {
                        receipt.failure = Some("fixture_action_sink_full".into());
                        return;
                    }
                    receipt.delivered_action_ids.push(action_id);
                    crate::runtime_policy::record_completed_fixture_effect();
                }
            });
            let action =
                |id: &str, title: &str| Action::new(id, title, None, ActionCategory::ScriptContext);
            let mut actions = vec![
                action("fixture:more", "More actions"),
                action("fixture:execute", "Complete fixture action"),
                action("fixture:disabled", "Unavailable action")
                    .disabled("Unavailable in this fixture"),
            ];
            actions.extend((0..8).map(|index| {
                action(
                    &format!("fixture:item-{index}"),
                    &format!("Review item {index}"),
                )
            }));
            let dialog = cx.new(|cx| {
                let mut dialog = ActionsDialog::with_config(
                    cx.focus_handle(),
                    on_select,
                    actions.clone(),
                    crate::theme::get_theme_snapshot().theme.clone(),
                    crate::actions::ActionsDialogConfig::default(),
                );
                dialog.set_skip_track_focus(true);
                dialog.set_root_route(ActionsDialogRoute {
                    id: "fixture:root".into(),
                    actions,
                    context_title: None,
                    search_placeholder: Some("Search actions".into()),
                    initial_selected_action_id: None,
                });
                dialog.register_drill_down_route(
                    "fixture:more",
                    ActionsDialogRoute {
                        id: "fixture:details".into(),
                        actions: vec![action("fixture:detail-complete", "Complete detail action")],
                        context_title: Some("More actions".into()),
                        search_placeholder: Some("Search details".into()),
                        initial_selected_action_id: None,
                    },
                );
                dialog
            });
            let window = crate::actions::open_actions_window(
                cx,
                crate::actions::ActionsWindowPlacement {
                    parent_window_handle: parent.handle,
                    main_bounds: bounds,
                    display_id,
                },
                dialog.clone(),
                crate::actions::WindowPosition::BottomRight,
                Some(&parent.id),
                WindowHostPolicy::OwnedHidden,
            )?;
            (
                window.into(),
                window.entity(cx)?.into(),
                SecondaryFixtureControls::Actions { dialog, receipt },
            )
        }
        "secondary.confirm" | "secondary.confirm-three-button" => {
            let parent = parent.ok_or_else(|| anyhow::anyhow!("secondary_parent_required"))?;
            let (bounds, display_id) = parent_geometry(&parent, cx)?;
            let (sender, result) = async_channel::bounded(1);
            let window = open_confirm_popup_window(
                cx,
                ConfirmPopupParentWindow {
                    handle: parent.handle,
                    bounds,
                    display_id,
                    automation_id: Some(parent.id),
                },
                ConfirmWindowOptions {
                    title: "Confirm fixture action".into(),
                    body: "This local action changes only the owned fixture.".into(),
                    confirm_text: "Continue".into(),
                    cancel_text: "Cancel".into(),
                    secondary_text: (fixture_id == "secondary.confirm-three-button")
                        .then(|| "Keep editing".into()),
                    confirm_variant: gpui_component::button::ButtonVariant::Primary,
                    width: px(360.0),
                },
                Rc::new(|| true),
                sender,
                WindowHostPolicy::OwnedHidden,
            )?;
            (
                window.into(),
                window.entity(cx)?.into(),
                SecondaryFixtureControls::Confirm {
                    result,
                    observed: RefCell::new(None),
                },
            )
        }
        "secondary.hud" | "secondary.hud-action" => {
            let has_action = fixture_id == "secondary.hud-action";
            let mounted = crate::hud_manager::open_hud_notification(
                crate::hud_manager::HudNotification {
                    text: if has_action {
                        "External actions remain refused".into()
                    } else {
                        "Fixture saved".into()
                    },
                    duration_ms: 2000,
                    created_at: cx.background_executor().now(),
                    action_label: has_action.then(|| "Open browser".into()),
                    action: has_action.then(|| {
                        crate::hud_manager::HudAction::OpenUrl(
                            "https://example.invalid/owned-fixture".into(),
                        )
                    }),
                },
                crate::hud_manager::HudHostOptions {
                    policy: WindowHostPolicy::OwnedHidden,
                    origin: Some(point(px(100.0), px(100.0))),
                },
                cx,
            )?
            .ok_or_else(|| anyhow::anyhow!("fixture_hud_queued_without_window"))?;
            let view = mounted.window.entity(cx)?;
            (
                mounted.window.into(),
                view.clone().into(),
                SecondaryFixtureControls::Hud {
                    id: mounted.id,
                    view,
                },
            )
        }
        "secondary.snap" => {
            let bounds = gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(500.0)));
            let window = open_snap_overlay_window(bounds, None, WindowHostPolicy::OwnedHidden, cx)?;
            let display = crate::window_control::Bounds {
                x: 0,
                y: 0,
                width: 800,
                height: 500,
            };
            window.update(cx, |view, _, cx| {
                view.set_model(
                    Some(SnapOverlayModel {
                        display_bounds: display,
                        mode: crate::window_control::SnapMode::Simple,
                        is_dominant: true,
                        targets: vec![SnapOverlayTarget {
                            tile: crate::window_control::TilePosition::LeftHalf,
                            bounds: crate::window_control::Bounds {
                                width: 400,
                                ..display
                            },
                            active: true,
                        }],
                    }),
                    cx,
                )
            })?;
            let view = window.entity(cx)?;
            (
                window.into(),
                view.clone().into(),
                SecondaryFixtureControls::Snap { view },
            )
        }
        _ => unreachable!("closed fixture catalogue validated above"),
    };
    let info = crate::windows::list_automation_windows()
        .into_iter()
        .find(|info| {
            info.generation.is_some_and(|generation| {
                crate::windows::get_runtime_window_handle_for_generation(&info.id, generation)
                    == Some(handle)
            })
        })
        .ok_or_else(|| anyhow::anyhow!("secondary_factory_did_not_register"))?;
    let generation = info
        .generation
        .ok_or_else(|| anyhow::anyhow!("secondary_generation_missing"))?;
    Ok(MountedSecondaryFixture {
        handle,
        root,
        automation_id: info.id,
        generation,
        controls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_receipt_survives_repeated_inspection_and_sender_close() {
        let (sender, result) = async_channel::bounded(1);
        let controls = SecondaryFixtureControls::Confirm {
            result,
            observed: RefCell::new(None),
        };
        assert_eq!(controls.confirm_result().unwrap(), None);
        sender.try_send(ParentDialogResult::Secondary).unwrap();
        assert_eq!(
            controls.confirm_result().unwrap(),
            Some(ParentDialogResult::Secondary)
        );
        drop(sender);
        assert_eq!(
            controls.confirm_result().unwrap(),
            Some(ParentDialogResult::Secondary)
        );
    }
}
