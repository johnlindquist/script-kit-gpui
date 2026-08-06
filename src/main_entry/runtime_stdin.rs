// External command listener - receives commands via stdin (event-driven, no polling)
let stdin_rx = start_stdin_listener(None);
let window_for_stdin = window;
let app_entity_for_stdin = app_entity.clone();

// Track if we've received any stdin commands (for timeout warning)
static STDIN_RECEIVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy)]
enum DevtoolsSessionLifecycleAction {
    None,
    Touch {
        command_type: &'static str,
        reason: &'static str,
    },
    ExplicitClose {
        command_type: &'static str,
        reason: &'static str,
    },
}

fn devtools_keep_actions_window_open_enabled() -> bool {
    std::env::var("SCRIPT_KIT_AGENTIC_KEEP_ACTIONS_WINDOW_OPEN").ok().as_deref() == Some("1")
}

fn devtools_lifecycle_action_for_stdin(cmd: &StdinCommand) -> DevtoolsSessionLifecycleAction {
    let command_type = cmd.command_type();
    match cmd {
        StdinCommand::External(ExternalCommand::Hide { .. }) => {
            DevtoolsSessionLifecycleAction::ExplicitClose {
                command_type,
                reason: "explicit_hide",
            }
        }
        _ if devtools_keep_actions_window_open_enabled() => DevtoolsSessionLifecycleAction::Touch {
            command_type,
            reason: "stdin_devtools_activity",
        },
        _ => DevtoolsSessionLifecycleAction::None,
    }
}

fn apply_devtools_lifecycle_action(action: DevtoolsSessionLifecycleAction) {
    match action {
        DevtoolsSessionLifecycleAction::None => {}
        DevtoolsSessionLifecycleAction::Touch {
            command_type,
            reason,
        } => {
            script_kit_gpui::mark_window_shown();
            tracing::info!(
                event = "devtools_session_activity",
                keep_actions_window_open = true,
                command_type,
                reason
            );
        }
        DevtoolsSessionLifecycleAction::ExplicitClose {
            command_type,
            reason,
        } => {
            tracing::info!(
                event = "devtools_session_explicit_close",
                keep_actions_window_open = devtools_keep_actions_window_open_enabled(),
                command_type,
                reason
            );
        }
    }
}

// Spawn a timeout warning task - helps AI agents detect when they forgot to use stdin protocol
cx.spawn(async move |_cx: &mut gpui::AsyncApp| {
    Timer::after(std::time::Duration::from_secs(2)).await;
    if !STDIN_RECEIVED.load(std::sync::atomic::Ordering::SeqCst) {
        logging::log("STDIN", "");
        logging::log(
            "STDIN",
            "╔════════════════════════════════════════════════════════════════════════════╗",
        );
        logging::log(
            "STDIN",
            "║  WARNING: No stdin JSON received after 2 seconds                          ║",
        );
        logging::log(
            "STDIN",
            "║                                                                            ║",
        );
        logging::log(
            "STDIN",
            "║  If you're testing, use the stdin JSON protocol:                          ║",
        );
        logging::log(
            "STDIN",
            "║  echo '{\"type\":\"run\",\"path\":\"...\"}' | ./target/debug/script-kit-gpui     ║",
        );
        logging::log(
            "STDIN",
            "║                                                                            ║",
        );
        logging::log(
            "STDIN",
            "║  Command line args do NOT work:                                           ║",
        );
        logging::log(
            "STDIN",
            "║  ./target/debug/script-kit-gpui test.ts  # WRONG - does nothing!          ║",
        );
        logging::log(
            "STDIN",
            "╚════════════════════════════════════════════════════════════════════════════╝",
        );
        logging::log("STDIN", "");
    }
})
.detach();

cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    logging::log("STDIN", "Async stdin command handler started");

    // Event-driven: recv().await yields until a command arrives
    while let Ok(StdinCommandEnvelope {
        command: cmd,
        correlation_id,
    }) = stdin_rx.recv().await
    {
        let _guard = logging::set_correlation_id(correlation_id);
        // Mark that we've received stdin (clears the timeout warning)
        STDIN_RECEIVED.store(true, std::sync::atomic::Ordering::SeqCst);
        logging::log(
            "STDIN",
            &format!("Processing external command type={}", cmd.command_type()),
        );

        let lifecycle_action = devtools_lifecycle_action_for_stdin(&cmd);
        let app_entity_inner = app_entity_for_stdin.clone();
        let _ = cx.update(|cx| {
            apply_devtools_lifecycle_action(lifecycle_action);
            // Use the Root window to get Window reference, then update the app entity
            let _ = window_for_stdin.update(cx, |_root, window, root_cx| {
                app_entity_inner.update(root_cx, |view, ctx| {
                    // Note: We have both `window` from Root and `view` from entity here
                    // ctx is Context<ScriptListApp>, window is &mut Window
                    match cmd {
                        StdinCommand::External(cmd) => match cmd {
                            ExternalCommand::Run { ref path, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Executing script: {}", rid, path));

                                // NOTE: This is a simplified show path for script execution.
                                // We show the window, then immediately run the script.
                                // The core logic matches show_main_window_helper().

                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown(); // Focus grace period
                                platform::ensure_move_to_active_space();

                                // Use Window::defer via window_ops to coalesce and defer window move.
                                // This avoids RefCell borrow conflicts from synchronous macOS window operations.
                                let window_size = crate::window_resize::initial_window_size();
                                let bounds = platform::calculate_eye_line_bounds_on_mouse_display(window_size);
                                window_ops::queue_move(bounds, window, ctx);

                                // Oracle-Session `window-activation-invariants-guard` PR1.
                                platform::ensure_main_panel_configured("runtime_stdin::run");

                                // Show window WITHOUT activating (floating panel behavior)
                                platform::show_main_window_without_activation();
                                window.activate_window();

                                // Send AI window to back so it doesn't come forward with main menu
                                platform::send_ai_window_to_back();

                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                sync_main_automation_window(None, true, true);

                                // Ensure render-loop focus state is set so the input autofocuses
                                view.focused_input = FocusedInput::MainFilter;
                                view.pending_focus = Some(FocusTarget::MainFilter);

                                // Send RunScript message to be handled
                                view.handle_prompt_message(PromptMessage::RunScript { path: path.clone() }, ctx);
                            }
                            ExternalCommand::Show { ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Showing window", rid));

                                // NOTE: This is a simplified show path for explicit stdin commands.
                                // Unlike the hotkey handler, we don't need NEEDS_RESET handling
                                // because this is an explicit show (not a toggle).
                                // The core logic matches show_main_window_helper().

                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown(); // Focus grace period
                                platform::ensure_move_to_active_space();

                                // Position window - try per-display saved position first, then fall back to eye-line
                                let window_size = crate::window_resize::initial_window_size();
                                let displays = platform::get_macos_displays();
                                let bounds = if let Some((mouse_x, mouse_y)) = platform::get_global_mouse_position() {
                                    // Try to restore saved position for the mouse display
                                    if let Some((saved, display)) =
                                        window_state::get_main_position_for_mouse_display(mouse_x, mouse_y, &displays)
                                    {
                                        // Validate the saved position is still visible
                                        if window_state::is_bounds_visible(&saved, &displays) {
                                            logging::log(
                                                "STDIN",
                                                &format!(
                                                    "Restoring saved position for display {}: ({:.0}, {:.0})",
                                                    window_state::display_key(&display),
                                                    saved.x,
                                                    saved.y
                                                ),
                                            );
                                            // Use saved position but with current window height (may have changed)
                                            gpui::Bounds {
                                                origin: gpui::point(px(saved.x as f32), px(saved.y as f32)),
                                                size: window_size,
                                            }
                                        } else {
                                            logging::log("STDIN", "Saved position no longer visible, using eye-line");
                                            platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                        }
                                    } else {
                                        logging::log("STDIN", "No saved position for this display, using eye-line");
                                        platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                    }
                                } else {
                                    logging::log("STDIN", "Could not get mouse position, using eye-line");
                                    platform::calculate_eye_line_bounds_on_mouse_display(window_size)
                                };
                                window_ops::queue_move(bounds, window, ctx);

                                // Oracle-Session `window-activation-invariants-guard` PR1.
                                platform::ensure_main_panel_configured("runtime_stdin::show");

                                // Show window WITHOUT activating (floating panel behavior)
                                platform::show_main_window_without_activation();
                                window.activate_window();

                                // Send AI window to back so it doesn't come forward with main menu
                                platform::send_ai_window_to_back();

                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                sync_main_automation_window(None, true, true);

                                // Ensure render-loop focus state is set so the input autofocuses
                                view.focused_input = FocusedInput::MainFilter;
                                view.pending_focus = Some(FocusTarget::MainFilter);

                                // Passive AX-only selection sniff for the "Rewrite
                                // selection" hint chip — mirrors show_main_window_helper.
                                view.refresh_shown_selection_hint(ctx);
                            }
                            ExternalCommand::Hide { ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Hiding main window", rid));

                                // Save window position for the current display BEFORE hiding
                                if let Some((x, y, width, height)) = platform::get_main_window_bounds() {
                                    let displays = platform::get_macos_displays();
                                    let bounds = window_state::PersistedWindowBounds::new(x, y, width, height);
                                    if let Some(display) = window_state::find_display_for_bounds(&bounds, &displays) {
                                        logging::log(
                                            "STDIN",
                                            &format!(
                                                "Saving position for display {}: ({:.0}, {:.0})",
                                                window_state::display_key(display),
                                                x,
                                                y
                                            ),
                                        );
                                        window_state::save_main_position_for_display(display, bounds);
                                    }
                                }

                                script_kit_gpui::set_main_window_visible(false);
                                sync_main_automation_window(current_main_automation_bounds(), false, false);

                                // Reset the hidden view back to ScriptList after the
                                // native hide has been enqueued. This preserves the
                                // Pass #19 automation re-key fix without rendering a
                                // visible ScriptList frame while AppKit is still
                                // closing the panel.
                                // Full teardown for actions-dialog
                                // (`id: "actions-dialog"`). Pass #29 fix
                                // (`cmd-k-on-unfocused-clipboard-pops-overlay-not-actions`):
                                // upgraded from bare `remove_automation_window` to full
                                // `close_actions_window`. Pass #23's bare registry op
                                // left the `ACTIONS_WINDOW` static holding a stale handle;
                                // a later `simulateKey cmd+k` on an unfocused window read
                                // `is_actions_window_open()=true` and took the CLOSE branch,
                                // popping whichever overlay was on top instead of opening
                                // the actions dialog. `close_actions_window` clears the
                                // static AND the registry; idempotent.
                                crate::actions::close_actions_window(ctx);
                                // Sibling teardown for confirm-popup
                                // (`id: "confirm-popup"`, PromptPopup kind).
                                // Pass #25 fix: close_confirm_window at
                                // src/confirm/window.rs:385 is the only
                                // production removal path; no hide dispatcher
                                // calls it (`attacker-hide-path-confirm-popup-registry-stale`).
                                // Pure registry op; idempotent.
                                crate::windows::remove_automation_window("confirm-popup");

                                // Check if Notes is open for logging only.
                                let notes_open = notes::is_notes_window_open();

                                // CRITICAL: Always hide only the main panel. `ctx.hide()`
                                // app-hides all windows, so a stale/false-negative Notes
                                // handle can hide Notes together with main.
                                logging::log(
                                    "STDIN",
                                    &format!(
                                        "Using defer_hide_main_window() - main-only hide, secondary_windows_open={}",
                                        notes_open
                                    ),
                                );
                                platform::defer_hide_main_window(ctx);
                                view.defer_reset_to_script_list_after_main_window_hidden(
                                    ctx,
                                    "stdin_hide_rpc",
                                    false,
                                );
                            }
                            ExternalCommand::SetFilter { ref text, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log("STDIN", &format!("[{}] Setting filter to: '{}'", rid, text));
                                view.menu_syntax_form_input_active = false;
                                view.menu_syntax_form_draft_field_id = None;
                                view.menu_syntax_form_draft_value.clear();
                                view.set_filter_text_immediate(text.clone(), window, ctx);
                                let _ = view.get_filtered_results_cached(); // Update cache
                                ctx.notify();
                            }
                            ExternalCommand::SetMenuSyntaxFormField { ref field, ref value, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log(
                                    "STDIN",
                                    &format!(
                                        "[{}] Setting menu-syntax form field {:?} to: '{}'",
                                        rid, field, value
                                    ),
                                );
                                let _ = view.update_menu_syntax_form_field(
                                    field.as_deref(),
                                    value.clone(),
                                    window,
                                    ctx,
                                );
                                let _ = view.get_filtered_results_cached();
                                ctx.notify();
                            }
                            ref cmd @ ExternalCommand::TriggerBuiltin { .. } => {
                                // Canonical dispatch lives in the shared helper — see
                                // src/app_impl/trigger_builtin_dispatch.rs. This
                                // file is only consumed by the source-audit tests
                                // in src/app_impl/tests.rs, so keep it in lock-step
                                // with app_run_setup.rs.
                                logging::log("STDIN", "Triggering built-in (see structured logs)");
                                let _ = view.dispatch_trigger_builtin(cmd, window, ctx);
                                let _ = view
                                    .rekey_main_automation_surface_after_trigger_builtin_dispatch();
                            }

                            ExternalCommand::SimulateKey { ref key, ref modifiers, ref target, ref request_id } => {
                                // SimulateKey: Enter - accept menu-syntax picker
                                // SimulateKey: Enter - execute selected
                                let simulate_key_response = request_id
                                    .as_ref()
                                    .and_then(|rid| {
                                        view.response_sender
                                            .clone()
                                            .map(|sender| (rid.to_string(), sender))
                                    });
                                view.dispatch_simulate_key(
                                    window,
                                    ctx,
                                    crate::simulate_key_dispatch::SimulatedKeyInput {
                                        key,
                                        modifiers,
                                        target: target.as_ref(),
                                     },
                                );
                                if let Some((rid, sender)) = simulate_key_response {
                                    let _ = sender.try_send(
                                        crate::protocol::Message::external_command_result(
                                            rid,
                                            "simulateKey".to_string(),
                                            true,
                                            None,
                                            None,
                                        ),
                                    );
                                }
                            }

                            ExternalCommand::SimulateMainHotkeyGesture { ref phase, ref request_id } => {
                                let rid = request_id.as_deref().unwrap_or("-");
                                logging::log(
                                    "STDIN",
                                    &format!("[{}] SimulateMainHotkeyGesture phase={}", rid, phase),
                                );
                                let normalized = phase.trim().to_ascii_lowercase();
                                if let Some(hotkey_phase) = match normalized.as_str() {
                                    "down" | "keydown" | "key-down" => {
                                        Some(hotkeys::MainHotkeyPhase::KeyDown)
                                    }
                                    "up" | "keyup" | "key-up" => {
                                        Some(hotkeys::MainHotkeyPhase::KeyUp)
                                    }
                                    _ => None,
                                } {
                                    hotkeys::inject_main_hotkey_phase_for_agentic(hotkey_phase);
                                    if let Some(rid) = request_id {
                                        if let Some(sender) = view.response_sender.clone() {
                                            let _ = sender.try_send(
                                                crate::protocol::Message::external_command_result(
                                                    rid.to_string(),
                                                    "simulateMainHotkeyGesture".to_string(),
                                                    true,
                                                    None,
                                                    None,
                                                ),
                                            );
                                        }
                                    }
                                } else if let Some(rid) = request_id {
                                    if let Some(sender) = view.response_sender.clone() {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "simulateMainHotkeyGesture".to_string(),
                                                false,
                                                Some("invalid_phase".to_string()),
                                                Some(format!(
                                                    "expected 'down' or 'up', got '{}'",
                                                    normalized
                                                )),
                                            ),
                                        );
                                    }
                                }
                            }

                            ExternalCommand::OpenNotes => {
                                logging::log("STDIN", "Opening notes window via stdin command");
                                if let Err(e) = notes::open_notes_window(ctx) {
                                    logging::log("STDIN", &format!("Failed to open notes window: {}", e));
                                }
                            }
                            ExternalCommand::OpenAbout => {
                                logging::log("STDIN", "Opening About surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                view.open_about_surface(
                                    std::sync::Arc::new(std::sync::RwLock::new(
                                        crate::updates::UpdateState::Idle,
                                    )),
                                    ctx,
                                );
                            }
                            ExternalCommand::OpenCreationFeedback { path, receipt_path, receipt_status, verification_status, request_id: _ } => {
                                logging::log("STDIN", "Opening CreationFeedback surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                let artifact_path = path
                                    .map(std::path::PathBuf::from)
                                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp/script-kit-liquid-glass-feedback-fixture.ts"));
                                let payload = crate::prompts::CreationFeedbackPayload::fixture(
                                    artifact_path,
                                    receipt_path.map(std::path::PathBuf::from),
                                    receipt_status
                                        .as_deref()
                                        .map(crate::prompts::CreationFeedbackReceiptStatus::from_fixture_str),
                                    verification_status,
                                );
                                view.open_creation_feedback_payload(payload, ctx);
                            }
                            ExternalCommand::OpenConfirmPrompt { title, body, confirm_text, cancel_text, request_id: _ } => {
                                logging::log("STDIN", "Opening ConfirmPrompt surface via stdin command");
                                script_kit_gpui::set_main_window_visible(true);
                                script_kit_gpui::mark_window_shown();
                                platform::show_main_window_without_activation();
                                window.activate_window();
                                window_ops::queue_move(
                                    gpui::Bounds {
                                        origin: gpui::point(gpui::px(585.), gpui::px(177.)),
                                        size: gpui::size(
                                            gpui::px(750.),
                                            crate::window_resize::height_for_view(
                                                crate::window_resize::ViewType::MainWindow,
                                                0,
                                            ),
                                        ),
                                    },
                                    window,
                                    ctx,
                                );
                                sync_main_automation_window(current_main_automation_bounds(), true, true);
                                let (sender, _receiver) = async_channel::bounded(1);
                                let options = crate::confirm::ParentConfirmOptions {
                                    title: title.unwrap_or_else(|| "Delete saved item?".to_string()).into(),
                                    body: body.unwrap_or_else(|| "This action changes local Script Kit state. Confirm to continue or cancel to return to the launcher.".to_string()).into(),
                                    confirm_text: confirm_text.unwrap_or_else(|| "Delete".to_string()).into(),
                                    cancel_text: cancel_text.unwrap_or_else(|| "Cancel".to_string()).into(),
                                    confirm_variant: gpui_component::button::ButtonVariant::Danger,
                                    width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
                                };
                                view.open_confirm_prompt(options, sender, ctx);
                            }
                            ExternalCommand::OpenAi => {
                                logging::log("STDIN", "Opening Agent Chat via openAi compatibility alias");
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenAgentChatDetachedFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let result = view.open_detached_agent_chat_mock_fixture(ctx);
                                tracing::info!(
                                    category = "STDIN",
                                    event = "agent_chat_detached_fixture_opened",
                                    command = "openAgentChatDetachedFixture",
                                    request_id = ?rid,
                                    ok = result.as_ref().map(|moved| *moved).unwrap_or(false),
                                    error = result.err().map(|err| err.to_string()),
                                    "Detached Agent Chat fixture open result"
                                );
                            }
                            ExternalCommand::OpenAgentChatHistoryPopupFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let opened = crate::ai::agent_chat::ui::chat_window::open_agent_chat_history_popup_fixture(ctx);
                                tracing::info!(
                                    category = "STDIN",
                                    event = "agent_chat_history_popup_fixture_opened",
                                    command = "openAgentChatHistoryPopupFixture",
                                    request_id = ?rid,
                                    opened,
                                    "Detached Agent Chat history popup fixture result"
                                );
                            }
                            ExternalCommand::OpenChatPromptFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                view.handle_prompt_message(
                                    PromptMessage::ShowChat {
                                        id: "fixture:ordinary-chat-prompt".to_string(),
                                        placeholder: Some("Message".to_string()),
                                        messages: vec![crate::protocol::ChatPromptMessage::assistant(
                                            "Fixture response",
                                        )],
                                        hint: Some("Ordinary Script ChatPrompt".to_string()),
                                        footer: None,
                                        actions: None,
                                        model: None,
                                        models: Vec::new(),
                                        save_history: false,
                                        use_builtin_ai: false,
                                    },
                                    ctx,
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "chat_prompt_fixture_opened",
                                    command = "openChatPromptFixture",
                                    request_id = ?rid,
                                );
                            }
                            ExternalCommand::ClosePromptPopupNatively { ref target, ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::components::inline_popup_window::close_prompt_popup_target_natively(target, ctx) {
                                    Ok((id, generation, native_window_number)) => tracing::info!(
                                        category = "STDIN",
                                        event = "prompt_popup_native_close_requested",
                                        command = "closePromptPopupNatively",
                                        request_id = ?rid,
                                        window_id = %id,
                                        generation,
                                        native_window_number,
                                    ),
                                    Err(error) => tracing::warn!(
                                        category = "STDIN",
                                        event = "prompt_popup_native_close_refused",
                                        command = "closePromptPopupNatively",
                                        request_id = ?rid,
                                        error = %error,
                                    ),
                                }
                            }
                            ExternalCommand::OpenMiniAi => {
                                logging::log("STDIN", "Opening Agent Chat via openMiniAi compatibility alias");
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenAiWithMockData => {
                                logging::log("STDIN", "Opening standard Agent Chat mock fixture");
                                view.open_standard_agent_chat_mock_fixture(ctx);
                            }
                            ExternalCommand::OpenMiniAiWithMockData => {
                                logging::log(
                                    "STDIN",
                                    "Ignoring deprecated mini mock-data AI alias and opening Agent Chat",
                                );
                                view.open_tab_ai_agent_chat_with_entry_intent(None, ctx);
                            }
                            ExternalCommand::OpenFocusedTextAgentChatWithMockData { text, instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat mock fixture");
                                let text_length = text.as_ref().map(|value| value.len()).unwrap_or("Hello world".len());
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_fixture(
                                    text,
                                    instruction,
                                    "focused_text_mock_fixture",
                                    ctx,
                                );
                                let ok = result.is_ok();
                                if let Err(error) = result {
                                    logging::log(
                                        "STDIN",
                                        &format!("Failed to open focused-text Agent Chat mock fixture: {error}"),
                                    );
                                }
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "mock".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                if ok { None } else { Some("open_failed".to_string()) },
                                                if ok {
                                                    None
                                                } else {
                                                    Some("Focused-text Agent Chat mock fixture open failed".to_string())
                                                },
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData { instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat live mock fixture");
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_from_focused_field_mock_fixture(
                                    instruction,
                                    ctx,
                                );
                                let (ok, text_length, error_code, error_message) = match result {
                                    Ok(text_length) => (true, text_length, None, None),
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to open focused-text Agent Chat live mock fixture: {error}"),
                                        );
                                        let error_code = if error.contains("SCRIPT_KIT_FOCUSED_TEXT_LIVE_FIXTURE") {
                                            "gated_off"
                                        } else {
                                            "open_failed"
                                        };
                                        (
                                            false,
                                            0,
                                            Some(error_code.to_string()),
                                            Some("Focused-text Agent Chat live mock fixture open failed".to_string()),
                                        )
                                    }
                                };
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "live-mock".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                error_code,
                                                error_message,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenFocusedTextAgentChatWithPiData { text, instruction, request_id } => {
                                logging::log("STDIN", "Opening focused-text Agent Chat real Pi fixture");
                                let text_length = text.as_ref().map(|value| value.len()).unwrap_or("Hello world".len());
                                let instruction_length = instruction
                                    .as_ref()
                                    .map(|value| value.trim().len())
                                    .unwrap_or(0);
                                let requested_submit = instruction_length > 0;
                                let result = view.open_focused_text_agent_chat_fixture(
                                    text,
                                    instruction,
                                    "focused_text_pi_fixture",
                                    ctx,
                                );
                                let ok = result.is_ok();
                                let (error_code, error_message) = match result {
                                    Ok(()) => (None, None),
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to open focused-text Agent Chat real Pi fixture: {error}"),
                                        );
                                        let error_text = error.to_string();
                                        if error_text.contains("SCRIPT_KIT_INLINE_AGENT_REAL_PI_FIXTURE") {
                                            (
                                                Some("gated_off".to_string()),
                                                Some("Focused-text Agent Chat real Pi fixture is gated off".to_string()),
                                            )
                                        } else {
                                            (
                                                Some("open_failed".to_string()),
                                                Some("Focused-text Agent Chat real Pi fixture open failed".to_string()),
                                            )
                                        }
                                    }
                                };
                                if let Some(rid) = request_id {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::focused_text_agent_chat_fixture_open_result(
                                                rid.to_string(),
                                                "pi".to_string(),
                                                ok,
                                                ok && requested_submit,
                                                text_length,
                                                instruction_length,
                                                error_code,
                                                error_message,
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::ShowAiCommandBar => {
                                logging::log("STDIN", "Ignoring showAiCommandBar: legacy AI window removed");
                            }
                            ExternalCommand::SimulateAiKey { key, modifiers } => {
                                logging::log(
                                    "STDIN",
                                    &format!("Ignoring simulateAiKey '{}' (modifiers: {:?}): legacy AI window removed", key, modifiers),
                                );
                            }
                            ExternalCommand::CaptureWindow { title, path } => {
                                // Extend grace period to prevent auto-hide during capture.
                                script_kit_gpui::mark_window_shown();
                                logging::log("STDIN", &format!("Capturing window with title '{}' to '{}'", title, path));
                                match validate_capture_window_output_path(&path) {
                                    Ok(validated_path) => {
                                        match capture_window_by_title_via_resolver(&title, false) {
                                            Ok((png_data, width, height)) => {
                                                let mut can_write = true;
                                                if let Some(parent) = validated_path.parent() {
                                                    if let Err(e) = std::fs::create_dir_all(parent) {
                                                        can_write = false;
                                                        logging::log(
                                                            "STDIN",
                                                            &format!(
                                                                "Failed to create screenshot directory '{}': {}",
                                                                parent.display(),
                                                                e
                                                            ),
                                                        );
                                                    }
                                                }

                                                if can_write {
                                                    if let Err(e) = std::fs::write(&validated_path, &png_data) {
                                                        logging::log(
                                                            "STDIN",
                                                            &format!("Failed to write screenshot: {}", e),
                                                        );
                                                    } else {
                                                        logging::log(
                                                            "STDIN",
                                                            &format!(
                                                                "Screenshot saved: {} ({}x{})",
                                                                validated_path.display(),
                                                                width,
                                                                height
                                                            ),
                                                        );
                                                    }
                                                } else {
                                                    tracing::warn!(
                                                        category = "STDIN",
                                                        event_type = "stdin_capture_window_dir_create_failed",
                                                        requested_path = %path,
                                                        resolved_path = %validated_path.display(),
                                                        correlation_id = %logging::current_correlation_id(),
                                                        "Skipping screenshot write due to directory creation failure"
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    category = "STDIN",
                                                    event_type = "stdin_capture_window_failed",
                                                    requested_title = %title,
                                                    requested_path = %path,
                                                    error = %e,
                                                    correlation_id = %logging::current_correlation_id(),
                                                    "captureWindow failed before writing screenshot"
                                                );
                                                logging::log("STDIN", &format!("Failed to capture window: {}", e));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let correlation_id = logging::current_correlation_id();
                                        tracing::warn!(
                                            category = "STDIN",
                                            event_type = "stdin_capture_window_path_rejected",
                                            requested_path = %path,
                                            reason = %e,
                                            correlation_id = %correlation_id,
                                            "Rejected captureWindow output path"
                                        );
                                        logging::log(
                                            "STDIN",
                                            &format!("Rejected captureWindow path '{}': {}", path, e),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAiSearch { text, ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_received",
                                    command = "setAiSearch",
                                    request_id = ?request_id,
                                    text_len = text.len(),
                                    "STDIN AI command received"
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_finished",
                                    command = "setAiSearch",
                                    request_id = ?request_id,
                                    status = "unsupported",
                                    "setAiSearch removed with the legacy AI window"
                                );
                            }
                            ExternalCommand::SetAiInput { text, submit, ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_received",
                                    command = "setAiInput",
                                    request_id = ?request_id,
                                    submit,
                                    text_len = text.len(),
                                    "STDIN AI command received"
                                );
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_ai_command_finished",
                                    command = "setAiInput",
                                    request_id = ?request_id,
                                    submit,
                                    status = "unsupported",
                                    "setAiInput removed with the legacy AI window; use setAgentChatInput"
                                );
                            }
                            ExternalCommand::SetAgentChatInput { text, submit, ref request_id } => {
                                let request_id_value = request_id.clone();
                                let request_id = request_id_value.as_deref();
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "setAgentChatInput",
                                    request_id = ?request_id,
                                    submit,
                                    text_len = text.len(),
                                    "STDIN Agent Chat command received"
                                );
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.set_input_in_window(text.clone(), window, cx);
                                            if submit {
                                                if chat.is_focused_text_mini() {
                                                    let _ = chat.submit_focused_text_from_enter(cx);
                                                } else if let Some(t) = chat.thread() {
                                                    let _ = t.update(cx, |thread, cx| thread.submit_input(cx));
                                                }
                                            }
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                match &result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatInput",
                                            request_id = ?request_id,
                                            submit,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to set Agent Chat input: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatInput",
                                            request_id = ?request_id,
                                            submit,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatInput".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAgentChatScopeInput {
                                ref text,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.scope_input = crate::ai::agent_chat::ui::view::AgentChatView::normalize_focused_text_scope_input_public(text);
                                            cx.notify();
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatScopeInput".to_string(),
                                                result.is_ok(),
                                                result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SelectAgentChatVariation {
                                index,
                                edit,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.select_focused_text_variation(index, cx);
                                            if edit {
                                                let _ = chat.enter_focused_text_variation_editor(cx);
                                            }
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "selectAgentChatVariation".to_string(),
                                                result.is_ok(),
                                                result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::GetAgentChatVariations {
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let variations_summary = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        let snapshots = entity.read(ctx).focused_text_variation_snapshots();
                                        let summaries: Vec<String> = snapshots
                                            .iter()
                                            .map(|s| {
                                                // Char-boundary-safe: `&s.text[..100]`
                                                // panicked on multibyte field text.
                                                let preview =
                                                    crate::logging::truncate_for_log(&s.text, 100);
                                                format!(
                                                    "[{}] {} ({}) sel={} len={}: {}",
                                                    s.index, s.label, s.status.state_id(),
                                                    s.selected, s.text.len(), preview
                                                )
                                            })
                                            .collect();
                                        if summaries.is_empty() {
                                            "no_variations".to_string()
                                        } else {
                                            summaries.join(" | ")
                                        }
                                    }
                                    _ => "not_agent_chat".to_string(),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "getAgentChatVariations".to_string(),
                                                true,
                                                None,
                                                Some(variations_summary),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::AgentChatEscape {
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.handle_protocol_escape(window, cx);
                                        });
                                        Ok(())
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "agent_chatEscape".to_string(),
                                                result.is_ok(),
                                                result.as_ref().err().map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAgentChatTestFixture {
                                ref phase,
                                ref user_text,
                                ref assistant_text,
                                ref message_count,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let request_id = request_id_value.as_deref();
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "setAgentChatTestFixture",
                                    request_id = ?request_id,
                                    phase = %phase,
                                    user_text_len = user_text.as_ref().map(|text| text.len()).unwrap_or(0),
                                    assistant_text_len = assistant_text.as_ref().map(|text| text.len()).unwrap_or(0),
                                    message_count = message_count.unwrap_or(0),
                                    "STDIN Agent Chat command received"
                                );
                                let result = if let Some(origin) = phase.strip_prefix("c06AgentReturn:") {
                                    view.set_agent_chat_return_route_fixture(origin, ctx)
                                } else if let Some(origin) = phase.strip_prefix("c06FlowReturn:") {
                                    view.set_flow_conversation_return_route_fixture(origin, ctx)
                                } else {
                                    match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.apply_test_fixture(
                                                phase,
                                                user_text.clone(),
                                                assistant_text.clone(),
                                                *message_count,
                                                cx,
                                            )
                                        })
                                    }
                                    AppView::FlowSessionView { .. } => view
                                        .apply_flow_conversation_test_fixture(
                                            phase,
                                            user_text.clone(),
                                            assistant_text.clone(),
                                            ctx,
                                        ),
                                    AppView::ChatPrompt { entity, .. } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.apply_transcript_geometry_fixture(
                                                phase,
                                                user_text.clone(),
                                                assistant_text.clone(),
                                                cx,
                                            )
                                        })
                                    }
                                    _ => crate::ai::agent_chat::ui::chat_window::apply_detached_agent_chat_test_fixture(
                                        phase,
                                        user_text.clone(),
                                        assistant_text.clone(),
                                        *message_count,
                                        ctx,
                                    ),
                                    }
                                };
                                match &result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatTestFixture",
                                            request_id = ?request_id,
                                            phase = %phase,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to set Agent Chat test fixture: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "setAgentChatTestFixture",
                                            request_id = ?request_id,
                                            phase = %phase,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatTestFixture".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::SetAgentChatTranscriptScroll {
                                item_ix,
                                offset_px,
                                ref request_id,
                            } => {
                                let request_id_value = request_id.clone();
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        entity.update(ctx, |chat, cx| {
                                            chat.scroll_test_transcript_to(item_ix, offset_px, cx)
                                        })
                                    }
                                    AppView::FlowSessionView { session_id } => view
                                        .conversations.flow_sessions
                                        .iter()
                                        .find(|(meta, _)| meta.id == *session_id)
                                        .map(|(_, entity)| entity.clone())
                                        .ok_or_else(|| "FlowSession entity is not active".to_string())
                                        .map(|entity| {
                                            entity.update(ctx, |chat, cx| {
                                                chat.set_transcript_geometry_scroll(
                                                    item_ix, offset_px, cx,
                                                );
                                            });
                                        }),
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                if let Some(rid) = request_id_value {
                                    if let Some(ref sender) = view.response_sender {
                                        let _ = sender.try_send(
                                            crate::protocol::Message::external_command_result(
                                                rid.to_string(),
                                                "setAgentChatTranscriptScroll".to_string(),
                                                result.is_ok(),
                                                result
                                                    .as_ref()
                                                    .err()
                                                    .map(|_| "agent_chat_inactive".to_string()),
                                                result.as_ref().err().cloned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            ExternalCommand::PasteClipboardIntoAgentChat { ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "stdin_agent_chat_command_received",
                                    command = "pasteClipboardIntoAgentChat",
                                    request_id = ?request_id,
                                    "STDIN Agent Chat command received"
                                );
                                let result = match &view.current_view {
                                    AppView::AgentChatView { entity } => {
                                        let entity = entity.clone();
                                        let pasted = entity
                                            .update(ctx, |chat, cx| chat.paste_text_from_clipboard(cx));
                                        if pasted {
                                            Ok(())
                                        } else {
                                            Err("clipboard is empty or text fetch failed"
                                                .to_string())
                                        }
                                    }
                                    _ => Err("Agent Chat view is not active".to_string()),
                                };
                                match result {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "pasteClipboardIntoAgentChat",
                                            request_id = ?request_id,
                                            status = "success",
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                    Err(error) => {
                                        logging::log(
                                            "STDIN",
                                            &format!("Failed to paste clipboard into Agent Chat: {}", error),
                                        );
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "stdin_agent_chat_command_finished",
                                            command = "pasteClipboardIntoAgentChat",
                                            request_id = ?request_id,
                                            status = "error",
                                            error = %error,
                                            "STDIN Agent Chat command finished"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::PushDictationResult {
                                ref transcript,
                                ref partial_transcript,
                                ref target,
                                freeze_only,
                                use_frozen_selection,
                                ref request_id,
                            } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                let target_label = target.as_deref().unwrap_or("unspecified");
                                let resolution =
                                    crate::dictation::resolve_final_or_partial_transcript(
                                        transcript,
                                        partial_transcript.as_deref(),
                                    );
                                match view.deliver_stdin_dictation_result(
                                    transcript.clone(),
                                    partial_transcript.as_deref(),
                                    target.as_deref(),
                                    freeze_only,
                                    use_frozen_selection,
                                    ctx,
                                ) {
                                    Ok(delivery_target) => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "push_dictation_result_delivered",
                                            command = "pushDictationResult",
                                            request_id = ?rid,
                                            transcript_len = resolution.transcript.as_ref().map_or(0, String::len),
                                            final_transcript_len = resolution.final_len,
                                            partial_transcript_len = ?resolution.partial_len,
                                            partial_fallback_used = resolution.used_partial_fallback,
                                            requested_target = target_label,
                                            delivery_target = ?delivery_target,
                                            "pushDictationResult RPC delivered through dictation pipeline"
                                        );
                                    }
                                    Err(error) => {
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "push_dictation_result_failed",
                                            command = "pushDictationResult",
                                            request_id = ?rid,
                                            transcript_len = resolution.transcript.as_ref().map_or(0, String::len),
                                            final_transcript_len = resolution.final_len,
                                            partial_transcript_len = ?resolution.partial_len,
                                            partial_fallback_used = resolution.used_partial_fallback,
                                            requested_target = target_label,
                                            error = %error,
                                            "pushDictationResult RPC failed"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::GetAiWindowState { ref request_id } => {
                                let request_id = request_id.as_ref().map(|id| id.as_str());
                                tracing::info!(
                                    category = "STDIN",
                                    event = "ai_window_state_result",
                                    command = "getAiWindowState",
                                    request_id = ?request_id,
                                    ok = false,
                                    error_code = "ai_window_removed",
                                    "legacy AI window removed"
                                );
                            }
                            ExternalCommand::OpenDictationOverlayFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                crate::dictation::set_dictation_overlay_fixture_mode(true);
                                match crate::dictation::open_dictation_overlay(ctx) {
                                    Ok(handle) => {
                                        let fixture_bounds = gpui::Bounds {
                                            origin: gpui::point(gpui::px(585.0), gpui::px(177.0)),
                                            size: gpui::size(gpui::px(560.0), gpui::px(100.0)),
                                        };
                                        let _ = handle.update(ctx, |_view, window, cx| {
                                            crate::components::inline_popup_window::set_inline_popup_window_bounds(window, fixture_bounds, cx);
                                        });
                                        crate::windows::set_automation_bounds(
                                            "dictation",
                                            Some(crate::protocol::AutomationWindowBounds {
                                                x: 585.0,
                                                y: 177.0,
                                                width: 560.0,
                                                height: 100.0,
                                            }),
                                        );
                                        let state = crate::dictation::DictationOverlayState {
                                            phase: crate::dictation::DictationSessionPhase::Recording,
                                            elapsed: std::time::Duration::from_secs(7),
                                            bars: [0.12, 0.34, 0.62, 0.88, 0.55, 0.31, 0.74, 0.42, 0.18],
                                            transcript: gpui::SharedString::default(),
                                            target: crate::dictation::DictationTarget::ExternalApp,
                                        };
                                        let _ = crate::dictation::update_dictation_overlay(state, ctx);
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "dictation_overlay_fixture_opened",
                                            command = "openDictationOverlayFixture",
                                            request_id = ?rid,
                                            "Dictation overlay fixture opened without media capture"
                                        );
                                    }
                                    Err(error) => {
                                        crate::dictation::set_dictation_overlay_fixture_mode(false);
                                        tracing::error!(
                                            category = "STDIN",
                                            event = "dictation_overlay_fixture_failed",
                                            command = "openDictationOverlayFixture",
                                            request_id = ?rid,
                                            error = %error,
                                            "Dictation overlay fixture failed"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::OpenDictationMicrophonePopupFixture { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::dictation::open_dictation_microphone_popup_fixture(ctx) {
                                    Ok(()) => tracing::info!(
                                        category = "STDIN",
                                        event = "dictation_microphone_popup_fixture_opened",
                                        command = "openDictationMicrophonePopupFixture",
                                        request_id = ?rid,
                                        "Dictation microphone popup fixture opened without persistence"
                                    ),
                                    Err(error) => tracing::error!(
                                        category = "STDIN",
                                        event = "dictation_microphone_popup_fixture_failed",
                                        command = "openDictationMicrophonePopupFixture",
                                        request_id = ?rid,
                                        error = %error,
                                        "Dictation microphone popup fixture failed"
                                    ),
                                }
                            }
                            ExternalCommand::GetConfigFingerprint { ref request_id } => {
                                let rid = request_id.as_ref().map(|id| id.as_str());
                                match crate::config::current_config_fingerprint_receipt() {
                                    Some(receipt) => {
                                        let json = serde_json::to_string(&receipt).unwrap_or_default();
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "config_fingerprint_result",
                                            command = "getConfigFingerprint",
                                            request_id = ?rid,
                                            ok = true,
                                            state = %json,
                                            "config.ts fingerprint snapshot"
                                        );
                                    }
                                    None => {
                                        tracing::info!(
                                            category = "STDIN",
                                            event = "config_fingerprint_result",
                                            command = "getConfigFingerprint",
                                            request_id = ?rid,
                                            ok = false,
                                            error_code = "config_file_missing",
                                            "config.ts not found or metadata unreadable"
                                        );
                                    }
                                }
                            }
                            ExternalCommand::ShowGrid { grid_size, show_bounds, show_box_model, show_alignment_guides, show_dimensions, ref depth } => {
                                logging::log("STDIN", &format!(
                                    "ShowGrid: size={}, bounds={}, box_model={}, guides={}, dimensions={}, depth={:?}",
                                    grid_size, show_bounds, show_box_model, show_alignment_guides, show_dimensions, depth
                                ));
                                let options = protocol::GridOptions {
                                    grid_size,
                                    show_bounds,
                                    show_box_model,
                                    show_alignment_guides,
                                    show_dimensions,
                                    depth: depth.clone(),
                                    color_scheme: None,
                                };
                                view.show_grid(options, ctx);
                            }
                            ExternalCommand::HideGrid => {
                                logging::log("STDIN", "HideGrid: hiding debug grid overlay");
                                view.hide_grid(ctx);
                            }
                            ExternalCommand::ExecuteFallback { ref fallback_id, ref input } => {
                                logging::log("STDIN", &format!("ExecuteFallback: id='{}', input='{}'", fallback_id, input));
                                execute_fallback_action(view, fallback_id, input, window, ctx);
                            }
                            ExternalCommand::ShowShortcutRecorder { ref command_id, ref command_name } => {
                                logging::log("STDIN", &format!("ShowShortcutRecorder: command_id='{}', command_name='{}'", command_id, command_name));
                                view.show_shortcut_recorder(command_id.clone(), command_name.clone(), window, ctx);
                            }
                        },
                        StdinCommand::Protocol(message) => {
                            logging::log("STDIN", "Routing stdin protocol message");
                            view.handle_stdin_protocol_message(*message, window, ctx);
                        }

                    }
                    view.sync_main_footer_popup(window, ctx);
                    ctx.notify();
                }); // close app_entity_inner.update
            }); // close window_for_stdin.update
        }); // close cx.update
    }

    logging::log("STDIN", "Async stdin command handler exiting");
})
.detach();
