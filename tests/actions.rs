// Integration-level behavior tests for action handler feedback contracts.
// These verify the structural invariants of the action system across files.

use std::fs;

/// Verify the feedback contract constants exist in helpers.rs:
/// success feedback uses HUD, errors use Toast.
#[test]
fn feedback_contract_constants_defined() {
    let content =
        fs::read_to_string("src/app_actions/helpers.rs").expect("Failed to read helpers.rs");

    // HUD constants for success paths
    assert!(
        content.contains("HUD_SHORT_MS"),
        "HUD_SHORT_MS must be defined for success feedback"
    );
    assert!(
        content.contains("HUD_MEDIUM_MS"),
        "HUD_MEDIUM_MS must be defined for success feedback"
    );

    // Toast constants for error paths
    assert!(
        content.contains("TOAST_ERROR_MS"),
        "TOAST_ERROR_MS must be defined for error feedback"
    );
    assert!(
        content.contains("TOAST_CRITICAL_MS"),
        "TOAST_CRITICAL_MS must be defined for critical errors"
    );
}

/// Verify that the action handler files have the expected dispatch structure.
#[test]
fn action_handler_dispatch_structure_exists() {
    // Collect all handler source from modular handler directory
    let handler_dir = std::path::Path::new("src/app_actions/handle_action");
    let mut content = String::new();
    for entry in fs::read_dir(handler_dir).expect("Failed to read handle_action directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let chunk = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read {}", path.display()));
            content.push_str(&chunk);
            content.push('\n');
        }
    }

    // Must handle copy actions
    assert!(
        content.contains("\"copy_path\""),
        "Action handler must dispatch copy_path"
    );
    assert!(
        content.contains("\"copy_deeplink\""),
        "Action handler must dispatch copy_deeplink"
    );
    assert!(
        content.contains("\"copy_content\""),
        "Action handler must dispatch copy_content"
    );

    // Must handle file search actions
    assert!(
        content.contains("\"open_file\""),
        "Action handler must dispatch open_file"
    );
    assert!(
        content.contains("\"quick_look\""),
        "Action handler must dispatch quick_look"
    );

    // Must have toast_manager usage for errors (may be in chunk files)
    assert!(
        content.contains("toast_manager.push("),
        "Action handler must use toast_manager for error feedback"
    );
}

#[test]
fn launcher_handoff_builtins_use_submit_reset_not_focus_loss_preserve() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin_execution.rs");

    let notes_start = content
        .find("fn execute_notes_builtin")
        .expect("execute_notes_builtin must exist");
    let notes_tail = &content[notes_start..];
    let notes_end = notes_tail
        .find("fn execute_sync_to_github_builtin")
        .expect("execute_notes_builtin boundary must exist");
    let notes_fn = &notes_tail[..notes_end];

    assert!(
        notes_fn.contains("event = \"notes_handoff_submit_reset\""),
        "Open Notes handoff should be treated as a launcher submit, not focus-loss preservation"
    );
    assert!(
        notes_fn.contains("self.close_and_reset_window(cx);"),
        "Open Notes must reset the launcher so the next main-menu open starts with an empty input"
    );
    assert!(
        !notes_fn.contains("defer_hide_main_window(cx)"),
        "Open Notes should not manually hide while preserving the launcher filter"
    );

    let notes_command_start = content
        .find("fn execute_notes_command_builtin")
        .expect("execute_notes_command_builtin must exist");
    let notes_command_tail = &content[notes_command_start..];
    let notes_command_end = notes_command_tail
        .find("fn execute_utility_open_builtin")
        .expect("execute_notes_command_builtin boundary must exist");
    let notes_command_fn = &notes_command_tail[..notes_command_end];

    assert!(
        notes_command_fn.contains("self.close_and_reset_window(cx);"),
        "Notes commands that hand off to the Notes window must use the submit reset path"
    );
    assert!(
        !notes_command_fn.contains("defer_hide_main_window(cx)"),
        "Notes commands should not preserve stale launcher input via manual hide"
    );

    let dictation_start = content
        .find("fn execute_dictation_builtin_action")
        .expect("execute_dictation_builtin_action must exist");
    let dictation_tail = &content[dictation_start..];
    let dictation_end = dictation_tail
        .find("fn prepare_dictation_builtin_start")
        .expect("execute_dictation_builtin_action boundary must exist");
    let dictation_fn = &dictation_tail[..dictation_end];

    assert!(
        dictation_fn.contains("DictationTarget::ExternalApp")
            && dictation_fn.contains("self.close_and_reset_window(cx);")
            && dictation_fn.find("self.close_and_reset_window(cx);")
                < dictation_fn.find("self.handle_dictation_started(action, dictation_target, cx);"),
        "external-app dictation starts from the launcher must reset/hide after capture starts but before opening the overlay"
    );
}

#[test]
fn chat_transcript_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/chat.rs")
        .expect("Failed to read chat action builder");

    assert!(
        content.contains("enum ChatTranscriptActionPlan")
            && content.contains("NoTranscriptActions")
            && content.contains("CopyLastResponse")
            && content.contains("ClearConversation")
            && content.contains("CopyLastResponseAndClearConversation"),
        "chat transcript copy/clear actions should be driven by named transcript action plan states"
    );
    assert!(
        content.contains("fn append_transcript_actions")
            && content.contains("ChatTranscriptActionPlan::from_info(info)"),
        "chat context actions should append transcript actions from the named plan"
    );
}

#[test]
fn chat_model_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/chat.rs")
        .expect("Failed to read chat action builder");

    assert!(
        content.contains("enum ChatModelPickerRowPlan")
            && content.contains("CurrentModel")
            && content.contains("AvailableModel"),
        "chat model picker checkmark copy should be driven by named row plan states"
    );
    assert!(
        content.contains("enum ChatChangeModelActionPlan")
            && content.contains("CurrentModelSelected")
            && content.contains("NoCurrentModelSelected"),
        "chat Change Model description should be driven by named action plan states"
    );
    assert!(
        content.contains("ChatModelPickerRowPlan::from_model(info, model)")
            && content.contains("row_plan.description(model)")
            && content.contains("ChatChangeModelActionPlan::from_info(info).description(info)"),
        "chat model rows and root Change Model copy should derive from named plans"
    );
}

#[test]
fn clipboard_entry_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/clipboard.rs")
        .expect("Failed to read clipboard action builder");

    assert!(
        content.contains("enum ClipboardEntryActionPlan")
            && content.contains("TextPinned")
            && content.contains("TextUnpinned")
            && content.contains("ImagePinned")
            && content.contains("ImageUnpinned")
            && content.contains("OtherPinned")
            && content.contains("OtherUnpinned"),
        "clipboard text/image and pin/unpin actions should be driven by named entry action plan states"
    );
    assert!(
        content.contains("ClipboardEntryActionPlan::from_entry(entry)")
            && content.contains("fn pin_action")
            && content.contains("fn is_image")
            && content.contains("fn is_text")
            && content.contains("Self::TextPinned | Self::ImagePinned | Self::OtherPinned")
            && content.contains("Self::TextUnpinned | Self::ImageUnpinned | Self::OtherUnpinned")
            && !content.contains("matches!(\n            self,\n            Self::TextPinned"),
        "clipboard context actions should derive pin/unpin and content-specific rows from named plan arms"
    );
}

#[test]
fn clipboard_pin_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardPinHandlerAction")
            && content.contains("Pin")
            && content.contains("Unpin"),
        "clipboard pin/unpin handler should be driven by named action states"
    );
    assert!(
        content.contains("ClipboardPinHandlerAction::from_action_id(action_id)")
            && content.contains("pin_action.apply(&entry.id)")
            && content.contains("pin_action.success_hud()")
            && content.contains("pin_action.selection_required_message()")
            && content.contains("pin_action.failure_message(e)"),
        "clipboard pin/unpin handler should derive storage operation, HUD text, and error text from the named action state"
    );
}

#[test]
fn clipboard_copy_paste_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardCopyPasteHandlerAction")
            && content.contains("PasteAndClose")
            && content.contains("CopyOnly")
            && content.contains("PasteKeepOpen"),
        "clipboard paste/copy handlers should be driven by named action states"
    );
    assert!(
        content.contains("ClipboardCopyPasteHandlerAction::from_action_id(action_id)")
            && content.contains("copy_paste_action.paste_close_behavior()")
            && content.contains("self.finalize_paste_after_clipboard_ready(")
            && content.contains("copy_paste_action.success_hud()")
            && content.contains("copy_paste_action.selection_required_message()")
            && content.contains("copy_paste_action.failure_message(e)"),
        "clipboard paste/copy handler should derive finalizer close behavior, HUD, guard, and error behavior from named states"
    );
}

#[test]
fn clipboard_history_paste_renderer_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/clipboard.rs")
        .expect("Failed to read clipboard history renderer");

    assert!(
        content.contains("enum ClipboardHistoryPasteAction")
            && content.contains("PasteSelectedEntry"),
        "clipboard history renderer copy/paste logs should be driven by a named action state"
    );
    assert!(
        content.contains("ClipboardHistoryPasteAction::PasteSelectedEntry")
            && content.contains("paste_action.copy_attempt_log(&entry.id)")
            && content.contains("paste_action.copy_failure_log(e)")
            && content.contains("paste_action.paste_failure_log(e)"),
        "clipboard history renderer should derive copy and paste failure logs from the named action state"
    );
    assert!(
        content.contains("format!(\"Copying clipboard entry: {entry_id}\")")
            && content.contains("format!(\"Failed to copy entry: {error}\")")
            && content.contains("format!(\"Failed to simulate paste: {error}\")"),
        "clipboard history paste feedback should preserve existing log copy"
    );
}

#[test]
fn clipboard_share_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardShareHandlerAction")
            && content.contains("TextLike")
            && content.contains("Image"),
        "clipboard share handler should classify share behavior with named action states"
    );
    assert!(
        content.contains("ClipboardShareHandlerAction::from_content_type(entry.content_type)")
            && content.contains("let share_result = share_action.share(content)")
            && content.contains("share_action.success_hud().to_string()")
            && content.contains("ClipboardShareHandlerAction::selection_required_message()")
            && content.contains("ClipboardShareHandlerAction::content_unavailable_message()"),
        "clipboard share handler should derive share item, HUD text, and guard text from the named action"
    );
    assert!(
        content.contains("ContentType::Text")
            && content.contains("ContentType::Link")
            && content.contains("ContentType::File")
            && content.contains("ContentType::Color")
            && content.contains("ContentType::Image")
            && content.contains("ShareSheetItem::Text(content)")
            && content.contains("ShareSheetItem::ImagePng")
            && content.contains("Failed to decode clipboard image")
            && content.contains("Share sheet opened"),
        "clipboard share action should preserve text-like and image share behavior"
    );
}

#[test]
fn clipboard_attach_to_ai_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardAttachToAiHandlerAction")
            && content.contains("TextInput")
            && content.contains("FileAttachment")
            && content.contains("ImageInput"),
        "clipboard attach-to-AI handler should classify content handoff with named action states"
    );
    assert!(
        content.contains("ClipboardAttachToAiHandlerAction::from_content_type(entry.content_type)")
            && content.contains("attach_action.deferred_action(content)")
            && content.contains("ClipboardAttachToAiHandlerAction::prepare_image_base64(&content)")
            && content.contains("ClipboardAttachToAiHandlerAction::selection_required_message()")
            && content.contains("ClipboardAttachToAiHandlerAction::content_unavailable_message()"),
        "clipboard attach-to-AI handler should derive guard and deferred handoff behavior from the named action"
    );
    assert!(
        content.contains("DeferredAgentChatAction::SetInput")
            && content.contains("DeferredAgentChatAction::AddAttachment")
            && content.contains("DeferredAgentChatAction::SetInputWithImage")
            && content.contains("Clipboard file path is empty")
            && content.contains("Failed to decode clipboard image")
            && content.contains("submit: false"),
        "clipboard attach-to-AI state should preserve text, file, image, and non-submitting handoff behavior"
    );
}

#[test]
fn clipboard_cleanshot_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardCleanShotHandlerAction")
            && content.contains("Annotate")
            && content.contains("Upload"),
        "clipboard CleanShot handlers should classify annotate/upload with named action states"
    );
    assert!(
        content.contains("ClipboardCleanShotHandlerAction::from_action_id(action_id)")
            && content.contains("cleanshot_action.image_required_message()")
            && content.contains("cleanshot_action.selection_required_message()")
            && content.contains("cleanshot_action.success_hud()")
            && content.contains("cleanshot_action.open_failure_message()")
            && content.contains("cleanshot_action.temp_save_failure_message()"),
        "clipboard CleanShot handlers should derive guards, HUD, and failure text from the named action"
    );
    assert!(
        content.contains("cleanshot://open-from-clipboard")
            && content.contains("cleanshot://open-annotate?filepath={}&action=upload")
            && content.contains("Opening CleanShot X…")
            && content.contains("Opening CleanShot X upload…")
            && content.contains("CleanShot actions are only available for images"),
        "clipboard CleanShot state should preserve annotate/upload URLs and user-facing copy"
    );
}

#[test]
fn clipboard_ocr_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardOcrHandlerAction") && content.contains("ExtractText"),
        "clipboard OCR handler should be driven by a named action state"
    );
    assert!(
        content.contains("let ocr_action = ClipboardOcrHandlerAction::ExtractText")
            && content.contains("ocr_action.selection_required_message()")
            && content.contains("ocr_action.image_required_message()")
            && content.contains("ocr_action.copied_hud()")
            && content.contains("ocr_action.load_failure_message()")
            && content.contains("ocr_action.decode_failure_message()")
            && content.contains("ocr_action.empty_text_message()")
            && content.contains("ocr_action.extract_failure_message(e)"),
        "clipboard OCR handler should derive guard, copy, and failure text from the named action"
    );
}

#[test]
fn clipboard_external_file_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardExternalFileHandlerAction")
            && content.contains("QuickLook")
            && content.contains("OpenWith"),
        "clipboard external file handlers should be driven by named action states"
    );
    assert!(
        content.contains("ClipboardExternalFileHandlerAction::from_action_id(action_id)")
            && content.contains("external_action.selection_required_message()")
            && content.contains("external_action.quick_look_failure_message(e)")
            && content.contains("external_action.load_failure_message()")
            && content.contains("external_action.temp_save_failure_message()")
            && content.contains("external_action.open_with_failure_message()")
            && content.contains("external_action.platform_name()"),
        "clipboard quick-look/open-with handlers should derive guard, failure, and platform text from named states"
    );
}

