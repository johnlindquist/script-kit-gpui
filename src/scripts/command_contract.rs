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

/// Public row target for the committed ScriptList projection. No source path,
/// URL, display label, or truncated digest escapes into the wire identifier.
pub fn main_menu_row_semantic_id(stable_key: &str) -> String {
    use sha2::Digest;
    format!(
        "main-list-row:v2:{:x}",
        sha2::Sha256::digest(stable_key.as_bytes())
    )
}

/// Producer-captured facts for one committed row. Absence means the producer
/// did not supply that fact; provider rank is never presented as relevance.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainMenuRankingEvidence {
    pub match_evidence: Option<RankingEvidence>,
    pub score: Option<i32>,
    pub tier: Option<i32>,
    pub frecency_boost: Option<i32>,
    pub exact_query_boost: Option<i32>,
    pub context_boost: Option<i32>,
    pub provider_rank: Option<usize>,
    pub provider_score: Option<f64>,
    pub frecency_score: Option<f64>,
    pub section: Option<String>,
    pub budget_limit: Option<usize>,
    pub admitted_count: Option<usize>,
    pub pin_reason: Option<&'static str>,
}

/// Internal keys only. Wire consumers attach values to opaque row IDs.
pub type MainMenuRankingEvidenceMap = std::collections::BTreeMap<String, MainMenuRankingEvidence>;

impl MainMenuRankingEvidence {
    pub(crate) fn active(result: &SearchResult) -> Self {
        let has_match = match result {
            SearchResult::Script(item) => item.match_evidence.is_some(),
            SearchResult::Scriptlet(item) => item.match_evidence.is_some(),
            SearchResult::Skill(item) => item.match_evidence.is_some(),
            SearchResult::BuiltIn(item) => item.match_evidence.is_some(),
            SearchResult::App(item) => item.match_evidence.is_some(),
            SearchResult::Window(item) => item.match_evidence.is_some(),
            _ => false,
        };
        Self {
            match_evidence: has_match.then(|| result.ranking_evidence()),
            score: Some(result.score()),
            tier: Some(result.match_tier()),
            ..Self::default()
        }
    }
}

pub(crate) fn record_main_menu_ranking_sections(
    ranking: Option<&mut MainMenuRankingEvidenceMap>,
    grouped: &[crate::list_item::GroupedListItem],
    results: &[SearchResult],
) {
    let Some(ranking) = ranking else {
        return;
    };
    let mut section = None;
    for row in grouped {
        match row {
            crate::list_item::GroupedListItem::SectionHeader(label, _) => section = Some(label),
            crate::list_item::GroupedListItem::Item(index) => {
                if let Some(key) = results
                    .get(*index)
                    .and_then(SearchResult::stable_selection_key)
                {
                    ranking.entry(key).or_default().section = section.cloned();
                }
            }
            _ => {}
        }
    }
}

// Hash length-prefixed fields, not Debug output or hash-map iteration order.
// The committed projection owns this computation; paint only reads its digest.
struct MainMenuContentDigest(sha2::Sha256);

impl MainMenuContentDigest {
    fn new() -> Self {
        use sha2::Digest;
        let mut digest = Self(sha2::Sha256::new());
        digest.text("script-kit-main-menu-content-v3");
        digest
    }
    fn bytes(&mut self, value: &[u8]) {
        use sha2::Digest;
        self.0.update((value.len() as u64).to_be_bytes());
        self.0.update(value);
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn number(&mut self, value: u64) {
        self.bytes(&value.to_be_bytes());
    }
    fn flag(&mut self, value: bool) {
        self.bytes(&[u8::from(value)]);
    }
    fn optional_text(&mut self, value: Option<&str>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.text(value);
        }
    }
    fn path(&mut self, value: &std::path::Path) {
        self.bytes(value.as_os_str().as_encoded_bytes());
    }
    fn strings(&mut self, values: &[String]) {
        self.number(values.len() as u64);
        for value in values {
            self.text(value);
        }
    }
    fn indices(&mut self, values: &[usize]) {
        self.number(values.len() as u64);
        for &value in values {
            self.number(value as u64);
        }
    }
    fn match_indices(&mut self, value: &super::MatchIndices) {
        self.indices(&value.name_indices);
        self.indices(&value.filename_indices);
        self.indices(&value.description_indices);
    }
    fn evidence(&mut self, value: Option<&MatchEvidence>) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.number(value.field as u64);
            self.text(&value.text);
            self.indices(&value.indices);
        }
    }
    fn long_text(
        &mut self,
        value: Option<&crate::scripts::search::sentence::LongTextMatchEvidence>,
    ) {
        self.flag(value.is_some());
        if let Some(value) = value {
            self.number(value.primary_field as u64);
            self.text(&value.title_text);
            self.text(&value.subtitle_text);
            self.indices(&value.title_indices);
            self.indices(&value.subtitle_indices);
            self.flag(value.hidden_excerpt.is_some());
            if let Some(excerpt) = &value.hidden_excerpt {
                self.number(excerpt.field as u64);
                self.text(&excerpt.text);
            }
        }
    }
    fn json(&mut self, value: &serde_json::Value) {
        use serde_json::Value;
        match value {
            Value::Null => self.text("null"),
            Value::Bool(value) => {
                self.text("bool");
                self.flag(*value);
            }
            Value::Number(value) => {
                self.text("number");
                self.text(&value.to_string());
            }
            Value::String(value) => {
                self.text("string");
                self.text(value);
            }
            Value::Array(values) => {
                self.text("array");
                self.number(values.len() as u64);
                for value in values {
                    self.json(value);
                }
            }
            Value::Object(values) => {
                self.text("object");
                self.number(values.len() as u64);
                let mut fields: Vec<_> = values.iter().collect();
                fields.sort_unstable_by(|a, b| a.0.cmp(b.0));
                for (key, value) in fields {
                    self.text(key);
                    self.json(value);
                }
            }
        }
    }
    #[expect(
        clippy::expect_used,
        reason = "Launcher metadata is JSON-compatible; failure must not produce a partial digest."
    )]
    fn serialized(&mut self, value: &impl Serialize) {
        self.json(&serde_json::to_value(value).expect("JSON-compatible launcher metadata"));
    }
    fn icon(&mut self, value: Option<&crate::app_launcher::DecodedIcon>) {
        self.flag(value.is_some());
        if let Some(image) = value {
            self.bytes(image.content_digest());
        }
    }
    fn script(&mut self, script: &super::Script) {
        self.path(&script.path);
        self.text(&script.name);
        self.text(&script.extension);
        self.optional_text(script.description.as_deref());
        self.optional_text(script.icon.as_deref());
        self.optional_text(script.alias.as_deref());
        self.optional_text(script.shortcut.as_deref());
        self.serialized(&script.typed_metadata);
        self.serialized(&script.schema);
        self.text(&script.plugin_id);
        self.optional_text(script.plugin_title.as_deref());
        self.optional_text(script.kit_name.as_deref());
        self.flag(script.body.is_some());
        if let Some(body) = &script.body {
            self.bytes(body.content_digest());
        }
    }
    fn finish(self) -> String {
        use sha2::Digest;
        format!("{:x}", self.0.finalize())
    }
}

