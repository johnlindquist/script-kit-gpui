# List Rendering & Search Performance Expert Bundle

## Executive Summary

This bundle covers the list virtualization, fuzzy search, and scroll performance systems in Script Kit GPUI. The application uses GPUI's `uniform_list` for virtualized rendering with a custom fuzzy search algorithm and frecency-based ranking.

### Key Problems:
1. **No event coalescing** - Rapid keyboard navigation can cause frame drops (20ms coalescing documented but not implemented)
2. **Scroll stabilization incomplete** - Only main list has jitter prevention; 6 other list components lack it
3. **Search not cached** - Full fuzzy search re-executed on every keystroke (2-5ms per search)
4. **Inconsistent item heights** - Different constants across designs (24px-64px) with some outdated comments

### Required Fixes:
1. `src/main.rs`: Implement 20ms event coalescing for arrow key navigation
2. `src/main.rs`: Extract `scroll_to_selected_if_needed()` pattern to all 7 list components
3. `src/main.rs`: Add search result memoization in `filtered_results()` / `get_filtered_results_cached()`
4. `src/perf.rs`: Connect `ScrollTimer` and `TimingGuard` to actual scroll code paths

### Files Included:
- `src/list_item.rs`: ListItem component, GroupedListState, LIST_ITEM_HEIGHT constant
- `src/perf.rs`: Performance timing utilities (KeyEventTracker, ScrollTimer, FrameTimer)
- `src/frecency.rs`: Frecency scoring with exponential decay (7-day half-life)
- `src/prompts/arg.rs`: ArgPrompt with inline filtering (no virtualization)
- `docs/LIST_RENDERING.md`: List item configuration and icon system
- `docs/perf/SCRIPT_SEARCH.md`: Search algorithm analysis and optimization recommendations
- `docs/perf/LIST_SCROLL.md`: Scroll virtualization audit and event coalescing gaps

---
[Original packx output follows]

This file is a merged representation of the filtered codebase, combined into a single document by packx.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of filtered repository contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<usage_guidelines>
- Treat this file as a snapshot of the repository's state
- Be aware that this file may contain sensitive information
</usage_guidelines>

<notes>
- Files were filtered by packx based on content and extension matching
- Total files included: 7
</notes>
</file_summary>

<directory_structure>
src/prompts/arg.rs
src/frecency.rs
src/perf.rs
src/list_item.rs
docs/perf/SCRIPT_SEARCH.md
docs/perf/LIST_SCROLL.md
docs/LIST_RENDERING.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/prompts/arg.rs">
//! ArgPrompt - Interactive argument selection with search
//!
//! Features:
//! - Searchable list of choices
//! - Keyboard navigation (up/down)
//! - Live filtering as you type
//! - Submit selected choice or cancel with Escape

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use std::sync::Arc;

use crate::designs::{get_tokens, DesignVariant};
use crate::logging;
use crate::protocol::{generate_semantic_id, Choice};
use crate::theme;

use super::SubmitCallback;

/// ArgPrompt - Interactive argument selection with search
///
/// Features:
/// - Searchable list of choices
/// - Keyboard navigation (up/down)
/// - Live filtering as you type
/// - Submit selected choice or cancel with Escape
pub struct ArgPrompt {
    pub id: String,
    pub placeholder: String,
    pub choices: Vec<Choice>,
    pub filtered_choices: Vec<usize>, // Indices into choices
    pub selected_index: usize,        // Index within filtered_choices
    pub input_text: String,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling (defaults to Default for theme-based styling)
    pub design_variant: DesignVariant,
}

impl ArgPrompt {
    pub fn new(
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::with_design(
            id,
            placeholder,
            choices,
            focus_handle,
            on_submit,
            theme,
            DesignVariant::Default,
        )
    }

    pub fn with_design(
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        design_variant: DesignVariant,
    ) -> Self {
        logging::log(
            "PROMPTS",
            &format!(
                "ArgPrompt::new with theme colors: bg={:#x}, text={:#x}, design: {:?}",
                theme.colors.background.main, theme.colors.text.primary, design_variant
            ),
        );
        let filtered_choices: Vec<usize> = (0..choices.len()).collect();
        ArgPrompt {
            id,
            placeholder,
            choices,
            filtered_choices,
            selected_index: 0,
            input_text: String::new(),
            focus_handle,
            on_submit,
            theme,
            design_variant,
        }
    }

    /// Refilter choices based on current input_text
    fn refilter(&mut self) {
        let filter_lower = self.input_text.to_lowercase();
        self.filtered_choices = self
            .choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.name.to_lowercase().contains(&filter_lower))
            .map(|(idx, _)| idx)
            .collect();
        self.selected_index = 0; // Reset selection when filtering
    }

    /// Handle character input - append to input_text and refilter
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.input_text.push(ch);
        self.refilter();
        cx.notify();
    }

    /// Handle backspace - remove last character and refilter
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.input_text.is_empty() {
            self.input_text.pop();
            self.refilter();
            cx.notify();
        }
    }

    /// Move selection up within filtered choices
    fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            cx.notify();
        }
    }

    /// Move selection down within filtered choices
    fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_choices.len().saturating_sub(1) {
            self.selected_index += 1;
            cx.notify();
        }
    }

    /// Submit the selected choice, or input_text if no choices available
    fn submit_selected(&mut self) {
        if let Some(&choice_idx) = self.filtered_choices.get(self.selected_index) {
            // Case 1: There are filtered choices - submit the selected one
            if let Some(choice) = self.choices.get(choice_idx) {
                (self.on_submit)(self.id.clone(), Some(choice.value.clone()));
            }
        } else if !self.input_text.is_empty() {
            // Case 2: No choices available but user typed something - submit input_text
            (self.on_submit)(self.id.clone(), Some(self.input_text.clone()));
        }
        // Case 3: No choices and no input - do nothing (prevent empty submissions)
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Get colors for search box based on design variant
    /// Returns: (search_box_bg, border_color, muted_text, dimmed_text, secondary_text)
    fn get_search_colors(
        &self,
        colors: &crate::designs::DesignColors,
    ) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.search_box),
                rgb(self.theme.colors.ui.border),
                rgb(self.theme.colors.text.muted),
                rgb(self.theme.colors.text.dimmed),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            (
                rgb(colors.background_secondary),
                rgb(colors.border),
                rgb(colors.text_muted),
                rgb(colors.text_dimmed),
                rgb(colors.text_secondary),
            )
        }
    }

    /// Get colors for main container based on design variant
    /// Returns: (main_bg, container_text)
    fn get_container_colors(
        &self,
        colors: &crate::designs::DesignColors,
    ) -> (gpui::Rgba, gpui::Rgba) {
        if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.main),
                rgb(self.theme.colors.text.secondary),
            )
        } else {
            (rgb(colors.background), rgb(colors.text_secondary))
        }
    }

    /// Get colors for a choice item based on selection state and design variant
    /// Returns: (bg, name_color, desc_color)
    fn get_item_colors(
        &self,
        is_selected: bool,
        colors: &crate::designs::DesignColors,
    ) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        if self.design_variant == DesignVariant::Default {
            (
                if is_selected {
                    rgb(self.theme.colors.accent.selected)
                } else {
                    rgb(self.theme.colors.background.main)
                },
                if is_selected {
                    rgb(self.theme.colors.text.primary)
                } else {
                    rgb(self.theme.colors.text.secondary)
                },
                if is_selected {
                    rgb(self.theme.colors.text.tertiary)
                } else {
                    rgb(self.theme.colors.text.muted)
                },
            )
        } else {
            (
                if is_selected {
                    rgb(colors.background_selected)
                } else {
                    rgb(colors.background)
                },
                if is_selected {
                    rgb(colors.text_on_accent)
                } else {
                    rgb(colors.text_secondary)
                },
                if is_selected {
                    rgb(colors.text_secondary)
                } else {
                    rgb(colors.text_muted)
                },
            )
        }
    }
}

