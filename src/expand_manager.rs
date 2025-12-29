//! Expand Manager - Text expansion system integration
//!
//! This module ties together all the components of the text expansion system:
//! - KeyboardMonitor: Global keystroke capture
//! - ExpandMatcher: Trigger detection with rolling buffer
//! - TextInjector: Backspace deletion + clipboard paste
//! - Scriptlets: Source of expand triggers and replacement text
//!
//! # Architecture
//!
//! The ExpandManager:
//! 1. Loads scriptlets with `expand` metadata from ~/.kenv/scriptlets/
//! 2. Registers each expand trigger with the ExpandMatcher
//! 3. Starts the KeyboardMonitor with a callback that feeds keystrokes to the matcher
//! 4. When a match is found, performs the expansion:
//!    a. Stops keyboard monitor (avoid capturing our own keystrokes)
//!    b. Deletes trigger characters with backspaces
//!    c. Pastes replacement text via clipboard
//!    d. Resumes keyboard monitor
//!
//! # Example
//!
//! ```ignore
//! use script_kit_gpui::expand_manager::ExpandManager;
//!
//! let mut manager = ExpandManager::new();
//! manager.load_scriptlets()?;  // Loads triggers from ~/.kenv/scriptlets/
//! manager.enable()?;           // Starts keyboard monitoring
//! // ... user types ":sig" in any app ...
//! // ... manager detects match and expands to signature text ...
//! manager.disable();           // Stops keyboard monitoring
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use tracing::{debug, error, info, instrument, warn};

// Import from crate (these are declared in main.rs)
use crate::expand_matcher::ExpandMatcher;
use crate::keyboard_monitor::{KeyboardMonitor, KeyboardMonitorError, KeyEvent};
use crate::scripts::read_scriptlets;
use crate::template_variables::substitute_variables;
use crate::text_injector::{TextInjector, TextInjectorConfig};

/// Delay after stopping monitor before performing expansion (ms)
const STOP_DELAY_MS: u64 = 50;

/// Delay after expansion before restarting monitor (ms)
const RESTART_DELAY_MS: u64 = 100;

/// Configuration for the expand manager
#[derive(Debug, Clone)]
pub struct ExpandManagerConfig {
    /// Configuration for text injection timing
    pub injector_config: TextInjectorConfig,
    /// Delay after stopping monitor before expansion (ms)
    pub stop_delay_ms: u64,
    /// Delay after expansion before restarting monitor (ms)
    #[allow(dead_code)]
    pub restart_delay_ms: u64,
}

impl Default for ExpandManagerConfig {
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
struct ExpandScriptlet {
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
pub struct ExpandManager {
    /// Configuration
    config: ExpandManagerConfig,
    /// Registered scriptlets by trigger keyword
    scriptlets: Arc<Mutex<HashMap<String, ExpandScriptlet>>>,
    /// The expand matcher for trigger detection
    matcher: Arc<Mutex<ExpandMatcher>>,
    /// The keyboard monitor (optional - created on enable)
    monitor: Option<KeyboardMonitor>,
    /// The text injector (reserved for future direct use)
    #[allow(dead_code)]
    injector: TextInjector,
    /// Whether the expand system is enabled
    enabled: bool,
}

impl ExpandManager {
    /// Create a new ExpandManager with default configuration
    pub fn new() -> Self {
        Self::with_config(ExpandManagerConfig::default())
    }

    /// Create a new ExpandManager with custom configuration
    pub fn with_config(config: ExpandManagerConfig) -> Self {
        let injector = TextInjector::with_config(config.injector_config.clone());
        
        Self {
            config,
            scriptlets: Arc::new(Mutex::new(HashMap::new())),
            matcher: Arc::new(Mutex::new(ExpandMatcher::new())),
            monitor: None,
            injector,
            enabled: false,
        }
    }

    /// Load scriptlets with expand metadata from ~/.kenv/scriptlets/
    ///
    /// This scans all markdown files and registers any scriptlet that has
    /// an `expand` metadata field as a trigger.
    #[instrument(skip(self))]
    pub fn load_scriptlets(&mut self) -> Result<usize> {
        info!("Loading scriptlets with expand triggers");
        
        let scriptlets = read_scriptlets();
        let mut loaded_count = 0;
        
        for scriptlet in scriptlets {
            // Only process scriptlets with expand metadata
            if let Some(ref expand_trigger) = scriptlet.expand {
                if expand_trigger.is_empty() {
                    debug!(
                        name = %scriptlet.name,
                        "Skipping scriptlet with empty expand trigger"
                    );
                    continue;
                }
                
                info!(
                    trigger = %expand_trigger,
                    name = %scriptlet.name,
                    tool = %scriptlet.tool,
                    "Registering expand trigger"
                );
                
                // Store the scriptlet info
                let expand_scriptlet = ExpandScriptlet {
                    trigger: expand_trigger.clone(),
                    name: scriptlet.name.clone(),
                    content: scriptlet.code.clone(),
                    tool: scriptlet.tool.clone(),
                    source_path: scriptlet.file_path.clone(),
                };
                
                // Register with matcher and scriptlets store
                {
                    let mut scriptlets_guard = self.scriptlets.lock().unwrap();
                    scriptlets_guard.insert(expand_trigger.clone(), expand_scriptlet);
                }
                
                {
                    let mut matcher_guard = self.matcher.lock().unwrap();
                    // Use a dummy path since we store scriptlet data separately
                    let dummy_path = PathBuf::from(format!("scriptlet:{}", scriptlet.name));
                    matcher_guard.register_trigger(expand_trigger, dummy_path);
                }
                
                loaded_count += 1;
            }
        }
        
        info!(count = loaded_count, "Loaded expand triggers from scriptlets");
        Ok(loaded_count)
    }