impl MainMenuContentDigest {
    fn builtin_feature(&mut self, feature: &crate::builtins::BuiltInFeature) {
        use crate::builtins::BuiltInFeature;
        match feature {
            BuiltInFeature::ClipboardHistory => self.text("ClipboardHistory"),
            BuiltInFeature::PasteSequentially => self.text("PasteSequentially"),
            BuiltInFeature::Favorites => self.text("Favorites"),
            BuiltInFeature::AppLauncher => self.text("AppLauncher"),
            BuiltInFeature::WindowSwitcher => self.text("WindowSwitcher"),
            BuiltInFeature::BrowserTabs => self.text("BrowserTabs"),
            BuiltInFeature::AiChat => self.text("AiChat"),
            BuiltInFeature::Notes => self.text("Notes"),
            BuiltInFeature::EmojiPicker => self.text("EmojiPicker"),
            BuiltInFeature::SyncToGithub => self.text("SyncToGithub"),
            BuiltInFeature::FileSearch => self.text("FileSearch"),
            BuiltInFeature::Webcam => self.text("Webcam"),
            BuiltInFeature::Dictation => self.text("Dictation"),
            BuiltInFeature::DictationToAiHarness => self.text("DictationToAiHarness"),
            BuiltInFeature::DictationToFrontmostApp => self.text("DictationToFrontmostApp"),
            BuiltInFeature::DictationToNotes => self.text("DictationToNotes"),
            BuiltInFeature::DictationHistory => self.text("DictationHistory"),
            BuiltInFeature::Settings => self.text("Settings"),
            BuiltInFeature::AgentChatHistory => self.text("AgentChatHistory"),
            BuiltInFeature::AiVault => self.text("AiVault"),
            BuiltInFeature::SdkReference => self.text("SdkReference"),
            BuiltInFeature::Tips => self.text("Tips"),
            BuiltInFeature::NewScriptFromTemplate => self.text("NewScriptFromTemplate"),
            BuiltInFeature::MigrateV1Scripts => self.text("MigrateV1Scripts"),
            BuiltInFeature::BackgroundEffectNext => self.text("BackgroundEffectNext"),
            BuiltInFeature::BackgroundEffectPrevious => self.text("BackgroundEffectPrevious"),
            BuiltInFeature::BackgroundEffectOff => self.text("BackgroundEffectOff"),
            BuiltInFeature::NewFlow => self.text("NewFlow"),
            BuiltInFeature::AiChatVariant(value) => {
                self.text("AiChatVariant");
                self.number(*value as u64);
            }
            BuiltInFeature::SystemAction(value) => {
                self.text("SystemAction");
                self.number(*value as u64);
            }
            BuiltInFeature::NotesCommand(value) => {
                self.text("NotesCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::AiCommand(value) => {
                self.text("AiCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::ScriptCommand(value) => {
                self.text("ScriptCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::PermissionCommand(value) => {
                self.text("PermissionCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::FrecencyCommand(value) => {
                self.text("FrecencyCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::SettingsCommand(value) => {
                self.text("SettingsCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::UtilityCommand(value) => {
                self.text("UtilityCommand");
                self.number(*value as u64);
            }
            BuiltInFeature::FlowUxVariant(value) => {
                self.text("FlowUxVariant");
                self.number(*value as u64);
            }
            BuiltInFeature::App(path) => {
                self.text("App");
                self.text(path);
            }
            BuiltInFeature::MenuBarAction(action) => {
                self.text("MenuBarAction");
                self.text(&action.bundle_id);
                self.strings(&action.menu_path);
                self.flag(action.enabled);
                self.optional_text(action.shortcut.as_deref());
            }
        }
    }
    fn fallback_action(&mut self, action: &crate::fallbacks::builtins::FallbackAction) {
        use crate::fallbacks::builtins::FallbackAction;
        match action {
            FallbackAction::RunInTerminal => self.text("RunInTerminal"),
            FallbackAction::AddToNotes => self.text("AddToNotes"),
            FallbackAction::CopyToClipboard => self.text("CopyToClipboard"),
            FallbackAction::OpenUrl => self.text("OpenUrl"),
            FallbackAction::Calculate => self.text("Calculate"),
            FallbackAction::OpenFile => self.text("OpenFile"),
            FallbackAction::SearchFiles => self.text("SearchFiles"),
            FallbackAction::SendToAiHarness => self.text("SendToAiHarness"),
            FallbackAction::SearchUrl { template } => {
                self.text("SearchUrl");
                self.text(template);
            }
            FallbackAction::ExecuteBuiltin { builtin_id } => {
                self.text("ExecuteBuiltin");
                self.text(builtin_id);
            }
        }
    }
    fn spine(&mut self, row: &crate::spine::SpineListRow) {
        use crate::spine::{SpineListAction, SpineListRowKind};
        self.optional_text(row.meta.as_deref().map(|value| &**value));
        self.optional_text(row.icon.as_deref().map(|value| &**value));
        self.number(row.badges.len() as u64);
        for badge in &row.badges {
            self.text(badge);
        }
        self.flag(row.is_selectable);
        self.optional_text(row.action_label.as_deref().map(|value| &**value));
        match &row.kind {
            SpineListRowKind::ContextBuiltin { context_type } => {
                self.text("ContextBuiltin");
                self.text(context_type);
            }
            SpineListRowKind::ContextSubSearch { context_type } => {
                self.text("ContextSubSearch");
                self.text(context_type);
            }
            SpineListRowKind::ContextResult {
                context_type,
                result_id,
            } => {
                self.text("ContextResult");
                self.text(context_type);
                self.text(result_id);
            }
            SpineListRowKind::SlashCommand { command } => {
                self.text("SlashCommand");
                self.text(command);
            }
            SpineListRowKind::Profile { profile_id } => {
                self.text("Profile");
                self.text(profile_id);
            }
            SpineListRowKind::Style { style_id } => {
                self.text("Style");
                self.text(style_id);
            }
            SpineListRowKind::CaptureTarget { target } => {
                self.text("CaptureTarget");
                self.text(target);
            }
            SpineListRowKind::Flow { flow_id } => {
                self.text("Flow");
                self.text(flow_id);
            }
            SpineListRowKind::Hint => self.text("Hint"),
            SpineListRowKind::Empty => self.text("Empty"),
        }
        match &row.action {
            SpineListAction::InsertSegmentText {
                segment_index,
                segment_byte_range,
                text,
                trailing_space,
            } => {
                self.text("InsertSegmentText");
                self.number(*segment_index as u64);
                self.number(segment_byte_range.start as u64);
                self.number(segment_byte_range.end as u64);
                self.text(text);
                self.flag(*trailing_space);
            }
            SpineListAction::ResolveSegment {
                segment_index,
                segment_byte_range,
                replacement,
                resolution_id,
                resolution_label,
                resolution_source,
                trailing_space,
            } => {
                self.text("ResolveSegment");
                self.number(*segment_index as u64);
                self.number(segment_byte_range.start as u64);
                self.number(segment_byte_range.end as u64);
                self.text(replacement);
                self.text(resolution_id);
                self.text(resolution_label);
                self.text(resolution_source);
                self.flag(*trailing_space);
            }
            SpineListAction::OpenModeExit { sigil, rest } => {
                self.text("OpenModeExit");
                self.number(*sigil as u64);
                self.text(rest);
            }
            SpineListAction::OpenFileSearchPortal {
                segment_index,
                segment_byte_range,
                query,
            } => {
                self.text("OpenFileSearchPortal");
                self.number(*segment_index as u64);
                self.number(segment_byte_range.start as u64);
                self.number(segment_byte_range.end as u64);
                self.text(query);
            }
            SpineListAction::AcceptMenuSyntaxTrigger { row_id } => {
                self.text("AcceptMenuSyntaxTrigger");
                self.text(row_id);
            }
            SpineListAction::AcceptMenuSyntaxObject { row_id } => {
                self.text("AcceptMenuSyntaxObject");
                self.text(row_id);
            }
            SpineListAction::AttachContextResult { source } => {
                self.text("AttachContextResult");
                self.text(source);
            }
            SpineListAction::Noop => self.text("Noop"),
        }
    }
}

impl SearchResult {
    /// Hash actual display, execution and eligibility inputs once at projection
    /// commit. Ranking scores and provider positions are deliberately not content.
    pub fn main_menu_content_fingerprint(&self) -> String {
        let mut digest = MainMenuContentDigest::new();
        digest.text(self.command_source().prefix());
        digest.optional_text(self.stable_selection_key().as_deref());
        digest.text(self.name());
        digest.optional_text(self.description());
        digest.optional_text(self.source_name());
        digest.text(self.get_default_action_text());
        match self {
            Self::Script(item) => {
                digest.script(&item.script);
                digest.text(&item.filename);
                digest.match_indices(&item.match_indices);
                digest.evidence(item.match_evidence.as_ref());
                digest.number(item.match_kind.clone() as u64);
                digest.flag(item.content_match.is_some());
                if let Some(content) = &item.content_match {
                    digest.number(content.line_number as u64);
                    digest.text(&content.line_text);
                    digest.indices(&content.line_match_indices);
                    digest.number(content.byte_range.start as u64);
                    digest.number(content.byte_range.end as u64);
                }
                digest.serialized(&script_command_availability(&item.script));
            }
            Self::Scriptlet(item) => {
                let snippet = &item.scriptlet;
                digest.text(&snippet.code);
                digest.text(&snippet.tool);
                digest.optional_text(snippet.shortcut.as_deref());
                digest.optional_text(snippet.keyword.as_deref());
                digest.optional_text(snippet.group.as_deref());
                digest.text(&snippet.plugin_id);
                digest.optional_text(snippet.plugin_title.as_deref());
                digest.optional_text(snippet.file_path.as_deref());
                digest.optional_text(snippet.command.as_deref());
                digest.optional_text(snippet.alias.as_deref());
                digest.optional_text(snippet.icon.as_deref());
                digest.optional_text(item.display_file_path.as_deref());
                digest.match_indices(&item.match_indices);
                digest.evidence(item.match_evidence.as_ref());
                digest.serialized(&scriptlet_command_availability(snippet));
            }
            Self::Flow(item) => {
                digest.match_indices(&item.match_indices);
                digest.flag(item.flow.is_some());
                if let Some(flow) = &item.flow {
                    digest.text(&flow.id);
                    digest.text(&flow.path);
                    digest.text(flow.source.label());
                    digest.text(&flow.name);
                    digest.optional_text(flow.description.as_deref());
                    digest.text(&flow.engine);
                    digest.optional_text(flow.engine_source.as_deref());
                    digest.flag(flow.is_workflow);
                    digest.flag(flow.interactive);
                    digest.number(flow.mtime_ms);
                    digest.optional_text(flow.origin.as_deref());
                    digest.optional_text(flow.wrapper_command.as_deref());
                    digest.number(flow.inputs.len() as u64);
                    for input in &flow.inputs {
                        digest.text(&input.name);
                        digest.number(input.input_type as u64);
                        digest.optional_text(input.message.as_deref());
                        digest.strings(&input.options);
                        digest.serialized(&input.default);
                    }
                }
            }
            Self::Skill(item) => {
                digest.path(&item.skill.path);
                digest.text(&item.skill.plugin_id);
                digest.text(&item.skill.plugin_title);
                digest.text(&item.skill.skill_id);
                digest.match_indices(&item.match_indices);
                digest.evidence(item.match_evidence.as_ref());
            }
            Self::BuiltIn(item) => {
                digest.builtin_feature(&item.entry.feature);
                digest.number(item.entry.group as u64);
                digest.strings(&item.entry.keywords);
                digest.optional_text(item.entry.icon.as_deref());
                digest.evidence(item.match_evidence.as_ref());
            }
            Self::App(item) => {
                digest.path(&item.app.path);
                digest.optional_text(item.app.bundle_id.as_deref());
                digest.icon(item.app.icon.as_ref());
                digest.evidence(item.match_evidence.as_ref());
            }
            Self::Window(item) => {
                let window = &item.window;
                digest.number(window.id as u64);
                digest.number(window.pid as u64);
                digest.text(&window.app);
                digest.optional_text(window.bundle_id.as_deref());
                digest.flag(window.app_path.is_some());
                if let Some(path) = &window.app_path {
                    digest.path(path);
                }
                digest.number(window.window_index as u64);
                digest.flag(window.is_frontmost_app);
                digest.flag(window.is_focused);
                digest.flag(window.is_main);
                digest.flag(window.is_minimized);
                digest.flag(window.is_on_current_space);
                digest.text(&window.descriptor);
                digest.icon(item.app_icon.as_ref());
                digest.evidence(item.match_evidence.as_ref());
            }
            Self::File(item) => {
                digest.text(&item.file.path);
                digest.number(item.file.size);
                digest.number(item.file.modified);
                digest.number(item.file.file_type as u64);
            }
            Self::Note(item) => {
                digest.bytes(item.hit.id.0.as_bytes());
                digest.text(&item.hit.title);
                digest.serialized(&item.hit.updated_at);
                digest.flag(item.hit.is_pinned);
                digest.number(item.hit.char_count as u64);
            }
            Self::BrainHit(item) => {
                digest.text(item.hit.source.as_str());
                digest.text(&item.hit.source_id);
                digest.text(&item.hit.excerpt);
                digest.text(item.hit.source_label);
            }
            Self::BrainInboxItem(item) => {
                digest.number(item.item.id as u64);
                digest.text(item.item.kind.as_str());
                digest.text(&item.item.detail);
                digest.text(&item.item.source);
                digest.text(&item.item.source_id);
                digest.number(item.item.created_at as u64);
                digest.serialized(&item.item.resolved_at);
            }
            Self::Todo(item) => {
                digest.text(&item.hit.body);
                digest.strings(&item.hit.tags);
                digest.serialized(&item.hit.priority);
                digest.optional_text(item.hit.due.as_deref());
                digest.optional_text(item.hit.created_at.as_deref());
                digest.path(&item.hit.path);
                digest.serialized(&item.hit.line_number);
                digest.text(&item.hit.raw_line);
            }
            Self::AgentChatHistory(item) => {
                digest.serialized(&item.entry);
                digest.long_text(item.evidence.as_ref());
            }
            Self::AiVault(item) => {
                digest.text(&item.hit.provider);
                digest.text(&item.hit.provider_display_name);
                digest.text(&item.hit.session_id);
                digest.optional_text(item.hit.source_kind.as_deref());
                digest.optional_text(item.hit.workspace_path.as_deref());
                digest.optional_text(item.hit.model.as_deref());
                digest.optional_text(item.hit.modified_at.as_deref());
            }
            Self::ClipboardHistory(item) => {
                let entry = &item.entry;
                digest.text(&entry.id);
                digest.number(entry.content_type as u64);
                digest.number(entry.timestamp as u64);
                digest.flag(entry.pinned);
                digest.text(&entry.text_preview);
                digest.serialized(&entry.image_width);
                digest.serialized(&entry.image_height);
                digest.number(entry.byte_size as u64);
                digest.optional_text(entry.ocr_text.as_deref());
            }
            Self::DictationHistory(item) => {
                digest.text(&item.id);
                digest.text(&item.target);
                digest.text(&item.timestamp);
                digest.number(item.audio_duration_ms);
                digest.long_text(item.evidence.as_ref());
            }
            Self::BrowserTab(item) => {
                digest.text(&item.hit.url);
                digest.text(&item.hit.provider_label);
                digest.text(&item.hit.domain);
                digest.text(&item.hit.tab.browser_name);
                digest.text(&item.hit.tab.browser_bundle_id);
                digest.number(item.hit.tab.window_index as u64);
                digest.number(item.hit.tab.tab_index as u64);
                digest.text(&item.hit.tab.title);
                digest.text(&item.hit.tab.url);
            }
            Self::BrowserHistory(item) => {
                digest.text(&item.hit.provider_label);
                digest.text(&item.hit.profile_label);
                digest.text(&item.hit.url);
                digest.text(&item.hit.domain);
                digest.number(item.hit.last_visit_unix_ms as u64);
                digest.number(item.hit.visit_count as u64);
            }
            Self::Agent(item) => {
                digest.path(&item.agent.path);
                digest.text(&item.display_name);
                digest.optional_text(item.agent.icon.as_deref());
                digest.optional_text(item.agent.shortcut.as_deref());
                digest.optional_text(item.agent.alias.as_deref());
                digest.match_indices(&item.match_indices);
            }
            Self::Fallback(item) => {
                digest.text(&item.display_label());
                digest.text(&item.display_name());
                digest.text(&item.display_description());
                digest.text(item.fallback.icon());
                match &item.fallback {
                    crate::fallbacks::collector::FallbackItem::Builtin(fallback) => {
                        digest.text("builtin");
                        digest.text(fallback.id);
                        digest.flag(fallback.enabled);
                        use crate::fallbacks::FallbackCondition;
                        match &fallback.condition {
                            FallbackCondition::Always => digest.text("always"),
                            FallbackCondition::WhenUrl => digest.text("url"),
                            FallbackCondition::WhenMath => digest.text("math"),
                            FallbackCondition::WhenFilePath => digest.text("file-path"),
                            FallbackCondition::WhenInputType(input_type) => {
                                digest.text("input-type");
                                digest.number(input_type.clone() as u64);
                            }
                        }
                        digest.fallback_action(&fallback.action);
                    }
                    crate::fallbacks::collector::FallbackItem::Script(fallback) => {
                        digest.text("script");
                        digest.script(&fallback.script);
                        digest.text(&fallback.label);
                        digest.text(&fallback.label_template);
                        digest.serialized(&script_command_availability(&fallback.script));
                    }
                }
            }
            Self::ScriptIssue(item) => {
                digest.number(item.failed_count as u64);
                digest.number(item.fatal_count as u64);
                digest.number(item.warning_count as u64);
            }
            Self::SpineProjection(row) => digest.spine(row),
        }
        digest.finish()
    }
}

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
        self.command_descriptor_with_optional_host(None)
    }

    fn command_descriptor_with_optional_host(
        &self,
        host: Option<&crate::mcp_resources::SdkHostAvailability>,
    ) -> Result<CommandDescriptor, ContractError> {
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
                    input_fields.sort_by_key(|(left, _)| *left);
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
                descriptor.availability = match host {
                    Some(host) => command_availability_from_validation_issues(
                        super::validate_declared_sdk_capabilities_with_host_availability(
                            script, host,
                        ),
                    ),
                    None => script_command_availability(script),
                };
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
                descriptor.availability = match host {
                    Some(host) => command_availability_from_validation_issues(
                        super::validate_scriptlet_capabilities_with_host_availability(
                            scriptlet, host,
                        ),
                    ),
                    None => scriptlet_command_availability(scriptlet),
                };
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
            Self::SpineProjection(row) => {
                descriptor.availability = if !row.is_selectable {
                    CommandAvailability::TemporarilyUnavailable
                } else {
                    match &row.action {
                        crate::spine::SpineListAction::Noop => CommandAvailability::Suppressed,
                        crate::spine::SpineListAction::AcceptMenuSyntaxTrigger { row_id }
                        | crate::spine::SpineListAction::AcceptMenuSyntaxObject { row_id }
                            if row_id.is_empty() =>
                        {
                            CommandAvailability::Suppressed
                        }
                        crate::spine::SpineListAction::AttachContextResult { source }
                            if !matches!(source.as_ref(), "calendar" | "notifications") =>
                        {
                            CommandAvailability::Suppressed
                        }
                        _ => CommandAvailability::Ready,
                    }
                };
                if let Some(primary) = descriptor.actions.first_mut() {
                    primary.availability = descriptor.availability.clone();
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
            | Self::ScriptIssue(_) => {}
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
        self.command_descriptor_with_optional_host(Some(host))
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
    /// Optional host facts come from an existing owned inventory, never a probe.
    pub fn redacted_command_receipt(
        &self,
        host: Option<&crate::mcp_resources::SdkHostAvailability>,
    ) -> Result<LauncherCommandReceipt, ContractError> {
        let descriptor = self.command_descriptor_with_optional_host(host)?;
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
    fn spine_action_result(
        action: crate::spine::SpineListAction,
        selectable: bool,
    ) -> SearchResult {
        SearchResult::SpineProjection(crate::spine::SpineListRow {
            id: "spine-action-contract".into(),
            kind: crate::spine::SpineListRowKind::Hint,
            title: "Visible row".into(),
            subtitle: None,
            meta: None,
            icon: None,
            badges: Vec::new(),
            score: 0,
            is_selectable: selectable,
            action_label: None,
            action,
        })
    }

    #[test]
    fn spine_noop_explanations_remain_selectable_but_cannot_authorize_dispatch() {
        use crate::spine::SpineListAction;
        for selectable in [false, true] {
            let result = spine_action_result(SpineListAction::Noop, selectable);
            let descriptor = result
                .command_descriptor()
                .expect("explanations stay inspectable");
            assert!(!descriptor.can_execute());
            assert!(!descriptor
                .primary_action()
                .unwrap()
                .availability
                .is_executable());
            assert!(result.authorize_launcher_submit().is_err());
            let SearchResult::SpineProjection(row) = result else {
                unreachable!()
            };
            assert_eq!(row.is_selectable, selectable);
            assert_eq!(row.default_action_text(), "No Action");
        }
    }

    #[test]
    fn owner_backed_spine_actions_have_real_availability_and_payload_fingerprints() {
        use crate::spine::SpineListAction;
        for (action, changed, verb) in [
            (
                SpineListAction::AcceptMenuSyntaxTrigger {
                    row_id: "capture-a".into(),
                },
                SpineListAction::AcceptMenuSyntaxTrigger {
                    row_id: "capture-b".into(),
                },
                "Accept",
            ),
            (
                SpineListAction::AcceptMenuSyntaxObject {
                    row_id: "object-a".into(),
                },
                SpineListAction::AcceptMenuSyntaxObject {
                    row_id: "object-b".into(),
                },
                "Insert",
            ),
            (
                SpineListAction::AttachContextResult {
                    source: "calendar".into(),
                },
                SpineListAction::AttachContextResult {
                    source: "notifications".into(),
                },
                "Attach",
            ),
        ] {
            let result = spine_action_result(action.clone(), true);
            assert!(result.command_descriptor().unwrap().can_execute());
            assert!(result.authorize_launcher_submit().is_ok());
            let SearchResult::SpineProjection(row) = &result else {
                unreachable!()
            };
            assert_eq!(row.default_action_text(), verb);
            let replacement = spine_action_result(changed, true);
            assert_eq!(
                result.stable_selection_key(),
                replacement.stable_selection_key()
            );
            assert_ne!(
                result.main_menu_content_fingerprint(),
                replacement.main_menu_content_fingerprint()
            );
            assert!(!spine_action_result(action, false)
                .command_descriptor()
                .unwrap()
                .can_execute());
        }
        for action in [
            SpineListAction::AttachContextResult {
                source: "unknown-provider".into(),
            },
            SpineListAction::AcceptMenuSyntaxTrigger { row_id: "".into() },
            SpineListAction::AcceptMenuSyntaxObject { row_id: "".into() },
        ] {
            assert!(spine_action_result(action, true)
                .authorize_launcher_submit()
                .is_err());
        }
    }

    #[test]
    fn same_identity_detects_builtin_action_and_spine_eligibility_transitions() {
        let mut builtin = SearchResult::BuiltIn(crate::scripts::BuiltInMatch {
            entry: crate::builtins::BuiltInEntry {
                id: "builtin/test".into(),
                name: "Same".into(),
                description: "Same".into(),
                keywords: vec![],
                feature: crate::builtins::BuiltInFeature::Settings,
                icon: None,
                group: crate::builtins::BuiltInGroup::Core,
            },
            score: 0,
            match_evidence: None,
        });
        let identity = builtin.stable_selection_key();
        let before = builtin.main_menu_content_fingerprint();
        let SearchResult::BuiltIn(item) = &mut builtin else {
            unreachable!()
        };
        item.entry.feature = crate::builtins::BuiltInFeature::Notes;
        assert_eq!(builtin.stable_selection_key(), identity);
        assert_ne!(builtin.main_menu_content_fingerprint(), before);

        let mut spine = SearchResult::SpineProjection(crate::spine::SpineListRow {
            id: "spine-fixture".into(),
            kind: crate::spine::SpineListRowKind::Hint,
            title: "Same".into(),
            subtitle: None,
            meta: None,
            icon: None,
            badges: vec![],
            score: 0,
            is_selectable: false,
            action_label: None,
            action: crate::spine::SpineListAction::Noop,
        });
        let inert = spine.main_menu_content_fingerprint();
        let identity = spine.stable_selection_key();
        let SearchResult::SpineProjection(row) = &mut spine else {
            unreachable!()
        };
        row.is_selectable = true;
        assert_eq!(spine.stable_selection_key(), identity);
        assert_ne!(spine.main_menu_content_fingerprint(), inert);
        let enabled = spine.main_menu_content_fingerprint();
        let SearchResult::SpineProjection(row) = &mut spine else {
            unreachable!()
        };
        row.action = crate::spine::SpineListAction::OpenModeExit {
            sigil: '@',
            rest: "file".into(),
        };
        assert_ne!(spine.main_menu_content_fingerprint(), enabled);
    }

    #[test]
    fn main_menu_semantic_id_is_full_sha256_and_never_exposes_source_text() {
        assert_eq!(
            main_menu_row_semantic_id("abc"),
            "main-list-row:v2:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let private_key = "file//Users/private/client/secret.txt";
        let semantic = main_menu_row_semantic_id(private_key);
        assert_eq!(semantic.len(), "main-list-row:v2:".len() + 64);
        assert!(!semantic.contains("private"));
        assert_ne!(
            semantic,
            main_menu_row_semantic_id("file//Users/private/client/other.txt")
        );
    }

    #[test]
    fn identical_content_ignores_ranking_but_detects_body_and_action_changes() {
        let mut result = script_result();
        let SearchResult::Script(item) = &mut result else {
            unreachable!()
        };
        Arc::make_mut(&mut item.script).body = Some("console.log('original')".into());
        let fingerprint = result.main_menu_content_fingerprint();
        assert_eq!(fingerprint.len(), 64);
        let mut ranked = result.clone();
        let SearchResult::Script(item) = &mut ranked else {
            unreachable!()
        };
        item.score += 100;
        item.match_evidence.as_mut().unwrap().score += 100;
        assert_eq!(fingerprint, ranked.main_menu_content_fingerprint());
        let SearchResult::Script(item) = &mut ranked else {
            unreachable!()
        };
        Arc::make_mut(&mut item.script).body = Some("console.log('changed')".into());
        assert_eq!(result.stable_selection_key(), ranked.stable_selection_key());
        assert_ne!(fingerprint, ranked.main_menu_content_fingerprint());
        let mut action = result.clone();
        let SearchResult::Script(item) = &mut action else {
            unreachable!()
        };
        Arc::make_mut(&mut item.script).typed_metadata =
            Some(crate::metadata_parser::TypedMetadata {
                enter: Some("Review changes".into()),
                ..Default::default()
            });
        assert_ne!(fingerprint, action.main_menu_content_fingerprint());
    }

    #[test]
    fn content_fingerprint_has_unambiguous_boundaries_and_canonical_maps() {
        let mut first = script_result();
        let mut second = script_result();
        for (result, name, description, reverse) in [
            (&mut first, "ab", "c", false),
            (&mut second, "a", "bc", true),
        ] {
            let SearchResult::Script(item) = result else {
                unreachable!()
            };
            let script = Arc::make_mut(&mut item.script);
            script.name = name.into();
            script.description = Some(description.into());
            script.path = "/tmp/shared-source.ts".into();
            let mut metadata = crate::metadata_parser::TypedMetadata::default();
            for key in if reverse { ["z", "a"] } else { ["a", "z"] } {
                metadata
                    .extra
                    .insert(key.into(), serde_json::json!({"inner": key}));
            }
            script.typed_metadata = Some(metadata);
        }
        assert_eq!(first.stable_selection_key(), second.stable_selection_key());
        assert_ne!(
            first.main_menu_content_fingerprint(),
            second.main_menu_content_fingerprint()
        );
        let SearchResult::Script(item) = &mut second else {
            unreachable!()
        };
        let script = Arc::make_mut(&mut item.script);
        script.name = "ab".into();
        script.description = Some("c".into());
        assert_eq!(
            first.main_menu_content_fingerprint(),
            second.main_menu_content_fingerprint()
        );
    }

    #[test]
    fn app_installation_identity_survives_metadata_and_separates_same_names_and_bundles() {
        let app = |path: &str, bundle_id: Option<&str>| {
            SearchResult::App(crate::scripts::AppMatch {
                app: crate::app_launcher::AppInfo {
                    name: "Editor".into(),
                    path: path.into(),
                    bundle_id: bundle_id.map(str::to_owned),
                    icon: None,
                },
                score: 1,
                match_evidence: None,
            })
        };
        for bundle in [None, Some("com.example.editor")] {
            let first = app("/Applications/Editor.app", bundle);
            let second = app("/Applications/Other/Editor.app", bundle);
            assert_ne!(first.stable_selection_key(), second.stable_selection_key());
            let mut renamed = first.clone();
            let SearchResult::App(item) = &mut renamed else {
                unreachable!()
            };
            item.app.name = "Better Editor".into();
            item.app.bundle_id = Some("com.example.metadata-renamed".into());
            assert_eq!(first.stable_selection_key(), renamed.stable_selection_key());
            assert_ne!(
                first.main_menu_content_fingerprint(),
                renamed.main_menu_content_fingerprint()
            );
        }
        assert_eq!(app("", None).stable_selection_key(), None);
    }

    #[test]
    fn fallback_identity_is_source_owned_while_action_changes_conflict() {
        use crate::fallbacks::{BuiltinFallback, FallbackAction, FallbackCondition, FallbackItem};
        let fallback = |id, template: &str| {
            SearchResult::Fallback(crate::scripts::FallbackMatch::new(
                FallbackItem::Builtin(BuiltinFallback::new(
                    id,
                    "Search",
                    "Same label",
                    "search",
                    FallbackAction::SearchUrl {
                        template: template.into(),
                    },
                    FallbackCondition::Always,
                    20,
                )),
                0,
            ))
        };
        let first = fallback("engine-a", "https://a.example/{query}");
        let same = fallback("engine-a", "https://a.example/{query}");
        let other = fallback("engine-b", "https://a.example/{query}");
        let changed = fallback("engine-a", "https://b.example/{query}");
        assert_ne!(first.stable_selection_key(), other.stable_selection_key());
        assert_eq!(first.stable_selection_key(), changed.stable_selection_key());
        assert_eq!(
            first.main_menu_content_fingerprint(),
            same.main_menu_content_fingerprint()
        );
        assert_ne!(
            first.main_menu_content_fingerprint(),
            changed.main_menu_content_fingerprint()
        );
        let mut handoff = first.clone();
        let SearchResult::Fallback(item) = &mut handoff else {
            unreachable!()
        };
        item.stable_selection_key_override =
            Some("fallback/root-file-search-handoff/global".into());
        item.title_override = Some("Search files now".into());
        assert_eq!(
            handoff.stable_selection_key().as_deref(),
            Some("fallback/root-file-search-handoff/global")
        );
    }

    #[test]
    fn same_named_script_fallbacks_remain_distinct_from_each_other_and_regular_scripts() {
        let first = script_result();
        let mut second = first.clone();
        let SearchResult::Script(item) = &mut second else {
            unreachable!()
        };
        Arc::make_mut(&mut item.script).path = "/tmp/other-fallback.ts".into();
        let wrap = |result: &SearchResult| {
            let SearchResult::Script(item) = result else {
                unreachable!()
            };
            SearchResult::Fallback(crate::scripts::FallbackMatch::new(
                crate::fallbacks::FallbackItem::Script(crate::scripts::FallbackConfig {
                    script: item.script.clone(),
                    label: "Search with it".into(),
                    label_template: "Search with {input}".into(),
                }),
                0,
            ))
        };
        assert_ne!(
            wrap(&first).stable_selection_key(),
            wrap(&second).stable_selection_key()
        );
        assert_ne!(
            first.stable_selection_key(),
            wrap(&first).stable_selection_key()
        );
    }

    #[test]
    fn note_and_brain_projections_never_share_identity() {
        let note = SearchResult::Note(crate::scripts::NoteMatch {
            hit: crate::notes::RootNoteSearchHit {
                id: crate::notes::NoteId::parse("00000000-0000-0000-0000-000000000001").unwrap(),
                title: "Same".into(),
                updated_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
                is_pinned: false,
                char_count: 4,
                score: 1,
            },
            title: "Same".into(),
            subtitle: "Same".into(),
            score: 1,
        });
        let brain = SearchResult::BrainHit(crate::scripts::BrainMatch {
            hit: crate::brain::RootBrainSearchHit {
                title: "Same".into(),
                excerpt: "Same".into(),
                source_label: "Notes",
                source: crate::brain::DocSource::Note,
                source_id: "00000000-0000-0000-0000-000000000001".into(),
            },
            subtitle: "Same".into(),
            score: 1,
        });
        assert_ne!(note.stable_selection_key(), brain.stable_selection_key());
        assert_ne!(
            note.main_menu_content_fingerprint(),
            brain.main_menu_content_fingerprint()
        );
    }

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
            for (host, descriptor) in [(Some(&granted), &ready), (Some(&denied), &refused)] {
                let receipt = result
                    .redacted_command_receipt(host)
                    .expect("canonical host-aware receipt");
                assert_eq!(receipt.availability, descriptor.availability);
                assert_eq!(
                    receipt.primary_action,
                    *descriptor.primary_action().unwrap()
                );
                assert!(!receipt.identity.raw_content_returned);
                let json = serde_json::to_string(&receipt).expect("serialize redacted receipt");
                assert!(!json.contains(descriptor.identity.as_str()));
            }
            assert_eq!(
                result.redacted_command_receipt(None).unwrap().availability,
                result.command_descriptor().unwrap().availability
            );
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
        let first_key = issue.stable_selection_key();
        let first_content = issue.main_menu_content_fingerprint();
        if let SearchResult::ScriptIssue(value) = &mut issue {
            value.title = "Three scripts need attention".to_owned();
            value.failed_count = 3;
        }
        assert_eq!(issue.command_identity().unwrap(), first);
        assert_eq!(issue.stable_selection_key(), first_key);
        assert_ne!(issue.main_menu_content_fingerprint(), first_content);
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
            .redacted_command_receipt(None)
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
