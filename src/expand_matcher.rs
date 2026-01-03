//! Expand trigger matching module
//!
//! This module provides a trigger detection system that buffers typed characters
//! and matches against registered expand triggers. When a trigger keyword is
//! fully typed, a match is returned with information needed to perform the
//! text expansion.
//!

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// Default maximum buffer size for typed characters
const DEFAULT_MAX_BUFFER_SIZE: usize = 50;

/// Characters that should clear the buffer when typed
const BUFFER_CLEAR_CHARS: &[char] = &[
    '\n',   // Enter/Return
    '\r',   // Carriage return
    '\x1b', // Escape
    '\t',   // Tab (optional, but common delimiter)
];

/// Result of a successful trigger match
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    /// The trigger keyword that was matched (e.g., ":sig")
    pub trigger: String,
    /// Path to the scriptlet to execute
    pub scriptlet_path: PathBuf,
    /// Number of characters to delete (length of trigger)
    pub chars_to_delete: usize,
}

/// Trigger detection system for text expansion
///
/// Maintains a rolling buffer of typed characters and checks for matches
/// against registered triggers. Uses immediate matching - the trigger fires
/// as soon as the keyword is fully typed.
#[derive(Debug)]
pub struct ExpandMatcher {
    /// Map of trigger keywords to their scriptlet paths
    triggers: HashMap<String, PathBuf>,
    /// Rolling buffer of recent keystrokes
    buffer: String,
    /// Maximum size of the buffer
    max_buffer_size: usize,
}

impl Default for ExpandMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpandMatcher {
    /// Create a new ExpandMatcher with default settings
    pub fn new() -> Self {
        Self {
            triggers: HashMap::new(),
            buffer: String::with_capacity(DEFAULT_MAX_BUFFER_SIZE),
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
        }
    }

    /// Create a new ExpandMatcher with a custom buffer size
    #[allow(dead_code)]
    pub fn with_buffer_size(max_size: usize) -> Self {
        Self {
            triggers: HashMap::new(),
            buffer: String::with_capacity(max_size),
            max_buffer_size: max_size,
        }
    }

    /// Register a trigger keyword with its associated scriptlet path
    ///
    /// If the trigger already exists, it will be replaced.
    ///
    /// # Arguments
    /// * `keyword` - The trigger text (e.g., ":sig", "!today", "/date")
    /// * `scriptlet_path` - Path to the scriptlet file to execute on match
    pub fn register_trigger(&mut self, keyword: &str, scriptlet_path: PathBuf) {
        if keyword.is_empty() {
            debug!("Attempted to register empty trigger, ignoring");
            return;
        }

        debug!(
            trigger = %keyword,
            path = %scriptlet_path.display(),
            "Registering expand trigger"
        );

        self.triggers.insert(keyword.to_string(), scriptlet_path);
    }

    /// Unregister a trigger keyword
    ///
    /// # Arguments
    /// * `keyword` - The trigger text to remove
    ///
    /// # Returns
    /// `true` if the trigger was removed, `false` if it didn't exist
    #[allow(dead_code)]
    pub fn unregister_trigger(&mut self, keyword: &str) -> bool {
        let removed = self.triggers.remove(keyword).is_some();

        if removed {
            debug!(trigger = %keyword, "Unregistered expand trigger");
        }

        removed
    }

    /// Process a keystroke and check for trigger matches
    ///
    /// # Arguments
    /// * `c` - The character that was typed
    ///
    /// # Returns
    /// `Some(MatchResult)` if a trigger was matched, `None` otherwise
    pub fn process_keystroke(&mut self, c: char) -> Option<MatchResult> {
        // Check for buffer-clearing characters
        if BUFFER_CLEAR_CHARS.contains(&c) {
            self.clear_buffer();
            return None;
        }

        // Add character to buffer
        self.buffer.push(c);

        // Trim buffer if it exceeds max size (remove from front)
        if self.buffer.len() > self.max_buffer_size {
            let excess = self.buffer.len() - self.max_buffer_size;
            self.buffer = self.buffer.chars().skip(excess).collect();
        }

        // Check for matches - look for triggers at the end of the buffer
        self.check_for_match()
    }

    /// Check if the buffer ends with any registered trigger
    fn check_for_match(&self) -> Option<MatchResult> {
        // Check each trigger to see if the buffer ends with it
        for (trigger, path) in &self.triggers {
            if self.buffer.ends_with(trigger) {
                debug!(
                    trigger = %trigger,
                    buffer = %self.buffer,
                    "Trigger matched"
                );

                return Some(MatchResult {
                    trigger: trigger.clone(),
                    scriptlet_path: path.clone(),
                    chars_to_delete: trigger.chars().count(),
                });
            }
        }

        None
    }

