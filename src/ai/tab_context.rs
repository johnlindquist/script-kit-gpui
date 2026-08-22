//! Tab AI context assembly types.
//!
//! Defines the schema-versioned context blob sent to the AI model when the
//! user submits an intent from the Tab AI overlay.  The blob combines a UI
//! snapshot (current view, focused element, visible elements) with a desktop
//! context snapshot (frontmost app, selected text, browser URL) and recent
//! input history.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Schema version for `TabAiContextBlob`. Bump when adding/removing/renaming fields.
pub const TAB_AI_CONTEXT_SCHEMA_VERSION: u32 = 3;

/// Snapshot of the Script Kit UI state at the moment Tab AI was invoked.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabAiUiSnapshot {
    /// The `AppView` variant name (e.g. "ScriptList", "ArgPrompt").
    pub prompt_type: String,
    /// Current text in the filter / input field, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_text: Option<String>,
    /// Semantic ID of the focused element (e.g. "input:filter").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_semantic_id: Option<String>,
    /// Semantic ID of the selected element (e.g. "choice:0:slack").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_semantic_id: Option<String>,
    /// Top visible elements (capped to keep token cost low).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_elements: Vec<crate::protocol::ElementInfo>,
}

/// Clipboard content summary for Tab AI context.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiClipboardContext {
    /// MIME-like content type (e.g. "text", "image").
    pub content_type: String,
    /// Truncated preview of the clipboard content.
    pub preview: String,
    /// OCR text extracted from clipboard image, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
}

/// Hydrated clipboard history entry for Tab AI context (v3+).
///
/// Provides richer data than `TabAiClipboardContext` — full text for text
/// entries, timestamps, image dimensions, and OCR text.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiClipboardHistoryEntry {
    /// Unique entry ID from the clipboard history store.
    pub id: String,
    /// Content type (e.g. "text", "image", "link", "file", "color").
    pub content_type: String,
    /// Unix timestamp in milliseconds when the entry was captured.
    pub timestamp: i64,
    /// Truncated preview of the content.
    pub preview: String,
    /// Full text content (up to 1000 chars) for text-like entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text: Option<String>,
    /// OCR text extracted from image entries, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    /// Image width in pixels, if this is an image entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_width: Option<u32>,
    /// Image height in pixels, if this is an image entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_height: Option<u32>,
}

/// Truncate a string to at most `limit` characters, appending `…` if truncated.
///
/// Returns an empty string when `limit` is zero.
pub fn truncate_tab_ai_text(value: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    let char_count = value.chars().count();
    if char_count <= limit {
        value.to_string()
    } else {
        let prefix: String = value.chars().take(limit.saturating_sub(1)).collect();
        format!("{prefix}…")
    }
}

/// Explicit target context resolved from the active surface.
///
/// When the user says "this", "it", or "selected", the model should use
/// `focusedTarget` as the default subject instead of guessing from the UI
/// snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiTargetContext {
    /// Surface that produced this target (e.g. "FileSearch", "ClipboardHistory").
    pub source: String,
    /// Kind of target (e.g. "file", "directory", "clipboard_entry", "app", "window").
    pub kind: String,
    /// Semantic ID matching the element collection scheme.
    pub semantic_id: String,
    /// Human-readable label for the target.
    pub label: String,
    /// Surface-specific metadata (path, bundleId, contentType, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Machine-readable audit of target resolution emitted at context assembly time.
///
/// Captures the `focusedTarget` and `visibleTargets` fields that were resolved
/// from the active surface, plus summary counts for downstream agents and
/// dashboards to verify target availability without parsing the full context blob.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiTargetAudit {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// `AppView` variant name at invocation time.
    pub prompt_type: String,
    /// Whether a focused target was resolved.
    pub has_focused_target: bool,
    /// Number of visible targets resolved.
    pub visible_target_count: usize,
    /// Source surface of the focused target, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_source: Option<String>,
    /// Kind of the focused target (e.g. "file", "app"), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_kind: Option<String>,
    /// Semantic ID of the focused target, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_semantic_id: Option<String>,
    /// Distinct target kinds among visible targets (e.g. ["file", "directory"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_kinds: Vec<String>,
}

/// Schema version for `TabAiTargetAudit`. Bump when adding/removing/renaming fields.
pub const TAB_AI_TARGET_AUDIT_SCHEMA_VERSION: u32 = 1;

impl TabAiTargetAudit {
    /// Build a target audit from the resolved target context.
    pub fn from_targets(
        prompt_type: &str,
        focused_target: &Option<TabAiTargetContext>,
        visible_targets: &[TabAiTargetContext],
    ) -> Self {
        let mut visible_kinds: Vec<String> = visible_targets
            .iter()
            .map(|t| t.kind.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        visible_kinds.sort();

        Self {
            schema_version: TAB_AI_TARGET_AUDIT_SCHEMA_VERSION,
            prompt_type: prompt_type.to_string(),
            has_focused_target: focused_target.is_some(),
            visible_target_count: visible_targets.len(),
            focused_source: focused_target.as_ref().map(|t| t.source.clone()),
            focused_kind: focused_target.as_ref().map(|t| t.kind.clone()),
            focused_semantic_id: focused_target.as_ref().map(|t| t.semantic_id.clone()),
            visible_kinds,
        }
    }

    /// Emit this audit as a structured `tracing::info` log line with a phase tag.
    pub fn emit_with_phase(&self, phase: &str) {
        let safe_focused_id = self
            .focused_semantic_id
            .as_deref()
            .map(crate::logging::log_private_user_value);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_target_audit",
            phase = phase,
            schema_version = self.schema_version,
            prompt_type = %self.prompt_type,
            has_focused_target = self.has_focused_target,
            visible_target_count = self.visible_target_count,
            focused_source = ?self.focused_source,
            focused_kind = ?self.focused_kind,
            focused_semantic_id_bytes = ?safe_focused_id.as_ref().map(|value| value.raw_bytes),
            focused_semantic_id_sha256 =
                ?safe_focused_id.as_ref().map(|value| value.sha256.as_str()),
            visible_kinds = ?self.visible_kinds,
        );
    }

    /// Emit this audit as a structured `tracing::info` log line.
    pub fn emit(&self) {
        self.emit_with_phase("unspecified");
    }
}

// ---------------------------------------------------------------------------
// Tab AI experience packs — named, surface-native intent planning
// ---------------------------------------------------------------------------

/// Semantic flavor of an experience intent — drives priority ranking without
/// relying on fragile label-string matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabAiExperienceFlavor {
    Generic,
    Teachable,
    Fusion,
    Batch,
    Adaptation,
}

/// A single experience-pack suggestion with a human label and full intent string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabAiExperienceIntent {
    pub label: String,
    pub intent: String,
    pub flavor: TabAiExperienceFlavor,
    pub spotlight_rank: u8,
}

impl TabAiExperienceIntent {
    pub fn new(label: impl Into<String>, intent: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            intent: intent.into(),
            flavor: TabAiExperienceFlavor::Generic,
            spotlight_rank: u8::MAX,
        }
    }

    pub fn with_flavor(mut self, flavor: TabAiExperienceFlavor) -> Self {
        self.flavor = flavor;
        self
    }

    pub fn with_spotlight_rank(mut self, spotlight_rank: u8) -> Self {
        self.spotlight_rank = spotlight_rank;
        self
    }

    /// Convert into a [`TabAiSuggestedIntentSpec`] for the card suggestion system.
    pub fn into_spec(self) -> TabAiSuggestedIntentSpec {
        TabAiSuggestedIntentSpec::new(self.label, self.intent)
    }
}

/// Named experience packs that map surfaces to distinct power-user moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAiExperiencePack {
    DesktopGeneral,
    ClipboardStudio,
    FileStudio,
    FolderStudio,
    CommandAlchemy,
    AppPilot,
    WindowPilot,
    ProcessPilot,
    GenericSelection,
}

/// A resolved experience spec ready for display in the empty-state card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabAiExperienceSpec {
    pub pack: TabAiExperiencePack,
    pub title: String,
    pub subtitle: String,
    pub intents: Vec<TabAiExperienceIntent>,
}

pub fn tab_ai_experience_pack_subtitle(pack: TabAiExperiencePack) -> &'static str {
    match pack {
        TabAiExperiencePack::DesktopGeneral => "Use the current desktop state as the live subject.",
        TabAiExperiencePack::ClipboardStudio => {
            "Transform copied content without opening another tool."
        }
        TabAiExperiencePack::FileStudio => "Act on the selected file in-place.",
        TabAiExperiencePack::FolderStudio => "Understand or reshape this folder quickly.",
        TabAiExperiencePack::CommandAlchemy => {
            "Teach the selected app command into something reusable."
        }
        TabAiExperiencePack::AppPilot => "Steer the current app like a custom operator console.",
        TabAiExperiencePack::WindowPilot => "Operate on this exact window, not the whole app.",
        TabAiExperiencePack::ProcessPilot => "Inspect or tame a live automation process.",
        TabAiExperiencePack::GenericSelection => "Use the selected thing as the subject.",
    }
}

/// Card-priority tier for an experience intent, derived from its flavor.
///
// Named spotlight ranks — lower values surface first in the three-card shortlist.
const SPOTLIGHT_CONTEXT_HERO: u8 = 0;
const SPOTLIGHT_PATTERN_HERO: u8 = 1;
const SPOTLIGHT_MEMORY_HERO: u8 = 2;
const SPOTLIGHT_TEACHABLE: u8 = 3;
const SPOTLIGHT_BATCH_HERO: u8 = 4;
const SPOTLIGHT_FALLBACK: u8 = 10;

/// Lower values surface first in the three-card shortlist.
/// Tier 0 = differentiated fusion/batch/adaptation (Raycast cannot do these).
/// Tier 1 = teachable/reusable command creation.
/// Tier 2 = everything else (generic pack verbs).
fn tab_ai_experience_card_priority(intent: &TabAiExperienceIntent) -> u8 {
    match intent.flavor {
        TabAiExperienceFlavor::Adaptation
        | TabAiExperienceFlavor::Fusion
        | TabAiExperienceFlavor::Batch => 0,
        TabAiExperienceFlavor::Teachable => 1,
        TabAiExperienceFlavor::Generic => 2,
    }
}

/// Re-sort intents so differentiated labels outrank generic verbs,
/// preserving stable order within the same priority bucket.
fn prioritize_tab_ai_experience_card_intents(
    intents: Vec<TabAiExperienceIntent>,
) -> Vec<TabAiExperienceIntent> {
    let mut indexed: Vec<(usize, TabAiExperienceIntent)> =
        intents.into_iter().enumerate().collect();
    indexed.sort_by(|(left_ix, left), (right_ix, right)| {
        tab_ai_experience_card_priority(left)
            .cmp(&tab_ai_experience_card_priority(right))
            .then_with(|| left.spotlight_rank.cmp(&right.spotlight_rank))
            .then_with(|| left_ix.cmp(right_ix))
    });
    indexed.into_iter().map(|(_, intent)| intent).collect()
}

/// Prioritize intents, but reserve the first three slots for a deliberate mix:
/// - one context-aware hero (Fusion / Adaptation / Batch)
/// - one teachable reusable move
/// - one generic fallback
///
/// After those are filled, continue with the normal sorted overflow.
fn prioritize_then_take_tab_ai_experience_intents(
    intents: Vec<TabAiExperienceIntent>,
    limit: usize,
) -> Vec<TabAiExperienceIntent> {
    let mut featured = Vec::new();
    let mut overflow = Vec::new();
    let mut seen_tier = [false; 3];

    for intent in prioritize_tab_ai_experience_card_intents(intents) {
        let tier = tab_ai_experience_card_priority(&intent) as usize;
        if tier < seen_tier.len() && !seen_tier[tier] && featured.len() < limit {
            seen_tier[tier] = true;
            featured.push(intent);
        } else {
            overflow.push(intent);
        }
    }

    featured.extend(
        overflow
            .into_iter()
            .take(limit.saturating_sub(featured.len())),
    );
    featured
}

/// Build a display-ready experience spec from the current context.
///
/// Returns `None` when no intents can be generated (nothing useful to show).
/// Intents are prioritized so differentiated labels (fusion, batching, adaptation)
/// outrank generic verbs, then truncated to the top 3 for a focused empty-state card.
pub fn build_tab_ai_experience_spec(
    focused_target: Option<&TabAiTargetContext>,
    visible_targets: &[TabAiTargetContext],
    clipboard: Option<&TabAiClipboardContext>,
    prior_automations: &[TabAiMemorySuggestion],
) -> Option<TabAiExperienceSpec> {
    let pack = TabAiExperiencePack::from_target(focused_target);
    let intents = prioritize_then_take_tab_ai_experience_intents(
        build_tab_ai_experience_intents(
            focused_target,
            visible_targets,
            clipboard,
            prior_automations,
        ),
        3,
    );
    if intents.is_empty() {
        return None;
    }
    Some(TabAiExperienceSpec {
        pack,
        title: tab_ai_experience_pack_name(pack).to_string(),
        subtitle: tab_ai_experience_pack_subtitle(pack).to_string(),
        intents,
    })
}

pub fn tab_ai_experience_pack_name(pack: TabAiExperiencePack) -> &'static str {
    match pack {
        TabAiExperiencePack::DesktopGeneral => "Next Move",
        TabAiExperiencePack::ClipboardStudio => "Clipboard Studio",
        TabAiExperiencePack::FileStudio => "File Studio",
        TabAiExperiencePack::FolderStudio => "Folder Studio",
        TabAiExperiencePack::CommandAlchemy => "Command Alchemy",
        TabAiExperiencePack::AppPilot => "App Pilot",
        TabAiExperiencePack::WindowPilot => "Window Pilot",
        TabAiExperiencePack::ProcessPilot => "Process Pilot",
        TabAiExperiencePack::GenericSelection => "Selected Item",
    }
}

impl TabAiExperiencePack {
    pub fn from_target(target: Option<&TabAiTargetContext>) -> Self {
        match target.map(|t| t.kind.as_str()) {
            Some("clipboard_entry") => Self::ClipboardStudio,
            Some("file") => Self::FileStudio,
            Some("directory") => Self::FolderStudio,
            Some("menu_command") => Self::CommandAlchemy,
            Some("app") => Self::AppPilot,
            Some("window") => Self::WindowPilot,
            Some("process") => Self::ProcessPilot,
            Some(_) => Self::GenericSelection,
            None => Self::DesktopGeneral,
        }
    }
}

fn push_unique_tab_ai_experience(
    out: &mut Vec<TabAiExperienceIntent>,
    seen: &mut BTreeSet<String>,
    label: impl Into<String>,
    intent: impl Into<String>,
) {
    push_unique_tab_ai_experience_with_flavor_and_rank(
        out,
        seen,
        label,
        intent,
        TabAiExperienceFlavor::Generic,
        u8::MAX,
    );
}

fn push_unique_tab_ai_experience_with_flavor(
    out: &mut Vec<TabAiExperienceIntent>,
    seen: &mut BTreeSet<String>,
    label: impl Into<String>,
    intent: impl Into<String>,
    flavor: TabAiExperienceFlavor,
) {
    push_unique_tab_ai_experience_with_flavor_and_rank(out, seen, label, intent, flavor, u8::MAX);
}

fn push_unique_tab_ai_experience_with_flavor_and_rank(
    out: &mut Vec<TabAiExperienceIntent>,
    seen: &mut BTreeSet<String>,
    label: impl Into<String>,
    intent: impl Into<String>,
    flavor: TabAiExperienceFlavor,
    spotlight_rank: u8,
) {
    let item = TabAiExperienceIntent::new(label, intent)
        .with_flavor(flavor)
        .with_spotlight_rank(spotlight_rank);
    let key = format!("{}::{}", item.label, item.intent);
    if seen.insert(key) {
        out.push(item);
    }
}

fn push_unique_tab_ai_experience_ranked(
    out: &mut Vec<TabAiExperienceIntent>,
    seen: &mut BTreeSet<String>,
    label: impl Into<String>,
    intent: impl Into<String>,
    spotlight_rank: u8,
) {
    push_unique_tab_ai_experience_with_flavor_and_rank(
        out,
        seen,
        label,
        intent,
        TabAiExperienceFlavor::Generic,
        spotlight_rank,
    );
}

fn focused_content_type<'a>(
    focused_target: Option<&'a TabAiTargetContext>,
    clipboard: Option<&'a TabAiClipboardContext>,
) -> Option<&'a str> {
    focused_target
        .and_then(|target| target.metadata.as_ref())
        .and_then(|metadata| metadata.get("contentType"))
        .and_then(|value| value.as_str())
        .or_else(|| clipboard.map(|entry| entry.content_type.as_str()))
}

