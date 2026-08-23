//! Tab AI harness configuration and context formatting.
//!
//! Defines the contract for connecting Tab AI to an external CLI harness
//! (Claude Code, Codex, AGY, Copilot CLI, or a custom command).
//! The context assembly pipeline (`TabAiContextBlob`) is unchanged — this
//! module only consumes it.

pub mod quick_submit;
pub(crate) mod screenshot_files;

pub use quick_submit::{
    plan_tab_ai_quick_submit, TabAiQuickSubmitKind, TabAiQuickSubmitPlan, TabAiQuickSubmitSource,
};
pub use screenshot_files::{
    capture_tab_ai_focused_window_screenshot_file, capture_tab_ai_screen_screenshot_file,
    cleanup_old_tab_ai_screenshot_files, cleanup_old_tab_ai_screenshot_files_in_dir,
    tab_ai_screenshot_prefix, TabAiScreenshotFile, TAB_AI_SCREENSHOT_MAX_KEEP,
};

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Capture kind
// ---------------------------------------------------------------------------

/// Declares what kind of pre-switch capture the harness launch should perform.
///
/// Threaded through [`TabAiLaunchRequest`] → [`spawn_tab_ai_pre_switch_capture`]
/// so each explicit AI command gets the appropriate screenshot/context capture
/// instead of always defaulting to focused-window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAiCaptureKind {
    /// Default Tab/Shift+Tab path: focused-window screenshot + full desktop context.
    DefaultContext,
    /// Full-screen screenshot (e.g. `SendScreenToAi`).
    FullScreen,
    /// Focused-window screenshot (e.g. `SendFocusedWindowToAi`).
    FocusedWindow,
    /// Selected text context only — no screenshot (e.g. `SendSelectedTextToAi`).
    SelectedText,
    /// Browser tab URL context only — no screenshot (e.g. `SendBrowserTabToAi`).
    BrowserTab,
}

// ---------------------------------------------------------------------------
// Artifact kind
// ---------------------------------------------------------------------------

/// Resolved artifact classification for a Tab AI prompt invocation.
///
/// Drives `use_quick_terminal` routing: only `Script` variants get the Bun
/// verification gate and quick-terminal surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabAiArtifactKind {
    Script,
    ExtensionBundle,
    Agent,
}

impl TabAiArtifactKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Script => "script",
            Self::ExtensionBundle => "extensionBundle",
            Self::Agent => "agent",
        }
    }
}

/// Resolve an artifact kind from the prompt type, intent, and submission mode.
///
/// Returns `None` when the intent does not look like an artifact-creation
/// request at all (e.g. "explain this selection").
fn resolve_tab_ai_artifact_kind(
    prompt_type: &str,
    effective_intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
) -> Option<TabAiArtifactKind> {
    let normalized_intent = effective_intent
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| collapse_inline_text(&value.to_ascii_lowercase()))?;

    // Agent / MDFlow / prompt-file keywords → Agent
    if ["agent", "mdflow", "prompt file"]
        .iter()
        .any(|needle| normalized_intent.contains(needle))
    {
        // ScriptList submit with an agent intent is still an Agent, not a Script.
        return Some(TabAiArtifactKind::Agent);
    }

    // Extension-bundle / scriptlet keywords → ExtensionBundle
    if [
        "scriptlet",
        "scriptlets",
        "extension",
        "extensions",
        "bundle",
        "bundles",
        "snippet",
        "snippets",
        "text expansion",
        "template",
    ]
    .iter()
    .any(|needle| normalized_intent.contains(needle))
    {
        return Some(TabAiArtifactKind::ExtensionBundle);
    }

    // Forced ScriptList submit with non-empty intent → Script
    if should_force_artifact_guidance_for_script_list_submit(
        prompt_type,
        Some(normalized_intent.as_str()),
        mode,
    ) {
        return Some(TabAiArtifactKind::Script);
    }

    // Explicit "script" keyword or command-like artifact request → Script
    if normalized_intent.contains("script")
        || looks_like_command_like_artifact_request(&normalized_intent)
        || COMMAND_LIKE_ARTIFACT_WORDS
            .iter()
            .any(|word| normalized_intent.ends_with(word))
    {
        return Some(TabAiArtifactKind::Script);
    }

    None
}

/// Schema version for `HarnessConfig` wire format.
pub const TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION: u32 = 1;

/// Schema version for the context block injected into harnesses.
pub const TAB_AI_HARNESS_CONTEXT_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which CLI harness to connect to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HarnessBackendKind {
    ClaudeCode,
    Codex,
    Agy,
    CopilotCli,
    Custom,
}

/// Persisted configuration for the Tab AI harness.
///
/// Stored at `~/.scriptkit/config.ts` under the `claudeCode` key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessConfig {
    pub schema_version: u32,
    pub backend: HarnessBackendKind,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default = "default_tab_ai_harness_warm_on_startup")]
    pub warm_on_startup: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, String>,
}

/// Default value for [`HarnessConfig::warm_on_startup`].
///
/// Returns `true` so that omitting the field from JSON (or using
/// `HarnessConfig::default()`) enables prewarm.  Users opt *out*
/// with `"warmOnStartup": false`.
fn default_tab_ai_harness_warm_on_startup() -> bool {
    true
}

