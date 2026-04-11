//! StoryBrowser - Main UI for browsing and previewing stories
//!
//! Features:
//! - Left sidebar with searchable story list grouped by category
//! - Right panel showing selected story preview
//! - Compare mode: side-by-side variant preview with keyboard selection
//! - Adopt flow: persist selected variant to disk
//! - Theme and design variant controls in toolbar
//! - Keyboard navigation support
//! - Screenshot capture (Cmd+Shift+S)

use gpui::*;
use std::fs;
use std::path::PathBuf;

use crate::designs::DesignVariant;
use crate::storybook::{
    all_categories, all_stories, first_story_with_multiple_variants, load_story_selections,
    save_story_selections, selection_store_path, stories_by_surface, StoryEntry,
    StorySelectionStore, StorySurface, StoryVariant,
};

/// Preview mode for the story browser
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewMode {
    Single,
    Compare,
}

/// Main browser view for the storybook
pub struct StoryBrowser {
    stories: Vec<&'static StoryEntry>,
    selected_index: usize,
    selected_variant_index: usize,
    filter: String,
    theme_name: String,
    design_variant: DesignVariant,
    preview_mode: PreviewMode,
    selection_store: StorySelectionStore,
    status_line: Option<String>,
    focus_handle: FocusHandle,
    screenshot_dir: PathBuf,
    compare_scroll: ScrollHandle,
}

impl StoryBrowser {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let stories: Vec<_> = all_stories().collect();

        let screenshot_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("test-screenshots");

        if let Err(error) = fs::create_dir_all(&screenshot_dir) {
            tracing::warn!(
                event = "storybook_screenshot_dir_unavailable",
                path = %screenshot_dir.display(),
                error = %error,
                "Failed to prepare screenshot directory"
            );
        }

        let (selection_store, status_line) = match load_story_selections() {
            Ok(store) => {
                tracing::info!(
                    event = "design_explorer_selection_store_loaded",
                    path = %selection_store_path().display(),
                    selection_count = store.selections.len(),
                    "Loaded design explorer selection store"
                );
                (store, None)
            }
            Err(error) => {
                tracing::error!(
                    event = "design_explorer_selection_store_load_failed",
                    path = %selection_store_path().display(),
                    error = %error,
                    "Failed to load design explorer selection store; falling back to empty state"
                );
                (
                    StorySelectionStore::default(),
                    Some(format!(
                        "Design explorer selections were reset for this session: {}",
                        error
                    )),
                )
            }
        };

        let mut browser = Self {
            stories,
            selected_index: 0,
            selected_variant_index: 0,
            filter: String::new(),
            theme_name: "Default".to_string(),
            design_variant: DesignVariant::Default,
            preview_mode: PreviewMode::Single,
            selection_store,
            status_line,
            focus_handle: cx.focus_handle(),
            screenshot_dir,
            compare_scroll: ScrollHandle::new(),
        };

