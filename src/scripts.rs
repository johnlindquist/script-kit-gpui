#![allow(dead_code)]

use glob::glob;
use nucleo_matcher::pattern::Pattern;
use nucleo_matcher::{Matcher, Utf32Str};
use std::cmp::Ordering;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, instrument, warn};

use crate::app_launcher::AppInfo;
pub use crate::builtins::BuiltInEntry;
use crate::frecency::FrecencyStore;
use crate::list_item::GroupedListItem;
use crate::metadata_parser::{extract_typed_metadata, TypedMetadata};
use crate::schema_parser::{extract_schema, Schema};
use crate::scriptlets as scriptlet_parser;

#[derive(Clone, Debug, Default)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub extension: String,
    pub description: Option<String>,
    /// Icon name from // Icon: metadata (e.g., "File", "Terminal", "Star")
    /// Defaults to "Code" if not specified
    pub icon: Option<String>,
    /// Alias for quick triggering (e.g., "gc" for "git-commit")
    pub alias: Option<String>,
    /// Keyboard shortcut for direct invocation (e.g., "opt i", "cmd shift k")
    pub shortcut: Option<String>,
    /// Typed metadata from `metadata = { ... }` declaration in script
    pub typed_metadata: Option<TypedMetadata>,
    /// Schema definition from `schema = { ... }` declaration in script
    pub schema: Option<Schema>,
}

/// Represents a scriptlet parsed from a markdown file
/// Scriptlets are code snippets extracted from .md files with metadata
#[derive(Clone, Debug)]
pub struct Scriptlet {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub tool: String, // "ts", "bash", "paste", etc.
    pub shortcut: Option<String>,
    pub expand: Option<String>,
    /// Group name from H1 header (e.g., "Productivity", "Development")
    pub group: Option<String>,
    /// Source file path with anchor for execution (e.g., "/path/to/file.md#slug")
    pub file_path: Option<String>,
    /// Command slug for execution
    pub command: Option<String>,
    /// Alias for quick triggering
    pub alias: Option<String>,
}

/// Represents match indices for highlighting matched characters
#[derive(Clone, Debug, Default)]
pub struct MatchIndices {
    /// Indices of matched characters in the name
    pub name_indices: Vec<usize>,
    /// Indices of matched characters in the filename/path
    pub filename_indices: Vec<usize>,
}

/// Represents a scored match result for fuzzy search
#[derive(Clone, Debug)]
pub struct ScriptMatch {
    pub script: Script,
    pub score: i32,
    /// The filename used for matching (e.g., "my-script.ts")
    pub filename: String,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
}

/// Represents a scored match result for fuzzy search on scriptlets
#[derive(Clone, Debug)]
pub struct ScriptletMatch {
    pub scriptlet: Scriptlet,
    pub score: i32,
    /// The display file path with anchor for matching (e.g., "url.md#open-github")
    pub display_file_path: Option<String>,
    /// Indices of matched characters for UI highlighting
    pub match_indices: MatchIndices,
}

/// Represents a scored match result for fuzzy search on built-in entries
#[derive(Clone, Debug)]
pub struct BuiltInMatch {
    pub entry: BuiltInEntry,
    pub score: i32,
}

/// Represents a scored match result for fuzzy search on applications
#[derive(Clone, Debug)]
pub struct AppMatch {
    pub app: crate::app_launcher::AppInfo,
    pub score: i32,
}

/// Represents a scored match result for fuzzy search on windows
#[derive(Clone, Debug)]
pub struct WindowMatch {
    pub window: crate::window_control::WindowInfo,
    pub score: i32,
}

/// Unified search result that can be a Script, Scriptlet, BuiltIn, App, or Window
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SearchResult {
    Script(ScriptMatch),
    Scriptlet(ScriptletMatch),
    BuiltIn(BuiltInMatch),
    App(AppMatch),
    Window(WindowMatch),
}

impl SearchResult {
    /// Get the display name for this result
    pub fn name(&self) -> &str {
        match self {
            SearchResult::Script(sm) => &sm.script.name,
            SearchResult::Scriptlet(sm) => &sm.scriptlet.name,
            SearchResult::BuiltIn(bm) => &bm.entry.name,
            SearchResult::App(am) => &am.app.name,
            SearchResult::Window(wm) => &wm.window.title,
        }
    }

    /// Get the description for this result
    pub fn description(&self) -> Option<&str> {
        match self {
            SearchResult::Script(sm) => sm.script.description.as_deref(),
            SearchResult::Scriptlet(sm) => sm.scriptlet.description.as_deref(),
            SearchResult::BuiltIn(bm) => Some(&bm.entry.description),
            SearchResult::App(am) => am.app.path.to_str(),
            SearchResult::Window(wm) => Some(&wm.window.app),
        }
    }

    /// Get the score for this result
    pub fn score(&self) -> i32 {
        match self {
            SearchResult::Script(sm) => sm.score,
            SearchResult::Scriptlet(sm) => sm.score,
            SearchResult::BuiltIn(bm) => bm.score,
            SearchResult::App(am) => am.score,
            SearchResult::Window(wm) => wm.score,
        }
    }

    /// Get the type label for UI display
    pub fn type_label(&self) -> &'static str {
        match self {
            SearchResult::Script(_) => "Script",
            SearchResult::Scriptlet(_) => "Snippet",
            SearchResult::BuiltIn(_) => "Built-in",
            SearchResult::App(_) => "App",
            SearchResult::Window(_) => "Window",
        }
    }
}

/// Metadata extracted from script file comments
#[derive(Debug, Default, Clone)]
pub struct ScriptMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Icon name (e.g., "File", "Terminal", "Star", "Folder")
    pub icon: Option<String>,
    /// Alias for quick invocation (e.g., "gpt" triggers on "gpt ")
    pub alias: Option<String>,
    /// Keyboard shortcut for direct invocation (e.g., "opt i", "cmd shift k")
    pub shortcut: Option<String>,
}

/// Schedule metadata extracted from script file comments
/// Used for cron-based script scheduling
#[derive(Debug, Default, Clone)]
pub struct ScheduleMetadata {
    /// Raw cron expression from `// Cron: */5 * * * *`
    pub cron: Option<String>,
    /// Natural language schedule from `// Schedule: every tuesday at 2pm`
    pub schedule: Option<String>,
}

/// Parse a single metadata line with lenient matching
/// Supports patterns like:
/// - "//Name:Value"
/// - "//Name: Value"
/// - "// Name:Value"
/// - "// Name: Value"
/// - "//  Name:Value"
/// - "//  Name: Value"
/// - "//\tName:Value"
/// - "//\tName: Value"
///
/// Returns Some((key, value)) if the line is a valid metadata comment, None otherwise.
/// Key matching is case-insensitive.
pub fn parse_metadata_line(line: &str) -> Option<(String, String)> {
    // Must start with //
    let after_slashes = line.strip_prefix("//")?;

    // Skip any whitespace (spaces or tabs) after the slashes
    let trimmed = after_slashes.trim_start();

    // Find the colon that separates key from value
    let colon_pos = trimmed.find(':')?;

    // Key is before the colon (no spaces in key names like "Name", "Description")
    let key = trimmed[..colon_pos].trim();

    // Key must be a single word (no spaces)
    if key.is_empty() || key.contains(' ') {
        return None;
    }

    // Value is after the colon, trimmed
    let value = trimmed[colon_pos + 1..].trim();

    Some((key.to_string(), value.to_string()))
}

