use anyhow::{Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::config::ModelInfo;
use super::providers::{AiProvider, ProviderMessage, ProviderRegistry};
use crate::menu_bar::current_app_commands::CurrentAppCommandRecipe;

const AI_SCRIPT_DEFAULT_SLUG: &str = "ai-script";
const AI_SCRIPT_MAX_SLUG_LEN: usize = 64;
const SCRIPT_KIT_SDK_IMPORT_MODULE: &str = "@scriptkit/sdk";
const SCRIPT_KIT_SDK_IMPORT_STATEMENT: &str = "import \"@scriptkit/sdk\";";
const AI_SCRIPT_USER_REQUEST_START_DELIMITER: &str = "---USER_REQUEST---";
const AI_SCRIPT_USER_REQUEST_END_DELIMITER: &str = "---END_REQUEST---";
pub const AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION: u32 = 2;
const AI_GENERATED_SCRIPT_VERIFY_TIMEOUT: Duration = Duration::from_secs(15);
const AI_GENERATED_SCRIPT_VERIFY_OUTPUT_LIMIT: usize = 4096;
static AI_GENERATED_RECEIPT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const AI_SCRIPT_SHELL_EXECUTION_PATTERNS: [(&str, &str); 5] = [
    ("child_process", "child_process"),
    ("exec", "exec"),
    ("execSync", "execsync"),
    ("spawn", "spawn"),
    ("spawnSync", "spawnsync"),
];

pub const AI_SCRIPT_GENERATION_SYSTEM_PROMPT: &str = r#"You write production-ready Script Kit TypeScript scripts.

CRITICAL: Return ONLY TypeScript code. No prose, markdown fences, explanations, preamble, or postamble. Start immediately with valid TypeScript source code.

NON-NEGOTIABLE OUTPUT FORMAT

Every script MUST start with:
import "@scriptkit/sdk";
export const metadata = {
  name: "<short, clear, user-facing title>",
  description: "<one-line summary>",
  sdkCapabilities: ["<every SDK capability this script actually invokes>"],
  executionTopology: "typescript-script",
};

Do NOT use legacy comment-header metadata (// Name:, // Description:). Always use export const metadata.

RULES:
1. Include EXACTLY ONE import (and no others): import "@scriptkit/sdk";
2. Follow the import with export const metadata = { name, description, sdkCapabilities, executionTopology }.
3. Use top-level await (no main(), no async IIFE, no servers).
4. Prefer Script Kit prompts + UI over console.log.
5. NO markdown fences. NO explanations. NO commentary. ONLY TypeScript code.
6. Use only actual supported SDK capabilities; the host-owned denylist appended to this prompt is authoritative.
7. TypeScript scripts and launcher-opened TypeScript scriptlets have interactive SDK transport. Only legacy synchronous TypeScript scriptlets lack interactive stdin; shell/Python scriptlets do not receive SDK globals.

RUNTIME ASSUMPTIONS

* Script Kit provides the exact reviewed globals listed below; JavaScript's standard fetch is available for HTTP.
* Do not import node:* modules. Use home/skPath/kitPath/tmpPath for filesystem paths.
* path(...) is a prompt function, not Node's path module: there is no path.join/path.basename/path.extname.
* Write scripts that feel like native tools: interactive, fast, keyboard-friendly.

UX QUALITY BAR

* Ask for missing inputs (arg/fields/path/drop/editor). Don't hardcode what you can prompt for.
* Default to the shared main-menu-sized prompt flow. Prefer arg/fields/select on the launcher shell before inventing a denser surface.
* Expanded split-view browsers are rare exceptions for preview-dense workflows like file search or clipboard history. Do not build one unless the preview is essential to the selection decision.
* Prefer interactive UI over logs: use arg/fields/select for input and use div(md(...)) or editor(...) after the user has made a choice.
* Lists should usually be simple choice objects: { name, value, description? }.
* Do not use choice `preview` fields, `setPreview()`, or `setPanel()` for ordinary commands. setPreview and setPanel are unsupported in GPUI and must never be called.
* Add actions for common operations (Copy/Open/Save/Retry) via Action[] on arg/div/editor/fields/form or setActions().
* Action shortcuts are explicit strings such as "cmd+c"; no global cmd helper exists.
* Prompt APIs are stateful UI surfaces. Never call them concurrently.
  * Do NOT use Promise.all / Promise.race / Promise.any / Promise.allSettled with arg, fields, editor, div, form, drop, path, select, mini, micro, hotkey, or confirm.
  * Multi-step prompt flows must always be sequential:
    `const first = await arg("First");`
    `const second = await arg("Second");`

ERROR HANDLING

* Treat Esc/cancel as normal: catch and exit quietly, or use hud/notify if useful.
* Validate input immediately after each sequential prompt.
* exec(executable, args?) uses shell:false; show typed error.result.stderr/stdout on failure and suggest next steps.
* Never use shell operators, command substitution, keyboard injection, mouse injection, screen capture, microphone, webcam, or unsupported SDK helpers.
* For long tasks use hud or an intentional div/editor; setStatus/setLoading/setProgress are unavailable.

SCRIPT KIT IDIOMS (PREFERRED)

* await arg()/select()/fields()/editor()/div(md(...))
* home()/skPath()/kitPath()/tmpPath() for resolved filesystem paths (no imports)
* When you must target the Script Kit workspace explicitly, use skPath("plugins", "main", ...)
* clipboard.* + clipboardHistory() for clipboard tools
* exec("open", [filePath]) executes an explicit binary/argv without a shell
* Use process.platform for intentional platform-specific behavior

TEACH BY EXAMPLE (REFERENCE ONLY — ADAPT PATTERNS, DO NOT COPY VERBATIM)

Example 1 — Simple input → output (arg + file write)
import "@scriptkit/sdk";
export const metadata = { name: "Save Note", description: "Save a note as a text file", sdkCapabilities: ["arg", "home", "writeFile", "hud"], executionTopology: "typescript-script" };
const note = await arg("Note text");
const filePath = home("Documents", `note-${Date.now()}.txt`);
await writeFile(filePath, note, "utf8");
hud("Note saved");

Example 2 — Main-menu-sized list with follow-up detail
import "@scriptkit/sdk";
export const metadata = { name: "Clipboard Picker", description: "Choose a recent clipboard item", sdkCapabilities: ["clipboardHistory", "arg", "clipboard.writeText", "hud"], executionTopology: "typescript-script" };
const items = (await clipboardHistory()).filter(item => item.contentType === "text").slice(0, 100);
const value = await arg("Pick a clipboard item", items.map((i) => ({
  name: i.content.slice(0, 80),
  description: new Date(i.timestamp).toLocaleString(),
  value: i.content,
})));
await clipboard.writeText(value);
hud("Copied");

Example 3 — Multi-step workflow + rich HTML output (div(md()))
import "@scriptkit/sdk";
export const metadata = { name: "Markdown Card Builder", description: "Write and preview a markdown document", sdkCapabilities: ["fields", "editor", "div", "md", "clipboard.writeText", "writeFile", "home"], executionTopology: "typescript-script" };
const [title, tags] = await fields(["Title", "Tags (comma-separated)"]);
const initial = `# ${title}\n\nTags: ${tags}\n\nWrite your content here...\n`;
const markdown = await editor(initial);
await div({ html: md(markdown), containerClasses: "p-6 prose dark:prose-invert" }, [
  { name: "Copy", shortcut: "cmd+c", onAction: () => clipboard.writeText(markdown) },
  { name: "Save", shortcut: "cmd+s", onAction: () => writeFile(home("Desktop", `${title}.md`), markdown, "utf8") },
]);

Example 4 — Explicit AI handoff without hidden provider submission
import "@scriptkit/sdk";
export const metadata = { name: "Prepare AI Rewrite", description: "Stage an explicit rewrite request for review", sdkCapabilities: ["arg", "editor", "aiStartChat"], executionTopology: "typescript-script" };
const tone = await arg("Tone", ["Concise", "Friendly", "Professional"]);
const input = await editor("Paste text to rewrite...");
await aiStartChat(`Rewrite this text in a ${tone} tone: ${input}`, { noResponse: true });

Example 5 — System automation (exec + readFile/writeFile)
import "@scriptkit/sdk";
export const metadata = { name: "Quick Replace In File", description: "Replace text in a file and open it", sdkCapabilities: ["path", "arg", "readFile", "writeFile", "exec", "hud"], executionTopology: "typescript-script" };
const filePath = await path({ hint: "Select a text file to edit" });
const findText = await arg("Find");
const replaceText = await arg("Replace with");
const before = await readFile(filePath, "utf8");
await writeFile(filePath, before.split(findText).join(replaceText), "utf8");
await exec("open", [filePath]);
hud("Updated");

COMPACT API REFERENCE (ONE LINE PER FUNCTION, GROUPED)

Prompts & Rendering
* arg(prompt?, choices?, actions?) — text input or searchable choices
* select(prompt, choices) — multi-select list; returns string[]
* fields(definitions, actions?) — typed form; returns string[]
* editor(content?, language?, actions?) — edit text; returns string
* div(htmlOrConfig?, actions?) — render HTML; returns string or void
* form(html, actions?) — HTML form; returns Record<string, string>
* drop() — chosen file descriptors; returns { path, name, size }[]
* path({ startPath?, hint? }?) — file/folder picker only; no path.join utilities
* mini(prompt, choices), micro(prompt, choices), hotkey(prompt?), confirm(...) — supported compact prompts

UI Helpers
* md(markdown) — markdown to HTML
* hud(message, { duration? }?) — host-owned status message
* notify(bodyOrOptions) — system notification with a typed dispatch receipt; OS delivery is not guaranteed
* setActions(actions) — action palette w/ shortcuts

Files & Paths
* home(...) — home-relative path formed from path segments
* home(".scriptkit", ...) — explicit Script Kit workspace path when needed
* skPath(...), kitPath(...), tmpPath(...) — host-owned filesystem paths
* readFile(filePath, encoding?) — read UTF-8 text
* writeFile(filePath, text, encoding?) — write text
* fileSearch(query, { onlyin? }?) — noninteractive indexed filesystem search
* fetch(url, options?) — standard JavaScript HTTP API

Automation
* exec(executable, args?: readonly string[]) — direct shell:false subprocess with typed stdout/stderr/exitCode
* browse(url) — open in browser
* editFile(filePath) — open in external editor
* clipboard.readText() — read clipboard text
* clipboard.writeText(text) — write clipboard text
* clipboard.readImage() — read image buffer
* clipboard.writeImage(buffer) — write image buffer
* copy(text), paste() — copy text or read clipboard text; paste does not inject keystrokes
* clipboardHistory() — { entryId, content, contentType, timestamp, pinned }[]
* clipboardHistoryPin(entryId), clipboardHistoryUnpin(entryId), clipboardHistoryRemove(entryId)
* clipboardHistoryClear(), clipboardHistoryTrimOversize()

AI
* aiIsOpen(), aiGetActiveChat(), aiListChats(), aiGetConversation(), aiGetStreamingStatus() — read-only AI state
* aiStartChat(message, { noResponse: true }) — stage a visible draft without inference
* aiSendMessage(chatId, text, imagePath?, parts?) — explicitly request AI inference
* aiAppendMessage(chatId, text, role) — append a message without requesting inference
* chat(options?) — prompt UI; the script owns generation
* mcp.listServers/getServer/listTools/discover/call — MCP object methods; mcp is not callable

FINAL CHECKLIST
* Only TypeScript source code.
* Includes export const metadata = { name, description, sdkCapabilities, executionTopology } (do NOT use // Name: or // Description: comment headers).
* Never use kenvPath() or reference ~/.kenv.
* Exactly one import: import "@scriptkit/sdk";
* Top-level await + only reviewed, supported Script Kit globals.
* Interactive UX (sequential prompts and executable actions) instead of console output.
* Practical errors + safe cancellation."#;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GeneratedScriptMetadataStyle {
    CommentHeaders,
    MetadataExport,
    Hybrid,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedScriptContractAudit {
    pub metadata_style: GeneratedScriptMetadataStyle,
    pub has_name: bool,
    pub has_description: bool,
    pub has_kit_import: bool,
    pub has_current_app_recipe_header: bool,
    pub current_app_recipe_header_at_top: bool,
    /// Explicit author claims, never inferred from unrelated source text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<crate::mcp_resources::SdkExecutionTopology>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata_parse_errors: Vec<String>,
    /// Pending permission warnings remain recoverable; fatal claims block save.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_issues: Vec<crate::scripts::ScriptValidationIssue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GeneratedScriptVerificationStatus {
    Passed,
    Failed,
    Skipped,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedScriptVerificationReceipt {
    pub status: GeneratedScriptVerificationStatus,
    pub command_kind: String,
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

impl Default for GeneratedScriptVerificationReceipt {
    fn default() -> Self {
        Self::skipped("legacy_receipt_missing_verification")
    }
}

impl GeneratedScriptVerificationReceipt {
    pub fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: GeneratedScriptVerificationStatus::Skipped,
            command_kind: "not_run".to_string(),
            command: Vec::new(),
            exit_code: None,
            duration_ms: 0,
            output_path: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            diagnostics: vec![reason.into()],
        }
    }

    fn blocked(reason: impl Into<String>, command_kind: impl Into<String>) -> Self {
        Self {
            status: GeneratedScriptVerificationStatus::Blocked,
            command_kind: command_kind.into(),
            command: Vec::new(),
            exit_code: None,
            duration_ms: 0,
            output_path: None,
            stdout_excerpt: None,
            stderr_excerpt: None,
            diagnostics: vec![reason.into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedScriptReceipt {
    pub schema_version: u32,
    pub prompt: String,
    pub slug: String,
    pub slug_source: String,
    pub slug_source_kind: String,
    pub model_id: String,
    pub provider_id: String,
    pub script_path: String,
    pub receipt_path: String,
    pub shell_execution_warning: bool,
    pub contract: GeneratedScriptContractAudit,
    #[serde(default)]
    pub verification: GeneratedScriptVerificationReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_app_recipe: Option<CurrentAppCommandRecipe>,
}

#[derive(Debug, Clone)]
struct PreparedGeneratedScript {
    slug: String,
    source: String,
    slug_source: String,
    slug_source_kind: &'static str,
    contract: GeneratedScriptContractAudit,
}

/// One pure pre-write security policy for both provider and Agent Chat saves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedScriptPersistencePlan {
    requested_slug: String,
    shell_execution_warning: bool,
    suspicious_shell_patterns: Vec<&'static str>,
}

fn generated_script_persistence_plan(
    prompt: &str,
    source: &str,
    derived_slug: &str,
    slug_override: Option<&str>,
) -> Result<GeneratedScriptPersistencePlan> {
    let requested_slug =
        crate::script_creation::sanitize_name(slug_override.unwrap_or(derived_slug));
    if requested_slug.is_empty() {
        anyhow::bail!("Generated script name is empty after safe sanitization");
    }
    let suspicious_shell_patterns = detect_unexpected_shell_execution_patterns(prompt, source);
    Ok(GeneratedScriptPersistencePlan {
        requested_slug,
        shell_execution_warning: !suspicious_shell_patterns.is_empty(),
        suspicious_shell_patterns,
    })
}

fn generated_script_created_slug(path: &Path) -> Result<String> {
    let slug = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("Generated script path has no safe file name"))?;
    if slug.is_empty() || crate::script_creation::sanitize_name(slug) != slug {
        anyhow::bail!("Generated script path does not contain a safely sanitized file name");
    }
    Ok(slug.to_string())
}

#[derive(Debug, Clone)]
pub struct GeneratedScriptOutput {
    pub path: PathBuf,
    pub slug: String,
    pub model_id: String,
    pub provider_id: String,
    pub shell_execution_warning: bool,
}

pub fn generated_script_receipt_path(script_path: &Path) -> PathBuf {
    let mut receipt_path = script_path.to_path_buf();
    receipt_path.set_extension("scriptkit.json");
    receipt_path
}

fn safe_generated_script_detail(raw: &str) -> String {
    let redacted = crate::ai::reliability::redact_diagnostic(raw);
    redacted
        .copyable_detail
        .unwrap_or_else(|| format!("[REDACTED:{}]", redacted.fingerprint.0))
}

fn ensure_safe_receipt_destination(receipt_path: &Path) -> Result<()> {
    match fs::symlink_metadata(receipt_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("Refusing to replace a symbolic-link generated script receipt")
        }
        Ok(metadata) if !metadata.is_file() => {
            anyhow::bail!("Generated script receipt destination is not a regular file")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => anyhow::bail!(
            "Unable to inspect generated script receipt destination: {}",
            safe_generated_script_detail(&error.to_string())
        ),
    }
}

struct PendingGeneratedReceipt {
    path: PathBuf,
    committed: bool,
}

impl Drop for PendingGeneratedReceipt {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn write_generated_script_receipt(
    receipt_path: &Path,
    receipt: &GeneratedScriptReceipt,
) -> Result<()> {
    let json = serde_json::to_string_pretty(receipt)
        .context("Failed to serialize generated script receipt")?;
    ensure_safe_receipt_destination(receipt_path)?;
    let parent = receipt_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = receipt_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Generated script receipt has no safe file name"))?;

    for _ in 0..32 {
        let sequence = AI_GENERATED_RECEIPT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => anyhow::bail!(
                "Failed creating isolated generated script receipt: {}",
                safe_generated_script_detail(&error.to_string())
            ),
        };
        let mut pending = PendingGeneratedReceipt {
            path: temp_path,
            committed: false,
        };
        file.write_all(json.as_bytes()).map_err(|error| {
            anyhow::anyhow!(
                "Failed writing isolated generated script receipt: {}",
                safe_generated_script_detail(&error.to_string())
            )
        })?;
        drop(file);

        // Rename replaces a directory entry rather than following it. Reject
        // visible symlink destinations anyway so malicious adjacent files
        // cannot become an accidental overwrite or disclosure target.
        ensure_safe_receipt_destination(receipt_path)?;
        fs::rename(&pending.path, receipt_path).map_err(|error| {
            anyhow::anyhow!(
                "Failed publishing generated script receipt atomically: {}",
                safe_generated_script_detail(&error.to_string())
            )
        })?;
        pending.committed = true;
        return Ok(());
    }

    anyhow::bail!("Failed to allocate an isolated generated script receipt")
}

fn truncate_verification_output(output: &[u8]) -> Option<String> {
    if output.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(output);
    let mut excerpt = text
        .chars()
        .take(AI_GENERATED_SCRIPT_VERIFY_OUTPUT_LIMIT)
        .collect::<String>();
    if text.chars().count() > AI_GENERATED_SCRIPT_VERIFY_OUTPUT_LIMIT {
        excerpt.push_str("\n... truncated ...");
    }
    Some(safe_generated_script_detail(&excerpt))
}

fn verification_output_path(script_path: &Path) -> PathBuf {
    let stem = script_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("generated-script");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!(
        "script-kit-generated-verification-{stem}-{timestamp}.mjs"
    ))
}

fn verify_generated_script_with_bun_build(
    script_path: &Path,
) -> GeneratedScriptVerificationReceipt {
    let output_path = verification_output_path(script_path);
    let command = vec![
        "bun".to_string(),
        "build".to_string(),
        script_path.display().to_string(),
        "--target=bun".to_string(),
        "--external".to_string(),
        SCRIPT_KIT_SDK_IMPORT_MODULE.to_string(),
        "--outfile".to_string(),
        output_path.display().to_string(),
    ];
    let started = Instant::now();

    let mut child = match Command::new("bun")
        .arg("build")
        .arg(script_path)
        .arg("--target=bun")
        .arg("--external")
        .arg(SCRIPT_KIT_SDK_IMPORT_MODULE)
        .arg("--outfile")
        .arg(&output_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let mut receipt = GeneratedScriptVerificationReceipt::blocked(
                safe_generated_script_detail(&error.to_string()),
                "bun_build",
            );
            receipt.command = command;
            receipt.output_path = Some(output_path.display().to_string());
            return receipt;
        }
    };

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                match child.wait_with_output() {
                    Ok(output) => {
                        let status = if output.status.success() {
                            GeneratedScriptVerificationStatus::Passed
                        } else {
                            GeneratedScriptVerificationStatus::Failed
                        };
                        return GeneratedScriptVerificationReceipt {
                            status,
                            command_kind: "bun_build".to_string(),
                            command,
                            exit_code: output.status.code(),
                            duration_ms,
                            output_path: Some(output_path.display().to_string()),
                            stdout_excerpt: truncate_verification_output(&output.stdout),
                            stderr_excerpt: truncate_verification_output(&output.stderr),
                            diagnostics: Vec::new(),
                        };
                    }
                    Err(error) => {
                        return GeneratedScriptVerificationReceipt {
                            status: GeneratedScriptVerificationStatus::Blocked,
                            command_kind: "bun_build".to_string(),
                            command,
                            exit_code: None,
                            duration_ms,
                            output_path: Some(output_path.display().to_string()),
                            stdout_excerpt: None,
                            stderr_excerpt: None,
                            diagnostics: vec![format!(
                                "verification_output_read_failed: {}",
                                safe_generated_script_detail(&error.to_string())
                            )],
                        };
                    }
                }
            }
            Ok(None) => {
                if started.elapsed() >= AI_GENERATED_SCRIPT_VERIFY_TIMEOUT {
                    let _ = child.kill();
                    let output = child.wait_with_output().ok();
                    return GeneratedScriptVerificationReceipt {
                        status: GeneratedScriptVerificationStatus::Blocked,
                        command_kind: "bun_build".to_string(),
                        command,
                        exit_code: output.as_ref().and_then(|output| output.status.code()),
                        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                        output_path: Some(output_path.display().to_string()),
                        stdout_excerpt: output
                            .as_ref()
                            .and_then(|output| truncate_verification_output(&output.stdout)),
                        stderr_excerpt: output
                            .as_ref()
                            .and_then(|output| truncate_verification_output(&output.stderr)),
                        diagnostics: vec!["verification_timed_out".to_string()],
                    };
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                return GeneratedScriptVerificationReceipt {
                    status: GeneratedScriptVerificationStatus::Blocked,
                    command_kind: "bun_build".to_string(),
                    command,
                    exit_code: None,
                    duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    output_path: Some(output_path.display().to_string()),
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    diagnostics: vec![format!(
                        "verification_wait_failed: {}",
                        safe_generated_script_detail(&error.to_string())
                    )],
                };
            }
        }
    }
}

pub(crate) fn write_script_creation_receipt_for_path(
    script_path: &Path,
    prompt: &str,
    slug_source: &str,
    slug_source_kind: &str,
    model_id: &str,
    provider_id: &str,
) -> Result<GeneratedScriptReceipt> {
    let source = fs::read_to_string(script_path).with_context(|| {
        format!(
            "Failed reading created script for verification receipt (state=script_creation_receipt_read_failed, path={})",
            script_path.display()
        )
    })?;
    let slug = script_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or_else(|| slugify_script_name(slug_source));
    let contract = audit_generated_script_contract(&source);
    let verification = verify_generated_script_with_bun_build(script_path);
    let receipt_path = generated_script_receipt_path(script_path);
    let shell_execution_warning =
        !detect_unexpected_shell_execution_patterns(prompt, &source).is_empty();
    let receipt = GeneratedScriptReceipt {
        schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
        prompt: safe_generated_script_detail(prompt.trim()),
        slug,
        slug_source: safe_generated_script_detail(slug_source),
        slug_source_kind: slug_source_kind.to_string(),
        model_id: model_id.to_string(),
        provider_id: provider_id.to_string(),
        script_path: script_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        shell_execution_warning,
        contract,
        verification,
        current_app_recipe: None,
    };

    write_generated_script_receipt(&receipt_path, &receipt)?;
    Ok(receipt)
}

pub fn extract_current_app_recipe_from_script(
    script_source: &str,
) -> Option<CurrentAppCommandRecipe> {
    use base64::Engine as _;

    let encoded = script_source.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("// Current-App-Recipe-Base64:")
            .map(str::trim)
    })?;

    if encoded.is_empty() {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let json = String::from_utf8(bytes).ok()?;

    crate::menu_bar::current_app_commands::parse_current_app_command_recipe_json(&json).ok()
}