impl Default for HarnessConfig {
    fn default() -> Self {
        let working_directory = Some(crate::setup::get_kit_path().to_string_lossy().into_owned());
        Self {
            schema_version: TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION,
            backend: HarnessBackendKind::ClaudeCode,
            command: "claude".to_string(),
            args: Vec::new(),
            warm_on_startup: default_tab_ai_harness_warm_on_startup(),
            working_directory,
            env: std::collections::BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell quoting
// ---------------------------------------------------------------------------

/// Minimally shell-quote a value.  Safe characters pass through; everything
/// else gets single-quoted with internal `'` escaped via `'"'"'`.
fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-:=@".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', r#"'"'"'"#))
    }
}

fn is_valid_shell_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

impl HarnessConfig {
    /// Build a shell command line from this config.
    ///
    /// Includes env vars as a prefix and `cd <dir> &&` when a working
    /// directory is set.
    pub fn command_line(&self) -> String {
        let command_and_args = std::iter::once(shell_quote(&self.command))
            .chain(self.args.iter().map(|arg| shell_quote(arg)))
            .collect::<Vec<_>>()
            .join(" ");

        let with_env = if self.env.is_empty() {
            command_and_args
        } else {
            let env_prefix = self
                .env
                .iter()
                .filter(|(key, _)| is_valid_shell_env_key(key))
                .map(|(key, value)| format!("{key}={}", shell_quote(value)))
                .collect::<Vec<_>>()
                .join(" ");
            if env_prefix.is_empty() {
                command_and_args
            } else {
                format!("{env_prefix} {command_and_args}")
            }
        };

        match &self.working_directory {
            Some(dir) if !dir.trim().is_empty() => {
                format!("cd {} && {}", shell_quote(dir), with_env)
            }
            _ => with_env,
        }
    }
}

// ---------------------------------------------------------------------------
// Config I/O
// ---------------------------------------------------------------------------

/// Path to the harness config file.
pub fn tab_ai_harness_config_path() -> Result<std::path::PathBuf, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "tab_ai_harness_config_path: HOME is not set".to_string())?;
    Ok(std::path::Path::new(&home)
        .join(".scriptkit")
        .join("config.ts"))
}

/// Read (or default) the harness config from disk.
pub fn read_tab_ai_harness_config() -> Result<HarnessConfig, String> {
    let path = tab_ai_harness_config_path()?;
    if !path.exists() {
        return Ok(HarnessConfig::default());
    }
    let config = crate::config::load_config();
    let claude_code = config.claude_code.unwrap_or_default();

    let mut args = Vec::new();
    if !claude_code.permission_mode.trim().is_empty() {
        args.push("--permission-mode".to_string());
        args.push(claude_code.permission_mode);
    }
    if let Some(allowed_tools) = claude_code
        .allowed_tools
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--allowedTools".to_string());
        args.push(allowed_tools);
    }
    for add_dir in claude_code
        .add_dirs
        .into_iter()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("--add-dir".to_string());
        args.push(add_dir);
    }

    Ok(HarnessConfig {
        schema_version: TAB_AI_HARNESS_CONFIG_SCHEMA_VERSION,
        backend: HarnessBackendKind::ClaudeCode,
        command: claude_code.path.unwrap_or_else(|| "claude".to_string()),
        args,
        // `warmOnStartup` no longer lives in config.ts. Keep the runtime
        // default disabled so migrated users do not get stale prewarm behavior
        // from a deleted config surface.
        warm_on_startup: false,
        working_directory: Some(crate::setup::get_kit_path().to_string_lossy().into_owned()),
        env: std::collections::BTreeMap::new(),
    })
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

