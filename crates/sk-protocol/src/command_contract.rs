//! Stable command identity and presentation primitives shared by every host.
//!
//! This domain intentionally knows nothing about processes, providers, GPUI,
//! or platform APIs. Application adapters project their existing models into
//! these values without replacing source-specific execution owners.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

pub const COMMAND_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// Exhaustive source families surfaced by the launcher or conversation hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSource {
    Builtin,
    Script,
    Scriptlet,
    Flow,
    Skill,
    App,
    Window,
    File,
    Note,
    Brain,
    BrainInbox,
    Todo,
    Conversation,
    AiVault,
    Clipboard,
    Dictation,
    BrowserTab,
    BrowserHistory,
    Fallback,
    ValidationIssue,
    Spine,
    PromptTarget,
    PromptAction,
    Agent,
}

impl CommandSource {
    pub const ALL: [Self; 24] = [
        Self::Builtin,
        Self::Script,
        Self::Scriptlet,
        Self::Flow,
        Self::Skill,
        Self::App,
        Self::Window,
        Self::File,
        Self::Note,
        Self::Brain,
        Self::BrainInbox,
        Self::Todo,
        Self::Conversation,
        Self::AiVault,
        Self::Clipboard,
        Self::Dictation,
        Self::BrowserTab,
        Self::BrowserHistory,
        Self::Fallback,
        Self::ValidationIssue,
        Self::Spine,
        Self::PromptTarget,
        Self::PromptAction,
        Self::Agent,
    ];

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Script => "script",
            Self::Scriptlet => "scriptlet",
            Self::Flow => "flow",
            Self::Skill => "skill",
            Self::App => "app",
            Self::Window => "window",
            Self::File => "file",
            Self::Note => "note",
            Self::Brain => "brain",
            Self::BrainInbox => "brain-inbox",
            Self::Todo => "todo",
            Self::Conversation => "conversation",
            Self::AiVault => "ai-vault",
            Self::Clipboard => "clipboard-history",
            Self::Dictation => "dictation-history",
            Self::BrowserTab => "browser-tab",
            Self::BrowserHistory => "browser-history",
            Self::Fallback => "fallback",
            Self::ValidationIssue => "script-issue",
            Self::Spine => "spine",
            Self::PromptTarget => "prompt-target",
            Self::PromptAction => "prompt-action",
            Self::Agent => "agent",
        }
    }

    pub fn from_prefix(prefix: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|source| source.prefix() == prefix)
    }

    /// Passive sources still expose executable primary actions; they simply
    /// must not displace active commands when asynchronous providers finish.
    pub const fn is_passive(self) -> bool {
        matches!(
            self,
            Self::Window
                | Self::File
                | Self::Note
                | Self::Brain
                | Self::BrainInbox
                | Self::Todo
                | Self::Conversation
                | Self::AiVault
                | Self::Clipboard
                | Self::Dictation
                | Self::BrowserTab
                | Self::BrowserHistory
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandIdentity {
    source: CommandSource,
    canonical_id: String,
}

impl<'de> Deserialize<'de> for CommandIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SerializedCommandIdentity {
            source: CommandSource,
            canonical_id: String,
        }

        let serialized = SerializedCommandIdentity::deserialize(deserializer)?;
        let identity = Self::parse(&serialized.canonical_id).map_err(serde::de::Error::custom)?;
        if identity.source() != serialized.source {
            return Err(serde::de::Error::custom(
                "command identity source does not match its canonical prefix",
            ));
        }
        Ok(identity)
    }
}

impl CommandIdentity {
    pub fn new(source: CommandSource, identifier: impl AsRef<str>) -> Result<Self, ContractError> {
        let identifier = identifier.as_ref();
        if identifier.trim().is_empty()
            || identifier != identifier.trim()
            || identifier.chars().any(char::is_control)
        {
            return Err(ContractError::InvalidIdentity);
        }
        Ok(Self {
            source,
            canonical_id: format!("{}/{}", source.prefix(), identifier),
        })
    }

    pub fn parse(canonical_id: impl AsRef<str>) -> Result<Self, ContractError> {
        let canonical_id = canonical_id.as_ref();
        let (prefix, identifier) = canonical_id
            .split_once('/')
            .ok_or(ContractError::InvalidIdentity)?;
        let source = CommandSource::from_prefix(prefix).ok_or(ContractError::UnknownSource)?;
        Self::new(source, identifier)
    }

    pub const fn source(&self) -> CommandSource {
        self.source
    }

