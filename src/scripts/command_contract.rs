//! Application adapters from existing launcher rows to the shared wire contract.
//!
//! `SearchResult` remains the rendering/execution owner. These projections make
//! its already-existing identity, metadata, ranking, and availability reusable
//! by footers, Actions, AI handoffs, and semantic automation without creating
//! another app-local command model.

use super::{ConversationRowTarget, MatchEvidence, MatchEvidenceField, SearchResult};
use serde::Serialize;
use sk_protocol::command_contract::{
    CommandAction, CommandArgument, CommandArgumentKind, CommandAvailability, CommandCapability,
    CommandContextPolicy, CommandDescriptor, CommandExecutionMode, CommandExecutionPolicy,
    CommandIdentity, CommandSource, CommandUnavailableReason, ContractError,
};
use sk_protocol::search_contract::{RankingEvidence, RankingField, SearchCandidate};

/// Privacy-safe selected-command projection for DevTools and launcher
/// preflight. Authored titles, paths, transcript text, URLs, and durable raw
/// identifiers remain private; the existing content owner provides SHA-256
/// identity evidence instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherCommandReceipt {
    pub schema_version: u32,
    pub source: CommandSource,
    pub identity: crate::protocol::RedactedElementContent,
    pub availability: CommandAvailability,
    pub primary_action: CommandAction,
    pub execution: CommandExecutionPolicy,
    pub context: CommandContextPolicy,
    pub argument_count: usize,
}

/// Source-owned preference identity with a read-only compatibility alias.
/// New shortcuts/aliases always bind the exact command, while old display-name
/// settings remain visible until deliberately replaced or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandPreferenceIdentity {
    pub(crate) exact_id: String,
    pub(crate) legacy_id: String,
}

impl CommandPreferenceIdentity {
    pub(crate) fn preferred_value<T>(
        &self,
        mut lookup: impl FnMut(&str) -> Option<T>,
    ) -> Option<T> {
        lookup(&self.exact_id).or_else(|| {
            (self.exact_id != self.legacy_id)
                .then(|| lookup(&self.legacy_id))
                .flatten()
        })
    }

    pub(crate) fn existing_id(&self, mut exists: impl FnMut(&str) -> bool) -> String {
        if exists(&self.exact_id) || self.exact_id == self.legacy_id {
            self.exact_id.clone()
        } else if exists(&self.legacy_id) {
            self.legacy_id.clone()
        } else {
            self.exact_id.clone()
        }
    }
}

fn declared_permission_capability(permission: &str) -> Option<CommandCapability> {
    match permission {
        "accessibility" => Some(CommandCapability::Accessibility),
        "screen-recording" => Some(CommandCapability::ScreenRecording),
        "microphone" => Some(CommandCapability::Microphone),
        "clipboard" => Some(CommandCapability::Clipboard),
        "network" => Some(CommandCapability::Network),
        "filesystem" | "file-system" => Some(CommandCapability::FileSystem),
        _ => None,
    }
}

fn command_availability_from_validation_issues(
    issues: Vec<super::ScriptValidationIssue>,
) -> CommandAvailability {
    use super::{MetadataField, ScriptValidationKind, ValidationSeverity};
    use crate::mcp_resources::SdkCapabilityDiagnosticCode;

    let issue = issues
        .iter()
        .find(|issue| issue.severity == ValidationSeverity::Fatal)
        .or_else(|| issues.first());
    let Some(issue) = issue else {
        return CommandAvailability::Ready;
    };

    match &issue.kind {
        ScriptValidationKind::CapabilityUnavailable {
            capability, code, ..
        } => match code {
            SdkCapabilityDiagnosticCode::UnknownCapability
            | SdkCapabilityDiagnosticCode::UnsupportedCapability => {
                CommandAvailability::UnsupportedSdkCapability {
                    capability: capability.clone(),
                }
            }
            SdkCapabilityDiagnosticCode::MissingSdkTransport => {
                CommandAvailability::MissingSdkTransport {
                    capability: capability.clone(),
                }
            }
            SdkCapabilityDiagnosticCode::InteractivePromptUnavailable => {
                CommandAvailability::InteractivePromptUnavailable {
                    capability: capability.clone(),
                }
            }
            SdkCapabilityDiagnosticCode::UnsupportedPlatform => {
                CommandAvailability::UnsupportedPlatform {
                    platform: std::env::consts::OS.to_owned(),
                }
            }
            SdkCapabilityDiagnosticCode::MissingPermission => {
                let permission = crate::mcp_resources::sdk_capability(capability)
                    .and_then(|entry| entry.required_permissions.first().cloned());
                match permission {
                    Some(permission) => match declared_permission_capability(&permission) {
                        Some(capability) => CommandAvailability::MissingPermission { capability },
                        None => CommandAvailability::UnknownPermission { permission },
                    },
                    None => CommandAvailability::Blocked {
                        reason: CommandUnavailableReason::PermissionPending,
                    },
                }
            }
            SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable => {
                CommandAvailability::Blocked {
                    reason: CommandUnavailableReason::PermissionPending,
                }
            }
            SdkCapabilityDiagnosticCode::HostVersionTooOld => {
                match crate::mcp_resources::sdk_capability(capability) {
                    Some(entry) => CommandAvailability::HostVersionTooOld {
                        minimum_version: entry.minimum_host_version,
                        current_version: env!("CARGO_PKG_VERSION").to_owned(),
                    },
                    None => CommandAvailability::UnsupportedSdkCapability {
                        capability: capability.clone(),
                    },
                }
            }
            SdkCapabilityDiagnosticCode::InvalidHostVersion => {
                CommandAvailability::InvalidCommandMetadata {
                    field: "hostVersion".to_owned(),
                }
            }
        },
        ScriptValidationKind::InvalidValue { .. } => CommandAvailability::InvalidCommandMetadata {
            field: match issue.field {
                Some(MetadataField::ExecutionTopology) => "executionTopology",
                Some(MetadataField::Capability) => "sdkCapabilities",
                _ => "metadata",
            }
            .to_owned(),
        },
        ScriptValidationKind::MetadataParse { .. }
        | ScriptValidationKind::SchemaParse { .. }
        | ScriptValidationKind::DuplicateBinding { .. } => {
            CommandAvailability::InvalidCommandMetadata {
                field: "metadata".to_owned(),
            }
        }
    }
}