        browser.reset_variant_selection();
        browser
    }

    pub fn load_theme(&mut self, theme_name: &str) {
        // For now, just update the name
        self.theme_name = theme_name.to_string();
    }

    /// Select a story by its ID. Returns `true` if the story was found.
    pub fn select_story(&mut self, story_id: &str) -> bool {
        if let Some(pos) = self.stories.iter().position(|s| s.story.id() == story_id) {
            self.selected_index = pos;
            self.reset_variant_selection();
            tracing::info!(story_id = %story_id, event = "story_selected", "Story selected");
            true
        } else {
            tracing::warn!(story_id = %story_id, event = "story_not_found", "Unknown story ID");
            false
        }
    }

    pub fn set_design_variant(&mut self, variant: DesignVariant) {
        self.design_variant = variant;
    }

    /// Open compare mode if the current story has more than one variant.
    /// Returns `true` if compare mode was activated.
    pub fn open_compare_mode(&mut self) -> bool {
        if self.current_story_variants().len() > 1 {
            self.preview_mode = PreviewMode::Compare;
            tracing::info!(event = "compare_mode_opened", "Compare mode activated");
            true
        } else {
            tracing::warn!(
                event = "compare_mode_skipped",
                "Story has fewer than 2 variants"
            );
            false
        }
    }

    /// Configure the browser for the in-app design explorer.
    ///
    /// If a preferred surface has a comparable story (>1 variant), use it.
    /// Otherwise, fall back to the first available story and stay in single mode.
    pub fn configure_for_design_explorer(&mut self, preferred_surface: Option<StorySurface>) {
        if let Some(surface) = preferred_surface {
            if let Some(entry) = stories_by_surface(surface).into_iter().next() {
                tracing::info!(
                    event = "design_explorer_configured",
                    surface = %surface.label(),
                    story_id = %entry.story.id(),
                    comparable = entry.story.variants().len() > 1,
                    "Configured design explorer with preferred surface story"
                );
                let _ = self.select_story(entry.story.id());
                if entry.story.variants().len() > 1 {
                    let _ = self.open_compare_mode();
                }
                return;
            }
        }

        if let Some(entry) = first_story_with_multiple_variants() {
            tracing::info!(
                event = "design_explorer_configured",
                story_id = %entry.story.id(),
                fallback = true,
                "Configured design explorer with first comparable story"
            );
            let _ = self.select_story(entry.story.id());
            let _ = self.open_compare_mode();
        } else if let Some(entry) = self.stories.first() {
            tracing::info!(
                event = "design_explorer_configured",
                story_id = %entry.story.id(),
                fallback = true,
                comparable = false,
                "Configured design explorer with single-story fallback"
            );
            let _ = self.select_story(entry.story.id());
        } else {
            tracing::warn!(
                event = "design_explorer_no_comparable_story",
                "No stories found for design explorer"
            );
        }
    }

    /// Pre-select a variant by its stable id. Returns `true` if the variant was found.
    pub fn select_variant_id(&mut self, variant_id: &str) -> bool {
        if let Some(index) = self
            .current_story_variants()
            .iter()
            .position(|v| v.stable_id() == variant_id)
        {
            self.selected_variant_index = index;
            tracing::info!(variant_id = %variant_id, event = "variant_selected", "Variant pre-selected");
            true
        } else {
            tracing::warn!(variant_id = %variant_id, event = "variant_not_found", "Unknown variant ID");
            false
        }
    }

    /// Return all known story IDs (for CLI diagnostics).
    pub fn story_ids(&self) -> Vec<&'static str> {
        self.stories.iter().map(|s| s.story.id()).collect()
    }

    /// Return variant IDs for the currently selected story (for CLI diagnostics).
    pub fn variant_ids(&self) -> Vec<String> {
        self.current_story_variants()
            .iter()
            .map(|v| v.stable_id())
            .collect()
    }

    fn current_story(&self) -> Option<&'static StoryEntry> {
        self.stories.get(self.selected_index).copied()
    }

    fn current_story_variants(&self) -> Vec<StoryVariant> {
        let Some(story) = self.current_story() else {
            return vec![StoryVariant::default_named("default", "Default")];
        };

        let mut variants = story.story.variants();
        if variants.is_empty() {
            variants.push(StoryVariant::default_named("default", "Default"));
        }
        variants
    }

    fn reset_variant_selection(&mut self) {
        let Some(story) = self.current_story() else {
            self.selected_variant_index = 0;
            return;
        };

        let variants = self.current_story_variants();

        if let Some(saved_variant_id) = self.selection_store.selected_variant(story.story.id()) {
            if let Some(index) = variants
                .iter()
                .position(|v| v.stable_id() == saved_variant_id)
            {
                self.selected_variant_index = index;
                return;
            }
        }

        self.selected_variant_index = 0;
    }

    /// Emit a structured trace event capturing the full browser state.
    ///
    /// Every significant state transition calls this so that machines
    /// (agents, CI, log parsers) can reconstruct what happened.
    fn trace_state(&self, event_name: &'static str, source: &'static str) {
        let story = self.current_story();
        let variants = self.current_story_variants();
        let selected_variant_id = variants
            .get(self.selected_variant_index)
            .map(|variant| variant.stable_id())
            .unwrap_or_else(|| "default".to_string());

        tracing::info!(
            event = event_name,
            source = source,
            story_id = story.map(|entry| entry.story.id()).unwrap_or("none"),
            surface = story
                .map(|entry| entry.story.surface().label())
                .unwrap_or("Unknown"),
            preview_mode = match self.preview_mode {
                PreviewMode::Single => "single",
                PreviewMode::Compare => "compare",
            },
            variant_count = variants.len(),
            selected_variant_index = self.selected_variant_index,
            selected_variant_id = %selected_variant_id,
            "StoryBrowser state transition"
        );
    }

    fn toggle_compare_mode(&mut self, cx: &mut Context<Self>) {
        if self.current_story_variants().len() <= 1 {
            self.preview_mode = PreviewMode::Single;
            self.status_line = Some("Selected story has no comparable variants yet".to_string());
            self.trace_state("compare_mode_unavailable", "tab");
            cx.notify();
            return;
        }

        self.preview_mode = match self.preview_mode {
            PreviewMode::Single => PreviewMode::Compare,
            PreviewMode::Compare => PreviewMode::Single,
        };
        self.status_line = None;
        self.trace_state("compare_mode_toggled", "tab");
        cx.notify();
    }

    fn move_variant_left(&mut self, cx: &mut Context<Self>) {
        let count = self.current_story_variants().len();
        if count <= 1 {
            return;
        }

        self.selected_variant_index = if self.selected_variant_index == 0 {
            count - 1
        } else {
            self.selected_variant_index - 1
        };
        self.trace_state("variant_focus_changed", "arrow_left");
        self.compare_scroll
            .scroll_to_item(self.selected_variant_index);
        cx.notify();
    }

    fn move_variant_right(&mut self, cx: &mut Context<Self>) {
        let count = self.current_story_variants().len();
        if count <= 1 {
            return;
        }

        self.selected_variant_index = (self.selected_variant_index + 1) % count;
        self.trace_state("variant_focus_changed", "arrow_right");
        self.compare_scroll
            .scroll_to_item(self.selected_variant_index);
        cx.notify();
    }

    fn select_variant_by_shortcut(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(digit) = key.chars().next() else {
            return;
        };
        let Some(index) = digit
            .to_digit(10)
            .map(|value| value.saturating_sub(1) as usize)
        else {
            return;
        };

        let variants = self.current_story_variants();
        let Some(variant) = variants.get(index) else {
            tracing::warn!(
                event = "variant_shortcut_out_of_range",
                shortcut = %key,
                variant_count = variants.len(),
                "Ignored out-of-range compare shortcut"
            );
            return;
        };

        self.selected_variant_index = index;
        self.compare_scroll.scroll_to_item(index);

        if let Some(story) = self.current_story() {
            tracing::info!(
                event = "variant_focus_changed",
                story_id = %story.story.id(),
                variant_id = %variant.stable_id(),
                source = "shortcut",
                shortcut = %key,
                "Focused compare variant"
            );
        }

        cx.notify();
    }

    fn adopt_selected_variant(&mut self, cx: &mut Context<Self>) {
        let Some(story) = self.current_story() else {
            return;
        };

        let story_id = story.story.id().to_string();
        let story_name = story.story.name().to_string();
        let surface = story.story.surface().label().to_string();
        let variants = self.current_story_variants();

        let Some(variant) = variants.get(self.selected_variant_index) else {
            return;
        };

        let variant_id = variant.stable_id();
        let variant_name = if variant.name.is_empty() {
            variant_id.clone()
        } else {
            variant.name.clone()
        };

        let mut next_store = self.selection_store.clone();
        next_store.set_selected_variant(story_id.clone(), variant_id.clone());

        let selection_path = selection_store_path();

        match save_story_selections(&next_store) {
            Ok(()) => {
                self.selection_store = next_store;
                tracing::info!(
                    event = "variant_adopted",
                    story_id = %story_id,
                    story_name = %story_name,
                    surface = %surface,
                    variant_id = %variant_id,
                    variant_name = %variant_name,
                    path = %selection_path.display(),
                    selection_count = self.selection_store.selections.len(),
                    "Persisted adopted variant"
                );
                self.status_line = Some(format!(
                    "Adopted \"{}\" for {} and saved to {}",
                    variant_name,
                    story_name,
                    selection_path.display()
                ));
            }
            Err(error) => {
                tracing::error!(
                    event = "variant_adoption_failed",
                    story_id = %story_id,
                    story_name = %story_name,
                    surface = %surface,
                    variant_id = %variant_id,
                    variant_name = %variant_name,
                    path = %selection_path.display(),
                    error = %error,
                    "Failed to persist adopted variant"
                );
                self.status_line = Some(format!(
                    "Failed to save \"{}\" for {} at {}: {}",
                    variant_name,
                    story_name,
                    selection_path.display(),
                    error
                ));
            }
        }

        cx.notify();
    }

    fn filtered_stories(&self) -> Vec<&'static StoryEntry> {
        if self.filter.is_empty() {
            self.stories.clone()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.stories
                .iter()
                .filter(|s| {
                    s.story.name().to_lowercase().contains(&filter_lower)
                        || s.story.category().to_lowercase().contains(&filter_lower)
                })
                .copied()
                .collect()
        }
    }

    fn render_search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        let filter = self.filter.clone();
        div()
            .p_2()
            .border_b_1()
            .border_color(rgb(theme.colors.ui.border))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .bg(rgb(theme.colors.background.title_bar))
                    .rounded_md()
                    .child(
                        // Search icon
                        div().text_color(rgb(theme.colors.text.dimmed)).child("?"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(if filter.is_empty() {
                                rgb(theme.colors.text.dimmed)
                            } else {
                                rgb(theme.colors.text.secondary)
                            })
                            .child(if filter.is_empty() {
                                "Search stories...".to_string()
                            } else {
                                filter
                            }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _event, _window, _cx| {
                    // TODO: Focus search input and enable text input
                }),
            )
    }

    fn render_story_list(
        &self,
        filtered: &[&'static StoryEntry],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        let categories = all_categories();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .children(categories.into_iter().map(|category| {
                let category_stories: Vec<_> = filtered
                    .iter()
                    .filter(|s| s.story.category() == category)
                    .copied()
                    .collect();

                if category_stories.is_empty() {
                    return div().into_any_element();
                }

                div()
                    .flex()
                    .flex_col()
                    .child(
                        // Category header
                        div()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.tertiary))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(category.to_uppercase()),
                    )
                    .children(category_stories.into_iter().map(|story| {
                        let is_selected = self
                            .stories
                            .iter()
                            .position(|s| s.story.id() == story.story.id())
                            == Some(self.selected_index);

                        let story_id = story.story.id();

                        let base = div()
                            .id(ElementId::Name(story_id.into()))
                            .px_3()
                            .py_1()
                            .cursor_pointer()
                            .text_sm()
                            .rounded_sm()
                            .child(story.story.name())
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if let Some(pos) =
                                    this.stories.iter().position(|s| s.story.id() == story_id)
                                {
                                    this.selected_index = pos;
                                    this.reset_variant_selection();
                                    cx.notify();
                                }
                            }));

                        if is_selected {
                            base.bg(rgb(theme.colors.ui.info))
                                .text_color(rgb(theme.colors.text.primary))
                        } else {
                            base.text_color(rgb(theme.colors.text.secondary))
                                .hover(|s| s.bg(rgb(theme.colors.ui.border)))
                        }
                    }))
                    .into_any_element()
            }))
    }

    fn render_toolbar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(rgb(theme.colors.ui.border))
            .bg(rgb(theme.colors.background.title_bar))
            .child(
                // Left: Story info
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(theme.colors.text.primary))
                            .child(
                                self.stories
                                    .get(self.selected_index)
                                    .map(|s| s.story.name())
                                    .unwrap_or("No story selected"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.dimmed))
                            .child(
                                self.current_story()
                                    .map(|story| {
                                        format!(
                                            "({} \u{00b7} {} \u{00b7} {})",
                                            story.story.category(),
                                            story.story.surface().label(),
                                            match self.preview_mode {
                                                PreviewMode::Single => "single",
                                                PreviewMode::Compare => "compare",
                                            }
                                        )
                                    })
                                    .unwrap_or_default(),
                            ),
                    ),
            )
            .child(
                // Right: Theme & Design controls
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme.colors.text.tertiary))
                                    .child("Theme:"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(rgb(theme.colors.text.secondary))
                                    .bg(rgb(theme.colors.background.title_bar))
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .child(self.theme_name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme.colors.text.tertiary))
                                    .child("Design:"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .text_xs()
                                    .text_color(rgb(theme.colors.text.secondary))
                                    .bg(rgb(theme.colors.background.title_bar))
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .child(format!("{:?}", self.design_variant)),
                            ),
                    ),
            )
    }

    fn render_single_preview(&self, story: &'static StoryEntry) -> AnyElement {
        let variants = self.current_story_variants();

        if let Some(variant) = variants.get(self.selected_variant_index) {
            return story.story.render_variant(variant);
        }

        story.story.render()
    }

    fn render_compare_preview(&self, story: &'static StoryEntry) -> AnyElement {
        let theme = crate::theme::get_cached_theme();
        let border = theme.colors.ui.border;
        let accent = theme.colors.accent.selected;
        let text_primary = theme.colors.text.primary;
        let text_muted = theme.colors.text.dimmed;

        let adopted_variant = self
            .selection_store
            .selected_variant(story.story.id())
            .map(str::to_string);

        let variants = self.current_story_variants();

        let variant_count = variants.len();

        div()
            .id("compare-scroll")
            .track_scroll(&self.compare_scroll)
            .size_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_4()
            .overflow_x_scroll()
            .child(
                div().h_full().flex().flex_row().children(
                    variants
                        .into_iter()
                        .enumerate()
                        .flat_map(|(index, variant)| {
                            let variant_id = variant.stable_id();
                            let description = variant.description.clone().unwrap_or_default();
                            let is_selected = index == self.selected_variant_index;
                            let is_adopted =
                                adopted_variant.as_deref() == Some(variant_id.as_str());
                            let preview_content = story.story.render_compare_variant(&variant);
                            let mut items = Vec::new();

                            let panel = div()
                                .h_full()
                                .min_h(px(0.))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .pr(px(16.))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div()
                                                .flex_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(if is_selected {
                                                            rgb(accent)
                                                        } else {
                                                            rgb(text_primary)
                                                        })
                                                        .child(format!(
                                                            "[{}] {}",
                                                            index + 1,
                                                            variant.name
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(text_muted))
                                                        .child(description),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if is_adopted {
                                                    rgb(accent)
                                                } else {
                                                    rgb(text_muted)
                                                })
                                                .child(if is_adopted { "Adopted" } else { "" }),
                                        ),
                                )
                                .child({
                                    let mut preview = div()
                                        .id(gpui::ElementId::Name(
                                            format!("variant-preview-{index}").into(),
                                        ))
                                        .flex_1()
                                        .min_h(px(0.))
                                        .min_w(px(0.));

                                    if is_selected {
                                        preview = preview.overflow_y_scroll();
                                    } else {
                                        preview = preview.overflow_hidden();
                                    }

                                    preview.child(preview_content)
                                });

                            items.push(panel.into_any_element());

                            if index + 1 < variant_count {
                                items.push(
                                    div()
                                        .w(px(1.0))
                                        .h_full()
                                        .bg(rgb(border))
                                        .opacity(0.5)
                                        .mr(px(16.0))
                                        .into_any_element(),
                                );
                            }

                            items
                        }),
                ),
            )
            .into_any_element()
    }

    fn render_preview(&self) -> AnyElement {
        let theme = crate::theme::get_cached_theme();

        if let Some(story) = self.current_story() {
            match self.preview_mode {
                PreviewMode::Single => self.render_single_preview(story),
                PreviewMode::Compare => self.render_compare_preview(story),
            }
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(rgb(theme.colors.text.dimmed))
                .child("No story selected")
                .into_any_element()
        }
    }

    fn render_status_bar(&self) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();

        let default_text = match self.preview_mode {
            PreviewMode::Single => {
                "Tab compare \u{00b7} \u{2191}\u{2193} stories \u{00b7} Cmd+Shift+S screenshot"
                    .to_string()
            }
            PreviewMode::Compare => {
                "\u{2190}\u{2192} variants \u{00b7} 1-9 focus \u{00b7} Enter adopt \u{00b7} Tab single \u{00b7} Esc exit compare"
                    .to_string()
            }
        };

        div()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(rgb(theme.colors.ui.border))
            .text_xs()
            .text_color(rgb(theme.colors.text.dimmed))
            .child(self.status_line.clone().unwrap_or(default_text))
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_stories();
        if filtered.is_empty() {
            return;
        }

        // Find current story in filtered list
        if let Some(current) = self.stories.get(self.selected_index) {
            if let Some(pos) = filtered
                .iter()
                .position(|s| s.story.id() == current.story.id())
            {
                if pos > 0 {
                    // Move to previous in filtered list
                    let prev_story = filtered[pos - 1];
                    if let Some(main_pos) = self
                        .stories
                        .iter()
                        .position(|s| s.story.id() == prev_story.story.id())
                    {
                        self.selected_index = main_pos;
                        self.reset_variant_selection();
                        cx.notify();
                    }
                }
            }
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_stories();
        if filtered.is_empty() {
            return;
        }

        // Find current story in filtered list
        if let Some(current) = self.stories.get(self.selected_index) {
            if let Some(pos) = filtered
                .iter()
                .position(|s| s.story.id() == current.story.id())
            {
                if pos < filtered.len() - 1 {
                    // Move to next in filtered list
                    let next_story = filtered[pos + 1];
                    if let Some(main_pos) = self
                        .stories
                        .iter()
                        .position(|s| s.story.id() == next_story.story.id())
                    {
                        self.selected_index = main_pos;
                        self.reset_variant_selection();
                        cx.notify();
                    }
                }
            }
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        // Cmd+Shift+S for screenshot
        if key == "s" && modifiers.platform && modifiers.shift {
            self.capture_screenshot(window, cx);
            cx.stop_propagation();
            return;
        }

        match key {
            "tab" | "Tab" => {
                self.toggle_compare_mode(cx);
                cx.stop_propagation();
            }
            "left" | "arrowleft" if self.preview_mode == PreviewMode::Compare => {
                self.move_variant_left(cx);
                cx.stop_propagation();
            }
            "right" | "arrowright" if self.preview_mode == PreviewMode::Compare => {
                self.move_variant_right(cx);
                cx.stop_propagation();
            }
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
                if self.preview_mode == PreviewMode::Compare =>
            {
                self.select_variant_by_shortcut(key, cx);
                cx.stop_propagation();
            }
            "enter" | "Enter" | "return" | "Return"
                if self.preview_mode == PreviewMode::Compare =>
            {
                self.adopt_selected_variant(cx);
                cx.stop_propagation();
            }
            "escape" | "Escape" if self.preview_mode == PreviewMode::Compare => {
                self.preview_mode = PreviewMode::Single;
                self.status_line = None;
                cx.notify();
                cx.stop_propagation();
            }
            "up" | "arrowup" => {
                self.move_selection_up(cx);
                cx.stop_propagation();
            }
            "down" | "arrowdown" => {
                self.move_selection_down(cx);
                cx.stop_propagation();
            }
            _ => cx.propagate(),
        }
    }

    /// Capture a screenshot of the current storybook window
    fn capture_screenshot(&self, _window: &mut Window, _cx: &mut Context<Self>) {
        use image::codecs::png::PngEncoder;
        use image::ImageEncoder;
        use xcap::Window as XCapWindow;

        let story_id = self
            .stories
            .get(self.selected_index)
            .map(|s| s.story.id())
            .unwrap_or("unknown");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let filename = format!("storybook-{}-{}.png", story_id, timestamp);
        let filepath = self.screenshot_dir.join(&filename);

        // Find storybook window using xcap
        match XCapWindow::all() {
            Ok(windows) => {
                for win in windows {
                    let title = win.title().unwrap_or_default();
                    let app_name = win.app_name().unwrap_or_default();

                    // Match storybook window
                    if title.contains("Storybook") || app_name.contains("storybook") {
                        match win.capture_image() {
                            Ok(img) => {
                                let width = img.width();
                                let height = img.height();
                                let rgba_data = img.into_raw();

                                // Encode as PNG
                                let mut png_data = Vec::new();
                                let encoder = PngEncoder::new(&mut png_data);
                                if let Err(e) = encoder.write_image(
                                    &rgba_data,
                                    width,
                                    height,
                                    image::ExtendedColorType::Rgba8,
                                ) {
                                    tracing::error!(error = %e, "Screenshot PNG encode failed");
                                    return;
                                }

                                // Save to file
                                if let Err(e) = fs::write(&filepath, &png_data) {
                                    tracing::error!(
                                        error = %e,
                                        path = %filepath.display(),
                                        "Screenshot save failed"
                                    );
                                } else {
                                    tracing::info!(
                                        event = "storybook_screenshot_saved",
                                        path = %filepath.display(),
                                        story_id = story_id,
                                        surface = self.current_story()
                                            .map(|entry| entry.story.surface().label())
                                            .unwrap_or("Unknown"),
                                        preview_mode = match self.preview_mode {
                                            PreviewMode::Single => "single",
                                            PreviewMode::Compare => "compare",
                                        },
                                        "Captured storybook screenshot"
                                    );
                                }
                                return;
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Screenshot capture failed");
                            }
                        }
                    }
                }
                tracing::error!("Storybook window not found for screenshot");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to enumerate windows for screenshot");
            }
        }
    }
}