impl Focusable for ArgPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ArgPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get design tokens for the current design variant
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let visual = tokens.visual();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                let key_str = event.keystroke.key.to_lowercase();

                match key_str.as_str() {
                    "up" | "arrowup" => this.move_up(cx),
                    "down" | "arrowdown" => this.move_down(cx),
                    "enter" => this.submit_selected(),
                    "escape" => this.submit_cancel(),
                    "backspace" => this.handle_backspace(cx),
                    _ => {
                        // Try to capture printable characters
                        if let Some(ref key_char) = event.keystroke.key_char {
                            if let Some(ch) = key_char.chars().next() {
                                if !ch.is_control() {
                                    this.handle_char(ch, cx);
                                }
                            }
                        }
                    }
                }
            },
        );

        // Render input field
        let input_display = if self.input_text.is_empty() {
            SharedString::from(self.placeholder.clone())
        } else {
            SharedString::from(self.input_text.clone())
        };

        // Use helper method for design/theme color extraction
        let (search_box_bg, border_color, muted_text, dimmed_text, secondary_text) =
            self.get_search_colors(&colors);

        let input_container = div()
            .id(gpui::ElementId::Name("input:filter".into()))
            .w_full()
            .px(px(spacing.item_padding_x))
            .py(px(spacing.padding_md))
            .bg(search_box_bg)
            .border_b_1()
            .border_color(border_color)
            .flex()
            .flex_row()
            .gap_2()
            .items_center()
            .child(div().text_color(muted_text).child("🔍"))
            .child(
                div()
                    .flex_1()
                    .text_color(if self.input_text.is_empty() {
                        dimmed_text
                    } else {
                        secondary_text
                    })
                    .child(input_display),
            );

        // Render choice list - fills all available vertical space
        // Uses flex_1() to grow and fill the remaining height after input container
        let mut choices_container = div()
            .id(gpui::ElementId::Name("list:choices".into()))
            .flex()
            .flex_col()
            .flex_1() // Grow to fill available space (no bottom gap)
            .min_h(px(0.)) // Allow shrinking (prevents overflow)
            .w_full()
            .overflow_y_hidden(); // Clip content at container boundary

        if self.filtered_choices.is_empty() {
            choices_container = choices_container.child(
                div()
                    .w_full()
                    .py(px(spacing.padding_xl))
                    .px(px(spacing.item_padding_x))
                    .text_color(dimmed_text)
                    .child("No choices match your filter"),
            );
        } else {
            for (idx, &choice_idx) in self.filtered_choices.iter().enumerate() {
                if let Some(choice) = self.choices.get(choice_idx) {
                    let is_selected = idx == self.selected_index;

                    // Generate semantic ID for this choice
                    // Use the choice's semantic_id if set, otherwise generate one
                    let semantic_id = choice
                        .semantic_id
                        .clone()
                        .unwrap_or_else(|| generate_semantic_id("choice", idx, &choice.value));

                    // Use helper method for item colors
                    let (bg, name_color, desc_color) = self.get_item_colors(is_selected, &colors);

                    let mut choice_item = div()
                        .id(gpui::ElementId::Name(semantic_id.clone().into()))
                        .w_full()
                        .px(px(spacing.item_padding_x))
                        .py(px(spacing.item_padding_y))
                        .bg(bg)
                        .border_b_1()
                        .border_color(border_color)
                        .rounded(px(visual.radius_sm))
                        .flex()
                        .flex_col()
                        .gap_1();

                    // Choice name (bold-ish via uppercase and text styling)
                    choice_item = choice_item.child(
                        div()
                            .text_color(name_color)
                            .text_base()
                            .child(choice.name.clone()),
                    );

                    // Choice description if present (dimmed)
                    if let Some(desc) = &choice.description {
                        choice_item = choice_item
                            .child(div().text_color(desc_color).text_sm().child(desc.clone()));
                    }

                    choices_container = choices_container.child(choice_item);
                }
            }
        }

        // Use helper method for container colors
        let (main_bg, container_text) = self.get_container_colors(&colors);

        // Generate semantic ID for the header based on prompt ID
        let header_semantic_id = format!("header:{}", self.id);

        // Main container - fills entire window height with no bottom gap
        // Layout: input_container (fixed height) + choices_container (flex_1 fills rest)
        div()
            .id(gpui::ElementId::Name("window:arg".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full() // Fill container height completely
            .min_h(px(0.)) // Allow proper flex behavior
            .bg(main_bg)
            .text_color(container_text)
            .key_context("arg_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                // Header wrapper with semantic ID
                div()
                    .id(gpui::ElementId::Name(header_semantic_id.into()))
                    .child(input_container),
            )
            .child(choices_container) // Uses flex_1 to fill all remaining space to bottom
    }
}
</file>

<file path="src/frecency.rs">
//! Frecency scoring for script usage tracking
//!
//! This module provides a frecency-based ranking system that combines
//! frequency (how often) and recency (how recently) scripts are used.
//! The scoring uses exponential decay with a configurable half-life.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, instrument, warn};

