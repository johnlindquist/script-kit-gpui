//! Command ID parsing, building, validation, and deeplink round-tripping.
//!
//! Canonical command identities use the shared domain's `{source}/{identifier}`
//! grammar. Only six sources currently have externally addressable dispatch
//! owners; passive/session-scoped identities remain valid without accidentally
//! becoming accepted shortcuts or executable deeplinks.

use anyhow::{anyhow, Result};
use sk_protocol::command_contract::{CommandIdentity, CommandSource};

/// The supported command ID categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Builtin,
    App,
    Script,
    Scriptlet,
    PromptTarget,
    PromptAction,
}

/// All runtime-supported command categories.
pub const SUPPORTED_COMMAND_CATEGORIES: &[CommandCategory] = &[
    CommandCategory::Builtin,
    CommandCategory::App,
    CommandCategory::Script,
    CommandCategory::Scriptlet,
    CommandCategory::PromptTarget,
    CommandCategory::PromptAction,
];

impl CommandCategory {
    /// Returns the string prefix for this category.
    pub const fn as_str(self) -> &'static str {
        self.source().prefix()
    }

    /// Domain identity owner for this externally dispatchable category.
    pub const fn source(self) -> CommandSource {
        match self {
            Self::Builtin => CommandSource::Builtin,
            Self::App => CommandSource::App,
            Self::Script => CommandSource::Script,
            Self::Scriptlet => CommandSource::Scriptlet,
            Self::PromptTarget => CommandSource::PromptTarget,
            Self::PromptAction => CommandSource::PromptAction,
        }
    }

    /// External command dispatch is deliberately narrower than launcher
    /// identity. A row without a persisted dispatch owner cannot pretend that
    /// a shortcut or deeplink will execute it.
    pub const fn from_source(source: CommandSource) -> Option<Self> {
        match source {
            CommandSource::Builtin => Some(Self::Builtin),
            CommandSource::App => Some(Self::App),
            CommandSource::Script => Some(Self::Script),
            CommandSource::Scriptlet => Some(Self::Scriptlet),
            CommandSource::PromptTarget => Some(Self::PromptTarget),
            CommandSource::PromptAction => Some(Self::PromptAction),
            CommandSource::Flow
            | CommandSource::Skill
            | CommandSource::Window
            | CommandSource::File
            | CommandSource::Note
            | CommandSource::Brain
            | CommandSource::BrainInbox
            | CommandSource::Todo
            | CommandSource::Conversation
            | CommandSource::AiVault
            | CommandSource::Clipboard
            | CommandSource::Dictation
            | CommandSource::BrowserTab
            | CommandSource::BrowserHistory
            | CommandSource::Fallback
            | CommandSource::ValidationIssue
            | CommandSource::Spine
            | CommandSource::Agent => None,
        }
    }
}

/// Parse the full, domain-owned launcher identity vocabulary. This does not
/// imply that its source has a shortcut, deeplink, or external execution owner.
pub fn parse_command_identity(value: &str) -> Result<CommandIdentity> {
    CommandIdentity::parse(value)
        .map_err(|error| anyhow!("invalid canonical command identity `{value}`: {error}"))
}

/// Parse a command ID into its category and identifier.
///
/// Valid format: `{category}/{identifier}` where category is one of the
/// supported categories and identifier is non-empty.
pub fn parse_command_id(value: &str) -> Result<(CommandCategory, &str)> {
    let identity = parse_command_identity(value)?;
    let category = CommandCategory::from_source(identity.source()).ok_or_else(|| {
        anyhow!(
            "command source `{}` has no external dispatch owner",
            identity.source().prefix()
        )
    })?;
    let identifier = &value[category.as_str().len() + 1..];
    Ok((category, identifier))
}

/// Check if a string is a valid canonical command ID.
pub fn is_valid_command_id(value: &str) -> bool {
    parse_command_id(value).is_ok()
}

