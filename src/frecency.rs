//! Frecency scoring for script usage tracking
//!
//! This module provides a frecency-based ranking system that combines
//! frequency (how often) and recency (how recently) scripts are used.
//! The scoring uses exponential decay with a configurable half-life.

use crate::config::{FrecencyConfig, DEFAULT_FRECENCY_HALF_LIFE_DAYS};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, instrument, warn};

/// Re-export for tests that need the half-life constant
#[allow(dead_code)]
pub const HALF_LIFE_DAYS: f64 = DEFAULT_FRECENCY_HALF_LIFE_DAYS;

/// Seconds in a day for timestamp calculations
const SECONDS_PER_DAY: f64 = 86400.0;

/// A single frecency entry tracking usage of a script
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrecencyEntry {
    /// Number of times this script has been used
    pub count: u32,
    /// Unix timestamp (seconds) of last use
    pub last_used: u64,
    /// Cached score (recalculated on load)
    #[serde(default)]
    pub score: f64,
}

impl FrecencyEntry {
    /// Create a new entry with initial use
    pub fn new() -> Self {
        let now = current_timestamp();
        FrecencyEntry {
            count: 1,
            last_used: now,
            score: 1.0, // Initial score is just the count (no decay yet)
        }
    }

    /// Record a new use of this script
    pub fn record_use(&mut self) {
        self.count += 1;
        self.last_used = current_timestamp();
        self.recalculate_score();
    }

    /// Recalculate the frecency score based on current time
    pub fn recalculate_score(&mut self) {
        self.score = calculate_score(self.count, self.last_used, DEFAULT_FRECENCY_HALF_LIFE_DAYS);
    }

    /// Recalculate the frecency score with a custom half-life
    pub fn recalculate_score_with_half_life(&mut self, half_life_days: f64) {
        self.score = calculate_score(self.count, self.last_used, half_life_days);
    }
}

