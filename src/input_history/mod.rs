//! Input history for navigation through previous user inputs
//!
//! This module provides a simple history mechanism that stores previous user inputs
//! and allows navigating through them with up/down arrow keys (shell-like behavior).
//! Entries are persisted to ~/.scriptkit/input_history.json.

// --- merged from part_000.rs ---
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, info, instrument, warn};
/// Maximum number of entries to store in history
const MAX_ENTRIES: usize = 100;
/// Input history with navigation state
///
/// NOTE: Clone is intentionally NOT derived to prevent accidental data loss
/// in multi-window contexts. If you need to share InputHistory across
/// multiple owners, use `Arc<Mutex<InputHistory>>` explicitly.
#[derive(Debug, Serialize, Deserialize)]
pub struct InputHistory {
    /// Stored entries (most recent first)
    entries: Vec<String>,
    /// Remembers which result was submitted for a normalized query
    selected_results: HashMap<String, String>,
    /// Current navigation index (None = not navigating, Some(i) = at entries[i])
    /// This is ephemeral and not persisted
    #[serde(skip)]
    current_index: Option<usize>,
    /// Path to the history file (not persisted)
    #[serde(skip)]
    file_path: PathBuf,
}
impl Default for InputHistory {
    fn default() -> Self {
        Self::new()
    }
}
impl InputHistory {
    /// Create a new InputHistory with the default path (~/.scriptkit/input_history.json)
    pub fn new() -> Self {
        let file_path = Self::default_path();
        InputHistory {
            entries: Vec::new(),
            selected_results: HashMap::new(),
            current_index: None,
            file_path,
        }
    }