    pub fn as_str(&self) -> &str {
        &self.canonical_id
    }

    pub fn identifier(&self) -> &str {
        &self.canonical_id[self.source.prefix().len() + 1..]
    }
}

impl fmt::Display for CommandIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandCapability {
    Accessibility,
    ScreenRecording,
    Microphone,
    Clipboard,
    KeyboardInput,
    FileSystem,
    Network,
    Authentication,
    Sidecar,
    InteractivePrompt,
    BackgroundExecution,
    Cancellation,
    Streaming,
    AiContext,
}

/// Host-independent, user-safe disabled reasons. Conversation adapters map
/// their existing richer host-owned reasons here for protocol projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandUnavailableReason {
    InputRequired,
    ContextPreparing,
    ResponseInProgress,
    NoResponseRunning,
    PermissionPending,
    DraftMustBeResolved,
    RuntimeDetached,
    ActiveWorkMustBeStopped,
}

/// Safe typed availability; raw provider errors and secret values never enter
/// this contract. Diagnostic detail belongs in its owning diagnostic vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum CommandAvailability {
    Ready,
    MissingAuthentication,
    MissingPermission {
        capability: CommandCapability,
    },
    MissingDependency {
        dependency: String,
    },
    UnsupportedCapability {
        capability: CommandCapability,
    },
    UnsupportedSdkCapability {
        capability: String,
    },
    MissingSdkTransport {
        capability: String,
    },
    InteractivePromptUnavailable {
        capability: String,
    },
    UnsupportedPlatform {
        platform: String,
    },
    HostVersionTooOld {
        minimum_version: String,
        current_version: String,
    },
    UnknownPermission {
        permission: String,
    },
    InvalidCommandMetadata {
        field: String,
    },
    Blocked {
        reason: CommandUnavailableReason,
    },
    TemporarilyUnavailable,
    RequiresConfirmation,
    Suppressed,
}