/// Build a canonical command ID from a category and identifier.
pub fn build_command_id(category: CommandCategory, identifier: &str) -> Result<String> {
    let identity = CommandIdentity::new(category.source(), identifier)
        .map_err(|error| anyhow!("invalid command identifier: {error}"))?;
    Ok(identity.to_string())
}

/// Extract the bare identifier from a builtin value, stripping any `builtin/` or `builtin-` prefix.
pub fn normalize_builtin_identifier(value: &str) -> &str {
    let value = value.strip_prefix("builtin/").unwrap_or(value);
    value.strip_prefix("builtin-").unwrap_or(value)
}

/// Convert any builtin ID form to canonical `builtin/{identifier}`.
///
/// Handles:
/// - `"builtin-clipboard-history"` → `"builtin/clipboard-history"`
/// - `"clipboard-history"` → `"builtin/clipboard-history"`
/// - `"builtin/clipboard-history"` → `"builtin/clipboard-history"` (no-op)
pub fn canonical_builtin_command_id(value: &str) -> String {
    format!("builtin/{}", normalize_builtin_identifier(value))
}

/// Convert a command ID to its deeplink URL.
///
/// Format: `scriptkit://commands/{command_id}`
pub fn command_id_to_deeplink(value: &str) -> Result<String> {
    parse_command_id(value)?;
    Ok(format!("scriptkit://commands/{}", value))
}

/// Extract a command ID from a deeplink URL.
///
/// Expects format: `scriptkit://commands/{command_id}`
pub fn command_id_from_deeplink(url: &str) -> Result<String> {
    let command_id = url
        .strip_prefix("scriptkit://commands/")
        .ok_or_else(|| anyhow!("unsupported deeplink: {}", url))?;
    parse_command_id(command_id)?;
    Ok(command_id.to_string())
}

#[cfg(test)]
mod identity_contract_tests {
    use super::*;

    #[test]
    fn every_launcher_source_uses_the_one_domain_identity_parser() {
        for source in CommandSource::ALL {
            let candidate = format!("{}/stable-id", source.prefix());
            let identity = parse_command_identity(&candidate).expect("domain source must parse");

            assert_eq!(identity.source(), source);
            assert_eq!(identity.identifier(), "stable-id");
            assert_eq!(identity.as_str(), candidate);
        }
    }

    #[test]
    fn all_existing_dispatch_categories_roundtrip_without_aliases() {
        for category in SUPPORTED_COMMAND_CATEGORIES {
            let command_id = build_command_id(*category, "stable-id").expect("build identity");
            let (parsed, identifier) = parse_command_id(&command_id).expect("parse dispatch id");
            let deeplink = command_id_to_deeplink(&command_id).expect("external dispatch owner");

            assert_eq!(parsed, *category);
            assert_eq!(identifier, "stable-id");
            assert_eq!(
                command_id_from_deeplink(&deeplink).expect("roundtrip deeplink"),
                command_id
            );
            assert_eq!(
                CommandCategory::from_source(category.source()),
                Some(*category)
            );
        }
    }

    #[test]
    fn valid_session_only_identities_cannot_pretend_to_be_external_deeplinks() {
        for source in CommandSource::ALL {
            if CommandCategory::from_source(source).is_some() {
                continue;
            }
            let command_id = format!("{}/private-session-id", source.prefix());

            assert!(parse_command_identity(&command_id).is_ok());
            assert!(parse_command_id(&command_id).is_err());
            assert!(command_id_to_deeplink(&command_id).is_err());
        }
    }

    #[test]
    fn domain_identity_rejects_unsafe_whitespace_controls_and_unknown_sources() {
        for value in [
            "script/ private",
            "script/private ",
            "script/private\nvalue",
            "unknown/private",
            "script/",
            "script",
        ] {
            assert!(parse_command_identity(value).is_err(), "accepted {value:?}");
        }
        assert!(build_command_id(CommandCategory::Script, " private").is_err());
    }
}