impl Default for FrecencyEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate frecency score using exponential decay
///
/// Formula: score = count * e^(-days_since_use / half_life_days)
///
/// This means (with default 7-day half-life):
/// - After 7 days (half_life), the score is reduced to ~50%
/// - After 14 days, the score is reduced to ~25%
/// - After 21 days, the score is reduced to ~12.5%
///
/// With a shorter half-life (e.g., 1 day), recent items dominate.
/// With a longer half-life (e.g., 30 days), frequently used items dominate.
fn calculate_score(count: u32, last_used: u64, half_life_days: f64) -> f64 {
    let now = current_timestamp();
    let seconds_since_use = now.saturating_sub(last_used);
    let days_since_use = seconds_since_use as f64 / SECONDS_PER_DAY;

    // Exponential decay: count * e^(-days / half_life)
    let decay_factor = (-days_since_use / half_life_days).exp();
    count as f64 * decay_factor
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Store for frecency data with persistence
#[derive(Debug, Clone)]
pub struct FrecencyStore {
    /// Map of script path to frecency entry
    entries: HashMap<String, FrecencyEntry>,
    /// Path to the frecency data file
    file_path: PathBuf,
    /// Whether there are unsaved changes
    dirty: bool,
    /// Half-life in days for score decay (from config)
    half_life_days: f64,
}

/// Raw data format for JSON serialization
#[derive(Debug, Serialize, Deserialize)]
struct FrecencyData {
    entries: HashMap<String, FrecencyEntry>,
}

impl FrecencyStore {
    /// Create a new FrecencyStore with the default path (~/.sk/kit/frecency.json)
    pub fn new() -> Self {
        let file_path = Self::default_path();
        FrecencyStore {
            entries: HashMap::new(),
            file_path,
            dirty: false,
            half_life_days: DEFAULT_FRECENCY_HALF_LIFE_DAYS,
        }
    }

    /// Create a FrecencyStore with config settings
    pub fn with_config(config: &FrecencyConfig) -> Self {
        let file_path = Self::default_path();
        FrecencyStore {
            entries: HashMap::new(),
            file_path,
            dirty: false,
            half_life_days: config.half_life_days,
        }
    }

    /// Create a FrecencyStore with a custom path (for testing)
    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        FrecencyStore {
            entries: HashMap::new(),
            file_path: path,
            dirty: false,
            half_life_days: DEFAULT_FRECENCY_HALF_LIFE_DAYS,
        }
    }

    /// Update the half-life setting (e.g., after config reload)
    #[allow(dead_code)]
    pub fn set_half_life_days(&mut self, half_life_days: f64) {
        if (self.half_life_days - half_life_days).abs() > 0.001 {
            self.half_life_days = half_life_days;
            // Recalculate all scores with new half-life
            for entry in self.entries.values_mut() {
                entry.recalculate_score_with_half_life(half_life_days);
            }
        }
    }

    /// Get the current half-life setting
    #[allow(dead_code)]
    pub fn half_life_days(&self) -> f64 {
        self.half_life_days
    }

    /// Get the default frecency file path
    fn default_path() -> PathBuf {
        PathBuf::from(shellexpand::tilde("~/.sk/kit/frecency.json").as_ref())
    }

    /// Load frecency data from disk
    ///
    /// Creates an empty store if the file doesn't exist.
    /// Recalculates all scores on load to account for time passed.
    #[instrument(name = "frecency_load", skip(self))]
    pub fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            info!(path = %self.file_path.display(), "Frecency file not found, starting fresh");
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.file_path).with_context(|| {
            format!("Failed to read frecency file: {}", self.file_path.display())
        })?;

        let data: FrecencyData =
            serde_json::from_str(&content).with_context(|| "Failed to parse frecency JSON")?;

        self.entries = data.entries;

        // Recalculate all scores to account for time passed since last save
        let half_life = self.half_life_days;
        for entry in self.entries.values_mut() {
            entry.recalculate_score_with_half_life(half_life);
        }

        info!(
            path = %self.file_path.display(),
            entry_count = self.entries.len(),
            "Loaded frecency data"
        );

        self.dirty = false;
        Ok(())
    }

    /// Save frecency data to disk
    #[instrument(name = "frecency_save", skip(self))]
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            debug!("No changes to save");
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let data = FrecencyData {
            entries: self.entries.clone(),
        };

        let json =
            serde_json::to_string_pretty(&data).context("Failed to serialize frecency data")?;

        std::fs::write(&self.file_path, json).with_context(|| {
            format!(
                "Failed to write frecency file: {}",
                self.file_path.display()
            )
        })?;

        info!(
            path = %self.file_path.display(),
            entry_count = self.entries.len(),
            "Saved frecency data"
        );

        self.dirty = false;
        Ok(())
    }

    /// Record a use of a script
    ///
    /// Increments the count and updates the last_used timestamp.
    /// Creates a new entry if the script hasn't been used before.
    #[instrument(name = "frecency_record_use", skip(self))]
    pub fn record_use(&mut self, path: &str) {
        if let Some(entry) = self.entries.get_mut(path) {
            entry.record_use();
            debug!(
                path = path,
                count = entry.count,
                score = entry.score,
                "Updated frecency entry"
            );
        } else {
            let entry = FrecencyEntry::new();
            debug!(path = path, "Created new frecency entry");
            self.entries.insert(path.to_string(), entry);
        }
        self.dirty = true;
    }

    /// Get the frecency score for a script
    ///
    /// Returns 0.0 if the script has never been used.
    pub fn get_score(&self, path: &str) -> f64 {
        self.entries.get(path).map(|e| e.score).unwrap_or(0.0)
    }

    /// Get the top N items by frecency score
    ///
    /// Returns a vector of (path, score) tuples sorted by score descending.
    pub fn get_recent_items(&self, limit: usize) -> Vec<(String, f64)> {
        let mut items: Vec<_> = self
            .entries
            .iter()
            .map(|(path, entry)| (path.clone(), entry.score))
            .collect();

        // Sort by score descending
        items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N
        items.truncate(limit);
        items
    }

    /// Get the number of tracked scripts
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the store is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if there are unsaved changes
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Remove an entry by path
    #[allow(dead_code)]
    pub fn remove(&mut self, path: &str) -> Option<FrecencyEntry> {
        let entry = self.entries.remove(path);
        if entry.is_some() {
            self.dirty = true;
        }
        entry
    }

    /// Clear all entries
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        if !self.entries.is_empty() {
            self.entries.clear();
            self.dirty = true;
        }
    }
}

