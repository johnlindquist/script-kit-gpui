// Clipboard action handlers for handle_action dispatch.
//
// Contains all `clipboard_*` action handling: pin/unpin, share, paste, copy,
// quick look, attach to AI, open with, CleanShot, OCR, delete, save file,
// and save snippet.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardPinHandlerAction {
    Pin,
    Unpin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardCopyPasteHandlerAction {
    PasteAndClose,
    CopyOnly,
    PasteKeepOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardShareHandlerAction {
    TextLike,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardAttachToAiHandlerAction {
    TextInput,
    FileAttachment,
    ImageInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardCleanShotHandlerAction {
    Annotate,
    Upload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardOcrHandlerAction {
    ExtractText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardExternalFileHandlerAction {
    QuickLook,
    OpenWith,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardSaveSnippetHandlerAction {
    SaveSnippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardSaveFileHandlerAction {
    SaveFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardDeleteEntryHandlerAction {
    DeleteEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBulkDeleteHandlerAction {
    MatchingEntries,
    AllUnpinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClipboardBulkDeleteResult {
    deleted: usize,
    failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBulkDeleteOutcome {
    Complete,
    PartialFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardUnpinnedDeleteAvailability {
    Empty,
    Available { unpinned_count: usize },
}

impl ClipboardPinHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "clipboard_pin" => Some(Self::Pin),
            "clipboard_unpin" => Some(Self::Unpin),
            _ => None,
        }
    }

    fn apply(self, entry_id: &str) -> anyhow::Result<()> {
        match self {
            Self::Pin => clipboard_history::pin_entry(entry_id),
            Self::Unpin => clipboard_history::unpin_entry(entry_id),
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::Pin => "Pinned",
            Self::Unpin => "Unpinned",
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Pin | Self::Unpin => "No clipboard entry selected",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::Pin | Self::Unpin => format!("Failed to update pin: {error}"),
        }
    }
}

impl ClipboardCopyPasteHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "clipboard_paste" => Some(Self::PasteAndClose),
            "clipboard_copy" => Some(Self::CopyOnly),
            "clipboard_paste_keep_open" => Some(Self::PasteKeepOpen),
            _ => None,
        }
    }

    fn action_id(self) -> &'static str {
        match self {
            Self::PasteAndClose => "clipboard_paste",
            Self::CopyOnly => "clipboard_copy",
            Self::PasteKeepOpen => "clipboard_paste_keep_open",
        }
    }

    fn paste_close_behavior(self) -> Option<PasteCloseBehavior> {
        match self {
            Self::PasteAndClose => Some(PasteCloseBehavior::HideWindow),
            Self::PasteKeepOpen => Some(PasteCloseBehavior::KeepWindowOpen),
            Self::CopyOnly => None,
        }
    }

    fn success_event(self) -> &'static str {
        match self {
            Self::PasteAndClose | Self::PasteKeepOpen => "clipboard_paste_start",
            Self::CopyOnly => "clipboard_copy_success",
        }
    }

    fn success_hud(self) -> Option<&'static str> {
        match self {
            Self::CopyOnly => Some("Copied to clipboard"),
            Self::PasteAndClose | Self::PasteKeepOpen => None,
        }
    }

    fn failure_prefix(self) -> &'static str {
        match self {
            Self::PasteAndClose | Self::PasteKeepOpen => "Failed to paste",
            Self::CopyOnly => "Failed to copy",
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::PasteAndClose | Self::CopyOnly | Self::PasteKeepOpen => {
                "No clipboard entry selected"
            }
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::PasteAndClose | Self::PasteKeepOpen | Self::CopyOnly => {
                format!("{}: {error}", self.failure_prefix())
            }
        }
    }
}

impl ClipboardShareHandlerAction {
    fn selection_required_message() -> &'static str {
        "No clipboard entry selected"
    }

    fn content_unavailable_message() -> &'static str {
        "Clipboard entry content unavailable"
    }

    fn from_content_type(content_type: clipboard_history::ContentType) -> Self {
        match content_type {
            clipboard_history::ContentType::Text
            | clipboard_history::ContentType::Link
            | clipboard_history::ContentType::File
            | clipboard_history::ContentType::Color => Self::TextLike,
            clipboard_history::ContentType::Image => Self::Image,
        }
    }

    fn share(self, content: String) -> Result<(), String> {
        match self {
            Self::TextLike => {
                crate::platform::show_share_sheet(crate::platform::ShareSheetItem::Text(content));
                Ok(())
            }
            Self::Image => {
                let Some(png_bytes) = clipboard_history::content_to_png_bytes(&content) else {
                    return Err(Self::image_decode_failure().to_string());
                };
                crate::platform::show_share_sheet(crate::platform::ShareSheetItem::ImagePng(
                    png_bytes,
                ));
                Ok(())
            }
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::TextLike | Self::Image => "Share sheet opened",
        }
    }

    fn image_decode_failure() -> &'static str {
        "Failed to decode clipboard image"
    }
}

