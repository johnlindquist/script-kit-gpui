//! MCP Resources Handler
//!
//! Implements MCP resources for Script Kit:
//! - `kit://state` - Current app state as JSON
//! - `scripts://` - List of available scripts
//! - `scriptlets://` - List of available scriptlets
//!
//! Resources are read-only data that clients can access without tool calls.

mod transaction_resources;

// --- merged from part_000.rs ---
use crate::scripts::Script;
use crate::scripts::Scriptlet;
use crate::scripts::{FailedScript, ScriptValidationIssue, ValidationReport};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

const NOTES_RESOURCE_URI: &str = "kit://notes";
const NOTES_RESOURCE_SCHEMA_VERSION: u32 = 1;
const AUDIT_RESOURCE_URI: &str = "kit://audit";
const AUDIT_RESOURCE_SCHEMA_VERSION: u32 = 1;
const AUDIT_DEFAULT_LIMIT: usize = 100;
const AUDIT_HARD_LIMIT: usize = 500;
const GIT_DIFF_DEFAULT_LIMIT_BYTES: usize = 1024 * 1024;
const GIT_DIFF_HARD_CAP_BYTES: usize = 8 * 1024 * 1024;
/// MCP Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// Unique URI for this resource (e.g., "scripts://", "kit://state")
    pub uri: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this resource provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type of the resource content
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}
/// Resource content returned by resources/read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The URI of the resource
    pub uri: String,
    /// MIME type of the content
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// The actual content (typically JSON stringified)
    pub text: String,
}
/// Application state exposed via kit://state resource
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppStateResource {
    /// Whether the app window is visible
    pub visible: bool,
    /// Whether the app window is focused
    pub focused: bool,
    /// Number of loaded scripts
    pub script_count: usize,
    /// Number of loaded scriptlets
    pub scriptlet_count: usize,
    /// Current filter text (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
    /// Currently selected index (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_index: Option<usize>,
}
/// Script metadata for the scripts:// resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptResourceEntry {
    /// Script name
    pub name: String,
    /// File path
    pub path: String,
    /// File extension (ts, js)
    pub extension: String,
    /// Description (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether script has a schema (makes it an MCP tool)
    pub has_schema: bool,
}
impl From<&Script> for ScriptResourceEntry {
    fn from(script: &Script) -> Self {
        Self {
            name: script.name.clone(),
            path: script.path.to_string_lossy().to_string(),
            extension: script.extension.clone(),
            description: script.description.clone(),
            has_schema: script.schema.is_some(),
        }
    }
}
/// Scriptlet metadata for the scriptlets:// resource  
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptletResourceEntry {
    /// Scriptlet name
    pub name: String,
    /// Tool type (bash, ts, paste, etc.)
    pub tool: String,
    /// Description (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Group name (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Expand trigger (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    /// Keyboard shortcut (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
}
impl From<&Scriptlet> for ScriptletResourceEntry {
    fn from(scriptlet: &Scriptlet) -> Self {
        Self {
            name: scriptlet.name.clone(),
            tool: scriptlet.tool.clone(),
            description: scriptlet.description.clone(),
            group: scriptlet.group.clone(),
            keyword: scriptlet.keyword.clone(),
            shortcut: scriptlet.shortcut.clone(),
        }
    }
}
/// Get all available MCP resources
pub fn get_resource_definitions() -> Vec<McpResource> {
    let mut resources = vec![
        McpResource {
            uri: "kit://state".to_string(),
            name: "App State".to_string(),
            description: Some(
                "Current Script Kit application state including visibility, focus, and counts"
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "scripts://".to_string(),
            name: "Scripts".to_string(),
            description: Some("List of all available scripts discovered from installed plugins (plugins/main/scripts/ is the default personal plugin). Scripts are loaded from all plugin roots under ~/.scriptkit/plugins/*/scripts/.".to_string()),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "scriptlets://".to_string(),
            name: "Scriptlets".to_string(),
            description: Some("List of all available scriptlets from markdown files".to_string()),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: NOTES_RESOURCE_URI.to_string(),
            name: "Notes".to_string(),
            description: Some(
                "Active Script Kit notes. Read kit://notes for a bounded list with metadata, kit://notes?tag=... to filter organized notes, add &full=true for full bodies, or kit://notes/{id} for a full note."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: crate::brain::resources::BRAIN_RESOURCE_URI.to_string(),
            name: "Brain".to_string(),
            description: Some(
                "Script Kit's local memory. kit://brain for status, kit://brain/recall?q=... for hybrid retrieval, add &format=json for source refs, kit://brain/doc?source=...&sourceId=... for one doc, kit://brain/docs?refs=... for batch doc reads, and kit://brain/signals for recent attention signals."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: AUDIT_RESOURCE_URI.to_string(),
            name: "MCP Audit Log".to_string(),
            description: Some(
                "Recent MCP mutation audit events from ~/.scriptkit/mcp-audit.jsonl. Supports ?limit=100 and ?traceId=..."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: crate::computer_use::COMPUTER_USE_READINESS_RESOURCE_URI.to_string(),
            name: "Computer Use Readiness".to_string(),
            description: Some(
                "Read-only fail-closed preflight receipt for third-party GUI Computer Use readiness."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://context".to_string(),
            name: "Current Context".to_string(),
            description: Some(
                "Deterministic snapshot of AI-relevant desktop context. Supports ?profile=minimal, ?diagnostics=1, and per-field flags: selectedText, frontmostApp, menuBar, browserUrl, focusedWindow, screenshot, panelScreenshot. See kit://context/schema for the full contract."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://context/schema".to_string(),
            name: "Current Context Schema".to_string(),
            description: Some(
                "Self-describing schema for kit://context profiles, flags, diagnostics output, and example URIs."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://scripts".to_string(),
            name: "Scripts (versioned)".to_string(),
            description: Some(
                "Schema-versioned list of all scripts discovered from installed plugins with metadata. plugins/main/scripts/ is the default personal plugin. Safe for repeated reads."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://scriptlets".to_string(),
            name: "Scriptlets (versioned)".to_string(),
            description: Some(
                "Schema-versioned list of all scriptlets from markdown extension files with metadata."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: COMMAND_DOCTOR_RESOURCE_URI.to_string(),
            name: "Command Doctor".to_string(),
            description: Some(
                "Read-only command readiness, SDK capability support, safe permission-pending state, and actionable author repairs.".to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://sdk-reference".to_string(),
            name: "SDK Reference".to_string(),
            description: Some(
                "Script Kit SDK functions, host capability contracts, authoring diagnostics resources, script metadata, and harness-safe directory conventions."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: FAILED_SCRIPTS_RESOURCE_URI.to_string(),
            name: "Failed Scripts".to_string(),
            description: Some(
                "Author diagnostics for excluded invalid scripts and retained, disabled scriptlets, including metadata collisions, unsupported SDK capabilities, incompatible execution topologies, permission-pending state, source paths, and actionable repairs."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: SCRIPT_TEMPLATES_RESOURCE_URI.to_string(),
            name: "Script Templates".to_string(),
            description: Some(
                "Curated starter-script templates for the launcher's New Script from Template catalog. Same Rust-owned data the in-launcher catalog renders, so templates cannot drift between the UI and any MCP harness. v1 templates omit binding fields (`alias`, `shortcut`, `keyword`, `trigger`) so newly-created scripts cannot be immediately hidden by validation."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://clipboard-history".to_string(),
            name: "Clipboard History".to_string(),
            description: Some(
                "Most recent clipboard entries in newest-first order with content type, preview, OCR text, timestamps, and image dimensions. Supports ?limit=N (default 10) and ?diagnostics=1."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://focused-item".to_string(),
            name: "Focused Item".to_string(),
            description: Some(
                "Precise focused or selected item metadata for the active surface. Includes source, kind, semantic ID, label, and surface-specific metadata. Supports ?diagnostics=1."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://git-status".to_string(),
            name: "Git Status".to_string(),
            description: Some(
                "Current git status output from the working directory."
                    .to_string(),
            ),
            mime_type: "text/plain".to_string(),
        },
        McpResource {
            uri: "kit://git-diff".to_string(),
            name: "Git Diff".to_string(),
            description: Some(
                "Current git diff output (staged and unstaged) from the working directory."
                    .to_string(),
            ),
            mime_type: "text/plain".to_string(),
        },
        McpResource {
            uri: "kit://processes".to_string(),
            name: "Processes".to_string(),
            description: Some(
                "Top running processes by CPU usage."
                    .to_string(),
            ),
            mime_type: "text/plain".to_string(),
        },
        McpResource {
            uri: "kit://system".to_string(),
            name: "System Info".to_string(),
            description: Some(
                "Basic system information: hostname, OS version, architecture, uptime, and shell."
                    .to_string(),
            ),
            mime_type: "text/plain".to_string(),
        },
        McpResource {
            uri: "kit://dictation".to_string(),
            name: "Dictation".to_string(),
            description: Some(
                "Most recent dictated text captured by Script Kit. Returns a stable JSON envelope and never fails when no provider is configured."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://dictation-history".to_string(),
            name: "Dictation History".to_string(),
            description: Some(
                "Saved dictation history. Supports ?id=<entry-id> for a single transcript and ?limit=N (default 10) for newest-first JSON summaries."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://calendar".to_string(),
            name: "Calendar".to_string(),
            description: Some(
                "Upcoming calendar events in a prompt-safe JSON envelope."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: "kit://notifications".to_string(),
            name: "Notifications".to_string(),
            description: Some(
                "Recent notifications in newest-first order, capped and summarized for prompt use."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
        McpResource {
            uri: STDIN_COMMANDS_REFERENCE_URI.to_string(),
            name: "Stdin JSONL Commands".to_string(),
            description: Some(
                "Canonical list of stdin JSONL `type` verbs accepted by the ExternalCommand parser. Payload is audited against `stdin_commands::all_external_command_verbs()` so documentation and runtime cannot drift."
                    .to_string(),
            ),
            mime_type: "text/markdown".to_string(),
        },
        McpResource {
            uri: TRIGGER_BUILTINS_REFERENCE_URI.to_string(),
            name: "Trigger Built-ins".to_string(),
            description: Some(
                "Canonical `builtin/...` command IDs accepted by `triggerBuiltin`. Payload is audited against `trigger_registry::all_trigger_builtin_command_ids()` to guarantee the list never goes stale."
                    .to_string(),
            ),
            mime_type: "text/markdown".to_string(),
        },
        McpResource {
            uri: PROTOCOL_STATS_DIAGNOSTICS_URI.to_string(),
            name: "Protocol Stats".to_string(),
            description: Some(
                "Rust↔Bun protocol-boundary counters plus a machine-readable `health.ok` / `health.flags` summary. Exposes `snapshot` (per-counter totals), `health` (threshold-crossed flags), and `thresholds` so MCP consumers can render a boundary health chip without hardcoding limits."
                    .to_string(),
            ),
            mime_type: "application/json".to_string(),
        },
    ];
    resources.extend(transaction_resources::transaction_resource_definitions());
    resources
}

pub const PROTOCOL_STATS_DIAGNOSTICS_URI: &str = "kit://diagnostics/protocol-stats";
/// Read a specific resource by URI
///
/// # Arguments
/// * `uri` - The resource URI to read
/// * `scripts` - Available scripts for scripts:// resource
/// * `scriptlets` - Available scriptlets for scriptlets:// resource
/// * `app_state` - Current app state for kit://state resource
///
/// # Returns
/// * `Ok(ResourceContent)` - The resource content
/// * `Err(String)` - Error message if resource not found
pub fn read_resource(
    uri: &str,
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    app_state: Option<&AppStateResource>,
) -> Result<ResourceContent, String> {
    match uri {
        "kit://state" => read_state_resource(app_state),
        "scripts://" => read_scripts_resource(scripts),
        "scriptlets://" => read_scriptlets_resource(scriptlets),
        "kit://scripts" => read_kit_scripts_resource(scripts),
        "kit://scriptlets" => read_kit_scriptlets_resource(scriptlets),
        "kit://sdk-reference" => read_sdk_reference_resource(),
        COMMAND_DOCTOR_RESOURCE_URI => read_command_doctor_resource(scripts, scriptlets),
        FAILED_SCRIPTS_RESOURCE_URI => read_kit_failed_scripts_resource(),
        SCRIPT_TEMPLATES_RESOURCE_URI => read_kit_script_templates_resource(),
        _ if uri == "kit://context"
            || uri.starts_with("kit://context?")
            || uri == "kit://context/schema"
            || uri.starts_with("kit://context/schema?") =>
        {
            read_context_resource(uri)
        }
        _ if uri == "kit://clipboard-history" || uri.starts_with("kit://clipboard-history?") => {
            read_clipboard_history_resource(uri)
        }
        _ if uri == "kit://focused-item" || uri.starts_with("kit://focused-item?") => {
            read_focused_item_resource(uri)
        }
        _ if uri == "kit://dictation" || uri.starts_with("kit://dictation?") => {
            read_dictation_resource(uri)
        }
        _ if uri == "kit://dictation-history" || uri.starts_with("kit://dictation-history?") => {
            read_dictation_history_resource(uri)
        }
        _ if uri == "kit://calendar" || uri.starts_with("kit://calendar?") => {
            read_calendar_resource(uri)
        }
        _ if uri == "kit://notifications" || uri.starts_with("kit://notifications?") => {
            read_notifications_resource(uri)
        }
        "kit://git-status" => read_git_status_resource(),
        _ if is_notes_resource_uri(uri) => read_notes_resource(uri),
        _ if crate::brain::resources::is_brain_resource_uri(uri) => {
            let (mime_type, text) = crate::brain::resources::read_brain_resource(uri)?;
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type,
                text,
            })
        }
        _ if is_audit_resource_uri(uri) => read_audit_resource(uri),
        crate::computer_use::COMPUTER_USE_READINESS_RESOURCE_URI => {
            read_computer_use_readiness_resource()
        }
        _ if uri == "kit://git-diff" || uri.starts_with("kit://git-diff?") => {
            read_git_diff_resource(uri)
        }
        "kit://processes" => read_processes_resource(),
        "kit://system" => read_system_info_resource(),
        STDIN_COMMANDS_REFERENCE_URI => read_stdin_commands_resource(),
        TRIGGER_BUILTINS_REFERENCE_URI => read_trigger_builtins_resource(),
        PROTOCOL_STATS_DIAGNOSTICS_URI => read_protocol_stats_resource(),
        _ if transaction_resources::is_transaction_resource_uri(uri) => {
            transaction_resources::read_transaction_resource(uri)
        }
        _ => Err(format!("Resource not found: {}", uri)),
    }
}

pub(crate) fn is_context_resource_uri(uri: &str) -> bool {
    uri == "kit://context"
        || uri.starts_with("kit://context?")
        || uri == "kit://context/schema"
        || uri.starts_with("kit://context/schema?")
}

pub(crate) fn is_notes_resource_uri(uri: &str) -> bool {
    uri == NOTES_RESOURCE_URI || uri.starts_with("kit://notes?") || uri.starts_with("kit://notes/")
}

pub(crate) fn is_audit_resource_uri(uri: &str) -> bool {
    uri == AUDIT_RESOURCE_URI || uri.starts_with("kit://audit?")
}

fn read_notes_resource(uri: &str) -> Result<ResourceContent, String> {
    crate::notes::init_notes_db()
        .map_err(|error| format!("Failed to initialize notes database: {error}"))?;

    if uri == NOTES_RESOURCE_URI || uri.starts_with("kit://notes?") {
        return read_notes_list_resource(uri);
    }

    read_single_note_resource(uri)
}

fn read_notes_list_resource(uri: &str) -> Result<ResourceContent, String> {
    let include_deleted = query_bool(uri, "includeDeleted");
    let list_query = notes_list_search_query(uri);
    let mut notes = if let Some(query) = &list_query {
        crate::notes::search_notes(query)
            .map_err(|error| format!("Failed to search notes: {error}"))?
    } else if include_deleted {
        let mut active = crate::notes::get_all_notes()
            .map_err(|error| format!("Failed to read active notes: {error}"))?;
        let mut deleted = crate::notes::get_deleted_notes()
            .map_err(|error| format!("Failed to read deleted notes: {error}"))?;
        active.append(&mut deleted);
        active
    } else {
        crate::notes::get_all_notes().map_err(|error| format!("Failed to read notes: {error}"))?
    };

    let original_len = notes.len();
    // full=true swaps the 240-char preview for the full note body, bounded
    // tighter so instruction-note loads stay a sane context size.
    let full_content = query_bool(uri, "full");
    let default_limit = if full_content { 20 } else { 100 };
    let max_limit = if full_content { 50 } else { 500 };
    let limit = parse_u64_query_param(uri, "limit")
        .unwrap_or(default_limit)
        .clamp(1, max_limit) as usize;
    notes.truncate(limit);

    let summaries: Vec<Value> = notes
        .iter()
        .map(|note| {
            if full_content {
                note_full_json(note)
            } else {
                note_summary_json(note)
            }
        })
        .collect();
    let json = serde_json::json!({
        "schemaVersion": NOTES_RESOURCE_SCHEMA_VERSION,
        "uri": NOTES_RESOURCE_URI,
        "query": list_query,
        "count": summaries.len(),
        "truncated": original_len > summaries.len(),
        "notes": summaries,
    });

    Ok(ResourceContent {
        uri: NOTES_RESOURCE_URI.to_string(),
        mime_type: "application/json".to_string(),
        text: serde_json::to_string_pretty(&json)
            .map_err(|error| format!("Failed to serialize notes resource: {error}"))?,
    })
}

fn read_single_note_resource(uri: &str) -> Result<ResourceContent, String> {
    let raw_id = uri
        .strip_prefix("kit://notes/")
        .and_then(|rest| rest.split('?').next())
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| format!("Invalid notes resource URI: {uri}"))?;
    let note_id = crate::notes::NoteId::parse(raw_id)
        .ok_or_else(|| format!("Invalid note id in URI: {raw_id}"))?;
    let note = crate::notes::get_note(note_id)
        .map_err(|error| format!("Failed to read note {note_id}: {error}"))?
        .ok_or_else(|| format!("Note not found: {note_id}"))?;

    let resource_uri = format!("kit://notes/{note_id}");
    let json = serde_json::json!({
        "schemaVersion": NOTES_RESOURCE_SCHEMA_VERSION,
        "uri": resource_uri,
        "note": note,
        "metadata": note_metadata_json(note_id),
    });

    Ok(ResourceContent {
        uri: resource_uri,
        mime_type: "application/json".to_string(),
        text: serde_json::to_string_pretty(&json)
            .map_err(|error| format!("Failed to serialize note resource: {error}"))?,
    })
}

fn note_summary_json(note: &crate::notes::Note) -> Value {
    let preview: String = note.content.chars().take(240).collect();
    let metadata = note_metadata_json(note.id);
    serde_json::json!({
        "id": note.id.as_str(),
        "uri": format!("kit://notes/{}", note.id),
        "title": note.title,
        "preview": preview,
        "charCount": note.content.chars().count(),
        "createdAt": note.created_at.to_rfc3339(),
        "updatedAt": note.updated_at.to_rfc3339(),
        "deletedAt": note.deleted_at.map(|dt| dt.to_rfc3339()),
        "isPinned": note.is_pinned,
        "sortOrder": note.sort_order,
        "metadata": metadata,
    })
}

/// Per-note body cap when `full=true` is requested on the notes list resource.
const NOTE_FULL_CONTENT_MAX_CHARS: usize = 20_000;

fn note_full_json(note: &crate::notes::Note) -> Value {
    let mut json = note_summary_json(note);
    let content: String = note
        .content
        .chars()
        .take(NOTE_FULL_CONTENT_MAX_CHARS)
        .collect();
    if let Some(object) = json.as_object_mut() {
        object.insert(
            "contentTruncated".to_string(),
            Value::Bool(note.content.chars().count() > NOTE_FULL_CONTENT_MAX_CHARS),
        );
        object.insert("content".to_string(), Value::String(content));
        object.remove("preview");
    }
    json
}

fn note_metadata_json(note_id: crate::notes::NoteId) -> Value {
    let tags = crate::notes::get_note_tags(note_id).unwrap_or_default();
    let aliases = crate::notes::get_note_aliases(note_id).unwrap_or_default();
    let tag_count = tags.len();
    let alias_count = aliases.len();
    let outbound_link_count = crate::notes::get_note_outbound_link_count(note_id).unwrap_or(0);
    let backlink_count = crate::notes::get_note_backlink_count(note_id).unwrap_or(0);
    serde_json::json!({
        "tags": tags,
        "aliases": aliases,
        "tagCount": tag_count,
        "aliasCount": alias_count,
        "outboundLinkCount": outbound_link_count,
        "backlinkCount": backlink_count,
    })
}

fn notes_list_search_query(uri: &str) -> Option<String> {
    if let Some(tag) = query_string_param(uri, "tag").filter(|value| !value.trim().is_empty()) {
        return Some(format!("tag:{tag}"));
    }
    if let Some(alias) = query_string_param(uri, "alias").filter(|value| !value.trim().is_empty()) {
        return Some(format!("alias:{alias}"));
    }
    if let Some(link) = query_string_param(uri, "link").filter(|value| !value.trim().is_empty()) {
        return Some(format!("link:{link}"));
    }
    query_string_param(uri, "q").filter(|value| !value.trim().is_empty())
}

pub(crate) fn parse_u64_query_param(uri: &str, key: &str) -> Option<u64> {
    let query = uri.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(k, v)| {
            if k == key {
                v.parse::<u64>().ok()
            } else {
                None
            }
        })
}

fn query_bool(uri: &str, key: &str) -> bool {
    let Some(query) = uri.split_once('?').map(|(_, query)| query) else {
        return false;
    };
    query.split('&').any(|pair| {
        let Some((k, v)) = pair.split_once('=') else {
            return pair == key;
        };
        k == key && matches!(v, "1" | "true" | "TRUE" | "yes")
    })
}

fn query_string_param(uri: &str, key: &str) -> Option<String> {
    let query = uri.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (percent_decode_query_component(k) == key).then(|| percent_decode_query_component(v))
    })
}

fn percent_decode_query_component(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let raw = input.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < raw.len() => {
                let hi = hex_value(raw[index + 1]);
                let lo = hex_value(raw[index + 2]);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    bytes.push((hi << 4) | lo);
                    index += 3;
                } else {
                    bytes.push(raw[index]);
                    index += 1;
                }
            }
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn read_audit_resource(uri: &str) -> Result<ResourceContent, String> {
    let limit = parse_u64_query_param(uri, "limit")
        .map(|value| value as usize)
        .unwrap_or(AUDIT_DEFAULT_LIMIT)
        .clamp(1, AUDIT_HARD_LIMIT);
    let trace_id_filter = query_string_param(uri, "traceId");

    let audit_path = dirs::home_dir()
        .ok_or_else(|| "Failed to resolve home directory for MCP audit log".to_string())?
        .join(".scriptkit")
        .join("mcp-audit.jsonl");

    let text = match std::fs::read_to_string(&audit_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "Failed to read MCP audit log {}: {error}",
                audit_path.display()
            ));
        }
    };

    let mut matched = Vec::new();
    for line in text.lines().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(trace_id) = &trace_id_filter {
            if event.get("traceId").and_then(|value| value.as_str()) != Some(trace_id.as_str()) {
                continue;
            }
        }
        matched.push(event);
        if matched.len() == limit {
            break;
        }
    }
    matched.reverse();

    let json = serde_json::json!({
        "schemaVersion": AUDIT_RESOURCE_SCHEMA_VERSION,
        "uri": uri,
        "count": matched.len(),
        "truncated": text.lines().count() > matched.len(),
        "events": matched,
    });

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text: serde_json::to_string_pretty(&json)
            .map_err(|error| format!("Failed to serialize MCP audit resource: {error}"))?,
    })
}
/// Read kit://state resource
fn read_state_resource(app_state: Option<&AppStateResource>) -> Result<ResourceContent, String> {
    let state = app_state.cloned().unwrap_or_default();
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize app state: {}", e))?;

    Ok(ResourceContent {
        uri: "kit://state".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

fn read_computer_use_readiness_resource() -> Result<ResourceContent, String> {
    let receipt = crate::computer_use::current_computer_use_readiness_receipt();
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("Failed to serialize computer-use readiness: {error}"))?;

    Ok(ResourceContent {
        uri: crate::computer_use::COMPUTER_USE_READINESS_RESOURCE_URI.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

/// Read scripts:// resource
fn read_scripts_resource(scripts: &[Arc<Script>]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptResourceEntry> = scripts
        .iter()
        .map(|s| ScriptResourceEntry::from(s.as_ref()))
        .collect();
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize scripts: {}", e))?;

    Ok(ResourceContent {
        uri: "scripts://".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}
/// Read scriptlets:// resource
fn read_scriptlets_resource(scriptlets: &[Arc<Scriptlet>]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptletResourceEntry> = scriptlets
        .iter()
        .map(|s| ScriptletResourceEntry::from(s.as_ref()))
        .collect();
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| format!("Failed to serialize scriptlets: {}", e))?;

    Ok(ResourceContent {
        uri: "scriptlets://".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

// ---------------------------------------------------------------
// Schema-versioned script/scriptlet/sdk-reference resources
// ---------------------------------------------------------------

/// URI for the stdin JSONL verb reference resource.
///
/// Declared payload entries live inside
/// `<!-- drift-audit:stdin-verbs:start -->` / `<!-- drift-audit:stdin-verbs:end -->`
/// and are audited against
/// [`crate::stdin_commands::all_external_command_verbs`] by
/// `tests/mcp_resource_drift.rs`.
pub const STDIN_COMMANDS_REFERENCE_URI: &str = "kit://stdin-commands";

/// URI for the canonical triggerBuiltin command-id reference resource.
///
/// Declared payload entries live inside
/// `<!-- drift-audit:trigger-builtin-ids:start -->` /
/// `<!-- drift-audit:trigger-builtin-ids:end -->` and are audited against
/// [`crate::builtins::trigger_registry::all_trigger_builtin_command_ids`]
/// by `tests/mcp_resource_drift.rs`.
pub const TRIGGER_BUILTINS_REFERENCE_URI: &str = "kit://trigger-builtins";

/// Schema version for the `kit://clipboard-history` resource envelope.
pub const CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Schema version for the `kit://focused-item` resource envelope.
pub const FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Schema version for the `kit://scripts` resource envelope.
pub const SCRIPTS_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Schema version for the `kit://scriptlets` resource envelope.
pub const SCRIPTLETS_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Schema version for the `kit://sdk-reference` resource.
/// Bumped to 6: adds a versioned host-owned capability catalog containing
/// support, transport, platform, permission, and migration requirements.
pub const SDK_REFERENCE_SCHEMA_VERSION: u32 = 6;

/// Independent schema version for the reusable SDK capability catalog.
pub const SDK_CAPABILITY_CATALOG_SCHEMA_VERSION: u32 = 1;

/// URI for the `kit://failed-scripts` resource.
///
/// Surfaces the `ValidationReport.failed_scripts` list so authors can see
/// which scripts were excluded from the kept catalog (today: duplicate
/// `shortcut` / `alias` / `keyword` / `trigger` bindings) instead of
/// silently disappearing from the launcher. Backed by
/// [`crate::scripts::read_scripts_report`].
pub const FAILED_SCRIPTS_RESOURCE_URI: &str = "kit://failed-scripts";

/// Schema version for the `kit://failed-scripts` resource envelope.
pub const FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Pure readiness projection over already-loaded command snapshots.
pub const COMMAND_DOCTOR_RESOURCE_URI: &str = "kit://command-doctor";
pub const COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// URI for the `kit://script-templates` resource.
///
/// Surfaces curated starter templates for newly created scripts so the
/// launcher's template catalog and any MCP harness share one Rust-owned
/// source of truth. v1 templates intentionally omit collision-bearing
/// binding fields (`alias`, `shortcut`, `keyword`, `trigger`) so a
/// newly-created script cannot be immediately hidden by
/// [`crate::scripts::validation::validate_script_catalog`].
pub const SCRIPT_TEMPLATES_RESOURCE_URI: &str = "kit://script-templates";

/// Schema version for the `kit://script-templates` resource envelope.
pub const SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION: u32 = 1;

/// Schema-versioned envelope for script metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptsResourceDocument {
    pub schema_version: u32,
    pub count: usize,
    pub scripts: Vec<ScriptResourceEntry>,
}

/// Schema-versioned envelope for scriptlet metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptletsResourceDocument {
    pub schema_version: u32,
    pub count: usize,
    pub scriptlets: Vec<ScriptletResourceEntry>,
}

/// A single failed-script entry for the `kit://failed-scripts` resource.
///
/// Mirrors [`crate::scripts::FailedScript`] but uses `Vec` for the fatal-issue
/// list so the resource envelope round-trips cleanly through
/// `serde_json::from_str` without Arc-slice deserialization surprises.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailedScriptEntry {
    pub path: std::path::PathBuf,
    pub name: String,
    pub fatal: Vec<ScriptValidationIssue>,
}

impl From<&FailedScript> for FailedScriptEntry {
    fn from(failed: &FailedScript) -> Self {
        Self {
            path: failed.path.clone(),
            name: failed.name.clone(),
            fatal: failed.fatal.iter().cloned().collect(),
        }
    }
}

/// Schema-versioned envelope for the `kit://failed-scripts` resource.
///
/// Carries both an envelope `schema_version` (this document format) and the
/// inner `validation_schema_version` from [`crate::scripts::VALIDATION_SCHEMA_VERSION`]
/// so consumers can detect changes at either layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedScriptsResourceDocument {
    pub schema_version: u32,
    pub validation_schema_version: u32,
    pub total_candidates: usize,
    pub valid_count: usize,
    pub fatal_count: usize,
    pub warning_count: usize,
    pub failed_scripts: Vec<FailedScriptEntry>,
    pub warnings: Vec<ScriptValidationIssue>,
    /// Scriptlet issues stay separate: their rows remain present but disabled,
    /// so they must never be mislabeled excluded failures or warning-only.
    #[serde(default)]
    pub retained_issue_count: usize,
    #[serde(default)]
    pub retained_issues: Vec<ScriptValidationIssue>,
}

/// Stable, safe command readiness. Permission uncertainty is neither silently
/// granted nor mislabeled denied; callers must supply an explicit inventory.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CommandDoctorState {
    Ready,
    Experimental,
    Unsupported,
    Blocked,
    PermissionPending,
}

/// One explicitly declared API projected from the reviewed host catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDoctorCapability {
    pub name: String,
    pub support: SdkSupport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_platforms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_host_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

/// Safe action preview derived only from an actual canonical descriptor.
/// Identity is fingerprinted so the preview never repeats a raw command key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDoctorPrimaryAction {
    pub title: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub identity_fingerprint: String,
}

/// Read-only diagnostics for a real script or retained scriptlet snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDoctorEntry {
    pub source: String,
    pub name: String,
    pub path: String,
    pub plugin_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_title: Option<String>,
    pub state: CommandDoctorState,
    pub executable: bool,
    /// Present only when an existing canonical descriptor was explicitly
    /// supplied; bare catalog snapshots must never invent an action or label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_action: Option<CommandDoctorPrimaryAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CommandDoctorCapability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<ScriptValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

/// Deterministic command-doctor receipt. Building it never loads another
/// process, scans source bodies, probes permissions, or contacts a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandDoctorReport {
    pub schema_version: u32,
    pub host_version: String,
    pub platform: String,
    pub permission_inventory_known: bool,
    pub total_commands: usize,
    pub ready_count: usize,
    pub experimental_count: usize,
    pub unsupported_count: usize,
    pub blocked_count: usize,
    pub permission_pending_count: usize,
    pub commands: Vec<CommandDoctorEntry>,
}

/// GPUI support status for a single SDK function.
///
/// `Supported` is the default; absent JSON fields deserialize as
/// `Supported` so older clients that do not know about this enum
/// continue to round-trip. `Unsupported` entries are documented in
/// `scripts/kit-sdk.ts` and fail before dispatch or return an explicit
/// negative compatibility receipt. `Experimental` is reserved for partially implemented
/// APIs so the next marking wave does not require another schema bump.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SdkSupport {
    #[default]
    Supported,
    Unsupported,
    Experimental,
}

/// Execution topologies have different transport guarantees even when their
/// source language is identical.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SdkExecutionTopology {
    #[serde(rename = "typescript-script")]
    TypeScriptScript,
    #[serde(rename = "typescript-scriptlet")]
    TypeScriptScriptlet,
    /// Launcher-owned TypeScript scriptlet executed through the interactive
    /// runner, which supplies the same piped prompt transport as a script.
    #[serde(rename = "typescript-scriptlet-interactive")]
    TypeScriptScriptletInteractive,
    ShellScriptlet,
    PythonScriptlet,
}

/// Machine-readable author-facing compatibility failure.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SdkCapabilityDiagnosticCode {
    UnknownCapability,
    UnsupportedCapability,
    MissingSdkTransport,
    InteractivePromptUnavailable,
    UnsupportedPlatform,
    MissingPermission,
    PermissionInventoryUnavailable,
    HostVersionTooOld,
    InvalidHostVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkCapabilityDiagnostic {
    pub code: SdkCapabilityDiagnosticCode,
    pub capability: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
}

/// Explicit, read-only facts used to preflight a capability without probing
/// permissions, revealing the app, or requesting access on the author's behalf.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkHostAvailability {
    pub host_version: String,
    /// Uses `std::env::consts::OS` identifiers such as `macos` and `linux`.
    pub platform: String,
    #[serde(default)]
    pub granted_permissions: Vec<String>,
}

impl SdkHostAvailability {
    /// Construct host facts from process constants and permissions explicitly
    /// supplied by an existing, separately authorized permission inventory.
    pub fn current(granted_permissions: Vec<String>) -> Self {
        Self {
            host_version: env!("CARGO_PKG_VERSION").into(),
            platform: std::env::consts::OS.into(),
            granted_permissions,
        }
    }
}

/// Rich compatibility metadata derived from the same function rows displayed
/// in the launcher and published through kit://sdk-reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkCapability {
    pub name: String,
    pub support: SdkSupport,
    /// Earliest host version guaranteed to expose this reviewed capability
    /// contract; this is not a claim about historical feature introduction.
    pub minimum_host_version: String,
    pub requires_interactive_prompt: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_permissions: Vec<String>,
    /// Empty means no platform-specific restriction is currently declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_platforms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkCapabilityCatalog {
    pub schema_version: u32,
    pub host_version: String,
    pub capabilities: Vec<SdkCapability>,
}

/// A single SDK function reference entry.
///
/// `support` is always serialized so agents can rely on a stable
/// `"support": "supported" | "unsupported" | "experimental"` field
/// rather than inferring state from absence. `unsupported_note` is
/// skipped when `None` to keep the envelope lean for the common case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkFunctionRef {
    pub name: String,
    pub signature: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub support: SdkSupport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported_note: Option<String>,
}

impl SdkFunctionRef {
    fn supported(
        name: impl Into<String>,
        signature: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            signature: signature.into(),
            description: description.into(),
            category: category.into(),
            support: SdkSupport::Supported,
            unsupported_note: None,
        }
    }

    fn unsupported(
        name: impl Into<String>,
        signature: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            signature: signature.into(),
            description: description.into(),
            category: category.into(),
            support: SdkSupport::Unsupported,
            unsupported_note: Some(note.into()),
        }
    }

    fn experimental(
        name: impl Into<String>,
        signature: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            signature: signature.into(),
            description: description.into(),
            category: category.into(),
            support: SdkSupport::Experimental,
            unsupported_note: Some(note.into()),
        }
    }
}

/// SDK capabilities that reject before an unsupported native operation can be
/// dispatched. Every entry is included in the same host-owned SDK reference;
/// implemented prompt variants must never appear in this inventory. Consumed by
/// [`tests::sdk_reference_marks_every_documented_unsupported_api`]
/// and by the capability catalog consistency audit.
const SDK_NOT_YET_IMPLEMENTED_IN_GPUI: &[&str] = &[
    "setStatus",
    "keyboard",
    "keyboard.type",
    "keyboard.tap",
    "mouse",
    "mouse.move",
    "mouse.leftClick",
    "mouse.rightClick",
    "mouse.setPosition",
    "setPanel",
    "setPreview",
    "setPrompt",
    "widget",
    "find",
    "menu",
    "webcam",
    "mic",
    "eyeDropper",
];

/// Exact SDK feature identifiers that must fail before native dispatch. Kept
/// public so author validators and generation prompts can share the same
/// reviewed inventory as the SDK reference without parsing prose or source.
pub fn unsupported_sdk_capability_names() -> &'static [&'static str] {
    SDK_NOT_YET_IMPLEMENTED_IN_GPUI
}

/// Default explanation for "not yet implemented" SDK APIs. The
/// [`SdkFunctionRef::unsupported`] constructor takes a custom note so
/// a function can point the user at a working alternative — this
/// constant is exposed for tests that want to pin the generic wording.
#[allow(dead_code)]
const SDK_UNSUPPORTED_IN_GPUI_NOTE: &str = "Defined in scripts/kit-sdk.ts, but GPUI does not handle this behavior yet; the SDK fails explicitly instead of sending a misleading fire-and-forget message.";

/// Needles for scanning starter-template bodies for references to
/// unsupported SDK APIs. A template that contains any of these
/// substrings is rejected by
/// [`tests::script_templates_do_not_reference_unsupported_sdk_apis`].
#[cfg(test)]
fn unsupported_sdk_reference_scan_needles() -> Vec<String> {
    build_sdk_function_refs()
        .into_iter()
        .filter(|entry| entry.support == SdkSupport::Unsupported)
        .flat_map(|entry| {
            if entry.name.contains('.') {
                vec![format!("{}(", entry.name), format!("{}.", entry.name)]
            } else {
                vec![format!("{}(", entry.name)]
            }
        })
        .collect()
}

/// Mandatory Bun verification contract for final user-authored scripts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessVerificationContract {
    /// Whether verification is mandatory before the agent can report success.
    pub required: bool,
    /// Canonical skill file that defines the verification loop.
    pub skill_path: String,
    /// Exact Bun syntax-check / transpile command for the final script.
    pub build_command: String,
    /// Exact Bun execution command for the final script.
    pub run_command: String,
    /// Observable result the agent must confirm after execution.
    pub success_criteria: String,
    /// What the agent must do if either Bun command fails.
    pub failure_policy: String,
}