/// Extract metadata from script content
/// Parses lines looking for "// Name:", "// Description:", and "// Icon:" with lenient matching
/// Only checks the first 20 lines of the file
pub fn extract_script_metadata(content: &str) -> ScriptMetadata {
    let mut metadata = ScriptMetadata::default();

    for line in content.lines().take(20) {
        if let Some((key, value)) = parse_metadata_line(line) {
            match key.to_lowercase().as_str() {
                "name" => {
                    if metadata.name.is_none() && !value.is_empty() {
                        metadata.name = Some(value);
                    }
                }
                "description" => {
                    if metadata.description.is_none() && !value.is_empty() {
                        metadata.description = Some(value);
                    }
                }
                "icon" => {
                    if metadata.icon.is_none() && !value.is_empty() {
                        metadata.icon = Some(value);
                    }
                }
                "alias" => {
                    if metadata.alias.is_none() && !value.is_empty() {
                        metadata.alias = Some(value);
                    }
                }
                "shortcut" => {
                    if metadata.shortcut.is_none() && !value.is_empty() {
                        metadata.shortcut = Some(value);
                    }
                }
                _ => {} // Ignore other metadata keys for now
            }
        }
    }

    metadata
}

/// Extract full metadata from script content including typed metadata and schema
///
/// Priority order for metadata extraction:
/// 1. Try typed `metadata = {...}` first (new format)
/// 2. Fall back to `// Name:` comments (legacy format)
///
/// For fields present in typed metadata, those values take precedence.
/// For fields NOT in typed metadata but present in comments, comment values are used.
///
/// Returns (ScriptMetadata, Option<TypedMetadata>, Option<Schema>)
pub fn extract_full_metadata(
    content: &str,
) -> (ScriptMetadata, Option<TypedMetadata>, Option<Schema>) {
    // Extract typed metadata first
    let typed_result = extract_typed_metadata(content);
    let typed_meta = typed_result.metadata;

    // Extract schema
    let schema_result = extract_schema(content);
    let schema = schema_result.schema;

    // Extract comment-based metadata as fallback
    let comment_meta = extract_script_metadata(content);

    // Build final ScriptMetadata, preferring typed values when available
    let script_meta = if let Some(ref typed) = typed_meta {
        ScriptMetadata {
            name: typed.name.clone().or(comment_meta.name),
            description: typed.description.clone().or(comment_meta.description),
            icon: typed.icon.clone().or(comment_meta.icon),
            alias: typed.alias.clone().or(comment_meta.alias),
            shortcut: typed.shortcut.clone().or(comment_meta.shortcut),
        }
    } else {
        comment_meta
    };

    (script_meta, typed_meta, schema)
}

/// Extract metadata from script file comments
/// Looks for lines starting with "// Name:" and "// Description:" with lenient matching
fn extract_metadata(path: &PathBuf) -> ScriptMetadata {
    match fs::read_to_string(path) {
        Ok(content) => extract_script_metadata(&content),
        Err(e) => {
            debug!(
                error = %e,
                path = %path.display(),
                "Could not read script file for metadata extraction"
            );
            ScriptMetadata::default()
        }
    }
}

/// Extract full metadata from a script file path
/// Returns (ScriptMetadata, Option<TypedMetadata>, Option<Schema>)
fn extract_metadata_full(
    path: &PathBuf,
) -> (ScriptMetadata, Option<TypedMetadata>, Option<Schema>) {
    match fs::read_to_string(path) {
        Ok(content) => extract_full_metadata(&content),
        Err(e) => {
            debug!(
                error = %e,
                path = %path.display(),
                "Could not read script file for full metadata extraction"
            );
            (ScriptMetadata::default(), None, None)
        }
    }
}

/// Extract schedule metadata from script content
/// Parses lines looking for "// Cron:" and "// Schedule:" with lenient matching
/// Only checks the first 30 lines of the file
pub fn extract_schedule_metadata(content: &str) -> ScheduleMetadata {
    let mut metadata = ScheduleMetadata::default();

    for line in content.lines().take(30) {
        if let Some((key, value)) = parse_metadata_line(line) {
            match key.to_lowercase().as_str() {
                "cron" => {
                    if metadata.cron.is_none() && !value.is_empty() {
                        metadata.cron = Some(value);
                    }
                }
                "schedule" => {
                    if metadata.schedule.is_none() && !value.is_empty() {
                        metadata.schedule = Some(value);
                    }
                }
                _ => {} // Ignore other metadata keys
            }
        }
    }

    metadata
}

/// Extract schedule metadata from a script file path
pub fn extract_schedule_metadata_from_file(path: &PathBuf) -> ScheduleMetadata {
    match fs::read_to_string(path) {
        Ok(content) => extract_schedule_metadata(&content),
        Err(e) => {
            debug!(
                error = %e,
                path = %path.display(),
                "Could not read script file for schedule metadata extraction"
            );
            ScheduleMetadata::default()
        }
    }
}

/// Extract metadata from HTML comments in scriptlet markdown
/// Looks for <!-- key: value --> patterns
fn extract_html_comment_metadata(text: &str) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut metadata = HashMap::new();

    // Find HTML comment blocks
    if let Some(start) = text.find("<!--") {
        if let Some(end) = text.find("-->") {
            if start < end {
                let comment_content = &text[start + 4..end];
                // Parse key: value pairs
                for line in comment_content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        // Handle format: "key: value"
                        if let Some(colon_pos) = trimmed.find(':') {
                            let key = trimmed[..colon_pos].trim().to_string();
                            let value = trimmed[colon_pos + 1..].trim().to_string();
                            metadata.insert(key, value);
                        }
                    }
                }
            }
        }
    }

    metadata
}

/// Extract code block from markdown text
/// Looks for ```language ... ``` pattern and returns (language, code)
fn extract_code_block(text: &str) -> Option<(String, String)> {
    // Find first code fence
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        // Get the language specifier (rest of line)
        if let Some(newline_pos) = after_fence.find('\n') {
            let language = after_fence[..newline_pos].trim().to_string();
            let code_start = start + 3 + newline_pos + 1;

            // Find closing fence
            if let Some(end_pos) = text[code_start..].find("```") {
                let code = text[code_start..code_start + end_pos].trim().to_string();
                return Some((language, code));
            }
        }
    }
    None
}