    /// Create an InputHistory with a custom path (for testing)
    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        InputHistory {
            entries: Vec::new(),
            selected_results: HashMap::new(),
            current_index: None,
            file_path: path,
        }
    }

    /// Get the default history file path
    fn default_path() -> PathBuf {
        PathBuf::from(shellexpand::tilde("~/.scriptkit/input_history.json").as_ref())
    }

    fn normalize_query(text: &str) -> Option<String> {
        let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            None
        } else {
            Some(collapsed.to_lowercase())
        }
    }

    fn prune_selected_results(&mut self) {
        let live_queries: HashSet<String> = self
            .entries
            .iter()
            .filter_map(|entry| Self::normalize_query(entry))
            .collect();
        self.selected_results
            .retain(|query, _| live_queries.contains(query));
    }

    /// Load history from disk
    ///
    /// Creates an empty history if the file doesn't exist.
    #[instrument(name = "input_history_load", skip(self))]
    pub fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            info!(path = %self.file_path.display(), "Input history file not found, starting fresh");
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.file_path).with_context(|| {
            format!(
                "Failed to read input history file: {}",
                self.file_path.display()
            )
        })?;

        // Deserialize just the entries array
        let data: InputHistoryData = match serde_json::from_str(&content) {
            Ok(data) => data,
            Err(parse_err) => {
                // Battery 13 (2026-07-18): preserve corrupt history in a
                // sidecar before the caller's fresh-start save can overwrite
                // it (same rescue as FrecencyStore::load).
                let rescue_path = PathBuf::from(format!("{}.corrupt", self.file_path.display()));
                match std::fs::rename(&self.file_path, &rescue_path) {
                    Ok(()) => warn!(
                        path = %self.file_path.display(),
                        rescue = %rescue_path.display(),
                        error = %parse_err,
                        "Input history file is corrupt; preserved as sidecar and starting fresh"
                    ),
                    Err(rename_err) => warn!(
                        path = %self.file_path.display(),
                        error = %parse_err,
                        rescue_error = %rename_err,
                        "Input history file is corrupt and could not be preserved"
                    ),
                }
                return Err(anyhow::Error::from(parse_err))
                    .with_context(|| "Failed to parse input history JSON");
            }
        };

        self.entries = data.entries;
        self.selected_results = data.selected_results;
        self.current_index = None; // Always reset navigation on load

        // Enforce max entries in case file was manually edited
        if self.entries.len() > MAX_ENTRIES {
            self.entries.truncate(MAX_ENTRIES);
        }
        self.prune_selected_results();

        info!(
            path = %self.file_path.display(),
            entry_count = self.entries.len(),
            "Loaded input history"
        );

        Ok(())
    }

    /// Save history to disk using atomic write (write temp + rename)
    #[instrument(name = "input_history_save", skip(self))]
    pub fn save(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Serialize just the entries
        let data = InputHistoryData {
            entries: self.entries.clone(),
            selected_results: self.selected_results.clone(),
        };
        let json =
            serde_json::to_string_pretty(&data).context("Failed to serialize input history")?;

        // Atomic write via a UNIQUE temp file + rename. A fixed temp path let
        // two concurrent savers (e.g. two app instances sharing $HOME) interleave
        // writes and corrupt the file; `write_atomic` gives each write its own
        // temp, so the result is always one writer's complete output.
        crate::atomic_file::write_atomic(&self.file_path, json.as_bytes()).with_context(|| {
            format!(
                "Failed to atomically write input history file: {}",
                self.file_path.display()
            )
        })?;

        info!(
            path = %self.file_path.display(),
            entry_count = self.entries.len(),
            bytes = json.len(),
            "Saved input history (atomic)"
        );

        Ok(())
    }

    /// Add an entry to history
    ///
    /// - Prepends the entry to the front
    /// - Removes any existing duplicate
    /// - Caps at MAX_ENTRIES (100)
    /// - Resets navigation state
    #[instrument(name = "input_history_add", skip(self))]
    pub fn add_entry(&mut self, text: &str) {
        self.add_entry_with_selection(text, None);
    }

    #[instrument(
        name = "input_history_add_with_selection",
        skip(self, selected_result_key)
    )]
    pub fn add_entry_with_selection(&mut self, text: &str, selected_result_key: Option<String>) {
        // Skip empty entries
        let text = text.trim();
        if text.is_empty() {
            debug!("Skipping empty input");
            return;
        }

        // Remove duplicate if exists
        self.entries.retain(|e| e != text);

        // Prepend new entry
        self.entries.insert(0, text.to_string());

        // Cap at max
        if self.entries.len() > MAX_ENTRIES {
            self.entries.truncate(MAX_ENTRIES);
        }

        if let (Some(query_key), Some(result_key)) =
            (Self::normalize_query(text), selected_result_key)
        {
            // Ambient learning: a query the user typed AND a result they chose
            // is the strongest "this matters to me" signal the launcher emits.
            // Recorded fire-and-forget into the brain's attention log.
            crate::brain::record_search_selection_signals(&query_key, &result_key);
            self.selected_results.insert(query_key, result_key);
        }
        self.prune_selected_results();

        // Reset navigation
        self.current_index = None;

        debug!(
            entry = text,
            total_entries = self.entries.len(),
            remembered_results = self.selected_results.len(),
            "Added entry to input history"
        );
    }

    pub fn preferred_result_key(&self, query: &str) -> Option<&str> {
        let query_key = Self::normalize_query(query)?;
        self.selected_results.get(&query_key).map(String::as_str)
    }

    /// Navigate up (to older entries)
    ///
    /// Returns the entry at the new position, or None if at the end.
    pub fn navigate_up(&mut self) -> Option<String> {
        let previous_index = self.current_index;
        if self.entries.is_empty() {
            info!(
                target: "script_kit::input_history",
                event = "input_history_navigate_up",
                previous_index = ?previous_index,
                new_index = ?Option::<usize>::None,
                entry_count = self.entries.len(),
                hit = false,
                reason = "empty",
            );
            return None;
        }

        let new_index = match self.current_index {
            None => 0, // Start at most recent
            Some(i) => {
                if i + 1 < self.entries.len() {
                    i + 1
                } else {
                    info!(
                        target: "script_kit::input_history",
                        event = "input_history_navigate_up",
                        previous_index = ?previous_index,
                        new_index = ?self.current_index,
                        entry_count = self.entries.len(),
                        hit = false,
                        reason = "at_oldest",
                    );
                    return None; // Already at oldest
                }
            }
        };

        self.current_index = Some(new_index);
        let entry = self.entries.get(new_index).cloned();

        debug!(
            index = new_index,
            entry = entry.as_deref().unwrap_or("<none>"),
            "Navigated up in history"
        );
        info!(
            target: "script_kit::input_history",
            event = "input_history_navigate_up",
            previous_index = ?previous_index,
            new_index,
            entry_count = self.entries.len(),
            hit = entry.is_some(),
            entry_len = entry.as_ref().map(|value| value.len()).unwrap_or(0),
        );

        entry
    }

    /// Navigate down (to newer entries)
    ///
    /// Returns the entry at the new position, or None if past the newest entry
    /// (indicating the user should see their current typed input).
    pub fn navigate_down(&mut self) -> Option<String> {
        let previous_index = self.current_index;
        match self.current_index {
            None => {
                info!(
                    target: "script_kit::input_history",
                    event = "input_history_navigate_down",
                    previous_index = ?previous_index,
                    new_index = ?Option::<usize>::None,
                    entry_count = self.entries.len(),
                    hit = false,
                    reason = "not_navigating",
                );
                None
            }
            Some(0) => {
                // At most recent entry, reset navigation
                self.current_index = None;
                debug!("Navigated past newest entry, returning to input");
                info!(
                    target: "script_kit::input_history",
                    event = "input_history_navigate_down",
                    previous_index = ?previous_index,
                    new_index = ?self.current_index,
                    entry_count = self.entries.len(),
                    hit = false,
                    reason = "past_newest",
                );
                None
            }
            Some(i) => {
                let new_index = i - 1;
                self.current_index = Some(new_index);
                let entry = self.entries.get(new_index).cloned();

                debug!(
                    index = new_index,
                    entry = entry.as_deref().unwrap_or("<none>"),
                    "Navigated down in history"
                );
                info!(
                    target: "script_kit::input_history",
                    event = "input_history_navigate_down",
                    previous_index = ?previous_index,
                    new_index,
                    entry_count = self.entries.len(),
                    hit = entry.is_some(),
                    entry_len = entry.as_ref().map(|value| value.len()).unwrap_or(0),
                );

                entry
            }
        }
    }

    /// Reset navigation state
    ///
    /// Call this when the user starts typing or submits input.
    pub fn reset_navigation(&mut self) {
        if self.current_index.is_some() {
            debug!("Reset input history navigation");
            self.current_index = None;
        }
    }

    /// Get the number of entries
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if history is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get current navigation index (for debugging/testing)
    #[allow(dead_code)]
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Get all entries (for debugging/testing)
    #[allow(dead_code)]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Return the most recent `limit` entries (most recent first).
    ///
    /// This is the deterministic bounded accessor used by the Tab AI context
    /// builder. It does not modify navigation state or storage semantics.
    pub fn recent_entries(&self, limit: usize) -> Vec<String> {
        self.entries.iter().take(limit).cloned().collect()
    }

    /// Clear all entries
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.selected_results.clear();
        self.current_index = None;
        debug!("Cleared input history");
    }
}
/// Raw data format for JSON serialization
#[derive(Debug, Serialize, Deserialize)]
struct InputHistoryData {
    entries: Vec<String>,
    #[serde(default)]
    selected_results: HashMap<String, String>,
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_history() -> (InputHistory, PathBuf) {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("input_history_test_{}.json", uuid::Uuid::new_v4()));
        let history = InputHistory::with_path(temp_path.clone());
        (history, temp_path)
    }

    fn cleanup_temp_file(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_new_history_is_empty() {
        let (history, path) = create_test_history();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry() {
        let (mut history, path) = create_test_history();

        history.add_entry("hello");
        assert_eq!(history.len(), 1);
        assert_eq!(history.entries()[0], "hello");

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_prepends() {
        let (mut history, path) = create_test_history();

        history.add_entry("first");
        history.add_entry("second");
        history.add_entry("third");

        assert_eq!(history.entries(), &["third", "second", "first"]);

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_deduplicates() {
        let (mut history, path) = create_test_history();

        history.add_entry("apple");
        history.add_entry("banana");
        history.add_entry("apple"); // Duplicate

        assert_eq!(history.len(), 2);
        assert_eq!(history.entries(), &["apple", "banana"]);

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_caps_at_max() {
        let (mut history, path) = create_test_history();

        // Add more than MAX_ENTRIES to verify truncation
        for i in 0..120 {
            history.add_entry(&format!("entry{}", i));
        }

        assert_eq!(history.len(), MAX_ENTRIES);
        assert_eq!(history.entries()[0], "entry119"); // Most recent

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_skips_empty() {
        let (mut history, path) = create_test_history();

        history.add_entry("");
        history.add_entry("   ");
        history.add_entry("\t\n");

        assert!(history.is_empty());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_trims_whitespace() {
        let (mut history, path) = create_test_history();

        history.add_entry("  hello  ");
        assert_eq!(history.entries()[0], "hello");

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_navigate_up_empty() {
        let (mut history, path) = create_test_history();

        assert!(history.navigate_up().is_none());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_navigate_up() {
        let (mut history, path) = create_test_history();

        history.add_entry("first");
        history.add_entry("second");
        history.add_entry("third");

        assert_eq!(history.navigate_up(), Some("third".to_string()));
        assert_eq!(history.current_index(), Some(0));

        assert_eq!(history.navigate_up(), Some("second".to_string()));
        assert_eq!(history.current_index(), Some(1));

        assert_eq!(history.navigate_up(), Some("first".to_string()));
        assert_eq!(history.current_index(), Some(2));

        assert_eq!(history.navigate_up(), None); // At end
        assert_eq!(history.current_index(), Some(2)); // Stays at 2

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_navigate_down() {
        let (mut history, path) = create_test_history();

        history.add_entry("first");
        history.add_entry("second");
        history.add_entry("third");

        // Navigate up first
        history.navigate_up(); // third (index 0)
        history.navigate_up(); // second (index 1)
        history.navigate_up(); // first (index 2)

        assert_eq!(history.navigate_down(), Some("second".to_string()));
        assert_eq!(history.current_index(), Some(1));

        assert_eq!(history.navigate_down(), Some("third".to_string()));
        assert_eq!(history.current_index(), Some(0));

        assert_eq!(history.navigate_down(), None); // Past newest
        assert_eq!(history.current_index(), None); // Reset

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_navigate_down_not_navigating() {
        let (mut history, path) = create_test_history();

        history.add_entry("test");
        assert!(history.navigate_down().is_none());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_reset_navigation() {
        let (mut history, path) = create_test_history();

        history.add_entry("test");
        history.navigate_up();
        assert!(history.current_index().is_some());

        history.reset_navigation();
        assert!(history.current_index().is_none());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_resets_navigation() {
        let (mut history, path) = create_test_history();

        history.add_entry("first");
        history.navigate_up();
        assert!(history.current_index().is_some());

        history.add_entry("second");
        assert!(history.current_index().is_none());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_save_and_load() {
        let (_, path) = create_test_history();

        // Create and populate history
        {
            let mut history = InputHistory::with_path(path.clone());
            history.add_entry("first");
            history.add_entry("second");
            history.add_entry("third");
            history.save().unwrap();
        }

        // Load into new history
        {
            let mut history = InputHistory::with_path(path.clone());
            history.load().unwrap();

            assert_eq!(history.len(), 3);
            assert_eq!(history.entries(), &["third", "second", "first"]);
            assert!(history.current_index().is_none()); // Navigation reset on load
        }

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_add_entry_with_selection_records_preferred_result() {
        let (mut history, path) = create_test_history();

        history
            .add_entry_with_selection("  Open   Tab  ", Some("script/main:open-tab".to_string()));

        assert_eq!(
            history.preferred_result_key("open tab"),
            Some("script/main:open-tab")
        );
        assert_eq!(
            history.preferred_result_key("OPEN\tTAB"),
            Some("script/main:open-tab")
        );

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_save_and_load_preserves_selected_results() {
        let (_, path) = create_test_history();

        {
            let mut history = InputHistory::with_path(path.clone());
            history.add_entry_with_selection("Open Tab", Some("script/main:open-tab".to_string()));
            history.save().unwrap();
        }

        {
            let mut history = InputHistory::with_path(path.clone());
            history.load().unwrap();
            assert_eq!(
                history.preferred_result_key("open tab"),
                Some("script/main:open-tab")
            );
        }

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_load_missing_file() {
        let mut history = InputHistory::with_path(PathBuf::from("/nonexistent/path/history.json"));
        let result = history.load();
        assert!(result.is_ok());
        assert!(history.is_empty());
    }

    #[test]
    fn test_load_invalid_json() {
        let (_, path) = create_test_history();
        fs::write(&path, "not valid json").unwrap();

        let mut history = InputHistory::with_path(path.clone());
        let result = history.load();
        assert!(result.is_err());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_load_truncates_excessive_entries() {
        let (_, path) = create_test_history();

        // Write file with too many entries
        let entries: Vec<String> = (0..100).map(|i| format!("entry{}", i)).collect();
        let data = InputHistoryData {
            entries,
            selected_results: HashMap::new(),
        };
        fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();

        let mut history = InputHistory::with_path(path.clone());
        history.load().unwrap();

        assert_eq!(history.len(), MAX_ENTRIES);

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_selected_results_prune_with_truncated_entries() {
        let (mut history, path) = create_test_history();

        for i in 0..120 {
            history.add_entry_with_selection(
                &format!("entry{}", i),
                Some(format!("script/main:entry{}", i)),
            );
        }

        assert_eq!(history.len(), MAX_ENTRIES);
        assert_eq!(
            history.preferred_result_key("entry119"),
            Some("script/main:entry119")
        );
        assert_eq!(history.preferred_result_key("entry0"), None);

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_clear() {
        let (mut history, path) = create_test_history();

        history.add_entry("test");
        history.navigate_up();

        history.clear();

        assert!(history.is_empty());
        assert!(history.current_index().is_none());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_max_entries_constant() {
        // User requirement: store at least 100 recent entries
        assert_eq!(MAX_ENTRIES, 100);
    }

    #[test]
    fn test_recent_entries_bounded() {
        let (mut history, path) = create_test_history();
        history.add_entry("a");
        history.add_entry("b");
        history.add_entry("c");
        history.add_entry("d");
        history.add_entry("e");
        history.add_entry("f");

        // Most recent first, bounded to 3
        let recent = history.recent_entries(3);
        assert_eq!(recent, vec!["f", "e", "d"]);

        // Limit larger than entries returns all
        let all = history.recent_entries(100);
        assert_eq!(all.len(), 6);

        // Zero limit returns empty
        let none = history.recent_entries(0);
        assert!(none.is_empty());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_recent_entries_does_not_modify_state() {
        let (mut history, path) = create_test_history();
        history.add_entry("first");
        history.add_entry("second");
        history.navigate_up(); // Start navigating

        let _recent = history.recent_entries(5);

        // Navigation state should be unchanged
        assert_eq!(history.current_index(), Some(0));

        cleanup_temp_file(&path);
    }

    /// Battery 13 lock: a corrupt input-history file must be preserved as a
    /// `.corrupt` sidecar by the failed load (same contract as FrecencyStore).
    #[test]
    fn test_corrupt_history_rescued_to_sidecar_before_fresh_start() {
        let (_, path) = create_test_history();
        let corrupt = "{\"entries\":[\"partial";
        fs::write(&path, corrupt).unwrap();

        let mut history = InputHistory::with_path(path.clone());
        assert!(
            history.load().is_err(),
            "corrupt load must still surface Err"
        );
        assert!(history.is_empty());

        let rescue = PathBuf::from(format!("{}.corrupt", path.display()));
        assert!(rescue.exists(), "corrupt file must be preserved as sidecar");
        assert_eq!(fs::read_to_string(&rescue).unwrap(), corrupt);
        assert!(
            !path.exists(),
            "original path must be clear for a fresh start"
        );

        history.add_entry("fresh entry");
        history.save().unwrap();
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&rescue).unwrap(), corrupt);

        cleanup_temp_file(&path);
        cleanup_temp_file(&rescue);
    }
}