/// Describes how a harness can create and verify scripts non-interactively.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessWorkflow {
    /// Dedicated directory for test/temp scripts that won't pollute the user's
    /// main `~/.scriptkit/plugins/main/scripts/` collection.
    pub test_script_directory: String,
    /// Dedicated directory for test scriptlet extension files.
    pub test_scriptlet_directory: String,
    /// Shell command to execute a script via the app stdin bridge.
    /// The harness replaces `{path}` with the absolute script path.
    pub run_command: String,
    /// JSONL message the app sends to its stdin to trigger a script run.
    /// Harnesses that communicate over the stdin bridge use this shape.
    pub stdin_run_message: String,
    /// Shape of a successful execution result on stdout (JSONL).
    pub success_output_shape: String,
    /// Shape of an error execution result on stdout (JSONL).
    pub error_output_shape: String,
    /// Mandatory Bun verification contract for the final user-authored script.
    pub verification: HarnessVerificationContract,
    /// Example minimal test script content (TypeScript).
    pub example_test_script: String,
    /// Example scriptlet (Markdown) content.
    pub example_scriptlet: String,
}

/// Schema-versioned SDK reference document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkAuthoringResource {
    /// Read-only MCP resource URI; this is not a callable SDK global.
    pub uri: String,
    pub name: String,
    pub description: String,
}

/// Schema-versioned SDK reference document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SdkReferenceDocument {
    pub schema_version: u32,
    pub sdk_package: String,
    pub script_directory: String,
    pub scriptlet_pattern: String,
    pub metadata_format: String,
    pub functions: Vec<SdkFunctionRef>,
    /// Versioned compatibility, permission, platform, and transport contract.
    #[serde(default)]
    pub capability_catalog: SdkCapabilityCatalog,
    /// Read-only host diagnostics; these resource URIs are not SDK functions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoring_resources: Vec<SdkAuthoringResource>,
    /// Non-interactive workflow for harness-driven script creation and execution.
    pub harness_workflow: HarnessWorkflow,
}

/// Optional metadata defaults written into a newly-created script.
///
/// v1 templates intentionally omit collision-bearing binding fields
/// (`alias`, `shortcut`, `keyword`, `trigger`) — those are what
/// [`crate::scripts::validation::detect_binding_collisions`] uses to
/// fatally exclude duplicates. A starter script should never land on
/// disk in a hidden state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateMetadataDefaults {
    /// `description:` value in the `export const metadata = { … }` block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A single starter-script template.
///
/// The `body_template` body is a fully-formed TypeScript file with a
/// `{{NAME}}` placeholder substituted by [`render_script_template_file`]
/// at write time — so the `metadata.name` in the on-disk file matches
/// the friendly name the user typed into the naming prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateRef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub filename_hint: String,
    pub body_template: String,
    #[serde(default)]
    pub metadata_defaults: ScriptTemplateMetadataDefaults,
}

/// Schema-versioned envelope for `kit://script-templates`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplatesResourceDocument {
    pub schema_version: u32,
    pub count: usize,
    pub templates: Vec<ScriptTemplateRef>,
}

// ---------------------------------------------------------------
// Clipboard history resource types
// ---------------------------------------------------------------

/// A single clipboard history entry in the MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryEntry {
    pub id: String,
    pub content_type: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_height: Option<u32>,
    pub pinned: bool,
}

/// Schema-versioned envelope for clipboard history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryDocument {
    pub schema_version: u32,
    pub count: usize,
    pub entries: Vec<ClipboardHistoryEntry>,
}

/// Diagnostics wrapper for clipboard history.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ClipboardHistoryDiagnosticsDocument {
    kind: &'static str,
    uri: String,
    document: ClipboardHistoryDocument,
    meta: ClipboardHistoryDiagnosticsMeta,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ClipboardHistoryDiagnosticsMeta {
    duration_ms: u128,
    entry_count: usize,
    source: &'static str,
}

// ---------------------------------------------------------------
// Focused item resource types
// ---------------------------------------------------------------

/// The focused/selected item from the active surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusedItemInfo {
    pub source: String,
    pub kind: String,
    pub semantic_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Schema-versioned envelope for focused item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusedItemDocument {
    pub schema_version: u32,
    pub has_focused_item: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_item: Option<FocusedItemInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Diagnostics wrapper for focused item.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct FocusedItemDiagnosticsDocument {
    kind: &'static str,
    uri: String,
    document: FocusedItemDocument,
    meta: FocusedItemDiagnosticsMeta,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct FocusedItemDiagnosticsMeta {
    duration_ms: u128,
    has_focused_item: bool,
    warning_count: usize,
    source: String,
}

fn build_sdk_function_refs() -> Vec<SdkFunctionRef> {
    let mut functions = vec![
        SdkFunctionRef::supported(
            "arg",
            "await arg(prompt?: string | ArgConfig, choices?: ChoicesInput, actions?: Action[]): Promise<string>",
            "Prompt the user with an input field, optionally with a list of choices.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "div",
            "await div(html?: string | DivConfig, actions?: Action[]): Promise<string | void>",
            "Display HTML content in a panel.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "editor",
            "await editor(content?: string, language?: string, actions?: Action[]): Promise<string>",
            "Open a full-screen code editor and return the content.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "mini",
            "await mini(placeholder: string, choices: (string | Choice)[]): Promise<string>",
            "Show the native compact-choice prompt and return the selected value.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "micro",
            "await micro(placeholder: string, choices: (string | Choice)[]): Promise<string>",
            "Show the native minimal-choice prompt and return the selected value.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "hotkey",
            "await hotkey(placeholder?: string): Promise<HotkeyInfo>",
            "Capture a keyboard shortcut using the native shortcut-recorder prompt.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "fields",
            "await fields(definitions: (string | FieldDef)[], actions?: Action[]): Promise<string[]>",
            "Show a native multi-field prompt and return field values in definition order.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "form",
            "await form(html: string, actions?: Action[]): Promise<Record<string, string>>",
            "Parse supported HTML form controls into the native form prompt.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "select",
            "await select(placeholder: string, choices: (string | Choice)[]): Promise<string[]>",
            "Show the native multi-select prompt and return its selected values.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "path",
            "await path(options?: PathOptions): Promise<string>",
            "Show the native path prompt and return the selected filesystem path.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "term",
            "await term(command?: string, actions?: Action[]): Promise<string>",
            "Open an interactive terminal, optionally running a command.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "drop",
            "await drop(): Promise<FileInfo[]>",
            "Accept drag-and-drop files from the user.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "template",
            "await template(template: string, options?: { language?: string }): Promise<string>",
            "Fill in a template string with user-provided values.",
            "prompts",
        ),
        SdkFunctionRef::supported(
            "exec",
            "await exec(command: string, args?: readonly string[]): Promise<ExecResult>",
            "Execute an explicitly named subprocess without invoking a shell; return stdout, stderr, and exitCode or a typed execution failure.",
            "system",
        ),
        SdkFunctionRef::supported(
            "clipboard",
            "clipboard: ClipboardApi",
            "Read and write text/images through the host-owned clipboard namespace.",
            "clipboard",
        ),
        SdkFunctionRef::supported(
            "copy",
            "await copy(text: string): Promise<void>",
            "Copy text to the clipboard.",
            "clipboard",
        ),
        SdkFunctionRef::supported(
            "paste",
            "await paste(): Promise<string>",
            "Read current clipboard text; this compatibility alias does not inject global input.",
            "clipboard",
        ),
        SdkFunctionRef::supported(
            "notify",
            "await notify(message: string | { title?: string; body?: string }): Promise<SystemFeedbackResult>",
            "Request an OS-level system notification (macOS Notification Center). Returns a dispatch receipt; delivery remains OS dependent. Distinct from hud(message), which is an in-launcher overlay.",
            "feedback",
        ),
        SdkFunctionRef::experimental(
            "beep",
            "await beep(): Promise<SystemFeedbackResult>",
            "Request a macOS system beep through afplay.",
            "feedback",
            "beep() returns a dispatch receipt when the feedback process is spawned; audible delivery is not verified and non-macOS platforms return unsupported.",
        ),
        SdkFunctionRef::experimental(
            "say",
            "await say(text: string, voice?: string): Promise<SystemFeedbackResult>",
            "Request macOS text-to-speech through the say command.",
            "feedback",
            "say() returns a dispatch receipt when the feedback process is spawned; speech delivery is not verified and non-macOS platforms return unsupported.",
        ),
        SdkFunctionRef::supported(
            "setSelectedText",
            "await setSelectedText(text: string): Promise<void>",
            "Replace the selected text in the focused application.",
            "system",
        ),
        SdkFunctionRef::supported(
            "getSelectedText",
            "await getSelectedText(): Promise<string>",
            "Read the selected text from the focused application.",
            "system",
        ),
        SdkFunctionRef::supported(
            "readFile",
            "await readFile(path: string, encoding?: BufferEncoding): Promise<string>",
            "Read text from an explicitly named filesystem path (UTF-8 by default).",
            "filesystem",
        ),
        SdkFunctionRef::supported(
            "writeFile",
            "await writeFile(path: string, content: string, encoding?: BufferEncoding): Promise<void>",
            "Write text to an explicitly named filesystem path (UTF-8 by default).",
            "filesystem",
        ),
        SdkFunctionRef::supported(
            "home",
            "home(...paths: string[]): string",
            "Resolve a path relative to the user's home directory.",
            "filesystem",
        ),
        SdkFunctionRef::supported(
            "fileSearch",
            "await fileSearch(query: string, options?: FindOptions): Promise<FileSearchResult[]>",
            "Search indexed files without opening the unsupported legacy find prompt.",
            "filesystem",
        ),
        SdkFunctionRef::supported(
            "getWindows",
            "await getWindows(): Promise<SystemWindowInfo[]>",
            "List observed native windows without activating, moving, or resizing them.",
            "window-management",
        ),
        SdkFunctionRef::supported(
            "focusWindow",
            "await focusWindow(windowId: number): Promise<void>",
            "Focus an explicitly identified native window; stale window IDs reject.",
            "window-management",
        ),
        SdkFunctionRef::supported(
            "moveWindow",
            "await moveWindow(windowId: number, x: number, y: number): Promise<void>",
            "Move an explicitly identified native window; stale window IDs reject.",
            "window-management",
        ),
        SdkFunctionRef::supported(
            "resizeWindow",
            "await resizeWindow(windowId: number, width: number, height: number): Promise<void>",
            "Resize an explicitly identified native window; stale window IDs reject.",
            "window-management",
        ),
        SdkFunctionRef::supported(
            "tileWindow",
            "await tileWindow(windowId: number, position: TilePosition): Promise<void>",
            "Tile an explicitly identified native window; stale window IDs reject.",
            "window-management",
        ),
        SdkFunctionRef::unsupported(
            "find",
            "await find(placeholder: string, options?: FindOptions): Promise<never>",
            "Legacy interactive find prompt. GPUI does not currently implement a Rust find prompt route, renderer, submit contract, or onlyin prompt semantics.",
            "filesystem",
            "Use fileSearch(query, { onlyin }) for non-interactive Spotlight/mdfind results, or path({ startPath }) / arg(...) for supported prompt-driven selection.",
        ),
        SdkFunctionRef::supported(
            "getState",
            "await getState(): Promise<PromptState>",
            "Read the current Script Kit prompt state without mutating the UI.",
            "automation",
        ),
        SdkFunctionRef::supported(
            "getElements",
            "await getElements(limit?: number): Promise<ElementsSnapshot>",
            "Return visible UI elements with semantic IDs, focus, selection, truncation, and warnings.",
            "automation",
        ),
        SdkFunctionRef::supported(
            "waitFor",
            "await waitFor(condition: WaitCondition, options?: WaitForOptions): Promise<WaitForResult>",
            "Poll until a UI condition is satisfied or the timeout expires. Returns { success, elapsed, error?, trace? }. On failure, error contains a stable code (wait_condition_timeout | element_not_found | unsupported_prompt | action_failed), a human message, and an optional suggestion. Pass trace: 'onFailure' in options to get poll-by-poll diagnostics on timeout.",
            "automation",
        ),
        SdkFunctionRef::supported(
            "batch",
            "await batch(commands: BatchCommand[], options?: BatchOptions): Promise<BatchResult>",
            "Execute a deterministic sequence of UI commands. Returns { success, results, failedAt?, totalElapsed, trace? }. Each result entry includes index, success, command, elapsed, value?, and a structured error with stable code on failure. Pass trace: 'onFailure' at the top-level message (not inside options) for per-command diagnostics. Error codes: wait_condition_timeout, element_not_found, selection_not_found, unsupported_command, unsupported_prompt, action_failed.",
            "automation",
        ),
        SdkFunctionRef::supported(
            "computer.listNativeWindows",
            "await computer.listNativeWindows(options?: ComputerUseListNativeWindowsOptions): Promise<ComputerUseListNativeWindowsResult>",
            "List native macOS windows grouped by running app through Script Kit's own local MCP server. Observation-only: does not focus, activate, move, resize, capture screenshots, or send input.",
            "computer-use",
        ),
        SdkFunctionRef::supported(
            "computer.captureNativeWindow",
            "await computer.captureNativeWindow(options: ComputerUseCaptureNativeWindowOptions): Promise<ComputerUseCaptureNativeWindowResult>",
            "Capture one exact native macOS window after PID/nativeWindowId ownership and capture-candidate validation. Returns the structured computer/capture_native_window receipt, optionally including pngBase64 when includeImage is true.",
            "computer-use",
        ),
        SdkFunctionRef::unsupported(
            "setStatus",
            "await setStatus(options: { status: 'busy' | 'idle' | 'error'; message: string }): Promise<SystemFeedbackResult>",
            "Return an explicit unsupported receipt; GPUI has no application-status surface.",
            "feedback",
            "setStatus(...) currently has no visible GPUI status surface or receipt. The SDK returns ERR_UNSUPPORTED_SDK_FEATURE before sending; use hud(message) for visible feedback, or render progress in a prompt.",
        ),
        SdkFunctionRef::unsupported(
            "menu",
            "await menu(icon: string, scripts?: string[]): Promise<SystemFeedbackResult>",
            "Return an explicit unsupported receipt; GPUI cannot mutate the tray/menu.",
            "system",
            "menu(...) currently has no GPUI tray/menu mutation handler. The SDK returns ERR_UNSUPPORTED_SDK_FEATURE before sending; use the built-in tray icon (System Actions) or prompt-scoped setActions(...) today.",
        ),
        SdkFunctionRef::unsupported(
            "setPanel",
            "setPanel(html: string): never",
            "Legacy panel mutation is not handled by the GPUI prompt host.",
            "prompt-control",
            "Use div(html), an arg(...) preview, or prompt-scoped setActions(...).",
        ),
        SdkFunctionRef::unsupported(
            "setPreview",
            "setPreview(html: string): never",
            "Legacy preview mutation is not handled by the GPUI prompt host.",
            "prompt-control",
            "Supply preview content through an arg(...) choice or display it with div(html).",
        ),
        SdkFunctionRef::unsupported(
            "setPrompt",
            "setPrompt(html: string): never",
            "Legacy prompt mutation is not handled by the GPUI prompt host.",
            "prompt-control",
            "Open a supported arg(...), div(...), fields(...), or editor(...) prompt.",
        ),
        SdkFunctionRef::unsupported(
            "widget",
            "await widget(html: string, options?: WidgetOptions): Promise<never>",
            "Floating HTML widgets have no native GPUI surface, event owner, or delivery receipt.",
            "prompts",
            "Use div(html) for an in-launcher surface or a supported native prompt.",
        ),
        SdkFunctionRef::unsupported(
            "keyboard.type",
            "await keyboard.type(text: string): Promise<never>",
            "Global keyboard injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use batch setInput plus getState/getElements/waitFor for prompt text.",
        ),
        SdkFunctionRef::unsupported(
            "keyboard",
            "keyboard: KeyboardApi",
            "The keyboard namespace exists for compatibility, but global input injection is unsupported.",
            "system-input",
            "Use batch setInput/forceSubmit and semantic action APIs instead of global keyboard injection.",
        ),
        SdkFunctionRef::unsupported(
            "keyboard.tap",
            "await keyboard.tap(...keys: string[]): Promise<never>",
            "Global keyboard injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use batch forceSubmit or semantic action APIs instead of global key injection.",
        ),
        SdkFunctionRef::unsupported(
            "mouse.move",
            "await mouse.move(positions: Position[]): Promise<never>",
            "Global pointer injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use semantic action APIs or batch/getState/getElements/waitFor.",
        ),
        SdkFunctionRef::unsupported(
            "mouse",
            "mouse: MouseApi",
            "The mouse namespace exists for compatibility, but global pointer injection is unsupported.",
            "system-input",
            "Use semantic action APIs or batch/getState/getElements/waitFor instead of global pointer injection.",
        ),
        SdkFunctionRef::unsupported(
            "mouse.leftClick",
            "await mouse.leftClick(): Promise<never>",
            "Global pointer injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use semantic action APIs instead of coordinate clicks.",
        ),
        SdkFunctionRef::unsupported(
            "mouse.rightClick",
            "await mouse.rightClick(): Promise<never>",
            "Global pointer injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use semantic action APIs instead of coordinate clicks.",
        ),
        SdkFunctionRef::unsupported(
            "mouse.setPosition",
            "await mouse.setPosition(position: Position): Promise<never>",
            "Global pointer injection has no explicit target, permission, or delivery receipt.",
            "system-input",
            "Use semantic action APIs instead of coordinate positioning.",
        ),
        SdkFunctionRef::unsupported(
            "webcam",
            "await webcam(): Promise<never>",
            "The JSONL SDK transport cannot stream camera frames.",
            "media",
            "Use an explicitly configured external camera tool and process its saved output.",
        ),
        SdkFunctionRef::unsupported(
            "mic",
            "await mic(): Promise<never>",
            "The JSONL SDK transport cannot stream microphone audio.",
            "media",
            "Use an explicitly configured external audio tool and process its saved output.",
        ),
        SdkFunctionRef::unsupported(
            "eyeDropper",
            "await eyeDropper(): Promise<never>",
            "Native color picking does not have a supported permission and capture contract.",
            "media",
            "Use a supported input prompt to request a color value explicitly.",
        ),
    ];

    // Keep executable global functions and object methods visible to authors.
    // These entries extend the reviewed rows above rather than creating a
    // parallel capabilities registry.
    functions.extend(
        [
            (
                "confirm",
                "await confirm(message?: string | ConfirmConfig): Promise<boolean>",
                "Show a native confirmation prompt and return the user's explicit choice.",
                "prompts",
            ),
            (
                "chat",
                "await chat(options?: ChatOptions): Promise<ChatResult>",
                "Open an inline conversation UI; the calling script owns provider requests and this prompt never starts built-in inference.",
                "prompts",
            ),
            (
                "chat.addMessage",
                "chat.addMessage(message: ChatMessage | CoreMessage): void",
                "Append a message to an already-active inline chat prompt; rejects outside a chat session and never requests provider inference.",
                "prompt-control",
            ),
            (
                "chat.startStream",
                "chat.startStream(position?: 'left' | 'right'): string",
                "Start a script-owned streaming UI message in an already-active inline chat prompt; never starts provider inference.",
                "prompt-control",
            ),
            (
                "chat.appendChunk",
                "chat.appendChunk(messageId: string, chunk: string): void",
                "Append an explicitly supplied UI text chunk to an active inline-chat stream; no provider request is made.",
                "prompt-control",
            ),
            (
                "chat.completeStream",
                "chat.completeStream(messageId: string): void",
                "Mark an active inline-chat UI stream complete without contacting an AI provider.",
                "prompt-control",
            ),
            (
                "chat.clear",
                "chat.clear(): void",
                "Clear messages from an already-active inline chat prompt; rejects outside a chat session.",
                "prompt-control",
            ),
            (
                "chat.setError",
                "chat.setError(messageId: string, error: string): void",
                "Attach an explicit script-owned error to an active inline-chat message without provider inference.",
                "prompt-control",
            ),
            (
                "chat.clearError",
                "chat.clearError(messageId: string): void",
                "Clear an inline-chat message error in an already-active chat session.",
                "prompt-control",
            ),
            (
                "chat.getMessages",
                "chat.getMessages(): CoreMessage[]",
                "Read script-local inline-chat message state without opening a prompt, sending protocol traffic, or requesting inference.",
                "ai-context",
            ),
            (
                "chat.getResult",
                "chat.getResult(): ChatResult",
                "Read the script-local inline-chat result snapshot without opening a prompt or requesting provider inference.",
                "ai-context",
            ),
            (
                "env",
                "await env(key: string, config?: string | EnvConfig | (() => Promise<string>)): Promise<string>",
                "Read an existing environment value or prompt explicitly when it is absent.",
                "prompts",
            ),
            (
                "md",
                "md(markdown: string): string",
                "Convert Markdown into displayable HTML without dispatching host commands.",
                "utilities",
            ),
            (
                "hud",
                "hud(message: string, options?: { duration?: number }): void",
                "Show a transient in-launcher status message without claiming OS notification delivery.",
                "feedback",
            ),
            (
                "setActions",
                "await setActions(actions: Action[]): Promise<void>",
                "Register executable actions for the currently active prompt.",
                "prompt-control",
            ),
            (
                "setInput",
                "setInput(text: string): void",
                "Update the active prompt input through its scoped SDK protocol.",
                "prompt-control",
            ),
            (
                "submit",
                "submit(value: unknown): void",
                "Submit the current prompt with an explicit value.",
                "prompt-control",
            ),
            (
                "exit",
                "exit(code?: number): void",
                "Terminate the current script with an explicit exit status.",
                "script-lifecycle",
            ),
            (
                "hasAccessibilityPermission",
                "await hasAccessibilityPermission(): Promise<boolean>",
                "Read current accessibility authorization without prompting for access.",
                "permissions",
            ),
            (
                "requestAccessibilityPermission",
                "await requestAccessibilityPermission(): Promise<boolean>",
                "Explicitly request accessibility access; may open system privacy settings.",
                "permissions",
            ),
            (
                "clipboard.readText",
                "await clipboard.readText(): Promise<string>",
                "Read current clipboard text through the host clipboard bridge.",
                "clipboard",
            ),
            (
                "clipboard.writeText",
                "await clipboard.writeText(text: string): Promise<void>",
                "Replace clipboard text through the host clipboard bridge.",
                "clipboard",
            ),
            (
                "clipboard.readImage",
                "await clipboard.readImage(): Promise<Buffer>",
                "Read PNG clipboard bytes or reject with a typed clipboard failure.",
                "clipboard",
            ),
            (
                "clipboard.writeImage",
                "await clipboard.writeImage(buffer: Buffer): Promise<void>",
                "Write image bytes only after the host confirms clipboard delivery.",
                "clipboard",
            ),
            (
                "clipboardHistory",
                "await clipboardHistory(): Promise<ClipboardHistoryEntry[]>",
                "Return available clipboard-history entries without modifying them.",
                "clipboard",
            ),
            (
                "clipboardHistoryPin",
                "await clipboardHistoryPin(entryId: string): Promise<void>",
                "Pin an explicitly identified clipboard-history entry.",
                "clipboard",
            ),
            (
                "clipboardHistoryUnpin",
                "await clipboardHistoryUnpin(entryId: string): Promise<void>",
                "Unpin an explicitly identified clipboard-history entry.",
                "clipboard",
            ),
            (
                "clipboardHistoryRemove",
                "await clipboardHistoryRemove(entryId: string): Promise<void>",
                "Remove an explicitly identified clipboard-history entry.",
                "clipboard",
            ),
            (
                "clipboardHistoryClear",
                "await clipboardHistoryClear(): Promise<void>",
                "Clear non-pinned clipboard-history entries while preserving pinned entries.",
                "clipboard",
            ),
            (
                "clipboardHistoryTrimOversize",
                "await clipboardHistoryTrimOversize(): Promise<void>",
                "Remove clipboard-history entries exceeding the configured maximum size.",
                "clipboard",
            ),
            (
                "show",
                "await show(): Promise<void>",
                "Explicitly reveal the Script Kit prompt window.",
                "window-control",
            ),
            (
                "hide",
                "await hide(): Promise<void>",
                "Explicitly hide the Script Kit prompt window.",
                "window-control",
            ),
            (
                "blur",
                "await blur(): Promise<void>",
                "Return focus from Script Kit to the previously focused application.",
                "window-control",
            ),
            (
                "showGrid",
                "await showGrid(options?: GridOptions): Promise<void>",
                "Show the prompt debug-grid overlay for an explicitly requested inspection.",
                "window-control",
            ),
            (
                "hideGrid",
                "await hideGrid(): Promise<void>",
                "Hide the prompt debug-grid overlay.",
                "window-control",
            ),
            (
                "getWindowBounds",
                "await getWindowBounds(): Promise<WindowBounds>",
                "Read the current Script Kit window bounds.",
                "window-control",
            ),
            (
                "captureScreenshot",
                "await captureScreenshot(options?: ScreenshotOptions): Promise<ScreenshotData>",
                "Capture the current Script Kit window only after explicit authorization.",
                "window-control",
            ),
            (
                "getLayoutInfo",
                "await getLayoutInfo(): Promise<LayoutInfo>",
                "Inspect the current prompt layout without screen capture.",
                "window-control",
            ),
            (
                "closeWindow",
                "await closeWindow(windowId: number): Promise<void>",
                "Close an explicitly identified native window; stale IDs reject.",
                "window-management",
            ),
            (
                "minimizeWindow",
                "await minimizeWindow(windowId: number): Promise<void>",
                "Minimize an explicitly identified native window; stale IDs reject.",
                "window-management",
            ),
            (
                "maximizeWindow",
                "await maximizeWindow(windowId: number): Promise<void>",
                "Maximize an explicitly identified native window; stale IDs reject.",
                "window-management",
            ),
            (
                "getDisplays",
                "await getDisplays(): Promise<DisplayInfo[]>",
                "Read connected display geometry without moving or focusing a window.",
                "window-management",
            ),
            (
                "getFrontmostWindow",
                "await getFrontmostWindow(): Promise<SystemWindowInfo | null>",
                "Read the previously active application's frontmost window.",
                "window-management",
            ),
            (
                "moveToNextDisplay",
                "await moveToNextDisplay(windowId: number): Promise<void>",
                "Move an explicitly identified window to the next display; stale IDs reject.",
                "window-management",
            ),
            (
                "moveToPreviousDisplay",
                "await moveToPreviousDisplay(windowId: number): Promise<void>",
                "Move an explicitly identified window to the previous display; stale IDs reject.",
                "window-management",
            ),
            (
                "getMenuBar",
                "await getMenuBar(bundleId?: string): Promise<MenuBarItem[]>",
                "Read native menu structure without selecting a menu action.",
                "menu-bar",
            ),
            (
                "executeMenuAction",
                "await executeMenuAction(bundleId: string, menuPath: string[]): Promise<void>",
                "Execute one explicitly targeted native menu action and reject invalid targets.",
                "menu-bar",
            ),
            (
                "aiIsOpen",
                "await aiIsOpen(): Promise<{ isOpen: boolean; activeChatId?: string }>",
                "Read Agent Chat visibility and active conversation identity without starting a provider request.",
                "ai-context",
            ),
            (
                "aiGetActiveChat",
                "await aiGetActiveChat(): Promise<AiChatInfo | null>",
                "Read the active Agent Chat conversation without triggering inference.",
                "ai-context",
            ),
            (
                "aiListChats",
                "await aiListChats(limit?: number, includeDeleted?: boolean): Promise<AiChatInfo[]>",
                "Read conversation metadata from Agent Chat storage without triggering inference.",
                "ai-context",
            ),
            (
                "aiGetConversation",
                "await aiGetConversation(chatId?: string, limit?: number): Promise<AiMessageInfo[]>",
                "Read explicitly selected conversation messages without triggering inference.",
                "ai-context",
            ),
            (
                "aiStartChat",
                "await aiStartChat(message: string, options?: AiChatOptions): Promise<AiStartChatResult>",
                "Start an Agent Chat conversation with explicitly declared context parts; noResponse controls whether inference begins.",
                "ai-interaction",
            ),
            (
                "aiAppendMessage",
                "await aiAppendMessage(chatId: string, content: string, role: 'user' | 'assistant' | 'system'): Promise<string>",
                "Append an explicitly scoped conversation message without triggering provider inference.",
                "ai-context",
            ),
            (
                "aiSendMessage",
                "await aiSendMessage(chatId: string, content: string, imagePath?: string, parts?: AiContextPartInput[]): Promise<{ userMessageId: string; streamingStarted: boolean }>",
                "Submit explicit text/context to one conversation and request provider inference.",
                "ai-interaction",
            ),
            (
                "aiSetSystemPrompt",
                "await aiSetSystemPrompt(chatId: string, prompt: string): Promise<void>",
                "Update one explicitly identified conversation's system prompt.",
                "ai-context",
            ),
            (
                "aiFocus",
                "await aiFocus(): Promise<{ wasOpen: boolean }>",
                "Explicitly reveal/focus Agent Chat without implicitly submitting a provider request.",
                "ai-interaction",
            ),
            (
                "aiGetStreamingStatus",
                "await aiGetStreamingStatus(chatId?: string): Promise<{ isStreaming: boolean; chatId?: string; partialContent?: string }>",
                "Read one conversation's streaming state without initiating inference.",
                "ai-context",
            ),
            (
                "aiDeleteChat",
                "await aiDeleteChat(chatId: string, permanent?: boolean): Promise<void>",
                "Delete an explicitly identified conversation; permanent removal must be requested.",
                "ai-context",
            ),
            (
                "aiOn",
                "await aiOn(eventType: AiEventType, handler: AiEventHandler, chatId?: string): Promise<() => void>",
                "Subscribe to explicitly selected Agent Chat events and receive an unsubscribe callback.",
                "ai-context",
            ),
            (
                "uuid",
                "uuid(): string",
                "Generate a cryptographically random UUID without host dispatch.",
                "utilities",
            ),
            (
                "compile",
                "compile(template: string): (values: Record<string, unknown>) => string",
                "Compile a local string-interpolation template without host dispatch.",
                "utilities",
            ),
            (
                "skPath",
                "skPath(...segments: string[]): string",
                "Resolve a path inside the Script Kit home directory.",
                "filesystem",
            ),
            (
                "kitPath",
                "kitPath(...segments: string[]): string",
                "Compatibility alias for a path inside the Script Kit home directory.",
                "filesystem",
            ),
            (
                "tmpPath",
                "tmpPath(...segments: string[]): string",
                "Resolve a path inside the Script Kit temporary directory.",
                "filesystem",
            ),
            (
                "isFile",
                "await isFile(path: string): Promise<boolean>",
                "Inspect whether an explicit path currently names a regular file.",
                "filesystem",
            ),
            (
                "isDir",
                "await isDir(path: string): Promise<boolean>",
                "Inspect whether an explicit path currently names a directory.",
                "filesystem",
            ),
            (
                "isBin",
                "await isBin(path: string): Promise<boolean>",
                "Inspect whether an explicit path currently names an executable file.",
                "filesystem",
            ),
            (
                "memoryMap",
                "memoryMap: MemoryMapAPI",
                "Access the script-local in-memory key/value namespace.",
                "utilities",
            ),
            (
                "memoryMap.get",
                "memoryMap.get(key: string): unknown",
                "Read a script-local in-memory value.",
                "utilities",
            ),
            (
                "memoryMap.set",
                "memoryMap.set(key: string, value: unknown): void",
                "Store a script-local in-memory value.",
                "utilities",
            ),
            (
                "memoryMap.delete",
                "memoryMap.delete(key: string): boolean",
                "Delete a script-local in-memory value.",
                "utilities",
            ),
            (
                "memoryMap.clear",
                "memoryMap.clear(): void",
                "Clear script-local in-memory values.",
                "utilities",
            ),
            (
                "browse",
                "await browse(url: string): Promise<void>",
                "Explicitly request that the host open one URL in the default browser.",
                "system",
            ),
            (
                "editFile",
                "await editFile(path: string): Promise<void>",
                "Explicitly request that the host open one path in its configured editor.",
                "system",
            ),
            (
                "run",
                "await run(scriptName: string, ...args: string[]): Promise<unknown>",
                "Invoke another explicitly identified Script Kit script and await its result.",
                "script-lifecycle",
            ),
            (
                "inspect",
                "await inspect(data: unknown): Promise<void>",
                "Send an explicit value to the host inspector.",
                "prompt-control",
            ),
            (
                "defineSchema",
                "defineSchema<T extends ScriptSchema>(schema: T): TypedSchemaAPI<InferInput<T>, InferOutput<T>>",
                "Declare a typed local script input/output schema without starting an AI request.",
                "script-lifecycle",
            ),
            (
                "input",
                "await input<T extends Record<string, unknown>>(): Promise<T>",
                "Receive explicit typed input supplied to the current script.",
                "script-lifecycle",
            ),
            (
                "output",
                "output(data: Record<string, unknown>): void",
                "Publish explicit typed output from the current script.",
                "script-lifecycle",
            ),
            (
                "mcp",
                "mcp: McpApi",
                "Access explicitly configured MCP server discovery and tool-call methods.",
                "mcp",
            ),
            (
                "mcp.listServers",
                "await mcp.listServers(): Promise<McpServerInfo[]>",
                "List locally configured, enabled MCP servers without calling their tools.",
                "mcp",
            ),
            (
                "mcp.getServer",
                "await mcp.getServer(id: string): Promise<McpServerInfo | null>",
                "Inspect one explicitly identified MCP server configuration.",
                "mcp",
            ),
            (
                "mcp.listTools",
                "await mcp.listTools(serverId?: string): Promise<McpToolInfo[]>",
                "Discover tools from explicitly configured MCP servers.",
                "mcp",
            ),
            (
                "mcp.discover",
                "await mcp.discover(query: string): Promise<McpToolInfo[]>",
                "Find configured MCP tools matching an explicit search query.",
                "mcp",
            ),
            (
                "mcp.call",
                "await mcp.call(serverId: string, toolName: string, args?: Record<string, unknown>): Promise<McpToolCallResult>",
                "Call one explicitly selected MCP server/tool with explicit arguments.",
                "mcp",
            ),
            (
                "computer",
                "computer: ComputerUseApi",
                "Access explicitly permission-scoped native window observation tools.",
                "computer-use",
            ),
        ]
        .into_iter()
        .map(|(name, signature, description, category)| {
            SdkFunctionRef::supported(name, signature, description, category)
        }),
    );

    functions
}

fn sdk_capability_from_function(entry: &SdkFunctionRef) -> SdkCapability {
    let required_permissions = match entry.name.as_str() {
        "getSelectedText"
        | "setSelectedText"
        | "focusWindow"
        | "moveWindow"
        | "resizeWindow"
        | "tileWindow"
        | "closeWindow"
        | "minimizeWindow"
        | "maximizeWindow"
        | "moveToNextDisplay"
        | "moveToPreviousDisplay"
        | "getMenuBar"
        | "executeMenuAction" => vec!["accessibility".into()],
        "captureScreenshot" => vec!["screen-recording".into()],
        "computer.captureNativeWindow" => {
            vec!["accessibility".into(), "screen-recording".into()]
        }
        _ => Vec::new(),
    };

    let supported_platforms = match entry.name.as_str() {
        "notify"
        | "beep"
        | "say"
        | "getSelectedText"
        | "setSelectedText"
        | "hasAccessibilityPermission"
        | "requestAccessibilityPermission"
        | "getWindows"
        | "focusWindow"
        | "moveWindow"
        | "resizeWindow"
        | "tileWindow"
        | "closeWindow"
        | "minimizeWindow"
        | "maximizeWindow"
        | "getDisplays"
        | "getFrontmostWindow"
        | "moveToNextDisplay"
        | "moveToPreviousDisplay"
        | "getMenuBar"
        | "executeMenuAction"
        | "computer"
        | "computer.listNativeWindows"
        | "computer.captureNativeWindow" => {
            vec!["macos".into()]
        }
        _ => Vec::new(),
    };

    let alternatives = match entry.name.as_str() {
        "find" => vec!["fileSearch(query, { onlyin })", "path({ startPath })"],
        "setStatus" => vec!["hud(message)", "div(html)"],
        "menu" => vec!["built-in System Actions", "setActions(actions)"],
        "setPanel" | "setPreview" => vec!["div(html)", "arg(placeholder, choices)"],
        "setPrompt" => vec!["arg(placeholder, choices)", "fields(definitions)"],
        "widget" => vec!["div(html)", "arg(placeholder, choices)"],
        "keyboard" | "keyboard.type" => vec!["batch([{ type: 'setInput', text }])"],
        "keyboard.tap" => vec!["batch([{ type: 'forceSubmit', value }])"],
        "mouse" | "mouse.move" | "mouse.leftClick" | "mouse.rightClick" | "mouse.setPosition" => {
            vec!["batch(commands)", "getElements()"]
        }
        "webcam" => vec!["an explicitly configured external camera tool"],
        "mic" => vec!["an explicitly configured external audio tool"],
        "eyeDropper" => vec!["arg('Enter a color value')"],
        _ => Vec::new(),
    }
    .into_iter()
    .map(str::to_string)
    .collect();

    SdkCapability {
        name: entry.name.clone(),
        support: entry.support,
        minimum_host_version: env!("CARGO_PKG_VERSION").into(),
        requires_interactive_prompt: entry.category == "prompts"
            || matches!(
                entry.name.as_str(),
                "setActions"
                    | "setInput"
                    | "submit"
                    | "chat.addMessage"
                    | "chat.startStream"
                    | "chat.appendChunk"
                    | "chat.completeStream"
                    | "chat.clear"
                    | "chat.setError"
                    | "chat.clearError"
            ),
        required_permissions,
        supported_platforms,
        alternatives,
        migration_note: entry.unsupported_note.clone(),
    }
}

fn build_sdk_capability_catalog(functions: &[SdkFunctionRef]) -> SdkCapabilityCatalog {
    SdkCapabilityCatalog {
        schema_version: SDK_CAPABILITY_CATALOG_SCHEMA_VERSION,
        host_version: env!("CARGO_PKG_VERSION").into(),
        capabilities: functions.iter().map(sdk_capability_from_function).collect(),
    }
}

/// One immutable, versioned host contract shared by every capability lookup.
///
/// Script indexing can inspect many declarations on many commands. Rebuilding
/// all author-facing strings for each declaration made that work proportional
/// to `scripts * declarations * catalog_size`; retain one reviewed snapshot and
/// borrow its entries through a name index instead.
struct SdkCapabilityCatalogIndex {
    catalog: SdkCapabilityCatalog,
    positions: HashMap<String, usize>,
    generation: u64,
}

static SDK_CAPABILITY_CATALOG_CACHE: OnceLock<RwLock<Option<Arc<SdkCapabilityCatalogIndex>>>> =
    OnceLock::new();
static SDK_CAPABILITY_CATALOG_GENERATION: AtomicU64 = AtomicU64::new(0);

fn sdk_capability_catalog_index() -> Arc<SdkCapabilityCatalogIndex> {
    let cache = SDK_CAPABILITY_CATALOG_CACHE.get_or_init(|| RwLock::new(None));
    if let Some(index) = cache
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return Arc::clone(index);
    }

    let mut guard = cache
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(index) = guard.as_ref() {
        return Arc::clone(index);
    }

    let functions = build_sdk_function_refs();
    let catalog = build_sdk_capability_catalog(&functions);
    let positions = catalog
        .capabilities
        .iter()
        .enumerate()
        .map(|(index, capability)| (capability.name.clone(), index))
        .collect();
    let index = Arc::new(SdkCapabilityCatalogIndex {
        catalog,
        positions,
        generation: SDK_CAPABILITY_CATALOG_GENERATION.load(Ordering::Acquire),
    });
    *guard = Some(Arc::clone(&index));
    index
}

/// Explicitly invalidate the reviewed catalog when a host/schema contract is
/// replaced. Ordinary command indexing and scriptlet refreshes must not call
/// this: their declarations do not alter the host's supported capabilities.
pub fn invalidate_sdk_capability_catalog() -> u64 {
    let cache = SDK_CAPABILITY_CATALOG_CACHE.get_or_init(|| RwLock::new(None));
    let mut guard = cache
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let generation = SDK_CAPABILITY_CATALOG_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    *guard = None;
    generation
}

/// Reviewed catalog generation. Schema and host version remain available in
/// the published catalog; this changes only on explicit contract invalidation.
pub fn sdk_capability_catalog_generation() -> u64 {
    sdk_capability_catalog_index().generation
}

/// Public authoring contract shared by the SDK Reference, MCP clients, and
/// future script validation without a second independently maintained list.
pub fn sdk_capability_catalog() -> SdkCapabilityCatalog {
    sdk_capability_catalog_index().catalog.clone()
}

/// Read one reviewed capability through the shared O(1) name index. Only the
/// selected row is cloned; launcher/dispatch projections never rebuild or copy
/// the complete author-reference catalog.
pub fn sdk_capability(name: &str) -> Option<SdkCapability> {
    let index = sdk_capability_catalog_index();
    index
        .positions
        .get(name)
        .and_then(|position| index.catalog.capabilities.get(*position))
        .cloned()
}

#[derive(Clone, Copy)]
struct SdkHostDiagnosticContext<'a> {
    host_version: &'a str,
    platform: &'a str,
    /// `None` means nobody supplied an already-known permission inventory. It
    /// never means permission is granted and never triggers a system probe.
    granted_permissions: Option<&'a [String]>,
}

