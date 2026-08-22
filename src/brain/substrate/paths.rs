//! Canonical path construction for `~/.scriptkit/brain/`.
//!
//! Every filesystem path under the brain substrate must be derived from
//! [`BrainPaths`] so no other module constructs these locations directly.

use std::path::{Component, Path, PathBuf};

use chrono::NaiveDate;

/// Rooted view of the brain directory layout.
#[derive(Debug, Clone)]
pub struct BrainPaths {
    base: PathBuf,
}

impl BrainPaths {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }

    pub fn default_kit() -> Self {
        Self::new(crate::setup::get_kit_path().join("brain"))
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn days_dir(&self) -> PathBuf {
        self.base.join("days")
    }

    pub fn day_page(&self, date: NaiveDate) -> PathBuf {
        self.days_dir().join(format!("{date}.md"))
    }

    pub fn fragments_dir(&self) -> PathBuf {
        self.base.join("fragments")
    }

    pub fn fragment_file(&self, fragment_id: &str) -> PathBuf {
        self.fragments_dir().join(format!("{fragment_id}.md"))
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.base.join("notes")
    }

    pub fn note_file(&self, slug: &str) -> PathBuf {
        self.notes_dir().join(format!("{slug}.md"))
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.base.join("trash")
    }

    /// Returns true when `path` is inside this brain tree (days, fragments,
    /// notes, or trash).
    pub fn contains(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.base) else {
            return false;
        };
        let mut components = relative.components();
        let Some(Component::Normal(directory)) = components.next() else {
            return false;
        };
        if !matches!(
            directory.to_str(),
            Some("days" | "fragments" | "notes" | "trash")
        ) {
            return false;
        }
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    }
}