#[test]
fn clipboard_save_snippet_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardSaveSnippetHandlerAction")
            && content.contains("SaveSnippet"),
        "clipboard save-snippet handler should be driven by a named action state"
    );
    assert!(
        content.contains("let save_snippet_action = ClipboardSaveSnippetHandlerAction::SaveSnippet")
            && content.contains("save_snippet_action.selection_required_message()")
            && content.contains("save_snippet_action.text_required_message()")
            && content.contains("save_snippet_action.content_unavailable_message()")
            && content.contains("save_snippet_action.default_keyword()")
            && content.contains("save_snippet_action.create_failure_message(e)")
            && content.contains("save_snippet_action.success_hud(&keyword)")
            && content.contains("save_snippet_action.save_failure_message(e)"),
        "clipboard save-snippet handler should derive guard, default, success, and failure text from the named action"
    );
}

#[test]
fn clipboard_save_file_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardSaveFileHandlerAction") && content.contains("SaveFile"),
        "clipboard save-file handler should be driven by a named action state"
    );
    assert!(
        content.contains("let save_file_action = ClipboardSaveFileHandlerAction::SaveFile")
            && content.contains("save_file_action.selection_required_message()")
            && content.contains("save_file_action.content_unavailable_message()")
            && content.contains("save_file_action.decode_failure_message()")
            && content.contains("save_file_action.saved_hud(&save_path)")
            && content.contains("save_file_action.save_failure_message(e)")
            && content.contains("format!(\"Saved to: {save_path}\")"),
        "clipboard save-file handler should derive guard, saved HUD, and failure text from the named action"
    );
}

#[test]
fn clipboard_delete_entry_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardDeleteEntryHandlerAction")
            && content.contains("DeleteEntry"),
        "clipboard single-delete handler should be driven by a named action state"
    );
    assert!(
        content.contains("let delete_action = ClipboardDeleteEntryHandlerAction::DeleteEntry")
            && content.contains("delete_action.selection_required_message()")
            && content.contains("delete_action.success_hud()")
            && content.contains("delete_action.failure_message(e)"),
        "clipboard single-delete handler should derive selection, success, and failure text from the named action"
    );
}

#[test]
fn clipboard_bulk_delete_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardBulkDeleteHandlerAction")
            && content.contains("MatchingEntries")
            && content.contains("AllUnpinned"),
        "clipboard bulk delete handlers should be driven by named action states"
    );
    assert!(
        content.contains("ClipboardBulkDeleteHandlerAction::from_action_id(action_id)")
            && content.contains("bulk_delete_action.confirm_title()")
            && content.contains("bulk_delete_action.confirm_message(delete_count)")
            && content.contains("bulk_delete_action.confirm_message(unpinned_count)")
            && content.contains("bulk_delete_action.confirm_button()")
            && content.contains("action.success_hud(self.deleted)")
            && content.contains("bulk_delete_action.success_hud(unpinned_count)")
            && content.contains("action.partial_failure_message(self.deleted, self.failed)")
            && content.contains("bulk_delete_action.failure_message(e)"),
        "clipboard bulk delete handlers should derive confirmation, success, and failure text from named states"
    );
}

#[test]
fn clipboard_paste_destination_uses_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/clipboard.rs")
        .expect("Failed to read clipboard action builder");

    assert!(
        content.contains("enum ClipboardPasteDestinationPlan")
            && content.contains("FrontmostApp")
            && content.contains("ActiveAppFallback"),
        "clipboard paste title should be driven by named destination plan states"
    );
    assert!(
        content.contains("ClipboardPasteDestinationPlan::from_entry(entry)")
            && content.contains("paste_destination_plan.title(entry)"),
        "clipboard paste action title should derive from the named destination plan"
    );
}

#[test]
fn emoji_entry_actions_use_named_plan_states() {
    let content =
        fs::read_to_string("src/actions/builders/emoji.rs").expect("Failed to read emoji builder");

    assert!(
        content.contains("enum EmojiEntryActionPlan")
            && content.contains("PinnedWithCategory")
            && content.contains("PinnedWithoutCategory")
            && content.contains("UnpinnedWithCategory")
            && content.contains("UnpinnedWithoutCategory"),
        "emoji pin/unpin and section-copy actions should be driven by named entry action plan states"
    );
    assert!(
        content.contains("EmojiEntryActionPlan::from_info(emoji)")
            && content.contains("fn pin_action")
            && content.contains("fn has_category"),
        "emoji context actions should derive pin and category rows from the named plan"
    );
}

#[test]
fn emoji_pin_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/emoji.rs")
        .expect("Failed to read emoji action handler");

    assert!(
        content.contains("enum EmojiPinHandlerAction")
            && content.contains("Pin")
            && content.contains("Unpin"),
        "emoji pin/unpin handler should be driven by named action states"
    );
    assert!(
        content.contains("EmojiPinHandlerAction::from_action_id(action_id)")
            && content.contains("pin_action.apply(&mut self.pinned_emojis, &emoji.value)")
            && content.contains("pin_action.success_hud(&emoji.value)")
            && content.contains("pin_action.selection_required_message()")
            && content.contains("pin_action.failure_message(error)"),
        "emoji pin/unpin handler should derive mutation, HUD copy, and error copy from the named action state"
    );
}

#[test]
fn emoji_copy_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/emoji.rs")
        .expect("Failed to read emoji action handler");

    assert!(
        content.contains("enum EmojiCopyHandlerAction")
            && content.contains("Emoji")
            && content.contains("Unicode")
            && content.contains("Section"),
        "emoji copy/Unicode/section handlers should be driven by named action states"
    );
    assert!(
        content.contains("EmojiCopyHandlerAction::from_action_id(action_id)")
            && content.contains("copy_action.payload(&emoji)")
            && content.contains("copy_action.selection_required_message()")
            && content.contains("clipboard_text")
            && content.contains("hud_text"),
        "emoji copy handler should derive selection, clipboard, and HUD copy from the named state"
    );
}

#[test]
fn emoji_paste_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/emoji.rs")
        .expect("Failed to read emoji action handler");

    assert!(
        content.contains("enum EmojiPasteHandlerAction")
            && content.contains("Paste")
            && content.contains("PasteKeepOpen"),
        "emoji paste/paste-keep-open handlers should be driven by named action states"
    );
    assert!(
        content.contains("EmojiPasteHandlerAction::from_action_id(action_id)")
            && content.contains("paste_action.selection_required_message()")
            && content.contains("paste_action.trace_action()")
            && content.contains("paste_action.close_behavior()"),
        "emoji paste handler should derive empty-selection copy, tracing action, and close behavior from the named state"
    );
}

#[test]
fn emoji_paste_destination_uses_named_plan_states() {
    let content =
        fs::read_to_string("src/actions/builders/emoji.rs").expect("Failed to read emoji builder");

    assert!(
        content.contains("enum EmojiPasteDestinationPlan")
            && content.contains("FrontmostApp")
            && content.contains("ActiveAppFallback"),
        "emoji paste title should be driven by named destination plan states"
    );
    assert!(
        content.contains("EmojiPasteDestinationPlan::from_info(emoji)")
            && content.contains("paste_destination_plan.title(emoji)"),
        "emoji paste action title should derive from the named destination plan"
    );
}

#[test]
fn notes_command_bar_actions_use_named_plan_states() {
    let content =
        fs::read_to_string("src/actions/builders/notes.rs").expect("Failed to read notes builder");

    assert!(
        content.contains("enum NotesCommandBarActionPlan")
            && content.contains("EmptyActiveAutoSized")
            && content.contains("EmptyActiveNeedsAutoSizing")
            && content.contains("SelectedActiveAutoSized")
            && content.contains("SelectedActiveNeedsAutoSizing")
            && content.contains("EmptyTrashAutoSized")
            && content.contains("EmptyTrashNeedsAutoSizing")
            && content.contains("SelectedTrashAutoSized")
            && content.contains("SelectedTrashNeedsAutoSizing"),
        "notes command-bar selection/trash/auto-size actions should be driven by named plan states"
    );
    assert!(
        content.contains("NotesCommandBarActionPlan::from_info(info)")
            && content.contains("fn has_active_note_actions")
            && content.contains("fn has_trash_note_actions")
            && content.contains("fn needs_auto_sizing_action"),
        "notes command-bar actions should derive visible action groups from the named plan"
    );
}

#[test]
fn command_bar_open_feedback_uses_named_action_state() {
    let content =
        fs::read_to_string("src/actions/command_bar.rs").expect("Failed to read command bar");

    assert!(
        content.contains("enum CommandBarOpenFeedbackAction") && content.contains("OpenWindow"),
        "command-bar open logging should be driven by a named action state"
    );
    assert!(
        content.contains("CommandBarOpenFeedbackAction::OpenWindow")
            && content.contains("open_feedback.success_log(position)")
            && content.contains("open_feedback.failure_log(e)"),
        "command-bar open success and failure text should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Command bar opened at {position:?}\")")
            && content.contains("format!(\"Failed to open command bar: {error}\")"),
        "command-bar open feedback should preserve the existing log copy"
    );
}

#[test]
fn notes_new_chat_actions_use_named_plan_states() {
    let content =
        fs::read_to_string("src/actions/builders/notes.rs").expect("Failed to read notes builder");

    assert!(
        content.contains("enum NotesNewChatActionPlan")
            && content.contains("LastUsedModel")
            && content.contains("Preset")
            && content.contains("Model"),
        "notes new-chat model and preset row copy should be driven by named action states"
    );
    assert!(
        content.contains("NotesNewChatActionPlan::LastUsedModel")
            && content.contains("NotesNewChatActionPlan::Preset")
            && content.contains("NotesNewChatActionPlan::Model")
            && content.contains("action_plan.model_description")
            && content.contains("action_plan.preset_description"),
        "notes new-chat rows should derive model and preset descriptions from the named plan"
    );
}

#[test]
fn note_switcher_actions_use_named_plan_states() {
    let content =
        fs::read_to_string("src/actions/builders/notes.rs").expect("Failed to read notes builder");

    assert!(
        content.contains("enum NoteSwitcherRowPlan")
            && content.contains("PinnedCurrent")
            && content.contains("PinnedOther")
            && content.contains("Current")
            && content.contains("Recent"),
        "note switcher pinned/current priority should be driven by named row plan states"
    );
    assert!(
        content.contains("enum NoteSwitcherDescriptionPlan")
            && content.contains("PreviewWithRelativeTime")
            && content.contains("PreviewOnly")
            && content.contains("RelativeTimeOnly")
            && content.contains("CharacterCount"),
        "note switcher preview/time/count text should be driven by named description plan states"
    );
    assert!(
        content.contains("NoteSwitcherRowPlan::from_note(note)")
            && content.contains("NoteSwitcherDescriptionPlan::from_note(note)")
            && content.contains("fn truncated_preview"),
        "note switcher actions should derive row text and description text from named plans"
    );
}

#[test]
fn scriptlet_context_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/scriptlet.rs")
        .expect("Failed to read scriptlet builder");

    assert!(
        content.contains("enum ScriptletContextActionPlan")
            && content.contains("NoShortcutNoAlias")
            && content.contains("ShortcutOnly")
            && content.contains("AliasOnly")
            && content.contains("ShortcutAndAlias"),
        "scriptlet shortcut and alias action rows should be driven by named context plan states"
    );
    assert!(
        content.contains("ScriptletContextActionPlan::from_script(script)")
            && content.contains("fn has_shortcut")
            && content.contains("fn has_alias"),
        "scriptlet context actions should derive add/update/remove shortcut and alias rows from the named plan"
    );
}

#[test]
fn scriptlet_defined_action_shortcuts_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/scriptlet.rs")
        .expect("Failed to read scriptlet builder");

    assert!(
        content.contains("enum ScriptletDefinedActionShortcutPlan")
            && content.contains("NoShortcut")
            && content.contains("Shortcut(&'a str)"),
        "scriptlet-defined H3 action shortcuts should be driven by named shortcut plan states"
    );
    assert!(
        content
            .contains("ScriptletDefinedActionShortcutPlan::from_shortcut(sa.shortcut.as_deref())")
            && content.contains("shortcut_plan.apply_to_action(Action::new"),
        "scriptlet-defined action rows should apply shortcut copy through the named plan"
    );
}

#[test]
fn scriptlet_source_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scriptlets.rs")
        .expect("Failed to read scriptlet action handler");

    assert!(
        content.contains("enum ScriptletSourceHandlerAction")
            && content.contains("enum ScriptletSourceTargetError")
            && content.contains("Edit")
            && content.contains("RevealInFinder")
            && content.contains("CopyPath"),
        "scriptlet edit/reveal/copy path handlers should be driven by named action states"
    );
    assert!(
        content.contains("ScriptletSourceHandlerAction::from_action_id(action_id)")
            && content.contains("scriptlet_source_target(self.get_selected_result())")
            && content.contains("source_action.copied_hud(&target.path_text)")
            && content.contains("source_action.reveal_success_hud()")
            && content.contains("source_action.target_error_message(error, action_id)"),
        "scriptlet source handlers should derive target resolution and feedback from named states"
    );
}

#[test]
fn scriptlet_dynamic_action_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scriptlets.rs")
        .expect("Failed to read scriptlet action handler");

    assert!(
        content.contains("struct ScriptletDynamicHandlerAction")
            && content.contains("enum ScriptletDynamicExecutionResult")
            && content.contains("Success")
            && content.contains("Failed(String)")
            && content.contains("LaunchFailed(String)"),
        "scriptlet dynamic actions should be driven by named action and execution-result states"
    );
    assert!(
        content.contains("ScriptletDynamicHandlerAction::from_action_id(action_id)")
            && content.contains("action_id.strip_prefix(\"scriptlet_action:\")")
            && content.contains("dynamic_action.command()")
            && content.contains("ScriptletDynamicExecutionResult::from_exec_result")
            && content.contains("execution_result.success_hud(&action.name)")
            && content.contains("execution_result.error_toast()"),
        "scriptlet dynamic action parsing and feedback should derive from named states"
    );
    assert!(
        content.contains("Executed: {action_name}")
            && content.contains("Failed to execute action: {message}")
            && content.contains("Unknown error")
            && content.contains("Scriptlet action not found"),
        "scriptlet dynamic action states should preserve user-facing success and failure copy"
    );
}

