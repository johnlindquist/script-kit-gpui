//! Typed seeds for the production prompt owners. Construction never reveals a
//! native window and completion never borrows the automation RPC response lane.

use std::path::PathBuf;
use std::sync::{mpsc::SyncSender, Arc};

use crate::prompt_completion::{
    PromptCompletionBinding, PromptCompletionObservation, PromptInstance, PromptOutcome,
    SubmissionError,
};
use crate::protocol::{Choice, Field, Message, ProtocolAction, SubmitValue};
use crate::{AppView, FocusTarget, FocusedInput, ScriptListApp};
use gpui::{px, App, AppContext, Context, Window};

pub(crate) struct PromptSeedCommon {
    pub completion: PromptCompletionBinding,
    pub actions: Option<Vec<ProtocolAction>>,
}
impl PromptSeedCommon {
    pub(crate) fn sdk(
        id: String,
        actions: Option<Vec<ProtocolAction>>,
        sender: Option<SyncSender<Message>>,
    ) -> Self {
        Self {
            completion: PromptCompletionBinding::sdk(id, sender),
            actions,
        }
    }
    pub(crate) fn local(id: &str) -> Self {
        Self {
            completion: PromptCompletionBinding::local(id.to_string()),
            actions: None,
        }
    }
    pub(crate) fn naming(id: String, sender: SyncSender<Option<String>>) -> Self {
        Self {
            completion: PromptCompletionBinding::naming(id, sender),
            actions: None,
        }
    }
}

pub(crate) struct ChoicePromptSeed {
    pub common: PromptSeedCommon,
    pub placeholder: String,
    pub choices: Vec<Choice>,
    pub input: String,
}
pub(crate) struct DivPromptSeed {
    pub common: PromptSeedCommon,
    pub html: String,
    pub options: crate::prompts::ContainerOptions,
}
pub(crate) struct FormPromptSeed {
    pub common: PromptSeedCommon,
    pub html: String,
}
pub(crate) struct FieldsPromptSeed {
    pub common: PromptSeedCommon,
    pub fields: Vec<Field>,
}
pub(crate) struct EditorPromptSeed {
    pub common: PromptSeedCommon,
    pub content: String,
    pub language: String,
    pub template: Option<String>,
}
pub(crate) struct SelectPromptSeed {
    pub common: PromptSeedCommon,
    pub placeholder: Option<String>,
    pub choices: Vec<Choice>,
    pub multiple: bool,
    pub disabled: Vec<usize>,
}
pub(crate) enum PathSource {
    Production(Option<String>),
    OwnedDirectory(PathBuf),
}
pub(crate) struct PathPromptSeed {
    pub common: PromptSeedCommon,
    pub source: PathSource,
    pub hint: Option<String>,
}
pub(crate) struct EnvSecretFacts {
    pub exists: bool,
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub stored_value: Option<String>,
    pub error: Option<crate::secrets::SecretStoreError>,
}
pub(crate) struct EnvPromptSeed {
    pub common: PromptSeedCommon,
    pub key: String,
    pub prompt: Option<String>,
    pub title: Option<String>,
    pub secret: bool,
    pub facts: EnvSecretFacts,
    pub local_storage: Option<Arc<parking_lot::Mutex<Option<String>>>>,
}
pub(crate) struct DropPromptSeed {
    pub common: PromptSeedCommon,
    pub placeholder: Option<String>,
    pub hint: Option<String>,
    pub owned_files: Option<Vec<crate::prompts::DroppedFile>>,
}
pub(crate) struct TemplatePromptSeed {
    pub common: PromptSeedCommon,
    pub template: String,
}
pub(crate) struct HotkeyPromptSeed {
    pub common: PromptSeedCommon,
    pub description: String,
}
pub(crate) struct ChatPromptSeed {
    pub common: PromptSeedCommon,
    pub placeholder: Option<String>,
    pub messages: Vec<crate::protocol::ChatPromptMessage>,
    pub hint: Option<String>,
    pub footer: Option<String>,
    pub model: Option<String>,
    pub models: Vec<String>,
    pub save_history: bool,
    pub dismiss: Option<crate::prompts::ChatPromptDismissCallback>,
}
pub(crate) struct TerminalPromptSeed {
    pub common: PromptSeedCommon,
    pub terminal: crate::terminal::TerminalHandle,
}
pub(crate) struct NamingPromptSeed {
    pub common: PromptSeedCommon,
    pub config: crate::prompts::NamingPromptConfig,
    pub input: String,
}
pub(crate) struct ConfirmPromptSeed {
    pub common: PromptSeedCommon,
    pub options: crate::confirm::ParentConfirmOptions,
}
pub(crate) struct WebcamPromptSeed {
    pub common: PromptSeedCommon,
    pub width: u32,
    pub height: u32,
    pub nv12: Vec<u8>,
}
pub(crate) struct ScratchPadPromptSeed {
    pub editor: EditorPromptSeed,
    pub path: PathBuf,
}
pub(crate) struct PresetPromptSeed {
    pub common: PromptSeedCommon,
    pub name: String,
    pub system_prompt: String,
    pub model: String,
    pub active_field: usize,
}
pub(crate) struct CreationFeedbackPromptSeed {
    pub common: PromptSeedCommon,
    pub payload: crate::prompts::CreationFeedbackPayload,
}