/// Half-life for frecency decay in days
/// After this many days, the score contribution decays to half
const HALF_LIFE_DAYS: f64 = 7.0;

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
        self.score = calculate_score(self.count, self.last_used);
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
/// This means:
/// - After 7 days (half_life), the score is reduced to ~50%
/// - After 14 days, the score is reduced to ~25%
/// - After 21 days, the score is reduced to ~12.5%
fn calculate_score(count: u32, last_used: u64) -> f64 {
    let now = current_timestamp();
    let seconds_since_use = now.saturating_sub(last_used);
    let days_since_use = seconds_since_use as f64 / SECONDS_PER_DAY;

    // Exponential decay: count * e^(-days / half_life)
    let decay_factor = (-days_since_use / HALF_LIFE_DAYS).exp();
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
        }
    }

    /// Create a FrecencyStore with a custom path (for testing)
    #[allow(dead_code)]
    pub fn with_path(path: PathBuf) -> Self {
        FrecencyStore {
            entries: HashMap::new(),
            file_path: path,
            dirty: false,
        }
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
        for entry in self.entries.values_mut() {
            entry.recalculate_score();
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
</file>

<file path="src/perf.rs">
#![allow(dead_code)]
//! Performance instrumentation and benchmarking utilities
//!
//! This module provides timing utilities for measuring:
//! - Key event processing rates and latency
//! - Scroll operation timing
//! - Frame timing and render performance
//!
//! Used to establish baseline metrics and identify performance bottlenecks.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tracing::{debug, warn};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Maximum number of samples to keep for rolling averages
const MAX_SAMPLES: usize = 100;

/// Threshold for "slow" key event processing (microseconds)
const SLOW_KEY_THRESHOLD_US: u128 = 16_666; // ~16ms (60fps frame budget)

/// Threshold for "slow" scroll operation (microseconds)
const SLOW_SCROLL_THRESHOLD_US: u128 = 8_000; // 8ms

// =============================================================================
// KEY EVENT TRACKING
// =============================================================================

/// Tracks key event timing and rate
pub struct KeyEventTracker {
    /// Timestamps of recent key events for rate calculation
    event_times: VecDeque<Instant>,
    /// Processing durations for recent key events
    processing_durations: VecDeque<Duration>,
    /// Last event timestamp for inter-event timing
    last_event: Option<Instant>,
    /// Count of events that exceeded threshold
    slow_event_count: usize,
    /// Total events processed
    total_events: usize,
}

impl KeyEventTracker {
    pub fn new() -> Self {
        Self {
            event_times: VecDeque::with_capacity(MAX_SAMPLES),
            processing_durations: VecDeque::with_capacity(MAX_SAMPLES),
            last_event: None,
            slow_event_count: 0,
            total_events: 0,
        }
    }

    /// Record the start of a key event, returns the start instant for timing
    pub fn start_event(&mut self) -> Instant {
        let now = Instant::now();

        // Track event time for rate calculation
        if self.event_times.len() >= MAX_SAMPLES {
            self.event_times.pop_front();
        }
        self.event_times.push_back(now);

        self.total_events += 1;
        now
    }

    /// Record the end of key event processing
    pub fn end_event(&mut self, start: Instant) {
        let duration = start.elapsed();

        // Track processing duration
        if self.processing_durations.len() >= MAX_SAMPLES {
            self.processing_durations.pop_front();
        }
        self.processing_durations.push_back(duration);

        // Check if this was a slow event
        if duration.as_micros() > SLOW_KEY_THRESHOLD_US {
            self.slow_event_count += 1;
        }

        self.last_event = Some(start);
    }

    /// Calculate events per second based on recent events
    pub fn events_per_second(&self) -> f64 {
        if self.event_times.len() < 2 {
            return 0.0;
        }

        let first = self.event_times.front().unwrap();
        let last = self.event_times.back().unwrap();
        let elapsed = last.duration_since(*first);

        if elapsed.as_secs_f64() < 0.001 {
            return 0.0;
        }

        (self.event_times.len() - 1) as f64 / elapsed.as_secs_f64()
    }

    /// Get average processing time in microseconds
    pub fn avg_processing_time_us(&self) -> u128 {
        if self.processing_durations.is_empty() {
            return 0;
        }

        let total: Duration = self.processing_durations.iter().sum();
        total.as_micros() / self.processing_durations.len() as u128
    }

    /// Get the percentage of events that were slow
    pub fn slow_event_percentage(&self) -> f64 {
        if self.total_events == 0 {
            return 0.0;
        }
        (self.slow_event_count as f64 / self.total_events as f64) * 100.0
    }

    /// Time since last event in milliseconds (for detecting bursts)
    pub fn time_since_last_event_ms(&self) -> Option<f64> {
        self.last_event
            .map(|last| last.elapsed().as_secs_f64() * 1000.0)
    }

    /// Log current statistics
    pub fn log_stats(&self) {
        debug!(
            category = "KEY_PERF",
            rate_per_sec = self.events_per_second(),
            avg_ms = self.avg_processing_time_us() as f64 / 1000.0,
            slow_percent = self.slow_event_percentage(),
            total_events = self.total_events,
            "Key event statistics"
        );
    }
}

impl Default for KeyEventTracker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// SCROLL TIMING
// =============================================================================

/// Tracks scroll operation timing
pub struct ScrollTimer {
    /// Start instant for current scroll operation
    start: Option<Instant>,
    /// Recent scroll operation durations
    durations: VecDeque<Duration>,
    /// Count of slow scroll operations
    slow_count: usize,
    /// Total scroll operations
    total_ops: usize,
}

impl ScrollTimer {
    pub fn new() -> Self {
        Self {
            start: None,
            durations: VecDeque::with_capacity(MAX_SAMPLES),
            slow_count: 0,
            total_ops: 0,
        }
    }

    /// Start timing a scroll operation
    pub fn start(&mut self) -> Instant {
        let now = Instant::now();
        self.start = Some(now);
        now
    }

    /// End timing and record the duration
    pub fn end(&mut self) -> Duration {
        let duration = self.start.map(|s| s.elapsed()).unwrap_or(Duration::ZERO);

        if self.durations.len() >= MAX_SAMPLES {
            self.durations.pop_front();
        }
        self.durations.push_back(duration);

        if duration.as_micros() > SLOW_SCROLL_THRESHOLD_US {
            self.slow_count += 1;
        }

        self.total_ops += 1;
        self.start = None;
        duration
    }

    /// Get average scroll time in microseconds
    pub fn avg_time_us(&self) -> u128 {
        if self.durations.is_empty() {
            return 0;
        }
        let total: Duration = self.durations.iter().sum();
        total.as_micros() / self.durations.len() as u128
    }

    /// Get max scroll time in microseconds from recent samples
    pub fn max_time_us(&self) -> u128 {
        self.durations
            .iter()
            .map(|d| d.as_micros())
            .max()
            .unwrap_or(0)
    }

    /// Log scroll timing stats
    pub fn log_stats(&self) {
        debug!(
            category = "SCROLL_TIMING",
            avg_ms = self.avg_time_us() as f64 / 1000.0,
            max_ms = self.max_time_us() as f64 / 1000.0,
            slow_count = self.slow_count,
            total_ops = self.total_ops,
            "Scroll timing statistics"
        );
    }
}

impl Default for ScrollTimer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// FRAME TIMING
// =============================================================================

/// Tracks frame timing for render performance
pub struct FrameTimer {
    /// Last frame timestamp
    last_frame: Option<Instant>,
    /// Recent frame durations
    frame_times: VecDeque<Duration>,
    /// Count of dropped/slow frames
    dropped_frames: usize,
    /// Total frames tracked
    total_frames: usize,
}

impl FrameTimer {
    pub fn new() -> Self {
        Self {
            last_frame: None,
            frame_times: VecDeque::with_capacity(MAX_SAMPLES),
            dropped_frames: 0,
            total_frames: 0,
        }
    }

    /// Mark a frame and calculate time since last frame
    pub fn mark_frame(&mut self) -> Option<Duration> {
        let now = Instant::now();
        let duration = self.last_frame.map(|last| now.duration_since(last));

        if let Some(d) = duration {
            if self.frame_times.len() >= MAX_SAMPLES {
                self.frame_times.pop_front();
            }
            self.frame_times.push_back(d);

            // Consider frame dropped if > 32ms (less than 30fps)
            if d.as_millis() > 32 {
                self.dropped_frames += 1;
            }
        }

        self.last_frame = Some(now);
        self.total_frames += 1;
        duration
    }

    /// Get average FPS from recent samples
    pub fn avg_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let avg_frame_time: Duration =
            self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;

        if avg_frame_time.as_secs_f64() < 0.001 {
            return 0.0;
        }

        1.0 / avg_frame_time.as_secs_f64()
    }

    /// Get dropped frame percentage
    pub fn dropped_percentage(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }
        (self.dropped_frames as f64 / self.total_frames as f64) * 100.0
    }

    /// Log frame timing stats
    pub fn log_stats(&self) {
        debug!(
            category = "FRAME_PERF",
            fps = self.avg_fps(),
            dropped_percent = self.dropped_percentage(),
            total_frames = self.total_frames,
            "Frame timing statistics"
        );
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GLOBAL PERF TRACKER
// =============================================================================

/// Global performance tracker instance
static PERF_TRACKER: OnceLock<Mutex<PerfTracker>> = OnceLock::new();

/// Combined performance tracker for all metrics
pub struct PerfTracker {
    pub key_events: KeyEventTracker,
    pub scroll: ScrollTimer,
    pub frames: FrameTimer,
}

impl PerfTracker {
    pub fn new() -> Self {
        Self {
            key_events: KeyEventTracker::new(),
            scroll: ScrollTimer::new(),
            frames: FrameTimer::new(),
        }
    }

    /// Log all performance stats
    pub fn log_all_stats(&self) {
        self.key_events.log_stats();
        self.scroll.log_stats();
        self.frames.log_stats();
    }
}

impl Default for PerfTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global performance tracker
pub fn get_perf_tracker() -> &'static Mutex<PerfTracker> {
    PERF_TRACKER.get_or_init(|| Mutex::new(PerfTracker::new()))
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Start timing a key event, returns start instant
pub fn start_key_event() -> Instant {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.key_events.start_event()
    } else {
        Instant::now()
    }
}

/// End timing a key event
pub fn end_key_event(start: Instant) {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.key_events.end_event(start);
    }
}

