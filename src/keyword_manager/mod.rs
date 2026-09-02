//! Keyword Manager - Text expansion system integration
//!
//! This module ties together all the components of the text expansion system:
//! - KeyboardMonitor: Global keystroke capture
//! - KeywordMatcher: Trigger detection with rolling buffer
//! - TextInjector: Backspace deletion + clipboard paste
//! - Scriptlets: Source of keyword triggers and replacement text
//!
//! # Architecture
//!
//! The KeywordManager:
//! 1. Loads scriptlets with `keyword` metadata from ~/.scriptkit/scriptlets/
//! 2. Registers each keyword trigger with the KeywordMatcher
//! 3. Starts the KeyboardMonitor with a callback that feeds keystrokes to the matcher
//! 4. When a match is found, performs the expansion:
//!    a. Stops keyboard monitor (avoid capturing our own keystrokes)
//!    b. Deletes trigger characters with backspaces
//!    c. Pastes replacement text via clipboard
//!    d. Resumes keyboard monitor
//!

// --- merged from part_000.rs ---
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};
// Import from crate (these are declared in main.rs)
use crate::keyboard_monitor::{KeyEvent, KeyboardMonitor, KeyboardMonitorError};
use crate::keystroke_logger::keystroke_logger;
use crate::keyword_matcher::KeywordMatcher;
use crate::scripts::load_scriptlets;
use crate::snippet::analysis::build_hybrid_snippet_plan;
use crate::template_variables::VariableContext;
use crate::text_injector::{TextInjector, TextInjectorConfig};
/// Delay after stopping monitor before performing expansion (ms)
const STOP_DELAY_MS: u64 = 50;
const DEFAULT_KEYWORD_POST_DELETE_DELAY_MS: u64 = 0;
const MAX_KEYWORD_POST_DELETE_DELAY_MS: u64 = 250;
const KEYWORD_POST_DELETE_DELAY_ENV: &str = "SCRIPT_KIT_KEYWORD_POST_DELETE_DELAY_MS";
/// Delay after expansion before restarting monitor (ms)
const RESTART_DELAY_MS: u64 = 100;

fn keyword_post_delete_delay_ms() -> u64 {
    std::env::var(KEYWORD_POST_DELETE_DELAY_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.min(MAX_KEYWORD_POST_DELETE_DELAY_MS))
        .unwrap_or(DEFAULT_KEYWORD_POST_DELETE_DELAY_MS)
}

fn should_enable_monitor_after_trigger_update(
    enabled_before: bool,
    trigger_count_after: usize,
) -> bool {
    !enabled_before && trigger_count_after > 0
}
/// Configuration for the keyword manager
#[derive(Debug, Clone)]
pub struct KeywordManagerConfig {
    /// Configuration for text injection timing
    pub injector_config: TextInjectorConfig,
    /// Delay after stopping monitor before expansion (ms)
    pub stop_delay_ms: u64,
    /// Delay after expansion before restarting monitor (ms)
    #[allow(dead_code)]
    pub restart_delay_ms: u64,
}
impl Default for KeywordManagerConfig {
    fn default() -> Self {
        Self {
            injector_config: TextInjectorConfig::default(),
            stop_delay_ms: STOP_DELAY_MS,
            restart_delay_ms: RESTART_DELAY_MS,
        }
    }
}
/// Stored scriptlet information for expansion
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct KeywordScriptlet {
    /// The trigger keyword (e.g., ":sig")
    trigger: String,
    /// The scriptlet name
    name: String,
    /// The replacement text (scriptlet body)
    content: String,
    /// Tool type (for future use - execute vs paste)
    tool: String,
    /// Source file path (for debugging)
    source_path: Option<String>,
}
/// Manages the text expansion system
///
/// Coordinates keyboard monitoring, trigger detection, and text injection
/// to provide system-wide text expansion functionality.
pub struct KeywordManager {
    /// Configuration
    config: KeywordManagerConfig,
    /// Registered scriptlets by trigger keyword
    scriptlets: Arc<Mutex<HashMap<String, KeywordScriptlet>>>,
    /// The expand matcher for trigger detection
    matcher: Arc<Mutex<KeywordMatcher>>,
    /// Reverse lookup: file path -> set of triggers from that file
    /// Used for efficient clearing/updating of triggers when a file changes
    file_triggers: Arc<Mutex<HashMap<PathBuf, HashSet<String>>>>,
    /// The keyboard monitor (optional - created on enable)
    monitor: Option<KeyboardMonitor>,
    /// The text injector (reserved for future direct use)
    #[allow(dead_code)]
    injector: TextInjector,
    /// Whether the keyword system is enabled
    enabled: bool,
}

// --- merged from part_001.rs ---
// --- merged from part_001_impl/methods_000.rs ---
impl KeywordManager {
    /// Create a new KeywordManager with default configuration
    pub fn new() -> Self {
        Self::with_config(KeywordManagerConfig::default())
    }

    /// Create a new KeywordManager with custom configuration
    pub fn with_config(config: KeywordManagerConfig) -> Self {
        let injector = TextInjector::with_config(config.injector_config.clone());

        Self {
            config,
            scriptlets: Arc::new(Mutex::new(HashMap::new())),
            matcher: Arc::new(Mutex::new(KeywordMatcher::new())),
            file_triggers: Arc::new(Mutex::new(HashMap::new())),
            monitor: None,
            injector,
            enabled: false,
        }
    }

