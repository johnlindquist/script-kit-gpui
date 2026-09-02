// Emoji action handlers for handle_action dispatch.
//
// Contains all `emoji_*` action handling: paste, copy, paste-keep-open, pin, unpin.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmojiPinHandlerAction {
    Pin,
    Unpin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmojiCopyHandlerAction {
    Emoji,
    Unicode,
    Section,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmojiPasteHandlerAction {
    Paste,
    PasteKeepOpen,
}

struct EmojiCopyPayload {
    clipboard_text: String,
    hud_text: String,
}

impl EmojiPasteHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "emoji_paste" => Some(Self::Paste),
            "emoji_paste_keep_open" => Some(Self::PasteKeepOpen),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Paste | Self::PasteKeepOpen => "No emoji selected",
        }
    }

    fn trace_action(self) -> &'static str {
        match self {
            Self::Paste => "emoji_paste",
            Self::PasteKeepOpen => "emoji_paste_keep_open",
        }
    }

    fn close_behavior(self) -> PasteCloseBehavior {
        match self {
            Self::Paste => PasteCloseBehavior::HideWindow,
            Self::PasteKeepOpen => PasteCloseBehavior::KeepWindowOpen,
        }
    }
}

impl EmojiPinHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "emoji_pin" => Some(Self::Pin),
            "emoji_unpin" => Some(Self::Unpin),
            _ => None,
        }
    }

    fn apply(self, pinned_emojis: &mut std::collections::HashSet<String>, emoji_value: &str) {
        match self {
            Self::Pin => {
                pinned_emojis.insert(emoji_value.to_string());
            }
            Self::Unpin => {
                pinned_emojis.remove(emoji_value);
            }
        }
    }

    fn error_prefix(self) -> &'static str {
        match self {
            Self::Pin => "Failed to pin emoji",
            Self::Unpin => "Failed to unpin emoji",
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Pin | Self::Unpin => "No emoji selected",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::Pin | Self::Unpin => format!("{}: {error}", self.error_prefix()),
        }
    }

    fn success_hud(self, emoji_value: &str) -> String {
        match self {
            Self::Pin => format!("Pinned {emoji_value}"),
            Self::Unpin => format!("Unpinned {emoji_value}"),
        }
    }
}

impl EmojiCopyHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "emoji_copy" => Some(Self::Emoji),
            "emoji_copy_unicode" => Some(Self::Unicode),
            "emoji_copy_section" => Some(Self::Section),
            _ => None,
        }
    }

    fn payload(
        self,
        emoji: &crate::actions::EmojiActionInfo,
    ) -> Result<EmojiCopyPayload, &'static str> {
        match self {
            Self::Emoji => Ok(EmojiCopyPayload {
                clipboard_text: emoji.value.clone(),
                hud_text: format!("Copied {}", emoji.value),
            }),
            Self::Unicode => {
                use itertools::Itertools as _;
                let unicode_str = emoji
                    .value
                    .chars()
                    .map(|c| format!("U+{:04X}", c as u32))
                    .join(" ");

                Ok(EmojiCopyPayload {
                    clipboard_text: unicode_str.clone(),
                    hud_text: format!("Copied {unicode_str}"),
                })
            }
            Self::Section => {
                let Some(category) = emoji.category else {
                    return Err("No category for this emoji");
                };
                let category_emojis = crate::emoji::emojis_by_category(category);
                let all_emojis = {
                    use itertools::Itertools as _;
                    category_emojis.iter().map(|e| e.emoji).join("")
                };

                Ok(EmojiCopyPayload {
                    clipboard_text: all_emojis,
                    hud_text: format!("Copied {} emojis from {}", category_emojis.len(), category),
                })
            }
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Emoji | Self::Unicode | Self::Section => "No emoji selected",
        }
    }
}

impl ScriptListApp {
    /// Build an EmojiActionInfo for the currently selected emoji in the picker view.
    fn selected_emoji_action_info(&self) -> Option<crate::actions::EmojiActionInfo> {
        let (filter, selected_index, selected_category) = match &self.current_view {
            AppView::EmojiPickerView {
                filter,
                selected_index,
                selected_category,
            } => (filter.as_str(), *selected_index, *selected_category),
            _ => return None,
        };

        let (ordered_emojis, _pin_count) =
            crate::emoji::filtered_ordered_emojis_with_pins(filter, selected_category, &self.pinned_emojis);
        let emoji = ordered_emojis.get(selected_index)?;

        let frontmost_app_name =
            crate::frontmost_app_tracker::get_last_real_app().map(|app| app.name);

        Some(crate::actions::EmojiActionInfo {
            value: emoji.emoji.to_string(),
            name: emoji.name.to_string(),
            pinned: self.pinned_emojis.contains(emoji.emoji),
            frontmost_app_name,
            category: Some(emoji.category),
        })
    }

    /// Handle emoji-specific actions. Returns a DispatchOutcome.
    fn handle_emoji_action(
        &mut self,
        action_id: &str,
        selected_emoji: Option<crate::actions::EmojiActionInfo>,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        match action_id {
            "emoji_paste" | "emoji_paste_keep_open" => {
                let Some(paste_action) = EmojiPasteHandlerAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                let Some(emoji) = selected_emoji else {
                    self.show_error_toast(paste_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                if let Err(error) = crate::runtime_policy::check(
                    crate::runtime_policy::ExternalEffect::NativeInput,
                ) {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        error.to_string(),
                    );
                }

                tracing::info!(
                    action = paste_action.trace_action(),
                    emoji = %emoji.value,
                    emoji_name = %emoji.name,
                    "emoji action"
                );
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(emoji.value.clone()));
                self.finalize_paste_after_clipboard_ready(
                    "emoji",
                    &emoji.name,
                    paste_action.close_behavior(),
                    cx,
                )
            }
            "emoji_copy" | "emoji_copy_unicode" | "emoji_copy_section" => {
                let Some(copy_action) = EmojiCopyHandlerAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                let Some(emoji) = selected_emoji else {
                    self.show_error_toast(copy_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                let payload = match copy_action.payload(&emoji) {
                    Ok(payload) => payload,
                    Err(message) => {
                        self.show_error_toast(message, cx);
                        return DispatchOutcome::success();
                    }
                };

                tracing::info!(action = %action_id, emoji = %emoji.value, "emoji action");
                match crate::platform::copy_text(&payload.clipboard_text) {
                    Ok(receipt) => {
                        self.show_hud(receipt.feedback(payload.hud_text), Some(HUD_SHORT_MS), cx);
                        DispatchOutcome::success()
                    }
                    Err(error) => DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        format!("Failed to copy emoji: {error}"),
                    ),
                }
            }
            "emoji_pin" | "emoji_unpin" => {
                let Some(pin_action) = EmojiPinHandlerAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                let Some(emoji) = selected_emoji else {
                    self.show_error_toast(pin_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                tracing::info!(action = %action_id, emoji = %emoji.value, "emoji action");
                pin_action.apply(&mut self.pinned_emojis, &emoji.value);
                if let Err(error) = crate::emoji_pins::save_pinned_emojis(&self.pinned_emojis) {
                    tracing::error!(error = %error, emoji = %emoji.value, action = %action_id, "failed to update pinned emoji");
                    self.show_error_toast(pin_action.failure_message(error), cx);
                } else {
                    self.show_hud(pin_action.success_hud(&emoji.value), Some(HUD_SHORT_MS), cx);
                    cx.notify();
                }
                DispatchOutcome::success()
            }
            _ => DispatchOutcome::not_handled(),
        }
    }
}