pub(crate) enum PromptSeed {
    Arg(ChoicePromptSeed),
    Mini(ChoicePromptSeed),
    Micro(ChoicePromptSeed),
    Div(DivPromptSeed),
    Form(FormPromptSeed),
    Fields(FieldsPromptSeed),
    Editor(EditorPromptSeed),
    Select(SelectPromptSeed),
    Path(PathPromptSeed),
    Env(EnvPromptSeed),
    Drop(DropPromptSeed),
    Template(TemplatePromptSeed),
    Hotkey(HotkeyPromptSeed),
    Chat(ChatPromptSeed),
    Term(TerminalPromptSeed),
    Naming(NamingPromptSeed),
    Confirm(ConfirmPromptSeed),
    Webcam(WebcamPromptSeed),
    ScratchPad(ScratchPadPromptSeed),
    QuickTerminal(TerminalPromptSeed),
    CreatePreset(PresetPromptSeed),
    CreationFeedback(CreationFeedbackPromptSeed),
}
impl PromptSeed {
    fn common_mut(&mut self) -> &mut PromptSeedCommon {
        match self {
            Self::Arg(seed) | Self::Mini(seed) | Self::Micro(seed) => &mut seed.common,
            Self::Div(seed) => &mut seed.common,
            Self::Form(seed) => &mut seed.common,
            Self::Fields(seed) => &mut seed.common,
            Self::Editor(seed) => &mut seed.common,
            Self::Select(seed) => &mut seed.common,
            Self::Path(seed) => &mut seed.common,
            Self::Env(seed) => &mut seed.common,
            Self::Drop(seed) => &mut seed.common,
            Self::Template(seed) => &mut seed.common,
            Self::Hotkey(seed) => &mut seed.common,
            Self::Chat(seed) => &mut seed.common,
            Self::Term(seed) | Self::QuickTerminal(seed) => &mut seed.common,
            Self::Naming(seed) => &mut seed.common,
            Self::Confirm(seed) => &mut seed.common,
            Self::Webcam(seed) => &mut seed.common,
            Self::ScratchPad(seed) => &mut seed.editor.common,
            Self::CreatePreset(seed) => &mut seed.common,
            Self::CreationFeedback(seed) => &mut seed.common,
        }
    }
}

