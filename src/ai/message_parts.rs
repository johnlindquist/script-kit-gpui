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
mod tests {
    use super::*;

    #[test]
    fn private_context_fingerprint_uses_ephemeral_key_not_predictable_public_hash() {
        use sha2::Digest as _;

        let secret = "private attached document and hidden model context";
        let actual = run_scoped_fingerprint(secret);

        assert_eq!(
            actual,
            crate::logging::log_private_user_value(secret).sha256
        );
        assert_eq!(actual, run_scoped_fingerprint(secret));
        assert_ne!(
            actual,
            run_scoped_fingerprint("a different private context")
        );
        assert_ne!(
            actual,
            format!("{:x}", sha2::Sha256::digest(secret.as_bytes()))
        );
    }

    #[test]
    fn private_context_receipt_fingerprints_actual_prepared_prompt_with_shared_key() {
        use sha2::Digest as _;

        let secret = "sensitive attached document content";
        let items = [ContextPreparationItem::primary(AiContextPart::TextBlock {
            label: "Private notes".to_string(),
            source: "synthetic://private".to_string(),
            text: secret.to_string(),
            mime_type: Some("text/plain".to_string()),
        })];
        let prepared = prepare_user_message("", &items, &[], &[]);
        let actual = prepared.receipt.outcomes[0]
            .content_fingerprint
            .as_deref()
            .expect("successful real context preparation has a private fingerprint");

        assert_eq!(
            actual,
            crate::logging::log_private_user_value(&prepared.final_user_content).sha256
        );
        assert_ne!(
            actual,
            format!(
                "{:x}",
                sha2::Sha256::digest(prepared.final_user_content.as_bytes())
            )
        );
        assert!(!serde_json::to_string(&prepared.receipt)
            .unwrap()
            .contains(secret));
    }

    #[test]
    fn semantic_chip_projection_is_stable_redacted_and_capability_neutral() {
        let a = AiContextPart::TextBlock {
            label: "Terminal output".to_string(),
            source: "terminal://session/42".to_string(),
            text: "secret first body".to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        let b = AiContextPart::TextBlock {
            label: "Terminal output".to_string(),
            source: "terminal://session/42".to_string(),
            text: "different secret body".to_string(),
            mime_type: Some("application/json".to_string()),
        };
        let removable = a.semantic_chip_projection(true);
        let retained = b.semantic_chip_projection(false);

        assert_eq!(removable.semantic_id, retained.semantic_id);
        assert_eq!(removable.label, "Terminal output");
        assert!(removable.removable);
        assert!(!retained.removable);
        assert!(!removable.semantic_id.contains("secret"));
        assert!(!removable.semantic_id.contains("Terminal output"));
        assert!(removable
            .semantic_id
            .starts_with("agent-chat-context-text-"));
    }

    #[test]
    fn semantic_chip_projection_changes_with_part_identity_not_content() {
        let first = AiContextPart::FilePath {
            path: "/tmp/one.txt".to_string(),
            label: "Notes".to_string(),
        };
        let same = first.clone();
        let other = AiContextPart::FilePath {
            path: "/tmp/two.txt".to_string(),
            label: "Notes".to_string(),
        };
        assert_eq!(
            first.semantic_chip_projection(true).semantic_id,
            same.semantic_chip_projection(true).semantic_id
        );
        assert_ne!(
            first.semantic_chip_projection(true).semantic_id,
            other.semantic_chip_projection(true).semantic_id
        );
    }

    #[test]
    fn test_serde_roundtrip_resource_uri() {
        let part = AiContextPart::ResourceUri {
            uri: "kit://context?profile=minimal".to_string(),
            label: "Current Context".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"resourceUri\""));
    }