impl ClipboardAttachToAiHandlerAction {
    fn selection_required_message() -> &'static str {
        "No clipboard entry selected"
    }

    fn content_unavailable_message() -> &'static str {
        "Clipboard entry content unavailable"
    }

    fn from_content_type(content_type: clipboard_history::ContentType) -> Self {
        match content_type {
            clipboard_history::ContentType::Text
            | clipboard_history::ContentType::Link
            | clipboard_history::ContentType::Color => Self::TextInput,
            clipboard_history::ContentType::File => Self::FileAttachment,
            clipboard_history::ContentType::Image => Self::ImageInput,
        }
    }

    fn deferred_action(self, content: String) -> Result<Option<DeferredAgentChatAction>, String> {
        match self {
            Self::TextInput => Ok(Some(DeferredAgentChatAction::SetInput {
                text: content,
                submit: false,
            })),
            Self::FileAttachment => {
                let attachment_path = shellexpand::tilde(content.trim()).into_owned();
                if attachment_path.is_empty() {
                    return Err(Self::empty_file_path_message().to_string());
                }
                Ok(Some(DeferredAgentChatAction::AddAttachment {
                    path: attachment_path,
                }))
            }
            Self::ImageInput => Ok(None),
        }
    }

    fn prepare_image_base64(content: &str) -> Result<String, String> {
        let png_bytes = clipboard_history::content_to_png_bytes(content)
            .ok_or_else(|| Self::image_decode_failure().to_string())?;
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(&png_bytes))
    }

    fn empty_file_path_message() -> &'static str {
        "Clipboard file path is empty"
    }

    fn image_decode_failure() -> &'static str {
        "Failed to decode clipboard image"
    }
}

impl ClipboardCleanShotHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "clipboard_annotate_cleanshot" => Some(Self::Annotate),
            "clipboard_upload_cleanshot" => Some(Self::Upload),
            _ => None,
        }
    }

    fn image_required_message(self) -> &'static str {
        match self {
            Self::Annotate | Self::Upload => "CleanShot actions are only available for images",
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::Annotate | Self::Upload => "No clipboard entry selected",
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::Annotate => "Opening CleanShot X…",
            Self::Upload => "Opening CleanShot X upload…",
        }
    }

    fn open_failure_message(self) -> &'static str {
        match self {
            Self::Annotate | Self::Upload => "Failed to open CleanShot X",
        }
    }

    fn copy_failure_message(self) -> &'static str {
        match self {
            Self::Annotate => "Failed to copy image",
            Self::Upload => "Failed to load image content",
        }
    }

    fn decode_failure_message(self) -> &'static str {
        match self {
            Self::Annotate | Self::Upload => "Failed to decode image",
        }
    }

    fn temp_save_failure_message(self) -> &'static str {
        match self {
            Self::Annotate | Self::Upload => "Failed to save image",
        }
    }
}

impl ClipboardOcrHandlerAction {
    fn selection_required_message(self) -> &'static str {
        match self {
            Self::ExtractText => "No clipboard entry selected",
        }
    }

    fn image_required_message(self) -> &'static str {
        match self {
            Self::ExtractText => "OCR is only available for images",
        }
    }

    fn copied_hud(self) -> &'static str {
        match self {
            Self::ExtractText => "Copied text from image",
        }
    }

    fn load_failure_message(self) -> &'static str {
        match self {
            Self::ExtractText => "Failed to load image content",
        }
    }

    fn decode_failure_message(self) -> &'static str {
        match self {
            Self::ExtractText => "Failed to decode image",
        }
    }

    fn empty_text_message(self) -> &'static str {
        match self {
            Self::ExtractText => "No text found in image",
        }
    }

    fn extract_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::ExtractText => format!("Failed to extract text: {error}"),
        }
    }
}

