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
include!("script_generation_tests.rs");