/// Validate that a harness config is usable: command is non-empty and the
/// binary is on PATH. Returns an actionable error message on failure.
pub fn validate_tab_ai_harness_config(config: &HarnessConfig) -> Result<(), String> {
    if config.command.trim().is_empty() {
        return Err(
            "Harness command is empty. Set claudeCode.path in ~/.scriptkit/config.ts \
             or leave it unset to use the default (claude)."
                .to_string(),
        );
    }
    if which::which(&config.command).is_err() {
        return Err(format!(
            "'{}' not found on PATH. Install the CLI or update \
             claudeCode.path in ~/.scriptkit/config.ts.",
            config.command,
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

/// Whether a harness session is a fresh prewarm (reusable once) or has been
/// consumed by a user-initiated Tab entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAiHarnessWarmState {
    /// Silently prewarmed — can be reused exactly once by the next Tab press.
    FreshPrewarm,
    /// Already consumed by a user interaction — must be torn down before reuse.
    Consumed,
}

/// Runtime state for a live harness terminal session.
#[derive(Clone)]
pub struct TabAiHarnessSessionState {
    pub config: HarnessConfig,
    pub entity: gpui::Entity<crate::term_prompt::TermPrompt>,
    pub id: String,
    pub warm_state: TabAiHarnessWarmState,
}

impl TabAiHarnessSessionState {
    pub fn new(
        config: HarnessConfig,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        id: impl Into<String>,
    ) -> Self {
        Self {
            config,
            entity,
            id: id.into(),
            warm_state: TabAiHarnessWarmState::Consumed,
        }
    }

    /// Returns `true` if this session is a fresh prewarm that has not yet been
    /// consumed by a user Tab press.
    pub fn is_fresh_prewarm(&self) -> bool {
        matches!(self.warm_state, TabAiHarnessWarmState::FreshPrewarm)
    }

    /// Mark the session as a newly created prewarm that may be reused once.
    pub fn mark_fresh_prewarm(&mut self) {
        self.warm_state = TabAiHarnessWarmState::FreshPrewarm;
    }

    /// Mark the session as consumed so it cannot be reused again.
    pub fn mark_consumed(&mut self) {
        self.warm_state = TabAiHarnessWarmState::Consumed;
    }
}

// ---------------------------------------------------------------------------
// Context formatting
// ---------------------------------------------------------------------------

/// Whether to submit context as a full turn or stage it for later input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAiHarnessSubmissionMode {
    /// Submit immediately as a full harness turn.
    Submit,
    /// Paste/stage context only; user will type intent next.
    PasteOnly,
}

fn collapse_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_line(out: &mut String, label: &str, value: impl AsRef<str>) {
    let value = value.as_ref().trim();
    if value.is_empty() {
        return;
    }
    out.push_str(label);
    out.push_str(": ");
    out.push_str(value);
    out.push('\n');
}

fn push_block(out: &mut String, label: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    out.push_str(label);
    out.push_str(":\n");
    out.push_str(value);
    if !value.ends_with('\n') {
        out.push('\n');
    }
}

/// Emit scalar fields from a JSON object as individual labeled lines.
/// Non-scalar values (arrays, nested objects) are silently skipped so the
/// output stays flat and token-efficient.
fn push_json_scalar_lines(out: &mut String, label_prefix: &str, value: &serde_json::Value) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, value) in object {
        match value {
            serde_json::Value::Null => {}
            serde_json::Value::Bool(v) => {
                push_line(out, &format!("{label_prefix} {key}"), v.to_string());
            }
            serde_json::Value::Number(v) => {
                push_line(out, &format!("{label_prefix} {key}"), v.to_string());
            }
            serde_json::Value::String(v) => {
                push_line(
                    out,
                    &format!("{label_prefix} {key}"),
                    collapse_inline_text(v),
                );
            }
            _ => {}
        }
    }
}

/// Emit a target's fields as sequential labeled lines instead of a single
/// pipe-delimited line.  This is more readable in the terminal and wastes
/// fewer tokens for the consuming LLM.
fn push_target_lines(out: &mut String, label_prefix: &str, target: &crate::ai::TabAiTargetContext) {
    push_line(out, &format!("{label_prefix} source"), &target.source);
    push_line(out, &format!("{label_prefix} kind"), &target.kind);
    push_line(
        out,
        &format!("{label_prefix} semantic id"),
        &target.semantic_id,
    );
    push_line(
        out,
        &format!("{label_prefix} label"),
        collapse_inline_text(&target.label),
    );
    if let Some(metadata) = target.metadata.as_ref() {
        push_json_scalar_lines(out, &format!("{label_prefix} metadata"), metadata);
    }
}

fn push_visible_element_lines(
    out: &mut String,
    label_prefix: &str,
    element: &crate::protocol::ElementInfo,
) {
    push_line(
        out,
        &format!("{label_prefix} semantic id"),
        &element.semantic_id,
    );
    if let Some(text) = element.text.as_deref() {
        push_line(
            out,
            &format!("{label_prefix} text"),
            collapse_inline_text(text),
        );
    }
    if let Some(value) = element.value.as_deref() {
        push_line(
            out,
            &format!("{label_prefix} value"),
            collapse_inline_text(value),
        );
    }
    if let Some(selected) = element.selected {
        push_line(
            out,
            &format!("{label_prefix} selected"),
            selected.to_string(),
        );
    }
    if let Some(focused) = element.focused {
        push_line(out, &format!("{label_prefix} focused"), focused.to_string());
    }
    if let Some(index) = element.index {
        push_line(out, &format!("{label_prefix} index"), index.to_string());
    }
}

fn push_clipboard_history_lines(
    out: &mut String,
    label_prefix: &str,
    entry: &crate::ai::TabAiClipboardHistoryEntry,
) {
    push_line(out, &format!("{label_prefix} type"), &entry.content_type);
    push_line(
        out,
        &format!("{label_prefix} preview"),
        collapse_inline_text(&entry.preview),
    );
    push_line(
        out,
        &format!("{label_prefix} timestamp"),
        entry.timestamp.to_string(),
    );
    if let Some(text) = entry
        .full_text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        push_block(out, &format!("{label_prefix} text"), text);
    }
    if let Some(ocr) = entry
        .ocr_text
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        push_block(out, &format!("{label_prefix} ocr"), ocr);
    }
    if let Some(width) = entry.image_width {
        push_line(
            out,
            &format!("{label_prefix} image width"),
            width.to_string(),
        );
    }
    if let Some(height) = entry.image_height {
        push_line(
            out,
            &format!("{label_prefix} image height"),
            height.to_string(),
        );
    }
}

fn push_prior_automation_lines(
    out: &mut String,
    label_prefix: &str,
    item: &crate::ai::TabAiMemorySuggestion,
) {
    push_line(out, &format!("{label_prefix} slug"), &item.slug);
    push_block(out, &format!("{label_prefix} query"), &item.effective_query);
    push_line(
        out,
        &format!("{label_prefix} prompt type"),
        &item.prompt_type,
    );
    push_line(out, &format!("{label_prefix} bundle id"), &item.bundle_id);
    push_line(out, &format!("{label_prefix} written at"), &item.written_at);
    push_line(
        out,
        &format!("{label_prefix} score"),
        format!("{:.3}", item.score),
    );
}