/// Convert a name to a command slug (lowercase, spaces/special chars to hyphens)
fn slugify_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Parse a single scriptlet section from markdown
/// Input should be text from ## Name to the next ## or end of file
/// `source_path` is the path to the .md file containing the scriptlet
fn parse_scriptlet_section(
    section: &str,
    source_path: Option<&std::path::Path>,
) -> Option<Scriptlet> {
    let lines: Vec<&str> = section.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // First line should be ## Name
    let first_line = lines[0];
    if !first_line.starts_with("##") {
        return None;
    }

    let name = first_line
        .strip_prefix("##")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if name.is_empty() {
        return None;
    }

    // Extract metadata from HTML comments
    let metadata = extract_html_comment_metadata(section);

    // Extract code block
    let (tool, code) = extract_code_block(section)?;

    // Generate command slug from name
    let command = slugify_name(&name);

    // Build file_path with anchor if source_path is provided
    let file_path = source_path.map(|p| format!("{}#{}", p.display(), command));

    Some(Scriptlet {
        name,
        description: metadata.get("description").cloned(),
        code,
        tool,
        shortcut: metadata.get("shortcut").cloned(),
        expand: metadata.get("expand").cloned(),
        group: None,
        file_path,
        command: Some(command),
        alias: metadata.get("alias").cloned(),
    })
}

/// Reads scriptlets from all *.md files in ~/.kenv/scriptlets/
/// Returns a sorted list of Scriptlet structs parsed from markdown
/// Returns empty vec if directory doesn't exist or is inaccessible
#[instrument(level = "debug", skip_all)]
pub fn read_scriptlets() -> Vec<Scriptlet> {
    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot read scriptlets");
            return vec![];
        }
    };

    let scriptlets_dir = home.join(".kenv/scriptlets");

    // Check if directory exists
    if !scriptlets_dir.exists() {
        debug!(path = %scriptlets_dir.display(), "Scriptlets directory does not exist");
        return vec![];
    }

    let mut scriptlets = Vec::new();

    // Read all .md files in the scriptlets directory
    match fs::read_dir(&scriptlets_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();

                // Only process .md files
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }

                // Skip if not a file
                if !path.is_file() {
                    continue;
                }

                debug!(path = %path.display(), "Reading scriptlets file");

                match fs::read_to_string(&path) {
                    Ok(content) => {
                        // Split by ## headings
                        let mut current_section = String::new();
                        for line in content.lines() {
                            if line.starts_with("##") && !current_section.is_empty() {
                                // Parse previous section
                                if let Some(scriptlet) =
                                    parse_scriptlet_section(&current_section, Some(&path))
                                {
                                    scriptlets.push(scriptlet);
                                }
                                current_section = line.to_string();
                            } else {
                                current_section.push('\n');
                                current_section.push_str(line);
                            }
                        }

                        // Parse the last section
                        if !current_section.is_empty() {
                            if let Some(scriptlet) =
                                parse_scriptlet_section(&current_section, Some(&path))
                            {
                                scriptlets.push(scriptlet);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            error = %e,
                            path = %path.display(),
                            "Failed to read scriptlets file"
                        );
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                path = %scriptlets_dir.display(),
                "Failed to read scriptlets directory"
            );
            return vec![];
        }
    }

    // Sort by name
    scriptlets.sort_by(|a, b| a.name.cmp(&b.name));

    debug!(
        count = scriptlets.len(),
        "Loaded scriptlets from all .md files"
    );
    scriptlets
}

/// Load scriptlets from markdown files using the comprehensive parser
///
/// Globs:
/// - ~/.kenv/scriptlets/*.md (main scriptlets directory)
/// - ~/.kenv/kenvs/*/scriptlets/*.md (nested kenvs)
///
/// Uses `crate::scriptlets::parse_markdown_as_scriptlets` for parsing.
/// Returns scriptlets sorted by group then by name.
#[instrument(level = "debug", skip_all)]
pub fn load_scriptlets() -> Vec<Scriptlet> {
    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot load scriptlets");
            return vec![];
        }
    };

    let mut scriptlets = Vec::new();

    // Glob patterns to search
    let patterns = [
        home.join(".kenv/scriptlets/*.md"),
        home.join(".kenv/kenvs/*/scriptlets/*.md"),
    ];

    for pattern in patterns {
        let pattern_str = pattern.to_string_lossy().to_string();
        debug!(pattern = %pattern_str, "Globbing for scriptlet files");

        match glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths {
                    match entry {
                        Ok(path) => {
                            debug!(path = %path.display(), "Parsing scriptlet file");

                            // Determine kenv from path (for nested kenvs)
                            let kenv = extract_kenv_from_path(&path, &home);

                            match fs::read_to_string(&path) {
                                Ok(content) => {
                                    let path_str = path.to_string_lossy().to_string();
                                    let parsed = scriptlet_parser::parse_markdown_as_scriptlets(
                                        &content,
                                        Some(&path_str),
                                    );

                                    // Convert parsed scriptlets to our Scriptlet format
                                    for parsed_scriptlet in parsed {
                                        let file_path = build_scriptlet_file_path(
                                            &path,
                                            &parsed_scriptlet.command,
                                        );

                                        scriptlets.push(Scriptlet {
                                            name: parsed_scriptlet.name,
                                            description: parsed_scriptlet.metadata.description,
                                            code: parsed_scriptlet.scriptlet_content,
                                            tool: parsed_scriptlet.tool,
                                            shortcut: parsed_scriptlet.metadata.shortcut,
                                            expand: parsed_scriptlet.metadata.expand,
                                            group: if parsed_scriptlet.group.is_empty() {
                                                kenv.clone()
                                            } else {
                                                Some(parsed_scriptlet.group)
                                            },
                                            file_path: Some(file_path),
                                            command: Some(parsed_scriptlet.command),
                                            alias: parsed_scriptlet.metadata.alias,
                                        });
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        error = %e,
                                        path = %path.display(),
                                        "Failed to read scriptlet file"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to process glob entry");
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    pattern = %pattern_str,
                    "Failed to glob scriptlet files"
                );
            }
        }
    }

    // Sort by group first (None last), then by name
    scriptlets.sort_by(|a, b| match (&a.group, &b.group) {
        (Some(g1), Some(g2)) => match g1.cmp(g2) {
            Ordering::Equal => a.name.cmp(&b.name),
            other => other,
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });

    debug!(count = scriptlets.len(), "Loaded scriptlets via parser");
    scriptlets
}

/// Extract kenv name from a nested kenv path
/// e.g., ~/.kenv/kenvs/my-kenv/scriptlets/file.md -> Some("my-kenv")
fn extract_kenv_from_path(path: &std::path::Path, home: &std::path::Path) -> Option<String> {
    let kenvs_prefix = home.join(".kenv/kenvs/");
    let kenvs_prefix_str = kenvs_prefix.to_string_lossy().to_string();
    let path_str = path.to_string_lossy().to_string();

    if path_str.starts_with(&kenvs_prefix_str) {
        // Extract the kenv name from the path
        let relative = &path_str[kenvs_prefix_str.len()..];
        // Find the first slash to get kenv name
        if let Some(slash_pos) = relative.find('/') {
            return Some(relative[..slash_pos].to_string());
        }
    }
    None
}

/// Build the file path with anchor for scriptlet execution
/// Format: /path/to/file.md#slug
fn build_scriptlet_file_path(md_path: &std::path::Path, command: &str) -> String {
    format!("{}#{}", md_path.display(), command)
}

/// Read scriptlets from a single markdown file
///
/// This function parses a single .md file and returns all scriptlets found in it.
/// Used for incremental updates when a scriptlet file changes.
///
/// # Arguments
/// * `path` - Path to the markdown file
///
/// # Returns
/// Vector of Scriptlet structs parsed from the file, or empty vec on error
#[instrument(level = "debug", skip_all, fields(path = %path.display()))]
pub fn read_scriptlets_from_file(path: &std::path::Path) -> Vec<Scriptlet> {
    // Verify it's a markdown file
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        debug!(path = %path.display(), "Not a markdown file, skipping");
        return vec![];
    }

    // Try to get home directory for kenv extraction
    let home = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));

    // Read file content
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                error = %e,
                path = %path.display(),
                "Failed to read scriptlet file"
            );
            return vec![];
        }
    };

    let path_str = path.to_string_lossy().to_string();
    let parsed = scriptlet_parser::parse_markdown_as_scriptlets(&content, Some(&path_str));

    // Determine kenv from path (for nested kenvs)
    let kenv = extract_kenv_from_path(path, &home);

    // Convert parsed scriptlets to our Scriptlet format
    let scriptlets: Vec<Scriptlet> = parsed
        .into_iter()
        .map(|parsed_scriptlet| {
            let file_path = build_scriptlet_file_path(path, &parsed_scriptlet.command);

            Scriptlet {
                name: parsed_scriptlet.name,
                description: parsed_scriptlet.metadata.description,
                code: parsed_scriptlet.scriptlet_content,
                tool: parsed_scriptlet.tool,
                shortcut: parsed_scriptlet.metadata.shortcut,
                expand: parsed_scriptlet.metadata.expand,
                group: if parsed_scriptlet.group.is_empty() {
                    kenv.clone()
                } else {
                    Some(parsed_scriptlet.group)
                },
                file_path: Some(file_path),
                command: Some(parsed_scriptlet.command),
                alias: parsed_scriptlet.metadata.alias,
            }
        })
        .collect();

    debug!(
        count = scriptlets.len(),
        path = %path.display(),
        "Parsed scriptlets from file"
    );

    scriptlets
}

