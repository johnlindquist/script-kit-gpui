#[path = "about_route.rs"]
mod about_route;
#[path = "actions_dialog.rs"]
mod actions_dialog;
#[path = "actions_toggle.rs"]
mod actions_toggle;
#[path = "agent_chat_surface_transitions.rs"]
mod agent_chat_surface_transitions;
mod agent_handoff;
#[path = "alias_input.rs"]
mod alias_input;
#[cfg(test)]
#[path = "tests.rs"]
mod app_impl_state_sync_tests;
#[path = "attachment_portal.rs"]
mod attachment_portal;
#[path = "automation_surface.rs"]
mod automation_surface;
mod background_effects;
#[path = "chat_actions.rs"]
mod chat_actions;
#[path = "execution_paths.rs"]
mod execution_paths;
#[path = "execution_scripts.rs"]
mod execution_scripts;
#[path = "feedback_route.rs"]
mod feedback_route;
#[path = "filter_input_change.rs"]
mod filter_input_change;
#[path = "filter_input_core.rs"]
mod filter_input_core;
#[path = "filter_input_updates.rs"]
mod filter_input_updates;
#[path = "filtering_cache.rs"]
mod filtering_cache;
#[path = "lifecycle_reset.rs"]
mod lifecycle_reset;
pub(crate) use lifecycle_reset::MainWindowPostHide;
#[path = "main_list_loading.rs"]
pub(crate) mod main_list_loading;
#[path = "menu_syntax_actions.rs"]
pub(crate) mod menu_syntax_actions;
#[path = "menu_syntax_ai.rs"]
pub(crate) mod menu_syntax_ai;
#[path = "menu_syntax_ai_apply.rs"]
pub(crate) mod menu_syntax_ai_apply;
#[path = "menu_syntax_main_hint.rs"]
mod menu_syntax_main_hint;
#[path = "menu_syntax_object_selector_main_list.rs"]
mod menu_syntax_object_selector_main_list;
#[path = "menu_syntax_trigger_picker.rs"]
mod menu_syntax_trigger_picker;
#[path = "menu_syntax_trigger_picker_main_list.rs"]
mod menu_syntax_trigger_picker_main_list;
#[path = "naming_dialog.rs"]
mod naming_dialog;
#[path = "path_action.rs"]
pub(crate) mod path_action;
#[path = "profile_search_view.rs"]
mod profile_search_view;
#[path = "prompt_ai.rs"]
mod prompt_ai;
#[path = "quick_terminal_warm.rs"]
mod quick_terminal_warm;
#[path = "refresh_scriptlets.rs"]
mod refresh_scriptlets;
#[path = "registries_state.rs"]
mod registries_state;
#[path = "root_brain_inbox.rs"]
mod root_brain_inbox;
#[path = "root_brain_search.rs"]
mod root_brain_search;
#[path = "root_file_search.rs"]
mod root_file_search;
#[path = "root_unified_result_actions.rs"]
pub(crate) mod root_unified_result_actions;
#[path = "routes.rs"]
pub(crate) mod routes;
#[path = "selection_fallback.rs"]
mod selection_fallback;
#[path = "shortcut_recorder.rs"]
mod shortcut_recorder;
#[path = "shortcuts_hud_grid.rs"]
mod shortcuts_hud_grid;
#[path = "simulate_key_dispatch.rs"]
mod simulate_key_dispatch;
/// Core ScriptListApp implementation: startup, event handling, UI wiring, and state management.

#[path = "startup.rs"]
mod startup;
#[path = "submit_diagnostics.rs"]
mod submit_diagnostics;
#[path = "theme_focus.rs"]
mod theme_focus;
#[path = "trigger_builtin_dispatch.rs"]
mod trigger_builtin_dispatch;
#[path = "ui_window.rs"]
mod ui_window;
#[path = "webcam_actions.rs"]
mod webcam_actions;
#[allow(dead_code)]
#[path = "window_orchestrator_bridge.rs"]
mod window_orchestrator_bridge;

pub(crate) use shortcuts_hud_grid::GlobalShortcutEscape;
pub(crate) use startup::calculate_fallback_error_message;
