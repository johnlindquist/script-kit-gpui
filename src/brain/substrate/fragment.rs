//! Fragment writer for long captures.

use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::notes::NoteId;

use super::day::local_day_and_time;
use super::frontmatter::BrainFrontmatter;
use super::io::{atomic_write, with_brain_write_lock};
use super::paths::BrainPaths;
use super::slug::{dedupe_slug_in_dir, source_slug};
use super::words::{excerpt_words, word_count};

/// Relative path + excerpt returned for day-page fragment references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentReference {
    pub excerpt: String,
    pub relative_link: String,
}

/// Word count above which content should become a fragment file.
pub const FRAGMENT_WORD_THRESHOLD: usize = 200;

/// Target excerpt length for day-page fragment reference lines.
pub const FRAGMENT_EXCERPT_WORDS: usize = 40;

#[allow(dead_code)] // WIP: public fragment-writing API, wired once the caller lands.
pub fn write_fragment(
    paths: &BrainPaths,
    now: DateTime<Utc>,
    tz: Tz,
    source_label: &str,
    source_uri: &str,
    content: &str,
) -> Result<Option<FragmentReference>> {
    write_fragment_with_why(paths, now, tz, source_label, source_uri, content, None)
}

pub fn write_fragment_with_why(
    paths: &BrainPaths,
    now: DateTime<Utc>,
    tz: Tz,
    source_label: &str,
    source_uri: &str,
    content: &str,
    why: Option<&str>,
) -> Result<Option<FragmentReference>> {
    if word_count(content) <= FRAGMENT_WORD_THRESHOLD {
        return Ok(None);
    }

    let (date, time_hm) = local_day_and_time(now, tz);
    let stamp = time_hm.replace(':', "");
    let base_id = format!("{date}-{stamp}-{}", source_slug(source_label));

    let mut frontmatter = BrainFrontmatter::new(NoteId::new(), now, now).with_source(source_uri);
    if let Some(why) = why.filter(|value| !value.trim().is_empty()) {
        frontmatter = frontmatter.with_why(why);
    }
    let document = frontmatter.render(content.trim())?;

    // Serialize slug dedup + write under the process-wide brain write lock so
    // two concurrent fragment writers cannot pick the same slug and clobber each
    // other's file. `atomic_write` does not take the lock, so no re-entry.
    let fragment_id = with_brain_write_lock(|| -> Result<String> {
        if let Some(policy) = crate::runtime_policy::owned_evaluation() {
            policy.require_owned_path(&paths.fragments_dir())?;
        }
        let fragment_id = dedupe_slug_in_dir(&paths.fragments_dir(), &base_id);
        atomic_write(&paths.fragment_file(&fragment_id), &document)?;
        Ok(fragment_id)
    })?;

    let excerpt = excerpt_words(content, FRAGMENT_EXCERPT_WORDS);
    let relative_link = format!("../fragments/{fragment_id}.md");

    Ok(Some(FragmentReference {
        excerpt,
        relative_link,
    }))
}