#[test]
fn script_source_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
        .expect("Failed to read script action handler");

    assert!(
        content.contains("enum ScriptSourceHandlerAction")
            && content.contains("Edit")
            && content.contains("CopyContent"),
        "script edit/copy-content handlers should be driven by named source action states"
    );
    assert!(
        content.contains("ScriptSourceHandlerAction::from_action_id(action_id)")
            && content.contains("source_action.path_from_result(&result)")
            && content.contains("source_action.unsupported_message()")
            && content.contains("source_action.copied_hud()")
            && content.contains("source_action.read_error(e)"),
        "script source handlers should derive source paths, unsupported text, copied HUD, and read errors from named states"
    );
}

#[test]
fn script_management_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
        .expect("Failed to read script action handler");

    assert!(
        content.contains("enum ScriptManagementHandlerAction")
            && content.contains("CreateScript")
            && content.contains("ReloadScripts"),
        "script create/reload handlers should be driven by named management action states"
    );
    assert!(
        content.contains("ScriptManagementHandlerAction::from_action_id(action_id)")
            && content.contains("management_action.success_hud()")
            && content.contains("management_action.open_failure_message(e)")
            && content.contains("format!(\"Failed to open scripts folder: {error}\")"),
        "script create/reload handlers should derive HUD and open-failure copy from named states"
    );
}

#[test]
fn script_removal_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
        .expect("Failed to read script action handler");

    assert!(
        content.contains("enum ScriptRemovalHandlerAction")
            && content.contains("enum ScriptRemovalTargetError")
            && content.contains("MoveToTrash")
            && content.contains("NoSelection")
            && content.contains("UnsupportedItemType")
            && content.contains("MissingPath"),
        "script removal should model target resolution and Trash feedback as named states"
    );
    assert!(
        content.contains("ScriptRemovalHandlerAction::from_action_id(action_id)")
            && content.contains("removal_action.target_error_message(")
            && content.contains("removal_action.confirm_body(&target)")
            && content.contains("removal_action.confirm_title()")
            && content.contains("removal_action.success_hud(&target)")
            && content.contains("removal_action.failure_message(e)"),
        "script removal should derive target errors, confirmation copy, success HUD, and failure copy from the named state"
    );
}

#[test]
fn settings_editor_launch_uses_named_plan_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
        .expect("Failed to read script action handler");

    assert!(
        content.contains("enum SettingsEditorLaunchPlan")
            && content.contains("ReuseWindowWithProject")
            && content.contains("FileOnlyZed")
            && content.contains("AddToSublimeProject")
            && content.contains("GenericFileOnly"),
        "settings editor launch should be driven by named editor plan states"
    );
    assert!(
        content.contains("SettingsEditorLaunchPlan::from_editor(&editor)")
            && content.contains(".spawn(&editor, &config_dir, &config_file)")
            && content.contains("launch_plan.success_hud(&editor_for_hud)")
            && content.contains("launch_plan.failure_message(&editor_for_hud, e)")
            && content.contains("\"code\" | \"cursor\"")
            && content.contains("Command::new(\"zed\")")
            && content.contains("Command::new(\"subl\")")
            && content.contains("Command::new(editor).arg(config_file).spawn()"),
        "settings should derive editor-specific command arguments and feedback copy from the named launch plan"
    );
}

#[test]
fn scriptlet_ranking_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/scriptlet.rs")
        .expect("Failed to read scriptlet builder");

    assert!(
        content.contains("enum ScriptletRankingActionPlan")
            && content.contains("NoRankingAction")
            && content.contains("ResetSuggestedRanking"),
        "scriptlet suggested ranking row should be driven by named ranking plan states"
    );
    assert!(
        content.contains("ScriptletRankingActionPlan::from_script(script)")
            && content.contains("fn reset_action")
            && content.contains("ranking_plan.reset_action()"),
        "scriptlet ranking row should derive availability and copy from the named plan"
    );
}

#[test]
fn file_path_actions_use_named_item_plan_states() {
    let content = fs::read_to_string("src/actions/builders/file_path.rs")
        .expect("Failed to read file path builder");

    assert!(
        content.contains("enum FileItemActionPlan")
            && content.contains("File")
            && content.contains("Directory"),
        "file/path primary, trash, and attach actions should be driven by named item plan states"
    );
    assert!(
        content.contains("FileItemActionPlan::from_is_dir(file_info.is_dir)")
            && content.contains("FileItemActionPlan::from_is_dir(path_info.is_dir)")
            && content.contains("fn file_context_primary_action")
            && content.contains("fn path_context_primary_action")
            && content.contains("fn supports_attach_to_ai")
            && content.contains("fn item_noun"),
        "file/path actions should derive file-vs-folder copy and availability from the named plan"
    );
}

#[test]
fn file_search_secondary_descriptions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/file_path.rs")
        .expect("Failed to read file path builder");

    assert!(
        content.contains("enum FileSearchSecondaryDescriptionPlan")
            && content.contains("StaticDescription")
            && content.contains("TrashItem"),
        "file-search secondary command descriptions should be driven by named description plan states"
    );
    assert!(
        content.contains("FileSearchSecondaryDescriptionPlan::from_action_id(self.action_id)")
            && content.contains("description_plan.description(self.description, action_plan)"),
        "file-search secondary actions should derive static/trash descriptions from the named plan"
    );
}

#[test]
fn file_search_handler_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchHandlerAction")
            && content.contains("Open")
            && content.contains("QuickLook")
            && content.contains("OpenWith")
            && content.contains("ShowInfo")
            && content.contains("AttachToAi"),
        "safe file-search handler actions should be driven by named action states"
    );
    assert!(
        content.contains("FileSearchHandlerAction::from_action_id(action_id)")
            && content.contains("file_action.success_hud(action_id)")
            && content.contains("file_action.error_prefix(action_id)")
            && content.contains("file_action.hides_main_after_success()"),
        "file-search handler should derive HUD, error, and hide behavior from the named action state"
    );
}

#[test]
fn file_search_editor_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchEditorHandlerAction") && content.contains("OpenInEditor"),
        "file-search open-in-editor handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchEditorHandlerAction::from_action_id(action_id)")
            && content.contains("editor_action.selection_required_message()")
            && content.contains("editor_action.success_hud()")
            && content.contains("editor_action.failure_message(e)"),
        "file-search open-in-editor handler should derive selection, HUD, and failure copy from the named state"
    );
}

#[test]
fn file_search_rename_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchRenameHandlerAction") && content.contains("RenamePath"),
        "file-search rename handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchRenameHandlerAction::from_action_id(action_id)")
            && content.contains("rename_action.selection_required_message()")
            && content.contains("rename_action.success_hud(&new_name)")
            && content.contains("rename_action.failure_message(e)"),
        "file-search rename handler should derive selection, HUD, and failure copy from the named state"
    );
}

#[test]
fn file_search_move_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchMoveHandlerAction") && content.contains("MovePath"),
        "file-search move handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchMoveHandlerAction::from_action_id(action_id)")
            && content.contains("move_action.selection_required_message()")
            && content.contains("move_action.success_hud(&destination_dir)")
            && content.contains("move_action.failure_message(e)"),
        "file-search move handler should derive selection, HUD, and failure copy from the named state"
    );
}

#[test]
fn file_search_duplicate_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchDuplicateHandlerAction")
            && content.contains("DuplicatePath"),
        "file-search duplicate handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchDuplicateHandlerAction::from_action_id(action_id)")
            && content.contains("duplicate_action.selection_required_message()")
            && content.contains("duplicate_action.success_hud(&name)")
            && content.contains("duplicate_action.failure_message(e)"),
        "file-search duplicate handler should derive selection, HUD, and failure copy from the named state"
    );
}

#[test]
fn file_search_trash_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchTrashHandlerAction") && content.contains("MoveToTrash"),
        "file-search trash handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchTrashHandlerAction::from_action_id(action_id)")
            && content.contains("trash_action.selection_required_message()")
            && content.contains("trash_action.confirm_title()")
            && content.contains("trash_action.confirm_message(&name)")
            && content.contains("trash_action.confirm_button()")
            && content.contains("trash_action.confirmation_failure_message()")
            && content.contains("trash_action.success_hud(&name)")
            && content.contains("trash_action.failure_message(e)"),
        "file-search trash handler should derive selection, confirmation, HUD, and failure copy from the named state"
    );
}

#[test]
fn file_search_copy_filename_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchFilenameCopyHandlerAction")
            && content.contains("CopyFilename"),
        "file-search copy-filename handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchFilenameCopyHandlerAction::from_action_id(action_id)")
            && content.contains("copy_filename_action.selection_required_message()")
            && content.contains("copy_filename_action.copied_hud(&name)"),
        "file-search copy-filename handler should derive selection and copied-HUD copy from the named state"
    );
}

#[test]
fn file_search_copy_path_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchPathCopyHandlerAction") && content.contains("CopyPath"),
        "file-search copy-path handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchPathCopyHandlerAction::from_action_id(action_id)")
            && content.contains("copy_path_action.copied_hud(&path_str)"),
        "file-search copy-path handler should derive copied-HUD copy from the named state"
    );
}

#[test]
fn file_search_copy_deeplink_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchDeeplinkCopyHandlerAction")
            && content.contains("CopyDeeplink"),
        "file-search copy-deeplink handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchDeeplinkCopyHandlerAction::from_action_id(action_id)")
            && content.contains("deeplink_action.share_hud(&bundle.title)")
            && content.contains("deeplink_action.deeplink_hud(&deeplink_url)")
            && content.contains("deeplink_action.share_failure_message(error)"),
        "file-search copy-deeplink handler should derive share, fallback, and failure copy from the named state"
    );
}

#[test]
fn file_search_reveal_handler_uses_named_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchRevealHandlerAction")
            && content.contains("RevealInFinder"),
        "file-search reveal handler should be driven by a named action state"
    );
    assert!(
        content.contains("FileSearchRevealHandlerAction::from_action_id(action_id)")
            && content.contains("reveal_action.success_hud()")
            && content.contains("reveal_action.unsupported_message()"),
        "file-search reveal handler should derive success and unsupported copy from the named state"
    );
}

#[test]
fn file_search_sort_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchSortHandlerAction")
            && content.contains("NameAsc")
            && content.contains("NameDesc")
            && content.contains("ModifiedDesc")
            && content.contains("ModifiedAsc"),
        "file-search sort handler should be driven by named sort action states"
    );
    assert!(
        content.contains("FileSearchSortHandlerAction::from_action_id(action_id)")
            && content.contains("let mode = sort_action.mode()")
            && content.contains("sort_action.success_hud()"),
        "file-search sort handler should derive sort mode and HUD copy from the named state"
    );
}

#[test]
fn file_search_current_directory_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/files.rs")
        .expect("Failed to read file action handler");

    assert!(
        content.contains("enum FileSearchCurrentDirectoryAction")
            && content.contains("Refresh")
            && content.contains("Reveal")
            && content.contains("CopyPath")
            && content.contains("OpenQuickTerminal"),
        "file-search current-directory handlers should be driven by named action states"
    );
    assert!(
        content.contains("FileSearchCurrentDirectoryAction::from_action_id(action_id)")
            && content.contains("directory_action.missing_directory_message()")
            && content.contains("directory_action.success_hud(&dir)")
            && content.contains("directory_action.error_prefix()"),
        "file-search current-directory handler should derive missing-target, HUD, and error copy from named states"
    );
}

#[test]
fn file_search_sort_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/file_path.rs")
        .expect("Failed to read file path builder");

    assert!(
        content.contains("enum FileSearchSortActionPlan")
            && content.contains("ActiveSort")
            && content.contains("AvailableSort"),
        "file-search sort checkmark and current-sort text should be driven by named sort action plan states"
    );
    assert!(
        content.contains("FileSearchSortActionPlan::from_active(name_asc_active)")
            && content.contains("fn title")
            && content.contains("fn description"),
        "file-search sort actions should derive title and description copy from the named sort plan"
    );
}

#[test]
fn script_context_preference_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum ScriptContextPreferenceActionPlan")
            && content.contains("AgentNoPreferenceActions")
            && content.contains("NoShortcutNoAlias")
            && content.contains("ShortcutOnly")
            && content.contains("AliasOnly")
            && content.contains("ShortcutAndAlias"),
        "script context shortcut and alias rows should be driven by named preference plan states"
    );
    assert!(
        content.contains("preference_action_plan(script)")
            && content.contains("fn append_shortcut_preference_actions")
            && content.contains("fn append_alias_preference_actions"),
        "script context preference rows should be appended from the named plan"
    );
}

#[test]
fn shortcut_alias_handler_uses_named_target_error_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/shortcuts.rs")
        .expect("Failed to read shortcut alias action handler");

    assert!(
        content.contains("enum ShortcutAliasTargetError")
            && content.contains("NoSelection")
            && content.contains("UnsupportedItemType")
            && content.contains("MissingCommandId"),
        "shortcut and alias handlers should model target-resolution failures as named states"
    );
    assert!(
        content.contains("shortcut_action.target_error_message(")
            && content.contains("alias_action.target_error_message(")
            && content.contains("remove_action.target_error_message(")
            && content.contains("ShortcutAliasTargetError::MissingCommandId")
            && content.contains("selection_required_message_for_action(action_id)"),
        "shortcut and alias handlers should derive missing-target and no-selection copy from named action states"
    );
}

#[test]
fn script_context_share_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum ScriptContextShareActionPlan")
            && content.contains("PortableShareLink")
            && content.contains("DirectRunDeepLink")
            && content.contains("struct ScriptContextShareActionCopy"),
        "script context Share vs Copy Deep Link copy should be driven by named share plan states"
    );
    assert!(
        content.contains("share_action_plan(script)")
            && content.contains("fn share_action_copy")
            && content.contains("let share_copy = share_action_copy(script)"),
        "script context copy_deeplink row should derive title and description from the named share plan"
    );
}

#[test]
fn script_context_favorite_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum FavoriteActionPlan")
            && content.contains("AddToFavorites")
            && content.contains("RemoveFromFavorites"),
        "script context favorite action copy should be driven by named favorite plan states"
    );
    assert!(
        content.contains("FavoriteActionPlan::from_is_favorite(is_favorite).copy()")
            && content.contains("fn favorite_action_copy"),
        "favorite action copy should derive title and description from the named favorite plan"
    );
}