fn script_command_availability(script: &super::Script) -> CommandAvailability {
    command_availability_from_validation_issues(super::validate_declared_sdk_capabilities(script))
}

fn scriptlet_command_availability(scriptlet: &super::Scriptlet) -> CommandAvailability {
    command_availability_from_validation_issues(super::validate_scriptlet_capabilities(scriptlet))
}

impl SearchResult {
    pub(crate) fn command_preference_identity(&self) -> Option<CommandPreferenceIdentity> {
        Some(CommandPreferenceIdentity {
            exact_id: self.external_command_id()?,
            legacy_id: self.launcher_command_id()?,
        })
    }

    /// Share links must identify the executable source, not the mutable
    /// display-name alias retained for existing configuration and history.
    /// Keep this transformation in the library so the real launcher action
    /// and its owner-specific round-trip are behavior-testable.
    pub fn external_command_id(&self) -> Option<String> {
        let legacy_id = self.launcher_command_id()?;
        Some(match self {
            Self::Script(_) | Self::Scriptlet(_) => {
                self.stable_selection_key().unwrap_or(legacy_id)
            }
            _ => legacy_id,
        })
    }

    /// Exhaustive semantic family; conversation rows retain their tagged
    /// identity rather than being misclassified as their former flow position.
    pub fn command_source(&self) -> CommandSource {
        match self {
            Self::Script(_) => CommandSource::Script,
            Self::Scriptlet(_) => CommandSource::Scriptlet,
            Self::Flow(flow) => match flow.target {
                ConversationRowTarget::Conversation(_) => CommandSource::Conversation,
                ConversationRowTarget::FlowIdentity { .. } => CommandSource::Flow,
            },
            Self::Skill(_) => CommandSource::Skill,
            Self::BuiltIn(_) => CommandSource::Builtin,
            Self::App(_) => CommandSource::App,
            Self::Window(_) => CommandSource::Window,
            Self::File(_) => CommandSource::File,
            Self::Note(_) => CommandSource::Note,
            Self::BrainHit(_) => CommandSource::Brain,
            Self::BrainInboxItem(_) => CommandSource::BrainInbox,
            Self::Todo(_) => CommandSource::Todo,
            Self::AgentChatHistory(_) => CommandSource::Conversation,
            Self::AiVault(_) => CommandSource::AiVault,
            Self::ClipboardHistory(_) => CommandSource::Clipboard,
            Self::DictationHistory(_) => CommandSource::Dictation,
            Self::BrowserTab(_) => CommandSource::BrowserTab,
            Self::BrowserHistory(_) => CommandSource::BrowserHistory,
            Self::Agent(_) => CommandSource::Agent,
            Self::Fallback(_) => CommandSource::Fallback,
            Self::ScriptIssue(_) => CommandSource::ValidationIssue,
            Self::SpineProjection(_) => CommandSource::Spine,
        }
    }

    /// Canonical identity projected from durable owner keys, never row order,
    /// a fuzzy-search title, or a provider completion index.
    pub fn command_identity(&self) -> Result<CommandIdentity, ContractError> {
        let source = self.command_source();
        if matches!(self, Self::ScriptIssue(_)) {
            return CommandIdentity::new(source, "catalog-validation");
        }
        let stable = self
            .stable_selection_key()
            .ok_or(ContractError::InvalidIdentity)?;
        if let Ok(existing) = CommandIdentity::parse(&stable) {
            if existing.source() == source {
                return Ok(existing);
            }
        }
        CommandIdentity::new(source, stable)
    }