    /// Clear the keystroke buffer
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
        debug!("Buffer cleared");
    }

    /// Get the current buffer contents (for debugging)
    #[allow(dead_code)]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Get the number of registered triggers
    pub fn trigger_count(&self) -> usize {
        self.triggers.len()
    }

    /// Check if a specific trigger is registered
    #[allow(dead_code)]
    pub fn has_trigger(&self, keyword: &str) -> bool {
        self.triggers.contains_key(keyword)
    }

    /// Get all registered triggers
    #[allow(dead_code)]
    pub fn triggers(&self) -> impl Iterator<Item = (&String, &PathBuf)> {
        self.triggers.iter()
    }

    /// Clear all registered triggers
    #[allow(dead_code)]
    pub fn clear_triggers(&mut self) {
        self.triggers.clear();
        debug!("All triggers cleared");
    }

    /// Bulk register triggers from an iterator
    #[allow(dead_code)]
    pub fn register_triggers<I>(&mut self, triggers: I)
    where
        I: IntoIterator<Item = (String, PathBuf)>,
    {
        for (keyword, path) in triggers {
            self.register_trigger(&keyword, path);
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // Construction Tests
    // ========================================

    #[test]
    fn test_new_creates_empty_matcher() {
        let matcher = ExpandMatcher::new();
        assert_eq!(matcher.trigger_count(), 0);
        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_default_creates_empty_matcher() {
        let matcher = ExpandMatcher::default();
        assert_eq!(matcher.trigger_count(), 0);
        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_with_buffer_size_sets_custom_size() {
        let matcher = ExpandMatcher::with_buffer_size(100);
        assert_eq!(matcher.max_buffer_size, 100);
    }

    // ========================================
    // Registration Tests
    // ========================================

    #[test]
    fn test_register_trigger_adds_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        assert_eq!(matcher.trigger_count(), 1);
        assert!(matcher.has_trigger(":sig"));
    }

    #[test]
    fn test_register_multiple_triggers() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));
        matcher.register_trigger("!today", PathBuf::from("/test/today.md"));
        matcher.register_trigger("/date", PathBuf::from("/test/date.md"));

        assert_eq!(matcher.trigger_count(), 3);
        assert!(matcher.has_trigger(":sig"));
        assert!(matcher.has_trigger("!today"));
        assert!(matcher.has_trigger("/date"));
    }

    #[test]
    fn test_register_empty_trigger_ignored() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("", PathBuf::from("/test/empty.md"));

        assert_eq!(matcher.trigger_count(), 0);
    }

    #[test]
    fn test_register_trigger_replaces_existing() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig1.md"));
        matcher.register_trigger(":sig", PathBuf::from("/test/sig2.md"));

        assert_eq!(matcher.trigger_count(), 1);

        // Should use the new path
        for c in ":sig".chars() {
            matcher.process_keystroke(c);
        }
        // The last registration should be used
    }

    #[test]
    fn test_unregister_trigger_removes_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        assert!(matcher.unregister_trigger(":sig"));
        assert_eq!(matcher.trigger_count(), 0);
        assert!(!matcher.has_trigger(":sig"));
    }

    #[test]
    fn test_unregister_nonexistent_returns_false() {
        let mut matcher = ExpandMatcher::new();

        assert!(!matcher.unregister_trigger(":nonexistent"));
    }

    #[test]
    fn test_clear_triggers_removes_all() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));
        matcher.register_trigger("!today", PathBuf::from("/test/today.md"));

        matcher.clear_triggers();

        assert_eq!(matcher.trigger_count(), 0);
    }

    #[test]
    fn test_bulk_register_triggers() {
        let mut matcher = ExpandMatcher::new();
        let triggers = vec![
            (":sig".to_string(), PathBuf::from("/test/sig.md")),
            ("!today".to_string(), PathBuf::from("/test/today.md")),
        ];

        matcher.register_triggers(triggers);

        assert_eq!(matcher.trigger_count(), 2);
        assert!(matcher.has_trigger(":sig"));
        assert!(matcher.has_trigger("!today"));
    }

    // ========================================
    // Basic Matching Tests
    // ========================================

    #[test]
    fn test_process_keystroke_no_match_without_triggers() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }
    }

    #[test]
    fn test_process_keystroke_matches_simple_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type ":sig"
        assert!(matcher.process_keystroke(':').is_none());
        assert!(matcher.process_keystroke('s').is_none());
        assert!(matcher.process_keystroke('i').is_none());

        let result = matcher.process_keystroke('g');
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.trigger, ":sig");
        assert_eq!(result.chars_to_delete, 4);
        assert_eq!(result.scriptlet_path, PathBuf::from("/test/sig.md"));
    }

    #[test]
    fn test_match_result_chars_to_delete_counts_chars_not_bytes() {
        let mut matcher = ExpandMatcher::new();
        // Unicode trigger
        matcher.register_trigger("✓ok", PathBuf::from("/test/ok.md"));

        for c in "✓ok".chars() {
            matcher.process_keystroke(c);
        }

        // Would have matched on 'k'
        matcher.clear_buffer();

        for c in "✓o".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        let result = matcher.process_keystroke('k');
        assert!(result.is_some());

        let result = result.unwrap();
        // "✓ok" is 3 chars (not 5 bytes)
        assert_eq!(result.chars_to_delete, 3);
    }

    #[test]
    fn test_match_fires_immediately_when_complete() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type "Hello :sig" - match should fire right after 'g'
        for c in "Hello :si".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        let result = matcher.process_keystroke('g');
        assert!(result.is_some());
    }

    // ========================================
    // Buffer Behavior Tests
    // ========================================

    #[test]
    fn test_buffer_stores_keystrokes() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        assert_eq!(matcher.buffer(), "hello");
    }

    #[test]
    fn test_buffer_clears_on_enter() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        matcher.process_keystroke('\n');

        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_buffer_clears_on_carriage_return() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        matcher.process_keystroke('\r');

        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_buffer_clears_on_escape() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        matcher.process_keystroke('\x1b');

        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_buffer_clears_on_tab() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        matcher.process_keystroke('\t');

        assert!(matcher.buffer().is_empty());
    }

    #[test]
    fn test_buffer_does_not_clear_on_space() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello world".chars() {
            matcher.process_keystroke(c);
        }

        assert_eq!(matcher.buffer(), "hello world");
    }

    #[test]
    fn test_buffer_trims_when_exceeds_max_size() {
        let mut matcher = ExpandMatcher::with_buffer_size(10);

        for c in "12345678901234567890".chars() {
            matcher.process_keystroke(c);
        }

        // Should only keep the last 10 characters
        assert_eq!(matcher.buffer().len(), 10);
        assert_eq!(matcher.buffer(), "1234567890");
    }

    #[test]
    fn test_clear_buffer_empties_buffer() {
        let mut matcher = ExpandMatcher::new();

        for c in "hello".chars() {
            matcher.process_keystroke(c);
        }

        matcher.clear_buffer();

        assert!(matcher.buffer().is_empty());
    }

    // ========================================
    // Trigger Prefix Tests
    // ========================================

    #[test]
    fn test_colon_prefix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        for c in "hello :sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig");
    }

    #[test]
    fn test_exclamation_prefix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("!today", PathBuf::from("/test/today.md"));

        for c in "hello !today".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, "!today");
                return;
            }
        }
        panic!("Expected match for !today");
    }

    #[test]
    fn test_slash_prefix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("/date", PathBuf::from("/test/date.md"));

        for c in "hello /date".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, "/date");
                return;
            }
        }
        panic!("Expected match for /date");
    }

    #[test]
    fn test_double_comma_suffix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("sig,,", PathBuf::from("/test/sig.md"));

        for c in "hello sig,,".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, "sig,,");
                assert_eq!(result.chars_to_delete, 5);
                return;
            }
        }
        panic!("Expected match for sig,,");
    }

    #[test]
    fn test_semicolon_suffix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("email;", PathBuf::from("/test/email.md"));

        for c in "email;".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, "email;");
                return;
            }
        }
        panic!("Expected match for email;");
    }

    #[test]
    fn test_no_prefix_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger("btw", PathBuf::from("/test/btw.md"));

        // Should match "btw" even without prefix
        for c in "btw".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, "btw");
                return;
            }
        }
        panic!("Expected match for btw");
    }

    // ========================================
    // Edge Cases and Complex Scenarios
    // ========================================

    #[test]
    fn test_partial_match_then_complete_different_trigger() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));
        matcher.register_trigger(":sign", PathBuf::from("/test/sign.md"));

        // Type ":sig" - should match first
        for c in ":sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig");
    }

    #[test]
    fn test_longer_trigger_preferred_when_both_match() {
        // Note: This test documents current behavior - first match wins
        // If we want longest match, implementation would need to change
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));
        matcher.register_trigger(":signature", PathBuf::from("/test/signature.md"));

        // Type ":sig" - matches immediately
        for c in ":sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig");
    }

    #[test]
    fn test_match_after_buffer_clear() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type some text, then Enter (clears buffer), then trigger
        for c in "hello\n:sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig");
    }

    #[test]
    fn test_no_match_when_trigger_split_by_clear() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type ":si", then Enter (clears buffer), then "g"
        for c in ":si".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        matcher.process_keystroke('\n'); // Clear buffer

        // "g" alone shouldn't match
        assert!(matcher.process_keystroke('g').is_none());
    }

    #[test]
    fn test_trigger_in_middle_of_sentence() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type "Please sign here :sig thanks"
        for c in "Please sign here :sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig");
    }

    #[test]
    fn test_multiple_triggers_same_text() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type ":sig" twice - should match twice
        let mut match_count = 0;

        for c in ":sig :sig".chars() {
            if matcher.process_keystroke(c).is_some() {
                match_count += 1;
            }
        }

        assert_eq!(match_count, 2);
    }

    #[test]
    fn test_trigger_with_numbers() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":addr1", PathBuf::from("/test/addr1.md"));

        for c in ":addr1".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":addr1");
                return;
            }
        }
        panic!("Expected match for :addr1");
    }

    #[test]
    fn test_case_sensitive_triggers() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":Sig", PathBuf::from("/test/sig.md"));

        // Lowercase should NOT match
        for c in ":sig".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        matcher.clear_buffer();

        // Correct case should match
        for c in ":Si".chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        let result = matcher.process_keystroke('g');
        assert!(result.is_some());
        assert_eq!(result.unwrap().trigger, ":Sig");
    }

    // ========================================
    // Iterator and Inspection Tests
    // ========================================

    #[test]
    fn test_triggers_iterator() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));
        matcher.register_trigger("!today", PathBuf::from("/test/today.md"));

        let triggers: Vec<_> = matcher.triggers().collect();

        assert_eq!(triggers.len(), 2);
    }

    #[test]
    fn test_has_trigger_returns_true_for_registered() {
        let mut matcher = ExpandMatcher::new();
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        assert!(matcher.has_trigger(":sig"));
        assert!(!matcher.has_trigger(":nonexistent"));
    }

    // ========================================
    // Integration-style Tests
    // ========================================

    #[test]
    fn test_realistic_usage_scenario() {
        let mut matcher = ExpandMatcher::new();

        // Register common text expansion triggers
        matcher.register_trigger(":sig", PathBuf::from("/scriptlets/signature.md"));
        matcher.register_trigger(":email", PathBuf::from("/scriptlets/email.md"));
        matcher.register_trigger("!date", PathBuf::from("/scriptlets/date.md"));
        matcher.register_trigger("addr,,", PathBuf::from("/scriptlets/address.md"));

        // Simulate typing an email
        let text = "Dear John,\n\nThank you for your :email regarding the project.\n\nHere is my address: addr,,\n\nBest regards,\n:sig";

        let mut matches = Vec::new();

        for c in text.chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                matches.push(result.trigger.clone());
            }
        }

        // Should have matched :email, addr,,, and :sig
        // Note: \n clears buffer, so triggers after newlines still work
        assert!(matches.contains(&":email".to_string()));
        assert!(matches.contains(&"addr,,".to_string()));
        assert!(matches.contains(&":sig".to_string()));
    }

    #[test]
    fn test_buffer_wrapping_preserves_recent_context() {
        let mut matcher = ExpandMatcher::with_buffer_size(20);
        matcher.register_trigger(":sig", PathBuf::from("/test/sig.md"));

        // Type a lot of text to cause buffer trimming
        let long_text = "This is a very long sentence that will definitely exceed the buffer size ";
        for c in long_text.chars() {
            assert!(matcher.process_keystroke(c).is_none());
        }

        // Now type the trigger - should still match because buffer keeps recent chars
        for c in ":sig".chars() {
            if let Some(result) = matcher.process_keystroke(c) {
                assert_eq!(result.trigger, ":sig");
                return;
            }
        }
        panic!("Expected match for :sig after buffer wrap");
    }
}