/// Build surface-native experience intents based on the focused target, visible
/// targets, clipboard, and prior automations.  Returns at most 5 suggestions.
pub fn build_tab_ai_experience_intents(
    focused_target: Option<&TabAiTargetContext>,
    visible_targets: &[TabAiTargetContext],
    clipboard: Option<&TabAiClipboardContext>,
    prior_automations: &[TabAiMemorySuggestion],
) -> Vec<TabAiExperienceIntent> {
    let pack = TabAiExperiencePack::from_target(focused_target);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();

    match pack {
        TabAiExperiencePack::ClipboardStudio => {
            match focused_content_type(focused_target, clipboard) {
                Some("image") => {
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Describe Image",
                        "Describe this copied image, extract any useful text, and suggest the best next action.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Make Alt Text",
                        "Write concise alt text for this copied image and copy the result.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Turn Into Script Input",
                        "Turn the useful text in this copied image into a clean Script Kit input value.",
                    );
                }
                Some("link") => {
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Open Best App",
                        "Open this copied link in the best app and tell me the fastest next step.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Summarize Link",
                        "Summarize what this copied link is likely for and suggest a command I can save.",
                    );
                    push_unique_tab_ai_experience_with_flavor(
                        &mut out,
                        &mut seen,
                        "Make Link Command",
                        "Create a reusable Script Kit command that works on copied links like this one.",
                        TabAiExperienceFlavor::Teachable,
                    );
                }
                Some("color") => {
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Build Palette",
                        "Turn this copied color into a five-color palette with CSS variables.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Theme Tokens",
                        "Generate light and dark theme tokens from this copied color.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Name This Color",
                        "Give this copied color a useful human name and a good design-token name.",
                    );
                }
                _ => {
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Clean Clipboard",
                        "Clean up this copied text and preserve the meaning.",
                    );
                    push_unique_tab_ai_experience(
                        &mut out,
                        &mut seen,
                        "Turn Into Checklist",
                        "Turn this copied text into a tight checklist.",
                    );
                    push_unique_tab_ai_experience_with_flavor(
                        &mut out,
                        &mut seen,
                        "Make Command",
                        "Turn this copied content into a reusable Script Kit command.",
                        TabAiExperienceFlavor::Teachable,
                    );
                }
            }
        }
        TabAiExperiencePack::FileStudio => {
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Clone This Pattern",
                "Use this file as a pattern and create the matching test, implementation, or sibling file I am probably missing.",
                TabAiExperienceFlavor::Adaptation,
                SPOTLIGHT_PATTERN_HERO,
            );
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Turn This Into a Tool",
                "Turn this file and its nearby project context into a reusable Script Kit command for the repeatable task around it.",
                TabAiExperienceFlavor::Teachable,
                SPOTLIGHT_TEACHABLE,
            );
            push_unique_tab_ai_experience_ranked(
                &mut out,
                &mut seen,
                "Summarize File",
                "Summarize this file, tell me what it is for, and suggest the next edit.",
                SPOTLIGHT_FALLBACK,
            );
        }
        TabAiExperiencePack::FolderStudio => {
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Spin Project Operator",
                "Turn this folder into a reusable Script Kit project operator with the most important entrypoints and actions.",
                TabAiExperienceFlavor::Teachable,
                SPOTLIGHT_TEACHABLE,
            );
            push_unique_tab_ai_experience_ranked(
                &mut out,
                &mut seen,
                "Find the Hot Path",
                "Find the real entrypoint in this folder, the file I should open next, and the fastest command to move forward.",
                SPOTLIGHT_FALLBACK,
            );
            push_unique_tab_ai_experience_ranked(
                &mut out,
                &mut seen,
                "Map the Territory",
                "Explain what this folder contains, where the real entrypoints are, and what I should open first.",
                SPOTLIGHT_FALLBACK.saturating_add(1),
            );
        }
        TabAiExperiencePack::CommandAlchemy => {
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Run This Command",
                "Run this selected current-app command.",
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Explain This Command",
                "Explain what this selected current-app command probably does and when to use it.",
            );
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Teach This Command",
                "Turn this selected current-app command into a reusable Script Kit command.",
                TabAiExperienceFlavor::Teachable,
            );
        }
        TabAiExperiencePack::AppPilot => {
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Turn This Into Command",
                "Capture what I need to do in this app as a reusable Script Kit command.",
                TabAiExperienceFlavor::Teachable,
            );
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Automate This App",
                "Find the fastest reusable Script Kit automation for what I need in this app right now.",
                TabAiExperienceFlavor::Teachable,
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Find Right Window",
                "Find or open the best window for the task I am trying to do in this app.",
            );
        }
        TabAiExperiencePack::WindowPilot => {
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Explain Window",
                "Explain what this window is for from its app and title.",
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Use Clipboard Here",
                "Use the copied content in this selected window in the fastest safe way.",
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Close Similar Windows",
                "Close the other windows from this app and keep this one.",
            );
        }
        TabAiExperiencePack::ProcessPilot => {
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Explain Process",
                "Explain what this running Script Kit process is doing and whether it looks healthy.",
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Stop Safely",
                "Stop this running Script Kit process and tell me what I should run next.",
            );
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Make It Reusable",
                "Turn what this running Script Kit process does into a reusable launcher command.",
                TabAiExperienceFlavor::Teachable,
            );
        }
        TabAiExperiencePack::GenericSelection => {
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Act On Selection",
                "Act on this selected item using the most direct Script Kit action.",
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Explain Selection",
                "Explain what this selected item is and what I can do with it.",
            );
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Make Command",
                "Turn this selection into a reusable Script Kit command.",
                TabAiExperienceFlavor::Teachable,
            );
        }
        TabAiExperiencePack::DesktopGeneral => {
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Continue the Thread",
                "Use whatever Script Kit can currently see -- frontmost app, selected text, browser URL, and clipboard if present -- to continue the task I am already doing.",
                TabAiExperienceFlavor::Fusion,
            );
            push_unique_tab_ai_experience_with_flavor(
                &mut out,
                &mut seen,
                "Make This Ritual",
                "Turn what I am doing right now into a reusable Script Kit command with smart defaults from the current app and selection.",
                TabAiExperienceFlavor::Teachable,
            );
            push_unique_tab_ai_experience(
                &mut out,
                &mut seen,
                "Inspect Current Context",
                "Summarize what Script Kit can currently see and tell me the best next move.",
            );
        }
    }

    // Visible-target batch suggestions
    if visible_targets.len() > 1 {
        let all_files = visible_targets
            .iter()
            .all(|target| target.kind == "file" || target.kind == "directory");
        let all_menu_commands = visible_targets
            .iter()
            .all(|target| target.kind == "menu_command");

        if all_files {
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Sweep Visible Files",
                "Act on the visible files as a set, not just the selected file.",
                TabAiExperienceFlavor::Batch,
                SPOTLIGHT_BATCH_HERO,
            );
        } else if all_menu_commands {
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Pick Best Command",
                "Compare the visible current-app commands and pick the best one for my goal.",
                TabAiExperienceFlavor::Batch,
                SPOTLIGHT_BATCH_HERO,
            );
        } else {
            push_unique_tab_ai_experience_with_flavor_and_rank(
                &mut out,
                &mut seen,
                "Use Visible Items",
                "Use the visible items on this surface, not just the selected one.",
                TabAiExperienceFlavor::Batch,
                SPOTLIGHT_BATCH_HERO,
            );
        }
    }

    // Cross-context fusion (focused target + clipboard)
    if let (Some(target), Some(entry)) = (focused_target, clipboard) {
        match (target.kind.as_str(), entry.content_type.as_str()) {
            ("file", "text") | ("file", "link") => {
                push_unique_tab_ai_experience_with_flavor_and_rank(
                    &mut out,
                    &mut seen,
                    "Rename From Clipboard",
                    "Rename this file using the clipboard text as the source of truth.",
                    TabAiExperienceFlavor::Fusion,
                    SPOTLIGHT_CONTEXT_HERO,
                );
            }
            ("window", "link") | ("app", "link") => {
                push_unique_tab_ai_experience_with_flavor_and_rank(
                    &mut out,
                    &mut seen,
                    "Send Link Here",
                    "Use the copied link in this selected app or window and continue the task.",
                    TabAiExperienceFlavor::Fusion,
                    SPOTLIGHT_CONTEXT_HERO,
                );
            }
            ("menu_command", "text") => {
                push_unique_tab_ai_experience_with_flavor_and_rank(
                    &mut out,
                    &mut seen,
                    "Apply Clipboard Then Run",
                    "Use the clipboard text with this selected current-app command if it helps complete the task.",
                    TabAiExperienceFlavor::Fusion,
                    SPOTLIGHT_CONTEXT_HERO,
                );
            }
            _ => {}
        }
    }

    // Prior automation adaptation
    if let Some(last) = prior_automations.first() {
        push_unique_tab_ai_experience_with_flavor_and_rank(
            &mut out,
            &mut seen,
            "Reuse My Last Flow",
            format!(
                "Adapt my previous successful automation '{}' to the current context.",
                last.effective_query
            ),
            TabAiExperienceFlavor::Adaptation,
            SPOTLIGHT_MEMORY_HERO,
        );
    }

    prioritize_then_take_tab_ai_experience_intents(out, 5)
}

#[cfg(test)]
mod tab_ai_experience_tests {
    use super::*;

    fn target(kind: &str, label: &str) -> TabAiTargetContext {
        TabAiTargetContext {
            source: "TestSurface".to_string(),
            kind: kind.to_string(),
            semantic_id: format!("choice:0:{label}"),
            label: label.to_string(),
            metadata: None,
        }
    }

    fn clipboard(kind: &str) -> TabAiClipboardContext {
        TabAiClipboardContext {
            content_type: kind.to_string(),
            preview: "example".to_string(),
            ocr_text: None,
        }
    }

    #[test]
    fn command_alchemy_prioritizes_teachable_actions() {
        let focused = target("menu_command", "New Private Window");
        let visible = vec![focused.clone()];
        let intents = build_tab_ai_experience_intents(Some(&focused), &visible, None, &[]);
        let labels: Vec<&str> = intents.iter().map(|item| item.label.as_str()).collect();
        // build_tab_ai_experience_intents now returns prioritized order
        assert_eq!(
            labels,
            vec![
                "Teach This Command",
                "Run This Command",
                "Explain This Command"
            ]
        );
    }

    #[test]
    fn file_studio_adds_clipboard_fusion() {
        let focused = target("file", "agent_handoff.rs");
        let visible = vec![focused.clone()];
        let intents = build_tab_ai_experience_intents(
            Some(&focused),
            &visible,
            Some(&clipboard("text")),
            &[],
        );
        let labels: Vec<&str> = intents.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"Rename From Clipboard"));
    }

    #[test]
    fn desktop_general_uses_visible_file_batching() {
        let visible = vec![target("file", "a.rs"), target("file", "b.rs")];
        let intents = build_tab_ai_experience_intents(None, &visible, None, &[]);
        let labels: Vec<&str> = intents.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"Sweep Visible Files"));
    }

    #[test]
    fn truncates_to_five_suggestions() {
        let focused = target("file", "main.rs");
        let visible = vec![focused.clone(), target("file", "lib.rs")];
        let intents = build_tab_ai_experience_intents(
            Some(&focused),
            &visible,
            Some(&clipboard("text")),
            &[TabAiMemorySuggestion {
                slug: "prev".to_string(),
                bundle_id: "com.test".to_string(),
                raw_query: "test".to_string(),
                effective_query: "test query".to_string(),
                prompt_type: "arg".to_string(),
                written_at: "2026-01-01T00:00:00Z".to_string(),
                score: 1.0,
            }],
        );
        assert!(intents.len() <= 5);
    }

    #[test]
    fn experience_intent_converts_to_spec() {
        let intent = TabAiExperienceIntent::new("Test Label", "test intent");
        let spec = intent.into_spec();
        assert_eq!(spec.label, "Test Label");
        assert_eq!(spec.intent, "test intent");
    }

    #[test]
    fn experience_spec_uses_file_studio_for_focused_file() {
        let focused = target("file", "agent_handoff.rs");
        let visible = vec![focused.clone()];
        let spec = build_tab_ai_experience_spec(Some(&focused), &visible, None, &[])
            .expect("file experience spec");
        assert_eq!(spec.title, "File Studio");
        assert_eq!(spec.subtitle, "Act on the selected file in-place.");
        assert_eq!(spec.intents.len(), 3);
        assert_eq!(spec.intents[0].label, "Clone This Pattern");
        assert_eq!(spec.intents[1].label, "Turn This Into a Tool");
        assert_eq!(spec.intents[2].label, "Summarize File");
    }

    #[test]
    fn experience_spec_uses_command_alchemy_for_menu_command() {
        let focused = target("menu_command", "New Private Window");
        let visible = vec![focused.clone()];
        let spec = build_tab_ai_experience_spec(Some(&focused), &visible, None, &[])
            .expect("command experience spec");
        assert_eq!(spec.title, "Command Alchemy");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        // "Teach This Command" is tier 1, promoted above tier 2 generic verbs
        assert_eq!(
            labels,
            vec![
                "Teach This Command",
                "Run This Command",
                "Explain This Command"
            ]
        );
    }

    #[test]
    fn experience_spec_uses_clipboard_studio_for_copied_color() {
        let focused = target("clipboard_entry", "Copied Color");
        let visible = vec![focused.clone()];
        let spec =
            build_tab_ai_experience_spec(Some(&focused), &visible, Some(&clipboard("color")), &[])
                .expect("clipboard color experience spec");
        assert_eq!(spec.title, "Clipboard Studio");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert!(labels.contains(&"Build Palette"));
    }

    #[test]
    fn experience_spec_promotes_clipboard_fusion_into_top_three() {
        let focused = target("file", "agent_handoff.rs");
        let visible = vec![focused.clone()];
        let spec =
            build_tab_ai_experience_spec(Some(&focused), &visible, Some(&clipboard("text")), &[])
                .expect("expected file studio experience spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(spec.title, "File Studio");
        assert_eq!(spec.subtitle, "Act on the selected file in-place.");
        assert_eq!(labels[0], "Rename From Clipboard");
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn experience_spec_promotes_visible_batch_into_top_three() {
        let visible = vec![target("file", "a.rs"), target("file", "b.rs")];
        let spec = build_tab_ai_experience_spec(None, &visible, None, &[])
            .expect("expected desktop experience spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(spec.title, "Next Move");
        // Mixed shortlist: one hero (Sweep), one teachable (Ritual), one fallback (Inspect)
        assert!(labels.contains(&"Sweep Visible Files"));
        assert!(labels.contains(&"Make This Ritual"));
        assert_eq!(labels.len(), 3);
    }

    #[test]
    fn experience_spec_mixed_shortlist_with_prior_automation() {
        let focused = target("file", "main.rs");
        let visible = vec![focused.clone()];
        let spec = build_tab_ai_experience_spec(
            Some(&focused),
            &visible,
            None,
            &[TabAiMemorySuggestion {
                slug: "rename-kebab".to_string(),
                bundle_id: "com.test".to_string(),
                raw_query: "rename files".to_string(),
                effective_query: "rename files to kebab case".to_string(),
                prompt_type: "arg".to_string(),
                written_at: "2026-01-01T00:00:00Z".to_string(),
                score: 1.0,
            }],
        )
        .expect("expected file studio experience spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        // Mixed shortlist picks one hero, one teachable, one fallback.
        // Clone This Pattern (hero/adaptation) beats Reuse My Last Flow on rank.
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0], "Clone This Pattern");
        assert_eq!(labels[1], "Turn This Into a Tool");
        assert_eq!(labels[2], "Summarize File");
    }

    fn memory(slug: &str, effective_query: &str) -> TabAiMemorySuggestion {
        TabAiMemorySuggestion {
            slug: slug.to_string(),
            bundle_id: "com.test".to_string(),
            raw_query: effective_query.to_string(),
            effective_query: effective_query.to_string(),
            prompt_type: "arg".to_string(),
            written_at: "2026-01-01T00:00:00Z".to_string(),
            score: 1.0,
        }
    }

    #[test]
    fn command_alchemy_shortlist_promotes_teaching() {
        let focused = target("menu_command", "New Private Window");
        let visible = vec![focused.clone()];
        let spec = build_tab_ai_experience_spec(Some(&focused), &visible, None, &[])
            .expect("command spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Teach This Command",
                "Run This Command",
                "Explain This Command"
            ]
        );
    }

    #[test]
    fn rich_file_context_shortlist_prefers_fusion_then_teachable_then_fallback() {
        let focused = target("file", "main.rs");
        let visible = vec![focused.clone(), target("file", "lib.rs")];
        let spec = build_tab_ai_experience_spec(
            Some(&focused),
            &visible,
            Some(&clipboard("text")),
            &[memory("rename-rust-module", "rename rust module")],
        )
        .expect("file spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        // Mixed shortlist: one hero (Rename), one teachable (Tool), one fallback (Summarize)
        assert_eq!(
            labels,
            vec![
                "Rename From Clipboard",
                "Turn This Into a Tool",
                "Summarize File"
            ]
        );
    }

    #[test]
    fn desktop_general_shortlist_uses_script_kit_native_language() {
        let visible: Vec<TabAiTargetContext> = vec![];
        let spec = build_tab_ai_experience_spec(None, &visible, None, &[]).expect("desktop spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Continue the Thread",
                "Make This Ritual",
                "Inspect Current Context"
            ]
        );
    }

    #[test]
    fn app_pilot_shortlist_contains_command_capture() {
        let focused = target("app", "Safari");
        let visible = vec![focused.clone()];
        let spec =
            build_tab_ai_experience_spec(Some(&focused), &visible, None, &[]).expect("app spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Turn This Into Command",
                "Find Right Window",
                "Automate This App"
            ]
        );
    }

    #[test]
    fn desktop_general_shortlist_prefers_thread_then_ritual() {
        let visible: Vec<TabAiTargetContext> = vec![];
        let spec = build_tab_ai_experience_spec(None, &visible, None, &[]).expect("desktop spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Continue the Thread",
                "Make This Ritual",
                "Inspect Current Context"
            ]
        );
    }

    #[test]
    fn file_studio_shortlist_prefers_pattern_then_tool_then_fallback() {
        let focused = target("file", "main.rs");
        let visible = vec![focused.clone()];
        let spec =
            build_tab_ai_experience_spec(Some(&focused), &visible, None, &[]).expect("file spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Clone This Pattern",
                "Turn This Into a Tool",
                "Summarize File"
            ]
        );
    }

    #[test]
    fn folder_studio_shortlist_prefers_operator_then_hot_path() {
        let focused = target("directory", "src");
        let visible = vec![focused.clone()];
        let spec =
            build_tab_ai_experience_spec(Some(&focused), &visible, None, &[]).expect("folder spec");
        let labels: Vec<&str> = spec
            .intents
            .iter()
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "Spin Project Operator",
                "Find the Hot Path",
                "Map the Territory"
            ]
        );
    }

    #[test]
    fn prior_automation_label_is_humanized() {
        let focused = target("file", "main.rs");
        let visible = vec![focused.clone()];
        let intents = build_tab_ai_experience_intents(
            Some(&focused),
            &visible,
            None,
            &[memory("rename-rust-module", "rename rust module")],
        );
        let labels: Vec<&str> = intents.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"Reuse My Last Flow"));
    }
}