/// Start timing a scroll operation
pub fn start_scroll() -> Instant {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.scroll.start()
    } else {
        Instant::now()
    }
}

/// End timing a scroll operation
pub fn end_scroll() -> Duration {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.scroll.end()
    } else {
        Duration::ZERO
    }
}

/// Mark a frame for FPS tracking
pub fn mark_frame() -> Option<Duration> {
    if let Ok(mut tracker) = get_perf_tracker().lock() {
        tracker.frames.mark_frame()
    } else {
        None
    }
}

/// Log current key event rate (useful for detecting fast repeat)
pub fn log_key_rate() {
    if let Ok(tracker) = get_perf_tracker().lock() {
        let rate = tracker.key_events.events_per_second();
        if rate > 20.0 {
            warn!(
                category = "KEY_PERF",
                rate_per_sec = rate,
                "High key event rate detected"
            );
        }
    }
}

/// Log all performance stats
pub fn log_perf_summary() {
    if let Ok(tracker) = get_perf_tracker().lock() {
        tracker.log_all_stats();
    }
}

// =============================================================================
// TIMING GUARD (RAII pattern for timing)
// =============================================================================

/// RAII guard for timing operations - logs when dropped
pub struct TimingGuard {
    operation: &'static str,
    start: Instant,
    threshold_us: u128,
}

impl TimingGuard {
    /// Create a new timing guard
    pub fn new(operation: &'static str, threshold_us: u128) -> Self {
        Self {
            operation,
            start: Instant::now(),
            threshold_us,
        }
    }

    /// Create a timing guard for key events
    pub fn key_event() -> Self {
        Self::new("key_event", SLOW_KEY_THRESHOLD_US)
    }

    /// Create a timing guard for scroll operations
    pub fn scroll() -> Self {
        Self::new("scroll", SLOW_SCROLL_THRESHOLD_US)
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        let duration_us = duration.as_micros();

        if duration_us > self.threshold_us {
            warn!(
                category = "PERF_SLOW",
                operation = self.operation,
                duration_ms = duration_us as f64 / 1000.0,
                threshold_ms = self.threshold_us as f64 / 1000.0,
                "Slow operation detected"
            );
        }
    }
}
</file>

<file path="src/list_item.rs">
//! Shared ListItem component for script list and arg prompt choice list
//!
//! This module provides a reusable, theme-aware list item component that can be
//! used in both the main script list and arg prompt choice lists.

#![allow(dead_code)]

use crate::designs::icon_variations::{icon_name_from_str, IconName};
use crate::logging;
use gpui::*;
use std::sync::Arc;

/// Icon type for list items - supports emoji strings, SVG icons, and pre-decoded images
#[derive(Clone)]
pub enum IconKind {
    /// Text/emoji icon (e.g., "📜", "⚡")
    Emoji(String),
    /// Pre-decoded render image (for app icons) - MUST be pre-decoded, not raw PNG bytes
    Image(Arc<RenderImage>),
    /// SVG icon by name (e.g., "File", "Terminal", "Code")
    /// Maps to IconName from designs::icon_variations
    Svg(String),
}

/// Fixed height for list items (same as main script list)
/// Height must accommodate: name (18px line-height) + description (14px line-height) + padding (12px)
/// Total content height: 18 + 14 + 12 = 44px minimum, using 48px for comfortable spacing
pub const LIST_ITEM_HEIGHT: f32 = 48.0;

/// Fixed height for section headers (RECENT, MAIN, etc.)
/// Total height includes: pt(8px) + text (~8px via text_xs) + pb(4px) = ~20px content
/// Using 24px for comfortable spacing while maintaining visual compactness.
///
/// ## Performance Note (uniform_list vs list)
/// The main menu uses GPUI's `list()` component (not `uniform_list`) to support variable heights:
/// - Section headers: 24px (SECTION_HEADER_HEIGHT)
/// - Regular items: 48px (LIST_ITEM_HEIGHT)
///
/// Performance comparison:
/// - `uniform_list`: O(1) scroll position calculation (all items same height)
/// - `list()`: O(log n) scroll position via SumTree (supports variable heights)
///
/// For typical menu sizes (< 1000 items), the performance difference is negligible.
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;

/// Enum for grouped list items - supports both regular items and section headers
///
/// Used with GPUI's `list()` component when rendering grouped results (e.g., frecency with RECENT/MAIN sections).
/// The usize in Item variant is the index into the flat results array.
#[derive(Clone, Debug)]
pub enum GroupedListItem {
    /// A section header (e.g., "RECENT", "MAIN")
    SectionHeader(String),
    /// A regular list item - usize is the index in the flat results array
    Item(usize),
}

/// Pre-computed grouped list state for efficient navigation
///
/// This struct caches header positions and total counts to avoid expensive
/// recalculation on every keypress. Build it once when the list data changes,
/// then reuse for navigation.
///
/// ## Performance
/// - `is_header()`: O(1) lookup via HashSet
/// - `next_selectable()` / `prev_selectable()`: O(k) where k is consecutive headers
/// - Memory: O(h) where h is number of headers (typically < 10)
///
/// ## Usage Pattern
/// ```ignore
/// // Build once when data changes
/// let grouped = GroupedListState::from_groups(&[
///     ("Today", 5),      // 5 items in Today group
///     ("Yesterday", 3),  // 3 items in Yesterday group
/// ]);
///
/// // Use for navigation (fast, no allocation)
/// let next = grouped.next_selectable(current_index);
/// let prev = grouped.prev_selectable(current_index);
/// let is_hdr = grouped.is_header(index);
/// ```
#[derive(Clone, Debug)]
pub struct GroupedListState {
    /// Set of indices that are headers (for O(1) lookup)
    header_indices: std::collections::HashSet<usize>,
    /// Total number of visual items (headers + entries)
    pub total_items: usize,
    /// Index of first selectable item (skips leading header)
    pub first_selectable: usize,
}