#[test]
fn favorites_browse_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/favorites.rs")
        .expect("Failed to read favorites action handler");

    assert!(
        content.contains("enum FavoritesBrowseHandlerAction")
            && content.contains("Run")
            && content.contains("EditScript")
            && content.contains("CopyScriptUrl")
            && content.contains("MoveUp")
            && content.contains("MoveDown")
            && content.contains("Remove"),
        "Favorites browse edit/copy URL/move/remove handlers should be driven by named action states"
    );
    assert!(
        content.contains("FavoritesBrowseHandlerAction::from_action_id(action_id)")
            && content.contains("favorites_action.run_selected(self, window, cx)")
            && content.contains("favorites_action.run_outcome(run_result)")
            && content.contains("favorites_action.selection_required_message()")
            && content.contains("favorites_action.copied_url_hud(&deeplink_url)")
            && content.contains("favorites_action.apply_list_mutation(self, cx)")
            && content.contains("favorites_action.mutation_outcome(message_result)"),
        "Favorites browse handlers should derive run, required-selection, copied URL, and list mutation feedback from the named state"
    );
}

#[test]
fn favorites_browse_renderer_uses_named_list_action_states() {
    let content = fs::read_to_string("src/render_builtins/favorites.rs")
        .expect("Failed to read favorites renderer");

    assert!(
        content.contains("enum FavoritesBrowseListAction")
            && content.contains("Run")
            && content.contains("Remove")
            && content.contains("MoveUp")
            && content.contains("MoveDown"),
        "Favorites browse renderer result feedback should be driven by named list action states"
    );
    assert!(
        content.contains("action.selection_required_message()")
            && content.contains("action.success_message(&id)")
            && content.contains("action.missing_favorite_message(&id)")
            && content.contains(".boundary_message(&id)")
            && content.contains("action.failure_message(e)"),
        "Favorites run/remove/reorder results should derive selection, success, boundary, and failure copy from the named state"
    );
    assert!(
        content.contains("format!(\"Running '{id}'\")")
            && content.contains("format!(\"Removed '{id}'\")")
            && content.contains("format!(\"Moved '{id}' up\")")
            && content.contains("format!(\"Moved '{id}' down\")")
            && content.contains("format!(\"Failed to remove favorite: {error}\")")
            && content.contains("format!(\"Failed to move favorite up: {error}\")")
            && content.contains("format!(\"Failed to move favorite down: {error}\")"),
        "Favorites renderer feedback should preserve existing visible copy"
    );
}

#[test]
fn dictation_history_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/dictation_history.rs")
        .expect("Failed to read dictation history action handler");

    assert!(
        content.contains("enum DictationHistoryHandlerAction")
            && content.contains("Paste")
            && content.contains("AttachToAi")
            && content.contains("SaveNote")
            && content.contains("Copy")
            && content.contains("Delete"),
        "dictation history paste/attach/save/copy/delete handlers should be driven by named action states"
    );
    assert!(
        content.contains("DictationHistoryHandlerAction::from_action_id(action_id)")
            && content.contains("history_action.selection_required_message()")
            && content.contains("history_action.user_message()")
            && content.contains("history_action.success_hud()")
            && content.contains("error_prefix()")
            && content.contains("history_action.failure_message(error)")
            && content.contains("\"Deleted dictation\"")
            && content.contains("\"Failed to delete dictation\""),
        "dictation history handlers should derive empty-selection, user message, HUD, and error copy from named states"
    );
}

#[test]
fn dictation_builtin_execution_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum DictationBuiltinAction")
            && content.contains("CurrentSurface")
            && content.contains("AgentChat")
            && content.contains("FrontmostApp")
            && content.contains("Notes"),
        "dictation built-ins should be routed through named action states"
    );
    assert!(
        content.contains("fn execute_dictation_builtin_action(")
            && content.contains("fn prepare_dictation_builtin_start(")
            && content.contains("fn handle_dictation_started(")
            && content.contains("action.opening_message()")
            && content.contains("action.failure_message()")
            && content.contains("action.success_detail()"),
        "dictation start, toggle, and feedback behavior should derive from the named action state"
    );
}

#[test]
fn paste_sequential_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum PasteSequentialBuiltinAction")
            && content.contains("PasteEntry(String)")
            && content.contains("SequenceExhausted")
            && content.contains("HistoryEmpty"),
        "Paste Sequentially built-in feedback should be driven by named action states"
    );
    assert!(
        content.contains("PasteSequentialBuiltinAction::from_outcome(")
            && content.contains("paste_action.telemetry_event()")
            && content.contains("paste_action.log_message()")
            && content.contains("paste_action.hud_message()")
            && content.contains("paste_action.success_detail()"),
        "Paste Sequentially should derive telemetry, HUD text, and success detail from the named state"
    );
}

#[test]
fn settings_snap_mode_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SettingsSnapModeBuiltinAction")
            && content.contains("Disable")
            && content.contains("Simple")
            && content.contains("Expanded")
            && content.contains("Precision"),
        "settings snap-mode built-ins should be routed through named action states"
    );
    assert!(
        content.contains("SettingsSnapModeBuiltinAction::from_command(command)")
            && content.contains("let target_mode = action.target_mode()")
            && content.contains("action.hud_text()")
            && content.contains("action.success_detail()"),
        "snap-mode target mode, HUD text, and success detail should derive from the named state"
    );
    assert!(
        content.contains("action.persistence_failure_log()")
            && content.contains("action.persistence_failure_hud(&error)")
            && content.contains("action.persistence_failure_code()")
            && content.contains("action.persistence_failure_message()")
            && content.contains("action.runtime_transition_failure_log()"),
        "snap-mode persistence and runtime-transition failure copy should derive from the named state"
    );
    assert!(
        content.contains("\"set_snap_mode_failed\"")
            && content.contains("format!(\"Failed to update snap mode: {error}\")")
            && content.contains("\"Failed to save snap mode\""),
        "snap-mode failure state should preserve current user-facing copy and error code"
    );
}

#[test]
fn permission_assistant_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum PermissionAssistantBuiltinAction")
            && content.contains("Accessibility")
            && content.contains("ScreenRecording"),
        "Permission Assistant built-ins should be routed through named action states"
    );
    assert!(
        content.contains("PermissionAssistantBuiltinAction::from_command(")
            && content.contains("fn execute_permission_assistant_builtin(")
            && content.contains("action.panel()")
            && content.contains("action.success_hud()")
            && content.contains("action.success_detail()")
            && content.contains("action.failure_message(&error)")
            && content.contains("action.failure_detail()"),
        "Permission Assistant panel, HUD text, failure copy, and dispatch details should derive from the named state"
    );
    assert!(
        content.contains("format!(\"Failed to open Permission Assistant: {error}\")"),
        "Permission Assistant state should preserve failure toast copy"
    );
}

#[test]
fn utility_open_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum UtilityOpenBuiltinAction")
            && content.contains("MainWindow")
            && content.contains("ScratchPad")
            && content.contains("QuickTerminal")
            && content.contains("ClaudeCode")
            && content.contains("ProcessManager"),
        "safe utility open built-ins should be routed through named action states"
    );
    assert!(
        content.contains("UtilityCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_utility_open_builtin(")
            && content.contains("action.opening_message()")
            && content.contains("action.opens_from_main_menu()")
            && content.contains("action.success_detail()"),
        "utility open logging, launcher-origin state, and success detail should derive from the named state"
    );
}

#[test]
fn browser_window_filterable_builtins_use_named_copy_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum BrowserTabsBuiltinAction")
            && content.contains("fn execute_browser_tabs_builtin(")
            && content.contains("action.opening_message()")
            && content.contains("action.loaded_message()")
            && content.contains("action.placeholder()")
            && content.contains("action.failure_message(&error)")
            && content.contains("action.failure_detail()"),
        "Browser Tabs built-in should derive logs, placeholder, failure copy, and failure detail from the named state"
    );
    assert!(
        content.contains("enum WindowSwitcherBuiltinAction")
            && content.contains("fn execute_window_switcher_builtin(")
            && content.contains("action.opening_message()")
            && content.contains("action.loaded_message()")
            && content.contains("action.placeholder()")
            && content.contains("action.failure_message(&error)")
            && content.contains("action.failure_detail()"),
        "Window Switcher built-in should derive logs, placeholder, failure copy, and failure detail from the named state"
    );
    assert!(
        content.contains("\"Search open browser tabs...\"")
            && content.contains("\"Search windows...\"")
            && content.contains("format!(\"Failed to list browser tabs: {error}\")")
            && content.contains("format!(\"Failed to list windows: {error}\")"),
        "filterable built-in state methods should preserve current user-facing copy"
    );
}

#[test]
fn notes_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum NotesCommandBuiltinAction")
            && content.contains("OpenNotes")
            && content.contains("NewNote")
            && content.contains("SearchNotes")
            && content.contains("QuickCapture"),
        "Notes command built-ins should be routed through named action states"
    );
    assert!(
        content.contains("NotesCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_notes_command_builtin(")
            && content
                .contains("NotesCommandBuiltinAction::SearchNotes => notes::open_notes_search(cx)")
            && content.contains("action.success_detail()")
            && content.contains("action.failure_message(&e)")
            && content.contains("action.failure_detail()"),
        "Notes command routing and dispatch details should derive from the named state"
    );
    assert!(
        content.contains("format!(\"Notes command failed: {error}\")"),
        "Notes command failure copy should derive from the named state"
    );
}

#[test]
fn script_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum ScriptCommandBuiltinAction")
            && content.contains("NewScript")
            && content.contains("NewExtension"),
        "Script creation built-ins should be routed through named action states"
    );
    assert!(
        content.contains("ScriptCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_script_command_builtin(")
            && content.contains("action.naming_target()")
            && content.contains("action.success_detail()"),
        "Script command routing and dispatch details should derive from the named state"
    );
    assert!(
        content.contains("Self::NewScript => prompts::NamingTarget::Script")
            && content.contains("Self::NewExtension => prompts::NamingTarget::Extension"),
        "Script command action states should own their naming dialog targets"
    );
}

#[test]
fn frecency_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum FrecencyCommandBuiltinAction") && content.contains("ClearSuggested"),
        "Frecency built-ins should be routed through named action states"
    );
    assert!(
        content.contains("FrecencyCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_frecency_command_builtin(")
            && content.contains("action.hud_text()")
            && content.contains("action.success_detail()")
            && content.contains("action.failure_message(&e)")
            && content.contains("action.failure_detail()"),
        "Frecency command routing and dispatch details should derive from the named state"
    );
    assert!(
        content.contains("format!(\"Failed to clear suggested: {error}\")"),
        "Clear Suggested failure copy should derive from the named state"
    );
    assert!(
        content.contains("self.frecency_store.clear()")
            && content.contains("self.invalidate_grouped_cache()")
            && content.contains("resize_to_view_sync(ViewType::ScriptList, 0)"),
        "Clear Suggested should keep clearing frecency, invalidating grouped cache, and resizing the script list"
    );
}

#[test]
fn settings_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SettingsCommandBuiltinAction")
            && content.contains("ResetWindowPositions")
            && content.contains("ChooseTheme")
            && content.contains("DictationSetup")
            && content.contains("SelectMicrophone(SettingsMicrophoneBuiltinAction)")
            && content.contains("SnapMode(SettingsSnapModeBuiltinAction)"),
        "Settings built-ins should be routed through named action states"
    );
    assert!(
        content.contains("SettingsCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_settings_command_builtin(")
            && content.contains("fn execute_settings_snap_mode_builtin(")
            && content.contains("fn execute_select_microphone_builtin("),
        "Settings command routing should delegate to focused state handlers"
    );
    assert!(
        content.contains("SettingsSnapModeBuiltinAction::from_command(command)")
            && content
                .contains("expect(\"snap mode settings command should map to snap mode action\")")
            && content.contains("Self::builtin_success(dctx, action.success_detail())")
            && content.contains("SettingsMicrophoneBuiltinAction::from_command(command)")
            && content
                .contains("expect(\"select microphone command should map to microphone action\")")
            && content.contains("self.execute_select_microphone_builtin(microphone_action"),
        "Settings command state should preserve snap-mode and microphone success details"
    );
}

#[test]
fn select_microphone_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SettingsMicrophoneBuiltinAction")
            && content.contains("fn execute_select_microphone_builtin(")
            && content.contains("action.enumeration_failure_log()")
            && content.contains("action.failure_hud(&error)")
            && content.contains("action.failure_code()")
            && content.contains("action.failure_message()")
            && content.contains("action.placeholder().to_string()")
            && content.contains("action.success_detail()"),
        "Select Microphone should derive failure copy, placeholder text, and success detail from the named state"
    );
    assert!(
        content.contains("\"select_microphone\"")
            && content.contains("\"select_microphone_failed\"")
            && content.contains("format!(\"Failed to list microphones: {error}\")")
            && content.contains("\"Failed to list microphones\"")
            && content.contains("\"Select microphone...\""),
        "Select Microphone named state should preserve current success detail, error code, error copy, and placeholder"
    );
}

#[test]
fn create_ai_preset_form_uses_named_submit_state() {
    let content = fs::read_to_string("src/render_builtins/ai_presets.rs")
        .expect("Failed to read AI presets renderer");

    assert!(
        content.contains("enum CreateAiPresetFormAction") && content.contains("Submit"),
        "create AI preset form submit feedback should be driven by a named action state"
    );
    assert!(
        content.contains("CreateAiPresetFormAction::Submit")
            && content.contains("form_action.success_hud(&preset.name)")
            && content.contains("form_action.failure_message(e)"),
        "create AI preset success HUD and failure toast should derive from the named submit state"
    );
    assert!(
        content.contains("format!(\"Preset '{preset_name}' created\")")
            && content.contains("format!(\"Failed to create preset: {error}\")"),
        "create AI preset form feedback should preserve existing visible copy"
    );
}