#[cfg(test)]
mod tab_ai_experience_shortlist_tests {
    use super::*;

    fn labels(intents: &[TabAiExperienceIntent]) -> Vec<String> {
        intents.iter().map(|intent| intent.label.clone()).collect()
    }

    #[test]
    fn spotlight_rank_breaks_ties_inside_hero_tier() {
        let ordered = prioritize_tab_ai_experience_card_intents(vec![
            TabAiExperienceIntent::new("Later Hero", "later")
                .with_flavor(TabAiExperienceFlavor::Fusion)
                .with_spotlight_rank(2),
            TabAiExperienceIntent::new("Sooner Hero", "sooner")
                .with_flavor(TabAiExperienceFlavor::Fusion)
                .with_spotlight_rank(0),
            TabAiExperienceIntent::new("Teach It", "teach")
                .with_flavor(TabAiExperienceFlavor::Teachable)
                .with_spotlight_rank(0),
        ]);
        assert_eq!(
            labels(&ordered),
            vec![
                "Sooner Hero".to_string(),
                "Later Hero".to_string(),
                "Teach It".to_string(),
            ],
        );
    }

    #[test]
    fn shortlist_keeps_one_hero_one_teachable_one_fallback_when_available() {
        let shortlisted = prioritize_then_take_tab_ai_experience_intents(
            vec![
                TabAiExperienceIntent::new("Rename From Clipboard", "rename")
                    .with_flavor(TabAiExperienceFlavor::Fusion)
                    .with_spotlight_rank(0),
                TabAiExperienceIntent::new("Reuse My Last Flow", "reuse")
                    .with_flavor(TabAiExperienceFlavor::Adaptation)
                    .with_spotlight_rank(2),
                TabAiExperienceIntent::new("Sweep Visible Files", "sweep")
                    .with_flavor(TabAiExperienceFlavor::Batch)
                    .with_spotlight_rank(4),
                TabAiExperienceIntent::new("Turn This Into a Tool", "teach")
                    .with_flavor(TabAiExperienceFlavor::Teachable)
                    .with_spotlight_rank(3),
                TabAiExperienceIntent::new("Summarize File", "summary")
                    .with_flavor(TabAiExperienceFlavor::Generic)
                    .with_spotlight_rank(10),
            ],
            3,
        );
        assert_eq!(
            labels(&shortlisted),
            vec![
                "Rename From Clipboard".to_string(),
                "Turn This Into a Tool".to_string(),
                "Summarize File".to_string(),
            ],
        );
    }

    #[test]
    fn shortlist_fills_remaining_slots_with_sorted_overflow() {
        let shortlisted = prioritize_then_take_tab_ai_experience_intents(
            vec![
                TabAiExperienceIntent::new("Rename From Clipboard", "rename")
                    .with_flavor(TabAiExperienceFlavor::Fusion)
                    .with_spotlight_rank(0),
                TabAiExperienceIntent::new("Reuse My Last Flow", "reuse")
                    .with_flavor(TabAiExperienceFlavor::Adaptation)
                    .with_spotlight_rank(2),
                TabAiExperienceIntent::new("Turn This Into a Tool", "teach")
                    .with_flavor(TabAiExperienceFlavor::Teachable)
                    .with_spotlight_rank(3),
                TabAiExperienceIntent::new("Summarize File", "summary")
                    .with_flavor(TabAiExperienceFlavor::Generic)
                    .with_spotlight_rank(10),
            ],
            4,
        );
        assert_eq!(
            labels(&shortlisted),
            vec![
                "Rename From Clipboard".to_string(),
                "Turn This Into a Tool".to_string(),
                "Summarize File".to_string(),
                "Reuse My Last Flow".to_string(),
            ],
        );
    }
}

/// What kind of source the user was focused on when Tab was pressed.
///
/// Used by the harness backend to understand the provenance of the context
/// and by the apply-back flow to route the result back to the right target.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TabAiSourceType {
    /// User had text selected on the desktop (not inside Script Kit).
    DesktopSelection,
    /// User was focused on a script in the main list.
    ScriptListItem,
    /// User was inside a running command with a focused choice or prompt.
    RunningCommand,
    /// User was focused on a clipboard history entry.
    ClipboardEntry,
    /// Fallback: user was on the desktop with nothing specific selected.
    Desktop,
}

/// Hint for the apply-back flow: what action to take when the user finishes
/// in the harness and wants to push the result back to the source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiApplyBackHint {
    /// Action identifier (e.g. "replaceSelectedText", "runGeneratedScript",
    /// "pasteToPrompt", "copyToClipboard", "pasteToFrontmostApp").
    pub action: String,
    /// Optional human-readable label for the target (e.g. "Frontmost selection").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_label: Option<String>,
}

/// Routing state for the apply-back flow: pairs the detected source classification
/// with the apply-back hint so the app can execute the right action when the user
/// presses ⌘⏎ in the harness terminal.
///
/// `focused_target` carries the resolved target metadata captured at Tab-press
/// time so the apply-back handler can route results without rediscovering UI
/// state after the harness closes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiApplyBackRoute {
    pub source_type: TabAiSourceType,
    pub hint: TabAiApplyBackHint,
    /// The focused target captured at invocation time. Populated for source
    /// types that resolve a concrete target (e.g. `ScriptListItem`,
    /// `RunningCommand`). `None` for generic desktop or desktop selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_target: Option<TabAiTargetContext>,
}

/// Return a concise footer label describing what ⌘↩ Apply does for a given source type.
pub fn tab_ai_apply_back_footer_label(source_type: Option<&TabAiSourceType>) -> &'static str {
    match source_type {
        Some(TabAiSourceType::RunningCommand) => "Paste Back to Prompt",
        Some(TabAiSourceType::ClipboardEntry) => "Copy Result",
        Some(TabAiSourceType::ScriptListItem) => "Save as Script & Run",
        Some(TabAiSourceType::DesktopSelection) => "Replace Selection",
        Some(TabAiSourceType::Desktop) => "Paste Back to App",
        None => "Preparing Paste Back\u{2026}",
    }
}

/// Detect the source type from the originating prompt type string and desktop snapshot.
///
/// This is the canonical detection logic, usable from both include!() files
/// and proper module tests.
///
/// Priority order (Script Kit origin surfaces beat incidental desktop selection):
/// 1. `"ScriptList"` with a resolved focused target → `ScriptListItem`
/// 2. `"ClipboardHistory"` → `ClipboardEntry`
/// 3. Prompt-like surfaces → `RunningCommand`
/// 4. Desktop selected text present (no stronger Script Kit origin) → `DesktopSelection`
/// 5. Fallback → `Desktop`
pub fn detect_tab_ai_source_type_from_prompt(
    prompt_type: &str,
    desktop: &crate::context_snapshot::AiContextSnapshot,
    focused_target: Option<&TabAiTargetContext>,
) -> Option<TabAiSourceType> {
    match prompt_type {
        "ScriptList" if focused_target.is_some() => Some(TabAiSourceType::ScriptListItem),
        "ClipboardHistory" => Some(TabAiSourceType::ClipboardEntry),
        "ArgPrompt" | "MiniPrompt" | "MicroPrompt" | "DivPrompt" | "FormPrompt"
        | "EditorPrompt" | "SelectPrompt" | "PathPrompt" | "DropPrompt" | "TemplatePrompt"
        | "HotkeyPrompt" | "TermPrompt" | "EnvPrompt" | "ChatPrompt" | "NamingPrompt" => {
            Some(TabAiSourceType::RunningCommand)
        }
        _ if desktop
            .selected_text
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty()) =>
        {
            Some(TabAiSourceType::DesktopSelection)
        }
        _ => Some(TabAiSourceType::Desktop),
    }
}

/// Build an apply-back hint from the detected source type.
pub fn build_tab_ai_apply_back_hint_from_source(
    source_type: Option<&TabAiSourceType>,
) -> Option<TabAiApplyBackHint> {
    let (action, label) = match source_type? {
        TabAiSourceType::DesktopSelection => ("replaceSelectedText", "Frontmost selection"),
        TabAiSourceType::ScriptListItem => ("runGeneratedScript", "Focused script"),
        TabAiSourceType::RunningCommand => ("pasteToPrompt", "Active prompt"),
        TabAiSourceType::ClipboardEntry => ("copyToClipboard", "Clipboard"),
        TabAiSourceType::Desktop => ("pasteToFrontmostApp", "Frontmost app"),
    };
    Some(TabAiApplyBackHint {
        action: action.to_string(),
        target_label: Some(label.to_string()),
    })
}

/// Complete context blob sent alongside the user's natural-language intent.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TabAiContextBlob {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// ISO-8601 timestamp of when the context was assembled.
    pub timestamp: String,
    /// UI state at invocation time.
    pub ui: TabAiUiSnapshot,
    /// The primary target the user is acting on (the "this" in "do this to that").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_target: Option<TabAiTargetContext>,
    /// Top visible targets from the active surface (fallback when focusedTarget is absent).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_targets: Vec<TabAiTargetContext>,
    /// Desktop context (frontmost app, selected text, browser URL).
    pub desktop: crate::context_snapshot::AiContextSnapshot,
    /// Recent input-history entries (most recent first).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_inputs: Vec<String>,
    /// Structured clipboard context (content type, preview, optional OCR).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<TabAiClipboardContext>,
    /// Hydrated clipboard history entries (last N, most recent first).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clipboard_history: Vec<TabAiClipboardHistoryEntry>,
    /// Prior automation suggestions from the Tab AI memory index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prior_automations: Vec<TabAiMemorySuggestion>,
    /// What kind of source the user was focused on when Tab was pressed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<TabAiSourceType>,
    /// Absolute path to a screenshot of the focused window captured at invocation time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
    /// Hint for the apply-back flow: what action to take when the user finishes in the harness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_back_hint: Option<TabAiApplyBackHint>,
}

impl TabAiContextBlob {
    /// Build a context blob with explicit target information.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts_with_targets(
        ui: TabAiUiSnapshot,
        focused_target: Option<TabAiTargetContext>,
        visible_targets: Vec<TabAiTargetContext>,
        desktop: crate::context_snapshot::AiContextSnapshot,
        recent_inputs: Vec<String>,
        clipboard: Option<TabAiClipboardContext>,
        clipboard_history: Vec<TabAiClipboardHistoryEntry>,
        prior_automations: Vec<TabAiMemorySuggestion>,
        timestamp: String,
    ) -> Self {
        Self {
            schema_version: TAB_AI_CONTEXT_SCHEMA_VERSION,
            timestamp,
            ui,
            focused_target,
            visible_targets,
            desktop,
            recent_inputs,
            clipboard,
            clipboard_history,
            prior_automations,
            source_type: None,
            screenshot_path: None,
            apply_back_hint: None,
        }
    }

    /// Apply deferred-capture fields after the initial blob was constructed.
    ///
    /// This is the extension point for the async capture pipeline: the blob
    /// is built synchronously with UI + desktop data, then enriched with
    /// screenshot path, source type, and apply-back hint once the deferred
    /// capture completes.
    pub fn with_deferred_capture_fields(
        mut self,
        source_type: Option<TabAiSourceType>,
        screenshot_path: Option<String>,
        apply_back_hint: Option<TabAiApplyBackHint>,
    ) -> Self {
        self.source_type = source_type;
        self.screenshot_path = screenshot_path;
        self.apply_back_hint = apply_back_hint;
        self
    }

    /// Build a context blob from provided parts — no system calls, fully
    /// deterministic.  Intended for tests and for callers that already hold
    /// resolved data.  Delegates to `from_parts_with_targets` with no targets.
    pub fn from_parts(
        ui: TabAiUiSnapshot,
        desktop: crate::context_snapshot::AiContextSnapshot,
        recent_inputs: Vec<String>,
        clipboard: Option<TabAiClipboardContext>,
        clipboard_history: Vec<TabAiClipboardHistoryEntry>,
        prior_automations: Vec<TabAiMemorySuggestion>,
        timestamp: String,
    ) -> Self {
        Self::from_parts_with_targets(
            ui,
            None,
            Vec::new(),
            desktop,
            recent_inputs,
            clipboard,
            clipboard_history,
            prior_automations,
            timestamp,
        )
    }
}

/// Legacy helper for the old inline script-generation flow.
///
/// This is not the primary Tab AI surface anymore.
/// The primary Tab AI path builds a `TabAiContextBlob` and injects it into the
/// warm harness terminal via `build_tab_ai_harness_submission()`.
///
/// Keep this only for compatibility code paths that still need the older
/// script-generation prompt contract.
pub fn build_tab_ai_user_prompt(intent: &str, context_json: &str) -> String {
    format!(
        "User intent:\n{intent}\n\n\
         Context JSON:\n\
         ```json\n\
         {context_json}\n\
         ```\n\n\
         Write one valid Script Kit TypeScript script.\n\
         - Use the live context as the source of truth.\n\
         - focusedTarget is the default subject when the intent says \"this\", \"it\", \"selected\", or leaves the object implicit.\n\
         - If focusedTarget.metadata contains identifiers (path, bundleId, pid, command, url), use those exact values instead of guessing from labels.\n\
         - visibleTargets are fallbacks only when focusedTarget is absent or the intent clearly refers to multiple visible items.\n\
         - If no focusedTarget exists, do not invent an implicit subject. Operate only on explicit data from the intent or desktop context.\n\
         - Prefer desktop.selectedText, desktop.browser.url, and desktop.frontmostApp for desktop targets.\n\
         - Use clipboard.preview or clipboard.ocrText when the request refers to copied or pasted content.\n\
         - Treat priorAutomations as hints only; borrow their shape if useful, but do not assume they are still correct if live context disagrees.\n\
         - Keep the script short and directly executable.\n\
         - Return only a fenced ```ts block.\n",
        intent = intent.trim(),
        context_json = context_json,
    )
}

/// Check whether the user's intent uses implicit target pronouns.
///
/// Returns `true` when the intent contains words like "this", "it", "that",
/// "selected", "current", or "focused" — tokens that imply the action targets
/// whatever is currently focused/selected on screen.
///
/// Also covers a small set of object-elision commands that the Tab AI contract
/// treats as acting on the current selection by default, such as
/// "rename to kebab-case" or bare "force quit".
pub fn tab_ai_intent_uses_implicit_target(intent: &str) -> bool {
    let normalized_tokens: Vec<String> = intent
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(ToString::to_string)
        .collect();

    let token_set: std::collections::BTreeSet<&str> =
        normalized_tokens.iter().map(String::as_str).collect();

    if token_set.contains("this")
        || token_set.contains("it")
        || token_set.contains("that")
        || token_set.contains("selected")
        || token_set.contains("current")
        || token_set.contains("focused")
    {
        return true;
    }

    let first = normalized_tokens.first().map(String::as_str);
    let second = normalized_tokens.get(1).map(String::as_str);
    let third = normalized_tokens.get(2).map(String::as_str);

    matches!(
        (first, second, third),
        (Some("rename"), Some("to" | "as" | "into"), _)
            | (
                Some("convert" | "change" | "transform" | "format"),
                Some("to" | "as" | "into"),
                _
            )
            | (Some("force"), Some("quit"), None)
            | (
                Some("quit" | "close" | "delete" | "remove" | "duplicate" | "kill"),
                None,
                None
            )
    )
}