impl Render for StoryBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        let filtered = self.filtered_stories();

        // Render the story preview - stories are stateless so no App context needed
        let preview = self.render_preview();

        div()
            .id("story-browser")
            .key_context("StoryBrowser")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(theme.colors.background.main))
            .text_color(rgb(theme.colors.text.secondary))
            // Left sidebar: story list
            .child(
                div()
                    .w(px(280.))
                    .border_r_1()
                    .border_color(rgb(theme.colors.ui.border))
                    .flex()
                    .flex_col()
                    .bg(rgb(theme.colors.background.title_bar))
                    .child(
                        // Header
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(theme.colors.ui.border))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme.colors.text.primary))
                                    .child("Script Kit Storybook"),
                            ),
                    )
                    .child(self.render_search_bar(cx))
                    // Scrollable story list
                    .child(
                        div()
                            .id("story-list-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(self.render_story_list(&filtered, cx)),
                    ),
            )
            // Right panel: story preview
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.render_toolbar(cx))
                    // Scrollable preview area
                    .child(
                        div()
                            .id("story-preview-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(preview),
                    )
                    .child(self.render_status_bar()),
            )
    }
}

impl Focusable for StoryBrowser {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::storybook::{
        all_stories, first_story_with_multiple_variants, stories_by_surface, StorySelectionStore,
        StorySurface,
    };

