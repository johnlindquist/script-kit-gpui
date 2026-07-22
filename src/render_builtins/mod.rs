// Builtin render modules split by builtin type.
// Pure design-contract resolvers shared with the lib-side token exporter —
// the SAME files are re-exported from `src/lib.rs` via `#[path]` modules
// (the `path_action` pattern) so `src/design_contract` and
// `cargo test --lib` consume exactly what the renderers paint with.
include!("builtin_main_input_contract.rs");
include!("settings_contract.rs");
include!("common.rs");
include!("actions.rs");
include!("clipboard.rs");
include!("emoji_picker.rs");
include!("clipboard_preview.rs");
include!("app_launcher.rs");
include!("window_switcher.rs");
include!("browser_tabs.rs");
include!("window_actions.rs");
include!("theme_chooser.rs");
include!("file_search.rs");
include!("profile_search.rs");
include!("kit_store.rs");
include!("migrate_v1.rs");
include!("process_manager.rs");
include!("flow_ux.rs");
include!("current_app_commands.rs");
include!("ai_presets.rs");
include!("settings.rs");
include!("permissions_wizard.rs");
include!("favorites.rs");
include!("agent_chat_history.rs");
include!("browser_history.rs");
include!("dictation_history.rs");
include!("notes_browse.rs");
include!("sdk_reference.rs");
include!("tips.rs");
include!("script_templates.rs");