/// Schema version for `TabAiExecutionRecord`. Bump when adding/removing/renaming fields.
pub const TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION: u32 = 2;

/// Schema version for `TabAiExecutionReceipt`. Bump when adding/removing/renaming fields.
pub const TAB_AI_EXECUTION_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Execution lifecycle status for append-only audit receipts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TabAiExecutionStatus {
    Dispatched,
    Succeeded,
    Failed,
}

/// Record captured at dispatch time and carried forward until completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabAiExecutionRecord {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// The user's original natural-language intent.
    pub intent: String,
    /// The TypeScript source the AI generated.
    pub generated_source: String,
    /// Path to the temp `.ts` file that was executed.
    pub temp_script_path: String,
    /// Slug derived from the AI response (used for save naming).
    pub slug: String,
    /// The `AppView` variant name at invocation time.
    pub prompt_type: String,
    /// Bundle ID of the frontmost app at invocation time, if captured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// AI model identifier used for generation.
    #[serde(default)]
    pub model_id: String,
    /// AI provider identifier used for generation.
    #[serde(default)]
    pub provider_id: String,
    /// Number of context-assembly warnings at build time.
    #[serde(default)]
    pub context_warning_count: usize,
    /// ISO-8601 timestamp when the script was executed.
    pub executed_at: String,
}

impl TabAiExecutionRecord {
    /// Build a record from parts — fully deterministic, no system calls.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        intent: String,
        generated_source: String,
        temp_script_path: String,
        slug: String,
        prompt_type: String,
        bundle_id: Option<String>,
        model_id: String,
        provider_id: String,
        context_warning_count: usize,
        executed_at: String,
    ) -> Self {
        Self {
            schema_version: TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION,
            intent,
            generated_source,
            temp_script_path,
            slug,
            prompt_type,
            bundle_id,
            model_id,
            provider_id,
            context_warning_count,
            executed_at,
        }
    }
}

/// Append-only audit receipt written on dispatch and again on completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiExecutionReceipt {
    pub schema_version: u32,
    pub status: TabAiExecutionStatus,
    pub intent: String,
    pub slug: String,
    pub prompt_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub model_id: String,
    pub provider_id: String,
    pub temp_script_path: String,
    pub context_warning_count: usize,
    pub save_offer_eligible: bool,
    pub memory_write_eligible: bool,
    pub cleanup_attempted: bool,
    pub cleanup_succeeded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub written_at: String,
}

/// Returns the file path for the Tab AI execution audit log.
///
/// Located at `~/.scriptkit/scripts/.tab-ai-executions.jsonl`.
pub fn tab_ai_execution_audit_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "tab_ai_execution_audit_path: HOME is not set".to_string())?;
    Ok(std::path::Path::new(&home)
        .join(".scriptkit")
        .join("scripts")
        .join(".tab-ai-executions.jsonl"))
}

/// Build an audit receipt from a record and completion metadata.
pub fn build_tab_ai_execution_receipt(
    record: &TabAiExecutionRecord,
    status: TabAiExecutionStatus,
    cleanup_attempted: bool,
    cleanup_succeeded: bool,
    error: Option<String>,
) -> TabAiExecutionReceipt {
    let memory_write_eligible = matches!(status, TabAiExecutionStatus::Succeeded);
    let save_offer_eligible = memory_write_eligible && should_offer_save(record);

    TabAiExecutionReceipt {
        schema_version: TAB_AI_EXECUTION_RECEIPT_SCHEMA_VERSION,
        status,
        intent: record.intent.clone(),
        slug: record.slug.clone(),
        prompt_type: record.prompt_type.clone(),
        bundle_id: record.bundle_id.clone(),
        model_id: record.model_id.clone(),
        provider_id: record.provider_id.clone(),
        temp_script_path: record.temp_script_path.clone(),
        context_warning_count: record.context_warning_count,
        save_offer_eligible,
        memory_write_eligible,
        cleanup_attempted,
        cleanup_succeeded,
        error,
        written_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Append a single audit receipt as one JSON line to the JSONL audit log.
pub fn append_tab_ai_execution_receipt(receipt: &TabAiExecutionReceipt) -> Result<(), String> {
    append_tab_ai_execution_receipt_to_path(receipt, &tab_ai_execution_audit_path()?)
}

/// Append a single audit receipt to a specific JSONL path (test-friendly).
pub fn append_tab_ai_execution_receipt_to_path(
    receipt: &TabAiExecutionReceipt,
    path: &std::path::Path,
) -> Result<(), String> {
    let line = serde_json::to_vec(receipt).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_execution_audit_serialize_failed", path, &error)
    })?;
    crate::atomic_file::append_private_jsonl_record(path, &line).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_execution_audit_write_failed", path, &error)
    })?;

    let safe_slug = crate::logging::log_private_user_value(&receipt.slug);
    tracing::info!(
        event = "tab_ai_execution_audit_written",
        status = ?receipt.status,
        slug_bytes = safe_slug.raw_bytes,
        slug_sha256 = %safe_slug.sha256,
        prompt_type = %receipt.prompt_type,
        model_id = %receipt.model_id,
        provider_id = %receipt.provider_id,
    );

    Ok(())
}

/// Schema version for `TabAiMemoryEntry`. Bump when adding/removing/renaming fields.
pub const TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION: u32 = 1;

/// Lightweight entry persisted to the Tab AI memory index for future intent matching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiMemoryEntry {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// The user's original natural-language intent.
    pub intent: String,
    /// The TypeScript source the AI generated.
    pub generated_source: String,
    /// Slug derived from the AI response.
    pub slug: String,
    /// The `AppView` variant name at invocation time.
    pub prompt_type: String,
    /// Bundle ID of the frontmost app, if captured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// ISO-8601 timestamp when the entry was written.
    pub written_at: String,
}

/// Returns the file path for the Tab AI memory index.
///
/// Located at `~/.scriptkit/scripts/.tab-ai-memory.json`.
pub fn tab_ai_memory_index_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "tab_ai_memory_index_path: HOME is not set".to_string())?;
    Ok(std::path::Path::new(&home)
        .join(".scriptkit")
        .join("scripts")
        .join(".tab-ai-memory.json"))
}

fn private_tab_ai_persistence_error(
    code: &str,
    path: &std::path::Path,
    error: &impl std::fmt::Display,
) -> String {
    let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
    let safe_error = crate::logging::log_private_user_value(&error.to_string());
    format!(
        "{code}: path_bytes={} path_sha256={} error_bytes={} error_sha256={}",
        safe_path.raw_bytes, safe_path.sha256, safe_error.raw_bytes, safe_error.sha256,
    )
}

/// Read the Tab AI memory index from an explicit path.
///
/// Returns an empty `Vec` if the index file does not exist.
pub fn read_tab_ai_memory_index_from_path(
    path: &std::path::Path,
) -> Result<Vec<TabAiMemoryEntry>, String> {
    let exists = crate::atomic_file::inspect_private_file(path).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_memory_read_failed", path, &error)
    })?;
    if !exists {
        return Ok(Vec::new());
    }
    let json = crate::atomic_file::read_private_file(path).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_memory_read_failed", path, &error)
    })?;
    serde_json::from_str(&json).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_memory_parse_failed", path, &error)
    })
}

/// Read the Tab AI memory index from the default location.
pub fn read_tab_ai_memory_index() -> Result<Vec<TabAiMemoryEntry>, String> {
    let path = tab_ai_memory_index_path()?;
    read_tab_ai_memory_index_from_path(&path)
}

/// Write a Tab AI memory entry to an explicit path.
///
/// Appends to the existing index (deduplicating by intent + bundle_id),
/// then writes back to disk.  Returns the entry that was written.
pub fn write_tab_ai_memory_entry_to_path(
    record: &TabAiExecutionRecord,
    path: &std::path::Path,
) -> Result<TabAiMemoryEntry, String> {
    let entry = TabAiMemoryEntry {
        schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
        intent: record.intent.clone(),
        generated_source: record.generated_source.clone(),
        slug: record.slug.clone(),
        prompt_type: record.prompt_type.clone(),
        bundle_id: record.bundle_id.clone(),
        written_at: record.executed_at.clone(),
    };

    let mut entries = read_tab_ai_memory_index_from_path(path)?;

    // Deduplicate: remove older entry with same intent + bundle_id
    entries.retain(|existing| {
        !(existing.intent == entry.intent && existing.bundle_id == entry.bundle_id)
    });

    entries.push(entry.clone());

    let json = serde_json::to_string_pretty(&entries).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_memory_serialize_failed", path, &error)
    })?;
    crate::atomic_file::write_private_atomic(path, json.as_bytes()).map_err(|error| {
        private_tab_ai_persistence_error("tab_ai_memory_write_failed", path, &error)
    })?;

    let safe_intent = crate::logging::log_private_user_value(&record.intent);
    let safe_slug = crate::logging::log_private_user_value(&record.slug);
    tracing::info!(
        event = "tab_ai_memory_written",
        intent_bytes = safe_intent.raw_bytes,
        intent_sha256 = %safe_intent.sha256,
        slug_bytes = safe_slug.raw_bytes,
        slug_sha256 = %safe_slug.sha256,
        prompt_type = %record.prompt_type,
    );

    Ok(entry)
}

/// Write a Tab AI memory entry to the default location.
pub fn write_tab_ai_memory_entry(
    record: &TabAiExecutionRecord,
) -> Result<TabAiMemoryEntry, String> {
    let path = tab_ai_memory_index_path()?;
    write_tab_ai_memory_entry_to_path(record, &path)
}

/// Clean up a temporary script file created for Tab AI execution.
///
/// Returns `true` if the file was successfully removed (or already absent),
/// `false` if removal failed.
pub fn cleanup_tab_ai_temp_script(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let safe_path = crate::logging::log_private_user_value(path);
    if !p.exists() {
        tracing::info!(
            event = "tab_ai_temp_cleanup_noop",
            path_bytes = safe_path.raw_bytes,
            path_sha256 = %safe_path.sha256,
            reason = "already_absent",
        );
        return true;
    }
    match std::fs::remove_file(p) {
        Ok(()) => {
            tracing::info!(
                event = "tab_ai_temp_cleanup_success",
                path_bytes = safe_path.raw_bytes,
                path_sha256 = %safe_path.sha256,
            );
            true
        }
        Err(e) => {
            let safe_error = crate::logging::log_private_user_value(&e.to_string());
            tracing::warn!(
                event = "tab_ai_temp_cleanup_failed",
                path_bytes = safe_path.raw_bytes,
                path_sha256 = %safe_path.sha256,
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
            );
            false
        }
    }
}

/// Decide whether to offer "Save as script?" after a successful Tab AI execution.
///
/// Requires at least 3 non-empty lines — trivial one-liners are not worth saving.
pub fn should_offer_save(record: &TabAiExecutionRecord) -> bool {
    let non_empty_line_count = record
        .generated_source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let offer = non_empty_line_count >= 3;
    let safe_slug = crate::logging::log_private_user_value(&record.slug);
    tracing::info!(
        event = "tab_ai_save_offer_decision",
        offer,
        slug_bytes = safe_slug.raw_bytes,
        slug_sha256 = %safe_slug.sha256,
        model_id = %record.model_id,
        provider_id = %record.provider_id,
        source_len = record.generated_source.len(),
        context_warning_count = record.context_warning_count,
    );
    offer
}

// ---------------------------------------------------------------------------
// Tab AI memory suggestion resolver
// ---------------------------------------------------------------------------

/// A suggestion surfaced from the Tab AI memory index for the current intent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiMemorySuggestion {
    pub slug: String,
    pub bundle_id: String,
    pub raw_query: String,
    pub effective_query: String,
    pub prompt_type: String,
    pub written_at: String,
    pub score: f32,
}

/// The reason a memory resolution produced (or failed to produce) suggestions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TabAiMemoryResolutionReason {
    MissingBundleId,
    EmptyQuery,
    ZeroLimit,
    IndexMissing,
    NoCandidatesForBundle,
    BelowThreshold,
    Matched,
}

/// Machine-readable outcome metadata from a memory resolution attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiMemoryResolutionOutcome {
    pub query: String,
    pub normalized_query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub limit: usize,
    pub threshold: f32,
    pub candidate_count: usize,
    pub match_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_slugs: Vec<String>,
    pub reason: TabAiMemoryResolutionReason,
    pub index_path: String,
}

/// Full resolution result: suggestions plus machine-readable outcome metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiMemoryResolution {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<TabAiMemorySuggestion>,
    pub outcome: TabAiMemoryResolutionOutcome,
}

const TAB_AI_MEMORY_SUGGESTION_MIN_SCORE: f32 = 0.35;

