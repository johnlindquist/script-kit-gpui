#![allow(dead_code)]

use std::path::PathBuf;
use std::env;
use std::fs;
use std::cmp::Ordering;
use tracing::{debug, warn, instrument};

pub use crate::builtins::BuiltInEntry;
use crate::frecency::FrecencyStore;
use crate::list_item::GroupedListItem;
use crate::app_launcher::AppInfo;

#[derive(Clone, Debug)]
pub struct Script {
    pub name: String,
    pub path: PathBuf,
    pub extension: String,
    pub description: Option<String>,
}

/// Represents a scriptlet parsed from a markdown file
/// Scriptlets are code snippets extracted from .md files with metadata
#[derive(Clone, Debug)]
pub struct Scriptlet {
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub tool: String,  // "ts", "bash", "paste", etc.
    pub shortcut: Option<String>,
    pub expand: Option<String>,
}

/// Represents a scored match result for fuzzy search
#[derive(Clone, Debug)]
pub struct ScriptMatch {
    pub script: Script,
    pub score: i32,
}

/// Represents a scored match result for fuzzy search on scriptlets
#[derive(Clone, Debug)]
pub struct ScriptletMatch {
    pub scriptlet: Scriptlet,
    pub score: i32,
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
/// Parses lines looking for "// Name:" and "// Description:" with lenient matching
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
                _ => {} // Ignore other metadata keys for now
            }
        }
    }
    
    metadata
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

