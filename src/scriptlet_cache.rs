//! Scriptlet cache module for tracking per-file scriptlet state with change detection.
//!
//! This module provides:
//! - `CachedScriptlet`: Lightweight struct tracking scriptlet registration metadata
//! - `CachedScriptletFile`: Per-file cache with mtime for staleness detection
//! - `ScriptletCache`: HashMap-based cache for all scriptlet files
//! - `ScriptletDiff`: Diff result identifying what changed between old and new scriptlets
//!
//! # Usage
//!
//! ```rust,ignore
//! use script_kit_gpui::scriptlet_cache::{ScriptletCache, CachedScriptlet, diff_scriptlets};
//!
//! let mut cache = ScriptletCache::new();
//!
//! // Add a file's scriptlets
//! let scriptlets = vec![
//!     CachedScriptlet::new("My Snippet", Some("cmd+shift+m"), None, None, "/path/to/file.md#my-snippet"),
//! ];
//! cache.update_file("/path/to/file.md", mtime, scriptlets);
//!
//! // Check if file is stale
//! if cache.is_stale("/path/to/file.md", current_mtime) {
//!     let old = cache.get_scriptlets("/path/to/file.md").unwrap_or_default();
//!     let new = load_scriptlets_from_file("/path/to/file.md");
//!     let diff = diff_scriptlets(&old, &new);
//!     // Apply diff to hotkeys/expand_manager
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Lightweight struct tracking scriptlet registration metadata.
/// This is a subset of the full Scriptlet struct, containing only
/// the fields needed for change detection and registration updates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedScriptlet {
    /// Name of the scriptlet (used as identifier)
    pub name: String,
    /// Keyboard shortcut (e.g., "cmd shift k")
    pub shortcut: Option<String>,
    /// Text expansion trigger (e.g., "type,,")
    pub expand: Option<String>,
    /// Alias trigger (e.g., "gpt")
    pub alias: Option<String>,
    /// Source file path with anchor (e.g., "/path/to/file.md#my-snippet")
    pub file_path: String,
}

impl CachedScriptlet {
    /// Create a new CachedScriptlet
    pub fn new(
        name: impl Into<String>,
        shortcut: Option<String>,
        expand: Option<String>,
        alias: Option<String>,
        file_path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            shortcut,
            expand,
            alias,
            file_path: file_path.into(),
        }
    }
}

/// Per-file cache entry tracking scriptlets and modification time.
#[derive(Clone, Debug)]
pub struct CachedScriptletFile {
    /// Absolute path to the markdown file
    pub path: PathBuf,
    /// Last modification time when the file was cached
    pub mtime: SystemTime,
    /// Scriptlets extracted from this file
    pub scriptlets: Vec<CachedScriptlet>,
}

impl CachedScriptletFile {
    /// Create a new CachedScriptletFile
    pub fn new(
        path: impl Into<PathBuf>,
        mtime: SystemTime,
        scriptlets: Vec<CachedScriptlet>,
    ) -> Self {
        Self {
            path: path.into(),
            mtime,
            scriptlets,
        }
    }
}

/// Cache for all scriptlet files, providing staleness detection and CRUD operations.
#[derive(Debug, Default)]
pub struct ScriptletCache {
    files: HashMap<PathBuf, CachedScriptletFile>,
}

impl ScriptletCache {
    /// Create a new empty ScriptletCache
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Check if a file is stale (mtime differs from cached mtime)
    pub fn is_stale(&self, path: impl AsRef<Path>, current_mtime: SystemTime) -> bool {
        match self.files.get(path.as_ref()) {
            Some(cached) => cached.mtime != current_mtime,
            None => true, // Not in cache means stale (needs initial load)
        }
    }

    /// Get the cached scriptlets for a file
    pub fn get_scriptlets(&self, path: impl AsRef<Path>) -> Option<Vec<CachedScriptlet>> {
        self.files.get(path.as_ref()).map(|f| f.scriptlets.clone())
    }

    /// Get the cached file entry
    pub fn get_file(&self, path: impl AsRef<Path>) -> Option<&CachedScriptletFile> {
        self.files.get(path.as_ref())
    }

    /// Update or insert a file's scriptlets in the cache
    pub fn update_file(
        &mut self,
        path: impl Into<PathBuf>,
        mtime: SystemTime,
        scriptlets: Vec<CachedScriptlet>,
    ) {
        let path = path.into();
        self.files.insert(
            path.clone(),
            CachedScriptletFile::new(path, mtime, scriptlets),
        );
    }