/// Build a flat, labeled context block from a resolved context blob.
pub fn build_tab_ai_harness_context_block(
    context: &crate::ai::TabAiContextBlob,
) -> Result<String, String> {
    let mut out = String::new();

    out.push_str("Script Kit context\n");
    out.push_str("Use this as ambient context for the next user request.\n");
    out.push_str(
        "Prefer focused target over visible targets when the user says \"this\", \"it\", or \"selected\".\n\n",
    );

    push_line(
        &mut out,
        "schema version",
        context.schema_version.to_string(),
    );
    push_line(&mut out, "timestamp", &context.timestamp);
    push_line(&mut out, "prompt type", &context.ui.prompt_type);

    if let Some(input_text) = context.ui.input_text.as_deref() {
        push_block(&mut out, "current input", input_text);
    }
    if let Some(id) = context.ui.focused_semantic_id.as_deref() {
        push_line(&mut out, "focused semantic id", id);
    }
    if let Some(id) = context.ui.selected_semantic_id.as_deref() {
        push_line(&mut out, "selected semantic id", id);
    }

    if let Some(target) = context.focused_target.as_ref() {
        push_target_lines(&mut out, "focused target", target);
    }
    let has_visible_targets = !context.visible_targets.is_empty();
    for (index, target) in context.visible_targets.iter().take(6).enumerate() {
        push_target_lines(&mut out, &format!("visible target {}", index + 1), target);
    }
    // Only emit raw visible elements when target resolution did not already
    // project the surface into higher-signal targets.
    if !has_visible_targets {
        for (index, element) in context.ui.visible_elements.iter().take(6).enumerate() {
            push_visible_element_lines(
                &mut out,
                &format!("visible element {}", index + 1),
                element,
            );
        }
    }

    if let Some(text) = context.desktop.selected_text.as_deref() {
        push_block(&mut out, "selected text", text);
    }
    if let Some(app) = context.desktop.frontmost_app.as_ref() {
        push_line(&mut out, "frontmost app name", &app.name);
        push_line(&mut out, "frontmost app bundle id", &app.bundle_id);
        push_line(&mut out, "frontmost app pid", app.pid.to_string());
    }
    if let Some(browser) = context.desktop.browser.as_ref() {
        push_line(&mut out, "browser url", &browser.url);
    }
    if let Some(window) = context.desktop.focused_window.as_ref() {
        push_line(
            &mut out,
            "focused window title",
            collapse_inline_text(&window.title),
        );
        push_line(&mut out, "focused window width", window.width.to_string());
        push_line(&mut out, "focused window height", window.height.to_string());
        push_line(
            &mut out,
            "focused window used fallback",
            window.used_fallback.to_string(),
        );
    }
    for (index, warning) in context.desktop.warnings.iter().enumerate() {
        push_line(&mut out, &format!("desktop warning {}", index + 1), warning);
    }

    for (index, recent_input) in context.recent_inputs.iter().take(5).enumerate() {
        push_line(
            &mut out,
            &format!("recent input {}", index + 1),
            collapse_inline_text(recent_input),
        );
    }

    if let Some(clipboard) = context.clipboard.as_ref() {
        push_line(&mut out, "clipboard type", &clipboard.content_type);
        push_line(
            &mut out,
            "clipboard preview",
            collapse_inline_text(&clipboard.preview),
        );
        if let Some(ocr) = clipboard.ocr_text.as_deref() {
            push_line(&mut out, "clipboard ocr", collapse_inline_text(ocr));
        }
    }

    for (index, entry) in context.clipboard_history.iter().take(5).enumerate() {
        push_clipboard_history_lines(&mut out, &format!("clipboard history {}", index + 1), entry);
    }
    for (index, item) in context.prior_automations.iter().take(3).enumerate() {
        push_prior_automation_lines(&mut out, &format!("prior automation {}", index + 1), item);
    }

    if let Some(source_type) = context.source_type.as_ref() {
        push_line(&mut out, "source type", format!("{source_type:?}"));
    }
    if let Some(path) = context.screenshot_path.as_deref() {
        push_line(&mut out, "screenshot path", path);
        out.push_str("NOTE: A screenshot of the user's focused window is included as an image in this message. You can also read it from the file path above. Use this visual context when the user asks about what's on their screen.\n");
    }
    if let Some(hint) = context.apply_back_hint.as_ref() {
        push_line(&mut out, "apply back action", &hint.action);
        if let Some(label) = hint.target_label.as_deref() {
            push_line(&mut out, "apply back target", label);
        }
    }

    Ok(out.trim_end().to_string())
}

// Hints block removed: submission uses flat context lines only (no XML blobs).

// ---------------------------------------------------------------------------
// Artifact authoring guidance
// ---------------------------------------------------------------------------

const ARTIFACT_AUTHORING_CONTAINS: &[&str] = &[
    "create", "make", "write", "build", "generate", "scaffold", "spin up", "set up",
];

const ARTIFACT_AUTHORING_PREFIXES: &[&str] = &[
    "new ",
    "add ",
    "need ",
    "want ",
    "help me make ",
    "help me create ",
];