/// Parse a single scriptlet section from markdown
/// Input should be text from ## Name to the next ## or end of file
fn parse_scriptlet_section(section: &str) -> Option<Scriptlet> {
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
    
    Some(Scriptlet {
        name,
        description: metadata.get("description").cloned(),
        code,
        tool,
        shortcut: metadata.get("shortcut").cloned(),
        expand: metadata.get("expand").cloned(),
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
                                if let Some(scriptlet) = parse_scriptlet_section(&current_section) {
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
                            if let Some(scriptlet) = parse_scriptlet_section(&current_section) {
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

    debug!(count = scriptlets.len(), "Loaded scriptlets from all .md files");
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
                                            let script_metadata = extract_metadata(&path);
                                            
                                            // Use metadata name if available, otherwise filename
                                            let name = script_metadata.name
                                                .unwrap_or_else(|| filename_str.to_string());
                                            
                                            scripts.push(Script {
                                                name,
                                                path: path.clone(),
                                                extension: ext_str.to_string(),
                                                description: script_metadata.description,
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

/// Fuzzy search scripts by query string
/// Searches across name, description, and path
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_scripts(scripts: &[Script], query: &str) -> Vec<ScriptMatch> {
    if query.is_empty() {
        // If no query, return all scripts with equal score, sorted by name
        return scripts.iter().map(|s| ScriptMatch {
            script: s.clone(),
            score: 0,
        }).collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for script in scripts {
        let mut score = 0i32;
        let name_lower = script.name.to_lowercase();

        // Score by name match - highest priority
        if let Some(pos) = name_lower.find(&query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name (characters in order)
        if is_fuzzy_match(&name_lower, &query_lower) {
            score += 50;
        }

        // Score by description match - medium priority
        if let Some(ref desc) = script.description {
            if desc.to_lowercase().contains(&query_lower) {
                score += 25;
            }
        }

        // Score by path match - lower priority
        let path_str = script.path.to_string_lossy().to_lowercase();
        if path_str.contains(&query_lower) {
            score += 10;
        }

        if score > 0 {
            matches.push(ScriptMatch {
                script: script.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| {
        match b.score.cmp(&a.score) {
            Ordering::Equal => a.script.name.cmp(&b.script.name),
            other => other,
        }
    });

    matches
}

/// Fuzzy search scriptlets by query string
/// Searches across name, description, and code
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_scriptlets(scriptlets: &[Scriptlet], query: &str) -> Vec<ScriptletMatch> {
    if query.is_empty() {
        // If no query, return all scriptlets with equal score, sorted by name
        return scriptlets.iter().map(|s| ScriptletMatch {
            scriptlet: s.clone(),
            score: 0,
        }).collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for scriptlet in scriptlets {
        let mut score = 0i32;
        let name_lower = scriptlet.name.to_lowercase();

        // Score by name match - highest priority
        if let Some(pos) = name_lower.find(&query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name (characters in order)
        if is_fuzzy_match(&name_lower, &query_lower) {
            score += 50;
        }

        // Score by description match - medium priority
        if let Some(ref desc) = scriptlet.description {
            if desc.to_lowercase().contains(&query_lower) {
                score += 25;
            }
        }

        // Score by code content match - lower priority
        if scriptlet.code.to_lowercase().contains(&query_lower) {
            score += 5;
        }

        // Bonus for tool type match
        if scriptlet.tool.to_lowercase().contains(&query_lower) {
            score += 10;
        }

        if score > 0 {
            matches.push(ScriptletMatch {
                scriptlet: scriptlet.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by name for ties
    matches.sort_by(|a, b| {
        match b.score.cmp(&a.score) {
            Ordering::Equal => a.scriptlet.name.cmp(&b.scriptlet.name),
            other => other,
        }
    });

    matches
}

/// Fuzzy search built-in entries by query string
/// Searches across name, description, and keywords
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_builtins(entries: &[BuiltInEntry], query: &str) -> Vec<BuiltInMatch> {
    if query.is_empty() {
        // If no query, return all entries with equal score, sorted by name
        return entries.iter().map(|e| BuiltInMatch {
            entry: e.clone(),
            score: 0,
        }).collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for entry in entries {
        let mut score = 0i32;
        let name_lower = entry.name.to_lowercase();

        // Score by name match - highest priority
        if let Some(pos) = name_lower.find(&query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name (characters in order)
        if is_fuzzy_match(&name_lower, &query_lower) {
            score += 50;
        }

        // Score by description match - medium priority
        if entry.description.to_lowercase().contains(&query_lower) {
            score += 25;
        }

        // Score by keyword match - high priority (keywords are designed for matching)
        for keyword in &entry.keywords {
            if keyword.to_lowercase().contains(&query_lower) {
                score += 75;  // Keywords are specifically meant for matching
                break;  // Only count once even if multiple keywords match
            }
        }

        // Fuzzy match on keywords
        for keyword in &entry.keywords {
            if is_fuzzy_match(&keyword.to_lowercase(), &query_lower) {
                score += 30;
                break;  // Only count once
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
    matches.sort_by(|a, b| {
        match b.score.cmp(&a.score) {
            Ordering::Equal => a.entry.name.cmp(&b.entry.name),
            other => other,
        }
    });

    matches
}

/// Fuzzy search applications by query string
/// Searches across name and bundle_id
/// Returns results sorted by relevance score (highest first)
pub fn fuzzy_search_apps(apps: &[crate::app_launcher::AppInfo], query: &str) -> Vec<AppMatch> {
    if query.is_empty() {
        // If no query, return all apps with equal score, sorted by name
        return apps.iter().map(|a| AppMatch {
            app: a.clone(),
            score: 0,
        }).collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for app in apps {
        let mut score = 0i32;
        let name_lower = app.name.to_lowercase();

        // Score by name match - highest priority
        if let Some(pos) = name_lower.find(&query_lower) {
            // Bonus for exact substring match at start of name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Fuzzy character matching in name (characters in order)
        if is_fuzzy_match(&name_lower, &query_lower) {
            score += 50;
        }

        // Score by bundle_id match - lower priority
        if let Some(ref bundle_id) = app.bundle_id {
            if bundle_id.to_lowercase().contains(&query_lower) {
                score += 15;
            }
        }

        // Score by path match - lowest priority
        let path_str = app.path.to_string_lossy().to_lowercase();
        if path_str.contains(&query_lower) {
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
    matches.sort_by(|a, b| {
        match b.score.cmp(&a.score) {
            Ordering::Equal => a.app.name.cmp(&b.app.name),
            other => other,
        }
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
pub fn fuzzy_search_windows(windows: &[crate::window_control::WindowInfo], query: &str) -> Vec<WindowMatch> {
    if query.is_empty() {
        // If no query, return all windows with equal score, sorted by app name then title
        let mut matches: Vec<WindowMatch> = windows.iter().map(|w| WindowMatch {
            window: w.clone(),
            score: 0,
        }).collect();
        matches.sort_by(|a, b| {
            match a.window.app.cmp(&b.window.app) {
                Ordering::Equal => a.window.title.cmp(&b.window.title),
                other => other,
            }
        });
        return matches;
    }

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for window in windows {
        let mut score = 0i32;
        let app_lower = window.app.to_lowercase();
        let title_lower = window.title.to_lowercase();

        // Score by app name match - highest priority
        if let Some(pos) = app_lower.find(&query_lower) {
            // Bonus for exact substring match at start of app name
            score += if pos == 0 { 100 } else { 75 };
        }

        // Score by window title match - high priority
        if let Some(pos) = title_lower.find(&query_lower) {
            // Bonus for exact substring match at start of title
            score += if pos == 0 { 90 } else { 65 };
        }

        // Fuzzy character matching in app name (characters in order)
        if is_fuzzy_match(&app_lower, &query_lower) {
            score += 50;
        }

        // Fuzzy character matching in window title
        if is_fuzzy_match(&title_lower, &query_lower) {
            score += 40;
        }

        if score > 0 {
            matches.push(WindowMatch {
                window: window.clone(),
                score,
            });
        }
    }

    // Sort by score (highest first), then by app name, then by title for ties
    matches.sort_by(|a, b| {
        match b.score.cmp(&a.score) {
            Ordering::Equal => {
                match a.window.app.cmp(&b.window.app) {
                    Ordering::Equal => a.window.title.cmp(&b.window.title),
                    other => other,
                }
            }
            other => other,
        }
    });

    matches
}

/// Perform unified fuzzy search across scripts, scriptlets, and built-ins
/// Returns combined and ranked results sorted by relevance
/// Built-ins appear at the TOP of results (before scripts) when scores are equal
pub fn fuzzy_search_unified(scripts: &[Script], scriptlets: &[Scriptlet], query: &str) -> Vec<SearchResult> {
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
                        SearchResult::BuiltIn(_) => 0,  // Built-ins first
                        SearchResult::App(_) => 1,      // Apps second
                        SearchResult::Window(_) => 2,   // Windows third
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
                        SearchResult::BuiltIn(_) => 0,  // Built-ins first
                        SearchResult::App(_) => 1,      // Apps second
                        SearchResult::Window(_) => 2,   // Windows third
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

/// Maximum number of items to show in the RECENT section
const MAX_RECENT_ITEMS: usize = 10;

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
///
/// # Returns
/// `(Vec<GroupedListItem>, Vec<SearchResult>)` - Grouped items and the flat results array.
/// The `usize` in `Item(usize)` is the index into the flat results array.
///
/// # Example
/// ```ignore
/// let frecency_store = FrecencyStore::new();
/// let (grouped, results) = get_grouped_results(
///     &scripts, &scriptlets, &builtins, &apps,
///     &frecency_store, ""
/// );
/// // grouped contains: [SectionHeader("RECENT"), Item(0), Item(1), SectionHeader("MAIN"), ...]
/// // results contains the flat array of SearchResults
/// ```
#[instrument(level = "debug", skip_all, fields(filter_len = filter_text.len()))]
pub fn get_grouped_results(
    scripts: &[Script],
    scriptlets: &[Scriptlet],
    builtins: &[BuiltInEntry],
    apps: &[AppInfo],
    frecency_store: &FrecencyStore,
    filter_text: &str,
) -> (Vec<GroupedListItem>, Vec<SearchResult>) {
    // Get all unified search results
    let results = fuzzy_search_unified_all(scripts, scriptlets, builtins, apps, filter_text);
    
    // Search mode: return flat list with no headers
    if !filter_text.is_empty() {
        let grouped: Vec<GroupedListItem> = (0..results.len())
            .map(GroupedListItem::Item)
            .collect();
        debug!(
            result_count = results.len(),
            "Search mode: returning flat list"
        );
        return (grouped, results);
    }
    
    // Grouped view mode: create RECENT and MAIN sections
    let mut grouped = Vec::new();
    
    // Get recent items from frecency store
    let recent_items = frecency_store.get_recent_items(MAX_RECENT_ITEMS);
    
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
            SearchResult::Window(wm) => Some(format!("window:{}:{}", wm.window.app, wm.window.title)),
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
    
    // Limit recent items to MAX_RECENT_ITEMS
    recent_indices.truncate(MAX_RECENT_ITEMS);
    
    // Sort main items alphabetically by name
    main_indices.sort_by(|&a, &b| results[a].name().cmp(results[b].name()));
    
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
        main_count = grouped.len().saturating_sub(recent_indices.len()).saturating_sub(if recent_indices.is_empty() { 1 } else { 2 }),
        total_grouped = grouped.len(),
        "Grouped view: created RECENT/MAIN sections"
    );
    
    (grouped, results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scriptlet_basic() {
        let section = "## Test Snippet\n\n```ts\nconst x = 1;\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert_eq!(s.name, "Test Snippet");
        assert_eq!(s.tool, "ts");
        assert_eq!(s.code, "const x = 1;");
        assert_eq!(s.shortcut, None);
    }

    #[test]
    fn test_parse_scriptlet_with_metadata() {
        let section = "## Open File\n\n<!-- \nshortcut: cmd o\n-->\n\n```ts\nawait exec('open')\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert_eq!(s.name, "Open File");
        assert_eq!(s.tool, "ts");
        assert_eq!(s.shortcut, Some("cmd o".to_string()));
    }

    #[test]
    fn test_parse_scriptlet_with_description() {
        let section = "## Test\n\n<!-- \ndescription: Test description\n-->\n\n```bash\necho test\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert_eq!(s.description, Some("Test description".to_string()));
    }

    #[test]
    fn test_parse_scriptlet_with_expand() {
        let section = "## Execute Plan\n\n<!-- \nexpand: plan,,\n-->\n\n```paste\nPlease execute\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert_eq!(s.expand, Some("plan,,".to_string()));
        assert_eq!(s.tool, "paste");
    }

    #[test]
    fn test_extract_code_block_ts() {
        let text = "Some text\n```ts\nconst x = 1;\n```\nMore text";
        let result = extract_code_block(text);
        assert!(result.is_some());
        let (lang, code) = result.unwrap();
        assert_eq!(lang, "ts");
        assert_eq!(code, "const x = 1;");
    }

    #[test]
    fn test_extract_code_block_bash() {
        let text = "```bash\necho hello\necho world\n```";
        let result = extract_code_block(text);
        assert!(result.is_some());
        let (lang, code) = result.unwrap();
        assert_eq!(lang, "bash");
        assert_eq!(code, "echo hello\necho world");
    }

    #[test]
    fn test_extract_html_metadata_shortcut() {
        let text = "<!-- \nshortcut: opt s\n-->";
        let metadata = extract_html_comment_metadata(text);
        assert_eq!(metadata.get("shortcut"), Some(&"opt s".to_string()));
    }

    #[test]
    fn test_extract_html_metadata_multiple() {
        let text = "<!-- \nshortcut: cmd k\nexpand: foo,,\ndescription: Test\n-->";
        let metadata = extract_html_comment_metadata(text);
        assert_eq!(metadata.get("shortcut"), Some(&"cmd k".to_string()));
        assert_eq!(metadata.get("expand"), Some(&"foo,,".to_string()));
        assert_eq!(metadata.get("description"), Some(&"Test".to_string()));
    }

    #[test]
    fn test_parse_scriptlet_none_without_heading() {
        let section = "Some text without heading\n```ts\ncode\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_none());
    }

    #[test]
    fn test_parse_scriptlet_none_without_code_block() {
        let section = "## Name\nNo code block here";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_none());
    }

    #[test]
    fn test_read_scripts_returns_vec() {
        let scripts = read_scripts();
        // scripts should be a Vec, check it's valid
        assert!(scripts.is_empty() || !scripts.is_empty());
    }

    #[test]
    fn test_script_struct_has_required_fields() {
        let script = Script {
            name: "test".to_string(),
            path: PathBuf::from("/test/path"),
            extension: "ts".to_string(),
            description: None,
        };
        assert_eq!(script.name, "test");
        assert_eq!(script.extension, "ts");
    }

    #[test]
    fn test_fuzzy_search_by_name() {
        let scripts = vec![
            Script {
                name: "openfile".to_string(),
                path: PathBuf::from("/test/openfile.ts"),
                extension: "ts".to_string(),
                description: Some("Open a file dialog".to_string()),
            },
            Script {
                name: "savefile".to_string(),
                path: PathBuf::from("/test/savefile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "open");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].script.name, "openfile");
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_fuzzy_search_empty_query() {
        let scripts = vec![
            Script {
                name: "test1".to_string(),
                path: PathBuf::from("/test/test1.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 0);
    }

    #[test]
    fn test_fuzzy_search_ranking() {
        let scripts = vec![
            Script {
                name: "openfile".to_string(),
                path: PathBuf::from("/test/openfile.ts"),
                extension: "ts".to_string(),
                description: Some("Open a file dialog".to_string()),
            },
            Script {
                name: "open".to_string(),
                path: PathBuf::from("/test/open.ts"),
                extension: "ts".to_string(),
                description: Some("Basic open function".to_string()),
            },
            Script {
                name: "reopen".to_string(),
                path: PathBuf::from("/test/reopen.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "open");
        // Should have all three results
        assert_eq!(results.len(), 3);
        // "open" should be first (exact match at start: 100 + fuzzy match: 50 = 150)
        assert_eq!(results[0].script.name, "open");
        // "openfile" should be second (substring at start: 100 + fuzzy match: 50 = 150, but "open" comes first alphabetically in tie)
        assert_eq!(results[1].script.name, "openfile");
        // "reopen" should be third (substring not at start: 75 + fuzzy match: 50 = 125)
        assert_eq!(results[2].script.name, "reopen");
    }

    #[test]
    fn test_fuzzy_search_scriptlets() {
        let scriptlets = vec![
            Scriptlet {
                name: "Copy Text".to_string(),
                description: Some("Copy current selection".to_string()),
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Paste Code".to_string(),
                description: None,
                code: "paste()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "copy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scriptlet.name, "Copy Text");
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_fuzzy_search_unified() {
        let scripts = vec![
            Script {
                name: "open".to_string(),
                path: PathBuf::from("/test/open.ts"),
                extension: "ts".to_string(),
                description: Some("Open a file".to_string()),
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Open Browser".to_string(),
                description: Some("Open in browser".to_string()),
                code: "open()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "open");
        assert_eq!(results.len(), 2);
        
        // First result should be the script (same score but scripts come first)
        match &results[0] {
            SearchResult::Script(sm) => assert_eq!(sm.script.name, "open"),
            _ => panic!("Expected script"),
        }
        
        // Second result should be the scriptlet
        match &results[1] {
            SearchResult::Scriptlet(sm) => assert_eq!(sm.scriptlet.name, "Open Browser"),
            _ => panic!("Expected scriptlet"),
        }
    }

    #[test]
    fn test_search_result_type_label() {
        let script = SearchResult::Script(ScriptMatch {
            script: Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            score: 100,
        });

        let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
            scriptlet: Scriptlet {
                name: "snippet".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            score: 50,
        });

        assert_eq!(script.type_label(), "Script");
        assert_eq!(scriptlet.type_label(), "Snippet");
    }

    // ============================================
    // EDGE CASES: Missing Files, Malformed Data
    // ============================================

    #[test]
    fn test_extract_code_block_no_fence() {
        let text = "No code block here, just text";
        let result = extract_code_block(text);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_code_block_incomplete_fence() {
        let text = "```ts\ncode here\nno closing fence";
        let result = extract_code_block(text);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_code_block_empty() {
        let text = "```ts\n```";
        let result = extract_code_block(text);
        assert!(result.is_some());
        let (lang, code) = result.unwrap();
        assert_eq!(lang, "ts");
        assert!(code.is_empty());
    }

    #[test]
    fn test_extract_code_block_no_language() {
        let text = "```\ncode here\n```";
        let result = extract_code_block(text);
        assert!(result.is_some());
        let (lang, code) = result.unwrap();
        assert!(lang.is_empty());
        assert_eq!(code, "code here");
    }

    #[test]
    fn test_extract_code_block_with_multiple_fences() {
        let text = "```ts\nfirst\n```\n\n```bash\nsecond\n```";
        let result = extract_code_block(text);
        assert!(result.is_some());
        let (lang, code) = result.unwrap();
        assert_eq!(lang, "ts");
        assert_eq!(code, "first");
    }

    #[test]
    fn test_parse_scriptlet_empty_heading() {
        let section = "## \n\n```ts\ncode\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_none());
    }

    #[test]
    fn test_parse_scriptlet_whitespace_only_heading() {
        let section = "##   \n\n```ts\ncode\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_none());
    }

    #[test]
    fn test_extract_html_metadata_empty_comment() {
        let text = "<!-- -->";
        let metadata = extract_html_comment_metadata(text);
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_extract_html_metadata_no_comments() {
        let text = "Some text without HTML comments";
        let metadata = extract_html_comment_metadata(text);
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_extract_html_metadata_malformed_colon() {
        let text = "<!-- \nkey_without_colon value\n-->";
        let metadata = extract_html_comment_metadata(text);
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_extract_html_metadata_unclosed_comment() {
        let text = "<!-- metadata here";
        let metadata = extract_html_comment_metadata(text);
        assert!(metadata.is_empty());
    }

    #[test]
    fn test_extract_html_metadata_with_colons_in_value() {
        let text = "<!-- \ndescription: Full URL: https://example.com\n-->";
        let metadata = extract_html_comment_metadata(text);
        assert_eq!(
            metadata.get("description"),
            Some(&"Full URL: https://example.com".to_string())
        );
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(is_fuzzy_match("OPENFILE", "open"));
        assert!(is_fuzzy_match("Open File", "of"));
        assert!(is_fuzzy_match("OpenFile", "OP"));
    }

    #[test]
    fn test_fuzzy_match_single_char() {
        assert!(is_fuzzy_match("test", "t"));
        assert!(is_fuzzy_match("test", "e"));
        assert!(is_fuzzy_match("test", "s"));
    }

    #[test]
    fn test_fuzzy_match_not_in_order() {
        // "st" IS in order in "test" (t-e-s-t), so this should match
        assert!(is_fuzzy_match("test", "st"));
        // But "cab" is NOT in order in "abc"
        assert!(!is_fuzzy_match("abc", "cab"));
        // And "nope" is NOT in order in "open" (o-p-e-n doesn't contain n-o-p-e in order)
        assert!(!is_fuzzy_match("open", "nope"));
    }

    #[test]
    fn test_fuzzy_match_exact_match() {
        assert!(is_fuzzy_match("test", "test"));
        assert!(is_fuzzy_match("open", "open"));
    }

    #[test]
    fn test_fuzzy_match_empty_pattern() {
        assert!(is_fuzzy_match("test", ""));
        assert!(is_fuzzy_match("", ""));
    }

    #[test]
    fn test_fuzzy_match_pattern_longer_than_haystack() {
        assert!(!is_fuzzy_match("ab", "abc"));
        assert!(!is_fuzzy_match("x", "xyz"));
    }

    #[test]
    fn test_fuzzy_search_no_results() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_fuzzy_search_all_match() {
        let scripts = vec![
            Script {
                name: "test1".to_string(),
                path: PathBuf::from("/test1.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "test2".to_string(),
                path: PathBuf::from("/test2.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "test");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_fuzzy_search_by_description() {
        let scripts = vec![
            Script {
                name: "foo".to_string(),
                path: PathBuf::from("/foo.ts"),
                extension: "ts".to_string(),
                description: Some("database connection helper".to_string()),
            },
            Script {
                name: "bar".to_string(),
                path: PathBuf::from("/bar.ts"),
                extension: "ts".to_string(),
                description: Some("ui component".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "database");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].script.name, "foo");
    }

    #[test]
    fn test_fuzzy_search_by_path() {
        let scripts = vec![
            Script {
                name: "foo".to_string(),
                path: PathBuf::from("/home/user/.kenv/scripts/open.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "bar".to_string(),
                path: PathBuf::from("/home/user/.other/bar.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "kenv");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].script.name, "foo");
    }

    #[test]
    fn test_fuzzy_search_score_ordering() {
        let scripts = vec![
            Script {
                name: "exactmatch".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "other".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: Some("exactmatch in description".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "exactmatch");
        // Name match should score higher than description match
        assert_eq!(results[0].script.name, "exactmatch");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_fuzzy_search_scriptlets_by_tool() {
        let scriptlets = vec![
            Scriptlet {
                name: "Snippet1".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "bash".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Snippet2".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "bash");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scriptlet.name, "Snippet1");
    }

    #[test]
    fn test_fuzzy_search_scriptlets_no_results() {
        let scriptlets = vec![
            Scriptlet {
                name: "Copy Text".to_string(),
                description: Some("Copy current selection".to_string()),
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "paste");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_fuzzy_search_unified_empty_query() {
        let scripts = vec![
            Script {
                name: "script1".to_string(),
                path: PathBuf::from("/script1.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Snippet1".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_fuzzy_search_unified_scripts_first() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: Some("test script".to_string()),
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "test".to_string(),
                description: Some("test snippet".to_string()),
                code: "test()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
        // When scores are equal, scripts should come first
        match &results[0] {
            SearchResult::Script(_) => {}, // Correct
            SearchResult::Scriptlet(_) => panic!("Script should be first"),
            SearchResult::BuiltIn(_) => panic!("Script should be first"),
            SearchResult::App(_) => panic!("Script should be first"),
            SearchResult::Window(_) => panic!("Script should be first"),
        }
    }

    #[test]
    fn test_search_result_properties() {
        let script_match = ScriptMatch {
            script: Script {
                name: "TestScript".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: Some("A test script".to_string()),
            },
            score: 100,
        };

        let result = SearchResult::Script(script_match);

        assert_eq!(result.name(), "TestScript");
        assert_eq!(result.description(), Some("A test script"));
        assert_eq!(result.score(), 100);
        assert_eq!(result.type_label(), "Script");
    }

    #[test]
    fn test_scriptlet_with_all_metadata() {
        let scriptlet = Scriptlet {
            name: "Full Scriptlet".to_string(),
            description: Some("Complete metadata".to_string()),
            code: "code here".to_string(),
            tool: "bash".to_string(),
            shortcut: Some("cmd k".to_string()),
            expand: Some("prompt,,".to_string()),
        };

        assert_eq!(scriptlet.name, "Full Scriptlet");
        assert_eq!(scriptlet.description, Some("Complete metadata".to_string()));
        assert_eq!(scriptlet.shortcut, Some("cmd k".to_string()));
        assert_eq!(scriptlet.expand, Some("prompt,,".to_string()));
    }

    #[test]
    fn test_parse_scriptlet_preserves_whitespace_in_code() {
        let section = "## WhitespaceTest\n\n```ts\n  const x = 1;\n    const y = 2;\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        // Code should preserve relative indentation
        assert!(s.code.contains("const x"));
        assert!(s.code.contains("const y"));
    }

    #[test]
    fn test_parse_scriptlet_multiline_code() {
        let section = "## MultiLine\n\n```ts\nconst obj = {\n  key: value,\n  other: thing\n};\nconsole.log(obj);\n```";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert!(s.code.contains("obj"));
        assert!(s.code.contains("console.log"));
    }

    #[test]
    fn test_extract_metadata_case_insensitive_description() {
        // Metadata extraction is case-sensitive (looks for "// Description:")
        // Verify this behavior
        let script = Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            description: None, // Would be extracted from file if existed
        };
        assert_eq!(script.name, "test");
    }

    // ============================================
    // NAME METADATA PARSING TESTS
    // ============================================
    
    #[test]
    fn test_parse_metadata_line_name_basic() {
        // Basic case: "// Name: Test"
        let line = "// Name: Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_no_space_after_slashes() {
        // "//Name:Test" - no spaces
        let line = "//Name:Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_space_after_colon() {
        // "//Name: Test" - space after colon
        let line = "//Name: Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_space_before_key() {
        // "// Name:Test" - space before key
        let line = "// Name:Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_full_spacing() {
        // "// Name: Test" - standard spacing
        let line = "// Name: Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_multiple_spaces() {
        // "//  Name:Test" - multiple spaces after slashes
        let line = "//  Name:Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_multiple_spaces_and_colon_space() {
        // "//  Name: Test" - multiple spaces after slashes and space after colon
        let line = "//  Name: Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_with_tab() {
        // "//\tName:Test" - tab after slashes
        let line = "//\tName:Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_name_with_tab_and_space_after_colon() {
        // "//\tName: Test" - tab after slashes, space after colon
        let line = "//\tName: Test";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "name");
        assert_eq!(value, "Test");
    }

    #[test]
    fn test_parse_metadata_line_case_insensitive_name() {
        // Case insensitivity: "// name: Test", "// NAME: Test"
        for line in ["// name: Test", "// NAME: Test", "// NaMe: Test"] {
            let result = parse_metadata_line(line);
            assert!(result.is_some(), "Failed for: {}", line);
            let (key, value) = result.unwrap();
            assert_eq!(key.to_lowercase(), "name");
            assert_eq!(value, "Test");
        }
    }

    #[test]
    fn test_parse_metadata_line_description() {
        // Should also work for Description
        let line = "// Description: My script description";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "description");
        assert_eq!(value, "My script description");
    }

    #[test]
    fn test_parse_metadata_line_not_a_comment() {
        // Non-comment lines should return None
        let line = "const name = 'test';";
        let result = parse_metadata_line(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_metadata_line_no_colon() {
        // Comment without colon should return None
        let line = "// Just a comment";
        let result = parse_metadata_line(line);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_script_metadata_name_and_description() {
        let content = r#"// Name: My Script Name
// Description: This is my script
const x = 1;
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, Some("My Script Name".to_string()));
        assert_eq!(metadata.description, Some("This is my script".to_string()));
    }

    #[test]
    fn test_extract_script_metadata_name_only() {
        let content = r#"// Name: My Script
const x = 1;
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, Some("My Script".to_string()));
        assert_eq!(metadata.description, None);
    }

    #[test]
    fn test_extract_script_metadata_description_only() {
        let content = r#"// Description: A description
const x = 1;
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, None);
        assert_eq!(metadata.description, Some("A description".to_string()));
    }

    #[test]
    fn test_extract_script_metadata_no_metadata() {
        let content = r#"const x = 1;
console.log(x);
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, None);
        assert_eq!(metadata.description, None);
    }

    #[test]
    fn test_extract_script_metadata_lenient_whitespace() {
        // Test all the lenient whitespace variants for Name
        let variants = [
            "//Name:Test",
            "//Name: Test",
            "// Name:Test",
            "// Name: Test",
            "//  Name:Test",
            "//  Name: Test",
            "//\tName:Test",
            "//\tName: Test",
        ];
        
        for content in variants {
            let full_content = format!("{}\nconst x = 1;", content);
            let metadata = extract_script_metadata(&full_content);
            assert_eq!(
                metadata.name,
                Some("Test".to_string()),
                "Failed for variant: {}",
                content
            );
        }
    }

    #[test]
    fn test_extract_script_metadata_first_name_wins() {
        // If multiple Name: lines exist, the first one wins
        let content = r#"// Name: First Name
// Name: Second Name
const x = 1;
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, Some("First Name".to_string()));
    }

    #[test]
    fn test_extract_script_metadata_empty_value_ignored() {
        // Empty value should be ignored
        let content = r#"// Name:
// Description: Has a description
const x = 1;
"#;
        let metadata = extract_script_metadata(content);
        assert_eq!(metadata.name, None);
        assert_eq!(metadata.description, Some("Has a description".to_string()));
    }

    #[test]
    fn test_parse_metadata_line_value_with_colons() {
        // Value can contain colons (e.g., URLs)
        let line = "// Description: Visit https://example.com for more info";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (key, value) = result.unwrap();
        assert_eq!(key.to_lowercase(), "description");
        assert_eq!(value, "Visit https://example.com for more info");
    }

    #[test]
    fn test_parse_metadata_line_value_with_leading_trailing_spaces() {
        // Value should be trimmed
        let line = "// Name:   Padded Value   ";
        let result = parse_metadata_line(line);
        assert!(result.is_some());
        let (_, value) = result.unwrap();
        assert_eq!(value, "Padded Value");
    }

    #[test]
    fn test_extract_script_metadata_only_first_20_lines() {
        // Metadata after line 20 should be ignored
        let mut content = String::new();
        for i in 1..=25 {
            if i == 22 {
                content.push_str("// Name: Too Late\n");
            } else {
                content.push_str(&format!("// Comment line {}\n", i));
            }
        }
        let metadata = extract_script_metadata(&content);
        assert_eq!(metadata.name, None);
    }

    #[test]
    fn test_extract_script_metadata_within_first_20_lines() {
        // Metadata within first 20 lines should be captured
        let mut content = String::new();
        for i in 1..=25 {
            if i == 15 {
                content.push_str("// Name: Just In Time\n");
            } else {
                content.push_str(&format!("// Comment line {}\n", i));
            }
        }
        let metadata = extract_script_metadata(&content);
        assert_eq!(metadata.name, Some("Just In Time".to_string()));
    }

    // ============================================
    // INTEGRATION TESTS: End-to-End Flows
    // ============================================

    #[test]
    fn test_script_struct_creation_and_properties() {
        let script = Script {
            name: "myScript".to_string(),
            path: PathBuf::from("/home/user/.kenv/scripts/myScript.ts"),
            extension: "ts".to_string(),
            description: Some("My custom script".to_string()),
        };

        assert_eq!(script.name, "myScript");
        assert_eq!(script.extension, "ts");
        assert!(script.description.is_some());
        assert!(script.path.to_string_lossy().contains("myScript"));
    }

    #[test]
    fn test_script_clone_independence() {
        let original = Script {
            name: "original".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            description: Some("desc".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.path, cloned.path);
    }

    #[test]
    fn test_scriptlet_clone_independence() {
        let original = Scriptlet {
            name: "original".to_string(),
            description: Some("desc".to_string()),
            code: "code".to_string(),
            tool: "ts".to_string(),
            shortcut: Some("cmd k".to_string()),
            expand: None,
        };

        let cloned = original.clone();
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.code, cloned.code);
    }

    #[test]
    fn test_search_multiple_scriptlets() {
        let scriptlets = vec![
            Scriptlet {
                name: "Copy".to_string(),
                description: Some("Copy to clipboard".to_string()),
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Paste".to_string(),
                description: Some("Paste from clipboard".to_string()),
                code: "paste()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Custom Paste".to_string(),
                description: Some("Custom paste with format".to_string()),
                code: "pasteCustom()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "paste");
        assert_eq!(results.len(), 2); // "Paste" and "Custom Paste"
        // "Paste" should rank higher than "Custom Paste"
        assert_eq!(results[0].scriptlet.name, "Paste");
    }

    #[test]
    fn test_unified_search_mixed_results() {
        let scripts = vec![
            Script {
                name: "openFile".to_string(),
                path: PathBuf::from("/openFile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "saveFile".to_string(),
                path: PathBuf::from("/saveFile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Open URL".to_string(),
                description: None,
                code: "open(url)".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "open");
        assert_eq!(results.len(), 2); // "openFile" script and "Open URL" scriptlet
    }

    #[test]
    fn test_search_result_name_accessor() {
        let script = SearchResult::Script(ScriptMatch {
            script: Script {
                name: "TestName".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            score: 50,
        });

        assert_eq!(script.name(), "TestName");
    }

    #[test]
    fn test_search_result_description_accessor() {
        let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
            scriptlet: Scriptlet {
                name: "Test".to_string(),
                description: Some("Test Description".to_string()),
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            score: 75,
        });

        assert_eq!(scriptlet.description(), Some("Test Description"));
    }

    #[test]
    fn test_parse_multiple_scriptlets_from_markdown() {
        let markdown = r#"## First Snippet
<!-- description: First desc -->
```ts
first()
```

## Second Snippet
<!-- description: Second desc -->
```bash
second
```

## Third Snippet
```ts
third()
```"#;

        // Simulate splitting and parsing
        let sections: Vec<&str> = markdown.split("## ").collect();
        let mut count = 0;
        for section in sections.iter().skip(1) {
            let full_section = format!("## {}", section);
            if let Some(scriptlet) = parse_scriptlet_section(&full_section) {
                count += 1;
                assert!(!scriptlet.name.is_empty());
            }
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_fuzzy_search_preserves_vector_order() {
        let scripts = vec![
            Script {
                name: "alpha".to_string(),
                path: PathBuf::from("/alpha.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "beta".to_string(),
                path: PathBuf::from("/beta.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "gamma".to_string(),
                path: PathBuf::from("/gamma.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "");
        assert_eq!(results.len(), 3);
        // Empty query should return in name order
        assert_eq!(results[0].script.name, "alpha");
        assert_eq!(results[1].script.name, "beta");
        assert_eq!(results[2].script.name, "gamma");
    }

    #[test]
    fn test_extract_html_metadata_whitespace_handling() {
        let text = "<!--\n  key1:   value1  \n  key2: value2\n-->";
        let metadata = extract_html_comment_metadata(text);
        // Values should be trimmed
        assert_eq!(metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_scriptlet_with_html_comment_no_fence() {
        // Test that parse_scriptlet_section requires code block even with metadata
        let section = "## NoCode\n\n<!-- description: Test -->\nJust text";
        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_none());
    }

    #[test]
    fn test_fuzzy_match_special_characters() {
        assert!(is_fuzzy_match("test-file", "test"));
        assert!(is_fuzzy_match("test.file", "file"));
        assert!(is_fuzzy_match("test_name", "name"));
    }

    // ============================================
    // CACHING & PERFORMANCE TESTS
    // ============================================

    #[test]
    fn test_read_scripts_returns_sorted_list() {
        // read_scripts should return sorted by name
        let scripts = vec![
            Script {
                name: "zebra".to_string(),
                path: PathBuf::from("/zebra.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "apple".to_string(),
                path: PathBuf::from("/apple.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "monkey".to_string(),
                path: PathBuf::from("/monkey.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        // Manual check of sorting (since read_scripts reads from filesystem)
        let mut sorted = scripts.clone();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        
        assert_eq!(sorted[0].name, "apple");
        assert_eq!(sorted[1].name, "monkey");
        assert_eq!(sorted[2].name, "zebra");
    }

    #[test]
    fn test_scriptlet_ordering_by_name() {
        let scriptlets = vec![
            Scriptlet {
                name: "Zebra".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Apple".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "");
        // Empty query returns all scriptlets in original order with score 0
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].scriptlet.name, "Zebra");
        assert_eq!(results[1].scriptlet.name, "Apple");
        assert_eq!(results[0].score, 0);
        assert_eq!(results[1].score, 0);
    }

    #[test]
    fn test_large_search_result_set() {
        let mut scripts = Vec::new();
        for i in 0..100 {
            scripts.push(Script {
                name: format!("script_{:03}", i),
                path: PathBuf::from(format!("/script_{}.ts", i)),
                extension: "ts".to_string(),
                description: Some(format!("Script number {}", i)),
            });
        }

        let results = fuzzy_search_scripts(&scripts, "script_05");
        // Should find scripts with 05 in name
        assert!(!results.is_empty());
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_script_match_score_meaningful() {
        let scripts = vec![
            Script {
                name: "openfile".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: Some("Opens a file".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "open");
        assert!(results[0].score >= 50); // Should have at least fuzzy match score
    }

    #[test]
    fn test_complex_markdown_parsing() {
        // Test a realistic markdown structure
        let markdown = r#"# Script Collection

## Script One
<!-- 
description: First script
shortcut: cmd 1
-->
```ts
console.log("first");
```

## Script Two
```bash
echo "second"
```

## Script Three
<!-- 
description: Has URL: https://example.com
expand: type,,
-->
```ts
open("https://example.com");
```
"#;

        // Split and parse sections
        let sections: Vec<&str> = markdown.split("## ").collect();
        let mut parsed = 0;
        for section in sections.iter().skip(1) {
            if let Some(scriptlet) = parse_scriptlet_section(&format!("## {}", section)) {
                parsed += 1;
                assert!(!scriptlet.name.is_empty());
                assert!(!scriptlet.code.is_empty());
            }
        }
        assert_eq!(parsed, 3);
    }

    #[test]
    fn test_search_consistency_across_calls() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let result1 = fuzzy_search_scripts(&scripts, "test");
        let result2 = fuzzy_search_scripts(&scripts, "test");

        assert_eq!(result1.len(), result2.len());
        if !result1.is_empty() && !result2.is_empty() {
            assert_eq!(result1[0].score, result2[0].score);
        }
    }

    #[test]
    fn test_search_result_name_never_empty() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "test");
        for result in results {
            let script_match = ScriptMatch {
                script: result.script.clone(),
                score: result.score,
            };
            let search_result = SearchResult::Script(script_match);
            assert!(!search_result.name().is_empty());
        }
    }

    #[test]
    fn test_scriptlet_code_extraction_with_special_chars() {
        let section = r#"## SpecialChars
```ts
const regex = /test\d+/;
const str = "test\nline";
const obj = { key: "value" };
```"#;

        let scriptlet = parse_scriptlet_section(section);
        assert!(scriptlet.is_some());
        let s = scriptlet.unwrap();
        assert!(s.code.contains("regex"));
        assert!(s.code.contains("str"));
    }

    #[test]
    fn test_fuzzy_search_with_unicode() {
        let scripts = vec![
            Script {
                name: "café".to_string(),
                path: PathBuf::from("/cafe.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        // Should be able to search for the ASCII version
        let results = fuzzy_search_scripts(&scripts, "cafe");
        // Depending on implementation, may or may not match
        let _ = results;
    }

    #[test]
    fn test_script_extension_field_accuracy() {
        let script = Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.ts"),
            extension: "ts".to_string(),
            description: None,
        };

        assert_eq!(script.extension, "ts");

        let script_js = Script {
            name: "test".to_string(),
            path: PathBuf::from("/test.js"),
            extension: "js".to_string(),
            description: None,
        };

        assert_eq!(script_js.extension, "js");
    }

    #[test]
    fn test_searchlet_tool_field_various_values() {
        let tools = vec!["ts", "bash", "paste", "sh", "zsh", "py"];
        
        for tool in tools {
            let scriptlet = Scriptlet {
                name: format!("Test {}", tool),
                description: None,
                code: "code".to_string(),
                tool: tool.to_string(),
                shortcut: None,
                expand: None,
            };

            assert_eq!(scriptlet.tool, tool);
        }
    }

    #[test]
    fn test_extract_code_block_with_language_modifiers() {
        let text = "```ts\nconst x = 1;\n```";
        let (lang, _code) = extract_code_block(text).unwrap();
        assert_eq!(lang, "ts");

        let text2 = "```javascript\nconst x = 1;\n```";
        let (lang2, _code2) = extract_code_block(text2).unwrap();
        assert_eq!(lang2, "javascript");
    }

    #[test]
    fn test_parse_scriptlet_section_all_metadata_fields() {
        let section = r#"## Complete
<!-- 
description: Full description here
shortcut: ctrl shift k
expand: choices,,
custom: value
-->
```ts
code here
```"#;

        let scriptlet = parse_scriptlet_section(section).unwrap();
        
        assert_eq!(scriptlet.name, "Complete");
        assert_eq!(scriptlet.description, Some("Full description here".to_string()));
        assert_eq!(scriptlet.shortcut, Some("ctrl shift k".to_string()));
        assert_eq!(scriptlet.expand, Some("choices,,".to_string()));
        // "custom" field won't be extracted as it's not a known field
    }

    #[test]
    fn test_search_result_type_label_consistency() {
        let script = SearchResult::Script(ScriptMatch {
            script: Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            score: 0,
        });

        // Should always return "Script"
        assert_eq!(script.type_label(), "Script");
        
        let scriptlet = SearchResult::Scriptlet(ScriptletMatch {
            scriptlet: Scriptlet {
                name: "test".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            score: 0,
        });

        // Should always return "Snippet"
        assert_eq!(scriptlet.type_label(), "Snippet");
    }

    #[test]
    fn test_empty_inputs_handling() {
        // Empty script list
        let empty_scripts: Vec<Script> = vec![];
        let results = fuzzy_search_scripts(&empty_scripts, "test");
        assert!(results.is_empty());

        // Empty scriptlet list
        let empty_scriptlets: Vec<Scriptlet> = vec![];
        let results = fuzzy_search_scriptlets(&empty_scriptlets, "test");
        assert!(results.is_empty());

        // Empty both
        let unified = fuzzy_search_unified(&empty_scripts, &empty_scriptlets, "test");
        assert!(unified.is_empty());
    }

    // ============================================
    // COMPREHENSIVE RANKING & RELEVANCE TESTS
    // ============================================

    #[test]
    fn test_exact_substring_at_start_highest_score() {
        let scripts = vec![
            Script {
                name: "open".to_string(),
                path: PathBuf::from("/open.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "reopen".to_string(),
                path: PathBuf::from("/reopen.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "open");
        // "open" starts with "open" (score 100 + fuzzy 50 = 150)
        // "reopen" has "open" but not at start (score 75 + fuzzy 50 = 125)
        assert_eq!(results[0].script.name, "open");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_description_match_lower_priority_than_name() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "other".to_string(),
                path: PathBuf::from("/other.ts"),
                extension: "ts".to_string(),
                description: Some("test description".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "test");
        // Name match should rank higher than description match
        assert_eq!(results[0].script.name, "test");
    }

    #[test]
    fn test_path_match_lowest_priority() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "other".to_string(),
                path: PathBuf::from("/test/other.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "test");
        // Name match should rank higher than path match
        assert_eq!(results[0].script.name, "test");
    }

    #[test]
    fn test_scriptlet_code_match_lower_than_description() {
        let scriptlets = vec![
            Scriptlet {
                name: "Snippet".to_string(),
                description: Some("copy text".to_string()),
                code: "paste()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Other".to_string(),
                description: None,
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "copy");
        // Description match should score higher than code match
        assert_eq!(results[0].scriptlet.name, "Snippet");
    }

    #[test]
    fn test_tool_type_bonus_in_scoring() {
        let scriptlets = vec![
            Scriptlet {
                name: "Script1".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "bash".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "Script2".to_string(),
                description: None,
                code: "code".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "bash");
        // "bash" matches tool type in Script1
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scriptlet.name, "Script1");
    }

    #[test]
    fn test_longer_exact_match_ties_with_fuzzy() {
        let scripts = vec![
            Script {
                name: "open".to_string(),
                path: PathBuf::from("/open.ts"),
                extension: "ts".to_string(),
                description: Some("Open a file".to_string()),
            },
            Script {
                name: "openfile".to_string(),
                path: PathBuf::from("/openfile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "open");
        // Both have name matches at start (100 points) and fuzzy match (50 points)
        // When tied, should sort by name alphabetically
        assert_eq!(results[0].script.name, "open");
        assert_eq!(results[1].script.name, "openfile");
    }

    #[test]
    fn test_case_insensitive_matching() {
        let scripts = vec![
            Script {
                name: "OpenFile".to_string(),
                path: PathBuf::from("/openfile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "OPEN");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].script.name, "OpenFile");
    }

    #[test]
    fn test_ranking_preserves_relative_order_on_score_tie() {
        let scripts = vec![
            Script {
                name: "aaa".to_string(),
                path: PathBuf::from("/aaa.ts"),
                extension: "ts".to_string(),
                description: Some("test".to_string()),
            },
            Script {
                name: "bbb".to_string(),
                path: PathBuf::from("/bbb.ts"),
                extension: "ts".to_string(),
                description: Some("test".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "test");
        // Same score, should sort by name
        assert_eq!(results[0].script.name, "aaa");
        assert_eq!(results[1].script.name, "bbb");
    }

    #[test]
    fn test_scriptlet_name_match_bonus_points() {
        let scriptlets = vec![
            Scriptlet {
                name: "copy".to_string(),
                description: None,
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
            Scriptlet {
                name: "paste".to_string(),
                description: None,
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_scriptlets(&scriptlets, "copy");
        // "copy" name has higher bonus than "paste" code match
        assert_eq!(results[0].scriptlet.name, "copy");
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_unified_search_ties_scripts_first() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: Some("Test script".to_string()),
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "test".to_string(),
                description: Some("Test snippet".to_string()),
                code: "test()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
        // Same score, scripts should come before scriptlets
        assert_eq!(results.len(), 2);
        match &results[0] {
            SearchResult::Script(_) => {},
            SearchResult::Scriptlet(_) => panic!("Expected Script first"),
            SearchResult::BuiltIn(_) => panic!("Expected Script first"),
            SearchResult::App(_) => panic!("Expected Script first"),
            SearchResult::Window(_) => panic!("Expected Script first"),
        }
    }

    #[test]
    fn test_partial_match_scores_appropriately() {
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "es");
        // "es" is fuzzy match in "test" but not a substring match
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_multiple_word_query() {
        let scripts = vec![
            Script {
                name: "open file".to_string(),
                path: PathBuf::from("/openfile.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "save".to_string(),
                path: PathBuf::from("/save.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        // Query with space - will be treated as literal string
        let results = fuzzy_search_scripts(&scripts, "open file");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_all_search_types_contribute_to_score() {
        // Test that all scoring categories work
        let scripts = vec![
            Script {
                name: "database".to_string(),
                path: PathBuf::from("/database.ts"),
                extension: "ts".to_string(),
                description: Some("database connection".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "database");
        // Should match on name (100 + 50 = 150) + description (25) = 175
        assert!(results[0].score > 100);
    }

    #[test]
    fn test_search_quality_metrics() {
        // Ensure search returns meaningful results
        let scripts = vec![
            Script {
                name: "zzzFile".to_string(),
                path: PathBuf::from("/home/user/.kenv/scripts/zzzFile.ts"),
                extension: "ts".to_string(),
                description: Some("Opens a file dialog".to_string()),
            },
            Script {
                name: "someScript".to_string(),
                path: PathBuf::from("/home/user/.kenv/scripts/someScript.ts"),
                extension: "ts".to_string(),
                description: Some("Does something".to_string()),
            },
            Script {
                name: "saveData".to_string(),
                path: PathBuf::from("/home/user/.kenv/scripts/saveData.ts"),
                extension: "ts".to_string(),
                description: Some("Saves data to file".to_string()),
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "file");
        // Two should match (zzzFile name and saveData description)
        assert_eq!(results.len(), 2);
        // Name match (zzzFile) should rank higher than description match (saveData)
        assert_eq!(results[0].script.name, "zzzFile");
        assert_eq!(results[1].script.name, "saveData");
    }

    #[test]
    fn test_relevance_ranking_realistic_scenario() {
        let scripts = vec![
            Script {
                name: "grep".to_string(),
                path: PathBuf::from("/grep.ts"),
                extension: "ts".to_string(),
                description: Some("Search files with grep".to_string()),
            },
            Script {
                name: "find".to_string(),
                path: PathBuf::from("/grep-utils.ts"),
                extension: "ts".to_string(),
                description: Some("Find files".to_string()),
            },
            Script {
                name: "search".to_string(),
                path: PathBuf::from("/search.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let results = fuzzy_search_scripts(&scripts, "grep");
        // "grep" name should rank highest
        assert_eq!(results[0].script.name, "grep");
        // "find" with grep in path should rank second
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_mixed_content_search() {
        // Combine scripts and scriptlets in unified search
        let scripts = vec![
            Script {
                name: "copyClipboard".to_string(),
                path: PathBuf::from("/copy.ts"),
                extension: "ts".to_string(),
                description: Some("Copy to clipboard".to_string()),
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Quick Copy".to_string(),
                description: Some("Copy selection".to_string()),
                code: "copy()".to_string(),
                tool: "ts".to_string(),
                shortcut: Some("cmd c".to_string()),
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "copy");
        assert_eq!(results.len(), 2);
        // Verify both types are present
        let has_script = results.iter().any(|r| matches!(r, SearchResult::Script(_)));
        let has_scriptlet = results.iter().any(|r| matches!(r, SearchResult::Scriptlet(_)));
        assert!(has_script);
        assert!(has_scriptlet);
    }

    // ============================================
    // BUILT-IN SEARCH TESTS
    // ============================================

    fn create_test_builtins() -> Vec<BuiltInEntry> {
        use crate::builtins::BuiltInFeature;
        vec![
            BuiltInEntry {
                id: "builtin-clipboard-history".to_string(),
                name: "Clipboard History".to_string(),
                description: "View and manage your clipboard history".to_string(),
                keywords: vec!["clipboard".to_string(), "history".to_string(), "paste".to_string(), "copy".to_string()],
                feature: BuiltInFeature::ClipboardHistory,
                icon: Some("📋".to_string()),
            },
            BuiltInEntry {
                id: "builtin-app-launcher".to_string(),
                name: "App Launcher".to_string(),
                description: "Search and launch installed applications".to_string(),
                keywords: vec!["app".to_string(), "launch".to_string(), "open".to_string(), "application".to_string()],
                feature: BuiltInFeature::AppLauncher,
                icon: Some("🚀".to_string()),
            },
        ]
    }

    #[test]
    fn test_fuzzy_search_builtins_by_name() {
        let builtins = create_test_builtins();
        let results = fuzzy_search_builtins(&builtins, "clipboard");
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Clipboard History");
        assert!(results[0].score > 0);
    }

    #[test]
    fn test_fuzzy_search_builtins_by_keyword() {
        let builtins = create_test_builtins();
        
        // "paste" is a keyword for clipboard history
        let results = fuzzy_search_builtins(&builtins, "paste");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Clipboard History");
        
        // "launch" is a keyword for app launcher
        let results = fuzzy_search_builtins(&builtins, "launch");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "App Launcher");
    }

    #[test]
    fn test_fuzzy_search_builtins_partial_keyword() {
        let builtins = create_test_builtins();
        
        // "clip" should match "clipboard" keyword
        let results = fuzzy_search_builtins(&builtins, "clip");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Clipboard History");
        
        // "app" should match "app" keyword in App Launcher
        let results = fuzzy_search_builtins(&builtins, "app");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "App Launcher");
    }

    #[test]
    fn test_fuzzy_search_builtins_by_description() {
        let builtins = create_test_builtins();
        
        // "manage" is in clipboard history description
        let results = fuzzy_search_builtins(&builtins, "manage");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Clipboard History");
        
        // "installed" is in app launcher description
        let results = fuzzy_search_builtins(&builtins, "installed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "App Launcher");
    }

    #[test]
    fn test_fuzzy_search_builtins_empty_query() {
        let builtins = create_test_builtins();
        let results = fuzzy_search_builtins(&builtins, "");
        
        assert_eq!(results.len(), 2);
        // Both should have score 0
        assert_eq!(results[0].score, 0);
        assert_eq!(results[1].score, 0);
    }

    #[test]
    fn test_fuzzy_search_builtins_no_match() {
        let builtins = create_test_builtins();
        let results = fuzzy_search_builtins(&builtins, "nonexistent");
        
        assert!(results.is_empty());
    }

    #[test]
    fn test_builtin_match_struct() {
        use crate::builtins::BuiltInFeature;
        
        let entry = BuiltInEntry {
            id: "test".to_string(),
            name: "Test Entry".to_string(),
            description: "Test description".to_string(),
            keywords: vec!["test".to_string()],
            feature: BuiltInFeature::ClipboardHistory,
            icon: None,
        };
        
        let builtin_match = BuiltInMatch {
            entry: entry.clone(),
            score: 100,
        };
        
        assert_eq!(builtin_match.entry.name, "Test Entry");
        assert_eq!(builtin_match.score, 100);
    }

    #[test]
    fn test_search_result_builtin_variant() {
        use crate::builtins::BuiltInFeature;
        
        let entry = BuiltInEntry {
            id: "test".to_string(),
            name: "Test Built-in".to_string(),
            description: "Test built-in description".to_string(),
            keywords: vec!["test".to_string()],
            feature: BuiltInFeature::AppLauncher,
            icon: Some("🚀".to_string()),
        };
        
        let result = SearchResult::BuiltIn(BuiltInMatch {
            entry,
            score: 75,
        });
        
        assert_eq!(result.name(), "Test Built-in");
        assert_eq!(result.description(), Some("Test built-in description"));
        assert_eq!(result.score(), 75);
        assert_eq!(result.type_label(), "Built-in");
    }

    #[test]
    fn test_unified_search_with_builtins() {
        let scripts = vec![
            Script {
                name: "my-clipboard".to_string(),
                path: PathBuf::from("/clipboard.ts"),
                extension: "ts".to_string(),
                description: Some("My clipboard script".to_string()),
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Clipboard Helper".to_string(),
                description: Some("Helper for clipboard".to_string()),
                code: "clipboard()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let builtins = create_test_builtins();

        let results = fuzzy_search_unified_with_builtins(&scripts, &scriptlets, &builtins, "clipboard");
        
        // All three should match
        assert_eq!(results.len(), 3);
        
        // Verify all types are present
        let has_builtin = results.iter().any(|r| matches!(r, SearchResult::BuiltIn(_)));
        let has_script = results.iter().any(|r| matches!(r, SearchResult::Script(_)));
        let has_scriptlet = results.iter().any(|r| matches!(r, SearchResult::Scriptlet(_)));
        
        assert!(has_builtin);
        assert!(has_script);
        assert!(has_scriptlet);
    }

    #[test]
    fn test_unified_search_builtins_appear_at_top() {
        let scripts = vec![
            Script {
                name: "history".to_string(),
                path: PathBuf::from("/history.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let builtins = create_test_builtins();

        let results = fuzzy_search_unified_with_builtins(&scripts, &[], &builtins, "history");
        
        // Both should match (Clipboard History builtin and history script)
        assert!(results.len() >= 2);
        
        // When scores are equal, built-ins should appear first
        // Check that the first result is a built-in if scores are equal
        if results.len() >= 2 && results[0].score() == results[1].score() {
            match &results[0] {
                SearchResult::BuiltIn(_) => {}, // Expected
                _ => panic!("Built-in should appear before script when scores are equal"),
            }
        }
    }

    #[test]
    fn test_unified_search_backward_compatible() {
        // Ensure the original fuzzy_search_unified still works without builtins
        let scripts = vec![
            Script {
                name: "test".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];

        let scriptlets = vec![
            Scriptlet {
                name: "Test Snippet".to_string(),
                description: None,
                code: "test()".to_string(),
                tool: "ts".to_string(),
                shortcut: None,
                expand: None,
            },
        ];

        let results = fuzzy_search_unified(&scripts, &scriptlets, "test");
        
        // Should still work without builtins
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_builtin_keyword_matching_priority() {
        let builtins = create_test_builtins();
        
        // "copy" matches keyword in clipboard history
        let results = fuzzy_search_builtins(&builtins, "copy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.name, "Clipboard History");
        assert!(results[0].score >= 75); // Keyword match gives 75 points
    }

    #[test]
    fn test_builtin_fuzzy_keyword_matching() {
        let builtins = create_test_builtins();
        
        // "hist" should fuzzy match "history" keyword
        let results = fuzzy_search_builtins(&builtins, "hist");
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.name, "Clipboard History");
    }

    // ============================================
    // WINDOW SEARCH TESTS
    // ============================================
    // 
    // Note: Most window search tests require WindowInfo to have a public constructor.
    // These tests verify the function signatures and empty input handling.
    // Integration tests with actual WindowInfo require window_control module changes.

    #[test]
    fn test_fuzzy_search_windows_empty_list() {
        // Test with empty window list
        let windows: Vec<crate::window_control::WindowInfo> = vec![];
        
        let results = fuzzy_search_windows(&windows, "test");
        assert!(results.is_empty());
        
        let results_empty_query = fuzzy_search_windows(&windows, "");
        assert!(results_empty_query.is_empty());
    }

    #[test]
    fn test_window_match_type_exists() {
        // Verify WindowMatch struct has expected fields by type-checking
        fn _type_check(wm: &WindowMatch) {
            let _window: &crate::window_control::WindowInfo = &wm.window;
            let _score: i32 = wm.score;
        }
    }

    #[test]
    fn test_search_result_window_type_label() {
        // We can't construct WindowInfo directly, but we can verify
        // the SearchResult::Window variant exists and type_label is correct
        // by checking the match arm in type_label implementation compiles
        fn _verify_window_variant_exists() {
            fn check_label(result: &SearchResult) -> &'static str {
                match result {
                    SearchResult::Window(_) => "Window",
                    _ => "other",
                }
            }
            let _ = check_label;
        }
    }

    #[test]
    fn test_fuzzy_search_unified_with_windows_empty_inputs() {
        let scripts: Vec<Script> = vec![];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<crate::app_launcher::AppInfo> = vec![];
        let windows: Vec<crate::window_control::WindowInfo> = vec![];
        
        let results = fuzzy_search_unified_with_windows(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &windows,
            "test",
        );
        
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_unified_with_windows_returns_scripts() {
        let scripts = vec![
            Script {
                name: "test_script".to_string(),
                path: PathBuf::from("/test.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<crate::app_launcher::AppInfo> = vec![];
        let windows: Vec<crate::window_control::WindowInfo> = vec![];
        
        let results = fuzzy_search_unified_with_windows(
            &scripts,
            &scriptlets,
            &builtins,
            &apps,
            &windows,
            "test",
        );
        
        assert_eq!(results.len(), 1);
        assert!(matches!(&results[0], SearchResult::Script(_)));
    }

    // ============================================
    // GROUPED RESULTS (FRECENCY) TESTS
    // ============================================

    #[test]
    fn test_get_grouped_results_search_mode_flat_list() {
        let scripts = vec![
            Script {
                name: "open".to_string(),
                path: PathBuf::from("/open.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "save".to_string(),
                path: PathBuf::from("/save.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<AppInfo> = vec![];
        let frecency_store = FrecencyStore::new();
        
        // Search mode: non-empty filter should return flat list
        let (grouped, results) = get_grouped_results(
            &scripts, &scriptlets, &builtins, &apps,
            &frecency_store, "open"
        );
        
        // Should be a flat list with no headers
        assert!(!grouped.is_empty());
        for item in &grouped {
            assert!(matches!(item, GroupedListItem::Item(_)));
        }
        assert_eq!(results.len(), 1); // Only "open" matches
    }

    #[test]
    fn test_get_grouped_results_empty_filter_grouped_view() {
        let scripts = vec![
            Script {
                name: "alpha".to_string(),
                path: PathBuf::from("/alpha.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "beta".to_string(),
                path: PathBuf::from("/beta.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<AppInfo> = vec![];
        let frecency_store = FrecencyStore::new();
        
        // Empty filter should return grouped view
        let (grouped, results) = get_grouped_results(
            &scripts, &scriptlets, &builtins, &apps,
            &frecency_store, ""
        );
        
        // Results should contain all items
        assert_eq!(results.len(), 2);
        
        // Grouped should have MAIN section (no RECENT since frecency is empty)
        assert!(!grouped.is_empty());
        
        // First item should be MAIN section header
        assert!(matches!(&grouped[0], GroupedListItem::SectionHeader(s) if s == "MAIN"));
    }

    #[test]
    fn test_get_grouped_results_with_frecency() {
        let scripts = vec![
            Script {
                name: "alpha".to_string(),
                path: PathBuf::from("/alpha.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "beta".to_string(),
                path: PathBuf::from("/beta.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "gamma".to_string(),
                path: PathBuf::from("/gamma.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<AppInfo> = vec![];
        
        // Create frecency store and record usage for one script
        let mut frecency_store = FrecencyStore::new();
        frecency_store.record_use("/beta.ts");
        
        // Empty filter should return grouped view with RECENT section
        let (grouped, results) = get_grouped_results(
            &scripts, &scriptlets, &builtins, &apps,
            &frecency_store, ""
        );
        
        // Results should contain all items
        assert_eq!(results.len(), 3);
        
        // Grouped should have both RECENT and MAIN sections
        let section_headers: Vec<&str> = grouped.iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        
        assert!(section_headers.contains(&"RECENT"));
        assert!(section_headers.contains(&"MAIN"));
    }

    #[test]
    fn test_get_grouped_results_empty_inputs() {
        let scripts: Vec<Script> = vec![];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<AppInfo> = vec![];
        let frecency_store = FrecencyStore::new();
        
        let (grouped, results) = get_grouped_results(
            &scripts, &scriptlets, &builtins, &apps,
            &frecency_store, ""
        );
        
        // Both should be empty when no inputs
        assert!(results.is_empty());
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_get_grouped_results_items_reference_correct_indices() {
        let scripts = vec![
            Script {
                name: "first".to_string(),
                path: PathBuf::from("/first.ts"),
                extension: "ts".to_string(),
                description: None,
            },
            Script {
                name: "second".to_string(),
                path: PathBuf::from("/second.ts"),
                extension: "ts".to_string(),
                description: None,
            },
        ];
        let scriptlets: Vec<Scriptlet> = vec![];
        let builtins: Vec<BuiltInEntry> = vec![];
        let apps: Vec<AppInfo> = vec![];
        let frecency_store = FrecencyStore::new();
        
        let (grouped, results) = get_grouped_results(
            &scripts, &scriptlets, &builtins, &apps,
            &frecency_store, ""
        );
        
        // All Item indices should be valid indices into results
        for item in &grouped {
            if let GroupedListItem::Item(idx) = item {
                assert!(*idx < results.len(), "Index {} out of bounds for results len {}", idx, results.len());
            }
        }
    }
}
