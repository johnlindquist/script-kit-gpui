#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FavoritesBrowseListAction {
    Run,
    Remove,
    MoveUp,
    MoveDown,
}

impl FavoritesBrowseListAction {
    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Run => "Select a favorite to run.",
            Self::Remove => "Select a favorite to remove.",
            Self::MoveUp | Self::MoveDown => "Select a favorite to move.",
        }
    }

    fn success_message(self, id: &str) -> String {
        match self {
            Self::Run => format!("Running '{id}'"),
            Self::Remove => format!("Removed '{id}'"),
            Self::MoveUp => format!("Moved '{id}' up"),
            Self::MoveDown => format!("Moved '{id}' down"),
        }
    }

    fn missing_favorite_message(self, id: &str) -> String {
        match self {
            Self::MoveUp | Self::MoveDown => format!("Favorite '{id}' was not found."),
            Self::Run | Self::Remove => format!("Favorite '{id}' was not found."),
        }
    }

    fn boundary_message(self, id: &str) -> Option<String> {
        match self {
            Self::MoveUp => Some(format!("'{id}' is already first")),
            Self::MoveDown => Some(format!("'{id}' is already last")),
            Self::Run | Self::Remove => None,
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::Remove => format!("Failed to remove favorite: {error}"),
            Self::MoveUp => format!("Failed to move favorite up: {error}"),
            Self::MoveDown => format!("Failed to move favorite down: {error}"),
            Self::Run => format!("Failed to run favorite: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FavoritesEmptyState {
    NoFavoritesYet,
    NoFilteredMatches,
}

impl FavoritesEmptyState {
    pub(crate) fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoFavoritesYet
        } else {
            Self::NoFilteredMatches
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::NoFavoritesYet => {
                "No favorites yet \u{00b7} Star scripts from the actions menu (Cmd+K)"
            }
            Self::NoFilteredMatches => "No favorites match your filter",
        }
    }
}

impl ScriptListApp {
    fn favorite_filter_matches(&self, id: &str, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }

        let filter_lower = filter.to_lowercase();
        let display_name = self
            .scripts
            .iter()
            .find(|s| s.name == id)
            .map(|s| s.name.as_str())
            .or_else(|| {
                self.scriptlets
                    .iter()
                    .find(|sl| sl.name == id)
                    .map(|sl| sl.name.as_str())
            })
            .unwrap_or(id);
        let description = self
            .scripts
            .iter()
            .find(|s| s.name == id)
            .and_then(|s| s.description.as_deref())
            .or_else(|| {
                self.scriptlets
                    .iter()
                    .find(|sl| sl.name == id)
                    .and_then(|sl| sl.description.as_deref())
            })
            .unwrap_or("");

