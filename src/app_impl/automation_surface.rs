use super::*;

impl ScriptListApp {
    pub(crate) fn note_main_route_changed(&mut self) {
        if !self.main_services.is_production() {
            self.note_owned_inline_semantics_changed();
        }
        let (id, entity, session) = match &self.current_view {
            AppView::ArgPrompt { id, .. }
            | AppView::MiniPrompt { id, .. }
            | AppView::MicroPrompt { id, .. } => (id.as_str(), None, 0),
            AppView::DivPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::FormPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::TermPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::EditorPrompt { id, entity, .. } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::SelectPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::PathPrompt { id, entity, .. } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::EnvPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::DropPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::TemplatePrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::HotkeyPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::ChatPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::NamingPrompt { id, entity } => (id.as_str(), Some(entity.entity_id()), 0),
            AppView::ScratchPadView { entity, .. } => ("", Some(entity.entity_id()), 0),
            AppView::QuickTerminalView { entity } => ("", Some(entity.entity_id()), 0),
            AppView::WebcamView { entity } => ("", Some(entity.entity_id()), 0),
            AppView::AgentChatView { entity } => ("", Some(entity.entity_id()), 0),
            AppView::DayPage { entity } => ("", Some(entity.entity_id()), 0),
            AppView::FlowSessionView { session_id } => ("", None, *session_id),
            _ => ("", None, 0),
        };
        let variant = self.current_view.app_view_variant();
        let previous = &self.main_revision_route;
        if previous.0 != variant
            || previous.1 != id
            || previous.2 != entity
            || previous.3 != session
        {
            if previous.0 == "ScriptList" || variant == "ScriptList" {
                // Retire immediately at the route owner, not on a later render.
                // Returning with equal text must never revive a departed query.
                self.root_search.retire_query_owner();
                self.filter_coalescer.reset();
            }
            self.main_revision_route = (variant, id.to_owned(), entity, session);
            self.mark_main_surface_changed();
        }
    }