    /// Load scriptlets with keyword metadata from ~/.scriptkit/plugins/*/scriptlets/
    ///
    /// This scans all markdown files and registers any scriptlet that has
    /// an `keyword` metadata field as a trigger.
    #[instrument(skip(self))]
    pub fn load_scriptlets(&mut self) -> Result<usize> {
        info!("Loading scriptlets with keyword triggers");

        // Use load_scriptlets() to load from all plugins (plugins/*/scriptlets/*.md)
        let scriptlets = load_scriptlets()?.into_scriptlets();
        self.clear_triggers();
        let mut loaded_count = 0;

        for scriptlet in scriptlets {
            // Only process scriptlets with keyword metadata
            if let Some(ref keyword_trigger) = scriptlet.keyword {
                if keyword_trigger.is_empty() {
                    debug!(
                        name = %scriptlet.name,
                        "Skipping scriptlet with empty keyword trigger"
                    );
                    continue;
                }

                info!(
                    trigger = %keyword_trigger,
                    name = %scriptlet.name,
                    tool = %scriptlet.tool,
                    "Registering keyword trigger"
                );

                // Store the scriptlet info
                let keyword_scriptlet = KeywordScriptlet {
                    trigger: keyword_trigger.clone(),
                    name: scriptlet.name.clone(),
                    content: scriptlet.code.clone(),
                    tool: scriptlet.tool.clone(),
                    source_path: scriptlet.file_path.clone(),
                };

                // Register with matcher and scriptlets store
                {
                    let mut scriptlets_guard =
                        self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                    scriptlets_guard.insert(keyword_trigger.clone(), keyword_scriptlet);
                }

                {
                    let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
                    // Use a dummy path since we store scriptlet data separately
                    let dummy_path = PathBuf::from(format!("scriptlet:{}", scriptlet.name));
                    matcher_guard.register_trigger(keyword_trigger, dummy_path);
                }

                // Track which file this trigger came from for incremental updates
                // Note: scriptlet.file_path includes an anchor like "/path/file.md#slug"
                // We need to strip the anchor for file-level tracking
                if let Some(ref file_path) = scriptlet.file_path {
                    let base_path = if let Some(hash_idx) = file_path.find('#') {
                        PathBuf::from(&file_path[..hash_idx])
                    } else {
                        PathBuf::from(file_path)
                    };
                    let mut file_triggers_guard =
                        self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
                    file_triggers_guard
                        .entry(base_path)
                        .or_default()
                        .insert(keyword_trigger.clone());
                }

                loaded_count += 1;
            }
        }

        info!(
            count = loaded_count,
            "Loaded keyword triggers from scriptlets"
        );
        Ok(loaded_count)
    }

    /// Register a single keyword trigger manually
    ///
    /// This is useful for adding triggers that don't come from scriptlets.
    #[allow(dead_code)]
    pub fn register_trigger(&mut self, trigger: &str, name: &str, content: &str, tool: &str) {
        if trigger.is_empty() {
            debug!("Attempted to register empty trigger, ignoring");
            return;
        }

        info!(
            trigger = %trigger,
            name = %name,
            "Manually registering keyword trigger"
        );

        let keyword_scriptlet = KeywordScriptlet {
            trigger: trigger.to_string(),
            name: name.to_string(),
            content: content.to_string(),
            tool: tool.to_string(),
            source_path: None,
        };

        {
            let mut scriptlets_guard = self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
            scriptlets_guard.insert(trigger.to_string(), keyword_scriptlet);
        }

        {
            let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
            let dummy_path = PathBuf::from(format!("manual:{}", name));
            matcher_guard.register_trigger(trigger, dummy_path);
        }
    }