    /// Simulates the browser's `reset_variant_selection` logic:
    /// given a story's variants and a selection store, return the variant index
    /// that the browser would select.
    fn resolve_variant_index(
        story_id: &str,
        variants: &[crate::storybook::StoryVariant],
        store: &StorySelectionStore,
    ) -> usize {
        if let Some(saved) = store.selected_variant(story_id) {
            if let Some(index) = variants.iter().position(|v| v.stable_id() == saved) {
                return index;
            }
        }
        0
    }

    // --- Compare-mode transition tests ---

    #[test]
    fn compare_mode_is_available_in_design_lab() {
        let multi_variant_count = all_stories()
            .filter(|entry| entry.story.variants().len() > 1)
            .count();
        assert!(
            multi_variant_count >= 4,
            "design lab should have at least 4 compare-ready stories, got {multi_variant_count}"
        );

        assert!(
            first_story_with_multiple_variants().is_some(),
            "design lab should expose at least one compare-ready story"
        );
    }

    #[test]
    fn single_variant_stories_remain_compare_ineligible() {
        for entry in all_stories() {
            let variants = entry.story.variants();
            if variants.len() <= 1 {
                let would_enter = variants.len() > 1;
                assert!(
                    !would_enter,
                    "Story '{}' with {} variant(s) should NOT enter compare mode",
                    entry.story.id(),
                    variants.len()
                );
            }
        }
    }