#[test]
fn ai_capture_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiCaptureBuiltinAction")
            && content.contains("FullScreen")
            && content.contains("FocusedWindow")
            && content.contains("SelectedText")
            && content.contains("BrowserTab"),
        "AI capture built-ins should be routed through named action states"
    );
    assert!(
        content.contains("AiCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_ai_capture_builtin(")
            && content.contains("action.capture_kind()")
            && content.contains("action.prompt()")
            && content.contains("action.success_detail()"),
        "AI capture commands should delegate through their named state"
    );
    assert!(
        content.contains("Self::FullScreen => crate::ai::TabAiCaptureKind::FullScreen")
            && content
                .contains("Self::FocusedWindow => crate::ai::TabAiCaptureKind::FocusedWindow")
            && content.contains("Self::SelectedText => crate::ai::TabAiCaptureKind::SelectedText")
            && content.contains("Self::BrowserTab => crate::ai::TabAiCaptureKind::BrowserTab"),
        "AI capture states should own their capture kind mapping"
    );
}

#[test]
fn ai_generate_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiGenerateBuiltinAction")
            && content.contains("NewScript")
            && content.contains("CurrentAppScript"),
        "AI generation built-ins should be routed through named action states"
    );
    assert!(
        content.contains("AiGenerateBuiltinAction::from_command(command)")
            && content.contains("fn execute_ai_generate_builtin(")
            && content.contains("query_override.unwrap_or(&self.filter_text)")
            && content.contains("action.normalized_request(query)")
            && content.contains("action.entry_intent(request)")
            && content.contains("action.opens_agent_chat_when_request_empty()")
            && content.contains("action.success_detail()"),
        "AI generation commands should delegate query handling, routing fallback, prompt text, and success details through their state"
    );
    assert!(
        content.contains("normalize_generate_script_request(Some(")
            && content.contains("normalize_generate_script_from_current_app_request")
            && content.contains("Some(query)")
            && content.contains("open_tab_ai_chat_with_entry_intent")
            && content.contains("open_tab_ai_agent_chat_with_entry_intent"),
        "AI generation state should preserve direct script and current-app intent routing"
    );
    assert!(
        content.contains("User request: {request}")
            && content.contains("using the current menu, selection, and browser context.")
            && content.contains("Self::NewScript => true")
            && content.contains("Self::CurrentAppScript => false"),
        "AI generation state should preserve current-app prompt copy and empty-request routing behavior"
    );
}

#[test]
fn ai_unavailable_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiUnavailableBuiltinAction")
            && content.contains("ScreenAreaCapture"),
        "Unavailable AI built-ins should be routed through named action states"
    );
    assert!(
        content.contains("AiUnavailableBuiltinAction::from_command(")
            && content.contains("*cmd_type")
            && content.contains("fn execute_ai_unavailable_builtin(")
            && content.contains("action.message()")
            && content.contains("action.failure_detail()"),
        "Unavailable AI command copy and error details should derive from the named state"
    );
    assert!(
        content.contains("Send Screen Area to Agent Chat is unavailable")
            && content.contains("ai_send_screen_area_unavailable")
            && content.contains("Toast::error(message, &self.theme)"),
        "Send Screen Area should keep its unavailable toast and error detail"
    );
}

#[test]
fn permission_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum PermissionCommandBuiltinAction")
            && content.contains("CheckPermissions")
            && content.contains("RequestAccessibility")
            && content.contains("OpenAccessibilitySettings")
            && content.contains("Assistant(PermissionAssistantBuiltinAction)"),
        "Permission built-ins should be routed through named action states"
    );
    assert!(
        content.contains("PermissionCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_permission_command_builtin(")
            && content.contains("PermissionAssistantBuiltinAction::from_command(command)")
            && content.contains(
                "expect(\"permission assistant command should map to assistant action\")"
            )
            && content.contains("action.success_detail()")
            && content.contains("action.failure_detail()"),
        "Permission command routing should delegate through named state details"
    );
    assert!(
        content.contains("action.all_permissions_granted_hud().to_string()")
            && content.contains("action.accessibility_granted_hud().to_string()")
            && content.contains("action.accessibility_not_granted_warning()")
            && content.contains("action.open_settings_failure_message(&e)"),
        "Permission command feedback copy should derive from the named state"
    );
    assert!(
        content.contains("All permissions granted!")
            && content.contains("Accessibility permission granted!")
            && content
                .contains("Accessibility permission not granted. Some features may not work.")
            && content.contains("format!(\"Failed to open settings: {error}\")")
            && content.contains("open_accessibility_settings_failed"),
        "Permission command states should preserve user-facing permission feedback"
    );
}

#[test]
fn utility_process_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum UtilityProcessBuiltinAction")
            && content.contains("StopAllProcesses"),
        "Utility process built-ins should be routed through named action states"
    );
    assert!(
        content.contains("UtilityCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_utility_process_builtin(")
            && content.contains("action.empty_hud()")
            && content.contains("action.success_hud(outcome.process_count())")
            && content.contains("action.success_detail()"),
        "Utility process command routing should delegate through named state details"
    );
    assert!(
        content.contains("PROCESS_MANAGER.active_count()")
            && content.contains("PROCESS_MANAGER.kill_all_processes()")
            && content.contains("Stopped {process_count} running script process(es).")
            && content.contains("stop_all_processes"),
        "Stop All Processes should preserve its guarded count check, destructive kill path, and success detail"
    );
}

#[test]
fn process_manager_terminate_selected_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/process_manager.rs")
        .expect("Failed to read process manager renderer");

    assert!(
        content.contains("enum ProcessManagerTerminateAction")
            && content.contains("StopSelectedProcess"),
        "process manager selected-process termination feedback should be driven by a named action state"
    );
    assert!(
        content.contains("ProcessManagerTerminateAction::StopSelectedProcess")
            && content.contains("terminate_action.success_hud(script_name)")
            && content.contains("terminate_action.failure_message(pid, &err_msg)"),
        "process manager terminate success and failure feedback should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Stopped {script_name}\")")
            && content.contains("format!(\"Failed to stop PID {pid}: {error}\")"),
        "process manager terminate feedback should preserve existing visible copy"
    );
}

#[test]
fn utility_command_dispatch_uses_named_wrapper_state() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum UtilityCommandBuiltinAction")
            && content.contains("Open(UtilityOpenBuiltinAction)")
            && content.contains("Process(UtilityProcessBuiltinAction)")
            && content.contains("Recipe(UtilityRecipeBuiltinAction)")
            && content.contains("DoInCurrentApp(UtilityDoInCurrentAppBuiltinAction)")
            && content.contains("CurrentAppCommands(UtilityCurrentAppCommandsBuiltinAction)"),
        "Utility command dispatch should use a named wrapper state over existing leaf actions"
    );
    assert!(
        content
            .contains("let utility_action = UtilityCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_utility_command_builtin(")
            && content.contains("self.execute_utility_open_builtin(open_action")
            && content.contains("self.execute_utility_process_builtin(process_action")
            && content.contains("execute_utility_turn_this_into_command_builtin")
            && content.contains(".execute_utility_do_in_current_app_builtin(do_in_action")
            && content.contains("self.execute_utility_current_app_commands_builtin("),
        "Utility command wrapper should route to every existing leaf executor"
    );
}

#[test]
fn utility_turn_this_into_command_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("TurnThisIntoCommand")
            && content.contains("UtilityRecipeBuiltinAction::from_command(command)")
            && content.contains("fn execute_utility_turn_this_into_command_builtin("),
        "Turn This Into a Command should share named utility recipe action routing"
    );
    assert!(
        content.contains("normalize_turn_this_into_a_command_request")
            && content.contains("action.missing_query_failure_detail()")
            && content.contains("action.serialize_failure_detail()")
            && content.contains("action.serialize_failure_message(&e)")
            && content.contains("action.capture_failure_detail()")
            && content.contains("action.success_detail()")
            && content.contains("action.copied_recipe_hud(&recipe.suggested_script_name)"),
        "Turn This Into a Command should use named state details for query, success, serialize, and capture outcomes"
    );
    assert!(
        content.contains("build_current_app_command_recipe")
            && content.contains("format!(\"Automation recipe copied: {suggested_script_name}\")")
            && content.contains("format!(\"Failed to serialize current app command recipe: {error}\")")
            && content.contains("spawn_generate_script_from_recipe_after_hide")
            && content.contains("turn_this_into_command_capture_failed"),
        "Turn This Into a Command should preserve recipe copy, HUD, deferred generation, and capture failure copy"
    );
}

#[test]
fn utility_do_in_current_app_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum UtilityDoInCurrentAppBuiltinAction")
            && content.contains("UtilityDoInCurrentAppBuiltinAction::from_command(command)")
            && content.contains("fn execute_utility_do_in_current_app_builtin("),
        "Do in Current App should route through a named utility action state"
    );
    assert!(
        content.contains("action.open_palette_success_detail()")
            && content.contains("action.generate_script_success_detail()")
            && content.contains("action.capture_failure_detail()")
            && content.contains("action.capture_failure_message(&e)")
            && content.contains("format!(\"Failed to load frontmost app menu bar: {error}\")"),
        "Do in Current App branch outcomes should use named state details"
    );
    assert!(
        content.contains("effective_do_in_current_app_query_for_submission")
            && content.contains("resolve_do_in_current_app_intent")
            && content.contains("present_current_app_commands_entries")
            && content.contains("spawn_generate_script_from_current_app_with_capture")
            && content.contains("execute_builtin_inner("),
        "Do in Current App should preserve query normalization, intent routing, palette, generation, and direct execution paths"
    );
}

#[test]
fn utility_current_app_commands_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum UtilityCurrentAppCommandsBuiltinAction")
            && content.contains("UtilityCurrentAppCommandsBuiltinAction::from_command(command)")
            && content.contains("fn execute_utility_current_app_commands_builtin("),
        "Current App Commands should route through a named utility action state"
    );
    assert!(
        content.contains("action.success_detail()")
            && content.contains("action.capture_failure_detail()")
            && content.contains("action.capture_failure_message(&e)")
            && content.contains(
                "UtilityCurrentAppCommandsBuiltinAction::Open.refresh_failure_message(&error)"
            )
            && content.contains("open_current_app_commands")
            && content.contains("current_app_commands_capture_failed")
            && content.contains("format!(\"Failed to refresh current app commands: {error}\")"),
        "Current App Commands outcomes should use named state details"
    );
    assert!(
        content.contains("load_frontmost_menu_snapshot")
            && content.contains("into_entries_with_receipt")
            && content.contains("present_current_app_commands_entries(entries, &receipt, pid, \"\", cx)"),
        "Current App Commands should preserve frontmost snapshot loading and empty-filter presentation"
    );
}

#[test]
fn menu_bar_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum MenuBarBuiltinAction")
            && content.contains("MenuBarBuiltinAction::from_action(action)")
            && content.contains("fn execute_menu_bar_builtin("),
        "Menu bar built-ins should route through a named action state"
    );
    assert!(
        content.contains("action_state.success_detail()")
            && content.contains("action_state.failure_message(&e)")
            && content.contains("action_state.failure_detail()")
            && content.contains("action_state.unsupported_detail()"),
        "Menu bar built-in outcomes should use named state feedback and details"
    );
    assert!(
        content.contains("format!(\"Menu action failed: {error}\")"),
        "Menu bar state should preserve failure toast copy"
    );
    assert!(
        content.contains("script_kit_gpui::menu_executor::execute_menu_action")
            && content.contains("&action.bundle_id")
            && content.contains("&action.menu_path")
            && content.contains("self.show_unsupported_platform_toast(\"Menu bar actions\", cx)"),
        "Menu bar built-ins should preserve platform execution and unsupported-platform behavior"
    );
}

#[test]
fn system_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SystemBuiltinAction")
            && content.contains("SystemBuiltinAction::from_action(action_type)")
            && content.contains("fn execute_system_builtin("),
        "System built-ins should route through a named action state before dispatch"
    );
    assert!(
        content.contains("action.handler_name()")
            && content.contains("self.dispatch_system_action(action_type, dctx, cx)"),
        "System built-in helper should preserve structured logging and shared dispatch"
    );
    assert!(
        content.contains("SystemActionType::Restart")
            && content.contains("SystemActionType::ShutDown")
            && content.contains("SystemActionType::EmptyTrash")
            && content.contains("SystemActionType::QuitScriptKit"),
        "System action dispatch should preserve destructive action branches without executing them in tests"
    );
}

#[test]
fn surface_open_builtins_use_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SurfaceOpenBuiltinAction")
            && content.contains("SurfaceOpenBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_surface_open_builtin("),
        "Open-surface built-ins should route through named action states"
    );
    assert!(
        content.contains("SurfaceOpenBuiltinAction::ClipboardHistory")
            && content.contains("SurfaceOpenBuiltinAction::Favorites")
            && content.contains("SurfaceOpenBuiltinAction::AppLauncher")
            && content.contains("SurfaceOpenBuiltinAction::AiChat")
            && content.contains("SurfaceOpenBuiltinAction::EmojiPicker")
            && content.contains("SurfaceOpenBuiltinAction::Webcam")
            && content.contains("SurfaceOpenBuiltinAction::FileSearch")
            && content.contains("SurfaceOpenBuiltinAction::Settings")
            && content.contains("SurfaceOpenBuiltinAction::AgentChatHistory")
            && content.contains("SurfaceOpenBuiltinAction::AiVault")
            && content.contains("SurfaceOpenBuiltinAction::DictationHistory")
            && content.contains("SurfaceOpenBuiltinAction::SdkReference")
            && content.contains("SurfaceOpenBuiltinAction::ScriptTemplateCatalog"),
        "Open-surface state machine should name each covered built-in"
    );
    assert!(
        content.contains("clipboard_history::get_cached_entries(100)")
            && content.contains("AppView::FavoritesBrowseView")
            && content.contains("app_launcher::scan_applications()")
            && content.contains("open_tab_ai_agent_chat_with_entry_intent(None, cx)")
            && content.contains("crate::emoji_usage::load_frequent_snapshot")
            && content.contains("self.open_webcam(cx)")
            && content.contains("self.open_file_search(String::new(), cx)")
            && content.contains("AppView::SettingsView")
            && content.contains("AppView::AgentChatHistoryView")
            && content.contains("self.open_ai_vault_source_filter(cx)")
            && content.contains("AppView::DictationHistoryView")
            && content.contains("sdk_reference_entries_for_ui")
            && content.contains("script_template_entries_for_ui")
            && content.contains("action.success_detail()"),
        "Open-surface helper should preserve the same view constructors, resource loaders, and success details"
    );
}

