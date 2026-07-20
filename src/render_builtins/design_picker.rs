use crate::designs::registry;

const DESIGN_PICKER_LIST_PAGE_SIZE: usize = 8;

impl ScriptListApp {
    pub(crate) fn current_design_id(&self) -> &'static str {
        crate::designs::legacy_migration::map_legacy_variant_to_id(self.current_design)
    }

    /// Structured `design` state receipt for `getState`/`kit/state`. Carries
    /// the runtime active id, the canonicalized persisted id from
    /// `DesignsConfig.active_id`, a `fallbackApplied` flag, and the legacy
    /// `DesignVariant` debug name. Phase 4 of the Cmd+1 Design Picker rollout
    /// pins this shape so the agentic matrix can prove persistence after
    /// restart.
    pub(crate) fn design_state_receipt(&self) -> serde_json::Value {
        let active_id = self.current_design_id();
        let raw_persisted = self
            .config
            .designs
            .as_ref()
            .and_then(|designs| designs.active_id.as_deref());
        let persisted_active_id: Option<&'static str> =
            raw_persisted.and_then(crate::designs::legacy_migration::resolve_possibly_legacy_id);
        let fallback_applied = match raw_persisted {
            Some(raw) => {
                crate::designs::legacy_migration::resolve_possibly_legacy_id(raw).is_none()
            }
            None => true,
        };
        serde_json::json!({
            "activeId": active_id,
            "persistedActiveId": persisted_active_id,
            "currentVariant": format!("{:?}", self.current_design),
            "fallbackApplied": fallback_applied,
        })
    }

    pub(crate) fn legacy_variant_for_design_id(id: &str) -> DesignVariant {
        match id {
            "script-kit-classic" => DesignVariant::Default,
            "pro-dense" | "high-density-list" => DesignVariant::Compact,
            "minimal-ink" | "mono-contrast" => DesignVariant::Minimal,
            "retro-terminal" | "retro-amber" => DesignVariant::RetroTerminal,
            "glass-frost" | "liquid-glass-compact" => DesignVariant::Glassmorphism,
            "editorial-brutalist" | "brutalist-grid" => DesignVariant::Brutalist,
            "neon-cyber" | "synthwave" => DesignVariant::NeonCyberpunk,
            "paper-print" => DesignVariant::Paper,
            "apple-hig" => DesignVariant::AppleHIG,
            "material-you" => DesignVariant::Material3,
            "playful-pop" | "gallery-visual" => DesignVariant::Playful,
            _ => registry::lookup(id)
                .map(|def| match def.renderer_mode {
                    registry::RendererMode::Minimal => DesignVariant::Minimal,
                    registry::RendererMode::RetroTerminal => DesignVariant::RetroTerminal,
                    registry::RendererMode::Glass => DesignVariant::Glassmorphism,
                    registry::RendererMode::Brutalist => DesignVariant::Brutalist,
                    registry::RendererMode::NeonCyber => DesignVariant::NeonCyberpunk,
                    registry::RendererMode::Paper => DesignVariant::Paper,
                    registry::RendererMode::AppleHig => DesignVariant::AppleHIG,
                    registry::RendererMode::Material => DesignVariant::Material3,
                    registry::RendererMode::Playful | registry::RendererMode::Gallery => {
                        DesignVariant::Playful
                    }
                    registry::RendererMode::Default => DesignVariant::Default,
                })
                .unwrap_or_default(),
        }
    }

    fn design_picker_filtered_indices(filter: &str) -> Vec<usize> {
        let catalog = registry::catalog();
        if filter.is_empty() {
            return (0..catalog.len()).collect();
        }

        let needle = filter.to_lowercase();
        catalog
            .iter()
            .enumerate()
            .filter(|(_, def)| {
                def.id.to_lowercase().contains(&needle)
                    || def.name.to_lowercase().contains(&needle)
                    || def.description.to_lowercase().contains(&needle)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn sync_design_picker_list_state(&mut self, item_count: usize) {
        let old_count = self.design_picker_list_state.item_count();
        if old_count != item_count {
            self.design_picker_list_state
                .splice(0..old_count, item_count);
        }
    }

    fn preview_design_picker_id(&mut self, id: &str, reason: &str, cx: &mut Context<Self>) {
        let next = Self::legacy_variant_for_design_id(id);
        if self.current_design != next {
            crate::logging::log(
                "DESIGNS",
                &format!("{}: preview design `{}` via {:?}", reason, id, next),
            );
            self.current_design = next;
            cx.notify();
        }
    }

    fn restore_design_picker_original(&mut self, reason: &str, cx: &mut Context<Self>) {
        if let Some(original) = self.design_before_picker.take() {
            self.preview_design_picker_id(&original, reason, cx);
        }
    }

    fn preview_design_picker_filtered_index(
        &mut self,
        filtered: &[usize],
        selected_index: usize,
        reason: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(catalog_index) = filtered.get(selected_index).copied() {
            if let Some(def) = registry::catalog().get(catalog_index) {
                self.preview_design_picker_id(def.id, reason, cx);
            }
        }
    }

    fn design_picker_id_for_filtered_index(
        filtered: &[usize],
        selected_index: usize,
    ) -> Option<&'static str> {
        let catalog_index = filtered.get(selected_index).copied()?;
        registry::catalog().get(catalog_index).map(|def| def.id)
    }

    fn persist_design_picker_selection(
        &mut self,
        id: &str,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.preview_design_picker_id(id, reason, cx);
        match crate::config::save_active_design_id(id) {
            Ok(()) => crate::logging::log(
                "DESIGNS",
                &format!("{reason}: persisted active design `{id}`"),
            ),
            Err(error) => {
                crate::logging::log(
                    "DESIGNS",
                    &format!("{reason}: failed to persist active design `{id}`: {error}"),
                );
                tracing::warn!(error = %error, design_id = %id, reason, "design_picker_persist_failed");
            }
        }
    }

    fn submit_design_picker_from_input_enter(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let AppView::DesignPickerView {
            filter,
            selected_index,
            ..
        } = &self.current_view
        {
            let filtered = Self::design_picker_filtered_indices(filter);
            if let Some(id) = Self::design_picker_id_for_filtered_index(&filtered, *selected_index)
            {
                self.persist_design_picker_selection(id, "design_picker_done", cx);
            }
        }
        self.design_before_picker = None;
        self.go_back_or_close(window, cx);
    }

    fn render_design_picker(
        &mut self,
        filter: &str,
        selected_index: usize,
        original_design_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let chrome = theme::AppChromeColors::from_theme(self.theme.as_ref());
        let list_colors = crate::list_item::ListItemColors::from_theme(self.theme.as_ref());
        let filtered_indices = std::sync::Arc::new(Self::design_picker_filtered_indices(filter));
        let filtered_count = filtered_indices.len();
        self.sync_design_picker_list_state(filtered_count);
        let catalog = registry::catalog();
        let original_id = original_design_id
            .as_deref()
            .or(self.design_before_picker.as_deref())
            .unwrap_or(self.current_design_id())
            .to_string();
        let live_design_id = self.current_design_id();
        let entity_handle = cx.entity().downgrade();

        // ── Keyboard handler ───────────────────────────────────────
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);
                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::DesignPicker,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        cx.stop_propagation();
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::DesignPicker,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                        return;
                    }
                }

                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        this.restore_design_picker_original("design_picker_escape_restore", cx);
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.restore_design_picker_original("design_picker_cmd_w_restore", cx);
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let current_filter =
                    if let AppView::DesignPickerView { ref filter, .. } = this.current_view {
                        filter.clone()
                    } else {
                        return;
                    };
                let filtered = Self::design_picker_filtered_indices(&current_filter);
                let count = filtered.len();
                if count == 0 {
                    if is_key_up(key)
                        || is_key_down(key)
                        || key.eq_ignore_ascii_case("left")
                        || key.eq_ignore_ascii_case("right")
                        || key.eq_ignore_ascii_case("home")
                        || key.eq_ignore_ascii_case("end")
                        || key.eq_ignore_ascii_case("pageup")
                        || key.eq_ignore_ascii_case("pagedown")
                    {
                        cx.stop_propagation();
                    }
                    return;
                }

                if let AppView::DesignPickerView {
                    ref mut selected_index,
                    ..
                } = this.current_view
                {
                    let page_size: usize = DESIGN_PICKER_LIST_PAGE_SIZE;
                    match key {
                        _ if is_key_up(key) || key.eq_ignore_ascii_case("left") => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                            }
                        }
                        _ if is_key_down(key) || key.eq_ignore_ascii_case("right") => {
                            if *selected_index < count - 1 {
                                *selected_index += 1;
                            }
                        }
                        _ if key.eq_ignore_ascii_case("home") => {
                            *selected_index = 0;
                        }
                        _ if key.eq_ignore_ascii_case("end") => {
                            *selected_index = count - 1;
                        }
                        _ if key.eq_ignore_ascii_case("pageup") => {
                            *selected_index = selected_index.saturating_sub(page_size);
                        }
                        _ if key.eq_ignore_ascii_case("pagedown") => {
                            *selected_index = (*selected_index + page_size).min(count - 1);
                        }
                        _ => return,
                    }
                    let idx = *selected_index;
                    this.preview_design_picker_filtered_index(
                        &filtered,
                        idx,
                        "design_picker_keyboard_preview",
                        cx,
                    );
                    this.design_picker_list_state.scroll_to(ListOffset {
                        item_ix: idx,
                        offset_in_item: px(0.),
                    });
                    cx.stop_propagation();
                }
            },
        );

        let selected = selected_index;
        let filtered_indices_for_list = std::sync::Arc::clone(&filtered_indices);
        let list = list(
            self.design_picker_list_state.clone(),
            move |ix, _window, _cx| {
                let catalog_index = filtered_indices_for_list[ix];
                let def = &catalog[catalog_index];
                let is_selected = ix == selected;
                let is_original = def.id == original_id;
                let is_live = def.id == live_design_id;
                let entity_handle = entity_handle.clone();
                let clicked_design_id = def.id;
                let click_handler =
                    move |_event: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
                        cx.stop_propagation();
                        if let Some(app) = entity_handle.upgrade() {
                            app.update(cx, |this, cx| {
                                if let AppView::DesignPickerView {
                                    ref mut selected_index,
                                    ..
                                } = this.current_view
                                {
                                    *selected_index = ix;
                                }
                                this.persist_design_picker_selection(
                                    clicked_design_id,
                                    "design_picker_mouse_click",
                                    cx,
                                );
                                this.design_before_picker = None;
                                this.go_back_or_close(window, cx);
                            });
                        }
                    };

                let swatch = div()
                    .w(px(8.0))
                    .h(px(28.0))
                    .rounded(px(4.0))
                    .bg(if is_live {
                        rgba(chrome.selection_rgba)
                    } else {
                        rgba(chrome.hover_rgba)
                    });

                let status = if is_original {
                    Some(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(rgba(chrome.accent_badge_border_rgba))
                            .bg(rgba(chrome.accent_badge_bg_rgba))
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(chrome.accent_badge_text_hex))
                            .child("Saved")
                            .into_any_element(),
                    )
                } else {
                    None
                };

                let item = crate::list_item::ListItem::new(def.name, list_colors)
                    .description(format!("{}  ·  {}", def.id, def.description))
                    .selected(is_selected || is_live)
                    .index(ix)
                    .leading_accessory(swatch)
                    .trailing_accessory_opt(status);

                div()
                    .id(ix)
                    .cursor_pointer()
                    .on_click(click_handler)
                    .child(item)
                    .into_any_element()
            },
        )
        .h_full()
        .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
        .into_any_element();

        let header = div()
            .w_full()
            .px(px(design_spacing.padding_lg))
            .pt(px(design_spacing.padding_sm))
            .pb(px(design_spacing.padding_sm))
            .child(
                Input::new(&self.gpui_input_state)
                    .w_full()
                    .text_size(px(design_typography.font_size_md))
                    .into_any_element(),
            );

        div()
            .id("design-picker-view")
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .flex()
            .flex_col()
            .bg(rgba(chrome.panel_surface_rgba))
            .child(header)
            .child(
                div()
                    .mx(px(design_spacing.padding_lg))
                    .h(px(1.0))
                    .bg(rgba(chrome.divider_rgba)),
            )
            .child(if filtered_count == 0 {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(chrome.text_muted_hex))
                    .child("No designs match your filter")
                    .into_any_element()
            } else {
                list
            })
            .into_any_element()
    }
}