    /// The one normalized host-facing descriptor for this launcher row.
    /// Existing display titles, primary verbs, shortcut persistence, and
    /// source-specific runners remain untouched.
    pub fn command_descriptor(&self) -> Result<CommandDescriptor, ContractError> {
        let mut descriptor = CommandDescriptor::new(
            self.command_identity()?,
            self.name(),
            self.get_default_action_text(),
        )?;
        descriptor.subtitle = self.description().map(ToOwned::to_owned);
        descriptor.source_name = self.source_name().map(ToOwned::to_owned);

        match self {
            Self::Script(script_match) => {
                let script = &script_match.script;
                descriptor.shortcut = script.shortcut.clone();
                if let Some(alias) = &script.alias {
                    descriptor.aliases.push(alias.clone());
                }
                if let Some(metadata) = &script.typed_metadata {
                    descriptor.keywords.extend(metadata.tags.iter().cloned());
                    if let Some(alias) = &metadata.alias {
                        if !descriptor
                            .aliases
                            .iter()
                            .any(|existing| existing.eq_ignore_ascii_case(alias))
                        {
                            descriptor.aliases.push(alias.clone());
                        }
                    }
                    if metadata.background {
                        descriptor.execution.backgroundable = true;
                        descriptor.execution.mode = CommandExecutionMode::BackgroundProcess;
                        descriptor
                            .capabilities
                            .push(CommandCapability::BackgroundExecution);
                    }
                }
                if descriptor.execution.mode != CommandExecutionMode::BackgroundProcess {
                    descriptor.execution.mode = CommandExecutionMode::ForegroundProcess;
                }
                descriptor.execution.cancellable = true;
                descriptor
                    .capabilities
                    .push(CommandCapability::Cancellation);

                if let Some(schema) = &script.schema {
                    let mut input_fields: Vec<_> = schema.input.iter().collect();
                    input_fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                    descriptor
                        .arguments
                        .extend(
                            input_fields
                                .into_iter()
                                .map(|(name, field)| CommandArgument {
                                    name: name.clone(),
                                    kind: CommandArgumentKind::Text,
                                    required: field.required,
                                }),
                        );
                }
                descriptor.availability = script_command_availability(script);
                if !descriptor.availability.is_executable() {
                    if let Some(primary) = descriptor.actions.first_mut() {
                        primary.availability = descriptor.availability.clone();
                    }
                }
            }
            Self::Scriptlet(scriptlet_match) => {
                let scriptlet = &scriptlet_match.scriptlet;
                descriptor.shortcut = scriptlet.shortcut.clone();
                if let Some(alias) = &scriptlet.alias {
                    descriptor.aliases.push(alias.clone());
                }
                if let Some(keyword) = &scriptlet.keyword {
                    descriptor.keywords.push(keyword.clone());
                }
                descriptor.execution.mode = CommandExecutionMode::ForegroundProcess;
                descriptor.execution.cancellable = true;
                descriptor
                    .capabilities
                    .push(CommandCapability::Cancellation);
                descriptor.availability = scriptlet_command_availability(scriptlet);
                if !descriptor.availability.is_executable() {
                    if let Some(primary) = descriptor.actions.first_mut() {
                        primary.availability = descriptor.availability.clone();
                    }
                }
            }
            Self::BuiltIn(builtin) => {
                descriptor
                    .keywords
                    .extend(builtin.entry.keywords.iter().cloned());
            }
            Self::Flow(_) | Self::Skill(_) | Self::AgentChatHistory(_) => {
                descriptor.execution = CommandExecutionPolicy {
                    mode: CommandExecutionMode::Conversation,
                    cancellable: true,
                    backgroundable: true,
                    streams_output: true,
                };
                descriptor.capabilities.extend([
                    CommandCapability::Cancellation,
                    CommandCapability::Streaming,
                    CommandCapability::AiContext,
                ]);
                descriptor.context = CommandContextPolicy::ExplicitOnly;
            }
            Self::File(_) | Self::Note(_) | Self::BrowserTab(_) | Self::BrowserHistory(_) => {
                descriptor.execution.mode = CommandExecutionMode::OpenResource;
            }
            Self::Agent(_) => {
                descriptor.availability = CommandAvailability::Suppressed;
                if let Some(primary) = descriptor.actions.first_mut() {
                    primary.availability = CommandAvailability::Suppressed;
                }
            }
            Self::SpineProjection(row) if !row.is_selectable => {
                descriptor.availability = CommandAvailability::TemporarilyUnavailable;
                if let Some(primary) = descriptor.actions.first_mut() {
                    primary.availability = CommandAvailability::TemporarilyUnavailable;
                }
            }
            Self::App(_)
            | Self::Window(_)
            | Self::BrainHit(_)
            | Self::BrainInboxItem(_)
            | Self::Todo(_)
            | Self::AiVault(_)
            | Self::ClipboardHistory(_)
            | Self::DictationHistory(_)
            | Self::Fallback(_)
            | Self::ScriptIssue(_)
            | Self::SpineProjection(_) => {}
        }
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Project the same canonical launcher descriptor using only host facts an
    /// existing inventory explicitly supplied. This never probes permissions,
    /// starts an application, contacts a provider, or guesses unknown grants.
    pub fn command_descriptor_with_host_availability(
        &self,
        host: &crate::mcp_resources::SdkHostAvailability,
    ) -> Result<CommandDescriptor, ContractError> {
        let mut descriptor = self.command_descriptor()?;
        let availability = match self {
            Self::Script(script_match) => command_availability_from_validation_issues(
                super::validate_declared_sdk_capabilities_with_host_availability(
                    &script_match.script,
                    host,
                ),
            ),
            Self::Scriptlet(scriptlet_match) => command_availability_from_validation_issues(
                super::validate_scriptlet_capabilities_with_host_availability(
                    &scriptlet_match.scriptlet,
                    host,
                ),
            ),
            _ => return Ok(descriptor),
        };
        descriptor.availability = availability.clone();
        if let Some(primary) = descriptor.actions.first_mut() {
            primary.availability = availability;
        }
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Return the same safe, actionable refusal used by Actions, the launcher
    /// footer, execution preflight, and actual dispatch. Invalid commands stay
    /// discoverable; they must never advertise an executable primary action.
    pub fn command_execution_block_reason(&self) -> Option<&'static str> {
        let descriptor = match self.command_descriptor() {
            Ok(descriptor) => descriptor,
            Err(_) => return Some("This command has invalid metadata and cannot run."),
        };
        if descriptor.can_execute() {
            return None;
        }

        descriptor
            .primary_action()
            .and_then(|action| action.availability.safe_message())
            .or_else(|| descriptor.availability.safe_message())
            .or(Some("This command is not available right now."))
    }

    /// Authorize a selected launcher command before its submit pipeline
    /// records history, mutates caches, stages context, or dispatches work.
    /// The refusal is exactly the existing footer/Actions/preflight reason;
    /// this seam introduces no alternate readiness or permission policy.
    pub fn authorize_launcher_submit(&self) -> Result<(), &'static str> {
        match self.command_execution_block_reason() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    /// Project only reviewed safe facts into a machine-readable selected-row
    /// receipt. Selection identity is observable but never raw user data.
    pub fn redacted_command_receipt(&self) -> Result<LauncherCommandReceipt, ContractError> {
        let descriptor = self.command_descriptor()?;
        let primary_action = descriptor
            .primary_action()
            .cloned()
            .ok_or(ContractError::InvalidPrimaryAction)?;

        Ok(LauncherCommandReceipt {
            schema_version: descriptor.schema_version,
            source: descriptor.identity.source(),
            identity: crate::protocol::RedactedElementContent::new(
                crate::protocol::ElementContentKind::ExternalContent,
                descriptor.identity.as_str(),
            ),
            availability: descriptor.availability,
            primary_action,
            execution: descriptor.execution,
            context: descriptor.context,
            argument_count: descriptor.arguments.len(),
        })
    }

    /// Carry the exact match evidence used for admission into the search
    /// projection; consumers must not recompute highlight positions later.
    pub fn ranking_evidence(&self) -> RankingEvidence {
        let evidence = match self {
            Self::Script(item) => item.match_evidence.as_ref(),
            Self::Scriptlet(item) => item.match_evidence.as_ref(),
            Self::Skill(item) => item.match_evidence.as_ref(),
            Self::BuiltIn(item) => item.match_evidence.as_ref(),
            Self::App(item) => item.match_evidence.as_ref(),
            Self::Window(item) => item.match_evidence.as_ref(),
            Self::AgentChatHistory(item) => {
                return project_long_text_ranking_evidence(
                    item.evidence.as_ref(),
                    LongTextRankingSource::Conversation,
                    self.score(),
                    self.match_tier(),
                );
            }
            Self::DictationHistory(item) => {
                return project_long_text_ranking_evidence(
                    item.evidence.as_ref(),
                    LongTextRankingSource::Dictation,
                    self.score(),
                    self.match_tier(),
                );
            }
            _ => None,
        };
        project_ranking_evidence(evidence, self.score(), self.match_tier())
    }

    pub fn search_candidate(
        &self,
        section: impl Into<String>,
    ) -> Result<SearchCandidate, ContractError> {
        Ok(SearchCandidate {
            identity: self.command_identity()?,
            evidence: self.ranking_evidence(),
            section: section.into(),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum LongTextRankingSource {
    Conversation,
    Dictation,
}

fn project_long_text_ranking_evidence(
    evidence: Option<&crate::scripts::search::sentence::LongTextMatchEvidence>,
    source: LongTextRankingSource,
    fallback_score: i32,
    fallback_tier: i32,
) -> RankingEvidence {
    use crate::scripts::search::sentence::LongTextFieldId;

    let Some(evidence) = evidence else {
        return project_ranking_evidence(None, fallback_score, fallback_tier);
    };
    let empty_indices: &[usize] = &[];
    let (field, matched_indices) = match (source, evidence.primary_field) {
        (LongTextRankingSource::Conversation, LongTextFieldId::Title)
        | (LongTextRankingSource::Dictation, LongTextFieldId::Preview) => {
            (RankingField::Title, evidence.title_indices.as_slice())
        }
        (LongTextRankingSource::Conversation, LongTextFieldId::Preview)
        | (LongTextRankingSource::Dictation, LongTextFieldId::Target) => {
            (RankingField::Subtitle, evidence.subtitle_indices.as_slice())
        }
        (
            LongTextRankingSource::Dictation,
            LongTextFieldId::Timestamp | LongTextFieldId::Duration,
        ) => (RankingField::Subtitle, empty_indices),
        (
            LongTextRankingSource::Conversation,
            LongTextFieldId::Timestamp | LongTextFieldId::Duration,
        ) => (RankingField::Source, empty_indices),
        // Full transcripts can qualify a row without being rendered. Their
        // other visible terms never become fabricated primary highlights.
        _ => (RankingField::Content, empty_indices),
    };

    RankingEvidence {
        field,
        score: fallback_score,
        tier: fallback_tier,
        matched_indices: matched_indices.to_vec(),
        frecency_boost: 0,
        context_boost: 0,
    }
}

fn project_ranking_evidence(
    evidence: Option<&MatchEvidence>,
    fallback_score: i32,
    fallback_tier: i32,
) -> RankingEvidence {
    match evidence {
        Some(evidence) => RankingEvidence {
            field: match evidence.field {
                MatchEvidenceField::Name => RankingField::Title,
                MatchEvidenceField::Description => RankingField::Subtitle,
                MatchEvidenceField::Filename => RankingField::Filename,
                MatchEvidenceField::Content => RankingField::Content,
                MatchEvidenceField::Alias => RankingField::Alias,
                MatchEvidenceField::Shortcut => RankingField::Shortcut,
                MatchEvidenceField::Keyword => RankingField::Keyword,
                MatchEvidenceField::Source
                | MatchEvidenceField::Tool
                | MatchEvidenceField::WindowApp
                | MatchEvidenceField::SkillId
                | MatchEvidenceField::PluginTitle => RankingField::Source,
            },
            score: evidence.score,
            tier: evidence.tier,
            matched_indices: evidence.indices.clone(),
            frecency_boost: 0,
            context_boost: 0,
        },
        None => RankingEvidence {
            field: RankingField::Title,
            score: fallback_score,
            tier: fallback_tier,
            matched_indices: Vec::new(),
            frecency_boost: 0,
            context_boost: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripts::search::sentence::{
        LongTextFieldId, LongTextMatchEvidence, LongTextMatchTier,
    };
    use crate::scripts::{
        AgentChatHistoryMatch, DictationHistoryMatch, MatchIndices, Script, ScriptMatch, Scriptlet,
        ScriptletMatch,
    };
    use std::sync::Arc;

    fn script_result() -> SearchResult {
        SearchResult::Script(ScriptMatch {
            script: Arc::new(Script {
                name: "Hello".to_owned(),
                plugin_id: "main".to_owned(),
                alias: Some("hi".to_owned()),
                shortcut: Some("cmd h".to_owned()),
                ..Script::default()
            }),
            score: 42,
            filename: "hello.ts".to_owned(),
            match_indices: MatchIndices::default(),
            match_kind: crate::scripts::ScriptMatchKind::Name,
            content_match: None,
            match_evidence: Some(MatchEvidence {
                field: MatchEvidenceField::Alias,
                text: "hi".to_owned(),
                indices: vec![0, 1],
                tier: 3,
                score: 42,
            }),
        })
    }

    fn declared_scriptlet_result(name: &str, tool: &str, capabilities: &[&str]) -> SearchResult {
        let scriptlet = Arc::new(Scriptlet {
            name: name.to_owned(),
            description: None,
            code: String::new(),
            tool: tool.to_owned(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_owned(),
            plugin_title: None,
            file_path: Some(format!("/tmp/command-contract-{name}.md#{name}")),
            command: Some(name.to_owned()),
            alias: None,
            icon: None,
        });
        let mut metadata = crate::metadata_parser::TypedMetadata::default();
        metadata.extra.insert(
            "sdkCapabilities".to_owned(),
            serde_json::json!(capabilities),
        );
        crate::scripts::validation::register_scriptlet_capabilities(&scriptlet, Some(&metadata));

        SearchResult::Scriptlet(ScriptletMatch {
            scriptlet,
            score: 10,
            display_file_path: None,
            match_indices: MatchIndices::default(),
            match_evidence: None,
        })
    }

    fn long_text_evidence(
        primary_field: LongTextFieldId,
        title_indices: &[usize],
        subtitle_indices: &[usize],
    ) -> LongTextMatchEvidence {
        LongTextMatchEvidence {
            tier: LongTextMatchTier::MixedVisibleHidden,
            primary_field,
            title_indices: title_indices.to_vec(),
            subtitle_indices: subtitle_indices.to_vec(),
            title_text: "Visible title".to_owned(),
            subtitle_text: "Visible subtitle".to_owned(),
            hidden_excerpt: None,
        }
    }

    fn conversation_result(
        matched_field: crate::ai::agent_chat::ui::history::AgentChatHistorySearchField,
        evidence: LongTextMatchEvidence,
    ) -> SearchResult {
        SearchResult::AgentChatHistory(AgentChatHistoryMatch {
            entry: crate::ai::agent_chat::ui::history::AgentChatHistoryEntry {
                title: "Visible title".to_owned(),
                preview: "Visible subtitle".to_owned(),
                ..crate::ai::agent_chat::ui::history::AgentChatHistoryEntry::default()
            },
            score: 73,
            matched_field,
            subtitle: "Visible subtitle".to_owned(),
            evidence: Some(evidence),
        })
    }

    fn dictation_result(
        matched_field: crate::dictation::DictationHistorySearchField,
        evidence: LongTextMatchEvidence,
    ) -> SearchResult {
        SearchResult::DictationHistory(DictationHistoryMatch {
            id: "fixture".to_owned(),
            preview: "Visible title".to_owned(),
            target: "Visible subtitle".to_owned(),
            timestamp: "2026-08-22".to_owned(),
            audio_duration_ms: 1_000,
            subtitle: "Visible subtitle".to_owned(),
            score: 81,
            matched_field,
            evidence: Some(evidence),
        })
    }

    #[test]
    fn script_descriptor_preserves_existing_identity_label_action_and_bindings() {
        let result = script_result();
        let descriptor = result.command_descriptor().unwrap();
        assert_eq!(descriptor.identity.as_str(), "script/main:Hello");
        assert_eq!(descriptor.title, result.name());
        assert_eq!(
            descriptor.primary_action().unwrap().title,
            result.get_default_action_text()
        );
        assert_eq!(descriptor.aliases, ["hi"]);
        assert_eq!(descriptor.shortcut.as_deref(), Some("cmd h"));
        assert!(descriptor.can_execute());
    }

    #[test]
    fn same_named_scripts_keep_legacy_aliases_but_have_exact_source_owned_identity() {
        let result_for_path = |path: &str| {
            let mut result = script_result();
            let SearchResult::Script(script_match) = &mut result else {
                unreachable!("script fixture");
            };
            let script = Arc::get_mut(&mut script_match.script).expect("owned script fixture");
            script.name = "Open".into();
            script.path = path.into();
            result
        };
        let first = result_for_path("/workspace/a/open.ts");
        let second = result_for_path("/workspace/b/./open.ts");

        assert_eq!(first.launcher_command_id(), second.launcher_command_id());
        assert_eq!(first.history_result_key(), second.history_result_key());
        let first_identity = first.stable_selection_key().expect("first source identity");
        let second_identity = second
            .stable_selection_key()
            .expect("second source identity");
        assert_ne!(first_identity, second_identity);
        assert!(first_identity.starts_with("script/main:source-sha256-"));
        assert!(!first_identity.contains("/workspace"));
        assert_eq!(
            first.command_descriptor().unwrap().identity.as_str(),
            first_identity
        );

        let first_script = match &first {
            SearchResult::Script(script_match) => &script_match.script,
            _ => unreachable!(),
        };
        let second_script = match &second {
            SearchResult::Script(script_match) => &script_match.script,
            _ => unreachable!(),
        };
        let second_identifier = second_identity.strip_prefix("script/").unwrap();
        assert!(!first_script.matches_launcher_command_identifier(second_identifier));
        assert!(second_script.matches_launcher_command_identifier(second_identifier));
        assert!(first_script.matches_launcher_command_identifier("main:Open"));
        assert!(first_script.matches_launcher_command_identifier("Open"));

        let mut renamed = result_for_path("/workspace/b/open.ts");
        let SearchResult::Script(renamed_match) = &mut renamed else {
            unreachable!();
        };
        Arc::get_mut(&mut renamed_match.script)
            .expect("owned rename fixture")
            .name = "A friendlier display title".into();
        assert_eq!(
            renamed.stable_selection_key(),
            Some(second_identity.clone())
        );

        let first_deeplink =
            crate::config::command_id_to_deeplink(&first.external_command_id().unwrap()).unwrap();
        let deeplink =
            crate::config::command_id_to_deeplink(&second.external_command_id().unwrap()).unwrap();
        assert_ne!(first_deeplink, deeplink);
        assert!(deeplink.starts_with("scriptkit://commands/script/main:source-sha256-"));
        assert_eq!(
            crate::config::command_id_from_deeplink(&deeplink).unwrap(),
            second_identity
        );
    }

    #[test]
    fn same_named_scriptlets_bind_exact_markdown_source_anchor_and_command() {
        let result_for_anchor = |anchor: &str, command: &str| {
            let mut result = declared_scriptlet_result("Open", "paste", &[]);
            let SearchResult::Scriptlet(scriptlet_match) = &mut result else {
                unreachable!("scriptlet fixture");
            };
            let scriptlet =
                Arc::get_mut(&mut scriptlet_match.scriptlet).expect("owned scriptlet fixture");
            scriptlet.file_path = Some(format!("/workspace/snippets.md#{anchor}"));
            scriptlet.command = Some(command.into());
            result
        };
        let first = result_for_anchor("open-one", "open-one");
        let second = result_for_anchor("open-two", "open-two");
        assert_eq!(first.launcher_command_id(), second.launcher_command_id());
        let first_identity = first.stable_selection_key().unwrap();
        let second_identity = second.stable_selection_key().unwrap();
        assert_ne!(first_identity, second_identity);
        assert!(second_identity.starts_with("scriptlet/main:source-sha256-"));
        assert!(!second_identity.contains("snippets.md"));
        assert_eq!(
            second.command_descriptor().unwrap().identity.as_str(),
            second_identity
        );

        let first_scriptlet = match &first {
            SearchResult::Scriptlet(scriptlet_match) => &scriptlet_match.scriptlet,
            _ => unreachable!(),
        };
        let second_scriptlet = match &second {
            SearchResult::Scriptlet(scriptlet_match) => &scriptlet_match.scriptlet,
            _ => unreachable!(),
        };
        let exact_identifier = second_identity.strip_prefix("scriptlet/").unwrap();
        assert!(!first_scriptlet.matches_launcher_command_identifier(exact_identifier));
        assert!(second_scriptlet.matches_launcher_command_identifier(exact_identifier));
        assert!(second_scriptlet.matches_launcher_command_identifier("main:Open"));
        assert!(second_scriptlet.matches_launcher_command_identifier("Open"));
        assert_ne!(first.external_command_id(), second.external_command_id());

        let different_command = result_for_anchor("open-two", "another-command");
        assert_ne!(
            different_command.stable_selection_key(),
            Some(second_identity)
        );
    }

    #[test]
    fn source_less_script_and_scriptlet_fixtures_preserve_legacy_identity() {
        let script = script_result();
        assert_eq!(script.stable_selection_key(), script.launcher_command_id());

        let mut scriptlet = declared_scriptlet_result("Open", "paste", &[]);
        let SearchResult::Scriptlet(scriptlet_match) = &mut scriptlet else {
            unreachable!();
        };
        Arc::get_mut(&mut scriptlet_match.scriptlet)
            .expect("owned scriptlet fixture")
            .file_path = None;
        assert_eq!(
            scriptlet.stable_selection_key(),
            scriptlet.launcher_command_id()
        );
        assert_eq!(
            scriptlet.command_descriptor().unwrap().identity.as_str(),
            "scriptlet/main:Open"
        );
    }

    #[test]
    fn same_named_commands_keep_independent_preferences_with_legacy_read_fallback() {
        let result_for_path = |path: &str| {
            let mut result = script_result();
            let SearchResult::Script(script_match) = &mut result else {
                unreachable!();
            };
            let script = Arc::get_mut(&mut script_match.script).unwrap();
            script.name = "Open".into();
            script.path = path.into();
            result
        };
        let first = result_for_path("/workspace/first/open.ts")
            .command_preference_identity()
            .unwrap();
        let second = result_for_path("/workspace/second/open.ts")
            .command_preference_identity()
            .unwrap();
        assert_eq!(first.legacy_id, "script/main:Open");
        assert_eq!(first.legacy_id, second.legacy_id);
        assert_ne!(first.exact_id, second.exact_id);

        let mut preferences = std::collections::HashMap::from([(
            first.legacy_id.clone(),
            "historical shortcut".to_owned(),
        )]);
        assert_eq!(
            first.preferred_value(|id| preferences.get(id).cloned()),
            Some("historical shortcut".to_owned())
        );
        assert_eq!(
            first.existing_id(|id| preferences.contains_key(id)),
            first.legacy_id
        );

        preferences.insert(first.exact_id.clone(), "first exact shortcut".into());
        preferences.insert(second.exact_id.clone(), "second exact shortcut".into());
        assert_eq!(
            first.preferred_value(|id| preferences.get(id).cloned()),
            Some("first exact shortcut".into())
        );
        assert_eq!(
            second.preferred_value(|id| preferences.get(id).cloned()),
            Some("second exact shortcut".into())
        );
        let removed = first.existing_id(|id| preferences.contains_key(id));
        preferences.remove(&removed);
        assert_eq!(
            second.preferred_value(|id| preferences.get(id).cloned()),
            Some("second exact shortcut".into())
        );
        assert_eq!(
            first.preferred_value(|id| preferences.get(id).cloned()),
            Some("historical shortcut".into())
        );
        assert_eq!(
            preferences.get(&first.legacy_id).unwrap(),
            "historical shortcut"
        );

        let source_less = script_result().command_preference_identity().unwrap();
        assert_eq!(source_less.exact_id, source_less.legacy_id);
    }

    #[test]
    fn malformed_command_descriptors_fail_closed_before_execution() {
        let mut result = script_result();
        let SearchResult::Script(script_match) = &mut result else {
            panic!("script fixture should contain a script");
        };
        Arc::get_mut(&mut script_match.script)
            .expect("fixture owns its script")
            .name
            .clear();

        assert!(result.command_descriptor().is_err());
        assert_eq!(
            result.command_execution_block_reason(),
            Some("This command has invalid metadata and cannot run.")
        );
    }

    #[test]
    fn script_and_scriptlet_pending_permissions_share_one_disabled_contract() {
        let mut result = script_result();
        let SearchResult::Script(script_match) = &mut result else {
            panic!("script fixture should contain a script");
        };
        let script = Arc::get_mut(&mut script_match.script).expect("fixture owns its script");
        script.typed_metadata = Some(crate::metadata_parser::TypedMetadata {
            extra: std::collections::HashMap::from([(
                "sdkCapabilities".to_owned(),
                serde_json::json!(["moveWindow"]),
            )]),
            ..crate::metadata_parser::TypedMetadata::default()
        });

        let descriptor = result
            .command_descriptor()
            .expect("pending row stays visible");
        assert_eq!(
            descriptor.availability,
            CommandAvailability::Blocked {
                reason: CommandUnavailableReason::PermissionPending,
            }
        );
        assert_eq!(
            descriptor.primary_action().unwrap().availability,
            descriptor.availability
        );
        assert_eq!(
            result.command_execution_block_reason(),
            Some("Resolve the permission request first.")
        );
        assert!(!descriptor.can_execute());
    }

    #[test]
    fn explicit_host_inventory_never_disagrees_with_canonical_command_readiness() {
        let _registry_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut script = script_result();
        let SearchResult::Script(script_match) = &mut script else {
            panic!("script fixture should contain a script");
        };
        Arc::get_mut(&mut script_match.script)
            .expect("fixture owns its script")
            .typed_metadata = Some(crate::metadata_parser::TypedMetadata {
            extra: std::collections::HashMap::from([(
                "sdkCapabilities".to_owned(),
                serde_json::json!(["moveWindow"]),
            )]),
            ..crate::metadata_parser::TypedMetadata::default()
        });

        let scriptlet = declared_scriptlet_result("host-aware-window", "ts", &["moveWindow"]);
        let granted = crate::mcp_resources::SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: std::env::consts::OS.to_owned(),
            granted_permissions: vec!["accessibility".to_owned()],
        };
        let denied = crate::mcp_resources::SdkHostAvailability {
            granted_permissions: Vec::new(),
            ..granted.clone()
        };

        for result in [&script, &scriptlet] {
            assert!(!result.command_descriptor().unwrap().can_execute());
            let ready = result
                .command_descriptor_with_host_availability(&granted)
                .expect("explicit granted inventory");
            assert!(ready.can_execute());
            let refused = result
                .command_descriptor_with_host_availability(&denied)
                .expect("explicit denied inventory");
            assert_eq!(
                refused.availability,
                CommandAvailability::MissingPermission {
                    capability: CommandCapability::Accessibility,
                }
            );
            assert!(!refused.can_execute());
        }
    }

    #[test]
    fn ranking_projection_preserves_the_actual_winning_field_and_indices() {
        let result = script_result();
        let candidate = result.search_candidate("Scripts").unwrap();
        assert_eq!(candidate.identity.as_str(), "script/main:Hello");
        assert_eq!(candidate.evidence.field, RankingField::Alias);
        assert_eq!(candidate.evidence.matched_indices, [0, 1]);
        assert_eq!(candidate.evidence.score, 42);
        assert_eq!(candidate.evidence.tier, 3);
    }

    #[test]
    fn conversation_ranking_projection_preserves_visible_title_and_subtitle_indices() {
        use crate::ai::agent_chat::ui::history::AgentChatHistorySearchField;

        let title = conversation_result(
            AgentChatHistorySearchField::Title,
            long_text_evidence(LongTextFieldId::Title, &[0, 2, 4], &[7]),
        )
        .ranking_evidence();
        assert_eq!(title.field, RankingField::Title);
        assert_eq!(title.matched_indices, [0, 2, 4]);
        assert_eq!(title.score, 73);

        let subtitle = conversation_result(
            AgentChatHistorySearchField::Preview,
            long_text_evidence(LongTextFieldId::Preview, &[3], &[1, 5]),
        )
        .ranking_evidence();
        assert_eq!(subtitle.field, RankingField::Subtitle);
        assert_eq!(subtitle.matched_indices, [1, 5]);
    }

    #[test]
    fn conversation_hidden_transcript_ranking_never_fabricates_visible_highlights() {
        let evidence = conversation_result(
            crate::ai::agent_chat::ui::history::AgentChatHistorySearchField::SearchText,
            long_text_evidence(LongTextFieldId::Transcript, &[0, 1], &[2, 3]),
        )
        .ranking_evidence();

        assert_eq!(evidence.field, RankingField::Content);
        assert!(evidence.matched_indices.is_empty());
        assert_eq!(evidence.score, 73);
    }

    #[test]
    fn dictation_ranking_projection_distinguishes_visible_preview_from_hidden_transcript() {
        let visible = dictation_result(
            crate::dictation::DictationHistorySearchField::Transcript,
            long_text_evidence(LongTextFieldId::Preview, &[1, 3], &[6]),
        )
        .ranking_evidence();
        assert_eq!(visible.field, RankingField::Title);
        assert_eq!(visible.matched_indices, [1, 3]);

        let hidden = dictation_result(
            crate::dictation::DictationHistorySearchField::Transcript,
            long_text_evidence(LongTextFieldId::Transcript, &[1, 3], &[6]),
        )
        .ranking_evidence();
        assert_eq!(hidden.field, RankingField::Content);
        assert!(hidden.matched_indices.is_empty());
    }

    #[test]
    fn dictation_metadata_ranking_preserves_subtitle_without_invented_indices() {
        let target = dictation_result(
            crate::dictation::DictationHistorySearchField::Target,
            long_text_evidence(LongTextFieldId::Target, &[4], &[0, 2]),
        )
        .ranking_evidence();
        assert_eq!(target.field, RankingField::Subtitle);
        assert_eq!(target.matched_indices, [0, 2]);

        let timestamp = dictation_result(
            crate::dictation::DictationHistorySearchField::Timestamp,
            long_text_evidence(LongTextFieldId::Timestamp, &[4], &[0, 2]),
        )
        .ranking_evidence();
        assert_eq!(timestamp.field, RankingField::Subtitle);
        assert!(timestamp.matched_indices.is_empty());
        assert_eq!(timestamp.score, 81);
    }

    #[test]
    fn validation_identity_does_not_depend_on_mutable_counts_or_title() {
        let mut issue = SearchResult::ScriptIssue(crate::scripts::ScriptIssueMatch {
            title: "One script needs attention".to_owned(),
            description: None,
            failed_count: 1,
            fatal_count: 1,
            warning_count: 0,
            score: 100,
        });
        let first = issue.command_identity().unwrap();
        if let SearchResult::ScriptIssue(value) = &mut issue {
            value.title = "Three scripts need attention".to_owned();
            value.failed_count = 3;
        }
        assert_eq!(issue.command_identity().unwrap(), first);
        assert_eq!(first.as_str(), "script-issue/catalog-validation");
    }

    #[test]
    fn scriptlet_descriptors_block_unsupported_capabilities_without_hiding_the_row() {
        let _registry_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let descriptor = declared_scriptlet_result("unsupported", "ts", &["find"])
            .command_descriptor()
            .expect("invalid authoring remains an inspectable launcher command");

        assert_eq!(
            descriptor.availability,
            CommandAvailability::UnsupportedSdkCapability {
                capability: "find".to_owned(),
            }
        );
        assert_eq!(
            descriptor.primary_action().unwrap().availability,
            descriptor.availability
        );
        assert!(!descriptor.can_execute());
    }

    #[test]
    fn scriptlet_descriptors_distinguish_interactive_transport_and_unknown_permissions() {
        let _registry_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let supported = declared_scriptlet_result("interactive", "ts", &["arg"])
            .command_descriptor()
            .unwrap();
        assert!(supported.can_execute());

        let missing_transport = declared_scriptlet_result("shell", "bash", &["arg"])
            .command_descriptor()
            .unwrap();
        assert_eq!(
            missing_transport.availability,
            CommandAvailability::MissingSdkTransport {
                capability: "arg".to_owned(),
            }
        );

        let pending = declared_scriptlet_result("window", "ts", &["moveWindow"])
            .command_descriptor()
            .unwrap();
        assert_eq!(
            pending.availability,
            CommandAvailability::Blocked {
                reason: CommandUnavailableReason::PermissionPending,
            }
        );
        assert!(!pending.can_execute());
    }

    #[test]
    fn execution_refusal_is_identical_for_footer_actions_and_preflight() {
        let _registry_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let supported = declared_scriptlet_result("shared-ready", "ts", &["arg"]);
        assert_eq!(supported.command_execution_block_reason(), None);

        let unsupported = declared_scriptlet_result("shared-blocked", "ts", &["find"]);
        assert_eq!(
            unsupported.command_execution_block_reason(),
            Some("This command requires an SDK capability the host does not support.")
        );

        let pending = declared_scriptlet_result("shared-pending", "ts", &["moveWindow"]);
        assert_eq!(
            pending.command_execution_block_reason(),
            Some("Resolve the permission request first.")
        );
    }

    #[test]
    fn launcher_submit_preflight_accepts_ready_and_rejects_malformed_commands() {
        let ready = script_result();
        assert_eq!(ready.authorize_launcher_submit(), Ok(()));

        let mut malformed = script_result();
        let SearchResult::Script(script_match) = &mut malformed else {
            panic!("script fixture should contain a script");
        };
        Arc::get_mut(&mut script_match.script)
            .expect("fixture owns its script")
            .name
            .clear();

        assert_eq!(
            malformed.authorize_launcher_submit(),
            Err("This command has invalid metadata and cannot run.")
        );
    }

    #[test]
    fn launcher_submit_preflight_preserves_unsupported_and_permission_refusals() {
        let _registry_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let ready = declared_scriptlet_result("submit-ready", "ts", &["arg"]);
        let unsupported = declared_scriptlet_result("submit-unsupported", "ts", &["find"]);
        let pending = declared_scriptlet_result("submit-pending", "ts", &["moveWindow"]);

        assert_eq!(ready.authorize_launcher_submit(), Ok(()));
        assert_eq!(
            unsupported.authorize_launcher_submit(),
            Err("This command requires an SDK capability the host does not support.")
        );
        assert_eq!(
            pending.authorize_launcher_submit(),
            Err("Resolve the permission request first.")
        );
    }

    #[test]
    fn launcher_submit_preflight_blocks_every_synthetic_side_effect() {
        #[derive(Debug, Default, PartialEq, Eq)]
        struct SyntheticSubmitEffects {
            history_writes: usize,
            cache_invalidations: usize,
            clipboard_writes: usize,
            portal_mutations: usize,
            frecency_writes: usize,
            dispatches: usize,
        }

        fn planned_effects(result: &SearchResult) -> SyntheticSubmitEffects {
            let mut effects = SyntheticSubmitEffects::default();
            if result.authorize_launcher_submit().is_err() {
                return effects;
            }
            effects.history_writes += 1;
            effects.cache_invalidations += 1;
            effects.clipboard_writes += 1;
            effects.portal_mutations += 1;
            effects.frecency_writes += 1;
            effects.dispatches += 1;
            effects
        }

        let ready = script_result();
        assert_eq!(
            planned_effects(&ready),
            SyntheticSubmitEffects {
                history_writes: 1,
                cache_invalidations: 1,
                clipboard_writes: 1,
                portal_mutations: 1,
                frecency_writes: 1,
                dispatches: 1,
            }
        );

        let mut malformed = script_result();
        let SearchResult::Script(script_match) = &mut malformed else {
            panic!("script fixture should contain a script");
        };
        Arc::get_mut(&mut script_match.script)
            .expect("fixture owns its script")
            .name
            .clear();
        assert_eq!(
            planned_effects(&malformed),
            SyntheticSubmitEffects::default()
        );
    }

    #[test]
    fn selected_command_receipt_preserves_availability_without_leaking_raw_identity() {
        let result = script_result();
        let receipt = result
            .redacted_command_receipt()
            .expect("redacted selected command");
        let json = serde_json::to_string(&receipt).expect("serialize redacted command");

        assert_eq!(receipt.source, CommandSource::Script);
        assert_eq!(receipt.availability, CommandAvailability::Ready);
        assert!(receipt.identity.fingerprint.starts_with("sha256:"));
        assert!(!receipt.identity.raw_content_returned);
        assert!(json.contains("\"primaryAction\""));
        assert!(!json.contains("main:Hello"));
        assert!(!json.contains("hello.ts"));
    }
}