pub fn generate_script_from_prompt(
    prompt: &str,
    config: Option<&crate::config::Config>,
) -> Result<GeneratedScriptOutput> {
    let (output, _receipt) = generate_script_from_prompt_with_receipt(prompt, config)?;
    Ok(output)
}

pub fn generate_script_from_prompt_with_receipt(
    prompt: &str,
    config: Option<&crate::config::Config>,
) -> Result<(GeneratedScriptOutput, GeneratedScriptReceipt)> {
    let normalized_prompt = prompt.trim();
    if normalized_prompt.is_empty() {
        anyhow::bail!("AI script generation requires a non-empty prompt");
    }

    let registry = ProviderRegistry::from_environment_with_config(config);
    if !registry.has_any_provider() {
        anyhow::bail!(
            "No AI providers configured. Configure an API key first (Vercel, OpenAI, Anthropic, etc.)."
        );
    }

    let (selected_model, provider) = select_generation_model(&registry)?;
    tracing::info!(
        target: "ai",
        correlation_id = "ai-script-generation",
        state = "provider_ready",
        model_id = %selected_model.id,
        provider_id = %selected_model.provider,
        prompt_len = normalized_prompt.len(),
        "Script generation provider ready"
    );

    let messages = build_script_generation_messages(normalized_prompt);

    let raw_response = provider
        .send_message(&messages, &selected_model.id)
        .with_context(|| {
            format!(
                "AI script generation failed (attempted=send_message, model_id={}, provider_id={})",
                selected_model.id, selected_model.provider
            )
        })?;

    let prepared = prepare_script_from_ai_response_with_contract(normalized_prompt, &raw_response)?;

    let persistence_plan = generated_script_persistence_plan(
        normalized_prompt,
        &prepared.source,
        &prepared.slug,
        None,
    )?;
    if persistence_plan.shell_execution_warning {
        tracing::warn!(
            target: "ai",
            correlation_id = "ai-script-generation",
            state = "suspicious_shell_pattern_detected",
            patterns = ?persistence_plan.suspicious_shell_patterns,
            model_id = %selected_model.id,
            provider_id = %selected_model.provider,
            "AI-generated script includes shell execution patterns without explicit shell intent"
        );
    }

    let path = crate::script_creation::create_new_script_with_contents(
        &persistence_plan.requested_slug,
        &prepared.source,
    )
    .with_context(|| {
        format!(
            "Failed creating AI-generated script (state=create_failed, slug={})",
            persistence_plan.requested_slug
        )
    })?;
    let created_slug = generated_script_created_slug(&path)?;

    let receipt_path = generated_script_receipt_path(&path);
    let verification = verify_generated_script_with_bun_build(&path);
    let receipt = GeneratedScriptReceipt {
        schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
        prompt: safe_generated_script_detail(normalized_prompt),
        slug: created_slug.clone(),
        slug_source: safe_generated_script_detail(&prepared.slug_source),
        slug_source_kind: prepared.slug_source_kind.to_string(),
        model_id: selected_model.id.clone(),
        provider_id: selected_model.provider.clone(),
        script_path: path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        shell_execution_warning: persistence_plan.shell_execution_warning,
        contract: prepared.contract.clone(),
        verification,
        current_app_recipe: None,
    };

    write_generated_script_receipt(&receipt_path, &receipt)?;

    if let Err(error) = crate::ai::upsert_current_app_automation_memory_from_receipt(&receipt) {
        tracing::warn!(
            target: "ai",
            error = %safe_generated_script_detail(&error.to_string()),
            slug = %receipt.slug,
            receipt_path = %receipt.receipt_path,
            "current_app_automation_memory.upsert_failed"
        );
    }

    tracing::info!(
        target: "ai",
        correlation_id = "ai-script-generation",
        state = "script_written",
        path = %path.display(),
        receipt_path = %receipt_path.display(),
        slug = %created_slug,
        metadata_style = ?prepared.contract.metadata_style,
        contract_warning_count = prepared.contract.warnings.len(),
        "AI-generated script written"
    );

    let output = GeneratedScriptOutput {
        path,
        slug: created_slug,
        model_id: selected_model.id,
        provider_id: selected_model.provider,
        shell_execution_warning: persistence_plan.shell_execution_warning,
    };

    Ok((output, receipt))
}

