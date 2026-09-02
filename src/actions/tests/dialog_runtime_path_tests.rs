use super::dialog::{ActionsDialog, ActionsDialogRoute, GroupedActionItem};
use super::types::{
    Action, ActionCallback, ActionCategory, ActionsDialogConfig, SearchPosition, SectionStyle,
};
use crate::{protocol::ProtocolAction, theme};
use gpui::{App, AppContext, Entity};
use gpui_platform::headless;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn sample_action(id: &str, title: &str, section: Option<&str>) -> Action {
    let mut action = Action::new(id, title, None, ActionCategory::ScriptContext);
    if let Some(section_name) = section {
        action = action.with_section(section_name);
    }
    action
}

fn run_headless_dialog_test(test_fn: impl FnOnce(&mut App) + 'static) {
    let did_run = Arc::new(Mutex::new(false));
    let did_run_for_app = Arc::clone(&did_run);

    headless().run(move |cx| {
        test_fn(cx);
        *did_run_for_app
            .lock()
            .expect("runtime dialog test run marker lock poisoned") = true;
        cx.quit();
    });

    assert!(
        *did_run
            .lock()
            .expect("runtime dialog test completion lock poisoned"),
        "headless dialog test closure did not execute"
    );
}

fn build_dialog_entity(
    cx: &mut App,
    actions: Vec<Action>,
    config: ActionsDialogConfig,
    selected_ids: Arc<Mutex<Vec<String>>>,
) -> Entity<ActionsDialog> {
    let on_select: ActionCallback = {
        let selected_ids = Arc::clone(&selected_ids);
        Arc::new(move |action_id| {
            selected_ids
                .lock()
                .expect("runtime dialog callback lock poisoned")
                .push(action_id);
        })
    };
    let theme = Arc::new(theme::Theme::default());

    cx.new(move |entity_cx| {
        ActionsDialog::with_config(entity_cx.focus_handle(), on_select, actions, theme, config)
    })
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn test_activate_selected_emits_action_id_when_item_is_selected() {
    let selected_ids = Arc::new(Mutex::new(Vec::new()));
    let selected_ids_for_test = Arc::clone(&selected_ids);

    run_headless_dialog_test(move |cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![
                sample_action("action_alpha", "Alpha", None),
                sample_action("action_beta", "Beta", None),
            ],
            ActionsDialogConfig::default(),
            Arc::clone(&selected_ids_for_test),
        );

        cx.update_entity(&dialog, |dialog, entity_cx| {
            dialog.selected_index = Some(1);
            assert!(matches!(
                dialog.activate_selected(entity_cx),
                crate::actions::ActionsDialogActivation::Executed {
                    action_id,
                    should_close: true,
                } if action_id == "action_beta"
            ));
        });
    });

    assert_eq!(
        *selected_ids
            .lock()
            .expect("submit_selected assertion lock poisoned"),
        vec!["action_beta".to_string()]
    );
}

#[gpui::test]
fn disabled_action_blocks_selected_and_direct_activation_without_callback(
    cx: &mut gpui::TestAppContext,
) {
    let selected_ids = Arc::new(Mutex::new(Vec::new()));
    let dialog = cx.update(|cx| {
        build_dialog_entity(
            cx,
            vec![sample_action("disabled", "Unavailable", None)
                .with_shortcut("⌘D")
                .disabled("Requires a selected file")],
            ActionsDialogConfig::default(),
            Arc::clone(&selected_ids),
        )
    });

    let (selected_outcome, direct_outcome, selected_index) = cx.update(|cx| {
        dialog.update(cx, |dialog, entity_cx| {
            let selected_outcome = dialog.activate_selected(entity_cx);
            let direct_outcome = dialog.activate_action_id("disabled".to_string(), entity_cx);
            (selected_outcome, direct_outcome, dialog.selected_index)
        })
    });

    for outcome in [selected_outcome, direct_outcome] {
        assert!(matches!(
            outcome,
            crate::actions::ActionsDialogActivation::Blocked {
                action_id,
                reason,
            } if action_id == "disabled" && reason == "Requires a selected file"
        ));
    }
    assert_eq!(selected_index, Some(0));
    assert!(selected_ids
        .lock()
        .expect("disabled callback lock")
        .is_empty());
}