/// Explain unsupported APIs and impossible scriptlet prompt topologies before
/// dispatch. `None` means this catalog knows no compatibility blocker.
pub fn diagnose_sdk_capability(
    name: &str,
    topology: SdkExecutionTopology,
) -> Option<SdkCapabilityDiagnostic> {
    diagnose_sdk_capability_inner(name, topology, None)
}

/// Validate process-known platform/version facts without querying macOS,
/// opening privacy settings, or assuming unavailable permission facts.
/// Permission-scoped features return `PermissionInventoryUnavailable` until a
/// caller supplies an explicitly known inventory through the context API.
pub fn diagnose_sdk_capability_for_current_host(
    name: &str,
    topology: SdkExecutionTopology,
) -> Option<SdkCapabilityDiagnostic> {
    diagnose_sdk_capability_inner(
        name,
        topology,
        Some(SdkHostDiagnosticContext {
            host_version: env!("CARGO_PKG_VERSION"),
            platform: std::env::consts::OS,
            granted_permissions: None,
        }),
    )
}

/// Diagnose a capability against explicit, already-known host facts. This
/// never opens privacy settings, requests permissions, captures the display,
/// or probes the current app state.
pub fn diagnose_sdk_capability_with_context(
    name: &str,
    topology: SdkExecutionTopology,
    availability: &SdkHostAvailability,
) -> Option<SdkCapabilityDiagnostic> {
    diagnose_sdk_capability_inner(
        name,
        topology,
        Some(SdkHostDiagnosticContext {
            host_version: &availability.host_version,
            platform: &availability.platform,
            granted_permissions: Some(&availability.granted_permissions),
        }),
    )
}

fn diagnose_sdk_capability_inner(
    name: &str,
    topology: SdkExecutionTopology,
    availability: Option<SdkHostDiagnosticContext<'_>>,
) -> Option<SdkCapabilityDiagnostic> {
    let index = sdk_capability_catalog_index();
    let Some(capability) = index
        .positions
        .get(name)
        .and_then(|position| index.catalog.capabilities.get(*position))
    else {
        return Some(SdkCapabilityDiagnostic {
            code: SdkCapabilityDiagnosticCode::UnknownCapability,
            capability: name.into(),
            message: format!("`{name}` is not present in this host's SDK capability catalog."),
            alternatives: vec!["Open SDK Reference to choose a supported API.".into()],
        });
    };

    if matches!(
        topology,
        SdkExecutionTopology::ShellScriptlet | SdkExecutionTopology::PythonScriptlet
    ) {
        return Some(SdkCapabilityDiagnostic {
            code: SdkCapabilityDiagnosticCode::MissingSdkTransport,
            capability: name.into(),
            message: format!(
                "`{name}` requires the TypeScript SDK transport; shell and Python scriptlets do not receive SDK globals."
            ),
            alternatives: vec!["Move this command into a TypeScript script.".into()],
        });
    }

    if capability.support == SdkSupport::Unsupported {
        return Some(SdkCapabilityDiagnostic {
            code: SdkCapabilityDiagnosticCode::UnsupportedCapability,
            capability: name.into(),
            message: capability
                .migration_note
                .clone()
                .unwrap_or_else(|| format!("`{name}` is unsupported by this host.")),
            alternatives: capability.alternatives.clone(),
        });
    }

    if topology == SdkExecutionTopology::TypeScriptScriptlet
        && capability.requires_interactive_prompt
    {
        return Some(SdkCapabilityDiagnostic {
            code: SdkCapabilityDiagnosticCode::InteractivePromptUnavailable,
            capability: name.into(),
            message: format!(
                "`{name}` requires an interactive stdin prompt response, but TypeScript scriptlets do not receive an interactive stdin pipe."
            ),
            alternatives: vec!["Move the interactive command into a TypeScript script.".into()],
        });
    }

    if let Some(availability) = availability {
        let Ok(host_version) = Version::parse(availability.host_version) else {
            return Some(SdkCapabilityDiagnostic {
                code: SdkCapabilityDiagnosticCode::InvalidHostVersion,
                capability: name.into(),
                message: format!(
                    "Host version `{}` is not valid semantic version data; capability availability cannot be verified.",
                    availability.host_version
                ),
                alternatives: vec!["Refresh the host capability catalog.".into()],
            });
        };
        let Ok(minimum_version) = Version::parse(&capability.minimum_host_version) else {
            return Some(SdkCapabilityDiagnostic {
                code: SdkCapabilityDiagnosticCode::InvalidHostVersion,
                capability: name.into(),
                message: format!(
                    "Capability `{name}` declares an invalid minimum host version `{}`.",
                    capability.minimum_host_version
                ),
                alternatives: vec!["Refresh the host capability catalog.".into()],
            });
        };

        if host_version < minimum_version {
            return Some(SdkCapabilityDiagnostic {
                code: SdkCapabilityDiagnosticCode::HostVersionTooOld,
                capability: name.into(),
                message: format!(
                    "`{name}` requires host version {} or newer; the inspected host is {}.",
                    capability.minimum_host_version, availability.host_version
                ),
                alternatives: vec!["Update Script Kit before running this command.".into()],
            });
        }

        if !capability.supported_platforms.is_empty()
            && !capability
                .supported_platforms
                .iter()
                .any(|platform| platform == availability.platform)
        {
            return Some(SdkCapabilityDiagnostic {
                code: SdkCapabilityDiagnosticCode::UnsupportedPlatform,
                capability: name.into(),
                message: format!(
                    "`{name}` supports {}, but the inspected host runs {}.",
                    capability.supported_platforms.join(", "),
                    availability.platform
                ),
                alternatives: vec!["Use a capability supported on this platform.".into()],
            });
        }

        if !capability.required_permissions.is_empty() {
            let Some(granted_permissions) = availability.granted_permissions else {
                return Some(SdkCapabilityDiagnostic {
                    code: SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable,
                    capability: name.into(),
                    message: format!(
                        "`{name}` requires {}, but no already-known permission inventory was supplied; access cannot be assumed.",
                        capability.required_permissions.join(", ")
                    ),
                    alternatives: vec![
                        "Supply an existing read-only permission inventory before running this command."
                            .into(),
                    ],
                });
            };

            if let Some(permission) = capability.required_permissions.iter().find(|permission| {
                !granted_permissions
                    .iter()
                    .any(|granted| granted == *permission)
            }) {
                return Some(SdkCapabilityDiagnostic {
                    code: SdkCapabilityDiagnosticCode::MissingPermission,
                    capability: name.into(),
                    message: format!(
                        "`{name}` requires the `{permission}` permission, which the inspected host has not granted."
                    ),
                    alternatives: vec![format!(
                        "Grant `{permission}` in System Settings, then refresh the permission inventory."
                    )],
                });
            }
        }
    }

    None
}

/// Cheap UI-facing slice of the SDK reference document.
///
/// Callers reuse the same Rust data that powers `kit://sdk-reference`
/// so the in-product SDK Reference view never drifts from the MCP
/// resource or hand-authors a second API list.
pub fn sdk_reference_entries_for_ui() -> std::sync::Arc<[SdkFunctionRef]> {
    std::sync::Arc::from(build_sdk_reference_document().functions)
}

/// Case-insensitive substring match across `name`, `signature`, `description`,
/// and `category`. Returns the indices (into `entries`) of matching rows, in
/// source order. An empty or whitespace-only filter returns every row.
pub fn filter_sdk_reference_entries(entries: &[SdkFunctionRef], filter: &str) -> Vec<usize> {
    let q = filter.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if q.is_empty()
                || entry.name.to_lowercase().contains(&q)
                || entry.signature.to_lowercase().contains(&q)
                || entry.description.to_lowercase().contains(&q)
                || entry.category.to_lowercase().contains(&q)
            {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

/// A visible SDK Reference row projected from the shared MCP-backed catalog.
#[derive(Debug, Clone, Copy)]
pub struct SdkReferenceVisibleRow<'a> {
    pub display_index: usize,
    pub source_index: usize,
    pub entry: &'a SdkFunctionRef,
}

/// Visible SDK Reference rows in display order.
pub fn sdk_reference_visible_rows<'a>(
    entries: &'a [SdkFunctionRef],
    filter: &str,
) -> Vec<SdkReferenceVisibleRow<'a>> {
    filter_sdk_reference_entries(entries, filter)
        .into_iter()
        .enumerate()
        .filter_map(|(display_index, source_index)| {
            entries
                .get(source_index)
                .map(|entry| SdkReferenceVisibleRow {
                    display_index,
                    source_index,
                    entry,
                })
        })
        .collect()
}

pub fn sdk_reference_visible_row_names(entries: &[SdkFunctionRef], filter: &str) -> Vec<String> {
    sdk_reference_visible_rows(entries, filter)
        .into_iter()
        .map(|row| row.entry.name.clone())
        .collect()
}

pub fn sdk_reference_dataset_and_visible_counts(
    entries: &[SdkFunctionRef],
    filter: &str,
) -> (usize, usize) {
    (
        entries.len(),
        sdk_reference_visible_rows(entries, filter).len(),
    )
}

pub fn sdk_reference_selected_visible_entry<'a>(
    entries: &'a [SdkFunctionRef],
    filter: &str,
    selected_index: usize,
) -> Option<SdkReferenceVisibleRow<'a>> {
    sdk_reference_visible_rows(entries, filter)
        .get(selected_index)
        .copied()
}

pub fn sdk_reference_visible_target_rows<'a>(
    entries: &'a [SdkFunctionRef],
    filter: &str,
    limit: usize,
) -> Vec<SdkReferenceVisibleRow<'a>> {
    sdk_reference_visible_rows(entries, filter)
        .into_iter()
        .take(limit)
        .collect()
}

/// Markdown preview for a single SDK function — used by the in-product
/// SDK Reference view (preview pane + Cmd+C clipboard copy).
///
/// Unsupported entries are prepended with a blockquote warning so a
/// snippet pasted into an editor still carries the "this will no-op or
/// throw" signal even after it leaves the launcher.
pub fn format_sdk_reference_entry_markdown(entry: &SdkFunctionRef) -> String {
    let mut out = String::new();
    if entry.support == SdkSupport::Unsupported {
        out.push_str("> ⚠ Unsupported in GPUI — this function fails before dispatch or returns an explicit unsupported receipt.\n");
        if let Some(note) = entry.unsupported_note.as_deref() {
            out.push_str("> ");
            out.push_str(note);
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&format!(
        "# {name}\n\n`{signature}`\n\n_{category}_\n\n{description}\n",
        name = entry.name,
        signature = entry.signature,
        category = entry.category,
        description = entry.description,
    ));
    out.push_str(&format!(
        "\nAuthor diagnostics: read `{COMMAND_DOCTOR_RESOURCE_URI}` for command readiness, permissions, and compatibility; read `{FAILED_SCRIPTS_RESOURCE_URI}` for excluded or retained validation issues. These are host MCP resources, not callable SDK globals.\n"
    ));
    out
}

/// Curated v1 starter templates.
///
/// Ordering is load-bearing: `blank-starter` is row #1 so the fastest
/// "new script" path (Enter → Enter → name → editor) feels identical
/// to the pre-catalog experience.
///
/// **Invariant:** no template may emit `alias:`, `shortcut:`, `keyword:`,
/// or `trigger:` in its body. `detect_binding_collisions` would mark a
/// fresh duplicate as fatal and hide the script from the launcher —
/// defeating the whole "first useful automation" purpose of templates.
fn build_script_templates() -> Vec<ScriptTemplateRef> {
    vec![
        ScriptTemplateRef {
            id: "blank-starter".into(),
            title: "Blank Starter".into(),
            description: "An empty script shape with an arg prompt and div output — the fastest path from naming to a working script.".into(),
            category: "starter".into(),
            filename_hint: "my-script".into(),
            body_template: concat!(
                "import \"@scriptkit/sdk\";\n",
                "\n",
                "export const metadata = {\n",
                "  name: \"{{NAME}}\",\n",
                "  description: \"{{DESCRIPTION}}\",\n",
                "  sdkCapabilities: [\"arg\", \"div\", \"md\"],\n",
                "  executionTopology: \"typescript-script\",\n",
                "};\n",
                "\n",
                "const value = await arg(\"Enter a value\");\n",
                "\n",
                "await div(md(`## You typed\\n\\n${value}`));\n",
            ).into(),
            metadata_defaults: ScriptTemplateMetadataDefaults {
                description: Some("A blank starter script".into()),
            },
        },
        ScriptTemplateRef {
            id: "choice-list".into(),
            title: "Choice List".into(),
            description: "Prompt the user to pick one option from a fixed list, then show the selection.".into(),
            category: "prompts".into(),
            filename_hint: "pick-one".into(),
            body_template: concat!(
                "import \"@scriptkit/sdk\";\n",
                "\n",
                "export const metadata = {\n",
                "  name: \"{{NAME}}\",\n",
                "  description: \"{{DESCRIPTION}}\",\n",
                "  sdkCapabilities: [\"arg\", \"div\", \"md\"],\n",
                "  executionTopology: \"typescript-script\",\n",
                "};\n",
                "\n",
                "const choice = await arg(\"Pick one\", [\"A\", \"B\", \"C\"]);\n",
                "\n",
                "await div(md(`## Selected\\n\\n${choice}`));\n",
            ).into(),
            metadata_defaults: ScriptTemplateMetadataDefaults {
                description: Some("Prompt the user to pick one option from a list".into()),
            },
        },
    ]
}

/// Pure builder for the `kit://script-templates` resource envelope.
pub fn build_script_templates_document() -> ScriptTemplatesResourceDocument {
    let templates = build_script_templates();
    ScriptTemplatesResourceDocument {
        schema_version: SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION,
        count: templates.len(),
        templates,
    }
}

/// Cheap UI-facing slice of the template catalog. Same objects the MCP
/// resource returns, so the in-launcher catalog and any agent reading
/// `kit://script-templates` cannot drift.
pub fn script_template_entries_for_ui() -> std::sync::Arc<[ScriptTemplateRef]> {
    std::sync::Arc::from(build_script_templates())
}

/// Case-insensitive substring match across `title`, `description`, and
/// `category`. Returns the indices (into `entries`) of matching rows in
/// source order. An empty or whitespace-only filter returns every row.
pub fn filter_script_template_entries(entries: &[ScriptTemplateRef], filter: &str) -> Vec<usize> {
    let q = filter.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            if q.is_empty()
                || entry.title.to_lowercase().contains(&q)
                || entry.description.to_lowercase().contains(&q)
                || entry.category.to_lowercase().contains(&q)
            {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

/// A visible starter-template row projected from the shared MCP-backed catalog.
#[derive(Debug, Clone, Copy)]
pub struct ScriptTemplateCatalogVisibleRow<'a> {
    pub display_index: usize,
    pub source_index: usize,
    pub template: &'a ScriptTemplateRef,
}

/// Visible starter-template rows in display order.
pub fn script_template_catalog_visible_rows<'a>(
    entries: &'a [ScriptTemplateRef],
    filter: &str,
) -> Vec<ScriptTemplateCatalogVisibleRow<'a>> {
    filter_script_template_entries(entries, filter)
        .into_iter()
        .enumerate()
        .filter_map(|(display_index, source_index)| {
            entries
                .get(source_index)
                .map(|template| ScriptTemplateCatalogVisibleRow {
                    display_index,
                    source_index,
                    template,
                })
        })
        .collect()
}