/// Reads scripts from ~/.kenv/scripts directory
/// Returns a sorted list of Script structs for .ts and .js files
/// Returns empty vec if directory doesn't exist or is inaccessible
#[instrument(level = "debug", skip_all)]
pub fn read_scripts() -> Vec<Script> {
    // Expand ~ to home directory using HOME environment variable
    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot read scripts");
            return vec![];
        }
    };

    let scripts_dir = home.join(".kenv/scripts");

    // Check if directory exists
    if !scripts_dir.exists() {
        debug!(path = %scripts_dir.display(), "Scripts directory does not exist");
        return vec![];
    }

    let mut scripts = Vec::new();

    // Read the directory contents
    match std::fs::read_dir(&scripts_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Ok(file_metadata) = entry.metadata() {
                    if file_metadata.is_file() {
                        let path = entry.path();

                        // Check extension
                        if let Some(ext) = path.extension() {
                            if let Some(ext_str) = ext.to_str() {
                                if ext_str == "ts" || ext_str == "js" {
                                    // Get filename without extension as fallback
                                    if let Some(file_name) = path.file_stem() {
                                        if let Some(filename_str) = file_name.to_str() {
                                            // Extract full metadata including typed and schema
                                            let (script_metadata, typed_metadata, schema) =
                                                extract_metadata_full(&path);

                                            // Use metadata name if available, otherwise filename
                                            let name = script_metadata
                                                .name
                                                .unwrap_or_else(|| filename_str.to_string());

                                            scripts.push(Script {
                                                name,
                                                path: path.clone(),
                                                extension: ext_str.to_string(),
                                                description: script_metadata.description,
                                                icon: script_metadata.icon,
                                                alias: script_metadata.alias,
                                                shortcut: script_metadata.shortcut,
                                                typed_metadata,
                                                schema,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                path = %scripts_dir.display(),
                "Failed to read scripts directory"
            );
            return vec![];
        }
    }

    // Sort by name
    scripts.sort_by(|a, b| a.name.cmp(&b.name));

    debug!(count = scripts.len(), "Loaded scripts");
    scripts
}

// ============================================
// ASCII CASE-FOLDING HELPERS (Performance-optimized)
// ============================================
// These functions avoid heap allocations by doing case-insensitive
// comparisons byte-by-byte instead of calling to_lowercase().

/// Check if haystack contains needle using ASCII case-insensitive matching.
/// `needle_lower` must already be lowercase.
/// Returns true if needle is found anywhere in haystack.
/// No allocation - O(n*m) worst case but typically much faster.
#[inline]
fn contains_ignore_ascii_case(haystack: &str, needle_lower: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() {
        return true;
    }
    if n.len() > h.len() {
        return false;
    }
    'outer: for i in 0..=(h.len() - n.len()) {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j] {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

/// Find the position of needle in haystack using ASCII case-insensitive matching.
/// `needle_lower` must already be lowercase.
/// Returns Some(position) if found, None otherwise.
/// No allocation - O(n*m) worst case.
#[inline]
fn find_ignore_ascii_case(haystack: &str, needle_lower: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() {
        return Some(0);
    }
    if n.len() > h.len() {
        return None;
    }
    'outer: for i in 0..=(h.len() - n.len()) {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

/// Perform fuzzy matching without allocating a lowercase copy of haystack.
/// `pattern_lower` must already be lowercase.
/// Returns (matched, indices) where matched is true if all pattern chars found in order.
/// The indices are positions in the original haystack.
#[inline]
fn fuzzy_match_with_indices_ascii(haystack: &str, pattern_lower: &str) -> (bool, Vec<usize>) {
    let mut indices = Vec::new();
    let mut pattern_chars = pattern_lower.chars().peekable();

    for (idx, ch) in haystack.chars().enumerate() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.to_ascii_lowercase() == p {
                indices.push(idx);
                pattern_chars.next();
            }
        }
    }

    let matched = pattern_chars.peek().is_none();
    (matched, if matched { indices } else { Vec::new() })
}

/// Check if a pattern is a fuzzy match for haystack (characters appear in order)
fn is_fuzzy_match(haystack: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    for ch in haystack.chars() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.eq_ignore_ascii_case(&p) {
                pattern_chars.next();
            }
        }
    }
    pattern_chars.peek().is_none()
}

/// Perform fuzzy matching and return the indices of matched characters
/// Returns (matched, indices) where matched is true if all pattern chars found in order
fn fuzzy_match_with_indices(haystack: &str, pattern: &str) -> (bool, Vec<usize>) {
    let mut indices = Vec::new();
    let mut pattern_chars = pattern.chars().peekable();

    for (idx, ch) in haystack.chars().enumerate() {
        if let Some(&p) = pattern_chars.peek() {
            if ch.eq_ignore_ascii_case(&p) {
                indices.push(idx);
                pattern_chars.next();
            }
        }
    }

    let matched = pattern_chars.peek().is_none();
    (matched, if matched { indices } else { Vec::new() })
}

/// Score a haystack against a nucleo pattern.
/// Returns Some(score) if the pattern matches, None otherwise.
/// Score range is typically 0-1000+ (higher = better match).
#[inline]
fn nucleo_score(haystack: &str, pattern: &Pattern, matcher: &mut Matcher) -> Option<u32> {
    let mut haystack_buf = Vec::new();
    let haystack_utf32 = Utf32Str::new(haystack, &mut haystack_buf);
    pattern.score(haystack_utf32, matcher)
}

/// Compute match indices for a search result on-demand (lazy evaluation)
///
/// This function is called by the UI layer only for visible rows, avoiding
/// the cost of computing indices for all results during the scoring phase.
///
/// # Arguments
/// * `result` - The search result to compute indices for
/// * `query` - The original search query (will be lowercased internally)
///
/// # Returns
/// MatchIndices containing the character positions that match the query
pub fn compute_match_indices_for_result(result: &SearchResult, query: &str) -> MatchIndices {
    if query.is_empty() {
        return MatchIndices::default();
    }

    let query_lower = query.to_lowercase();

    match result {
        SearchResult::Script(sm) => {
            let mut indices = MatchIndices::default();

            // Try name first
            let (name_matched, name_indices) =
                fuzzy_match_with_indices_ascii(&sm.script.name, &query_lower);
            if name_matched {
                indices.name_indices = name_indices;
                return indices;
            }

            // Fall back to filename
            let (filename_matched, filename_indices) =
                fuzzy_match_with_indices_ascii(&sm.filename, &query_lower);
            if filename_matched {
                indices.filename_indices = filename_indices;
            }

            indices
        }
        SearchResult::Scriptlet(sm) => {
            let mut indices = MatchIndices::default();

            // Try name first
            let (name_matched, name_indices) =
                fuzzy_match_with_indices_ascii(&sm.scriptlet.name, &query_lower);
            if name_matched {
                indices.name_indices = name_indices;
                return indices;
            }

            // Fall back to file path
            if let Some(ref fp) = sm.display_file_path {
                let (fp_matched, fp_indices) = fuzzy_match_with_indices_ascii(fp, &query_lower);
                if fp_matched {
                    indices.filename_indices = fp_indices;
                }
            }

            indices
        }
        SearchResult::BuiltIn(bm) => {
            let mut indices = MatchIndices::default();

            let (name_matched, name_indices) =
                fuzzy_match_with_indices_ascii(&bm.entry.name, &query_lower);
            if name_matched {
                indices.name_indices = name_indices;
            }

            indices
        }
        SearchResult::App(am) => {
            let mut indices = MatchIndices::default();

            let (name_matched, name_indices) =
                fuzzy_match_with_indices_ascii(&am.app.name, &query_lower);
            if name_matched {
                indices.name_indices = name_indices;
            }

            indices
        }
        SearchResult::Window(wm) => {
            let mut indices = MatchIndices::default();

            // Try app name first, then title
            let (app_matched, app_indices) =
                fuzzy_match_with_indices_ascii(&wm.window.app, &query_lower);
            if app_matched {
                indices.name_indices = app_indices;
                return indices;
            }

            let (title_matched, title_indices) =
                fuzzy_match_with_indices_ascii(&wm.window.title, &query_lower);
            if title_matched {
                indices.filename_indices = title_indices;
            }

            indices
        }
    }
}

/// Extract filename from a path for display
fn extract_filename(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Extract display-friendly file path from scriptlet file_path
/// Converts "/path/to/file.md#slug" to "file.md#slug"
fn extract_scriptlet_display_path(file_path: &Option<String>) -> Option<String> {
    file_path.as_ref().map(|fp| {
        // Split on # to get path and anchor
        let parts: Vec<&str> = fp.splitn(2, '#').collect();
        let path_part = parts[0];
        let anchor = parts.get(1);

        // Extract just the filename from the path
        let filename = std::path::Path::new(path_part)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_part);

        // Reconstruct with anchor if present
        match anchor {
            Some(a) => format!("{}#{}", filename, a),
            None => filename.to_string(),
        }
    })
}

/// Fuzzy search scripts by query string
/// Searches across name, filename (e.g., "my-script.ts"), description, and path
/// Returns results sorted by relevance score (highest first)
/// Match indices are provided to enable UI highlighting of matched characters
pub fn fuzzy_search_scripts(scripts: &[Script], query: &str) -> Vec<ScriptMatch> {
    if query.is_empty() {
        // If no query, return all scripts with equal score, sorted by name
        return scripts
            .iter()
            .map(|s| {
                let filename = extract_filename(&s.path);
                ScriptMatch {
                    script: s.clone(),
                    score: 0,
                    filename,
                    match_indices: MatchIndices::default(),
                }
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    // Create nucleo pattern and matcher once for all scripts
    let pattern = Pattern::parse(
        &query_lower,
        nucleo_matcher::pattern::CaseMatching::Ignore,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    for script in scripts {
        let mut score = 0i32;
        // Lazy match indices - don't compute during scoring, will be computed on-demand
        let match_indices = MatchIndices::default();

        let filename = extract_filename(&script.path);

        // Score by name match - highest priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&script.name, &query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name using nucleo
        if let Some(nucleo_s) = nucleo_score(&script.name, &pattern, &mut matcher) {
            // Scale nucleo score (0-1000+) to match existing weights (~50 for fuzzy match)
            score += 50 + (nucleo_s / 20) as i32;
        }

        // Score by filename match - high priority (allows searching by ".ts", ".js", etc.)
        if let Some(pos) = find_ignore_ascii_case(&filename, &query_lower) {
            // Bonus for exact substring match at start of filename
            score += if pos == 0 { 60 } else { 45 };
        }

        // Fuzzy character matching in filename using nucleo
        if let Some(nucleo_s) = nucleo_score(&filename, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~35 for filename fuzzy match)
            score += 35 + (nucleo_s / 30) as i32;
        }

        // Score by description match - medium priority (no allocation)
        if let Some(ref desc) = script.description {
            if contains_ignore_ascii_case(desc, &query_lower) {
                score += 25;
            }
        }

        // Score by path match - lower priority (no allocation for lowercase)
        let path_str = script.path.to_string_lossy();
        if contains_ignore_ascii_case(&path_str, &query_lower) {
            score += 10;
        }

        if score > 0 {
            matches.push(ScriptMatch {
                script: script.clone(),
                score,
                filename,
                match_indices,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => a.script.name.cmp(&b.script.name),
        other => other,
    });

    matches
}

/// Fuzzy search scriptlets by query string
/// Searches across name, file_path with anchor (e.g., "url.md#open-github"), description, and code
/// Returns results sorted by relevance score (highest first)
/// Match indices are provided to enable UI highlighting of matched characters
pub fn fuzzy_search_scriptlets(scriptlets: &[Scriptlet], query: &str) -> Vec<ScriptletMatch> {
    if query.is_empty() {
        // If no query, return all scriptlets with equal score, sorted by name
        return scriptlets
            .iter()
            .map(|s| {
                let display_file_path = extract_scriptlet_display_path(&s.file_path);
                ScriptletMatch {
                    scriptlet: s.clone(),
                    score: 0,
                    display_file_path,
                    match_indices: MatchIndices::default(),
                }
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    // Create nucleo pattern and matcher once for all scriptlets
    let pattern = Pattern::parse(
        &query_lower,
        nucleo_matcher::pattern::CaseMatching::Ignore,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    for scriptlet in scriptlets {
        let mut score = 0i32;
        // Lazy match indices - don't compute during scoring
        let match_indices = MatchIndices::default();

        let display_file_path = extract_scriptlet_display_path(&scriptlet.file_path);

        // Score by name match - highest priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&scriptlet.name, &query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name using nucleo
        if let Some(nucleo_s) = nucleo_score(&scriptlet.name, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~50 for fuzzy match)
            score += 50 + (nucleo_s / 20) as i32;
        }

        // Score by file_path match - high priority (allows searching by ".md", anchor names)
        if let Some(ref fp) = display_file_path {
            if let Some(pos) = find_ignore_ascii_case(fp, &query_lower) {
                // Bonus for exact substring match at start of file_path
                score += if pos == 0 { 60 } else { 45 };
            }

            // Fuzzy character matching in file_path using nucleo
            if let Some(nucleo_s) = nucleo_score(fp, &pattern, &mut matcher) {
                // Scale nucleo score to match existing weights (~35 for file_path fuzzy match)
                score += 35 + (nucleo_s / 30) as i32;
            }
        }

        // Score by description match - medium priority (no allocation)
        if let Some(ref desc) = scriptlet.description {
            if contains_ignore_ascii_case(desc, &query_lower) {
                score += 25;
            }
        }

        // CRITICAL OPTIMIZATION: Only search code when query is long enough (>=4 chars)
        // and no other matches were found. Code search is the biggest performance hit
        // because scriptlet.code can be very large.
        if query_lower.len() >= 4
            && score == 0
            && contains_ignore_ascii_case(&scriptlet.code, &query_lower)
        {
            score += 5;
        }

        // Bonus for tool type match (no allocation)
        if contains_ignore_ascii_case(&scriptlet.tool, &query_lower) {
            score += 10;
        }

        if score > 0 {
            matches.push(ScriptletMatch {
                scriptlet: scriptlet.clone(),
                score,
                display_file_path,
                match_indices,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => a.scriptlet.name.cmp(&b.scriptlet.name),
        other => other,
    });

    matches
}

/// Fuzzy search built-in entries by query string
/// Searches across name, description, and keywords
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_builtins(entries: &[BuiltInEntry], query: &str) -> Vec<BuiltInMatch> {
    if query.is_empty() {
        // If no query, return all entries with equal score, sorted by name
        return entries
            .iter()
            .map(|e| BuiltInMatch {
                entry: e.clone(),
                score: 0,
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    // Create nucleo pattern and matcher once for all entries
    let pattern = Pattern::parse(
        &query_lower,
        nucleo_matcher::pattern::CaseMatching::Ignore,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    for entry in entries {
        let mut score = 0i32;

        // Score by name match - highest priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&entry.name, &query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name using nucleo
        if let Some(nucleo_s) = nucleo_score(&entry.name, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~50 for fuzzy match)
            score += 50 + (nucleo_s / 20) as i32;
        }

        // Score by description match - medium priority (no allocation)
        if contains_ignore_ascii_case(&entry.description, &query_lower) {
            score += 25;
        }

        // Score by keyword match - high priority (keywords are designed for matching)
        for keyword in &entry.keywords {
            if contains_ignore_ascii_case(keyword, &query_lower) {
                score += 75; // Keywords are specifically meant for matching
                break; // Only count once even if multiple keywords match
            }
        }

        // Fuzzy match on keywords using nucleo
        for keyword in &entry.keywords {
            if let Some(nucleo_s) = nucleo_score(keyword, &pattern, &mut matcher) {
                // Scale nucleo score to match existing weights (~30 for keyword fuzzy match)
                score += 30 + (nucleo_s / 30) as i32;
                break; // Only count once
            }
        }

        if score > 0 {
            matches.push(BuiltInMatch {
                entry: entry.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => a.entry.name.cmp(&b.entry.name),
        other => other,
    });

    matches
}

/// Fuzzy search applications by query string
/// Searches across name and bundle_id
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_apps(apps: &[crate::app_launcher::AppInfo], query: &str) -> Vec<AppMatch> {
    if query.is_empty() {
        // If no query, return all apps with equal score, sorted by name
        return apps
            .iter()
            .map(|a| AppMatch {
                app: a.clone(),
                score: 0,
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    // Create nucleo pattern and matcher once for all apps
    let pattern = Pattern::parse(
        &query_lower,
        nucleo_matcher::pattern::CaseMatching::Ignore,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    for app in apps {
        let mut score = 0i32;

        // Score by name match - highest priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&app.name, &query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name using nucleo
        if let Some(nucleo_s) = nucleo_score(&app.name, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~50 for fuzzy match)
            score += 50 + (nucleo_s / 20) as i32;
        }

        // Score by bundle_id match - lower priority (no allocation)
        if let Some(ref bundle_id) = app.bundle_id {
            if contains_ignore_ascii_case(bundle_id, &query_lower) {
                score += 15;
            }
        }

        // Score by path match - lowest priority (no allocation for lowercase)
        let path_str = app.path.to_string_lossy();
        if contains_ignore_ascii_case(&path_str, &query_lower) {
            score += 5;
        }

        if score > 0 {
            matches.push(AppMatch {
                app: app.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => a.app.name.cmp(&b.app.name),
        other => other,
    });

    matches
}

/// Fuzzy search windows by query string
/// Searches across app name and window title
/// Returns results sorted by relevance score (highest first)
///
/// Scoring priorities:
/// - App name match at start: 100 points
/// - App name match elsewhere: 75 points
/// - Window title match at start: 90 points  
/// - Window title match elsewhere: 65 points
/// - Fuzzy match on app name: 50 points
/// - Fuzzy match on window title: 40 points
pub fn fuzzy_search_windows(
    windows: &[crate::window_control::WindowInfo],
    query: &str,
) -> Vec<WindowMatch> {
    if query.is_empty() {
        // If no query, return all windows with equal score, sorted by app name then title
        let mut matches: Vec<WindowMatch> = windows
            .iter()
            .map(|w| WindowMatch {
                window: w.clone(),
                score: 0,
            })
            .collect();
        matches.sort_by(|a, b| match a.window.app.cmp(&b.window.app) {
            Ordering::Equal => a.window.title.cmp(&b.window.title),
            other => other,
        });
        return matches;
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    // Create nucleo pattern and matcher once for all windows
    let pattern = Pattern::parse(
        &query_lower,
        nucleo_matcher::pattern::CaseMatching::Ignore,
        nucleo_matcher::pattern::Normalization::Smart,
    );
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);

    for window in windows {
        let mut score = 0i32;

        // Score by app name match - highest priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&window.app, &query_lower) {
            // Bonus for exact substring match at start of app name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Score by window title match - high priority (no allocation)
        if let Some(pos) = find_ignore_ascii_case(&window.title, &query_lower) {
            // Bonus for exact substring match at start of title
            score += if pos == 0 { 90 } else { 65 };
        }

        // Fuzzy character matching in app name using nucleo
        if let Some(nucleo_s) = nucleo_score(&window.app, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~50 for app name fuzzy match)
            score += 50 + (nucleo_s / 20) as i32;
        }

        // Fuzzy character matching in window title using nucleo
        if let Some(nucleo_s) = nucleo_score(&window.title, &pattern, &mut matcher) {
            // Scale nucleo score to match existing weights (~40 for title fuzzy match)
            score += 40 + (nucleo_s / 25) as i32;
        }

        if score > 0 {
            matches.push(WindowMatch {
                window: window.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by app name, then by title for ties
    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => match a.window.app.cmp(&b.window.app) {
            Ordering::Equal => a.window.title.cmp(&b.window.title),
            other => other,
        },
        other => other,
    });

    matches
}

/// Perform unified fuzzy search across scripts, scriptlets, and built-ins
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
pub fn fuzzy_search_unified(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    query: &str,
) -> Vec<SearchResult> {
    fuzzy_search_unified_with_builtins(scripts, scriptlets, &[], query)
}

/// Perform unified fuzzy search across scripts, scriptlets, and built-ins
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
pub fn fuzzy_search_unified_with_builtins(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    builtins: &[BuiltInEntry],
    query: &str,
) -> Vec<SearchResult> {
    // Use the new function with empty apps list for backwards compatibility
    fuzzy_search_unified_all(scripts, scriptlets, builtins, &[], query)
}

/// Perform unified fuzzy search across scripts, scriptlets, built-ins, and apps
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
/// Apps appear after built-ins but before scripts when scores are equal
pub fn fuzzy_search_unified_all(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    builtins: &[BuiltInEntry],
    apps: &[crate::app_launcher::AppInfo],
    query: &str,
) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Search built-ins first (they should appear at top when scores are equal)
    let builtin_matches = fuzzy_search_builtins(builtins, query);
    for bm in builtin_matches {
        results.push(SearchResult::BuiltIn(bm));
    }

    // Search apps (appear after built-ins but before scripts)
    let app_matches = fuzzy_search_apps(apps, query);
    for am in app_matches {
        results.push(SearchResult::App(am));
    }

    // Search scripts
    let script_matches = fuzzy_search_scripts(scripts, query);
    for sm in script_matches {
        results.push(SearchResult::Script(sm));
    }

    // Search scriptlets
    let scriptlet_matches = fuzzy_search_scriptlets(scriptlets, query);
    for sm in scriptlet_matches {
        results.push(SearchResult::Scriptlet(sm));
    }

    // Sort by score (highest first), then by type (builtins first, apps, windows, scripts, scriptlets), then by name
    results.sort_by(|a, b| {
        match b.score().cmp(&a.score()) {
            Ordering::Equal => {
                // Prefer builtins over apps over windows over scripts over scriptlets when scores are equal
                let type_order = |r: &SearchResult| -> i32 {
                    match r {
                        SearchResult::BuiltIn(_) => 0, // Built-ins first
                        SearchResult::App(_) => 1,     // Apps second
                        SearchResult::Window(_) => 2,  // Windows third
                        SearchResult::Script(_) => 3,
                        SearchResult::Scriptlet(_) => 4,
                    }
                };
                let type_order_a = type_order(a);
                let type_order_b = type_order(b);
                match type_order_a.cmp(&type_order_b) {
                    Ordering::Equal => a.name().cmp(b.name()),
                    other => other,
                }
            }
            other => other,
        }
    });

    results
}

/// Perform unified fuzzy search across scripts, scriptlets, built-ins, apps, and windows
/// Returns combined and ranked results sorted by relevance
/// Order by type when scores are equal: Built-ins > Apps > Windows > Scripts > Scriptlets
pub fn fuzzy_search_unified_with_windows(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    builtins: &[BuiltInEntry],
    apps: &[crate::app_launcher::AppInfo],
    windows: &[crate::window_control::WindowInfo],
    query: &str,
) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Search built-ins first (they should appear at top when scores are equal)
    let builtin_matches = fuzzy_search_builtins(builtins, query);
    for bm in builtin_matches {
        results.push(SearchResult::BuiltIn(bm));
    }

    // Search apps (appear after built-ins)
    let app_matches = fuzzy_search_apps(apps, query);
    for am in app_matches {
        results.push(SearchResult::App(am));
    }

    // Search windows (appear after apps)
    let window_matches = fuzzy_search_windows(windows, query);
    for wm in window_matches {
        results.push(SearchResult::Window(wm));
    }

    // Search scripts
    let script_matches = fuzzy_search_scripts(scripts, query);
    for sm in script_matches {
        results.push(SearchResult::Script(sm));
    }

    // Search scriptlets
    let scriptlet_matches = fuzzy_search_scriptlets(scriptlets, query);
    for sm in scriptlet_matches {
        results.push(SearchResult::Scriptlet(sm));
    }

    // Sort by score (highest first), then by type (builtins first, apps, windows, scripts, scriptlets), then by name
    results.sort_by(|a, b| {
        match b.score().cmp(&a.score()) {
            Ordering::Equal => {
                // Prefer builtins over apps over windows over scripts over scriptlets when scores are equal
                let type_order = |r: &SearchResult| -> i32 {
                    match r {
                        SearchResult::BuiltIn(_) => 0, // Built-ins first
                        SearchResult::App(_) => 1,     // Apps second
                        SearchResult::Window(_) => 2,  // Windows third
                        SearchResult::Script(_) => 3,
                        SearchResult::Scriptlet(_) => 4,
                    }
                };
                let type_order_a = type_order(a);
                let type_order_b = type_order(b);
                match type_order_a.cmp(&type_order_b) {
                    Ordering::Equal => a.name().cmp(b.name()),
                    other => other,
                }
            }
            other => other,
        }
    });

    results
}

/// Default maximum number of items to show in the RECENT section
pub const DEFAULT_MAX_RECENT_ITEMS: usize = 10;

/// Get grouped results with RECENT/MAIN sections based on frecency
///
/// This function creates a grouped view of search results:
///
/// **When filter_text is empty (grouped view):**
/// 1. Returns `SectionHeader("RECENT")` if any items have frecency score > 0
/// 2. Recent items sorted by frecency score (top 5-10 with score > 0)
/// 3. Returns `SectionHeader("MAIN")`
/// 4. Remaining items sorted alphabetically by name
///
/// **When filter_text has content (search mode):**
/// - Returns flat list of `Item(index)` - no headers
/// - Uses existing fuzzy_search_unified logic for filtering
///
/// # Arguments
/// * `scripts` - Scripts to include in results
/// * `scriptlets` - Scriptlets to include in results
/// * `builtins` - Built-in entries to include in results
/// * `apps` - Application entries to include in results
/// * `frecency_store` - Store containing frecency data for ranking
/// * `filter_text` - Search filter text (empty = grouped view, non-empty = search mode)
/// * `max_recent_items` - Maximum items to show in RECENT section (from config)
///
/// # Returns
/// `(Vec<GroupedListItem>, Vec<SearchResult>)` - Grouped items and the flat results array.
/// The `usize` in `Item(usize)` is the index into the flat results array.
///
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
pub fn get_grouped_results(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    frecency_store: &FrecencyStore,
    filter_text: &str,
    max_recent_items: usize,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    // Get all unified search results
    let results = fuzzy_search_unified_all(scripts, scriptlets, builtins, apps, filter_text);

    // Search mode: return flat list with no headers
    if !filter_text.is_empty() {
        let grouped: Vec<GroupedListItem> = (0..results.len()).map(GroupedListItem::Item).collect();
        debug!(
            result_count = results.len(),
            "Search mode: returning flat list"
        );
        return (grouped, results);
    }

    // Grouped view mode: create RECENT and MAIN sections
    let mut grouped = Vec::new();

    // Get recent items from frecency store
    let recent_items = frecency_store.get_recent_items(max_recent_items);

    // Build a set of paths that are "recent" (have frecency score > 0)
    let recent_paths: std::collections::HashSet<String> = recent_items
        .iter()
        .filter(|(_, score): &&(String, f64)| *score > 0.0)
        .map(|(path, _): &(String, f64)| path.clone())
        .collect();

    // Map each result to its frecency score (if any)
    // We need to get the path for each result type
    let get_result_path = |result: &SearchResult| -> Option<String> {
        match result {
            SearchResult::Script(sm) => Some(sm.script.path.to_string_lossy().to_string()),
            SearchResult::App(am) => Some(am.app.path.to_string_lossy().to_string()),
            SearchResult::BuiltIn(bm) => Some(format!("builtin:{}", bm.entry.name)),
            SearchResult::Scriptlet(sm) => Some(format!("scriptlet:{}", sm.scriptlet.name)),
            SearchResult::Window(wm) => {
                Some(format!("window:{}:{}", wm.window.app, wm.window.title))
            }
        }
    };

    // Find indices of results that are "recent"
    let mut recent_indices: Vec<(usize, f64)> = Vec::new();
    let mut main_indices: Vec<usize> = Vec::new();

    for (idx, result) in results.iter().enumerate() {
        if let Some(path) = get_result_path(result) {
            let score = frecency_store.get_score(&path);
            if score > 0.0 && recent_paths.contains(&path) {
                recent_indices.push((idx, score));
            } else {
                main_indices.push(idx);
            }
        } else {
            main_indices.push(idx);
        }
    }

    // Sort recent items by frecency score (highest first)
    recent_indices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Limit recent items to max_recent_items
    recent_indices.truncate(max_recent_items);

    // Sort main items alphabetically by name (case-insensitive)
    main_indices.sort_by(|&a, &b| {
        results[a]
            .name()
            .to_lowercase()
            .cmp(&results[b].name().to_lowercase())
    });

    // Build grouped list
    if !recent_indices.is_empty() {
        grouped.push(GroupedListItem::SectionHeader("RECENT".to_string()));
        for (idx, _score) in &recent_indices {
            grouped.push(GroupedListItem::Item(*idx));
        }
    }

    // Always add MAIN section if there are any items
    if !main_indices.is_empty() || !recent_indices.is_empty() {
        grouped.push(GroupedListItem::SectionHeader("MAIN".to_string()));
        for idx in main_indices {
            grouped.push(GroupedListItem::Item(idx));
        }
    }

    debug!(
        recent_count = recent_indices.len(),
        main_count = grouped
            .len()
            .saturating_sub(recent_indices.len())
            .saturating_sub(if recent_indices.is_empty() { 1 } else { 2 }),
        total_grouped = grouped.len(),
        "Grouped view: created RECENT/MAIN sections"
    );

    (grouped, results)
}

/// Scan scripts directory and register scripts with schedule metadata
///
/// Walks through ~/.kenv/scripts/ looking for .ts/.js files with
/// `// Cron:` or `// Schedule:` metadata comments, and registers them
/// with the provided scheduler.
///
/// Returns the count of scripts successfully registered.
#[instrument(level = "debug", skip(scheduler))]
pub fn register_scheduled_scripts(scheduler: &crate::scheduler::Scheduler) -> usize {
    use tracing::info;

    let home = match env::var("HOME") {
        Ok(home_path) => PathBuf::from(home_path),
        Err(e) => {
            warn!(error = %e, "HOME environment variable not set, cannot scan for scheduled scripts");
            return 0;
        }
    };

    let scripts_dir = home.join(".kenv/scripts");

    if !scripts_dir.exists() {
        debug!(path = %scripts_dir.display(), "Scripts directory does not exist");
        return 0;
    }

    let mut registered_count = 0;

    match fs::read_dir(&scripts_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Ok(file_metadata) = entry.metadata() {
                    if file_metadata.is_file() {
                        let path = entry.path();

                        // Only process .ts and .js files
                        let is_script = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|ext| ext == "ts" || ext == "js")
                            .unwrap_or(false);

                        if !is_script {
                            continue;
                        }

                        // Extract schedule metadata
                        let schedule_meta = extract_schedule_metadata_from_file(&path);

                        // Skip if no schedule metadata
                        if schedule_meta.cron.is_none() && schedule_meta.schedule.is_none() {
                            continue;
                        }

                        // Register with scheduler
                        match scheduler.add_script(
                            path.clone(),
                            schedule_meta.cron.clone(),
                            schedule_meta.schedule.clone(),
                        ) {
                            Ok(()) => {
                                registered_count += 1;
                                info!(
                                    path = %path.display(),
                                    cron = ?schedule_meta.cron,
                                    schedule = ?schedule_meta.schedule,
                                    "Registered scheduled script"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    path = %path.display(),
                                    "Failed to register scheduled script"
                                );
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!(
                error = %e,
                path = %scripts_dir.display(),
                "Failed to read scripts directory for scheduling"
            );
        }
    }

    if registered_count > 0 {
        info!(count = registered_count, "Registered scheduled scripts");
    }

    registered_count
}

#[cfg(test)]
#[path = "scripts_tests.rs"]
mod tests;
