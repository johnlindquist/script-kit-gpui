use super::types::conversation_turn_pending_indicator_visible;
#[cfg(test)]
use super::types::conversation_turn_streaming_copy_available;
use super::*;
use std::borrow::Cow;

/// The SINGLE decision of whether a turn draws an assistant answer region, and
/// what markdown that region contains.
///
/// Both the renderer and `reconcile_assistant_text_views` call this. They used
/// to decide separately, which is a silent-degradation trap: the reconciler
/// building a state for a region the renderer skips just wastes memory, but the
/// renderer drawing a region the reconciler skipped falls back to the old
/// unselectable path — visually almost identical, and the exact defect this
/// work exists to remove. Returning `None` for "no region" makes the two
/// impossible to disagree.
pub(super) fn assistant_answer_region_source(
    turn: &ConversationTurn,
    script_generation_mode: bool,
) -> Option<Cow<'_, str>> {
    if turn.failure.is_some() || turn.error.is_some() {
        // Failure rows show a safe partial answer ONLY when there is one; the
        // recovery card carries the rest and is not markdown.
        let response = turn.assistant_response.as_deref()?;
        if response.trim().is_empty() {
            return None;
        }
        Some(super::types::assistant_response_markdown_source(
            script_generation_mode,
            response,
        ))
    } else if turn.assistant_response.is_some() || turn.streaming {
        // A freshly submitted turn is `streaming` with NO response yet. It
        // still draws a region ("_Thinking…_"), so it still needs a state.
        let response = turn.assistant_response.as_deref().unwrap_or("");
        Some(assistant_response_region_source(
            script_generation_mode,
            response,
            turn.streaming,
        ))
    } else {
        None
    }
}

pub(super) fn assistant_response_region_source<'a>(
    script_generation_mode: bool,
    response: &'a str,
    streaming: bool,
) -> Cow<'a, str> {
    match (response.trim().is_empty(), streaming) {
        (true, true) => Cow::Borrowed("_Thinking…_"),
        (true, false) => Cow::Borrowed("_No response returned._"),
        (false, _) => {
            super::types::assistant_response_markdown_source(script_generation_mode, response)
        }
    }
}

impl ChatPrompt {
    /// Render one assistant answer region through the SHARED conversation
    /// renderer, so Flow answers are selectable and styled identically to
    /// Agent Chat's.
    ///
    /// Falls back to the legacy bespoke renderer only when this turn has no
    /// reconciled `TextViewState`. That should not happen —
    /// `reconcile_assistant_text_views` mirrors the regions this function
    /// draws — but a missing state must degrade to visible-but-unselectable
    /// text rather than a blank answer. `expect()` here would turn a
    /// reconciliation gap into a crash in front of the user.
    fn render_answer_region(
        &self,
        turn: &ConversationTurn,
        source: &str,
        colors: &theme::PromptColors,
    ) -> gpui::AnyElement {
        match self.assistant_text_views.get(&turn.render_key) {
            Some(state) => crate::components::conversation_text::conversation_markdown_view(
                state,
                &self.theme,
                colors,
                gpui::rgb(colors.text_primary),
                &crate::components::conversation_style::production_conversation_style(),
                format!(
                    "chat-prompt/{}/turn/{}/assistant",
                    self.id,
                    turn.render_key.as_str()
                )
                .into(),
                gpui_component::text::MarkdownLinkLabelPolicy::Preserve,
            )
            .into_any_element(),
            None => render_markdown(source, colors).into_any_element(),
        }
    }