    /// Enable the keyword system (start keyboard monitoring)
    ///
    /// # Errors
    /// - `AccessibilityNotGranted`: Accessibility permissions not enabled
    /// - `EventTapCreationFailed`: Failed to create macOS event tap
    #[instrument(skip(self))]
    pub fn enable(&mut self) -> Result<(), KeyboardMonitorError> {
        if self.enabled {
            debug!("Keyword system already enabled");
            return Ok(());
        }

        info!("Enabling keyword system");

        // Check trigger count
        let trigger_count = {
            let matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
            matcher_guard.trigger_count()
        };

        if trigger_count == 0 {
            info!(
                trigger_count,
                "Keyword monitor starting with no triggers; waiting for future scriptlet updates"
            );
        }

        // Clone Arc references for the closure
        let matcher = Arc::clone(&self.matcher);
        let scriptlets = Arc::clone(&self.scriptlets);
        // Wrap configs in Arc to avoid cloning on every keystroke
        let config = Arc::new(self.config.clone());
        let injector_config = Arc::new(self.config.injector_config.clone());

        // Create keyboard monitor with callback
        let mut monitor = KeyboardMonitor::new(move |event: KeyEvent| {
            // Only process printable characters (ignore modifier keys, etc.)
            if let Some(ref character) = event.character {
                // Skip if any modifier is held (except shift for capitals)
                if event.command || event.control || event.option {
                    keystroke_logger().record_skipped();
                    return;
                }

                // Process each character in the string (usually just 1)
                for c in character.chars() {
                    // Record keystroke for debounced logging
                    keystroke_logger().record_keystroke(c);

                    // Feed to matcher
                    let match_result = {
                        let mut matcher_guard = matcher.lock().unwrap_or_else(|e| e.into_inner());
                        matcher_guard.process_keystroke(c)
                    };

                    // Handle match if found
                    if let Some(result) = match_result {
                        // Log match immediately (important event)
                        keystroke_logger().log_match(&result.trigger, result.chars_to_delete);

                        // Get the scriptlet content
                        let scriptlet_opt = {
                            let scriptlets_guard =
                                scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                            scriptlets_guard.get(&result.trigger).cloned()
                        };

                        if let Some(scriptlet) = scriptlet_opt {
                            // Perform expansion in a separate thread to not block the callback
                            let chars_to_delete = result.chars_to_delete;
                            let content = scriptlet.content.clone();
                            let tool = scriptlet.tool.clone();
                            let name = scriptlet.name.clone();
                            // Arc clone is cheap - just increments reference count
                            let config_clone = Arc::clone(&config);
                            let injector_config_clone = Arc::clone(&injector_config);

                            thread::spawn(move || {
                                // Small delay to let the keyboard event complete
                                thread::sleep(Duration::from_millis(config_clone.stop_delay_ms));

                                // Get raw content based on tool type
                                let raw_content = match tool.as_str() {
                                    "paste" | "type" | "template" => content.clone(),
                                    _ => {
                                        // For other tools, use the content as-is for now
                                        // Future: execute the scriptlet and capture output
                                        info!(
                                            tool = %tool,
                                            name = %name,
                                            "Tool type not yet fully supported for keyword, using raw content"
                                        );
                                        content.clone()
                                    }
                                };

                                // Build hybrid snippet plan (resolves known variables,
                                // detects whether interactive template is needed)
                                let ctx = VariableContext::new();
                                let plan = build_hybrid_snippet_plan(&raw_content, &ctx);

                                debug!(
                                    trigger = %result.trigger,
                                    kind = ?plan.kind,
                                    unresolved = ?plan.unresolved_variables,
                                    has_explicit_tabstops = plan.has_explicit_tabstops,
                                    "Built hybrid text expansion plan"
                                );

                                if plan.needs_interaction() {
                                    info!(
                                        trigger = %result.trigger,
                                        unresolved = ?plan.unresolved_variables,
                                        has_explicit_tabstops = plan.has_explicit_tabstops,
                                        "Interactive hybrid snippet detected; session bridge follow-up required"
                                    );
                                }

                                // Use resolved content for paste (same behavior as before for static snippets)
                                let replacement = plan.resolved_content.clone();

                                debug!(
                                    original_len = raw_content.len(),
                                    substituted_len = replacement.len(),
                                    had_substitutions = raw_content != replacement,
                                    "Variable substitution completed"
                                );

                                // Create injector and perform expansion
                                // Dereference Arc to get the config
                                let injector =
                                    TextInjector::with_config((*injector_config_clone).clone());

                                let expansion_start = Instant::now();
                                let trigger_len = result.trigger.chars().count();

                                // Delete trigger characters
                                let delete_start = Instant::now();
                                if let Err(e) = injector.delete_chars(chars_to_delete) {
                                    let total_ms = expansion_start.elapsed().as_millis() as u64;
                                    error!(
                                        error = %e,
                                        category = "KEYWORD",
                                        event = "keyword_expansion_timing",
                                        chars_to_delete,
                                        trigger_len,
                                        replacement_len = replacement.len(),
                                        stop_delay_ms = config_clone.stop_delay_ms,
                                        delete_ms = delete_start.elapsed().as_millis() as u64,
                                        post_delete_delay_ms = keyword_post_delete_delay_ms(),
                                        paste_ms = 0_u64,
                                        total_ms,
                                        success = false,
                                        "Failed to delete trigger characters"
                                    );
                                    return;
                                }
                                let delete_ms = delete_start.elapsed().as_millis() as u64;

                                let post_delete_delay_ms = keyword_post_delete_delay_ms();
                                if post_delete_delay_ms > 0 {
                                    thread::sleep(Duration::from_millis(post_delete_delay_ms));
                                }

                                // Paste replacement text
                                let paste_start = Instant::now();
                                if let Err(e) = injector.paste_text(&replacement) {
                                    let paste_ms = paste_start.elapsed().as_millis() as u64;
                                    let total_ms = expansion_start.elapsed().as_millis() as u64;
                                    error!(
                                        error = %e,
                                        category = "KEYWORD",
                                        event = "keyword_expansion_timing",
                                        chars_to_delete,
                                        trigger_len,
                                        replacement_len = replacement.len(),
                                        stop_delay_ms = config_clone.stop_delay_ms,
                                        delete_ms,
                                        post_delete_delay_ms,
                                        paste_ms,
                                        total_ms,
                                        success = false,
                                        "Failed to paste replacement text"
                                    );
                                    return;
                                }
                                let paste_ms = paste_start.elapsed().as_millis() as u64;
                                let total_ms = expansion_start.elapsed().as_millis() as u64;

                                info!(
                                    trigger = %name,
                                    replacement_len = replacement.len(),
                                    "Expansion completed successfully"
                                );
                                info!(
                                    category = "KEYWORD",
                                    event = "keyword_expansion_timing",
                                    chars_to_delete,
                                    trigger_len,
                                    replacement_len = replacement.len(),
                                    stop_delay_ms = config_clone.stop_delay_ms,
                                    delete_ms,
                                    post_delete_delay_ms,
                                    paste_ms,
                                    total_ms,
                                    success = true,
                                    "Keyword expansion timing"
                                );
                            });

                            // Clear the buffer after a match to prevent re-triggering
                            let mut matcher_guard =
                                matcher.lock().unwrap_or_else(|e| e.into_inner());
                            matcher_guard.clear_buffer();
                        } else {
                            warn!(
                                trigger = %result.trigger,
                                "Matched trigger but scriptlet not found in store"
                            );
                        }
                    }
                }
            }
        });

        // Start the monitor
        monitor.start()?;

        self.monitor = Some(monitor);
        self.enabled = true;

        info!("Keyword system enabled, keyboard monitoring active");
        Ok(())
    }
}

// --- merged from part_001_impl/methods_001.rs ---
impl KeywordManager {
    /// Disable the keyword system (stop keyboard monitoring)
    #[instrument(skip(self))]
    pub fn disable(&mut self) {
        if !self.enabled {
            debug!("Keyword system already disabled");
            return;
        }

        info!("Disabling keyword system");

        if let Some(ref mut monitor) = self.monitor {
            monitor.stop();
        }
        self.monitor = None;
        self.enabled = false;

        info!("Keyword system disabled");
    }

