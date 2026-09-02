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
                                let outcome = view.dispatch_simulate_key(
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
                                            outcome.status == crate::action_helpers::ActionOutcomeStatus::Success,
                                            outcome.error_code.map(str::to_string),
                                            outcome.user_message.or(outcome.detail),
                                        ),
                                    );
                                }
                            }