    pub(super) fn render_turn(
        &self,
        turn: &ConversationTurn,
        turn_index: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let colors = &self.prompt_colors;
        let theme_colors = &self.theme.colors;

        // VIBRANCY: Use theme-aware overlay for subtle lift that lets blur show through
        // Dark mode: white overlay brightens; Light mode: much subtler black overlay
        let container_bg = if self.theme.is_dark_mode() {
            theme::hover_overlay_bg(&self.theme, 0x15) // ~8% white overlay for dark mode
        } else {
            theme::hover_overlay_bg(&self.theme, 0x08) // ~3% black overlay for light mode
        };
        let copy_hover_bg = theme::hover_overlay_bg(&self.theme, 0x28); // ~16% for hover
        let error_color = theme_colors.ui.error;
        let user_fidelity_id = format!("chat-transcript-user-turn-{turn_index}");
        let response_fidelity_id = format!("chat-transcript-response-turn-{turn_index}");
        let pending_fidelity_id = format!("chat-transcript-pending-turn-{turn_index}");
        let copy_fidelity_id = format!("chat-transcript-copy-turn-{turn_index}");

        let mut content = div().flex().flex_col().gap(px(6.0)).w_full().min_w_0();
        // Note: removed overflow_hidden() to allow text to wrap naturally

        // User prompt (small, bold) - only if not empty
        if !turn.user_prompt.is_empty() {
            content = content.child(
                div()
                    .debug_selector(move || user_fidelity_id)
                    .w_full()
                    .min_w_0()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(theme_colors.text.secondary))
                    .child(turn.user_prompt.clone()),
            );
        }

        // User image thumbnail (if attached)
        if let Some(ref user_image) = turn.user_image {
            let render_img = user_image.clone();
            content = content.child(
                img(move |_window: &mut Window, _cx: &mut App| Some(Ok(render_img.clone())))
                    .w(px(64.))
                    .h(px(64.))
                    .rounded_sm(),
            );
        }

        // Failure state (S10): safe partial response + the SHARED recovery
        // card. No raw detail is ever rendered inline — diagnostics stay
        // behind the redacted CopyDetails action.
        if turn.failure.is_some() || turn.error.is_some() {
            // Any safe partial assistant response stays visible above the card.
            if let Some(markdown_response) =
                assistant_answer_region_source(turn, self.script_generation_mode)
            {
                content =
                    content.child(div().w_full().min_w_0().overflow_x_hidden().child(
                        self.render_answer_region(turn, markdown_response.as_ref(), colors),
                    ));
            }
            match turn.failure.as_ref() {
                Some(failure) if !self.host_mode.is_transcript_only() => {
                    content = content.child(self.render_turn_recovery_card(
                        failure,
                        turn.message_id.clone(),
                        turn_index,
                        cx,
                    ));
                }
                Some(failure) => {
                    // TranscriptOnly hosting (a flow session): the HOST owns
                    // the actionable recovery card; the transcript row keeps
                    // only the safe failure copy. Intentional divergence per
                    // the S10 contract — ChatPrompt must not invent Flow
                    // retry behavior.
                    let error_fidelity_id = response_fidelity_id.clone();
                    content = content.child(
                        div()
                            .debug_selector(move || error_fidelity_id)
                            .text_sm()
                            .text_color(rgb(error_color))
                            .child(
                                crate::ai::reliability::primary_message_for_failure(failure)
                                    .to_string(),
                            ),
                    );
                }
                None => {
                    // Defensive: every ingestion path classifies raw error
                    // strings, so an untyped error should not exist. Render
                    // stable safe copy — never the raw string.
                    let error_fidelity_id = response_fidelity_id.clone();
                    content = content.child(
                        div()
                            .debug_selector(move || error_fidelity_id)
                            .text_sm()
                            .text_color(rgb(error_color))
                            .child("The AI request did not finish. Your work is saved."),
                    );
                }
            }
        }
        // AI response (only show if no error, or show partial if stream interrupted)
        else if let Some(markdown_response) =
            assistant_answer_region_source(turn, self.script_generation_mode)
        {
            // Empty pending, first text, and terminal-empty responses all use
            // the same markdown response region. Streaming activity lives in
            // the card's fixed trailing slot, so it cannot add a row below it.
            content = content.child(
                div()
                    .debug_selector(move || response_fidelity_id)
                    .w_full()
                    .min_w_0()
                    .overflow_x_hidden()
                    .child(self.render_answer_region(turn, markdown_response.as_ref(), colors)),
            );
        }