        display_name.to_lowercase().contains(&filter_lower)
            || description.to_lowercase().contains(&filter_lower)
    }

    pub(crate) fn filtered_favorite_ids_for_filter(&self, filter: &str) -> Vec<String> {
        script_kit_gpui::favorites::load_favorites()
            .unwrap_or_default()
            .script_ids
            .into_iter()
            .filter(|id| self.favorite_filter_matches(id, filter))
            .collect()
    }

    pub(crate) fn selected_favorite_id(&self) -> Option<String> {
        let AppView::FavoritesBrowseView {
            filter,
            selected_index,
        } = &self.current_view
        else {
            return None;
        };

        self.filtered_favorite_ids_for_filter(filter)
            .get(*selected_index)
            .cloned()
    }

    pub(crate) fn selected_favorite_source_path(&self) -> Option<(String, std::path::PathBuf)> {
        let id = self.selected_favorite_id()?;
        if let Some(script) = self.scripts.iter().find(|s| s.name == id) {
            return Some((id, script.path.clone()));
        }

        let scriptlet = self.scriptlets.iter().find(|sl| sl.name == id)?;
        let file_path = scriptlet.file_path.as_ref()?;
        let path_str = file_path.split('#').next().unwrap_or(file_path);
        Some((id, std::path::PathBuf::from(path_str)))
    }

    fn clamp_favorites_selection(&mut self) {
        let (filter, selected_index) = match &self.current_view {
            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => (filter.clone(), *selected_index),
            _ => return,
        };
        let filtered_len = self.filtered_favorite_ids_for_filter(&filter).len();

        if let AppView::FavoritesBrowseView {
            selected_index: index,
            ..
        } = &mut self.current_view
        {
            *index = if filtered_len == 0 {
                0
            } else {
                selected_index.min(filtered_len - 1)
            };
        }
    }

    /// Render the favorites browse list with search/filter.
    fn render_favorites_browse(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Load favorites and resolve to script/scriptlet names
        let favorites = script_kit_gpui::favorites::load_favorites().unwrap_or_default();

        let resolved: Vec<(String, String)> = favorites
            .script_ids
            .iter()
            .map(|id| {
                let display_name = self
                    .scripts
                    .iter()
                    .find(|s| s.name == *id)
                    .map(|s| s.name.clone())
                    .or_else(|| {
                        self.scriptlets
                            .iter()
                            .find(|sl| sl.name == *id)
                            .map(|sl| sl.name.clone())
                    })
                    .unwrap_or_else(|| id.clone());
                let description = self
                    .scripts
                    .iter()
                    .find(|s| s.name == *id)
                    .and_then(|s| s.description.clone())
                    .or_else(|| {
                        self.scriptlets
                            .iter()
                            .find(|sl| sl.name == *id)
                            .and_then(|sl| sl.description.clone())
                    })
                    .unwrap_or_default();
                (id.clone(), format!("{}\x00{}", display_name, description))
            })
            .collect();

        let filter_lower = filter.to_lowercase();
        let filtered: Vec<(usize, &str, &str, &str)> = resolved
            .iter()
            .enumerate()
            .filter_map(|(idx, (id, packed))| {
                let (name, desc) = packed.split_once('\x00').unwrap_or((packed, ""));
                if filter.is_empty()
                    || name.to_lowercase().contains(&filter_lower)
                    || desc.to_lowercase().contains(&filter_lower)
                {
                    Some((idx, id.as_str(), name, desc))
                } else {
                    None
                }
            })
            .collect();

        let count = filtered.len();
        let list_colors = ListItemColors::from_theme(&self.theme);
        let entity = cx.entity().downgrade();

        let list_items: Vec<AnyElement> = filtered
            .iter()
            .enumerate()
            .map(
                |(display_idx, (_original_idx, fav_id, name, description))| {
                    let is_selected = display_idx == selected_index;
                    let fav_id_owned = fav_id.to_string();
                    let entity_clone = entity.clone();

                    div()
                        .id(display_idx)
                        .cursor_pointer()
                        .on_click(move |_event, window, cx| {
                            if let Some(app) = entity_clone.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.run_favorite(&fav_id_owned, window, cx);
                                });
                            }
                        })
                        .child(
                            ListItem::new(name.to_string(), list_colors)
                                .description_opt(if description.is_empty() {
                                    None
                                } else {
                                    Some(description.to_string())
                                })
                                .selected(is_selected),
                        )
                        .into_any_element()
                },
            )
            .collect();

        let list_element: AnyElement = if count == 0 {
            let message = FavoritesEmptyState::from_filter(&filter).message();
            crate::components::render_simple_empty_state(
                "favorites-empty",
                message,
                "star",
                None,
                &self.theme,
                cx,
            )
        } else {
            crate::components::scrollbar::render_tracked_scroll_column(
                "favorites-row-stack",
                &self.builtin_row_stack_scroll_handle,
                list_items,
            )
        };
        // Every list leads with a persistent section separator (POLISH.md
        // layout-stability bar; same rule as the main menu's "Results"
        // header, 4d76327b8): the label may swap but the row never appears
        // or disappears, so filtering can't shift the rows below it.
        let list_element = div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.))
            .child(crate::list_item::render_section_header(
                if filter.trim().is_empty() {
                    "Favorites"
                } else {
                    "Results"
                },
                None,
                list_colors,
                true,
            ))
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element))
            .into_any_element();

        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            vec![
                gpui::SharedString::from("↵ Run"),
                gpui::SharedString::from("U Move Up"),
                gpui::SharedString::from("J Move Down"),
                gpui::SharedString::from("D Remove"),
                gpui::SharedString::from("Esc Back"),
            ],
            None,
        ));

        self.render_generic_filterable_search_surface(
            "favorites",
            format!("{} favorites", count),
            list_element,
            footer,
            cx,
        )
    }

    /// Run a favorite script by its ID (script name).
    fn run_favorite(&mut self, id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        // Try scripts first
        if let Some(script) = self.scripts.iter().find(|s| s.name == *id) {
            let path = script.path.to_string_lossy().to_string();
            tracing::info!(
                favorite_id = %id,
                path = %path,
                action = "run_favorite",
                "Running favorite script"
            );
            self.execute_script_by_path(&path, cx);
            return;
        }

        // Try scriptlets
        if let Some(scriptlet) = self.scriptlets.iter().find(|sl| sl.name == *id) {
            let scriptlet_clone = scriptlet.clone();
            tracing::info!(
                favorite_id = %id,
                action = "run_favorite_scriptlet",
                "Running favorite scriptlet"
            );
            self.execute_scriptlet(&scriptlet_clone, cx);
            return;
        }

        tracing::warn!(
            favorite_id = %id,
            action = "favorite_not_found",
            "Favorite script not found in scripts or scriptlets"
        );
        self.show_error_toast(format!("Script '{}' not found", id), cx);
    }

    pub(crate) fn run_selected_favorite(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let action = FavoritesBrowseListAction::Run;
        let id = self
            .selected_favorite_id()
            .ok_or_else(|| action.selection_required_message().to_string())?;
        self.run_favorite(&id, window, cx);
        Ok(action.success_message(&id))
    }

    pub(crate) fn remove_selected_favorite(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let action = FavoritesBrowseListAction::Remove;
        let id = self
            .selected_favorite_id()
            .ok_or_else(|| action.selection_required_message().to_string())?;

        tracing::info!(
            favorite_id = %id,
            action = "favorite_remove",
            "Removing favorite"
        );
        match script_kit_gpui::favorites::remove_favorite(&id) {
            Ok(_) => {
                self.clamp_favorites_selection();
                let success_message = action.success_message(&id);
                self.show_hud(success_message.clone(), Some(HUD_SHORT_MS), cx);
                cx.notify();
                Ok(success_message)
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    action = "favorite_remove_failed",
                    "Failed to remove favorite"
                );
                Err(action.failure_message(e))
            }
        }
    }

    pub(crate) fn move_selected_favorite_up(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let action = FavoritesBrowseListAction::MoveUp;
        let id = self
            .selected_favorite_id()
            .ok_or_else(|| action.selection_required_message().to_string())?;
        let favorites = script_kit_gpui::favorites::load_favorites().unwrap_or_default();
        let Some(original_index) = favorites.script_ids.iter().position(|item| item == &id) else {
            return Err(action.missing_favorite_message(&id));
        };
        if original_index == 0 {
            return Ok(action
                .boundary_message(&id)
                .unwrap_or_else(|| format!("'{id}' is already first")));
        }

        tracing::info!(
            favorite_id = %id,
            action = "favorite_move_up",
            "Moving favorite up"
        );
        script_kit_gpui::favorites::move_favorite_up(&id).map_err(|e| action.failure_message(e))?;
        if let AppView::FavoritesBrowseView { selected_index, .. } = &mut self.current_view {
            *selected_index = selected_index.saturating_sub(1);
        }
        cx.notify();
        Ok(action.success_message(&id))
    }

    pub(crate) fn move_selected_favorite_down(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let action = FavoritesBrowseListAction::MoveDown;
        let id = self
            .selected_favorite_id()
            .ok_or_else(|| action.selection_required_message().to_string())?;
        let favorites = script_kit_gpui::favorites::load_favorites().unwrap_or_default();
        let Some(original_index) = favorites.script_ids.iter().position(|item| item == &id) else {
            return Err(action.missing_favorite_message(&id));
        };
        if original_index + 1 >= favorites.script_ids.len() {
            return Ok(action
                .boundary_message(&id)
                .unwrap_or_else(|| format!("'{id}' is already last")));
        }

        tracing::info!(
            favorite_id = %id,
            action = "favorite_move_down",
            "Moving favorite down"
        );
        script_kit_gpui::favorites::move_favorite_down(&id)
            .map_err(|e| action.failure_message(e))?;
        if let AppView::FavoritesBrowseView { selected_index, .. } = &mut self.current_view {
            *selected_index += 1;
        }
        self.clamp_favorites_selection();
        cx.notify();
        Ok(action.success_message(&id))
    }

    /// Handle keyboard input for the favorites browse view.
    #[allow(dead_code)] // Called from startup_new_actions.rs interceptor
    pub(crate) fn handle_favorites_browse_key(
        &mut self,
        key: &str,
        has_cmd: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::FavoritesBrowseView { .. }) {
            return;
        }

        if has_cmd && key.eq_ignore_ascii_case("k") {
            if self.selected_favorite_id().is_some()
                || self.show_actions_popup
                || crate::actions::is_actions_window_open()
            {
                self.toggle_favorites_actions(window, cx);
            }
            return;
        }

        if crate::ui_foundation::is_key_enter(key) {
            if let Err(message) = self.run_selected_favorite(window, cx) {
                self.show_error_toast(message, cx);
            }
        } else if crate::ui_foundation::is_key_escape(key) {
            self.go_back_or_close(window, cx);
        } else if key.eq_ignore_ascii_case("d") && self.filter_text.is_empty() {
            if let Err(message) = self.remove_selected_favorite(cx) {
                self.show_error_toast(message, cx);
            }
        } else if key.eq_ignore_ascii_case("u") && self.filter_text.is_empty() {
            if let Err(message) = self.move_selected_favorite_up(cx) {
                self.show_error_toast(message, cx);
            }
        } else if key.eq_ignore_ascii_case("j") && self.filter_text.is_empty() {
            if let Err(message) = self.move_selected_favorite_down(cx) {
                self.show_error_toast(message, cx);
            }
        }
    }
}