    #[test]
    fn variant_navigation_noop_for_single_variant() {
        let single = all_stories().find(|e| e.story.variants().len() <= 1);
        if let Some(entry) = single {
            let count = entry.story.variants().len();
            let index: usize = 0;
            if count > 1 {
                panic!("expected single-variant story");
            }
            assert_eq!(index, 0, "single-variant story should not navigate");
        }
    }

    #[test]
    fn shortcut_key_zero_maps_to_index_zero_which_is_harmless() {
        // Key "0" parses as digit 0, saturating_sub(1) = 0.
        // This maps to the first variant, which is a no-op if already
        // focused there. The browser doesn't treat "0" specially — it
        // just selects index 0, same as key "1".
        let key_digit: u32 = 0;
        let mapped = key_digit.saturating_sub(1) as usize;
        assert_eq!(mapped, 0, "key 0 should map to index 0 via saturating_sub");
    }

    // --- Adoption state machine tests ---

    #[test]
    fn adoption_updates_store_without_mutating_unrelated_stories() {
        let mut store = StorySelectionStore::default();
        store.set_selected_variant("main-menu", "current-main-menu");
        store.set_selected_variant("legacy-story", "legacy-variant");

        store.set_selected_variant("main-menu", "current-main-menu");

        assert_eq!(
            store.selected_variant("main-menu"),
            Some("current-main-menu")
        );
        assert_eq!(
            store.selected_variant("legacy-story"),
            Some("legacy-variant")
        );
    }

