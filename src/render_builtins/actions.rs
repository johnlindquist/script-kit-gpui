#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltInActionsWindowFeedback {
    FileSearch,
    ClipboardHistory,
}

impl BuiltInActionsWindowFeedback {
    fn opened_log(self) -> &'static str {
        match self {
            Self::FileSearch => "File search actions popup window opened",
            Self::ClipboardHistory => "Clipboard actions popup window opened",
        }
    }

    fn failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::FileSearch | Self::ClipboardHistory => {
                format!("Failed to open actions window: {error}")
            }
        }
    }
}

impl ScriptListApp {
    fn dictation_history_actions_dialog_config(
        placeholder: String,
    ) -> crate::actions::ActionsDialogConfig {
        crate::actions::ActionsDialogConfig {
            search_position: crate::actions::SearchPosition::Top,
            section_style: crate::actions::SectionStyle::Headers,
            anchor: crate::actions::AnchorPosition::Top,
            show_icons: true,
            search_placeholder: Some(placeholder),
            show_context_header: false,
            ..crate::actions::ActionsDialogConfig::default()
        }
    }

    fn dictation_history_actions_for_dialog() -> Vec<crate::actions::Action> {
        use crate::actions::{Action, ActionCategory};
        use crate::designs::icon_variations::IconName;

        vec![
            Action::new(
                "dictation_history_paste",
                "Paste to Frontmost App",
                Some("Hide Script Kit and paste this transcript into the active app".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵")
            .with_section("Reuse")
            .with_icon(IconName::ArrowRight),
            Action::new(
                "dictation_history_attach_to_ai",
                "Attach to Agent Chat",
                Some("Open Agent Chat and stage this transcript in the composer".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌃⌘A")
            .with_section("Reuse")
            .with_icon(IconName::MessageCircle),
            Action::new(
                "dictation_history_save_note",
                "Save as Note",
                Some("Create a new note pre-filled with this transcript".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Reuse")
            .with_icon(IconName::Plus),
            Action::new(
                "dictation_history_copy",
                "Copy Transcript",
                Some("Copy this transcript to the clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘C")
            .with_section("Reuse")
            .with_icon(IconName::Copy),
            Action::new(
                "dictation_history_delete",
                "Delete from History",
                Some("Remove this saved transcript from dictation history".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⌫")
            .with_section("Manage")
            .with_icon(IconName::Trash),
        ]
    }

    fn favorites_actions_for_dialog() -> Vec<crate::actions::Action> {
        use crate::actions::{Action, ActionCategory};
        use crate::designs::icon_variations::IconName;

        vec![
            Action::new(
                "favorites_run",
                "Run",
                Some("Run the selected favorite".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵")
            .with_section("Actions")
            .with_icon(IconName::PlayFilled),
            Action::new(
                "favorites_edit_script",
                "Edit Script",
                Some("Open the selected favorite in the configured editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Actions")
            .with_icon(IconName::Pencil),
            Action::new(
                "favorites_copy_script_url",
                "Copy Script URL",
                Some("Copy the selected favorite's scriptkit://run URL".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Actions")
            .with_icon(IconName::Copy),
            Action::new(
                "favorites_move_up",
                "Move Up",
                Some("Move the selected favorite up".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("U")
            .with_section("Actions")
            .with_icon(IconName::ArrowUp),
            Action::new(
                "favorites_move_down",
                "Move Down",
                Some("Move the selected favorite down".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("J")
            .with_section("Actions")
            .with_icon(IconName::ArrowDown),
            Action::new(
                "favorites_remove",
                "Remove from Favorites",
                Some("Remove the selected favorite".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("D")
            .with_section("Manage")
            .with_icon(IconName::Trash),
        ]
    }

    fn theme_chooser_actions_for_dialog() -> Vec<crate::actions::Action> {
        use crate::actions::{Action, ActionCategory};
        use crate::designs::icon_variations::IconName;

        vec![
            Action::new(
                "theme_chooser_done",
                "Done",
                Some("Persist the current theme and return to the launcher".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵")
            .with_section("Theme")
            .with_icon(IconName::Check),
            Action::new(
                "theme_chooser_toggle_customize",
                "Toggle Customize Panel",
                Some("Switch the right panel between Preview and Customize".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_section("Theme")
            .with_icon(IconName::Settings),
            Action::new(
                "theme_chooser_undo_close",
                "Undo Changes and Close",
                Some("Restore the theme from when Theme Designer opened".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Theme")
            .with_icon(IconName::Close),
            Action::new(
                "theme_chooser_remix",
                "Surprise Me",
                Some("Remix accent, opacity, and material from the current theme".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘J")
            .with_section("Customize")
            .with_icon(IconName::BoltFilled),
            Action::new(
                "theme_chooser_reset",
                "Reset to Defaults",
                Some("Reset customization controls to the selected preset".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘R")
            .with_section("Customize")
            .with_icon(IconName::Refresh),
            Action::new(
                "theme_chooser_save_as_user_theme",
                "Save Copy as User Theme",
                Some("Save the current Theme Designer state as a new user theme".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Manage")
            .with_icon(IconName::Plus),
            Action::new(
                "theme_chooser_edit_theme_as_text",
                "Edit Theme as Text",
                Some(
                    "Open the current Theme Designer theme JSON in your configured editor"
                        .to_string(),
                ),
                ActionCategory::ScriptContext,
            )
            .with_section("Manage")
            .with_icon(IconName::Pencil),
            Action::new(
                "theme_chooser_update_user_theme",
                "Update Selected User Theme",
                Some(
                    "Overwrite the selected user theme with the current Theme Designer state"
                        .to_string(),
                ),
                ActionCategory::ScriptContext,
            )
            .with_section("Manage")
            .with_icon(IconName::Check),
            Action::new(
                "theme_chooser_delete_user_theme",
                "Delete Selected User Theme",
                Some(
                    "Stage deletion; run again to confirm. Built-in themes are read-only"
                        .to_string(),
                ),
                ActionCategory::ScriptContext,
            )
            .with_section("Manage")
            .with_icon(IconName::Trash),
            Action::new(
                "theme_chooser_restore_deleted_user_theme",
                "Restore Deleted User Theme",
                Some(
                    "Restore the most recently deleted user theme from this Theme Designer session"
                        .to_string(),
                ),
                ActionCategory::ScriptContext,
            )
            .with_section("Manage")
            .with_icon(IconName::Refresh),
            Action::new(
                "theme_chooser_gradient_cycle",
                "Cycle Background Gradient",
                Some("Toggle or cycle optional background gradient flair".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Customize")
            .with_icon(IconName::BoltOutlined),
            Action::new(
                "theme_chooser_gradient_layer_add",
                "Add Gradient Layer",
                Some("Stack another gradient layer on the backdrop".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Customize")
            .with_icon(IconName::Plus),
            Action::new(
                "theme_chooser_gradient_layer_remove",
                "Remove Last Gradient Layer",
                Some("Remove the most recently added gradient layer".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Customize")
            .with_icon(IconName::Trash),
            Action::new(
                "theme_chooser_accent_previous",
                "Previous Accent Color",
                Some("Move to the previous accent swatch".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘[")
            .with_section("Customize")
            .with_icon(IconName::ChevronRight),
            Action::new(
                "theme_chooser_accent_next",
                "Next Accent Color",
                Some("Move to the next accent swatch".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘]")
            .with_section("Customize")
            .with_icon(IconName::ArrowRight),
            Action::new(
                "theme_chooser_opacity_decrease",
                "Decrease Surface Opacity",
                Some("Use the next lower opacity preset".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘-")
            .with_section("Customize")
            .with_icon(IconName::ArrowDown),
            Action::new(
                "theme_chooser_opacity_increase",
                "Increase Surface Opacity",
                Some("Use the next higher opacity preset".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘=")
            .with_section("Customize")
            .with_icon(IconName::ArrowUp),
            Action::new(
                "theme_chooser_vibrancy_toggle",
                "Toggle Vibrancy Blur",
                Some("Turn vibrancy blur on or off".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘B")
            .with_section("Customize")
            .with_icon(IconName::EyeOff),
            Action::new(
                "theme_chooser_material_cycle",
                "Cycle Vibrancy Material",
                Some("Switch to the next AppKit vibrancy material".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘M")
            .with_section("Customize")
            .with_icon(IconName::Sidebar),
            Action::new(
                "theme_chooser_font_size_decrease",
                "Decrease UI Font Size",
                Some("Use the next smaller UI font preset".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Typography")
            .with_icon(IconName::ArrowDown),
            Action::new(
                "theme_chooser_font_size_increase",
                "Increase UI Font Size",
                Some("Use the next larger UI font preset".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Typography")
            .with_icon(IconName::ArrowUp),
        ]
    }

    fn toggle_theme_chooser_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        logging::log("KEY", "Toggling theme chooser actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            self.close_actions_popup(ActionsDialogHost::ThemeChooser, window, cx);
            return;
        }

        self.mark_actions_popup_opening();
        self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);
        self.focus_handle.focus(window, cx);
        self.gpui_input_focused = false;
        self.focused_input = FocusedInput::ActionsSearch;

        let theme_arc = std::sync::Arc::clone(&self.theme);
        let actions = Self::theme_chooser_actions_for_dialog();
        let dialog = cx.new(move |cx| {
            let focus_handle = cx.focus_handle();
            let mut dialog = ActionsDialog::with_config(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}),
                actions,
                theme_arc,
                crate::actions::ActionsDialogConfig {
                    search_position: crate::actions::SearchPosition::Top,
                    section_style: crate::actions::SectionStyle::Headers,
                    anchor: crate::actions::AnchorPosition::Top,
                    show_icons: true,
                    search_placeholder: Some("Theme Designer actions".to_string()),
                    show_context_header: false,
                    ..crate::actions::ActionsDialogConfig::default()
                },
            );
            dialog.set_match_main_window_background(true);
            dialog
        });

        self.actions_dialog = Some(dialog.clone());

        let app_entity = cx.entity().clone();
        dialog.update(cx, |d, _cx| {
            d.set_on_activation(Self::make_actions_dialog_activation_callback(
                app_entity.clone(),
                ActionsDialogHost::ThemeChooser,
            ));
            d.set_on_close(Self::make_actions_window_on_close_callback(
                app_entity,
                ActionsDialogHost::ThemeChooser,
                "Theme chooser actions closed via escape, focus restored via coordinator",
            ));
        });

        let parent_window_handle = window.window_handle();
        let main_bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());

        Self::spawn_open_actions_window(
            cx,
            parent_window_handle,
            main_bounds,
            display_id,
            dialog,
            crate::actions::WindowPosition::TopCenter,
            "Theme chooser actions popup window opened",
            "Failed to open theme chooser actions window",
        );

        cx.notify();
    }

    fn toggle_favorites_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        logging::log("KEY", "Toggling favorites actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            self.close_actions_popup(ActionsDialogHost::Favorites, window, cx);
            return;
        }

        let Some(selected_id) = self.selected_favorite_id() else {
            logging::log("ACTIONS", "Favorites actions ignored: no selected favorite");
            return;
        };

        self.mark_actions_popup_opening();
        self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);
        self.focus_handle.focus(window, cx);
        self.gpui_input_focused = false;
        self.focused_input = FocusedInput::ActionsSearch;

        let theme_arc = std::sync::Arc::clone(&self.theme);
        let actions = Self::favorites_actions_for_dialog();
        let dialog = cx.new(move |cx| {
            let focus_handle = cx.focus_handle();
            let mut dialog = ActionsDialog::with_config(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}),
                actions,
                theme_arc,
                crate::actions::ActionsDialogConfig {
                    search_position: crate::actions::SearchPosition::Top,
                    section_style: crate::actions::SectionStyle::Headers,
                    anchor: crate::actions::AnchorPosition::Top,
                    show_icons: true,
                    search_placeholder: Some(selected_id),
                    show_context_header: false,
                    ..crate::actions::ActionsDialogConfig::default()
                },
            );
            dialog.set_match_main_window_background(true);
            dialog
        });

        self.actions_dialog = Some(dialog.clone());

        let app_entity = cx.entity().clone();
        dialog.update(cx, |d, _cx| {
            d.set_on_activation(Self::make_actions_dialog_activation_callback(
                app_entity.clone(),
                ActionsDialogHost::Favorites,
            ));
            d.set_on_close(Self::make_actions_window_on_close_callback(
                app_entity,
                ActionsDialogHost::Favorites,
                "Favorites actions closed via escape, focus restored via coordinator",
            ));
        });

        let parent_window_handle = window.window_handle();
        let main_bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());

        Self::spawn_open_actions_window(
            cx,
            parent_window_handle,
            main_bounds,
            display_id,
            dialog,
            crate::actions::WindowPosition::TopCenter,
            "Favorites actions popup window opened",
            "Failed to open favorites actions window",
        );

        cx.notify();
    }

    fn toggle_dictation_history_actions(
        &mut self,
        entry: crate::dictation::DictationHistoryEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::log("KEY", "Toggling dictation history actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            self.close_actions_popup(ActionsDialogHost::DictationHistory, window, cx);
            return;
        }

        self.mark_actions_popup_opening();
        self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);
        self.focus_handle.focus(window, cx);
        self.gpui_input_focused = false;
        self.focused_input = FocusedInput::ActionsSearch;

        let theme_arc = std::sync::Arc::clone(&self.theme);
        let placeholder = entry.preview.clone();
        let actions = Self::dictation_history_actions_for_dialog();
        let dialog = cx.new(move |cx| {
            let focus_handle = cx.focus_handle();
            let mut dialog = ActionsDialog::with_config(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}),
                actions,
                theme_arc,
                Self::dictation_history_actions_dialog_config(placeholder),
            );
            dialog.set_match_main_window_background(true);
            dialog
        });

        self.actions_dialog = Some(dialog.clone());

        let app_entity = cx.entity().clone();
        dialog.update(cx, |d, _cx| {
            d.set_on_activation(Self::make_actions_dialog_activation_callback(
                app_entity.clone(),
                ActionsDialogHost::DictationHistory,
            ));
            d.set_on_close(Self::make_actions_window_on_close_callback(
                app_entity,
                ActionsDialogHost::DictationHistory,
                "Dictation history actions closed via escape, focus restored via coordinator",
            ));
        });

        let parent_window_handle = window.window_handle();
        let main_bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());

        Self::spawn_open_actions_window(
            cx,
            parent_window_handle,
            main_bounds,
            display_id,
            dialog,
            crate::actions::WindowPosition::TopCenter,
            "Dictation history actions popup window opened",
            "Failed to open dictation history actions window",
        );

        cx.notify();
    }

    /// Toggle the actions dialog for file search results.
    ///
    /// When a row is selected, shows both row-scoped file actions and
    /// current-directory actions.  When no row is selected but a browsed
    /// directory exists, shows directory-only actions.
    fn toggle_file_search_actions(
        &mut self,
        selected_file: Option<&file_search::FileResult>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::log("KEY", "Toggling file search actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            // Close the actions popup
            self.mark_actions_popup_closed();
            self.file_search_actions_path = None;

            // Close the actions window via spawn
            cx.spawn(async move |_this, cx| {
                cx.update(|cx| {
                    close_actions_window(cx);
                });
            })
            .detach();

            // Use coordinator to restore focus (will pop the overlay and set pending_focus)
            self.pop_focus_overlay(cx);

            // Also directly focus main filter for immediate feedback
            self.focus_main_filter(window, cx);
            logging::log(
                "FOCUS",
                "File search actions closed, focus restored via coordinator",
            );
            cx.notify();
            return;
        }

        // Build current-directory context if browsing a concrete directory
        let dir_path = self.current_file_search_directory_abs();
        let dir_info = dir_path.as_ref().map(|path| {
            let dir_name = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| path.clone());
            crate::actions::FileSearchDirectoryInfo::new(
                path.clone(),
                dir_name,
                self.file_search_sort_mode,
            )
        });

        // Run 14 Pass 1 — story `actions-debounce-builtins-cross-host-live`:
        // when neither a file nor a directory context is available the
        // dialog used to silently close. Now we always open the dialog —
        // `with_file_search_context` will fall through to the global
        // actions block (Pass 3 of Run 13) so the user sees that Cmd+K
        // landed even when the file-search input is empty.

        // Open actions popup
        self.mark_actions_popup_opening();

        // Use coordinator to push overlay - saves current focus state for restore
        self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);

        // CRITICAL: Transfer focus from Input to main focus_handle
        self.focus_handle.focus(window, cx);
        self.gpui_input_focused = false;
        self.focused_input = FocusedInput::ActionsSearch;

        // Store the file path for action handling
        self.file_search_actions_path = selected_file.map(|file| file.path.clone());

        // Create file info from the result
        let file_info = selected_file.map(file_search::FileInfo::from_result);

        // Determine placeholder text — show both scopes when available
        let placeholder_text = match (file_info.as_ref(), dir_info.as_ref()) {
            (Some(file), Some(dir)) => format!("{} · {}", file.name, dir.name),
            (Some(file), None) => file.name.clone(),
            (None, Some(dir)) => dir.name.clone(),
            (None, None) => "Actions".to_string(),
        };

        // Create the dialog entity
        let theme_arc = std::sync::Arc::clone(&self.theme);
        let dialog = cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            let mut dialog = ActionsDialog::with_file_search_context(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}), // Callback handled via main app
                file_info.as_ref(),
                dir_info.as_ref(),
                theme_arc,
            );

            // Match the mini main menu's actions dialog config:
            // search at top, anchor top, centered, icons visible
            dialog.set_config(crate::actions::ActionsDialogConfig {
                search_position: crate::actions::SearchPosition::Top,
                section_style: crate::actions::SectionStyle::Headers,
                anchor: crate::actions::AnchorPosition::Top,
                show_icons: true,
                search_placeholder: Some(placeholder_text),
                show_context_header: false,
                ..crate::actions::ActionsDialogConfig::default()
            });

            dialog.set_match_main_window_background(true);
            dialog
        });

        // Store the dialog entity for keyboard routing
        self.actions_dialog = Some(dialog.clone());

        // Set up the on_close callback to restore focus when escape is pressed in ActionsWindow
        // Match what close_actions_popup does for FileSearch host
        let app_entity = cx.entity().clone();
        dialog.update(cx, |d, _cx| {
            d.set_on_activation(Self::make_actions_dialog_activation_callback(
                app_entity.clone(),
                ActionsDialogHost::FileSearch,
            ));
            d.set_on_close(std::sync::Arc::new(move |cx| {
                let app_entity = app_entity.clone();
                cx.defer(move |cx| {
                    app_entity.update(cx, |app, cx| {
                        if !app.show_actions_popup && app.actions_dialog.is_none() {
                            app.file_search_actions_path = None;
                            return;
                        }

                        app.mark_actions_popup_closed();
                        app.file_search_actions_path = None;
                        // Use coordinator to pop overlay and restore previous focus
                        app.pop_focus_overlay(cx);
                        logging::log(
                            "FOCUS",
                            "File search actions closed via escape, focus restored via coordinator",
                        );
                    });
                });
            }));
        });

        // Get main window bounds and display_id for positioning
        let parent_window_handle = window.window_handle();
        let main_bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());
        logging::log(
            "ACTIONS",
            &format!(
                "Opening file search actions: file={}, dir={}",
                selected_file.map(|f| f.name.as_str()).unwrap_or("none"),
                dir_info.as_ref().map(|d| d.name.as_str()).unwrap_or("none"),
            ),
        );

        // Open the actions window — centered like the mini main menu
        let parent_automation_id = crate::windows::focused_automation_window_id();
        let actions_window_feedback = BuiltInActionsWindowFeedback::FileSearch;
        cx.spawn(async move |_this, cx| {
            cx.update(|cx| {
                match open_actions_window(
                    cx,
                    parent_window_handle,
                    main_bounds,
                    display_id,
                    dialog,
                    crate::actions::WindowPosition::TopCenter,
                    parent_automation_id.as_deref(),
                ) {
                    Ok(_handle) => {
                        logging::log("ACTIONS", actions_window_feedback.opened_log());
                    }
                    Err(e) => {
                        logging::log("ERROR", &actions_window_feedback.failure_log(e));
                    }
                }
            });
        })
        .detach();

        cx.notify();
    }

    /// Toggle the actions dialog for a clipboard history entry
    fn toggle_clipboard_actions(
        &mut self,
        entry: clipboard_history::ClipboardEntryMeta,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::log("KEY", "Toggling clipboard actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            // Close the actions popup
            self.mark_actions_popup_closed();

            // Close the actions window via spawn
            cx.spawn(async move |_this, cx| {
                cx.update(|cx| {
                    close_actions_window(cx);
                });
            })
            .detach();

            // Use coordinator to restore focus (will pop the overlay and set pending_focus)
            self.pop_focus_overlay(cx);

            // Also directly focus main filter for immediate feedback
            self.focus_main_filter(window, cx);
            logging::log(
                "FOCUS",
                "Clipboard actions closed, focus restored via coordinator",
            );
        } else {
            // Open actions popup for the selected clipboard entry
            self.mark_actions_popup_opening();
            self.focused_clipboard_entry_id = Some(entry.id.clone());

            // Use coordinator to push overlay - saves current focus state for restore
            self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);

            // Transfer focus from Input to main focus_handle for actions routing
            self.focus_handle.focus(window, cx);
            self.gpui_input_focused = false;
            self.focused_input = FocusedInput::ActionsSearch;

            let entry_content_type = entry.content_type;
            let entry_info = crate::actions::ClipboardEntryInfo {
                id: entry.id.clone(),
                content_type: entry.content_type,
                pinned: entry.pinned,
                preview: entry.display_preview(),
                image_dimensions: entry.image_width.zip(entry.image_height),
                frontmost_app_name: None,
            };

            // Create the dialog entity
            let theme_arc = std::sync::Arc::clone(&self.theme);
            let entry_placeholder = entry_info.preview.clone();
            let entry_info_for_dialog = entry_info.clone();
            let dialog = cx.new(move |cx| {
                let focus_handle = cx.focus_handle();
                let mut dialog = ActionsDialog::with_clipboard_entry(
                    focus_handle,
                    std::sync::Arc::new(|_action_id| {}), // Callback handled via main app
                    &entry_info_for_dialog,
                    theme_arc,
                );

                // Match the mini main menu's actions dialog config:
                // search at top, anchor top, centered, icons visible
                dialog.set_config(crate::actions::ActionsDialogConfig {
                    search_position: crate::actions::SearchPosition::Top,
                    section_style: crate::actions::SectionStyle::Headers,
                    anchor: crate::actions::AnchorPosition::Top,
                    show_icons: true,
                    search_placeholder: Some(entry_placeholder),
                    show_context_header: false,
                    ..crate::actions::ActionsDialogConfig::default()
                });

                dialog
            });

            // Store the dialog entity for keyboard routing
            self.actions_dialog = Some(dialog.clone());

            // Set up the on_close callback to restore focus when escape is pressed
            let app_entity = cx.entity().clone();
            dialog.update(cx, |d, _cx| {
                d.set_on_activation(Self::make_actions_dialog_activation_callback(
                    app_entity.clone(),
                    ActionsDialogHost::ClipboardHistory,
                ));
                d.set_on_close(std::sync::Arc::new(move |cx| {
                    let app_entity = app_entity.clone();
                    cx.defer(move |cx| {
                        app_entity.update(cx, |app, cx| {
                            if !app.show_actions_popup && app.actions_dialog.is_none() {
                                return;
                            }

                            app.mark_actions_popup_closed();
                            // Use coordinator to pop overlay and restore previous focus
                            app.pop_focus_overlay(cx);
                            logging::log(
                                "FOCUS",
                                "Clipboard actions closed via escape, focus restored via coordinator",
                            );
                        });
                    });
                }));
            });

            // Get main window bounds and display_id for positioning
            let parent_window_handle = window.window_handle();
            let main_bounds = window.bounds();
            let display_id = window.display(cx).map(|d| d.id());
            logging::log(
                "ACTIONS",
                &format!(
                    "Opening clipboard actions for entry: {} (type={:?}, pinned={})",
                    entry.id, entry_content_type, entry.pinned
                ),
            );

            // Open the actions window
            let parent_automation_id = crate::windows::focused_automation_window_id();
            let actions_window_feedback = BuiltInActionsWindowFeedback::ClipboardHistory;
            cx.spawn(async move |_this, cx| {
                cx.update(|cx| {
                    match open_actions_window(
                        cx,
                        parent_window_handle,
                        main_bounds,
                        display_id,
                        dialog,
                        crate::actions::WindowPosition::TopCenter,
                        parent_automation_id.as_deref(),
                    ) {
                        Ok(_handle) => {
                            logging::log("ACTIONS", actions_window_feedback.opened_log());
                        }
                        Err(e) => {
                            logging::log("ERROR", &actions_window_feedback.failure_log(e));
                        }
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    // ------------------------------------------------------------------
    // Flow Desk actions (Conversation Desk ⌘K contract): every focused-item
    // verb lives here, never inline in the desk chrome.
    // ------------------------------------------------------------------

    /// Subject the desk dialog acts on, derived from the live view state so
    /// no stale copy is captured while the popup is open.
    fn flow_desk_actions_subject(&self) -> Option<FlowDeskSubject> {
        match &self.current_view {
            AppView::FlowSessionView { session_id } => Some(FlowDeskSubject::Session(*session_id)),
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                let rows = self.flow_desk_rows(filter);
                match rows.get(*selected_index)? {
                    FlowDeskRow::Session(id) => Some(FlowDeskSubject::Session(*id)),
                    FlowDeskRow::Run(id) => Some(FlowDeskSubject::Run(*id)),
                    FlowDeskRow::Flow(flow) => Some(FlowDeskSubject::Flow(flow.clone())),
                    // Setup affordance rows carry exactly one verb — Enter
                    // already is that verb, so ⌘K has nothing to add.
                    FlowDeskRow::InstallMdflow | FlowDeskRow::InitFlows => None,
                    FlowDeskRow::CreateFlow => Some(FlowDeskSubject::Create),
                }
            }
            _ => None,
        }
    }

    /// Pure: the ⌘K verb list is a function of the subject alone, so the
    /// discoverability tests below can hold it to the bar without an app.
    fn flow_desk_actions_for_dialog(subject: &FlowDeskSubject) -> Vec<crate::actions::Action> {
        use crate::actions::{Action, ActionCategory};
        use crate::designs::icon_variations::IconName;

        match subject {
            FlowDeskSubject::Flow(flow) => {
                let mut actions = vec![
                    Action::new(
                        "flow_desk_converse",
                        "Start Conversation",
                        Some(format!("Talk to {} interactively", flow.friendly_name())),
                        ActionCategory::ScriptContext,
                    )
                    .with_shortcut("↵")
                    .with_section("Flow")
                    .with_icon(IconName::Terminal),
                    Action::new(
                        "flow_desk_run_once",
                        "Run Once in Background",
                        Some("One `--events` run, supervised from the desk".to_string()),
                        ActionCategory::ScriptContext,
                    )
                    .with_shortcut("⇧↵")
                    .with_section("Flow")
                    .with_icon(IconName::PlayFilled),
                    Action::new(
                        "flow_desk_view",
                        "View Flow",
                        Some(
                            "Render the resolved prompt + full config to a page (md render)"
                                .to_string(),
                        ),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Flow")
                    .with_icon(IconName::MagnifyingGlass),
                    Action::new(
                        "flow_desk_edit",
                        "Edit Definition",
                        Some("Open the flow's Markdown in your editor".to_string()),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Definition")
                    .with_icon(IconName::Pencil),
                    Action::new(
                        "flow_desk_reveal",
                        "Reveal Source in Finder",
                        Some(flow.path.clone()),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Definition")
                    .with_icon(IconName::Folder),
                    Action::new(
                        "flow_desk_copy_path",
                        "Copy Definition Path",
                        None,
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Definition")
                    .with_icon(IconName::Copy),
                ];
                if flow.wrapper_command.is_some() {
                    actions.push(
                        Action::new(
                            "flow_desk_copy_command",
                            "Copy Wrapper Command",
                            flow.wrapper_command.clone(),
                            ActionCategory::ScriptContext,
                        )
                        .with_section("Definition")
                        .with_icon(IconName::Copy),
                    );
                }
                actions.push(
                    Action::new(
                        "flow_desk_create",
                        "New Flow",
                        Some("Describe an agent in plain English (md create)".to_string()),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Create")
                    .with_icon(IconName::Plus),
                );
                actions
            }
            FlowDeskSubject::Session(_) => vec![
                Action::new(
                    "flow_desk_session_open",
                    "Open Conversation",
                    Some("Reattach to the same live session".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("↵")
                .with_section("Session")
                .with_icon(IconName::Terminal),
                Action::new(
                    "flow_desk_session_background",
                    "Background",
                    Some("Leave it running and return to the desk".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⌘⇧D")
                .with_section("Session")
                .with_icon(IconName::ArrowDown),
                Action::new(
                    "flow_desk_session_copy_transcript",
                    "Copy Transcript",
                    Some("All turns as Markdown".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_section("Session")
                .with_icon(IconName::Copy),
                Action::new(
                    "flow_desk_create",
                    "New Flow",
                    Some("Describe an agent in plain English (md create)".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_section("Create")
                .with_icon(IconName::Plus),
                Action::new(
                    "flow_desk_session_stop",
                    "Stop Current Turn",
                    Some("Cancel the in-flight turn — the conversation survives".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_section("Danger")
                .with_icon(IconName::Close),
                Action::new(
                    "flow_desk_session_terminate",
                    "Terminate Flow",
                    Some(
                        "Cancel any active turn and permanently end this conversation".to_string(),
                    ),
                    ActionCategory::ScriptContext,
                )
                .with_shortcut("⇧⌘⎋")
                .with_section("Danger")
                .with_icon(IconName::Trash),
            ],
            FlowDeskSubject::Run(id) => {
                let run = crate::flows::run_registry::flow_run_registry().get(*id);
                let mut actions = Vec::new();
                let active = run.as_ref().is_some_and(|run| !run.phase.is_terminal());
                if active {
                    actions.push(
                        Action::new(
                            "flow_desk_run_cancel",
                            "Cancel Run",
                            Some("SIGTERM the run's process group (SIGKILL after 2s)".to_string()),
                            ActionCategory::ScriptContext,
                        )
                        .with_section("Run")
                        .with_icon(IconName::Close),
                    );
                }
                actions.push(
                    Action::new(
                        "flow_desk_run_copy_output",
                        "Copy Output",
                        Some("Interleaved stdout + stderr tail".to_string()),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Run")
                    .with_icon(IconName::Copy),
                );
                actions.push(
                    Action::new(
                        "flow_desk_runs_clear_finished",
                        "Clear Finished Runs",
                        Some("Remove completed/failed/cancelled runs from the desk".to_string()),
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Run")
                    .with_icon(IconName::Trash),
                );
                actions
            }
            FlowDeskSubject::Create => vec![Action::new(
                "flow_desk_create",
                "Create a Flow",
                Some("Describe an agent in plain English (md create)".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("↵")
            .with_section("Flow")
            .with_icon(IconName::Plus)],
        }
    }

    pub(crate) fn toggle_flow_desk_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        logging::log("KEY", "Toggling Flow Desk actions popup");

        if self.show_actions_popup || is_actions_window_open() {
            self.close_actions_popup(ActionsDialogHost::FlowDesk, window, cx);
            return;
        }

        let Some(subject) = self.flow_desk_actions_subject() else {
            logging::log("ACTIONS", "Flow Desk actions ignored: no subject");
            return;
        };

        self.mark_actions_popup_opening();
        self.push_focus_overlay(focus_coordinator::FocusRequest::actions_dialog(), cx);
        self.focus_handle.focus(window, cx);
        self.gpui_input_focused = false;
        self.focused_input = FocusedInput::ActionsSearch;

        let theme_arc = std::sync::Arc::clone(&self.theme);
        let actions = Self::flow_desk_actions_for_dialog(&subject);
        let placeholder = match &subject {
            FlowDeskSubject::Flow(flow) => flow.friendly_name(),
            FlowDeskSubject::Session(_) => "Session actions".to_string(),
            FlowDeskSubject::Run(id) => crate::flows::run_registry::flow_run_registry()
                .get(*id)
                .map(|run| {
                    format!(
                        "{} run",
                        crate::flows::model::friendly_flow_name(&run.flow_name)
                    )
                })
                .unwrap_or_else(|| "Run actions".to_string()),
            FlowDeskSubject::Create => "Create a flow".to_string(),
        };
        let dialog = cx.new(move |cx| {
            let focus_handle = cx.focus_handle();
            let mut dialog = ActionsDialog::with_config(
                focus_handle,
                std::sync::Arc::new(|_action_id| {}),
                actions,
                theme_arc,
                crate::actions::ActionsDialogConfig {
                    search_position: crate::actions::SearchPosition::Top,
                    section_style: crate::actions::SectionStyle::Headers,
                    anchor: crate::actions::AnchorPosition::Top,
                    show_icons: true,
                    search_placeholder: Some(placeholder),
                    show_context_header: false,
                    ..crate::actions::ActionsDialogConfig::default()
                },
            );
            dialog.set_match_main_window_background(true);
            dialog
        });

        self.actions_dialog = Some(dialog.clone());

        let app_entity = cx.entity().clone();
        dialog.update(cx, |d, _cx| {
            d.set_on_activation(Self::make_actions_dialog_activation_callback(
                app_entity.clone(),
                ActionsDialogHost::FlowDesk,
            ));
            d.set_on_close(Self::make_actions_window_on_close_callback(
                app_entity,
                ActionsDialogHost::FlowDesk,
                "Flow Desk actions closed via escape, focus restored via coordinator",
            ));
        });

        let parent_window_handle = window.window_handle();
        let main_bounds = window.bounds();
        let display_id = window.display(cx).map(|d| d.id());

        Self::spawn_open_actions_window(
            cx,
            parent_window_handle,
            main_bounds,
            display_id,
            dialog,
            crate::actions::WindowPosition::TopCenter,
            "Flow Desk actions popup window opened",
            "Failed to open Flow Desk actions window",
        );

        cx.notify();
    }

    pub(crate) fn execute_flow_desk_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subject = self.flow_desk_actions_subject();
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_desk_action",
            action_id = %action_id,
            "Executing Flow Desk action"
        );
        self.close_actions_popup(ActionsDialogHost::FlowDesk, window, cx);

        match (action_id, subject) {
            ("flow_desk_converse", Some(FlowDeskSubject::Flow(flow))) => {
                self.resume_or_start_flow_session(&flow, None, cx);
            }
            ("flow_desk_run_once", Some(FlowDeskSubject::Flow(flow))) => {
                self.flow_desk_run_once(&flow, cx);
            }
            ("flow_desk_view", Some(FlowDeskSubject::Flow(flow))) => {
                self.flow_desk_view_flow(&flow);
            }
            ("flow_desk_edit", Some(FlowDeskSubject::Flow(flow))) => {
                let _ = std::process::Command::new("open")
                    .arg("-t")
                    .arg(&flow.path)
                    .spawn();
            }
            ("flow_desk_reveal", Some(FlowDeskSubject::Flow(flow))) => {
                let _ = std::process::Command::new("open")
                    .arg("-R")
                    .arg(&flow.path)
                    .spawn();
            }
            ("flow_desk_copy_path", Some(FlowDeskSubject::Flow(flow))) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(flow.path.clone()));
            }
            ("flow_desk_copy_command", Some(FlowDeskSubject::Flow(flow))) => {
                if let Some(wrapper) = flow.wrapper_command {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(wrapper));
                }
            }
            ("flow_desk_session_open", Some(FlowDeskSubject::Session(id))) => {
                self.open_flow_session(id, cx);
            }
            ("flow_desk_session_background", _) => {
                self.background_flow_session(window, cx);
            }
            ("flow_desk_session_copy_transcript", Some(FlowDeskSubject::Session(id))) => {
                if let Some((meta, _)) = self.flow_sessions.iter().find(|(meta, _)| meta.id == id) {
                    let transcript = meta
                        .turns
                        .iter()
                        .map(|turn| format!("**You:** {}\n\n{}", turn.user, turn.assistant))
                        .collect::<Vec<_>>()
                        .join("\n\n---\n\n");
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(transcript));
                }
            }
            ("flow_desk_session_stop", Some(FlowDeskSubject::Session(id))) => {
                self.stop_flow_session(id, cx);
            }
            ("flow_desk_session_terminate", Some(FlowDeskSubject::Session(id))) => {
                self.terminate_flow_session(id, window, cx);
            }
            ("flow_desk_run_cancel", Some(FlowDeskSubject::Run(id))) => {
                crate::flows::runner::cancel_run(id);
                self.toast_manager.push(
                    crate::components::toast::Toast::success(
                        "Cancelling run…".to_string(),
                        &self.theme,
                    )
                    .duration_ms(Some(1500)),
                );
            }
            ("flow_desk_run_copy_output", Some(FlowDeskSubject::Run(id))) => {
                if let Some(run) = crate::flows::run_registry::flow_run_registry().get(id) {
                    let output = run.merged_tail.lines().collect::<Vec<&str>>().join("\n");
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(output));
                }
            }
            ("flow_desk_runs_clear_finished", _) => {
                crate::flows::run_registry::flow_run_registry().clear_finished();
            }
            ("flow_desk_create", _) => {
                self.start_flow_create_session(cx);
            }
            (other, _) => {
                tracing::warn!(
                    target: "script_kit::flows",
                    event = "flow_desk_action_unknown",
                    action_id = %other,
                    "Unknown Flow Desk action id"
                );
            }
        }
    }

    /// "View Flow": render the flow's resolved prompt + full config to a
    /// self-contained HTML page via `md render <flow> --open` (FREE — no
    /// engine call; mdflow opens the page in the default browser). Runs off
    /// the main thread; the outcome is logged so devtools receipts can
    /// assert on it.
    fn flow_desk_view_flow(&self, flow: &crate::flows::model::FlowDescriptor) {
        let path = flow.path.clone();
        let cwd = self.flow_ux_cwd();
        std::thread::Builder::new()
            .name("flow-desk-view".into())
            .spawn(move || {
                let Some(binary) = crate::flows::catalog::mdflow_binary() else {
                    tracing::warn!(
                        target: "script_kit::flows",
                        event = "flow_desk_view_failed",
                        path = %path,
                        error = "mdflow CLI not found on PATH",
                        "View Flow could not run md render"
                    );
                    return;
                };
                match std::process::Command::new(binary)
                    .arg("render")
                    .arg(&path)
                    .arg("--open")
                    .current_dir(&cwd)
                    .output()
                {
                    Ok(output) if output.status.success() => {
                        let page = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        tracing::info!(
                            target: "script_kit::flows",
                            event = "flow_desk_view_rendered",
                            path = %path,
                            page = %page,
                            "Rendered flow view page"
                        );
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let first = stderr.lines().next().unwrap_or("md render failed");
                        tracing::warn!(
                            target: "script_kit::flows",
                            event = "flow_desk_view_failed",
                            path = %path,
                            error = %first,
                            "md render failed"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            target: "script_kit::flows",
                            event = "flow_desk_view_failed",
                            path = %path,
                            error = %err,
                            "md render failed to spawn"
                        );
                    }
                }
            })
            .ok();
    }
}

#[cfg(test)]
mod on_close_reentrancy_tests {
    use std::fs;

    #[test]
    fn test_render_builtins_actions_clipboard_popup_uses_mini_menu_contract() {
        let source = fs::read_to_string("src/render_builtins/actions.rs")
            .expect("Failed to read src/render_builtins/actions.rs");

        let clipboard_fn = source
            .split("fn toggle_clipboard_actions(")
            .nth(1)
            .expect("missing toggle_clipboard_actions");

        assert!(
            clipboard_fn.contains("dialog.set_config(crate::actions::ActionsDialogConfig {"),
            "clipboard actions should set an explicit ActionsDialogConfig"
        );
        assert!(
            clipboard_fn.contains("search_position: crate::actions::SearchPosition::Top"),
            "clipboard actions should place search at the top"
        );
        assert!(
            clipboard_fn.contains("section_style: crate::actions::SectionStyle::Headers"),
            "clipboard actions should use section headers"
        );
        assert!(
            clipboard_fn.contains("anchor: crate::actions::AnchorPosition::Top"),
            "clipboard actions should anchor to the top"
        );
        assert!(
            clipboard_fn.contains("show_icons: true"),
            "clipboard actions should show icons"
        );
        assert!(
            clipboard_fn.contains("show_context_header: false"),
            "clipboard actions should hide the context header"
        );
        assert!(
            clipboard_fn.contains("crate::actions::WindowPosition::TopCenter"),
            "clipboard actions should open in the top-center mini-menu position"
        );
        assert!(
            !clipboard_fn.contains("crate::actions::WindowPosition::BottomRight"),
            "clipboard actions should not open in the bottom-right position"
        );
    }
}

#[cfg(test)]
mod flow_desk_create_discoverability_tests {
    //! Actions-menu leg of the discoverability bar (paired with
    //! tests/launcher_discoverability_contract.rs): every Flow Desk ⌘K
    //! subject must surface the create-flow affordance, and surfacing it
    //! must not degrade the menu (ids stay unique, Danger stays last).

    use super::{FlowDeskSubject, ScriptListApp};
    use crate::flows::model::{FlowDescriptor, FlowSource};

    fn sample_flow() -> FlowDescriptor {
        FlowDescriptor {
            id: "project:flow-gmail".to_string(),
            path: "/test/flows/flow-gmail.md".to_string(),
            source: FlowSource::Project,
            name: "flow-gmail".to_string(),
            description: Some("Triage email".to_string()),
            engine: "codex".to_string(),
            engine_source: None,
            inputs: Vec::new(),
            is_workflow: false,
            interactive: false,
            mtime_ms: 0,
            origin: Some("repo flows/".to_string()),
            wrapper_command: None,
        }
    }

    fn subjects() -> Vec<(&'static str, FlowDeskSubject)> {
        vec![
            ("flow", FlowDeskSubject::Flow(sample_flow())),
            ("session", FlowDeskSubject::Session(7)),
            ("create", FlowDeskSubject::Create),
        ]
    }

    #[test]
    fn every_flow_desk_subject_surfaces_the_create_affordance() {
        for (label, subject) in subjects() {
            let actions = ScriptListApp::flow_desk_actions_for_dialog(&subject);
            assert!(
                actions.iter().any(|action| action.id == "flow_desk_create"),
                "the {label} subject must surface the flow_desk_create action"
            );
        }
    }

    #[test]
    fn action_ids_stay_unique_per_subject() {
        for (label, subject) in subjects() {
            let actions = ScriptListApp::flow_desk_actions_for_dialog(&subject);
            let mut ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();
            let before = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(
                before,
                ids.len(),
                "the {label} subject repeats an action id"
            );
        }
    }

    #[test]
    fn danger_actions_stay_last_for_sessions() {
        let actions = ScriptListApp::flow_desk_actions_for_dialog(&FlowDeskSubject::Session(7));
        let first_danger = actions
            .iter()
            .position(|action| action.section.as_deref() == Some("Danger"))
            .expect("session subject must keep its Danger section");
        assert!(
            actions[first_danger..]
                .iter()
                .all(|action| action.section.as_deref() == Some("Danger")),
            "Danger must remain the trailing section — new verbs go before it"
        );
    }
}