pub fn script_template_catalog_visible_row_names(
    entries: &[ScriptTemplateRef],
    filter: &str,
) -> Vec<String> {
    script_template_catalog_visible_rows(entries, filter)
        .into_iter()
        .map(|row| row.template.title.clone())
        .collect()
}

pub fn script_template_catalog_dataset_and_visible_counts(
    entries: &[ScriptTemplateRef],
    filter: &str,
) -> (usize, usize) {
    (
        entries.len(),
        script_template_catalog_visible_rows(entries, filter).len(),
    )
}

pub fn script_template_catalog_selected_visible_template<'a>(
    entries: &'a [ScriptTemplateRef],
    filter: &str,
    selected_index: usize,
) -> Option<ScriptTemplateCatalogVisibleRow<'a>> {
    script_template_catalog_visible_rows(entries, filter)
        .get(selected_index)
        .copied()
}

pub fn script_template_catalog_visible_target_rows<'a>(
    entries: &'a [ScriptTemplateRef],
    filter: &str,
    limit: usize,
) -> Vec<ScriptTemplateCatalogVisibleRow<'a>> {
    script_template_catalog_visible_rows(entries, filter)
        .into_iter()
        .take(limit)
        .collect()
}

/// Instantiate a template's `body_template` for on-disk write.
///
/// Substitutes `{{NAME}}` with `friendly_name` and `{{DESCRIPTION}}` with
/// the template's `metadata_defaults.description` (falling back to the
/// template title). Substitution is single-pass and JSON-string escaped: valid
/// friendly names can contain quotes, braces, Unicode, or placeholder-shaped
/// text without becoming executable TypeScript or being expanded recursively.
/// The returned string is the exact content that
/// [`crate::app_impl::naming_dialog::ScriptListApp::handle_naming_dialog_completion`]
/// supplies to [`crate::script_creation::create_new_script_with_contents`], so
/// the original exclusively created file handle writes the final bytes before
/// [`crate::script_creation::open_in_editor`].
pub fn render_script_template_file(template: &ScriptTemplateRef, friendly_name: &str) -> String {
    const NAME_PLACEHOLDER: &str = "{{NAME}}";
    const DESCRIPTION_PLACEHOLDER: &str = "{{DESCRIPTION}}";

    let description = template
        .metadata_defaults
        .description
        .as_deref()
        .unwrap_or(&template.title);

    // Value::Display serializes strings infallibly. The surrounding quotes are
    // supplied by the TypeScript template, so retain only escaped JSON content.
    let encoded_name = Value::String(friendly_name.to_owned()).to_string();
    let encoded_description = Value::String(description.to_owned()).to_string();
    let escaped_name = &encoded_name[1..encoded_name.len() - 1];
    let escaped_description = &encoded_description[1..encoded_description.len() - 1];

    let source = template.body_template.as_str();
    let mut rendered = String::with_capacity(source.len());
    let mut cursor = 0;

    while let Some(offset) = source[cursor..].find("{{") {
        let start = cursor + offset;
        rendered.push_str(&source[cursor..start]);
        let remainder = &source[start..];

        if remainder.starts_with(NAME_PLACEHOLDER) {
            rendered.push_str(escaped_name);
            cursor = start + NAME_PLACEHOLDER.len();
        } else if remainder.starts_with(DESCRIPTION_PLACEHOLDER) {
            rendered.push_str(escaped_description);
            cursor = start + DESCRIPTION_PLACEHOLDER.len();
        } else {
            rendered.push_str("{{");
            cursor = start + 2;
        }
    }

    rendered.push_str(&source[cursor..]);
    rendered
}

/// Markdown preview for a single template — used by the catalog view's
/// preview pane and Cmd+C clipboard copy.
pub fn format_script_template_markdown(template: &ScriptTemplateRef) -> String {
    format!(
        "# {title}\n\n_{category}_\n\n{description}\n\n```ts\n{body}```\n",
        title = template.title,
        category = template.category,
        description = template.description,
        body = template.body_template,
    )
}

/// Resolve a template by `id`. Used by
/// [`crate::app_impl::naming_dialog::ScriptListApp::handle_naming_dialog_completion`]
/// to turn the `template_id` carried through [`crate::prompts::NamingSubmitResult`]
/// back into the in-memory [`ScriptTemplateRef`] consumed by
/// [`render_script_template_file`].
pub fn find_script_template(id: &str) -> Option<ScriptTemplateRef> {
    build_script_templates_document()
        .templates
        .into_iter()
        .find(|template| template.id == id)
}

pub(crate) fn build_sdk_reference_document() -> SdkReferenceDocument {
    let functions = build_sdk_function_refs();
    let capability_catalog = build_sdk_capability_catalog(&functions);

    SdkReferenceDocument {
        schema_version: SDK_REFERENCE_SCHEMA_VERSION,
        sdk_package: "@scriptkit/sdk".into(),
        script_directory: "~/.scriptkit/plugins/main/scripts/ (default personal plugin; all plugins under plugins/*/scripts/ are discovered)".into(),
        scriptlet_pattern: "~/.scriptkit/plugins/*/scriptlets/*.md".into(),
        metadata_format:
            "export const metadata = { name: \"My Script\", description: \"What it does\" }".into(),
        functions,
        capability_catalog,
        authoring_resources: vec![
            SdkAuthoringResource {
                uri: COMMAND_DOCTOR_RESOURCE_URI.to_string(),
                name: "Command Doctor".to_string(),
                description: "Read-only readiness, capability support, safe permission-pending state, genuine launcher action previews, and repair guidance for already-loaded commands.".to_string(),
            },
            SdkAuthoringResource {
                uri: FAILED_SCRIPTS_RESOURCE_URI.to_string(),
                name: "Script Issues".to_string(),
                description: "Excluded invalid scripts and retained scriptlet diagnostics with source paths, typed reasons, and repair alternatives.".to_string(),
            },
        ],
        harness_workflow: build_harness_workflow(),
    }
}

/// Shell-quote a literal value for safe embedding in a command string.
fn shell_quote_literal(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-:=@".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', r#"'"'"'"#))
    }
}

/// Resolve the absolute path to the running binary, falling back to bare name.
fn resolve_harness_run_binary() -> String {
    match std::env::current_exe() {
        Ok(path) => {
            let text = path.to_string_lossy().into_owned();
            shell_quote_literal(&text)
        }
        Err(_) => "script-kit-gpui".to_string(),
    }
}

/// Build the harness run command using the resolved absolute binary path.
fn build_harness_run_command() -> String {
    format!(
        "echo '{{\"type\":\"run\",\"path\":\"{{path}}\"}}' | {}",
        resolve_harness_run_binary()
    )
}

fn build_harness_workflow() -> HarnessWorkflow {
    HarnessWorkflow {
        test_script_directory: "~/.scriptkit/tmp/test-scripts/".into(),
        test_scriptlet_directory: "~/.scriptkit/tmp/test-scriptlets/".into(),
        run_command: build_harness_run_command(),
        stdin_run_message: r#"{"type":"run","path":"/absolute/path/to/script.ts"}"#.into(),
        success_output_shape: "No dedicated success envelope is emitted for stdin `run`; successful scripts communicate through their normal stdout JSONL protocol and app logs. The mandatory Bun gate for final user scripts is published in `verification`.".into(),
        error_output_shape: "No dedicated error envelope is emitted for stdin `run`; failures surface through script error protocol messages, app logs, and HUD/toast feedback. If either Bun verification command fails, the agent must fix the script and rerun both commands before reporting success.".into(),
        verification: HarnessVerificationContract {
            required: true,
            skill_path: crate::ai::harness::SCRIPT_AUTHORING_SKILL_MARKER.into(),
            build_command: crate::ai::harness::BUN_BUILD_VERIFICATION_MARKER.into(),
            run_command: crate::ai::harness::BUN_EXECUTE_VERIFICATION_MARKER.into(),
            success_criteria: crate::ai::harness::BUN_VERIFICATION_SUCCESS_CRITERIA.into(),
            failure_policy: crate::ai::harness::BUN_VERIFICATION_FAILURE_POLICY.into(),
        },
        example_test_script: concat!(
            "import \"@scriptkit/sdk\";\n",
            "\n",
            "export const metadata = {\n",
            "  name: \"Harness Test\",\n",
            "  description: \"Automated test script\",\n",
            "};\n",
            "\n",
            "const isVerify = process.env.SK_VERIFY === \"1\";\n",
            "\n",
            "const result = isVerify\n",
            "  ? \"a\"\n",
            "  : await arg(\"Pick one\", [\"a\", \"b\", \"c\"]);\n",
            "\n",
            "if (isVerify) {\n",
            "  console.log(JSON.stringify({ ok: true, result }));\n",
            "} else {\n",
            "  await div(md(`## ${result}`));\n",
            "}\n",
        ).into(),
        example_scriptlet: concat!(
            "---\n",
            "name: Date Tools\n",
            "description: Helpful date utilities\n",
            "icon: calendar-days\n",
            "---\n",
            "\n",
            "## Copy Date\n",
            "\n",
            "```metadata\n",
            "description: Copy today's date\n",
            "shortcut: opt d\n",
            "```\n",
            "\n",
            "```tool:copy-date\n",
            "import \"@scriptkit/sdk\";\n",
            "\n",
            "await copy(new Date().toISOString().slice(0, 10));\n",
            "hud(\"Copied today's date\");\n",
            "```\n",
        ).into(),
    }
}

fn doctor_capability(name: &str) -> CommandDoctorCapability {
    match sdk_capability(name) {
        Some(capability) => CommandDoctorCapability {
            name: capability.name,
            support: capability.support,
            required_permissions: capability.required_permissions,
            supported_platforms: capability.supported_platforms,
            minimum_host_version: Some(capability.minimum_host_version),
            alternatives: capability.alternatives,
        },
        None => CommandDoctorCapability {
            name: name.to_string(),
            support: SdkSupport::Unsupported,
            required_permissions: Vec::new(),
            supported_platforms: Vec::new(),
            minimum_host_version: None,
            alternatives: vec!["Open SDK Reference to choose a supported API.".to_string()],
        },
    }
}

fn doctor_entry(
    source: &str,
    name: &str,
    path: String,
    plugin_id: &str,
    plugin_title: Option<String>,
    declared_capabilities: Vec<String>,
    mut issues: Vec<ScriptValidationIssue>,
) -> CommandDoctorEntry {
    let mut capabilities: Vec<_> = declared_capabilities
        .into_iter()
        .map(|name| doctor_capability(&name))
        .collect();
    capabilities.sort_by(|left, right| left.name.cmp(&right.name));
    capabilities.dedup_by(|left, right| left.name == right.name);
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.message.cmp(&right.message))
    });

    let has_unsupported = capabilities
        .iter()
        .any(|capability| capability.support == SdkSupport::Unsupported)
        || issues.iter().any(|issue| {
            matches!(
                &issue.kind,
                crate::scripts::ScriptValidationKind::CapabilityUnavailable {
                    code: SdkCapabilityDiagnosticCode::UnknownCapability
                        | SdkCapabilityDiagnosticCode::UnsupportedCapability,
                    ..
                }
            )
        });
    let has_blocking_issue = issues
        .iter()
        .any(|issue| issue.severity == crate::scripts::ValidationSeverity::Fatal);
    let permission_pending = issues.iter().any(|issue| {
        matches!(
            &issue.kind,
            crate::scripts::ScriptValidationKind::CapabilityUnavailable {
                code: SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable,
                ..
            }
        )
    });

    let state = if has_unsupported {
        CommandDoctorState::Unsupported
    } else if has_blocking_issue {
        CommandDoctorState::Blocked
    } else if permission_pending {
        CommandDoctorState::PermissionPending
    } else if capabilities
        .iter()
        .any(|capability| capability.support == SdkSupport::Experimental)
    {
        CommandDoctorState::Experimental
    } else {
        CommandDoctorState::Ready
    };

    let mut alternatives: Vec<_> = capabilities
        .iter()
        .flat_map(|capability| capability.alternatives.iter().cloned())
        .chain(issues.iter().flat_map(|issue| match &issue.kind {
            crate::scripts::ScriptValidationKind::CapabilityUnavailable {
                alternatives, ..
            } => alternatives.clone(),
            _ => Vec::new(),
        }))
        .collect();
    alternatives.sort();
    alternatives.dedup();

    CommandDoctorEntry {
        source: source.to_string(),
        name: name.to_string(),
        path,
        plugin_id: plugin_id.to_string(),
        plugin_title,
        state,
        executable: matches!(
            state,
            CommandDoctorState::Ready | CommandDoctorState::Experimental
        ),
        primary_action: None,
        capabilities,
        issues,
        alternatives,
    }
}

/// Project the host-owned descriptor's real action and disabled reason. This
/// never synthesizes an SDK call, executes a command, or exposes its raw key.
pub fn command_doctor_preview_from_descriptor(
    descriptor: &sk_protocol::command_contract::CommandDescriptor,
) -> Option<CommandDoctorPrimaryAction> {
    let primary = descriptor.primary_action()?;
    let identity_fingerprint = crate::protocol::RedactedElementContent::new(
        crate::protocol::ElementContentKind::ExternalContent,
        descriptor.identity.as_str(),
    )
    .fingerprint;
    Some(CommandDoctorPrimaryAction {
        title: primary.title.clone(),
        enabled: descriptor.availability.is_executable() && primary.availability.is_executable(),
        reason: primary
            .availability
            .reason_code()
            .or_else(|| descriptor.availability.reason_code())
            .map(str::to_string),
        identity_fingerprint,
    })
}

/// Attach a genuine launcher descriptor to an existing doctor row. A blocked
/// capability remains blocked even if an outdated descriptor says otherwise.
pub fn attach_command_doctor_descriptor_preview(
    entry: &mut CommandDoctorEntry,
    descriptor: &sk_protocol::command_contract::CommandDescriptor,
) -> bool {
    let Some(mut preview) = command_doctor_preview_from_descriptor(descriptor) else {
        return false;
    };
    preview.enabled &= entry.executable;
    entry.primary_action = Some(preview);
    true
}

/// Build a complete command-doctor receipt from explicitly supplied snapshots.
/// This is pure: missing permission facts remain pending and never cause an OS
/// preflight, settings dialog, capture, app launch, or provider request.
pub fn build_command_doctor_report(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
    availability: Option<&SdkHostAvailability>,
) -> CommandDoctorReport {
    let mut commands = Vec::with_capacity(scripts.len() + scriptlets.len());
    for script in scripts {
        let issues = availability.map_or_else(
            || crate::scripts::validate_declared_sdk_capabilities(script),
            |host| {
                crate::scripts::validate_declared_sdk_capabilities_with_host_availability(
                    script, host,
                )
            },
        );
        let declared_capabilities = script
            .typed_metadata
            .as_ref()
            .and_then(|metadata| metadata.extra.get("sdkCapabilities"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let mut entry = doctor_entry(
            "script",
            &script.name,
            script.path.to_string_lossy().into_owned(),
            &script.plugin_id,
            script.plugin_title.clone(),
            declared_capabilities,
            issues,
        );
        let row = crate::scripts::SearchResult::Script(crate::scripts::ScriptMatch {
            script: Arc::clone(script),
            score: 0,
            filename: script
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            match_indices: crate::scripts::MatchIndices::default(),
            match_kind: crate::scripts::ScriptMatchKind::default(),
            content_match: None,
            match_evidence: None,
        });
        let descriptor = availability.map_or_else(
            || row.command_descriptor(),
            |host| row.command_descriptor_with_host_availability(host),
        );
        if let Ok(descriptor) = descriptor {
            attach_command_doctor_descriptor_preview(&mut entry, &descriptor);
        }
        commands.push(entry);
    }

    for scriptlet in scriptlets {
        let issues = availability.map_or_else(
            || crate::scripts::validate_scriptlet_capabilities(scriptlet),
            |host| {
                crate::scripts::validate_scriptlet_capabilities_with_host_availability(
                    scriptlet, host,
                )
            },
        );
        let path = scriptlet.file_path.clone().unwrap_or_else(|| {
            format!(
                "plugins/{}/scriptlets/{}",
                scriptlet.plugin_id,
                scriptlet.command.as_deref().unwrap_or(&scriptlet.name)
            )
        });
        let mut entry = doctor_entry(
            "scriptlet",
            &scriptlet.name,
            path,
            &scriptlet.plugin_id,
            scriptlet.plugin_title.clone(),
            crate::scripts::scriptlet_declared_sdk_capabilities(scriptlet),
            issues,
        );
        let row = crate::scripts::SearchResult::Scriptlet(crate::scripts::ScriptletMatch {
            scriptlet: Arc::clone(scriptlet),
            score: 0,
            display_file_path: scriptlet.file_path.clone(),
            match_indices: crate::scripts::MatchIndices::default(),
            match_evidence: None,
        });
        let descriptor = availability.map_or_else(
            || row.command_descriptor(),
            |host| row.command_descriptor_with_host_availability(host),
        );
        if let Ok(descriptor) = descriptor {
            attach_command_doctor_descriptor_preview(&mut entry, &descriptor);
        }
        commands.push(entry);
    }

    commands.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.plugin_id.cmp(&right.plugin_id))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.name.cmp(&right.name))
    });

    let count = |state| commands.iter().filter(|entry| entry.state == state).count();
    CommandDoctorReport {
        schema_version: COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION,
        host_version: availability
            .map(|host| host.host_version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        platform: availability
            .map(|host| host.platform.clone())
            .unwrap_or_else(|| std::env::consts::OS.to_string()),
        permission_inventory_known: availability.is_some(),
        total_commands: commands.len(),
        ready_count: count(CommandDoctorState::Ready),
        experimental_count: count(CommandDoctorState::Experimental),
        unsupported_count: count(CommandDoctorState::Unsupported),
        blocked_count: count(CommandDoctorState::Blocked),
        permission_pending_count: count(CommandDoctorState::PermissionPending),
        commands,
    }
}

fn read_command_doctor_resource(
    scripts: &[Arc<Script>],
    scriptlets: &[Arc<Scriptlet>],
) -> Result<ResourceContent, String> {
    let report = build_command_doctor_report(scripts, scriptlets, None);
    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("Failed to serialize command doctor: {error}"))?;
    Ok(ResourceContent {
        uri: COMMAND_DOCTOR_RESOURCE_URI.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

/// Read kit://scripts schema-versioned resource
fn read_kit_scripts_resource(scripts: &[Arc<Script>]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptResourceEntry> = scripts
        .iter()
        .map(|s| ScriptResourceEntry::from(s.as_ref()))
        .collect();
    let doc = ScriptsResourceDocument {
        schema_version: SCRIPTS_RESOURCE_SCHEMA_VERSION,
        count: entries.len(),
        scripts: entries,
    };
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize scripts document: {e}"))?;
    Ok(ResourceContent {
        uri: "kit://scripts".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Pure builder for [`FailedScriptsResourceDocument`]. Split from the
/// resource handler so tests can exercise envelope shape against hand-built
/// [`ValidationReport`]s without touching the filesystem.
pub(crate) fn build_failed_scripts_document(
    report: &ValidationReport,
) -> FailedScriptsResourceDocument {
    FailedScriptsResourceDocument {
        schema_version: FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION,
        validation_schema_version: report.schema_version,
        total_candidates: report.total_candidates,
        valid_count: report.valid_count,
        fatal_count: report.fatal_count,
        warning_count: report.warning_count,
        failed_scripts: report
            .failed_scripts
            .iter()
            .map(FailedScriptEntry::from)
            .collect(),
        warnings: report.warnings.iter().cloned().collect(),
        retained_issue_count: report.retained_issues.len(),
        retained_issues: report.retained_issues.iter().cloned().collect(),
    }
}

/// Read kit://failed-scripts schema-versioned resource.
///
/// Calls [`crate::scripts::read_scripts_report`] at read time (rather than
/// requiring a cached report threaded through [`read_resource`]), so the
/// response always reflects the current disk state. This is cheap relative
/// to MCP request cadence — script loading already runs at startup.
fn read_kit_failed_scripts_resource() -> Result<ResourceContent, String> {
    let report = crate::scripts::read_scripts_report();
    let merged = crate::scripts::merge_registered_scriptlet_validation_issues(&report.validation);
    let doc = build_failed_scripts_document(&merged);
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize failed-scripts document: {e}"))?;
    Ok(ResourceContent {
        uri: FAILED_SCRIPTS_RESOURCE_URI.to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Read kit://script-templates schema-versioned resource.
fn read_kit_script_templates_resource() -> Result<ResourceContent, String> {
    let doc = build_script_templates_document();
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize script-templates document: {e}"))?;
    Ok(ResourceContent {
        uri: SCRIPT_TEMPLATES_RESOURCE_URI.to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Read kit://scriptlets schema-versioned resource
fn read_kit_scriptlets_resource(scriptlets: &[Arc<Scriptlet>]) -> Result<ResourceContent, String> {
    let entries: Vec<ScriptletResourceEntry> = scriptlets
        .iter()
        .map(|s| ScriptletResourceEntry::from(s.as_ref()))
        .collect();
    let doc = ScriptletsResourceDocument {
        schema_version: SCRIPTLETS_RESOURCE_SCHEMA_VERSION,
        count: entries.len(),
        scriptlets: entries,
    };
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize scriptlets document: {e}"))?;
    Ok(ResourceContent {
        uri: "kit://scriptlets".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Read kit://sdk-reference resource
fn read_sdk_reference_resource() -> Result<ResourceContent, String> {
    let doc = build_sdk_reference_document();
    tracing::info!(
        category = "MCP",
        schema_version = doc.schema_version,
        function_count = doc.functions.len(),
        "Built kit://sdk-reference document"
    );
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize SDK reference: {e}"))?;
    Ok(ResourceContent {
        uri: "kit://sdk-reference".to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}
// ---------------------------------------------------------------
// Clipboard history resource
// ---------------------------------------------------------------

/// Default limit for clipboard history entries returned.
const CLIPBOARD_HISTORY_DEFAULT_LIMIT: usize = 10;

/// Maximum limit for clipboard history entries.
const CLIPBOARD_HISTORY_MAX_LIMIT: usize = 50;

/// Default limit for dictation history entries returned.
const DICTATION_HISTORY_DEFAULT_LIMIT: usize = 10;

/// Maximum limit for dictation history entries.
const DICTATION_HISTORY_MAX_LIMIT: usize = 50;

/// Parsed clipboard history request — either a list query or a single-entry lookup.
#[derive(Debug)]
enum ClipboardHistoryRequest {
    /// List mode: fetch up to `limit` entries, optionally with diagnostics wrapper.
    List { limit: usize, diagnostics: bool },
    /// Single-entry mode: fetch the entry with the given ID.
    SingleEntry { id: String },
}

fn parse_clipboard_history_request(uri: &str) -> Result<ClipboardHistoryRequest, String> {
    if uri == "kit://clipboard-history" {
        return Ok(ClipboardHistoryRequest::List {
            limit: CLIPBOARD_HISTORY_DEFAULT_LIMIT,
            diagnostics: false,
        });
    }

    let (_base, query) = uri
        .split_once('?')
        .ok_or_else(|| format!("Resource not found: {uri}"))?;

    let mut limit = CLIPBOARD_HISTORY_DEFAULT_LIMIT;
    let mut diagnostics = false;
    let mut entry_id: Option<String> = None;

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "1"));
        match key {
            "id" => {
                entry_id = Some(value.to_string());
            }
            "limit" => {
                limit = value
                    .parse::<usize>()
                    .map_err(|_| {
                        format!("Invalid limit value: {value}. Expected a positive integer.")
                    })?
                    .min(CLIPBOARD_HISTORY_MAX_LIMIT);
            }
            "diagnostics" => diagnostics = parse_bool_param(value)?,
            _ => {
                return Err(format!(
                    "Invalid kit://clipboard-history parameter: {key}. Supported parameters: id, limit, diagnostics."
                ));
            }
        }
    }

    if let Some(id) = entry_id {
        Ok(ClipboardHistoryRequest::SingleEntry { id })
    } else {
        Ok(ClipboardHistoryRequest::List { limit, diagnostics })
    }
}

fn read_clipboard_history_entries(limit: usize) -> Vec<ClipboardHistoryEntry> {
    let cached = crate::clipboard_history::get_cached_entries(limit);
    cached
        .into_iter()
        .map(|entry| ClipboardHistoryEntry {
            id: entry.id,
            content_type: entry.content_type.as_str().to_string(),
            timestamp: entry.timestamp,
            text_preview: if entry.text_preview.is_empty() || entry.text_preview == "[Image]" {
                None
            } else {
                Some(entry.text_preview)
            },
            ocr_text: entry.ocr_text,
            image_width: entry.image_width,
            image_height: entry.image_height,
            pinned: entry.pinned,
        })
        .collect()
}

/// Read kit://clipboard-history resource
fn read_clipboard_history_resource(uri: &str) -> Result<ResourceContent, String> {
    let request = parse_clipboard_history_request(uri)?;

    match request {
        ClipboardHistoryRequest::SingleEntry { id } => {
            // Single-entry mode: return the entry's text content directly.
            let content = crate::clipboard_history::get_entry_content(&id)
                .ok_or_else(|| format!("Clipboard entry not found: {id}"))?;
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: "text/plain".to_string(),
                text: content,
            })
        }
        ClipboardHistoryRequest::List { limit, diagnostics } => {
            let started = Instant::now();
            let entries = read_clipboard_history_entries(limit);
            let duration_ms = started.elapsed().as_millis();

            let doc = ClipboardHistoryDocument {
                schema_version: CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION,
                count: entries.len(),
                entries,
            };

            let json = if diagnostics {
                let diag = ClipboardHistoryDiagnosticsDocument {
                    kind: "clipboard_history_diagnostics",
                    uri: uri.to_string(),
                    meta: ClipboardHistoryDiagnosticsMeta {
                        duration_ms,
                        entry_count: doc.count,
                        source: "cached_entries",
                    },
                    document: doc,
                };
                serde_json::to_string_pretty(&diag).map_err(|e| {
                    format!("Failed to serialize clipboard history diagnostics: {e}")
                })?
            } else {
                serde_json::to_string_pretty(&doc)
                    .map_err(|e| format!("Failed to serialize clipboard history: {e}"))?
            };

            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: "application/json".to_string(),
                text: json,
            })
        }
    }
}

#[derive(Debug)]
enum DictationHistoryRequest {
    List { limit: usize },
    SingleEntry { id: String },
}

fn parse_dictation_history_request(uri: &str) -> Result<DictationHistoryRequest, String> {
    if uri == "kit://dictation-history" {
        return Ok(DictationHistoryRequest::List {
            limit: DICTATION_HISTORY_DEFAULT_LIMIT,
        });
    }

    let (_base, query) = uri
        .split_once('?')
        .ok_or_else(|| format!("Resource not found: {uri}"))?;

    let mut limit = DICTATION_HISTORY_DEFAULT_LIMIT;
    let mut entry_id: Option<String> = None;

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "1"));
        match key {
            "id" => entry_id = Some(value.to_string()),
            "limit" => {
                limit = value
                    .parse::<usize>()
                    .map_err(|_| {
                        format!("Invalid limit value: {value}. Expected a positive integer.")
                    })?
                    .min(DICTATION_HISTORY_MAX_LIMIT);
            }
            _ => {
                return Err(format!(
                    "Invalid kit://dictation-history parameter: {key}. Supported parameters: id, limit."
                ));
            }
        }
    }

    if let Some(id) = entry_id {
        Ok(DictationHistoryRequest::SingleEntry { id })
    } else {
        Ok(DictationHistoryRequest::List { limit })
    }
}

fn read_dictation_history_resource(uri: &str) -> Result<ResourceContent, String> {
    match parse_dictation_history_request(uri)? {
        DictationHistoryRequest::SingleEntry { id } => {
            let entry = crate::dictation::get_history_entry(&id)
                .ok_or_else(|| format!("Dictation history entry not found: {id}"))?;
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: "text/plain".to_string(),
                text: entry.transcript,
            })
        }
        DictationHistoryRequest::List { limit } => {
            let entries: Vec<crate::dictation::DictationHistoryEntry> =
                crate::dictation::load_history()
                    .into_iter()
                    .take(limit)
                    .collect();
            let json = serde_json::to_string_pretty(&entries)
                .map_err(|e| format!("Failed to serialize dictation history: {e}"))?;
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: "application/json".to_string(),
                text: json,
            })
        }
    }
}

// ---------------------------------------------------------------
// Focused item resource
// ---------------------------------------------------------------

fn parse_focused_item_request(uri: &str) -> Result<bool, String> {
    if uri == "kit://focused-item" {
        return Ok(false);
    }

    let (_base, query) = uri
        .split_once('?')
        .ok_or_else(|| format!("Resource not found: {uri}"))?;

    let mut diagnostics = false;

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "1"));
        match key {
            "diagnostics" => diagnostics = parse_bool_param(value)?,
            _ => {
                return Err(format!(
                    "Invalid kit://focused-item parameter: {key}. Supported parameters: diagnostics."
                ));
            }
        }
    }

    Ok(diagnostics)
}

/// Read the focused item from the global focused-item slot.
///
/// The slot is populated by surfaces (e.g., Tab AI orchestration) when they
/// resolve the focused/selected item. Outside of those flows, the slot is empty
/// and the resource returns `hasFocusedItem: false`.
fn read_focused_item_data() -> (Option<FocusedItemInfo>, Vec<String>) {
    let guard = FOCUSED_ITEM_SLOT.lock();
    match guard.as_ref() {
        Some(item) => (Some(item.clone()), Vec::new()),
        None => (
            None,
            vec!["no_active_surface: No surface has published a focused item.".to_string()],
        ),
    }
}

/// Read kit://focused-item resource
fn read_focused_item_resource(uri: &str) -> Result<ResourceContent, String> {
    let diagnostics = parse_focused_item_request(uri)?;

    let started = Instant::now();
    let (focused_item, warnings) = read_focused_item_data();
    let duration_ms = started.elapsed().as_millis();

    let doc = FocusedItemDocument {
        schema_version: FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION,
        has_focused_item: focused_item.is_some(),
        focused_item,
        warnings: warnings.clone(),
    };

    let json = if diagnostics {
        let diag = FocusedItemDiagnosticsDocument {
            kind: "focused_item_diagnostics",
            uri: uri.to_string(),
            meta: FocusedItemDiagnosticsMeta {
                duration_ms,
                has_focused_item: doc.has_focused_item,
                warning_count: warnings.len(),
                source: "focused_item_slot".to_string(),
            },
            document: doc,
        };
        serde_json::to_string_pretty(&diag)
            .map_err(|e| format!("Failed to serialize focused item diagnostics: {e}"))?
    } else {
        serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("Failed to serialize focused item: {e}"))?
    };

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text: json,
    })
}