pub(crate) fn prepare_script_from_ai_response(
    prompt: &str,
    raw_response: &str,
) -> Result<(String, String)> {
    let prepared = prepare_script_from_ai_response_with_contract(prompt, raw_response)?;
    Ok((prepared.slug, prepared.source))
}

fn prepare_script_from_ai_response_with_contract(
    prompt: &str,
    raw_response: &str,
) -> Result<PreparedGeneratedScript> {
    let normalized_prompt = prompt.trim();
    if normalized_prompt.is_empty() {
        anyhow::bail!("AI script generation requires a non-empty prompt");
    }

    let extracted = extract_script_code(raw_response);
    if extracted.trim().is_empty() {
        anyhow::bail!("AI returned an empty response for script generation (state=empty_response)");
    }

    let (slug_source, slug_source_kind) = resolve_slug_source(&extracted, normalized_prompt);

    tracing::info!(
        target: "ai",
        correlation_id = "ai-script-generation",
        state = "slug_source_resolved",
        source = slug_source_kind,
        slug_source = %safe_generated_script_detail(&slug_source),
        "Resolved slug source for generated script"
    );

    let slug = slugify_script_name(&slug_source);
    let finalized = enforce_script_kit_conventions(&extracted, normalized_prompt, &slug);
    let contract = audit_generated_script_contract(&finalized);

    if contract
        .warnings
        .iter()
        .any(|warning| warning == "concurrent_prompt_apis")
    {
        anyhow::bail!(
            "Generated script contract invalid (state=concurrent_prompt_apis). \
             Script Kit prompt APIs must not be called concurrently with Promise combinators."
        );
    }

    if !contract.metadata_parse_errors.is_empty() {
        anyhow::bail!(
            "Generated script host compatibility rejected before file creation \
             (state=generated_script_metadata_invalid): {}",
            contract.metadata_parse_errors.join("; ")
        );
    }

    if let Some(issue) = contract
        .capability_issues
        .iter()
        .find(|issue| issue.severity == crate::scripts::ValidationSeverity::Fatal)
    {
        let detail = crate::scripts::format_script_validation_issue_detail(issue);
        anyhow::bail!(
            "Generated script host compatibility rejected before file creation \
             (state=generated_script_capability_unavailable): {}{}",
            issue.message,
            if detail.is_empty() {
                String::new()
            } else {
                format!(" — {detail}")
            }
        );
    }

    if !contract.warnings.is_empty() {
        tracing::warn!(
            target: "ai",
            correlation_id = "ai-script-generation",
            state = "contract_warnings",
            warnings = ?contract.warnings,
            metadata_style = ?contract.metadata_style,
            "Generated script finalized with contract warnings"
        );
    }

    Ok(PreparedGeneratedScript {
        slug,
        source: finalized,
        slug_source,
        slug_source_kind,
        contract,
    })
}

fn resolve_slug_source(script: &str, normalized_prompt: &str) -> (String, &'static str) {
    if let Some(name) = extract_name_comment(script) {
        (name, "comment_header")
    } else if let Some(name) = extract_metadata_name(script) {
        (name, "metadata_export")
    } else {
        (normalized_prompt.to_string(), "normalized_prompt")
    }
}

pub(crate) fn save_generated_script_from_response(
    prompt: &str,
    raw_response: &str,
) -> Result<PathBuf> {
    save_generated_script_from_response_with_slug(prompt, raw_response, None)
}

/// Save an AI-generated script through the full contract pipeline —
/// extraction, convention enforcement, contract audit (rejecting
/// concurrent-prompt-API usage), receipt, and Bun verification.
///
/// `slug_override` preserves a user-chosen filename (e.g. the Tab-AI save
/// offer) while still routing the source through the contract checks.
///
/// Bun verification runs on a background thread: callers are on the UI
/// thread and `bun build` can take up to its 15s timeout. The receipt is
/// written immediately with a skipped/pending marker and rewritten with the
/// real verification result when the background run completes.
pub(crate) fn save_generated_script_from_response_with_slug(
    prompt: &str,
    raw_response: &str,
    slug_override: Option<&str>,
) -> Result<PathBuf> {
    let prepared = prepare_script_from_ai_response_with_contract(prompt, raw_response)?;
    let persistence_plan =
        generated_script_persistence_plan(prompt, &prepared.source, &prepared.slug, slug_override)?;
    if persistence_plan.shell_execution_warning {
        tracing::warn!(
            target: "ai",
            correlation_id = "ai-script-generation",
            state = "suspicious_shell_pattern_detected",
            patterns = ?persistence_plan.suspicious_shell_patterns,
            "Saved AI-generated script includes shell execution patterns without explicit shell intent"
        );
    }
    let script_path = crate::script_creation::create_new_script_with_contents(
        &persistence_plan.requested_slug,
        &prepared.source,
    )
    .with_context(|| {
        format!(
            "Failed to create script for AI response (state=create_failed, sanitized_slug={})",
            persistence_plan.requested_slug
        )
    })?;
    let slug = generated_script_created_slug(&script_path)?;

    let receipt_path = generated_script_receipt_path(&script_path);
    let mut receipt = GeneratedScriptReceipt {
        schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
        prompt: safe_generated_script_detail(prompt.trim()),
        slug,
        slug_source: safe_generated_script_detail(&prepared.slug_source),
        slug_source_kind: prepared.slug_source_kind.to_string(),
        model_id: "unknown".to_string(),
        provider_id: "unknown".to_string(),
        script_path: script_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        shell_execution_warning: persistence_plan.shell_execution_warning,
        contract: prepared.contract,
        verification: GeneratedScriptVerificationReceipt::skipped(
            "bun_build_running_in_background",
        ),
        current_app_recipe: None,
    };
    write_generated_script_receipt(&receipt_path, &receipt)?;

    let verify_script_path = script_path.clone();
    let verify_receipt_path = receipt_path;
    std::thread::spawn(move || {
        receipt.verification = verify_generated_script_with_bun_build(&verify_script_path);
        if let Err(error) = write_generated_script_receipt(&verify_receipt_path, &receipt) {
            tracing::warn!(
                target: "ai",
                error = %safe_generated_script_detail(&error.to_string()),
                receipt_path = %receipt.receipt_path,
                "generated_script_receipt.verification_rewrite_failed"
            );
        }
        if let Err(error) = crate::ai::upsert_current_app_automation_memory_from_receipt(&receipt) {
            tracing::warn!(
                target: "ai",
                error = %safe_generated_script_detail(&error.to_string()),
                slug = %receipt.slug,
                receipt_path = %receipt.receipt_path,
                "current_app_automation_memory.upsert_failed"
            );
        }
    });

    Ok(script_path)
}