impl ScriptListApp {
    /// Refresh clipboard selection after a delete operation, respecting the active filter.
    ///
    /// After entries are removed from `cached_clipboard_entries`, this method
    /// re-applies the current filter, clamps `selected_index` to the visible
    /// set, scrolls to the new position, and updates `focused_clipboard_entry_id`
    /// to point at a visible entry.
    fn refresh_clipboard_selection_after_delete(&mut self) {
        if let AppView::ClipboardHistoryView {
            filter,
            selected_index,
        } = &mut self.current_view
        {
            let filtered_entries: Vec<_> = if filter.is_empty() {
                self.cached_clipboard_entries.iter().enumerate().collect()
            } else {
                let filter_lower = filter.to_lowercase();
                self.cached_clipboard_entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.text_preview.to_lowercase().contains(&filter_lower))
                    .collect()
            };

            if !filtered_entries.is_empty() {
                *selected_index =
                    (*selected_index).min(filtered_entries.len().saturating_sub(1));
                self.clipboard_list_scroll_handle
                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                self.focused_clipboard_entry_id = filtered_entries
                    .get(*selected_index)
                    .map(|(_, entry)| entry.id.clone());
            } else {
                *selected_index = 0;
                self.focused_clipboard_entry_id = None;
            }
        }
    }

    fn spawn_clipboard_paste_simulation(&self) {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Err(e) = selected_text::simulate_paste_with_cg() {
                tracing::error!(error = %e, "failed to simulate paste");
            } else {
                tracing::info!(
                    category = "UI",
                    event = "clipboard_paste_success",
                    "simulated Cmd+V paste"
                );
            }
        });
    }

    /// Handle clipboard-specific actions. Returns `true` if handled.
    ///
    /// Clipboard actions manage their own `cx.notify()` calls and early returns;
    /// the caller should **not** call `cx.notify()` when this returns a handled outcome.
    fn handle_clipboard_action(
        &mut self,
        action_id: &str,
        selected_clipboard_entry: Option<clipboard_history::ClipboardEntryMeta>,
        dctx: &DispatchContext,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let trace_id = &dctx.trace_id;
        match action_id {
            "clipboard_pin" | "clipboard_unpin" => {
                let Some(pin_action) = ClipboardPinHandlerAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(pin_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                let result = pin_action.apply(&entry.id);

                match result {
                    Ok(()) => {
                        // Refresh cached entries (pin/unpin updates cache ordering)
                        self.cached_clipboard_entries =
                            clipboard_history::get_cached_entries(CLIPBOARD_CACHE_SIZE);

                        // Keep selection on the same entry when possible
                        if let AppView::ClipboardHistoryView {
                            filter,
                            selected_index,
                        } = &mut self.current_view
                        {
                            let filtered_entries: Vec<_> = if filter.is_empty() {
                                self.cached_clipboard_entries.iter().enumerate().collect()
                            } else {
                                let filter_lower = filter.to_lowercase();
                                self.cached_clipboard_entries
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, e)| {
                                        e.text_preview.to_lowercase().contains(&filter_lower)
                                    })
                                    .collect()
                            };

                            if let Some(new_index) =
                                filtered_entries.iter().position(|(_, e)| e.id == entry.id)
                            {
                                *selected_index = new_index;
                            } else if !filtered_entries.is_empty() {
                                *selected_index =
                                    (*selected_index).min(filtered_entries.len().saturating_sub(1));
                            } else {
                                *selected_index = 0;
                            }

                            if !filtered_entries.is_empty() {
                                self.clipboard_list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            }
                            self.focused_clipboard_entry_id = filtered_entries
                                .get(*selected_index)
                                .map(|(_, entry)| entry.id.clone());
                        }

                        self.show_hud(
                            pin_action.success_hud().to_string(),
                            Some(HUD_SHORT_MS),
                            cx,
                        );
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to toggle clipboard pin");
                        self.show_error_toast(pin_action.failure_message(e), cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_keep_in_today" => {
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast("Select a clipboard entry to keep", cx);
                    return DispatchOutcome::success();
                };

                match clipboard_history::keep_entry_in_today(&entry.id, chrono::Utc::now()) {
                    Ok(true) => {
                        tracing::info!(entry_id = %entry.id, "clipboard entry kept to Today via action");
                        self.show_hud("Kept in Today".to_string(), Some(HUD_SHORT_MS), cx);
                    }
                    Ok(false) => {
                        self.show_hud("Already on Today".to_string(), Some(HUD_SHORT_MS), cx);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to keep clipboard entry in Today");
                        self.show_error_toast(format!("Couldn't keep in Today: {e}"), cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_share" => {
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(
                        ClipboardShareHandlerAction::selection_required_message(),
                        cx,
                    );
                    return DispatchOutcome::success();
                };

                let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                    self.show_error_toast(
                        ClipboardShareHandlerAction::content_unavailable_message(),
                        cx,
                    );
                    return DispatchOutcome::success();
                };

                tracing::info!(entry_id = %entry.id, content_type = ?entry.content_type, "opening share sheet");

                let share_action =
                    ClipboardShareHandlerAction::from_content_type(entry.content_type);
                let share_result = share_action.share(content);

                match share_result {
                    Ok(()) => self.show_hud(
                        share_action.success_hud().to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    ),
                    Err(message) => {
                        self.show_error_toast(message, cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_paste" | "clipboard_copy" | "clipboard_paste_keep_open" => {
                let Some(copy_paste_action) =
                    ClipboardCopyPasteHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(copy_paste_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                tracing::info!(
                    entry_id = %entry.id,
                    action = copy_paste_action.action_id(),
                    "clipboard copy/paste action"
                );
                match clipboard_history::copy_entry_to_clipboard(&entry.id) {
                    Ok(()) => {
                        tracing::info!(
                            category = "UI",
                            event = copy_paste_action.success_event(),
                            "entry copied to clipboard"
                        );
                        if let Some(close_behavior) = copy_paste_action.paste_close_behavior() {
                            return self.finalize_paste_after_clipboard_ready(
                                "clipboard",
                                &entry.id,
                                close_behavior,
                                cx,
                            );
                        }
                        if let Some(hud) = copy_paste_action.success_hud() {
                            self.show_hud(hud.to_string(), Some(HUD_SHORT_MS), cx);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, action = copy_paste_action.action_id(), "clipboard copy/paste failed");
                        self.show_error_toast(copy_paste_action.failure_message(e), cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_attach_to_ai" => {
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(
                        ClipboardAttachToAiHandlerAction::selection_required_message(),
                        cx,
                    );
                    return DispatchOutcome::success();
                };

                let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                    self.show_error_toast(
                        ClipboardAttachToAiHandlerAction::content_unavailable_message(),
                        cx,
                    );
                    return DispatchOutcome::success();
                };

                tracing::info!(
                    category = "AI",
                    entry_id = %entry.id,
                    content_type = ?entry.content_type,
                    "Attaching clipboard entry to Agent Chat"
                );

                let attach_action =
                    ClipboardAttachToAiHandlerAction::from_content_type(entry.content_type);
                match attach_action {
                    ClipboardAttachToAiHandlerAction::ImageInput => {
                        // Offload PNG decode + base64 encode to a background thread
                        // to avoid blocking the UI during large image processing.
                        let (result_tx, result_rx) =
                            async_channel::bounded::<Result<String, String>>(1);
                        let trace_id = dctx.trace_id.clone();
                        let action_id_owned = action_id.to_string();

                        std::thread::spawn(move || {
                            let started_at = std::time::Instant::now();
                            let result =
                                ClipboardAttachToAiHandlerAction::prepare_image_base64(&content);
                            tracing::info!(
                                category = "AI",
                                event = "clipboard_image_prep_done",
                                trace_id = %trace_id,
                                duration_ms = started_at.elapsed().as_millis() as u64,
                                success = result.is_ok(),
                                "Clipboard image preparation finished"
                            );
                            let _ = result_tx.send_blocking(result);
                        });

                        let trace_id = dctx.trace_id.clone();
                        let action_id_owned2 = action_id.to_string();
                        cx.spawn(async move |this, cx| {
                            let Ok(result) = result_rx.recv().await else {
                                return;
                            };
                            let _ = this.update(cx, |this, cx| match result {
                                Ok(image_base64) => {
                                    this.open_agent_chat_after_main_hide(
                                        &action_id_owned2,
                                        &trace_id,
                                        DeferredAgentChatAction::SetInputWithImage {
                                            text: String::new(),
                                            image_base64,
                                            submit: false,
                                        },
                                        cx,
                                    );
                                }
                                Err(message) => {
                                    tracing::error!(
                                        category = "AI",
                                        action = %action_id_owned,
                                        error = %message,
                                        "Clipboard image decode failed"
                                    );
                                    this.show_error_toast(message, cx);
                                }
                            });
                        })
                        .detach();

                        return DispatchOutcome::success();
                    }
                    ClipboardAttachToAiHandlerAction::TextInput
                    | ClipboardAttachToAiHandlerAction::FileAttachment => {}
                }

                let deferred_action = match attach_action.deferred_action(content) {
                    Ok(Some(deferred_action)) => deferred_action,
                    Ok(None) => return DispatchOutcome::success(),
                    Err(message) => {
                        self.show_error_toast(message, cx);
                        return DispatchOutcome::success();
                    }
                };

                self.open_agent_chat_after_main_hide(action_id, &dctx.trace_id, deferred_action, cx);
                DispatchOutcome::success()
            }
            "clipboard_quick_look" => {
                let Some(external_action) =
                    ClipboardExternalFileHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(external_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                if let Err(e) = clipboard_history::quick_look_entry(&entry) {
                    tracing::error!(error = %e, "failed to Quick Look");
                    self.show_error_toast(external_action.quick_look_failure_message(e), cx);
                }
                DispatchOutcome::success()
            }
            "clipboard_open_with" => {
                let Some(external_action) =
                    ClipboardExternalFileHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(external_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                    self.show_error_toast(external_action.load_failure_message(), cx);
                    return DispatchOutcome::success();
                };

                let full_entry = clipboard_history::ClipboardEntry {
                    id: entry.id.clone(),
                    content,
                    content_type: entry.content_type,
                    timestamp: entry.timestamp,
                    pinned: entry.pinned,
                    ocr_text: entry.ocr_text.clone(),
                    source_app_name: None,
                    source_app_bundle_id: None,
                };

                let temp_path = match clipboard_history::save_entry_to_temp_file(&full_entry) {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to save temp file");
                        self.show_error_toast(external_action.temp_save_failure_message(), cx);
                        return DispatchOutcome::success();
                    }
                };

                #[cfg(target_os = "macos")]
                {
                    let path_str = temp_path.to_string_lossy().to_string();
                    if let Err(e) = crate::file_search::open_with(&path_str) {
                        tracing::error!(error = %e, "failed to Open With");
                        self.show_error_toast(external_action.open_with_failure_message(), cx);
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    let _ = temp_path;
                    self.show_unsupported_platform_toast(external_action.platform_name(), cx);
                }
                DispatchOutcome::success()
            }
            "clipboard_annotate_cleanshot" | "clipboard_upload_cleanshot" => {
                let Some(cleanshot_action) =
                    ClipboardCleanShotHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(cleanshot_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                if entry.content_type != clipboard_history::ContentType::Image {
                    self.show_error_toast(cleanshot_action.image_required_message(), cx);
                    return DispatchOutcome::success();
                }

                #[cfg(target_os = "macos")]
                {
                    let url = match cleanshot_action {
                        ClipboardCleanShotHandlerAction::Annotate => {
                            if let Err(e) = clipboard_history::copy_entry_to_clipboard(&entry.id) {
                                tracing::error!(error = %e, "failed to copy image");
                                self.show_error_toast(
                                    cleanshot_action.copy_failure_message(),
                                    cx,
                                );
                                return DispatchOutcome::success();
                            }
                            "cleanshot://open-from-clipboard".to_string()
                        }
                        ClipboardCleanShotHandlerAction::Upload => {
                            let Some(content) = clipboard_history::get_entry_content(&entry.id)
                            else {
                                self.show_error_toast(
                                    cleanshot_action.copy_failure_message(),
                                    cx,
                                );
                                return DispatchOutcome::success();
                            };

                            let Some(png_bytes) =
                                clipboard_history::content_to_png_bytes(&content)
                            else {
                                self.show_error_toast(
                                    cleanshot_action.decode_failure_message(),
                                    cx,
                                );
                                return DispatchOutcome::success();
                            };

                            let temp_path = std::env::temp_dir()
                                .join(format!("script-kit-clipboard-{}.png", uuid::Uuid::new_v4()));

                            if let Err(e) = std::fs::write(&temp_path, png_bytes) {
                                tracing::error!(error = %e, "failed to write temp image");
                                self.show_error_toast(
                                    cleanshot_action.temp_save_failure_message(),
                                    cx,
                                );
                                return DispatchOutcome::success();
                            }

                            let path_str = temp_path.to_string_lossy();
                            let encoded_path = self.percent_encode_for_url(&path_str);
                            format!("cleanshot://open-annotate?filepath={}&action=upload", encoded_path)
                        }
                    };

                    match std::process::Command::new("open").arg(url).spawn() {
                        Ok(_) => {
                            self.show_hud(
                                cleanshot_action.success_hud().to_string(),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                            self.hide_main_and_reset(cx);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "failed to open CleanShot X");
                            self.show_error_toast(cleanshot_action.open_failure_message(), cx);
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    self.show_unsupported_platform_toast("CleanShot", cx);
                }
                DispatchOutcome::success()
            }
            "clipboard_ocr" => {
                let ocr_action = ClipboardOcrHandlerAction::ExtractText;
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(ocr_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                if entry.content_type != clipboard_history::ContentType::Image {
                    self.show_error_toast(ocr_action.image_required_message(), cx);
                    return DispatchOutcome::success();
                }

                // Check if we already have cached OCR text
                if let Some(ref cached_text) = entry.ocr_text {
                    if !cached_text.trim().is_empty() {
                        tracing::debug!(category = "UI", event = "clipboard_ocr_cached", "using cached OCR text");
                        self.copy_to_clipboard_with_feedback(
                            cached_text,
                            ocr_action.copied_hud().to_string(),
                            true,
                            cx,
                        );
                        return DispatchOutcome::success();
                    }
                }

                #[cfg(all(target_os = "macos", feature = "ocr"))]
                {
                    // Get image content
                    let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                        self.show_error_toast(ocr_action.load_failure_message(), cx);
                        return DispatchOutcome::success();
                    };

                    // Decode to RGBA bytes for OCR
                    let Some((width, height, rgba_bytes)) =
                        clipboard_history::decode_to_rgba_bytes(&content)
                    else {
                        self.show_error_toast(ocr_action.decode_failure_message(), cx);
                        return DispatchOutcome::success();
                    };

                    tracing::debug!(width, height, "starting OCR on image");

                    let entry_id = entry.id.clone();
                    match script_kit_gpui::ocr::extract_text_from_rgba(
                        width,
                        height,
                        &rgba_bytes,
                    ) {
                        Ok(text) => {
                            if text.trim().is_empty() {
                                tracing::debug!(category = "UI", event = "clipboard_ocr_empty", "no text found in image");
                                self.show_error_toast(ocr_action.empty_text_message(), cx);
                            } else {
                                tracing::debug!(chars = text.len(), "extracted OCR text");

                                // Cache the OCR result
                                let _ = clipboard_history::update_ocr_text(&entry_id, &text);

                                // Copy to clipboard
                                self.copy_to_clipboard_with_feedback(
                                    &text,
                                    ocr_action.copied_hud().to_string(),
                                    true,
                                    cx,
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "OCR failed");
                            self.show_error_toast(ocr_action.extract_failure_message(e), cx);
                        }
                    }
                }

                #[cfg(not(all(target_os = "macos", feature = "ocr")))]
                {
                    self.show_unsupported_platform_toast("OCR", cx);
                }
                DispatchOutcome::success()
            }
            // Clipboard delete actions
            "clipboard_delete_multiple" => {
                let Some(bulk_delete_action) =
                    ClipboardBulkDeleteHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let filter_text = match &self.current_view {
                    AppView::ClipboardHistoryView { filter, .. } => filter.trim().to_string(),
                    _ => String::new(),
                };

                if filter_text.is_empty() {
                    self.show_error_toast(bulk_delete_action.search_required_message(), cx);
                    return DispatchOutcome::success();
                }

                let filter_lower = filter_text.to_lowercase();
                let ids_to_delete: Vec<String> = self
                    .cached_clipboard_entries
                    .iter()
                    .filter(|entry| entry.text_preview.to_lowercase().contains(&filter_lower))
                    .map(|entry| entry.id.clone())
                    .collect();

                if ids_to_delete.is_empty() {
                    self.show_error_toast(bulk_delete_action.no_matches_message(), cx);
                    return DispatchOutcome::success();
                }

                let delete_count = ids_to_delete.len();
                let confirm_options = crate::confirm::ParentConfirmOptions::destructive(
                    bulk_delete_action.confirm_title(),
                    bulk_delete_action.confirm_message(delete_count),
                    bulk_delete_action.confirm_button(),
                );
                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();

                cx.spawn(async move |this, cx| {
                    match crate::confirm::confirm_with_parent_dialog(cx, confirm_options, &trace_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::info!(
                                trace_id = %trace_id,
                                status = "cancelled",
                                duration_ms = start.elapsed().as_millis() as u64,
                                "Async action cancelled: clipboard_delete_multiple"
                            );
                            return;
                        }
                        Err(e) => {
                            let _ = this.update(cx, |this, cx| {
                                tracing::error!(
                                    trace_id = %trace_id,
                                    status = "failed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    error = %e,
                                    "failed to open confirmation modal"
                                );
                                this.show_error_toast_with_code(
                                    bulk_delete_action.confirmation_failure_message(),
                                    Some(crate::action_helpers::ERROR_MODAL_FAILED),
                                    cx,
                                );
                            });
                            return;
                        }
                    }

                    let _ = this.update(cx, move |this, cx| {
                        let mut deleted = 0usize;
                        let mut failed = 0usize;
                        for id in ids_to_delete {
                            match clipboard_history::remove_entry(&id) {
                                Ok(()) => deleted += 1,
                                Err(e) => {
                                    failed += 1;
                                    tracing::error!(entry_id = %id, error = %e, "failed to delete clipboard entry");
                                }
                            }
                        }

                        this.cached_clipboard_entries =
                            clipboard_history::get_cached_entries(CLIPBOARD_CACHE_SIZE);
                        this.refresh_clipboard_selection_after_delete();
                        cx.notify();

                        ClipboardBulkDeleteResult { deleted, failed }
                            .show_feedback(bulk_delete_action, this, cx);
                    });
                })
                .detach();
                DispatchOutcome::success()
            }
            "clipboard_delete" => {
                let delete_action = ClipboardDeleteEntryHandlerAction::DeleteEntry;
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(delete_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                match clipboard_history::remove_entry(&entry.id) {
                    Ok(()) => {
                        tracing::info!(entry_id = %entry.id, "deleted clipboard entry");
                        // Refresh cached entries
                        self.cached_clipboard_entries =
                            clipboard_history::get_cached_entries(CLIPBOARD_CACHE_SIZE);

                        // Update selection respecting active filter
                        self.refresh_clipboard_selection_after_delete();

                        self.show_hud(
                            delete_action.success_hud().to_string(),
                            Some(HUD_SHORT_MS),
                            cx,
                        );
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to delete clipboard entry");
                        self.show_error_toast(delete_action.failure_message(e), cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_delete_all" => {
                let Some(bulk_delete_action) =
                    ClipboardBulkDeleteHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                // Delete all unpinned entries
                let unpinned_count = self
                    .cached_clipboard_entries
                    .iter()
                    .filter(|e| !e.pinned)
                    .count();

                let Some(unpinned_count) = bulk_delete_action
                    .unpinned_availability(unpinned_count)
                    .count()
                else {
                    self.show_error_toast(bulk_delete_action.no_unpinned_message(), cx);
                    return DispatchOutcome::success();
                };

                let confirm_options = crate::confirm::ParentConfirmOptions::destructive(
                    bulk_delete_action.confirm_title(),
                    bulk_delete_action.confirm_message(unpinned_count),
                    bulk_delete_action.confirm_button(),
                );
                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();

                cx.spawn(async move |this, cx| {
                    match crate::confirm::confirm_with_parent_dialog(cx, confirm_options, &trace_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::info!(
                                trace_id = %trace_id,
                                status = "cancelled",
                                duration_ms = start.elapsed().as_millis() as u64,
                                "Async action cancelled: clipboard_delete_all"
                            );
                            return;
                        }
                        Err(e) => {
                            let _ = this.update(cx, |this, cx| {
                                tracing::error!(
                                    trace_id = %trace_id,
                                    status = "failed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    error = %e,
                                    "failed to open confirmation modal"
                                );
                                this.show_error_toast_with_code(
                                    bulk_delete_action.confirmation_failure_message(),
                                    Some(crate::action_helpers::ERROR_MODAL_FAILED),
                                    cx,
                                );
                            });
                            return;
                        }
                    }

                    let _ = this.update(cx, move |this, cx| {
                        match clipboard_history::clear_unpinned_history() {
                            Ok(()) => {
                                tracing::info!(count = unpinned_count, "deleted unpinned clipboard entries");
                                this.cached_clipboard_entries =
                                    clipboard_history::get_cached_entries(CLIPBOARD_CACHE_SIZE);

                                // Reset selection respecting active filter
                                this.refresh_clipboard_selection_after_delete();

                                this.show_hud(
                                    bulk_delete_action.success_hud(unpinned_count),
                                    Some(HUD_2500_MS),
                                    cx,
                                );
                                cx.notify();
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "failed to clear unpinned history");
                                this.show_error_toast(bulk_delete_action.failure_message(e), cx);
                            }
                        }
                    });
                })
                .detach();
                DispatchOutcome::success()
            }

            "clipboard_save_file" => {
                let save_file_action = ClipboardSaveFileHandlerAction::SaveFile;
                let Some(entry) = selected_clipboard_entry.clone() else {
                    self.show_error_toast(save_file_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                    self.show_error_toast(save_file_action.content_unavailable_message(), cx);
                    return DispatchOutcome::success();
                };

                // Determine filename and content based on type
                let (file_content, extension) = match entry.content_type {
                    clipboard_history::ContentType::Text
                    | clipboard_history::ContentType::Link
                    | clipboard_history::ContentType::File
                    | clipboard_history::ContentType::Color => (content.into_bytes(), "txt"),
                    clipboard_history::ContentType::Image => {
                        let Some(png_bytes) =
                            clipboard_history::content_to_png_bytes(&content)
                        else {
                            self.show_error_toast(save_file_action.decode_failure_message(), cx);
                            return DispatchOutcome::success();
                        };
                        (png_bytes, "png")
                    }
                };

                // Get save location (Desktop or home)
                let home =
                    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"));
                let desktop = home.join("Desktop");
                let save_dir = if desktop.exists() { desktop } else { home };

                // Generate unique filename with timestamp
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let filename = format!("clipboard-{}.{}", timestamp, extension);
                let save_path = save_dir.join(&filename);

                match std::fs::write(&save_path, &file_content) {
                    Ok(()) => {
                        tracing::info!(path = ?save_path, "saved clipboard to file");
                        let reveal_result_rx =
                            self.reveal_in_finder_with_feedback_async(&save_path, trace_id);
                        let trace_id = trace_id.to_string();
                        let start = std::time::Instant::now();
                        cx.spawn(async move |this, cx| {
                            let Ok(reveal_result) = reveal_result_rx.recv().await else {
                                return;
                            };

                            let _ = this.update(cx, |this, cx| match reveal_result {
                                Ok(()) => {
                                    tracing::info!(
                                        trace_id = %trace_id,
                                        status = "completed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        "Async action completed: clipboard_save_file"
                                    );
                                    this.show_hud(
                                        save_file_action.saved_hud(&save_path),
                                        Some(HUD_LONG_MS),
                                        cx,
                                    );
                                    this.hide_main_and_reset(cx);
                                }
                                Err(message) => {
                                    // File was saved but reveal failed — log the reveal error,
                                    // show only HUD for the successful save (no dual feedback).
                                    tracing::warn!(
                                        trace_id = %trace_id,
                                        status = "completed",
                                        duration_ms = start.elapsed().as_millis() as u64,
                                        error = %message,
                                        "Async action completed with warning: clipboard_save_file reveal failed"
                                    );
                                    this.show_hud(
                                        save_file_action.saved_hud(&save_path),
                                        Some(HUD_LONG_MS),
                                        cx,
                                    );
                                    this.hide_main_and_reset(cx);
                                }
                            });
                        })
                        .detach();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to save file");
                        self.show_error_toast(save_file_action.save_failure_message(e), cx);
                    }
                }
                DispatchOutcome::success()
            }
            "clipboard_save_snippet" => {
                let save_snippet_action = ClipboardSaveSnippetHandlerAction::SaveSnippet;
                let Some(entry) = selected_clipboard_entry else {
                    self.show_error_toast(save_snippet_action.selection_required_message(), cx);
                    return DispatchOutcome::success();
                };

                if let Some(message) = save_snippet_action.content_type_error(entry.content_type) {
                    self.show_error_toast(message, cx);
                    return DispatchOutcome::success();
                }

                let Some(content) = clipboard_history::get_entry_content(&entry.id) else {
                    self.show_error_toast(save_snippet_action.content_unavailable_message(), cx);
                    return DispatchOutcome::success();
                };

                // Generate a default keyword from the first few words
                let default_keyword: String = content
                    .chars()
                    .take(20)
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect::<String>()
                    .to_lowercase();
                let default_keyword = if default_keyword.is_empty() {
                    save_snippet_action.default_keyword().to_string()
                } else {
                    default_keyword
                };

                // Save snippets into the default main plugin's scriptlets directory.
                let scriptlets_dir = crate::script_creation::scriptlets_dir();
                let snippets_file = scriptlets_dir.join("clipboard-snippets.md");

                if !scriptlets_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(&scriptlets_dir) {
                        tracing::error!(error = %e, "failed to create scriptlets dir");
                        self.show_error_toast(
                            save_snippet_action.create_failure_message(e),
                            cx,
                        );
                        return DispatchOutcome::success();
                    }
                }

                // Generate unique keyword with timestamp suffix
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() % 10000)
                    .unwrap_or(0);
                let keyword = format!("{}-{}", default_keyword, timestamp);

                // Create snippet entry with proper fence handling
                let fence = if content.contains("```") {
                    "~~~~"
                } else {
                    "```"
                };
                let snippet_entry = format!(
                    "\n## {}\n\n{}\nname: {}\ntool: paste\nkeyword: {}\n{}\n\n{}paste\n{}\n{}\n",
                    keyword, fence, keyword, keyword, fence, fence, content, fence
                );

                // Append to snippets file
                let result = if snippets_file.exists() {
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&snippets_file)
                        .and_then(|mut f| {
                            use std::io::Write;
                            f.write_all(snippet_entry.as_bytes())
                        })
                } else {
                    let header = "# Clipboard Snippets\n\nSnippets created from clipboard history.\n";
                    std::fs::write(
                        &snippets_file,
                        format!("{}{}", header, snippet_entry),
                    )
                };

                match result {
                    Ok(()) => {
                        tracing::info!(keyword = %keyword, "created snippet");
                        self.show_hud(
                            save_snippet_action.success_hud(&keyword),
                            Some(HUD_LONG_MS),
                            cx,
                        );
                        // Refresh scripts to pick up new snippet
                        self.refresh_scripts(cx);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to save snippet");
                        self.show_error_toast(save_snippet_action.save_failure_message(e), cx);
                    }
                }
                DispatchOutcome::success()
            }

            _ => DispatchOutcome::not_handled(),
        }
    }
}

impl ClipboardExternalFileHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "clipboard_quick_look" => Some(Self::QuickLook),
            "clipboard_open_with" => Some(Self::OpenWith),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::QuickLook | Self::OpenWith => "No clipboard entry selected",
        }
    }

    fn quick_look_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::QuickLook => format!("Failed to Quick Look: {error}"),
            Self::OpenWith => format!("Failed to Open With: {error}"),
        }
    }

    fn load_failure_message(self) -> &'static str {
        match self {
            Self::QuickLook | Self::OpenWith => "Failed to load clipboard content",
        }
    }

    fn temp_save_failure_message(self) -> &'static str {
        match self {
            Self::QuickLook | Self::OpenWith => "Failed to save temp file",
        }
    }

    fn open_with_failure_message(self) -> &'static str {
        match self {
            Self::QuickLook | Self::OpenWith => "Failed to Open With",
        }
    }

    fn platform_name(self) -> &'static str {
        match self {
            Self::QuickLook => "Quick Look",
            Self::OpenWith => "Open With",
        }
    }
}

impl ClipboardSaveSnippetHandlerAction {
    fn selection_required_message(self) -> &'static str {
        match self {
            Self::SaveSnippet => "No clipboard entry selected",
        }
    }

    fn text_required_message(self) -> &'static str {
        match self {
            Self::SaveSnippet => "Only text can be saved as snippet",
        }
    }

    fn content_type_error(
        self,
        content_type: clipboard_history::ContentType,
    ) -> Option<&'static str> {
        match content_type {
            clipboard_history::ContentType::Text => None,
            clipboard_history::ContentType::Image
            | clipboard_history::ContentType::Link
            | clipboard_history::ContentType::File
            | clipboard_history::ContentType::Color => Some(self.text_required_message()),
        }
    }

    fn content_unavailable_message(self) -> &'static str {
        match self {
            Self::SaveSnippet => "Clipboard content unavailable",
        }
    }

    fn default_keyword(self) -> &'static str {
        match self {
            Self::SaveSnippet => "snippet",
        }
    }

    fn create_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SaveSnippet => format!("Failed to create snippets: {error}"),
        }
    }

    fn success_hud(self, keyword: &str) -> String {
        match self {
            Self::SaveSnippet => format!("Snippet created: type '{keyword}' to paste"),
        }
    }

    fn save_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SaveSnippet => format!("Failed to save: {error}"),
        }
    }
}

