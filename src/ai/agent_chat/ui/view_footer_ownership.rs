impl AgentChatView {
    /// The footer-owner decision inputs, resolved once from the live window and
    /// host state. Shared by `render_resolved_footer` (the paint path) and
    /// `automation_layout_info` (the measure path) so both agree on the footer
    /// owner.
    fn footer_inputs(
        &self,
        window: &Window,
    ) -> crate::ai::agent_chat::ui::layout::AgentChatFooterInputs {
        let is_main_window =
            crate::get_main_window_handle().is_some_and(|handle| handle == window.window_handle());

        #[cfg(target_os = "macos")]
        let glass_in_window_footer = !is_main_window
            && crate::platform::tahoe_liquid_glass_available()
            && crate::theme::get_cached_theme().is_vibrancy_enabled();
        #[cfg(not(target_os = "macos"))]
        let glass_in_window_footer = false;

        crate::ai::agent_chat::ui::layout::AgentChatFooterInputs {
            uses_external_footer_host: false,
            is_main_window,
            glass_in_window_footer,
            platform_native_detached_footer: cfg!(target_os = "macos"),
            main_active_surface_is_agent_chat:
                crate::footer_popup::active_main_window_footer_surface() == Some("agent_chat"),
        }
    }

    /// The single footer-owner state machine (C-R5). Every footer branch —
    /// normal, setup, runtime-setup, FocusedTextMini, bottom-dock — routes
    /// through here so exactly one owner is live per frame and the Native→*
    /// transition tears the native host down explicitly (a detached window
    /// otherwise leaves an orphan native footer host when it flips to an inline
    /// rail). Native side-effects apply only to detached windows; the embedded
    /// main window's native footer surface is owned by the main-window footer
    /// system and is never installed or torn down from here.
    fn reconcile_footer_owner(
        &mut self,
        desired: AgentChatFooterOwner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        // Run the lifecycle side-effects (install / tear-down / re-sync the
        // native footer host, spawn / drop the listener) — memoized, so nothing
        // happens on a frame that did not change the footer presentation.
        self.transition_footer_owner(desired, window, cx);

        // Element construction is pure and runs every frame — the memoized
        // transition above already reconciled the native host lifecycle.
        match desired {
            AgentChatFooterOwner::Native => Some(
                crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(),
            ),
            AgentChatFooterOwner::Inline => Some(self.render_agent_chat_config_footer_rail(cx)),
            AgentChatFooterOwner::External => None,
        }
    }

    /// Apply the native footer lifecycle side-effects for a resolved owner,
    /// MEMOIZED on [`Self::last_footer_presentation`] (BC-2, Oracle seat 3).
    ///
    /// Render side-effects used to re-sync the native footer popup and (re)spawn
    /// the action listener every frame the owner was Native. This drives them
    /// only on an actual transition:
    /// - change TO a detached Native footer (or a change to its synced config):
    ///   ensure the action listener + sync the native popup;
    /// - change AWAY from a detached Native footer: clear the native popup +
    ///   drop the listener task so a detached window never leaves an orphan host.
    ///
    /// Native side-effects apply only to detached windows; the embedded main
    /// window's native footer surface is owned by the main-window footer system
    /// and is never installed or torn down from here.
    fn transition_footer_owner(
        &mut self,
        desired: AgentChatFooterOwner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_main_window =
            crate::get_main_window_handle().is_some_and(|handle| handle == window.window_handle());
        // The native config is materialised only when a DETACHED window owns the
        // native footer — that is the sole case with native lifecycle effects.
        let native_config = (desired == AgentChatFooterOwner::Native && !is_main_window)
            .then(|| self.agent_chat_detached_native_footer_config(cx));
        let next = AgentChatFooterPresentationState {
            owner: desired,
            is_main_window,
            native_config,
        };

        // Keep the C-R5 owner mirror current every frame (cheap; read by the
        // pure transition planner and tests).
        self.footer_owner = Some(desired);

        let lifecycle = plan_native_footer_lifecycle(self.last_footer_presentation.as_ref(), &next);
        if lifecycle.unchanged {
            // No presentation change — no lifecycle side-effects this frame.
            return;
        }

        if lifecycle.tear_down_previous_native {
            // A detached native host installed by the previous presentation must
            // be torn down when we move off it. `clear_window_footer_popup`
            // guards on the current window and no-ops for the shared main-window
            // host, and the listener is dropped since nothing drives it now.
            crate::footer_popup::clear_window_footer_popup(window);
            self._footer_action_task = None;
        }

        if lifecycle.sync_next_native {
            self.ensure_native_footer_action_listener(window, cx);
            if let Some(config) = next.native_config.as_ref() {
                crate::footer_popup::sync_window_footer_popup(window, config);
            }
        }

        self.last_footer_presentation = Some(next);
    }

    /// The ONE footer decision for the resolved shell (WP6 / C-R5). Both
    /// composer slots and every host route through the single owner state
    /// machine: an external host reserves no local band, a native-owned footer
    /// reserves a spacer band, and everything else renders the inline config
    /// rail. Returns `None` when no local footer band is reserved.
    fn render_resolved_footer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let presentation = crate::ai::agent_chat::ui::layout::resolve_footer_presentation(
            self.footer_inputs(window),
        );
        let desired = AgentChatFooterOwner::from_presentation(presentation);
        self.reconcile_footer_owner(desired, window, cx)
    }

    fn ensure_native_footer_action_listener(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self._footer_action_task.is_some() {
            return;
        }

        let rx = crate::footer_popup::agent_chat_footer_action_channel()
            .1
            .clone();
        self._footer_action_task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(action) = rx.recv().await {
                if let Err(error) = this.update_in(cx, |view, window, cx| {
                    view.dispatch_footer_button(action, window, cx);
                }) {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "agent_chat_native_footer_action_dispatch_failed",
                        action = ?action,
                        %error,
                        "Failed to dispatch native footer action into AgentChatView"
                    );
                }
            }
        }));
    }

    fn retryable_recovery_active(thread: &AgentChatThread) -> bool {
        matches!(thread.status, AgentChatThreadStatus::Error)
            && thread.recovery_card_spec().is_some_and(|spec| {
                spec.actions.iter().any(|action| {
                    action.enabled && action.action.kind() == RecoveryActionKind::Retry
                })
            })
    }

    fn retry_footer_button(thread: &AgentChatThread) -> Option<AgentChatFooterButtonSpec> {
        Self::retryable_recovery_active(thread).then_some(AgentChatFooterButtonSpec {
            action: crate::footer_popup::FooterAction::Retry,
            key: "⌘⇧R",
            label: "Retry",
            selected: false,
            enabled: true,
            disabled_reason: None,
        })
    }

    pub(crate) fn conversation_command_facts(
        &self,
        cx: &App,
    ) -> crate::components::conversation_actions::AgentChatConversationCommandFacts {
        let thread = self.live_thread().read(cx);
        crate::components::conversation_actions::AgentChatConversationCommandFacts {
            response_in_progress: matches!(thread.status, AgentChatThreadStatus::Streaming),
            waiting_for_permission: matches!(
                thread.status,
                AgentChatThreadStatus::WaitingForPermission
            ),
            context_preparing: self.context_capture_pending,
            composer_has_text: !thread.input.text().trim().is_empty()
                || !thread.pending_context_items().is_empty(),
            retry_available: Self::retryable_recovery_active(thread),
            has_response: Self::has_pastable_assistant_response(thread),
            dismiss_installed: self.on_close_requested.is_some()
                || self.on_close_window_requested.is_some(),
            active_work:
                crate::components::conversation_actions::ActiveWorkDismissal::RequiresExplicitStop,
        }
    }

    pub(crate) fn conversation_command_bindings(
        &self,
        cx: &App,
    ) -> Vec<
        crate::components::conversation_actions::BoundConversationCommand<
            crate::components::conversation_actions::AgentChatConversationCommand,
        >,
    > {
        crate::components::conversation_actions::agent_chat_conversation_commands(
            self.conversation_command_facts(cx),
        )
    }

    pub(crate) fn execute_conversation_command(
        &mut self,
        command: crate::components::conversation_actions::AgentChatConversationCommand,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::components::conversation_actions::AgentChatConversationCommand;

        let Some(binding) = self
            .conversation_command_bindings(cx)
            .into_iter()
            .find(|binding| binding.handler == command)
        else {
            return false;
        };
        if !binding.descriptor.availability.is_enabled() {
            return false;
        }

        match command {
            AgentChatConversationCommand::Send => self.submit_with_expanded_tokens(cx),
            AgentChatConversationCommand::Stop => {
                let _ = self.stop_streaming_explicitly(cx);
            }
            AgentChatConversationCommand::Retry => self.retry_last_user_turn(cx),
            AgentChatConversationCommand::NewConversation => {
                return self.start_new_conversation(cx);
            }
            AgentChatConversationCommand::CopyLastResponse => {
                return self.copy_last_response(cx);
            }
            AgentChatConversationCommand::Close => return false,
        }
        true
    }
}