/// Global slot for the currently focused item, populated by surface resolvers.
static FOCUSED_ITEM_SLOT: parking_lot::Mutex<Option<FocusedItemInfo>> =
    parking_lot::Mutex::new(None);

/// Publish a focused item to the global slot so `kit://focused-item` can serve it.
#[allow(dead_code)] // Public API surface — called by Tab AI orchestration at runtime
pub fn publish_focused_item(item: FocusedItemInfo) {
    *FOCUSED_ITEM_SLOT.lock() = Some(item);
}

/// Clear the focused item slot (e.g., when the surface is dismissed).
#[allow(dead_code)] // Public API surface — called when surfaces are dismissed at runtime
pub fn clear_focused_item() {
    *FOCUSED_ITEM_SLOT.lock() = None;
}

// ---------------------------------------------------------------
// Provider-backed JSON resources: dictation, calendar, notifications
//
// Resolution priority:
// 1. In-process JSON slot (published by app features at runtime)
// 2. Environment variable (legacy / external script bridge)
// 3. Static empty fallback envelope
// ---------------------------------------------------------------

static DICTATION_JSON_SLOT: parking_lot::Mutex<Option<String>> = parking_lot::Mutex::new(None);
static CALENDAR_JSON_SLOT: parking_lot::Mutex<Option<String>> = parking_lot::Mutex::new(None);
static NOTIFICATIONS_JSON_SLOT: parking_lot::Mutex<Option<String>> = parking_lot::Mutex::new(None);

/// Publish dictation data into the in-process slot for `kit://dictation`.
pub fn publish_dictation_json(json: impl Into<String>) {
    *DICTATION_JSON_SLOT.lock() = Some(json.into());
}

/// Publish calendar data into the in-process slot for `kit://calendar`.
pub fn publish_calendar_json(json: impl Into<String>) {
    *CALENDAR_JSON_SLOT.lock() = Some(json.into());
}

/// Publish notifications data into the in-process slot for `kit://notifications`.
pub fn publish_notifications_json(json: impl Into<String>) {
    *NOTIFICATIONS_JSON_SLOT.lock() = Some(json.into());
}

/// Clear all provider JSON slots (e.g. on app reset).
pub fn clear_provider_json_slots() {
    *DICTATION_JSON_SLOT.lock() = None;
    *CALENDAR_JSON_SLOT.lock() = None;
    *NOTIFICATIONS_JSON_SLOT.lock() = None;
}

/// Provider-backed resource kinds that may or may not have real data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderJsonResourceKind {
    Dictation,
    Calendar,
    Notifications,
}

/// Returns `true` when the raw JSON text represents a provider payload
/// with real data, not just a placeholder envelope.
fn provider_json_text_has_real_data(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("available").and_then(|v| v.as_bool()) == Some(false) {
        return false;
    }

    let envelope_only = object.keys().all(|key| {
        matches!(
            key.as_str(),
            "schemaVersion"
                | "type"
                | "ok"
                | "available"
                | "source"
                | "items"
                | "note"
                | "nextStep"
        )
    });

    if let Some(items) = object.get("items").and_then(|v| v.as_array()) {
        if !items.is_empty() {
            return true;
        }
        if envelope_only {
            return false;
        }
    }

    if object.get("available").and_then(|v| v.as_bool()) == Some(true) && !envelope_only {
        return true;
    }

    // Treat any other non-empty provider object as real data too. Some
    // callers seed legacy payloads like {"transcription":"test"} or valid
    // empty-state payloads like {"events":[]} that should still surface the
    // provider-backed picker entries.
    !object.is_empty()
}

/// Resolve the raw JSON candidate text and its source label for a provider kind.
fn provider_json_candidate(kind: ProviderJsonResourceKind) -> (Option<String>, &'static str) {
    match kind {
        ProviderJsonResourceKind::Dictation => {
            if let Some(text) = DICTATION_JSON_SLOT.lock().clone() {
                (Some(text), "slot")
            } else {
                (std::env::var("SCRIPT_KIT_DICTATION_JSON").ok(), "env")
            }
        }
        ProviderJsonResourceKind::Calendar => {
            if let Some(text) = CALENDAR_JSON_SLOT.lock().clone() {
                (Some(text), "slot")
            } else {
                (std::env::var("SCRIPT_KIT_CALENDAR_JSON").ok(), "env")
            }
        }
        ProviderJsonResourceKind::Notifications => {
            if let Some(text) = NOTIFICATIONS_JSON_SLOT.lock().clone() {
                (Some(text), "slot")
            } else {
                (std::env::var("SCRIPT_KIT_NOTIFICATIONS_JSON").ok(), "env")
            }
        }
    }
}

/// Returns `true` when the provider has real data (parsed payload truth),
/// as opposed to only a placeholder or empty envelope.
pub fn has_provider_json_resource(kind: ProviderJsonResourceKind) -> bool {
    let (candidate, source) = provider_json_candidate(kind);
    let has_real_data = candidate
        .as_deref()
        .map(provider_json_text_has_real_data)
        .unwrap_or(false);
    tracing::info!(
        target: "ai",
        event = "mcp_provider_json_availability_checked",
        kind = ?kind,
        source,
        has_candidate = candidate.is_some(),
        has_real_data,
    );
    has_real_data
}

pub struct ProviderJsonItem {
    pub title: String,
    pub subtitle: Option<String>,
}

pub fn read_provider_json_items(kind: ProviderJsonResourceKind) -> Vec<ProviderJsonItem> {
    let (candidate, _source) = provider_json_candidate(kind);
    let Some(text) = candidate else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(items) = value.get("items").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())?;
            let subtitle = item
                .get("subtitle")
                .or_else(|| item.get("app"))
                .or_else(|| item.get("source"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(ProviderJsonItem {
                title: title.to_string(),
                subtitle,
            })
        })
        .collect()
}

/// Determine the resolution source for a provider-backed JSON resource.
fn provider_json_source(slot_value: &Option<String>, env_key: &str) -> &'static str {
    if slot_value.is_some() {
        "slot"
    } else if std::env::var_os(env_key).is_some() {
        "env"
    } else {
        "empty-fallback"
    }
}

/// Build an explicit empty-fallback JSON envelope with stable fields.
fn empty_provider_json(kind: &str, note: &str, next_step: &str, source: &str) -> String {
    format!(
        r#"{{"schemaVersion":1,"type":"{kind}","ok":true,"available":false,"source":"{source}","items":[],"note":"{note}","nextStep":"{next_step}"}}"#
    )
}

/// Read a JSON resource from an in-process slot, falling back to an environment
/// variable, then to a static empty envelope with explicit `source` tracking.
fn read_slot_or_env_backed_json_resource(
    uri: &str,
    slot_value: Option<String>,
    env_key: &str,
    kind: &str,
    note: &str,
    next_step: &str,
    event_name: &'static str,
) -> Result<ResourceContent, String> {
    let source = provider_json_source(&slot_value, env_key);
    let raw = slot_value.or_else(|| std::env::var(env_key).ok());
    let text = match raw {
        Some(text) if provider_json_text_has_real_data(&text) => text,
        Some(text) => {
            tracing::info!(
                target: "ai",
                event = "mcp_provider_json_placeholder_normalized",
                %uri,
                env_key,
                source,
                bytes = text.len(),
            );
            empty_provider_json(kind, note, next_step, source)
        }
        None => empty_provider_json(kind, note, next_step, source),
    };
    tracing::info!(
        target: "ai",
        event = %event_name,
        %uri,
        env_key,
        source,
        bytes = text.len(),
        "mcp_provider_json_resource_read"
    );
    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

fn read_dictation_resource(uri: &str) -> Result<ResourceContent, String> {
    read_slot_or_env_backed_json_resource(
        uri,
        DICTATION_JSON_SLOT.lock().clone(),
        "SCRIPT_KIT_DICTATION_JSON",
        "dictation",
        "No dictation provider configured.",
        "Publish dictation JSON or set SCRIPT_KIT_DICTATION_JSON.",
        "mcp_dictation_resource_read",
    )
}

fn read_calendar_resource(uri: &str) -> Result<ResourceContent, String> {
    read_slot_or_env_backed_json_resource(
        uri,
        CALENDAR_JSON_SLOT.lock().clone(),
        "SCRIPT_KIT_CALENDAR_JSON",
        "calendar",
        "No calendar provider configured.",
        "Publish calendar JSON or set SCRIPT_KIT_CALENDAR_JSON.",
        "mcp_calendar_resource_read",
    )
}

fn read_notifications_resource(uri: &str) -> Result<ResourceContent, String> {
    read_slot_or_env_backed_json_resource(
        uri,
        NOTIFICATIONS_JSON_SLOT.lock().clone(),
        "SCRIPT_KIT_NOTIFICATIONS_JSON",
        "notifications",
        "No notifications provider configured.",
        "Publish notifications JSON or set SCRIPT_KIT_NOTIFICATIONS_JSON.",
        "mcp_notifications_resource_read",
    )
}

// ---------------------------------------------------------------
// Shell-backed resources: git-status, git-diff, processes, system
// ---------------------------------------------------------------

/// Run a shell command and capture stdout, returning a fallback on failure.
fn run_shell_resource(program: &str, args: &[&str], uri: &str) -> Result<ResourceContent, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {program}: {e}"))?;

    let text = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!("Command exited with {}: {}", output.status, stderr.trim())
    };

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "text/plain".to_string(),
        text,
    })
}

/// Read `kit://git-status` — runs `git status` in the current directory.
fn read_git_status_resource() -> Result<ResourceContent, String> {
    run_shell_resource("git", &["status"], "kit://git-status")
}

/// Read `kit://git-diff` — runs `git diff` (combined staged + unstaged).
fn read_git_diff_resource(uri: &str) -> Result<ResourceContent, String> {
    // Show both staged and unstaged changes
    let staged = std::process::Command::new("git")
        .args(["diff", "--cached"])
        .output()
        .map_err(|e| format!("Failed to run git diff --cached: {e}"))?;
    let unstaged = std::process::Command::new("git")
        .args(["diff"])
        .output()
        .map_err(|e| format!("Failed to run git diff: {e}"))?;

    let mut text = String::new();
    let staged_out = String::from_utf8_lossy(&staged.stdout);
    let unstaged_out = String::from_utf8_lossy(&unstaged.stdout);
    let staged_err = String::from_utf8_lossy(&staged.stderr);
    let unstaged_err = String::from_utf8_lossy(&unstaged.stderr);

    if !staged.status.success() {
        text.push_str("=== Staged changes ===\n");
        text.push_str(&format!(
            "Command exited with {}: {}\n",
            staged.status,
            staged_err.trim()
        ));
    }

    if staged.status.success() && !staged_out.is_empty() {
        text.push_str("=== Staged changes ===\n");
        text.push_str(&staged_out);
    }
    if !unstaged.status.success() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("=== Unstaged changes ===\n");
        text.push_str(&format!(
            "Command exited with {}: {}\n",
            unstaged.status,
            unstaged_err.trim()
        ));
    }
    if unstaged.status.success() && !unstaged_out.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("=== Unstaged changes ===\n");
        text.push_str(&unstaged_out);
    }
    if text.is_empty() {
        text.push_str("No changes.");
    }
    let total_bytes = text.len();
    let limit = parse_u64_query_param(uri, "limitBytes")
        .map(|value| value as usize)
        .unwrap_or(GIT_DIFF_DEFAULT_LIMIT_BYTES)
        .clamp(1, GIT_DIFF_HARD_CAP_BYTES);
    let offset = parse_u64_query_param(uri, "offsetBytes")
        .map(|value| value as usize)
        .unwrap_or(0)
        .min(total_bytes);
    let end = next_char_boundary(&text, (offset + limit).min(total_bytes));
    let start = next_char_boundary(&text, offset);
    let truncated = end < total_bytes || start > 0;
    let mut bounded_text = text[start..end].to_string();
    if truncated {
        bounded_text.push_str(&format!(
            "\n\n[kit://git-diff truncated: offsetBytes={start}, limitBytes={limit}, totalBytes={total_bytes}]"
        ));
    }

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "text/plain".to_string(),
        text: bounded_text,
    })
}