const ARTIFACT_AUTHORING_WORDS: &[&str] = &[
    "script",
    "scriptlet",
    "scriptlets",
    "extension",
    "extensions",
    "bundle",
    "bundles",
    "extension bundle",
    "extension bundles",
    "scriptlet bundle",
    "scriptlet bundles",
    "snippet",
    "snippets",
    "snippet bundle",
    "snippet bundles",
    "text expansion",
    "quick command",
    "template",
    "agent",
    "mdflow",
    "prompt file",
];

/// Returns `true` for bare artifact nouns like "snippet", "a script",
/// "new extension" where the noun alone signals authoring intent.
fn looks_like_bare_artifact_request(intent: &str) -> bool {
    let prefixes = ["", "a ", "an ", "new ", "my "];
    ARTIFACT_AUTHORING_WORDS.iter().any(|artifact| {
        prefixes.iter().any(|prefix| {
            let candidate = format!("{prefix}{artifact}");
            intent == candidate || intent.starts_with(&format!("{candidate} "))
        })
    })
}

/// Non-creation verbs that, when starting a phrase, indicate the user is
/// operating on an existing artifact rather than requesting a new one.
const NON_CREATION_LEADING_VERBS: &[&str] = &[
    "run ",
    "open ",
    "edit ",
    "delete ",
    "remove ",
    "rename ",
    "move ",
    "copy ",
    "list ",
    "show ",
    "find ",
    "search ",
    "debug ",
    "fix ",
    "update ",
    "test ",
    "check ",
    "explain ",
    "describe ",
];

/// Returns `true` for short descriptive phrases ending with an artifact noun,
/// e.g. "PR review agent", "date snippet", "clipboard cleanup script".
/// These imply creation intent even without an explicit verb.
fn looks_like_descriptive_artifact_phrase(intent: &str) -> bool {
    let words: Vec<&str> = intent.split_whitespace().collect();
    // Only match short phrases (2-6 words) — longer sentences likely have
    // their own verb structure and should be caught by the verb+noun path.
    if words.len() < 2 || words.len() > 6 {
        return false;
    }
    // Exclude phrases that start with a non-creation verb.
    if NON_CREATION_LEADING_VERBS
        .iter()
        .any(|verb| intent.starts_with(verb))
    {
        return false;
    }
    // Check if the phrase ends with an artifact noun.
    ARTIFACT_AUTHORING_WORDS
        .iter()
        .any(|artifact| intent.ends_with(artifact))
}

/// Words that users treat as synonyms for "Script Kit artifact" without using
/// any of the canonical artifact nouns (script, bundle, agent, etc.).
const COMMAND_LIKE_ARTIFACT_WORDS: &[&str] = &[
    "command",
    "commands",
    "helper",
    "helpers",
    "tool",
    "tools",
    "workflow",
    "workflows",
];

/// Returns `true` for short command-like requests that end with an artifact
/// synonym (e.g. "clipboard cleanup command", "jira helper") but whose leading
/// verb is not a non-creation verb ("run", "fix", "edit", …).
fn looks_like_command_like_artifact_request(intent: &str) -> bool {
    let words: Vec<&str> = intent.split_whitespace().collect();
    if words.len() < 2 || words.len() > 8 {
        return false;
    }
    if NON_CREATION_LEADING_VERBS
        .iter()
        .any(|verb| intent.starts_with(verb))
    {
        return false;
    }
    COMMAND_LIKE_ARTIFACT_WORDS
        .iter()
        .any(|word| intent.ends_with(word))
}

/// Returns `true` when the intent looks like a request to create/scaffold a
/// Script Kit artifact (script, scriptlet bundle, agent).  Used to decide
/// whether to inject the artifact authoring guidance block.
pub fn should_include_artifact_authoring_guidance(intent: Option<&str>) -> bool {
    let Some(intent) = intent.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    let intent = collapse_inline_text(&intent.to_ascii_lowercase());

    let has_authoring_signal = ARTIFACT_AUTHORING_CONTAINS
        .iter()
        .any(|needle| intent.contains(needle))
        || ARTIFACT_AUTHORING_PREFIXES
            .iter()
            .any(|needle| intent.starts_with(needle));

    let has_artifact_word = ARTIFACT_AUTHORING_WORDS
        .iter()
        .any(|needle| intent.contains(needle));

    let has_command_like_suffix = COMMAND_LIKE_ARTIFACT_WORDS
        .iter()
        .any(|word| intent.ends_with(word));

    (has_authoring_signal && (has_artifact_word || has_command_like_suffix))
        || looks_like_bare_artifact_request(&intent)
        || looks_like_descriptive_artifact_phrase(&intent)
        || looks_like_command_like_artifact_request(&intent)
}

/// Force authoring guidance for the ScriptList submit flow.
///
/// This covers terse generation queries like "clipboard cleanup" that do not
/// contain explicit artifact words but still mean "create a Script Kit artifact".
/// The current heuristic-based classifier remains as a fallback for other prompt
/// types and submission modes.
fn should_force_artifact_guidance_for_script_list_submit(
    prompt_type: &str,
    effective_intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
) -> bool {
    let has_non_empty_intent = effective_intent
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false);

    prompt_type == "ScriptList"
        && matches!(mode, TabAiHarnessSubmissionMode::Submit)
        && has_non_empty_intent
}