    /// Check if the keyword system is currently enabled
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the number of registered triggers
    #[allow(dead_code)]
    pub fn trigger_count(&self) -> usize {
        let matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
        matcher_guard.trigger_count()
    }

    /// Check if accessibility permissions are granted
    ///
    /// Returns true if the application has accessibility permissions.
    /// These are required for keyboard monitoring and text injection.
    pub fn has_accessibility_permission() -> bool {
        KeyboardMonitor::has_accessibility_permission()
    }

    /// Request accessibility permissions, showing the system dialog if needed
    ///
    /// Returns true if permissions are granted (either already or after user action).
    #[allow(dead_code)]
    pub fn request_accessibility_permission() -> bool {
        KeyboardMonitor::request_accessibility_permission()
    }

    /// Clear all registered triggers
    #[allow(dead_code)]
    pub fn clear_triggers(&mut self) {
        {
            let mut scriptlets_guard = self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
            scriptlets_guard.clear();
        }
        {
            let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
            matcher_guard.clear_triggers();
        }
        {
            let mut file_triggers_guard =
                self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            file_triggers_guard.clear();
        }

        debug!("All keyword triggers cleared");
    }

    /// Reload scriptlets (clear existing and load fresh)
    #[allow(dead_code)]
    #[instrument(skip(self))]
    pub fn reload(&mut self) -> Result<usize> {
        info!("Reloading keyword scriptlets");

        self.load_scriptlets()
    }

    /// Get list of all registered triggers (for debugging/UI)
    pub fn list_triggers(&self) -> Vec<(String, String)> {
        let scriptlets_guard = self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
        scriptlets_guard
            .iter()
            .map(|(trigger, scriptlet)| (trigger.clone(), scriptlet.name.clone()))
            .collect()
    }

    /// Unregister a single trigger by its keyword
    ///
    /// This removes the trigger from the matcher and the scriptlets store.
    ///
    /// # Arguments
    /// * `trigger` - The trigger keyword to remove (e.g., ":sig")
    ///
    /// # Returns
    /// `true` if the trigger was removed, `false` if it didn't exist
    #[allow(dead_code)]
    pub fn unregister_trigger(&mut self, trigger: &str) -> bool {
        let scriptlet_removed = {
            let mut scriptlets_guard = self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
            scriptlets_guard.remove(trigger).is_some()
        };

        let matcher_removed = {
            let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
            matcher_guard.unregister_trigger(trigger)
        };

        // Also remove from file_triggers tracking
        {
            let mut file_triggers_guard =
                self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            for triggers_set in file_triggers_guard.values_mut() {
                triggers_set.remove(trigger);
            }
            // Clean up empty entries
            file_triggers_guard.retain(|_, triggers| !triggers.is_empty());
        }

        if scriptlet_removed || matcher_removed {
            debug!(trigger = %trigger, "Unregistered keyword trigger");
            true
        } else {
            false
        }
    }

    /// Clear all triggers that came from a specific file
    ///
    /// This is useful when a scriptlet file is deleted - all triggers
    /// registered from that file should be removed.
    ///
    /// # Arguments
    /// * `path` - The path to the scriptlet file
    ///
    /// # Returns
    /// The number of triggers that were removed
    #[allow(dead_code)]
    pub fn clear_triggers_for_file(&mut self, path: &Path) -> usize {
        // Get the triggers registered from this file
        let triggers_to_remove: Vec<String> = {
            let file_triggers_guard = self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            file_triggers_guard
                .get(path)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default()
        };

        if triggers_to_remove.is_empty() {
            debug!(path = %path.display(), "No triggers to clear for file");
            return 0;
        }

        let count = triggers_to_remove.len();

        // Remove each trigger
        for trigger in &triggers_to_remove {
            {
                let mut scriptlets_guard =
                    self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                scriptlets_guard.remove(trigger);
            }
            {
                let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
                matcher_guard.unregister_trigger(trigger);
            }
        }

        // Remove the file entry from tracking
        {
            let mut file_triggers_guard =
                self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            file_triggers_guard.remove(path);
        }

        info!(
            path = %path.display(),
            count = count,
            "Cleared triggers for file"
        );

        count
    }

