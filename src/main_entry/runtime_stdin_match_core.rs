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
                                platform::ensure_main_panel_configured(
                                    "runtime_stdin_match_core::run",
                                );

                                // Show window WITHOUT activating (floating panel behavior)
                                platform::show_main_window_without_activation();
                                window.activate_window();

                                // Send AI window to back so it doesn't come forward with main menu
                                platform::send_ai_window_to_back();

                                let focus_handle = view.focus_handle(ctx);
                                window.focus(&focus_handle, ctx);
                                sync_main_automation_window(None, true, true);

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
                                platform::ensure_main_panel_configured(
                                    "runtime_stdin_match_core::show",
                                );

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