// ---------------------------------------------------------------------------
// Verification-marker constants and detection
// ---------------------------------------------------------------------------

pub const SCRIPT_AUTHORING_SKILL_MARKER: &str =
    "~/.scriptkit/plugins/scriptkit/skills/new-script/SKILL.md";
pub const BUN_BUILD_VERIFICATION_MARKER: &str = "bun build ~/.scriptkit/plugins/main/scripts/<name>.ts --target=bun --outfile ~/.scriptkit/tmp/test-scripts/<name>.verify.mjs";
pub const BUN_EXECUTE_VERIFICATION_MARKER: &str =
    "SK_VERIFY=1 bun ~/.scriptkit/plugins/main/scripts/<name>.ts";
pub(crate) const BUN_VERIFICATION_SUCCESS_CRITERIA: &str = "Confirm the stdout, written file, or other observable result from the script in ~/.scriptkit/plugins/main/scripts/ matches the user's request.";
pub(crate) const BUN_VERIFICATION_FAILURE_POLICY: &str = "If either Bun command fails, fix the script and rerun both commands inside the same Claude Code terminal session before reporting success.";
pub const SCRIPT_READY_RECEIPT_MARKER: &str =
    "SCRIPT_READY path=~/.scriptkit/plugins/main/scripts/<name>.ts validated=true";

/// Structured detection of which verification markers are present in a
/// guidance block.  Used by both the Agent Chat and PTY telemetry paths so marker
/// detection cannot drift between surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TabAiVerificationGuidanceMarkers {
    pub includes_script_authoring_skill: bool,
    pub includes_bun_build_verification: bool,
    pub includes_bun_execute_verification: bool,
    pub includes_script_ready_receipt: bool,
}

impl TabAiVerificationGuidanceMarkers {
    pub(crate) fn from_guidance(guidance: &str) -> Self {
        Self {
            includes_script_authoring_skill: guidance.contains(SCRIPT_AUTHORING_SKILL_MARKER),
            includes_bun_build_verification: guidance.contains(BUN_BUILD_VERIFICATION_MARKER),
            includes_bun_execute_verification: guidance.contains(BUN_EXECUTE_VERIFICATION_MARKER),
            includes_script_ready_receipt: guidance.contains(SCRIPT_READY_RECEIPT_MARKER),
        }
    }
}

// ---------------------------------------------------------------------------
// Cached structured artifact-authoring appendix resolver
// ---------------------------------------------------------------------------

/// Pre-computed metadata from the static guidance block.  Allocated once via
/// `LazyLock` so marker detection is never re-derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TabAiCachedArtifactAuthoringGuidance {
    guidance: &'static str,
    has_script_verification_gate_header: bool,
    markers: TabAiVerificationGuidanceMarkers,
}

/// Fully resolved appendix for a single prompt invocation.
///
/// This is the crate-visible structured result that PTY submission, Agent Chat initial
/// input, and surface-preference selection all consume directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TabAiArtifactAuthoringAppendix {
    pub artifact_kind: Option<TabAiArtifactKind>,
    pub guidance: &'static str,
    pub forced_by_script_list_submit: bool,
    pub has_script_verification_gate_header: bool,
    pub markers: TabAiVerificationGuidanceMarkers,
    pub use_quick_terminal: bool,
}

static TAB_AI_CACHED_ARTIFACT_AUTHORING_GUIDANCE: LazyLock<TabAiCachedArtifactAuthoringGuidance> =
    LazyLock::new(|| {
        let guidance = build_tab_ai_artifact_authoring_guidance_block();
        let markers = TabAiVerificationGuidanceMarkers::from_guidance(guidance);
        TabAiCachedArtifactAuthoringGuidance {
            guidance,
            has_script_verification_gate_header: guidance.contains(SCRIPT_VERIFICATION_GATE_HEADER),
            markers,
        }
    });

fn resolve_tab_ai_artifact_authoring_appendix_for_prompt(
    prompt_type: &str,
    effective_intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
) -> Option<TabAiArtifactAuthoringAppendix> {
    let effective_intent = effective_intent
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let forced_by_script_list_submit =
        should_force_artifact_guidance_for_script_list_submit(prompt_type, effective_intent, mode);

    if !(forced_by_script_list_submit
        || should_include_artifact_authoring_guidance(effective_intent))
    {
        return None;
    }

    let artifact_kind = resolve_tab_ai_artifact_kind(prompt_type, effective_intent, mode);
    let cached = &*TAB_AI_CACHED_ARTIFACT_AUTHORING_GUIDANCE;

    // use_quick_terminal is true only for Script artifacts whose cached
    // guidance includes all three verification markers.
    let use_quick_terminal = matches!(artifact_kind, Some(TabAiArtifactKind::Script))
        && cached.markers.includes_script_authoring_skill
        && cached.markers.includes_bun_build_verification
        && cached.markers.includes_bun_execute_verification;

    Some(TabAiArtifactAuthoringAppendix {
        artifact_kind,
        guidance: cached.guidance,
        forced_by_script_list_submit,
        has_script_verification_gate_header: cached.has_script_verification_gate_header,
        markers: cached.markers,
        use_quick_terminal,
    })
}

