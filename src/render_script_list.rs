// Script list render method - extracted from app_render.rs
// This file is included via include!() macro in main.rs

impl ScriptListApp {
    fn render_script_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Get grouped or flat results based on filter state (cached) - MUST come first
        // to avoid borrow conflicts with theme access below
        // When filter is empty, use frecency-grouped results with RECENT/MAIN sections
        // When filtering, use flat fuzzy search results
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        // Clone for use in closures and to avoid borrow issues
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get design tokens for current design variant
        let tokens = get_tokens(self.current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_visual = tokens.visual();
        let design_typography = tokens.typography();
        let theme = &self.theme;

        // For Default design, use theme.colors for backward compatibility
        // For other designs, use design tokens
        let is_default_design = self.current_design == DesignVariant::Default;

        // P4: Pre-compute theme values using ListItemColors
        let _list_colors = ListItemColors::from_theme(theme);
        logging::log_debug("PERF", "P4: Using ListItemColors for render closure");

        let item_count = grouped_items.len();
        let _total_len = self.scripts.len() + self.scriptlets.len();

        // Handle edge cases - keep selected_index in valid bounds
        // Also skip section headers when adjusting bounds
        if item_count > 0 {
            if self.selected_index >= item_count {
                self.selected_index = item_count.saturating_sub(1);
            }
            // If we land on a section header, move to first valid item
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(self.selected_index)
            {
                // Move down to find first Item
                for (i, item) in grouped_items.iter().enumerate().skip(self.selected_index) {
                    if matches!(item, GroupedListItem::Item(_)) {
                        self.selected_index = i;
                        break;
                    }
                }
            }
        }

        // Build script list using uniform_list for proper virtualized scrolling
        // Use design tokens for empty state styling
        let empty_text_color = if is_default_design {
            theme.colors.text.muted
        } else {
            design_colors.text_muted
        };
        let empty_font_family = if is_default_design {
            ".AppleSystemUIFont"
        } else {
            design_typography.font_family
        };

        let list_element: AnyElement = if item_count == 0 {
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(empty_text_color))
                .font_family(empty_font_family)
                .child(if self.filter_text.is_empty() {
                    "No scripts or snippets found".to_string()
                } else {
                    format!("No results match '{}'", self.filter_text)
                })
                .into_any_element()
        } else {
            // Use GPUI's list() component for variable-height items
            // Section headers render at 24px, regular items at 48px
            // This gives true visual compression for headers without the uniform_list hack

            // Clone grouped_items and flat_results for the closure
            let grouped_items_clone = grouped_items.clone();
            let flat_results_clone = flat_results.clone();

            // Calculate scrollbar parameters for variable-height items
            // Count section headers vs regular items to get true content height
            let mut header_count = 0_usize;
            let mut item_count_regular = 0_usize;
            for item in grouped_items.iter() {
                match item {
                    GroupedListItem::SectionHeader(_) => header_count += 1,
                    GroupedListItem::Item(_) => item_count_regular += 1,
                }
            }

            // Calculate true content height: headers at 24px, items at 48px
            let total_content_height = (header_count as f32 * SECTION_HEADER_HEIGHT)
                + (item_count_regular as f32 * LIST_ITEM_HEIGHT);

            // Typical visible container height
            let estimated_container_height = 400.0_f32;

            // Calculate visible items as a ratio of container to total content
            // This gives a more accurate thumb size for the scrollbar
            let visible_ratio = if total_content_height > 0.0 {
                (estimated_container_height / total_content_height).min(1.0)
            } else {
                1.0
            };
            let visible_items = ((item_count as f32) * visible_ratio).ceil() as usize;

            // Get actual scroll position from ListState (not approximated from selected_index)
            let scroll_offset = self.main_list_state.logical_scroll_top().item_ix;

            // Get scrollbar colors from theme or design
            let scrollbar_colors = if is_default_design {
                ScrollbarColors::from_theme(theme)
            } else {
                ScrollbarColors::from_design(&design_colors)
            };

            // Create scrollbar (only visible if content overflows and scrolling is active)
            let scrollbar =
                Scrollbar::new(item_count, visible_items, scroll_offset, scrollbar_colors)
                    .container_height(estimated_container_height)
                    .visible(self.is_scrolling);

            // Update list state if item count changed
            if self.main_list_state.item_count() != item_count {
                self.main_list_state.reset(item_count);
            }

            // Scroll to reveal selected item
            self.main_list_state
                .scroll_to_reveal_item(self.selected_index);

            // Capture entity handle for use in the render closure
            let entity = cx.entity();

            // Clone values needed in the closure (can't access self in FnMut)
            let theme_colors = ListItemColors::from_theme(&self.theme);
            let current_design = self.current_design;

            let variable_height_list =
                list(self.main_list_state.clone(), move |ix, _window, cx| {
                    // Access entity state inside the closure
                    entity.update(cx, |this, cx| {
                        let current_selected = this.selected_index;
                        let current_hovered = this.hovered_index;

                        if let Some(grouped_item) = grouped_items_clone.get(ix) {
                            match grouped_item {
                                GroupedListItem::SectionHeader(label) => {
                                    // Section header at 24px height (SECTION_HEADER_HEIGHT)
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "section-header".into(),
                                            ix as u64,
                                        ))
                                        .h(px(SECTION_HEADER_HEIGHT))
                                        .child(render_section_header(label, theme_colors))
                                        .into_any_element()
                                }
                                GroupedListItem::Item(result_idx) => {
                                    // Regular item at 48px height (LIST_ITEM_HEIGHT)
                                    if let Some(result) = flat_results_clone.get(*result_idx) {
                                        let is_selected = ix == current_selected;
                                        let is_hovered = current_hovered == Some(ix);

                                        // Create hover handler
                                        let hover_handler = cx.listener(
                                            move |this: &mut ScriptListApp,
                                                  hovered: &bool,
                                                  _window,
                                                  cx| {
                                                let now = std::time::Instant::now();
                                                const HOVER_DEBOUNCE_MS: u64 = 16;

                                                if *hovered {
                                                    // Mouse entered - set hovered_index with debounce
                                                    if this.hovered_index != Some(ix)
                                                        && now
                                                            .duration_since(this.last_hover_notify)
                                                            .as_millis()
                                                            >= HOVER_DEBOUNCE_MS as u128
                                                    {
                                                        this.hovered_index = Some(ix);
                                                        this.last_hover_notify = now;
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_index == Some(ix) {
                                                    // Mouse left - clear hovered_index if it was this item
                                                    this.hovered_index = None;
                                                    this.last_hover_notify = now;
                                                    cx.notify();
                                                }
                                            },
                                        );

                                        // Create click handler
                                        let click_handler = cx.listener(
                                            move |this: &mut ScriptListApp,
                                                  _event: &gpui::ClickEvent,
                                                  _window,
                                                  cx| {
                                                if this.selected_index != ix {
                                                    this.selected_index = ix;
                                                    cx.notify();
                                                }
                                            },
                                        );

                                        // Dispatch to design-specific item renderer
                                        let item_element = render_design_item(
                                            current_design,
                                            result,
                                            ix,
                                            is_selected,
                                            is_hovered,
                                            theme_colors,
                                        );

                                        div()
                                            .id(ElementId::NamedInteger(
                                                "script-item".into(),
                                                ix as u64,
                                            ))
                                            .h(px(LIST_ITEM_HEIGHT)) // Explicit 48px height
                                            .on_hover(hover_handler)
                                            .on_click(click_handler)
                                            .child(item_element)
                                            .into_any_element()
                                    } else {
                                        // Fallback for missing result
                                        div().h(px(LIST_ITEM_HEIGHT)).into_any_element()
                                    }
                                }
                            }
                        } else {
                            // Fallback for out-of-bounds index
                            div().h(px(LIST_ITEM_HEIGHT)).into_any_element()
                        }
                    })
                })
                // Enable proper scroll handling for mouse wheel/trackpad
                // ListSizingBehavior::Infer sets overflow.y = Overflow::Scroll internally
                // which is required for the list's hitbox to capture scroll wheel events
                .with_sizing_behavior(ListSizingBehavior::Infer)
                .h_full();

            // Wrap list in a relative container with scrollbar overlay
            // The list() component with ListSizingBehavior::Infer handles scroll internally
            // No custom on_scroll_wheel handler needed - let GPUI handle it natively
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .h_full()
                .child(variable_height_list)
                .child(scrollbar)
                .into_any_element()
        };