    /// Get triggers registered for a specific file (for debugging/testing)
    #[allow(dead_code)]
    pub fn get_triggers_for_file(&self, path: &Path) -> Vec<String> {
        let file_triggers_guard = self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
        file_triggers_guard
            .get(path)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Register a trigger from a specific file
    ///
    /// This is like `register_trigger` but also tracks the source file
    /// for incremental updates.
    ///
    /// # Arguments
    /// * `trigger` - The trigger keyword (e.g., ":sig")
    /// * `name` - The scriptlet name
    /// * `content` - The replacement text
    /// * `tool` - The tool type (e.g., "paste", "type")
    /// * `source_path` - The file this trigger came from
    #[allow(dead_code)]
    pub fn register_trigger_from_file(
        &mut self,
        trigger: &str,
        name: &str,
        content: &str,
        tool: &str,
        source_path: &Path,
    ) {
        if trigger.is_empty() {
            debug!("Attempted to register empty trigger, ignoring");
            return;
        }

        info!(
            trigger = %trigger,
            name = %name,
            source = %source_path.display(),
            "Registering keyword trigger from file"
        );

        let keyword_scriptlet = KeywordScriptlet {
            trigger: trigger.to_string(),
            name: name.to_string(),
            content: content.to_string(),
            tool: tool.to_string(),
            source_path: Some(source_path.to_string_lossy().into_owned()),
        };

        {
            let mut scriptlets_guard = self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
            scriptlets_guard.insert(trigger.to_string(), keyword_scriptlet);
        }

        {
            let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
            let dummy_path = PathBuf::from(format!("manual:{}", name));
            matcher_guard.register_trigger(trigger, dummy_path);
        }

        // Track the file -> trigger mapping
        {
            let mut file_triggers_guard =
                self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            file_triggers_guard
                .entry(source_path.to_path_buf())
                .or_default()
                .insert(trigger.to_string());
        }
    }
}

// --- merged from part_001_impl/methods_002.rs ---
impl KeywordManager {
    /// Update triggers for a file with new scriptlet data
    ///
    /// This performs a diff between the existing triggers and the new triggers:
    /// - Triggers that no longer exist are removed
    /// - New triggers are added
    /// - Triggers with changed content are updated
    ///
    /// # Arguments
    /// * `path` - The path to the scriptlet file
    /// * `new_triggers` - The new trigger definitions: (trigger, name, content, tool)
    ///
    /// # Returns
    /// A tuple of (added_count, removed_count, updated_count)
    #[allow(dead_code)]
    pub fn update_triggers_for_file(
        &mut self,
        path: &Path,
        new_triggers: &[(String, String, String, String)],
    ) -> (usize, usize, usize) {
        // Get existing triggers for this file
        let existing_triggers: HashSet<String> = {
            let file_triggers_guard = self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            let result = file_triggers_guard.get(path).cloned().unwrap_or_default();
            info!(
                path = %path.display(),
                found = !result.is_empty(),
                existing_triggers = ?result,
                "Looking up existing triggers for path"
            );
            result
        };

        // Build set of new trigger keywords
        let new_trigger_keys: HashSet<String> =
            new_triggers.iter().map(|(t, _, _, _)| t.clone()).collect();

        // Find triggers to remove (exist in old but not in new)
        let to_remove: Vec<String> = existing_triggers
            .difference(&new_trigger_keys)
            .cloned()
            .collect();

        // Find triggers to add (exist in new but not in old)
        let to_add: Vec<_> = new_triggers
            .iter()
            .filter(|(t, _, _, _)| !existing_triggers.contains(t))
            .collect();

        // Find triggers to update (exist in both, check if content changed)
        let mut updated_count = 0;
        for (trigger, name, content, tool) in new_triggers {
            if existing_triggers.contains(trigger) {
                // Check if content changed
                let content_changed = {
                    let scriptlets_guard =
                        self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(existing) = scriptlets_guard.get(trigger) {
                        existing.content != *content
                            || existing.name != *name
                            || existing.tool != *tool
                    } else {
                        true // Treat as changed if not found
                    }
                };

                if content_changed {
                    // Update the scriptlet
                    let keyword_scriptlet = KeywordScriptlet {
                        trigger: trigger.clone(),
                        name: name.clone(),
                        content: content.clone(),
                        tool: tool.clone(),
                        source_path: Some(path.to_string_lossy().into_owned()),
                    };

                    {
                        let mut scriptlets_guard =
                            self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                        scriptlets_guard.insert(trigger.clone(), keyword_scriptlet);
                    }

                    debug!(
                        trigger = %trigger,
                        path = %path.display(),
                        "Updated trigger content"
                    );
                    updated_count += 1;
                }
            }
        }

        // Remove old triggers
        for trigger in &to_remove {
            {
                let mut scriptlets_guard =
                    self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                scriptlets_guard.remove(trigger);
            }
            {
                let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
                matcher_guard.unregister_trigger(trigger);
            }
            debug!(trigger = %trigger, path = %path.display(), "Removed trigger");
        }

        // Add new triggers
        for (trigger, name, content, tool) in &to_add {
            let keyword_scriptlet = KeywordScriptlet {
                trigger: trigger.clone(),
                name: name.clone(),
                content: content.clone(),
                tool: tool.clone(),
                source_path: Some(path.to_string_lossy().into_owned()),
            };

            {
                let mut scriptlets_guard =
                    self.scriptlets.lock().unwrap_or_else(|e| e.into_inner());
                scriptlets_guard.insert(trigger.clone(), keyword_scriptlet);
            }

            {
                let mut matcher_guard = self.matcher.lock().unwrap_or_else(|e| e.into_inner());
                let dummy_path = PathBuf::from(format!("scriptlet:{}", name));
                matcher_guard.register_trigger(trigger, dummy_path);
            }

            debug!(trigger = %trigger, path = %path.display(), "Added trigger");
        }

        // Update file_triggers tracking
        {
            let mut file_triggers_guard =
                self.file_triggers.lock().unwrap_or_else(|e| e.into_inner());
            if new_trigger_keys.is_empty() {
                file_triggers_guard.remove(path);
            } else {
                file_triggers_guard.insert(path.to_path_buf(), new_trigger_keys);
            }
        }

        let added_count = to_add.len();
        let removed_count = to_remove.len();

        info!(
            path = %path.display(),
            added = added_count,
            removed = removed_count,
            updated = updated_count,
            "Updated triggers for file"
        );

        (added_count, removed_count, updated_count)
    }
}

// --- merged from part_002.rs ---
impl Default for KeywordManager {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for KeywordManager {
    fn drop(&mut self) {
        self.disable();
    }
}
// ============================================================================
// Global Singleton
// ============================================================================

use std::sync::OnceLock;
/// Global singleton for the keyword manager
/// This allows the manager to be accessed from anywhere in the application,
/// including the file watcher callback that needs to update triggers.
static KEYWORD_MANAGER: OnceLock<Mutex<KeywordManager>> = OnceLock::new();
/// Initialize the global keyword manager singleton
///
/// This should be called once at startup. Returns the number of triggers loaded,
/// or an error if initialization fails.
///
/// # Returns
/// - `Ok(Some(count))` if initialization succeeded and keyboard monitoring is enabled
/// - `Ok(None)` if accessibility permissions are not granted (manager not initialized)
/// - `Err(e)` if there was an error during initialization
#[cfg(target_os = "macos")]
pub fn init_keyword_manager() -> Result<Option<usize>> {
    // Check accessibility permissions first
    if !KeywordManager::has_accessibility_permission() {
        info!("Accessibility permissions not granted - text expansion disabled");
        return Ok(None);
    }

    let manager = KEYWORD_MANAGER.get_or_init(|| Mutex::new(KeywordManager::new()));

    let mut guard = manager
        .lock()
        .map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;

    // Load scriptlets with keyword triggers
    let count = guard.load_scriptlets()?;
    let enabled_before = guard.is_enabled();
    if count == 0 {
        info!(
            count,
            enabled_before,
            "No keyword triggers found at startup; enabling keyword monitor for future scriptlet updates"
        );
    }

    // Enable keyboard monitoring
    guard.enable()?;
    info!(
        count,
        enabled_before,
        enabled_after = guard.is_enabled(),
        "Keyword manager initialized"
    );

    // List registered triggers for debugging
    for (trigger, name) in guard.list_triggers() {
        debug!(trigger = %trigger, name = %name, "Registered trigger");
    }

    Ok(Some(count))
}
#[cfg(not(target_os = "macos"))]
pub fn init_keyword_manager() -> Result<Option<usize>> {
    // Text expansion is macOS-only for now
    Ok(None)
}

/// Block until Accessibility permission is granted, then initialize.
///
/// Granting Accessibility in System Settings must arm text expansion without
/// an app restart. Call on a dedicated background thread after
/// `init_keyword_manager()` returned `Ok(None)`; polls AXIsProcessTrusted
/// (cheap) until it flips, then performs the normal initialization.
#[cfg(target_os = "macos")]
pub fn init_keyword_manager_when_accessibility_granted() -> Result<Option<usize>> {
    const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
    while !KeywordManager::has_accessibility_permission() {
        std::thread::sleep(POLL_INTERVAL);
    }
    info!("Accessibility permission granted - arming text expansion without restart");
    init_keyword_manager()
}
/// Update keyword triggers when a scriptlet file changes
///
/// This is called by the file watcher when a scriptlet file is modified.
/// It updates the keyword matcher with any added/removed/changed triggers.
///
/// # Arguments
/// * `path` - Path to the changed scriptlet file
/// * `new_scriptlets` - The newly parsed scriptlets from the file (empty if file was deleted)
#[cfg(target_os = "macos")]
pub fn update_keyword_triggers_for_file(
    path: &Path,
    new_scriptlets: &[std::sync::Arc<crate::scripts::Scriptlet>],
) -> (usize, usize, usize) {
    info!(
        path = %path.display(),
        scriptlet_count = new_scriptlets.len(),
        "update_keyword_triggers_for_file called"
    );

    let Some(manager) = KEYWORD_MANAGER.get() else {
        warn!("Keyword manager not initialized yet, skipping trigger update");
        return (0, 0, 0);
    };

    let Ok(mut guard) = manager.lock() else {
        warn!("Failed to lock keyword manager");
        return (0, 0, 0);
    };

    // Log existing file_triggers for debugging
    {
        let file_triggers_guard = guard
            .file_triggers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let existing_keys: Vec<_> = file_triggers_guard.keys().collect();
        info!(
            path = %path.display(),
            existing_paths = ?existing_keys,
            "Checking file_triggers for match"
        );
    }

    // Collect the new triggers from the scriptlets
    let new_triggers: Vec<(String, String, String, String)> = new_scriptlets
        .iter()
        .filter_map(|s| {
            s.keyword.as_ref().map(|kw| {
                info!(
                    name = %s.name,
                    trigger_len = kw.chars().count(),
                    content_len = s.code.len(),
                    "Found scriptlet with keyword trigger"
                );
                (kw.clone(), s.name.clone(), s.code.clone(), s.tool.clone())
            })
        })
        .collect();

    info!(
        trigger_count = new_triggers.len(),
        "Calling update_triggers_for_file"
    );

    let enabled_before = guard.is_enabled();
    let trigger_count_before = guard.trigger_count();
    let (added, removed, updated) = guard.update_triggers_for_file(path, &new_triggers);
    let trigger_count_after = guard.trigger_count();
    let mut enable_attempted = false;
    let mut enable_failed = false;

    if should_enable_monitor_after_trigger_update(enabled_before, trigger_count_after) {
        enable_attempted = true;
        if let Err(error) = guard.enable() {
            enable_failed = true;
            warn!(
                path = %path.display(),
                added,
                removed,
                updated,
                trigger_count_before,
                trigger_count_after,
                error = %error,
                "keyword_monitor_enable_after_update_failed"
            );
        }
    }

    info!(
        path = %path.display(),
        scriptlet_count = new_scriptlets.len(),
        keyword_trigger_count = new_triggers.len(),
        added,
        removed,
        updated,
        trigger_count_before,
        trigger_count_after,
        enabled_before,
        enabled_after = guard.is_enabled(),
        enable_attempted,
        enable_failed,
        "keyword_trigger_update_applied"
    );

    (added, removed, updated)
}
#[cfg(not(target_os = "macos"))]
pub fn update_keyword_triggers_for_file(
    _path: &Path,
    _new_scriptlets: &[std::sync::Arc<crate::scripts::Scriptlet>],
) -> (usize, usize, usize) {
    (0, 0, 0)
}
/// Check if the keyword manager is currently enabled
#[allow(dead_code)]
pub fn is_keyword_manager_enabled() -> bool {
    KEYWORD_MANAGER
        .get()
        .and_then(|m| m.lock().ok())
        .map(|g| g.is_enabled())
        .unwrap_or(false)
}

// --- merged from part_003.rs ---
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_disabled_manager() {
        let manager = KeywordManager::new();
        assert!(!manager.is_enabled());
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_default_creates_disabled_manager() {
        let manager = KeywordManager::default();
        assert!(!manager.is_enabled());
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_custom_config() {
        let config = KeywordManagerConfig {
            stop_delay_ms: 100,
            restart_delay_ms: 200,
            ..Default::default()
        };
        let manager = KeywordManager::with_config(config.clone());
        assert_eq!(manager.config.stop_delay_ms, 100);
        assert_eq!(manager.config.restart_delay_ms, 200);
    }

    #[test]
    fn test_should_enable_monitor_after_trigger_update() {
        assert!(should_enable_monitor_after_trigger_update(false, 1));
        assert!(!should_enable_monitor_after_trigger_update(false, 0));
        assert!(!should_enable_monitor_after_trigger_update(true, 1));
    }

    #[test]
    fn test_register_trigger_manually() {
        let mut manager = KeywordManager::new();

        manager.register_trigger(":test", "Test Snippet", "Hello, World!", "paste");

        assert_eq!(manager.trigger_count(), 1);

        let triggers = manager.list_triggers();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].0, ":test");
        assert_eq!(triggers[0].1, "Test Snippet");
    }