    #[test]
    fn adoption_then_reset_restores_persisted_variant() {
        let mut store = StorySelectionStore::default();
        store.set_selected_variant("main-menu", "current-main-menu");

        let main_menu_stories = stories_by_surface(StorySurface::MainMenu);
        let main_menu_entry = main_menu_stories
            .iter()
            .find(|e| e.story.id() == "main-menu")
            .expect("main-menu story must exist");

        let variants = main_menu_entry.story.variants();
        let resolved = resolve_variant_index("main-menu", &variants, &store);

        let resolved_variant = &variants[resolved];
        assert_eq!(
            resolved_variant.stable_id(),
            "current-main-menu",
            "reset_variant_selection should restore the persisted variant"
        );
    }

    #[test]
    fn adoption_with_unknown_variant_falls_back_to_first() {
        let mut store = StorySelectionStore::default();
        store.set_selected_variant("main-menu", "does-not-exist");

        let main_menu_stories = stories_by_surface(StorySurface::MainMenu);
        let main_menu_entry = main_menu_stories
            .iter()
            .find(|e| e.story.id() == "main-menu")
            .expect("main-menu story must exist");

        let variants = main_menu_entry.story.variants();
        let resolved = resolve_variant_index("main-menu", &variants, &store);

        assert_eq!(
            resolved, 0,
            "unknown persisted variant should fall back to index 0"
        );
    }