#[gpui::test]
fn direct_activation_uses_the_activated_actions_close_policy(cx: &mut gpui::TestAppContext) {
    let selected_ids = Arc::new(Mutex::new(Vec::new()));
    let dialog = cx.update(|cx| {
        build_dialog_entity(
            cx,
            Vec::new(),
            ActionsDialogConfig::default(),
            Arc::clone(&selected_ids),
        )
    });

    let outcome = cx.update(|cx| {
        dialog.update(cx, |dialog, entity_cx| {
            let mut stays_open = ProtocolAction::new("stays_open".to_string());
            stays_open.close = Some(false);
            let mut closes = ProtocolAction::new("closes".to_string());
            closes.close = Some(true);
            dialog.set_sdk_actions(vec![stays_open, closes]);

            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("stays_open")
            );
            dialog.activate_action_id("closes".to_string(), entity_cx)
        })
    });

    assert!(matches!(
        outcome,
        crate::actions::ActionsDialogActivation::Executed {
            action_id,
            should_close: true,
        } if action_id == "closes"
    ));
    assert_eq!(
        *selected_ids.lock().expect("direct callback lock"),
        vec!["closes".to_string()]
    );
}

#[gpui::test]
fn refresh_restores_identity_then_uses_nearest_eligible_row(cx: &mut gpui::TestAppContext) {
    let dialog = cx.update(|cx| {
        build_dialog_entity(
            cx,
            vec![
                sample_action("a", "A", None),
                sample_action("b", "B", None),
                sample_action("c", "C", None),
            ],
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        )
    });

    cx.update(|cx| {
        dialog.update(cx, |dialog, entity_cx| {
            assert_eq!(
                dialog.select_action_by_id("b", entity_cx),
                Some("b".to_string())
            );
            dialog.replace_actions_for_test(vec![
                sample_action("x", "X", None),
                sample_action("a", "A", None),
                sample_action("b", "B", None).disabled("Temporarily unavailable"),
                sample_action("c", "C", None),
            ]);
            assert_eq!(dialog.get_selected_action_id().as_deref(), Some("b"));
            assert_eq!(dialog.selected_index, Some(2));
            assert!(!dialog
                .get_selected_action()
                .expect("selected b")
                .is_enabled());

            dialog.replace_actions_for_test(vec![
                sample_action("x", "X", None),
                sample_action("a", "A", None),
                sample_action("c", "C", None),
            ]);
            assert_eq!(dialog.get_selected_action_id().as_deref(), Some("c"));
            assert_eq!(dialog.selected_index, Some(2));

            dialog.replace_actions_for_test(Vec::new());
            assert_eq!(dialog.selected_index, None);
            assert_eq!(dialog.get_selected_action_id(), None);
        });
    });
}

#[gpui::test]
fn empty_actions_dialog_has_no_selected_row_and_cannot_activate(cx: &mut gpui::TestAppContext) {
    let dialog = cx.update(|cx| {
        build_dialog_entity(
            cx,
            Vec::new(),
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        )
    });

    let (selected_index, outcome) = cx.update(|cx| {
        dialog.update(cx, |dialog, entity_cx| {
            (dialog.selected_index, dialog.activate_selected(entity_cx))
        })
    });
    assert_eq!(selected_index, None);
    assert!(matches!(
        outcome,
        crate::actions::ActionsDialogActivation::NoSelection
    ));
}