pub(crate) fn select_generation_model(
    registry: &ProviderRegistry,
) -> Result<(ModelInfo, Arc<dyn AiProvider>)> {
    let models = registry.get_all_models();
    let selected_model = models
        .iter()
        .find(|model| model.provider.eq_ignore_ascii_case("vercel"))
        .or_else(|| models.first())
        .cloned()
        .context("No AI models available in provider registry")?;

    let provider = registry
        .find_provider_for_model(&selected_model.id)
        .cloned()
        .with_context(|| {
            format!(
                "No provider found for selected model '{}' (state=provider_missing)",
                selected_model.id
            )
        })?;

    Ok((selected_model, provider))
}

fn build_script_generation_messages(normalized_prompt: &str) -> Vec<ProviderMessage> {
    let unsupported = crate::mcp_resources::unsupported_sdk_capability_names().join(", ");
    let system_prompt = format!(
        "{AI_SCRIPT_GENERATION_SYSTEM_PROMPT}\n\nHOST SDK CAPABILITY CONTRACT\n\nThe following host-owned SDK identifiers are unsupported and MUST NEVER be invoked in generated code: {unsupported}. Choose a supported alternative from SDK Reference, preserve the user's intent, and never promise unavailable automation. If your host has already supplied MCP resource access, `kit://command-doctor` reports command readiness and safe permission-pending state, while `kit://failed-scripts` reports author validation issues. Resource URIs are host-side diagnostics, never callable SDK functions; do not invent `commandDoctor()`, request permissions, probe the system, or contact a provider merely to inspect them."
    );
    vec![
        ProviderMessage::system(system_prompt),
        ProviderMessage::user(format!(
            "Generate a Script Kit script for this user request:\n\n{}\n{}\n{}",
            AI_SCRIPT_USER_REQUEST_START_DELIMITER,
            normalized_prompt,
            AI_SCRIPT_USER_REQUEST_END_DELIMITER
        )),
    ]
}

fn prompt_allows_shell_execution(prompt: &str) -> bool {
    prompt
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .any(|token| {
            let normalized_token = token.to_ascii_lowercase();
            normalized_token.starts_with("shell")
                || normalized_token.starts_with("exec")
                || normalized_token.starts_with("command")
                || normalized_token.starts_with("terminal")
                || normalized_token.starts_with("process")
        })
}

fn detect_shell_execution_patterns(script_source: &str) -> Vec<&'static str> {
    let normalized_tokens = script_source
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();

    AI_SCRIPT_SHELL_EXECUTION_PATTERNS
        .iter()
        .filter_map(|(pattern_name, normalized_pattern)| {
            normalized_tokens
                .iter()
                .any(|token| token == normalized_pattern)
                .then_some(*pattern_name)
        })
        .collect()
}

fn detect_unexpected_shell_execution_patterns(
    prompt: &str,
    script_source: &str,
) -> Vec<&'static str> {
    if prompt_allows_shell_execution(prompt) {
        return Vec::new();
    }

    detect_shell_execution_patterns(script_source)
}

fn split_fence_header_and_body(fence: &str) -> (&str, &str) {
    match fence.find('\n') {
        Some(newline_index) => (&fence[..newline_index], &fence[newline_index + 1..]),
        None => ("", fence),
    }
}

fn extract_fenced_code(response: &str, preferred_languages: Option<&[&str]>) -> Option<String> {
    let mut remaining = response;

    while let Some(start) = remaining.find("```") {
        let after_start = &remaining[start + 3..];
        let Some(end) = after_start.find("```") else {
            break;
        };

        let fence_contents = &after_start[..end];
        let (header, body) = split_fence_header_and_body(fence_contents);
        let language = header
            .trim()
            .split(|c: char| c.is_whitespace() || c == '{')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let code = body.trim();

        if !code.is_empty() {
            match preferred_languages {
                Some(preferred) => {
                    if preferred.iter().any(|candidate| *candidate == language) {
                        return Some(code.to_string());
                    }
                }
                None => return Some(code.to_string()),
            }
        }

        remaining = &after_start[end + 3..];
    }

    None
}

fn extract_script_code(response: &str) -> String {
    const PREFERRED_LANGUAGES: [&str; 6] = ["typescript", "ts", "javascript", "js", "tsx", "jsx"];

    extract_fenced_code(response, Some(&PREFERRED_LANGUAGES))
        .or_else(|| extract_fenced_code(response, None))
        .unwrap_or_else(|| strip_leading_prose(response.trim()))
}

/// Strip leading non-TypeScript prose from an AI response that wasn't fenced.
/// Looks for the first line that starts with a valid TS/JS construct and drops everything before it.
fn strip_leading_prose(response: &str) -> String {
    let lines: Vec<&str> = response.lines().collect();

    // Find the first line that looks like TypeScript/JavaScript code
    let code_start = lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("//")
            || trimmed.starts_with("import ")
            || trimmed.starts_with("import{")
            || trimmed.starts_with("import\"")
            || trimmed.starts_with("import'")
            || trimmed.starts_with("export ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("let ")
            || trimmed.starts_with("var ")
            || trimmed.starts_with("async ")
            || trimmed.starts_with("await ")
            || trimmed.starts_with("function ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("interface ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("enum ")
    });

    match code_start {
        Some(0) => response.to_string(),
        Some(idx) => {
            let stripped = lines[idx..].join("\n");
            tracing::warn!(
                category = "AI",
                stripped_lines = idx,
                first_stripped_line = lines[0],
                "Stripped leading prose from AI script response"
            );
            stripped
        }
        None => response.to_string(),
    }
}

/// Extract the value from a `// Name: <value>` comment line in the script source.
fn extract_name_comment(script: &str) -> Option<String> {
    script
        .lines()
        .find_map(|line| line.trim().strip_prefix("// Name:"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// Extract the name from an `export const metadata = { name: "..." }` block.
fn extract_metadata_name(source: &str) -> Option<String> {
    extract_metadata_string_field(source, "name")
}

/// Return the source slice from `{` through its matching `}`.
fn extract_braced_region(source: &str) -> Option<&str> {
    if !source.starts_with('{') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (idx, ch) in source.char_indices() {
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_string = Some(ch),
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&source[..=idx]);
                }
            }
            _ => {}
        }
    }

    None
}

fn slugify_script_name(prompt: &str) -> String {
    let mut slug = String::new();
    let mut last_was_hyphen = false;

    for character in prompt.to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_hyphen = false;
        } else if matches!(character, ' ' | '_' | '-') && !slug.is_empty() && !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.len() > AI_SCRIPT_MAX_SLUG_LEN {
        slug.truncate(AI_SCRIPT_MAX_SLUG_LEN);
        while slug.ends_with('-') {
            slug.pop();
        }
    }

    if slug.is_empty() {
        AI_SCRIPT_DEFAULT_SLUG.to_string()
    } else {
        slug
    }
}

fn slug_to_title(slug: &str) -> String {
    slug.split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .join(" ")
}

fn description_from_prompt(prompt: &str) -> String {
    let normalized = prompt.split_whitespace().join(" ");
    if normalized.is_empty() {
        return "AI-generated Script Kit script".to_string();
    }

    let mut shortened = normalized;
    if shortened.chars().count() > 110 {
        shortened = format!("{}...", shortened.chars().take(107).collect::<String>());
    }
    shortened
}

fn quote_typescript_string(value: &str) -> String {
    let escaped = value
        .chars()
        .flat_map(char::escape_default)
        .collect::<String>();
    format!("\"{escaped}\"")
}

fn has_kit_import(script: &str) -> bool {
    script.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("import")
            && trimmed.contains(SCRIPT_KIT_SDK_IMPORT_MODULE)
            && (trimmed.contains('\"') || trimmed.contains('\''))
    })
}

fn has_description_comment(script: &str) -> bool {
    script
        .lines()
        .any(|line| line.trim_start().starts_with("// Description:"))
}

fn has_name_contract(script: &str) -> bool {
    extract_name_comment(script).is_some() || extract_metadata_name(script).is_some()
}

fn has_description_contract(script: &str) -> bool {
    has_description_comment(script) || extract_metadata_description(script).is_some()
}

fn has_current_app_recipe_header(script: &str) -> bool {
    script
        .lines()
        .any(|line| line.trim_start().starts_with("// Current-App-Recipe-"))
}

fn current_app_recipe_header_at_top(script: &str) -> bool {
    match script.lines().find(|line| !line.trim().is_empty()) {
        Some(line) => line.trim_start().starts_with("// Current-App-Recipe-"),
        None => false,
    }
}

fn extract_metadata_description(source: &str) -> Option<String> {
    extract_metadata_string_field(source, "description")
}

fn extract_metadata_string_field(source: &str, field_name: &str) -> Option<String> {
    let metadata_start = source.find("export const metadata")?;
    let metadata_region = &source[metadata_start..];
    let object_start = metadata_region.find('{')?;
    let object_body = extract_braced_region(&metadata_region[object_start..])?;

    let needle = format!("{field_name}:");
    let field_start = object_body.find(&needle)?;
    let value = object_body[field_start + needle.len()..].trim_start();

    let quote = match value.chars().next() {
        Some('"') => '"',
        Some('\'') => '\'',
        _ => return None,
    };

    let closing_index = value[1..].find(quote)?;
    let field_value = value[1..1 + closing_index].trim();

    if field_value.is_empty() {
        None
    } else {
        Some(field_value.to_string())
    }
}

fn detect_metadata_style(script: &str) -> GeneratedScriptMetadataStyle {
    let has_comment_headers =
        extract_name_comment(script).is_some() || has_description_comment(script);
    let has_metadata_export = script.contains("export const metadata");

    match (has_comment_headers, has_metadata_export) {
        (true, false) => GeneratedScriptMetadataStyle::CommentHeaders,
        (false, true) => GeneratedScriptMetadataStyle::MetadataExport,
        (true, true) => GeneratedScriptMetadataStyle::Hybrid,
        (false, false) => GeneratedScriptMetadataStyle::Missing,
    }
}

fn audit_generated_script_contract(script: &str) -> GeneratedScriptContractAudit {
    let has_name = has_name_contract(script);
    let has_description = has_description_contract(script);
    let has_kit_import = has_kit_import(script);
    let recipe_header = has_current_app_recipe_header(script);
    let recipe_at_top = !recipe_header || current_app_recipe_header_at_top(script);

    let metadata_style = detect_metadata_style(script);
    let mut warnings = Vec::new();
    let parsed_metadata = crate::metadata_parser::extract_typed_metadata(script);
    let declared_capabilities = parsed_metadata
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.extra.get("sdkCapabilities"))
        .and_then(serde_json::Value::as_array)
        .map(|capabilities| {
            capabilities
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let execution_topology = parsed_metadata
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.extra.get("executionTopology"))
        .and_then(|topology| serde_json::from_value(topology.clone()).ok());
    let capability_issues = parsed_metadata
        .metadata
        .as_ref()
        .map(|metadata| {
            let name = metadata
                .name
                .clone()
                .unwrap_or_else(|| "AI-generated script".to_string());
            let validation_subject = crate::scripts::Script {
                path: PathBuf::from(format!("{}.ts", slugify_script_name(&name))),
                extension: "ts".to_string(),
                plugin_id: "main".to_string(),
                typed_metadata: Some(metadata.clone()),
                name,
                ..crate::scripts::Script::default()
            };
            crate::scripts::validate_declared_sdk_capabilities(&validation_subject)
        })
        .unwrap_or_default();

    if !has_name {
        warnings.push("missing_name_contract".to_string());
    }
    if !has_description {
        warnings.push("missing_description_contract".to_string());
    }
    if !has_kit_import {
        warnings.push("missing_scriptkit_import".to_string());
    }
    if matches!(metadata_style, GeneratedScriptMetadataStyle::Hybrid) {
        warnings.push("mixed_metadata_formats".to_string());
    }
    if recipe_header && !recipe_at_top {
        warnings.push("current_app_recipe_header_not_at_top".to_string());
    }
    if has_concurrent_prompt_api_usage(script) {
        warnings.push("concurrent_prompt_apis".to_string());
    }

    GeneratedScriptContractAudit {
        metadata_style,
        has_name,
        has_description,
        has_kit_import,
        has_current_app_recipe_header: recipe_header,
        current_app_recipe_header_at_top: recipe_at_top,
        declared_capabilities,
        execution_topology,
        metadata_parse_errors: parsed_metadata.errors,
        capability_issues,
        warnings,
    }
}