        let show_streaming_indicator = conversation_turn_pending_indicator_visible(turn);
        let trailing_control = div()
            .id(format!("copy-turn-{}", turn_index))
            .debug_selector(move || copy_fidelity_id)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(px(24.0))
            .h(px(24.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .opacity(0.7)
            .hover(|s| s.opacity(1.0).bg(copy_hover_bg))
            .child(
                svg()
                    .path(IconName::Copy.asset_path())
                    .size(px(16.))
                    .text_color(rgb(theme_colors.text.secondary)),
            )
            .when(show_streaming_indicator, |slot| {
                slot.child(
                    div()
                        .debug_selector(move || pending_fidelity_id)
                        .absolute()
                        .right(px(1.0))
                        .bottom(px(1.0))
                        .size(px(7.0))
                        .rounded(px(999.0))
                        .bg(rgb(theme_colors.accent.selected))
                        .with_animation(
                            ("chat-turn-streaming-dot-pulse", turn_index),
                            Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                            |style, delta| {
                                let sine = (delta * std::f32::consts::PI * 2.0).sin();
                                style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                            },
                        ),
                )
            })
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.copy_turn_response(turn_index, cx);
            }));

        // The full-width container with copy button
        div()
            .w_full()
            .px(px(CHAT_LAYOUT_CARD_PADDING_X))
            .py(px(CHAT_LAYOUT_CARD_PADDING_Y))
            .bg(container_bg)
            .rounded(px(8.0))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .child(content.flex_1())
            .child(trailing_control)
    }

    /// Recovery actions this ChatPrompt host can actually perform (S10):
    /// Retry needs the host retry callback; model/provider/auth/config
    /// actions need the host recovery callback; CopyDetails is always safe.
    pub(crate) fn turn_recovery_capabilities(
        &self,
    ) -> crate::ai::reliability::SurfaceRecoveryCapabilities {
        use sk_protocol::ai_reliability::RecoveryActionKind;
        let mut kinds = vec![RecoveryActionKind::CopyDetails];
        if self.on_retry.is_some() {
            kinds.push(RecoveryActionKind::Retry);
        }
        if self.on_recovery.is_some() {
            kinds.extend([
                RecoveryActionKind::ChooseCompatibleModel,
                RecoveryActionKind::ChooseProvider,
                RecoveryActionKind::UpdateClient,
                RecoveryActionKind::CheckAgain,
                RecoveryActionKind::SignIn,
                RecoveryActionKind::SwitchAccount,
                RecoveryActionKind::ConfigureProvider,
                RecoveryActionKind::RepairComponent,
                RecoveryActionKind::TrimContext,
                // A flow conversation's engine death is repaired by starting
                // a fresh thread. The flow session host performs it in
                // `dispatch_flow_recovery_action`; omitting it here hid the
                // button on exactly the failure it fixes.
                RecoveryActionKind::RethreadFlow,
            ]);
        }
        crate::ai::reliability::SurfaceRecoveryCapabilities::only(kinds)
            .layout(crate::ai::reliability::AiRecoveryLayout::TranscriptCard)
    }

    /// One shared recovery card for a failed transcript turn (S10). Same
    /// anatomy, copy, and `ai-recovery-*` semantic ids as Agent Chat,
    /// Quick AI, and Flow sessions.
    fn render_turn_recovery_card(
        &self,
        failure: &sk_protocol::ai_reliability::AiFailure,
        message_id: Option<String>,
        turn_index: usize,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        use sk_protocol::ai_reliability::{
            AiSurfaceIdentity, AiWorkSnapshot, Fingerprint, ModelId, PreservationReceipt, PromptId,
            WorkKey,
        };
        let identity = AiSurfaceIdentity::LegacyChatPrompt {
            prompt_id: PromptId::from(self.id.as_str()),
            provider_id: None,
            model_id: self.model.as_deref().map(ModelId::from),
        };
        let work =
            AiWorkSnapshot {
                key: WorkKey::from(format!("chat-prompt:{}", self.id)),
                transcript: PreservationReceipt::Preserved {
                    fingerprint: Fingerprint(crate::ai::reliability::redacted_fingerprint(
                        &format!("{}:{}", self.id, self.messages.len()),
                    )),
                },
                draft: PreservationReceipt::NotApplicable,
                attachments: PreservationReceipt::NotApplicable,
                partial_output: PreservationReceipt::NotApplicable,
            };
        let capabilities = self.turn_recovery_capabilities();
        let Some(spec) = crate::ai::reliability::standalone_failure_recovery_spec(
            identity,
            failure,
            work,
            &capabilities,
        ) else {
            return div().into_any_element();
        };
        let action_weak = cx.entity().downgrade();
        let dismiss_weak = cx.entity().downgrade();
        let action_message_id = message_id.clone();
        let handlers = crate::components::AiRecoveryCardHandlers {
            on_action: std::rc::Rc::new(move |action, _window, cx| {
                if let Some(entity) = action_weak.upgrade() {
                    let message_id = action_message_id.clone();
                    entity.update(cx, |this, cx| {
                        this.dispatch_turn_recovery_action(message_id, action, cx);
                    });
                }
            }),
            on_dismiss: Some(std::rc::Rc::new(move |_window, cx| {
                if let Some(entity) = dismiss_weak.upgrade() {
                    let message_id = message_id.clone();
                    entity.update(cx, |this, cx| {
                        if let Some(message_id) = message_id {
                            this.clear_message_error(&message_id, cx);
                        }
                    });
                }
            })),
        };
        // The card is the message. Its actions live in the shared footer rail
        // below it, never as loose buttons inside the transcript.
        let plan = crate::ai::reliability::plan_recovery_presentation(&spec);
        let footer = crate::components::render_ai_recovery_footer(&plan, &handlers, None);
        div()
            .id(("chat-turn-recovery", turn_index))
            .w_full()
            .child(crate::components::render_ai_recovery_card(
                spec,
                &self.theme,
            ))
            .children(footer)
            .into_any_element()
    }

    /// Route one recovery-card action: Retry and CopyDetails are handled
    /// here; everything else goes to the host recovery callback.
    fn dispatch_turn_recovery_action(
        &mut self,
        message_id: Option<String>,
        action: sk_protocol::ai_reliability::AiRecoveryAction,
        cx: &mut Context<Self>,
    ) {
        use sk_protocol::ai_reliability::AiRecoveryAction;
        match action {
            AiRecoveryAction::Retry => {
                if let Some(message_id) = message_id {
                    self.handle_retry(message_id);
                }
            }
            AiRecoveryAction::CopyDetails => {
                let failure = message_id
                    .as_deref()
                    .and_then(|id| {
                        self.messages
                            .iter()
                            .rev()
                            .find(|message| message.id.as_deref() == Some(id))
                    })
                    .and_then(|message| message.failure.as_ref());
                let details = match failure {
                    Some(failure) => format!(
                        "Failure code: {:?}\nSummary: {}\nDiagnostic fingerprint: {}",
                        failure.code,
                        crate::ai::reliability::primary_message_for_failure(failure),
                        failure
                            .diagnostic
                            .as_ref()
                            .map(|diagnostic| diagnostic.fingerprint.0.clone())
                            .unwrap_or_else(|| "unavailable".to_string()),
                    ),
                    None => "No failure details recorded for this turn.".to_string(),
                };
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(details));
            }
            other => {
                if let (Some(message_id), Some(callback)) = (message_id, self.on_recovery.as_ref())
                {
                    callback(message_id, other);
                }
            }
        }
    }

    /// Handle retry for a failed message
    pub(super) fn handle_retry(&self, message_id: String) {
        logging::log(
            "CHAT",
            &format!("Retry requested for message: {}", message_id),
        );
        if let Some(ref callback) = self.on_retry {
            callback(self.id.clone(), message_id);
        }
    }

    /// Copy the assistant response from a specific turn
    pub(super) fn copy_turn_response(&mut self, turn_index: usize, cx: &mut Context<Self>) {
        self.ensure_conversation_turns_cache();
        if let Some(turn) = self.conversation_turns_cache.get(turn_index) {
            if let Some(ref response) = turn.assistant_response {
                let content = response.clone();
                logging::log(
                    "CHAT",
                    &format!(
                        "Copied turn {} response: {} chars",
                        turn_index,
                        content.len()
                    ),
                );
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            } else if !turn.user_prompt.is_empty() {
                // If no assistant response, copy the user prompt
                let content = turn.user_prompt.clone();
                logging::log(
                    "CHAT",
                    &format!(
                        "Copied turn {} user prompt: {} chars",
                        turn_index,
                        content.len()
                    ),
                );
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assistant_answer_region_source, assistant_response_region_source,
        conversation_turn_pending_indicator_visible, conversation_turn_streaming_copy_available,
        ConversationTurn,
    };
    const CHAT_RENDER_TURNS_SOURCE: &str = include_str!("render_turns.rs");

    /// Only the PRODUCTION half of this file.
    ///
    /// A whole-file audit matches the assertion's own string literals, so an
    /// absence check written that way can never pass. Split at the LAST
    /// `#[cfg(test)]` attribute: this file opens with one on line 2 (a
    /// test-only import), so splitting at the first would return an empty
    /// production half and silently pass every absence check.
    fn production_source() -> &'static str {
        let marker = "\n#[cfg(test)]\nmod tests {";
        CHAT_RENDER_TURNS_SOURCE
            .split_once(marker)
            .map(|(production, _)| production)
            .expect("this file ends with a `mod tests` block")
    }

    /// Extract one function's body by brace matching, so an audit can be
    /// scoped to a region instead of sniffing the whole file.
    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("signature `{signature}` should exist"));
        let source = &source[start..];
        let open = source.find('{').expect("function body should open");
        let mut depth = 0usize;
        for (offset, byte) in source.as_bytes()[open..].iter().copied().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("function body should close")
    }

    /// Production source MINUS the shared answer region.
    ///
    /// The "turn chrome uses theme colors" audit below is about chrome — the
    /// user prompt line, error copy, the copy control. It is deliberately NOT
    /// about the assistant answer body: that body is painted by the SHARED
    /// `conversation_markdown_view`, and its text color must be the same value
    /// Agent Chat passes (`rgb(colors.text_primary)`) or the two surfaces stop
    /// matching, which is the whole point of the shared renderer.
    ///
    /// So the exception is ONE enumerated function, carved out structurally.
    /// A new prompt-palette color anywhere else in this file still fails.
    fn chrome_source() -> String {
        let production = production_source();
        let answer_region = function_body(production, "fn render_answer_region(");
        production.replace(answer_region, "")
    }

    /// S10 contract: the failed-turn renderer draws the SHARED recovery card,
    /// never an always-visible raw error detail.
    ///
    /// This stays a source audit only for the POSITIVE half — that the shared
    /// card is what gets rendered — because no cheaper rung can express "this
    /// renderer calls the shared component" without standing up a GPUI window.
    /// The negative half is deliberately NOT asserted here: the string-sniffing
    /// `ChatErrorType::from_error_string` classifier is deleted, so the
    /// compiler already refuses any call to it. See `types.rs` for why it went.
    #[test]
    fn failed_turn_renders_shared_recovery_card() {
        let source = production_source();
        assert!(
            source.contains("render_ai_recovery_card"),
            "failed turns must render the shared recovery card"
        );
        assert!(
            !source.contains("Model unavailable. Using default model"),
            "the false model-fallback copy must stay deleted"
        );
    }

    #[test]
    fn assistant_response_region_has_stable_honest_empty_states() {
        assert_eq!(
            assistant_response_region_source(false, "", true),
            "_Thinking…_"
        );
        assert_eq!(
            assistant_response_region_source(false, "", false),
            "_No response returned._"
        );
        assert_eq!(
            assistant_response_region_source(false, "First token", true),
            "First token"
        );
    }

    #[test]
    fn pending_indicator_hands_off_to_streaming_copy_after_first_visible_text() {
        let turn = |assistant_response: Option<&str>, streaming: bool, error: Option<&str>| {
            ConversationTurn {
                user_prompt: "user".to_string(),
                assistant_response: assistant_response.map(str::to_string),
                model: None,
                streaming,
                error: error.map(str::to_string),
                failure: None,
                message_id: None,
                user_image: None,
                render_key: super::ConversationTurnRenderKey::resolve(None, None, 0),
            }
        };

        for pending in [turn(None, true, None), turn(Some(""), true, None)] {
            assert!(conversation_turn_pending_indicator_visible(&pending));
            assert!(!conversation_turn_streaming_copy_available(&pending));
        }
        for visible in [
            turn(Some("First"), true, None),
            turn(Some("First token"), true, None),
        ] {
            assert!(!conversation_turn_pending_indicator_visible(&visible));
            assert!(conversation_turn_streaming_copy_available(&visible));
        }
        for terminal in [
            turn(Some("done"), false, None),
            turn(None, false, Some("error")),
        ] {
            assert!(!conversation_turn_pending_indicator_visible(&terminal));
            assert!(!conversation_turn_streaming_copy_available(&terminal));
        }
    }

    #[test]
    fn test_render_turn_uses_theme_colors_and_keeps_copy_alignment_without_manual_offset() {
        let legacy_text_pattern = ["rgb(colors.", "text_"].concat();
        let legacy_copy_button_margin = ["copy_button", ".mt(px(1.0))"].concat();

        assert!(
            CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.text.secondary")
                && CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.text.primary")
                && CHAT_RENDER_TURNS_SOURCE.contains("theme_colors.accent.selected"),
            "Turn renderer should use theme color scheme entries for turn text and accents"
        );
        assert!(
            !chrome_source().contains(&legacy_text_pattern),
            "Turn renderer should avoid prompt palette text colors for turn chrome"
        );
        assert!(
            !CHAT_RENDER_TURNS_SOURCE.contains(&legacy_copy_button_margin),
            "Copy button should align without a manual margin offset wrapper"
        );
    }

    fn turn_for_region(
        assistant_response: Option<&str>,
        streaming: bool,
        error: Option<&str>,
    ) -> ConversationTurn {
        ConversationTurn {
            user_prompt: "user".to_string(),
            assistant_response: assistant_response.map(str::to_string),
            model: None,
            streaming,
            error: error.map(str::to_string),
            failure: None,
            message_id: None,
            user_image: None,
            render_key: super::ConversationTurnRenderKey::resolve(None, None, 0),
        }
    }

    /// The renderer draws a region for a just-submitted turn (streaming, no
    /// response yet) — so the reconciler must build a state for it. An earlier
    /// draft of the reconciler skipped `assistant_response == None` entirely,
    /// which left every turn's FIRST frame falling back to the old
    /// unselectable renderer. Both now read this one function.
    #[test]
    fn a_streaming_turn_with_no_response_yet_still_gets_an_answer_region() {
        let turn = turn_for_region(None, true, None);
        let source = assistant_answer_region_source(&turn, false)
            .expect("a pending streaming turn draws the thinking region");
        assert_eq!(source, "_Thinking…_");
    }

    #[test]
    fn a_settled_empty_response_still_gets_an_answer_region() {
        let turn = turn_for_region(Some(""), false, None);
        let source = assistant_answer_region_source(&turn, false)
            .expect("a terminal empty response draws explanatory copy");
        assert_eq!(source, "_No response returned._");
    }

    #[test]
    fn an_untouched_turn_gets_no_answer_region() {
        let turn = turn_for_region(None, false, None);
        assert!(
            assistant_answer_region_source(&turn, false).is_none(),
            "a turn with neither a response nor a stream draws nothing to select"
        );
    }

    /// Failure rows show a partial answer only when one exists; the recovery
    /// card carries everything else and is not markdown.
    #[test]
    fn failure_rows_get_a_region_only_for_a_non_empty_partial_answer() {
        let no_answer = turn_for_region(None, false, Some("boom"));
        assert!(assistant_answer_region_source(&no_answer, false).is_none());
        let blank_answer = turn_for_region(Some("   "), false, Some("boom"));
        assert!(assistant_answer_region_source(&blank_answer, false).is_none());
        let partial_turn = turn_for_region(Some("half an answer"), false, Some("boom"));
        let partial = assistant_answer_region_source(&partial_turn, false)
            .expect("a non-empty partial answer stays visible above the card");
        assert_eq!(partial, "half an answer");
    }

    /// A failed turn must NOT get the `_Thinking…_`/`_No response returned._`
    /// substitutions — those would read as a normal pending answer sitting
    /// above a recovery card.
    #[test]
    fn failure_rows_never_substitute_pending_placeholder_copy() {
        for streaming in [true, false] {
            let turn = turn_for_region(None, streaming, Some("x"));
            assert!(
                assistant_answer_region_source(&turn, false).is_none(),
                "failed turn (streaming={streaming}) must not borrow pending copy"
            );
        }
    }
}