    fn note_owned_inline_semantics_changed(&mut self) {
        use std::hash::{Hash, Hasher};
        let mut token = std::collections::hash_map::DefaultHasher::new();
        self.filter_text.hash(&mut token);
        self.selected_index.hash(&mut token);
        match &self.current_view {
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            }
            | AppView::AppLauncherView {
                filter,
                selected_index,
            }
            | AppView::WindowSwitcherView {
                filter,
                selected_index,
            }
            | AppView::BrowserTabsView {
                filter,
                selected_index,
            }
            | AppView::ProfileSearchView {
                filter,
                selected_index,
            }
            | AppView::ThemeChooserView {
                filter,
                selected_index,
            }
            | AppView::EmojiPickerView {
                filter,
                selected_index,
                ..
            }
            | AppView::SdkReferenceView {
                filter,
                selected_index,
                ..
            }
            | AppView::TipsView {
                filter,
                selected_index,
                ..
            }
            | AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                ..
            }
            | AppView::MigrateV1View {
                filter,
                selected_index,
                ..
            }
            | AppView::InstalledKitsView {
                filter,
                selected_index,
                ..
            }
            | AppView::ProcessManagerView {
                filter,
                selected_index,
                ..
            }
            | AppView::SearchAiPresetsView {
                filter,
                selected_index,
            }
            | AppView::FlowUxView {
                filter,
                selected_index,
                ..
            }
            | AppView::SettingsView {
                filter,
                selected_index,
            }
            | AppView::FavoritesBrowseView {
                filter,
                selected_index,
            }
            | AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            }
            | AppView::AgentChatHistoryView {
                filter,
                selected_index,
            }
            | AppView::BrowserHistoryView {
                filter,
                selected_index,
            }
            | AppView::DictationHistoryView {
                filter,
                selected_index,
                ..
            } => {
                filter.hash(&mut token);
                selected_index.hash(&mut token);
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            }
            | AppView::BrowseKitsView {
                query,
                selected_index,
                ..
            } => {
                query.hash(&mut token);
                selected_index.hash(&mut token);
            }
            AppView::PermissionsWizardView { selected_index } => selected_index.hash(&mut token),
            AppView::CreateAiPresetView {
                name,
                system_prompt,
                model,
                active_field,
            } => {
                name.hash(&mut token);
                system_prompt.hash(&mut token);
                model.hash(&mut token);
                active_field.hash(&mut token);
            }
            AppView::NotesBrowseView { search } => {
                search.generation.hash(&mut token);
                format!("{:?}", search.selected_id).hash(&mut token);
            }
            _ => {}
        }
        let next = token.finish();
        if self
            .main_inline_semantic_token
            .replace(next)
            .is_some_and(|previous| previous != next)
        {
            self.mark_main_data_changed();
        }
    }

    /// Switch the main route and re-key the main automation `semanticSurface`
    /// from the new [`AppView`] contract.
    pub(crate) fn transition_current_view_and_rekey_main_automation_surface(
        &mut self,
        next_view: AppView,
    ) -> bool {
        self.current_view = next_view;
        self.note_main_route_changed();
        self.rekey_main_automation_surface_from_current_view()
    }

    /// Restore a previously captured main route and focus target.
    ///
    /// This deliberately does not re-key automation or emit notifications.
    /// Callers that close child windows or Agent Chat surfaces still own those
    /// side-effects in their local route contract.
    pub(crate) fn restore_current_view_with_focus(
        &mut self,
        next_view: AppView,
        focus_target: FocusTarget,
    ) {
        self.current_view = next_view;
        self.note_main_route_changed();
        self.pending_focus = Some(focus_target);
        self.focused_input = match focus_target {
            FocusTarget::MainFilter => FocusedInput::MainFilter,
            FocusTarget::ActionsDialog => FocusedInput::ActionsSearch,
            _ => FocusedInput::None,
        };
    }

    /// Return the main route to ScriptList and target the shared filter input.
    ///
    /// Use this for ScriptList entries that already handle their own caches,
    /// sizing, and notifications locally. The helper still re-keys the main
    /// automation surface because the view/focus pair is observable by agents.
    pub(crate) fn show_script_list_with_main_filter_focus(&mut self) -> bool {
        self.restore_current_view_with_focus(AppView::ScriptList, FocusTarget::MainFilter);
        self.rekey_main_automation_surface_from_current_view()
    }

    /// Log top-level launcher view transitions once per active variant.
    ///
    /// View assignment is intentionally spread across route owners. Sampling at
    /// render time records the observable surface without logging hot-path
    /// selection/filter churn.
    pub(crate) fn log_current_view_transition_if_changed(&mut self, source: &'static str) {
        self.note_main_route_changed();
        let current_view = self.current_view.app_view_variant();
        if self.last_logged_app_view_variant == Some(current_view) {
            return;
        }

        let previous_view = self.last_logged_app_view_variant.unwrap_or("unknown");
        self.last_logged_app_view_variant = Some(current_view);
        let contract = self.current_view.surface_contract();
        tracing::info!(
            event_type = "main_view_transition",
            source,
            previous_view,
            current_view,
            surface_kind = ?self.current_view.surface_kind(),
            native_footer_surface = ?self.current_view.native_footer_surface(),
            surface_family = ?contract.vocabulary.family,
            input_ownership = ?contract.vocabulary.input_ownership,
            preview_role = ?contract.vocabulary.preview_role,
            focus_policy = ?contract.focus_policy,
            keyboard_policy = ?contract.keyboard_policy,
            actions_policy = ?contract.actions_policy,
            proof_policy = ?contract.proof_policy,
            visual_policy = ?contract.visual_policy,
            automation_semantic_surface = contract.automation_semantic_surface,
            "Main view transition"
        );
    }

    /// Re-key the main window automation `semanticSurface` from the active
    /// `AppView` contract without replacing the whole automation window record.
    pub(crate) fn rekey_main_automation_surface_from_current_view(&self) -> bool {
        let semantic_surface = crate::semantic_surface_for_main_view(&self.current_view);
        crate::windows::update_automation_semantic_surface("main", semantic_surface)
    }
}