impl Default for FrecencyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Helper to create a test store with a temp file
    fn create_test_store() -> (FrecencyStore, PathBuf) {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("frecency_test_{}.json", uuid::Uuid::new_v4()));
        let store = FrecencyStore::with_path(temp_path.clone());
        (store, temp_path)
    }

    // Cleanup helper
    fn cleanup_temp_file(path: &PathBuf) {
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_frecency_entry_new() {
        let entry = FrecencyEntry::new();
        assert_eq!(entry.count, 1);
        assert!(entry.last_used > 0);
        assert!(entry.score > 0.0);
    }

    #[test]
    fn test_frecency_entry_record_use() {
        let mut entry = FrecencyEntry::new();
        let initial_count = entry.count;
        let initial_last_used = entry.last_used;

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        entry.record_use();

        assert_eq!(entry.count, initial_count + 1);
        assert!(entry.last_used >= initial_last_used);
    }

    #[test]
    fn test_calculate_score_no_decay() {
        // Score right now should be close to count
        let now = current_timestamp();
        let score = calculate_score(5, now, HALF_LIFE_DAYS);

        // Should be approximately 5 (allowing for tiny time difference)
        assert!((score - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_score_with_decay() {
        let now = current_timestamp();
        let count = 10;

        // One half-life ago (7 days)
        let one_half_life_ago = now - (HALF_LIFE_DAYS * SECONDS_PER_DAY) as u64;
        let score = calculate_score(count, one_half_life_ago, HALF_LIFE_DAYS);

        // Should be approximately count/2 (half due to decay)
        // e^(-7/7) = e^(-1) ≈ 0.368
        let expected = count as f64 * (-1.0_f64).exp();
        assert!((score - expected).abs() < 0.01);
    }

    #[test]
    fn test_calculate_score_old_item() {
        let now = current_timestamp();
        let count = 100;

        // 30 days ago (about 4+ half-lives)
        let thirty_days_ago = now - (30 * SECONDS_PER_DAY as u64);
        let score = calculate_score(count, thirty_days_ago, HALF_LIFE_DAYS);

        // Should be heavily decayed
        assert!(score < 2.0); // Much less than original 100
    }

    #[test]
    fn test_frecency_store_new() {
        let store = FrecencyStore::new();
        assert!(store.is_empty());
        assert!(!store.is_dirty());
    }

    #[test]
    fn test_frecency_store_record_use() {
        let (mut store, _temp) = create_test_store();

        store.record_use("/path/to/script.ts");

        assert_eq!(store.len(), 1);
        assert!(store.is_dirty());
        assert!(store.get_score("/path/to/script.ts") > 0.0);
    }

    #[test]
    fn test_frecency_store_record_use_increments() {
        let (mut store, _temp) = create_test_store();

        store.record_use("/path/to/script.ts");
        let score1 = store.get_score("/path/to/script.ts");

        store.record_use("/path/to/script.ts");
        let score2 = store.get_score("/path/to/script.ts");

        // Second use should have higher score
        assert!(score2 > score1);
    }

    #[test]
    fn test_frecency_store_get_score_unknown() {
        let (store, _temp) = create_test_store();
        assert_eq!(store.get_score("/unknown/script.ts"), 0.0);
    }

    #[test]
    fn test_frecency_store_get_recent_items() {
        let (mut store, _temp) = create_test_store();

        // Add items with different use counts
        store.record_use("/a.ts");
        store.record_use("/b.ts");
        store.record_use("/b.ts");
        store.record_use("/c.ts");
        store.record_use("/c.ts");
        store.record_use("/c.ts");

        let recent = store.get_recent_items(2);

        assert_eq!(recent.len(), 2);
        // c.ts should be first (3 uses), b.ts second (2 uses)
        assert_eq!(recent[0].0, "/c.ts");
        assert_eq!(recent[1].0, "/b.ts");
    }

    #[test]
    fn test_frecency_store_get_recent_items_limit() {
        let (mut store, _temp) = create_test_store();

        for i in 0..10 {
            store.record_use(&format!("/script{}.ts", i));
        }

        let recent = store.get_recent_items(5);
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_frecency_store_save_and_load() {
        let (_, path) = create_test_store();

        // Create and populate store
        {
            let mut store = FrecencyStore::with_path(path.clone());
            store.record_use("/script1.ts");
            store.record_use("/script1.ts");
            store.record_use("/script2.ts");
            store.save().unwrap();
        }

        // Load into new store
        {
            let mut store = FrecencyStore::with_path(path.clone());
            store.load().unwrap();

            assert_eq!(store.len(), 2);
            assert!(store.get_score("/script1.ts") > store.get_score("/script2.ts"));
        }

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_frecency_store_load_missing_file() {
        let mut store = FrecencyStore::with_path(PathBuf::from("/nonexistent/path/frecency.json"));
        let result = store.load();
        assert!(result.is_ok());
        assert!(store.is_empty());
    }

    #[test]
    fn test_frecency_store_load_invalid_json() {
        let (_, path) = create_test_store();
        fs::write(&path, "not valid json").unwrap();

        let mut store = FrecencyStore::with_path(path.clone());
        let result = store.load();
        assert!(result.is_err());

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_frecency_store_remove() {
        let (mut store, _temp) = create_test_store();

        store.record_use("/script.ts");
        assert_eq!(store.len(), 1);

        let removed = store.remove("/script.ts");
        assert!(removed.is_some());
        assert!(store.is_empty());
        assert!(store.is_dirty());
    }

    #[test]
    fn test_frecency_store_remove_nonexistent() {
        let (mut store, _temp) = create_test_store();

        let removed = store.remove("/nonexistent.ts");
        assert!(removed.is_none());
    }

    #[test]
    fn test_frecency_store_clear() {
        let (mut store, _temp) = create_test_store();

        store.record_use("/a.ts");
        store.record_use("/b.ts");
        store.dirty = false; // Reset dirty flag

        store.clear();

        assert!(store.is_empty());
        assert!(store.is_dirty());
    }

    #[test]
    fn test_frecency_store_save_not_dirty() {
        let (mut store, _temp) = create_test_store();

        // Save without changes should succeed without writing
        let result = store.save();
        assert!(result.is_ok());
    }

    #[test]
    fn test_frecency_entry_serialization() {
        let entry = FrecencyEntry {
            count: 5,
            last_used: 1704067200, // 2024-01-01 00:00:00 UTC
            score: 4.5,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FrecencyEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.count, deserialized.count);
        assert_eq!(entry.last_used, deserialized.last_used);
        assert_eq!(entry.score, deserialized.score);
    }

    #[test]
    fn test_frecency_entry_deserialization_without_score() {
        // Score was added later, so old data might not have it
        let json = r#"{"count": 5, "last_used": 1704067200}"#;
        let entry: FrecencyEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.count, 5);
        assert_eq!(entry.last_used, 1704067200);
        assert_eq!(entry.score, 0.0); // Default
    }

    #[test]
    fn test_frecency_store_recalculates_scores_on_load() {
        let (_, path) = create_test_store();

        // Write data with stale scores
        let old_data =
            r#"{"entries": {"/script.ts": {"count": 10, "last_used": 0, "score": 100.0}}}"#;
        fs::write(&path, old_data).unwrap();

        let mut store = FrecencyStore::with_path(path.clone());
        store.load().unwrap();

        // Score should be recalculated (timestamp 0 is very old, so score should be tiny)
        let score = store.get_score("/script.ts");
        assert!(score < 1.0); // Should be heavily decayed, not the stale 100.0

        cleanup_temp_file(&path);
    }

    #[test]
    fn test_half_life_constant() {
        assert_eq!(HALF_LIFE_DAYS, 7.0);
    }

    #[test]
    fn test_seconds_per_day_constant() {
        assert_eq!(SECONDS_PER_DAY, 86400.0);
    }
}