fn has_concurrent_prompt_api_usage(script: &str) -> bool {
    const PROMISE_COMBINATORS: [&str; 4] = [
        "promise.all",
        "promise.race",
        "promise.any",
        "promise.allsettled",
    ];
    const PROMPT_APIS: [&str; 11] = [
        "arg(",
        "fields(",
        "editor(",
        "div(",
        "form(",
        "drop(",
        "find(",
        "path(",
        "textarea(",
        "select(",
        "grid(",
    ];

    let normalized = script
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();

    PROMISE_COMBINATORS.iter().any(|combinator| {
        let mut search_start = 0usize;

        while let Some(relative_index) = normalized[search_start..].find(combinator) {
            let start = search_start + relative_index;
            let remainder = &normalized[start..];
            let end = remainder
                .find(';')
                .map(|offset| start + offset)
                .unwrap_or(normalized.len());
            let window = &normalized[start..end];

            if PROMPT_APIS
                .iter()
                .any(|prompt_api| window.contains(prompt_api))
            {
                return true;
            }

            search_start = start + combinator.len();
        }

        false
    })
}

/// Drop machine-only recipe headers from generated scripts so the final file is
/// ordinary shareable code instead of a container for generation metadata.
fn strip_reserved_header_prefix(script: &str) -> String {
    let mut body_lines = Vec::new();
    let mut saw_recipe_header = false;
    let mut collecting_prefix = true;

    for line in script.lines() {
        let trimmed = line.trim_start();

        if collecting_prefix && trimmed.starts_with("// Current-App-Recipe-") {
            saw_recipe_header = true;
            continue;
        }

        if collecting_prefix && saw_recipe_header && trimmed.is_empty() {
            continue;
        }

        collecting_prefix = false;
        body_lines.push(line.to_string());
    }

    if saw_recipe_header {
        body_lines.join("\n").trim().to_string()
    } else {
        script.trim().to_string()
    }
}