/// Build the artifact-authoring guidance appendix for a Tab AI submission.
///
/// Returns the full structured appendix so PTY submission, Agent Chat initial input,
/// and surface-preference logic all consume the same resolved fields.
pub(crate) fn build_tab_ai_artifact_authoring_appendix_for_prompt(
    prompt_type: &str,
    effective_intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
) -> Option<TabAiArtifactAuthoringAppendix> {
    resolve_tab_ai_artifact_authoring_appendix_for_prompt(prompt_type, effective_intent, mode)
}

// ---------------------------------------------------------------------------
// Surface-preference helper (derived from shared appendix builder)
// ---------------------------------------------------------------------------

/// Derived surface preference for verification-bearing script authoring prompts.
///
/// All marker flags are computed from the shared appendix builder output so
/// detection cannot drift between surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabAiSurfacePreference {
    pub use_quick_terminal: bool,
    pub includes_script_authoring_skill: bool,
    pub includes_bun_build_verification: bool,
    pub includes_bun_execute_verification: bool,
}

/// Derive the preferred Tab AI surface from the shared appendix builder.
///
/// Returns `use_quick_terminal = true` only when the guidance includes the
/// new-script marker AND both Bun verification markers.  When no
/// appendix is produced, all flags are `false`.
pub fn tab_ai_surface_preference_for_prompt(
    prompt_type: &str,
    effective_intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
) -> TabAiSurfacePreference {
    let Some(appendix) =
        build_tab_ai_artifact_authoring_appendix_for_prompt(prompt_type, effective_intent, mode)
    else {
        return TabAiSurfacePreference {
            use_quick_terminal: false,
            includes_script_authoring_skill: false,
            includes_bun_build_verification: false,
            includes_bun_execute_verification: false,
        };
    };

    TabAiSurfacePreference {
        use_quick_terminal: appendix.use_quick_terminal,
        includes_script_authoring_skill: appendix.markers.includes_script_authoring_skill,
        includes_bun_build_verification: appendix.markers.includes_bun_build_verification,
        includes_bun_execute_verification: appendix.markers.includes_bun_execute_verification,
    }
}

// ---------------------------------------------------------------------------
// Agent Chat initial-input builder (single-sourced)
// ---------------------------------------------------------------------------

/// Structured result from building Agent Chat initial input, carrying telemetry
/// fields that record which verification markers were present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TabAiAgentChatInitialInput {
    pub text: String,
    pub guidance_appended: bool,
    pub forced_by_script_list_submit: bool,
    pub artifact_kind: Option<TabAiArtifactKind>,
    pub use_quick_terminal: bool,
    pub includes_script_authoring_skill: bool,
    pub includes_bun_build_verification: bool,
    pub includes_bun_execute_verification: bool,
}

/// Build the Agent Chat initial input for a given prompt type and intent.
///
/// This is the single-sourced formatter that both the PTY and Agent Chat paths
/// consume, ensuring the mandatory Bun verification guidance cannot drift
/// between the two surfaces.
pub(crate) fn build_tab_ai_agent_chat_initial_input_for_prompt(
    prompt_type: &str,
    intent: &str,
) -> TabAiAgentChatInitialInput {
    let intent = intent.trim();

    let result = if intent.is_empty() {
        TabAiAgentChatInitialInput {
            text: String::new(),
            guidance_appended: false,
            forced_by_script_list_submit: false,
            artifact_kind: None,
            use_quick_terminal: false,
            includes_script_authoring_skill: false,
            includes_bun_build_verification: false,
            includes_bun_execute_verification: false,
        }
    } else if let Some(appendix) = build_tab_ai_artifact_authoring_appendix_for_prompt(
        prompt_type,
        Some(intent),
        TabAiHarnessSubmissionMode::Submit,
    ) {
        let guidance = appendix.guidance;
        TabAiAgentChatInitialInput {
            text: format!("{guidance}\n\nUser intent:\n{intent}\n"),
            guidance_appended: true,
            forced_by_script_list_submit: appendix.forced_by_script_list_submit,
            artifact_kind: appendix.artifact_kind,
            use_quick_terminal: appendix.use_quick_terminal,
            includes_script_authoring_skill: appendix.markers.includes_script_authoring_skill,
            includes_bun_build_verification: appendix.markers.includes_bun_build_verification,
            includes_bun_execute_verification: appendix.markers.includes_bun_execute_verification,
        }
    } else {
        TabAiAgentChatInitialInput {
            text: intent.to_string(),
            guidance_appended: false,
            forced_by_script_list_submit: false,
            artifact_kind: None,
            use_quick_terminal: false,
            includes_script_authoring_skill: false,
            includes_bun_build_verification: false,
            includes_bun_execute_verification: false,
        }
    };

    tracing::info!(
        event = "tab_ai_agent_chat_initial_input_built",
        prompt_type,
        guidance_appended = result.guidance_appended,
        forced_by_script_list_submit = result.forced_by_script_list_submit,
        artifact_kind = result
            .artifact_kind
            .map(TabAiArtifactKind::as_str)
            .unwrap_or("unknown"),
        use_quick_terminal = result.use_quick_terminal,
        includes_script_authoring_skill = result.includes_script_authoring_skill,
        includes_bun_build_verification = result.includes_bun_build_verification,
        includes_bun_execute_verification = result.includes_bun_execute_verification,
        text_len = result.text.len(),
    );

    result
}