#[test]
fn script_template_catalog_copy_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/script_templates.rs")
        .expect("Failed to read script template catalog renderer");

    assert!(
        content.contains("enum ScriptTemplateCatalogAction")
            && content.contains("CopyMarkdownCard"),
        "script template catalog copy feedback should be driven by a named action state"
    );
    assert!(
        content.contains("ScriptTemplateCatalogAction::CopyMarkdownCard")
            && content.contains("catalog_action.copied_hud(&template.title)"),
        "script template catalog copy HUD should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Copied {template_title} template\")"),
        "script template catalog copy feedback should preserve existing visible copy"
    );
}

#[test]
fn sdk_reference_catalog_copy_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/sdk_reference.rs")
        .expect("Failed to read SDK reference renderer");

    assert!(
        content.contains("enum SdkReferenceCatalogAction") && content.contains("CopyMarkdownCard"),
        "SDK reference copy feedback should be driven by a named action state"
    );
    assert!(
        content.contains("SdkReferenceCatalogAction::CopyMarkdownCard")
            && content.contains("catalog_action.copied_hud(&entry.name)"),
        "SDK reference Cmd+C and Enter copy HUDs should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Copied {entry_name} reference\")"),
        "SDK reference copy feedback should preserve existing visible copy"
    );
}

#[test]
fn browser_tabs_activation_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/browser_tabs.rs")
        .expect("Failed to read browser tabs renderer");

    assert!(
        content.contains("enum BrowserTabsActivationAction")
            && content.contains("ActivateSelectedTab"),
        "Browser Tabs activation feedback should be driven by a named action state"
    );
    assert!(
        content.contains("BrowserTabsActivationAction::ActivateSelectedTab")
            && content.contains("activation_action.failure_message(error)")
            && content.contains("activation_action")
            && content.contains(".generic_failure_message()"),
        "Browser Tabs Enter and double-click activation failures should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Failed to activate tab: {error}\")")
            && content.contains("\"Failed to activate tab\""),
        "Browser Tabs activation feedback should preserve existing visible copy"
    );
}

#[test]
fn terminal_utility_views_use_named_failure_states() {
    let content = fs::read_to_string("src/app_execute/utility_views.rs")
        .expect("Failed to read utility view handler");

    assert!(
        content.contains("enum TerminalOpenUtilityAction")
            && content.contains("SdkCommandTerminal")
            && content.contains("QuickTerminal"),
        "terminal utility views should distinguish SDK command terminals from Quick Terminal with named action states"
    );
    assert!(
        content.contains("terminal_action.creation_failure_log(&e)")
            && content.contains("terminal_action.open_failure_message(&e)")
            && content.contains("format!(\"Failed to create terminal: {error}\")")
            && content.contains("format!(\"Failed to create quick terminal: {error}\")")
            && content.contains("format!(\"Failed to open terminal: {error}\")"),
        "terminal utility view failure logs and user-facing toasts should be derived from named action states"
    );
}

#[test]
fn webcam_utility_view_uses_named_failure_state() {
    let content = fs::read_to_string("src/app_execute/utility_views.rs")
        .expect("Failed to read utility view handler");

    assert!(
        content.contains("enum WebcamOpenUtilityAction") && content.contains("OpenWebcamPrompt"),
        "webcam utility view should route startup feedback through a named action state"
    );
    assert!(
        content.contains("WebcamOpenUtilityAction::OpenWebcamPrompt")
            && content.contains("webcam_action.start_failure_log(&err)")
            && content.contains("format!(\"Failed to start webcam: {error}\")"),
        "webcam startup failure log copy should derive from the named action state"
    );
    assert!(
        content.contains("WebcamStartError::PermissionDenied")
            && content.contains("System Settings → Privacy & Security → Camera"),
        "webcam permission-denied startup errors should tell users where to enable camera access"
    );
}

#[test]
fn search_result_execution_helpers_use_named_failure_states() {
    let content = fs::read_to_string("src/app_execute/execution_helpers.rs")
        .expect("Failed to read execution helper handler");

    assert!(
        content.contains("enum SearchResultExecutionAction")
            && content.contains("LaunchApp")
            && content.contains("FocusWindow"),
        "search result execution helpers should name app-launch and window-focus action states"
    );
    assert!(
        content.contains("SearchResultExecutionAction::LaunchApp")
            && content.contains("SearchResultExecutionAction::FocusWindow")
            && content.contains("execution_action.failure_message(")
            && content.contains("format!(\"Failed to launch {target_name}: {error}\")")
            && content.contains("format!(\"Failed to focus window: {error}\")"),
        "search result launch and focus failures should derive visible copy from the named action state"
    );
}

#[test]
fn scratch_pad_execution_helpers_use_named_failure_states() {
    let content = fs::read_to_string("src/app_execute/execution_helpers.rs")
        .expect("Failed to read execution helper handler");

    assert!(
        content.contains("enum ScratchPadExecutionAction")
            && content.contains("CreateDirectory")
            && content.contains("CreateFile")
            && content.contains("ReadFile")
            && content.contains("SubmitSave")
            && content.contains("AutoSave"),
        "scratch pad file-operation failures should be driven by named action states"
    );
    assert!(
        content.contains("action.log_message(&e)")
            && content.contains("action.toast_message(&e)")
            && content.contains("action.log_message(&write_err)")
            && content.contains("action.toast_message(&write_err)")
            && content.contains("format!(\"Failed to create scratch pad directory: {error}\")")
            && content.contains("format!(\"Failed to create directory: {error}\")")
            && content.contains("format!(\"Failed to save scratch pad: {error}\")")
            && content.contains("format!(\"Auto-save failed: {error}\")"),
        "scratch pad logs and toasts should derive visible copy from the named action state"
    );
}

#[test]
fn claude_code_enable_helper_uses_named_failure_state() {
    let content = fs::read_to_string("src/app_execute/execution_helpers.rs")
        .expect("Failed to read execution helper handler");

    assert!(
        content.contains("enum ClaudeCodeEnableAction")
            && content.contains("EnableProvider")
            && content.contains("let enable_action = ClaudeCodeEnableAction::EnableProvider"),
        "Claude Code enable flow should be driven by a named action state"
    );
    assert!(
        content.contains("enable_action.validation_restored_message()")
            && content.contains("enable_action.validation_no_backup_message(&reason)")
            && content.contains("enable_action.validation_recovery_failed_message(&reason, &recover_err)")
            && content.contains("enable_action.write_failure_message(&e)")
            && content.contains("format!(\"Failed to enable Claude Code: {reason}. No backup available.\")")
            && content.contains("format!(\"Failed to enable Claude Code: {error}\")"),
        "Claude Code enable recovery and write failures should derive visible copy from the named action state"
    );
}

#[test]
fn menu_syntax_execution_uses_named_feedback_states() {
    let content = fs::read_to_string("src/app_execute/menu_syntax_execution.rs")
        .expect("Failed to read menu syntax execution handler");

    assert!(
        content.contains("enum MenuSyntaxCommandInvocationAction")
            && content.contains("AmbiguousCommand")
            && content.contains("MissingCommand")
            && content
                .contains("MenuSyntaxCommandInvocationAction::AmbiguousCommand.hud_message(&head)")
            && content
                .contains("MenuSyntaxCommandInvocationAction::MissingCommand.hud_message(&head)"),
        "menu syntax command invocation HUDs should be derived from named states"
    );
    assert!(
        content.contains("enum MenuSyntaxCaptureSpawnAction")
            && content.contains("DetachedHandler")
            && content.contains("MenuSyntaxCaptureSpawnAction::DetachedHandler.failure_message(&executable, e)")
            && content.contains("format!(\"Failed to spawn '{executable}': {error}\")"),
        "menu syntax capture spawn failures should be derived from the named detached-handler state"
    );
}

#[test]
fn browser_tabs_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum BrowserTabsBuiltinAction")
            && content.contains("BrowserTabsBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_browser_tabs_builtin("),
        "Browser Tabs should route through a named action state"
    );
    assert!(
        content.contains("crate::browser_tabs::list_open_tabs()")
            && content.contains("self.cached_browser_tabs = tabs")
            && content.contains("crate::favicons::domains_needing_favicons")
            && content.contains("crate::favicons::fetch_favicons_blocking(&domains)")
            && content.contains("AppView::BrowserTabsView"),
        "Browser Tabs should preserve tab loading, favicon fetch scheduling, and view opening"
    );
    assert!(
        content.contains("action.success_detail()")
            && content.contains("action.failure_detail()")
            && content.contains("open_browser_tabs")
            && content.contains("open_browser_tabs_failed"),
        "Browser Tabs success and failure details should come from the named state"
    );
}

#[test]
fn window_switcher_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum WindowSwitcherBuiltinAction")
            && content.contains("WindowSwitcherBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_window_switcher_builtin("),
        "Window Switcher should route through a named action state"
    );
    assert!(
        content.contains("window_control::list_windows()")
            && content.contains("self.cached_windows = windows")
            && content.contains("AppView::WindowSwitcherView"),
        "Window Switcher should preserve window loading, cache assignment, and view opening"
    );
    assert!(
        content.contains("action.success_detail()")
            && content.contains("action.failure_detail()")
            && content.contains("open_window_switcher")
            && content.contains("open_window_switcher_failed"),
        "Window Switcher success and failure details should come from the named state"
    );
}

#[test]
fn app_launch_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AppLaunchBuiltinAction")
            && content.contains("AppLaunchBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_app_launch_builtin("),
        "App launch should route through a named action state"
    );
    assert!(
        content.contains("app_launcher::scan_applications()")
            && content.contains("app_launcher::launch_application(app)")
            && content.contains("self.close_and_reset_window(cx)"),
        "App launch should preserve app scanning, launch dispatch, and close/reset behavior"
    );
    assert!(
        content.contains("action.success_detail(app_name)")
            && content.contains("action.not_found_detail(app_name)")
            && content.contains("action.opening_message()")
            && content.contains("action.launch_failure_message(app_name, &error)")
            && content.contains("action.not_found_message(app_name)")
            && content.contains("launch_app::{app_name}")
            && content.contains("launch_app_not_found::{app_name}"),
        "App launch copy, success detail, and missing-app details should come from the named state"
    );
    assert!(
        content.contains("format!(\"Failed to launch {app_name}: {error}\")")
            && content.contains("format!(\"App not found: {app_name}\")"),
        "App launch named state should preserve launch-failure and missing-app user-facing copy"
    );
}

#[test]
fn notes_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum NotesBuiltinAction")
            && content.contains("NotesBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_notes_builtin("),
        "Notes should route through a named action state"
    );
    assert!(
        content.contains("notes_handoff_submit_reset")
            && content.contains("crate::confirm::route_key_to_confirm_popup(\"escape\", cx)")
            && content.contains("crate::actions::close_actions_window(cx)")
            && content.contains("self.mark_filter_resync_after_actions_if_needed()")
            && content.contains("self.pending_focus = None")
            && content.contains("notes::open_notes_window_without_launcher_restore(cx)")
            && content.contains("self.close_and_reset_window(cx)"),
        "Notes should preserve submit-reset handoff, popup cleanup, and focus reset behavior"
    );
    assert!(
        content.contains("action.success_detail()")
            && content.contains("action.failure_detail()")
            && content.contains("action.opening_message()")
            && content.contains("action.failure_message(&error)")
            && content.contains("open_notes")
            && content.contains("open_notes_failed"),
        "Notes copy, success detail, and failure detail should come from the named state"
    );
    assert!(
        content.contains("\"Opening Notes window\"")
            && content.contains("format!(\"Failed to open Notes: {error}\")"),
        "Notes named state should preserve handoff log and failure copy"
    );
}

#[test]
fn sync_to_github_builtin_uses_named_action_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum SyncToGithubBuiltinAction")
            && content.contains("SyncToGithubBuiltinAction::from_feature(&entry.feature)")
            && content.contains("fn execute_sync_to_github_builtin("),
        "Sync to GitHub should route through a named action state"
    );
    assert!(
        content.contains("action.request_log_message()")
            && content.contains("action.start_hud().to_string()")
            && content.contains("action_for_task.completed_log_message()")
            && content.contains("action_for_task.failed_log_message()")
            && content.contains("action_for_task.failure_message(&error)")
            && content.contains("crate::sync::github::sync_to_github_workspace()")
            && content.contains("report.summary_message()")
            && content.contains("this.close_and_reset_window(cx)"),
        "Sync to GitHub should preserve async dispatch, HUD updates, close/reset, and error handling"
    );
    assert!(
        content.contains("action.success_detail()")
            && content.contains("sync_to_github_dispatched")
            && content.contains("\"Syncing Script Kit to GitHub...\"")
            && content.contains("format!(\"GitHub sync failed: {error}\")"),
        "Sync to GitHub dispatch detail and user-facing copy should come from the named state"
    );
}

#[test]
fn window_switcher_focus_feedback_uses_named_action_state() {
    let content = fs::read_to_string("src/render_builtins/window_switcher.rs")
        .expect("Failed to read window switcher renderer");

    assert!(
        content.contains("enum WindowSwitcherFocusAction")
            && content.contains("FocusSelectedWindow"),
        "Window Switcher selected-window focus feedback should be driven by a named action state"
    );
    assert!(
        content.contains("WindowSwitcherFocusAction::FocusSelectedWindow")
            && content.contains("focus_action.attempt_log(&window_info.title)")
            && content.contains("focus_action.success_log(&window_info.title)")
            && content.contains("focus_action.failure_message(e)"),
        "Window Switcher focus logs and error toast should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Focusing window: {window_title}\")")
            && content.contains("format!(\"Focused window: {window_title}\")")
            && content.contains("format!(\"Failed to focus window: {error}\")"),
        "Window Switcher focus feedback should preserve existing visible copy"
    );
}

#[test]
fn ai_command_window_policy_uses_named_plan_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiCommandWindowPlan")
            && content.contains("KeepMainWindowVisible")
            && content.contains("HideMainWindowDeferred")
            && content.contains("HideMainWindowForCapture"),
        "AI command window policy should be represented by named plan states"
    );
    assert!(
        content.contains("AiCommandWindowPlan::from_command(cmd_type)")
            && content.contains("fn apply_ai_command_window_plan("),
        "AI command dispatch should apply the named window plan before routing actions"
    );
    assert!(
        content.contains("AiCommandWindowPlan::KeepMainWindowVisible => {}")
            && content.contains("AiCommandWindowPlan::HideMainWindowDeferred")
            && content.contains("platform::defer_hide_main_window(cx)")
            && content.contains("AiCommandWindowPlan::HideMainWindowForCapture")
            && content.contains("hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS"),
        "AI command window plan should preserve keep-visible, deferred-hide, and capture-hide behavior"
    );
}