impl GroupedListState {
    /// Create from a list of (group_name, item_count) pairs
    ///
    /// Each group gets a header at the start, followed by its items.
    /// Empty groups are skipped (no header for empty groups).
    pub fn from_groups(groups: &[(&str, usize)]) -> Self {
        let mut header_indices = std::collections::HashSet::new();
        let mut idx = 0;

        for (_, count) in groups {
            if *count > 0 {
                header_indices.insert(idx); // Header position
                idx += 1 + count; // Header + items
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: idx,
            first_selectable,
        }
    }

    /// Create from pre-built GroupedListItem vec (when you already have the items)
    pub fn from_items(items: &[GroupedListItem]) -> Self {
        let mut header_indices = std::collections::HashSet::new();

        for (idx, item) in items.iter().enumerate() {
            if matches!(item, GroupedListItem::SectionHeader(_)) {
                header_indices.insert(idx);
            }
        }

        let first_selectable = if header_indices.contains(&0) { 1 } else { 0 };

        Self {
            header_indices,
            total_items: items.len(),
            first_selectable,
        }
    }

    /// Create an empty state (no headers, for flat lists)
    pub fn flat(item_count: usize) -> Self {
        Self {
            header_indices: std::collections::HashSet::new(),
            total_items: item_count,
            first_selectable: 0,
        }
    }

    /// Check if an index is a header (O(1))
    #[inline]
    pub fn is_header(&self, index: usize) -> bool {
        self.header_indices.contains(&index)
    }

    /// Get next selectable index (skips headers), or None if at end
    pub fn next_selectable(&self, current: usize) -> Option<usize> {
        let mut next = current + 1;
        while next < self.total_items && self.is_header(next) {
            next += 1;
        }
        if next < self.total_items {
            Some(next)
        } else {
            None
        }
    }

    /// Get previous selectable index (skips headers), or None if at start
    pub fn prev_selectable(&self, current: usize) -> Option<usize> {
        if current == 0 {
            return None;
        }
        let mut prev = current - 1;
        while prev > 0 && self.is_header(prev) {
            prev -= 1;
        }
        if !self.is_header(prev) {
            Some(prev)
        } else {
            None
        }
    }

    /// Get number of headers
    pub fn header_count(&self) -> usize {
        self.header_indices.len()
    }
}

/// Pre-computed colors for ListItem rendering
///
/// This struct holds the primitive color values needed for list item rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy)]
pub struct ListItemColors {
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_dimmed: u32,
    pub accent_selected: u32,
    pub accent_selected_subtle: u32,
    pub background: u32,
    pub background_selected: u32,
}

impl ListItemColors {
    /// Create from theme reference
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            text_primary: theme.colors.text.primary,
            text_secondary: theme.colors.text.secondary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            accent_selected: theme.colors.accent.selected,
            accent_selected_subtle: theme.colors.accent.selected_subtle,
            background: theme.colors.background.main,
            background_selected: theme.colors.accent.selected_subtle,
        }
    }

    /// Create from design colors for GLOBAL theming support
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        Self {
            text_primary: colors.text_primary,
            text_secondary: colors.text_secondary,
            text_muted: colors.text_muted,
            text_dimmed: colors.text_dimmed,
            accent_selected: colors.accent,
            accent_selected_subtle: colors.background_selected,
            background: colors.background,
            background_selected: colors.background_selected,
        }
    }
}

/// Fixed height for list items (same as main script list)
pub const ACCENT_BAR_WIDTH: f32 = 3.0;

/// A reusable list item component for displaying selectable items
///
/// Supports:
/// - Name (required)
/// - Description (optional, shown below name)
/// - Icon (optional, emoji or PNG image displayed left of name)
/// - Shortcut badge (optional, right-aligned)
/// - Selection state with themed colors (full focus styling)
/// - Hover state with subtle visual feedback (separate from selection)
/// - Hover callback for mouse interaction (optional)
/// - Semantic ID for AI-driven targeting (optional)
#[derive(IntoElement)]
pub struct ListItem {
    name: SharedString,
    description: Option<String>,
    shortcut: Option<String>,
    icon: Option<IconKind>,
    selected: bool,
    /// Whether this item is being hovered (subtle visual feedback, separate from selected)
    hovered: bool,
    colors: ListItemColors,
    /// Index of this item in the list (needed for hover callback)
    index: Option<usize>,
    /// Optional callback triggered when mouse enters/leaves this item
    on_hover: Option<Box<dyn Fn(usize, bool) + 'static>>,
    /// Semantic ID for AI-driven UX targeting. Format: {type}:{index}:{value}
    semantic_id: Option<String>,
    /// Show left accent bar when selected (3px colored bar on left edge)
    show_accent_bar: bool,
}

impl ListItem {
    /// Create a new list item with the given name and pre-computed colors
    pub fn new(name: impl Into<SharedString>, colors: ListItemColors) -> Self {
        Self {
            name: name.into(),
            description: None,
            shortcut: None,
            icon: None,
            selected: false,
            hovered: false,
            colors,
            index: None,
            on_hover: None,
            semantic_id: None,
            show_accent_bar: false,
        }
    }

    /// Enable the left accent bar (3px colored bar shown when selected)
    pub fn with_accent_bar(mut self, show: bool) -> Self {
        self.show_accent_bar = show;
        self
    }

    /// Set the index of this item in the list (required for hover callback to work)
    pub fn index(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    /// Set a callback to be triggered when mouse enters or leaves this item.
    /// The callback receives (index, is_hovered) where is_hovered is true when entering.
    pub fn on_hover(mut self, callback: Box<dyn Fn(usize, bool) + 'static>) -> Self {
        self.on_hover = Some(callback);
        self
    }

    /// Set the semantic ID for AI-driven UX targeting.
    /// Format: {type}:{index}:{value} (e.g., "choice:0:apple")
    pub fn semantic_id(mut self, id: impl Into<String>) -> Self {
        self.semantic_id = Some(id.into());
        self
    }

    /// Set an optional semantic ID (convenience for Option<String>)
    pub fn semantic_id_opt(mut self, id: Option<String>) -> Self {
        self.semantic_id = id;
        self
    }

    /// Set the description text (shown below the name)
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    /// Set an optional description (convenience for Option<String>)
    pub fn description_opt(mut self, d: Option<String>) -> Self {
        self.description = d;
        self
    }

    /// Set the shortcut badge text (shown right-aligned)
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Set an optional shortcut (convenience for Option<String>)
    pub fn shortcut_opt(mut self, s: Option<String>) -> Self {
        self.shortcut = s;
        self
    }

    /// Set the icon (emoji) to display on the left side
    pub fn icon(mut self, i: impl Into<String>) -> Self {
        self.icon = Some(IconKind::Emoji(i.into()));
        self
    }

    /// Set an optional emoji icon (convenience for Option<String>)
    pub fn icon_opt(mut self, i: Option<String>) -> Self {
        self.icon = i.map(IconKind::Emoji);
        self
    }

    /// Set a pre-decoded RenderImage icon
    pub fn icon_image(mut self, image: Arc<RenderImage>) -> Self {
        self.icon = Some(IconKind::Image(image));
        self
    }

    /// Set icon from IconKind enum (for mixed icon types)
    pub fn icon_kind(mut self, kind: IconKind) -> Self {
        self.icon = Some(kind);
        self
    }

    /// Set whether this item is selected
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Set whether this item is hovered
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }
}
</file>

<file path="docs/perf/SCRIPT_SEARCH.md">
# Script Loading & Fuzzy Search Performance Audit

## Executive Summary

This document analyzes the script loading and search performance of Script Kit GPUI. The system handles script discovery, scriptlet parsing, fuzzy search, and frecency-based ranking across ~4,000 lines of code in `src/scripts.rs`.

**Key Findings:**
- Script loading is synchronous and blocking (I/O-bound on file reads)
- Fuzzy search algorithm is O(n * m) where n=items, m=query length
- Frecency uses exponential decay with 7-day half-life, recalculated on every load
- No search result caching; full re-search on every keystroke
- File watchers use 500ms debounce but still trigger full reloads

---

## 1. Script Loading Timeline Analysis

### 1.1 Script Discovery (`read_scripts()`)

**Location:** `src/scripts.rs:556-628`

```
Timeline:
1. Expand HOME variable           ~1μs
2. Check directory exists         ~10μs
3. Read directory (fs::read_dir)  ~100-500μs (depends on file count)
4. For each .ts/.js file:
   a. Check file metadata         ~10μs
   b. Read file content           ~50-200μs (I/O bound)
   c. Parse first 20 lines        ~5μs
   d. Extract metadata (Name, Description, Icon)  ~2μs
5. Sort by name                   O(n log n) ~1μs per comparison

Total: ~50-200μs per script + ~100-500μs base overhead
```

**Complexity:** O(n) for n scripts, but dominated by I/O latency