        // Log panel
        let log_panel = if self.show_logs {
            let logs = logging::get_last_logs(10);
            let mut log_container = div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(theme.colors.background.log_panel))
                .border_t_1()
                .border_color(rgb(theme.colors.ui.border))
                .p(px(design_spacing.padding_md))
                .max_h(px(120.))
                .font_family("SF Mono");

            for log_line in logs.iter().rev() {
                log_container = log_container.child(
                    div()
                        .text_color(rgb(theme.colors.ui.success))
                        .text_xs()
                        .child(log_line.clone()),
                );
            }
            Some(log_container)
        } else {
            None
        };

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                // Global shortcuts (Cmd+W only - ScriptList has special ESC handling below)
                if this.handle_global_shortcut_with_options(event, false, cx) {
                    return;
                }

                let key_str = event.keystroke.key.to_lowercase();
                let has_cmd = event.keystroke.modifiers.platform;

                // Check SDK action shortcuts FIRST (before built-in shortcuts)
                // This allows scripts to override default shortcuts via setActions()
                if !this.action_shortcuts.is_empty() {
                    let key_combo =
                        shortcuts::keystroke_to_shortcut(&key_str, &event.keystroke.modifiers);
                    if let Some(action_name) = this.action_shortcuts.get(&key_combo).cloned() {
                        logging::log(
                            "ACTIONS",
                            &format!(
                                "SDK action shortcut matched: '{}' -> '{}'",
                                key_combo, action_name
                            ),
                        );
                        if this.trigger_action_by_name(&action_name, cx) {
                            return;
                        }
                    }
                }

                if has_cmd {
                    let has_shift = event.keystroke.modifiers.shift;

                    match key_str.as_str() {
                        "l" => {
                            this.toggle_logs(cx);
                            return;
                        }
                        "k" => {
                            this.toggle_actions(cx, window);
                            return;
                        }
                        // Cmd+1 cycles through all designs
                        "1" => {
                            this.cycle_design(cx);
                            return;
                        }
                        // Script context shortcuts (require a selected script)
                        "e" => {
                            // Cmd+E - Edit Script
                            this.handle_action("edit_script".to_string(), cx);
                            return;
                        }
                        "f" if has_shift => {
                            // Cmd+Shift+F - Reveal in Finder
                            this.handle_action("reveal_in_finder".to_string(), cx);
                            return;
                        }
                        "c" if has_shift => {
                            // Cmd+Shift+C - Copy Path
                            this.handle_action("copy_path".to_string(), cx);
                            return;
                        }
                        // Global shortcuts
                        "n" => {
                            // Cmd+N - Create Script
                            this.handle_action("create_script".to_string(), cx);
                            return;
                        }
                        "r" => {
                            // Cmd+R - Reload Scripts
                            this.handle_action("reload_scripts".to_string(), cx);
                            return;
                        }
                        "," => {
                            // Cmd+, - Settings
                            this.handle_action("settings".to_string(), cx);
                            return;
                        }
                        "q" => {
                            // Cmd+Q - Quit
                            this.handle_action("quit".to_string(), cx);
                            return;
                        }
                        _ => {}
                    }
                }

                // If actions popup is open, route keyboard events to it
                if this.show_actions_popup {
                    if let Some(ref dialog) = this.actions_dialog {
                        match key_str.as_str() {
                            "up" | "arrowup" => {
                                dialog.update(cx, |d, cx| d.move_up(cx));
                                return;
                            }
                            "down" | "arrowdown" => {
                                dialog.update(cx, |d, cx| d.move_down(cx));
                                return;
                            }
                            "enter" => {
                                // Get the selected action and execute it
                                let action_id = dialog.read(cx).get_selected_action_id();
                                let should_close = dialog.read(cx).selected_action_should_close();
                                if let Some(action_id) = action_id {
                                    logging::log(
                                        "ACTIONS",
                                        &format!(
                                            "Executing action: {} (close={})",
                                            action_id, should_close
                                        ),
                                    );
                                    // Only close if action has close: true (default)
                                    if should_close {
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focus_main_filter(window, cx);
                                    }
                                    this.handle_action(action_id, cx);
                                }
                                return;
                            }
                            "escape" => {
                                this.show_actions_popup = false;
                                this.actions_dialog = None;
                                this.focus_main_filter(window, cx);
                                cx.notify();
                                return;
                            }
                            "backspace" => {
                                dialog.update(cx, |d, cx| d.handle_backspace(cx));
                                return;
                            }
                            _ => {
                                // Route character input to the dialog for search
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            dialog.update(cx, |d, cx| d.handle_char(ch, cx));
                                        }
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                match key_str.as_str() {
                    "up" | "arrowup" => {
                        let _key_perf = crate::perf::KeyEventPerfGuard::new();
                        match this.nav_coalescer.record(NavDirection::Up) {
                            NavRecord::ApplyImmediate => this.move_selection_up(cx),
                            NavRecord::Coalesced => {}
                            NavRecord::FlushOld { dir, delta } => {
                                if delta != 0 {
                                    this.apply_nav_delta(dir, delta, cx);
                                }
                                this.move_selection_up(cx);
                            }
                        }
                        this.ensure_nav_flush_task(cx);
                    }
                    "down" | "arrowdown" => {
                        let _key_perf = crate::perf::KeyEventPerfGuard::new();
                        match this.nav_coalescer.record(NavDirection::Down) {
                            NavRecord::ApplyImmediate => this.move_selection_down(cx),
                            NavRecord::Coalesced => {}
                            NavRecord::FlushOld { dir, delta } => {
                                if delta != 0 {
                                    this.apply_nav_delta(dir, delta, cx);
                                }
                                this.move_selection_down(cx);
                            }
                        }
                        this.ensure_nav_flush_task(cx);
                    }
                    "enter" => {
                        if !this.gpui_input_focused {
                            this.execute_selected(cx);
                        }
                    }
                    "escape" => {
                        if !this.filter_text.is_empty() {
                            // Clear filter first if there's text
                            this.clear_filter(window, cx);
                        } else {
                            // Filter is empty - close window
                            this.close_and_reset_window(cx);
                        }
                    }
                    _ => {}
                }
            },
        );

        // Main container with system font and transparency
        // Use theme opacity settings for background transparency
        let opacity = self.theme.get_opacity();

        // Use design tokens for background color (or theme for Default design)
        let bg_hex = if is_default_design {
            theme.colors.background.main
        } else {
            design_colors.background
        };
        let bg_with_alpha = self.hex_to_rgba_with_opacity(bg_hex, opacity.main);

        // Create box shadows from theme
        let box_shadows = self.create_box_shadows();

        // Use design tokens for border radius
        let border_radius = if is_default_design {
            12.0 // Default radius
        } else {
            design_visual.radius_lg
        };

        // Use design tokens for text color
        let text_primary = if is_default_design {
            theme.colors.text.primary
        } else {
            design_colors.text_primary
        };

        // Use design tokens for font family
        let font_family = if is_default_design {
            ".AppleSystemUIFont"
        } else {
            design_typography.font_family
        };

        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgba(bg_with_alpha))
            .shadow(box_shadows)
            .w_full()
            .h_full()
            .rounded(px(border_radius))
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Header: Search Input + Run + Actions + Logo
            // Use shared header layout constants for consistency with all prompts
            .child({
                // Use shared header constants for default design, design tokens for others
                let header_padding_x = if is_default_design {
                    HEADER_PADDING_X
                } else {
                    design_spacing.padding_lg
                };
                let header_padding_y = if is_default_design {
                    HEADER_PADDING_Y
                } else {
                    design_spacing.padding_sm
                };
                let header_gap = if is_default_design {
                    HEADER_GAP
                } else {
                    design_spacing.gap_md
                };
                let text_muted = if is_default_design {
                    theme.colors.text.muted
                } else {
                    design_colors.text_muted
                };
                let text_dimmed = if is_default_design {
                    theme.colors.text.dimmed
                } else {
                    design_colors.text_dimmed
                };
                let accent_color = if is_default_design {
                    theme.colors.accent.selected
                } else {
                    design_colors.accent
                };
                let input_height = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0);

                div()
                    .w_full()
                    .px(px(header_padding_x))
                    .py(px(header_padding_y))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(header_gap))
                    // Search input with cursor and selection support
                    .child(
                        div().flex_1().flex().flex_row().items_center().child(
                            Input::new(&self.gpui_input_state)
                                .w_full()
                                .h(px(input_height))
                                .px(px(0.))
                                .py(px(0.))
                                .with_size(Size::Size(px(20.))) // text_xl = 1.25rem = 20px
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false),
                        ),
                    )
                    // CLS-FREE ACTIONS AREA: Fixed-size relative container with stacked children
                    // Both states are always rendered at the same position, visibility toggled via opacity
                    // This prevents any layout shift when toggling between Run/Actions and search input
                    .child({
                        let button_colors = ButtonColors::from_theme(&self.theme);
                        let handle_run = cx.entity().downgrade();
                        let handle_actions = cx.entity().downgrade();
                        let show_actions = self.show_actions_popup;

                        // Get actions search text from the dialog
                        let search_text = self
                            .actions_dialog
                            .as_ref()
                            .map(|dialog| dialog.read(cx).search_text.clone())
                            .unwrap_or_default();
                        let search_is_empty = search_text.is_empty();
                        let search_display = if search_is_empty {
                            SharedString::from("Search actions...")
                        } else {
                            SharedString::from(search_text.clone())
                        };

                        // Outer container: relative positioned, fixed height to match header
                        div()
                            .relative()
                            .h(px(28.)) // Fixed height to prevent vertical CLS
                            .flex()
                            .items_center()
                            // Run + Actions buttons - absolute positioned, hidden when actions shown
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    // Visibility: hidden when actions popup is shown
                                    .when(show_actions, |d| d.opacity(0.).invisible())
                                    // Run button with click handler
                                    .child(
                                        Button::new("Run", button_colors)
                                            .variant(ButtonVariant::Ghost)
                                            .shortcut("↵")
                                            .on_click(Box::new(move |_, _window, cx| {
                                                if let Some(app) = handle_run.upgrade() {
                                                    app.update(cx, |this, cx| {
                                                        this.execute_selected(cx);
                                                    });
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    )
                                    // Actions button with click handler
                                    .child(
                                        Button::new("Actions", button_colors)
                                            .variant(ButtonVariant::Ghost)
                                            .shortcut("⌘ K")
                                            .on_click(Box::new(move |_, window, cx| {
                                                if let Some(app) = handle_actions.upgrade() {
                                                    app.update(cx, |this, cx| {
                                                        this.toggle_actions(cx, window);
                                                    });
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    ),
                            )
                            // Actions search input - absolute positioned, visible when actions shown
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(8.))
                                    // Visibility: hidden when actions popup is NOT shown
                                    .when(!show_actions, |d| d.opacity(0.).invisible())
                                    // ⌘K indicator
                                    .child(div().text_color(rgb(text_dimmed)).text_xs().child("⌘K"))
                                    // Search input display - compact style matching buttons
                                    // CRITICAL: Fixed width prevents resize when typing
                                    .child(
                                        div()
                                            .flex_shrink_0() // PREVENT flexbox from shrinking this
                                            .w(px(130.0)) // Compact width
                                            .min_w(px(130.0))
                                            .max_w(px(130.0))
                                            .h(px(24.0)) // Comfortable height with padding
                                            .min_h(px(24.0))
                                            .max_h(px(24.0))
                                            .overflow_hidden()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .px(px(8.)) // Comfortable horizontal padding
                                            .rounded(px(4.)) // Match button border radius
                                            // ALWAYS show background - just vary intensity
                                            .bg(rgba(
                                                (theme.colors.background.search_box << 8)
                                                    | if search_is_empty { 0x40 } else { 0x80 },
                                            ))
                                            .border_1()
                                            // ALWAYS show border - just vary intensity
                                            .border_color(rgba(
                                                (accent_color << 8)
                                                    | if search_is_empty { 0x20 } else { 0x40 },
                                            ))
                                            .text_sm()
                                            .text_color(if search_is_empty {
                                                rgb(text_muted)
                                            } else {
                                                rgb(text_primary)
                                            })
                                            // Cursor before placeholder when empty
                                            .when(search_is_empty, |d| {
                                                d.child(
                                                    div()
                                                        .w(px(2.))
                                                        .h(px(14.)) // Cursor height for comfortable input
                                                        .mr(px(2.))
                                                        .rounded(px(1.))
                                                        .when(
                                                            self.focused_input
                                                                == FocusedInput::ActionsSearch
                                                                && self.cursor_visible,
                                                            |d| d.bg(rgb(accent_color)),
                                                        ),
                                                )
                                            })
                                            .child(search_display)
                                            // Cursor after text when not empty
                                            .when(!search_is_empty, |d| {
                                                d.child(
                                                    div()
                                                        .w(px(2.))
                                                        .h(px(14.)) // Cursor height for comfortable input
                                                        .ml(px(2.))
                                                        .rounded(px(1.))
                                                        .when(
                                                            self.focused_input
                                                                == FocusedInput::ActionsSearch
                                                                && self.cursor_visible,
                                                            |d| d.bg(rgb(accent_color)),
                                                        ),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .mx(px(4.)) // Horizontal margin for spacing
                                            .text_color(rgba((text_dimmed << 8) | 0x60)) // Reduced opacity (60%)
                                            .text_sm() // Slightly smaller text
                                            .child("|"),
                                    ),
                            )
                    })
                    // Script Kit Logo - ALWAYS visible
                    // Size slightly larger than text for visual presence
                    .child(
                        svg()
                            .external_path(utils::get_logo_path())
                            .size(px(16.)) // Slightly larger than text_sm for visual presence
                            .text_color(rgb(accent_color)),
                    )
            })
            // Subtle divider - semi-transparent
            // Use design tokens for border color and spacing
            .child({
                let divider_margin = if is_default_design {
                    16.0
                } else {
                    design_spacing.margin_lg
                };
                let border_color = if is_default_design {
                    theme.colors.ui.border
                } else {
                    design_colors.border
                };
                let border_width = if is_default_design {
                    1.0
                } else {
                    design_visual.border_thin
                };

                div()
                    .mx(px(divider_margin))
                    .h(px(border_width))
                    .bg(rgba((border_color << 8) | 0x60))
            });

        // Main content area - 50/50 split: List on left, Preview on right
        main_div = main_div
            // Uses min_h(px(0.)) to prevent flex children from overflowing
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.)) // Critical: allows flex container to shrink properly
                    .w_full()
                    .overflow_hidden()
                    // Left side: Script list (50% width) - uses uniform_list for auto-scrolling
                    .child(
                        div()
                            .w_1_2() // 50% width
                            .h_full() // Take full height
                            .min_h(px(0.)) // Allow shrinking
                            .child(list_element),
                    )
                    // Right side: Preview panel (50% width) with actions overlay
                    // Preview ALWAYS renders, actions panel overlays on top when visible
                    .child(
                        div()
                            .relative() // Enable absolute positioning for overlay
                            .w_1_2() // 50% width
                            .h_full() // Take full height
                            .min_h(px(0.)) // Allow shrinking
                            .overflow_hidden()
                            // Preview panel ALWAYS renders (visible behind actions overlay)
                            .child(self.render_preview_panel(cx))
                            // Actions dialog overlays on top using absolute positioning
                            // Includes a backdrop to capture clicks outside the dialog
                            .when_some(
                                if self.show_actions_popup {
                                    self.actions_dialog.clone()
                                } else {
                                    None
                                },
                                |d, dialog| {
                                    // Create click handler for backdrop to dismiss dialog
                                    let backdrop_click = cx.listener(|this: &mut Self, _event: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>| {
                                        logging::log("FOCUS", "Actions backdrop clicked - dismissing dialog");
                                        this.show_actions_popup = false;
                                        this.actions_dialog = None;
                                        this.focus_main_filter(window, cx);
                                        cx.notify();
                                    });

                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0() // Cover entire preview area
                                            // Backdrop layer - captures clicks outside the dialog
                                            .child(
                                                div()
                                                    .id("actions-backdrop")
                                                    .absolute()
                                                    .inset_0()
                                                    .on_click(backdrop_click)
                                            )
                                            // Dialog container - positioned at top-right
                                            .child(
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .flex()
                                                    .justify_end()
                                                    .pr(px(8.)) // Small padding from right edge
                                                    .pt(px(8.)) // Small padding from top
                                                    .child(dialog),
                                            ),
                                    )
                                },
                            ),
                    ),
            );

        if let Some(panel) = log_panel {
            main_div = main_div.child(panel);
        }

        // Note: Toast notifications are now handled by gpui-component's NotificationList
        // via the Root wrapper. Toasts are flushed in render() via flush_pending_toasts().

        // Note: HUD overlay is added at the top-level render() method for all views

        main_div.into_any_element()
    }
}