impl ClipboardSaveFileHandlerAction {
    fn selection_required_message(self) -> &'static str {
        match self {
            Self::SaveFile => "No clipboard entry selected",
        }
    }

    fn content_unavailable_message(self) -> &'static str {
        match self {
            Self::SaveFile => "Clipboard content unavailable",
        }
    }

    fn decode_failure_message(self) -> &'static str {
        match self {
            Self::SaveFile => "Failed to decode image",
        }
    }

    fn saved_hud(self, save_path: &std::path::Path) -> String {
        match self {
            Self::SaveFile => {
                let save_path = save_path.display();
                format!("Saved to: {save_path}")
            }
        }
    }

    fn save_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SaveFile => format!("Failed to save: {error}"),
        }
    }
}

impl ClipboardDeleteEntryHandlerAction {
    fn selection_required_message(self) -> &'static str {
        match self {
            Self::DeleteEntry => "No clipboard entry selected",
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::DeleteEntry => "Entry deleted",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::DeleteEntry => format!("Failed to delete: {error}"),
        }
    }
}

impl ClipboardBulkDeleteHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "clipboard_delete_multiple" => Some(Self::MatchingEntries),
            "clipboard_delete_all" => Some(Self::AllUnpinned),
            _ => None,
        }
    }

    fn search_required_message(self) -> &'static str {
        match self {
            Self::MatchingEntries => "Type in search first, then use Delete Entries...",
            Self::AllUnpinned => "Type in search first, then use Delete Entries...",
        }
    }

    fn no_matches_message(self) -> &'static str {
        match self {
            Self::MatchingEntries => "No matching entries to delete",
            Self::AllUnpinned => "No matching entries to delete",
        }
    }

    fn no_unpinned_message(self) -> &'static str {
        match self {
            Self::MatchingEntries | Self::AllUnpinned => "No unpinned entries to delete",
        }
    }

    fn confirm_title(self) -> &'static str {
        match self {
            Self::MatchingEntries => "Delete Clipboard Entries",
            Self::AllUnpinned => "Delete All Clipboard Entries",
        }
    }

    fn confirm_message(self, count: usize) -> String {
        match self {
            Self::MatchingEntries => {
                format!("Are you sure you want to delete these {count} matching clipboard entries?")
            }
            Self::AllUnpinned => {
                format!("Are you sure you want to delete all {count} unpinned clipboard entries?")
            }
        }
    }

    fn confirm_button(self) -> &'static str {
        match self {
            Self::MatchingEntries => "Delete",
            Self::AllUnpinned => "Delete All",
        }
    }

    fn confirmation_failure_message(self) -> &'static str {
        match self {
            Self::MatchingEntries | Self::AllUnpinned => "Failed to open confirmation dialog",
        }
    }

    fn success_hud(self, deleted: usize) -> String {
        match self {
            Self::MatchingEntries => format!("Deleted {deleted} entries"),
            Self::AllUnpinned => format!("Deleted {deleted} entries (pinned preserved)"),
        }
    }

    fn partial_failure_message(self, deleted: usize, failed: usize) -> String {
        match self {
            Self::MatchingEntries | Self::AllUnpinned => {
                format!("Deleted {deleted}, failed {failed}")
            }
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::MatchingEntries | Self::AllUnpinned => {
                format!("Failed to delete: {error}")
            }
        }
    }

    fn unpinned_availability(self, unpinned_count: usize) -> ClipboardUnpinnedDeleteAvailability {
        let _ = self;
        ClipboardUnpinnedDeleteAvailability::from_count(unpinned_count)
    }
}