#[test]
fn ai_command_dispatch_uses_named_wrapper_state() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiCommandBuiltinAction")
            && content.contains("Generate(AiGenerateBuiltinAction)")
            && content.contains("Capture(AiCaptureBuiltinAction)")
            && content.contains("Unavailable(AiUnavailableBuiltinAction)"),
        "AI command dispatch should use a named wrapper state over the existing leaf actions"
    );
    assert!(
        content.contains("let ai_action = AiCommandBuiltinAction::from_command(*cmd_type)")
            && content.contains("fn execute_ai_command_builtin(")
            && content.contains("self.execute_ai_generate_builtin(generate_action")
            && content.contains("self.execute_ai_capture_builtin(capture_action")
            && content.contains("self.execute_ai_unavailable_builtin(unavailable_action"),
        "AI command wrapper state should route to each existing leaf executor"
    );
}

#[test]
fn script_context_ranking_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum RankingActionPlan")
            && content.contains("NoRankingAction")
            && content.contains("ResetSuggestedRanking"),
        "suggested ranking destructive row should be driven by named ranking plan states"
    );
    assert!(
        content.contains("RankingActionPlan::from_is_suggested(script.is_suggested)")
            && content.contains("fn reset_action")
            && content.contains("ranking_plan.reset_action()"),
        "script context ranking row should derive availability and copy from the named plan"
    );
}

#[test]
fn script_ranking_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
        .expect("Failed to read script action handler");

    assert!(
        content.contains("enum ScriptRankingHandlerAction") && content.contains("ResetRanking"),
        "reset ranking handler should be driven by a named action state"
    );
    assert!(
        content.contains("ScriptRankingHandlerAction::from_action_id(action_id)")
            && content.contains("ranking_action.reset_hud(&script_info.name)")
            && content.contains("ranking_action.no_ranking_message()")
            && content.contains("Ranking reset for \\\"{script_name}\\\"")
            && content.contains("Item has no ranking to reset"),
        "reset ranking handler should derive success and no-ranking feedback from the named state"
    );
}

#[test]
fn agent_chat_model_selection_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum AgentChatModelSelectionActionPlan")
            && content.contains("CurrentModel")
            && content.contains("AvailableModel"),
        "Agent Chat model current/switch labels and descriptions should be driven by named selection plan states"
    );
    assert!(
        content.contains("AgentChatModelSelectionActionPlan::from_is_selected(is_selected)")
            && content.contains("fn picker_title")
            && content.contains("fn description"),
        "Agent Chat model picker rows should derive visible copy from the named selection plan"
    );
}

#[test]
fn agent_chat_model_switch_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatModelSwitchHandlerAction")
            && content.contains("SwitchModel"),
        "Agent Chat model switch handler should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatModelSwitchHandlerAction::from_action_id(action_id)")
            && content.contains("model_action.unavailable_message(model_id)")
            && content.contains("model_action.already_selected_message(&model_display_name)")
            && content.contains("model_action.hud_message(&model_display_name)")
            && content.contains("model_action.switched_message(&model_display_name)")
            && content.contains("Model '{model_id}' is no longer available")
            && content.contains("Already using {display_name}")
            && content.contains("Model: {display_name}")
            && content.contains("Switched model to {display_name}"),
        "Agent Chat model switch handler should derive unavailable/current/HUD/switched feedback from named state"
    );
}

#[test]
fn agent_chat_profile_switch_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatProfileSwitchHandlerAction")
            && content.contains("SwitchProfile"),
        "Agent Chat profile switch handler should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatProfileSwitchHandlerAction::from_action_id(action_id)")
            && content.contains("profile_action.unavailable_message(profile_id)")
            && content.contains("profile_action.persist_failure_message(&profile.name, error)")
            && content.contains("profile_action.relaunch_message(&profile.name)")
            && content.contains("Profile '{profile_id}' is no longer available")
            && content.contains("Failed to persist profile '{profile_name}': {error}")
            && content.contains("Switching profile to {profile_name}"),
        "Agent Chat profile switch handler should derive unavailable, persistence, and relaunch feedback from named state"
    );
}

#[test]
fn agent_chat_root_change_actions_use_named_plan_states() {
    let content = fs::read_to_string("src/actions/builders/script_context.rs")
        .expect("Failed to read script context builder");

    assert!(
        content.contains("enum AgentChatRootPickerActionPlan")
            && content.contains("CurrentSelection")
            && content.contains("NoCurrentSelection"),
        "Agent Chat root Change Profile/Model descriptions should be driven by named picker plan states"
    );
    assert!(
        content.contains("AgentChatRootPickerActionPlan::from_selected_display_name")
            && content
                .contains("profile_picker_plan.description(AGENT_CHAT_CHANGE_PROFILE_DESCRIPTION)")
            && content.contains("model_picker_plan.description(AGENT_CHAT_CHANGE_MODEL_DESCRIPTION)"),
        "Agent Chat root Change Profile/Model actions should derive current/fallback copy from the named picker plan"
    );
}

#[test]
fn agent_chat_last_response_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatLastResponseHandlerAction")
            && content.contains("CopyToClipboard")
            && content.contains("PasteToFrontmost"),
        "Agent Chat last-response copy/paste handlers should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatLastResponseHandlerAction::from_action_id(action_id)")
            && content.contains("last_response_action.success_message()")
            && content.contains("Copied last response to clipboard")
            && content.contains("Pasting to frontmost app"),
        "Agent Chat last-response handlers should derive user-facing copy/paste feedback from the named state"
    );
}

#[test]
fn deferred_agent_chat_handoff_uses_named_failure_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum DeferredAgentChatAction")
            && content.contains("enum DeferredAgentChatActionKind")
            && content.contains("fn name(self) -> &'static str")
            && content.contains("fn failure_message(self, error: impl std::fmt::Display)")
            && content.contains("let deferred_action_kind = deferred_action.kind();")
            && content.contains("deferred_action_kind.failure_message(&error)"),
        "deferred Agent Chat handoff failure feedback should derive from the named deferred action state"
    );
    assert!(
        content.contains("Failed to open Agent Chat: {error}")
            && content.contains("Failed to send to Agent Chat: {error}")
            && content.contains("Failed to attach file to Agent Chat: {error}")
            && content.contains("Failed to apply AI preset: {error}"),
        "deferred Agent Chat handoff state should preserve action-specific failure copy"
    );
    // The legacy AI window module is deleted, so any reference to its entry
    // points now fails compilation — the compiler enforces the absence.
    for legacy_symbol in ["requires_legacy_ai_window", "ai::apply_ai_preset"] {
        assert!(
            !content.contains(legacy_symbol),
            "deferred Agent Chat handoff must not route through the legacy AI window: {legacy_symbol}"
        );
    }
}

#[test]
fn ai_image_capture_builtins_use_named_failure_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiImageCaptureBuiltinAction")
            && content.contains("SendScreen")
            && content.contains("SendFocusedWindow")
            && content.contains("SendScreenArea"),
        "AI image capture handoffs should be driven by named capture action states"
    );
    assert!(
        content.contains("AiImageCaptureBuiltinAction::SendScreen")
            && content.contains("AiImageCaptureBuiltinAction::SendFocusedWindow")
            && content.contains("AiImageCaptureBuiltinAction::SendScreenArea")
            && content.contains("capture_action.failure_message(&error)")
            && content.contains("format!(\"Failed to capture screen: {error}\")")
            && content.contains("format!(\"Failed to capture window: {error}\")")
            && content.contains("format!(\"Failed to capture screen area: {error}\")"),
        "AI image capture failure toasts should derive visible copy from the named action state"
    );
}

#[test]
fn ai_text_capture_builtins_use_named_failure_states() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("enum AiTextCaptureBuiltinAction")
            && content.contains("AgentChatContent")
            && content.contains("CurrentAppContext"),
        "AI text capture handoffs should be driven by named capture action states"
    );
    assert!(
        content.contains("AiTextCaptureBuiltinAction::AgentChatContent")
            && content
                .contains("AiTextCaptureBuiltinAction::CurrentAppContext.failure_message(&error)")
            && content.contains("capture_action.failure_message(&error)")
            && content.contains("format!(\"Failed to capture content for Agent Chat: {error}\")")
            && content.contains("format!(\"Failed to capture current app context: {error}\")"),
        "AI text capture failure toasts should derive visible copy from the named action state"
    );
}

#[test]
fn ai_open_failure_helper_preserves_actionable_copy() {
    let content = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin execution handler");

    assert!(
        content.contains("fn ai_open_failure_message(error: impl std::fmt::Display) -> String")
            && content.contains("format!(\"Failed to open AI: {error}\")"),
        "AI open failure helper should preserve actionable user-facing copy with error details"
    );
}

#[test]
fn agent_chat_conversation_session_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatConversationSessionHandlerAction")
            && content.contains("NewConversation")
            && content.contains("ClearConversation"),
        "Agent Chat new/clear conversation handlers should be driven by named session action states"
    );
    assert!(
        content.contains("AgentChatConversationSessionHandlerAction::from_action_id(action_id)")
            && content.contains("session_action.preserves_session()")
            && content.contains("thread.clear_messages(cx)")
            && content.contains("self.close_tab_ai_harness_terminal(cx)")
            && content.contains("self.open_tab_ai_agent_chat_with_entry_intent(None, cx)"),
        "Agent Chat conversation session handlers should derive keep-session vs fresh-session behavior from the named state"
    );
}

#[test]
fn agent_chat_retry_last_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatRetryLastHandlerAction")
            && content.contains("RetryLastMessage"),
        "Agent Chat retry-last handler should be driven by a named action state"
    );
    assert!(
        content.contains("AgentChatRetryLastHandlerAction::from_action_id(action_id)")
            && content.contains("retry_action.missing_user_message()")
            && content.contains("No previous message to retry"),
        "Agent Chat retry-last handler should derive missing-message copy from the named state"
    );
}

#[test]
fn agent_chat_code_copy_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatCodeCopyHandlerAction") && content.contains("CopyAllCode"),
        "Agent Chat copy-all-code handler should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatCodeCopyHandlerAction::from_action_id(action_id)")
            && content.contains("code_copy_action.result_message(false)")
            && content.contains("code_copy_action.result_message(true)")
            && content.contains("No code blocks found")
            && content.contains("All code blocks copied"),
        "Agent Chat copy-all-code handler should derive found/missing feedback from the named state"
    );
}

#[test]
fn agent_chat_conversation_markdown_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatConversationMarkdownHandlerAction")
            && content.contains("CopyToClipboard")
            && content.contains("SaveAsNote"),
        "Agent Chat conversation markdown copy/save handlers should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatConversationMarkdownHandlerAction::from_action_id(action_id)")
            && content.contains("markdown_action.empty_message()")
            && content.contains("markdown_action.success_message()")
            && content.contains(".failure_message(e)")
            && content.contains("No Agent Chat messages to copy")
            && content.contains("No Agent Chat messages to save")
            && content.contains("Copied Agent Chat conversation as markdown")
            && content.contains("Saved Agent Chat conversation to Notes")
            && content.contains("Failed to save note: {error}"),
        "Agent Chat markdown handlers should derive empty, success, and save-failure feedback from the named state"
    );
}

#[test]
fn agent_chat_last_code_block_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatLastCodeBlockHandlerAction")
            && content.contains("SaveAsScript")
            && content.contains("RunLastCode"),
        "Agent Chat save/run-last-code handlers should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatLastCodeBlockHandlerAction::from_action_id(action_id)")
            && content.contains("code_block_action.missing_code_message()")
            && content.contains("code_block_action.saved_script_message(&name, ext)")
            && content.contains("code_block_action.temp_write_failure_message(e)")
            && content.contains("code_block_action.running_message(&name)")
            && content.contains("code_block_action\n                                            .run_success_message(&stdout)")
            && content.contains("code_block_action\n                                            .run_failure_message(output.status, &out)")
            && content.contains("code_block_action\n                                    .run_spawn_failure_message(e)")
            && content.contains("No code block found in last response")
            && content.contains("No code block found")
            && content.contains("Saved as {name}.{ext}")
            && content.contains("Failed to write temp file: {error}")
            && content.contains("Running `{name}`...")
            && content.contains("Finished (no output)")
            && content.contains("Error (exit {status})")
            && content.contains("Failed to run: {error}"),
        "Agent Chat save/run-last-code handlers should derive missing-code, save-success, run status, and failure feedback from the named state"
    );
}

#[test]
fn agent_chat_panel_window_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatPanelWindowHandlerAction")
            && content.contains("ShowHistory")
            && content.contains("DetachWindow")
            && content.contains("ReattachPanel"),
        "Agent Chat history/detach/reattach handlers should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatPanelWindowHandlerAction::from_action_id(action_id)")
            && content.contains("panel_action.success_message().map(String::from)")
            && content.contains(".history_search_placeholder()")
            && content.contains("Opened conversation history")
            && content.contains("Search conversation history...")
            && content.contains("Chat kept open in window")
            && content.contains("Chat returned to panel"),
        "Agent Chat panel/window handlers should derive success messages and history placeholder from the named state"
    );
}

#[test]
fn agent_chat_history_mutation_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read action handler");

    assert!(
        content.contains("enum AgentChatHistoryMutationHandlerAction")
            && content.contains("ClearHistory"),
        "Agent Chat history mutation handlers should be driven by named action states"
    );
    assert!(
        content.contains("AgentChatHistoryMutationHandlerAction::from_action_id(action_id)")
            && content.contains("history_action.history_index_path(&kit)")
            && content.contains("history_action.conversations_dir(&kit)")
            && content.contains("history_action.success_message()")
            && content.contains("Conversation history cleared"),
        "Agent Chat clear-history should derive deletion targets and success copy from the named state"
    );
}

/// Verify SDK actions use tracing, not legacy logging.
#[test]
fn sdk_actions_uses_modern_logging() {
    let content = fs::read_to_string("src/app_actions/sdk_actions.rs")
        .expect("Failed to read sdk_actions.rs");

    assert!(
        !content.contains("logging::log("),
        "sdk_actions.rs must not use legacy logging::log — use tracing:: instead"
    );
    assert!(
        content.contains("tracing::info!(") || content.contains("tracing::warn!("),
        "sdk_actions.rs must use tracing for observability"
    );
}