    #[test]
    fn test_register_empty_trigger_ignored() {
        let mut manager = KeywordManager::new();

        manager.register_trigger("", "Empty", "Content", "paste");

        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_clear_triggers() {
        let mut manager = KeywordManager::new();

        manager.register_trigger(":a", "A", "Content A", "paste");
        manager.register_trigger(":b", "B", "Content B", "paste");

        assert_eq!(manager.trigger_count(), 2);

        manager.clear_triggers();

        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_list_triggers() {
        let mut manager = KeywordManager::new();

        manager.register_trigger(":sig", "Signature", "Best regards", "paste");
        manager.register_trigger(":addr", "Address", "123 Main St", "type");

        let triggers = manager.list_triggers();
        assert_eq!(triggers.len(), 2);

        // Check both triggers exist (order not guaranteed due to HashMap)
        let trigger_names: Vec<_> = triggers.iter().map(|(t, _)| t.as_str()).collect();
        assert!(trigger_names.contains(&":sig"));
        assert!(trigger_names.contains(&":addr"));
    }

    #[test]
    fn test_accessibility_check_does_not_panic() {
        // Just verify it doesn't panic - actual result depends on system
        let _ = KeywordManager::has_accessibility_permission();
    }

    // ========================================
    // Unregister Trigger Tests
    // ========================================

    #[test]
    fn test_unregister_trigger() {
        let mut manager = KeywordManager::new();

        manager.register_trigger(":test", "Test Snippet", "Hello, World!", "paste");
        assert_eq!(manager.trigger_count(), 1);

        // Unregister the trigger
        let removed = manager.unregister_trigger(":test");
        assert!(removed);
        assert_eq!(manager.trigger_count(), 0);

        // Verify it's not in the list
        let triggers = manager.list_triggers();
        assert!(triggers.is_empty());
    }

    #[test]
    fn test_unregister_nonexistent_trigger() {
        let mut manager = KeywordManager::new();

        let removed = manager.unregister_trigger(":nonexistent");
        assert!(!removed);
    }

    #[test]
    fn test_unregister_one_of_multiple_triggers() {
        let mut manager = KeywordManager::new();

        manager.register_trigger(":a", "A", "Content A", "paste");
        manager.register_trigger(":b", "B", "Content B", "paste");
        manager.register_trigger(":c", "C", "Content C", "paste");

        assert_eq!(manager.trigger_count(), 3);

        // Unregister just one
        let removed = manager.unregister_trigger(":b");
        assert!(removed);
        assert_eq!(manager.trigger_count(), 2);

        // Verify the right ones remain
        let triggers = manager.list_triggers();
        let trigger_names: Vec<_> = triggers.iter().map(|(t, _)| t.as_str()).collect();
        assert!(trigger_names.contains(&":a"));
        assert!(!trigger_names.contains(&":b"));
        assert!(trigger_names.contains(&":c"));
    }

    // ========================================
    // Clear Triggers For File Tests
    // ========================================

    #[test]
    fn test_clear_triggers_for_file() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/scriptlets/test.md");

        // Register triggers from a file
        manager.register_trigger_from_file(":sig", "Signature", "Best regards", "paste", &path);
        manager.register_trigger_from_file(":addr", "Address", "123 Main St", "paste", &path);

        assert_eq!(manager.trigger_count(), 2);
        assert_eq!(manager.get_triggers_for_file(&path).len(), 2);

        // Clear triggers for the file
        let cleared = manager.clear_triggers_for_file(&path);
        assert_eq!(cleared, 2);
        assert_eq!(manager.trigger_count(), 0);
        assert!(manager.get_triggers_for_file(&path).is_empty());
    }

    #[test]
    fn test_clear_triggers_for_file_only_affects_that_file() {
        let mut manager = KeywordManager::new();
        let path1 = PathBuf::from("/test/file1.md");
        let path2 = PathBuf::from("/test/file2.md");

        // Register triggers from two different files
        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path1);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path1);
        manager.register_trigger_from_file(":c", "C", "Content C", "paste", &path2);