fn normalize_tab_ai_match_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut last_was_space = false;

    for ch in input.chars() {
        let ch = if ch == '\u{2192}' { ' ' } else { ch };

        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tab_ai_token_set(input: &str) -> std::collections::BTreeSet<String> {
    normalize_tab_ai_match_text(input)
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn tab_ai_jaccard_similarity(left: &str, right: &str) -> f32 {
    let left_set = tab_ai_token_set(left);
    let right_set = tab_ai_token_set(right);

    if left_set.is_empty() || right_set.is_empty() {
        return 0.0;
    }

    let intersection = left_set.intersection(&right_set).count() as f32;
    let union = left_set.union(&right_set).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn score_tab_ai_memory_candidate(query: &str, entry: &TabAiMemoryEntry) -> f32 {
    let query_norm = normalize_tab_ai_match_text(query);
    let intent_norm = normalize_tab_ai_match_text(&entry.intent);

    if query_norm.is_empty() || intent_norm.is_empty() {
        return 0.0;
    }

    if query_norm == intent_norm {
        return 1.0;
    }

    let overlap = tab_ai_jaccard_similarity(&query_norm, &intent_norm);

    // Small bonus when one normalized phrase contains the other.
    // This keeps "force quit app" and "force quit current app" related.
    let contains_bonus = if intent_norm.contains(&query_norm) || query_norm.contains(&intent_norm) {
        0.20
    } else {
        0.0
    };

    (overlap * 0.80) + contains_bonus
}

/// Emit the structured log event for a memory resolution outcome.
fn log_tab_ai_memory_resolution(outcome: &TabAiMemoryResolutionOutcome) {
    let safe_query = crate::logging::log_private_user_value(&outcome.query);
    let safe_normalized_query = crate::logging::log_private_user_value(&outcome.normalized_query);
    let safe_index_path = crate::logging::log_private_user_value(&outcome.index_path);
    let safe_matched_slugs: Vec<String> = outcome
        .matched_slugs
        .iter()
        .map(|slug| crate::logging::log_private_user_value(slug).sha256)
        .collect();
    tracing::info!(
        event = "tab_ai_memory_resolution",
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        normalized_query_bytes = safe_normalized_query.raw_bytes,
        normalized_query_sha256 = %safe_normalized_query.sha256,
        bundle_id = ?outcome.bundle_id,
        limit = outcome.limit,
        threshold = outcome.threshold,
        candidate_count = outcome.candidate_count,
        match_count = outcome.match_count,
        top_score = ?outcome.top_score,
        reason = ?outcome.reason,
        matched_slugs_sha256 = ?safe_matched_slugs,
        index_path_bytes = safe_index_path.raw_bytes,
        index_path_sha256 = %safe_index_path.sha256,
    );
}

/// Build the initial outcome template shared by all resolution paths.
fn base_resolution_outcome(
    query: &str,
    normalized_query: &str,
    bundle_id: Option<String>,
    limit: usize,
    index_path: &std::path::Path,
) -> TabAiMemoryResolutionOutcome {
    TabAiMemoryResolutionOutcome {
        query: query.to_string(),
        normalized_query: normalized_query.to_string(),
        bundle_id,
        limit,
        threshold: TAB_AI_MEMORY_SUGGESTION_MIN_SCORE,
        candidate_count: 0,
        match_count: 0,
        top_score: None,
        matched_slugs: Vec::new(),
        reason: TabAiMemoryResolutionReason::Matched,
        index_path: index_path.display().to_string(),
    }
}

/// Canonical, outcome-aware resolver for Tab AI memory suggestions.
/// This is the machine-readable surface callers and tests should prefer.
pub fn resolve_tab_ai_memory_suggestions_with_outcome(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
) -> Result<TabAiMemoryResolution, String> {
    resolve_tab_ai_memory_suggestions_with_outcome_from_path(
        raw_query,
        bundle_id,
        limit,
        &tab_ai_memory_index_path()?,
    )
}

/// Outcome-aware resolver against an explicit index path.
pub fn resolve_tab_ai_memory_suggestions_with_outcome_from_path(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
    path: &std::path::Path,
) -> Result<TabAiMemoryResolution, String> {
    let query = raw_query.trim().to_string();
    let normalized_query = normalize_tab_ai_match_text(&query);
    let bundle_id_clean = bundle_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let mut outcome = base_resolution_outcome(
        &query,
        &normalized_query,
        bundle_id_clean.clone(),
        limit,
        path,
    );

    // --- Early-exit branches with explicit reasons ---

    if bundle_id_clean.is_none() {
        outcome.reason = TabAiMemoryResolutionReason::MissingBundleId;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    if query.is_empty() {
        outcome.reason = TabAiMemoryResolutionReason::EmptyQuery;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    if limit == 0 {
        outcome.reason = TabAiMemoryResolutionReason::ZeroLimit;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    if !path.exists() {
        outcome.reason = TabAiMemoryResolutionReason::IndexMissing;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    // --- Read and filter candidates ---

    let bundle_id_norm =
        normalize_tab_ai_match_text(bundle_id_clean.as_deref().unwrap_or_default());

    let bundle_entries: Vec<TabAiMemoryEntry> = read_tab_ai_memory_index_from_path(path)?
        .into_iter()
        .filter(|entry| {
            entry
                .bundle_id
                .as_ref()
                .map(|value| normalize_tab_ai_match_text(value) == bundle_id_norm)
                .unwrap_or(false)
        })
        .collect();

    outcome.candidate_count = bundle_entries.len();

    if bundle_entries.is_empty() {
        outcome.reason = TabAiMemoryResolutionReason::NoCandidatesForBundle;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    // --- Score and rank ---

    let mut matches: Vec<TabAiMemorySuggestion> = bundle_entries
        .into_iter()
        .filter_map(|entry| {
            let score = score_tab_ai_memory_candidate(&query, &entry);
            if score < TAB_AI_MEMORY_SUGGESTION_MIN_SCORE {
                return None;
            }
            Some(TabAiMemorySuggestion {
                slug: entry.slug,
                bundle_id: entry.bundle_id.unwrap_or_default(),
                raw_query: entry.intent.clone(),
                effective_query: entry.intent,
                prompt_type: entry.prompt_type,
                written_at: entry.written_at,
                score,
            })
        })
        .collect();

    matches.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| right.written_at.cmp(&left.written_at))
            .then_with(|| left.slug.cmp(&right.slug))
    });

    if matches.is_empty() {
        outcome.reason = TabAiMemoryResolutionReason::BelowThreshold;
        log_tab_ai_memory_resolution(&outcome);
        return Ok(TabAiMemoryResolution {
            suggestions: Vec::new(),
            outcome,
        });
    }

    matches.truncate(limit);

    outcome.reason = TabAiMemoryResolutionReason::Matched;
    outcome.match_count = matches.len();
    outcome.top_score = matches.first().map(|item| item.score);
    outcome.matched_slugs = matches.iter().map(|item| item.slug.clone()).collect();

    log_tab_ai_memory_resolution(&outcome);

    Ok(TabAiMemoryResolution {
        suggestions: matches,
        outcome,
    })
}

/// Back-compat wrapper: existing callers can keep asking for just the suggestions.
pub fn resolve_tab_ai_memory_suggestions(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    Ok(resolve_tab_ai_memory_suggestions_with_outcome(raw_query, bundle_id, limit)?.suggestions)
}

/// Back-compat wrapper against an explicit path.
pub fn resolve_tab_ai_memory_suggestions_from_path(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
    path: &std::path::Path,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    Ok(
        resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            raw_query, bundle_id, limit, path,
        )?
        .suggestions,
    )
}

// ---------------------------------------------------------------------------
// Tab AI suggested intents — deterministic "next best action" generation
// ---------------------------------------------------------------------------

/// A deterministic, pre-computed intent suggestion surfaced in the Tab AI empty state.
///
/// At most 3 suggestions are returned, preferring app-specific verbs when the
/// focused target has `kind == "app"`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabAiSuggestedIntentSpec {
    pub label: String,
    pub intent: String,
}

impl TabAiSuggestedIntentSpec {
    pub fn new(label: impl Into<String>, intent: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            intent: intent.into(),
        }
    }
}

/// Look up a string field inside the optional `metadata` JSON blob on a target.
fn suggested_intent_metadata_str<'a>(target: &'a TabAiTargetContext, key: &str) -> Option<&'a str> {
    target
        .metadata
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
}

/// Look up an integer field inside the optional `metadata` JSON blob on a target.
fn suggested_intent_metadata_u64(target: &TabAiTargetContext, key: &str) -> Option<u64> {
    target
        .metadata
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_u64())
}

/// Build deterministic suggested intents based on the focused target, clipboard,
/// and prior automations.  Returns at most 3 suggestions and prefers app-specific
/// verbs when `kind == "app"`.
pub fn build_tab_ai_suggested_intents(
    focused_target: Option<&TabAiTargetContext>,
    clipboard: Option<&TabAiClipboardContext>,
    prior_automations: &[TabAiMemorySuggestion],
) -> Vec<TabAiSuggestedIntentSpec> {
    let mut suggestions = Vec::new();

    if let Some(target) = focused_target {
        match target.kind.as_str() {
            "app" => {
                suggestions.push(TabAiSuggestedIntentSpec::new("Focus", "focus on this app"));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Explain",
                    "what does this app do?",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Automate",
                    "create a quick automation for this app",
                ));
            }
            "file" if target.source == "FileSearch" => {
                let query_mode =
                    suggested_intent_metadata_str(target, "queryMode").unwrap_or("spotlight-basic");
                let visible_count =
                    suggested_intent_metadata_u64(target, "visibleResultCount").unwrap_or(0);
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Summarize",
                    "summarize this file in the context of this search",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Related",
                    format!(
                        "what other files in this {} search are most related to this one?",
                        query_mode
                    ),
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Plan",
                    format!(
                        "use this selected file as the primary target and the other {} visible results as supporting context; propose the next edits",
                        visible_count
                    ),
                ));
            }
            "directory" if target.source == "FileSearch" => {
                let query_mode =
                    suggested_intent_metadata_str(target, "queryMode").unwrap_or("path-browse");
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Map",
                    format!(
                        "map this directory and explain what matters in this {} view",
                        query_mode
                    ),
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Batch Rename",
                    "propose a safe batch-rename plan for the currently visible files",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Compare",
                    "group the currently visible results by purpose and tell me what to inspect first",
                ));
            }
            "file" => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Summarize",
                    "summarize this file",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new("Rename", "rename this file"));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Open",
                    "open this file with the right app",
                ));
            }
            "directory" => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Inspect",
                    "what is in this folder?",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Organize",
                    "organize this folder",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Batch Rename",
                    "rename the files in this folder",
                ));
            }
            "window" => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Focus",
                    "focus on this window",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new("Tile", "tile this window"));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Explain",
                    "what is this window for?",
                ));
            }
            _ => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Act on Selection",
                    "do something useful with what is selected",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Explain Selection",
                    "what is currently selected?",
                ));
            }
        }
    } else if let Some(clipboard) = clipboard {
        match clipboard.content_type.as_str() {
            "image" => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Extract Text",
                    "extract the text from this image",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Describe",
                    "describe this image",
                ));
            }
            _ => {
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Transform",
                    "transform this clipboard text",
                ));
                suggestions.push(TabAiSuggestedIntentSpec::new(
                    "Summarize",
                    "summarize this clipboard text",
                ));
            }
        }
    } else {
        suggestions.push(TabAiSuggestedIntentSpec::new(
            "What Can I Do?",
            "what can I do with what is currently selected?",
        ));
        suggestions.push(TabAiSuggestedIntentSpec::new(
            "Automate Here",
            "create a quick automation for the current surface",
        ));
    }

    if let Some(memory) = prior_automations.first() {
        suggestions.push(TabAiSuggestedIntentSpec::new(
            format!("Repeat {}", memory.slug),
            memory.effective_query.clone(),
        ));
    }

    suggestions.truncate(3);
    suggestions
}

// ---------------------------------------------------------------------------
// Tab AI recent automations by bundle — most-recent-first lookup
// ---------------------------------------------------------------------------

/// Return recent Tab AI automations matching a bundle ID, most-recent first.
///
/// Uses the default memory index path.
pub fn recent_tab_ai_automations_for_bundle(
    bundle_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    recent_tab_ai_automations_for_bundle_from_path(bundle_id, limit, &tab_ai_memory_index_path()?)
}

/// Return recent Tab AI automations matching a bundle ID from an explicit path.
///
/// Returns most-recent-first, capped to `limit`.  Does not change the existing
/// memory schema — reads `TabAiMemoryEntry` and converts to `TabAiMemorySuggestion`.
pub fn recent_tab_ai_automations_for_bundle_from_path(
    bundle_id: Option<&str>,
    limit: usize,
    path: &std::path::Path,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    let bundle_id_norm = normalize_tab_ai_match_text(bundle_id.unwrap_or_default());
    if bundle_id_norm.is_empty() || limit == 0 || !path.exists() {
        return Ok(Vec::new());
    }

    let mut suggestions: Vec<TabAiMemorySuggestion> = read_tab_ai_memory_index_from_path(path)?
        .into_iter()
        .filter(|entry| {
            entry
                .bundle_id
                .as_ref()
                .map(|value| normalize_tab_ai_match_text(value) == bundle_id_norm)
                .unwrap_or(false)
        })
        .map(|entry| TabAiMemorySuggestion {
            slug: entry.slug,
            bundle_id: entry.bundle_id.unwrap_or_default(),
            raw_query: entry.intent.clone(),
            effective_query: entry.intent,
            prompt_type: entry.prompt_type,
            written_at: entry.written_at,
            score: 1.0,
        })
        .collect();

    suggestions.sort_by(|left, right| {
        right
            .written_at
            .cmp(&left.written_at)
            .then_with(|| left.slug.cmp(&right.slug))
    });

    suggestions.truncate(limit);
    Ok(suggestions)
}

// ---------------------------------------------------------------------------
// Tab AI entry-aware prior automation resolution
// ---------------------------------------------------------------------------

/// Resolve prior automations for a Tab AI entry.  When `raw_query` is empty
/// (zero-intent open), falls back to `recent_tab_ai_automations_for_bundle`
/// so the harness always receives bundle-matched suggestions even before the
/// user types anything.
pub fn resolve_tab_ai_prior_automations_for_entry(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    resolve_tab_ai_prior_automations_for_entry_from_path(
        raw_query,
        bundle_id,
        limit,
        &tab_ai_memory_index_path()?,
    )
}

/// Path-parameterized variant for testability.
pub fn resolve_tab_ai_prior_automations_for_entry_from_path(
    raw_query: &str,
    bundle_id: Option<&str>,
    limit: usize,
    path: &std::path::Path,
) -> Result<Vec<TabAiMemorySuggestion>, String> {
    let query = raw_query.trim();
    if query.is_empty() {
        return recent_tab_ai_automations_for_bundle_from_path(bundle_id, limit, path);
    }
    resolve_tab_ai_memory_suggestions_from_path(query, bundle_id, limit, path)
}

// ---------------------------------------------------------------------------
// Tab AI invocation receipt — machine-readable richness/degradation signal
// ---------------------------------------------------------------------------

/// Schema version for `TabAiInvocationReceipt`. Bump when adding/removing/renaming fields.
pub const TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Tri-state field status used in invocation receipts.
///
/// - `Captured` — data was successfully extracted from the surface.
/// - `Degraded` — the surface structurally supports the data but it could not
///   be extracted (e.g. panel-only element collection, terminal input).
/// - `Unavailable` — the surface has no concept of this data (e.g. webcam has
///   no input text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabAiFieldStatus {
    Captured,
    Degraded,
    Unavailable,
}

impl std::fmt::Display for TabAiFieldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Captured => f.write_str("captured"),
            Self::Degraded => f.write_str("degraded"),
            Self::Unavailable => f.write_str("unavailable"),
        }
    }
}

/// Stable, machine-readable reason code explaining why a field is degraded or
/// unavailable.  These are enumerated so downstream consumers (tests, agents,
/// dashboards) can match on them without parsing free-form strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabAiDegradationReason {
    /// `collect_visible_elements` returned only `panel:*` placeholders.
    PanelOnlyElements,
    /// `collect_visible_elements` used the `current_view` fallback collector
    /// instead of a view-specific one.
    CollectorFallback,
    /// `collect_visible_elements` returned zero elements and no warnings.
    NoSemanticElements,
    /// No focused or selected semantic ID was found.
    MissingFocusTarget,
    /// `current_input_text()` returned `None` on a surface that structurally
    /// supports input (e.g. terminal where content exists but is not
    /// user-typed text).
    InputNotExtractable,
    /// The surface has no user-editable text concept at all (e.g. webcam,
    /// drop zone).
    InputNotApplicable,
}

impl std::fmt::Display for TabAiDegradationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PanelOnlyElements => f.write_str("panel_only_elements"),
            Self::CollectorFallback => f.write_str("collector_fallback"),
            Self::NoSemanticElements => f.write_str("no_semantic_elements"),
            Self::MissingFocusTarget => f.write_str("missing_focus_target"),
            Self::InputNotExtractable => f.write_str("input_not_extractable"),
            Self::InputNotApplicable => f.write_str("input_not_applicable"),
        }
    }
}

/// Machine-readable receipt emitted on every Tab AI invocation.
///
/// Identifies the prompt/view type and whether UI context was rich or
/// degraded, with explicit reasons for each degradation.  Designed to be
/// inspectable in tests and parseable from structured logs without human
/// interpretation of free-form strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TabAiInvocationReceipt {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// `AppView` variant name at invocation time.
    pub prompt_type: String,
    /// Tri-state status for input text extraction.
    pub input_status: TabAiFieldStatus,
    /// Tri-state status for focus/selection target.
    pub focus_status: TabAiFieldStatus,
    /// Tri-state status for semantic element collection.
    pub elements_status: TabAiFieldStatus,
    /// Number of semantic elements collected.
    pub element_count: usize,
    /// Number of element-collection warnings.
    pub warning_count: usize,
    /// Whether any focused or selected semantic ID was captured.
    pub has_focus_target: bool,
    /// Whether input text was captured.
    pub has_input_text: bool,
    /// Machine-readable reason codes for any degraded or unavailable fields.
    /// Empty when all fields are `Captured`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degradation_reasons: Vec<TabAiDegradationReason>,
    /// Overall richness: `true` when all three statuses are `Captured`.
    pub rich: bool,
}

/// Classifies how a surface treats its input field for receipt purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabAiInputSemantics {
    /// Empty input is valid (e.g. ScriptList with no filter typed yet).
    CapturableEvenWhenEmpty,
    /// Input exists structurally but `current_input_text()` may return `None`
    /// for non-user-typed content (e.g. terminal buffer).
    DegradedWhenMissing,
    /// Surface has no user-editable text concept at all.
    NotApplicable,
}

/// Classify a prompt type's input semantics.
///
/// Names must match what `app_view_name()` returns at runtime.
fn tab_ai_input_semantics(prompt_type: &str) -> TabAiInputSemantics {
    match prompt_type {
        "DivPrompt" | "DropPrompt" | "Webcam" | "CreationFeedback" | "ActionsDialog"
        | "Settings" | "InstalledKits" => TabAiInputSemantics::NotApplicable,
        "FormPrompt" | "TermPrompt" | "QuickTerminal" => TabAiInputSemantics::DegradedWhenMissing,
        _ => TabAiInputSemantics::CapturableEvenWhenEmpty,
    }
}

/// Returns `true` when any warning starts with `panel_only_`.
fn has_panel_only_warning(warnings: &[String]) -> bool {
    warnings
        .iter()
        .any(|warning| warning.starts_with("panel_only_"))
}

/// Returns `true` when warnings include `collector_used_current_view_fallback`.
fn has_collector_fallback_warning(warnings: &[String]) -> bool {
    warnings
        .iter()
        .any(|warning| warning == "collector_used_current_view_fallback")
}

impl TabAiInvocationReceipt {
    /// Build a receipt from snapshot extraction results.
    ///
    /// `input_text` is the value from `current_input_text()` — `None` means
    /// the extractor returned nothing (which is valid on surfaces where empty
    /// input is the default state).  `warnings` are from
    /// `ElementCollectionOutcome`.
    pub fn from_snapshot(
        prompt_type: &str,
        input_text: &Option<String>,
        focused_id: &Option<String>,
        selected_id: &Option<String>,
        element_count: usize,
        warnings: &[String],
    ) -> Self {
        let input_was_extracted = input_text.is_some();
        let has_input_text = input_text
            .as_ref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false);

        // --- input_status ---
        let input_status = match tab_ai_input_semantics(prompt_type) {
            TabAiInputSemantics::CapturableEvenWhenEmpty => TabAiFieldStatus::Captured,
            TabAiInputSemantics::DegradedWhenMissing => {
                if input_was_extracted {
                    TabAiFieldStatus::Captured
                } else {
                    TabAiFieldStatus::Degraded
                }
            }
            TabAiInputSemantics::NotApplicable => TabAiFieldStatus::Unavailable,
        };

