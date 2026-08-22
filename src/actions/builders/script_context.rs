use super::shared::to_deeplink_name;
use super::types::{Action, ActionCategory, ScriptInfo};
use crate::actions::builders::file_path::open_in_quick_terminal_action;
use crate::designs::icon_variations::IconName;
use itertools::Itertools;
use std::collections::HashSet;
use std::path::Path;

fn has_invalid_script_context_input(script: &ScriptInfo) -> bool {
    script.name.trim().is_empty() || script.action_verb.trim().is_empty()
}

fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut normalized = first.to_uppercase().collect::<String>();
                    normalized.push_str(&chars.as_str().to_lowercase());
                    normalized
                }
                None => String::new(),
            }
        })
        .join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptContextKind {
    App,
    Agent,
    Scriptlet,
    Script,
    Skill,
    BuiltIn,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimaryActionPlan {
    PreserveCatalogActionText,
    VerbForSubject { subject: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PrimaryActionCopy {
    title: String,
    description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptContextPreferenceActionPlan {
    AgentNoPreferenceActions,
    NoShortcutNoAlias,
    ShortcutOnly,
    AliasOnly,
    ShortcutAndAlias,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptContextShareActionPlan {
    PortableShareLink,
    DirectRunDeepLink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FavoriteActionPlan {
    AddToFavorites,
    RemoveFromFavorites,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RankingActionPlan {
    NoRankingAction,
    ResetSuggestedRanking,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScriptContextShareActionCopy {
    title: &'static str,
    description: String,
}

impl FavoriteActionPlan {
    fn from_is_favorite(is_favorite: bool) -> Self {
        if is_favorite {
            Self::RemoveFromFavorites
        } else {
            Self::AddToFavorites
        }
    }

    fn copy(self) -> (&'static str, &'static str) {
        match self {
            Self::AddToFavorites => ("Add to Favorites", "Save this item to your favorites list"),
            Self::RemoveFromFavorites => (
                "Remove from Favorites",
                "Remove this item from your favorites list",
            ),
        }
    }
}

fn favorite_action_copy(is_favorite: bool) -> (&'static str, &'static str) {
    FavoriteActionPlan::from_is_favorite(is_favorite).copy()
}

impl RankingActionPlan {
    fn from_is_suggested(is_suggested: bool) -> Self {
        if is_suggested {
            Self::ResetSuggestedRanking
        } else {
            Self::NoRankingAction
        }
    }

    fn reset_action(self) -> Option<Action> {
        match self {
            Self::NoRankingAction => None,
            Self::ResetSuggestedRanking => Some(
                Action::new(
                    "reset_ranking",
                    "Delete Ranking Entry",
                    Some("Remove this item from Suggested section".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌃⌘R")
                .with_icon(IconName::Trash)
                .with_section("Destructive"),
            ),
        }
    }
}

fn script_context_kind(script: &ScriptInfo) -> ScriptContextKind {
    if script.is_app {
        ScriptContextKind::App
    } else if script.is_agent {
        ScriptContextKind::Agent
    } else if script.is_scriptlet {
        ScriptContextKind::Scriptlet
    } else if script.is_script {
        ScriptContextKind::Script
    } else if script.path.starts_with("skill:") {
        ScriptContextKind::Skill
    } else if script.path.starts_with("builtin:") {
        ScriptContextKind::BuiltIn
    } else {
        ScriptContextKind::Generic
    }
}

fn share_action_plan(script: &ScriptInfo) -> ScriptContextShareActionPlan {
    match script_context_kind(script) {
        ScriptContextKind::Script
        | ScriptContextKind::Scriptlet
        | ScriptContextKind::Agent
        | ScriptContextKind::Skill => ScriptContextShareActionPlan::PortableShareLink,
        ScriptContextKind::App | ScriptContextKind::BuiltIn | ScriptContextKind::Generic => {
            ScriptContextShareActionPlan::DirectRunDeepLink
        }
    }
}

fn preference_action_plan(script: &ScriptInfo) -> ScriptContextPreferenceActionPlan {
    if script.is_agent {
        return ScriptContextPreferenceActionPlan::AgentNoPreferenceActions;
    }

    match (script.shortcut.is_some(), script.alias.is_some()) {
        (false, false) => ScriptContextPreferenceActionPlan::NoShortcutNoAlias,
        (true, false) => ScriptContextPreferenceActionPlan::ShortcutOnly,
        (false, true) => ScriptContextPreferenceActionPlan::AliasOnly,
        (true, true) => ScriptContextPreferenceActionPlan::ShortcutAndAlias,
    }
}

fn share_action_copy(script: &ScriptInfo) -> ScriptContextShareActionCopy {
    match share_action_plan(script) {
        ScriptContextShareActionPlan::PortableShareLink => ScriptContextShareActionCopy {
            title: "Share",
            description: "Copy a portable Script Kit share link to clipboard".to_string(),
        },
        ScriptContextShareActionPlan::DirectRunDeepLink => {
            let deeplink_name = to_deeplink_name(&script.name);
            ScriptContextShareActionCopy {
                title: "Copy Deep Link",
                description: format!("Copy scriptkit://run/{} URL to clipboard", deeplink_name),
            }
        }
    }
}

fn primary_action_plan(kind: ScriptContextKind) -> PrimaryActionPlan {
    match kind {
        ScriptContextKind::BuiltIn => PrimaryActionPlan::PreserveCatalogActionText,
        ScriptContextKind::Script => PrimaryActionPlan::VerbForSubject { subject: "script" },
        ScriptContextKind::App => PrimaryActionPlan::VerbForSubject {
            subject: "application",
        },
        ScriptContextKind::Scriptlet => PrimaryActionPlan::VerbForSubject {
            subject: "scriptlet",
        },
        ScriptContextKind::Agent => PrimaryActionPlan::VerbForSubject { subject: "agent" },
        ScriptContextKind::Skill => PrimaryActionPlan::VerbForSubject { subject: "skill" },
        ScriptContextKind::Generic => PrimaryActionPlan::VerbForSubject { subject: "item" },
    }
}

fn append_shortcut_preference_actions(
    plan: ScriptContextPreferenceActionPlan,
    actions: &mut Vec<Action>,
    destructive_actions: &mut Vec<Action>,
) {
    match plan {
        ScriptContextPreferenceActionPlan::AgentNoPreferenceActions => {}
        ScriptContextPreferenceActionPlan::ShortcutOnly
        | ScriptContextPreferenceActionPlan::ShortcutAndAlias => {
            actions.push(
                Action::new(
                    "update_shortcut",
                    "Edit Keyboard Shortcut",
                    Some("Change the keyboard shortcut for this item".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⇧K")
                .with_icon(IconName::Settings)
                .with_section("Edit"),
            );
            destructive_actions.push(
                Action::new(
                    "remove_shortcut",
                    "Delete Keyboard Shortcut",
                    Some("Remove the keyboard shortcut from this item".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⌥K")
                .with_icon(IconName::Trash)
                .with_section("Destructive"),
            );
        }
        ScriptContextPreferenceActionPlan::NoShortcutNoAlias
        | ScriptContextPreferenceActionPlan::AliasOnly => {
            actions.push(
                Action::new(
                    "add_shortcut",
                    "Add Keyboard Shortcut",
                    Some("Set a keyboard shortcut for this item".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⇧K")
                .with_icon(IconName::Settings)
                .with_section("Edit"),
            );
        }
    }
}

fn append_alias_preference_actions(
    plan: ScriptContextPreferenceActionPlan,
    actions: &mut Vec<Action>,
    destructive_actions: &mut Vec<Action>,
) {
    match plan {
        ScriptContextPreferenceActionPlan::AgentNoPreferenceActions => {}
        ScriptContextPreferenceActionPlan::AliasOnly
        | ScriptContextPreferenceActionPlan::ShortcutAndAlias => {
            actions.push(
                Action::new(
                    "update_alias",
                    "Edit Alias",
                    Some("Change the alias trigger for this item".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⇧A")
                .with_icon(IconName::Settings)
                .with_section("Edit"),
            );
            destructive_actions.push(
                Action::new(
                    "remove_alias",
                    "Delete Alias",
                    Some("Remove the alias trigger from this item".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⌥A")
                .with_icon(IconName::Trash)
                .with_section("Destructive"),
            );
        }
        ScriptContextPreferenceActionPlan::NoShortcutNoAlias
        | ScriptContextPreferenceActionPlan::ShortcutOnly => {
            actions.push(
                Action::new(
                    "add_alias",
                    "Add Alias",
                    Some(
                        "Set an alias trigger for this item (type alias + space to run)"
                            .to_string(),
                    ),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⇧A")
                .with_icon(IconName::Settings)
                .with_section("Edit"),
            );
        }
    }
}

fn primary_action_copy(script: &ScriptInfo) -> PrimaryActionCopy {
    if let Some(primary) = script
        .command_descriptor
        .as_ref()
        .and_then(|descriptor| descriptor.primary_action())
    {
        return PrimaryActionCopy {
            title: primary.title.clone(),
            description: primary.title.clone(),
        };
    }

    match primary_action_plan(script_context_kind(script)) {
        PrimaryActionPlan::PreserveCatalogActionText => PrimaryActionCopy {
            title: script.action_verb.clone(),
            description: script.action_verb.clone(),
        },
        PrimaryActionPlan::VerbForSubject { subject } => PrimaryActionCopy {
            title: title_case_words(&script.action_verb),
            description: format!("{} this {}", script.action_verb, subject),
        },
    }
}

/// Get actions specific to the focused script.
pub fn get_script_context_actions(script: &ScriptInfo) -> Vec<Action> {
    if has_invalid_script_context_input(script) {
        tracing::warn!(
            target: "script_kit::actions",
            builder = "script_context",
            name = %script.name,
            action_verb = %script.action_verb,
            "Invalid script context input; returning empty actions"
        );
        return vec![];
    }

    let mut actions = Vec::new();
    let mut destructive_actions = Vec::new();
    let primary_copy = primary_action_copy(script);
    let preference_plan = preference_action_plan(script);
    let ranking_plan = RankingActionPlan::from_is_suggested(script.is_suggested);

    tracing::debug!(
        target: "script_kit::actions",
        name = %script.name,
        is_script = script.is_script,
        is_scriptlet = script.is_scriptlet,
        is_agent = script.is_agent,
        has_shortcut = script.shortcut.is_some(),
        has_alias = script.alias.is_some(),
        is_suggested = script.is_suggested,
        "Building script context actions"
    );

    let mut primary_action = Action::new(
        "run_script",
        primary_copy.title,
        Some(primary_copy.description),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("↵")
    .with_icon(IconName::PlayFilled)
    .with_section("Actions");
    if let Some(descriptor) = &script.command_descriptor {
        primary_action = primary_action.with_command_contract(descriptor);
    }
    actions.push(primary_action);

    actions.push(
        Action::new(
            "toggle_info",
            "Show Info",
            Some("Toggle detailed info about this item".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘I")
        .with_icon(IconName::File)
        .with_section("Actions"),
    );

    append_shortcut_preference_actions(preference_plan, &mut actions, &mut destructive_actions);
    append_alias_preference_actions(preference_plan, &mut actions, &mut destructive_actions);

    if (script.is_script || script.is_scriptlet || script.is_agent || script.is_app)
        && !script.path.trim().is_empty()
    {
        let (title, description) =
            favorite_action_copy(crate::favorites::is_favorite(&script.path));
        actions.push(
            Action::new(
                "toggle_favorite",
                title,
                Some(description.to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Star)
            .with_section("Edit"),
        );
    }

    if script.is_app {
        // Finder actions
        actions.push(
            Action::new(
                "reveal_in_finder",
                "Show in Finder",
                Some("Reveal application in Finder".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧F")
            .with_icon(IconName::FolderOpen)
            .with_section("Finder"),
        );

        actions.push(open_in_quick_terminal_action(Path::new(&script.path)));

        actions.push(
            Action::new(
                "show_info_in_finder",
                "Show Info in Finder",
                Some("Open Finder info window for this application".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::File)
            .with_section("Finder"),
        );

        actions.push(
            Action::new(
                "show_package_contents",
                "Show Package Contents",
                Some("Open the application bundle Contents folder".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::FolderOpen)
            .with_section("Finder"),
        );

        // Copy actions
        actions.push(
            Action::new(
                "copy_name",
                "Copy Name",
                Some("Copy application name to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘.")
            .with_icon(IconName::Copy)
            .with_section("Copy"),
        );

        actions.push(
            Action::new(
                "copy_path",
                "Copy Path",
                Some("Copy application path to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⇧⌘.")
            .with_icon(IconName::Copy)
            .with_section("Copy"),
        );

        if script.bundle_id.is_some() {
            actions.push(
                Action::new(
                    "copy_bundle_id",
                    "Copy Bundle Identifier",
                    Some("Copy bundle identifier to clipboard".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⇧⌘C")
                .with_icon(IconName::Copy)
                .with_section("Copy"),
            );
        }

        // Process actions
        actions.push(
            Action::new(
                "quit_app",
                "Quit Application",
                Some("Gracefully quit this application".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⇧Q")
            .with_icon(IconName::Close)
            .with_section("Process"),
        );

        actions.push(
            Action::new(
                "restart_app",
                "Restart Application",
                Some("Quit and relaunch this application".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Refresh)
            .with_section("Process"),
        );

        destructive_actions.push(
            Action::new(
                "force_quit_app",
                "Force Quit Application",
                Some("Force quit this application immediately".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Close)
            .with_section("Destructive"),
        );
    }

    // Built-in command rows (identified by their `builtin:<id>` path) get a
    // Copy Command ID action so the id is reachable without opening config.
    if !script.is_script
        && !script.is_scriptlet
        && !script.is_app
        && !script.is_agent
        && script.path.starts_with("builtin:")
    {
        actions.push(
            Action::new(
                "copy_command_id",
                "Copy Command ID",
                Some("Copy this built-in command's id to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⇧⌘.")
            .with_icon(IconName::Copy)
            .with_section("Copy"),
        );
    }

    if script.is_script {
        actions.push(
            Action::new(
                "edit_script",
                "Edit Script",
                Some("Open in $EDITOR".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_icon(IconName::Pencil)
            .with_section("Edit"),
        );

        actions.push(
            Action::new(
                "view_logs",
                "Show Logs",
                Some("Show script execution logs".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘L")
            .with_icon(IconName::File)
            .with_section("Edit"),
        );

        actions.push(
            Action::new(
                "reveal_in_finder",
                "Open in Finder",
                Some("Reveal script file in Finder".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧F")
            .with_icon(IconName::FolderOpen)
            .with_section("Share"),
        );

        actions.push(open_in_quick_terminal_action(Path::new(&script.path)));

        actions.push(
            Action::new(
                "copy_path",
                "Copy Path",
                Some("Copy script path to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );

        actions.push(
            Action::new(
                "copy_content",
                "Copy Content",
                Some("Copy entire file content to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌥C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );

        destructive_actions.push(
            Action::new(
                "delete_script",
                "Delete Script?",
                Some("Move the selected script to Trash".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Trash)
            .with_section("Destructive"),
        );
    }

    if script.is_scriptlet {
        actions.push(
            Action::new(
                "edit_scriptlet",
                "Edit Scriptlet",
                Some("Open the markdown file in $EDITOR".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_icon(IconName::Pencil)
            .with_section("Edit"),
        );

        actions.push(
            Action::new(
                "reveal_scriptlet_in_finder",
                "Open in Finder",
                Some("Reveal scriptlet bundle in Finder".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧F")
            .with_icon(IconName::FolderOpen)
            .with_section("Share"),
        );

        actions.push(open_in_quick_terminal_action(Path::new(&script.path)));

        actions.push(
            Action::new(
                "copy_scriptlet_path",
                "Copy Path",
                Some("Copy scriptlet bundle path to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );

        actions.push(
            Action::new(
                "copy_content",
                "Copy Content",
                Some("Copy entire file content to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌥C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );
    }

    if script.is_agent {
        actions.push(
            Action::new(
                "edit_script",
                "Edit Agent",
                Some("Open the agent file in $EDITOR".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_icon(IconName::Pencil)
            .with_section("Edit"),
        );

        actions.push(
            Action::new(
                "reveal_in_finder",
                "Open in Finder",
                Some("Reveal agent file in Finder".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧F")
            .with_icon(IconName::FolderOpen)
            .with_section("Share"),
        );

        actions.push(open_in_quick_terminal_action(Path::new(&script.path)));

        actions.push(
            Action::new(
                "copy_path",
                "Copy Path",
                Some("Copy agent path to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );

        actions.push(
            Action::new(
                "copy_content",
                "Copy Content",
                Some("Copy entire file content to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌥C")
            .with_icon(IconName::Copy)
            .with_section("Share"),
        );
    }

    let share_copy = share_action_copy(script);
    actions.push(
        Action::new(
            "copy_deeplink",
            share_copy.title,
            Some(share_copy.description),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧D")
        .with_icon(IconName::Copy)
        .with_section("Share"),
    );

    if let Some(reset_action) = ranking_plan.reset_action() {
        destructive_actions.push(reset_action);
    }

    actions.extend(destructive_actions);
    let mut seen_ids = HashSet::new();
    let mut duplicate_ids = Vec::new();
    let deduped_actions: Vec<Action> = actions
        .into_iter()
        .filter(|action| {
            if seen_ids.insert(action.id.clone()) {
                true
            } else {
                duplicate_ids.push(action.id.clone());
                false
            }
        })
        .collect();

    if !duplicate_ids.is_empty() {
        tracing::warn!(
            target: "script_kit::actions",
            name = %script.name,
            duplicate_ids = ?duplicate_ids,
            "Deduplicated overlapping script context action IDs"
        );
    }

    tracing::debug!(
        target: "script_kit::actions",
        action_count = deduped_actions.len(),
        action_ids = ?deduped_actions.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        "Created script context actions"
    );

    deduped_actions
}

/// Predefined global actions surfaced in the Cmd+K dialog when the focused
/// row offers no script-context entries (e.g. on the main script list).
///
/// The actions menu is contextual to the focused item, so this list must stay
/// free of rows that act on some other surface's state. In particular the
/// prompt export/handoff rows ("Export Prompt…", "Send Prompt to…") operate on
/// the Agent Chat composer's built prompt and are owned by
/// `get_agent_chat_actions`; adding them here leaks guaranteed-failure rows
/// into every host (main list, file search, built-ins, …).
pub fn get_global_actions() -> Vec<Action> {
    vec![
        Action::new(
            "reload_scripts",
            "Reload Scripts",
            Some("Re-scan ~/.scriptkit and rebuild the script index".into()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘R"),
        Action::new(
            "settings",
            "Edit Config File",
            Some("Open ~/.scriptkit/config.ts in your editor".into()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘,"),
        Action::new(
            "view_logs",
            "Show Logs",
            Some("Toggle the in-launcher log panel".into()),
            ActionCategory::GlobalOps,
        )
        .with_shortcut("⌘L"),
        Action::new(
            "open_help",
            "Help & User Guide",
            Some("Open the Script Kit guide (~/.scriptkit/GUIDE.md)".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
        Action::new(
            "ask_ai_settings",
            "Change Settings with AI",
            Some("Ask Agent Chat to update ~/.scriptkit/config.ts for you".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
        Action::new(
            "open_settings_menu",
            "Open Settings Menu",
            Some("Theme, dictation, microphone, permissions, window options".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
        Action::new(
            "setup_dictation",
            "Set Up Dictation",
            Some("Check microphone, model, and hotkey readiness".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
        Action::new(
            "check_permissions",
            "Check Permissions",
            Some("Review the macOS permissions Script Kit needs".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
        Action::new(
            "sdk_reference",
            "Browse SDK Reference",
            Some("Search the Script Kit SDK API documentation".into()),
            ActionCategory::GlobalOps,
        )
        .with_section("Discover"),
    ]
}

fn get_prompt_export_actions(config: &crate::config::Config) -> Vec<Action> {
    crate::ai::agent_prompt_handoff::builtin_prompt_actions()
        .into_iter()
        .map(|action| {
            let action_id = action.action_id();
            let icon = match action {
                crate::ai::agent_prompt_handoff::AgentPromptActionId::CopyPrompt => IconName::Copy,
                _ => IconName::File,
            };
            let shortcut = config
                .get_command_shortcut(action_id)
                .map(|shortcut| shortcut.to_shortcut_string());
            Action::new(
                action_id,
                action.title(),
                Some(action.description().to_string()),
                ActionCategory::GlobalOps,
            )
            .with_shortcut_opt(shortcut)
            .with_icon(icon)
            .with_section("Export")
        })
        .collect()
}

fn get_prompt_target_actions(config: &crate::config::Config) -> Vec<Action> {
    crate::ai::agent_prompt_handoff::all_prompt_targets(config)
        .into_iter()
        .map(|target| {
            let action_id = target.action_id();
            let description = match &target {
                crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId::CmuxCodex => {
                    Some("Open cmux with Codex using the current prompt".to_string())
                }
                crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId::Command(target) => {
                    target
                        .description
                        .clone()
                        .or_else(|| Some(format!("Send the current prompt to {}", target.command)))
                }
            };
            let shortcut = config
                .get_command_shortcut(&action_id)
                .map(|shortcut| shortcut.to_shortcut_string());
            Action::new(
                action_id,
                format!("Send Prompt to {}", target.title()),
                description,
                ActionCategory::GlobalOps,
            )
            .with_shortcut_opt(shortcut)
            .with_icon(IconName::ArrowRight)
            .with_section("Handoff")
        })
        .collect()
}

#[allow(dead_code)] // Used by the binary Agent Chat actions surface.
const AGENT_CHAT_SWITCH_MODEL_ACTION_PREFIX: &str = "agent_chat_switch_model:";
const AGENT_CHAT_SWITCH_PROFILE_ACTION_PREFIX: &str = "agent_chat_switch_profile:";

#[allow(dead_code)] // Used by the binary Agent Chat actions surface.
fn agent_chat_switch_model_action_id(model_id: &str) -> String {
    format!("{AGENT_CHAT_SWITCH_MODEL_ACTION_PREFIX}{model_id}")
}

#[allow(dead_code)] // Used by Agent Chat chat action dispatch in the binary target.
pub(crate) fn agent_chat_switch_model_id_from_action(action_id: &str) -> Option<&str> {
    action_id.strip_prefix(AGENT_CHAT_SWITCH_MODEL_ACTION_PREFIX)
}

#[allow(dead_code)] // Used by Agent Chat action dispatch in the binary target.
fn agent_chat_switch_profile_action_id(profile_id: &str) -> String {
    format!("{AGENT_CHAT_SWITCH_PROFILE_ACTION_PREFIX}{profile_id}")
}

#[allow(dead_code)] // Used by Agent Chat action dispatch in the binary target.
pub(crate) fn agent_chat_switch_profile_id_from_action(action_id: &str) -> Option<&str> {
    action_id.strip_prefix(AGENT_CHAT_SWITCH_PROFILE_ACTION_PREFIX)
}

/// Action ID for reviewing session "Allow always" permission grants.
pub(crate) const AGENT_CHAT_REVIEW_APPROVALS_ACTION_ID: &str = "agent_chat_review_approvals";

/// Action ID for starting a fresh thread while the current one keeps
/// streaming in the background.
pub(crate) const AGENT_CHAT_NEW_THREAD_ACTION_ID: &str = "agent_chat_new_thread";

const AGENT_CHAT_SWITCH_THREAD_ACTION_PREFIX: &str = "agent_chat_switch_thread:";

fn agent_chat_switch_thread_action_id(ui_thread_id: &str) -> String {
    format!("{AGENT_CHAT_SWITCH_THREAD_ACTION_PREFIX}{ui_thread_id}")
}

#[allow(dead_code)] // Used by Agent Chat action dispatch in the binary target.
pub(crate) fn agent_chat_switch_thread_id_from_action(action_id: &str) -> Option<&str> {
    action_id.strip_prefix(AGENT_CHAT_SWITCH_THREAD_ACTION_PREFIX)
}

/// Action ID for the "Rewind & Edit Message" drill-down trigger.
pub(crate) const AGENT_CHAT_REWIND_ACTION_ID: &str = "agent_chat_rewind_edit";

/// Route ID for the rewind checkpoint picker sub-route.
pub(crate) const AGENT_CHAT_FORK_PICKER_ROUTE_ID: &str = "agent_chat:fork_picker";

const AGENT_CHAT_FORK_EDIT_ACTION_PREFIX: &str = "agent_chat_fork_edit:";

fn agent_chat_fork_edit_action_id(entry_id: &str) -> String {
    format!("{AGENT_CHAT_FORK_EDIT_ACTION_PREFIX}{entry_id}")
}

#[allow(dead_code)] // Used by Agent Chat action dispatch in the binary target.
pub(crate) fn agent_chat_fork_edit_entry_from_action(action_id: &str) -> Option<&str> {
    action_id.strip_prefix(AGENT_CHAT_FORK_EDIT_ACTION_PREFIX)
}

/// Truncate a user message to a single action-row title line.
fn agent_chat_message_action_title(text: &str) -> String {
    const MAX_CHARS: usize = 64;
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return "(empty message)".to_string();
    }
    if line.chars().count() <= MAX_CHARS {
        return line.to_string();
    }
    let truncated: String = line.chars().take(MAX_CHARS).collect();
    format!("{}\u{2026}", truncated.trim_end())
}

/// Actions available in the Agent Chat chat view (Cmd+K menu).
#[allow(dead_code)]
pub fn get_agent_chat_actions() -> Vec<Action> {
    get_agent_chat_actions_with_facts(
        crate::components::conversation_actions::AgentChatConversationCommandFacts {
            response_in_progress: false,
            waiting_for_permission: false,
            context_preparing: false,
            composer_has_text: false,
            retry_available: false,
            has_response: true,
            ..Default::default()
        },
    )
}

pub(crate) fn get_agent_chat_actions_with_facts(
    facts: crate::components::conversation_actions::AgentChatConversationCommandFacts,
) -> Vec<Action> {
    let config = crate::config::load_config();
    let mut actions = get_prompt_export_actions(&config)
        .into_iter()
        .chain(get_prompt_target_actions(&config))
        .map(|mut action| {
            action.category = ActionCategory::ScriptContext;
            action
        })
        .collect::<Vec<_>>();
    actions.extend(vec![
        // ── Response ─────────────────────────────────────────
        Action::new(
            "agent_chat_copy_last_response",
            "Copy Last Response",
            Some("Copy the most recent assistant response".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{21e7}\u{2318}C")
        .with_icon(IconName::Copy)
        .with_section("Response"),
        Action::new(
            "agent_chat_paste_to_frontmost",
            "Paste Response to App",
            Some("Paste into the frontmost application".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowRight)
        .with_section("Response"),
        Action::new(
            "agent_chat_retry_last",
            "Retry Last Message",
            Some("Resend the last user message".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowRight)
        .with_section("Response"),
        Action::new(
            "agent_chat_export_markdown",
            "Copy Conversation as Markdown",
            Some("Copy the full conversation as markdown".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::FileCode)
        .with_section("Response"),
        Action::new(
            "agent_chat_save_as_note",
            "Save as Note",
            Some("Create or update a note from the current Agent Chat content".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{21e7}\u{2318}S")
        .with_icon(IconName::File)
        .with_section("Response"),
        Action::new(
            "agent_chat_append_last_response_to_today",
            "Append Last Response to Today",
            Some("Write the latest assistant response back to Today's brain".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::File)
        .with_section("Response"),
        // ── Code ─────────────────────────────────────────────
        Action::new(
            "agent_chat_copy_all_code",
            "Copy All Code Blocks",
            Some("Copy all code blocks to clipboard".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Code)
        .with_section("Code"),
        Action::new(
            "agent_chat_save_as_script",
            "Save as Script",
            Some("Save last code block as a Script Kit script".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::FileCode)
        .with_section("Code"),
        Action::new(
            "agent_chat_run_last_code",
            "Run Last Code Block",
            Some("Execute the last code block and show output".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::BoltFilled)
        .with_section("Code"),
        Action::new(
            "agent_chat_open_in_editor",
            "Open in Editor",
            Some("Open ~/.scriptkit in editor".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Code)
        .with_section("Code"),
        // ── Navigate ─────────────────────────────────────────
        Action::new(
            "agent_chat_scroll_to_top",
            "Scroll to Top",
            None,
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowUp)
        .with_section("Navigate"),
        Action::new(
            "agent_chat_scroll_to_bottom",
            "Scroll to Latest",
            None,
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowDown)
        .with_section("Navigate"),
        Action::new(
            "agent_chat_show_history",
            "Agent Chat History",
            Some("Browse and manage past Agent Chat conversations".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2318}P")
        .with_icon(IconName::MagnifyingGlass)
        .with_section("Navigate"),
        Action::new(
            AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID,
            "Receipt History",
            Some("Inspect recent semantic automation receipts for this session".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::FileCode)
        .with_section("Proof"),
        // ── View ─────────────────────────────────────────────
        Action::new(
            "agent_chat_expand_all",
            "Expand All Blocks",
            None,
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ChevronDown)
        .with_section("View"),
        Action::new(
            "agent_chat_collapse_all",
            "Collapse All Blocks",
            None,
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ChevronRight)
        .with_section("View"),
        // ── Session ──────────────────────────────────────────
        Action::new(
            "agent_chat_new_conversation",
            "New Conversation",
            Some("Clear messages, keep session".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2318}L")
        .with_icon(IconName::Plus)
        .with_section("Session"),
        Action::new(
            "agent_chat_clear_conversation",
            "Clear & Restart",
            Some("Close and reopen a fresh session".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Trash)
        .with_section("Session"),
        Action::new(
            "agent_chat_clear_history",
            "Delete All History",
            Some("Delete all saved conversations and prompt history".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Trash)
        .with_section("Session"),
        // ── Window ───────────────────────────────────────────
        Action::new(
            "agent_chat_detach_window",
            "Keep Open in Window",
            Some("Keep this chat open in a separate window".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowUp)
        .with_section("Window"),
        Action::new(
            "agent_chat_reattach_panel",
            "Return to Panel",
            Some("Move this chat back to the main panel".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::ArrowDown)
        .with_section("Window"),
        Action::new(
            "agent_chat_close",
            "Close Agent Chat",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2318}W")
        .with_icon(IconName::Close)
        .with_section("Window"),
    ]);

    actions.retain(|action| {
        !matches!(
            action.id.as_str(),
            "agent_chat_copy_last_response"
                | "agent_chat_retry_last"
                | "agent_chat_new_conversation"
                | "agent_chat_close"
        )
    });
    for binding in crate::components::conversation_actions::agent_chat_conversation_commands(facts)
    {
        use crate::components::conversation_actions::{
            AgentChatConversationCommand, ConversationCommandAvailability,
        };
        let (action_id, description, section, icon) = match binding.handler {
            AgentChatConversationCommand::Send => (
                "agent_chat_send",
                "Send the current message",
                "Conversation",
                IconName::ArrowRight,
            ),
            AgentChatConversationCommand::Stop => (
                "agent_chat_stop",
                "Stop the current response",
                "Conversation",
                IconName::Close,
            ),
            AgentChatConversationCommand::Retry => (
                "agent_chat_retry_last",
                "Retry the immutable last user request",
                "Response",
                IconName::ArrowRight,
            ),
            AgentChatConversationCommand::NewConversation => (
                "agent_chat_new_conversation",
                "Clear messages and keep this session",
                "Session",
                IconName::Plus,
            ),
            AgentChatConversationCommand::CopyLastResponse => (
                "agent_chat_copy_last_response",
                "Copy the most recent assistant response",
                "Response",
                IconName::Copy,
            ),
            AgentChatConversationCommand::Close => (
                "agent_chat_close",
                "Close Agent Chat",
                "Window",
                IconName::Close,
            ),
        };
        let mut action = Action::new(
            action_id,
            binding.descriptor.label,
            Some(description.to_string()),
            ActionCategory::ScriptContext,
        )
        .with_section(section)
        .with_icon(icon);
        if let Some(shortcut) = binding.descriptor.shortcut {
            action = action.with_shortcut(shortcut);
        }
        if let ConversationCommandAvailability::Disabled { reason } =
            binding.descriptor.availability
        {
            action = action.disabled(reason.as_str());
        }
        actions.push(action);
    }
    actions
}

pub(crate) fn get_focused_text_agent_chat_actions(expanded: bool) -> Vec<Action> {
    let expand_id = if expanded {
        "focused-text-action-collapse"
    } else {
        "focused-text-action-expand"
    };
    let expand_label = if expanded { "Collapse" } else { "Chat" };
    let expand_description = if expanded {
        "Return to the compact focused-text editor"
    } else {
        "Expand into the full Agent Chat conversation"
    };

    vec![
        Action::new(
            "focused-text-action-replace",
            "Replace Selected Text",
            Some("Replace the captured focused field with the latest response".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2318}\u{21b5}")
        .with_icon(IconName::ArrowRight)
        .with_section("Focused Text"),
        Action::new(
            "focused-text-action-append",
            "Append to Selected Text",
            Some("Append the latest response to the captured focused field".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Plus)
        .with_section("Focused Text"),
        Action::new(
            "focused-text-action-copy",
            "Copy Response",
            Some("Copy the latest response without changing the focused field".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Copy)
        .with_section("Focused Text"),
        Action::new(
            expand_id,
            expand_label,
            Some(expand_description.to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::MessageCircle)
        .with_section("Focused Text"),
        Action::new(
            "focused-text-action-retry",
            "Retry",
            Some("Retry the last focused-text turn".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::Refresh)
        .with_section("Focused Text"),
        Action::new(
            "focused-text-action-stop",
            "Stop",
            Some("Stop the current focused-text response".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("Esc")
        .with_icon(IconName::Close)
        .with_section("Focused Text"),
    ]
}

pub(crate) fn get_focused_text_agent_chat_root_route(
    expanded: bool,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_ROOT_ROUTE_ID.to_string(),
        actions: get_focused_text_agent_chat_actions(expanded),
        context_title: Some("Focused Text".to_string()),
        search_placeholder: Some("Search focused text actions...".to_string()),
        initial_selected_action_id: Some("focused-text-action-replace".to_string()),
    }
}

// ── Agent Chat route builders ───────────────────────────────────────────────────────
/// Action ID for the root-level "Change Model" entry that pushes the model picker.
pub const AGENT_CHAT_CHANGE_MODEL_ACTION_ID: &str = "agent_chat:change_model";
/// Action ID for the root-level "Profile picker" entry that pushes Agent Chat profiles.
pub const AGENT_CHAT_CHANGE_PROFILE_ACTION_ID: &str = "agent_chat:change_profile";

/// Stable root labels and descriptions for Agent Chat Actions Menu parity across hosts.
const AGENT_CHAT_CHANGE_MODEL_LABEL: &str = "Change Model";
const AGENT_CHAT_CHANGE_MODEL_DESCRIPTION: &str = "Pick the model for this chat";
const AGENT_CHAT_CHANGE_PROFILE_LABEL: &str = "Profile picker";
const AGENT_CHAT_CHANGE_PROFILE_DESCRIPTION: &str =
    "Pick the Agent Chat profile. Switching profiles starts a new chat";
/// Route ID for the Agent Chat root actions menu.
pub const AGENT_CHAT_ROOT_ROUTE_ID: &str = "agent_chat:root";
/// Route ID for the model picker sub-route.
pub const AGENT_CHAT_MODEL_PICKER_ROUTE_ID: &str = "agent_chat:model_picker";
/// Route ID for the Agent Chat profile picker sub-route.
pub const AGENT_CHAT_PROFILE_PICKER_ROUTE_ID: &str = "agent_chat:profile_picker";
/// Route ID for the Agent Chat history sub-route.
pub const AGENT_CHAT_HISTORY_ROUTE_ID: &str = "agent_chat:history";
/// Prefix for Agent Chat history row actions.
pub const AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX: &str = "agent_chat_history:select:";
/// Action ID for the root-level receipt-history entry.
pub const AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID: &str = "agent_chat_show_receipt_history";
/// Route ID for the Agent Chat receipt-history sub-route.
pub const AGENT_CHAT_RECEIPT_HISTORY_ROUTE_ID: &str = "agent_chat:receipt_history";
/// Prefix for receipt-history copy row actions.
pub const AGENT_CHAT_RECEIPT_HISTORY_COPY_ACTION_PREFIX: &str = "agent_chat_receipt_history:copy:";
/// Maximum receipt rows displayed in the compact ActionsDialog route.
pub const AGENT_CHAT_RECEIPT_HISTORY_ROUTE_LIMIT: usize = 20;

fn agent_chat_model_display_name(
    entry: &crate::ai::agent_chat::ui::config::AgentChatModelEntry,
) -> String {
    entry
        .display_name
        .clone()
        .unwrap_or_else(|| entry.id.clone())
}

pub(crate) fn agent_chat_history_select_action_id(session_id: &str) -> String {
    format!("{AGENT_CHAT_HISTORY_SELECT_ACTION_PREFIX}{session_id}")
}

pub(crate) fn agent_chat_receipt_history_copy_action_id(request_id: &str) -> String {
    format!("{AGENT_CHAT_RECEIPT_HISTORY_COPY_ACTION_PREFIX}{request_id}")
}

pub(crate) fn agent_chat_receipt_history_request_id_from_action(action_id: &str) -> Option<&str> {
    action_id.strip_prefix(AGENT_CHAT_RECEIPT_HISTORY_COPY_ACTION_PREFIX)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AgentChatModelSelectionActionPlan {
    CurrentModel,
    AvailableModel,
}

impl AgentChatModelSelectionActionPlan {
    fn from_is_selected(is_selected: bool) -> Self {
        if is_selected {
            Self::CurrentModel
        } else {
            Self::AvailableModel
        }
    }

    fn picker_title(self, display_name: &str) -> String {
        match self {
            Self::CurrentModel => format!("{display_name} \u{2713}"),
            Self::AvailableModel => display_name.to_string(),
        }
    }

    fn description(self, display_name: &str) -> String {
        match self {
            Self::CurrentModel => format!("Currently selected model: {display_name}"),
            Self::AvailableModel => format!("Switch Agent Chat to {display_name}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AgentChatRootPickerActionPlan {
    CurrentSelection(String),
    NoCurrentSelection,
}

impl AgentChatRootPickerActionPlan {
    fn from_selected_display_name(selected_display_name: Option<String>) -> Self {
        selected_display_name
            .map(Self::CurrentSelection)
            .unwrap_or(Self::NoCurrentSelection)
    }

    fn description(&self, fallback_description: &'static str) -> String {
        match self {
            Self::CurrentSelection(display_name) => format!("Current: {display_name}"),
            Self::NoCurrentSelection => fallback_description.to_string(),
        }
    }
}

fn agent_chat_model_switch_description(
    entry: &crate::ai::agent_chat::ui::config::AgentChatModelEntry,
    is_selected: bool,
) -> String {
    let display_name = agent_chat_model_display_name(entry);
    AgentChatModelSelectionActionPlan::from_is_selected(is_selected).description(&display_name)
}

fn agent_chat_profile_backend_label(_backend: crate::config::AgentChatBackend) -> &'static str {
    "Pi"
}

fn agent_chat_profile_source_label(
    source: crate::ai::agent_chat::profiles::AgentChatProfileSource,
) -> &'static str {
    match source {
        crate::ai::agent_chat::profiles::AgentChatProfileSource::BuiltIn => "Built-in",
        crate::ai::agent_chat::profiles::AgentChatProfileSource::User => "Custom",
        crate::ai::agent_chat::profiles::AgentChatProfileSource::Mdflow => "Markdown",
    }
}

fn agent_chat_profile_picker_entries(
) -> Vec<crate::ai::agent_chat::profiles::AgentChatProfilePickerEntry> {
    let ai_preferences = crate::config::load_user_preferences().ai;
    let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    crate::ai::agent_chat::profiles::agent_chat_profile_picker_entries(&ai_preferences, &ctx)
}

fn selected_agent_chat_profile_picker_id() -> String {
    let ai_preferences = crate::config::load_user_preferences().ai;
    let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
    crate::ai::agent_chat::profiles::selected_agent_chat_profile_picker_id(&ai_preferences, &ctx)
}

fn agent_chat_profile_picker_actions(
    entries: &[crate::ai::agent_chat::profiles::AgentChatProfilePickerEntry],
    selected_profile_id: &str,
) -> Vec<Action> {
    entries
        .iter()
        .map(|entry| {
            let is_selected = entry.id == selected_profile_id;
            let title = if is_selected {
                format!("{} \u{2713}", entry.name)
            } else {
                entry.name.clone()
            };
            let backend = agent_chat_profile_backend_label(entry.backend);
            let source = agent_chat_profile_source_label(entry.source);
            let description = if is_selected {
                format!("Currently selected. {source} · {backend}")
            } else {
                format!(
                    "Switch to this profile in a new chat. {source} · {backend} · Starts a new chat when a conversation is already active"
                )
            };

            Action::new(
                agent_chat_switch_profile_action_id(&entry.id),
                title,
                Some(description),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Settings)
        })
        .collect()
}

/// Build the root-level Agent Chat actions list. Includes the profile and model
/// picker entries (which trigger drill-down) plus the standard Agent Chat chat
/// actions. (The legacy "Change Agent" picker was removed — all sessions use
/// the Pi backend, with provider/model chosen via the Shift+Tab picker.)
pub(crate) fn get_agent_chat_root_actions(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
    standing_approval_count: usize,
    thread_summaries: &[crate::ai::agent_chat::ui::AgentChatThreadSummary],
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
    command_facts: crate::components::conversation_actions::AgentChatConversationCommandFacts,
) -> Vec<Action> {
    let profile_entries = agent_chat_profile_picker_entries();
    let selected_profile_id = selected_agent_chat_profile_picker_id();
    let selected_profile_name = profile_entries
        .iter()
        .find(|entry| entry.id == selected_profile_id)
        .map(|entry| entry.name.clone());
    let selected_model =
        selected_model_id.and_then(|id| available_models.iter().find(|entry| entry.id == id));
    let model_picker_plan = AgentChatRootPickerActionPlan::from_selected_display_name(
        selected_model.map(agent_chat_model_display_name),
    );
    let profile_picker_plan =
        AgentChatRootPickerActionPlan::from_selected_display_name(selected_profile_name);

    let mut actions = vec![Action::new(
        AGENT_CHAT_CHANGE_PROFILE_ACTION_ID,
        AGENT_CHAT_CHANGE_PROFILE_LABEL,
        Some(profile_picker_plan.description(AGENT_CHAT_CHANGE_PROFILE_DESCRIPTION)),
        ActionCategory::ScriptContext,
    )
    .with_icon(IconName::Settings)
    .with_section("Agent")];

    if !available_models.is_empty() {
        actions.push(
            Action::new(
                AGENT_CHAT_CHANGE_MODEL_ACTION_ID,
                AGENT_CHAT_CHANGE_MODEL_LABEL,
                Some(model_picker_plan.description(AGENT_CHAT_CHANGE_MODEL_DESCRIPTION)),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Settings)
            .with_section("Agent"),
        );
    }

    if standing_approval_count > 0 {
        actions.push(
            Action::new(
                AGENT_CHAT_REVIEW_APPROVALS_ACTION_ID,
                format!("Review Auto-Approvals ({standing_approval_count})"),
                Some("List the permissions this session was granted with Allow Always".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Check)
            .with_section("Agent"),
        );
    }

    actions.push(
        Action::new(
            AGENT_CHAT_NEW_THREAD_ACTION_ID,
            "New Thread",
            Some(
                "Start a fresh thread; the current conversation keeps running in the background"
                    .to_string(),
            ),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("\u{2318}N")
        .with_icon(IconName::Plus)
        .with_section("Threads"),
    );

    for summary in thread_summaries {
        let mut title = format!("Switch to: {}", summary.title);
        if summary.unread > 0 {
            title.push_str(&format!(" ({} new)", summary.unread));
        }
        let description = if summary.is_streaming {
            "Streaming in the background — switch to view live output"
        } else {
            "Resume this background thread"
        };
        actions.push(
            Action::new(
                agent_chat_switch_thread_action_id(&summary.ui_thread_id),
                title,
                Some(description.to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::MessageCircle)
            .with_section("Threads"),
        );
    }

    if !fork_points.is_empty() {
        actions.push(
            Action::new(
                AGENT_CHAT_REWIND_ACTION_ID,
                "Rewind & Edit Message",
                Some(
                    "Pick an earlier message: the conversation rewinds to that point and the text returns to the composer for editing"
                        .to_string(),
                ),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Pencil)
            .with_section("Session"),
        );
    }

    actions.extend(get_agent_chat_actions_with_facts(command_facts));
    actions
}

/// Build the second-level Agent Chat profile picker actions.
pub(crate) fn get_agent_chat_profile_picker_actions() -> Vec<Action> {
    let entries = agent_chat_profile_picker_entries();
    let selected_profile_id = selected_agent_chat_profile_picker_id();
    agent_chat_profile_picker_actions(&entries, &selected_profile_id)
}

/// Build the second-level Agent Chat model picker actions.
pub(crate) fn get_agent_chat_model_picker_actions(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
) -> Vec<Action> {
    let selected_model_id = selected_model_id.filter(|id| !id.is_empty());
    available_models
        .iter()
        .map(|entry| {
            let is_selected = selected_model_id == Some(entry.id.as_str());
            let selection_plan = AgentChatModelSelectionActionPlan::from_is_selected(is_selected);
            let display_name = agent_chat_model_display_name(entry);
            Action::new(
                agent_chat_switch_model_action_id(&entry.id),
                selection_plan.picker_title(&display_name),
                Some(agent_chat_model_switch_description(entry, is_selected)),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Settings)
        })
        .collect()
}

// ── Host-aware Agent Chat action filtering ─────────────────────────────────────────

/// Distinguishes whether the Agent Chat actions dialog is hosted in the shared main
/// panel or in the detached Agent Chat chat window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatActionsDialogHost {
    /// Shared Agent Chat surface in the main Script Kit panel — all actions available.
    Shared,
    /// Shared main-panel Agent Chat opened from the Day Page. Same as shared,
    /// plus the Day-return action that writes the last assistant reply back to Today.
    #[allow(dead_code)] // WIP: constructed once the Day Page Agent Chat host lands.
    DayPage,
    /// Historical Notes-origin compatibility policy. Notes now stages explicit
    /// context in the main Agent Chat host instead of hosting its own chat.
    #[allow(
        dead_code,
        reason = "historical Notes-origin policy remains covered by backward-compatibility tests"
    )]
    Notes,
    /// Detached Agent Chat chat window — only actions that work without the main panel.
    Detached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentChatHostActionPlan {
    IncludeWithShortcut,
    IncludeWithoutShortcut,
    Exclude,
}

impl AgentChatHostActionPlan {
    fn is_included(self) -> bool {
        !matches!(self, Self::Exclude)
    }

    fn keeps_shortcut(self) -> bool {
        matches!(self, Self::IncludeWithShortcut)
    }
}

fn agent_chat_host_action_plan(
    host: AgentChatActionsDialogHost,
    action_id: &str,
) -> AgentChatHostActionPlan {
    match host {
        AgentChatActionsDialogHost::Shared | AgentChatActionsDialogHost::DayPage => {
            if action_id == "agent_chat_close" {
                AgentChatHostActionPlan::IncludeWithoutShortcut
            } else if action_id == "agent_chat_append_last_response_to_today" {
                if matches!(host, AgentChatActionsDialogHost::DayPage) {
                    AgentChatHostActionPlan::IncludeWithShortcut
                } else {
                    AgentChatHostActionPlan::Exclude
                }
            } else {
                AgentChatHostActionPlan::IncludeWithShortcut
            }
        }
        AgentChatActionsDialogHost::Notes => {
            // Notes-hosted: same as Detached, keeping `agent_chat_close`
            // (returns to editor), and routing `agent_chat_show_history`
            // through the shared ActionsDialog.
            if matches!(
                action_id,
                "agent_chat:change_profile"
                    | "agent_chat:change_model"
                    | "agent_chat_send"
                    | "agent_chat_stop"
                    | "agent_chat_copy_last_response"
                    | "agent_chat_retry_last"
                    | "agent_chat_export_markdown"
                    | "agent_chat_save_as_note"
                    | "agent_chat_show_history"
                    | AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID
                    | "agent_chat_scroll_to_top"
                    | "agent_chat_scroll_to_bottom"
                    | "agent_chat_expand_all"
                    | "agent_chat_collapse_all"
                    | "agent_chat_new_conversation"
                    | "agent_chat_clear_history"
                    | "agent_chat_close"
                    | AGENT_CHAT_NEW_THREAD_ACTION_ID
                    | AGENT_CHAT_REWIND_ACTION_ID
            ) || action_id.starts_with(AGENT_CHAT_SWITCH_PROFILE_ACTION_PREFIX)
                || action_id.starts_with(AGENT_CHAT_SWITCH_MODEL_ACTION_PREFIX)
                || action_id.starts_with(AGENT_CHAT_SWITCH_THREAD_ACTION_PREFIX)
                || action_id.starts_with(AGENT_CHAT_FORK_EDIT_ACTION_PREFIX)
            {
                if action_id == "agent_chat_close" {
                    AgentChatHostActionPlan::IncludeWithoutShortcut
                } else {
                    AgentChatHostActionPlan::IncludeWithShortcut
                }
            } else {
                AgentChatHostActionPlan::Exclude
            }
        }
        AgentChatActionsDialogHost::Detached => {
            if matches!(
                action_id,
                "agent_chat:change_model"
                    | "agent_chat_send"
                    | "agent_chat_stop"
                    | "agent_chat_copy_last_response"
                    | "agent_chat_retry_last"
                    | "agent_chat_export_markdown"
                    | "agent_chat_save_as_note"
                    | "agent_chat_show_history"
                    | AGENT_CHAT_SHOW_RECEIPT_HISTORY_ACTION_ID
                    | "agent_chat_scroll_to_top"
                    | "agent_chat_scroll_to_bottom"
                    | "agent_chat_expand_all"
                    | "agent_chat_collapse_all"
                    | "agent_chat_new_conversation"
                    | "agent_chat_clear_history"
                    | "agent_chat_close"
                    | AGENT_CHAT_NEW_THREAD_ACTION_ID
                    | AGENT_CHAT_REWIND_ACTION_ID
            ) || action_id.starts_with(AGENT_CHAT_SWITCH_MODEL_ACTION_PREFIX)
                || action_id.starts_with(AGENT_CHAT_SWITCH_THREAD_ACTION_PREFIX)
                || action_id.starts_with(AGENT_CHAT_FORK_EDIT_ACTION_PREFIX)
            {
                AgentChatHostActionPlan::IncludeWithShortcut
            } else {
                AgentChatHostActionPlan::Exclude
            }
        }
    }
}

fn filter_agent_chat_actions_for_host(
    host: AgentChatActionsDialogHost,
    actions: Vec<Action>,
) -> Vec<Action> {
    let host_label = match host {
        AgentChatActionsDialogHost::Shared => "shared",
        AgentChatActionsDialogHost::DayPage => "day_page",
        AgentChatActionsDialogHost::Notes => "notes",
        AgentChatActionsDialogHost::Detached => "detached",
    };
    actions
        .into_iter()
        .filter_map(|mut action| {
            let plan = agent_chat_host_action_plan(host, &action.id);
            if !plan.is_included() {
                tracing::warn!(
                    event = "agent_chat_actions_menu_filtered",
                    host = host_label,
                    action_id = %action.id,
                    reason = "unsupported_in_host",
                    "Filtered unsupported Agent Chat Actions Menu item"
                );
                return None;
            }
            if !plan.keeps_shortcut() {
                action.shortcut = None;
                action.shortcut_tokens = None;
                action.shortcut_lower = None;
            }
            Some(action)
        })
        .collect()
}

/// Build an `ActionsDialogRoute` for the Agent Chat root menu, filtered for the given host.
pub(crate) fn get_agent_chat_root_route_for_host(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
    standing_approval_count: usize,
    thread_summaries: &[crate::ai::agent_chat::ui::AgentChatThreadSummary],
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
    command_facts: crate::components::conversation_actions::AgentChatConversationCommandFacts,
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    let host_label = match host {
        AgentChatActionsDialogHost::Shared => "shared",
        AgentChatActionsDialogHost::DayPage => "day_page",
        AgentChatActionsDialogHost::Notes => "notes",
        AgentChatActionsDialogHost::Detached => "detached",
    };

    let actions = filter_agent_chat_actions_for_host(
        host,
        get_agent_chat_root_actions(
            available_models,
            selected_model_id,
            standing_approval_count,
            thread_summaries,
            fork_points,
            command_facts,
        ),
    );

    let model_count = available_models.len();
    tracing::info!(
        event = "agent_chat_actions_menu_built",
        host = host_label,
        model_count,
        action_count = actions.len(),
        "Built Agent Chat Actions Menu"
    );

    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_ROOT_ROUTE_ID.to_string(),
        actions,
        context_title: Some("Agent Chat".to_string()),
        search_placeholder: Some("Search Agent Chat actions...".to_string()),
        initial_selected_action_id: Some(AGENT_CHAT_CHANGE_PROFILE_ACTION_ID.to_string()),
    }
}

/// Build an `ActionsDialogRoute` for the Agent Chat model picker sub-route, filtered for the given host.
pub(crate) fn get_agent_chat_model_picker_route_for_host(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_MODEL_PICKER_ROUTE_ID.to_string(),
        actions: filter_agent_chat_actions_for_host(
            host,
            get_agent_chat_model_picker_actions(available_models, selected_model_id),
        ),
        context_title: Some("Change Model".to_string()),
        search_placeholder: Some("Search models...".to_string()),
        initial_selected_action_id: selected_model_id.map(agent_chat_switch_model_action_id),
    }
}

/// Build the second-level rewind checkpoint picker actions (latest first,
/// since the most recent message is the most likely edit target).
pub(crate) fn get_agent_chat_fork_picker_actions(
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
) -> Vec<Action> {
    fork_points
        .iter()
        .rev()
        .map(|point| {
            Action::new(
                agent_chat_fork_edit_action_id(&point.entry_id),
                agent_chat_message_action_title(&point.text),
                Some("Rewind here and edit this message; later replies are discarded".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Pencil)
            .with_section("Messages")
        })
        .collect()
}

/// Build an `ActionsDialogRoute` for the rewind checkpoint picker.
pub(crate) fn get_agent_chat_fork_picker_route_for_host(
    fork_points: &[crate::ai::agent_chat::ui::AgentChatForkPoint],
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_FORK_PICKER_ROUTE_ID.to_string(),
        actions: filter_agent_chat_actions_for_host(
            host,
            get_agent_chat_fork_picker_actions(fork_points),
        ),
        context_title: Some("Rewind & Edit".to_string()),
        search_placeholder: Some("Search messages...".to_string()),
        initial_selected_action_id: fork_points
            .last()
            .map(|point| agent_chat_fork_edit_action_id(&point.entry_id)),
    }
}

/// Build an `ActionsDialogRoute` for recent Agent Chat conversations.
pub(crate) fn get_agent_chat_history_route() -> crate::actions::ActionsDialogRoute {
    let actions = crate::ai::agent_chat::ui::history::load_history()
        .into_iter()
        .take(100)
        .map(|entry| {
            Action::new(
                agent_chat_history_select_action_id(&entry.session_id),
                entry.title_display().to_string(),
                Some(entry.preview_display().to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::MessageCircle)
            .with_section("History")
        })
        .collect();

    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_HISTORY_ROUTE_ID.to_string(),
        actions,
        context_title: Some("Agent Chat History".to_string()),
        search_placeholder: Some("Search conversation history...".to_string()),
        initial_selected_action_id: None,
    }
}

/// Build an `ActionsDialogRoute` for recent semantic automation receipts.
pub(crate) fn get_agent_chat_receipt_history_route() -> crate::actions::ActionsDialogRoute {
    let mut actions = crate::agentic_protocol_bus::load_recent_protocol_response_summaries(
        AGENT_CHAT_RECEIPT_HISTORY_ROUTE_LIMIT,
    )
    .into_iter()
    .map(|summary| {
        let description = format!(
            "{} · session {} · {}",
            summary.preview,
            summary.session,
            summary
                .automation_id
                .as_deref()
                .unwrap_or("no automation target")
        );
        Action::new(
            agent_chat_receipt_history_copy_action_id(&summary.request_id),
            format!("{} · {}", summary.response_type, summary.request_id),
            Some(description),
            ActionCategory::ScriptContext,
        )
        .with_icon(IconName::FileCode)
        .with_section("Receipts")
    })
    .collect::<Vec<_>>();

    if actions.is_empty() {
        actions.push(
            Action::new(
                "agent_chat_receipt_history_empty",
                "No receipt history",
                Some("No protocol receipts have been recorded for this app session.".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::FileCode)
            .with_section("Receipts"),
        );
    }

    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_RECEIPT_HISTORY_ROUTE_ID.to_string(),
        actions,
        context_title: Some("Receipt History".to_string()),
        search_placeholder: Some("Search receipts...".to_string()),
        initial_selected_action_id: None,
    }
}

/// Build an `ActionsDialogRoute` for the Agent Chat profile picker sub-route.
pub(crate) fn get_agent_chat_profile_picker_route_for_host(
    host: AgentChatActionsDialogHost,
) -> crate::actions::ActionsDialogRoute {
    crate::actions::ActionsDialogRoute {
        id: AGENT_CHAT_PROFILE_PICKER_ROUTE_ID.to_string(),
        actions: filter_agent_chat_actions_for_host(host, get_agent_chat_profile_picker_actions()),
        context_title: Some("Profile picker".to_string()),
        search_placeholder: Some("Search profiles...".to_string()),
        initial_selected_action_id: Some(agent_chat_switch_profile_action_id(
            &selected_agent_chat_profile_picker_id(),
        )),
    }
}

/// Build an `ActionsDialogRoute` for the Agent Chat profile picker sub-route (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_profile_picker_route() -> crate::actions::ActionsDialogRoute {
    get_agent_chat_profile_picker_route_for_host(AgentChatActionsDialogHost::Shared)
}

/// Build an `ActionsDialogRoute` for the Agent Chat root menu (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_root_route(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
) -> crate::actions::ActionsDialogRoute {
    get_agent_chat_root_route_for_host(
        available_models,
        selected_model_id,
        0,
        &[],
        &[],
        crate::components::conversation_actions::AgentChatConversationCommandFacts::default(),
        AgentChatActionsDialogHost::Shared,
    )
}

/// Build an `ActionsDialogRoute` for the Agent Chat model picker sub-route (shared host).
#[allow(dead_code)]
pub(crate) fn get_agent_chat_model_picker_route(
    available_models: &[crate::ai::agent_chat::ui::config::AgentChatModelEntry],
    selected_model_id: Option<&str>,
) -> crate::actions::ActionsDialogRoute {
    get_agent_chat_model_picker_route_for_host(
        available_models,
        selected_model_id,
        AgentChatActionsDialogHost::Shared,
    )
}

#[cfg(test)]
#[path = "script_context/tests.rs"]
mod tests;