impl ScriptListApp {
    /// SDK hosts perform reveal/resize around this constructor. Evaluators only
    /// use their exact hidden window handle; no native global is changed here.
    pub(crate) fn construct_prompt_seed(
        &mut self,
        mut seed: PromptSeed,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<PromptInstance> {
        let quick_terminal = matches!(&seed, PromptSeed::QuickTerminal(_));
        let confirm_lifetime = matches!(&seed, PromptSeed::Confirm(_));
        let common = seed.common_mut();
        let mut binding = common.completion.clone();
        binding.confirm_lifetime = confirm_lifetime;
        let instance = binding.instance().clone();
        anyhow::ensure!(!instance.id.is_empty(), "empty_prompt_id");
        let actions = common.actions.take();
        let id = instance.id.clone();
        let callback = binding.submit_callback();
        let theme = self.theme.clone();
        // Validation and fallible leaf construction precede replacing the active
        // lifetime. A failed constructor cannot retire the previous prompt.
        let (view, focus) = match seed {
            PromptSeed::Arg(seed) => {
                self.seed_choice_input(&seed.input, &seed.placeholder);
                (
                    AppView::ArgPrompt {
                        id,
                        placeholder: seed.placeholder,
                        choices: seed.choices,
                        actions: actions.clone(),
                    },
                    FocusTarget::MainFilter,
                )
            }
            PromptSeed::Mini(seed) => {
                self.seed_choice_input(&seed.input, &seed.placeholder);
                (
                    AppView::MiniPrompt {
                        id,
                        placeholder: seed.placeholder,
                        choices: seed.choices,
                    },
                    FocusTarget::MainFilter,
                )
            }
            PromptSeed::Micro(seed) => {
                self.seed_choice_input(&seed.input, &seed.placeholder);
                (
                    AppView::MicroPrompt {
                        id,
                        placeholder: seed.placeholder,
                        choices: seed.choices,
                    },
                    FocusTarget::AppRoot,
                )
            }
            PromptSeed::Div(seed) => {
                let prompt = crate::prompts::DivPrompt::with_options(
                    id.clone(),
                    seed.html,
                    None,
                    cx.focus_handle(),
                    callback,
                    theme,
                    crate::designs::DesignVariant::Default,
                    seed.options,
                );
                (
                    AppView::DivPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::AppRoot,
                )
            }
            PromptSeed::Form(seed) => {
                let prompt = crate::form_prompt::FormPromptState::new(
                    id.clone(),
                    seed.html,
                    crate::components::FormFieldColors::from_theme(&theme),
                    cx,
                );
                (
                    AppView::FormPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::FormPrompt,
                )
            }
            PromptSeed::Fields(seed) => {
                let prompt = crate::form_prompt::FormPromptState::from_fields(
                    id.clone(),
                    seed.fields,
                    crate::components::FormFieldColors::from_theme(&theme),
                    cx,
                );
                (
                    AppView::FormPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::FormPrompt,
                )
            }
            PromptSeed::Editor(seed) => {
                let focus_handle = cx.focus_handle();
                let prompt = self.editor_from_seed(seed, focus_handle.clone(), callback);
                (
                    AppView::EditorPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                        focus_handle,
                    },
                    FocusTarget::EditorPrompt,
                )
            }
            PromptSeed::ScratchPad(seed) => {
                if let Some(policy) = crate::runtime_policy::owned_evaluation() {
                    policy.require_owned_path(&seed.path)?;
                }
                let focus_handle = cx.focus_handle();
                let save_path = seed.path.clone();
                let complete = callback;
                let save_completion = binding.clone();
                let (save_error_sender, save_error_receiver) = async_channel::bounded(1);
                let expected = instance.clone();
                cx.spawn(async move |this, cx| {
                    while let Ok(error) = save_error_receiver.recv().await {
                        if this
                            .update(cx, |app, cx| {
                                if app
                                    .prompt_completion
                                    .as_ref()
                                    .map(|binding| binding.instance())
                                    != Some(&expected)
                                {
                                    return false;
                                }
                                app.show_error_toast(error, cx);
                                true
                            })
                            .ok()
                            != Some(true)
                        {
                            break;
                        }
                    }
                })
                .detach();
                let save_callback = Arc::new(move |id, value: Option<String>| {
                    let state = save_completion.observation();
                    if state.retired || state.completed {
                        complete(id, value);
                        return;
                    }
                    if let Some(content) = value.as_deref() {
                        if let Err(error) = crate::editor::save_scratch_content(&save_path, content)
                        {
                            save_completion.record_error(SubmissionError::StorageFailure);
                            tracing::error!(%error, "Scratch submit save failed");
                            let _ = save_error_sender
                                .try_send(format!("Failed to save scratch pad: {error}"));
                            return;
                        }
                    }
                    complete(id, value);
                });
                let prompt =
                    self.editor_from_seed(seed.editor, focus_handle.clone(), save_callback);
                let entity = cx.new(|_| prompt);
                entity.update(cx, |editor, cx| editor.start_autosave(seed.path, cx));
                let mut last_error = None;
                cx.observe(&entity, move |app, editor, cx| {
                    if !matches!(&app.current_view, AppView::ScratchPadView { entity, .. } if entity == &editor) { return; }
                    let error = editor.read(cx).autosave_error.clone();
                    if error != last_error {
                        if let Some(error) = error.as_ref() { app.show_error_toast(format!("Failed to auto-save scratch pad: {error}"), cx); }
                        last_error = error;
                    }
                }).detach();
                (
                    AppView::ScratchPadView {
                        entity,
                        focus_handle,
                    },
                    FocusTarget::EditorPrompt,
                )
            }
            PromptSeed::Select(seed) => {
                let prompt = crate::prompts::SelectPrompt::new(
                    id.clone(),
                    seed.placeholder,
                    seed.choices,
                    seed.multiple,
                    cx.focus_handle(),
                    callback,
                    theme,
                )
                .with_disabled_choices(seed.disabled);
                (
                    AppView::SelectPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::SelectPrompt,
                )
            }
            PromptSeed::Path(seed) => {
                let focus_handle = cx.focus_handle();
                let prompt = match seed.source {
                    PathSource::Production(path) => {
                        crate::runtime_policy::check(
                            crate::runtime_policy::ExternalEffect::ExternalStorage,
                        )?;
                        crate::prompts::PathPrompt::new(
                            id.clone(),
                            path,
                            seed.hint,
                            focus_handle.clone(),
                            callback,
                            theme,
                        )
                    }
                    PathSource::OwnedDirectory(path) => {
                        crate::prompts::PathPrompt::from_owned_directory(
                            id.clone(),
                            path,
                            seed.hint,
                            focus_handle.clone(),
                            callback,
                            theme,
                        )?
                    }
                }
                .with_actions_showing(self.path_actions_showing.clone())
                .with_actions_search_text(self.path_actions_search_text.clone());
                let entity = cx.new(|_| prompt);
                cx.subscribe(
                    &entity,
                    |this, _entity, event: &crate::prompts::PathPromptEvent, cx| match event {
                        crate::prompts::PathPromptEvent::ShowActions(info) => {
                            this.handle_show_path_actions(info.clone(), cx)
                        }
                        crate::prompts::PathPromptEvent::CloseActions => {
                            this.handle_close_path_actions(cx)
                        }
                    },
                )
                .detach();
                if let Ok(mut showing) = self.path_actions_showing.lock() {
                    *showing = false;
                }
                (
                    AppView::PathPrompt {
                        id,
                        entity,
                        focus_handle,
                    },
                    FocusTarget::PathPrompt,
                )
            }
            PromptSeed::Env(seed) => {
                anyhow::ensure!(
                    !crate::runtime_policy::is_owned_evaluation() || seed.local_storage.is_some(),
                    "env_owned_storage_required"
                );
                let mut prompt = crate::prompts::EnvPrompt::new(
                    id.clone(),
                    seed.key,
                    seed.prompt,
                    seed.title,
                    seed.secret,
                    cx.focus_handle(),
                    callback,
                    theme,
                    seed.facts.exists,
                    seed.facts.modified_at,
                    seed.facts.stored_value,
                    seed.facts.error,
                );
                if let Some(storage) = seed.local_storage {
                    prompt = prompt.with_local_storage(storage);
                }
                (
                    AppView::EnvPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::EnvPrompt,
                )
            }
            PromptSeed::Drop(seed) => {
                let mut prompt = crate::prompts::DropPrompt::new(
                    id.clone(),
                    seed.placeholder,
                    seed.hint,
                    cx.focus_handle(),
                    callback,
                    theme,
                );
                if let Some(files) = seed.owned_files {
                    prompt = prompt.with_owned_files(files)?;
                }
                (
                    AppView::DropPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::DropPrompt,
                )
            }
            PromptSeed::Template(seed) => {
                let prompt = crate::prompts::TemplatePrompt::new(
                    id.clone(),
                    seed.template,
                    cx.focus_handle(),
                    callback,
                    theme,
                );
                (
                    AppView::TemplatePrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::TemplatePrompt,
                )
            }
            PromptSeed::Hotkey(seed) => {
                let entity = cx.new(|cx| {
                    crate::components::shortcut_recorder::ShortcutRecorder::new(cx, theme)
                        .with_capture_only(true)
                        .with_command_name(seed.description)
                });
                let completion = binding.clone();
                cx.observe(&entity, move |this, recorder, cx| {
                    if !matches!(&this.current_view, AppView::HotkeyPrompt { entity, .. } if entity == &recorder) { return; }
                    if completion.observation().receipt.is_some() { return; }
                    let outcome = recorder.update(cx, |recorder, _cx| {
                        use crate::components::shortcut_recorder::RecorderAction;
                        match recorder.take_pending_action() {
                            Some(RecorderAction::Cancel) => Some(PromptOutcome::Cancelled),
                            Some(RecorderAction::Save(shortcut)) => Some(PromptOutcome::Submitted(SubmitValue::Text(shortcut.to_hotkey_info_json()))),
                            None if recorder.shortcut.is_complete() => Some(PromptOutcome::Submitted(SubmitValue::Text(recorder.shortcut.to_hotkey_info_json()))),
                            None => None,
                        }
                    });
                    if let Some(outcome) = outcome {
                        if let Err(error) = completion.try_complete(outcome) { this.show_error_toast(error.to_string(), cx); }
                        this.mark_main_data_changed();
                        cx.notify();
                    }
                }).detach();
                (AppView::HotkeyPrompt { id, entity }, FocusTarget::AppRoot)
            }
            PromptSeed::Chat(seed) => {
                anyhow::ensure!(
                    !crate::runtime_policy::is_owned_evaluation() || !seed.save_history,
                    "sdk_fixture_history_forbidden"
                );
                let chat_callback = binding.chat_submit_callback();
                let dismiss = binding.clone();
                let dismiss_host = seed.dismiss;
                let mut prompt = crate::prompts::ChatPrompt::new_sdk(id.clone(), seed.placeholder, seed.messages, seed.hint, seed.footer,
                    cx.focus_handle(), Some(chat_callback), seed.save_history, theme)
                    .with_dismiss_binding(crate::prompts::ChatPromptDismissBinding {
                        route: crate::prompts::ChatPromptDismissRoute::Back,
                        active_work: crate::components::conversation_actions::ActiveWorkDismissal::RequiresExplicitStop,
                        callback: Arc::new(move |request| {
                            if dismiss.try_complete(PromptOutcome::Cancelled).is_ok() {
                                if let Some(host) = &dismiss_host { host(request); }
                            }
                        }),
                    }).with_mini_mode(self.main_window_mode == crate::MainWindowMode::Mini);
                if !seed.models.is_empty() {
                    prompt = prompt.with_model_names(seed.models);
                }
                if let Some(model) = seed.model {
                    prompt = prompt.with_default_model(model);
                }
                (
                    AppView::ChatPrompt {
                        id,
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::ChatPrompt,
                )
            }
            PromptSeed::Term(seed) | PromptSeed::QuickTerminal(seed) => {
                let quick = quick_terminal;
                let prompt = crate::term_prompt::TermPrompt::with_existing_terminal(
                    id.clone(),
                    seed.terminal,
                    cx.focus_handle(),
                    callback,
                    theme,
                    Arc::new(self.config.clone()),
                    Some(
                        crate::window_resize::layout::MAX_HEIGHT
                            - px(crate::window_resize::layout::FOOTER_HEIGHT),
                    ),
                )?;
                let entity = cx.new(|_| prompt);
                (
                    if quick {
                        AppView::QuickTerminalView { entity }
                    } else {
                        AppView::TermPrompt { id, entity }
                    },
                    FocusTarget::TermPrompt,
                )
            }
            PromptSeed::Naming(seed) => {
                if let Some(policy) = crate::runtime_policy::owned_evaluation() {
                    policy.require_owned_path(&seed.config.target_directory)?;
                }
                let prompt = crate::prompts::NamingPrompt::new(
                    id.clone(),
                    seed.config,
                    cx.focus_handle(),
                    callback,
                    theme,
                );
                let entity = cx.new(|_| prompt);
                entity.update(cx, |prompt, cx| prompt.set_input(seed.input, cx));
                (
                    AppView::NamingPrompt { id, entity },
                    FocusTarget::NamingPrompt,
                )
            }
            PromptSeed::Confirm(seed) => {
                let (sender, receiver) = async_channel::bounded(1);
                let completion = binding.clone();
                cx.spawn(async move |this, cx| {
                    while let Ok(confirmed) = receiver.recv().await {
                        let result = completion.try_complete(PromptOutcome::Confirmed(confirmed));
                        let delivered = result.is_ok();
                        let still_current = this
                            .update(cx, |app, cx| {
                                if app
                                    .prompt_completion
                                    .as_ref()
                                    .map(|binding| binding.instance())
                                    != Some(completion.instance())
                                {
                                    return false;
                                }
                                match result {
                                    Ok(_) => {
                                        if let AppView::ConfirmPrompt { previous, .. } =
                                            &app.current_view
                                        {
                                            app.current_view = (**previous).clone();
                                            app.note_main_route_changed();
                                            app.mark_main_surface_changed();
                                        }
                                        completion.retire();
                                    }
                                    Err(error) => app.show_error_toast(error.to_string(), cx),
                                }
                                app.mark_main_data_changed();
                                if matches!(app.current_view, AppView::ScriptList) {
                                    app.flush_pending_main_menu_query(cx);
                                }
                                cx.notify();
                                true
                            })
                            .ok()
                            == Some(true);
                        if delivered || !still_current {
                            break;
                        }
                    }
                })
                .detach();
                (
                    AppView::ConfirmPrompt {
                        options: seed.options,
                        sender,
                        focused_button: crate::ConfirmFocusedButton::default(),
                        previous: Box::new(self.current_view.clone()),
                    },
                    FocusTarget::AppRoot,
                )
            }
            PromptSeed::Webcam(seed) => {
                let prompt = crate::prompts::WebcamPrompt::from_nv12_frame(
                    id,
                    cx.focus_handle(),
                    callback,
                    theme,
                    seed.width,
                    seed.height,
                    &seed.nv12,
                )?;
                (
                    AppView::WebcamView {
                        entity: cx.new(|_| prompt),
                    },
                    FocusTarget::AppRoot,
                )
            }
            PromptSeed::CreatePreset(seed) => (
                AppView::CreateAiPresetView {
                    name: seed.name,
                    system_prompt: seed.system_prompt,
                    model: seed.model,
                    active_field: seed.active_field.min(2),
                },
                FocusTarget::AppRoot,
            ),
            PromptSeed::CreationFeedback(seed) => {
                if let Some(policy) = crate::runtime_policy::owned_evaluation() {
                    policy.require_owned_path(&seed.payload.artifact_path)?;
                }
                (
                    AppView::CreationFeedback {
                        payload: seed.payload,
                    },
                    FocusTarget::AppRoot,
                )
            }
        };
        if let Some(previous) = self.prompt_completion.replace(binding) {
            previous.retire();
        }
        self.current_view = view;
        self.note_main_route_changed();
        self.mark_main_surface_changed();
        self.sdk_actions = None;
        self.action_shortcuts.clear();
        if let Some(actions) = actions {
            self.set_sdk_actions_and_shortcuts(actions, "UI", false);
        }
        self.focused_input = if matches!(
            self.current_view,
            AppView::ArgPrompt { .. } | AppView::MiniPrompt { .. } | AppView::MicroPrompt { .. }
        ) {
            FocusedInput::ArgPrompt
        } else {
            FocusedInput::None
        };
        self.pending_focus = Some(focus);
        self.bind_owned_surface_revision_observers(cx);
        cx.notify();
        Ok(instance)
    }

    pub(crate) fn mount_prompt_seed(
        &mut self,
        seed: PromptSeed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<PromptInstance> {
        let instance = self.construct_prompt_seed(seed, cx)?;
        if matches!(
            self.current_view,
            AppView::ArgPrompt { .. } | AppView::MiniPrompt { .. }
        ) {
            let value = self.filter_text.clone();
            self.gpui_input_state
                .update(cx, |input, cx| input.set_value(value, window, cx));
        }
        if crate::runtime_policy::is_owned_evaluation()
            && matches!(self.current_view, AppView::MiniPrompt { .. })
        {
            let view_type = crate::window_resize::ViewType::MiniPrompt;
            let size = gpui::size(
                crate::window_resize::width_for_view(view_type)
                    .map(px)
                    .unwrap_or(window.viewport_size().width),
                crate::window_resize::height_for_view(view_type, self.filtered_arg_choices().len()),
            );
            if window.viewport_size() != size {
                window.resize(size);
                self.mark_main_presentation_changed();
            }
        }
        Ok(instance)
    }

    fn seed_choice_input(&mut self, input: &str, placeholder: &str) {
        self.arg_input.set_text(input.to_string());
        self.filter_text = input.to_string();
        self.set_arg_selected_index(0);
        self.arg_list_scroll_handle
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
    }

    fn editor_from_seed(
        &self,
        seed: EditorPromptSeed,
        focus: gpui::FocusHandle,
        callback: crate::prompts::SubmitCallback,
    ) -> crate::editor::EditorPrompt {
        let height = Some(px(700.0 - crate::window_resize::layout::FOOTER_HEIGHT));
        let id = seed.common.completion.instance().id.clone();
        let template = seed.template.or_else(|| {
            crate::snippet::analysis::contains_explicit_tabstops(&seed.content)
                .then(|| seed.content.clone())
        });
        if let Some(template) = template {
            crate::editor::EditorPrompt::with_template(
                id,
                template,
                seed.language,
                focus,
                callback,
                self.theme.clone(),
                Arc::new(self.config.clone()),
                height,
            )
        } else {
            crate::editor::EditorPrompt::with_height(
                id,
                seed.content,
                seed.language,
                focus,
                callback,
                self.theme.clone(),
                Arc::new(self.config.clone()),
                height,
            )
        }
    }
}

/// Actual list layout and pending reveal, not a selected-index proxy or predicted size.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptChoiceViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub content_height: f32,
    pub scroll_offset_y: f32,
    pub pending_reveal_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromptObservation {
    pub instance: PromptInstance,
    pub input: String,
    pub choice_count: usize,
    pub selected_index: Option<usize>,
    pub choice_viewport: Option<PromptChoiceViewport>,
    pub values: Vec<(String, String)>,
    pub validation_error: Option<String>,
    pub completion: PromptCompletionObservation,
}

impl ScriptListApp {
    pub(crate) fn prompt_observation(&self, cx: &App) -> Option<PromptObservation> {
        let binding = self.prompt_completion.as_ref()?;
        let completion = binding.observation();
        if completion.retired {
            return None;
        }
        let current_id = match &self.current_view {
            AppView::ArgPrompt { id, .. }
            | AppView::MiniPrompt { id, .. }
            | AppView::MicroPrompt { id, .. }
            | AppView::DivPrompt { id, .. }
            | AppView::FormPrompt { id, .. }
            | AppView::EditorPrompt { id, .. }
            | AppView::SelectPrompt { id, .. }
            | AppView::PathPrompt { id, .. }
            | AppView::EnvPrompt { id, .. }
            | AppView::DropPrompt { id, .. }
            | AppView::TemplatePrompt { id, .. }
            | AppView::HotkeyPrompt { id, .. }
            | AppView::ChatPrompt { id, .. }
            | AppView::TermPrompt { id, .. }
            | AppView::NamingPrompt { id, .. } => Some(id.as_str()),
            _ => None,
        };
        if current_id.is_some_and(|id| id != binding.instance().id) {
            return None;
        }
        let mut observation = PromptObservation {
            instance: binding.instance().clone(),
            input: String::new(),
            choice_count: 0,
            selected_index: None,
            choice_viewport: None,
            values: Vec::new(),
            validation_error: None,
            completion,
        };
        match &self.current_view {
            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => {
                observation.input = self.arg_input.text().to_owned();
                observation.choice_count = self.filtered_arg_choices().len();
                observation.selected_index =
                    (observation.choice_count > 0).then_some(self.arg_selected_index);
                if !matches!(self.current_view, AppView::MicroPrompt { .. }) {
                    let scroll = self.arg_list_scroll_handle.0.borrow();
                    observation.choice_viewport = scroll.last_item_size.map(|size| {
                        let bounds = scroll.base_handle.bounds();
                        PromptChoiceViewport {
                            x: f32::from(bounds.origin.x),
                            y: f32::from(bounds.origin.y),
                            width: f32::from(bounds.size.width),
                            height: f32::from(bounds.size.height),
                            content_height: f32::from(size.contents.height),
                            scroll_offset_y: f32::from(scroll.base_handle.offset().y),
                            pending_reveal_index: scroll
                                .deferred_scroll_to_item
                                .as_ref()
                                .map(|reveal| reveal.item_index),
                        }
                    });
                }
            }
            AppView::FormPrompt { entity, .. } => {
                let form = entity.read(cx);
                observation.input = form.focused_value(cx).unwrap_or_default();
                observation.choice_count = form.fields.len();
                observation.selected_index =
                    (!form.fields.is_empty()).then_some(form.focused_index);
                observation.values = form.field_values(cx);
                observation.validation_error = form.validation_error.clone();
            }
            AppView::EditorPrompt { entity, .. } | AppView::ScratchPadView { entity, .. } => {
                let editor = entity.read(cx);
                observation.input = editor.content_from_app(cx);
                observation.validation_error = editor.autosave_error.clone();
                observation
                    .values
                    .push(("language".into(), editor.language().into()));
                if let Some(snippet) = editor.snippet_state() {
                    observation.choice_count = snippet.snippet.tabstops.len();
                    observation.selected_index = Some(snippet.current_tabstop_idx);
                }
            }
            AppView::SelectPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = prompt.filter_text.clone();
                observation.choice_count = prompt.filtered_choices.len();
                observation.selected_index =
                    (!prompt.filtered_choices.is_empty()).then_some(prompt.focused_index);
                let mut selected: Vec<_> = prompt.selected.iter().copied().collect();
                selected.sort_unstable();
                observation.values = selected
                    .into_iter()
                    .filter_map(|index| {
                        prompt
                            .choices
                            .get(index)
                            .map(|choice| (choice.name.clone(), choice.value.clone()))
                    })
                    .collect();
                observation.validation_error = prompt.submission_hint.clone();
            }
            AppView::PathPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = prompt.filter_text.clone();
                observation.choice_count = prompt.filtered_entries.len();
                observation.selected_index =
                    (!prompt.filtered_entries.is_empty()).then_some(prompt.selected_index);
                observation
                    .values
                    .push(("directory".into(), prompt.current_path.clone()));
                if prompt.load_status.is_error() {
                    observation.validation_error = Some(prompt.load_status.message.clone());
                }
            }
            AppView::EnvPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = if prompt.secret {
                    "*".repeat(prompt.input_text().chars().count())
                } else {
                    prompt.input_text().to_owned()
                };
                observation.values.push(("key".into(), prompt.key.clone()));
                observation.validation_error = prompt.validation_message().map(str::to_owned);
            }
            AppView::DropPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.choice_count = prompt.dropped_files.len();
                observation.values = prompt
                    .dropped_files
                    .iter()
                    .map(|file| (file.name.clone(), file.path.clone()))
                    .collect();
            }
            AppView::TemplatePrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = prompt
                    .values
                    .get(prompt.current_input)
                    .cloned()
                    .unwrap_or_default();
                observation.choice_count = prompt.inputs.len();
                observation.selected_index =
                    (!prompt.inputs.is_empty()).then_some(prompt.current_input);
                observation.values = prompt
                    .inputs
                    .iter()
                    .zip(&prompt.values)
                    .map(|(input, value)| (input.name.clone(), value.clone()))
                    .collect();
                observation.validation_error =
                    prompt.validation_errors.iter().flatten().next().cloned();
            }
            AppView::HotkeyPrompt { entity, .. } => {
                observation.input = entity.read(cx).shortcut.to_config_string()
            }
            AppView::ChatPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = prompt.input_text().to_owned();
                observation.choice_count = prompt.message_count();
            }
            AppView::TermPrompt { entity, .. } | AppView::QuickTerminalView { entity } => {
                let prompt = entity.read(cx);
                observation.values.push((
                    "terminalText".into(),
                    prompt.terminal.text_snapshot(256, 32768).text,
                ));
                if let Some(bytes) = prompt.terminal.fixture_input() {
                    observation.input = String::from_utf8_lossy(bytes).into_owned();
                }
            }
            AppView::NamingPrompt { entity, .. } => {
                let prompt = entity.read(cx);
                observation.input = prompt.friendly_name.clone();
                observation
                    .values
                    .push(("filename".into(), prompt.filename.clone()));
                observation.validation_error = prompt
                    .validation_error
                    .as_ref()
                    .map(|error| format!("{error:?}"));
            }
            AppView::ConfirmPrompt {
                options,
                focused_button,
                ..
            } => {
                observation.choice_count = 2;
                observation.selected_index =
                    Some(if *focused_button == crate::ConfirmFocusedButton::Confirm {
                        0
                    } else {
                        1
                    });
                observation
                    .values
                    .push(("message".into(), options.body.to_string()));
            }
            AppView::WebcamView { entity } => {
                let prompt = entity.read(cx);
                observation
                    .values
                    .push(("frameWidth".into(), prompt.frame_width.to_string()));
                observation
                    .values
                    .push(("frameHeight".into(), prompt.frame_height.to_string()));
            }
            AppView::CreateAiPresetView {
                name,
                system_prompt,
                model,
                active_field,
            } => {
                observation.values = vec![
                    ("name".into(), name.clone()),
                    ("systemPrompt".into(), system_prompt.clone()),
                    ("model".into(), model.clone()),
                ];
                observation.input = observation
                    .values
                    .get(*active_field)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                observation.choice_count = 3;
                observation.selected_index = Some(*active_field);
            }
            AppView::CreationFeedback { payload } => observation.values.push((
                "artifact".into(),
                payload.artifact_path.to_string_lossy().into_owned(),
            )),
            AppView::DivPrompt { entity, .. } => observation
                .values
                .push(("document".into(), entity.read(cx).html.clone())),
            _ => return None,
        }
        Some(observation)
    }

    /// Semantic state only: theme, animation, focus notifications and frame
    /// invalidation do not advance an owned surface's data revision.
    pub(crate) fn prompt_semantic_token(&self, cx: &App) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let observation = self.prompt_observation(cx)?;
        let mut state = std::collections::hash_map::DefaultHasher::new();
        observation.instance.id.hash(&mut state);
        observation.instance.generation.hash(&mut state);
        observation.input.hash(&mut state);
        observation.choice_count.hash(&mut state);
        observation.selected_index.hash(&mut state);
        observation.values.hash(&mut state);
        observation.validation_error.hash(&mut state);
        observation.completion.semantic_revision.hash(&mut state);
        self.owned_child_semantic_revision(cx).hash(&mut state);
        observation
            .completion
            .receipt
            .as_ref()
            .map(|receipt| receipt.sequence)
            .hash(&mut state);
        observation.completion.error.hash(&mut state);
        observation.completion.retired.hash(&mut state);
        observation.completion.completed.hash(&mut state);
        observation
            .completion
            .chat_submission_count
            .hash(&mut state);
        match &self.current_view {
            AppView::EnvPrompt { entity, .. } => entity.read(cx).input_text().hash(&mut state),
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => {
                for choice in choices {
                    choice.name.hash(&mut state);
                    choice.value.hash(&mut state);
                    choice.description.hash(&mut state);
                }
            }
            AppView::SelectPrompt { entity, .. } => {
                for choice in &entity.read(cx).choices {
                    choice.name.hash(&mut state);
                    choice.value.hash(&mut state);
                    choice.description.hash(&mut state);
                }
            }
            _ => {}
        }
        Some(state.finish())
    }

    pub(crate) fn update_prompt_theme(&mut self, cx: &mut Context<Self>) {
        let theme = self.theme.clone();
        let mut view = &self.current_view;
        while let AppView::ConfirmPrompt { previous, .. } = view {
            view = previous.as_ref();
        }
        match view {
            AppView::DivPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.update_theme(theme, cx))
            }
            AppView::FormPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.update_theme(&theme, cx))
            }
            AppView::EditorPrompt { entity, .. } | AppView::ScratchPadView { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.update_theme(theme, cx))
            }
            AppView::SelectPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::PathPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::EnvPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::DropPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::TemplatePrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::NamingPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.theme = theme;
                cx.notify();
            }),
            AppView::HotkeyPrompt { entity, .. } => entity.update(cx, |prompt, cx| {
                prompt.update_theme(theme);
                cx.notify();
            }),
            AppView::WebcamView { entity } => entity.update(cx, |prompt, cx| {
                prompt.base.theme = theme;
                cx.notify();
            }),
            AppView::TermPrompt { entity, .. } | AppView::QuickTerminalView { entity } => entity
                .update(cx, |prompt, cx| {
                    prompt.terminal.update_theme(&theme);
                    cx.notify();
                }),
            _ => {}
        }
    }
}