fn enforce_script_kit_conventions(script: &str, prompt: &str, slug: &str) -> String {
    let body = strip_reserved_header_prefix(script);

    let mut prefix_lines: Vec<String> = Vec::new();

    // When neither comment header nor metadata export provides name/description,
    // inject a proper export const metadata block (canonical format).
    let missing_name = !has_name_contract(&body);
    let missing_description = !has_description_contract(&body);

    if missing_name || missing_description {
        // If there's already a metadata export, we only inject comment-header fallbacks
        // for backwards compat. Otherwise, inject the canonical metadata export block.
        let has_existing_metadata = body.contains("export const metadata");

        if has_existing_metadata {
            // Existing metadata export is missing one field — inject comment fallback
            if missing_name {
                prefix_lines.push(format!("// Name: {}", slug_to_title(slug)));
            }
            if missing_description {
                prefix_lines.push(format!(
                    "// Description: {}",
                    description_from_prompt(prompt)
                ));
            }
        } else if extract_name_comment(&body).is_some() || has_description_comment(&body) {
            // Legacy comment headers already present — fill in missing fields as comments
            if missing_name {
                prefix_lines.push(format!("// Name: {}", slug_to_title(slug)));
            }
            if missing_description {
                prefix_lines.push(format!(
                    "// Description: {}",
                    description_from_prompt(prompt)
                ));
            }
        } else {
            // No metadata at all — inject the canonical export const metadata block
            let name = slug_to_title(slug);
            let desc = description_from_prompt(prompt);
            prefix_lines.push(format!(
                "export const metadata = {{\n  name: {},\n  description: {},\n}};",
                quote_typescript_string(&name),
                quote_typescript_string(&desc)
            ));
        }
    }

    // Strip SDK import lines — the preload (--preload kit-sdk.ts) provides all
    // globals, so import "@scriptkit/sdk" and import "@johnlindquist/kit" are
    // dead code that crash because neither package is resolvable from temp dirs.
    let body = body
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !(trimmed.starts_with("import")
                && (trimmed.contains("@scriptkit/sdk") || trimmed.contains("@johnlindquist/kit")))
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut sections = Vec::new();

    if !prefix_lines.is_empty() {
        sections.push(prefix_lines.join("\n"));
    }

    sections.push(body.trim().to_string());

    let mut output = sections
        .into_iter()
        .filter(|section| !section.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn generated_receipt_fixture(path: &Path, prompt: &str) -> GeneratedScriptReceipt {
        GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: safe_generated_script_detail(prompt),
            slug: "fixture".to_string(),
            slug_source: "Fixture".to_string(),
            slug_source_kind: "metadata_export".to_string(),
            model_id: "test-model".to_string(),
            provider_id: "test-provider".to_string(),
            script_path: path.with_extension("ts").display().to_string(),
            receipt_path: path.display().to_string(),
            shell_execution_warning: false,
            contract: GeneratedScriptContractAudit {
                metadata_style: GeneratedScriptMetadataStyle::MetadataExport,
                has_name: true,
                has_description: true,
                has_kit_import: false,
                has_current_app_recipe_header: false,
                current_app_recipe_header_at_top: true,
                declared_capabilities: vec![],
                execution_topology: None,
                metadata_parse_errors: vec![],
                capability_issues: vec![],
                warnings: vec![],
            },
            verification: GeneratedScriptVerificationReceipt::skipped("safe_test_fixture"),
            current_app_recipe: None,
        }
    }

    #[test]
    fn test_slugify_script_name_handles_spaces_and_symbols() {
        assert_eq!(
            slugify_script_name("Build: API Client!"),
            "build-api-client"
        );
        assert_eq!(slugify_script_name("  ___  "), "ai-script");
    }

    #[test]
    fn generated_persistence_plan_applies_identical_shell_policy_to_both_save_paths() {
        let source = r#"import { execSync } from "child_process";
const output = execSync("printenv");"#;
        let provider_save = generated_script_persistence_plan(
            "Show deployment status",
            source,
            "deployment-status",
            None,
        )
        .expect("provider-result save has a valid derived name");
        let direct_save = generated_script_persistence_plan(
            "Show deployment status",
            source,
            "deployment-status",
            Some("../../Private API_TOKEN"),
        )
        .expect("Agent Chat save sanitizes its override before any filesystem work");

        assert!(provider_save.shell_execution_warning);
        assert!(direct_save.shell_execution_warning);
        assert_eq!(
            provider_save.suspicious_shell_patterns,
            direct_save.suspicious_shell_patterns
        );
        assert_eq!(direct_save.requested_slug, "private-api-token");
        assert!(!direct_save.requested_slug.contains(".."));
        assert!(!direct_save.requested_slug.contains('/'));

        let explicitly_approved = generated_script_persistence_plan(
            "Run a shell command in the terminal",
            source,
            "approved-shell",
            None,
        )
        .expect("an explicitly approved shell request has a valid plan");
        assert!(!explicitly_approved.shell_execution_warning);
    }

    #[test]
    fn generated_persistence_plan_rejects_empty_hostile_override_without_leaking_it() {
        let raw_override = "../../🔥";
        let error = generated_script_persistence_plan(
            "Summarize this note",
            "await div(\"safe\");",
            "safe-default",
            Some(raw_override),
        )
        .expect_err("a hostile override that sanitizes to empty must fail before creation");
        let safe_error = error.to_string();
        assert!(safe_error.contains("empty after safe sanitization"));
        assert!(!safe_error.contains(raw_override));
        assert!(!safe_error.contains(".."));
    }

    #[test]
    fn generated_persistence_identity_uses_actual_collision_resolved_safe_stem() {
        let path = Path::new("/synthetic/plugins/main/scripts/safe-script-7.ts");
        let slug = generated_script_created_slug(path)
            .expect("the collision-resolved filename is the only receipt identity");
        assert_eq!(slug, "safe-script-7");

        let unsafe_path = Path::new("/synthetic/plugins/main/scripts/Unsafe Script.ts");
        let error = generated_script_created_slug(unsafe_path)
            .expect_err("an unsanitized external path must never enter a script receipt");
        assert!(error.to_string().contains("safely sanitized"));
    }

    #[test]
    fn generated_receipt_atomic_rewrites_preserve_safe_content_without_temp_artifacts() {
        let temp = tempdir().expect("create isolated receipt workspace");
        let destination = temp.path().join("fixture.scriptkit.json");
        let mut receipt = generated_receipt_fixture(
            &destination,
            "Summarize this article token=sk-private-token Authorization: Bearer sk-private-bearer api_key=sk-private-api",
        );
        write_generated_script_receipt(&destination, &receipt)
            .expect("publish initial receipt atomically");
        receipt.model_id = "updated-model".to_string();
        write_generated_script_receipt(&destination, &receipt)
            .expect("replace existing regular receipt atomically");

        let persisted = fs::read_to_string(&destination).expect("read final receipt");
        let saved: GeneratedScriptReceipt =
            serde_json::from_str(&persisted).expect("parse final receipt");
        assert_eq!(saved.model_id, "updated-model");
        assert!(saved.prompt.contains("Summarize this article"));
        assert!(saved.prompt.contains("[REDACTED]"));
        assert!(!persisted.contains("sk-private-token"));
        assert!(!persisted.contains("sk-private-bearer"));
        assert!(!persisted.contains("sk-private-api"));
        let entries = fs::read_dir(temp.path())
            .expect("inspect isolated receipt workspace")
            .count();
        assert_eq!(
            entries, 1,
            "temporary sibling receipts must always be cleaned"
        );
    }

    #[cfg(unix)]
    #[test]
    fn generated_receipt_refuses_symlink_destination_without_touching_target() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("create isolated receipt workspace");
        let protected = temp.path().join("protected.txt");
        fs::write(&protected, "protected-content").expect("seed protected fixture");
        let destination = temp.path().join("fixture.scriptkit.json");
        symlink(&protected, &destination).expect("install isolated hostile receipt symlink");
        let receipt = generated_receipt_fixture(&destination, "Summarize safely");

        let error = write_generated_script_receipt(&destination, &receipt)
            .expect_err("receipt publishing must never follow an existing symlink");
        assert!(error.to_string().contains("symbolic-link"));
        assert_eq!(
            fs::read_to_string(&protected).expect("read protected fixture"),
            "protected-content"
        );
    }

    #[test]
    fn generated_prompt_and_verifier_output_redact_secrets_without_losing_safe_copy() {
        let prompt = "Summarize deployment token=sk-private-do-not-save for the team";
        let safe_prompt = safe_generated_script_detail(prompt);
        assert!(safe_prompt.contains("Summarize deployment"));
        assert!(safe_prompt.contains("[REDACTED]"));
        assert!(!safe_prompt.contains("sk-private-do-not-save"));

        let stderr = b"Build failed: api_key=sk-never-persist-this at /Users/alice/private.ts";
        let excerpt = truncate_verification_output(stderr).expect("safe diagnostics remain useful");
        assert!(excerpt.contains("Build failed"));
        assert!(!excerpt.contains("sk-never-persist-this"));
        assert!(!excerpt.contains("/Users/alice"));
    }

    #[test]
    fn generated_author_prompt_distinguishes_interactive_and_legacy_scriptlets() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("launcher-opened TypeScript scriptlets have interactive SDK transport"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("Only legacy synchronous TypeScript scriptlets lack interactive stdin"));
    }

    #[test]
    fn test_build_script_generation_messages_wraps_prompt_with_request_delimiters() {
        let messages = build_script_generation_messages("show today's weather");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "user");
        assert!(messages[1]
            .content
            .contains(AI_SCRIPT_USER_REQUEST_START_DELIMITER));
        assert!(messages[1]
            .content
            .contains(AI_SCRIPT_USER_REQUEST_END_DELIMITER));
        assert!(messages[1]
            .content
            .contains("---USER_REQUEST---\nshow today's weather\n---END_REQUEST---"));
    }

    #[test]
    fn test_detect_unexpected_shell_execution_patterns_returns_patterns_when_prompt_disallows_shell(
    ) {
        let prompt = "Show CPU usage in a rich UI";
        let script_source = r#"
import { execSync } from "child_process";
await div(execSync("top -l 1").toString());
"#;

        let patterns = detect_unexpected_shell_execution_patterns(prompt, script_source);
        assert_eq!(patterns, vec!["child_process", "execSync"]);
    }

    #[test]
    fn test_detect_unexpected_shell_execution_patterns_returns_empty_when_prompt_allows_shell() {
        let prompt = "Run a shell command in the terminal and show output";
        let script_source = r#"
import { execSync } from "child_process";
await div(execSync("pwd").toString());
"#;

        let patterns = detect_unexpected_shell_execution_patterns(prompt, script_source);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_extract_script_code_prefers_typescript_fence_when_multiple_blocks_exist() {
        let response = r#"
Here's one idea:
```markdown
Not code
```
```typescript
await div("hello");
```
"#;
        assert_eq!(extract_script_code(response), "await div(\"hello\");");
    }

    #[test]
    fn test_extract_script_code_falls_back_to_first_fenced_block() {
        let response = r#"
```python
print("hello")
```
"#;
        assert_eq!(extract_script_code(response), "print(\"hello\")");
    }

    #[test]
    fn test_extract_script_code_returns_trimmed_response_when_no_fence_exists() {
        let response = "const answer = 42;";
        assert_eq!(extract_script_code(response), "const answer = 42;");
    }

    #[test]
    fn test_enforce_script_kit_conventions_adds_missing_metadata_as_export() {
        let script = "const name = await arg(\"Name?\");";
        let output = enforce_script_kit_conventions(script, "Ask for user name", "ask-user-name");

        assert!(
            output.contains("export const metadata = {"),
            "should inject export const metadata when no metadata exists"
        );
        assert!(
            output.contains("name: \"Ask User Name\""),
            "metadata should contain name"
        );
        assert!(
            output.contains("description: \"Ask for user name\""),
            "metadata should contain description"
        );
        assert!(
            !output.contains("// Name:"),
            "should not inject legacy comment-header metadata"
        );
        assert!(
            !output.contains("// Description:"),
            "should not inject legacy comment-header metadata"
        );
        assert!(
            !output.contains("import \"@scriptkit/sdk\";"),
            "SDK import should be stripped (preload provides globals)"
        );
        assert!(output.contains("await arg(\"Name?\");"));
    }

    #[test]
    fn test_enforce_script_kit_conventions_keeps_existing_metadata_and_import() {
        let script = r#"// Name: Existing
// Description: Existing description
import "@scriptkit/sdk";

await div("ready");
"#;
        let output = enforce_script_kit_conventions(script, "ignored", "ignored");

        assert_eq!(
            output.matches("// Name:").count(),
            1,
            "should not duplicate existing Name metadata"
        );
        assert_eq!(
            output.matches("// Description:").count(),
            1,
            "should not duplicate existing Description metadata"
        );
        assert_eq!(
            output.matches("import \"@scriptkit/sdk\";").count(),
            0,
            "SDK import should be stripped (preload provides globals)"
        );
    }

    #[test]
    fn test_has_kit_import_accepts_scriptkit_sdk_and_rejects_legacy_kit_import() {
        assert!(has_kit_import("import \"@scriptkit/sdk\";"));
        assert!(has_kit_import("import '@scriptkit/sdk'"));
        assert!(!has_kit_import("import \"@johnlindquist/kit\";"));
    }

    #[test]
    fn test_enforce_conventions_strips_legacy_johnlindquist_kit_import() {
        let script = "// Name: Focus Notion\n// Description: Bring Notion to the front\n\nimport \"@johnlindquist/kit\";\n\nawait $`open -a \"Notion\"`;";
        let output = enforce_script_kit_conventions(script, "focus on this app", "focus-notion");
        assert!(
            !output.contains("@johnlindquist/kit"),
            "should strip legacy @johnlindquist/kit import"
        );
        assert!(
            !output.contains("@scriptkit/sdk"),
            "should not add @scriptkit/sdk import (preload provides globals)"
        );
        assert!(
            output.contains("open -a"),
            "should preserve the actual script body"
        );
    }

    #[test]
    fn ai_script_generation_system_prompt_is_not_accidentally_truncated() {
        assert!(
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.len() > 100,
            "AI_SCRIPT_GENERATION_SYSTEM_PROMPT looks truncated (len={})",
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.len()
        );
    }

    #[test]
    fn ai_script_generation_system_prompt_keeps_typescript_only_contract() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(
            prompt.contains("production-ready Script Kit TypeScript scripts"),
            "system prompt must keep the Script Kit TypeScript framing"
        );
        assert!(
            prompt.contains("ONLY TypeScript code"),
            "system prompt must explicitly forbid extra commentary"
        );
        assert!(
            prompt
                .to_ascii_lowercase()
                .contains("typescript source code"),
            "system prompt must explicitly require TypeScript source output"
        );
        // System prompt must NOT bless legacy comment-header metadata
        assert!(
            !prompt.contains("Format A (comment headers)"),
            "system prompt must not document legacy comment-header format"
        );
        assert!(
            prompt.contains("Do NOT use legacy comment-header metadata"),
            "system prompt must explicitly forbid legacy comment headers"
        );
    }

    #[test]
    fn test_ai_script_generation_system_prompt_uses_modern_sdk_conventions() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("import \"@scriptkit/sdk\";"));
        assert!(!AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("@johnlindquist/kit"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("arg("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("div("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("editor("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("notify("));
        // New prompt includes examples and comprehensive API reference
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("TEACH BY EXAMPLE"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("COMPACT API REFERENCE"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("aiStartChat("));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("clipboard"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("home("));
        // All examples must use export const metadata, not comment headers
        assert!(
            AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("export const metadata"),
            "system prompt examples must use export const metadata"
        );
    }

    #[test]
    fn ai_script_generation_reference_preserves_actual_workspace_and_api_contracts() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(prompt.contains("skPath(\"plugins\", \"main\", ...)"));
        assert!(!prompt.contains("home(\".scriptkit\", \"kit\", \"main\""));
        assert!(prompt.contains("typed dispatch receipt; OS delivery is not guaranteed"));
        assert!(!prompt.contains("typed delivery result"));
        assert!(prompt.contains("* setActions(actions)"));
        assert!(!prompt.contains("* setActions(actions, options?)"));
    }

    #[test]
    fn test_ai_script_generation_system_prompt_defaults_to_main_menu_sized_flows() {
        let prompt = AI_SCRIPT_GENERATION_SYSTEM_PROMPT;

        assert!(
            prompt.contains("Default to the shared main-menu-sized prompt flow"),
            "system prompt should bias generated commands toward main-menu-sized flows"
        );
        assert!(
            prompt.contains("Expanded split-view browsers are rare exceptions"),
            "system prompt should make expanded split-view browsers opt-in exceptions"
        );
        assert!(
            prompt.contains("Do not use choice `preview` fields, `setPreview()`, or `setPanel()`"),
            "system prompt should steer models away from dense preview APIs by default"
        );
    }

    #[test]
    fn generated_system_message_uses_the_live_host_unsupported_capability_inventory() {
        let messages = build_script_generation_messages("Write a note");
        let system = &messages[0];

        assert_eq!(system.role, "system");
        assert!(system.content.contains("HOST SDK CAPABILITY CONTRACT"));
        for unsupported in crate::mcp_resources::unsupported_sdk_capability_names() {
            assert!(
                system.content.contains(unsupported),
                "system prompt must reject the host-owned unsupported capability {unsupported}"
            );
        }
        assert!(!system.content.contains("* find(...) — file search prompt"));
        assert!(!system
            .content
            .contains("* keyboard.type(...textOrKeys) — type (use with caution)"));
        assert!(!system
            .content
            .contains("* mouse.move(points) — move mouse (use with caution)"));
        assert!(system.content.contains("kit://command-doctor"));
        assert!(system.content.contains("kit://failed-scripts"));
        assert!(system.content.contains("never callable SDK functions"));
        assert!(system.content.contains("do not invent `commandDoctor()`"));
    }

    #[test]
    fn script_generation_examples_use_real_host_globals_and_no_imaginary_helpers() {
        use crate::mcp_resources::{diagnose_sdk_capability, SdkExecutionTopology};

        for capability in [
            "arg",
            "select",
            "fields",
            "editor",
            "div",
            "form",
            "drop",
            "path",
            "mini",
            "micro",
            "hotkey",
            "confirm",
            "md",
            "hud",
            "notify",
            "home",
            "skPath",
            "kitPath",
            "tmpPath",
            "readFile",
            "writeFile",
            "fileSearch",
            "exec",
            "browse",
            "editFile",
            "clipboard.readText",
            "clipboard.writeText",
            "clipboardHistory",
            "aiStartChat",
            "aiSendMessage",
            "mcp.discover",
        ] {
            assert!(
                diagnose_sdk_capability(capability, SdkExecutionTopology::TypeScriptScript)
                    .is_none(),
                "AI script guidance advertises unsupported SDK capability {capability}"
            );
        }

        for imaginary in [
            "* grid(",
            "* textarea(",
            "* onTab(",
            "* toast(",
            "* openActions(",
            "* setHint(",
            "* setLoading(",
            "* setProgress(",
            "* db(",
            "* store(",
            "* ensureDir(",
            "* globby(",
            "* getClipboardHistory(",
            "* removeClipboardItem(",
            "* clearClipboardHistory(",
            "* ai(",
            "* assistant(",
            "* generate(",
            "* mcp(options?)",
            "path.join/etc",
            "exec(command, options?)",
            "`${cmd}",
        ] {
            assert!(
                !AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains(imaginary),
                "AI script guidance advertises imaginary SDK helper {imaginary}"
            );
        }
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("exec(\"open\", [filePath])"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("noResponse: true"));
    }

    #[test]
    fn test_prepare_script_from_ai_response_adds_conventions_when_ai_omits_them() {
        let prompt = "Create a weather checker";
        let response = "await div(\"Sunny\");";

        let (slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(slug, "create-a-weather-checker");
        assert!(
            source.contains("export const metadata = {"),
            "should inject export const metadata"
        );
        assert!(
            source.contains("name: \"Create A Weather Checker\""),
            "metadata should contain name"
        );
        assert!(
            source.contains("description: \"Create a weather checker\""),
            "metadata should contain description"
        );
        assert!(
            !source.contains("// Name:"),
            "should not use legacy comment-header metadata"
        );
        assert!(
            !source.contains("import \"@scriptkit/sdk\";"),
            "SDK import should be stripped (preload provides globals)"
        );
        assert!(source.contains("await div(\"Sunny\");"));
    }

    #[test]
    fn test_enforce_script_kit_conventions_escapes_metadata_string_literals() {
        let script = "await div(\"ok\");";
        let output = enforce_script_kit_conventions(
            script,
            "Say \"hello\" from C:\\Temp\nnow",
            "say-\"hello\"-tool",
        );

        assert!(
            output.contains("name: \"Say \\\"hello\\\" Tool\""),
            "metadata name should escape double quotes"
        );
        assert!(
            output.contains("description: \"Say \\\"hello\\\" from C:\\\\Temp now\""),
            "metadata description should escape quotes and backslashes"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_uses_name_comment_for_slug() {
        let prompt =
            "Generate a Script Kit script that automates what I am doing in the current app";
        let response = r#"// Name: cmux Quick Actions
// Description: Quick action palette for cmux
import "@scriptkit/sdk";

await arg("Pick an action");
"#;
        let (slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "cmux-quick-actions",
            "slug should come from // Name:, not the prompt"
        );
        assert!(source.contains("// Name: cmux Quick Actions"));
    }

    #[test]
    fn test_extract_name_comment_finds_name_line() {
        assert_eq!(
            extract_name_comment("// Name: My Cool Script\nimport \"@scriptkit/sdk\";"),
            Some("My Cool Script".to_string())
        );
        assert_eq!(
            extract_name_comment("import \"@scriptkit/sdk\";\nawait arg(\"hi\");"),
            None
        );
        assert_eq!(
            extract_name_comment("// Name: \nimport \"@scriptkit/sdk\";"),
            None,
            "empty name should return None"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_extracts_typescript_fence_when_present() {
        let prompt = "Build script";
        let response = r#"
```typescript
await arg("Name?");
```
"#;

        let (_slug, source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert!(source.contains("await arg(\"Name?\");"));
        assert!(!source.contains("```"));
    }

    #[test]
    fn test_strip_leading_prose_removes_markdown_preamble() {
        let response = r#"**Assumed:** You're using cmux and want a quick-action palette.

**Required permissions:** Accessibility access for Script Kit.

// Name: cmux Quick Actions
// Description: Quick action palette for cmux
import "@scriptkit/sdk";

await arg("Pick an action");
"#;
        let stripped = strip_leading_prose(response.trim());
        assert!(
            stripped.starts_with("// Name:"),
            "Should start with // Name:, got: {}",
            &stripped[..stripped.len().min(50)]
        );
        assert!(!stripped.contains("**Assumed:**"));
        assert!(stripped.contains("await arg"));
    }

    #[test]
    fn test_strip_leading_prose_preserves_clean_response() {
        let response = r#"// Name: My Script
// Description: Does something
import "@scriptkit/sdk";

await div("hello");
"#;
        let stripped = strip_leading_prose(response.trim());
        assert!(stripped.starts_with("// Name:"));
        assert!(stripped.contains("await div"));
    }

    #[test]
    fn test_extract_script_code_strips_prose_when_no_fence() {
        let response = r#"Here's a script for you:

// Name: Test
import "@scriptkit/sdk";

await arg("hello");
"#;
        let extracted = extract_script_code(response);
        assert!(
            extracted.starts_with("// Name:"),
            "Should start with code, got: {}",
            &extracted[..extracted.len().min(50)]
        );
        assert!(!extracted.contains("Here's a script"));
    }

    #[test]
    fn test_extract_metadata_name_finds_double_quoted_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "My Cool Script",
  description: "Does cool things",
};
await arg("hello");
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("My Cool Script".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_finds_single_quoted_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = {
  name: 'Single Quoted',
  description: 'desc',
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Single Quoted".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_returns_none_when_no_metadata() {
        let source = r#"import "@scriptkit/sdk";
await arg("hello");
"#;
        assert_eq!(extract_metadata_name(source), None);
    }

    #[test]
    fn test_extract_metadata_name_returns_none_for_empty_name() {
        let source = r#"export const metadata = {
  name: "",
  description: "desc",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            None,
            "empty name should return None"
        );
    }

    #[test]
    fn test_extract_metadata_name_ignores_name_outside_metadata_block() {
        let source = r#"const config = { name: "Not This" };
export const metadata = {
  name: "Correct Name",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Correct Name".to_string()),
            "should find name only after export const metadata"
        );
    }

    #[test]
    fn test_extract_metadata_name_finds_inline_metadata_export_name() {
        let source = r#"import "@scriptkit/sdk";
export const metadata = { name: "Inline Name", description: "desc" };
await arg("hello");
"#;
        assert_eq!(
            extract_metadata_name(source),
            Some("Inline Name".to_string())
        );
    }

    #[test]
    fn test_extract_metadata_name_does_not_leak_past_metadata_block() {
        let source = r#"export const metadata = {
  description: "desc",
};
const other = {
  name: "Wrong Name",
};
"#;
        assert_eq!(
            extract_metadata_name(source),
            None,
            "name outside metadata block should not be used"
        );
    }

    #[test]
    fn test_prepare_script_from_ai_response_uses_metadata_name_for_slug() {
        let prompt = "Generate a Script Kit script for the current app";
        let response = r#"import "@scriptkit/sdk";

export const metadata = {
  name: "App Automator",
  description: "Automates the current app",
};

await arg("Pick an action");
"#;
        let (slug, _source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "app-automator",
            "slug should come from metadata export name, not the prompt"
        );
    }

    #[test]
    fn test_prepare_script_comment_header_takes_priority_over_metadata_export_for_slug() {
        // When both comment headers and metadata export are present (hybrid),
        // the comment header name still wins for slug resolution (backwards compat).
        let prompt = "do something";
        let response = r#"// Name: Comment Winner
// Description: from comment
import "@scriptkit/sdk";

export const metadata = {
  name: "Metadata Loser",
  description: "from metadata",
};

await arg("hi");
"#;
        let (slug, _source) = prepare_script_from_ai_response(prompt, response).unwrap();
        assert_eq!(
            slug, "comment-winner",
            "comment header should take priority over metadata export for slug"
        );
    }

    // --- Contract-aware finalization tests ---

    #[test]
    fn metadata_export_prevents_comment_header_injection() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Save Selection",
  description: "Save the current selection",
};

await div("ok");
"#;

        let output = enforce_script_kit_conventions(input, "save selection", "save-selection");

        assert!(
            !output.contains("// Name: Save Selection"),
            "should not inject // Name: when metadata export has name"
        );
        assert!(
            !output.contains("// Description: Save the current selection"),
            "should not inject // Description: when metadata export has description"
        );
        assert!(output.contains("export const metadata = {"));
    }

    #[test]
    fn metadata_name_is_used_for_slug_source() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "My AI Tool",
  description: "Do something useful",
};

await div("ok");
"#;

        let (slug, _) = prepare_script_from_ai_response("fallback prompt", input).unwrap();
        assert_eq!(slug, "my-ai-tool");
    }

    #[test]
    fn incomplete_metadata_export_only_injects_missing_fields() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Only Name Present",
};

await div("ok");
"#;

        let output =
            enforce_script_kit_conventions(input, "fallback description", "only-name-present");

        assert!(
            !output.contains("// Name: Only Name Present"),
            "should not inject // Name: when metadata export has name"
        );
        assert!(
            output.contains("// Description: fallback description"),
            "should inject // Description: when metadata export lacks description"
        );
        assert!(matches!(
            audit_generated_script_contract(&output).metadata_style,
            GeneratedScriptMetadataStyle::Hybrid
        ));
    }

    #[test]
    fn current_app_recipe_headers_are_stripped_during_enforcement() {
        let input = r#"// Current-App-Recipe-Base64: abc123
// Current-App-Recipe-Name: Safari Save Selection

export const metadata = {
  name: "Safari Save Selection",
  description: "Save the current Safari selection",
};

await div("ok");
"#;

        let output = enforce_script_kit_conventions(
            input,
            "save the current Safari selection",
            "safari-save-selection",
        );

        assert!(!output.contains("Current-App-Recipe-"));
        let contract = audit_generated_script_contract(&output);
        assert!(!contract.has_current_app_recipe_header);
        assert!(contract.current_app_recipe_header_at_top);
    }

    #[test]
    fn audit_detects_missing_metadata() {
        let input = r#"import "@scriptkit/sdk";
await div("hello");
"#;
        let contract = audit_generated_script_contract(input);
        assert!(matches!(
            contract.metadata_style,
            GeneratedScriptMetadataStyle::Missing
        ));
        assert!(!contract.has_name);
        assert!(!contract.has_description);
        assert!(contract
            .warnings
            .contains(&"missing_name_contract".to_string()));
        assert!(contract
            .warnings
            .contains(&"missing_description_contract".to_string()));
    }

    #[test]
    fn audit_detects_hybrid_metadata() {
        let input = r#"// Name: Comment Name
import "@scriptkit/sdk";
export const metadata = {
  name: "Metadata Name",
  description: "desc",
};
"#;
        let contract = audit_generated_script_contract(input);
        assert!(matches!(
            contract.metadata_style,
            GeneratedScriptMetadataStyle::Hybrid
        ));
        assert!(contract
            .warnings
            .contains(&"mixed_metadata_formats".to_string()));
    }

    #[test]
    fn audit_detects_concurrent_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const urls = await Promise.all([
  arg("URL 1"),
  arg("URL 2"),
  arg("URL 3"),
]);

await div(JSON.stringify(urls));
"#;

        let contract = audit_generated_script_contract(input);
        assert!(contract
            .warnings
            .contains(&"concurrent_prompt_apis".to_string()));
    }

    #[test]
    fn audit_allows_sequential_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const url1 = await arg("URL 1");