    #[test]
    fn test_serde_roundtrip_file_path() {
        let part = AiContextPart::FilePath {
            path: "/tmp/test.rs".to_string(),
            label: "test.rs".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"filePath\""));
    }

    #[test]
    fn test_label_accessor() {
        let uri_part = AiContextPart::ResourceUri {
            uri: "kit://context".to_string(),
            label: "Context".to_string(),
        };
        assert_eq!(uri_part.label(), "Context");

        let file_part = AiContextPart::FilePath {
            path: "/tmp/foo.rs".to_string(),
            label: "foo.rs".to_string(),
        };
        assert_eq!(file_part.label(), "foo.rs");
    }

    #[test]
    fn test_resolve_readable_file_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "Hello, world!").expect("write temp file");

        let part = AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "hello.txt".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("<attachment path=\""));
        assert!(block.contains("Hello, world!"));
        assert!(block.contains("</attachment>"));
        assert!(!block.contains("unreadable"));
    }

    #[test]
    fn test_resolve_skill_file_path_builds_staged_skill_prompt() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("SKILL.md");
        std::fs::write(&file_path, "# Review\nReview the current diff.").expect("write temp file");

        let part = AiContextPart::SkillFile {
            path: file_path.to_string_lossy().to_string(),
            label: "/review".to_string(),
            skill_name: "Review".to_string(),
            owner_label: "Script Kit".to_string(),
            slash_name: "review".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("Use the attached skill \"Review\""));
        assert!(block.contains("from plugin \"Script Kit\""));
        assert!(block.contains("<skill path=\""));
        assert!(block.contains("Review the current diff."));
        assert!(block.contains("</skill>"));
    }

    /// Regression: a `kit://context` resource once carried a 758KB base64
    /// screenshot into the prompt text and overflowed the model's context.
    #[test]
    fn test_sanitize_resource_text_strips_base64_payloads_from_json() {
        let big = "A".repeat(200_000);
        let json = format!(
            "{{\"focusedWindowImage\":{{\"mimeType\":\"image/png\",\"base64Data\":\"{big}\"}},\"selectedText\":\"hello\"}}"
        );
        let sanitized =
            sanitize_resource_text_for_prompt(&json, "application/json", "kit://context?test");
        assert!(!sanitized.contains(&big), "base64 payload must be stripped");
        assert!(sanitized.contains("[binary omitted: 200000 base64 chars]"));
        assert!(sanitized.contains("hello"), "non-binary fields survive");
        assert!(sanitized.chars().count() <= MAX_RESOURCE_PROMPT_CHARS + 100);
    }

    #[test]
    fn test_sanitize_resource_text_truncates_oversized_content() {
        let huge = "x".repeat(200_000);
        let sanitized = sanitize_resource_text_for_prompt(&huge, "text/plain", "kit://big");
        assert!(sanitized.chars().count() < huge.chars().count());
        assert!(sanitized.contains("[truncated: context attachment exceeded"));

        let small = "small content";
        assert_eq!(
            sanitize_resource_text_for_prompt(small, "application/json", "kit://small"),
            small,
            "content without base64 and under the ceiling passes through unchanged"
        );
    }

    #[test]
    fn test_resolve_unreadable_file_path_does_not_panic() {
        // Create a file, make it exist but unreadable by removing read permissions
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("binary.dat");
        std::fs::write(&file_path, vec![0u8; 64]).expect("write temp file");

        // On Unix, remove read permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000))
                .expect("set permissions");
        }

        let part = AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "binary.dat".to_string(),
        };

        // On unix, this should produce an unreadable fallback (metadata-only)
        #[cfg(unix)]
        {
            let block = resolve_context_part_to_prompt_block(&part, &[], &[])
                .expect("resolve should not panic");
            assert!(block.contains("unreadable=\"true\""));
            assert!(block.contains("bytes=\"64\""));
        }

        // On non-unix, file is readable, so just verify no panic
        #[cfg(not(unix))]
        {
            let _ = resolve_context_part_to_prompt_block(&part, &[], &[]);
        }

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644));
        }
    }

    #[test]
    fn test_resolve_nonexistent_file_returns_error() {
        let part = AiContextPart::FilePath {
            path: "/nonexistent/path/that/does/not/exist.txt".to_string(),
            label: "ghost.txt".to_string(),
        };

        let result = resolve_context_part_to_prompt_block(&part, &[], &[]);
        assert!(result.is_err(), "nonexistent file should error");
    }

    #[test]
    fn test_resolve_multiple_parts() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file1 = dir.path().join("a.txt");
        let file2 = dir.path().join("b.txt");
        std::fs::write(&file1, "content A").expect("write");
        std::fs::write(&file2, "content B").expect("write");

        let parts = vec![
            AiContextPart::FilePath {
                path: file1.to_string_lossy().to_string(),
                label: "a.txt".to_string(),
            },
            AiContextPart::FilePath {
                path: file2.to_string_lossy().to_string(),
                label: "b.txt".to_string(),
            },
        ];

        let prefix =
            resolve_context_parts_to_prompt_prefix(&parts, &[], &[]).expect("resolve prefix");
        assert!(prefix.contains("content A"));
        assert!(prefix.contains("content B"));
        // Two blocks separated by double newline
        assert!(prefix.contains("</attachment>\n\n<attachment"));
    }

    #[test]
    fn test_resolve_empty_parts_returns_empty_string() {
        let prefix = resolve_context_parts_to_prompt_prefix(&[], &[], &[]).expect("resolve empty");
        assert!(prefix.is_empty());
    }

    // --- PreparedMessageReceipt tests ---

    #[test]
    fn test_prepare_user_message_no_parts_is_ready() {
        let receipt = prepare_user_message_with_receipt("hello", &[], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(
            receipt.schema_version,
            AI_MESSAGE_PREPARATION_SCHEMA_VERSION
        );
        assert_eq!(receipt.authored_content_chars, 5);
        assert_eq!(receipt.final_user_content, "hello");
        assert!(receipt.outcomes.is_empty());
        assert!(receipt.unresolved_parts().is_empty());
        assert!(receipt.user_error.is_none());
        assert!(receipt.can_send_message());
    }

    #[test]
    fn test_prepare_user_message_blocks_when_all_parts_fail() {
        let parts = vec![AiContextPart::FilePath {
            path: "/definitely/missing/file.txt".to_string(),
            label: "missing.txt".to_string(),
        }];

        let items = parts
            .iter()
            .cloned()
            .map(ContextPreparationItem::primary)
            .collect::<Vec<_>>();
        let receipt = prepare_user_message("hello", &items, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Blocked);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 0);
        assert_eq!(receipt.unresolved_parts(), parts);
        assert!(receipt.user_error.is_some());
        assert_eq!(receipt.outcomes.len(), 1);
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::Failed
        );
        assert!(!receipt.can_send_message());
    }

    #[test]
    fn test_prepare_user_message_marks_unreadable_file_as_metadata_only() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("binary.dat");
        std::fs::write(&file_path, vec![0u8; 64]).expect("write temp file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o000))
                .expect("set permissions");
        }

        let parts = vec![AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "binary.dat".to_string(),
        }];

        let receipt = prepare_user_message_with_receipt("", &parts, &[], &[]);

        #[cfg(unix)]
        {
            assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
            assert_eq!(receipt.context.resolved, 1);
            assert!(receipt.context.failed == 0);
            assert_eq!(receipt.outcomes.len(), 1);
            assert_eq!(
                receipt.outcomes[0].kind,
                ContextPartPreparationOutcomeKind::MetadataOnly
            );
            assert!(receipt.final_user_content.contains("unreadable=\"true\""));
            assert!(receipt.can_send_message());
        }

        #[cfg(not(unix))]
        {
            assert_eq!(receipt.context.resolved, 1);
        }

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644));
        }
    }

    #[test]
    fn test_prepare_user_message_appends_prompt_prefix_before_raw_content() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("note.txt");
        std::fs::write(&file_path, "attached text").expect("write temp file");

        let parts = vec![AiContextPart::FilePath {
            path: file_path.to_string_lossy().to_string(),
            label: "note.txt".to_string(),
        }];

        let receipt = prepare_user_message_with_receipt("user text", &parts, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert!(receipt.final_user_content.contains("attached text"));
        assert!(receipt.final_user_content.ends_with("user text"));
        assert_eq!(receipt.outcomes.len(), 1);
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::FullContent
        );
    }

    #[test]
    fn test_prepare_user_message_partial_when_mixed_success_failure() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let good_file = dir.path().join("good.txt");
        std::fs::write(&good_file, "good content").expect("write temp file");

        let parts = vec![
            AiContextPart::FilePath {
                path: good_file.to_string_lossy().to_string(),
                label: "good.txt".to_string(),
            },
            AiContextPart::FilePath {
                path: "/definitely/missing/bad.txt".to_string(),
                label: "bad.txt".to_string(),
            },
        ];

        let receipt = prepare_user_message_with_receipt("query", &parts, &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Partial);
        assert_eq!(receipt.context.attempted, 2);
        assert_eq!(receipt.context.resolved, 1);
        assert_eq!(receipt.context.failed, 1);
        assert_eq!(receipt.unresolved_parts().len(), 1);
        assert!(receipt.final_user_content.contains("good content"));
        assert!(receipt.final_user_content.ends_with("query"));
        assert!(receipt.user_error.is_some());
        assert!(receipt.can_send_message());
    }

    #[test]
    fn merge_context_parts_deduplicates_and_preserves_order() {
        let selection = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0"
                    .to_string(),
            label: "Selection".to_string(),
        };
        let browser = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        };

        let merged = merge_context_parts(
            &[selection.clone(), browser.clone()],
            std::slice::from_ref(&selection),
        );

        assert_eq!(merged, vec![selection, browser]);
    }

    #[test]
    fn merge_context_parts_empty_inputs() {
        let merged = merge_context_parts(&[], &[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_context_parts_preserves_left_then_right_order() {
        let a = AiContextPart::FilePath {
            path: "/a.rs".to_string(),
            label: "a.rs".to_string(),
        };
        let b = AiContextPart::FilePath {
            path: "/b.rs".to_string(),
            label: "b.rs".to_string(),
        };
        let c = AiContextPart::FilePath {
            path: "/c.rs".to_string(),
            label: "c.rs".to_string(),
        };

        let merged = merge_context_parts(&[a.clone(), b.clone()], &[c.clone(), a.clone()]);
        assert_eq!(merged, vec![a, b, c]);
    }

    #[test]
    fn test_prepare_user_message_receipt_serde_roundtrip() {
        let receipt = PreparedMessageReceipt {
            schema_version: AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
            decision: PreparedMessageDecision::Ready,
            authored_content_chars: 5,
            final_content_chars: 13,
            context: ContextPreparationSummary {
                attempted: 1,
                resolved: 1,
                failed: 0,
                primary_failed: 0,
                supplemental_failed: 0,
            },
            assembly: None,
            outcomes: vec![ContextPartPreparationOutcome {
                part_id: "context-0000".to_string(),
                source_kind: ContextSourceKind::File,
                role: ContextPreparationRole::Supplemental,
                kind: ContextPartPreparationOutcomeKind::FullContent,
                content_chars: 6,
                content_fingerprint: Some("run-scoped-fingerprint".to_string()),
                failure_code: None,
                diagnostic_fingerprint: None,
            }],
            user_error: None,
        };

        let json = serde_json::to_string(&receipt).expect("serialize");
        let deserialized: PreparedMessageReceipt =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(receipt, deserialized);

        // Verify camelCase serde
        assert!(json.contains("\"schemaVersion\""));
        assert!(json.contains("\"authoredContentChars\""));
        assert!(json.contains("\"fullContent\""));
        assert!(!json.contains("rawContent"));
        assert!(!json.contains("finalUserContent"));
        assert!(!json.contains("promptPrefix"));
        assert!(!json.contains("/tmp/note.txt"));
    }

    #[test]
    fn merge_context_parts_with_receipt_reports_duplicate_provenance() {
        let selection = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0"
                    .to_string(),
            label: "Selection".to_string(),
        };
        let browser = AiContextPart::ResourceUri {
            uri:
                "kit://context?selectedText=0&frontmostApp=0&menuBar=0&browserUrl=1&focusedWindow=0"
                    .to_string(),
            label: "Browser URL".to_string(),
        };

        let receipt = merge_context_parts_with_receipt(
            &[selection.clone(), browser.clone()],
            std::slice::from_ref(&selection),
        );

        assert_eq!(receipt.merged_parts, vec![selection.clone(), browser]);
        assert_eq!(receipt.duplicates_removed, 1);
        assert_eq!(receipt.duplicates.len(), 1);
        assert_eq!(
            receipt.duplicates[0].kept_from,
            ContextAssemblyOrigin::Mention
        );
        assert_eq!(
            receipt.duplicates[0].dropped_from,
            ContextAssemblyOrigin::Pending
        );
        assert_eq!(receipt.duplicates[0].label, "Selection");
    }

    #[test]
    fn prepare_user_message_from_sources_with_receipt_attaches_assembly_receipt() {
        crate::context_snapshot::enable_deterministic_context_capture();
        let prepared = prepare_user_message_from_sources_with_receipt(
            "ship it",
            &[AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Current Context".to_string(),
            }],
            &[AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Current Context".to_string(),
            }],
            &[],
            &[],
        );

        assert!(prepared.can_send_message());
        let assembly = prepared
            .receipt
            .assembly
            .as_ref()
            .expect("assembly receipt must be present");
        assert_eq!(assembly.mention_count, 1);
        assert_eq!(assembly.pending_count, 1);
        assert_eq!(assembly.merged_count, 1);
        assert_eq!(assembly.duplicates_removed, 1);
    }

    #[test]
    fn current_context_selector_part_is_not_treated_as_ambient_bootstrap() {
        let part = AiContextPart::ResourceUri {
            uri: ASK_ANYTHING_RESOURCE_URI.to_string(),
            label: "Current Context".to_string(),
        };

        assert!(
            !part.is_ambient_bootstrap_resource(),
            "@context should resolve directly on submit instead of waiting on deferred capture"
        );
    }

    #[test]
    fn ask_anything_and_explicit_capture_labels_still_use_ambient_bootstrap() {
        for label in [
            ASK_ANYTHING_LABEL,
            "Full Screen",
            "Focused Window",
            "Selected Text",
            "Browser Tab",
        ] {
            let part = AiContextPart::ResourceUri {
                uri: ASK_ANYTHING_RESOURCE_URI.to_string(),
                label: label.to_string(),
            };
            assert!(
                part.is_ambient_bootstrap_resource(),
                "{label} should keep using deferred ambient capture"
            );
        }
    }

    #[test]
    fn test_serde_roundtrip_focused_target() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:main.rs".to_string(),
                label: "main.rs".to_string(),
                metadata: Some(serde_json::json!({ "path": "/tmp/main.rs" })),
            },
            label: "File: main.rs".to_string(),
        };
        let json = serde_json::to_string(&part).expect("serialize");
        let deserialized: AiContextPart = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(part, deserialized);
        assert!(json.contains("\"kind\":\"focusedTarget\""));
        assert!(json.contains("\"semanticId\""));
    }

    #[test]
    fn test_focused_target_label_and_source() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:2:my-script".to_string(),
                label: "My Script".to_string(),
                metadata: None,
            },
            label: "Command: My Script".to_string(),
        };
        assert_eq!(part.label(), "Command: My Script");
        assert_eq!(part.source(), "choice:2:my-script");
    }

    #[test]
    fn test_resolve_focused_target_produces_context_block() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:agent_handoff.rs".to_string(),
                label: "agent_handoff.rs".to_string(),
                metadata: Some(serde_json::json!({ "path": "/tmp/agent_handoff.rs" })),
            },
            label: "File: agent_handoff.rs".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("source=\"focusedTarget\""));
        assert!(block.contains("itemSource=\"FileSearch\""));
        assert!(block.contains("itemKind=\"file\""));
        assert!(block.contains("semanticId=\"choice:0:agent_handoff.rs\""));
        assert!(block.contains("Label: File: agent_handoff.rs"));
        assert!(block.contains("/tmp/agent_handoff.rs"));
    }

    #[test]
    fn test_resolve_focused_target_no_metadata() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:hello".to_string(),
                label: "hello".to_string(),
                metadata: None,
            },
            label: "Command: hello".to_string(),
        };

        let block =
            resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve should succeed");

        assert!(block.contains("source=\"focusedTarget\""));
        assert!(block.contains("{}"), "empty metadata should be '{{}}'");
    }

    #[test]
    fn test_prepare_user_message_with_focused_target() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "ClipboardHistory".to_string(),
                kind: "clipboard_entry".to_string(),
                semantic_id: "choice:0:clip".to_string(),
                label: "clip".to_string(),
                metadata: Some(serde_json::json!({ "contentType": "text/plain" })),
            },
            label: "Clipboard: clip".to_string(),
        };

        let receipt = prepare_user_message_with_receipt("explain this", &[part], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 1);
        assert!(receipt.final_user_content.contains("focusedTarget"));
        assert!(receipt.final_user_content.ends_with("explain this"));
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::FullContent
        );
    }

    #[test]
    fn test_prepare_user_message_with_ambient_context_is_display_only() {
        let part = AiContextPart::AmbientContext {
            label: ASK_ANYTHING_LABEL.to_string(),
        };

        let receipt = prepare_user_message_with_receipt("answer this", &[part], &[], &[]);

        assert_eq!(receipt.decision, PreparedMessageDecision::Ready);
        assert_eq!(receipt.context.attempted, 1);
        assert_eq!(receipt.context.resolved, 0);
        assert!(receipt.context.failed == 0);
        assert!(receipt.unresolved_parts.is_empty());
        assert_eq!(receipt.final_user_content, "answer this");
        assert_eq!(
            receipt.outcomes[0].kind,
            ContextPartPreparationOutcomeKind::DisplayOnly
        );
    }

    #[test]
    fn context_assembly_receipt_serde_roundtrip() {
        let receipt = ContextAssemblyReceipt {
            mention_count: 2,
            pending_count: 1,
            merged_count: 2,
            duplicates_removed: 1,
            duplicates: vec![ContextAssemblyDuplicate {
                kept_from: ContextAssemblyOrigin::Mention,
                dropped_from: ContextAssemblyOrigin::Pending,
                label: "Selection".to_string(),
                source: "kit://context?selectedText=1".to_string(),
            }],
            merged_parts: vec![AiContextPart::ResourceUri {
                uri: "kit://context?profile=minimal".to_string(),
                label: "Context".to_string(),
            }],
        };

        let json = serde_json::to_string(&receipt).expect("serialize");
        let deserialized: ContextAssemblyReceipt =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(receipt, deserialized);
        assert!(json.contains("\"mentionCount\""));
        assert!(json.contains("\"pendingCount\""));
        assert!(json.contains("\"keptFrom\""));
        assert!(json.contains("\"droppedFrom\""));
    }

    #[test]
    fn canonical_sanitizer_strips_nested_base64_from_wrong_mime_text_block() {
        let canary = "BASE64_CANARY".repeat(20_000);
        let part = AiContextPart::TextBlock {
            label: "Logs".to_string(),
            source: "text://synthetic".to_string(),
            text: serde_json::json!({
                "outer": [{"inner": {"base64Data": canary, "keep": "nonbinary-survives"}}]
            })
            .to_string(),
            mime_type: Some("text/plain".to_string()),
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared.final_user_content.contains("BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("nonbinary-survives"));
        assert!(prepared
            .final_user_content
            .contains("[binary omitted: 260000 base64 chars]"));
        assert!(prepared.final_user_content.chars().count() < 101_000);
    }

    #[test]
    fn canonical_sanitizer_strips_base64_when_text_block_mime_is_absent() {
        let part = AiContextPart::TextBlock {
            label: "Synthetic JSON".to_string(),
            source: "text://synthetic".to_string(),
            text: serde_json::json!({
                "base64Data": "MISSING_MIME_BASE64_CANARY",
                "keep": "still-here"
            })
            .to_string(),
            mime_type: None,
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared
            .final_user_content
            .contains("MISSING_MIME_BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("still-here"));
    }

    #[test]
    fn canonical_sanitizer_strips_base64_from_focused_metadata() {
        let part = AiContextPart::FocusedTarget {
            target: crate::ai::tab_context::TabAiTargetContext {
                source: "File<Search".to_string(),
                kind: "file&row".to_string(),
                semantic_id: "choice:\"unsafe\"".to_string(),
                label: "Focused".to_string(),
                metadata: Some(serde_json::json!({
                    "nested": {"base64Data": "FOCUSED_BASE64_CANARY", "keep": 42}
                })),
            },
            label: "Focused item".to_string(),
        };
        let prepared = prepare_user_message(
            "inspect",
            &[ContextPreparationItem::supplemental(part)],
            &[],
            &[],
        );
        assert!(!prepared
            .final_user_content
            .contains("FOCUSED_BASE64_CANARY"));
        assert!(prepared.final_user_content.contains("\"keep\": 42"));
        assert!(prepared.final_user_content.contains("File&lt;Search"));
        assert!(prepared.final_user_content.contains("file&amp;row"));
        assert!(prepared
            .final_user_content
            .contains("choice:&quot;unsafe&quot;"));
    }

    #[test]
    fn wrapper_attributes_escape_xml_metacharacters() {
        let part = AiContextPart::TextBlock {
            label: "label<&\"'>".to_string(),
            source: "source<&\"'>".to_string(),
            text: "safe body".to_string(),
            mime_type: Some("text/<&\"'>".to_string()),
        };
        let block = resolve_context_part_to_prompt_block(&part, &[], &[]).expect("resolve");
        assert!(block.contains("source=\"source&lt;&amp;&quot;&apos;&gt;\""));
        assert!(block.contains("label=\"label&lt;&amp;&quot;&apos;&gt;\""));
        assert!(block.contains("mimeType=\"text/&lt;&amp;&quot;&apos;&gt;\""));
    }

    #[test]
    fn primary_failure_blocks_while_supplemental_failure_can_be_partial() {
        let missing_primary = ContextPreparationItem::primary(AiContextPart::FilePath {
            path: "/missing/PRIMARY_PATH_CANARY".to_string(),
            label: "Primary".to_string(),
        });
        let blocked = prepare_user_message("authored", &[missing_primary], &[], &[]);
        assert_eq!(blocked.decision, PreparedMessageDecision::Blocked);
        assert_eq!(blocked.context.primary_failed, 1);
        assert!(!blocked.can_send_message());

        let missing_supplemental = ContextPreparationItem::supplemental(AiContextPart::FilePath {
            path: "/missing/SUPPLEMENTAL_PATH_CANARY".to_string(),
            label: "Supplemental".to_string(),
        });
        let partial = prepare_user_message("authored", &[missing_supplemental], &[], &[]);
        assert_eq!(partial.decision, PreparedMessageDecision::Partial);
        assert_eq!(partial.context.supplemental_failed, 1);
        assert!(partial.can_send_message());
        assert_eq!(partial.final_user_content, "authored");
    }

    #[test]
    fn valid_primary_plus_missing_supplemental_preserves_private_payload() {
        let good = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(good.path(), "PRIMARY_CONTENT_CANARY").expect("write");
        let items = vec![
            ContextPreparationItem::primary(AiContextPart::FilePath {
                path: good.path().to_string_lossy().to_string(),
                label: "Primary".to_string(),
            }),
            ContextPreparationItem::supplemental(AiContextPart::FilePath {
                path: "/missing/RAW_PATH_CANARY".to_string(),
                label: "Supplemental".to_string(),
            }),
        ];
        let prepared = prepare_user_message("", &items, &[], &[]);
        assert_eq!(prepared.decision, PreparedMessageDecision::Partial);
        assert!(prepared
            .final_user_content
            .contains("PRIMARY_CONTENT_CANARY"));
        let serialized = serde_json::to_string(&prepared.receipt).expect("serialize receipt");
        for canary in [
            "PRIMARY_CONTENT_CANARY",
            "RAW_PATH_CANARY",
            "rawContent",
            "finalUserContent",
            "promptPrefix",
            "metadata failed",
        ] {
            assert!(!serialized.contains(canary), "receipt leaked {canary}");
        }
        assert!(!format!("{prepared:?}").contains("PRIMARY_CONTENT_CANARY"));
    }

    #[test]
    fn legacy_v1_receipt_discards_content_bearing_fields() {
        let legacy = serde_json::json!({
            "schemaVersion": 1,
            "decision": "partial",
            "rawContent": "LEGACY_RAW_CANARY",
            "finalUserContent": "LEGACY_FINAL_CANARY",
            "context": {
                "attempted": 2,
                "resolved": 1,
                "failures": [{
                    "label": "LEGACY_LABEL_CANARY",
                    "source": "kit://URI_CANARY",
                    "error": "OS_ERROR_CANARY"
                }],
                "promptPrefix": "PROMPT_PREFIX_CANARY"
            },
            "outcomes": []
        });
        let loaded: PreparedMessageReceipt =
            serde_json::from_value(legacy).expect("load legacy receipt");
        let serialized = serde_json::to_string(&loaded).expect("serialize redacted receipt");
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.context.attempted, 2);
        assert_eq!(loaded.context.resolved, 1);
        for canary in [
            "LEGACY_RAW_CANARY",
            "LEGACY_FINAL_CANARY",
            "LEGACY_LABEL_CANARY",
            "URI_CANARY",
            "OS_ERROR_CANARY",
            "PROMPT_PREFIX_CANARY",
        ] {
            assert!(!serialized.contains(canary), "legacy load leaked {canary}");
        }
    }
}