fn owned_prompt_path(relative: &str) -> anyhow::Result<PathBuf> {
    let policy = crate::runtime_policy::owned_evaluation()
        .ok_or_else(|| anyhow::anyhow!("owned_prompt_policy_missing"))?;
    let path = policy.root().join(relative);
    policy.require_owned_path(&path)?;
    Ok(path)
}

pub(crate) fn prompt_fixture_seed(
    fixture_id: &str,
    theme: &crate::theme::Theme,
) -> anyhow::Result<PromptSeed> {
    let common = PromptSeedCommon::local(fixture_id);
    let choices = || {
        (1..=6)
            .map(|index| {
                Choice::new(format!("Choice {index}"), format!("value-{index}"))
                    .with_key(format!("fixture-{index}"))
            })
            .collect()
    };
    let editor = |common| EditorPromptSeed {
        common,
        content: "First line\nSecond line".into(),
        language: "markdown".into(),
        template: None,
    };
    Ok(match fixture_id {
        "prompt.arg" => PromptSeed::Arg(ChoicePromptSeed { common, placeholder: "Choose an item".into(), choices: choices(), input: String::new() }),
        "prompt.mini" => PromptSeed::Mini(ChoicePromptSeed { common, placeholder: "Choose an item".into(), choices: choices(), input: String::new() }),
        "prompt.micro" => PromptSeed::Micro(ChoicePromptSeed { common, placeholder: "Type a value".into(), choices: choices(), input: String::new() }),
        "prompt.div" => PromptSeed::Div(DivPromptSeed { common, html: "<h2>Prompt document</h2><p>Production native content.</p><a href=\"submit:accepted\">Accept document</a>".into(), options: Default::default() }),
        "prompt.form" => PromptSeed::Form(FormPromptSeed { common, html: "<form><input name=\"email\" type=\"email\" value=\"invalid\"/><textarea name=\"notes\">First line\nSecond line</textarea></form>".into() }),
        "prompt.fields" => {
            let mut email = Field::new("email".into()).with_type("email".into()); email.value = Some("invalid".into());
            let mut notes = Field::new("notes".into()).with_type("textarea".into()); notes.value = Some("First line\nSecond line".into());
            PromptSeed::Fields(FieldsPromptSeed { common, fields: vec![email, notes] })
        }
        "prompt.editor" => PromptSeed::Editor(editor(common)),
        "prompt.select" => PromptSeed::Select(SelectPromptSeed { common, placeholder: Some("Select items".into()), choices: choices(), multiple: true, disabled: vec![5] }),
        "prompt.path" => {
            let directory = owned_prompt_path("prompt-files")?;
            std::fs::create_dir_all(directory.join("Folder"))?;
            crate::atomic_file::write_private_atomic(&directory.join("alpha.txt"), b"alpha\n")?;
            crate::atomic_file::write_private_atomic(&directory.join("Folder").join("nested.txt"), b"nested\n")?;
            PromptSeed::Path(PathPromptSeed { common, source: PathSource::OwnedDirectory(directory), hint: Some("Owned fixture directory".into()) })
        }
        "prompt.env" => PromptSeed::Env(EnvPromptSeed { common, key: "FIXTURE_VALUE".into(), prompt: Some("Enter a local fixture value".into()), title: Some("Local value".into()), secret: true,
            facts: EnvSecretFacts { exists: false, modified_at: None, stored_value: None, error: None }, local_storage: Some(Arc::new(parking_lot::Mutex::new(None))) }),
        "prompt.drop" => {
            let path = owned_prompt_path("dropped.txt")?;
            crate::atomic_file::write_private_atomic(&path, b"owned\n")?;
            PromptSeed::Drop(DropPromptSeed { common, placeholder: Some("Drop files".into()), hint: None, owned_files: Some(vec![crate::prompts::DroppedFile { path: path.to_string_lossy().into_owned(), name: "dropped.txt".into(), size: 6 }]) })
        }
        "prompt.template" => PromptSeed::Template(TemplatePromptSeed { common, template: "Hello {{script_name}}, reply to {{email}}.".into() }),
        "prompt.hotkey" => PromptSeed::Hotkey(HotkeyPromptSeed { common, description: "Capture a local shortcut".into() }),
        "prompt.chat" => PromptSeed::Chat(ChatPromptSeed { common, placeholder: Some("Message the fixture".into()), messages: vec![crate::protocol::ChatPromptMessage::assistant("Ready for a local message.")], hint: None, footer: None, model: None, models: Vec::new(), save_history: false, dismiss: None }),
        "prompt.term" | "prompt.quick-terminal" => {
            let terminal = crate::terminal::TerminalHandle::from_bytes(80, 24, theme, b"\x1b[2J\x1b[H\x1b[32mOwned terminal\x1b[0m\r\nReady for input\r\n")?;
            let seed = TerminalPromptSeed { common, terminal };
            if fixture_id == "prompt.term" { PromptSeed::Term(seed) } else { PromptSeed::QuickTerminal(seed) }
        }
        "prompt.naming" => {
            let directory = owned_prompt_path("created")?; std::fs::create_dir_all(&directory)?;
            PromptSeed::Naming(NamingPromptSeed { common, config: crate::prompts::NamingPromptConfig::new(crate::prompts::NamingTarget::Script, directory, "ts"), input: "Fixture Script".into() })
        }
        "prompt.confirm" => PromptSeed::Confirm(ConfirmPromptSeed { common, options: crate::confirm::ParentConfirmOptions { title: "Confirm fixture".into(), body: "Use this local choice?".into(), confirm_text: "Use choice".into(), cancel_text: "Cancel".into(), ..Default::default() } }),
        "prompt.webcam" => {
            let (width, height) = (320, 180);
            let mut nv12 = vec![128; width * height * 3 / 2];
            for row in 0..height { for col in 0..width { nv12[row * width + col] = 32 + ((row + col) % 192) as u8; } }
            PromptSeed::Webcam(WebcamPromptSeed { common, width: width as u32, height: height as u32, nv12 })
        }
        "prompt.scratch-pad" => {
            let path = owned_prompt_path("scratch-pad.md")?;
            let editor = editor(common); crate::atomic_file::write_private_atomic(&path, editor.content.as_bytes())?;
            PromptSeed::ScratchPad(ScratchPadPromptSeed { editor, path })
        }
        "prompt.create-preset" => PromptSeed::CreatePreset(PresetPromptSeed { common, name: "Fixture Preset".into(), system_prompt: "Answer with one short sentence.".into(), model: "fixture-model".into(), active_field: 0 }),
        "prompt.creation-feedback" => {
            let path = owned_prompt_path("created-script.ts")?; crate::atomic_file::write_private_atomic(&path, b"export {};\n")?;
            PromptSeed::CreationFeedback(CreationFeedbackPromptSeed { common, payload: crate::prompts::CreationFeedbackPayload::local_artifact(path) })
        }
        _ => anyhow::bail!("unknown_prompt_fixture:{fixture_id}"),
    })
}
