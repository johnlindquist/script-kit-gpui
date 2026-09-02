use super::*;

fn focus_for_about_restore(view: &AppView) -> (FocusTarget, FocusedInput) {
    match view {
        AppView::ActionsDialog => (FocusTarget::ActionsDialog, FocusedInput::ActionsSearch),
        view if matches!(
            view.surface_contract().vocabulary.input_ownership,
            LauncherSurfaceInputOwnership::LauncherFilter
        ) =>
        {
            (FocusTarget::MainFilter, FocusedInput::MainFilter)
        }
        _ => (FocusTarget::AppRoot, FocusedInput::None),
    }
}

impl ScriptListApp {
    pub(crate) fn open_about_surface(
        &mut self,
        update_state: std::sync::Arc<std::sync::RwLock<crate::updates::UpdateState>>,
        cx: &mut Context<Self>,
    ) {
        let previous = match &self.current_view {
            AppView::About { previous, .. } => previous.clone(),
            _ => Box::new(self.current_view.clone()),
        };

        self.transition_current_view_and_rekey_main_automation_surface(AppView::About {
            previous,
            state: crate::about::AboutState::new(),
            update_state,
        });
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::AppRoot);
        cx.notify();
    }

    pub(crate) fn dismiss_about(&mut self, cx: &mut Context<Self>) {
        let previous = match std::mem::replace(&mut self.current_view, AppView::ScriptList) {
            AppView::About { previous, .. } => *previous,
            other => {
                self.current_view = other;
                return;
            }
        };

        self.transition_current_view_and_rekey_main_automation_surface(previous);
        let (focus_target, focused_input) = focus_for_about_restore(&self.current_view);
        self.pending_focus = Some(focus_target);
        self.focused_input = focused_input;
        if matches!(self.current_view, AppView::ScriptList) {
            self.flush_pending_main_menu_query(cx);
        }
        cx.notify();
    }

    pub(crate) fn toggle_about_acknowledgements(&mut self, cx: &mut Context<Self>) {
        if let AppView::About { state, .. } = &mut self.current_view {
            state.acks_open = !state.acks_open;
            cx.notify();
        }
    }

    /// Push an in-window confirm prompt as a state of the main window.
    ///
    /// Replaces the current launcher view; restored to the same view on
    /// confirm/cancel via [`Self::resolve_confirm_prompt`].
    pub(crate) fn open_confirm_prompt(
        &mut self,
        options: crate::confirm::ParentConfirmOptions,
        sender: async_channel::Sender<bool>,
        cx: &mut Context<Self>,
    ) {
        let previous = match &self.current_view {
            AppView::ConfirmPrompt { previous, .. } => previous.clone(),
            _ => Box::new(self.current_view.clone()),
        };
        self.transition_current_view_and_rekey_main_automation_surface(AppView::ConfirmPrompt {
            options,
            sender,
            focused_button: ConfirmFocusedButton::default(),
            previous,
        });
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::AppRoot);
        cx.notify();
    }
}
