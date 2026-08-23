use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Canonical label for the Ask Anything ambient context chip.
pub const ASK_ANYTHING_LABEL: &str = "Ask Anything";

/// Canonical resource URI for the Ask Anything minimal desktop context.
pub const ASK_ANYTHING_RESOURCE_URI: &str = "kit://context?profile=minimal";

const DEFERRED_AMBIENT_CAPTURE_LABELS: &[&str] = &[
    ASK_ANYTHING_LABEL,
    "Full Screen",
    "Focused Window",
    "Selected Text",
    "Browser Tab",
];

/// A typed context part that can be attached to an AI composer message.
///
/// Each variant represents a different source of context that will be
/// resolved into a prompt block at submit time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AiContextPart {
    /// An MCP resource URI (e.g. `kit://context?profile=minimal`)
    ResourceUri { uri: String, label: String },
    /// A local file path attachment
    FilePath { path: String, label: String },
    /// A local skill attachment selected from slash-mode or the main menu.
    SkillFile {
        path: String,
        label: String,
        skill_name: String,
        owner_label: String,
        slash_name: String,
    },
    /// A focused UI target resolved from the active surface (e.g. a selected
    /// script, clipboard entry, or file). Carries the full target context so
    /// it can be rendered as a chip and resolved into a deterministic prompt
    /// block at submit time.
    FocusedTarget {
        target: crate::ai::tab_context::TabAiTargetContext,
        label: String,
    },
    /// Display-only ambient context chip. Represents promoted Ask Anything
    /// context that has already been staged as `pending_context_blocks`.
    /// Resolves to an empty prompt block (the real content lives in the
    /// staged blocks).
    AmbientContext { label: String },
    /// A raw text block — terminal logs, pasted snippets, URLs, or note
    /// content stashed into a Context Cart session. Resolves into a
    /// `<context>` block at submit time.
    TextBlock {
        label: String,
        source: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

impl AiContextPart {
    pub fn label(&self) -> &str {
        match self {
            Self::ResourceUri { label, .. }
            | Self::FilePath { label, .. }
            | Self::SkillFile { label, .. }
            | Self::FocusedTarget { label, .. }
            | Self::AmbientContext { label }
            | Self::TextBlock { label, .. } => label,
        }
    }

    /// Returns the originating URI or file path for this context part.
    pub fn source(&self) -> &str {
        match self {
            Self::ResourceUri { uri, .. } => uri,
            Self::FilePath { path, .. } => path,
            Self::SkillFile { path, .. } => path,
            Self::FocusedTarget { target, .. } => &target.semantic_id,
            Self::AmbientContext { .. } => "ambient://ask-anything",
            Self::TextBlock { source, .. } => source,
        }
    }

    /// Stable owner identity for context selection and synchronization.
    /// Labels and editable content may change without turning one selected
    /// file, command, transcript, or resource into a different attachment.
    pub(crate) fn has_same_attachment_owner(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && self.source() == other.source()
    }

    /// Returns `true` when this part is the initial Ask Anything resource
    /// chip (before promotion to `AmbientContext`).
    pub fn is_ask_anything_resource(&self) -> bool {
        matches!(
            self,
            Self::ResourceUri { uri, label }
                if uri == ASK_ANYTHING_RESOURCE_URI && label == ASK_ANYTHING_LABEL
        )
    }

    /// Returns `true` when this part is a promoted ambient context chip.
    pub fn is_ambient_context_chip(&self) -> bool {
        matches!(self, Self::AmbientContext { .. })
    }

    /// Returns `true` only for resource chips that must wait on a deferred
    /// ambient capture task before submit.
    ///
    /// Inline picker attachments such as `@context` also use the minimal
    /// desktop-context URI, but they should resolve directly on submit rather
    /// than entering the ambient bootstrap state machine.
    pub fn is_ambient_bootstrap_resource(&self) -> bool {
        matches!(
            self,
            Self::ResourceUri { uri, label }
                if uri == ASK_ANYTHING_RESOURCE_URI
                    && DEFERRED_AMBIENT_CAPTURE_LABELS.contains(&label.as_str())
        )
    }

    /// Return the display label for an ambient bootstrap or promoted ambient chip.
    pub fn ambient_chip_label(&self) -> Option<&str> {
        match self {
            Self::ResourceUri { uri, label } if uri == ASK_ANYTHING_RESOURCE_URI => {
                Some(label.as_str())
            }
            Self::AmbientContext { label } => Some(label.as_str()),
            _ => None,
        }
    }

    /// Renderer-neutral context-chip identity. Content never participates: the
    /// digest uses only part kind, source identity, and the user-visible label.
    pub(crate) fn semantic_chip_projection(&self, removable: bool) -> AiContextChipProjection {
        let kind = match self {
            Self::ResourceUri { .. } => "resource",
            Self::FilePath { .. } => "file",
            Self::SkillFile { .. } => "skill",
            Self::FocusedTarget { .. } => "focused",
            Self::AmbientContext { .. } => "ambient",
            Self::TextBlock { .. } => "text",
        };
        let mut hash = 0xcbf29ce484222325u64;
        for byte in kind
            .bytes()
            .chain([0])
            .chain(self.source().bytes())
            .chain([0])
            .chain(self.label().bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        AiContextChipProjection {
            semantic_id: format!("agent-chat-context-{kind}-{hash:016x}"),
            label: self.label().to_string(),
            removable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AiContextChipProjection {
    pub(crate) semantic_id: String,
    pub(crate) label: String,
    pub(crate) removable: bool,
}

/// Extract file paths from a slice of context parts.
///
/// Returns only the `path` values from `AiContextPart::FilePath` variants,
/// preserving order. This is the canonical way to derive the attachment list
/// from the single source of truth (`pending_context_parts`).
pub fn file_path_parts(parts: &[AiContextPart]) -> Vec<String> {
    parts
        .iter()
        .filter_map(|part| match part {
            AiContextPart::FilePath { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

/// Whether a context item is required for this send or may be omitted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContextPreparationRole {
    Primary,
    #[default]
    Supplemental,
}

/// Privacy-safe source classification. The source value itself (URI/path/text)
/// never belongs in a receipt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContextSourceKind {
    Resource,
    File,
    Skill,
    FocusedTarget,
    Ambient,
    #[default]
    Text,
}

impl AiContextPart {
    pub fn source_kind(&self) -> ContextSourceKind {
        match self {
            Self::ResourceUri { .. } => ContextSourceKind::Resource,
            Self::FilePath { .. } => ContextSourceKind::File,
            Self::SkillFile { .. } => ContextSourceKind::Skill,
            Self::FocusedTarget { .. } => ContextSourceKind::FocusedTarget,
            Self::AmbientContext { .. } => ContextSourceKind::Ambient,
            Self::TextBlock { .. } => ContextSourceKind::Text,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ContextPreparationItem {
    pub part: AiContextPart,
    pub role: ContextPreparationRole,
}

impl ContextPreparationItem {
    pub fn primary(part: AiContextPart) -> Self {
        Self {
            part,
            role: ContextPreparationRole::Primary,
        }
    }

    pub fn supplemental(part: AiContextPart) -> Self {
        Self {
            part,
            role: ContextPreparationRole::Supplemental,
        }
    }
}

impl std::fmt::Debug for ContextPreparationItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextPreparationItem")
            .field("source_kind", &self.part.source_kind())
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// A typed resolution failure. Raw source identities and error strings live
/// only in the diagnostic vault carried by `AppFailureRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextResolutionFailure {
    pub part_id: String,
    pub source_kind: ContextSourceKind,
    pub role: ContextPreparationRole,
    pub failure: crate::ai::reliability::AppFailureRecord,
}

/// Content-bearing resolution state. Deliberately non-serializable and
/// redacted in Debug; only model-bound code may inspect `prompt_prefix`.
#[derive(Clone, PartialEq, Eq)]
pub struct ContextResolutionReceipt {
    pub attempted: usize,
    pub resolved: usize,
    pub failures: Vec<ContextResolutionFailure>,
    pub prompt_prefix: String,
}

impl std::fmt::Debug for ContextResolutionReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextResolutionReceipt")
            .field("attempted", &self.attempted)
            .field("resolved", &self.resolved)
            .field("failure_count", &self.failures.len())
            .field("prompt_chars", &self.prompt_prefix.chars().count())
            .finish()
    }
}

impl ContextResolutionReceipt {
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }
}

/// Hard ceiling on text embedded from one context item.
const MAX_RESOURCE_PROMPT_CHARS: usize = 100_000;
const CONTEXT_TRUNCATION_NOTE: &str =
    "\n[truncated: context attachment exceeded the prompt size ceiling]";

/// Canonical sanitizer for every content-bearing `AiContextPart` variant.
/// JSON is detected by parsing rather than trusting a MIME hint, so nested
/// `base64Data` values cannot bypass stripping through a wrong or absent MIME.
fn sanitize_resource_text_for_prompt(text: &str, _mime_type: &str, _source: &str) -> String {
    let mut sanitized = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(mut value) => {
            let stripped = strip_base64_data_fields(&mut value);
            if stripped > 0 {
                tracing::warn!(
                    target: "script_kit::message_parts",
                    event = "context_base64_stripped_from_prompt_text",
                    stripped_fields = stripped,
                    original_chars = text.chars().count(),
                    "stripped base64 payloads from context text"
                );
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string())
            } else {
                text.to_string()
            }
        }
        Err(_) => text.to_string(),
    };

    if sanitized.chars().count() > MAX_RESOURCE_PROMPT_CHARS {
        let original_chars = sanitized.chars().count();
        sanitized = sanitized.chars().take(MAX_RESOURCE_PROMPT_CHARS).collect();
        sanitized.push_str(CONTEXT_TRUNCATION_NOTE);
        tracing::warn!(
            target: "script_kit::message_parts",
            event = "context_prompt_text_truncated",
            original_chars,
            max_chars = MAX_RESOURCE_PROMPT_CHARS,
        );
    }
    sanitized
}

fn strip_base64_data_fields(value: &mut serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            let mut stripped = 0;
            for (key, entry) in map.iter_mut() {
                if key == "base64Data" {
                    if let Some(data) = entry.as_str() {
                        let chars = data.chars().count();
                        *entry = serde_json::Value::String(format!(
                            "[binary omitted: {chars} base64 chars]"
                        ));
                        stripped += 1;
                        continue;
                    }
                }
                stripped += strip_base64_data_fields(entry);
            }
            stripped
        }
        serde_json::Value::Array(items) => items.iter_mut().map(strip_base64_data_fields).sum(),
        _ => 0,
    }
}

fn escape_xml_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(crate) fn run_scoped_fingerprint(value: &str) -> String {
    crate::logging::log_private_user_value(value).sha256
}

#[derive(Clone)]
struct PreparedContextBlock {
    content: String,
    kind: ContextPartPreparationOutcomeKind,
}

fn context_unavailable(detail: &str) -> crate::ai::reliability::AppFailureRecord {
    crate::ai::reliability::context_unavailable_failure(detail)
}

/// The one content resolver for all six `AiContextPart` variants.
fn resolve_context_part_sanitized(
    item: &ContextPreparationItem,
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> std::result::Result<PreparedContextBlock, Box<crate::ai::reliability::AppFailureRecord>> {
    let full = |content: String| PreparedContextBlock {
        content,
        kind: ContextPartPreparationOutcomeKind::FullContent,
    };
    match &item.part {
        AiContextPart::ResourceUri { uri, .. } => {
            let content = crate::mcp_resources::read_resource(uri, scripts, scriptlets, None)
                .map_err(|error| {
                    Box::new(context_unavailable(&format!(
                        "resource read failed: {error}"
                    )))
                })?;
            let text =
                sanitize_resource_text_for_prompt(&content.text, &content.mime_type, &content.uri);
            Ok(full(format!(
                "<context source=\"{}\" mimeType=\"{}\">\n{}\n</context>",
                escape_xml_attribute(&content.uri),
                escape_xml_attribute(&content.mime_type),
                text
            )))
        }
        AiContextPart::FilePath { path, .. } => match std::fs::read_to_string(path) {
            Ok(text) => Ok(full(format!(
                "<attachment path=\"{}\">\n{}\n</attachment>",
                escape_xml_attribute(path),
                sanitize_resource_text_for_prompt(&text, "text/plain", path)
            ))),
            Err(read_error) => match std::fs::metadata(path) {
                Ok(metadata) => Ok(PreparedContextBlock {
                    content: format!(
                        "<attachment path=\"{}\" unreadable=\"true\" bytes=\"{}\" />",
                        escape_xml_attribute(path),
                        metadata.len()
                    ),
                    kind: ContextPartPreparationOutcomeKind::MetadataOnly,
                }),
                Err(stat_error) => Err(Box::new(context_unavailable(&format!(
                    "attachment read failed: {read_error}; metadata failed: {stat_error}"
                )))),
            },
        },
        AiContextPart::SkillFile {
            path,
            skill_name,
            owner_label,
            ..
        } => {
            let raw = std::fs::read_to_string(path).map_err(|error| {
                Box::new(context_unavailable(&format!("skill read failed: {error}")))
            })?;
            let content = sanitize_resource_text_for_prompt(&raw, "text/markdown", path);
            let owner = escape_xml_attribute(owner_label);
            let title = escape_xml_attribute(skill_name);
            if owner_label == "Flow" {
                Ok(full(format!(
                    "Follow the attached flow \"{title}\" from the mdflow roster for this session.\n\n<flow path=\"{}\">\n{}\n</flow>",
                    escape_xml_attribute(path),
                    content
                )))
            } else {
                let owner_phrase = if owner_label == "Claude Code" {
                    format!("from {owner}")
                } else {
                    format!("from plugin \"{owner}\"")
                };
                Ok(full(format!(
                    "Use the attached skill \"{title}\" {owner_phrase} for this session.\n\n<skill path=\"{}\">\n{}\n</skill>",
                    escape_xml_attribute(path),
                    content
                )))
            }
        }
        AiContextPart::FocusedTarget { target, label } => {
            let metadata = target
                .metadata
                .as_ref()
                .map(serde_json::to_string_pretty)
                .transpose()
                .map_err(|error| {
                    Box::new(context_unavailable(&format!(
                        "focused metadata serialization failed: {error}"
                    )))
                })?
                .unwrap_or_else(|| "{}".to_string());
            let metadata = sanitize_resource_text_for_prompt(
                &metadata,
                "application/json",
                &target.semantic_id,
            );
            Ok(full(format!(
                "<context source=\"focusedTarget\" itemSource=\"{}\" itemKind=\"{}\" semanticId=\"{}\">\nLabel: {}\nMetadata:\n{}\n</context>",
                escape_xml_attribute(&target.source),
                escape_xml_attribute(&target.kind),
                escape_xml_attribute(&target.semantic_id),
                escape_xml_attribute(label),
                metadata,
            )))
        }
        AiContextPart::AmbientContext { .. } => Ok(PreparedContextBlock {
            content: String::new(),
            kind: ContextPartPreparationOutcomeKind::DisplayOnly,
        }),
        AiContextPart::TextBlock {
            label,
            source,
            text,
            mime_type,
        } => {
            let mime = mime_type.as_deref().unwrap_or("text/plain");
            Ok(full(format!(
                "<context source=\"{}\" mimeType=\"{}\" label=\"{}\">\n{}\n</context>",
                escape_xml_attribute(source),
                escape_xml_attribute(mime),
                escape_xml_attribute(label),
                sanitize_resource_text_for_prompt(text, mime, source)
            )))
        }
    }
}

#[expect(
    clippy::result_large_err,
    reason = "Agent Chat's cross-surface context boundary preserves the complete unboxed AppFailureRecord for typed recovery"
)]
pub(crate) fn resolve_context_item_to_prompt_block(
    part: &AiContextPart,
    role: ContextPreparationRole,
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> std::result::Result<String, crate::ai::reliability::AppFailureRecord> {
    let item = ContextPreparationItem {
        part: part.clone(),
        role,
    };
    resolve_context_part_sanitized(&item, scripts, scriptlets)
        .map(|block| block.content)
        .map_err(|failure| *failure)
}

/// Compatibility resolver. All content still passes through the canonical
/// sanitizer; raw diagnostics are intentionally reduced to safe copy here.
pub fn resolve_context_part_to_prompt_block(
    part: &AiContextPart,
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> Result<String> {
    resolve_context_item_to_prompt_block(part, ContextPreparationRole::Primary, scripts, scriptlets)
        .map_err(|failure| anyhow::anyhow!(failure.primary_message()))
}

pub fn resolve_context_parts_with_receipt(
    parts: &[AiContextPart],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> ContextResolutionReceipt {
    let items = parts
        .iter()
        .cloned()
        .map(ContextPreparationItem::primary)
        .collect::<Vec<_>>();
    resolve_context_items_with_receipt(&items, scripts, scriptlets)
}

fn resolve_context_items_with_receipt(
    items: &[ContextPreparationItem],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> ContextResolutionReceipt {
    let mut blocks = Vec::new();
    let mut failures = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match resolve_context_part_sanitized(item, scripts, scriptlets) {
            Ok(block) if !block.content.trim().is_empty() => blocks.push(block.content),
            Ok(_) => {}
            Err(failure) => failures.push(ContextResolutionFailure {
                part_id: format!("context-{index:04}"),
                source_kind: item.part.source_kind(),
                role: item.role,
                failure: *failure,
            }),
        }
    }
    ContextResolutionReceipt {
        attempted: items.len(),
        resolved: blocks.len(),
        failures,
        prompt_prefix: blocks.join("\n\n"),
    }
}

pub fn resolve_context_parts_to_prompt_prefix(
    parts: &[AiContextPart],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> Result<String> {
    let receipt = resolve_context_parts_with_receipt(parts, scripts, scriptlets);
    if let Some(failure) = receipt.failures.first() {
        anyhow::bail!(failure.failure.primary_message());
    }
    Ok(receipt.prompt_prefix)
}

/// Provenance tag for a context part in the assembly pipeline.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ContextAssemblyOrigin {
    /// Part came from parsed `@context` / `@file` directives in the message text.
    Mention,
    /// Part came from the pending context chips (UI or SDK).
    Pending,
}

/// A duplicate that was dropped during context assembly, with provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssemblyDuplicate {
    pub kept_from: ContextAssemblyOrigin,
    pub dropped_from: ContextAssemblyOrigin,
    pub label: String,
    pub source: String,
}

/// Deterministic receipt from merging mention-derived and pending context parts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssemblyReceipt {
    pub mention_count: usize,
    pub pending_count: usize,
    pub merged_count: usize,
    pub duplicates_removed: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicates: Vec<ContextAssemblyDuplicate>,
    pub merged_parts: Vec<AiContextPart>,
}

/// Merge mention-derived and pending context parts with full provenance tracking.
///
/// Returns a [`ContextAssemblyReceipt`] recording which parts survived, which
/// duplicates were dropped, and where each came from. Mentions are processed
/// first so they take priority in first-seen deduplication.
pub(crate) fn merge_context_parts_with_receipt(
    mentions: &[AiContextPart],
    pending: &[AiContextPart],
) -> ContextAssemblyReceipt {
    let mut merged = Vec::with_capacity(mentions.len() + pending.len());
    let mut origins = Vec::with_capacity(mentions.len() + pending.len());
    let mut duplicates = Vec::new();

    for (origin, part) in mentions
        .iter()
        .map(|part| (ContextAssemblyOrigin::Mention, part))
        .chain(
            pending
                .iter()
                .map(|part| (ContextAssemblyOrigin::Pending, part)),
        )
    {
        if let Some(existing_idx) = merged.iter().position(|existing| existing == part) {
            duplicates.push(ContextAssemblyDuplicate {
                kept_from: origins[existing_idx],
                dropped_from: origin,
                label: part.label().to_string(),
                source: part.source().to_string(),
            });
            continue;
        }

        merged.push(part.clone());
        origins.push(origin);
    }

    let receipt = ContextAssemblyReceipt {
        mention_count: mentions.len(),
        pending_count: pending.len(),
        merged_count: merged.len(),
        duplicates_removed: duplicates.len(),
        duplicates,
        merged_parts: merged,
    };

    tracing::info!(
        target: "ai",
        checkpoint = "context_assembly",
        mention_count = receipt.mention_count,
        pending_count = receipt.pending_count,
        merged_count = receipt.merged_count,
        duplicates_removed = receipt.duplicates_removed,
        "context parts assembled"
    );

    receipt
}

/// Merge two slices of context parts into a single list with first-seen order
/// preserved and duplicates removed by value equality.
///
/// This is a backward-compatible wrapper around [`merge_context_parts_with_receipt`].
/// It treats `left` as mentions and `right` as pending parts, returning only the
/// merged list without provenance metadata.
pub fn merge_context_parts(left: &[AiContextPart], right: &[AiContextPart]) -> Vec<AiContextPart> {
    merge_context_parts_with_receipt(left, right).merged_parts
}

// ---------------------------------------------------------------------------
// Schema-versioned message-preparation receipt
// ---------------------------------------------------------------------------

pub const AI_MESSAGE_PREPARATION_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreparedMessageDecision {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ContextPartPreparationOutcomeKind {
    FullContent,
    MetadataOnly,
    #[default]
    Failed,
    DisplayOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextPartPreparationOutcome {
    #[serde(default)]
    pub part_id: String,
    #[serde(default)]
    pub source_kind: ContextSourceKind,
    #[serde(default)]
    pub role: ContextPreparationRole,
    pub kind: ContextPartPreparationOutcomeKind,
    #[serde(default)]
    pub content_chars: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<sk_protocol::ai_reliability::AiFailureCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextPreparationSummary {
    #[serde(default)]
    pub attempted: usize,
    #[serde(default)]
    pub resolved: usize,
    #[serde(default)]
    pub failed: usize,
    #[serde(default)]
    pub primary_failed: usize,
    #[serde(default)]
    pub supplemental_failed: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssemblySummary {
    #[serde(default)]
    pub mention_count: usize,
    #[serde(default)]
    pub pending_count: usize,
    #[serde(default)]
    pub merged_count: usize,
    #[serde(default)]
    pub duplicates_removed: usize,
}

/// Serializable preparation receipt. This type intentionally contains no raw
/// content, prompt prefix, path, URI, provider/OS error, or unresolved part.
/// Unknown v1 content fields are discarded by serde during compatibility load.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMessageReceipt {
    pub schema_version: u32,
    pub decision: PreparedMessageDecision,
    #[serde(default)]
    pub authored_content_chars: usize,
    #[serde(default)]
    pub final_content_chars: usize,
    #[serde(default)]
    pub context: ContextPreparationSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<ContextAssemblySummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<ContextPartPreparationOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_error: Option<String>,
}

impl PreparedMessageReceipt {
    pub fn can_send_message(&self) -> bool {
        self.decision != PreparedMessageDecision::Blocked
    }
}

/// Private model-bound payload paired with its privacy-safe public receipt.
/// It is deliberately non-serializable and its Debug implementation is fully
/// redacted.
#[derive(Clone)]
pub struct PreparedUserMessage {
    pub final_user_content: String,
    pub receipt: PreparedMessageReceipt,
    unresolved_parts: Vec<AiContextPart>,
}

impl std::fmt::Debug for PreparedUserMessage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedUserMessage")
            .field("receipt", &self.receipt)
            .field("unresolved_count", &self.unresolved_parts.len())
            .finish()
    }
}

impl std::ops::Deref for PreparedUserMessage {
    type Target = PreparedMessageReceipt;

    fn deref(&self) -> &Self::Target {
        &self.receipt
    }
}

impl PreparedUserMessage {
    pub fn unresolved_parts(&self) -> &[AiContextPart] {
        &self.unresolved_parts
    }
}

fn join_prompt_prefix_and_raw_content(prompt_prefix: &str, raw_content: &str) -> String {
    if !prompt_prefix.is_empty() && !raw_content.trim().is_empty() {
        format!("{prompt_prefix}\n\n{raw_content}")
    } else if !prompt_prefix.is_empty() {
        prompt_prefix.to_string()
    } else {
        raw_content.to_string()
    }
}

fn build_user_visible_context_error(failures: &[ContextResolutionFailure]) -> Option<String> {
    if failures.is_empty() {
        return None;
    }
    if failures
        .iter()
        .any(|failure| failure.role == ContextPreparationRole::Primary)
    {
        Some("This context could not be prepared. Retry or remove it before sending.".to_string())
    } else {
        Some("One attachment could not be added. The remaining message is ready.".to_string())
    }
}

fn safe_outcome(
    index: usize,
    item: &ContextPreparationItem,
    result: &std::result::Result<
        PreparedContextBlock,
        Box<crate::ai::reliability::AppFailureRecord>,
    >,
) -> ContextPartPreparationOutcome {
    let part_id = format!("context-{index:04}");
    match result {
        Ok(block) => ContextPartPreparationOutcome {
            part_id,
            source_kind: item.part.source_kind(),
            role: item.role,
            kind: block.kind.clone(),
            content_chars: block.content.chars().count(),
            content_fingerprint: (!block.content.is_empty())
                .then(|| run_scoped_fingerprint(&block.content)),
            failure_code: None,
            diagnostic_fingerprint: None,
        },
        Err(record) => ContextPartPreparationOutcome {
            part_id,
            source_kind: item.part.source_kind(),
            role: item.role,
            kind: ContextPartPreparationOutcomeKind::Failed,
            content_chars: 0,
            content_fingerprint: None,
            failure_code: Some(record.failure.code),
            diagnostic_fingerprint: record
                .failure
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.fingerprint.0.clone()),
        },
    }
}

/// Canonical preparation entry with explicit primary/supplemental roles.
pub fn prepare_user_message(
    raw_content: &str,
    items: &[ContextPreparationItem],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> PreparedUserMessage {
    let mut outcomes = Vec::with_capacity(items.len());
    let mut failures = Vec::new();
    let mut unresolved_parts = Vec::new();
    let mut prompt_blocks = Vec::new();

    for (index, item) in items.iter().enumerate() {
        let result = resolve_context_part_sanitized(item, scripts, scriptlets);
        outcomes.push(safe_outcome(index, item, &result));
        match result {
            Ok(block) if !block.content.trim().is_empty() => prompt_blocks.push(block.content),
            Ok(_) => {}
            Err(failure) => {
                unresolved_parts.push(item.part.clone());
                failures.push(ContextResolutionFailure {
                    part_id: format!("context-{index:04}"),
                    source_kind: item.part.source_kind(),
                    role: item.role,
                    failure: *failure,
                });
            }
        }
    }

    let prompt_prefix = prompt_blocks.join("\n\n");
    let final_user_content = join_prompt_prefix_and_raw_content(&prompt_prefix, raw_content);
    let primary_failed = failures
        .iter()
        .filter(|failure| failure.role == ContextPreparationRole::Primary)
        .count();
    let supplemental_failed = failures.len() - primary_failed;
    let decision = if primary_failed > 0
        || (supplemental_failed > 0 && prompt_blocks.is_empty() && raw_content.trim().is_empty())
    {
        PreparedMessageDecision::Blocked
    } else if supplemental_failed > 0 {
        PreparedMessageDecision::Partial
    } else {
        PreparedMessageDecision::Ready
    };
    let user_error = build_user_visible_context_error(&failures);
    let context = ContextPreparationSummary {
        attempted: items.len(),
        resolved: prompt_blocks.len(),
        failed: failures.len(),
        primary_failed,
        supplemental_failed,
    };

    tracing::info!(
        checkpoint = "message_prepare",
        decision = ?decision,
        attempted = context.attempted,
        resolved = context.resolved,
        failures = context.failed,
        final_user_content_len = final_user_content.len(),
        "ai message preparation complete"
    );

    PreparedUserMessage {
        receipt: PreparedMessageReceipt {
            schema_version: AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
            decision,
            authored_content_chars: raw_content.chars().count(),
            final_content_chars: final_user_content.chars().count(),
            context,
            assembly: None,
            outcomes,
            user_error,
        },
        final_user_content,
        unresolved_parts,
    }
}

/// Compatibility entry. Authored text is the primary payload, so attached
/// parts are supplemental; when no authored text exists, each supplied part is
/// primary. New callers should use [`prepare_user_message`] with explicit roles.
pub fn prepare_user_message_with_receipt(
    raw_content: &str,
    parts: &[AiContextPart],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> PreparedUserMessage {
    let role = if raw_content.trim().is_empty() {
        ContextPreparationRole::Primary
    } else {
        ContextPreparationRole::Supplemental
    };
    let items = parts
        .iter()
        .cloned()
        .map(|part| ContextPreparationItem { part, role })
        .collect::<Vec<_>>();
    prepare_user_message(raw_content, &items, scripts, scriptlets)
}

pub fn prepare_user_message_from_sources_with_receipt(
    raw_content: &str,
    mention_parts: &[AiContextPart],
    pending_parts: &[AiContextPart],
    scripts: &[Arc<crate::scripts::Script>],
    scriptlets: &[Arc<crate::scripts::Scriptlet>],
) -> PreparedUserMessage {
    let assembly = merge_context_parts_with_receipt(mention_parts, pending_parts);
    let authored_message_exists = !raw_content.trim().is_empty();
    let items = assembly
        .merged_parts
        .iter()
        .cloned()
        .map(|part| ContextPreparationItem {
            part,
            role: if authored_message_exists {
                ContextPreparationRole::Supplemental
            } else {
                ContextPreparationRole::Primary
            },
        })
        .collect::<Vec<_>>();
    let mut prepared = prepare_user_message(raw_content, &items, scripts, scriptlets);
    prepared.receipt.assembly = Some(ContextAssemblySummary {
        mention_count: assembly.mention_count,
        pending_count: assembly.pending_count,
        merged_count: assembly.merged_count,
        duplicates_removed: assembly.duplicates_removed,
    });
    prepared
}

#[cfg(test)]
include!("message_parts_tests.rs");