        // --- elements_status ---
        let has_focus_target = focused_id.is_some() || selected_id.is_some();
        let has_panel_only = has_panel_only_warning(warnings);
        let has_collector_fallback = has_collector_fallback_warning(warnings);
        let degraded_elements = has_panel_only || has_collector_fallback;

        // Warnings win over element_count==0: a fallback or panel-only surface
        // is degraded (structurally supports elements but couldn't fully
        // extract), not unavailable.
        let elements_status = if degraded_elements {
            TabAiFieldStatus::Degraded
        } else if element_count == 0 {
            TabAiFieldStatus::Unavailable
        } else {
            TabAiFieldStatus::Captured
        };

        // --- focus_status ---
        let focus_status = if has_focus_target {
            TabAiFieldStatus::Captured
        } else if degraded_elements {
            TabAiFieldStatus::Degraded
        } else {
            TabAiFieldStatus::Unavailable
        };

        // --- degradation_reasons ---
        let mut degradation_reasons = Vec::new();
        if has_panel_only {
            degradation_reasons.push(TabAiDegradationReason::PanelOnlyElements);
        }
        if has_collector_fallback {
            degradation_reasons.push(TabAiDegradationReason::CollectorFallback);
        }
        if element_count == 0 {
            degradation_reasons.push(TabAiDegradationReason::NoSemanticElements);
        }
        if !has_focus_target && focus_status == TabAiFieldStatus::Degraded {
            degradation_reasons.push(TabAiDegradationReason::MissingFocusTarget);
        }
        match input_status {
            TabAiFieldStatus::Degraded => {
                degradation_reasons.push(TabAiDegradationReason::InputNotExtractable);
            }
            TabAiFieldStatus::Unavailable => {
                degradation_reasons.push(TabAiDegradationReason::InputNotApplicable);
            }
            TabAiFieldStatus::Captured => {}
        }

        let rich = input_status == TabAiFieldStatus::Captured
            && focus_status == TabAiFieldStatus::Captured
            && elements_status == TabAiFieldStatus::Captured;

        Self {
            schema_version: TAB_AI_INVOCATION_RECEIPT_SCHEMA_VERSION,
            prompt_type: prompt_type.to_string(),
            input_status,
            focus_status,
            elements_status,
            element_count,
            warning_count: warnings.len(),
            has_focus_target,
            has_input_text,
            degradation_reasons,
            rich,
        }
    }
}

#[cfg(test)]
mod execution_record_compat_tests {
    use super::*;

    #[test]
    fn legacy_v1_execution_record_fixture_still_deserializes() {
        let json = std::fs::read_to_string("tests/fixtures/tab_ai_execution_record_v1.json")
            .expect("missing tests/fixtures/tab_ai_execution_record_v1.json");
        let record: TabAiExecutionRecord =
            serde_json::from_str(&json).expect("legacy v1 record should deserialize");
        assert!(!record.intent.is_empty());
        assert!(!record.generated_source.is_empty());
        assert_eq!(record.context_warning_count, 0);
        assert!(
            record.model_id.is_empty(),
            "v1 had no model_id — default should be empty string"
        );
        assert!(
            record.provider_id.is_empty(),
            "v1 had no provider_id — default should be empty string"
        );

        tracing::info!(
            event = "execution_record_compat_test_passed",
            schema_version = record.schema_version,
            intent = %record.intent,
            context_warning_count = record.context_warning_count,
        );
    }

    #[test]
    fn v2_record_with_all_fields_still_deserializes() {
        let json = r#"{
            "schemaVersion": 2,
            "intent": "open browser",
            "generatedSource": "line1\nline2\nline3",
            "tempScriptPath": "/tmp/test.ts",
            "slug": "open-browser",
            "promptType": "ScriptList",
            "modelId": "gpt-4.1",
            "providerId": "vercel",
            "contextWarningCount": 2,
            "executedAt": "2026-03-28T00:00:00Z"
        }"#;
        let record: TabAiExecutionRecord =
            serde_json::from_str(json).expect("v2 record should deserialize");
        assert_eq!(record.schema_version, 2);
        assert_eq!(record.model_id, "gpt-4.1");
        assert_eq!(record.provider_id, "vercel");
        assert_eq!(record.context_warning_count, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_ai_context_blob_default_roundtrip() {
        let blob = TabAiContextBlob {
            schema_version: TAB_AI_CONTEXT_SCHEMA_VERSION,
            timestamp: "2026-03-28T00:00:00Z".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&blob).expect("serialize");
        let parsed: TabAiContextBlob = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(parsed.timestamp, "2026-03-28T00:00:00Z");
    }

    #[test]
    fn tab_ai_ui_snapshot_skips_empty_fields() {
        let snap = TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&snap).expect("serialize");
        // Empty optional fields should be omitted
        assert!(!json.contains("inputText"));
        assert!(!json.contains("focusedSemanticId"));
        assert!(!json.contains("visibleElements"));
    }

    #[test]
    fn tab_ai_context_blob_from_parts_deterministic() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ArgPrompt".to_string(),
            input_text: Some("Slack".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            selected_semantic_id: Some("choice:0:slack".to_string()),
            visible_elements: vec![crate::protocol::ElementInfo::product_static_choice(
                0, "Slack", "slack", true,
            )],
        };
        let desktop = crate::context_snapshot::AiContextSnapshot {
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                name: "Slack".to_string(),
                bundle_id: "com.tinyspeck.slackmacgap".to_string(),
                pid: 1234,
            }),
            ..Default::default()
        };
        let recent_inputs = vec!["copy url".to_string(), "open finder".to_string()];
        let ts = "2026-03-28T12:00:00Z".to_string();

        let blob = TabAiContextBlob::from_parts(
            ui,
            desktop,
            recent_inputs,
            None,
            vec![],
            vec![],
            ts.clone(),
        );

        assert_eq!(blob.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(blob.timestamp, ts);
        assert_eq!(blob.ui.prompt_type, "ArgPrompt");
        assert_eq!(blob.ui.input_text.as_deref(), Some("Slack"));
        assert_eq!(blob.ui.visible_elements.len(), 1);
        assert_eq!(
            blob.desktop.frontmost_app.as_ref().map(|a| a.name.as_str()),
            Some("Slack")
        );
        assert_eq!(blob.recent_inputs.len(), 2);
        assert!(blob.clipboard.is_none());
    }

    #[test]
    fn tab_ai_context_blob_camel_case_json_fields() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ScriptList".to_string(),
            input_text: Some("test".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            selected_semantic_id: None,
            visible_elements: vec![],
        };
        let blob = TabAiContextBlob::from_parts(
            ui,
            Default::default(),
            vec!["recent".to_string()],
            Some(TabAiClipboardContext {
                content_type: "text".to_string(),
                preview: "clipboard text".to_string(),
                ocr_text: None,
            }),
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");

        // Verify camelCase field names in JSON output
        assert!(json.contains("schemaVersion"));
        assert!(json.contains("promptType"));
        assert!(json.contains("inputText"));
        assert!(json.contains("focusedSemanticId"));
        assert!(json.contains("recentInputs"));
        assert!(json.contains("contentType"));

        // Verify snake_case is NOT present
        assert!(!json.contains("schema_version"));
        assert!(!json.contains("prompt_type"));
        assert!(!json.contains("input_text"));
        assert!(!json.contains("recent_inputs"));
        assert!(!json.contains("content_type"));
    }

    #[test]
    fn tab_ai_context_blob_json_roundtrip_with_all_fields() {
        let ui = TabAiUiSnapshot {
            prompt_type: "ClipboardHistory".to_string(),
            input_text: Some("search term".to_string()),
            focused_semantic_id: Some("choice:2:item".to_string()),
            selected_semantic_id: Some("choice:2:item".to_string()),
            visible_elements: vec![
                crate::protocol::ElementInfo::input("filter", Some("search term"), true),
                crate::protocol::ElementInfo::product_static_choice(0, "Item A", "a", false),
                crate::protocol::ElementInfo::product_static_choice(1, "Item B", "b", false),
                crate::protocol::ElementInfo::product_static_choice(2, "Item C", "item", true),
            ],
        };
        let desktop = crate::context_snapshot::AiContextSnapshot {
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                name: "Chrome".to_string(),
                bundle_id: "com.google.Chrome".to_string(),
                pid: 5678,
            }),
            selected_text: Some("selected words".to_string()),
            browser: Some(crate::context_snapshot::BrowserContext::from_url(
                "https://example.com".to_string(),
            )),
            ..Default::default()
        };
        let blob = TabAiContextBlob::from_parts(
            ui,
            desktop,
            vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()],
            Some(TabAiClipboardContext {
                content_type: "text".to_string(),
                preview: "clipboard preview".to_string(),
                ocr_text: None,
            }),
            vec![],
            vec![],
            "2026-03-28T18:30:00Z".to_string(),
        );

        let json = serde_json::to_string_pretty(&blob).expect("serialize");
        let parsed: TabAiContextBlob = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.schema_version, TAB_AI_CONTEXT_SCHEMA_VERSION);
        assert_eq!(parsed.ui.prompt_type, "ClipboardHistory");
        assert_eq!(parsed.ui.visible_elements.len(), 4);
        assert_eq!(
            parsed.desktop.selected_text.as_deref(),
            Some("selected words")
        );
        assert_eq!(
            parsed.desktop.browser.as_ref().map(|b| b.url.as_str()),
            Some("https://example.com")
        );
        assert_eq!(parsed.recent_inputs.len(), 3);
        assert_eq!(
            parsed.clipboard.as_ref().map(|c| c.preview.as_str()),
            Some("clipboard preview")
        );
    }

    #[test]
    fn tab_ai_context_schema_version_is_three() {
        assert_eq!(TAB_AI_CONTEXT_SCHEMA_VERSION, 3);
    }

    #[test]
    fn tab_ai_context_blob_omits_empty_optional_fields() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                ..Default::default()
            },
            Default::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");
        assert!(json.contains("\"schemaVersion\":3"));
        assert!(
            !json.contains("recentInputs"),
            "empty Vec should be omitted"
        );
        assert!(!json.contains("clipboard"), "None should be omitted");
        assert!(
            !json.contains("priorAutomations"),
            "empty Vec should be omitted"
        );
    }

    #[test]
    fn tab_ai_user_prompt_preserves_multiline_intent_and_contract() {
        let prompt = build_tab_ai_user_prompt(
            "rename selection\nthen copy it",
            r#"{"ui":{"promptType":"ScriptList"}}"#,
        );
        assert!(prompt.contains("User intent:\nrename selection\nthen copy it"));
        assert!(prompt.contains("Context JSON:"));
        assert!(prompt.contains(r#"{"ui":{"promptType":"ScriptList"}}"#));
        assert!(prompt.contains("Script Kit TypeScript"));
        assert!(prompt.contains("fenced ```ts block"));
    }

    // --- TabAiExecutionRecord tests ---

    fn sample_execution_record() -> TabAiExecutionRecord {
        TabAiExecutionRecord::from_parts(
            "force quit Slack".to_string(),
            "import '@anthropic-ai/sdk';\nawait exec('kill Slack');\nconsole.log('done');"
                .to_string(),
            "/tmp/scriptlet-abc123.ts".to_string(),
            "force-quit-slack".to_string(),
            "AppLauncher".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T12:00:00Z".to_string(),
        )
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_stores_memory_atomically_and_repairs_legacy_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private Tab AI memory fixture");
        let path = directory.path().join("private-tab-ai-memory.json");
        let record = sample_execution_record();
        let written = write_tab_ai_memory_entry_to_path(&record, &path)
            .expect("private owner-only Tab AI memory write");
        assert_eq!(written.intent, record.intent);
        assert_eq!(written.generated_source, record.generated_source);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let entries = read_tab_ai_memory_index_from_path(&path)
            .expect("legacy Tab AI memory permissions repair before read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].generated_source, record.generated_source);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_repairs_receipt_boundary_and_owner_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("isolated private Tab AI receipt fixture");
        let path = directory.path().join("private-tab-ai-executions.jsonl");
        let record = sample_execution_record();
        let first = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        std::fs::write(&path, serde_json::to_vec(&first).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let second = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );

        append_tab_ai_execution_receipt_to_path(&second, &path)
            .expect("append safe receipt after a legacy unterminated line");
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<TabAiExecutionReceipt>(lines[1])
                .unwrap()
                .status,
            TabAiExecutionStatus::Succeeded
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_tab_ai_persistence_refuses_symlinks_without_exposing_private_paths() {
        let directory = tempfile::tempdir().expect("isolated private Tab AI symlink fixture");
        let external = directory.path().join("unrelated-private-data.json");
        let planted = directory
            .path()
            .join("private-cancer-treatment-memory.json");
        std::fs::write(&external, "preserve unrelated private data").unwrap();
        std::os::unix::fs::symlink(&external, &planted).unwrap();
        let record = sample_execution_record();
        let receipt = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );

        let read_error = read_tab_ai_memory_index_from_path(&planted)
            .expect_err("Tab AI memory must not read a planted symlink");
        assert!(!read_error.contains("private-cancer-treatment"));
        let write_error = write_tab_ai_memory_entry_to_path(&record, &planted)
            .expect_err("Tab AI memory must not write through a planted symlink");
        assert!(!write_error.contains("private-cancer-treatment"));
        let audit_error = append_tab_ai_execution_receipt_to_path(&receipt, &planted)
            .expect_err("Tab AI receipts must not append through a planted symlink");
        assert!(!audit_error.contains("private-cancer-treatment"));
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            "preserve unrelated private data"
        );
        assert!(std::fs::symlink_metadata(&planted)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn tab_ai_execution_record_from_parts_sets_schema_version() {
        let record = sample_execution_record();
        assert_eq!(
            record.schema_version,
            TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION
        );
        assert_eq!(record.intent, "force quit Slack");
        assert_eq!(record.slug, "force-quit-slack");
        assert_eq!(record.prompt_type, "AppLauncher");
    }

    #[test]
    fn tab_ai_execution_record_serde_roundtrip() {
        let record = sample_execution_record();
        let json = serde_json::to_string(&record).expect("serialize");
        let parsed: TabAiExecutionRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.schema_version, record.schema_version);
        assert_eq!(parsed.intent, record.intent);
        assert_eq!(parsed.slug, record.slug);
        assert_eq!(parsed.bundle_id, record.bundle_id);
    }

    #[test]
    fn tab_ai_execution_record_omits_none_bundle_id() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "code".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&record).expect("serialize");
        assert!(!json.contains("bundleId"));
    }

    #[test]
    fn should_offer_save_returns_true_for_three_plus_lines() {
        let record = sample_execution_record();
        // sample has 3 non-empty lines
        assert!(should_offer_save(&record));
    }

    #[test]
    fn should_offer_save_returns_false_for_fewer_than_three_lines() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "one\ntwo".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(!should_offer_save(&record));
    }

    #[test]
    fn should_offer_save_returns_false_for_empty_source() {
        let record = TabAiExecutionRecord::from_parts(
            "test".to_string(),
            "   ".to_string(),
            "/tmp/x.ts".to_string(),
            "test".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(!should_offer_save(&record));
    }

    // --- TabAiExecutionReceipt tests ---

    #[test]
    fn append_tab_ai_execution_receipt_writes_one_json_line() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join(".tab-ai-executions.jsonl");

        let record = sample_execution_record();
        let receipt = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&receipt, &path).expect("append");

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one line per receipt");

        let parsed: TabAiExecutionReceipt = serde_json::from_str(lines[0]).expect("valid JSON");
        assert_eq!(parsed.status, TabAiExecutionStatus::Dispatched);
        assert_eq!(parsed.slug, "force-quit-slack");
        assert_eq!(parsed.model_id, "gpt-4.1");
        assert_eq!(parsed.provider_id, "vercel");
        assert!(!parsed.save_offer_eligible);
        assert!(!parsed.memory_write_eligible);

        // camelCase check
        assert!(lines[0].contains("modelId"));
        assert!(lines[0].contains("providerId"));
        assert!(lines[0].contains("contextWarningCount"));
        assert!(lines[0].contains("saveOfferEligible"));
        assert!(!lines[0].contains("model_id"));
        assert!(!lines[0].contains("provider_id"));
    }

    #[test]
    fn append_receipt_is_append_only() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join(".tab-ai-executions.jsonl");

        let record = sample_execution_record();

        let r1 = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&r1, &path).expect("append 1");

        let r2 = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );
        append_tab_ai_execution_receipt_to_path(&r2, &path).expect("append 2");

        let content = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2, "two receipts = two lines");

        let p1: TabAiExecutionReceipt = serde_json::from_str(lines[0]).expect("parse line 1");
        let p2: TabAiExecutionReceipt = serde_json::from_str(lines[1]).expect("parse line 2");
        assert_eq!(p1.status, TabAiExecutionStatus::Dispatched);
        assert_eq!(p2.status, TabAiExecutionStatus::Succeeded);
        assert!(p2.save_offer_eligible);
        assert!(p2.memory_write_eligible);
    }

    #[test]
    fn build_receipt_sets_eligibility_based_on_status() {
        let record = sample_execution_record();

        let dispatched = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Dispatched,
            false,
            false,
            None,
        );
        assert!(!dispatched.memory_write_eligible);
        assert!(!dispatched.save_offer_eligible);

        let succeeded = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Succeeded,
            true,
            true,
            None,
        );
        assert!(succeeded.memory_write_eligible);
        assert!(succeeded.save_offer_eligible);

        let failed = build_tab_ai_execution_receipt(
            &record,
            TabAiExecutionStatus::Failed,
            true,
            true,
            Some("exit code 1".to_string()),
        );
        assert!(!failed.memory_write_eligible);
        assert!(!failed.save_offer_eligible);
        assert_eq!(failed.error.as_deref(), Some("exit code 1"));
    }

    #[test]
    fn cleanup_tab_ai_temp_script_returns_true_for_absent_file() {
        assert!(cleanup_tab_ai_temp_script(
            "/tmp/nonexistent-tab-ai-test-12345.ts"
        ));
    }

    #[test]
    fn cleanup_tab_ai_temp_script_removes_existing_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("tab-ai-test-cleanup.ts");
        std::fs::write(&path, "console.log('cleanup test')").expect("write test file");
        assert!(path.exists());
        assert!(cleanup_tab_ai_temp_script(path.to_str().expect("utf8")));
        assert!(!path.exists());
    }

    #[test]
    fn tab_ai_memory_entry_serde_roundtrip() {
        let entry = TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "copy url".to_string(),
            generated_source: "await copy(browser.url)".to_string(),
            slug: "copy-url".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.google.Chrome".to_string()),
            written_at: "2026-03-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: TabAiMemoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, entry);
    }

    #[test]
    fn tab_ai_memory_entry_omits_none_bundle_id() {
        let entry = TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "test".to_string(),
            generated_source: "code".to_string(),
            slug: "test".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: None,
            written_at: "2026-03-28T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(!json.contains("bundleId"));
    }
}