impl ClipboardBulkDeleteResult {
    fn outcome(self) -> ClipboardBulkDeleteOutcome {
        match self.failed {
            0 => ClipboardBulkDeleteOutcome::Complete,
            _ => ClipboardBulkDeleteOutcome::PartialFailure,
        }
    }

    fn show_feedback(
        self,
        action: ClipboardBulkDeleteHandlerAction,
        app: &mut ScriptListApp,
        cx: &mut Context<ScriptListApp>,
    ) {
        match self.outcome() {
            ClipboardBulkDeleteOutcome::Complete => {
                app.show_hud(action.success_hud(self.deleted), Some(HUD_2500_MS), cx);
            }
            ClipboardBulkDeleteOutcome::PartialFailure => {
                app.show_error_toast(
                    action.partial_failure_message(self.deleted, self.failed),
                    cx,
                );
            }
        }
    }
}

impl ClipboardUnpinnedDeleteAvailability {
    fn from_count(unpinned_count: usize) -> Self {
        match unpinned_count {
            0 => Self::Empty,
            unpinned_count => Self::Available { unpinned_count },
        }
    }

    fn count(self) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::Available { unpinned_count } => Some(unpinned_count),
        }
    }
}

#[cfg(test)]
mod clipboard_save_snippet_action_tests {
    use super::*;

    #[test]
    fn content_type_gate_accepts_only_text() {
        let action = ClipboardSaveSnippetHandlerAction::SaveSnippet;

        assert_eq!(
            action.content_type_error(clipboard_history::ContentType::Text),
            None
        );
        for content_type in [
            clipboard_history::ContentType::Image,
            clipboard_history::ContentType::Link,
            clipboard_history::ContentType::File,
            clipboard_history::ContentType::Color,
        ] {
            assert_eq!(
                action.content_type_error(content_type),
                Some("Only text can be saved as snippet"),
                "{content_type:?} must be rejected"
            );
        }
    }
}
