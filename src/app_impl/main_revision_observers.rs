use super::*;

impl ScriptListApp {
    /// These epochs belong to mutation owners and are readable before GPUI
    /// observers run or another frame paints. Never advance authority here.
    pub(crate) fn owned_child_semantic_revision(&self, cx: &App) -> u64 {
        match &self.current_view {
            AppView::ArgPrompt { .. } | AppView::MiniPrompt { .. } | AppView::MicroPrompt { .. } => self.arg_selection_revision,
            AppView::TemplatePrompt { entity, .. } => entity.read(cx).dictation_input_revision(cx),
            AppView::SelectPrompt { entity, .. } => entity.read(cx).dictation_input_revision(cx),
            AppView::PathPrompt { entity, .. } => entity.read(cx).dictation_input_revision(cx),
            AppView::EnvPrompt { entity, .. } => entity.read(cx).dictation_input_revision(cx),
            AppView::AgentChatView { entity } => entity.read(cx).semantic_revision(cx),
            AppView::DayPage { entity } => entity.read(cx).semantic_revision(cx),
            _ => 0,
        }
    }

    fn owned_child_semantic_token(&self, cx: &App) -> (u64, u64, u64) {
        let prompt = self.prompt_semantic_token(cx).unwrap_or(0);
        match &self.current_view {
            AppView::DayPage { entity } => (prompt, entity.read(cx).semantic_revision(cx), entity.read(cx).document_revision()),
            AppView::AgentChatView { entity } => (prompt, entity.read(cx).semantic_token(cx), 0),
            AppView::ChatPrompt { entity, .. } => (prompt, entity.read(cx).semantic_token(cx), 0),
            AppView::FlowSessionView { session_id } => self.conversations.flow_sessions.iter()
                .find(|(meta, _)| meta.id == *session_id)
                .map(|(_, entity)| (prompt, entity.read(cx).semantic_token(cx), 0)).unwrap_or((prompt, 0, 0)),
            _ => (prompt, 0, 0),
        }
    }

    fn observe_owned_child_semantic_change(&mut self, cx: &mut Context<Self>) {
        let next = self.owned_child_semantic_token(cx);
        if self.owned_child_semantic_value != Some(next) {
            self.owned_child_semantic_value = Some(next);
            self.mark_main_data_changed();
            cx.notify();
        }
    }

    /// Retain only the active production root's subscriptions. Notifications
    /// are wakeups; mutation-owner epochs preserve ABA across coalesced notifications.
    pub(crate) fn bind_owned_surface_revision_observers(&mut self, cx: &mut Context<Self>) {
        self.note_main_route_changed();
        let generation = self.main_revisions.surface_generation;
        if self.owned_observed_surface_generation == Some(generation) { return; }
        self.owned_surface_subscriptions.clear();
        self.owned_observed_surface_generation = Some(generation);
        self.owned_child_semantic_value = Some(self.owned_child_semantic_token(cx));
        let prompt_instance = self.prompt_completion.as_ref()
            .filter(|binding| !binding.observation().retired && self.prompt_observation(cx).is_some())
            .map(|binding| binding.instance().clone());
        macro_rules! observe {
            ($entity:expr) => {{
                let entity = $entity.clone();
                let prompt_instance = prompt_instance.clone();
                self.owned_surface_subscriptions.push(cx.observe(&entity, move |this, _, cx| {
                    if this.main_revisions.surface_generation != generation { return; }
                    if let Some(instance) = prompt_instance.as_ref() {
                        if !this.prompt_completion.as_ref().is_some_and(|binding|
                            binding.instance() == instance && !binding.observation().retired) { return; }
                    }
                    this.observe_owned_child_semantic_change(cx);
                }));
            }};
        }
        match &self.current_view {
            AppView::DivPrompt { entity, .. } => observe!(entity),
            AppView::FormPrompt { entity, .. } => {
                observe!(entity);
                let fields: Vec<_> = entity.read(cx).fields.iter().map(|(_, field)| field.clone()).collect();
                for field in fields {
                    match field {
                        crate::form_prompt::FormFieldEntity::TextField(entity) => observe!(entity),
                        crate::form_prompt::FormFieldEntity::TextArea(entity) => observe!(entity),
                        crate::form_prompt::FormFieldEntity::Checkbox(entity) => observe!(entity),
                    }
                }
            }
            AppView::TermPrompt { entity, .. } | AppView::QuickTerminalView { entity } => observe!(entity),
            AppView::EditorPrompt { entity, .. } | AppView::ScratchPadView { entity, .. } => observe!(entity),
            AppView::SelectPrompt { entity, .. } => observe!(entity),
            AppView::PathPrompt { entity, .. } => observe!(entity),
            AppView::EnvPrompt { entity, .. } => observe!(entity),
            AppView::DropPrompt { entity, .. } => observe!(entity),
            AppView::TemplatePrompt { entity, .. } => observe!(entity),
            AppView::HotkeyPrompt { entity, .. } => observe!(entity),
            AppView::ChatPrompt { entity, .. } => observe!(entity),
            AppView::NamingPrompt { entity, .. } => observe!(entity),
            AppView::WebcamView { entity } => observe!(entity),
            AppView::AgentChatView { entity } => observe!(entity),
            AppView::DayPage { entity } => observe!(entity),
            AppView::FlowSessionView { session_id } => {
                if let Some((_, entity)) = self.conversations.flow_sessions.iter().find(|(meta, _)| meta.id == *session_id) { observe!(entity); }
            }
            _ => {}
        }
    }
}
