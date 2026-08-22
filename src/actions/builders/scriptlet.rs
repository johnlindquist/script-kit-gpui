use super::shared::{format_shortcut_hint, to_deeplink_name};
use super::types::{Action, ActionCategory, ScriptInfo};
use crate::designs::icon_variations::IconName;
use crate::scriptlets::{Scriptlet, ScriptletAction};
use std::collections::HashSet;

const SCRIPTLET_ACTION_ID_PREFIX: &str = "scriptlet_action:";

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn has_invalid_scriptlet_context_input(script: &ScriptInfo) -> bool {
    is_blank(&script.name) || is_blank(&script.action_verb)
}

fn has_invalid_scriptlet_action_input(action: &ScriptletAction) -> bool {
    is_blank(&action.name) || is_blank(&action.command)
}

fn count_invalid_scriptlet_actions(scriptlet: &Scriptlet) -> usize {
    scriptlet
        .actions
        .iter()
        .filter(|action| has_invalid_scriptlet_action_input(action))
        .count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptletContextActionPlan {
    NoShortcutNoAlias,
    ShortcutOnly,
    AliasOnly,
    ShortcutAndAlias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptletRankingActionPlan {
    NoRankingAction,
    ResetSuggestedRanking,
}

impl ScriptletContextActionPlan {
    fn from_script(script: &ScriptInfo) -> Self {
        match (script.shortcut.is_some(), script.alias.is_some()) {
            (false, false) => Self::NoShortcutNoAlias,
            (true, false) => Self::ShortcutOnly,
            (false, true) => Self::AliasOnly,
            (true, true) => Self::ShortcutAndAlias,
        }
    }

    fn has_shortcut(self) -> bool {
        matches!(self, Self::ShortcutOnly | Self::ShortcutAndAlias)
    }

    fn has_alias(self) -> bool {
        matches!(self, Self::AliasOnly | Self::ShortcutAndAlias)
    }
}

impl ScriptletRankingActionPlan {
    fn from_script(script: &ScriptInfo) -> Self {
        if script.is_suggested {
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
                    "Reset Ranking",
                    Some("Remove this item from Suggested section".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌃⌘R")
                .with_icon(IconName::Refresh)
                .with_section("Destructive"),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptletDefinedActionShortcutPlan<'a> {
    NoShortcut,
    Shortcut(&'a str),
}

impl<'a> ScriptletDefinedActionShortcutPlan<'a> {
    fn from_shortcut(shortcut: Option<&'a str>) -> Self {
        shortcut.map(Self::Shortcut).unwrap_or(Self::NoShortcut)
    }

    fn apply_to_action(self, action: Action) -> Action {
        match self {
            Self::NoShortcut => action,
            Self::Shortcut(shortcut) => action.with_shortcut(format_shortcut_hint(shortcut)),
        }
    }
}

fn parse_scriptlet_action_command(action_id: &str) -> Option<&str> {
    action_id
        .strip_prefix(SCRIPTLET_ACTION_ID_PREFIX)
        .filter(|command| !command.trim().is_empty())
}

fn unique_scriptlet_action_id(
    raw_action_id: &str,
    used_action_ids: &mut HashSet<String>,
) -> Option<String> {
    let action_command = parse_scriptlet_action_command(raw_action_id)?;

    let mut unique_action_id = raw_action_id.to_string();
    let mut duplicate_index = 2;
    while used_action_ids.contains(&unique_action_id) {
        unique_action_id =
            format!("{SCRIPTLET_ACTION_ID_PREFIX}{action_command}__{duplicate_index}");
        duplicate_index += 1;
    }

    if unique_action_id != raw_action_id {
        tracing::debug!(
            target: "script_kit::actions",
            original_action_id = %raw_action_id,
            deduped_action_id = %unique_action_id,
            "Deduplicated scriptlet H3 action id"
        );
    }

    used_action_ids.insert(unique_action_id.clone());
    Some(unique_action_id)
}

/// Convert scriptlet-defined actions (from H3 headers) to Action structs for the UI.
pub fn get_scriptlet_defined_actions(scriptlet: &Scriptlet) -> Vec<Action> {
    let invalid_action_count = count_invalid_scriptlet_actions(scriptlet);
    if invalid_action_count > 0 {
        tracing::warn!(
            target: "script_kit::actions",
            builder = "scriptlet_defined",
            scriptlet_name = %scriptlet.name,
            action_count = scriptlet.actions.len(),
            invalid_action_count,
            "Invalid scriptlet-defined action input; returning empty actions"
        );
        return vec![];
    }

    let mut used_action_ids = HashSet::new();
    let mut actions: Vec<Action> = Vec::with_capacity(scriptlet.actions.len());

    for sa in &scriptlet.actions {
        let raw_action_id = sa.action_id();
        let Some(action_id) = unique_scriptlet_action_id(&raw_action_id, &mut used_action_ids)
        else {
            tracing::warn!(
                target: "script_kit::actions",
                builder = "scriptlet_defined",
                scriptlet_name = %scriptlet.name,
                action_name = %sa.name,
                action_id = %raw_action_id,
                "Invalid scriptlet action id input; returning empty actions"
            );
            return vec![];
        };

        let shortcut_plan =
            ScriptletDefinedActionShortcutPlan::from_shortcut(sa.shortcut.as_deref());
        let mut action = shortcut_plan.apply_to_action(Action::new(
            action_id,
            &sa.name,
            sa.description.clone(),
            ActionCategory::ScriptContext,
        ));

        action = action
            .with_icon(IconName::PlayFilled)
            .with_section("Actions");
        action.has_action = true;
        action.value = Some(sa.command.clone());

        tracing::debug!(
            target: "script_kit::actions",
            action_id = %action.id,
            has_action = action.has_action,
            has_shortcut = action.shortcut.is_some(),
            "Created scriptlet-defined action (has_action=true)"
        );

        actions.push(action);
    }

    if !actions.is_empty() {
        tracing::debug!(
            target: "script_kit::actions",
            scriptlet_name = %scriptlet.name,
            custom_action_count = actions.len(),
            "Built scriptlet-defined actions from H3 headers"
        );
    }

    actions
}

/// Get actions for a scriptlet, including both custom (H3-defined) and built-in actions.
pub fn get_scriptlet_context_actions_with_custom(
    script: &ScriptInfo,
    scriptlet: Option<&Scriptlet>,
) -> Vec<Action> {
    if has_invalid_scriptlet_context_input(script) {
        tracing::warn!(
            target: "script_kit::actions",
            builder = "scriptlet_context",
            name = %script.name,
            action_verb = %script.action_verb,
            "Invalid scriptlet context input; returning empty actions"
        );
        return vec![];
    }

    if let Some(scriptlet) = scriptlet {
        let invalid_action_count = count_invalid_scriptlet_actions(scriptlet);
        if invalid_action_count > 0 {
            tracing::warn!(
                target: "script_kit::actions",
                builder = "scriptlet_context",
                scriptlet_name = %scriptlet.name,
                action_count = scriptlet.actions.len(),
                invalid_action_count,
                "Invalid scriptlet-defined action input; returning empty actions"
            );
            return vec![];
        }
    }

    let mut actions = Vec::new();
    let mut destructive_actions = Vec::new();
    let action_plan = ScriptletContextActionPlan::from_script(script);
    let ranking_plan = ScriptletRankingActionPlan::from_script(script);

    let mut primary_action = Action::new(
        "run_script",
        format!("{} \"{}\"", script.action_verb, script.name),
        Some(format!("{} this item", script.action_verb)),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("↵")
    .with_icon(IconName::PlayFilled)
    .with_section("Actions");
    if let Some(descriptor) = &script.command_descriptor {
        primary_action = primary_action.with_command_contract(descriptor);
    }
    actions.push(primary_action);

    if let Some(scriptlet) = scriptlet {
        actions.extend(get_scriptlet_defined_actions(scriptlet));
    }

    if action_plan.has_shortcut() {
        actions.push(
            Action::new(
                "update_shortcut",
                "Update Keyboard Shortcut",
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
                "Remove Keyboard Shortcut",
                Some("Remove the keyboard shortcut from this item".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌥K")
            .with_icon(IconName::Trash)
            .with_section("Destructive"),
        );
    } else {
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

    if action_plan.has_alias() {
        actions.push(
            Action::new(
                "update_alias",
                "Update Alias",
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
                "Remove Alias",
                Some("Remove the alias trigger from this item".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌥A")
            .with_icon(IconName::Trash)
            .with_section("Destructive"),
        );
    } else {
        actions.push(
            Action::new(
                "add_alias",
                "Add Alias",
                Some("Set an alias trigger for this item (type alias + space to run)".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧A")
            .with_icon(IconName::Settings)
            .with_section("Edit"),
        );
    }

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
            "Reveal in Finder",
            Some("Reveal scriptlet bundle in Finder".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧F")
        .with_icon(IconName::FolderOpen)
        .with_section("Share"),
    );

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

    let deeplink_name = to_deeplink_name(&script.name);
    actions.push(
        Action::new(
            "copy_deeplink",
            "Copy Deeplink",
            Some(format!(
                "Copy scriptkit://run/{} URL to clipboard",
                deeplink_name
            )),
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
    actions
}

#[cfg(test)]
mod tests {
    use super::{get_scriptlet_context_actions_with_custom, get_scriptlet_defined_actions};
    use crate::actions::types::ScriptInfo;
    use crate::designs::icon_variations::IconName;
    use crate::scriptlets::{Scriptlet, ScriptletAction};
    use std::collections::HashSet;

    fn scriptlet_action(name: &str, command: &str) -> ScriptletAction {
        ScriptletAction {
            name: name.to_string(),
            command: command.to_string(),
            tool: "bash".to_string(),
            code: "echo hi".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }
    }

    #[test]
    fn test_get_scriptlet_defined_actions_adds_counter_suffix_when_commands_repeat() {
        let mut scriptlet =
            Scriptlet::new("Dupes".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![
            scriptlet_action("First Copy", "copy"),
            scriptlet_action("Second Copy", "copy"),
            scriptlet_action("Third Copy", "copy"),
        ];

        let actions = get_scriptlet_defined_actions(&scriptlet);
        let ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();

        assert_eq!(actions.len(), 3);
        assert_eq!(ids[0], "scriptlet_action:copy");
        assert_eq!(ids[1], "scriptlet_action:copy__2");
        assert_eq!(ids[2], "scriptlet_action:copy__3");
        let unique_count = ids.iter().copied().collect::<HashSet<&str>>().len();
        assert_eq!(unique_count, ids.len());
    }

    #[test]
    fn test_get_scriptlet_defined_actions_avoids_suffix_collision_with_existing_command() {
        let mut scriptlet = Scriptlet::new(
            "Overlap".to_string(),
            "bash".to_string(),
            "echo".to_string(),
        );
        scriptlet.actions = vec![
            scriptlet_action("A", "copy"),
            scriptlet_action("B", "copy"),
            scriptlet_action("Explicit Suffix", "copy__2"),
        ];

        let actions = get_scriptlet_defined_actions(&scriptlet);
        let ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();

        assert_eq!(actions.len(), 3);
        assert_eq!(ids[0], "scriptlet_action:copy");
        assert_eq!(ids[1], "scriptlet_action:copy__2");
        assert_eq!(ids[2], "scriptlet_action:copy__2__2");
    }

    #[test]
    fn test_get_scriptlet_defined_actions_returns_empty_when_command_is_empty() {
        let mut scriptlet = Scriptlet::new(
            "Malformed".to_string(),
            "bash".to_string(),
            "echo".to_string(),
        );
        scriptlet.actions = vec![scriptlet_action("Bad", "")];

        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_scriptlet_defined_actions_returns_empty_when_any_action_is_invalid() {
        let mut scriptlet =
            Scriptlet::new("Mixed".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![
            scriptlet_action("Valid", "copy"),
            scriptlet_action("   ", "invalid"),
        ];

        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_scriptlet_context_actions_returns_empty_when_script_name_is_blank() {
        let mut script =
            ScriptInfo::scriptlet("Demo", "/tmp/demo.md", Some("cmd d".to_string()), None);
        script.name = "   ".to_string();

        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_scriptlet_context_actions_returns_empty_when_action_verb_is_blank() {
        let mut script =
            ScriptInfo::scriptlet("Demo", "/tmp/demo.md", Some("cmd d".to_string()), None);
        script.action_verb = "   ".to_string();

        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_get_scriptlet_context_actions_returns_empty_when_scriptlet_action_input_is_invalid() {
        let script = ScriptInfo::scriptlet("Demo", "/tmp/demo.md", Some("cmd d".to_string()), None);
        let mut scriptlet = Scriptlet::new(
            "Malformed".to_string(),
            "bash".to_string(),
            "echo".to_string(),
        );
        scriptlet.actions = vec![scriptlet_action("Bad", "   ")];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        assert!(actions.is_empty());
    }

    #[test]
    fn test_scriptlet_context_primary_action_uses_canonical_availability() {
        use sk_protocol::command_contract::{
            CommandAvailability, CommandDescriptor, CommandIdentity, CommandSource,
        };

        let mut descriptor = CommandDescriptor::new(
            CommandIdentity::new(CommandSource::Scriptlet, "main:demo").unwrap(),
            "Demo",
            "Run Verified Snippet",
        )
        .unwrap();
        descriptor.availability = CommandAvailability::UnknownPermission {
            permission: "unknown-device".to_owned(),
        };
        descriptor.actions[0].availability = descriptor.availability.clone();
        let script = ScriptInfo::scriptlet("Demo", "/tmp/demo.md", None, None)
            .with_command_descriptor(descriptor);

        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let primary = actions
            .iter()
            .find(|action| action.id == "run_script")
            .unwrap();

        assert_eq!(primary.title, "Run Verified Snippet");
        assert!(!primary.is_enabled());
        assert_eq!(
            primary.disabled_reason(),
            Some("This command requests a permission the host does not recognize.")
        );
    }

    #[test]
    fn test_get_scriptlet_context_actions_assigns_consistent_primary_icons() {
        let script = ScriptInfo::scriptlet("Demo", "/tmp/demo.md", Some("cmd d".to_string()), None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);

        let run_action = actions
            .iter()
            .find(|action| action.id == "run_script")
            .expect("missing run_script action");
        let edit_action = actions
            .iter()
            .find(|action| action.id == "edit_scriptlet")
            .expect("missing edit_scriptlet action");
        let reveal_action = actions
            .iter()
            .find(|action| action.id == "reveal_scriptlet_in_finder")
            .expect("missing reveal_scriptlet_in_finder action");
        let copy_path_action = actions
            .iter()
            .find(|action| action.id == "copy_scriptlet_path")
            .expect("missing copy_scriptlet_path action");
        let copy_content_action = actions
            .iter()
            .find(|action| action.id == "copy_content")
            .expect("missing copy_content action");
        let copy_deeplink_action = actions
            .iter()
            .find(|action| action.id == "copy_deeplink")
            .expect("missing copy_deeplink action");

        assert_eq!(run_action.icon, Some(IconName::PlayFilled));
        assert_eq!(edit_action.icon, Some(IconName::Pencil));
        assert_eq!(reveal_action.icon, Some(IconName::FolderOpen));
        assert_eq!(copy_path_action.icon, Some(IconName::Copy));
        assert_eq!(copy_content_action.icon, Some(IconName::Copy));
        assert_eq!(copy_deeplink_action.icon, Some(IconName::Copy));
    }
}