/// Canonical one-shot authoring launchpad for harness mode.
///
/// Keep `kit-init/examples/START_HERE.md` as the single source of truth.
/// `ROOT_CLAUDE.md` and `ROOT_AGENTS.md` should route here instead of
/// duplicating starter templates or artifact-branching copy.
const TAB_AI_ONE_SHOT_LAUNCHPAD_SOURCE: &str =
    include_str!("../../../kit-init/examples/START_HERE.md");

const SCRIPT_VERIFICATION_GATE_HEADER: &str = "MANDATORY SCRIPT VERIFICATION";

fn build_tab_ai_script_verification_gate() -> String {
    format!(
        concat!(
            "{}\n",
            "If the correct artifact is a `.ts` script under `~/.scriptkit/plugins/main/scripts/`, ",
            "verify that final script inside this Claude Code terminal session before reporting success.\n",
            "Read: {}\n",
            "Build: {}\n",
            "Run: {}\n",
            "Observe: {}\n",
            "Failure policy: {}\n",
        ),
        SCRIPT_VERIFICATION_GATE_HEADER,
        SCRIPT_AUTHORING_SKILL_MARKER,
        BUN_BUILD_VERIFICATION_MARKER,
        BUN_EXECUTE_VERIFICATION_MARKER,
        BUN_VERIFICATION_SUCCESS_CRITERIA,
        BUN_VERIFICATION_FAILURE_POLICY,
    )
}

/// Cached guidance block — allocated once on first access.
static TAB_AI_ARTIFACT_AUTHORING_GUIDANCE_BLOCK: LazyLock<String> = LazyLock::new(|| {
    format!(
        "--- Script Kit artifact authoring guidance ---\n{}\n\n{}\n--- end artifact authoring guidance ---",
        build_tab_ai_script_verification_gate().trim_end(),
        TAB_AI_ONE_SHOT_LAUNCHPAD_SOURCE.trim_end()
    )
});

/// Wrap the canonical launchpad content in delimiters for PTY injection.
fn build_tab_ai_artifact_authoring_guidance_block() -> &'static str {
    TAB_AI_ARTIFACT_AUTHORING_GUIDANCE_BLOCK.as_str()
}

// ---------------------------------------------------------------------------
// Full submission builder
// ---------------------------------------------------------------------------

/// Build a full harness submission: flat context block + optional user intent.
///
/// Behavior depends on `mode`:
/// - `Submit` without intent: appends a sentinel asking the harness to wait.
/// - `PasteOnly` without intent: stages context only, no synthetic turn text.
/// - With intent (either mode): appends the intent as `User intent:`.
///
/// When the submission is an artifact-creation request — either by heuristic
/// intent detection or by the deterministic `ScriptList` submit path — a
/// text-native artifact authoring guidance block is appended between context
/// and intent.
pub fn build_tab_ai_harness_submission(
    context: &crate::ai::TabAiContextBlob,
    intent: Option<&str>,
    mode: TabAiHarnessSubmissionMode,
    quick_submit: Option<&TabAiQuickSubmitPlan>,
    _invocation_receipt: Option<&crate::ai::TabAiInvocationReceipt>,
    _suggested_intents: &[crate::ai::TabAiSuggestedIntentSpec],
) -> Result<String, String> {
    let mut output = build_tab_ai_harness_context_block(context)?;

    // Prefer the quick-submit plan's submission_intent() (which returns
    // raw_query for Fallback sources) over the caller-provided intent string.
    let effective_intent = quick_submit
        .map(TabAiQuickSubmitPlan::submission_intent)
        .or(intent)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(appendix) = build_tab_ai_artifact_authoring_appendix_for_prompt(
        &context.ui.prompt_type,
        effective_intent,
        mode,
    ) {
        // Source-contract audit anchor: ~/.scriptkit/plugins/scriptkit/skills/new-script/SKILL.md
        tracing::info!(
            event = "tab_ai_artifact_authoring_guidance_appended",
            script_authoring_skill_path = SCRIPT_AUTHORING_SKILL_MARKER,
            forced_by_script_list_submit = appendix.forced_by_script_list_submit,
            artifact_kind = appendix
                .artifact_kind
                .map(TabAiArtifactKind::as_str)
                .unwrap_or("unknown"),
            script_verification_gate_present = appendix.has_script_verification_gate_header,
            includes_script_authoring_skill = appendix.markers.includes_script_authoring_skill,
            includes_bun_build_verification = appendix.markers.includes_bun_build_verification,
            includes_bun_execute_verification = appendix.markers.includes_bun_execute_verification,
            use_quick_terminal = appendix.use_quick_terminal,
        );
        output.push_str("\n\n");
        output.push_str(appendix.guidance);
    }

    match effective_intent {
        Some(intent) => {
            output.push_str("\n\nUser intent:\n");
            output.push_str(intent);
            output.push('\n');
        }
        None if matches!(mode, TabAiHarnessSubmissionMode::Submit) => {
            output.push_str("\n\nAwait the user's next terminal input.\n");
        }
        None => {
            // PasteOnly: stage context only, but leave the cursor on a fresh
            // line so the user's next keystrokes do not join the closing tag.
            if !output.ends_with('\n') {
                output.push('\n');
            }
        }
    }
    Ok(output)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
include!("mod_tests.rs");
