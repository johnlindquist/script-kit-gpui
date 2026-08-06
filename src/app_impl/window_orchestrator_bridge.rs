use super::*;

impl ScriptListApp {
    fn focus_target_for_orchestrator_token(
        &self,
        token: crate::window_orchestrator::FocusToken,
    ) -> Option<FocusTarget> {
        match token {
            crate::window_orchestrator::FocusToken::MainFilter => Some(FocusTarget::MainFilter),
            crate::window_orchestrator::FocusToken::PromptInput => Some(match &self.current_view {
                AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::BrowserTabsView { .. }
                | AppView::FileSearchView { .. }
                | AppView::ProfileSearchView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::BrowseKitsView { .. }
                | AppView::MigrateV1View { .. }
                | AppView::InstalledKitsView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::FlowUxView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::CreateAiPresetView { .. }
                | AppView::SettingsView { .. }
                | AppView::FavoritesBrowseView { .. }
                | AppView::AgentChatHistoryView { .. }
                | AppView::BrowserHistoryView { .. }
                | AppView::DictationHistoryView { .. }
                | AppView::NotesBrowseView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::CreationFeedback { .. }
                | AppView::ScriptIssuesView { .. }
                | AppView::SdkReferenceView { .. }
                | AppView::TipsView { .. }
                | AppView::ScriptTemplateCatalogView { .. }
                | AppView::ActionsDialog => FocusTarget::MainFilter,
                AppView::About { .. } => FocusTarget::AppRoot,
                AppView::ArgPrompt { .. }
                | AppView::MiniPrompt { .. }
                | AppView::MicroPrompt { .. }
                | AppView::DivPrompt { .. }
                | AppView::HotkeyPrompt { .. }
                | AppView::WebcamView { .. } => FocusTarget::AppRoot,
                AppView::FormPrompt { .. } => FocusTarget::FormPrompt,
                AppView::EditorPrompt { .. } | AppView::ScratchPadView { .. } => {
                    FocusTarget::EditorPrompt
                }
                AppView::SelectPrompt { .. } => FocusTarget::SelectPrompt,
                AppView::PathPrompt { .. } => FocusTarget::PathPrompt,
                AppView::EnvPrompt { .. } => FocusTarget::EnvPrompt,
                AppView::DropPrompt { .. } => FocusTarget::DropPrompt,
                AppView::TemplatePrompt { .. } => FocusTarget::TemplatePrompt,
                AppView::TermPrompt { .. } | AppView::QuickTerminalView { .. } => {
                    FocusTarget::TermPrompt
                }
                AppView::ChatPrompt { .. } => FocusTarget::ChatPrompt,
                // Flow sessions compose in the shared MAIN input.
                AppView::FlowSessionView { .. } => FocusTarget::MainFilter,
                AppView::AgentChatView { .. } => FocusTarget::AgentChat,
                AppView::DayPage { .. } => FocusTarget::EditorPrompt,
                AppView::NamingPrompt { .. } => FocusTarget::NamingPrompt,
                AppView::ConfirmPrompt { .. } => FocusTarget::AppRoot,
                AppView::PermissionsWizardView { .. } => FocusTarget::AppRoot,
            }),
            crate::window_orchestrator::FocusToken::ChatComposer => Some(FocusTarget::AgentChat),
            crate::window_orchestrator::FocusToken::TermInput => Some(FocusTarget::TermPrompt),
            crate::window_orchestrator::FocusToken::NotesEditor
            | crate::window_orchestrator::FocusToken::DetachedAiComposer
            | crate::window_orchestrator::FocusToken::None => None,
        }
    }

    /// Dispatch a window event through the orchestrator state machine,
    /// then execute the resulting commands.
    ///
    /// Must be called from an entity update context (i.e., inside
    /// `app_entity.update(cx, |view, cx| { ... })`).
    ///
    /// Platform calls that trigger AppKit delegate callbacks are deferred
    /// via `cx.spawn()` to avoid `RefCell` reentrancy panics.
    pub(crate) fn dispatch_window_event(
        &mut self,
        event: crate::window_orchestrator::WindowEvent,
        cx: &mut Context<Self>,
    ) {
        let restore_dictation_origin = matches!(
            &event,
            crate::window_orchestrator::WindowEvent::AbortDictation
        );
        let commands = self.window_orchestrator.dispatch(event);
        if commands.is_empty() {
            return;
        }

        tracing::debug!(
            category = "ORCHESTRATOR",
            count = commands.len(),
            "Dispatching window commands"
        );

        // Spawn command execution to avoid RefCell conflicts — platform calls
        // like orderOut:/makeKeyWindow trigger synchronous delegate callbacks
        // that re-enter GPUI.
        cx.spawn({
            let commands = commands.clone();
            async move |this, cx| {
                cx.update(|cx| {
                    crate::window_orchestrator::executor::execute_commands(&commands, cx);
                });

                let _ = this.update(cx, |this, cx| {
                    let pending_focus = commands.iter().rev().find_map(|command| {
                        if let crate::window_orchestrator::WindowCommand::FocusMain(token) = command
                        {
                            this.focus_target_for_orchestrator_token(*token)
                        } else {
                            None
                        }
                    });

                    if let Some(target) = pending_focus {
                        tracing::info!(
                            category = "ORCHESTRATOR",
                            ?target,
                            "Queued main focus after window command execution"
                        );
                        this.pending_focus = Some(target);
                        cx.notify();
                    }
                    if restore_dictation_origin {
                        let _ = this.restore_dictation_return_origin(cx);
                    }
                });
            }
        })
        .detach();
    }
}
