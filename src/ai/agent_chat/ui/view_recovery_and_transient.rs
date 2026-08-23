impl AgentChatView {
    pub(crate) fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self
            .live_thread()
            .update(cx, |thread, cx| thread.retry_last_user_turn(cx))
        {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_retry_failed",
                error = %error,
            );
        }
    }

    pub(crate) fn ai_recovery_actions(&self, cx: &App) -> Vec<crate::actions::Action> {
        let thread = self.live_thread().read(cx);
        Self::recovery_action_specs(thread.recovery_card_spec().as_ref())
    }

    fn recovery_action_specs(
        spec: Option<&crate::ai::reliability::AiRecoveryCardSpec>,
    ) -> Vec<crate::actions::Action> {
        let Some(spec) = spec else {
            return Vec::new();
        };
        use crate::actions::{Action, ActionCategory};
        use crate::designs::icon_variations::IconName;

        spec.actions
            .iter()
            .filter(|recovery| recovery.enabled)
            .map(|recovery| {
                let icon = match recovery.action.kind() {
                    RecoveryActionKind::CopyDetails | RecoveryActionKind::CopyTranscript => {
                        IconName::Copy
                    }
                    RecoveryActionKind::Retry
                    | RecoveryActionKind::RetrySameDestination
                    | RecoveryActionKind::CheckAgain => IconName::Refresh,
                    RecoveryActionKind::ChooseCompatibleModel
                    | RecoveryActionKind::ChooseProvider
                    | RecoveryActionKind::ChooseProfile
                    | RecoveryActionKind::ConfigureProvider
                    | RecoveryActionKind::UpdateClient
                    | RecoveryActionKind::SignIn
                    | RecoveryActionKind::SwitchAccount
                    | RecoveryActionKind::RepairComponent
                    | RecoveryActionKind::ChooseDestination => IconName::Settings,
                    RecoveryActionKind::UseCurrentResults
                    | RecoveryActionKind::ContinueInAgentChat
                    | RecoveryActionKind::Reattach
                    | RecoveryActionKind::RethreadFlow
                    | RecoveryActionKind::RestartFlowRun
                    | RecoveryActionKind::TrimContext
                    | RecoveryActionKind::OpenDictationHistory => IconName::ArrowRight,
                };
                Action::new(
                    recovery.semantic_id,
                    recovery.label.to_string(),
                    Some("Recover this AI operation without losing your work".to_string()),
                    ActionCategory::ScriptContext,
                )
                .with_icon(icon)
                .with_section("Recovery")
            })
            .collect()
    }

    pub(crate) fn dispatch_ai_recovery_action(
        &mut self,
        action_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let action = {
            let thread = self.live_thread().read(cx);
            thread
                .recovery_card_spec()
                .and_then(|spec| {
                    spec.actions
                        .into_iter()
                        .find(|action| action.semantic_id == action_id && action.enabled)
                })
                .map(|spec| spec.action)
        };
        let Some(action) = action else {
            return false;
        };
        self.dispatch_recovery_action(action, cx);
        true
    }

    fn dispatch_recovery_action(&mut self, action: AiRecoveryAction, cx: &mut Context<Self>) {
        let commands = match self
            .live_thread()
            .update(cx, |thread, cx| thread.select_recovery_action(action, cx))
        {
            Ok(commands) => commands,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_recovery_transition_rejected",
                    error_code = %error,
                );
                return;
            }
        };
        self.interpret_recovery_commands(commands, cx);
    }

    fn interpret_recovery_commands(&mut self, commands: Vec<AiCommand>, cx: &mut Context<Self>) {
        for command in commands {
            match command {
                AiCommand::CopyRedactedDiagnostics(_) => {
                    let receipt = {
                        let thread = self.live_thread().read(cx);
                        thread
                            .reliability_state()
                            .diagnostic
                            .as_ref()
                            .map(|diagnostic| {
                                format!(
                                    "AI recovery diagnostic\ncode={:?}\nfingerprint={}",
                                    match &thread.reliability_state().phase {
                                        sk_protocol::ai_reliability::AiPhase::AwaitingRecovery {
                                            failure,
                                            ..
                                        } => failure.code,
                                        _ => sk_protocol::ai_reliability::AiFailureCode::Unknown,
                                    },
                                    diagnostic.fingerprint.0
                                )
                            })
                    };
                    if let Some(receipt) = receipt {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(receipt));
                    }
                }
                AiCommand::LaunchAuthentication(auth) => {
                    let selected_model_id = self
                        .live_thread()
                        .read(cx)
                        .selected_model_id()
                        .map(str::to_string);
                    let action = match auth.mode {
                        AuthRecoveryMode::SignIn => {
                            crate::ai::agent_chat::pi::auth_recovery::PiAuthRecoveryAction::SignInAgain
                        }
                        AuthRecoveryMode::SwitchAccount => {
                            crate::ai::agent_chat::pi::auth_recovery::PiAuthRecoveryAction::SwitchAccount
                        }
                    };
                    let launched =
                        crate::ai::agent_chat::pi::auth_recovery::resolve_auth_recovery_provider(
                            selected_model_id.as_deref(),
                            None,
                        )
                        .zip(crate::ai::agent_chat::pi::binary::default_pi_binary())
                        .is_some_and(|(provider, binary)| {
                            crate::ai::agent_chat::pi::auth_recovery::launch_pi_auth_recovery(
                                binary, provider, action,
                            )
                            .is_ok()
                        });
                    if launched {
                        self.live_thread().update(cx, |thread, cx| {
                            thread.complete_recovery_command(
                                auth.command_id,
                                RecoveryEffectResult::ExternalActionLaunched,
                                cx,
                            );
                        });
                    } else {
                        let failure = crate::ai::reliability::provider_failure(
                            sk_protocol::ai_reliability::ProtocolComponent::Pi,
                            "authentication recovery could not start",
                        );
                        self.live_thread().update(cx, |thread, cx| {
                            thread.fail_recovery_command(auth.command_id, failure.failure, cx);
                        });
                    }
                }
                AiCommand::OpenConfiguration(target) => match target.kind {
                    ConfigurationTargetKind::Model
                    | ConfigurationTargetKind::Provider
                    | ConfigurationTargetKind::Profile => {
                        self.open_profile_trigger_picker(cx);
                    }
                    ConfigurationTargetKind::Context => {
                        self.live_thread().update(cx, |thread, cx| {
                            thread.clear_pending_context_for_new_entry_intent(cx);
                            thread.complete_recovery_command(
                                target.command_id,
                                RecoveryEffectResult::ContextTrimmed,
                                cx,
                            );
                        });
                    }
                    ConfigurationTargetKind::Mdflow => {}
                },
                AiCommand::RecheckClientCapability(check) => {
                    self.live_thread().update(cx, |thread, cx| {
                        thread.complete_recovery_command(
                            check.command_id,
                            RecoveryEffectResult::CapabilityRechecked,
                            cx,
                        );
                        if let Err(error) = thread.resume_recovered_turn_from_view(cx) {
                            tracing::warn!(
                                target: "script_kit::agent_chat",
                                event = "agent_chat_recovery_resume_failed",
                                error_code = %error,
                            );
                        }
                    });
                }
                AiCommand::OpenClientUpdate(update) => {
                    let url = match update.client {
                        sk_protocol::ai_reliability::ClientKind::Codex => {
                            "https://github.com/openai/codex/releases/latest"
                        }
                        sk_protocol::ai_reliability::ClientKind::Pi
                        | sk_protocol::ai_reliability::ClientKind::Mdflow
                        | sk_protocol::ai_reliability::ClientKind::LocalLlm
                        | sk_protocol::ai_reliability::ClientKind::Other => {
                            "https://scriptkit.com/"
                        }
                    };
                    match std::process::Command::new("open").arg(url).spawn() {
                        Ok(_) => {
                            self.live_thread().update(cx, |thread, cx| {
                                thread.complete_recovery_command(
                                    update.command_id,
                                    RecoveryEffectResult::ExternalActionLaunched,
                                    cx,
                                );
                            });
                        }
                        Err(error) => {
                            let failure = crate::ai::reliability::provider_failure(
                                sk_protocol::ai_reliability::ProtocolComponent::Codex,
                                format!("client update page could not open: {error}"),
                            );
                            self.live_thread().update(cx, |thread, cx| {
                                thread.fail_recovery_command(
                                    update.command_id,
                                    failure.failure,
                                    cx,
                                );
                            });
                        }
                    }
                }
                AiCommand::InstallOrRepairComponent(_) => {
                    self.open_profile_trigger_picker(cx);
                }
                AiCommand::RunSurfaceRecovery { command_id, action } => {
                    let failure = crate::ai::reliability::destination_failure(
                        true,
                        "surface-owned recovery callback is unavailable in Agent Chat",
                    );
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "agent_chat_surface_recovery_rejected",
                        ?action,
                    );
                    self.live_thread().update(cx, |thread, cx| {
                        thread.fail_recovery_command(command_id, failure.failure, cx);
                    });
                }
                AiCommand::ContinueInAgentChat(escalation) => {
                    let seed = self.live_thread().read(cx).quick_ai_handoff_seed();
                    match (seed, self.on_continue_in_agent_chat.clone()) {
                        (Some(seed), Some(callback)) => {
                            self.live_thread().update(cx, |thread, cx| {
                                thread.complete_recovery_command(
                                    escalation.command_id,
                                    RecoveryEffectResult::AgentChatOpened,
                                    cx,
                                );
                            });
                            cx.defer(move |cx| callback(seed.clone(), cx));
                        }
                        _ => {
                            let failure = crate::ai::reliability::provider_failure(
                                sk_protocol::ai_reliability::ProtocolComponent::Provider,
                                "Quick AI handoff could not be prepared",
                            );
                            self.live_thread().update(cx, |thread, cx| {
                                thread.fail_recovery_command(
                                    escalation.command_id,
                                    failure.failure,
                                    cx,
                                );
                            });
                        }
                    }
                }
                AiCommand::PersistWork(_)
                | AiCommand::CheckCapabilities(_)
                | AiCommand::StartTurn(_)
                | AiCommand::CancelTurn { .. }
                | AiCommand::ScheduleBackoff { .. }
                | AiCommand::ApplySelection(_)
                | AiCommand::ReattachSession(_)
                | AiCommand::RethreadFlow(_)
                | AiCommand::RestartFlowRun(_)
                | AiCommand::ClearPendingWork(_)
                | AiCommand::ScheduleRecoveredDismiss => {}
            }
        }
    }

    fn render_reserved_transient_lane(
        id: &'static str,
        height_px: f32,
        content: Option<gpui::AnyElement>,
    ) -> gpui::AnyElement {
        let height_px = agent_chat_transient_lane_height(height_px, content.is_some());
        div()
            .id(id)
            .w_full()
            .h(px(height_px))
            .overflow_hidden()
            .when_some(content, |d, content| d.child(content))
            .into_any_element()
    }

    fn message_queue_lane_active(&self, cx: &App) -> bool {
        !self.live_thread().read(cx).queued_messages().is_empty()
    }

    fn context_bootstrap_note_lane_active(&self, cx: &App) -> bool {
        self.live_thread()
            .read(cx)
            .context_bootstrap_note()
            .is_some_and(|note| !note.trim().is_empty())
    }

    fn render_message_queue_strip(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (queued, paused) = {
            let thread = self.live_thread().read(cx);
            (
                thread.queued_messages().iter().cloned().collect::<Vec<_>>(),
                thread.queue_paused(),
            )
        };

        if queued.is_empty() {
            return div()
                .id("agent_chat-message-queue-empty")
                .into_any_element();
        }

        let theme = theme::get_cached_theme();
        let border = theme.colors.ui.border;
        let accent = theme.colors.accent.selected;
        let expanded = self.message_queue_expanded;
        let count = queued.len();
        let mut container = div()
            .id("agent_chat-message-queue-strip")
            .w_full()
            .px(px(12.0))
            .pb(px(6.0));

        let header = div()
            .id("agent_chat-message-queue-header")
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(6.0))
            .bg(rgba((border << 8) | 0x10))
            .border_1()
            .border_color(rgba((border << 8) | 0x28))
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.message_queue_expanded = !this.message_queue_expanded;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().text_xs().text_color(rgb(accent)).child("↑"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.muted))
                            .child(if paused {
                                format!("{count} queued · paused")
                            } else {
                                format!("{count} queued")
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .opacity(0.45)
                    .child(if expanded { "Hide" } else { "Show" }),
            );

        container = container.child(header);

        if expanded {
            let mut list = div()
                .mt(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(rgba((border << 8) | 0x22))
                .bg(rgba((theme.colors.text.primary << 8) | 0x04));

            for (index, message) in queued.into_iter().enumerate() {
                let text = message.text.trim().replace('\n', " ");
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(8.0))
                        .py(px(5.0))
                        .child(
                            div()
                                .flex_grow()
                                .min_w(px(0.0))
                                .text_xs()
                                .text_color(rgb(theme.colors.text.dimmed))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(text),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!(
                                    "agent_chat-queue-remove-{index}"
                                ))))
                                .cursor_pointer()
                                .text_xs()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(px(999.0))
                                .text_color(rgba((theme.colors.text.muted << 8) | 0x70))
                                .hover(|d| d.bg(rgba((border << 8) | 0x18)))
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.live_thread().update(cx, |thread, cx| {
                                        thread.remove_queued_message(index, cx);
                                    });
                                }))
                                .child("×"),
                        ),
                );
            }

            list = list.child(
                div()
                    .id("agent_chat-message-queue-clear")
                    .px(px(8.0))
                    .py(px(5.0))
                    .text_xs()
                    .text_color(rgb(theme.colors.text.muted))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba((border << 8) | 0x12)))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.live_thread()
                            .update(cx, |thread, cx| thread.clear_queued_messages(cx));
                    }))
                    .child("Clear"),
            );
            container = container.child(list);
        }

        container.into_any_element()
    }

    fn render_context_bootstrap_note(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (state, note) = {
            let thread = self.live_thread().read(cx);
            (
                thread.context_bootstrap_state(),
                thread.context_bootstrap_note().map(|v| v.to_string()),
            )
        };

        let Some(note) = note.filter(|v| !v.trim().is_empty()) else {
            return div()
                .id("agent_chat-context-bootstrap-note-empty")
                .into_any_element();
        };

        let theme = theme::get_cached_theme();
        let accent = theme.colors.accent.selected;
        let border = theme.colors.ui.border;

        let (fg_color, bg, outline) = match state {
            AgentChatContextBootstrapState::Preparing => {
                (accent, (accent << 8) | 0x10, (accent << 8) | 0x24)
            }
            AgentChatContextBootstrapState::Ready => (
                theme.colors.text.muted,
                (border << 8) | 0x10,
                (border << 8) | 0x24,
            ),
            AgentChatContextBootstrapState::Failed => (
                theme.colors.text.primary,
                (border << 8) | 0x14,
                (border << 8) | 0x28,
            ),
        };

        div()
            .id("agent_chat-context-bootstrap-note")
            .px(px(12.0))
            .pb(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(6.0))
                    .bg(rgba(bg))
                    .border_1()
                    .border_color(rgba(outline))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(fg_color))
                            .child(SharedString::from(note)),
                    ),
            )
            .into_any_element()
    }
}