const url2 = await arg("URL 2");
const url3 = await arg("URL 3");

await div(JSON.stringify([url1, url2, url3]));
"#;

        let contract = audit_generated_script_contract(input);
        assert!(!contract
            .warnings
            .contains(&"concurrent_prompt_apis".to_string()));
    }

    #[test]
    fn prepare_script_rejects_concurrent_prompt_apis() {
        let input = r#"import "@scriptkit/sdk";
export const metadata = {
  name: "Open 3 Windows",
  description: "Open 3 new browser windows to different URLs",
};

const urls = await Promise.all([
  arg("URL 1"),
  arg("URL 2"),
  arg("URL 3"),
]);

await div(JSON.stringify(urls));
"#;

        let error = prepare_script_from_ai_response_with_contract("open three windows", input)
            .expect_err("concurrent prompt APIs must be rejected");

        let message = format!("{error:#}");
        assert!(message.contains("concurrent_prompt_apis"));
    }

    #[test]
    fn generated_script_rejects_unsupported_capabilities_before_any_file_creation() {
        let input = r#"export const metadata = {
  name: "Unavailable Widget",
  description: "An unsupported native widget",
  sdkCapabilities: ["widget"],
  executionTopology: "typescript-script",
};
await widget("<p>Hello</p>");
"#;

        let error = save_generated_script_from_response("render a widget", input)
            .expect_err("unsupported generated commands must fail before create_new_script");
        let message = format!("{error:#}");
        assert!(message.contains("generated_script_capability_unavailable"));
        assert!(message.contains("widget"));
    }

    #[test]
    fn generated_script_rejects_malformed_capability_and_topology_declarations() {
        let malformed_capabilities = r#"export const metadata = {
  name: "Bad Capability Shape",
  description: "Invalid metadata",
  sdkCapabilities: "arg",
};
await arg("Name?");
"#;
        let capability_error =
            prepare_script_from_ai_response_with_contract("ask for a name", malformed_capabilities)
                .expect_err("a bare capability string is never a valid author declaration");
        assert!(format!("{capability_error:#}").contains("must be an array"));

        let malformed_topology = r#"export const metadata = {
  name: "Bad Topology",
  description: "Invalid transport",
  executionTopology: "ruby-scriptlet",
};
await arg("Name?");
"#;
        let topology_error =
            prepare_script_from_ai_response_with_contract("ask for a name", malformed_topology)
                .expect_err("topology must be checked even without sdkCapabilities");
        assert!(format!("{topology_error:#}").contains("executionTopology"));
    }

    #[test]
    fn generated_script_rejects_unparseable_typed_metadata_before_creation() {
        let malformed = r#"export const metadata = {
  name: "Broken Metadata",
  description: "Missing closing brace",
  sdkCapabilities: ["arg"];
await arg("Name?");
"#;
        let error = prepare_script_from_ai_response_with_contract("ask a question", malformed)
            .expect_err("invalid typed metadata must not produce a hidden generated script");
        assert!(format!("{error:#}").contains("generated_script_metadata_invalid"));
    }

    #[test]
    fn generated_script_audit_reports_supported_capabilities_and_topology() {
        let valid = r#"export const metadata = {
  name: "Supported Author Flow",
  description: "Prompt and format a response",
  sdkCapabilities: ["arg", "div", "md"],
  executionTopology: "typescript-script",
};
const answer = await arg("Question?");
await div(md(answer));
"#;

        let prepared = prepare_script_from_ai_response_with_contract("ask a question", valid)
            .expect("supported author declarations should remain saveable");
        assert_eq!(
            prepared.contract.declared_capabilities,
            ["arg", "div", "md"]
        );
        assert_eq!(
            prepared.contract.execution_topology,
            Some(crate::mcp_resources::SdkExecutionTopology::TypeScriptScript)
        );
        assert!(prepared.contract.metadata_parse_errors.is_empty());
        assert!(prepared.contract.capability_issues.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn generated_permission_pending_script_stays_recoverable_without_privacy_probe() {
        let input = r#"export const metadata = {
  name: "Move Existing Window",
  description: "Needs explicitly granted Accessibility",
  sdkCapabilities: ["moveWindow"],
  executionTopology: "typescript-script",
};
await moveWindow("example", { x: 0, y: 0 });
"#;

        let prepared = prepare_script_from_ai_response_with_contract("move a window", input)
            .expect("unknown permission inventory is a recoverable author warning");
        assert_eq!(prepared.contract.capability_issues.len(), 1);
        assert_eq!(
            prepared.contract.capability_issues[0].severity,
            crate::scripts::ValidationSeverity::Warning
        );
    }

    #[test]
    fn receipt_serde_roundtrip() {
        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "close duplicate tabs".to_string(),
            slug: "close-duplicate-tabs".to_string(),
            slug_source: "Close Duplicate Tabs".to_string(),
            slug_source_kind: "metadata_export".to_string(),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: GeneratedScriptContractAudit {
                metadata_style: GeneratedScriptMetadataStyle::MetadataExport,
                has_name: true,
                has_description: true,
                has_kit_import: true,
                has_current_app_recipe_header: false,
                current_app_recipe_header_at_top: true,
                declared_capabilities: vec![],
                execution_topology: None,
                metadata_parse_errors: vec![],
                capability_issues: vec![],
                warnings: vec![],
            },
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");
        assert_eq!(receipt, deserialized);
        assert!(json.contains("\"schemaVersion\": 2"));
        assert!(json.contains("\"metadataStyle\": \"metadataExport\""));
        assert!(json.contains("\"verification\""));
        assert!(json.contains("\"status\": \"skipped\""));
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn receipt_serde_defaults_missing_verification_to_skipped() {
        let json = r#"{
  "schemaVersion": 1,
  "prompt": "close duplicate tabs",
  "slug": "close-duplicate-tabs",
  "slugSource": "Close Duplicate Tabs",
  "slugSourceKind": "metadata_export",
  "modelId": "gpt-4",
  "providerId": "openai",
  "scriptPath": "/tmp/test.ts",
  "receiptPath": "/tmp/test.scriptkit.json",
  "shellExecutionWarning": false,
  "contract": {
    "metadataStyle": "metadataExport",
    "hasName": true,
    "hasDescription": true,
    "hasKitImport": true,
    "hasCurrentAppRecipeHeader": false,
    "currentAppRecipeHeaderAtTop": true
  }
}"#;

        let receipt: GeneratedScriptReceipt =
            serde_json::from_str(json).expect("deserialize legacy receipt");
        assert_eq!(
            receipt.verification.status,
            GeneratedScriptVerificationStatus::Skipped
        );
        assert!(receipt
            .verification
            .diagnostics
            .contains(&"legacy_receipt_missing_verification".to_string()));
    }

    #[test]
    fn recipe_header_with_metadata_export_gets_correct_slug_source() {
        let input = r#"// Current-App-Recipe-Base64: abc123
// Current-App-Recipe-Name: Safari Close Duplicate Tabs

export const metadata = {
  name: "Safari Close Duplicate Tabs",
  description: "Close duplicate tabs in the current Safari window",
};

await div("Ready");
"#;

        let prepared =
            prepare_script_from_ai_response_with_contract("close duplicate tabs in Safari", input)
                .unwrap();
        assert_eq!(prepared.slug_source_kind, "metadata_export");
        assert_eq!(prepared.slug, "safari-close-duplicate-tabs");
        assert!(!prepared.source.contains("Current-App-Recipe-"));
        assert!(!prepared.contract.has_current_app_recipe_header);
    }

    #[test]
    fn extract_metadata_description_finds_value() {
        let source = r#"export const metadata = {
  name: "Test",
  description: "A useful test script",
};
"#;
        assert_eq!(
            extract_metadata_description(source),
            Some("A useful test script".to_string())
        );
    }

    #[test]
    fn generated_script_receipt_path_replaces_extension() {
        let script_path = PathBuf::from("/tmp/my-script.ts");
        let receipt = generated_script_receipt_path(&script_path);
        assert_eq!(receipt, PathBuf::from("/tmp/my-script.scriptkit.json"));
    }

    #[test]
    fn generated_script_receipt_omits_current_app_recipe_payloads() {
        let script_source = format!(
            r#"export const metadata = {{
  name: "Safari Close Duplicate Tabs",
  description: "Close duplicate tabs in the current Safari window",
}};

