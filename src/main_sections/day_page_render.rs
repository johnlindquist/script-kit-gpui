// Day Page render host: editor, navigation, clipboard shelf, and shared chrome.
impl Render for DayPageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_published_theme(cx);
        self.poll_external_disk_changes(window, cx);
        self.maybe_rebind_after_midnight(window, cx);
        self.maybe_autosave(window, cx);

        let Some(app) = self.app.upgrade() else {
            tracing::warn!("day_page.render_abandoned_after_host_dropped");
            return div().into_any_element();
        };

        let app_state = app.read(cx);
        let menu_def = app_state.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let search = menu_def.search;
        let tokens = get_tokens(app_state.current_design);
        let design_visual = tokens.visual();
        let is_default_design = app_state.current_design.is_default();
        let text_primary = app_state.theme.colors.text.primary;
        let font_family = app_state.theme_font_family();

        let columns = crate::components::main_view_chrome::main_view_content_columns(menu_def);
        let editor_layout = self.notes_editor.read(cx).layout();
        let viewport_height = window.viewport_size().height.as_f32();
        // Day Page is the explicit view-owned context-only policy: its editor
        // belongs to MainViewMain, so no phantom input lane or input gap is
        // reserved above it.
        let header_height =
            crate::components::main_view_chrome::main_view_header_metrics(menu_def, None)
                .header_height;
        let footer_height = crate::components::footer_chrome::current_main_menu_footer_height();
        let shelf_count = if self.kit_resource_preview.is_none() {
            self.clipboard_shelf.len()
        } else {
            0
        };
        let layout_budget = day_page_layout_budget(
            viewport_height,
            header_height,
            footer_height,
            shelf_count,
            self.clipboard_shelf_expanded,
            editor_layout.padding_y,
        );
        let editor_input = self.notes_editor.read(cx).render_input(cx);
        // Hover discoverability: names the click action while the mouse is
        // over a deeplink (the vendored input paints underline + pointer
        // cursor). Absolute overlay — never reflows the editor. The receipt
        // records what this render actually built (only meaningful in edit
        // mode; preview/read branches below don't mount the editor).
        let hover_hint_model = if self.kit_resource_preview.is_some() || self.read_mode {
            None
        } else {
            crate::notes::deeplink_activation::hover_hint_model(
                self.notes_editor.read(cx).hovered_deeplink(cx),
                crate::notes::deeplink_activation::ActivationSurface::DayPage,
            )
        };
        self.last_deeplink_hover_hint = hover_hint_model
            .as_ref()
            .map(|(verb, href)| serde_json::json!({ "verb": verb, "href": href }));
        let deeplink_hover_hint = hover_hint_model.map(|(verb, href)| {
            crate::components::resource_preview::render_deeplink_hover_hint(
                "day-page-deeplink-hover-hint",
                verb,
                &href,
                cx,
            )
        });
        let editor_input = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .h_full()
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseUpEvent, window, cx| {
                    this.activate_deeplink_from_mouse_up(event.clone(), window, cx);
                }),
            )
            .child(editor_input);
        let viewing_fragment = self.session.is_viewing_fragment();
        let theme = app_state.theme.clone();

        let local_today = self.now()
            .with_timezone(&self.session.substrate().timezone())
            .date_naive();
        let viewing_past_day = !viewing_fragment
            && self
                .session
                .bound_date()
                .is_some_and(|date| date != local_today);

        let back_bar = if viewing_fragment {
            let label = match self.session.binding() {
                DayPageBinding::Fragment {
                    return_day_date, ..
                } => {
                    format!("Today · {return_day_date}")
                }
                DayPageBinding::Day => "Today".to_string(),
                DayPageBinding::Note { title, .. } => title.clone(),
            };
            Some(crate::components::render_back_affordance(
                script_kit_gpui::day_page::FRAGMENT_BACK_ID.into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.return_to_day_page(window, cx);
                }),
            ))
        } else if viewing_past_day {
            let label = self
                .session
                .bound_date()
                .map(|date| format!("Back to Today · viewing {date}"))
                .unwrap_or_else(|| "Back to Today".to_string());
            Some(crate::components::render_back_affordance(
                "day-page-past-day-back".into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.bind_today(window, cx);
                    this.focus_editor(window, cx);
                }),
            ))
        } else if self.session.is_viewing_note() {
            let label = self
                .session
                .viewing_note_title()
                .map(|title| format!("Back to Today · viewing {title}"))
                .unwrap_or_else(|| "Back to Today".to_string());
            Some(crate::components::render_back_affordance(
                "day-page-note-back".into(),
                label.into(),
                &theme,
                cx.listener(|this, _, window, cx| {
                    this.return_to_day_page(window, cx);
                    this.focus_editor(window, cx);
                }),
            ))
        } else {
            None
        };

        let editor_content = if self.kit_resource_preview.is_some() {
            self.render_kit_resource_preview(cx)
        } else if self.read_mode {
            self.render_day_page_read_mode(cx)
        } else {
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .child(editor_input)
                .when_some(deeplink_hover_hint, |d, chip| d.child(chip))
                .into_any_element()
        };

        let clipboard_shelf = self
            .render_clipboard_shelf(layout_budget.shelf_list_height, cx)
            .map(|shelf| self.notes_editor.read(cx).render_content_accessory(shelf));

        let editor_body = div()
            .id(DAY_PAGE_EDITOR_ID)
            .flex_1()
            .min_h(px(0.))
            .h_full()
            // Symmetric content padding matching the notes/markdown editors,
            // rather than the launcher's list-text column inset
            // (`input_text_inset_left`) which pushed the day-page prose far to
            // the right and looked inconsistent with every other markdown view.
            .pl(px(columns.content_right_inset_x))
            .pr(px(columns.content_right_inset_x))
            .flex()
            .flex_col()
            .when_some(back_bar, |parent, bar| parent.child(bar))
            .child(
                div()
                    // GPUI divs are display:block by default; without .flex()
                    // the editor's flex_1/h_full chain resolves against an
                    // auto height and collapses to a single line.
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(DAY_PAGE_MIN_EDITOR_HEIGHT_PX))
                    .child(editor_content),
            )
            .when_some(clipboard_shelf, |parent, shelf| parent.child(shelf));

        let context_zone = app.update(cx, |app, _cx| {
            app.render_inert_main_view_context_zone(menu_def)
        });

        let main = div()
            .flex()
            .flex_col()
            .h_full()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .min_h(px(search.height))
                    .flex_1()
                    .min_h(px(0.))
                    .child(editor_body),
            )
            .into_any_element();

        let header = crate::components::main_view_chrome::MainViewHeaderChrome::context_only(
            menu_def,
            context_zone,
        );

        let divider = crate::components::main_view_chrome::MainViewDividerChrome {
            margin_x: shell.divider_margin_x,
            height: if is_default_design {
                shell.divider_height
            } else {
                design_visual.border_thin
            },
            visible: false,
        };

        let root = crate::components::main_view_chrome::render_main_view_shell()
            .text_color(rgb(text_primary))
            .font_family(font_family)
            .key_context("day_page")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_key_down(event, window, cx);
            }));

        let preview_availability = self.kit_resource_preview_action_availability();
        let preview_return_label = self.kit_resource_preview_return_label();
        let footer = if let Some(mut footer_config) =
            app.read(cx).main_window_footer_config_with_cx(None)
        {
            footer_config.buttons = day_page_footer_buttons_for_preview(
                app.read(cx),
                preview_availability,
                preview_return_label,
            );
            let footer_app = app.downgrade();
            Some(
                crate::components::prompt_layout_shell::render_main_window_footer_slot_for_prompt_surface(
                    "day_page",
                    move || {
                        crate::components::footer_chrome::render_main_window_footer_config_rail(
                            footer_config,
                            move |action, window, cx| {
                                if let Some(app) = footer_app.upgrade() {
                                    app.update(cx, |app, cx| {
                                        app.dispatch_main_window_footer_action(
                                            action,
                                            window,
                                            cx,
                                            "gpui_footer_click",
                                        );
                                    });
                                }
                            },
                        )
                    },
                ),
            )
        } else {
            tracing::warn!("day_page.footer_config_unavailable");
            None
        };

        crate::components::main_view_chrome::render_main_view_chrome(
            root,
            &theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider,
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }
}