fn next_char_boundary(text: &str, mut idx: usize) -> usize {
    idx = idx.min(text.len());
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

/// Read `kit://processes` — top processes by CPU.
fn read_processes_resource() -> Result<ResourceContent, String> {
    run_shell_resource("ps", &["aux", "--sort=-%cpu"], "kit://processes").or_else(|_| {
        // macOS ps doesn't support --sort; fall back to piped sort
        let ps = std::process::Command::new("ps")
            .args(["aux"])
            .output()
            .map_err(|e| format!("Failed to run ps: {e}"))?;
        let text = String::from_utf8_lossy(&ps.stdout).to_string();
        Ok(ResourceContent {
            uri: "kit://processes".to_string(),
            mime_type: "text/plain".to_string(),
            text,
        })
    })
}

/// Read `kit://system` — basic system info.
fn read_system_info_resource() -> Result<ResourceContent, String> {
    let mut lines = Vec::new();

    if let Ok(output) = std::process::Command::new("uname").args(["-a"]).output() {
        if output.status.success() {
            lines.push(format!(
                "System: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ));
        }
    }
    if let Ok(output) = std::process::Command::new("hostname").output() {
        if output.status.success() {
            lines.push(format!(
                "Hostname: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ));
        }
    }
    if let Ok(output) = std::process::Command::new("uptime").output() {
        if output.status.success() {
            lines.push(format!(
                "Uptime: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ));
        }
    }
    if let Ok(shell) = std::env::var("SHELL") {
        lines.push(format!("Shell: {shell}"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        lines.push(format!("CWD: {}", cwd.display()));
    }

    let text = if lines.is_empty() {
        "System info unavailable.".to_string()
    } else {
        lines.join("\n")
    };

    Ok(ResourceContent {
        uri: "kit://system".to_string(),
        mime_type: "text/plain".to_string(),
        text,
    })
}

/// Read the `kit://stdin-commands` drift-audited reference resource.
///
/// Emits markdown prose documenting the stdin JSONL envelope, with a
/// `<!-- drift-audit:stdin-verbs:start -->` … `:end` block that enumerates
/// every accepted `type` verb in the shape `- \`verbName\`: description`.
/// `tests/mcp_resource_drift.rs` pins the block against
/// [`crate::stdin_commands::all_external_command_verbs`].
fn read_stdin_commands_resource() -> Result<ResourceContent, String> {
    let mut body = String::new();
    body.push_str(
        "# Stdin JSONL Commands\n\n\
         Script Kit GPUI accepts one JSON object per line on stdin. Each \
         command is dispatched through `ExternalCommand` after the optional \
         `protocolVersion` gate in `src/stdin_commands/mod.rs`.\n\n\
         Example:\n\n\
         ```json\n\
         {\"type\":\"triggerBuiltin\",\"builtinId\":\"builtin/clipboard-history\"}\n\
         ```\n\n\
         The list below is the only source agents should trust for the \
         accepted `type` verb spelling. It is kept in sync with the \
         `ExternalCommand::command_type` match by the drift-audit in \
         `tests/mcp_resource_drift.rs`.\n\n\
         ## Verbs\n\n\
         <!-- drift-audit:stdin-verbs:start -->\n",
    );
    for verb in crate::stdin_commands::all_external_command_verbs() {
        body.push_str(&format!(
            "- `{verb}`: Dispatched as `ExternalCommand::{variant}` in `src/stdin_commands/mod.rs`.\n",
            variant = stdin_verb_variant_hint(verb),
        ));
    }
    body.push_str("<!-- drift-audit:stdin-verbs:end -->\n");

    Ok(ResourceContent {
        uri: STDIN_COMMANDS_REFERENCE_URI.to_string(),
        mime_type: "text/markdown".to_string(),
        text: body,
    })
}

/// Map a stdin verb back to its `ExternalCommand` variant name for the
/// resource prose. Purely cosmetic — the drift audit is on the verb, not
/// the hint string.
fn stdin_verb_variant_hint(verb: &str) -> &'static str {
    match verb {
        "run" => "Run",
        "show" => "Show",
        "hide" => "Hide",
        "setFilter" => "SetFilter",
        "triggerBuiltin" => "TriggerBuiltin",
        "simulateKey" => "SimulateKey",
        "openNotes" => "OpenNotes",
        "openAbout" => "OpenAbout",
        "openAi" => "OpenAi",
        "openMiniAi" => "OpenMiniAi",
        "openAiWithMockData" => "OpenAiWithMockData",
        "openMiniAiWithMockData" => "OpenMiniAiWithMockData",
        "showAiCommandBar" => "ShowAiCommandBar",
        "simulateAiKey" => "SimulateAiKey",
        "captureWindow" => "CaptureWindow",
        "setAiSearch" => "SetAiSearch",
        "setAiInput" => "SetAiInput",
        "setAgentChatInput" => "SetAgentChatInput",
        "getAiWindowState" => "GetAiWindowState",
        "showGrid" => "ShowGrid",
        "hideGrid" => "HideGrid",
        "showShortcutRecorder" => "ShowShortcutRecorder",
        "executeFallback" => "ExecuteFallback",
        "triggerAction" => "TriggerAction",
        "pasteClipboardIntoAgentChat" => "PasteClipboardIntoAgentChat",
        "pushDictationResult" => "PushDictationResult",
        "getConfigFingerprint" => "GetConfigFingerprint",
        _ => "(unknown)",
    }
}

/// Read the `kit://trigger-builtins` drift-audited reference resource.
///
/// Emits markdown prose listing every canonical `builtin/...` command id
/// accepted by the `triggerBuiltin` stdin verb, wrapped in a
/// `<!-- drift-audit:trigger-builtin-ids:start -->` block.
/// `tests/mcp_resource_drift.rs` pins the block against
/// [`crate::builtins::trigger_registry::all_trigger_builtin_command_ids`].
fn read_trigger_builtins_resource() -> Result<ResourceContent, String> {
    let mut body = String::new();
    body.push_str(
        "# Trigger Built-ins\n\n\
         Canonical `builtin/...` command IDs accepted by the `triggerBuiltin` \
         stdin verb. Legacy lowercase aliases (e.g. `clipboard`, `apps`) are \
         still resolved via the registry in \
         `src/builtins/trigger_registry.rs`, but new callers should use the \
         canonical IDs below.\n\n\
         Example:\n\n\
         ```json\n\
         {\"type\":\"triggerBuiltin\",\"builtinId\":\"builtin/clipboard-history\"}\n\
         ```\n\n\
         The list below is the only source agents should trust. It is kept \
         in sync with `TriggerBuiltin::ALL` by the drift-audit in \
         `tests/mcp_resource_drift.rs`.\n\n\
         ## Command IDs\n\n\
         <!-- drift-audit:trigger-builtin-ids:start -->\n",
    );
    for id in crate::builtins::trigger_registry::all_trigger_builtin_command_ids() {
        body.push_str(&format!(
            "- `{id}`: Canonical trigger-builtin command id.\n",
        ));
    }
    body.push_str("<!-- drift-audit:trigger-builtin-ids:end -->\n");

    Ok(ResourceContent {
        uri: TRIGGER_BUILTINS_REFERENCE_URI.to_string(),
        mime_type: "text/markdown".to_string(),
        text: body,
    })
}

/// Read the `kit://diagnostics/protocol-stats` resource
/// (Oracle-Session `protocol-builtin-boundary-refactor-plan` PR4).
///
/// Returns a serialized [`crate::protocol_stats::ProtocolStatsReport`]
/// so MCP consumers can render a live protocol-boundary health chip
/// without shelling out to logs. camelCase field names are baked into
/// the struct via `serde(rename_all = "camelCase")` so the wire shape
/// is stable.
fn read_protocol_stats_resource() -> Result<ResourceContent, String> {
    let report = crate::protocol_stats::current_report();
    let text = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("failed to serialize protocol stats report: {e}"))?;
    Ok(ResourceContent {
        uri: PROTOCOL_STATS_DIAGNOSTICS_URI.to_string(),
        mime_type: "application/json".to_string(),
        text,
    })
}

/// Convert resource content to JSON-RPC result format
pub fn resource_content_to_value(content: ResourceContent) -> Value {
    serde_json::json!({
        "contents": [{
            "uri": content.uri,
            "mimeType": content.mime_type,
            "text": content.text
        }]
    })
}
/// Convert resource list to JSON-RPC result format
pub fn resource_list_to_value(resources: &[McpResource]) -> Value {
    serde_json::to_value(serde_json::json!({
        "resources": resources
    }))
    .unwrap_or(serde_json::json!({"resources": []}))
}

// ---------------------------------------------------------------
// Context resource types and helpers
// ---------------------------------------------------------------

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextResourceKind {
    Snapshot,
    Schema,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContextResourceRequest {
    kind: ContextResourceKind,
    options: crate::context_snapshot::CaptureContextOptions,
    effective_profile: String,
    diagnostics: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextProfileDescriptor {
    name: &'static str,
    description: &'static str,
    options: crate::context_snapshot::CaptureContextOptions,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextParameterDescriptor {
    name: &'static str,
    value_type: &'static str,
    description: &'static str,
    default_value: &'static str,
    allowed_values: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextSchemaDocument {
    kind: &'static str,
    schema_version: u32,
    default_profile: &'static str,
    diagnostics_supported: bool,
    profiles: Vec<ContextProfileDescriptor>,
    parameters: Vec<ContextParameterDescriptor>,
    examples: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ContextFieldCaptureState {
    Disabled,
    Captured,
    Empty,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextFieldStatus {
    field: &'static str,
    enabled: bool,
    present: bool,
    state: ContextFieldCaptureState,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextWarningDescriptor {
    field: String,
    code: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ContextDiagnosticsStatus {
    Ok,
    Partial,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextDiagnosticsMeta {
    effective_profile: String,
    options: crate::context_snapshot::CaptureContextOptions,
    status: ContextDiagnosticsStatus,
    duration_ms: u128,
    snapshot_bytes: usize,
    enabled_field_count: usize,
    warning_count: usize,
    field_statuses: Vec<ContextFieldStatus>,
    warnings: Vec<ContextWarningDescriptor>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ContextDiagnosticsDocument {
    kind: &'static str,
    uri: String,
    snapshot: crate::context_snapshot::AiContextSnapshot,
    meta: ContextDiagnosticsMeta,
}

fn supported_context_examples() -> Vec<&'static str> {
    vec![
        "kit://context",
        "kit://context?profile=minimal",
        "kit://context?profile=minimal&diagnostics=1",
        "kit://context?selectedText=0&browserUrl=1&focusedWindow=1",
        "kit://context?screenshot=1",
        "kit://context?panelScreenshot=1",
        "kit://context?screenshot=1&panelScreenshot=1",
        "kit://context?screenshot=1&panelScreenshot=1&diagnostics=1",
        "kit://context/schema",
    ]
}

fn supported_context_param_names() -> &'static [&'static str] {
    &[
        "profile",
        "diagnostics",
        "selectedText",
        "frontmostApp",
        "menuBar",
        "browserUrl",
        "focusedWindow",
        "screenshot",
        "panelScreenshot",
    ]
}

fn invalid_context_param(key: &str, _value: &str) -> String {
    format!(
        "Invalid kit://context parameter: {key}. Supported parameters: {}. See kit://context/schema for the full contract and examples.",
        supported_context_param_names().join(", ")
    )
}

fn parse_bool_param(value: &str) -> Result<bool, String> {
    match value {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => Err(format!(
            "Invalid boolean value: {value}. Expected one of: 1, 0, true, false. See kit://context/schema."
        )),
    }
}

fn parse_context_resource_request(uri: &str) -> Result<ContextResourceRequest, String> {
    use crate::context_snapshot::CaptureContextOptions;

    if uri == "kit://context/schema" {
        return Ok(ContextResourceRequest {
            kind: ContextResourceKind::Schema,
            options: CaptureContextOptions::default(),
            effective_profile: "full".to_string(),
            diagnostics: false,
        });
    }

    if uri == "kit://context" {
        return Ok(ContextResourceRequest {
            kind: ContextResourceKind::Snapshot,
            options: CaptureContextOptions::default(),
            effective_profile: "full".to_string(),
            diagnostics: false,
        });
    }

    let (base, query) = uri
        .split_once('?')
        .ok_or_else(|| format!("Resource not found: {uri}"))?;

    if base == "kit://context/schema" {
        return Err(
            "kit://context/schema does not accept query parameters. Use plain kit://context/schema."
                .to_string(),
        );
    }

    if base != "kit://context" {
        return Err(format!("Resource not found: {uri}"));
    }

    let mut options = CaptureContextOptions::default();
    let mut selected_profile: Option<&str> = None;
    let mut diagnostics = false;
    let mut saw_override = false;
    let mut saw_explicit_screenshot = false;
    let mut saw_explicit_panel_screenshot = false;

    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, "1"));

        match (key, value) {
            ("profile", "full") => {
                options = CaptureContextOptions::all();
                selected_profile = Some("full");
            }
            ("profile", "minimal") => {
                options = CaptureContextOptions::minimal();
                selected_profile = Some("minimal");
            }
            ("profile", other) => {
                return Err(format!(
                    "Unknown profile: {other}. Supported profiles: full, minimal. See kit://context/schema."
                ));
            }
            ("diagnostics", v) => diagnostics = parse_bool_param(v)?,
            ("selectedText", v) => {
                options.include_selected_text = parse_bool_param(v)?;
                saw_override = true;
            }
            ("frontmostApp", v) => {
                options.include_frontmost_app = parse_bool_param(v)?;
                saw_override = true;
            }
            ("menuBar", v) => {
                options.include_menu_bar = parse_bool_param(v)?;
                saw_override = true;
            }
            ("browserUrl", v) => {
                options.include_browser_url = parse_bool_param(v)?;
                saw_override = true;
            }
            ("focusedWindow", v) => {
                options.include_focused_window = parse_bool_param(v)?;
                saw_override = true;
            }
            ("screenshot", v) => {
                options.include_screenshot = parse_bool_param(v)?;
                saw_override = true;
                saw_explicit_screenshot = true;
            }
            ("panelScreenshot", v) => {
                options.include_panel_screenshot = parse_bool_param(v)?;
                saw_override = true;
                saw_explicit_panel_screenshot = true;
            }
            _ => return Err(invalid_context_param(key, value)),
        }
    }

    // Pixel data must be opted into explicitly on custom (per-field) queries.
    // The baseline for overrides is `all()`, which includes the focused-window
    // screenshot — inherited silently, a metadata query like `?selectedText=1&
    // focusedWindow=0` used to embed a full-window base64 PNG and blow the
    // model's context window (758KB observed from the @selection attachment).
    // An explicit `profile=` keeps its documented pixel semantics.
    if (saw_override || diagnostics) && selected_profile.is_none() {
        if !saw_explicit_screenshot {
            options.include_screenshot = false;
        }
        if !saw_explicit_panel_screenshot {
            options.include_panel_screenshot = false;
        }
    }

    let effective_profile = if saw_override {
        "custom".to_string()
    } else {
        selected_profile.unwrap_or("full").to_string()
    };

    Ok(ContextResourceRequest {
        kind: ContextResourceKind::Snapshot,
        options,
        effective_profile,
        diagnostics,
    })
}

fn build_context_schema_document() -> ContextSchemaDocument {
    ContextSchemaDocument {
        kind: "context_schema",
        schema_version: crate::context_snapshot::AI_CONTEXT_SNAPSHOT_SCHEMA_VERSION,
        default_profile: "full",
        diagnostics_supported: true,
        profiles: vec![
            ContextProfileDescriptor {
                name: "full",
                description: "Capture every currently supported context provider.",
                options: crate::context_snapshot::CaptureContextOptions::all(),
            },
            ContextProfileDescriptor {
                name: "minimal",
                description: "Lower-cost profile that omits selected text and menu bar.",
                options: crate::context_snapshot::CaptureContextOptions::minimal(),
            },
        ],
        parameters: vec![
            ContextParameterDescriptor {
                name: "profile",
                value_type: "enum",
                description: "Named bundle of capture flags.",
                default_value: "full",
                allowed_values: vec!["full", "minimal"],
            },
            ContextParameterDescriptor {
                name: "diagnostics",
                value_type: "boolean",
                description: "Wrap the snapshot in machine-readable metadata, warnings, and field-level status.",
                default_value: "false",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "selectedText",
                value_type: "boolean",
                description: "Include the current selection, if the platform/provider can read it.",
                default_value: "1",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "frontmostApp",
                value_type: "boolean",
                description: "Include the frontmost application identity.",
                default_value: "1",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "menuBar",
                value_type: "boolean",
                description: "Include summarized menu bar items for the frontmost app.",
                default_value: "1",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "browserUrl",
                value_type: "boolean",
                description: "Include the focused browser tab URL when available.",
                default_value: "1",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "focusedWindow",
                value_type: "boolean",
                description: "Include focused-window metadata derived from window capture.",
                default_value: "1",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "screenshot",
                value_type: "boolean",
                description: "Include focused-window screenshot bytes as base64 PNG in focusedWindowImage.",
                default_value: "false",
                allowed_values: vec!["1", "0", "true", "false"],
            },
            ContextParameterDescriptor {
                name: "panelScreenshot",
                value_type: "boolean",
                description: "Include Script Kit's visible panel screenshot as base64 PNG in scriptKitPanelImage.",
                default_value: "false",
                allowed_values: vec!["1", "0", "true", "false"],
            },
        ],
        examples: supported_context_examples(),
    }
}

fn context_warning_code(field: &str) -> &'static str {
    match field {
        "selectedText" => "selected_text_capture_failed",
        "frontmostApp" => "frontmost_app_capture_failed",
        "menuBar" => "menu_bar_capture_failed",
        "browserUrl" => "browser_url_capture_failed",
        "focusedWindow" => "focused_window_capture_failed",
        "screenshot" => "screenshot_capture_failed",
        "panelScreenshot" => "panel_screenshot_capture_failed",
        _ => "capture_failed",
    }
}

fn parse_context_warning(raw: &str) -> ContextWarningDescriptor {
    let (field, message) = raw
        .split_once(':')
        .map(|(field, message)| (field.trim(), message.trim()))
        .unwrap_or(("unknown", raw.trim()));

    ContextWarningDescriptor {
        field: field.to_string(),
        code: context_warning_code(field).to_string(),
        message: message.to_string(),
    }
}

fn build_context_field_status(
    field: &'static str,
    enabled: bool,
    present: bool,
    warnings_by_field: &HashMap<String, ContextWarningDescriptor>,
) -> ContextFieldStatus {
    let state = if !enabled {
        ContextFieldCaptureState::Disabled
    } else if warnings_by_field.contains_key(field) {
        ContextFieldCaptureState::Failed
    } else if present {
        ContextFieldCaptureState::Captured
    } else {
        ContextFieldCaptureState::Empty
    };

    ContextFieldStatus {
        field,
        enabled,
        present,
        state,
    }
}

fn build_context_field_statuses(
    options: &crate::context_snapshot::CaptureContextOptions,
    snapshot: &crate::context_snapshot::AiContextSnapshot,
    warnings_by_field: &HashMap<String, ContextWarningDescriptor>,
) -> Vec<ContextFieldStatus> {
    vec![
        build_context_field_status(
            "selectedText",
            options.include_selected_text,
            snapshot.selected_text.is_some(),
            warnings_by_field,
        ),
        build_context_field_status(
            "frontmostApp",
            options.include_frontmost_app,
            snapshot.frontmost_app.is_some(),
            warnings_by_field,
        ),
        build_context_field_status(
            "menuBar",
            options.include_menu_bar,
            !snapshot.menu_bar_items.is_empty(),
            warnings_by_field,
        ),
        build_context_field_status(
            "browserUrl",
            options.include_browser_url,
            snapshot.browser.is_some(),
            warnings_by_field,
        ),
        build_context_field_status(
            "focusedWindow",
            options.include_focused_window,
            snapshot.focused_window.is_some(),
            warnings_by_field,
        ),
        build_context_field_status(
            "screenshot",
            options.include_screenshot,
            snapshot.focused_window_image.is_some(),
            warnings_by_field,
        ),
        build_context_field_status(
            "panelScreenshot",
            options.include_panel_screenshot,
            snapshot.script_kit_panel_image.is_some(),
            warnings_by_field,
        ),
    ]
}

fn build_context_diagnostics_document(
    uri: &str,
    request: &ContextResourceRequest,
    snapshot: &crate::context_snapshot::AiContextSnapshot,
    duration_ms: u128,
) -> ContextDiagnosticsDocument {
    let warnings: Vec<ContextWarningDescriptor> = snapshot
        .warnings
        .iter()
        .map(|warning| parse_context_warning(warning))
        .collect();

    let warnings_by_field: HashMap<String, ContextWarningDescriptor> = warnings
        .iter()
        .cloned()
        .map(|warning| (warning.field.clone(), warning))
        .collect();

    let enabled_field_count = [
        request.options.include_selected_text,
        request.options.include_frontmost_app,
        request.options.include_menu_bar,
        request.options.include_browser_url,
        request.options.include_focused_window,
        request.options.include_screenshot,
        request.options.include_panel_screenshot,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count();

    let snapshot_bytes = serde_json::to_vec(snapshot)
        .map(|bytes| bytes.len())
        .unwrap_or_default();

    let warning_count = warnings.len();
    let field_statuses =
        build_context_field_statuses(&request.options, snapshot, &warnings_by_field);

    ContextDiagnosticsDocument {
        kind: "context_diagnostics",
        uri: uri.to_string(),
        snapshot: snapshot.clone(),
        meta: ContextDiagnosticsMeta {
            effective_profile: request.effective_profile.clone(),
            options: request.options.clone(),
            status: if warnings.is_empty() {
                ContextDiagnosticsStatus::Ok
            } else {
                ContextDiagnosticsStatus::Partial
            },
            duration_ms,
            snapshot_bytes,
            enabled_field_count,
            warning_count,
            field_statuses,
            warnings,
        },
    }
}

fn serialize_context_resource(
    uri: &str,
    request: &ContextResourceRequest,
    snapshot: Option<&crate::context_snapshot::AiContextSnapshot>,
    duration_ms: u128,
) -> Result<String, String> {
    match request.kind {
        ContextResourceKind::Schema => {
            serde_json::to_string_pretty(&build_context_schema_document())
                .map_err(|error| format!("Failed to serialize context schema: {error}"))
        }
        ContextResourceKind::Snapshot => {
            let snapshot = snapshot.ok_or_else(|| {
                "Context snapshot missing while serializing response.".to_string()
            })?;

            if request.diagnostics {
                serde_json::to_string_pretty(&build_context_diagnostics_document(
                    uri,
                    request,
                    snapshot,
                    duration_ms,
                ))
                .map_err(|error| format!("Failed to serialize context diagnostics: {error}"))
            } else {
                serde_json::to_string_pretty(snapshot)
                    .map_err(|error| format!("Failed to serialize context snapshot: {error}"))
            }
        }
    }
}

/// Read kit://context or kit://context/schema resource
fn read_context_resource(uri: &str) -> Result<ResourceContent, String> {
    let request = parse_context_resource_request(uri).map_err(|error| {
        tracing::warn!(
            target: "script_kit::mcp_context_resource",
            uri = %uri,
            error = %error,
            "context_resource_read_invalid_request"
        );
        error
    })?;

    if matches!(request.kind, ContextResourceKind::Schema) {
        tracing::info!(
            target: "script_kit::mcp_context_resource",
            uri = %uri,
            "context_resource_schema_read"
        );

        return Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: "application/json".to_string(),
            text: serialize_context_resource(uri, &request, None, 0)?,
        });
    }

    tracing::info!(
        target: "script_kit::mcp_context_resource",
        uri = %uri,
        diagnostics = request.diagnostics,
        effective_profile = %request.effective_profile,
        selected_text = request.options.include_selected_text,
        frontmost_app = request.options.include_frontmost_app,
        menu_bar = request.options.include_menu_bar,
        browser_url = request.options.include_browser_url,
        focused_window = request.options.include_focused_window,
        "context_resource_read_start"
    );

    let started = Instant::now();
    let snapshot = crate::context_snapshot::capture_context_snapshot(&request.options);
    let duration_ms = started.elapsed().as_millis();

    tracing::info!(
        target: "script_kit::mcp_context_resource",
        uri = %uri,
        diagnostics = request.diagnostics,
        effective_profile = %request.effective_profile,
        duration_ms = duration_ms,
        warning_count = snapshot.warnings.len(),
        status = if snapshot.warnings.is_empty() { "ok" } else { "partial" },
        "context_resource_read_complete"
    );

    Ok(ResourceContent {
        uri: uri.to_string(),
        mime_type: "application/json".to_string(),
        text: serialize_context_resource(uri, &request, Some(&snapshot), duration_ms)?,
    })
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Helper to wrap Vec<Script> into Vec<Arc<Script>> for tests
    fn wrap_scripts(scripts: Vec<Script>) -> Vec<Arc<Script>> {
        scripts.into_iter().map(Arc::new).collect()
    }

    /// Helper to wrap Vec<Scriptlet> into Vec<Arc<Scriptlet>> for tests
    fn wrap_scriptlets(scriptlets: Vec<Scriptlet>) -> Vec<Arc<Scriptlet>> {
        scriptlets.into_iter().map(Arc::new).collect()
    }

    fn provider_json_test_lock() -> &'static std::sync::Mutex<()> {
        crate::test_utils::PROVIDER_JSON_TEST_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn unique_notes_resource_token(prefix: &str) -> String {
        format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
    }

    // =======================================================
    // TDD Tests - Written FIRST per spec requirements
    // =======================================================

    /// Helper to create a test script
    fn test_script(name: &str, description: Option<&str>) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(format!(
                "/test/{}.ts",
                name.to_lowercase().replace(' ', "-")
            )),
            extension: "ts".to_string(),
            description: description.map(|s| s.to_string()),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
            plugin_id: String::new(),
            plugin_title: None,
            kit_name: None,
            body: None,
        }
    }

    /// Helper to create a test scriptlet
    fn test_scriptlet(name: &str, tool: &str, description: Option<&str>) -> Scriptlet {
        Scriptlet {
            icon: None,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            code: "echo test".to_string(),
            tool: tool.to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: String::new(),
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
        }
    }

    #[test]
    fn test_resources_list_includes_all() {
        // REQUIREMENT: resources/list returns the full MCP resource registry.
        let resources = get_resource_definitions();

        assert_eq!(
            resources.len(),
            29,
            "Resource registry count should be updated when new MCP resources land"
        );

        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"kit://state"), "Should include kit://state");
        assert!(uris.contains(&"kit://notes"), "Should include kit://notes");
        assert!(uris.contains(&"kit://brain"), "Should include kit://brain");
        assert!(uris.contains(&"kit://audit"), "Should include kit://audit");
        assert!(uris.contains(&"scripts://"), "Should include scripts://");
        assert!(
            uris.contains(&"scriptlets://"),
            "Should include scriptlets://"
        );
        assert!(
            uris.contains(&"kit://transactions/latest"),
            "Should include kit://transactions/latest"
        );
        assert!(
            uris.contains(&"kit://transactions/schema"),
            "Should include kit://transactions/schema"
        );

        // Verify all have required fields
        for resource in &resources {
            assert!(!resource.name.is_empty(), "Resource should have a name");
            assert!(
                resource.mime_type == "application/json"
                    || resource.mime_type == "text/plain"
                    || resource.mime_type == "text/markdown",
                "Should be JSON, text, or markdown mime type, got: {}",
                resource.mime_type
            );
            assert!(resource.description.is_some(), "Should have a description");
        }
    }

    #[test]
    fn brain_resource_description_lists_provenance_reads() {
        let resources = get_resource_definitions();
        let brain = resources
            .iter()
            .find(|resource| resource.uri == "kit://brain")
            .expect("brain resource definition");
        let description = brain.description.as_deref().unwrap_or("");
        assert!(description.contains("format=json"));
        assert!(description.contains("kit://brain/doc"));
        assert!(description.contains("kit://brain/docs"));
    }

    #[test]
    fn test_scripts_resource_read() {
        // REQUIREMENT: scripts:// returns array of script metadata
        let scripts = wrap_scripts(vec![
            test_script("My Script", Some("Does something")),
            test_script("Another Script", None),
        ]);

        let result = read_resource("scripts://", &scripts, &[], None);
        assert!(result.is_ok(), "Should successfully read scripts resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "scripts://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "My Script");
        assert_eq!(parsed[0].description, Some("Does something".to_string()));
        assert_eq!(parsed[1].name, "Another Script");
        assert_eq!(parsed[1].description, None);
    }

    #[test]
    fn test_scriptlets_resource_read() {
        // REQUIREMENT: scriptlets:// returns array of scriptlet metadata
        let scriptlets = wrap_scriptlets(vec![
            test_scriptlet("Open URL", "open", Some("Opens a URL")),
            test_scriptlet("Paste Text", "paste", None),
        ]);

        let result = read_resource("scriptlets://", &[], &scriptlets, None);
        assert!(
            result.is_ok(),
            "Should successfully read scriptlets resource"
        );

        let content = result.unwrap();
        assert_eq!(content.uri, "scriptlets://");
        assert_eq!(content.mime_type, "application/json");

        // Parse the JSON and verify structure
        let parsed: Vec<ScriptletResourceEntry> =
            serde_json::from_str(&content.text).expect("Should be valid JSON array");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Open URL");
        assert_eq!(parsed[0].tool, "open");
        assert_eq!(parsed[0].description, Some("Opens a URL".to_string()));
        assert_eq!(parsed[1].name, "Paste Text");
        assert_eq!(parsed[1].tool, "paste");
    }

    #[test]
    fn test_state_resource_read() {
        // REQUIREMENT: kit://state returns current app state
        let app_state = AppStateResource {
            visible: true,
            focused: true,
            script_count: 10,
            scriptlet_count: 5,
            filter_text: Some("test".to_string()),
            selected_index: Some(3),
        };

        let result = read_resource("kit://state", &[], &[], Some(&app_state));
        assert!(result.is_ok(), "Should successfully read state resource");

        let content = result.unwrap();
        assert_eq!(content.uri, "kit://state");
        assert_eq!(content.mime_type, "application/json");

        // Parse and verify
        let parsed: AppStateResource =
            serde_json::from_str(&content.text).expect("Should be valid JSON");

        assert!(parsed.visible);
        assert!(parsed.focused);
        assert_eq!(parsed.script_count, 10);
        assert_eq!(parsed.scriptlet_count, 5);
        assert_eq!(parsed.filter_text, Some("test".to_string()));
        assert_eq!(parsed.selected_index, Some(3));
    }

    #[test]
    fn test_state_resource_read_default() {
        // When no app state is provided, should return defaults
        let result = read_resource("kit://state", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: AppStateResource = serde_json::from_str(&content.text).unwrap();

        assert!(!parsed.visible);
        assert!(!parsed.focused);
        assert_eq!(parsed.script_count, 0);
        assert_eq!(parsed.scriptlet_count, 0);
        assert_eq!(parsed.filter_text, None);
        assert_eq!(parsed.selected_index, None);
    }

    #[test]
    fn test_unknown_resource_returns_error() {
        // REQUIREMENT: Unknown URI returns error
        let result = read_resource("unknown://resource", &[], &[], None);

        assert!(result.is_err(), "Unknown resource should return error");
        let error = result.unwrap_err();
        assert!(
            error.contains("Resource not found"),
            "Error should mention resource not found"
        );
        assert!(
            error.contains("unknown://resource"),
            "Error should include the URI"
        );
    }

    #[test]
    fn test_resource_content_to_value() {
        let content = ResourceContent {
            uri: "test://uri".to_string(),
            mime_type: "application/json".to_string(),
            text: r#"{"foo":"bar"}"#.to_string(),
        };

        let value = resource_content_to_value(content);

        // Should have contents array
        let contents = value.get("contents").and_then(|c| c.as_array());
        assert!(contents.is_some());

        let contents = contents.unwrap();
        assert_eq!(contents.len(), 1);

        let first = &contents[0];
        assert_eq!(
            first.get("uri").and_then(|u| u.as_str()),
            Some("test://uri")
        );
        assert_eq!(
            first.get("mimeType").and_then(|m| m.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn test_resource_list_to_value() {
        let resources = get_resource_definitions();
        let value = resource_list_to_value(&resources);

        // Should have resources array
        let resource_array = value.get("resources").and_then(|r| r.as_array());
        assert!(resource_array.is_some());

        let resource_array = resource_array.unwrap();
        assert_eq!(resource_array.len(), resources.len());

        // First resource should have expected fields
        let first = &resource_array[0];
        assert!(first.get("uri").is_some());
        assert!(first.get("name").is_some());
        assert!(first.get("mimeType").is_some());
    }

    // =======================================================
    // Additional Unit Tests
    // =======================================================

    #[test]
    fn test_script_resource_entry_from_script() {
        use crate::schema_parser::{FieldDef, FieldType, Schema};
        use std::collections::HashMap;

        // Script without schema
        let script_no_schema = test_script("No Schema", Some("Test"));
        let entry: ScriptResourceEntry = (&script_no_schema).into();
        assert!(!entry.has_schema);

        // Script with schema
        let mut input = HashMap::new();
        input.insert(
            "name".to_string(),
            FieldDef {
                field_type: FieldType::String,
                required: true,
                ..Default::default()
            },
        );

        let script_with_schema = Script {
            name: "With Schema".to_string(),
            path: PathBuf::from("/test/with-schema.ts"),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: Some(Schema {
                input,
                output: HashMap::new(),
            }),
            plugin_id: String::new(),
            plugin_title: None,
            kit_name: None,
            body: None,
        };

        let entry: ScriptResourceEntry = (&script_with_schema).into();
        assert!(entry.has_schema);
    }

    #[test]
    fn test_scriptlet_resource_entry_from_scriptlet() {
        let scriptlet = Scriptlet {
            icon: None,
            name: "Full Scriptlet".to_string(),
            description: Some("Test description".to_string()),
            code: "echo test".to_string(),
            tool: "bash".to_string(),
            shortcut: Some("cmd k".to_string()),
            keyword: Some(":test".to_string()),
            group: Some("My Group".to_string()),
            plugin_id: String::new(),
            plugin_title: None,
            file_path: None,
            command: None,
            alias: None,
        };

        let entry: ScriptletResourceEntry = (&scriptlet).into();

        assert_eq!(entry.name, "Full Scriptlet");
        assert_eq!(entry.description, Some("Test description".to_string()));
        assert_eq!(entry.tool, "bash");
        assert_eq!(entry.shortcut, Some("cmd k".to_string()));
        assert_eq!(entry.keyword, Some(":test".to_string()));
        assert_eq!(entry.group, Some("My Group".to_string()));
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = McpResource {
            uri: "test://".to_string(),
            name: "Test".to_string(),
            description: Some("Test description".to_string()),
            mime_type: "application/json".to_string(),
        };

        let json = serde_json::to_string(&resource).unwrap();

        // Should have mimeType (camelCase)
        assert!(json.contains("\"mimeType\""));
        assert!(!json.contains("\"mime_type\""));

        // Deserialize back
        let parsed: McpResource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.uri, "test://");
        assert_eq!(parsed.mime_type, "application/json");
    }

    #[test]
    fn test_empty_scripts_resource() {
        let result = read_resource("scripts://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_empty_scriptlets_resource() {
        let result = read_resource("scriptlets://", &[], &[], None);
        assert!(result.is_ok());

        let content = result.unwrap();
        let parsed: Vec<ScriptletResourceEntry> = serde_json::from_str(&content.text).unwrap();
        assert!(parsed.is_empty());
    }

    // =======================================================
    // Context resource URI parsing tests
    // =======================================================

    #[test]
    fn parse_context_bare_uri_returns_default() {
        let request = parse_context_resource_request("kit://context").unwrap();
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::default()
        );
        assert_eq!(request.effective_profile, "full");
        assert!(!request.diagnostics);
    }

    #[test]
    fn parse_context_resource_options_supports_minimal_profile() {
        let request = parse_context_resource_request("kit://context?profile=minimal").unwrap();
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
        assert_eq!(request.effective_profile, "minimal");
    }

    #[test]
    fn parse_context_resource_options_allows_profile_overrides() {
        let request = parse_context_resource_request(
            "kit://context?profile=minimal&menuBar=1&selectedText=0",
        )
        .unwrap();

        assert!(!request.options.include_selected_text);
        assert!(request.options.include_menu_bar);
        assert!(request.options.include_frontmost_app);
        assert!(request.options.include_browser_url);
        assert!(request.options.include_focused_window);
        assert_eq!(request.effective_profile, "custom");
    }

    #[test]
    fn parse_context_resource_options_rejects_unknown_flags() {
        let error = parse_context_resource_request("kit://context?nope=1").unwrap_err();
        assert!(
            error.contains("Invalid kit://context parameter: nope"),
            "Error should mention the invalid parameter"
        );
    }

    #[test]
    fn parse_context_rejects_unknown_profile() {
        let error = parse_context_resource_request("kit://context?profile=heavy").unwrap_err();
        assert!(error.contains("Unknown profile"), "Error: {error}");
    }

    #[test]
    fn context_resource_preserves_query_uri() {
        crate::context_snapshot::enable_deterministic_context_capture();
        let content =
            read_resource("kit://context?profile=minimal", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://context?profile=minimal");
    }

    #[test]
    fn is_context_resource_uri_only_matches_supported_forms() {
        assert!(is_context_resource_uri("kit://context"));
        assert!(is_context_resource_uri("kit://context?profile=minimal"));
        assert!(is_context_resource_uri("kit://context/schema"));
        assert!(!is_context_resource_uri("kit://contextual"));
        assert!(!is_context_resource_uri("kit://context-schema"));
        assert!(!is_context_resource_uri("unknown://context"));
    }

    // =======================================================
    // Context resource: diagnostics, schema, and self-describing tests
    // =======================================================

    #[test]
    fn parse_context_resource_request_supports_diagnostics_flag() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&diagnostics=1").unwrap();

        assert!(matches!(request.kind, ContextResourceKind::Snapshot));
        assert_eq!(
            request.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
        assert_eq!(request.effective_profile, "minimal");
        assert!(request.diagnostics);
    }

    #[test]
    fn parse_context_resource_request_marks_profile_override_as_custom() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&selectedText=1").unwrap();

        assert_eq!(request.effective_profile, "custom");
        assert!(request.options.include_selected_text);
    }

    #[test]
    fn parse_context_resource_request_supports_schema_uri() {
        let request = parse_context_resource_request("kit://context/schema").unwrap();
        assert!(matches!(request.kind, ContextResourceKind::Schema));
    }

    /// Per-field queries inherit their baseline from `all()`, which includes
    /// pixel capture. Pixel data must be explicit opt-in: the `@selection`
    /// attachment URI once inherited `include_screenshot` silently and shipped
    /// a 758KB base64 PNG as prompt text, overflowing the model's context.
    #[test]
    fn parse_context_resource_request_field_overrides_disable_pixels_unless_explicit() {
        let request = parse_context_resource_request(
            "kit://context?selectedText=1&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0",
        )
        .unwrap();
        assert!(request.options.include_selected_text);
        assert!(!request.options.include_screenshot);
        assert!(!request.options.include_panel_screenshot);

        let diagnostics = parse_context_resource_request("kit://context?diagnostics=1").unwrap();
        assert!(!diagnostics.options.include_screenshot);
        assert!(!diagnostics.options.include_panel_screenshot);

        let explicit = parse_context_resource_request(
            "kit://context?screenshot=1&selectedText=0&frontmostApp=0&menuBar=0&browserUrl=0&focusedWindow=0",
        )
        .unwrap();
        assert!(explicit.options.include_screenshot);
        assert!(!explicit.options.include_panel_screenshot);

        // An explicit profile keeps its documented pixel semantics.
        let minimal = parse_context_resource_request("kit://context?profile=minimal").unwrap();
        assert_eq!(
            minimal.options,
            crate::context_snapshot::CaptureContextOptions::minimal()
        );
    }

    #[test]
    fn serialize_context_resource_diagnostics_includes_machine_readable_meta() {
        let request =
            parse_context_resource_request("kit://context?profile=minimal&diagnostics=1").unwrap();

        let snapshot = crate::context_snapshot::AiContextSnapshot {
            schema_version: crate::context_snapshot::AI_CONTEXT_SNAPSHOT_SCHEMA_VERSION,
            frontmost_app: Some(crate::context_snapshot::FrontmostAppContext {
                pid: 42,
                bundle_id: "com.example.App".to_string(),
                name: "Example App".to_string(),
            }),
            browser: Some(crate::context_snapshot::BrowserContext::from_url(
                "https://example.com".to_string(),
            )),
            warnings: vec!["focusedWindow: permission denied".to_string()],
            ..Default::default()
        };

        let json = serialize_context_resource(
            "kit://context?profile=minimal&diagnostics=1",
            &request,
            Some(&snapshot),
            12,
        )
        .unwrap();

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["kind"], "context_diagnostics");
        assert_eq!(value["meta"]["effectiveProfile"], "minimal");
        assert_eq!(value["meta"]["status"], "partial");
        assert_eq!(value["meta"]["durationMs"], 12);
        // minimal() enables frontmostApp, browserUrl, focusedWindow, and (since
        // 19db0e0e5, "Enable screenshots in @here (minimal) ... profiles")
        // screenshot — 4 fields total.
        assert_eq!(value["meta"]["enabledFieldCount"], 4);
        assert_eq!(value["meta"]["warningCount"], 1);
        assert_eq!(value["meta"]["fieldStatuses"][0]["field"], "selectedText");
        assert_eq!(value["meta"]["fieldStatuses"][0]["state"], "disabled");
        assert_eq!(value["meta"]["fieldStatuses"][4]["field"], "focusedWindow");
        assert_eq!(value["meta"]["fieldStatuses"][4]["state"], "failed");
        assert_eq!(
            value["meta"]["warnings"][0]["code"],
            "focused_window_capture_failed"
        );
        assert_eq!(value["meta"]["warnings"][0]["message"], "permission denied");
    }

    #[test]
    fn serialize_context_schema_includes_diagnostics_parameter() {
        let request = parse_context_resource_request("kit://context/schema").unwrap();

        let json = serialize_context_resource("kit://context/schema", &request, None, 0).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["kind"], "context_schema");
        assert_eq!(value["diagnosticsSupported"], true);

        let parameter_names: Vec<&str> = value["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|param| param["name"].as_str())
            .collect();

        assert!(parameter_names.contains(&"diagnostics"));

        let has_diagnostics_example =
            value["examples"].as_array().unwrap().iter().any(|example| {
                example.as_str() == Some("kit://context?profile=minimal&diagnostics=1")
            });

        assert!(has_diagnostics_example);
    }

    // =======================================================
    // Schema-versioned script/scriptlet/sdk-reference resources
    // =======================================================

    #[test]
    fn kit_scripts_resource_returns_schema_versioned_envelope() {
        let scripts = wrap_scripts(vec![
            test_script("Hello World", Some("A greeting script")),
            test_script("Fetch Data", None),
        ]);

        let content = read_resource("kit://scripts", &scripts, &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://scripts");
        assert_eq!(content.mime_type, "application/json");

        let doc: ScriptsResourceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 2);
        assert_eq!(doc.scripts.len(), 2);
        assert_eq!(doc.scripts[0].name, "Hello World");
        assert_eq!(
            doc.scripts[0].description,
            Some("A greeting script".to_string())
        );
    }

    #[test]
    fn kit_scripts_resource_empty_returns_zero_count() {
        let content = read_resource("kit://scripts", &[], &[], None).expect("should resolve");
        let doc: ScriptsResourceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 0);
        assert!(doc.scripts.is_empty());
    }

    #[test]
    fn kit_scriptlets_resource_returns_schema_versioned_envelope() {
        let scriptlets = wrap_scriptlets(vec![
            test_scriptlet("Open URL", "open", Some("Opens a URL")),
            test_scriptlet("Paste Text", "paste", None),
        ]);

        let content =
            read_resource("kit://scriptlets", &[], &scriptlets, None).expect("should resolve");
        assert_eq!(content.uri, "kit://scriptlets");

        let doc: ScriptletsResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SCRIPTLETS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, 2);
        assert_eq!(doc.scriptlets.len(), 2);
        assert_eq!(doc.scriptlets[0].name, "Open URL");
        assert_eq!(doc.scriptlets[0].tool, "open");
    }

    #[test]
    fn kit_scriptlets_resource_empty_returns_zero_count() {
        let content = read_resource("kit://scriptlets", &[], &[], None).expect("should resolve");
        let doc: ScriptletsResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.count, 0);
        assert!(doc.scriptlets.is_empty());
    }

    #[test]
    fn sdk_reference_resource_returns_valid_document() {
        let content = read_resource("kit://sdk-reference", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://sdk-reference");

        let doc: SdkReferenceDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, SDK_REFERENCE_SCHEMA_VERSION);
        assert_eq!(doc.sdk_package, "@scriptkit/sdk");
        assert!(!doc.functions.is_empty());

        // Verify key functions are present
        let names: Vec<&str> = doc.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"arg"), "should include arg()");
        assert!(names.contains(&"div"), "should include div()");
        assert!(names.contains(&"exec"), "should include exec()");
        assert!(names.contains(&"copy"), "should include copy()");
    }

    #[test]
    fn sdk_reference_has_categories() {
        let doc = build_sdk_reference_document();
        let categories: Vec<&str> = doc
            .functions
            .iter()
            .map(|f| f.category.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        assert!(categories.contains(&"prompts"));
        assert!(categories.contains(&"system"));
        assert!(categories.contains(&"clipboard"));
        assert!(categories.contains(&"filesystem"));
    }

    #[test]
    fn kit_scripts_resource_json_uses_camel_case() {
        let scripts = wrap_scripts(vec![test_script("Test", None)]);
        let content = read_resource("kit://scripts", &scripts, &[], None).unwrap();
        assert!(content.text.contains("\"schemaVersion\""));
        assert!(!content.text.contains("\"schema_version\""));
    }

    #[test]
    fn resource_definitions_include_new_resources() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"kit://scripts"));
        assert!(uris.contains(&"kit://scriptlets"));
        assert!(uris.contains(&"kit://sdk-reference"));
    }

    #[test]
    fn sdk_reference_includes_metadata_format() {
        let doc = build_sdk_reference_document();
        assert!(doc.metadata_format.contains("export const metadata"));
        assert!(doc.script_directory.contains("plugins/main/scripts"));
        assert!(doc.scriptlet_pattern.contains("scriptlets"));
    }

    #[test]
    fn sdk_reference_discovers_host_diagnostics_without_inventing_sdk_globals() {
        let doc = build_sdk_reference_document();
        let doctor = doc
            .authoring_resources
            .iter()
            .find(|resource| resource.uri == COMMAND_DOCTOR_RESOURCE_URI)
            .expect("command doctor is discoverable in the host authoring reference");
        assert_eq!(doctor.name, "Command Doctor");
        assert!(doc
            .authoring_resources
            .iter()
            .any(|resource| resource.uri == FAILED_SCRIPTS_RESOURCE_URI));
        assert!(!doc
            .functions
            .iter()
            .any(|function| function.name == "commandDoctor"));

        let json = serde_json::to_string(&doc).expect("serialize machine-readable reference");
        assert!(json.contains("\"authoringResources\""));
        assert!(json.contains(COMMAND_DOCTOR_RESOURCE_URI));
    }

    #[test]
    fn sdk_reference_roundtrips_through_json() {
        let doc = build_sdk_reference_document();
        let json = serde_json::to_string(&doc).expect("serialize");
        let parsed: SdkReferenceDocument = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(doc, parsed);
    }

    // =======================================================
    // kit://failed-scripts resource tests
    // =======================================================

    #[test]
    fn command_doctor_reports_supported_unsupported_and_malformed_commands() {
        use crate::metadata_parser::TypedMetadata;
        use std::path::PathBuf;

        let make_script = |name: &str, extension: &str, capabilities: Value| {
            Arc::new(Script {
                name: name.to_string(),
                path: PathBuf::from(format!("/tmp/{name}.{extension}")),
                extension: extension.to_string(),
                plugin_id: "main".to_string(),
                plugin_title: Some("Main".to_string()),
                typed_metadata: Some(TypedMetadata {
                    extra: HashMap::from([("sdkCapabilities".to_string(), capabilities)]),
                    ..TypedMetadata::default()
                }),
                ..Script::default()
            })
        };
        let supported = make_script("supported", "ts", serde_json::json!(["home"]));
        let unsupported = make_script("unsupported", "ts", serde_json::json!(["widget"]));
        let malformed = make_script("malformed", "ts", serde_json::json!("home"));
        let no_transport = make_script("shell", "sh", serde_json::json!(["readFile"]));

        let report = build_command_doctor_report(
            &[no_transport, unsupported, supported, malformed],
            &[],
            None,
        );
        assert_eq!(report.total_commands, 4);
        assert_eq!(report.ready_count, 1);
        assert_eq!(report.unsupported_count, 1);
        assert_eq!(report.blocked_count, 2);
        assert!(!report.permission_inventory_known);
        assert_eq!(
            report
                .commands
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["malformed", "shell", "supported", "unsupported"]
        );
        let denied = report
            .commands
            .iter()
            .find(|entry| entry.name == "unsupported")
            .expect("unsupported command stays visible");
        assert_eq!(denied.state, CommandDoctorState::Unsupported);
        assert!(!denied.executable);
        assert!(!denied.alternatives.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn command_doctor_treats_unknown_permission_as_pending_not_denied() {
        let script = Arc::new(Script {
            name: "Move Window".to_string(),
            path: std::path::PathBuf::from("/tmp/move-window.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([(
                    "sdkCapabilities".to_string(),
                    serde_json::json!(["moveWindow"]),
                )]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });

        let pending = build_command_doctor_report(&[Arc::clone(&script)], &[], None);
        assert_eq!(pending.permission_pending_count, 1);
        assert_eq!(pending.blocked_count, 0);
        assert!(!pending.commands[0].executable);
        assert_eq!(
            pending.commands[0].state,
            CommandDoctorState::PermissionPending
        );
        let pending_action = pending.commands[0]
            .primary_action
            .as_ref()
            .expect("pending script retains its actual canonical launcher action");
        assert!(!pending_action.enabled);
        assert_eq!(pending_action.reason.as_deref(), Some("permission_pending"));

        let known = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "macos".to_string(),
            granted_permissions: vec!["accessibility".to_string()],
        };
        let ready = build_command_doctor_report(&[script], &[], Some(&known));
        assert!(ready.permission_inventory_known);
        assert_eq!(ready.ready_count, 1);
        assert_eq!(ready.commands[0].state, CommandDoctorState::Ready);
        assert!(
            ready.commands[0]
                .primary_action
                .as_ref()
                .expect("granted script exposes its actual canonical launcher action")
                .enabled
        );
    }

    #[test]
    fn command_doctor_experimental_features_remain_explicitly_executable() {
        let script = Arc::new(Script {
            name: "Experimental Feedback".to_string(),
            path: std::path::PathBuf::from("/tmp/experimental-feedback.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([(
                    "sdkCapabilities".to_string(),
                    serde_json::json!(["beep"]),
                )]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });
        let host = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "macos".to_string(),
            granted_permissions: Vec::new(),
        };

        let report = build_command_doctor_report(&[script], &[], Some(&host));
        assert_eq!(report.experimental_count, 1);
        assert_eq!(report.commands[0].state, CommandDoctorState::Experimental);
        assert_eq!(
            report.commands[0].capabilities[0].support,
            SdkSupport::Experimental
        );
        assert!(report.commands[0].executable);
    }

    #[test]
    fn command_doctor_preview_uses_real_descriptor_without_leaking_identity() {
        use sk_protocol::command_contract::{
            CommandAvailability, CommandDescriptor, CommandIdentity, CommandSource,
        };

        let identity = CommandIdentity::new(CommandSource::Script, "main:private-script")
            .expect("canonical identity");
        let mut descriptor = CommandDescriptor::new(identity, "Private Script", "Run Script")
            .expect("real canonical descriptor");
        let ready = command_doctor_preview_from_descriptor(&descriptor)
            .expect("descriptor has real primary action");
        assert_eq!(ready.title, "Run Script");
        assert!(ready.enabled);
        let digest = ready
            .identity_fingerprint
            .strip_prefix("sha256:")
            .expect("identity uses the shared cryptographic receipt-redaction contract");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert!(!ready.identity_fingerprint.contains("private-script"));

        descriptor.availability = CommandAvailability::TemporarilyUnavailable;
        descriptor.actions[0].availability = CommandAvailability::TemporarilyUnavailable;
        let blocked = command_doctor_preview_from_descriptor(&descriptor)
            .expect("blocked real action remains inspectable");
        assert!(!blocked.enabled);
        assert_eq!(blocked.reason.as_deref(), Some("temporarily_unavailable"));
        assert_eq!(blocked.identity_fingerprint, ready.identity_fingerprint);
    }

    #[test]
    fn command_doctor_excludes_source_code_credentials_and_custom_secret_values() {
        let secret = "sk_live_doctor_must_never_appear";
        let script = Arc::new(Script {
            name: "Safe author diagnostics".to_string(),
            path: std::path::PathBuf::from("/tmp/safe-author-command.ts"),
            extension: "ts".to_string(),
            plugin_id: "main".to_string(),
            body: Some(format!("const token = '{secret}';")),
            typed_metadata: Some(crate::metadata_parser::TypedMetadata {
                extra: HashMap::from([
                    ("sdkCapabilities".to_string(), serde_json::json!(["home"])),
                    ("privateToken".to_string(), serde_json::json!(secret)),
                ]),
                ..crate::metadata_parser::TypedMetadata::default()
            }),
            ..Script::default()
        });

        let report = build_command_doctor_report(&[script], &[], None);
        let json = serde_json::to_string(&report).expect("serialize safe receipt");
        assert!(json.contains("/tmp/safe-author-command.ts"));
        assert!(json.contains("Safe author diagnostics"));
        assert!(!json.contains(secret));
        assert!(!json.contains("privateToken"));
        assert!(!json.contains("const token"));
    }

    #[test]
    fn command_doctor_resource_uses_only_explicit_loaded_snapshots() {
        let resource = read_resource(COMMAND_DOCTOR_RESOURCE_URI, &[], &[], None)
            .expect("command doctor resolves without app/provider access");
        assert_eq!(resource.uri, COMMAND_DOCTOR_RESOURCE_URI);
        let report: CommandDoctorReport =
            serde_json::from_str(&resource.text).expect("parse command doctor receipt");
        assert_eq!(
            report.schema_version,
            COMMAND_DOCTOR_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(report.total_commands, 0);
        assert!(report.commands.is_empty());
        assert!(!report.permission_inventory_known);
    }

    #[test]
    fn failed_scripts_resource_is_listed() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(
            uris.contains(&FAILED_SCRIPTS_RESOURCE_URI),
            "{FAILED_SCRIPTS_RESOURCE_URI} should be in resource definitions"
        );
    }

    #[test]
    fn failed_scripts_resource_lists_validation_failures() {
        use crate::scripts::{
            BindingKind, FailedScript, MetadataField, RelatedScript, ScriptValidationIssue,
            ScriptValidationKind, ValidationReport, ValidationSeverity, VALIDATION_SCHEMA_VERSION,
        };
        use std::path::PathBuf;

        // Two scripts colliding on `cmd k` — mirrors what `validate_script_catalog`
        // would emit for real duplicate-shortcut metadata on disk.
        let issue_for = |path: &str, peer: &str| ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: PathBuf::from(path),
            script_name: path.into(),
            field: Some(MetadataField::Shortcut),
            message: "Shortcut `cmd k` is declared by 2 scripts".into(),
            kind: ScriptValidationKind::DuplicateBinding {
                binding: BindingKind::Shortcut,
                value: "cmd k".into(),
            },
            related: vec![RelatedScript {
                path: PathBuf::from(peer),
                name: peer.into(),
            }],
        };
        let failed = vec![
            FailedScript {
                path: PathBuf::from("/tmp/a.ts"),
                name: "a".into(),
                fatal: Arc::from(vec![issue_for("/tmp/a.ts", "/tmp/b.ts")]),
            },
            FailedScript {
                path: PathBuf::from("/tmp/b.ts"),
                name: "b".into(),
                fatal: Arc::from(vec![issue_for("/tmp/b.ts", "/tmp/a.ts")]),
            },
        ];
        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 2,
            valid_count: 0,
            fatal_count: 2,
            warning_count: 0,
            failed_scripts: Arc::from(failed),
            warnings: Arc::from(Vec::<ScriptValidationIssue>::new()),
            retained_issues: Arc::from(Vec::<ScriptValidationIssue>::new()),
        };

        let doc = build_failed_scripts_document(&report);
        assert_eq!(doc.schema_version, FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.validation_schema_version, VALIDATION_SCHEMA_VERSION);
        assert_eq!(doc.total_candidates, 2);
        assert_eq!(doc.valid_count, 0);
        assert_eq!(doc.fatal_count, 2);
        assert_eq!(doc.failed_scripts.len(), 2);

        // Each failure must name its peer so the author can repair both sides.
        for entry in &doc.failed_scripts {
            assert_eq!(entry.fatal.len(), 1);
            assert_eq!(entry.fatal[0].related.len(), 1);
            assert!(matches!(
                entry.fatal[0].kind,
                ScriptValidationKind::DuplicateBinding {
                    binding: BindingKind::Shortcut,
                    ..
                }
            ));
        }

        let json = serde_json::to_string(&doc).expect("serialize");
        assert!(json.contains("\"schemaVersion\""));
        assert!(!json.contains("\"schema_version\""));
        assert!(json.contains("\"duplicateBinding\""));
        let parsed: FailedScriptsResourceDocument =
            serde_json::from_str(&json).expect("round-trip");
        assert_eq!(parsed.failed_scripts.len(), 2);
    }

    #[test]
    fn failed_scripts_resource_empty_report_serializes_cleanly() {
        use crate::scripts::{ValidationReport, VALIDATION_SCHEMA_VERSION};

        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 0,
            valid_count: 0,
            fatal_count: 0,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(Vec::new()),
        };
        let doc = build_failed_scripts_document(&report);
        assert_eq!(doc.fatal_count, 0);
        assert!(doc.failed_scripts.is_empty());
        assert!(doc.warnings.is_empty());

        let json = serde_json::to_string(&doc).expect("serialize");
        let parsed: FailedScriptsResourceDocument =
            serde_json::from_str(&json).expect("round-trip");
        assert_eq!(
            parsed.schema_version,
            FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(parsed.retained_issue_count, 0);
        assert!(parsed.retained_issues.is_empty());
    }

    #[test]
    fn failed_scripts_resource_keeps_retained_fatal_scriptlet_issues_distinct() {
        use crate::scripts::{
            MetadataField, ScriptValidationIssue, ScriptValidationKind, ValidationReport,
            ValidationSeverity, VALIDATION_SCHEMA_VERSION,
        };

        let issue = ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: std::path::PathBuf::from("/tmp/retained-scriptlet.md"),
            script_name: "Retained Shell Command".to_string(),
            field: Some(MetadataField::Capability),
            message: "Shell scriptlets do not receive SDK globals.".to_string(),
            kind: ScriptValidationKind::CapabilityUnavailable {
                capability: "readFile".to_string(),
                code: SdkCapabilityDiagnosticCode::MissingSdkTransport,
                alternatives: vec!["Move the command into a TypeScript script.".to_string()],
            },
            related: Vec::new(),
        };
        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 1,
            valid_count: 0,
            fatal_count: 1,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(vec![issue]),
        };

        let document = build_failed_scripts_document(&report);
        assert!(document.failed_scripts.is_empty());
        assert!(document.warnings.is_empty());
        assert_eq!(document.retained_issue_count, 1);
        assert_eq!(
            document.retained_issues[0].severity,
            ValidationSeverity::Fatal
        );
        assert_eq!(
            document.retained_issues[0].path,
            std::path::PathBuf::from("/tmp/retained-scriptlet.md")
        );
        assert!(!document.retained_issues[0].message.is_empty());

        let json = serde_json::to_value(&document).expect("serialize resource");
        assert_eq!(json["retainedIssueCount"], 1);
        assert_eq!(json["retainedIssues"][0]["severity"], "fatal");
        assert_eq!(json["failedScripts"], serde_json::json!([]));
    }

    #[test]
    fn failed_scripts_resource_accepts_legacy_documents_without_retained_fields() {
        let report = ValidationReport {
            schema_version: crate::scripts::VALIDATION_SCHEMA_VERSION,
            total_candidates: 0,
            valid_count: 0,
            fatal_count: 0,
            warning_count: 0,
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(Vec::new()),
        };
        let mut legacy = serde_json::to_value(build_failed_scripts_document(&report))
            .expect("serialize current resource");
        let object = legacy.as_object_mut().expect("resource object");
        object.remove("retainedIssueCount");
        object.remove("retainedIssues");

        let restored: FailedScriptsResourceDocument =
            serde_json::from_value(legacy).expect("legacy authoring resources remain readable");
        assert_eq!(restored.retained_issue_count, 0);
        assert!(restored.retained_issues.is_empty());
    }

    #[test]
    fn failed_scripts_resource_read_returns_parseable_envelope() {
        // End-to-end: resolves the URI through `read_resource` which calls
        // `read_scripts_report()` internally. Machine state may be non-empty,
        // so assert envelope shape, not failure count.
        let content = read_resource(FAILED_SCRIPTS_RESOURCE_URI, &[], &[], None)
            .expect("resource should resolve");
        assert_eq!(content.uri, FAILED_SCRIPTS_RESOURCE_URI);
        assert_eq!(content.mime_type, "application/json");

        let doc: FailedScriptsResourceDocument =
            serde_json::from_str(&content.text).expect("valid envelope JSON");
        assert_eq!(doc.schema_version, FAILED_SCRIPTS_RESOURCE_SCHEMA_VERSION);
        // If any script failed, its fatal-issue total must be at least as large
        // as the distinct failed-script count (each failed script has ≥1 issue).
        assert!(doc.fatal_count >= doc.failed_scripts.len());
    }

    #[test]
    fn parse_context_request_accepts_panel_screenshot_flag() {
        let request = parse_context_resource_request(
            "kit://context?screenshot=1&panelScreenshot=1&diagnostics=1",
        )
        .expect("request");
        assert!(request.options.include_screenshot);
        assert!(request.options.include_panel_screenshot);
        assert!(request.diagnostics);
    }

    #[test]
    fn diagnostics_surface_reports_panel_screenshot_state() {
        let request =
            parse_context_resource_request("kit://context?panelScreenshot=1&diagnostics=1")
                .expect("request");

        let snapshot = crate::context_snapshot::AiContextSnapshot {
            schema_version: crate::context_snapshot::AI_CONTEXT_SNAPSHOT_SCHEMA_VERSION,
            script_kit_panel_image: Some(crate::context_snapshot::Base64PngContext {
                mime_type: "image/png".to_string(),
                width: 700,
                height: 520,
                base64_data: "cGFuZWw=".to_string(),
                title: Some("Script Kit - Clipboard History".to_string()),
            }),
            ..Default::default()
        };

        let doc = build_context_diagnostics_document(
            "kit://context?panelScreenshot=1&diagnostics=1",
            &request,
            &snapshot,
            1,
        );
        assert!(doc
            .meta
            .field_statuses
            .iter()
            .any(|field| field.field == "panelScreenshot"
                && field.enabled
                && field.present
                && matches!(field.state, ContextFieldCaptureState::Captured)));
    }

    #[test]
    fn schema_document_includes_panel_screenshot_parameter() {
        let schema = build_context_schema_document();
        assert!(
            schema
                .parameters
                .iter()
                .any(|p| p.name == "panelScreenshot"),
            "schema must list panelScreenshot parameter"
        );
    }

    // =======================================================
    // Clipboard history resource tests
    // =======================================================

    #[test]
    fn clipboard_history_resource_is_listed() {
        let resources = get_resource_definitions();
        assert!(
            resources.iter().any(|r| r.uri == "kit://clipboard-history"),
            "kit://clipboard-history should be in resource definitions"
        );
    }

    #[test]
    fn clipboard_history_resource_resolves_with_valid_schema() {
        let content =
            read_resource("kit://clipboard-history", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://clipboard-history");
        assert_eq!(content.mime_type, "application/json");

        let doc: ClipboardHistoryDocument =
            serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(
            doc.schema_version,
            CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(doc.count, doc.entries.len());
    }

    #[test]
    fn clipboard_history_parse_accepts_limit_param() {
        let req = parse_clipboard_history_request("kit://clipboard-history?limit=5").unwrap();
        match req {
            ClipboardHistoryRequest::List { limit, diagnostics } => {
                assert_eq!(limit, 5);
                assert!(!diagnostics);
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_parse_clamps_limit_to_max() {
        let req = parse_clipboard_history_request("kit://clipboard-history?limit=999").unwrap();
        match req {
            ClipboardHistoryRequest::List { limit, .. } => {
                assert_eq!(limit, CLIPBOARD_HISTORY_MAX_LIMIT);
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_parse_rejects_unknown_param() {
        let err = parse_clipboard_history_request("kit://clipboard-history?foo=1").unwrap_err();
        assert!(err.contains("Invalid kit://clipboard-history parameter"));
    }

    #[test]
    fn clipboard_history_parse_accepts_id_param() {
        let req = parse_clipboard_history_request("kit://clipboard-history?id=abc123").unwrap();
        match req {
            ClipboardHistoryRequest::SingleEntry { id } => {
                assert_eq!(id, "abc123");
            }
            other => panic!("Expected SingleEntry, got {other:?}"),
        }
    }

    #[test]
    fn clipboard_history_diagnostics_returns_wrapper() {
        let content = read_resource("kit://clipboard-history?diagnostics=1", &[], &[], None)
            .expect("should resolve");

        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(value["kind"], "clipboard_history_diagnostics");
        assert_eq!(
            value["document"]["schemaVersion"],
            CLIPBOARD_HISTORY_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(value["meta"]["source"], "cached_entries");
    }

    #[test]
    fn clipboard_history_entry_serialization_roundtrip() {
        let entry = ClipboardHistoryEntry {
            id: "abc-123".to_string(),
            content_type: "text".to_string(),
            timestamp: 1711700000,
            text_preview: Some("Hello world".to_string()),
            ocr_text: None,
            image_width: None,
            image_height: None,
            pinned: false,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: ClipboardHistoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(entry, parsed);
    }

    // =======================================================
    // Focused item resource tests
    // =======================================================

    #[test]
    fn focused_item_resource_is_listed() {
        let resources = get_resource_definitions();
        assert!(
            resources.iter().any(|r| r.uri == "kit://focused-item"),
            "kit://focused-item should be in resource definitions"
        );
    }

    #[test]
    fn focused_item_resource_returns_empty_when_no_slot() {
        // Ensure slot is clear
        clear_focused_item();

        let content = read_resource("kit://focused-item", &[], &[], None).expect("should resolve");
        assert_eq!(content.uri, "kit://focused-item");

        let doc: FocusedItemDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(doc.schema_version, FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION);
        assert!(!doc.has_focused_item);
        assert!(doc.focused_item.is_none());
        assert!(
            !doc.warnings.is_empty(),
            "should have a warning when no item"
        );
    }

    #[test]
    fn focused_item_resource_returns_published_item() {
        publish_focused_item(FocusedItemInfo {
            source: "ClipboardHistory".to_string(),
            kind: "clipboard_entry".to_string(),
            semantic_id: "choice:0:hello".to_string(),
            label: "hello world".to_string(),
            metadata: Some(serde_json::json!({"contentType": "text"})),
        });

        let content = read_resource("kit://focused-item", &[], &[], None).expect("should resolve");

        let doc: FocusedItemDocument = serde_json::from_str(&content.text).expect("valid JSON");
        assert!(doc.has_focused_item);
        let item = doc.focused_item.expect("item present");
        assert_eq!(item.source, "ClipboardHistory");
        assert_eq!(item.semantic_id, "choice:0:hello");
        assert!(doc.warnings.is_empty());

        // Clean up
        clear_focused_item();
    }

    #[test]
    fn focused_item_parse_rejects_unknown_param() {
        let err = parse_focused_item_request("kit://focused-item?foo=1").unwrap_err();
        assert!(err.contains("Invalid kit://focused-item parameter"));
    }

    #[test]
    fn focused_item_diagnostics_returns_wrapper() {
        clear_focused_item();

        let content = read_resource("kit://focused-item?diagnostics=1", &[], &[], None)
            .expect("should resolve");

        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        assert_eq!(value["kind"], "focused_item_diagnostics");
        assert_eq!(
            value["document"]["schemaVersion"],
            FOCUSED_ITEM_RESOURCE_SCHEMA_VERSION
        );
        assert_eq!(value["meta"]["source"], "focused_item_slot");
        assert_eq!(value["meta"]["hasFocusedItem"], false);
        assert!(value["meta"]["warningCount"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn focused_item_info_serialization_roundtrip() {
        let item = FocusedItemInfo {
            source: "FileSearch".to_string(),
            kind: "file".to_string(),
            semantic_id: "choice:2:readme".to_string(),
            label: "README.md".to_string(),
            metadata: Some(serde_json::json!({"path": "/tmp/README.md"})),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let parsed: FocusedItemInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(item, parsed);
    }

    #[test]
    fn test_notes_list_resource_full_param_returns_full_content() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_full");
        let body: String = format!(
            "---\ntags: [{token}]\n---\n# Full Body\n{}",
            "x".repeat(600)
        );
        let note = crate::notes::Note::with_content(body.clone());
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes full-content test note");

        let content = read_notes_list_resource(&format!("kit://notes?tag={token}&full=true"))
            .expect("full-content notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        let notes = value["notes"].as_array().expect("notes array");
        let entry = notes
            .iter()
            .find(|candidate| candidate["id"] == note_id.as_str())
            .expect("created note should be returned by full-content resource");
        let entry = entry.clone();

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes full-content test");

        assert_eq!(
            entry["content"].as_str().expect("content string"),
            body,
            "full=true should return the complete note body, not a preview"
        );
        assert!(entry.get("preview").is_none(), "full entries drop preview");
        assert_eq!(entry["contentTruncated"], serde_json::Value::Bool(false));
    }

    #[test]
    fn test_notes_list_resource_can_filter_and_report_metadata() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_tag");
        let note = crate::notes::Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Resource Metadata\nBody [[{token} Target]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes resource test note");

        let content = read_notes_list_resource(&format!("kit://notes?tag={token}&limit=10"))
            .expect("tag-filtered notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
        let notes = value["notes"].as_array().expect("notes array");
        let summary = notes
            .iter()
            .find(|candidate| candidate["id"] == note_id.as_str())
            .expect("created note should be returned by tag-filtered resource");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes resource metadata test");

        assert_eq!(value["query"], format!("tag:{token}"));
        assert!(
            summary["metadata"]["tags"]
                .as_array()
                .expect("tags array")
                .iter()
                .any(|tag| tag == token.as_str()),
            "summary metadata should include indexed tags"
        );
        assert!(
            summary["metadata"]["aliases"]
                .as_array()
                .expect("aliases array")
                .iter()
                .any(|alias| alias == format!("{token} Alias").as_str()),
            "summary metadata should include indexed aliases"
        );
        assert_eq!(summary["metadata"]["outboundLinkCount"], 1);
    }

    #[test]
    fn test_single_note_resource_reports_metadata() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("single_resource");
        let note = crate::notes::Note::with_content(format!(
            "---\ntags: [{token}]\naliases: [{token} Alias]\n---\n# Single Resource\nBody [[{token} Target]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save single notes resource test note");

        let content = read_single_note_resource(&format!("kit://notes/{note_id}"))
            .expect("single notes resource should resolve");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for single notes resource metadata test");

        assert_eq!(value["note"]["id"], note_id.as_str());
        assert!(
            value["metadata"]["tags"]
                .as_array()
                .expect("tags array")
                .iter()
                .any(|tag| tag == token.as_str()),
            "single note metadata should include indexed tags"
        );
        assert!(
            value["metadata"]["aliases"]
                .as_array()
                .expect("aliases array")
                .iter()
                .any(|alias| alias == format!("{token} Alias").as_str()),
            "single note metadata should include indexed aliases"
        );
        assert_eq!(value["metadata"]["outboundLinkCount"], 1);
    }

    #[test]
    fn test_notes_resource_query_params_are_url_decoded() {
        assert_eq!(
            query_string_param("kit://notes?q=project%20plan", "q"),
            Some("project plan".to_string())
        );
        assert_eq!(
            query_string_param("kit://notes?alias=Project+Plan", "alias"),
            Some("Project Plan".to_string())
        );
        assert_eq!(
            notes_list_search_query("kit://notes?tag=projects%2Fscript-kit"),
            Some("tag:projects/script-kit".to_string())
        );
    }

    #[test]
    fn test_notes_list_resource_filters_alias_link_q_and_plus_decoding() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::notes::init_notes_db().expect("notes db should initialize before resource test");
        let token = unique_notes_resource_token("resource_query");
        let alias = format!("{token} Project Plan");
        let target_title = format!("{token} Target Note");
        let body_token = format!("{token}_body");
        let note = crate::notes::Note::with_content(format!(
            "---\naliases: [{alias}]\n---\n# Resource Query\n{body_token} links to [[{target_title}]]"
        ));
        let note_id = note.id;
        crate::notes::save_note(&note).expect("failed to save notes resource query test note");

        let alias_uri = format!("kit://notes?alias={}&limit=10", alias.replace(' ', "+"));
        let link_uri = format!(
            "kit://notes?link={}&limit=10",
            target_title.replace(' ', "+")
        );
        let text_uri = format!("kit://notes?q={body_token}&limit=10");
        let alias_content =
            read_notes_list_resource(&alias_uri).expect("alias-filtered notes should resolve");
        let link_content =
            read_notes_list_resource(&link_uri).expect("link-filtered notes should resolve");
        let text_content =
            read_notes_list_resource(&text_uri).expect("text-filtered notes should resolve");

        crate::notes::delete_note_permanently(note_id)
            .expect("cleanup failed for notes resource query test");

        for (label, content) in [
            ("alias", alias_content),
            ("link", link_content),
            ("q", text_content),
        ] {
            let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");
            let notes = value["notes"].as_array().expect("notes array");
            assert!(
                notes
                    .iter()
                    .any(|candidate| candidate["id"] == note_id.as_str()),
                "{label} resource filter should return the created note"
            );
        }
    }

    // ── Provider-backed JSON resource tests ───────────────────────

    #[test]
    fn dictation_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_DICTATION_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://dictation", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "dictation");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn calendar_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_CALENDAR_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://calendar", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "calendar");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn notifications_resource_empty_fallback_has_explicit_envelope() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        std::env::remove_var("SCRIPT_KIT_NOTIFICATIONS_JSON");

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://notifications", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "notifications");
        assert_eq!(value["ok"], true);
        assert_eq!(value["available"], false);
        assert_eq!(value["source"], "empty-fallback");
        assert!(value["items"].is_array());
        assert!(value["note"].is_string());
        assert!(value["nextStep"].is_string());
    }

    #[test]
    fn dictation_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_dictation_json(
            r#"{"schemaVersion":1,"type":"dictation","ok":true,"available":true,"source":"slot","items":[{"text":"hello"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://dictation", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    #[test]
    fn calendar_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_calendar_json(
            r#"{"schemaVersion":1,"type":"calendar","ok":true,"available":true,"source":"slot","items":[{"title":"Demo"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://calendar", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    #[test]
    fn notifications_resource_prefers_slot_data() {
        let _guard = provider_json_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_provider_json_slots();
        publish_notifications_json(
            r#"{"schemaVersion":1,"type":"notifications","ok":true,"available":true,"source":"slot","items":[{"title":"Build complete"}]}"#,
        );

        let scripts = Vec::new();
        let scriptlets = Vec::new();
        let content =
            read_resource("kit://notifications", &scripts, &scriptlets, None).expect("should read");
        let value: serde_json::Value = serde_json::from_str(&content.text).expect("valid JSON");

        assert_eq!(value["available"], true);
        assert_eq!(value["source"], "slot");
        assert_eq!(value["items"].as_array().expect("items array").len(), 1);

        clear_provider_json_slots();
    }

    fn sdk_ref(name: &str, signature: &str, description: &str, category: &str) -> SdkFunctionRef {
        SdkFunctionRef::supported(name, signature, description, category)
    }

    #[test]
    fn filter_sdk_reference_entries_empty_filter_returns_all_indices() {
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompt user", "input"),
            sdk_ref("div", "div(html)", "Render HTML", "output"),
        ];
        let indices = filter_sdk_reference_entries(&entries, "");
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn filter_sdk_reference_entries_whitespace_filter_returns_all_indices() {
        let entries = vec![sdk_ref("arg", "arg(p)", "Prompt", "input")];
        let indices = filter_sdk_reference_entries(&entries, "   ");
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn filter_sdk_reference_entries_matches_case_insensitively_across_fields() {
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompts the user", "input"),
            sdk_ref("div", "div(html)", "Renders HTML content", "output"),
            sdk_ref("path", "path(opts)", "File picker", "input"),
        ];
        assert_eq!(filter_sdk_reference_entries(&entries, "INPUT"), vec![0, 2]);
        assert_eq!(filter_sdk_reference_entries(&entries, "html"), vec![1]);
        assert_eq!(filter_sdk_reference_entries(&entries, "picker"), vec![2]);
        assert_eq!(
            filter_sdk_reference_entries(&entries, "no-such-thing"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_contains_all_fields() {
        let entry = sdk_ref(
            "arg",
            "arg(prompt: string)",
            "Prompts the user for input",
            "input",
        );
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(md.contains("# arg"), "missing heading: {md}");
        assert!(
            md.contains("`arg(prompt: string)`"),
            "missing signature: {md}"
        );
        assert!(md.contains("_input_"), "missing category: {md}");
        assert!(
            md.contains("Prompts the user for input"),
            "missing description: {md}"
        );
        assert!(md.contains(COMMAND_DOCTOR_RESOURCE_URI));
        assert!(md.contains(FAILED_SCRIPTS_RESOURCE_URI));
        assert!(md.contains("host MCP resources, not callable SDK globals"));
    }

    #[test]
    fn sdk_support_serde_roundtrips_lowercase() {
        // Pins the wire shape: lowercase strings, not PascalCase.
        let supported = serde_json::to_string(&SdkSupport::Supported).expect("serialize");
        let unsupported = serde_json::to_string(&SdkSupport::Unsupported).expect("serialize");
        let experimental = serde_json::to_string(&SdkSupport::Experimental).expect("serialize");
        assert_eq!(supported, "\"supported\"");
        assert_eq!(unsupported, "\"unsupported\"");
        assert_eq!(experimental, "\"experimental\"");

        for raw in [&supported, &unsupported, &experimental] {
            let parsed: SdkSupport = serde_json::from_str(raw).expect("deserialize");
            let again = serde_json::to_string(&parsed).expect("re-serialize");
            assert_eq!(&again, raw, "round-trip mismatch for {raw}");
        }
    }

    #[test]
    fn sdk_function_ref_deserializes_old_shape_as_supported() {
        // Pins backward compatibility: older JSON without `support` still
        // parses, defaulting to Supported with no note.
        let json = r#"{
            "name": "arg",
            "signature": "arg(prompt)",
            "description": "Prompt",
            "category": "prompts"
        }"#;
        let parsed: SdkFunctionRef = serde_json::from_str(json).expect("legacy shape must parse");
        assert_eq!(parsed.support, SdkSupport::Supported);
        assert!(parsed.unsupported_note.is_none());
    }

    #[test]
    fn sdk_function_ref_always_serializes_support_field() {
        // Agents should not have to infer support from field absence.
        let entry = SdkFunctionRef::supported("arg", "arg(p)", "Prompt", "prompts");
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(
            json.contains("\"support\":\"supported\""),
            "support field must be serialized for Supported entries: {json}"
        );
        assert!(
            !json.contains("unsupportedNote"),
            "Option::None should not emit unsupportedNote: {json}"
        );
    }

    #[test]
    fn sdk_reference_marks_notify_as_supported_system_notification_api() {
        // Pins the user's correction: notify() is intentional OS-level
        // feedback (macOS Notification Center via notify-rust), distinct
        // from hud(message) which is in-launcher. Both must coexist, and
        // kit://sdk-reference must not treat notify() as a dead end.
        let doc = build_sdk_reference_document();
        let notify = doc
            .functions
            .iter()
            .find(|entry| entry.name == "notify")
            .expect("notify must appear in the SDK reference");
        assert_eq!(notify.support, SdkSupport::Supported);
        assert!(
            notify.unsupported_note.is_none(),
            "notify is Supported; it must not carry an unsupported_note"
        );
        let description = notify.description.as_str();
        assert!(
            description.to_lowercase().contains("system notification")
                || description.to_lowercase().contains("notification center"),
            "notify description must advertise it as an OS-level notification API: {description}"
        );
        assert!(
            description.contains("hud"),
            "notify description must contrast itself with hud(message) so readers can pick the right API: {description}"
        );
    }

    #[test]
    fn sdk_reference_marks_every_documented_unsupported_api() {
        let doc = build_sdk_reference_document();
        for unsupported_name in SDK_NOT_YET_IMPLEMENTED_IN_GPUI {
            let entry = doc
                .functions
                .iter()
                .find(|entry| entry.name == *unsupported_name)
                .unwrap_or_else(|| panic!("unsupported SDK API `{unsupported_name}` is missing from the author-facing reference"));
            assert_eq!(
                entry.support,
                SdkSupport::Unsupported,
                "`{unsupported_name}` appears in the unsupported inventory but is marked available in the SDK reference"
            );
            assert!(
                entry.unsupported_note.is_some(),
                "`{unsupported_name}` must carry an actionable support explanation"
            );
        }
    }

    #[test]
    fn sdk_reference_marks_implemented_prompt_variants_as_supported() {
        let doc = build_sdk_reference_document();

        for name in [
            "mini", "micro", "hotkey", "fields", "form", "select", "path",
        ] {
            let entry = doc
                .functions
                .iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("implemented native prompt `{name}` is missing"));

            assert_eq!(entry.support, SdkSupport::Supported);
            assert!(entry.unsupported_note.is_none());
            assert!(
                !SDK_NOT_YET_IMPLEMENTED_IN_GPUI.contains(&name),
                "implemented prompt `{name}` must not appear in the unsupported inventory"
            );
        }
    }

    #[test]
    fn sdk_capability_catalog_matches_every_reference_row_exactly_once() {
        let doc = build_sdk_reference_document();
        assert_eq!(
            doc.capability_catalog.schema_version,
            SDK_CAPABILITY_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(
            doc.capability_catalog.host_version,
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(
            doc.capability_catalog.capabilities.len(),
            doc.functions.len()
        );

        let mut seen = std::collections::HashSet::new();
        for (entry, capability) in doc
            .functions
            .iter()
            .zip(doc.capability_catalog.capabilities.iter())
        {
            assert!(seen.insert(capability.name.as_str()));
            assert_eq!(capability.name, entry.name);
            assert_eq!(capability.support, entry.support);
            assert!(!capability.minimum_host_version.is_empty());
            if capability.support == SdkSupport::Unsupported {
                assert!(!capability.alternatives.is_empty());
                assert!(capability.migration_note.is_some());
            }
        }
    }

    #[test]
    fn sdk_capability_catalog_reuses_index_until_explicit_invalidation() {
        let first = sdk_capability_catalog_index();
        let second = sdk_capability_catalog_index();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            first.catalog.schema_version,
            SDK_CAPABILITY_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(first.catalog.host_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(first.positions.len(), first.catalog.capabilities.len());

        let next_generation = invalidate_sdk_capability_catalog();
        let refreshed = sdk_capability_catalog_index();
        assert!(!Arc::ptr_eq(&first, &refreshed));
        assert_eq!(refreshed.generation, next_generation);
        assert_eq!(sdk_capability_catalog_generation(), next_generation);
        assert_eq!(refreshed.catalog, first.catalog);
    }

    #[test]
    fn sdk_capability_catalog_declares_native_permission_and_platform_boundaries() {
        let catalog = sdk_capability_catalog();
        let move_window = catalog
            .capabilities
            .iter()
            .find(|capability| capability.name == "moveWindow")
            .expect("moveWindow capability");
        assert_eq!(move_window.required_permissions, vec!["accessibility"]);
        assert_eq!(move_window.supported_platforms, vec!["macos"]);

        let screenshot = catalog
            .capabilities
            .iter()
            .find(|capability| capability.name == "computer.captureNativeWindow")
            .expect("capture capability");
        assert_eq!(
            screenshot.required_permissions,
            vec!["accessibility", "screen-recording"]
        );

        for name in [
            "closeWindow",
            "minimizeWindow",
            "maximizeWindow",
            "moveToNextDisplay",
            "moveToPreviousDisplay",
            "getMenuBar",
            "executeMenuAction",
        ] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing native capability `{name}`"));
            assert_eq!(capability.required_permissions, vec!["accessibility"]);
            assert_eq!(capability.supported_platforms, vec!["macos"]);
        }
    }

    #[test]
    fn sdk_capability_catalog_covers_real_namespaces_without_claiming_input_injection() {
        let catalog = sdk_capability_catalog();
        for name in [
            "exec",
            "readFile",
            "writeFile",
            "confirm",
            "chat",
            "clipboard.readImage",
            "clipboardHistoryPin",
            "chat.addMessage",
            "chat.startStream",
            "chat.getMessages",
            "chat.getResult",
            "memoryMap.get",
            "mcp.call",
            "aiGetActiveChat",
            "aiSendMessage",
        ] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing executable capability `{name}`"));
            assert_eq!(capability.support, SdkSupport::Supported);
        }

        for name in ["keyboard", "mouse"] {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap_or_else(|| panic!("missing denied input namespace `{name}`"));
            assert_eq!(capability.support, SdkSupport::Unsupported);
            assert!(!capability.alternatives.is_empty());
        }

        let paste = build_sdk_reference_document()
            .functions
            .into_iter()
            .find(|entry| entry.name == "paste")
            .expect("paste reference");
        assert_eq!(paste.signature, "await paste(): Promise<string>");
        assert!(paste.description.contains("does not inject global input"));
    }

    #[test]
    fn unsupported_sdk_capability_inventory_matches_public_author_contract() {
        let catalog = sdk_capability_catalog();
        for name in unsupported_sdk_capability_names() {
            let capability = catalog
                .capabilities
                .iter()
                .find(|capability| capability.name == *name)
                .unwrap_or_else(|| panic!("missing denied capability `{name}`"));
            assert_eq!(capability.support, SdkSupport::Unsupported);
        }
    }

    #[test]
    fn sdk_capability_transport_names_match_the_typescript_wire_contract() {
        for (topology, expected) in [
            (SdkExecutionTopology::TypeScriptScript, "typescript-script"),
            (
                SdkExecutionTopology::TypeScriptScriptlet,
                "typescript-scriptlet",
            ),
            (
                SdkExecutionTopology::TypeScriptScriptletInteractive,
                "typescript-scriptlet-interactive",
            ),
            (SdkExecutionTopology::ShellScriptlet, "shell-scriptlet"),
            (SdkExecutionTopology::PythonScriptlet, "python-scriptlet"),
        ] {
            assert_eq!(
                serde_json::to_value(topology).expect("serialize topology"),
                serde_json::json!(expected)
            );
        }
    }

    #[test]
    fn sdk_reference_deserializes_legacy_documents_without_a_capability_catalog() {
        let mut legacy = serde_json::to_value(build_sdk_reference_document())
            .expect("serialize current SDK reference");
        legacy
            .as_object_mut()
            .expect("reference object")
            .remove("capabilityCatalog");
        let restored: SdkReferenceDocument =
            serde_json::from_value(legacy).expect("legacy SDK reference remains readable");
        assert!(restored.capability_catalog.capabilities.is_empty());
    }

    #[test]
    fn sdk_capability_diagnostics_reject_unsupported_apis_before_dispatch() {
        for name in [
            "widget",
            "setPanel",
            "keyboard.type",
            "mouse.leftClick",
            "find",
        ] {
            let diagnostic = diagnose_sdk_capability(name, SdkExecutionTopology::TypeScriptScript)
                .unwrap_or_else(|| panic!("unsupported capability `{name}` needs a diagnostic"));
            assert_eq!(
                diagnostic.code,
                SdkCapabilityDiagnosticCode::UnsupportedCapability
            );
            assert!(!diagnostic.alternatives.is_empty());
        }

        assert!(diagnose_sdk_capability("mini", SdkExecutionTopology::TypeScriptScript).is_none());
        assert!(
            diagnose_sdk_capability("fields", SdkExecutionTopology::TypeScriptScript).is_none()
        );
    }

    #[test]
    fn sdk_capability_diagnostics_reject_impossible_scriptlet_prompt_topologies() {
        let interactive = diagnose_sdk_capability("arg", SdkExecutionTopology::TypeScriptScriptlet)
            .expect("interactive scriptlet prompt must fail closed");
        assert_eq!(
            interactive.code,
            SdkCapabilityDiagnosticCode::InteractivePromptUnavailable
        );
        assert!(interactive.message.contains("stdin"));

        assert!(diagnose_sdk_capability(
            "arg",
            SdkExecutionTopology::TypeScriptScriptletInteractive,
        )
        .is_none());
        assert!(diagnose_sdk_capability(
            "chat.startStream",
            SdkExecutionTopology::TypeScriptScriptletInteractive,
        )
        .is_none());

        for topology in [
            SdkExecutionTopology::ShellScriptlet,
            SdkExecutionTopology::PythonScriptlet,
        ] {
            let unavailable = diagnose_sdk_capability("home", topology)
                .expect("non-TypeScript scriptlets have no SDK transport");
            assert_eq!(
                unavailable.code,
                SdkCapabilityDiagnosticCode::MissingSdkTransport
            );
        }

        assert!(
            diagnose_sdk_capability("home", SdkExecutionTopology::TypeScriptScriptlet).is_none()
        );

        let active_chat = diagnose_sdk_capability(
            "chat.startStream",
            SdkExecutionTopology::TypeScriptScriptlet,
        )
        .expect("inline-chat mutations require an interactive active chat session");
        assert_eq!(
            active_chat.code,
            SdkCapabilityDiagnosticCode::InteractivePromptUnavailable
        );
        assert!(diagnose_sdk_capability(
            "chat.getMessages",
            SdkExecutionTopology::TypeScriptScriptlet
        )
        .is_none());
        assert!(diagnose_sdk_capability(
            "chat.getResult",
            SdkExecutionTopology::TypeScriptScriptlet
        )
        .is_none());
    }

    #[test]
    fn sdk_capability_diagnostics_reject_unknown_apis() {
        let diagnostic =
            diagnose_sdk_capability("doesNotExist", SdkExecutionTopology::TypeScriptScript)
                .expect("unknown capability must not be assumed supported");
        assert_eq!(
            diagnostic.code,
            SdkCapabilityDiagnosticCode::UnknownCapability
        );
    }

    #[test]
    fn sdk_capability_context_rejects_unknown_or_outdated_semver_fail_closed() {
        let mut host = SdkHostAvailability {
            host_version: "not-a-semver".into(),
            platform: "macos".into(),
            granted_permissions: Vec::new(),
        };
        let malformed = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("invalid version must not pass capability preflight");
        assert_eq!(
            malformed.code,
            SdkCapabilityDiagnosticCode::InvalidHostVersion
        );

        host.host_version = "0.0.0".into();
        let outdated = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("older host must not claim current capability support");
        assert_eq!(
            outdated.code,
            SdkCapabilityDiagnosticCode::HostVersionTooOld
        );
        assert!(outdated.message.contains("0.0.0"));
    }

    #[test]
    fn sdk_capability_context_enforces_platform_then_explicit_permission_facts() {
        let mut host = SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").into(),
            platform: "linux".into(),
            granted_permissions: vec!["accessibility".into()],
        };
        let unsupported = diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("native macOS capability must reject other platforms");
        assert_eq!(
            unsupported.code,
            SdkCapabilityDiagnosticCode::UnsupportedPlatform
        );

        host.platform = "macos".into();
        host.granted_permissions.clear();
        let missing = diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("accessibility grant is an explicit capability prerequisite");
        assert_eq!(missing.code, SdkCapabilityDiagnosticCode::MissingPermission);
        assert!(missing.message.contains("accessibility"));

        host.granted_permissions.push("accessibility".into());
        assert!(diagnose_sdk_capability_with_context(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .is_none());

        let capture = diagnose_sdk_capability_with_context(
            "computer.captureNativeWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("capture requires both accessibility and screen-recording");
        assert_eq!(capture.code, SdkCapabilityDiagnosticCode::MissingPermission);
        assert!(capture.message.contains("screen-recording"));

        host.granted_permissions.push("screen-recording".into());
        assert!(diagnose_sdk_capability_with_context(
            "computer.captureNativeWindow",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .is_none());
    }

    #[test]
    fn sdk_capability_current_host_never_assumes_unknown_permission_granted() {
        assert!(diagnose_sdk_capability_for_current_host(
            "home",
            SdkExecutionTopology::TypeScriptScript,
        )
        .is_none());

        let host = SdkHostDiagnosticContext {
            host_version: env!("CARGO_PKG_VERSION"),
            platform: "macos",
            granted_permissions: None,
        };
        let pending = diagnose_sdk_capability_inner(
            "moveWindow",
            SdkExecutionTopology::TypeScriptScript,
            Some(host),
        )
        .expect("unknown permission inventory must never be treated as a grant");
        assert_eq!(
            pending.code,
            SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable
        );
        assert!(pending
            .message
            .contains("no already-known permission inventory"));
        assert!(!pending.message.contains("has not granted"));
    }

    #[test]
    fn sdk_capability_context_preserves_topology_and_unsupported_precedence() {
        let host = SdkHostAvailability {
            host_version: "not-a-semver".into(),
            platform: "linux".into(),
            granted_permissions: Vec::new(),
        };
        let denied = diagnose_sdk_capability_with_context(
            "keyboard.type",
            SdkExecutionTopology::TypeScriptScript,
            &host,
        )
        .expect("unsupported global input must reject before inspecting host facts");
        assert_eq!(
            denied.code,
            SdkCapabilityDiagnosticCode::UnsupportedCapability
        );

        let missing_transport = diagnose_sdk_capability_with_context(
            "home",
            SdkExecutionTopology::ShellScriptlet,
            &host,
        )
        .expect("missing SDK transport must reject before inspecting host facts");
        assert_eq!(
            missing_transport.code,
            SdkCapabilityDiagnosticCode::MissingSdkTransport
        );
    }

    #[test]
    fn sdk_host_availability_wire_is_explicit_and_does_not_probe_permissions() {
        let host = SdkHostAvailability::current(vec!["accessibility".into()]);
        assert_eq!(host.host_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(host.platform, std::env::consts::OS);
        assert_eq!(host.granted_permissions, vec!["accessibility"]);

        let encoded = serde_json::to_value(&host).expect("serialize host availability");
        assert_eq!(encoded["hostVersion"], env!("CARGO_PKG_VERSION"));
        assert_eq!(encoded["platform"], std::env::consts::OS);
        assert_eq!(
            encoded["grantedPermissions"],
            serde_json::json!(["accessibility"])
        );
    }

    #[test]
    fn sdk_reference_marks_find_as_unsupported_prompt_gap() {
        let doc = build_sdk_reference_document();
        let find = doc
            .functions
            .iter()
            .find(|entry| entry.name == "find")
            .expect("find must appear in the SDK reference");
        assert_eq!(find.support, SdkSupport::Unsupported);
        let note = find
            .unsupported_note
            .as_deref()
            .expect("find must explain its unsupported GPUI boundary");
        assert!(
            note.contains("fileSearch") && note.contains("onlyin"),
            "find unsupported note must point users to the supported onlyin-capable fileSearch API: {note}"
        );
        assert!(
            find.description
                .to_lowercase()
                .contains("does not currently implement"),
            "find description must not imply a working GPUI prompt: {}",
            find.description
        );
    }

    #[test]
    fn filter_sdk_reference_entries_includes_unsupported_results() {
        // Pins: unsupported entries stay discoverable. Filtering does NOT
        // skip them — the label is the only thing that changes.
        let entries = vec![
            sdk_ref("arg", "arg(prompt)", "Prompt user", "prompts"),
            SdkFunctionRef::unsupported(
                "notify",
                "notify(message)",
                "Show notification",
                "feedback",
                "Use hud(...) in GPUI today.",
            ),
        ];
        assert_eq!(filter_sdk_reference_entries(&entries, "notify"), vec![1]);
        assert_eq!(
            filter_sdk_reference_entries(&entries, "hud"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_warns_for_unsupported() {
        let entry = SdkFunctionRef::unsupported(
            "notify",
            "notify(message)",
            "Show notification",
            "feedback",
            "Use hud(message) instead.",
        );
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(
            md.starts_with("> ⚠ Unsupported in GPUI"),
            "unsupported entry markdown must lead with a blockquote warning: {md}"
        );
        assert!(
            md.contains("Use hud(message) instead."),
            "unsupported entry markdown must surface the note: {md}"
        );
        // Body sections still present.
        assert!(md.contains("# notify"), "missing heading: {md}");
        assert!(md.contains("`notify(message)`"), "missing signature: {md}");
        assert!(md.contains("_feedback_"), "missing category: {md}");
        assert!(
            md.contains("Show notification"),
            "missing description: {md}"
        );
    }

    #[test]
    fn format_sdk_reference_entry_markdown_does_not_warn_for_supported() {
        let entry = sdk_ref("arg", "arg(p)", "Prompt", "prompts");
        let md = format_sdk_reference_entry_markdown(&entry);
        assert!(
            !md.contains("Unsupported in GPUI"),
            "supported entry markdown must not carry an unsupported warning: {md}"
        );
    }

    #[test]
    fn sdk_reference_supported_count_exceeds_unsupported_count() {
        let doc = build_sdk_reference_document();
        let supported = doc
            .functions
            .iter()
            .filter(|f| f.support == SdkSupport::Supported)
            .count();
        let unsupported = doc
            .functions
            .iter()
            .filter(|f| f.support == SdkSupport::Unsupported)
            .count();
        assert!(
            unsupported > 0,
            "at least one SDK entry (notify) must be labeled unsupported"
        );
        assert!(
            supported > unsupported,
            "SDK reference is meant to guide authors to working APIs: supported ({supported}) should exceed unsupported ({unsupported})"
        );
    }

    #[test]
    fn sdk_reference_schema_version_is_six() {
        // Pin the current schema version so any accidental bump is visible
        // in the diff and stays paired with an envelope-shape change.
        assert_eq!(SDK_REFERENCE_SCHEMA_VERSION, 6);
    }

    #[test]
    fn script_templates_do_not_reference_unsupported_sdk_apis() {
        // Starter templates cannot silently depend on a stub SDK API. If a
        // future template calls e.g. `notify(...)` or `keyboard.type(...)`,
        // this test must fail so the template author either chooses a
        // working API or we intentionally upgrade the SDK entry's support
        // status first.
        let templates = build_script_templates_document().templates;
        let needles = unsupported_sdk_reference_scan_needles();
        assert!(
            !needles.is_empty(),
            "needle list must be non-empty — if every SDK entry becomes Supported, the needle builder drifted and this test becomes a no-op"
        );
        for template in &templates {
            let rendered = render_script_template_file(template, "Demo");
            for needle in &needles {
                assert!(
                    !rendered.contains(needle.as_str()),
                    "Template `{}` references unsupported SDK API `{needle}`. Rendered body:\n{rendered}",
                    template.id
                );
            }
        }
    }

    #[test]
    fn harness_workflow_examples_do_not_reference_unsupported_sdk_apis() {
        // The kit://sdk-reference harness workflow ships concrete example
        // scripts (test-script + scriptlet) that agents and users copy
        // verbatim. After i008 started flagging `notify` as Unsupported in
        // kit://sdk-reference, any example that still calls `notify(...)`
        // contradicts the product. This test pins the invariant.
        let workflow = build_harness_workflow();
        let examples: [(&str, &str); 2] = [
            ("example_test_script", workflow.example_test_script.as_str()),
            ("example_scriptlet", workflow.example_scriptlet.as_str()),
        ];
        let needles = unsupported_sdk_reference_scan_needles();
        assert!(
            !needles.is_empty(),
            "needle list must be non-empty — if every SDK entry becomes Supported, the needle builder drifted and this test becomes a no-op"
        );
        for (label, body) in &examples {
            for needle in &needles {
                assert!(
                    !body.contains(needle.as_str()),
                    "Harness workflow `{label}` references unsupported SDK API `{needle}`.\nBody:\n{body}"
                );
            }
        }
    }

    #[test]
    fn harness_workflow_example_scriptlet_uses_hud_for_feedback() {
        // Pins the intent of the copy-today's-date scriptlet: because the
        // desired feedback is launcher-local (flash a confirmation while the
        // launcher is the active surface), the canonical example uses
        // `hud(...)` rather than `notify(...)`. `notify(...)` is a
        // Supported, real OS-notification API — equally legitimate when the
        // caller wants Notification Center delivery that lasts past a dismiss
        // — but mixing it into this example would misinform authors about
        // when to pick each one.
        let workflow = build_harness_workflow();
        assert!(
            workflow
                .example_scriptlet
                .contains("hud(\"Copied today's date\")"),
            "example_scriptlet must give launcher-local feedback via `hud(...)`; reach for `notify(...)` only when you want OS Notification Center delivery.\nBody:\n{}",
            workflow.example_scriptlet
        );
        assert!(
            !workflow.example_scriptlet.contains("notify("),
            "example_scriptlet must not call `notify(...)`; this copy-date scriptlet is a launcher-local feedback example — `hud(message)` is the right choice here.\nBody:\n{}",
            workflow.example_scriptlet
        );
    }

    // =======================================================
    // kit://script-templates resource tests
    // =======================================================

    fn template_ref(id: &str, title: &str, description: &str, category: &str) -> ScriptTemplateRef {
        ScriptTemplateRef {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            category: category.to_string(),
            filename_hint: id.to_string(),
            body_template: "// placeholder for {{NAME}}\n".to_string(),
            metadata_defaults: ScriptTemplateMetadataDefaults::default(),
        }
    }

    #[test]
    fn script_templates_document_has_schema_version_and_templates() {
        let doc = build_script_templates_document();
        assert_eq!(doc.schema_version, SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, doc.templates.len());
        assert!(
            !doc.templates.is_empty(),
            "v1 should ship at least one starter template"
        );
        // Blank Starter must stay in row #1 so the fast path feels identical
        // to the pre-catalog experience.
        assert_eq!(
            doc.templates[0].id, "blank-starter",
            "Blank Starter must be the first row"
        );
    }

    #[test]
    fn every_starter_template_declares_only_real_supported_host_capabilities() {
        for template in build_script_templates_document().templates {
            let source = render_script_template_file(&template, "Compatibility Fixture");
            let parsed = crate::metadata_parser::extract_typed_metadata(&source);
            assert!(
                parsed.errors.is_empty(),
                "template {} has malformed metadata: {:?}",
                template.id,
                parsed.errors
            );
            let metadata = parsed
                .metadata
                .expect("starter template declares typed metadata");
            assert_eq!(
                metadata.extra.get("sdkCapabilities"),
                Some(&serde_json::json!(["arg", "div", "md"])),
                "template {} must truthfully declare the globals it invokes",
                template.id
            );
            assert_eq!(
                metadata.extra.get("executionTopology"),
                Some(&serde_json::json!("typescript-script")),
                "template {} must declare its real interactive script transport",
                template.id
            );

            let script = Script {
                name: "Compatibility Fixture".to_string(),
                path: std::path::PathBuf::from("compatibility-fixture.ts"),
                extension: "ts".to_string(),
                plugin_id: "main".to_string(),
                typed_metadata: Some(metadata),
                ..Script::default()
            };
            assert!(
                crate::scripts::validate_declared_sdk_capabilities(&script).is_empty(),
                "template {} must satisfy its actual host capability contract",
                template.id
            );
        }
    }

    #[test]
    fn script_template_ids_are_unique() {
        let doc = build_script_templates_document();
        let mut ids: Vec<&str> = doc.templates.iter().map(|t| t.id.as_str()).collect();
        ids.sort();
        let original_len = ids.len();
        ids.dedup();
        assert_eq!(
            ids.len(),
            original_len,
            "Template ids must be unique: {ids:?}"
        );
    }

    #[test]
    fn filter_script_template_entries_matches_title_description_and_category() {
        let entries = vec![
            template_ref("t-1", "Blank Starter", "Empty shape", "starter"),
            template_ref("t-2", "Choice List", "Pick one from a list", "prompts"),
            template_ref("t-3", "Daily Note", "Writes today's text", "files"),
        ];
        let all = filter_script_template_entries(&entries, "");
        assert_eq!(all, vec![0, 1, 2]);
        let whitespace = filter_script_template_entries(&entries, "   ");
        assert_eq!(whitespace, vec![0, 1, 2]);

        // Title match (case-insensitive).
        assert_eq!(filter_script_template_entries(&entries, "CHOICE"), vec![1]);
        // Description match.
        assert_eq!(filter_script_template_entries(&entries, "today"), vec![2]);
        // Category match.
        assert_eq!(filter_script_template_entries(&entries, "starter"), vec![0]);
        // No matches.
        assert_eq!(
            filter_script_template_entries(&entries, "no-such-thing"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn render_script_template_file_includes_metadata_name() {
        let template = ScriptTemplateRef {
            id: "demo".into(),
            title: "Demo".into(),
            description: "test".into(),
            category: "starter".into(),
            filename_hint: "demo".into(),
            body_template: concat!(
                "export const metadata = {\n",
                "  name: \"{{NAME}}\",\n",
                "  description: \"{{DESCRIPTION}}\",\n",
                "};\n",
            )
            .into(),
            metadata_defaults: ScriptTemplateMetadataDefaults {
                description: Some("seeded description".into()),
            },
        };
        let rendered = render_script_template_file(&template, "My Friendly Name");
        assert!(
            rendered.contains("name: \"My Friendly Name\""),
            "friendly name should be substituted into metadata.name: {rendered}"
        );
        assert!(
            rendered.contains("description: \"seeded description\""),
            "description default should be substituted: {rendered}"
        );
        assert!(
            !rendered.contains("{{NAME}}"),
            "all placeholders should be replaced: {rendered}"
        );
        assert!(
            !rendered.contains("{{DESCRIPTION}}"),
            "all placeholders should be replaced: {rendered}"
        );
    }

    #[test]
    fn render_script_template_file_escapes_valid_names_without_changing_host_metadata() {
        let friendly_names = [
            r#"John's "Favorite" Script"#,
            "Crème brûlée 東京 🦀 {draft}",
            "Literal {{DESCRIPTION}} and {{NAME}}",
            r#"Harmless"; globalThis.__scriptKitTemplateInjection = true; const text = "data"#,
        ];

        for template in build_script_templates_document().templates {
            for friendly_name in friendly_names {
                let rendered = render_script_template_file(&template, friendly_name);
                let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
                assert!(
                    parsed.errors.is_empty(),
                    "template {} must parse an accepted friendly name safely: {:?}",
                    template.id,
                    parsed.errors,
                );
                let metadata = parsed
                    .metadata
                    .expect("escaped starter must retain its real typed metadata");
                assert_eq!(metadata.name.as_deref(), Some(friendly_name));
                assert_eq!(
                    metadata.extra.get("sdkCapabilities"),
                    Some(&serde_json::json!(["arg", "div", "md"])),
                );
                assert_eq!(
                    metadata.extra.get("executionTopology"),
                    Some(&serde_json::json!("typescript-script")),
                );

                let name_line = rendered
                    .lines()
                    .find(|line| line.trim_start().starts_with("name:"))
                    .expect("starter must expose one metadata name field");
                let expected_literal = Value::String(friendly_name.to_owned()).to_string();
                assert_eq!(name_line.trim(), format!("name: {expected_literal},"));

                let script = Script {
                    name: friendly_name.to_owned(),
                    path: std::path::PathBuf::from("escaped-starter.ts"),
                    extension: "ts".to_owned(),
                    plugin_id: "main".to_owned(),
                    typed_metadata: Some(metadata),
                    ..Script::default()
                };
                assert!(
                    crate::scripts::validate_declared_sdk_capabilities(&script).is_empty(),
                    "escaping must preserve the actual supported starter capabilities"
                );
            }
        }
    }

    #[test]
    fn render_script_template_file_never_recursively_expands_name_or_description_data() {
        let mut template = find_script_template("blank-starter")
            .expect("the real first-run starter must remain available");
        let friendly_name = r#"Keep {{DESCRIPTION}}, {{NAME}}, {braces}, and "quotes" 東京"#;
        let description =
            "Keep {{NAME}}, {{DESCRIPTION}}, braces {}, \\slashes\\, \"quotes\", and\nnewlines";
        template.metadata_defaults.description = Some(description.to_owned());

        let rendered = render_script_template_file(&template, friendly_name);
        let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let metadata = parsed
            .metadata
            .expect("both escaped template fields must parse as ordinary strings");
        assert_eq!(metadata.name.as_deref(), Some(friendly_name));
        assert_eq!(metadata.description.as_deref(), Some(description));

        let expected_name = Value::String(friendly_name.to_owned()).to_string();
        let expected_description = Value::String(description.to_owned()).to_string();
        assert!(rendered.contains(&format!("  name: {expected_name},\n")));
        assert!(rendered.contains(&format!("  description: {expected_description},\n")));
        assert!(rendered.contains("{{NAME}}"));
        assert!(rendered.contains("{{DESCRIPTION}}"));
    }

    #[test]
    fn render_script_template_file_keeps_statement_injection_inside_one_string_literal() {
        let template = find_script_template("choice-list")
            .expect("the production choice-list starter must remain available");
        let friendly_name = r#"Safe"}; globalThis.__SCRIPT_KIT_HOSTILE__ = true; {"name":"Again"#;
        let rendered = render_script_template_file(&template, friendly_name);

        let parsed = crate::metadata_parser::extract_typed_metadata(&rendered);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let metadata = parsed
            .metadata
            .expect("hostile-looking text must remain one parsed metadata value");
        assert_eq!(metadata.name.as_deref(), Some(friendly_name));
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "div", "md"])),
        );

        let expected_literal = Value::String(friendly_name.to_owned()).to_string();
        let name_line = rendered
            .lines()
            .find(|line| line.trim_start().starts_with("name:"))
            .expect("starter name must remain on one metadata line");
        assert_eq!(name_line.trim(), format!("name: {expected_literal},"));
        assert!(!rendered.contains("name: \"Safe\"}; globalThis"));
    }

    #[test]
    fn render_script_template_file_falls_back_to_title_when_no_description_default() {
        let mut template = ScriptTemplateRef {
            id: "demo".into(),
            title: "Demo Title".into(),
            description: "card text".into(),
            category: "starter".into(),
            filename_hint: "demo".into(),
            body_template: "{{DESCRIPTION}}".into(),
            metadata_defaults: ScriptTemplateMetadataDefaults::default(),
        };
        template.metadata_defaults.description = None;
        let rendered = render_script_template_file(&template, "unused");
        assert_eq!(
            rendered, "Demo Title",
            "missing description_default should fall back to title"
        );
    }

    #[test]
    fn find_script_template_returns_template_by_id() {
        let found = find_script_template("blank-starter").expect("blank-starter must exist");
        assert_eq!(found.id, "blank-starter");
    }

    #[test]
    fn find_script_template_returns_none_for_unknown_id() {
        assert!(find_script_template("no-such-template-id").is_none());
    }

    #[test]
    fn starter_templates_do_not_emit_collision_binding_fields() {
        let doc = build_script_templates_document();
        for template in &doc.templates {
            let rendered = render_script_template_file(template, "Demo");
            for banned in ["alias:", "shortcut:", "keyword:", "trigger:"] {
                assert!(
                    !rendered.contains(banned),
                    "Template `{}` must not emit `{}` (would be fatally hidden by validate_script_catalog). Rendered:\n{}",
                    template.id,
                    banned,
                    rendered
                );
            }
        }
    }

    #[test]
    fn script_templates_resource_is_listed_and_readable() {
        let resources = get_resource_definitions();
        let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(
            uris.contains(&SCRIPT_TEMPLATES_RESOURCE_URI),
            "{SCRIPT_TEMPLATES_RESOURCE_URI} should be in resource definitions"
        );

        let content = read_resource(SCRIPT_TEMPLATES_RESOURCE_URI, &[], &[], None)
            .expect("script-templates resource should be readable");
        assert_eq!(content.uri, SCRIPT_TEMPLATES_RESOURCE_URI);
        assert_eq!(content.mime_type, "application/json");
        let doc: ScriptTemplatesResourceDocument =
            serde_json::from_str(&content.text).expect("valid JSON envelope");
        assert_eq!(doc.schema_version, SCRIPT_TEMPLATES_RESOURCE_SCHEMA_VERSION);
        assert_eq!(doc.count, doc.templates.len());
    }
}