#[gpui::test]
fn actions_search_edits_use_input_state_cursor_selection_and_history(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(gpui_component::init);
    let dialog_slot = Rc::new(RefCell::new(None));
    let dialog_slot_for_window = Rc::clone(&dialog_slot);
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), move |window, cx| {
            let dialog = build_dialog_entity(
                cx,
                vec![sample_action("reopen", "Réopen File", None)],
                ActionsDialogConfig::default(),
                Arc::new(Mutex::new(Vec::new())),
            );
            dialog.update(cx, |dialog, cx| {
                dialog.ensure_search_input(window, cx);
            });
            *dialog_slot_for_window.borrow_mut() = Some(dialog);
            cx.new(|_| gpui::Empty)
        })
        .expect("Actions search test window opens")
    });
    let dialog = dialog_slot
        .borrow()
        .clone()
        .expect("Actions search dialog was installed");

    window
        .update(cx, |_empty, window, cx| {
            dialog.update(cx, |dialog, cx| {
                assert!(dialog.insert_search_text("réopen-file", window, cx));
                for _ in 0..5 {
                    assert!(dialog.move_search_cursor_left(false, window, cx));
                }
                assert!(dialog.insert_search_text("!", window, cx));
                assert_eq!(dialog.search_text, "réopen!-file");

                assert!(dialog.move_search_cursor_left(false, window, cx));
                assert!(dialog.move_search_cursor_right(true, window, cx));
                assert!(dialog.insert_search_text("?", window, cx));
                assert_eq!(dialog.search_text, "réopen?-file");

                assert!(dialog.undo_search_input(window, cx));
                assert_eq!(dialog.search_text, "");
                assert!(dialog.redo_search_input(window, cx));
                assert_eq!(dialog.search_text, "réopen?-file");
                assert_eq!(dialog.filtered_actions.len(), 0);

                assert!(dialog.select_all_search_input(window, cx));
                assert!(dialog.insert_search_text("a😀é界z", window, cx));
                for _ in 0..4 {
                    assert!(dialog.move_search_cursor_left(false, window, cx));
                }
                assert!(dialog.delete_search_input(window, cx));
                assert_eq!(dialog.search_text, "aé界z");
                assert!(dialog.delete_search_input(window, cx));
                assert_eq!(dialog.search_text, "a界z");
                assert!(dialog.move_search_cursor_right(true, window, cx));
                assert!(dialog.delete_search_input(window, cx));
                assert_eq!(dialog.search_text, "az");
                assert!(dialog.backspace_search_input(window, cx));
                assert_eq!(dialog.search_text, "z");
                assert!(dialog.delete_search_input(window, cx));
                assert_eq!(dialog.search_text, "");
                assert!(dialog.delete_search_input(window, cx));
                assert_eq!(dialog.search_text, "");

                assert!(dialog.insert_search_text("alpha beta", window, cx));
                for _ in 0..4 {
                    assert!(dialog.move_search_cursor_left(false, window, cx));
                }
                assert!(dialog.delete_next_search_word(window, cx));
                assert_eq!(dialog.search_text, "alpha ");
                assert!(dialog.select_all_search_input(window, cx));
                assert!(dialog.delete_next_search_word(window, cx));
                assert_eq!(dialog.search_text, "");
            });
        })
        .expect("Actions search test window remains available");
}

#[gpui::test]
fn ux13_opening_shell_uses_root_unfiltered_actions_through_filter_and_routes(
    cx: &mut gpui::TestAppContext,
) {
    let root_actions = vec![
        sample_action("alpha", "Alpha", Some("One")),
        sample_action("beta", "Beta", Some("One")),
        sample_action("gamma", "Gamma", Some("Two")),
    ];
    let dialog = cx.update(|cx| {
        build_dialog_entity(
            cx,
            root_actions.clone(),
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        )
    });

    cx.update(|cx| {
        dialog.update(cx, |dialog, entity_cx| {
            dialog.set_root_route(ActionsDialogRoute {
                id: "root".to_string(),
                actions: root_actions,
                context_title: Some("Root".to_string()),
                search_placeholder: None,
                initial_selected_action_id: Some("alpha".to_string()),
            });
            let opening = dialog.opening_shell_sizing_snapshot();
            assert_eq!(opening.route_id.as_deref(), Some("root"));
            assert_eq!(opening.action_count, 3);
            assert_eq!(opening.section_header_count, 2);
            assert!(opening.search_visible);
            assert!(opening.context_header_visible);

            dialog.attach_to_fixed_shell(300.0);
            dialog.set_search_text("no-match".to_string(), entity_cx);
            assert!(dialog.filtered_actions.is_empty());
            assert_eq!(dialog.fixed_shell_height_px(), Some(300.0));
            assert_eq!(dialog.opening_shell_sizing_snapshot(), opening);

            dialog.push_route(
                ActionsDialogRoute {
                    id: "child".to_string(),
                    actions: vec![sample_action("child", "Child", None)],
                    context_title: Some("Child".to_string()),
                    search_placeholder: None,
                    initial_selected_action_id: Some("child".to_string()),
                },
                entity_cx,
            );
            assert_eq!(dialog.route_depth(), 2);
            assert_eq!(dialog.opening_shell_sizing_snapshot(), opening);
            assert_eq!(dialog.fixed_shell_height_px(), Some(300.0));

            dialog.release_fixed_shell();
            assert_eq!(dialog.fixed_shell_height_px(), None);
        });
    });
}