import "@scriptkit/sdk";
await div("Ready");
"#
        );

        let prepared = prepare_script_from_ai_response_with_contract(
            "close duplicate tabs in Safari",
            &script_source,
        )
        .expect("should prepare script");

        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "close duplicate tabs in Safari".to_string(),
            slug: prepared.slug.clone(),
            slug_source: prepared.slug_source.clone(),
            slug_source_kind: prepared.slug_source_kind.to_string(),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: prepared.contract.clone(),
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");

        assert_eq!(receipt, deserialized);
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn generated_script_receipt_strips_invalid_current_app_recipe_header() {
        let script_with_bad_base64 = r#"// Current-App-Recipe-Base64: not-valid-base64!!!
// Current-App-Recipe-Name: Bad Recipe

export const metadata = {
  name: "Bad Recipe Test",
  description: "A script with invalid recipe header",
};

import "@scriptkit/sdk";
await div("ok");
"#;

        let extracted = extract_current_app_recipe_from_script(script_with_bad_base64);
        assert!(
            extracted.is_none(),
            "invalid base64 should return None, not error"
        );

        let prepared = prepare_script_from_ai_response_with_contract(
            "test with bad recipe",
            script_with_bad_base64,
        )
        .expect("should prepare script despite invalid recipe header");

        assert!(!prepared.source.contains("Current-App-Recipe-"));
        assert!(!prepared.contract.has_current_app_recipe_header);

        let receipt = GeneratedScriptReceipt {
            schema_version: AI_GENERATED_SCRIPT_RECEIPT_SCHEMA_VERSION,
            prompt: "test with bad recipe".to_string(),
            slug: prepared.slug.clone(),
            slug_source: prepared.slug_source.clone(),
            slug_source_kind: prepared.slug_source_kind.to_string(),
            model_id: "unknown".to_string(),
            provider_id: "unknown".to_string(),
            script_path: "/tmp/test.ts".to_string(),
            receipt_path: "/tmp/test.scriptkit.json".to_string(),
            shell_execution_warning: false,
            contract: prepared.contract.clone(),
            verification: GeneratedScriptVerificationReceipt::skipped("unit_test_fixture"),
            current_app_recipe: None,
        };

        let json = serde_json::to_string_pretty(&receipt).expect("serialize receipt");
        let deserialized: GeneratedScriptReceipt =
            serde_json::from_str(&json).expect("deserialize receipt");
        assert_eq!(receipt, deserialized);
        assert!(!json.contains("\"currentAppRecipe\""));
    }

    #[test]
    fn extract_current_app_recipe_returns_none_for_no_header() {
        let script = r#"import "@scriptkit/sdk";
// Name: Simple Script
// Description: No recipe here
await div("hello");
"#;
        assert!(extract_current_app_recipe_from_script(script).is_none());
    }

    #[test]
    fn extract_current_app_recipe_returns_none_for_empty_base64() {
        let script = r#"// Current-App-Recipe-Base64:
import "@scriptkit/sdk";
await div("hello");
"#;
        assert!(extract_current_app_recipe_from_script(script).is_none());
    }

    #[test]
    fn chaos_nn29_hostile_generated_artifact_stays_in_scratch_and_is_never_executed() {
        let temp = tempdir().expect("create NN29 scratch workspace");
        let kit_path = temp.path().join(".scriptkit");
        std::fs::create_dir_all(&kit_path).expect("create scratch SK_PATH");
        let original_sk_path = std::env::var_os("SK_PATH");
        // SAFETY: this focused test runs alone in the NN29 probe invocation and
        // restores the process environment before returning.
        unsafe { std::env::set_var("SK_PATH", &kit_path) };

        let execution_canary = temp.path().join("generated-script-executed");
        let canary_literal = serde_json::to_string(&execution_canary.display().to_string())
            .expect("encode canary path as a TypeScript string");
        let raw_response = format!(
            r#"```typescript
export const metadata = {{
  name: "../../../../tmp/NN29 👩🏽‍💻 مرحبا",
  description: "Hostile path and Unicode must remain inert",
}};

await Bun.write({canary_literal}, "NN29_EXECUTED");
await div("NN29_INERT_CANARY Z̴̙̓͗a̷̻͒l̵͎͋g̶̯͗o̴̰̕");
```"#
        );

        let script_path = save_generated_script_from_response(
            "Create an inert NN29 script with hostile metadata",
            &raw_response,
        )
        .expect("save generated script through the real contract pipeline");
        let scripts_dir = kit_path.join("plugins/main/scripts");
        let canonical_script = script_path
            .canonicalize()
            .expect("canonicalize generated script");
        let canonical_scripts_dir = scripts_dir
            .canonicalize()
            .expect("canonicalize scratch scripts directory");
        assert!(
            canonical_script.starts_with(&canonical_scripts_dir),
            "generated path escaped scratch scripts dir: {}",
            canonical_script.display()
        );
        assert_eq!(
            script_path.extension().and_then(|value| value.to_str()),
            Some("ts")
        );
        assert!(!script_path.to_string_lossy().contains(".."));

        let source = std::fs::read_to_string(&script_path).expect("read generated artifact");
        assert!(source.contains("NN29_INERT_CANARY"));
        assert!(source.contains("Hostile path and Unicode must remain inert"));
        assert!(
            !execution_canary.exists(),
            "generated script ran during save"
        );

        let receipt_path = generated_script_receipt_path(&script_path);
        let mut final_verification = None;
        for _ in 0..200 {
            if let Ok(body) = std::fs::read_to_string(&receipt_path) {
                if let Ok(receipt) = serde_json::from_str::<GeneratedScriptReceipt>(&body) {
                    if receipt.verification.status != GeneratedScriptVerificationStatus::Skipped {
                        final_verification = Some(receipt.verification.status);
                        break;
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(
            final_verification.is_some(),
            "bounded Bun-build verification never finalized"
        );
        assert!(
            !execution_canary.exists(),
            "generated script executed instead of being build-verified only"
        );

        match original_sk_path {
            Some(value) => unsafe { std::env::set_var("SK_PATH", value) },
            None => unsafe { std::env::remove_var("SK_PATH") },
        }
    }

    #[test]
    fn chaos_nn29_unavailable_generation_fails_before_artifact_work_with_context() {
        let registry = ProviderRegistry::new();
        let error = match select_generation_model(&registry) {
            Ok(_) => panic!("an empty provider registry must reject generation"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "No AI models available in provider registry"
        );
    }
}