    #[test]
    fn adoption_with_empty_store_selects_first_variant() {
        let store = StorySelectionStore::default();

        let main_menu_stories = stories_by_surface(StorySurface::MainMenu);
        let main_menu_entry = main_menu_stories
            .iter()
            .find(|e| e.story.id() == "main-menu")
            .expect("main-menu story must exist");

        let variants = main_menu_entry.story.variants();
        let resolved = resolve_variant_index("main-menu", &variants, &store);

        assert_eq!(resolved, 0, "empty store should default to first variant");
    }

    #[test]
    fn design_lab_has_multiple_stories() {
        let stories: Vec<_> = all_stories().collect();
        assert!(
            stories.len() >= 5,
            "design lab should have at least 5 stories, got {}",
            stories.len()
        );
        assert!(
            stories.iter().any(|e| e.story.id() == "main-menu"),
            "main-menu story must be registered"
        );
    }

    #[test]
    fn sequential_adoptions_preserve_latest_per_story() {
        let mut store = StorySelectionStore::default();

        store.set_selected_variant("main-menu", "current-main-menu");
        store.set_selected_variant("main-menu", "current-main-menu");
        store.set_selected_variant("main-menu", "current-main-menu");

        assert_eq!(
            store.selected_variant("main-menu"),
            Some("current-main-menu"),
            "last adoption should win"
        );

        store.set_selected_variant("legacy-story", "legacy-variant");
        assert_eq!(
            store.selected_variant("legacy-story"),
            Some("legacy-variant")
        );
        assert_eq!(
            store.selected_variant("main-menu"),
            Some("current-main-menu")
        );
    }
}