#[test]
fn sdk_action_lookup_uses_named_plan_states() {
    let content = fs::read_to_string("src/app_actions/sdk_actions.rs")
        .expect("Failed to read sdk_actions.rs");

    assert!(
        content.contains("enum SdkActionLookupPlan")
            && content.contains("NoActionsDefined")
            && content.contains("ActionFound")
            && content.contains("ActionMissing"),
        "SDK action lookup should be driven by named action-list states"
    );
    assert!(
        content.contains("SdkActionLookupPlan::from_actions(self.sdk_actions.as_deref(), action_name)")
            && content.contains("fn is_found"),
        "SDK action dispatch and shortcut triggering should derive found/missing behavior from the named lookup plan"
    );
}

#[test]
fn selection_required_messages_use_named_plan_states() {
    let content =
        fs::read_to_string("src/app_actions/helpers.rs").expect("Failed to read action helpers");

    assert!(
        content.contains("enum SelectionRequiredMessagePlan")
            && content.contains("CopyPath")
            && content.contains("ConfigureShortcut")
            && content.contains("RunScriptletAction")
            && content.contains("Default"),
        "selection-required guidance should be driven by named message plan states"
    );
    assert!(
        content.contains("SelectionRequiredMessagePlan::from_action_id(action_id).message()")
            && content.contains("fn message(self) -> &'static str"),
        "selection-required guidance should derive user-facing text from the named plan"
    );
}

#[test]
fn file_search_feedback_helpers_use_named_plan_states() {
    let content =
        fs::read_to_string("src/app_actions/helpers.rs").expect("Failed to read action helpers");

    assert!(
        content.contains("enum FileSearchActionFeedbackPlan")
            && content.contains("Open")
            && content.contains("QuickLook")
            && content.contains("OpenWith")
            && content.contains("ShowInfo")
            && content.contains("Unsupported"),
        "file-search success and error feedback should be driven by named action states"
    );
    assert!(
        content.contains("FileSearchActionFeedbackPlan::from_action_id(action_id).success_hud()")
            && content
                .contains("FileSearchActionFeedbackPlan::from_action_id(action_id).error_prefix()"),
        "file-search feedback helpers should derive visible text from the named plan"
    );
}

#[test]
fn async_external_tool_feedback_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/mod.rs")
        .expect("Failed to read shared action handler");

    assert!(
        content.contains("enum AsyncExternalToolFeedbackAction")
            && content.contains("RevealInFileManager")
            && content.contains("LaunchEditor"),
        "shared async external-tool feedback should be driven by named action states"
    );
    assert!(
        content.contains("AsyncExternalToolFeedbackAction::RevealInFileManager")
            && content.contains("AsyncExternalToolFeedbackAction::LaunchEditor")
            && content.contains("feedback_action.failure_message(file_manager, error)")
            && content.contains("feedback_action.failure_message(&editor, error)"),
        "file-manager and editor failure feedback should derive from the named action state"
    );
    assert!(
        content.contains("format!(\"Failed to reveal in {tool_name}: {error}\")")
            && content.contains("format!(\"Failed to open in {tool_name}: {error}\")"),
        "external-tool failure copy should preserve the visible file-manager and editor error text"
    );
}

#[test]
fn shortcut_alias_edit_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/shortcuts.rs")
        .expect("Failed to read shortcut/alias action handler");

    assert!(
        content.contains("enum ShortcutRecorderAction")
            && content.contains("Configure")
            && content.contains("Add")
            && content.contains("Update")
            && content.contains("enum AliasInputAction"),
        "shortcut recorder and alias input handlers should be driven by named action states"
    );
    assert!(
        content.contains("ShortcutRecorderAction::from_action_id(action_id)")
            && content.contains("shortcut_action.target_error_message(")
            && content.contains("ShortcutAliasTargetError::UnsupportedItemType")
            && content.contains("ShortcutAliasTargetError::MissingCommandId")
            && content.contains("AliasInputAction::from_action_id(action_id)")
            && content.contains("alias_action.target_error_message("),
        "shortcut and alias edit handlers should derive unsupported and cannot-assign text from named states"
    );
}

#[test]
fn shortcut_alias_remove_handlers_use_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/shortcuts.rs")
        .expect("Failed to read shortcut/alias action handler");

    assert!(
        content.contains("enum ShortcutAliasRemoveAction")
            && content.contains("Shortcut")
            && content.contains("Alias"),
        "shortcut and alias remove handlers should be driven by named action states"
    );
    assert!(
        content.contains("ShortcutAliasRemoveAction::from_action_id(action_id)")
            && content.contains("remove_action.success_hud()")
            && content.contains(".failure_message(e)")
            && content.contains("cannot_remove_message()"),
        "shortcut and alias remove handlers should derive HUD and failure copy from named states"
    );
    assert!(
        content.contains("format!(\"Failed to remove shortcut: {error}\")")
            && content.contains("format!(\"Failed to remove alias: {error}\")"),
        "shortcut and alias remove failure copy should be owned by the named remove action state"
    );
}

#[test]
fn clipboard_pin_feedback_uses_handler_action_state() {
    let content = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
        .expect("Failed to read clipboard action handler");

    assert!(
        content.contains("enum ClipboardPinHandlerAction")
            && content.contains("Pin")
            && content.contains("Unpin"),
        "clipboard pin/unpin success feedback should be driven by the handler action state"
    );
    assert!(
        content.contains("fn success_hud(self) -> &'static str")
            && content.contains("Self::Pin => \"Pinned\"")
            && content.contains("Self::Unpin => \"Unpinned\"")
            && content.contains("pin_action.success_hud().to_string()"),
        "clipboard pin/unpin handler should derive visible HUD text from the named action"
    );
}

#[test]
fn app_copy_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/apps.rs")
        .expect("Failed to read app action handler");

    assert!(
        content.contains("enum AppCopyHandlerAction")
            && content.contains("Name")
            && content.contains("BundleIdentifier"),
        "application copy handlers should be driven by named copy action states"
    );
    assert!(
        content.contains("AppCopyHandlerAction::from_action_id(action_id)")
            && content.contains("copy_action.copy_value(&result)")
            && content.contains("copy_action.copied_hud(&value)")
            && content.contains("copy_action.selection_required_message()"),
        "application copy handlers should derive copied value, HUD copy, and empty-selection text from the named state"
    );
}

#[test]
fn app_open_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/apps.rs")
        .expect("Failed to read app action handler");

    assert!(
        content.contains("enum AppOpenHandlerAction")
            && content.contains("ShowInfoInFinder")
            && content.contains("ShowPackageContents"),
        "application open/show handlers should be driven by named action states"
    );
    assert!(
        content.contains("AppOpenHandlerAction::from_action_id(action_id)")
            && content.contains("open_action.success_hud()")
            && content.contains("self.error_prefix()")
            && content.contains("open_action.failure_message(e)")
            && content.contains("self.missing_target_message()")
            && content.contains("open_action.target_error_message(msg)")
            && content.contains("open_action.run(path)"),
        "application open/show handler should derive execution and feedback copy from the named state"
    );
    assert!(
        content.contains("format!(\"{}: {error}\", self.error_prefix())"),
        "application open/show handler should derive final failure toast from the named state"
    );
}

#[test]
fn app_lifecycle_handler_uses_named_action_states() {
    let content = fs::read_to_string("src/app_actions/handle_action/apps.rs")
        .expect("Failed to read app action handler");

    assert!(
        content.contains("enum AppLifecycleHandlerAction")
            && content.contains("enum AppLifecycleAppleScriptAction")
            && content.contains("struct AppLifecycleTarget")
            && content.contains("Quit")
            && content.contains("ForceQuit")
            && content.contains("Restart"),
        "application quit/force-quit/restart handlers should be driven by named action states"
    );
    assert!(
        content.contains("AppLifecycleHandlerAction::from_action_id")
            && content.contains("lifecycle_action.trace_message()")
            && content.contains("lifecycle_action.target_from_result(self.get_selected_result())")
            && content.contains("lifecycle_action.hud_message(&app_name)")
            && content.contains("lifecycle_action.async_failure_log()")
            && content.contains("lifecycle_action.restart_quit_failure_log()")
            && content.contains("lifecycle_script.osascript_failure_message(e)")
            && content.contains("self.unsupported_message()")
            && content.contains("Quitting {app_name}")
            && content.contains("Force quitting {app_name}")
            && content.contains("Restarting {app_name}")
            && content.contains("Quit is only available for applications")
            && content.contains("Force Quit is only available for applications")
            && content.contains("Restart is only available for applications"),
        "application lifecycle handlers should derive trace, HUD, unsupported, and async failure feedback from the named state"
    );
    assert!(
        content.contains("\"quit_app failed\"")
            && content.contains("\"force_quit_app failed\"")
            && content.contains("\"restart relaunch failed\"")
            && content.contains("\"quit before restart failed, attempting launch anyway\"")
            && content.contains("format!(\"Failed to run osascript: {error}\")"),
        "application lifecycle state should preserve async failure log and AppleScript subprocess copy"
    );
}

// ---------------------------------------------------------------------------
// Structural coverage tests for action handler consistency
// ---------------------------------------------------------------------------

/// Every known action ID that appears in the modular handle_action handlers
/// must have a match arm. This test collects all quoted action
/// ID strings from the dispatch match and verifies they form a known set.
#[test]
fn all_action_ids_have_handler_arms() {
    // Collect all handler source from modular handler directory
    let handler_dir = std::path::Path::new("src/app_actions/handle_action");
    let mut all_content = String::new();
    for entry in fs::read_dir(handler_dir).expect("Failed to read handle_action directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let chunk = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("Failed to read {}", path.display()));
            all_content.push_str(&chunk);
            all_content.push('\n');
        }
    }

    // These are the action IDs that must have handler arms in the dispatch.
    // Each entry represents a user-facing action the system can trigger.
    let required_action_ids = [
        // Clipboard actions
        "clipboard_pin",
        "clipboard_unpin",
        "clipboard_share",
        "clipboard_paste",
        "clipboard_attach_to_ai",
        "clipboard_copy",
        "clipboard_paste_keep_open",
        "clipboard_quick_look",
        "clipboard_open_with",
        "clipboard_annotate_cleanshot",
        "clipboard_upload_cleanshot",
        "clipboard_ocr",
        "clipboard_delete",
        "clipboard_delete_multiple",
        "clipboard_delete_all",
        "clipboard_save_file",
        "clipboard_save_snippet",
        // Script management actions
        "create_script",
        "run_script",
        "view_logs",
        "reveal_in_finder",
        "copy_path",
        "copy_deeplink",
        "copy_content",
        "copy_filename",
        "edit_script",
        "remove_script",
        "delete_script",
        "reload_scripts",
        "settings",
        "quit",
        // Shortcut / alias actions
        "configure_shortcut",
        "add_shortcut",
        "update_shortcut",
        "remove_shortcut",
        "add_alias",
        "update_alias",
        "remove_alias",
        // File search actions
        "open_file",
        "quick_look",
        "open_with",
        "show_info",
        "attach_to_ai",
        // Scriptlet actions
        "edit_scriptlet",
        "reveal_scriptlet_in_finder",
        "copy_scriptlet_path",
        "reset_ranking",
        // Control signals
        "__cancel__",
    ];

    for action_id in &required_action_ids {
        let pattern = format!("\"{}\"", action_id);
        assert!(
            all_content.contains(&pattern),
            "Action ID '{}' must have a handler arm in handle_action dispatch",
            action_id
        );
    }
}

/// No handler files should use the legacy `logging::log()` pattern.
/// All logging must use `tracing::` macros.
#[test]
fn no_legacy_logging_in_handler_files() {
    let handler_dir = std::path::Path::new("src/app_actions/handle_action");
    let files_to_check: Vec<std::path::PathBuf> = {
        let mut files = Vec::new();
        for entry in fs::read_dir(handler_dir).expect("Failed to read handle_action directory") {
            let entry = entry.expect("Failed to read directory entry");
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
        files
    };

    let mut violations = Vec::new();
    for file_path in &files_to_check {
        let content = fs::read_to_string(file_path)
            .unwrap_or_else(|_| panic!("Failed to read {}", file_path.display()));
        let count = content.matches("logging::log(").count();
        if count > 0 {
            violations.push(format!(
                "{}: {} logging::log() calls",
                file_path.display(),
                count
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "Legacy logging::log() calls found in handler files (must use tracing::):\n  {}",
        violations.join("\n  ")
    );
}

/// Every variant of `BuiltInFeature` must have a corresponding match arm
/// in `execute_builtin_with_query` in `builtin_execution.rs`.
#[test]
fn all_builtin_feature_variants_have_execution_arms() {
    let enum_source =
        fs::read_to_string("src/builtins/mod.rs").expect("Failed to read builtins/mod.rs");
    let exec_source = fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("Failed to read builtin_execution.rs");

    // Extract variant names from the enum definition.
    // We look for lines between `pub enum BuiltInFeature {` and the closing `}`.
    let enum_start = enum_source
        .find("pub enum BuiltInFeature {")
        .expect("BuiltInFeature enum not found");
    let enum_body_start = enum_source[enum_start..]
        .find('{')
        .expect("Opening brace not found")
        + enum_start
        + 1;

    // Find matching closing brace (handle nested braces)
    let mut depth = 1;
    let mut enum_body_end = enum_body_start;
    for (i, ch) in enum_source[enum_body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    enum_body_end = enum_body_start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    let enum_body = &enum_source[enum_body_start..enum_body_end];

    // Extract variant names (identifier before `(` or `,` or end-of-line)
    let mut variants = Vec::new();
    for line in enum_body.lines() {
        let trimmed = line.trim();
        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }
        // Extract the variant name (first identifier on the line)
        let variant_name: String = trimmed
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !variant_name.is_empty() {
            variants.push(variant_name);
        }
    }

    assert!(
        !variants.is_empty(),
        "Failed to extract any variants from BuiltInFeature enum"
    );

    // Verify each variant appears in execute_builtin_with_query as a match arm
    let mut missing = Vec::new();
    for variant in &variants {
        // Look for `BuiltInFeature::VariantName` in the execution source
        let pattern = format!("BuiltInFeature::{}", variant);
        if !exec_source.contains(&pattern) {
            missing.push(variant.as_str());
        }
    }

    assert!(
        missing.is_empty(),
        "BuiltInFeature variants missing from execute_builtin_with_query:\n  {}",
        missing.join(", ")
    );
}