    /// Remove a file from the cache
    pub fn remove_file(&mut self, path: impl AsRef<Path>) -> Option<CachedScriptletFile> {
        self.files.remove(path.as_ref())
    }

    /// Get the number of cached files
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get all cached file paths
    pub fn file_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.files.clear();
    }
}

/// Represents a change to a scriptlet's shortcut
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutChange {
    pub name: String,
    pub file_path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Represents a change to a scriptlet's expand trigger
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpandChange {
    pub name: String,
    pub file_path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Represents a change to a scriptlet's alias
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasChange {
    pub name: String,
    pub file_path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Diff result identifying what changed between old and new scriptlets.
/// Used to update hotkey and expand registrations incrementally.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptletDiff {
    /// Scriptlets that were added (not present in old)
    pub added: Vec<CachedScriptlet>,
    /// Scriptlets that were removed (not present in new)
    pub removed: Vec<CachedScriptlet>,
    /// Scriptlets whose shortcut changed
    pub shortcut_changes: Vec<ShortcutChange>,
    /// Scriptlets whose expand trigger changed
    pub expand_changes: Vec<ExpandChange>,
    /// Scriptlets whose alias changed
    pub alias_changes: Vec<AliasChange>,
}

impl ScriptletDiff {
    /// Check if there are no changes
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.shortcut_changes.is_empty()
            && self.expand_changes.is_empty()
            && self.alias_changes.is_empty()
    }

    /// Get total number of changes
    pub fn change_count(&self) -> usize {
        self.added.len()
            + self.removed.len()
            + self.shortcut_changes.len()
            + self.expand_changes.len()
            + self.alias_changes.len()
    }
}

/// Compute the diff between old and new scriptlets.
///
/// Scriptlets are matched by name. A scriptlet is considered:
/// - **Added**: Present in new but not in old
/// - **Removed**: Present in old but not in new
/// - **Changed**: Present in both but with different shortcut/expand/alias
pub fn diff_scriptlets(old: &[CachedScriptlet], new: &[CachedScriptlet]) -> ScriptletDiff {
    let mut diff = ScriptletDiff::default();

    // Build lookup maps by name
    let old_by_name: HashMap<&str, &CachedScriptlet> =
        old.iter().map(|s| (s.name.as_str(), s)).collect();
    let new_by_name: HashMap<&str, &CachedScriptlet> =
        new.iter().map(|s| (s.name.as_str(), s)).collect();

    // Find added and changed
    for new_scriptlet in new {
        match old_by_name.get(new_scriptlet.name.as_str()) {
            Some(old_scriptlet) => {
                // Check for changes
                if old_scriptlet.shortcut != new_scriptlet.shortcut {
                    diff.shortcut_changes.push(ShortcutChange {
                        name: new_scriptlet.name.clone(),
                        file_path: new_scriptlet.file_path.clone(),
                        old: old_scriptlet.shortcut.clone(),
                        new: new_scriptlet.shortcut.clone(),
                    });
                }
                if old_scriptlet.expand != new_scriptlet.expand {
                    diff.expand_changes.push(ExpandChange {
                        name: new_scriptlet.name.clone(),
                        file_path: new_scriptlet.file_path.clone(),
                        old: old_scriptlet.expand.clone(),
                        new: new_scriptlet.expand.clone(),
                    });
                }
                if old_scriptlet.alias != new_scriptlet.alias {
                    diff.alias_changes.push(AliasChange {
                        name: new_scriptlet.name.clone(),
                        file_path: new_scriptlet.file_path.clone(),
                        old: old_scriptlet.alias.clone(),
                        new: new_scriptlet.alias.clone(),
                    });
                }
            }
            None => {
                // Added
                diff.added.push(new_scriptlet.clone());
            }
        }
    }

    // Find removed
    for old_scriptlet in old {
        if !new_by_name.contains_key(old_scriptlet.name.as_str()) {
            diff.removed.push(old_scriptlet.clone());
        }
    }

    diff
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Helper to create a test mtime
    fn test_mtime(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    // -------------------------------------------------------------------------
    // Cache tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cache_add_and_retrieve() {
        let mut cache = ScriptletCache::new();
        let mtime = test_mtime(1000);
        let path = PathBuf::from("/path/to/file.md");

        let scriptlets = vec![
            CachedScriptlet::new(
                "Snippet One",
                Some("cmd+shift+1".to_string()),
                None,
                None,
                "/path/to/file.md#snippet-one",
            ),
            CachedScriptlet::new(
                "Snippet Two",
                None,
                Some("snip,,".to_string()),
                None,
                "/path/to/file.md#snippet-two",
            ),
        ];

        cache.update_file(&path, mtime, scriptlets.clone());

        // Verify file is in cache
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        // Retrieve scriptlets
        let retrieved = cache.get_scriptlets(&path).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].name, "Snippet One");
        assert_eq!(retrieved[0].shortcut, Some("cmd+shift+1".to_string()));
        assert_eq!(retrieved[1].name, "Snippet Two");
        assert_eq!(retrieved[1].expand, Some("snip,,".to_string()));

        // Verify get_file
        let file = cache.get_file(&path).unwrap();
        assert_eq!(file.mtime, mtime);
        assert_eq!(file.path, path);
    }

    #[test]
    fn test_cache_staleness_detection() {
        let mut cache = ScriptletCache::new();
        let path = PathBuf::from("/path/to/file.md");
        let mtime_old = test_mtime(1000);
        let mtime_new = test_mtime(2000);

        // Not in cache = stale
        assert!(cache.is_stale(&path, mtime_old));

        // Add to cache
        cache.update_file(&path, mtime_old, vec![]);

        // Same mtime = not stale
        assert!(!cache.is_stale(&path, mtime_old));

        // Different mtime = stale
        assert!(cache.is_stale(&path, mtime_new));
    }

    #[test]
    fn test_cache_remove_file() {
        let mut cache = ScriptletCache::new();
        let path = PathBuf::from("/path/to/file.md");
        let mtime = test_mtime(1000);

        let scriptlets = vec![CachedScriptlet::new(
            "Test",
            None,
            None,
            None,
            "/path/to/file.md#test",
        )];

        cache.update_file(&path, mtime, scriptlets);
        assert_eq!(cache.len(), 1);

        // Remove file
        let removed = cache.remove_file(&path);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().scriptlets.len(), 1);

        // Verify removed
        assert_eq!(cache.len(), 0);
        assert!(cache.get_scriptlets(&path).is_none());

        // Remove non-existent returns None
        assert!(cache.remove_file(&path).is_none());
    }

    #[test]
    fn test_cache_update_existing() {
        let mut cache = ScriptletCache::new();
        let path = PathBuf::from("/path/to/file.md");
        let mtime1 = test_mtime(1000);
        let mtime2 = test_mtime(2000);

        // Initial add
        let scriptlets1 = vec![CachedScriptlet::new(
            "Original",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#original",
        )];
        cache.update_file(&path, mtime1, scriptlets1);

        // Update with new data
        let scriptlets2 = vec![
            CachedScriptlet::new(
                "Updated",
                Some("cmd+2".to_string()),
                None,
                None,
                "/path/to/file.md#updated",
            ),
            CachedScriptlet::new(
                "New One",
                None,
                Some("new,,".to_string()),
                None,
                "/path/to/file.md#new-one",
            ),
        ];
        cache.update_file(&path, mtime2, scriptlets2);

        // Verify update
        assert_eq!(cache.len(), 1); // Still one file
        let file = cache.get_file(&path).unwrap();
        assert_eq!(file.mtime, mtime2);
        assert_eq!(file.scriptlets.len(), 2);
        assert_eq!(file.scriptlets[0].name, "Updated");
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = ScriptletCache::new();
        let mtime = test_mtime(1000);

        cache.update_file("/path/a.md", mtime, vec![]);
        cache.update_file("/path/b.md", mtime, vec![]);
        cache.update_file("/path/c.md", mtime, vec![]);

        assert_eq!(cache.len(), 3);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_file_paths() {
        let mut cache = ScriptletCache::new();
        let mtime = test_mtime(1000);

        cache.update_file("/path/a.md", mtime, vec![]);
        cache.update_file("/path/b.md", mtime, vec![]);

        let paths: Vec<_> = cache.file_paths().collect();
        assert_eq!(paths.len(), 2);
    }

    // -------------------------------------------------------------------------
    // Diff tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_diff_no_changes() {
        let scriptlets = vec![
            CachedScriptlet::new(
                "Snippet One",
                Some("cmd+1".to_string()),
                None,
                Some("s1".to_string()),
                "/path/to/file.md#snippet-one",
            ),
            CachedScriptlet::new(
                "Snippet Two",
                None,
                Some("snip,,".to_string()),
                None,
                "/path/to/file.md#snippet-two",
            ),
        ];

        let diff = diff_scriptlets(&scriptlets, &scriptlets);

        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_diff_scriptlet_added() {
        let old = vec![CachedScriptlet::new(
            "Existing",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#existing",
        )];

        let new = vec![
            CachedScriptlet::new(
                "Existing",
                Some("cmd+1".to_string()),
                None,
                None,
                "/path/to/file.md#existing",
            ),
            CachedScriptlet::new(
                "New Snippet",
                Some("cmd+2".to_string()),
                Some("new,,".to_string()),
                None,
                "/path/to/file.md#new-snippet",
            ),
        ];

        let diff = diff_scriptlets(&old, &new);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "New Snippet");
        assert_eq!(diff.added[0].shortcut, Some("cmd+2".to_string()));
        assert_eq!(diff.added[0].expand, Some("new,,".to_string()));
        assert!(diff.removed.is_empty());
        assert!(diff.shortcut_changes.is_empty());
        assert!(diff.expand_changes.is_empty());
        assert!(diff.alias_changes.is_empty());
    }

    #[test]
    fn test_diff_scriptlet_removed() {
        let old = vec![
            CachedScriptlet::new(
                "Will Stay",
                Some("cmd+1".to_string()),
                None,
                None,
                "/path/to/file.md#will-stay",
            ),
            CachedScriptlet::new(
                "Will Be Removed",
                Some("cmd+2".to_string()),
                None,
                None,
                "/path/to/file.md#will-be-removed",
            ),
        ];

        let new = vec![CachedScriptlet::new(
            "Will Stay",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#will-stay",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert!(diff.added.is_empty());
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "Will Be Removed");
        assert!(diff.shortcut_changes.is_empty());
        assert!(diff.expand_changes.is_empty());
        assert!(diff.alias_changes.is_empty());
    }

    #[test]
    fn test_diff_shortcut_changed() {
        let old = vec![CachedScriptlet::new(
            "Snippet",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let new = vec![CachedScriptlet::new(
            "Snippet",
            Some("cmd+2".to_string()),
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.shortcut_changes.len(), 1);
        assert_eq!(diff.shortcut_changes[0].name, "Snippet");
        assert_eq!(diff.shortcut_changes[0].old, Some("cmd+1".to_string()));
        assert_eq!(diff.shortcut_changes[0].new, Some("cmd+2".to_string()));
        assert!(diff.expand_changes.is_empty());
        assert!(diff.alias_changes.is_empty());
    }

    #[test]
    fn test_diff_shortcut_added() {
        let old = vec![CachedScriptlet::new(
            "Snippet",
            None, // No shortcut initially
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let new = vec![CachedScriptlet::new(
            "Snippet",
            Some("cmd+1".to_string()), // Shortcut added
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert_eq!(diff.shortcut_changes.len(), 1);
        assert_eq!(diff.shortcut_changes[0].old, None);
        assert_eq!(diff.shortcut_changes[0].new, Some("cmd+1".to_string()));
    }

    #[test]
    fn test_diff_shortcut_removed() {
        let old = vec![CachedScriptlet::new(
            "Snippet",
            Some("cmd+1".to_string()), // Has shortcut
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let new = vec![CachedScriptlet::new(
            "Snippet",
            None, // Shortcut removed
            None,
            None,
            "/path/to/file.md#snippet",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert_eq!(diff.shortcut_changes.len(), 1);
        assert_eq!(diff.shortcut_changes[0].old, Some("cmd+1".to_string()));
        assert_eq!(diff.shortcut_changes[0].new, None);
    }

    #[test]
    fn test_diff_expand_changed() {
        let old = vec![CachedScriptlet::new(
            "Snippet",
            None,
            Some("old,,".to_string()),
            None,
            "/path/to/file.md#snippet",
        )];

        let new = vec![CachedScriptlet::new(
            "Snippet",
            None,
            Some("new,,".to_string()),
            None,
            "/path/to/file.md#snippet",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.shortcut_changes.is_empty());
        assert_eq!(diff.expand_changes.len(), 1);
        assert_eq!(diff.expand_changes[0].name, "Snippet");
        assert_eq!(diff.expand_changes[0].old, Some("old,,".to_string()));
        assert_eq!(diff.expand_changes[0].new, Some("new,,".to_string()));
        assert!(diff.alias_changes.is_empty());
    }

    #[test]
    fn test_diff_alias_changed() {
        let old = vec![CachedScriptlet::new(
            "Snippet",
            None,
            None,
            Some("old".to_string()),
            "/path/to/file.md#snippet",
        )];

        let new = vec![CachedScriptlet::new(
            "Snippet",
            None,
            None,
            Some("new".to_string()),
            "/path/to/file.md#snippet",
        )];

        let diff = diff_scriptlets(&old, &new);

        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.shortcut_changes.is_empty());
        assert!(diff.expand_changes.is_empty());
        assert_eq!(diff.alias_changes.len(), 1);
        assert_eq!(diff.alias_changes[0].name, "Snippet");
        assert_eq!(diff.alias_changes[0].old, Some("old".to_string()));
        assert_eq!(diff.alias_changes[0].new, Some("new".to_string()));
    }

    #[test]
    fn test_diff_multiple_changes() {
        let old = vec![
            CachedScriptlet::new(
                "Unchanged",
                Some("cmd+1".to_string()),
                None,
                None,
                "/path/to/file.md#unchanged",
            ),
            CachedScriptlet::new(
                "Will Be Removed",
                Some("cmd+2".to_string()),
                None,
                None,
                "/path/to/file.md#will-be-removed",
            ),
            CachedScriptlet::new(
                "Shortcut Changed",
                Some("cmd+3".to_string()),
                None,
                None,
                "/path/to/file.md#shortcut-changed",
            ),
            CachedScriptlet::new(
                "Expand Changed",
                None,
                Some("old,,".to_string()),
                None,
                "/path/to/file.md#expand-changed",
            ),
            CachedScriptlet::new(
                "Alias Changed",
                None,
                None,
                Some("oldalias".to_string()),
                "/path/to/file.md#alias-changed",
            ),
        ];

        let new = vec![
            CachedScriptlet::new(
                "Unchanged",
                Some("cmd+1".to_string()),
                None,
                None,
                "/path/to/file.md#unchanged",
            ),
            CachedScriptlet::new(
                "Shortcut Changed",
                Some("cmd+9".to_string()), // Changed
                None,
                None,
                "/path/to/file.md#shortcut-changed",
            ),
            CachedScriptlet::new(
                "Expand Changed",
                None,
                Some("new,,".to_string()), // Changed
                None,
                "/path/to/file.md#expand-changed",
            ),
            CachedScriptlet::new(
                "Alias Changed",
                None,
                None,
                Some("newalias".to_string()), // Changed
                "/path/to/file.md#alias-changed",
            ),
            CachedScriptlet::new(
                "New Snippet",
                Some("cmd+0".to_string()),
                None,
                None,
                "/path/to/file.md#new-snippet",
            ),
        ];

        let diff = diff_scriptlets(&old, &new);

        // Verify added
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "New Snippet");

        // Verify removed
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "Will Be Removed");

        // Verify shortcut change
        assert_eq!(diff.shortcut_changes.len(), 1);
        assert_eq!(diff.shortcut_changes[0].name, "Shortcut Changed");
        assert_eq!(diff.shortcut_changes[0].old, Some("cmd+3".to_string()));
        assert_eq!(diff.shortcut_changes[0].new, Some("cmd+9".to_string()));

        // Verify expand change
        assert_eq!(diff.expand_changes.len(), 1);
        assert_eq!(diff.expand_changes[0].name, "Expand Changed");
        assert_eq!(diff.expand_changes[0].old, Some("old,,".to_string()));
        assert_eq!(diff.expand_changes[0].new, Some("new,,".to_string()));

        // Verify alias change
        assert_eq!(diff.alias_changes.len(), 1);
        assert_eq!(diff.alias_changes[0].name, "Alias Changed");
        assert_eq!(diff.alias_changes[0].old, Some("oldalias".to_string()));
        assert_eq!(diff.alias_changes[0].new, Some("newalias".to_string()));

        // Total changes
        assert_eq!(diff.change_count(), 5);
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_diff_empty_to_empty() {
        let diff = diff_scriptlets(&[], &[]);
        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_diff_empty_to_some() {
        let new = vec![CachedScriptlet::new(
            "New",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#new",
        )];

        let diff = diff_scriptlets(&[], &new);

        assert_eq!(diff.added.len(), 1);
        assert!(diff.removed.is_empty());
        assert_eq!(diff.change_count(), 1);
    }

    #[test]
    fn test_diff_some_to_empty() {
        let old = vec![CachedScriptlet::new(
            "Old",
            Some("cmd+1".to_string()),
            None,
            None,
            "/path/to/file.md#old",
        )];

        let diff = diff_scriptlets(&old, &[]);

        assert!(diff.added.is_empty());
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.change_count(), 1);
    }
}