#[cfg(test)]
mod tab_ai_memory_suggestion_tests {
    use super::*;

    fn memory_entry(
        intent: &str,
        bundle_id: Option<&str>,
        slug: &str,
        written_at: &str,
    ) -> TabAiMemoryEntry {
        TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: intent.to_string(),
            generated_source: "import \"@scriptkit/sdk\";\nawait hide();\n".to_string(),
            slug: slug.to_string(),
            prompt_type: "AppLauncher".to_string(),
            bundle_id: bundle_id.map(str::to_string),
            written_at: written_at.to_string(),
        }
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_returns_similar_non_exact_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![memory_entry(
            "force quit current app",
            Some("com.apple.Safari"),
            "force-quit-current-app",
            "2026-03-28T00:00:00Z",
        )];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "force quit app",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].slug, "force-quit-current-app");
        assert_eq!(results[0].effective_query, "force quit current app");
        assert!(results[0].score >= TAB_AI_MEMORY_SUGGESTION_MIN_SCORE);
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_filters_by_bundle_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![
            memory_entry(
                "copy browser url",
                Some("com.apple.Safari"),
                "copy-browser-url",
                "2026-03-28T00:00:00Z",
            ),
            memory_entry(
                "copy browser url",
                Some("com.tinyspeck.slackmacgap"),
                "copy-browser-url-slack",
                "2026-03-28T00:00:01Z",
            ),
        ];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "copy url",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].slug, "copy-browser-url");
        assert_eq!(results[0].bundle_id, "com.apple.Safari");
    }

    #[test]
    fn resolve_tab_ai_memory_suggestions_prefers_exact_match_then_recency() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".tab-ai-memory.json");
        let entries = vec![
            memory_entry(
                "force quit current app",
                Some("com.apple.Safari"),
                "older-similar",
                "2026-03-28T00:00:00Z",
            ),
            memory_entry(
                "force quit app",
                Some("com.apple.Safari"),
                "exact-match",
                "2026-03-28T00:00:01Z",
            ),
        ];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let results = resolve_tab_ai_memory_suggestions_from_path(
            "force quit app",
            Some("com.apple.Safari"),
            3,
            &path,
        )
        .expect("resolve suggestions");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].slug, "exact-match");
        assert!(results[0].score >= results[1].score);
    }
}

#[cfg(test)]
mod tab_ai_memory_resolution_tests {
    use super::*;

    #[test]
    fn tab_ai_memory_resolution_reports_missing_bundle_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit slack",
            None,
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::MissingBundleId
        );
        assert_eq!(resolution.outcome.candidate_count, 0);
        assert_eq!(resolution.outcome.match_count, 0);
        assert!(resolution.outcome.top_score.is_none());
        assert!(resolution.outcome.matched_slugs.is_empty());
    }

    #[test]
    fn tab_ai_memory_resolution_reports_empty_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "   ",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::EmptyQuery
        );
    }

    #[test]
    fn tab_ai_memory_resolution_reports_zero_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit",
            Some("com.tinyspeck.slackmacgap"),
            0,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::ZeroLimit
        );
    }

    #[test]
    fn tab_ai_memory_resolution_reports_index_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.json");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit slack",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::IndexMissing
        );
        assert!(resolution.outcome.index_path.contains("missing.json"));
    }

    #[test]
    fn tab_ai_memory_resolution_reports_no_candidates_for_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");
        let entries = vec![TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "force quit".to_string(),
            generated_source: "import \"@scriptkit/sdk\";\n".to_string(),
            slug: "force-quit".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.apple.Safari".to_string()),
            written_at: "2026-03-28T00:00:00Z".to_string(),
        }];
        std::fs::write(&path, serde_json::to_string_pretty(&entries).expect("ser")).expect("write");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert!(resolution.suggestions.is_empty());
        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::NoCandidatesForBundle
        );
        assert_eq!(resolution.outcome.candidate_count, 0);
    }

    #[test]
    fn tab_ai_memory_resolution_prefers_recent_high_score_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        // Both intents share enough tokens with the query "force quit app"
        // to score above the 0.35 threshold.
        let older = TabAiExecutionRecord::from_parts(
            "force quit current app".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"old\");\n".to_string(),
            "/tmp/old.ts".to_string(),
            "force-quit-old".to_string(),
            "ScriptList".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let newer = TabAiExecutionRecord::from_parts(
            "force quit app".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"new\");\n".to_string(),
            "/tmp/new.ts".to_string(),
            "force-quit-new".to_string(),
            "ScriptList".to_string(),
            Some("com.tinyspeck.slackmacgap".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T01:00:00Z".to_string(),
        );

        write_tab_ai_memory_entry_to_path(&older, &path).expect("write older");
        write_tab_ai_memory_entry_to_path(&newer, &path).expect("write newer");

        let resolution = resolve_tab_ai_memory_suggestions_with_outcome_from_path(
            "force quit app",
            Some("com.tinyspeck.slackmacgap"),
            3,
            &path,
        )
        .expect("resolve");

        assert_eq!(
            resolution.outcome.reason,
            TabAiMemoryResolutionReason::Matched
        );
        assert_eq!(resolution.outcome.match_count, 2);
        assert_eq!(resolution.outcome.top_score, Some(1.0));
        // Exact match "force quit app" scores 1.0, should be first
        assert_eq!(
            resolution.suggestions.first().map(|s| s.slug.as_str()),
            Some("force-quit-new")
        );
        assert_eq!(resolution.outcome.candidate_count, 2);
        assert!(!resolution.outcome.matched_slugs.is_empty());
    }

    #[test]
    fn tab_ai_memory_write_dedupes_same_intent_and_bundle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let first = TabAiExecutionRecord::from_parts(
            "copy url".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"a\");\n".to_string(),
            "/tmp/one.ts".to_string(),
            "copy-url-one".to_string(),
            "ScriptList".to_string(),
            Some("com.google.Chrome".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let second = TabAiExecutionRecord::from_parts(
            "copy url".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"b\");\n".to_string(),
            "/tmp/two.ts".to_string(),
            "copy-url-two".to_string(),
            "ScriptList".to_string(),
            Some("com.google.Chrome".to_string()),
            "model-a".to_string(),
            "provider-a".to_string(),
            0,
            "2026-03-28T01:00:00Z".to_string(),
        );

        write_tab_ai_memory_entry_to_path(&first, &path).expect("write first");
        write_tab_ai_memory_entry_to_path(&second, &path).expect("write second");

        let entries = read_tab_ai_memory_index_from_path(&path).expect("read");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].slug, "copy-url-two");
    }
}

#[cfg(test)]
mod tab_ai_entry_resolution_tests {
    use super::*;

    #[test]
    fn empty_entry_query_uses_recent_bundle_automations() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "rename this file".to_string(),
                generated_source: "code".to_string(),
                slug: "older".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T10:00:00Z".to_string(),
            },
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "summarize this file".to_string(),
                generated_source: "code".to_string(),
                slug: "newer".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T11:00:00Z".to_string(),
            },
        ];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "",
            Some("com.apple.finder"),
            2,
            &path,
        )
        .expect("resolve");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].slug, "newer");
        assert_eq!(items[1].slug, "older");
    }

    #[test]
    fn non_empty_entry_query_uses_fuzzy_matching() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "rename this file".to_string(),
                generated_source: "code".to_string(),
                slug: "rename-entry".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T10:00:00Z".to_string(),
            },
            TabAiMemoryEntry {
                schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
                intent: "summarize this file".to_string(),
                generated_source: "code".to_string(),
                slug: "summarize-entry".to_string(),
                prompt_type: "FileSearch".to_string(),
                bundle_id: Some("com.apple.finder".to_string()),
                written_at: "2026-03-29T11:00:00Z".to_string(),
            },
        ];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        // Non-empty query should use fuzzy matching, not recent-bundle fallback
        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "rename",
            Some("com.apple.finder"),
            2,
            &path,
        )
        .expect("resolve");

        // Should match the rename entry via query matching
        assert!(
            items.iter().any(|item| item.slug == "rename-entry"),
            "expected rename-entry in results: {items:?}"
        );
    }

    #[test]
    fn whitespace_only_query_treated_as_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tab-ai-memory.json");

        let entries = vec![TabAiMemoryEntry {
            schema_version: TAB_AI_MEMORY_ENTRY_SCHEMA_VERSION,
            intent: "open terminal".to_string(),
            generated_source: "code".to_string(),
            slug: "open-term".to_string(),
            prompt_type: "ScriptList".to_string(),
            bundle_id: Some("com.apple.Terminal".to_string()),
            written_at: "2026-03-29T12:00:00Z".to_string(),
        }];

        std::fs::write(
            &path,
            serde_json::to_string_pretty(&entries).expect("serialize"),
        )
        .expect("write");

        let items = resolve_tab_ai_prior_automations_for_entry_from_path(
            "   ",
            Some("com.apple.Terminal"),
            5,
            &path,
        )
        .expect("resolve");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "open-term");
    }
}

#[cfg(test)]
mod tab_ai_invocation_receipt_tests {
    use super::*;

    fn receipt(
        prompt_type: &str,
        input_text: Option<&str>,
        focused_id: Option<&str>,
        selected_id: Option<&str>,
        element_count: usize,
        warnings: &[&str],
    ) -> TabAiInvocationReceipt {
        let input_text = input_text.map(ToString::to_string);
        let focused_id = focused_id.map(ToString::to_string);
        let selected_id = selected_id.map(ToString::to_string);
        let warnings = warnings
            .iter()
            .map(|w| (*w).to_string())
            .collect::<Vec<_>>();
        TabAiInvocationReceipt::from_snapshot(
            prompt_type,
            &input_text,
            &focused_id,
            &selected_id,
            element_count,
            &warnings,
        )
    }

    #[test]
    fn script_list_with_empty_filter_is_still_rich_when_focus_and_elements_exist() {
        let r = receipt(
            "ScriptList",
            None,
            Some("input:filter"),
            Some("choice:0:slack"),
            3,
            &[],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert_eq!(r.focus_status, TabAiFieldStatus::Captured);
        assert_eq!(r.elements_status, TabAiFieldStatus::Captured);
        assert!(!r.has_input_text);
        assert!(r.has_focus_target);
        assert!(r.degradation_reasons.is_empty());
        assert!(r.rich);
    }

    #[test]
    fn term_prompt_without_linear_text_is_degraded() {
        let r = receipt(
            "TermPrompt",
            None,
            None,
            None,
            1,
            &["panel_only_term_prompt"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotExtractable));
        assert!(!r.rich);
    }

    #[test]
    fn current_view_fallback_is_never_reported_as_captured_elements() {
        let r = receipt(
            "SearchAiPresets",
            Some("claude"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.focus_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::MissingFocusTarget));
        assert!(!r.rich);
    }

    #[test]
    fn settings_surface_reports_input_not_applicable() {
        let r = receipt(
            "Settings",
            None,
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotApplicable));
        assert!(!r.rich);
    }

    #[test]
    fn receipt_serializes_machine_readable_statuses() {
        let r = receipt(
            "SearchAiPresets",
            Some("claude"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        let json = serde_json::to_string(&r).expect("receipt should serialize");
        assert!(json.contains("\"inputStatus\":\"captured\""));
        assert!(json.contains("\"elementsStatus\":\"degraded\""));
        assert!(json
            .contains("\"degradationReasons\":[\"collector_fallback\",\"missing_focus_target\"]"));
    }

    #[test]
    fn receipt_marks_empty_script_list_input_as_captured() {
        let r = receipt(
            "ScriptList",
            None,
            Some("input:filter"),
            Some("choice:0:slack"),
            3,
            &[],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Captured);
        assert!(!r.has_input_text);
        assert!(r.rich);
        assert!(r.degradation_reasons.is_empty());
    }

    #[test]
    fn receipt_marks_quick_terminal_missing_input_as_degraded() {
        let r = receipt(
            "QuickTerminal",
            None,
            None,
            None,
            1,
            &["panel_only_quick_terminal"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.focus_status, TabAiFieldStatus::Degraded);
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotExtractable));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::PanelOnlyElements));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::MissingFocusTarget));
        assert!(!r.rich);
    }

    #[test]
    fn receipt_marks_actions_dialog_input_as_unavailable() {
        let r = receipt(
            "ActionsDialog",
            None,
            None,
            None,
            1,
            &["panel_only_actions_dialog"],
        );
        assert_eq!(r.input_status, TabAiFieldStatus::Unavailable);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::InputNotApplicable));
    }

    #[test]
    fn receipt_marks_collector_fallback_explicitly() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            1,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
    }

    #[test]
    fn receipt_marks_collector_fallback_degraded_even_with_zero_elements() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            0,
            &["collector_used_current_view_fallback"],
        );
        assert_eq!(
            r.elements_status,
            TabAiFieldStatus::Degraded,
            "warnings should win over element_count==0"
        );
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::NoSemanticElements));
    }

    #[test]
    fn receipt_emits_both_panel_only_and_collector_fallback_independently() {
        let r = receipt(
            "FuturePrompt",
            Some("query"),
            None,
            None,
            1,
            &[
                "panel_only_future_prompt",
                "collector_used_current_view_fallback",
            ],
        );
        assert_eq!(r.elements_status, TabAiFieldStatus::Degraded);
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::PanelOnlyElements));
        assert!(r
            .degradation_reasons
            .contains(&TabAiDegradationReason::CollectorFallback));
    }
}

#[cfg(test)]
mod target_context_tests {
    use super::*;