**Issues:**
1. **Synchronous file reads** - Blocks the main thread
2. **Full file read** - Reads entire file even though only first 20 lines are needed
3. **No incremental updates** - Full reload on any script change

### 1.2 Metadata Extraction Performance

**Location:** `src/scripts.rs:145-212`

```rust
pub fn extract_script_metadata(content: &str) -> ScriptMetadata {
    // Iterates first 20 lines only - good!
    for line in content.lines().take(20) {
        if let Some((key, value)) = parse_metadata_line(line) {
            // O(1) pattern matching per line
        }
    }
}
```

**Complexity:** O(20) = O(1) per file - well optimized

---

## 2. Fuzzy Search Algorithm Analysis

### 2.1 Core Algorithm (`is_fuzzy_match()`)

**Location:** `src/scripts.rs:631-641`

```rust
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
```

**Complexity:** O(h) where h = haystack length

### 2.2 Search Functions Complexity

| Function | Complexity | Notes |
|----------|------------|-------|
| `fuzzy_search_scripts()` | O(n * h) | n=scripts, h=avg name length |
| `fuzzy_search_scriptlets()` | O(n * (h + d + c)) | + description + code |
| `fuzzy_search_builtins()` | O(n * (h + d + k)) | + keywords |
| `fuzzy_search_apps()` | O(n * (h + b + p)) | + bundle_id + path |
| `fuzzy_search_windows()` | O(n * (a + t)) | app + title |
| `fuzzy_search_unified_with_windows()` | O(total) | Sum of all above |

### 2.3 Scoring Breakdown

| Match Type | Score | Priority |
|------------|-------|----------|
| Name match at start | +100 | Highest |
| Name match elsewhere | +75 | High |
| Fuzzy match in name | +50 | Medium |
| Description match | +25 | Lower |
| Keyword match (builtins) | +75 | High |
| Fuzzy keyword match | +30 | Medium |
| Path match | +10 | Lowest |
| Code content match (scriptlets) | +5 | Lowest |

---

## 3. Frecency Computation Analysis

### 3.1 Score Calculation

**Location:** `src/frecency.rs:64-79`

```rust
fn calculate_score(count: u32, last_used: u64) -> f64 {
    let now = current_timestamp();
    let seconds_since_use = now.saturating_sub(last_used);
    let days_since_use = seconds_since_use as f64 / SECONDS_PER_DAY;
    
    // Exponential decay: count * e^(-days / half_life)
    let decay_factor = (-days_since_use / HALF_LIFE_DAYS).exp();
    count as f64 * decay_factor
}
```

**Decay Profile (7-day half-life):**
| Days Since Use | Score Multiplier |
|----------------|------------------|
| 0 | 1.00 (100%) |
| 7 | 0.37 (37%) |
| 14 | 0.14 (14%) |
| 21 | 0.05 (5%) |
| 30 | 0.01 (1%) |

---

## 4. Search Result Caching Analysis

### 4.1 Current State: Partial Caching

The system has a `filter_cache_key` pattern in `main.rs`:

```rust
// main.rs fields
cached_filtered_results: Vec<scripts::SearchResult>,
filter_cache_key: String,

// Cache check
fn get_filtered_results_cached(&mut self) -> &Vec<scripts::SearchResult> {
    if self.filter_text != self.filter_cache_key {
        // Cache miss - recompute
        self.cached_filtered_results = scripts::fuzzy_search_unified_all(...);
        self.filter_cache_key = self.filter_text.clone();
    }
    &self.cached_filtered_results
}
```

**Issues:**
- Cache only works for exact query match
- No prefix-based optimization (typing more characters could filter cached results)
- Cache invalidated on any filter_text change

---

## 5. Recommendations

### 5.1 High Priority (Significant Impact)

1. **Implement Incremental Search Filtering**
   - When user types more characters, filter previous results
   - Only do full search when query shrinks or is cleared
   - **Expected improvement:** 50-80% reduction in search time

2. **Add Prefix-Based Cache**
   ```rust
   struct SearchCache {
       query: String,
       results: Vec<SearchResult>,
   }
   
   // If new_query.starts_with(cached.query), filter cached results
   ```

3. **Implement Event Coalescing**
   - 20ms window for rapid keystrokes
   - Only perform search after window expires
   - **Expected improvement:** Prevent frame drops during fast typing

### 5.2 Medium Priority

4. **Lazy Metadata Loading**
   - Load script paths first (fast)
   - Load metadata on-demand or in background

5. **Stream First 20 Lines**
   - Use BufReader instead of reading full file

### 5.3 Low Priority

6. **Pre-Build Search Index on Startup**
7. **Async Script Loading**

---

## 6. Performance Metrics Summary

| Operation | Current | Target | Priority |
|-----------|---------|--------|----------|
| Initial script load | 100-500ms | <50ms | High |
| Script reload (1 file change) | 100-500ms | <10ms | High |
| Fuzzy search (per keystroke) | 2-5ms | <1ms | Medium |
| Frecency recalculation | 2-10μs/entry | 0μs (cached) | Low |
</file>

<file path="docs/perf/LIST_SCROLL.md">
# List Virtualization & Scroll Performance Audit

## Executive Summary

This document audits the scroll and virtualization implementation in Script Kit GPUI. The codebase uses GPUI's `uniform_list` for virtualized rendering across multiple list components. While the core patterns are sound, there are opportunities for optimization around scroll handle overhead, event coalescing, and item height consistency.

---

## 1. Virtualization Analysis

### 1.1 uniform_list Usage

The application uses `uniform_list` for virtualized list rendering across 7 distinct list contexts:

| Component | Handle Field | Item Height | Location |
|-----------|--------------|-------------|----------|
| Script List | `list_scroll_handle` | 48px (LIST_ITEM_HEIGHT) | main.rs |
| Arg Prompt Choices | `arg_list_scroll_handle` | 48px | main.rs |
| Clipboard History | `clipboard_list_scroll_handle` | 48px | main.rs |
| Window Switcher | `window_list_scroll_handle` | 48px | main.rs |
| Design Gallery | `design_gallery_scroll_handle` | 48px | main.rs |
| Actions Dialog | `scroll_handle` (in ActionsDialog) | 42px (ACTION_ITEM_HEIGHT) | actions.rs |
| Editor Lines | `scroll_handle` (in EditorPrompt) | dynamic | editor.rs |

### 1.2 Item Height Constants

| Constant | Value | Used By |
|----------|-------|---------|
| `LIST_ITEM_HEIGHT` | 48px | Default design, most components |
| `SECTION_HEADER_HEIGHT` | 24px | Section headers in grouped lists |
| `ACTION_ITEM_HEIGHT` | 42px | Actions popup |

---

## 2. Scroll Handle Assessment

### 2.1 Handle Inventory

The application maintains **5 scroll handles** in the main `ScriptListApp` struct plus 2 in sub-components:

```rust
// main.rs
list_scroll_handle: UniformListScrollHandle,
arg_list_scroll_handle: UniformListScrollHandle,
clipboard_list_scroll_handle: UniformListScrollHandle,
window_list_scroll_handle: UniformListScrollHandle,
design_gallery_scroll_handle: UniformListScrollHandle,

// actions.rs
pub scroll_handle: UniformListScrollHandle,

// editor.rs
scroll_handle: UniformListScrollHandle,
```

### 2.2 Overhead Analysis

**Memory Overhead per Handle:**
- `UniformListScrollHandle` is a lightweight wrapper around scroll state
- Estimated: ~24-48 bytes per handle (minimal)

**Verdict:** Handle count is not a performance concern

---

## 3. Scroll Stabilization

### 3.1 last_scrolled_index Pattern

The codebase implements scroll stabilization to prevent jitter from redundant `scroll_to_item` calls:

```rust
// main.rs
fn scroll_to_selected_if_needed(&mut self, _reason: &str) {
    let target = self.selected_index;
    
    // Check if we've already scrolled to this index
    if self.last_scrolled_index == Some(target) {
        return;
    }
    
    self.list_scroll_handle.scroll_to_item(target, ScrollStrategy::Nearest);
    self.last_scrolled_index = Some(target);
}
```

**Coverage:**
- Main script list: Yes (via `scroll_to_selected_if_needed`)
- Arg prompt: No (direct `scroll_to_item` calls)
- Clipboard history: No
- Window switcher: No
- Design gallery: No
- Actions dialog: No

**Total: 15 call sites, 1 with stabilization (6.7%)**

---

## 4. Event Coalescing

### 4.1 Documented 20ms Window

AGENTS.md documents a 20ms coalescing window for keyboard events:
> Implement a 20ms coalescing window for rapid key events

**However, this is NOT implemented in the current codebase.**

### 4.2 Current Key Handling

Arrow key events are processed synchronously without debouncing:
```rust
// main.rs keyboard handler pattern
match key.as_str() {
    "up" | "arrowup" => this.move_selection_up(cx),
    "down" | "arrowdown" => this.move_selection_down(cx),
    // Immediate processing, no coalescing
}
```

**Impact:** Rapid key repeat could cause:
- Multiple `scroll_to_item` calls in quick succession
- Unnecessary re-renders via `cx.notify()`
- Frame drops if render exceeds 16.67ms budget

---

## 5. Performance Thresholds

### 5.1 Defined Thresholds (perf.rs)

| Metric | Threshold | Purpose |
|--------|-----------|---------|
| SLOW_KEY_THRESHOLD_US | 16,666 us (16.67ms) | 60fps frame budget |
| SLOW_SCROLL_THRESHOLD_US | 8,000 us (8ms) | Scroll operation budget |
| MAX_SAMPLES | 100 | Rolling average window |

### 5.2 Performance Instrumentation

The `perf.rs` module provides:
- `KeyEventTracker`: Track key event rates and processing time
- `ScrollTimer`: Track scroll operation duration
- `FrameTimer`: Track render FPS and dropped frames
- `TimingGuard`: RAII guard for timing operations

**Issue:** These utilities exist but are NOT connected to the actual scroll/key handling code paths.

---

## 6. Identified Issues

### 6.1 Critical

1. **No event coalescing implemented** despite AGENTS.md documentation
   - Risk: Frame drops during rapid scrolling
   - Impact: High on slow systems or long lists

2. **Scroll stabilization only on main list**
   - 6 other list components lack jitter prevention
   - Risk: Visual jitter on arg/clipboard/window lists

### 6.2 Moderate

3. **Performance instrumentation not connected**
   - `ScrollTimer`, `TimingGuard` exist but unused
   - Risk: Blind spots in performance monitoring

---

## 7. Optimization Recommendations

### 7.1 Implement Event Coalescing (Priority: High)

Add 20ms coalescing window for keyboard navigation:

```rust
// Suggested addition to ScriptListApp
struct ScrollCoalescer {
    pending_direction: Option<ScrollDirection>,
    pending_delta: i32,
    last_event: Instant,
}

impl ScrollCoalescer {
    const WINDOW_MS: u64 = 20;
    
    fn process(&mut self, direction: ScrollDirection) -> Option<i32> {
        let now = Instant::now();
        if now.duration_since(self.last_event) < Duration::from_millis(Self::WINDOW_MS)
           && self.pending_direction == Some(direction) {
            self.pending_delta += 1;
            None // Coalesce
        } else {
            let result = self.pending_delta.take();
            self.pending_direction = Some(direction);
            self.pending_delta = 1;
            self.last_event = now;
            result
        }
    }
}
```

### 7.2 Extract Scroll Stabilization Helper (Priority: High)

```rust
// New utility function
fn scroll_if_needed(
    handle: &UniformListScrollHandle,
    last_index: &mut Option<usize>,
    target: usize,
) {
    if *last_index != Some(target) {
        handle.scroll_to_item(target, ScrollStrategy::Nearest);
        *last_index = Some(target);
    }
}
```

### 7.3 Connect Performance Instrumentation (Priority: Medium)

Wrap scroll operations with timing:
```rust
fn scroll_to_selected_if_needed(&mut self, reason: &str) {
    let _guard = perf::TimingGuard::scroll();
    // existing logic
}
```

---

## 8. Performance Metrics Summary

### Current State

| Metric | Target | Actual | Status |
|--------|--------|--------|--------|
| P95 Key Latency | < 50ms | Unknown | Not measured |
| Single Key Event | < 16.67ms | Unknown | Not measured |
| Scroll Operation | < 8ms | Unknown | Not measured |
| Event Coalescing | 20ms window | Not implemented | Missing |
| Scroll Jitter Prevention | All lists | Main list only | Partial |
</file>

<file path="docs/LIST_RENDERING.md">
# List Rendering Guide

This document explains how to customize the appearance of list items and section headers in Script Kit GPUI.

## Key Files

| File | Purpose |
|------|---------|
| `src/list_item.rs` | `ListItem` component, `IconKind` enum, `render_section_header()` |
| `src/designs/mod.rs` | `render_design_item()` - maps SearchResult to ListItem |
| `src/designs/icon_variations.rs` | `IconName` enum, SVG icon paths, `icon_name_from_str()` |
| `src/main.rs` | Main list rendering in `uniform_list` closure |

## List Item Configuration

### Constants

```rust
// src/list_item.rs
pub const LIST_ITEM_HEIGHT: f32 = 48.0;      // Height of each list item
pub const SECTION_HEADER_HEIGHT: f32 = 24.0; // Height of section headers
pub const ACCENT_BAR_WIDTH: f32 = 3.0;       // Left accent bar when selected
```

### ListItem Builder Methods

```rust
ListItem::new(name, colors)
    .index(0)                           // Item index for hover handling
    .icon("📜")                         // Emoji icon
    .icon_kind(IconKind::Svg("Code"))   // SVG icon by name
    .icon_image(arc_render_image)       // Pre-decoded image (app icons)
    .description("Optional description")
    .shortcut("⌘K")                     // Right-aligned shortcut badge
    .selected(true)                     // Selection state
    .with_accent_bar(true)              // Show 3px left accent when selected
    .semantic_id("choice:0:my-item")    // AI-targeting ID
```

### Icon Types

```rust
pub enum IconKind {
    Emoji(String),           // Text/emoji: "📜", "⚡"
    Image(Arc<RenderImage>), // Pre-decoded PNG (app icons)
    Svg(String),             // SVG by name: "Code", "Terminal", "File"
}
```

## Section Headers

Section headers are rendered at 24px height for visual compactness:

```rust
// GroupedListItem::SectionHeader rendering
div()
    .h(px(SECTION_HEADER_HEIGHT))
    .child(render_section_header(label, theme_colors))
```

## Performance Considerations

- Use `uniform_list` for O(1) scroll calculation with fixed heights
- Use `list()` for variable heights (O(log n) via SumTree)
- Pre-compute `ListItemColors` once per render cycle
- Use `GroupedListState` for O(1) header lookup during navigation
</file>

</files>

---
## Implementation Guide

### Step 1: Implement 20ms Event Coalescing

