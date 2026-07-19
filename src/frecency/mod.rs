//! Frecency scoring for script usage tracking
//!
//! This module provides a frecency-based ranking system that combines
//! frequency (how often) and recency (how recently) scripts are used.
//! The scoring uses exponential decay with a configurable half-life.

// --- merged from part_000.rs ---
use crate::config::{SuggestedConfig, DEFAULT_SUGGESTED_HALF_LIFE_DAYS};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, instrument, warn};
/// Re-export for tests that need the half-life constant
#[allow(dead_code)]
pub const HALF_LIFE_DAYS: f64 = DEFAULT_SUGGESTED_HALF_LIFE_DAYS;
/// Seconds in a day for timestamp calculations
const SECONDS_PER_DAY: f64 = 86400.0;
const SECONDS_PER_DAY_U64: u64 = 86400;
/// A single frecency entry tracking usage of a script
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrecencyEntry {
    /// Number of times this script has been used
    pub count: u32,
    /// Unix timestamp (seconds) of last use
    pub last_used: u64,
    /// Cached score (recalculated on load, not persisted)
    /// This is a derived value computed from count, last_used, and half_life.
    /// We skip serializing it because it's always recalculated on load.
    #[serde(skip_serializing, default)]
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

    /// Calculate the decay factor for a given time elapsed
    fn decay_factor(seconds_elapsed: u64, half_life_days: f64) -> f64 {
        // Guard against nonsense config (zero or negative half-life)
        let hl = half_life_days.max(0.001);
        let days_elapsed = seconds_elapsed as f64 / SECONDS_PER_DAY;
        // True half-life decay: 2^(-days/hl) == e^(-ln(2) * days/hl)
        (-std::f64::consts::LN_2 * days_elapsed / hl).exp()
    }

    /// Compute the score at a given timestamp (live computation with decay)
    ///
    /// This decays the stored score by the time elapsed since last_used.
    /// Use this for ranking to avoid stale scores.
    pub fn score_at(&self, now: u64, half_life_days: f64) -> f64 {
        let dt = now.saturating_sub(self.last_used);
        self.score * Self::decay_factor(dt, half_life_days)
    }

    /// Record a new use with explicit timestamp (incremental frecency model)
    ///
    /// Uses the incremental model: new_score = old_score * decay(elapsed_time) + 1
    /// This prevents "rich get richer" by decaying historical usage.
    pub fn record_use_with_timestamp(&mut self, now: u64, half_life_days: f64) {
        // Compute current score with decay
        let current_score = self.score_at(now, half_life_days);
        // Add 1 for this new use
        self.score = current_score + 1.0;
        self.last_used = now;
        self.count = self.count.saturating_add(1);
    }

    /// Record a new use of this script using the default half-life
    ///
    /// NOTE: Prefer using FrecencyStore::record_use() which uses the store's
    /// configured half-life instead of the default.
    #[allow(dead_code)]
    pub fn record_use(&mut self) {
        self.record_use_with_timestamp(current_timestamp(), DEFAULT_SUGGESTED_HALF_LIFE_DAYS);
    }

    /// Recalculate the frecency score based on current time using default half-life
    ///
    /// NOTE: Prefer using recalculate_score_with_half_life() with the store's
    /// configured half-life.
    #[allow(dead_code)]
    pub fn recalculate_score(&mut self) {
        self.score = calculate_score(self.count, self.last_used, DEFAULT_SUGGESTED_HALF_LIFE_DAYS);
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
/// Calculate frecency score using exponential decay with true half-life
///
/// Formula: score = count * 2^(-days_since_use / half_life_days)
///        = count * e^(-ln(2) * days_since_use / half_life_days)
///
/// This means (with default 7-day half-life):
/// - After 7 days (half_life), the score is reduced to exactly 50%
/// - After 14 days, the score is reduced to exactly 25%
/// - After 21 days, the score is reduced to exactly 12.5%
///
/// With a shorter half-life (e.g., 1 day), recent items dominate.
/// With a longer half-life (e.g., 30 days), frequently used items dominate.
fn calculate_score(count: u32, last_used: u64, half_life_days: f64) -> f64 {
    let now = current_timestamp();
    let seconds_since_use = now.saturating_sub(last_used);
    let days_since_use = seconds_since_use as f64 / SECONDS_PER_DAY;

    // Guard against nonsense config (zero or negative half-life)
    let hl = half_life_days.max(0.001);

    // True half-life decay: 2^(-days/hl) == e^(-ln(2) * days/hl)
    // At days == hl: decay_factor = 2^(-1) = 0.5 (exactly 50%)
    let decay_factor = (-std::f64::consts::LN_2 * days_since_use / hl).exp();
    f64::from(count) * decay_factor
}
/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
/// Store for frecency data with persistence
///
/// NOTE: Clone is intentionally NOT derived to prevent accidental data loss
/// in multi-window contexts. If you need to share FrecencyStore across
/// multiple owners, use `Arc<Mutex<FrecencyStore>>` explicitly.
#[derive(Debug)]
pub struct FrecencyStore {
    /// Map of script path to frecency entry
    entries: HashMap<String, FrecencyEntry>,
    /// Path to the frecency data file
    file_path: PathBuf,
    /// Whether there are unsaved changes
    dirty: bool,
    /// Half-life in days for score decay (from config)
    half_life_days: f64,
    /// Revision counter for cache invalidation
    /// Incremented on any change affecting ranking
    revision: u64,
}
/// Raw data format for JSON serialization (owned, for deserialization)
#[derive(Debug, Serialize, Deserialize)]
struct FrecencyData {
    entries: HashMap<String, FrecencyEntry>,
}
/// Raw data format for JSON serialization (borrowed, for serialization without cloning)
#[derive(Serialize)]
struct FrecencyDataRef<'a> {
    entries: &'a HashMap<String, FrecencyEntry>,
}
// --- merged from part_001.rs ---
impl FrecencyStore {
    /// Create a new FrecencyStore with the default path (~/.scriptkit/frecency.json)
    pub fn new() -> Self {
        let file_path = Self::default_path();
        FrecencyStore {
            entries: HashMap::new(),
            file_path,
            dirty: false,
            half_life_days: DEFAULT_SUGGESTED_HALF_LIFE_DAYS,
            revision: 0,
        }
    }

    /// Create a FrecencyStore with config settings
    pub fn with_config(config: &SuggestedConfig) -> Self {
        let file_path = Self::default_path();
        FrecencyStore {
            entries: HashMap::new(),
            file_path,
            dirty: false,
            half_life_days: config.half_life_days,
            revision: 0,
        }
    }

    /// Create a FrecencyStore with a custom path (for testing)
    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        FrecencyStore {
            entries: HashMap::new(),
            file_path: path,
            dirty: false,
            half_life_days: DEFAULT_SUGGESTED_HALF_LIFE_DAYS,
            revision: 0,
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
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Get the current half-life setting
    #[allow(dead_code)]
    pub fn half_life_days(&self) -> f64 {
        self.half_life_days
    }

    /// Get the current revision counter for cache invalidation
    ///
    /// This value increments on any change that affects ranking:
    /// record_use, remove, clear, set_half_life_days
    #[allow(dead_code)]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Get the default frecency file path
    fn default_path() -> PathBuf {
        PathBuf::from(shellexpand::tilde("~/.scriptkit/frecency.json").as_ref())
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

        let data: FrecencyData = match serde_json::from_str(&content) {
            Ok(data) => data,
            Err(parse_err) => {
                // Battery 13 (2026-07-18): a corrupt file used to be silently
                // clobbered — startup `.ok()`s this error, starts fresh, and
                // the next save overwrites the user's entire frecency history.
                // Preserve the bad bytes in a sidecar so the data stays
                // recoverable; still return Err so callers keep their
                // fresh-start behavior.
                let rescue_path = PathBuf::from(format!("{}.corrupt", self.file_path.display()));
                match std::fs::rename(&self.file_path, &rescue_path) {
                    Ok(()) => warn!(
                        path = %self.file_path.display(),
                        rescue = %rescue_path.display(),
                        error = %parse_err,
                        "Frecency file is corrupt; preserved as sidecar and starting fresh"
                    ),
                    Err(rename_err) => warn!(
                        path = %self.file_path.display(),
                        error = %parse_err,
                        rescue_error = %rename_err,
                        "Frecency file is corrupt and could not be preserved"
                    ),
                }
                return Err(anyhow::Error::from(parse_err))
                    .with_context(|| "Failed to parse frecency JSON");
            }
        };

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

    /// Save frecency data to disk using atomic write (write temp + rename)
    ///
    /// Uses compact JSON for performance and atomic rename for crash safety.
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

        // Serialize directly from reference to avoid cloning the entire map
        let json = serde_json::to_string(&FrecencyDataRef {
            entries: &self.entries,
        })
        .context("Failed to serialize frecency data")?;

        // Atomic write via a UNIQUE temp file + rename. A fixed temp path let
        // concurrent savers corrupt the file; `write_atomic` isolates each write.
        crate::atomic_file::write_atomic(&self.file_path, json.as_bytes()).with_context(|| {
            format!(
                "Failed to atomically write frecency file: {}",
                self.file_path.display()
            )
        })?;

        info!(
            path = %self.file_path.display(),
            entry_count = self.entries.len(),
            bytes = json.len(),
            "Saved frecency data (atomic)"
        );

        self.dirty = false;
        Ok(())
    }

    /// Record a use of a script
    ///
    /// Uses the incremental frecency model: score = decayed_score + 1
    /// Creates a new entry if the script hasn't been used before.
    /// Uses the store's configured half-life for score calculation.
    #[instrument(name = "frecency_record_use", skip(self))]
    pub fn record_use(&mut self, path: &str) {
        let half_life = self.half_life_days;
        let now = current_timestamp();

        if let Some(entry) = self.entries.get_mut(path) {
            // Use incremental model: decay existing score, then add 1
            entry.record_use_with_timestamp(now, half_life);
            debug!(
                path = path,
                count = entry.count,
                score = entry.score,
                half_life_days = half_life,
                "Updated frecency entry (incremental model)"
            );
        } else {
            // New entry starts with score 1.0
            let entry = FrecencyEntry {
                count: 1,
                last_used: now,
                score: 1.0,
            };
            debug!(
                path = path,
                half_life_days = half_life,
                "Created new frecency entry"
            );
            self.entries.insert(path.to_string(), entry);
        }
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Record a use of a script at a specific timestamp (for testing)
    ///
    /// Same as `record_use()` but allows injecting a specific timestamp
    /// for deterministic testing.
    #[allow(dead_code)]
    pub fn record_use_at(&mut self, path: &str, timestamp: u64) {
        let half_life = self.half_life_days;

        if let Some(entry) = self.entries.get_mut(path) {
            entry.record_use_with_timestamp(timestamp, half_life);
            debug!(
                path = path,
                count = entry.count,
                score = entry.score,
                half_life_days = half_life,
                timestamp = timestamp,
                "Updated frecency entry at timestamp"
            );
        } else {
            let entry = FrecencyEntry {
                count: 1,
                last_used: timestamp,
                score: 1.0,
            };
            debug!(
                path = path,
                half_life_days = half_life,
                timestamp = timestamp,
                "Created new frecency entry at timestamp"
            );
            self.entries.insert(path.to_string(), entry);
        }
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Get the frecency score for a script
    ///
    /// Returns 0.0 if the script has never been used.
    pub fn get_score(&self, path: &str) -> f64 {
        self.entries.get(path).map(|e| e.score).unwrap_or(0.0)
    }

    /// Get the top N items by frecency score
    ///
    /// Computes live scores (with decay) for accurate ranking.
    /// Returns a vector of (path, score) tuples sorted by:
    /// 1. Score descending
    /// 2. Last used descending (tie-breaker)
    /// 3. Path ascending (final tie-breaker for determinism)
    pub fn get_recent_items(&self, limit: usize) -> Vec<(String, f64)> {
        let now = current_timestamp();
        let hl = self.half_life_days;

        // Compute live scores with decay for accurate ranking
        let mut items: Vec<_> = self
            .entries
            .iter()
            .map(|(path, entry)| {
                let live_score = entry.score_at(now, hl);
                (path.clone(), live_score, entry.last_used)
            })
            .collect();

        // Sort by score descending, then last_used descending, then path ascending
        items.sort_by(|a, b| {
            // Primary: score descending
            match b.1.partial_cmp(&a.1) {
                Some(std::cmp::Ordering::Equal) | None => {}
                Some(ord) => return ord,
            }
            // Secondary: last_used descending (more recent first)
            match b.2.cmp(&a.2) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
            // Tertiary: path ascending (alphabetical)
            a.0.cmp(&b.0)
        });

        // Take top N and drop last_used from result
        items
            .into_iter()
            .take(limit)
            .map(|(path, score, _)| (path, score))
            .collect()
    }

    /// Return recent launcher file paths recorded under the `file/<absolute-path>` key.
    ///
    /// This intentionally exposes only file frecency entries so callers cannot
    /// turn the generic frecency store into an arbitrary prefix query source.
    pub fn top_file_paths(&self, limit: usize) -> Vec<(String, f64)> {
        const FILE_PREFIX: &str = "file/";
        let now = current_timestamp();
        let hl = self.half_life_days;
        let mut items: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, score)| {
                key.strip_prefix(FILE_PREFIX)
                    .map(|path| (path.to_string(), score.score_at(now, hl), score.last_used))
            })
            .collect();

        items.sort_by(|a, b| {
            match b.1.partial_cmp(&a.1) {
                Some(std::cmp::Ordering::Equal) | None => {}
                Some(ord) => return ord,
            }

            match b.2.cmp(&a.2) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }

            a.0.cmp(&b.0)
        });

        items
            .into_iter()
            .take(limit)
            .map(|(path, score, _)| (path, score))
            .collect()
    }

    pub fn top_directory_paths(&self, limit: usize) -> Vec<(String, f64)> {
        const DIR_PREFIX: &str = "dir/";
        let now = current_timestamp();
        let hl = self.half_life_days;
        let mut items: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(key, score)| {
                key.strip_prefix(DIR_PREFIX)
                    .map(|path| (path.to_string(), score.score_at(now, hl), score.last_used))
            })
            .collect();

        items.sort_by(|a, b| {
            match b.1.partial_cmp(&a.1) {
                Some(std::cmp::Ordering::Equal) | None => {}
                Some(ord) => return ord,
            }
            match b.2.cmp(&a.2) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
            a.0.cmp(&b.0)
        });

        items
            .into_iter()
            .take(limit)
            .map(|(path, score, _)| (path, score))
            .collect()
    }

    /// Get the top N items by frecency score at a specific timestamp (for testing)
    ///
    /// Same as `get_recent_items()` but allows querying at a specific timestamp
    /// for deterministic testing.
    #[allow(dead_code)]
    pub fn get_recent_items_at(&self, limit: usize, at_timestamp: u64) -> Vec<(String, f64)> {
        let hl = self.half_life_days;

        let mut items: Vec<_> = self
            .entries
            .iter()
            .map(|(path, entry)| {
                let live_score = entry.score_at(at_timestamp, hl);
                (path.clone(), live_score, entry.last_used)
            })
            .collect();

        items.sort_by(|a, b| {
            match b.1.partial_cmp(&a.1) {
                Some(std::cmp::Ordering::Equal) | None => {}
                Some(ord) => return ord,
            }
            match b.2.cmp(&a.2) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
            a.0.cmp(&b.0)
        });

        items
            .into_iter()
            .take(limit)
            .map(|(path, score, _)| (path, score))
            .collect()
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
            self.revision = self.revision.wrapping_add(1);
        }
        entry
    }

    /// Clear all entries
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        if !self.entries.is_empty() {
            self.entries.clear();
            self.dirty = true;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Prune stale entries to keep the frecency file bounded
    ///
    /// Removes entries where:
    /// - Live score (with decay) is below `score_threshold`
    /// - AND last_used is older than `min_age_days` days
    ///
    /// Returns the number of entries pruned.
    #[allow(dead_code)]
    pub fn prune_stale_entries(&mut self, score_threshold: f64, min_age_days: u64) -> usize {
        let now = current_timestamp();
        let hl = self.half_life_days;
        let min_age_seconds = min_age_days * SECONDS_PER_DAY_U64;

        let entries_before = self.entries.len();

        self.entries.retain(|_path, entry| {
            let live_score = entry.score_at(now, hl);
            let age_seconds = now.saturating_sub(entry.last_used);

            // Keep if: score is above threshold OR entry is recent enough
            live_score >= score_threshold || age_seconds < min_age_seconds
        });

        let pruned_count = entries_before - self.entries.len();

        if pruned_count > 0 {
            self.dirty = true;
            self.revision = self.revision.wrapping_add(1);
            debug!(
                pruned_count = pruned_count,
                remaining = self.entries.len(),
                score_threshold = score_threshold,
                min_age_days = min_age_days,
                "Pruned stale frecency entries"
            );
        }

        pruned_count
    }
}
impl Default for FrecencyStore {
    fn default() -> Self {
        Self::new()
    }
}
// --- merged from part_002.rs ---
#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;
    use std::fs;

    fn days_to_seconds(days: f64) -> u64 {
        std::time::Duration::from_secs_f64(days * SECONDS_PER_DAY).as_secs()
    }

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
    fn test_calculate_score_with_decay_true_half_life() {
        let now = current_timestamp();
        let count = 10;

        // One half-life ago (7 days)
        let one_half_life_ago = now - days_to_seconds(HALF_LIFE_DAYS);
        let score = calculate_score(count, one_half_life_ago, HALF_LIFE_DAYS);

        // With TRUE half-life, score should be exactly count/2 (50% decay at one half-life)
        // Formula: count * 2^(-days/half_life) = count * 2^(-1) = count/2
        let expected = f64::from(count) * 0.5;
        assert!(
            (score - expected).abs() < 0.01,
            "Expected ~{} (50% of {}), got {} - half-life formula should give 50% decay at half-life",
            expected,
            count,
            score
        );
    }
    #[test]
    fn test_calculate_score_two_half_lives() {
        let now = current_timestamp();
        let count = 100;

        // Two half-lives ago (14 days)
        let two_half_lives_ago = now - days_to_seconds(2.0 * HALF_LIFE_DAYS);
        let score = calculate_score(count, two_half_lives_ago, HALF_LIFE_DAYS);

        // After 2 half-lives, should be 25% (0.5^2 = 0.25)
        let expected = f64::from(count) * 0.25;
        assert!(
            (score - expected).abs() < 0.1,
            "Expected ~{} (25% of {}), got {} - two half-lives should give 25% remaining",
            expected,
            count,
            score
        );
    }
    #[test]
    fn test_calculate_score_old_item() {
        let now = current_timestamp();
        let count = 100;

        // 30 days ago (about 4.3 half-lives with 7-day half-life)
        let thirty_days_ago = now - (30 * SECONDS_PER_DAY_U64);
        let score = calculate_score(count, thirty_days_ago, HALF_LIFE_DAYS);

        // With true half-life: 100 * 0.5^(30/7) = 100 * 0.5^4.28 ≈ 5.15
        // Should be heavily decayed but still detectable
        let expected = f64::from(count) * 0.5_f64.powf(30.0 / HALF_LIFE_DAYS);
        assert!(
            (score - expected).abs() < 0.5,
            "Expected ~{:.2}, got {:.2}",
            expected,
            score
        );
        // Verify it's indeed heavily decayed (less than 10% of original)
        assert!(score < 10.0, "Should be heavily decayed, got {}", score);
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
    fn test_frecency_store_record_use_respects_configured_half_life() {
        // Create two stores with different half-lives
        let temp_dir = std::env::temp_dir();
        let path1 = temp_dir.join(format!("frecency_test_hl1_{}.json", uuid::Uuid::new_v4()));
        let path2 = temp_dir.join(format!("frecency_test_hl2_{}.json", uuid::Uuid::new_v4()));

        // Store with short half-life (1 day) - scores should decay faster
        let mut store_short = FrecencyStore::with_path(path1.clone());
        store_short.set_half_life_days(1.0);

        // Store with long half-life (30 days) - scores should decay slower
        let mut store_long = FrecencyStore::with_path(path2.clone());
        store_long.set_half_life_days(30.0);

        // Record use on both stores
        store_short.record_use("/test.ts");
        store_long.record_use("/test.ts");

        // Scores should be identical right after use (both just recorded)
        let score_short = store_short.get_score("/test.ts");
        let score_long = store_long.get_score("/test.ts");

        // Both should be approximately 1.0 (count=1, no decay yet)
        assert!(
            (score_short - 1.0).abs() < 0.01,
            "Short half-life store: expected ~1.0, got {}",
            score_short
        );
        assert!(
            (score_long - 1.0).abs() < 0.01,
            "Long half-life store: expected ~1.0, got {}",
            score_long
        );

        cleanup_temp_file(&path1);
        cleanup_temp_file(&path2);
    }
    #[test]
    fn test_frecency_store_record_use_uses_store_half_life_not_default() {
        // This test verifies that record_use() uses the store's configured half-life
        // instead of the DEFAULT_SUGGESTED_HALF_LIFE_DAYS constant
        //
        // With the incremental model: new_score = old_score * decay(elapsed) + 1
        // Different half-lives produce different decay factors for the same elapsed time.
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_hl_{}.json", uuid::Uuid::new_v4()));

        // Create store with custom half-life (much longer than default 7 days)
        let mut store = FrecencyStore::with_path(path.clone());
        let custom_half_life = 30.0; // 30 days (longer than default 7)
        store.set_half_life_days(custom_half_life);

        // Manually create an entry with an old timestamp (7 days ago) and accumulated score
        let now = current_timestamp();
        let seven_days_ago = now - (7 * SECONDS_PER_DAY_U64);
        let old_entry = FrecencyEntry {
            count: 5,
            last_used: seven_days_ago,
            score: 5.0, // Accumulated score as of 7 days ago
        };
        store.entries.insert("/test.ts".to_string(), old_entry);

        // Now record another use - this should apply decay with custom half-life (30 days)
        store.record_use("/test.ts");

        // Get the entry and verify it was calculated with the custom half-life
        let entry = store.entries.get("/test.ts").expect("Entry should exist");

        // count should now be 6
        assert_eq!(entry.count, 6, "Count should be incremented to 6");

        // With incremental model: new_score = old_score * decay(7 days, half_life=30) + 1
        // decay(7 days, 30) = 2^(-7/30) ≈ 0.85
        // new_score = 5.0 * 0.85 + 1.0 ≈ 5.25
        let decay_factor_30 = 2f64.powf(-7.0 / 30.0); // ≈ 0.85
        let expected_score = 5.0 * decay_factor_30 + 1.0;

        assert!(
            (entry.score - expected_score).abs() < 0.1,
            "Entry score {} should match expected {} using custom half-life {}. \
             With 30-day half-life, 7 days only decays to ~85%. \
             If this is wrong, record_use() isn't using store config.",
            entry.score,
            expected_score,
            custom_half_life
        );

        // With default 7-day half-life, 7 days would decay to 50%:
        // default_score = 5.0 * 0.5 + 1.0 = 3.5
        let decay_factor_7 = 0.5;
        let default_half_life_score = 5.0 * decay_factor_7 + 1.0;

        // Our score should be higher than what we'd get with default half-life
        // because 30-day half-life decays more slowly
        assert!(
            entry.score > default_half_life_score,
            "Score {} with 30-day half-life should be higher than {} with 7-day half-life",
            entry.score,
            default_half_life_score
        );

        cleanup_temp_file(&path);
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
    fn test_frecency_store_top_file_paths_only_returns_file_entries() {
        let (mut store, _temp) = create_test_store();

        store.record_use("builtin:search-files");
        store.record_use("file//tmp/older.txt");
        store.record_use("file//tmp/recent.txt");
        store.record_use("file//tmp/recent.txt");
        store.record_use("builtin:open");
        store.record_use("builtin:open");
        store.record_use("builtin:open");

        let recent = store.top_file_paths(1);

        assert_eq!(recent[0].0, "/tmp/recent.txt");
        assert!(
            recent.iter().all(|(path, _)| !path.starts_with("builtin:")),
            "top_file_paths should expose only stripped file paths"
        );
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
        // score is NOT serialized (it's derived on load), so we only verify
        // that count and last_used round-trip correctly
        let entry = FrecencyEntry {
            count: 5,
            last_used: 1704067200, // 2024-01-01 00:00:00 UTC
            score: 4.5,            // This will NOT be serialized
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FrecencyEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.count, deserialized.count);
        assert_eq!(entry.last_used, deserialized.last_used);
        // score defaults to 0.0 since it's not serialized
        assert_eq!(
            deserialized.score, 0.0,
            "Score should default to 0.0 when deserializing"
        );
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
    // --- merged from part_001.rs ---
    // ========================================
    // New incremental frecency model tests
    // ========================================

    #[test]
    fn test_score_at_computes_decay_at_query_time() {
        // score_at() should compute the decayed score at query time,
        // not return a stale cached value
        let now = current_timestamp();
        let half_life = 7.0;

        let entry = FrecencyEntry {
            count: 5,
            last_used: now - (7 * SECONDS_PER_DAY_U64), // 7 days ago
            score: 10.0,                                // stored score (as of last_used)
        };

        // score_at(now) should decay the stored score by elapsed time
        // 7 days = 1 half-life, so score should be ~10.0 * 0.5 = 5.0
        let score = entry.score_at(now, half_life);
        assert!(
            (score - 5.0).abs() < 0.1,
            "Expected ~5.0 (50% of 10.0), got {}",
            score
        );
    }
    #[test]
    fn test_score_at_zero_elapsed_time() {
        // When queried at the exact moment of last_used, no decay
        let now = current_timestamp();

        let entry = FrecencyEntry {
            count: 1,
            last_used: now,
            score: 3.0,
        };

        let score = entry.score_at(now, 7.0);
        assert!(
            (score - 3.0).abs() < 0.01,
            "No decay expected, got {}",
            score
        );
    }
    #[test]
    fn test_record_use_with_timestamp_incremental_model() {
        // Test the new incremental model: score = score*decay(dt) + 1
        let now = current_timestamp();
        let half_life = 7.0;

        let mut entry = FrecencyEntry {
            count: 10,
            last_used: now - (7 * SECONDS_PER_DAY_U64), // 7 days ago
            score: 4.0,                                 // accumulated score as of 7 days ago
        };

        // record_use should:
        // 1. compute current score: 4.0 * 0.5 = 2.0 (one half-life decay)
        // 2. add 1 for new use: 2.0 + 1.0 = 3.0
        entry.record_use_with_timestamp(now, half_life);

        assert_eq!(entry.count, 11);
        assert_eq!(entry.last_used, now);
        assert!(
            (entry.score - 3.0).abs() < 0.1,
            "Expected ~3.0 (2.0 decayed + 1.0 new), got {}",
            entry.score
        );
    }
    #[test]
    fn test_incremental_model_prevents_rich_get_richer() {
        // Scenario: Script A was used 100 times last year (then abandoned)
        // Script B was used 3 times this week
        // Script B should rank higher
        let now = current_timestamp();
        let half_life = 7.0;
        let year_ago = now - (365 * SECONDS_PER_DAY_U64);
        let two_days_ago = now - (2 * SECONDS_PER_DAY_U64);

        // Script A: high historical usage, long ago
        // With incremental model, even if it had score=100, after a year it's nearly 0
        // 365/7 ≈ 52 half-lives, 0.5^52 ≈ 2.2e-16
        let entry_a = FrecencyEntry {
            count: 100,
            last_used: year_ago,
            score: 100.0, // high accumulated score... a year ago
        };

        // Script B: recent usage
        let entry_b = FrecencyEntry {
            count: 3,
            last_used: two_days_ago,
            score: 3.0, // recently accumulated
        };

        let score_a = entry_a.score_at(now, half_life);
        let score_b = entry_b.score_at(now, half_life);

        assert!(
            score_b > score_a,
            "Recent script B (score={}) should rank higher than abandoned A (score={})",
            score_b,
            score_a
        );
    }
    #[test]
    fn test_get_recent_items_uses_live_scores() {
        // get_recent_items() should compute score_at(now) for ranking,
        // not use stale cached scores
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_live_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();
        let week_ago = now - (7 * SECONDS_PER_DAY_U64);

        // Insert entries with explicit timestamps via entries map
        // Entry A: high stored score but old
        store.entries.insert(
            "/old-popular.ts".to_string(),
            FrecencyEntry {
                count: 50,
                last_used: week_ago,
                score: 10.0, // will decay to ~5.0
            },
        );

        // Entry B: lower stored score but recent
        store.entries.insert(
            "/recent.ts".to_string(),
            FrecencyEntry {
                count: 3,
                last_used: now,
                score: 3.0, // no decay
            },
        );

        let _recent = store.get_recent_items(10);

        // This test is superseded by test_get_recent_items_live_vs_stale
        // which uses more extreme values to clearly demonstrate live computation.
        // This test just verifies no crashes with mixed timestamp entries.
        cleanup_temp_file(&path);
    }
    #[test]
    fn test_get_recent_items_live_vs_stale() {
        // This test verifies that get_recent_items uses live scores
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_live2_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();
        let month_ago = now - (30 * SECONDS_PER_DAY_U64); // ~4.3 half-lives

        // Entry A: very high stored score but a month old
        // 4.3 half-lives: 0.5^4.3 ≈ 0.05, so 100 * 0.05 = ~5.0 live
        store.entries.insert(
            "/old.ts".to_string(),
            FrecencyEntry {
                count: 100,
                last_used: month_ago,
                score: 100.0, // stale score
            },
        );

        // Entry B: moderate stored score but recent
        store.entries.insert(
            "/recent.ts".to_string(),
            FrecencyEntry {
                count: 8,
                last_used: now,
                score: 8.0, // live score
            },
        );

        let recent = store.get_recent_items(10);

        // With live computation: /recent.ts (8.0) should beat /old.ts (~5.0)
        assert_eq!(
            recent[0].0, "/recent.ts",
            "Recent script should rank first with live score computation. \
             Got {:?}",
            recent
        );

        cleanup_temp_file(&path);
    }
    // ========================================
    // Revision counter tests
    // ========================================

    #[test]
    fn test_revision_increments_on_record_use() {
        let (mut store, path) = create_test_store();
        let initial_rev = store.revision();

        store.record_use("/test.ts");

        assert!(
            store.revision() > initial_rev,
            "Revision should increment after record_use"
        );
        cleanup_temp_file(&path);
    }
    #[test]
    fn test_revision_increments_on_remove() {
        let (mut store, path) = create_test_store();
        store.record_use("/test.ts");
        let rev_after_add = store.revision();

        store.remove("/test.ts");

        assert!(
            store.revision() > rev_after_add,
            "Revision should increment after remove"
        );
        cleanup_temp_file(&path);
    }
    #[test]
    fn test_revision_increments_on_clear() {
        let (mut store, path) = create_test_store();
        store.record_use("/test.ts");
        let rev_after_add = store.revision();

        store.clear();

        assert!(
            store.revision() > rev_after_add,
            "Revision should increment after clear"
        );
        cleanup_temp_file(&path);
    }
    #[test]
    fn test_revision_increments_on_half_life_change() {
        let (mut store, path) = create_test_store();
        let initial_rev = store.revision();

        store.set_half_life_days(14.0);

        assert!(
            store.revision() > initial_rev,
            "Revision should increment after half-life change"
        );
        cleanup_temp_file(&path);
    }
    // ========================================
    // Deterministic tie-breaker tests
    // ========================================

    #[test]
    fn test_tie_breaker_by_last_used() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_tie_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();

        // Two items with identical scores but different last_used
        store.entries.insert(
            "/older.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: now - 100, // 100 seconds older
                score: 1.0,
            },
        );
        store.entries.insert(
            "/newer.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: now,
                score: 1.0,
            },
        );

        let recent = store.get_recent_items(10);

        // With tie-breaker by last_used desc, newer should be first
        assert_eq!(
            recent[0].0, "/newer.ts",
            "More recent item should win tie-breaker"
        );
        cleanup_temp_file(&path);
    }
    #[test]
    fn test_tie_breaker_by_path() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_tie2_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();

        // Two items with identical scores AND identical last_used
        store.entries.insert(
            "/bbb.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: now,
                score: 1.0,
            },
        );
        store.entries.insert(
            "/aaa.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: now,
                score: 1.0,
            },
        );

        let recent = store.get_recent_items(10);

        // With tie-breaker by path asc, /aaa.ts should be first
        assert_eq!(
            recent[0].0, "/aaa.ts",
            "Alphabetically first path should win final tie-breaker"
        );
        cleanup_temp_file(&path);
    }
    // ========================================
    // Atomic save tests
    // ========================================

    #[test]
    fn test_save_creates_valid_json() {
        let (mut store, path) = create_test_store();
        store.record_use("/test.ts");
        store.save().unwrap();

        // Verify the file exists and contains valid JSON
        let content = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("entries").is_some());

        cleanup_temp_file(&path);
    }
    #[test]
    fn test_save_no_temp_file_left_behind() {
        let (mut store, path) = create_test_store();
        store.record_use("/test.ts");
        store.save().unwrap();

        // Verify no temp file is left behind
        let temp_path = path.with_extension("json.tmp");
        assert!(
            !temp_path.exists(),
            "Temp file should be cleaned up after save"
        );

        cleanup_temp_file(&path);
    }
    #[test]
    fn test_save_preserves_data_integrity() {
        let (mut store, path) = create_test_store();

        // Add multiple entries
        store.record_use("/a.ts");
        store.record_use("/a.ts");
        store.record_use("/b.ts");
        store.save().unwrap();

        // Load into new store and verify data
        let mut loaded = FrecencyStore::with_path(path.clone());
        loaded.load().unwrap();

        assert_eq!(loaded.len(), 2);
        assert!(loaded.get_score("/a.ts") > 0.0);
        assert!(loaded.get_score("/b.ts") > 0.0);

        cleanup_temp_file(&path);
    }
    #[test]
    fn test_save_uses_compact_json() {
        // Verify we're not using pretty-print (for performance)
        let (mut store, path) = create_test_store();
        store.record_use("/test.ts");
        store.save().unwrap();

        let content = fs::read_to_string(&path).unwrap();

        // Compact JSON should not have excessive newlines
        // Pretty JSON has newlines after every field
        let newline_count = content.matches('\n').count();
        // Compact JSON has at most a few newlines (maybe 0-2)
        // Pretty JSON with 1 entry would have ~5+ newlines
        assert!(
            newline_count <= 2,
            "Expected compact JSON with few newlines, got {} newlines",
            newline_count
        );

        cleanup_temp_file(&path);
    }
    // ========================================
    // Clock injection tests
    // ========================================

    #[test]
    fn test_record_use_with_injected_timestamp() {
        // Test that we can inject timestamps for deterministic testing
        let (mut store, path) = create_test_store();
        let fixed_time = 1704067200u64; // 2024-01-01 00:00:00 UTC

        store.record_use_at("/test.ts", fixed_time);

        let entry = store.entries.get("/test.ts").expect("Entry should exist");
        assert_eq!(entry.last_used, fixed_time, "Should use injected timestamp");

        cleanup_temp_file(&path);
    }
    // --- merged from part_002.rs ---
    #[test]
    fn test_get_recent_items_at_timestamp() {
        // Test that we can compute rankings at a specific timestamp
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_at_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let base_time = 1704067200u64; // 2024-01-01
        let week_later = base_time + (7 * SECONDS_PER_DAY_U64);

        // Insert entries with explicit timestamps
        // /old.ts: score=10.0 at base_time, will decay to 5.0 at week_later (1 half-life)
        store.entries.insert(
            "/old.ts".to_string(),
            FrecencyEntry {
                count: 10,
                last_used: base_time,
                score: 10.0,
            },
        );
        // /recent.ts: score=8.0 at week_later, no decay when queried at week_later
        // This beats the decayed /old.ts score of 5.0
        store.entries.insert(
            "/recent.ts".to_string(),
            FrecencyEntry {
                count: 8,
                last_used: week_later,
                score: 8.0,
            },
        );

        // Query at week_later:
        // - /old.ts: 10.0 * 0.5 = 5.0 (decayed by 1 half-life)
        // - /recent.ts: 8.0 * 1.0 = 8.0 (no decay)
        // So /recent.ts should rank first
        let recent = store.get_recent_items_at(10, week_later);

        assert_eq!(
            recent[0].0, "/recent.ts",
            "Recent script (8.0) should rank first over decayed old script (5.0) at week_later"
        );

        // Query at base_time:
        // - /old.ts: 10.0 * 1.0 = 10.0 (no decay at creation time)
        // - /recent.ts: last_used is in the future, so dt < 0, saturating to 0 -> no decay = 8.0
        // Wait, that's wrong - /recent.ts wasn't created yet at base_time!
        // score_at handles future timestamps by using saturating_sub which gives 0 elapsed time
        // So both get no decay, and /old.ts (10.0) > /recent.ts (8.0)
        let at_base = store.get_recent_items_at(10, base_time);

        assert_eq!(
            at_base[0].0, "/old.ts",
            "Old script (10.0) should rank first over future recent script (8.0) at base_time"
        );

        cleanup_temp_file(&path);
    }
    // ========================================
    // Score skip_serializing tests
    // ========================================

    #[test]
    fn test_score_not_serialized() {
        // Score should not be written to JSON (it's derived on load)
        let entry = FrecencyEntry {
            count: 5,
            last_used: 1704067200,
            score: 999.0, // This should not be serialized
        };

        let json = serde_json::to_string(&entry).unwrap();

        // JSON should NOT contain "score" field
        assert!(
            !json.contains("\"score\""),
            "score field should not be serialized, but got: {}",
            json
        );
    }
    #[test]
    fn test_score_defaults_on_deserialization() {
        // When loading old data without score, it should default to 0.0
        let json = r#"{"count": 5, "last_used": 1704067200}"#;
        let entry: FrecencyEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.count, 5);
        assert_eq!(entry.last_used, 1704067200);
        assert_eq!(entry.score, 0.0, "Score should default to 0.0");
    }
    // ========================================
    // Pruning tests
    // ========================================

    #[test]
    fn test_prune_stale_entries() {
        // Entries with very low scores and old timestamps should be prunable
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!("frecency_test_prune_{}.json", uuid::Uuid::new_v4()));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();
        let year_ago = now - (365 * SECONDS_PER_DAY_U64);
        let week_ago = now - (7 * SECONDS_PER_DAY_U64);

        // Old entry that should be pruned
        store.entries.insert(
            "/abandoned.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: year_ago,
                score: 0.001, // Very low score after decay
            },
        );

        // Recent entry that should be kept
        store.entries.insert(
            "/active.ts".to_string(),
            FrecencyEntry {
                count: 5,
                last_used: week_ago,
                score: 5.0,
            },
        );

        // Current entry that should be kept
        store.entries.insert(
            "/current.ts".to_string(),
            FrecencyEntry {
                count: 1,
                last_used: now,
                score: 1.0,
            },
        );

        // Prune entries with live score < 0.01 AND last_used > 180 days ago
        let pruned_count = store.prune_stale_entries(0.01, 180);

        assert_eq!(pruned_count, 1, "Should prune exactly 1 stale entry");
        assert_eq!(store.len(), 2, "Should have 2 entries remaining");
        assert!(
            store.entries.contains_key("/active.ts"),
            "Active entry should remain"
        );
        assert!(
            store.entries.contains_key("/current.ts"),
            "Current entry should remain"
        );
        assert!(
            !store.entries.contains_key("/abandoned.ts"),
            "Abandoned entry should be pruned"
        );
        assert!(store.is_dirty(), "Store should be dirty after pruning");

        cleanup_temp_file(&path);
    }
    #[test]
    fn test_prune_keeps_high_score_old_entries() {
        // Old entries with high accumulated score should NOT be pruned
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(format!(
            "frecency_test_prune2_{}.json",
            uuid::Uuid::new_v4()
        ));
        let mut store = FrecencyStore::with_path(path.clone());

        let now = current_timestamp();
        let year_ago = now - (365 * SECONDS_PER_DAY_U64);

        // Old but heavily used entry - even after a year of decay,
        // if it had score=1000, it might still be above threshold
        store.entries.insert(
            "/popular-old.ts".to_string(),
            FrecencyEntry {
                count: 1000,
                last_used: year_ago,
                score: 1000.0, // High accumulated score
            },
        );

        // With 7-day half-life, after 365 days (52 half-lives):
        // 1000 * 0.5^52 ≈ 0.0000000000000002 - this WOULD be pruned
        // So this test verifies the score_threshold works correctly
        let pruned = store.prune_stale_entries(0.0001, 180);

        // This entry SHOULD be pruned because even high scores decay to nothing after a year
        assert_eq!(
            pruned, 1,
            "Even high-score entries should be pruned after sufficient decay"
        );

        cleanup_temp_file(&path);
    }
    #[test]
    fn test_prune_no_op_on_empty_store() {
        let (mut store, path) = create_test_store();

        let pruned = store.prune_stale_entries(0.01, 180);

        assert_eq!(pruned, 0, "Empty store should prune nothing");
        assert!(!store.is_dirty(), "Empty store should not be marked dirty");

        cleanup_temp_file(&path);
    }

    /// Battery 13 lock: a corrupt frecency file must be preserved as a
    /// `.corrupt` sidecar by the failed load, so the caller's fresh-start
    /// save cannot silently destroy the user's history.
    #[test]
    fn test_corrupt_frecency_rescued_to_sidecar_before_fresh_start() {
        let (_, path) = create_test_store();
        let corrupt = "{\"entries\":{\"script/foo.ts\":{\"count\":5,\"last_used\":175";
        fs::write(&path, corrupt).unwrap();

        let mut store = FrecencyStore::with_path(path.clone());
        assert!(store.load().is_err(), "corrupt load must still surface Err");
        assert!(store.is_empty());

        let rescue = PathBuf::from(format!("{}.corrupt", path.display()));
        assert!(rescue.exists(), "corrupt file must be preserved as sidecar");
        assert_eq!(fs::read_to_string(&rescue).unwrap(), corrupt);
        assert!(
            !path.exists(),
            "original path must be clear for a fresh start"
        );

        // The fresh-start save must not touch the sidecar.
        store.record_use("script/new.ts");
        store.save().unwrap();
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&rescue).unwrap(), corrupt);

        cleanup_temp_file(&path);
        cleanup_temp_file(&rescue);
    }

    /// Battery 13: interleaved saves from independent stores sharing one path
    /// (two-app-instances shape) must always leave one writer's COMPLETE
    /// output — never torn JSON (write_atomic contract, exercised end-to-end).
    #[test]
    fn test_concurrent_frecency_saves_never_tear() {
        let (_, path) = create_test_store();
        let _ = fs::remove_file(&path);
        let mut handles = Vec::new();
        for t in 0..4 {
            let path = path.clone();
            handles.push(std::thread::spawn(move || {
                let mut store = FrecencyStore::with_path(path);
                for i in 0..25 {
                    store.record_use(&format!("script/t{}-{}.ts", t, i));
                    store.save().unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let content = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .expect("final frecency file must be complete valid JSON");
        assert!(parsed.get("entries").is_some());
        cleanup_temp_file(&path);
    }
}