    #[test]
    fn tab_ai_context_blob_serializes_focused_target() {
        let blob = TabAiContextBlob::from_parts_with_targets(
            TabAiUiSnapshot {
                prompt_type: "FileSearch".to_string(),
                selected_semantic_id: Some("choice:0:report.md".to_string()),
                ..Default::default()
            },
            Some(TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:report.md".to_string(),
                label: "report.md".to_string(),
                metadata: Some(serde_json::json!({
                    "path": "/tmp/report.md",
                    "fileType": "File"
                })),
            }),
            vec![],
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T13:37:35Z".to_string(),
        );
        let json = serde_json::to_value(&blob).expect("serialize context blob");
        assert_eq!(json["focusedTarget"]["kind"], "file");
        assert_eq!(json["focusedTarget"]["metadata"]["path"], "/tmp/report.md");
    }

    #[test]
    fn tab_ai_user_prompt_teaches_model_not_to_guess_target() {
        let prompt = build_tab_ai_user_prompt(
            "rename to kebab-case",
            r#"{"focusedTarget":null,"ui":{"promptType":"ScriptList"}}"#,
        );
        assert!(prompt.contains("focusedTarget is the default subject"));
        assert!(prompt.contains("do not invent an implicit subject"));
    }

    #[test]
    fn implicit_target_detection_avoids_false_positive_on_split() {
        assert!(tab_ai_intent_uses_implicit_target(
            "rename this to kebab-case"
        ));
        assert!(tab_ai_intent_uses_implicit_target("rename to kebab-case"));
        assert!(!tab_ai_intent_uses_implicit_target(
            "rename report.md to kebab-case"
        ));
        assert!(!tab_ai_intent_uses_implicit_target("split lines"));
        assert!(!tab_ai_intent_uses_implicit_target("copy url"));
    }

    #[test]
    fn from_parts_without_targets_leaves_focused_target_none() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot::default(),
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert!(blob.focused_target.is_none());
        assert!(blob.visible_targets.is_empty());
    }

    #[test]
    fn focused_target_serializes_camel_case() {
        let target = TabAiTargetContext {
            source: "ClipboardHistory".to_string(),
            kind: "clipboard_entry".to_string(),
            semantic_id: "choice:2:link".to_string(),
            label: "https://example.com".to_string(),
            metadata: Some(serde_json::json!({"contentType": "text"})),
        };
        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["semanticId"], "choice:2:link");
        assert_eq!(json["metadata"]["contentType"], "text");
    }

    #[test]
    fn implicit_target_detection_recognizes_all_pronouns() {
        assert!(tab_ai_intent_uses_implicit_target("open it"));
        assert!(tab_ai_intent_uses_implicit_target("paste that here"));
        assert!(tab_ai_intent_uses_implicit_target("copy selected text"));
        assert!(tab_ai_intent_uses_implicit_target("show current status"));
        assert!(tab_ai_intent_uses_implicit_target("close focused window"));
        assert!(tab_ai_intent_uses_implicit_target("force quit"));
    }

    #[test]
    fn visible_targets_omitted_when_empty() {
        let blob = TabAiContextBlob::from_parts(
            TabAiUiSnapshot::default(),
            crate::context_snapshot::AiContextSnapshot::default(),
            vec![],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&blob).expect("serialize");
        assert!(!json.contains("visibleTargets"));
        assert!(!json.contains("focusedTarget"));
    }
}

#[cfg(test)]
mod target_audit_tests {
    use super::*;

    fn sample_focused_target() -> TabAiTargetContext {
        TabAiTargetContext {
            source: "FileSearch".to_string(),
            kind: "file".to_string(),
            semantic_id: "choice:0:report.md".to_string(),
            label: "report.md".to_string(),
            metadata: Some(serde_json::json!({"path": "/tmp/report.md"})),
        }
    }

    fn sample_visible_targets() -> Vec<TabAiTargetContext> {
        vec![
            TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "file".to_string(),
                semantic_id: "choice:0:report.md".to_string(),
                label: "report.md".to_string(),
                metadata: None,
            },
            TabAiTargetContext {
                source: "FileSearch".to_string(),
                kind: "directory".to_string(),
                semantic_id: "choice:1:src".to_string(),
                label: "src".to_string(),
                metadata: None,
            },
        ]
    }

    #[test]
    fn target_audit_schema_version_is_one() {
        assert_eq!(TAB_AI_TARGET_AUDIT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn target_audit_from_focused_target_captures_fields() {
        let focused = sample_focused_target();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &[]);
        assert!(audit.has_focused_target);
        assert_eq!(audit.visible_target_count, 0);
        assert_eq!(audit.focused_source.as_deref(), Some("FileSearch"));
        assert_eq!(audit.focused_kind.as_deref(), Some("file"));
        assert_eq!(
            audit.focused_semantic_id.as_deref(),
            Some("choice:0:report.md")
        );
        assert!(audit.visible_kinds.is_empty());
    }

    #[test]
    fn target_audit_from_visible_targets_deduplicates_kinds() {
        let visible = sample_visible_targets();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &None, &visible);
        assert!(!audit.has_focused_target);
        assert_eq!(audit.visible_target_count, 2);
        assert!(audit.focused_source.is_none());
        assert!(audit.focused_kind.is_none());
        assert!(audit.focused_semantic_id.is_none());
        assert_eq!(audit.visible_kinds, vec!["directory", "file"]);
    }

    #[test]
    fn target_audit_serializes_camel_case() {
        let focused = sample_focused_target();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &[]);
        let json = serde_json::to_string(&audit).expect("serialize");

        assert!(json.contains("schemaVersion"));
        assert!(json.contains("promptType"));
        assert!(json.contains("hasFocusedTarget"));
        assert!(json.contains("visibleTargetCount"));
        assert!(json.contains("focusedSource"));
        assert!(json.contains("focusedKind"));
        assert!(json.contains("focusedSemanticId"));

        // Snake case must not appear
        assert!(!json.contains("schema_version"));
        assert!(!json.contains("prompt_type"));
        assert!(!json.contains("has_focused_target"));
        assert!(!json.contains("visible_target_count"));
        assert!(!json.contains("focused_source"));
        assert!(!json.contains("focused_kind"));
        assert!(!json.contains("focused_semantic_id"));
    }

    #[test]
    fn target_audit_roundtrip_preserves_all_fields() {
        let focused = sample_focused_target();
        let visible = sample_visible_targets();
        let audit = TabAiTargetAudit::from_targets("FileSearch", &Some(focused), &visible);
        let json = serde_json::to_string(&audit).expect("serialize");
        let parsed: TabAiTargetAudit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, audit);
    }

    #[test]
    fn target_audit_omits_empty_optional_fields() {
        let audit = TabAiTargetAudit::from_targets("ScriptList", &None, &[]);
        let json = serde_json::to_string(&audit).expect("serialize");
        assert!(!json.contains("focusedSource"));
        assert!(!json.contains("focusedKind"));
        assert!(!json.contains("focusedSemanticId"));
        assert!(!json.contains("visibleKinds"));
    }

    #[test]
    fn target_audit_fails_deserialization_without_required_fields() {
        // Missing hasFocusedTarget — required field
        let json = r#"{"schemaVersion":1,"promptType":"X","visibleTargetCount":0}"#;
        let result = serde_json::from_str::<TabAiTargetAudit>(json);
        assert!(result.is_err(), "should fail without hasFocusedTarget");
    }
}

#[cfg(test)]
mod tab_ai_source_type_tests {
    use super::*;

    #[test]
    fn script_list_origin_beats_desktop_selection() {
        // When the user is on ScriptList with a focused target AND desktop
        // happens to have selected text, the Script Kit origin surface wins.
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        let target = TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, Some(&target)),
            Some(TabAiSourceType::ScriptListItem),
            "ScriptList with focused target should beat incidental desktop selection"
        );
    }

    #[test]
    fn desktop_selection_wins_when_no_stronger_origin() {
        // Without a Script Kit-specific origin surface, desktop selection applies.
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
            Some(TabAiSourceType::DesktopSelection)
        );
    }

    #[test]
    fn script_list_requires_focused_target() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
            Some(TabAiSourceType::Desktop),
            "ScriptList without a focused target should fall through to Desktop"
        );

        let target = TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, Some(&target)),
            Some(TabAiSourceType::ScriptListItem)
        );
    }

    #[test]
    fn clipboard_history_maps_to_clipboard_entry() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("ClipboardHistory", &desktop, None),
            Some(TabAiSourceType::ClipboardEntry)
        );
    }

    #[test]
    fn prompt_surfaces_map_to_running_command() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        for prompt_type in &[
            "ArgPrompt",
            "MiniPrompt",
            "MicroPrompt",
            "DivPrompt",
            "FormPrompt",
            "EditorPrompt",
            "SelectPrompt",
            "PathPrompt",
            "DropPrompt",
            "TemplatePrompt",
            "HotkeyPrompt",
            "TermPrompt",
            "EnvPrompt",
            "ChatPrompt",
            "NamingPrompt",
        ] {
            assert_eq!(
                detect_tab_ai_source_type_from_prompt(prompt_type, &desktop, None),
                Some(TabAiSourceType::RunningCommand),
                "{prompt_type} should map to RunningCommand"
            );
        }
    }

    #[test]
    fn unknown_surface_falls_through_to_desktop() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();
        assert_eq!(
            detect_tab_ai_source_type_from_prompt("SomeOtherView", &desktop, None),
            Some(TabAiSourceType::Desktop)
        );
    }

    #[test]
    fn empty_or_whitespace_selected_text_does_not_trigger_desktop_selection() {
        for text in &["", "   ", "\n\t  "] {
            let desktop = crate::context_snapshot::AiContextSnapshot {
                selected_text: Some(text.to_string()),
                ..Default::default()
            };
            assert_ne!(
                detect_tab_ai_source_type_from_prompt("ScriptList", &desktop, None),
                Some(TabAiSourceType::DesktopSelection),
                "whitespace-only selected_text should not trigger DesktopSelection"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Source-text contract tests: verify structural invariants of agent_handoff.rs
    // -----------------------------------------------------------------------

    const TAB_AI_MODE_SRC: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/app_impl/agent_handoff/mod.rs"
    ));

    #[test]
    fn quick_terminal_switch_and_notify_happen_before_capture_await() {
        let open_idx = TAB_AI_MODE_SRC
            .find("self.current_view = AppView::QuickTerminalView")
            .expect("quick terminal view switch");
        let notify_idx = TAB_AI_MODE_SRC[open_idx..]
            .find("cx.notify();")
            .map(|idx| open_idx + idx)
            .expect("notify after view switch");
        // Search from the view switch onwards so we find the await in the
        // new-session path, not the live-session helper above it.
        let await_idx = TAB_AI_MODE_SRC[open_idx..]
            .find("capture_rx.recv().await")
            .map(|idx| open_idx + idx)
            .expect("deferred capture await after view switch");
        assert!(
            open_idx < await_idx,
            "QuickTerminalView must be visible before deferred capture is awaited"
        );
        assert!(
            notify_idx < await_idx,
            "cx.notify() must happen before deferred capture is awaited"
        );
    }

    #[test]
    fn deferred_capture_is_started_before_harness_open_call() {
        let scoped_start = TAB_AI_MODE_SRC
            .find("let capture_rx = if use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {")
            .expect("capture staging block");
        let scoped = &TAB_AI_MODE_SRC[scoped_start..];
        let spawn_idx = scoped
            .find("self.spawn_tab_ai_pre_switch_capture(&request)")
            .expect("capture spawn");
        let open_idx = scoped
            .find("self.open_tab_ai_agent_chat_view_from_request_impl(")
            .expect("harness open");
        assert!(
            spawn_idx < open_idx,
            "capture must be started before the harness open call"
        );
    }

    #[test]
    fn pre_switch_capture_uses_immediate_thread_spawn() {
        let fn_start = TAB_AI_MODE_SRC
            .find("fn spawn_tab_ai_pre_switch_capture(")
            .expect("function start");
        let fn_end = TAB_AI_MODE_SRC[fn_start..]
            .find("fn open_tab_ai_harness_terminal_from_request(")
            .map(|idx| fn_start + idx)
            .expect("next function");
        let body = &TAB_AI_MODE_SRC[fn_start..fn_end];
        assert!(
            body.contains("std::thread::spawn(move ||"),
            "capture must start immediately on its own thread"
        );
        assert!(
            !body.contains("cx.background_executor().spawn(async move {"),
            "do not add an extra scheduler hop before desktop capture begins"
        );
    }

    #[test]
    fn apply_back_hint_matches_source_type() {
        let cases = [
            (TabAiSourceType::DesktopSelection, "replaceSelectedText"),
            (TabAiSourceType::ScriptListItem, "runGeneratedScript"),
            (TabAiSourceType::RunningCommand, "pasteToPrompt"),
            (TabAiSourceType::ClipboardEntry, "copyToClipboard"),
            (TabAiSourceType::Desktop, "pasteToFrontmostApp"),
        ];
        for (source_type, expected_action) in &cases {
            let hint = build_tab_ai_apply_back_hint_from_source(Some(source_type))
                .expect("should produce a hint");
            assert_eq!(
                hint.action, *expected_action,
                "wrong action for {source_type:?}"
            );
            assert!(
                hint.target_label.is_some(),
                "target_label should be set for {source_type:?}"
            );
        }
        assert!(build_tab_ai_apply_back_hint_from_source(None).is_none());
    }

    #[test]
    fn apply_back_footer_label_matches_source_type() {
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::RunningCommand)),
            "Paste Back to Prompt"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::ClipboardEntry)),
            "Copy Result"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::ScriptListItem)),
            "Save as Script & Run"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::DesktopSelection)),
            "Replace Selection"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(Some(&TabAiSourceType::Desktop)),
            "Paste Back to App"
        );
        assert_eq!(
            tab_ai_apply_back_footer_label(None),
            "Preparing Paste Back\u{2026}"
        );
    }
}

#[cfg(test)]
mod tab_ai_apply_back_route_tests {
    use super::*;

    #[test]
    fn apply_back_route_serde_roundtrip() {
        let route = TabAiApplyBackRoute {
            source_type: TabAiSourceType::DesktopSelection,
            hint: TabAiApplyBackHint {
                action: "replaceSelectedText".to_string(),
                target_label: Some("Frontmost selection".to_string()),
            },
            focused_target: None,
        };
        let json = serde_json::to_string(&route).expect("serialize");
        let back: TabAiApplyBackRoute = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(route, back);

        // Route with focused_target round-trips correctly
        let route_with_target = TabAiApplyBackRoute {
            source_type: TabAiSourceType::ScriptListItem,
            hint: TabAiApplyBackHint {
                action: "runGeneratedScript".to_string(),
                target_label: Some("Focused script".to_string()),
            },
            focused_target: Some(TabAiTargetContext {
                source: "ScriptList".to_string(),
                kind: "script".to_string(),
                semantic_id: "choice:0:my-script".to_string(),
                label: "My Script".to_string(),
                metadata: Some(serde_json::json!({"path": "/scripts/my-script.ts"})),
            }),
        };
        let json2 = serde_json::to_string(&route_with_target).expect("serialize with target");
        let back2: TabAiApplyBackRoute =
            serde_json::from_str(&json2).expect("deserialize with target");
        assert_eq!(route_with_target, back2);
        assert!(
            json2.contains("focusedTarget"),
            "focusedTarget must appear when Some"
        );
    }

    #[test]
    fn tab_ai_harness_tracks_apply_back_route_state() {
        let source =
            std::fs::read_to_string("src/main_sections/app_state.rs").expect("read app_state.rs");
        assert!(
            source.contains("tab_ai_harness_apply_back_route"),
            "ScriptListApp must persist apply-back routing state for the active harness session"
        );
    }

    #[test]
    fn quick_terminal_cmd_enter_routes_to_apply_back() {
        // Cmd+Enter now goes through the shared dispatcher, which routes to
        // the de-raced apply-back helper (after the portal-attach check).
        let source = std::fs::read_to_string("src/render_prompts/term.rs").expect("read term.rs");
        assert!(
            source.contains("this.dispatch_quick_terminal_cmd_enter(entity.clone(), cx);"),
            "QuickTerminalView must route Cmd+Enter through the shared dispatcher"
        );
        let dispatcher = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff/mod.rs");
        let start = dispatcher
            .find("fn dispatch_quick_terminal_cmd_enter")
            .expect("dispatch_quick_terminal_cmd_enter should exist");
        let body = &dispatcher[start..(start + 2000).min(dispatcher.len())];
        assert!(
            body.contains("self.apply_tab_ai_result_from_terminal(entity, cx);"),
            "dispatcher must route Cmd+Enter through the de-raced apply-back helper"
        );
    }

    #[test]
    fn tab_ai_apply_back_uses_running_command_prompt_reinjection() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff.rs");
        assert!(
            source.contains("self.try_set_prompt_input(text.clone(), cx)"),
            "RunningCommand apply-back must reuse try_set_prompt_input"
        );
    }

    #[test]
    fn tab_ai_frontmost_apply_back_hides_before_paste() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff.rs");
        let hide_pos = source
            .find("crate::platform::defer_hide_main_window(cx)")
            .expect("apply-back must defer-hide the main window");
        let replace_pos = source
            .find("selected_text::set_selected_text(&text_for_apply)")
            .expect("apply-back must support selected-text replacement");
        let paste_pos = source
            .find(".paste_text(&text_for_apply)")
            .expect("apply-back must support frontmost-app paste");
        assert!(
            hide_pos < replace_pos,
            "main window must hide before set_selected_text fires"
        );
        assert!(
            hide_pos < paste_pos,
            "main window must hide before TextInjector::paste_text fires"
        );
    }

    #[test]
    fn tab_ai_apply_back_route_cleared_on_close() {
        let source = std::fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
            .expect("read agent_handoff.rs");
        let close_fn_pos = source
            .find("fn close_tab_ai_harness_terminal")
            .expect("close_tab_ai_harness_terminal must exist");
        let clear_pos = source[close_fn_pos..]
            .find("self.tab_ai_harness_apply_back_route = None")
            .expect("close must clear apply-back route");
        let slice = &source[close_fn_pos..close_fn_pos + clear_pos];
        let lines_between = slice.lines().count();
        assert!(
            lines_between < 60,
            "route clear should be near the top of close_tab_ai_harness_terminal, found at line offset {lines_between}"
        );
    }
}