```rust
// File: src/main.rs
// Location: Add to ScriptListApp struct fields (around line 1580)

/// Scroll coalescing state for rapid key events
scroll_coalescer: ScrollCoalescer,

// Add this new struct near the top of the file
#[derive(Default)]
struct ScrollCoalescer {
    pending_direction: Option<ScrollDirection>,
    pending_delta: i32,
    last_event: std::time::Instant,
}

#[derive(Clone, Copy, PartialEq)]
enum ScrollDirection { Up, Down }

impl ScrollCoalescer {
    const WINDOW_MS: u64 = 20;
    
    fn new() -> Self {
        Self {
            pending_direction: None,
            pending_delta: 0,
            last_event: std::time::Instant::now(),
        }
    }
    
    /// Process a scroll event, returning accumulated delta if coalescing window expired
    fn process(&mut self, direction: ScrollDirection) -> Option<i32> {
        let now = std::time::Instant::now();
        let within_window = now.duration_since(self.last_event) 
            < std::time::Duration::from_millis(Self::WINDOW_MS);
        
        if within_window && self.pending_direction == Some(direction) {
            // Coalesce: accumulate delta
            self.pending_delta += 1;
            None
        } else {
            // Window expired or direction changed: flush pending and start new
            let result = if self.pending_delta > 0 { Some(self.pending_delta) } else { None };
            self.pending_direction = Some(direction);
            self.pending_delta = 1;
            self.last_event = now;
            result
        }
    }
    
    /// Flush any pending scroll delta
    fn flush(&mut self) -> Option<i32> {
        if self.pending_delta > 0 {
            let result = self.pending_delta;
            self.pending_delta = 0;
            self.pending_direction = None;
            Some(result)
        } else {
            None
        }
    }
}
```

### Step 2: Update Keyboard Handler to Use Coalescing

```rust
// File: src/main.rs
// Location: In keyboard event handler (search for "arrowup" | "up")
// Replace direct move_selection_up/down calls with coalesced version

// BEFORE:
"up" | "arrowup" => {
    this.move_selection_up(cx);
    this.scroll_to_selected_if_needed("keyboard_up");
    this.trigger_scroll_activity(cx);
}

// AFTER:
"up" | "arrowup" => {
    if let Some(delta) = this.scroll_coalescer.process(ScrollDirection::Up) {
        this.move_selection_by(-delta, cx);
    } else {
        // Schedule flush after coalescing window
        cx.spawn(|this, mut cx| async move {
            Timer::after(Duration::from_millis(25)).await;
            this.update(&mut cx, |this, cx| {
                if let Some(delta) = this.scroll_coalescer.flush() {
                    this.move_selection_by(-delta, cx);
                }
            }).ok();
        }).detach();
    }
}

// Add this helper method to ScriptListApp impl:
fn move_selection_by(&mut self, delta: i32, cx: &mut Context<Self>) {
    let new_index = (self.selected_index as i32 + delta)
        .max(0)
        .min(self.filtered_results().len() as i32 - 1) as usize;
    
    if new_index != self.selected_index {
        self.selected_index = new_index;
        self.scroll_to_selected_if_needed("coalesced_move");
        self.trigger_scroll_activity(cx);
        cx.notify();
    }
}
```

### Step 3: Extract Scroll Stabilization Helper

```rust
// File: src/main.rs
// Location: Add as a helper function or method

/// Scroll stabilization helper - prevents redundant scroll_to_item calls
fn scroll_if_needed(
    handle: &UniformListScrollHandle,
    last_index: &mut Option<usize>,
    target: usize,
) {
    if *last_index != Some(target) {
        handle.scroll_to_item(target, ScrollStrategy::Nearest);
        *last_index = Some(target);
    }
}

// Apply to all list navigation points:
// - arg_list_scroll_handle (arg prompt up/down)
// - clipboard_list_scroll_handle (clipboard history navigation)
// - window_list_scroll_handle (window switcher navigation)
// - design_gallery_scroll_handle (design gallery navigation)
// - actions.rs scroll_handle (actions dialog navigation)
```

### Step 4: Connect Performance Instrumentation

```rust
// File: src/main.rs
// Location: In scroll_to_selected_if_needed()

fn scroll_to_selected_if_needed(&mut self, reason: &str) {
    let _guard = crate::perf::TimingGuard::scroll(); // Add timing
    
    let target = self.selected_index;
    
    if self.last_scrolled_index == Some(target) {
        return;
    }
    
    self.main_list_state.scroll_to_reveal_item(target);
    self.last_scrolled_index = Some(target);
}
```

### Step 5: Add Incremental Search Filtering (Optional Optimization)

```rust
// File: src/main.rs
// Location: In get_filtered_results_cached()

fn get_filtered_results_cached(&mut self) -> &Vec<scripts::SearchResult> {
    // Optimization: If new query is a prefix extension of cached query,
    // filter the cached results instead of full re-search
    if self.filter_text.starts_with(&self.filter_cache_key) 
       && !self.filter_cache_key.is_empty() 
       && self.filter_text.len() > self.filter_cache_key.len() {
        // Incremental filter: only keep results that still match
        let additional_chars = &self.filter_text[self.filter_cache_key.len()..];
        self.cached_filtered_results.retain(|result| {
            result.name().to_lowercase().contains(&self.filter_text.to_lowercase())
        });
        self.filter_cache_key = self.filter_text.clone();
        return &self.cached_filtered_results;
    }
    
    // Full re-search for other cases (query shortened, different prefix, etc.)
    if self.filter_text != self.filter_cache_key {
        self.cached_filtered_results = scripts::fuzzy_search_unified_all(
            &self.scripts,
            &self.scriptlets,
            &self.builtin_entries,
            &self.apps,
            &self.filter_text,
        );
        self.filter_cache_key = self.filter_text.clone();
    }
    
    &self.cached_filtered_results
}
```

### Testing

1. **Event coalescing verification:**
   ```bash
   # Hold down arrow key rapidly and observe logs
   echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/hello-world-args.ts"}' | \
     SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep -i coalesce
   ```

2. **Scroll performance check:**
   ```bash
   # Run scroll benchmark
   bun run scripts/scroll-bench.ts
   ```

3. **Search latency check:**
   ```bash
   # Observe search timing in logs
   echo '{"type": "run", "path": "'$(pwd)'/tests/smoke/hello-world.ts"}' | \
     SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1 | grep -i "Search.*took"
   ```

All code snippets here are copy-pasteable and consistent with the earlier packx code sections.

---

## Instructions For The Next AI Agent

You are reading the "List Rendering & Search Performance Expert Bundle". This file is self-contained and includes all the context you should assume you have.

Your job:

* Design and describe the minimal, safe changes needed to fully resolve the issues described in the Executive Summary and Key Problems.
* Operate **only** on the files and code snippets included in this bundle. If you need additional files or context, clearly say so.

When you propose changes, follow these rules strictly:

1. Always provide **precise code snippets** that can be copy-pasted directly into the repo.
2. Always include **exact file paths** (e.g. `src/main.rs`) and, when possible, line numbers or a clear description of the location (e.g. "replace the existing `scroll_to_selected_if_needed` function").
3. Never describe code changes only in prose. Show the full function or block as it should look **after** the change, or show both "before" and "after" versions.
4. Keep instructions **unmistakable and unambiguous**. A human or tool following your instructions should not need to guess what to do.
5. Assume you cannot see any files outside this bundle. If you must rely on unknown code, explicitly note assumptions and risks.

**Key files you may need but are NOT in this bundle:**
- `src/main.rs`: Contains the main `ScriptListApp` struct, keyboard handlers, and list rendering (13,000+ lines)
- `src/scripts.rs`: Contains `fuzzy_search_*` functions and `SearchResult` types
- `src/actions.rs`: Contains `ActionsDialog` with its own scroll handling

When you answer, you do not need to restate this bundle. Work directly with the code and instructions it contains and return a clear, step-by-step plan plus exact code edits.

---