impl CommandAvailability {
    pub const fn is_executable(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub const fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Ready => None,
            Self::MissingAuthentication => Some("missing_authentication"),
            Self::MissingPermission { .. } => Some("missing_permission"),
            Self::MissingDependency { .. } => Some("missing_dependency"),
            Self::UnsupportedCapability { .. } => Some("unsupported_capability"),
            Self::UnsupportedSdkCapability { .. } => Some("unsupported_sdk_capability"),
            Self::MissingSdkTransport { .. } => Some("missing_sdk_transport"),
            Self::InteractivePromptUnavailable { .. } => Some("interactive_prompt_unavailable"),
            Self::UnsupportedPlatform { .. } => Some("unsupported_platform"),
            Self::HostVersionTooOld { .. } => Some("host_version_too_old"),
            Self::UnknownPermission { .. } => Some("unknown_permission"),
            Self::InvalidCommandMetadata { .. } => Some("invalid_command_metadata"),
            Self::Blocked { reason } => Some(match reason {
                CommandUnavailableReason::InputRequired => "input_required",
                CommandUnavailableReason::ContextPreparing => "context_preparing",
                CommandUnavailableReason::ResponseInProgress => "response_in_progress",
                CommandUnavailableReason::NoResponseRunning => "no_response_running",
                CommandUnavailableReason::PermissionPending => "permission_pending",
                CommandUnavailableReason::DraftMustBeResolved => "draft_must_be_resolved",
                CommandUnavailableReason::RuntimeDetached => "runtime_detached",
                CommandUnavailableReason::ActiveWorkMustBeStopped => "active_work_must_be_stopped",
            }),
            Self::TemporarilyUnavailable => Some("temporarily_unavailable"),
            Self::RequiresConfirmation => Some("requires_confirmation"),
            Self::Suppressed => Some("suppressed"),
        }
    }

    pub const fn safe_message(&self) -> Option<&'static str> {
        match self {
            Self::Ready => None,
            Self::MissingAuthentication => Some("Sign in to continue."),
            Self::MissingPermission { .. } => Some("Grant the required permission to continue."),
            Self::MissingDependency { .. } => Some("Install the required dependency to continue."),
            Self::UnsupportedCapability { .. } => Some("This action is not supported here."),
            Self::UnsupportedSdkCapability { .. } => {
                Some("This command requires an SDK capability the host does not support.")
            }
            Self::MissingSdkTransport { .. } => {
                Some("This command cannot access the SDK from its current execution environment.")
            }
            Self::InteractivePromptUnavailable { .. } => {
                Some("This command cannot open an interactive prompt from its current environment.")
            }
            Self::UnsupportedPlatform { .. } => {
                Some("This command does not support the current platform.")
            }
            Self::HostVersionTooOld { .. } => Some("Update Script Kit to use this command."),
            Self::UnknownPermission { .. } => {
                Some("This command requests a permission the host does not recognize.")
            }
            Self::InvalidCommandMetadata { .. } => {
                Some("Correct this command's metadata before running it.")
            }
            Self::Blocked { reason } => Some(match reason {
                CommandUnavailableReason::InputRequired => "Type a message first.",
                CommandUnavailableReason::ContextPreparing => "Wait for context to finish loading.",
                CommandUnavailableReason::ResponseInProgress => "Stop the current response first.",
                CommandUnavailableReason::NoResponseRunning => "No response is running.",
                CommandUnavailableReason::PermissionPending => {
                    "Resolve the permission request first."
                }
                CommandUnavailableReason::DraftMustBeResolved => {
                    "Return to Current and send or clear the draft first."
                }
                CommandUnavailableReason::RuntimeDetached => "The runtime is already terminated.",
                CommandUnavailableReason::ActiveWorkMustBeStopped => {
                    "Stop the current response first; this host cannot keep it running after you leave."
                }
            }),
            Self::TemporarilyUnavailable => Some("This action is temporarily unavailable."),
            Self::RequiresConfirmation => Some("Confirm this action before continuing."),
            Self::Suppressed => Some("This action cannot be launched from this surface."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandActionRole {
    Primary,
    Secondary,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAction {
    pub id: String,
    pub title: String,
    pub shortcut: Option<String>,
    pub role: CommandActionRole,
    pub availability: CommandAvailability,
    pub requires_confirmation: bool,
}

impl CommandAction {
    pub fn primary(title: impl Into<String>) -> Self {
        Self {
            id: "primary".to_owned(),
            title: title.into(),
            shortcut: Some("enter".to_owned()),
            role: CommandActionRole::Primary,
            availability: CommandAvailability::Ready,
            requires_confirmation: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandArgumentKind {
    Text,
    Password,
    Selection,
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandArgument {
    pub name: String,
    pub kind: CommandArgumentKind,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandContextPolicy {
    None,
    ExplicitOnly,
    SelectedLauncherItem,
    PreserveExisting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandExecutionMode {
    HostAction,
    ForegroundProcess,
    BackgroundProcess,
    Conversation,
    OpenResource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionPolicy {
    pub mode: CommandExecutionMode,
    pub cancellable: bool,
    pub backgroundable: bool,
    pub streams_output: bool,
}

impl Default for CommandExecutionPolicy {
    fn default() -> Self {
        Self {
            mode: CommandExecutionMode::HostAction,
            cancellable: false,
            backgroundable: false,
            streams_output: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDescriptor {
    pub schema_version: u32,
    pub identity: CommandIdentity,
    pub title: String,
    pub subtitle: Option<String>,
    pub source_name: Option<String>,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub shortcut: Option<String>,
    pub arguments: Vec<CommandArgument>,
    pub capabilities: Vec<CommandCapability>,
    pub availability: CommandAvailability,
    pub actions: Vec<CommandAction>,
    pub execution: CommandExecutionPolicy,
    pub context: CommandContextPolicy,
}

impl CommandDescriptor {
    pub fn new(
        identity: CommandIdentity,
        title: impl Into<String>,
        primary_action: impl Into<String>,
    ) -> Result<Self, ContractError> {
        let descriptor = Self {
            schema_version: COMMAND_CONTRACT_SCHEMA_VERSION,
            identity,
            title: title.into(),
            subtitle: None,
            source_name: None,
            aliases: Vec::new(),
            keywords: Vec::new(),
            shortcut: None,
            arguments: Vec::new(),
            capabilities: Vec::new(),
            availability: CommandAvailability::Ready,
            actions: vec![CommandAction::primary(primary_action)],
            execution: CommandExecutionPolicy::default(),
            context: CommandContextPolicy::None,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != COMMAND_CONTRACT_SCHEMA_VERSION {
            return Err(ContractError::UnsupportedSchemaVersion);
        }
        if self.title.trim().is_empty() {
            return Err(ContractError::MissingTitle);
        }
        let mut action_ids = HashSet::new();
        let mut primary_count = 0;
        for action in &self.actions {
            if action.id.trim().is_empty() || action.title.trim().is_empty() {
                return Err(ContractError::InvalidAction);
            }
            if !action_ids.insert(action.id.as_str()) {
                return Err(ContractError::DuplicateAction);
            }
            if action.role == CommandActionRole::Primary {
                primary_count += 1;
            }
            if action.role == CommandActionRole::Destructive && !action.requires_confirmation {
                return Err(ContractError::UnconfirmedDestructiveAction);
            }
        }
        if primary_count != 1 {
            return Err(ContractError::InvalidPrimaryAction);
        }

        let mut argument_names = HashSet::new();
        for argument in &self.arguments {
            if argument.name.trim().is_empty() {
                return Err(ContractError::InvalidArgument);
            }
            if !argument_names.insert(argument.name.as_str()) {
                return Err(ContractError::DuplicateArgument);
            }
        }

        let mut aliases = HashSet::new();
        for alias in &self.aliases {
            if alias.trim().is_empty() {
                return Err(ContractError::InvalidAlias);
            }
            if !aliases.insert(alias.to_lowercase()) {
                return Err(ContractError::DuplicateAlias);
            }
        }
        Ok(())
    }

    pub fn primary_action(&self) -> Option<&CommandAction> {
        self.actions
            .iter()
            .find(|action| action.role == CommandActionRole::Primary)
    }

    pub fn can_execute(&self) -> bool {
        self.availability.is_executable()
            && self
                .primary_action()
                .is_some_and(|action| action.availability.is_executable())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    InvalidIdentity,
    UnknownSource,
    UnsupportedSchemaVersion,
    MissingTitle,
    InvalidAction,
    DuplicateAction,
    InvalidPrimaryAction,
    UnconfirmedDestructiveAction,
    InvalidArgument,
    DuplicateArgument,
    InvalidAlias,
    DuplicateAlias,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidIdentity => "invalid command identity",
            Self::UnknownSource => "unknown command source",
            Self::UnsupportedSchemaVersion => "unsupported command contract schema version",
            Self::MissingTitle => "command title cannot be empty",
            Self::InvalidAction => "command action id and title cannot be empty",
            Self::DuplicateAction => "command action ids must be unique",
            Self::InvalidPrimaryAction => "command must have exactly one primary action",
            Self::UnconfirmedDestructiveAction => "destructive actions require confirmation",
            Self::InvalidArgument => "command argument names cannot be empty",
            Self::DuplicateArgument => "command argument names must be unique",
            Self::InvalidAlias => "command aliases cannot be empty",
            Self::DuplicateAlias => "command aliases must be unique",
        };
        f.write_str(code)
    }
}

impl std::error::Error for ContractError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_has_a_unique_stable_parseable_identity() {
        let mut values = HashSet::new();
        for source in CommandSource::ALL {
            let identity = CommandIdentity::new(source, "owner:item").unwrap();
            assert!(values.insert(identity.as_str().to_owned()));
            assert_eq!(CommandIdentity::parse(identity.as_str()).unwrap(), identity);
            assert_eq!(identity.identifier(), "owner:item");
        }
    }

    #[test]
    fn identities_reject_missing_unknown_or_unsafe_identifiers() {
        for invalid in [
            "",
            "script",
            "script/",
            "script/ ",
            "other/item",
            "script/a\n",
        ] {
            assert!(
                CommandIdentity::parse(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn serialized_identity_cannot_bypass_canonical_source_or_identifier_validation() {
        fn deserialize_identity<'de>(
            source: &'de str,
            canonical_id: &'de str,
        ) -> Result<CommandIdentity, serde::de::value::Error> {
            CommandIdentity::deserialize(serde::de::value::MapDeserializer::new(
                [("source", source), ("canonicalId", canonical_id)].into_iter(),
            ))
        }

        let identity = CommandIdentity::new(CommandSource::Script, "main:hello").unwrap();
        assert_eq!(
            deserialize_identity("script", "script/main:hello").unwrap(),
            identity
        );

        for (source, canonical_id) in [
            ("script", "builtin/open-settings"),
            ("script", "script/"),
            ("script", "script/ padded "),
            ("script", "unknown/item"),
        ] {
            assert!(
                deserialize_identity(source, canonical_id).is_err(),
                "accepted forged serialized identity: {source}/{canonical_id}"
            );
        }
    }

    #[test]
    fn descriptor_projects_one_executable_primary_action() {
        let identity = CommandIdentity::new(CommandSource::Script, "main:hello").unwrap();
        let descriptor = CommandDescriptor::new(identity, "Hello", "Run Script").unwrap();
        assert_eq!(descriptor.schema_version, COMMAND_CONTRACT_SCHEMA_VERSION);
        assert_eq!(descriptor.primary_action().unwrap().title, "Run Script");
        assert!(descriptor.can_execute());
    }

    #[test]
    fn availability_blocks_execution_without_leaking_provider_detail() {
        let identity = CommandIdentity::new(CommandSource::Flow, "assistant").unwrap();
        let mut descriptor = CommandDescriptor::new(identity, "Assistant", "Open Flow").unwrap();
        descriptor.availability = CommandAvailability::MissingAuthentication;
        assert!(!descriptor.can_execute());
        assert_eq!(
            descriptor.availability.reason_code(),
            Some("missing_authentication")
        );
        assert_eq!(
            descriptor.availability.safe_message(),
            Some("Sign in to continue.")
        );
    }

    #[test]
    fn compatibility_failures_have_truthful_exhaustive_codes_and_safe_messages() {
        for (availability, code, safe_message) in [
            (
                CommandAvailability::UnsupportedPlatform {
                    platform: "macos".to_owned(),
                },
                "unsupported_platform",
                "This command does not support the current platform.",
            ),
            (
                CommandAvailability::HostVersionTooOld {
                    minimum_version: "2.0.0".to_owned(),
                    current_version: "1.0.0".to_owned(),
                },
                "host_version_too_old",
                "Update Script Kit to use this command.",
            ),
            (
                CommandAvailability::UnknownPermission {
                    permission: "screen-scrape-everything".to_owned(),
                },
                "unknown_permission",
                "This command requests a permission the host does not recognize.",
            ),
            (
                CommandAvailability::UnsupportedSdkCapability {
                    capability: "find".to_owned(),
                },
                "unsupported_sdk_capability",
                "This command requires an SDK capability the host does not support.",
            ),
            (
                CommandAvailability::MissingSdkTransport {
                    capability: "arg".to_owned(),
                },
                "missing_sdk_transport",
                "This command cannot access the SDK from its current execution environment.",
            ),
            (
                CommandAvailability::InteractivePromptUnavailable {
                    capability: "arg".to_owned(),
                },
                "interactive_prompt_unavailable",
                "This command cannot open an interactive prompt from its current environment.",
            ),
            (
                CommandAvailability::InvalidCommandMetadata {
                    field: "executionTopology".to_owned(),
                },
                "invalid_command_metadata",
                "Correct this command's metadata before running it.",
            ),
        ] {
            assert!(!availability.is_executable());
            assert_eq!(availability.reason_code(), Some(code));
            assert_eq!(availability.safe_message(), Some(safe_message));
        }
    }

    #[test]
    fn descriptor_rejects_duplicate_actions_arguments_aliases_and_unconfirmed_destruction() {
        let identity = CommandIdentity::new(CommandSource::Script, "main:hello").unwrap();
        let original = CommandDescriptor::new(identity, "Hello", "Run Script").unwrap();

        let mut duplicate_action = original.clone();
        duplicate_action
            .actions
            .push(duplicate_action.actions[0].clone());
        assert_eq!(
            duplicate_action.validate(),
            Err(ContractError::DuplicateAction)
        );

        let mut duplicate_argument = original.clone();
        let argument = CommandArgument {
            name: "query".to_owned(),
            kind: CommandArgumentKind::Text,
            required: true,
        };
        duplicate_argument
            .arguments
            .extend([argument.clone(), argument]);
        assert_eq!(
            duplicate_argument.validate(),
            Err(ContractError::DuplicateArgument)
        );

        let mut duplicate_alias = original.clone();
        duplicate_alias
            .aliases
            .extend(["hello".to_owned(), "HELLO".to_owned()]);
        assert_eq!(
            duplicate_alias.validate(),
            Err(ContractError::DuplicateAlias)
        );

        let mut destructive = original;
        destructive.actions.push(CommandAction {
            id: "delete".to_owned(),
            title: "Delete".to_owned(),
            shortcut: None,
            role: CommandActionRole::Destructive,
            availability: CommandAvailability::Ready,
            requires_confirmation: false,
        });
        assert_eq!(
            destructive.validate(),
            Err(ContractError::UnconfirmedDestructiveAction)
        );
    }

    #[test]
    fn passive_sources_remain_explicitly_distinct_from_active_commands() {
        assert!(CommandSource::File.is_passive());
        assert!(CommandSource::BrowserHistory.is_passive());
        assert!(!CommandSource::Script.is_passive());
        assert!(!CommandSource::Flow.is_passive());
    }
}