        assert_eq!(manager.trigger_count(), 3);

        // Clear triggers for file1 only
        let cleared = manager.clear_triggers_for_file(&path1);
        assert_eq!(cleared, 2);
        assert_eq!(manager.trigger_count(), 1);

        // Verify file2's trigger is still there
        let triggers = manager.list_triggers();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].0, ":c");
    }

    #[test]
    fn test_clear_triggers_for_nonexistent_file() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/nonexistent.md");

        let cleared = manager.clear_triggers_for_file(&path);
        assert_eq!(cleared, 0);
    }

    // ========================================
    // Update Triggers For File Tests
    // ========================================

    #[test]
    fn test_update_triggers_add_new() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        // Start with no triggers
        assert_eq!(manager.trigger_count(), 0);

        // Add new triggers
        let new_triggers = vec![
            (
                ":a".to_string(),
                "A".to_string(),
                "Content A".to_string(),
                "paste".to_string(),
            ),
            (
                ":b".to_string(),
                "B".to_string(),
                "Content B".to_string(),
                "paste".to_string(),
            ),
        ];

        let (added, removed, updated) = manager.update_triggers_for_file(&path, &new_triggers);

        assert_eq!(added, 2);
        assert_eq!(removed, 0);
        assert_eq!(updated, 0);
        assert_eq!(manager.trigger_count(), 2);
    }

    #[test]
    fn test_update_triggers_remove_old() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        // Start with two triggers
        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path);
        assert_eq!(manager.trigger_count(), 2);

        // Update with only one trigger (removes :b)
        let new_triggers = vec![(
            ":a".to_string(),
            "A".to_string(),
            "Content A".to_string(),
            "paste".to_string(),
        )];

        let (added, removed, updated) = manager.update_triggers_for_file(&path, &new_triggers);

        assert_eq!(added, 0);
        assert_eq!(removed, 1);
        assert_eq!(updated, 0);
        assert_eq!(manager.trigger_count(), 1);

        let triggers = manager.list_triggers();
        assert_eq!(triggers[0].0, ":a");
    }

    #[test]
    fn test_update_triggers_change_content() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        // Start with a trigger
        manager.register_trigger_from_file(":sig", "Signature", "Old content", "paste", &path);
        assert_eq!(manager.trigger_count(), 1);

        // Update with changed content
        let new_triggers = vec![(
            ":sig".to_string(),
            "Signature".to_string(),
            "New content".to_string(),
            "paste".to_string(),
        )];

        let (added, removed, updated) = manager.update_triggers_for_file(&path, &new_triggers);

        assert_eq!(added, 0);
        assert_eq!(removed, 0);
        assert_eq!(updated, 1);
        assert_eq!(manager.trigger_count(), 1);
    }

    #[test]
    fn test_update_triggers_mixed_operations() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        // Start with triggers :a, :b, :c
        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path);
        manager.register_trigger_from_file(":c", "C", "Content C", "paste", &path);
        assert_eq!(manager.trigger_count(), 3);

        // Update:
        // - Keep :a unchanged
        // - Remove :b
        // - Change :c content
        // - Add :d
        let new_triggers = vec![
            (
                ":a".to_string(),
                "A".to_string(),
                "Content A".to_string(),
                "paste".to_string(),
            ),
            (
                ":c".to_string(),
                "C".to_string(),
                "New content C".to_string(),
                "paste".to_string(),
            ),
            (
                ":d".to_string(),
                "D".to_string(),
                "Content D".to_string(),
                "paste".to_string(),
            ),
        ];

        let (added, removed, updated) = manager.update_triggers_for_file(&path, &new_triggers);

        assert_eq!(added, 1); // :d
        assert_eq!(removed, 1); // :b
        assert_eq!(updated, 1); // :c
        assert_eq!(manager.trigger_count(), 3);

        let triggers = manager.list_triggers();
        let trigger_names: Vec<_> = triggers.iter().map(|(t, _)| t.as_str()).collect();
        assert!(trigger_names.contains(&":a"));
        assert!(!trigger_names.contains(&":b"));
        assert!(trigger_names.contains(&":c"));
        assert!(trigger_names.contains(&":d"));
    }

    #[test]
    fn test_update_triggers_empty_removes_all() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        // Start with triggers
        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path);
        assert_eq!(manager.trigger_count(), 2);

        // Update with empty list
        let new_triggers: Vec<(String, String, String, String)> = vec![];

        let (added, removed, updated) = manager.update_triggers_for_file(&path, &new_triggers);

        assert_eq!(added, 0);
        assert_eq!(removed, 2);
        assert_eq!(updated, 0);
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_update_triggers_does_not_affect_other_files() {
        let mut manager = KeywordManager::new();
        let path1 = PathBuf::from("/test/file1.md");
        let path2 = PathBuf::from("/test/file2.md");

        // Register triggers from two files
        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path1);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path2);
        assert_eq!(manager.trigger_count(), 2);

        // Update file1 to remove its trigger
        let new_triggers: Vec<(String, String, String, String)> = vec![];
        manager.update_triggers_for_file(&path1, &new_triggers);

        // File2's trigger should still exist
        assert_eq!(manager.trigger_count(), 1);
        let triggers = manager.list_triggers();
        assert_eq!(triggers[0].0, ":b");
    }

    // ========================================
    // Register Trigger From File Tests
    // ========================================

    #[test]
    fn test_register_trigger_from_file() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        manager.register_trigger_from_file(":test", "Test", "Content", "paste", &path);

        assert_eq!(manager.trigger_count(), 1);
        assert_eq!(
            manager.get_triggers_for_file(&path),
            vec![":test".to_string()]
        );
    }

    #[test]
    fn test_register_trigger_from_file_empty_ignored() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        manager.register_trigger_from_file("", "Test", "Content", "paste", &path);

        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_get_triggers_for_file() {
        let mut manager = KeywordManager::new();
        let path = PathBuf::from("/test/file.md");

        manager.register_trigger_from_file(":a", "A", "Content A", "paste", &path);
        manager.register_trigger_from_file(":b", "B", "Content B", "paste", &path);

        let triggers = manager.get_triggers_for_file(&path);
        assert_eq!(triggers.len(), 2);
        assert!(triggers.contains(&":a".to_string()));
        assert!(triggers.contains(&":b".to_string()));
    }

    // Integration tests that require system permissions
    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_enable_disable_cycle() {
        let mut manager = KeywordManager::new();
        manager.register_trigger(":test", "Test", "Content", "paste");

        assert!(manager.enable().is_ok());
        assert!(manager.is_enabled());

        manager.disable();
        assert!(!manager.is_enabled());
    }
}