#[gpui::test]
fn ux13_hidden_search_never_installs_or_focuses_input(cx: &mut gpui::TestAppContext) {
    cx.update(gpui_component::init);
    let dialog_slot = Rc::new(RefCell::new(None));
    let dialog_slot_for_window = Rc::clone(&dialog_slot);
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), move |window, cx| {
            let dialog = build_dialog_entity(
                cx,
                vec![sample_action("alpha", "Alpha", None)],
                ActionsDialogConfig {
                    search_position: SearchPosition::Hidden,
                    ..ActionsDialogConfig::default()
                },
                Arc::new(Mutex::new(Vec::new())),
            );
            dialog.update(cx, |dialog, cx| {
                dialog.ensure_search_input(window, cx);
            });
            *dialog_slot_for_window.borrow_mut() = Some(dialog);
            cx.new(|_| gpui::Empty)
        })
        .expect("hidden Actions test window opens")
    });
    let dialog = dialog_slot
        .borrow()
        .clone()
        .expect("hidden dialog installed");

    window
        .update(cx, |_empty, window, cx| {
            dialog.update(cx, |dialog, cx| {
                assert!(!dialog.search_is_visible());
                assert!(dialog.search_input.is_none());
                assert!(!dialog.focus_search_input(window, cx));
                assert!(!dialog.search_input_is_focused(window, cx));
                assert!(!dialog.opening_shell_sizing_snapshot().search_visible);
            });
        })
        .expect("hidden Actions test window remains available");
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn test_submit_cancel_does_emit_cancel_sentinel_when_cancel_is_triggered() {
    let selected_ids = Arc::new(Mutex::new(Vec::new()));
    let selected_ids_for_test = Arc::clone(&selected_ids);

    run_headless_dialog_test(move |cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![sample_action("action_alpha", "Alpha", None)],
            ActionsDialogConfig::default(),
            Arc::clone(&selected_ids_for_test),
        );

        cx.update_entity(&dialog, |dialog, _| {
            dialog.submit_cancel();
        });
    });

    assert_eq!(
        *selected_ids
            .lock()
            .expect("submit_cancel assertion lock poisoned"),
        vec!["__cancel__".to_string()]
    );
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn test_move_navigation_does_skip_headers_when_moving_up_and_down() {
    run_headless_dialog_test(|cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![
                sample_action("action_one", "One", Some("Scripts")),
                sample_action("action_two", "Two", Some("Scripts")),
                sample_action("action_three", "Three", Some("Global")),
            ],
            ActionsDialogConfig {
                section_style: SectionStyle::Headers,
                ..ActionsDialogConfig::default()
            },
            Arc::new(Mutex::new(Vec::new())),
        );

        cx.update_entity(&dialog, |dialog, entity_cx| {
            assert_eq!(dialog.grouped_items.len(), 5);
            assert!(matches!(
                dialog.grouped_items.first(),
                Some(GroupedActionItem::SectionHeader(section)) if section == "Scripts"
            ));
            assert!(matches!(
                dialog.grouped_items.get(3),
                Some(GroupedActionItem::SectionHeader(section)) if section == "Global"
            ));
            assert_eq!(dialog.selected_index, Some(1));

            dialog.selected_index = Some(2);
            dialog.move_down(entity_cx);
            assert_eq!(dialog.selected_index, Some(4));

            dialog.move_up(entity_cx);
            assert_eq!(dialog.selected_index, Some(2));

            dialog.selected_index = Some(1);
            dialog.move_up(entity_cx);
            assert_eq!(dialog.selected_index, Some(1));
        });
    });
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn test_search_model_replacement_updates_results_and_empty_state() {
    run_headless_dialog_test(|cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![
                sample_action("action_alpha", "Alpha", None),
                sample_action("action_beta", "Beta", None),
                sample_action("action_gamma", "Gamma", None),
            ],
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        );

        cx.update_entity(&dialog, |dialog, entity_cx| {
            assert_eq!(dialog.search_text, "");
            assert_eq!(dialog.filtered_actions.len(), 3);

            dialog.set_search_text("b".to_string(), entity_cx);
            assert_eq!(dialog.search_text, "b");
            assert_eq!(dialog.filtered_actions.len(), 1);
            assert_eq!(
                dialog
                    .get_selected_action()
                    .expect("expected selected action after searching 'b'")
                    .id,
                "action_beta"
            );

            dialog.set_search_text("be".to_string(), entity_cx);
            assert_eq!(dialog.search_text, "be");
            assert_eq!(dialog.filtered_actions.len(), 1);

            dialog.set_search_text(String::new(), entity_cx);
            assert_eq!(dialog.search_text, "");
            assert_eq!(dialog.filtered_actions.len(), 3);
        });
    });
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn typing_snaps_to_first_scored_action_even_when_previous_identity_still_matches() {
    run_headless_dialog_test(|cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![
                sample_action("delete_script", "Delete Script?", Some("Destructive")),
                sample_action("open_finder", "Open in Finder", Some("Share")),
                sample_action(
                    "save_filter",
                    "Save del filter as named search",
                    Some("Power Syntax"),
                ),
            ],
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        );

        cx.update_entity(&dialog, |dialog, entity_cx| {
            dialog
                .select_action_by_id("save_filter", entity_cx)
                .expect("fixture action should be selectable");
            dialog.set_search_text("del".to_string(), entity_cx);

            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("delete_script"),
                "typing must select the first scored selectable row"
            );
        });
    });
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn navigated_selection_survives_config_driven_grouped_row_refresh() {
    run_headless_dialog_test(|cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![
                sample_action("delete_script", "Delete Script?", Some("Destructive")),
                sample_action(
                    "delete_ranking",
                    "Delete Ranking Entry",
                    Some("Destructive"),
                ),
            ],
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        );

        cx.update_entity(&dialog, |dialog, entity_cx| {
            dialog.move_down(entity_cx);
            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("delete_ranking")
            );

            dialog.set_config(ActionsDialogConfig {
                section_style: SectionStyle::Separators,
                ..ActionsDialogConfig::default()
            });

            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("delete_ranking"),
                "host/config refresh must preserve an explicitly navigated identity"
            );
        });
    });
}

#[test]
#[cfg_attr(target_os = "macos", ignore = "requires main thread (run via GPUI)")]
fn test_actions_dialog_defaults_to_matching_main_window_background() {
    run_headless_dialog_test(|cx| {
        let dialog = build_dialog_entity(
            cx,
            vec![sample_action("action_alpha", "Alpha", None)],
            ActionsDialogConfig::default(),
            Arc::new(Mutex::new(Vec::new())),
        );

        cx.update_entity(&dialog, |dialog, _| {
            assert!(
                dialog.match_main_window_background,
                "ActionsDialog should default to matching the main window background"
            );
        });
    });
}

#[test]
fn test_actions_dialog_source_defaults_to_matching_main_window_background() {
    let source = include_str!("../dialog.rs");

    assert!(
        source.contains("match_main_window_background: true,"),
        "ActionsDialog constructor should default to matching the main window background"
    );
}