    /// Register a single expand trigger manually
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
            "Manually registering expand trigger"
        );
        
        let expand_scriptlet = ExpandScriptlet {
            trigger: trigger.to_string(),
            name: name.to_string(),
            content: content.to_string(),
            tool: tool.to_string(),
            source_path: None,
        };
        
        {
            let mut scriptlets_guard = self.scriptlets.lock().unwrap();
            scriptlets_guard.insert(trigger.to_string(), expand_scriptlet);
        }
        
        {
            let mut matcher_guard = self.matcher.lock().unwrap();
            let dummy_path = PathBuf::from(format!("manual:{}", name));
            matcher_guard.register_trigger(trigger, dummy_path);
        }
    }

    /// Enable the expand system (start keyboard monitoring)
    ///
    /// # Errors
    /// - `AccessibilityNotGranted`: Accessibility permissions not enabled
    /// - `EventTapCreationFailed`: Failed to create macOS event tap
    #[instrument(skip(self))]
    pub fn enable(&mut self) -> Result<(), KeyboardMonitorError> {
        if self.enabled {
            debug!("Expand system already enabled");
            return Ok(());
        }
        
        info!("Enabling expand system");
        
        // Check trigger count
        let trigger_count = {
            let matcher_guard = self.matcher.lock().unwrap();
            matcher_guard.trigger_count()
        };
        
        if trigger_count == 0 {
            warn!("No expand triggers registered, keyboard monitoring will be ineffective");
        }
        
        // Clone Arc references for the closure
        let matcher = Arc::clone(&self.matcher);
        let scriptlets = Arc::clone(&self.scriptlets);
        let config = self.config.clone();
        let injector_config = self.config.injector_config.clone();
        
        // Create keyboard monitor with callback
        let mut monitor = KeyboardMonitor::new(move |event: KeyEvent| {
            // Log every keystroke for debugging
            debug!(
                character = ?event.character,
                key_code = event.key_code,
                command = event.command,
                control = event.control,
                option = event.option,
                "Keyboard event received"
            );
            
            // Only process printable characters (ignore modifier keys, etc.)
            if let Some(ref character) = event.character {
                // Skip if any modifier is held (except shift for capitals)
                if event.command || event.control || event.option {
                    debug!(character = %character, "Skipping due to modifier key");
                    return;
                }
                
                // Process each character in the string (usually just 1)
                for c in character.chars() {
                    debug!(char = ?c, "Processing character");
                    // Feed to matcher
                    let match_result = {
                        let mut matcher_guard = matcher.lock().unwrap();
                        matcher_guard.process_keystroke(c)
                    };
                    
                    // Handle match if found
                    if let Some(result) = match_result {
                        debug!(
                            trigger = %result.trigger,
                            chars_to_delete = result.chars_to_delete,
                            "Trigger matched, performing expansion"
                        );
                        
                        // Get the scriptlet content
                        let scriptlet_opt = {
                            let scriptlets_guard = scriptlets.lock().unwrap();
                            scriptlets_guard.get(&result.trigger).cloned()
                        };
                        
                        if let Some(scriptlet) = scriptlet_opt {
                            // Perform expansion in a separate thread to not block the callback
                            let chars_to_delete = result.chars_to_delete;
                            let content = scriptlet.content.clone();
                            let tool = scriptlet.tool.clone();
                            let name = scriptlet.name.clone();
                            let config_clone = config.clone();
                            let injector_config_clone = injector_config.clone();
                            
                            thread::spawn(move || {
                                // Small delay to let the keyboard event complete
                                thread::sleep(Duration::from_millis(config_clone.stop_delay_ms));
                                
                                // Get raw content based on tool type
                                let raw_content = match tool.as_str() {
                                    "paste" | "type" | "template" => {
                                        content.clone()
                                    }
                                    _ => {
                                        // For other tools, use the content as-is for now
                                        // Future: execute the scriptlet and capture output
                                        info!(
                                            tool = %tool,
                                            name = %name,
                                            "Tool type not yet fully supported for expand, using raw content"
                                        );
                                        content.clone()
                                    }
                                };
                                
                                // Substitute template variables (${clipboard}, ${date}, etc.)
                                // Uses the centralized template_variables module
                                let replacement = substitute_variables(&raw_content);
                                
                                debug!(
                                    original_len = raw_content.len(),
                                    substituted_len = replacement.len(),
                                    had_substitutions = raw_content != replacement,
                                    "Variable substitution completed"
                                );
                                
                                // Create injector and perform expansion
                                let injector = TextInjector::with_config(injector_config_clone);
                                
                                // Delete trigger characters
                                if let Err(e) = injector.delete_chars(chars_to_delete) {
                                    error!(
                                        error = %e,
                                        chars = chars_to_delete,
                                        "Failed to delete trigger characters"
                                    );
                                    return;
                                }
                                
                                // Small delay between delete and paste
                                thread::sleep(Duration::from_millis(50));
                                
                                // Paste replacement text
                                if let Err(e) = injector.paste_text(&replacement) {
                                    error!(
                                        error = %e,
                                        "Failed to paste replacement text"
                                    );
                                    return;
                                }
                                
                                info!(
                                    trigger = %name,
                                    replacement_len = replacement.len(),
                                    "Expansion completed successfully"
                                );
                            });
                            
                            // Clear the buffer after a match to prevent re-triggering
                            let mut matcher_guard = matcher.lock().unwrap();
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
        
        info!("Expand system enabled, keyboard monitoring active");
        Ok(())
    }

    /// Disable the expand system (stop keyboard monitoring)
    #[instrument(skip(self))]
    pub fn disable(&mut self) {
        if !self.enabled {
            debug!("Expand system already disabled");
            return;
        }
        
        info!("Disabling expand system");
        
        if let Some(ref mut monitor) = self.monitor {
            monitor.stop();
        }
        self.monitor = None;
        self.enabled = false;
        
        info!("Expand system disabled");
    }

    /// Check if the expand system is currently enabled
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the number of registered triggers
    #[allow(dead_code)]
    pub fn trigger_count(&self) -> usize {
        let matcher_guard = self.matcher.lock().unwrap();
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
            let mut scriptlets_guard = self.scriptlets.lock().unwrap();
            scriptlets_guard.clear();
        }
        {
            let mut matcher_guard = self.matcher.lock().unwrap();
            matcher_guard.clear_triggers();
        }
        
        debug!("All expand triggers cleared");
    }

    /// Reload scriptlets (clear existing and load fresh)
    #[allow(dead_code)]
    #[instrument(skip(self))]
    pub fn reload(&mut self) -> Result<usize> {
        info!("Reloading expand scriptlets");
        
        self.clear_triggers();
        self.load_scriptlets()
    }

    /// Get list of all registered triggers (for debugging/UI)
    pub fn list_triggers(&self) -> Vec<(String, String)> {
        let scriptlets_guard = self.scriptlets.lock().unwrap();
        scriptlets_guard
            .iter()
            .map(|(trigger, scriptlet)| (trigger.clone(), scriptlet.name.clone()))
            .collect()
    }
}

impl Default for ExpandManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpandManager {
    fn drop(&mut self) {
        self.disable();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_disabled_manager() {
        let manager = ExpandManager::new();
        assert!(!manager.is_enabled());
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_default_creates_disabled_manager() {
        let manager = ExpandManager::default();
        assert!(!manager.is_enabled());
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_custom_config() {
        let config = ExpandManagerConfig {
            stop_delay_ms: 100,
            restart_delay_ms: 200,
            ..Default::default()
        };
        let manager = ExpandManager::with_config(config.clone());
        assert_eq!(manager.config.stop_delay_ms, 100);
        assert_eq!(manager.config.restart_delay_ms, 200);
    }

    #[test]
    fn test_register_trigger_manually() {
        let mut manager = ExpandManager::new();
        
        manager.register_trigger(":test", "Test Snippet", "Hello, World!", "paste");
        
        assert_eq!(manager.trigger_count(), 1);
        
        let triggers = manager.list_triggers();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].0, ":test");
        assert_eq!(triggers[0].1, "Test Snippet");
    }

    #[test]
    fn test_register_empty_trigger_ignored() {
        let mut manager = ExpandManager::new();
        
        manager.register_trigger("", "Empty", "Content", "paste");
        
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_clear_triggers() {
        let mut manager = ExpandManager::new();
        
        manager.register_trigger(":a", "A", "Content A", "paste");
        manager.register_trigger(":b", "B", "Content B", "paste");
        
        assert_eq!(manager.trigger_count(), 2);
        
        manager.clear_triggers();
        
        assert_eq!(manager.trigger_count(), 0);
    }

    #[test]
    fn test_list_triggers() {
        let mut manager = ExpandManager::new();
        
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
        let _ = ExpandManager::has_accessibility_permission();
    }

    // Integration tests that require system permissions
    #[test]
    #[ignore = "Requires accessibility permissions"]
    fn test_enable_disable_cycle() {
        let mut manager = ExpandManager::new();
        manager.register_trigger(":test", "Test", "Content", "paste");
        
        assert!(manager.enable().is_ok());
        assert!(manager.is_enabled());
        
        manager.disable();
        assert!(!manager.is_enabled());
    }
}
