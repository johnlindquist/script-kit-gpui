#[cfg(test)]
mod builtin_execution_ai_feedback_tests {
    use super::{
        ai_capture_hide_settle_duration, ai_command_keeps_main_window_visible,
        ai_command_uses_hide_then_capture_flow, ai_open_failure_message,
        created_file_path_for_feedback, emoji_picker_label, favorites_loaded_message,
        AiCommandWindowPlan, AI_CAPTURE_HIDE_SETTLE_MS,
    };
    use crate::builtins::AiCommandType;
    use script_kit_gpui::emoji::{Emoji, EmojiCategory};
    use std::path::PathBuf;

    #[test]
    fn all_active_ai_commands_keep_main_window_visible_for_harness() {
        // All active AI commands now route to the harness terminal (a view
        // inside the main window), so they must all keep the window visible.
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::GenerateScript
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::GenerateScriptFromCurrentApp
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendScreenToAi
        ));
        assert!(ai_command_keeps_main_window_visible(&AiCommandType::OpenAi));
        assert!(ai_command_keeps_main_window_visible(&AiCommandType::MiniAi));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::NewConversation
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::ClearConversation
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendFocusedWindowToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendSelectedTextToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendBrowserTabToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendScreenAreaToAi
        ));
    }

    #[test]
    fn test_ai_open_failure_message_includes_error_details() {
        assert_eq!(
            ai_open_failure_message("window init failed"),
            "Failed to open AI: window init failed"
        );
    }

    #[test]
    fn test_favorites_loaded_message_uses_singular_for_one() {
        assert_eq!(favorites_loaded_message(1), "Loaded 1 favorite");
    }

    #[test]
    fn test_favorites_loaded_message_uses_plural_for_many() {
        assert_eq!(favorites_loaded_message(3), "Loaded 3 favorites");
    }

    #[test]
    fn no_ai_commands_use_hide_then_capture_flow_after_harness_redirect() {
        // Legacy capture flow is no longer used — all active AI commands
        // route to the harness terminal which captures context inline.
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::GenerateScriptFromCurrentApp
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendScreenToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendFocusedWindowToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendScreenAreaToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendSelectedTextToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendBrowserTabToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::MiniAi
        ));
    }

    #[test]
    fn ai_command_window_plan_names_harness_visible_paths() {
        assert_eq!(
            AiCommandWindowPlan::from_command(&AiCommandType::GenerateScript),
            AiCommandWindowPlan::KeepMainWindowVisible
        );
        assert_eq!(
            AiCommandWindowPlan::from_command(&AiCommandType::SendScreenToAi),
            AiCommandWindowPlan::KeepMainWindowVisible
        );
        assert_eq!(
            AiCommandWindowPlan::from_command(&AiCommandType::OpenAi),
            AiCommandWindowPlan::KeepMainWindowVisible
        );
        assert!(
            !AiCommandWindowPlan::from_command(&AiCommandType::SendBrowserTabToAi)
                .uses_hide_then_capture_flow()
        );
    }

    #[test]
    fn test_ai_capture_hide_settle_duration_matches_constant() {
        assert_eq!(
            ai_capture_hide_settle_duration(),
            std::time::Duration::from_millis(AI_CAPTURE_HIDE_SETTLE_MS)
        );
    }

    #[test]
    fn test_ai_capture_hide_settle_duration_waits_150ms() {
        assert_eq!(AI_CAPTURE_HIDE_SETTLE_MS, 150);
        assert_eq!(
            ai_capture_hide_settle_duration(),
            std::time::Duration::from_millis(150)
        );
    }

    #[test]
    fn test_emoji_picker_label_includes_emoji_and_name() {
        let emoji = Emoji {
            emoji: "🚀",
            name: "rocket",
            keywords: &["launch", "ship"],
            category: EmojiCategory::TravelPlaces,
        };

        assert_eq!(emoji_picker_label(&emoji), "🚀  rocket");
    }

    #[test]
    fn test_created_file_path_for_feedback_returns_same_path_when_already_absolute() {
        let absolute_path = PathBuf::from("/tmp/new-script.ts");
        let feedback_path = created_file_path_for_feedback(&absolute_path);

        assert_eq!(feedback_path, absolute_path);
    }

    #[test]
    fn test_created_file_path_for_feedback_joins_current_dir_when_relative() {
        let relative_path = PathBuf::from("new-script.ts");
        let current_dir = std::env::current_dir().expect("current dir should be available");
        let feedback_path = created_file_path_for_feedback(&relative_path);

        assert_eq!(feedback_path, current_dir.join(relative_path));
    }
}

#[cfg(test)]
mod dictation_model_prompt_tests {
    use super::*;

    /// A missing Whisper model must never offer a "download" choice — the
    /// download pipeline only fetches Parakeet. It offers switching to the
    /// recommended model instead.
    #[test]
    fn missing_whisper_model_offers_switch_to_recommended_not_download() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt_for_model(
            crate::dictation::DictationModelStatus::NotDownloaded,
            crate::dictation::DictationModelId::WhisperMedium,
            None,
        );
        assert!(
            title.contains("not installed"),
            "title must say the model is not installed, got: {title}"
        );
        assert!(
            placeholder.contains("whisper-medium-q4_1.bin"),
            "placeholder must name the expected model file, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_USE_RECOMMENDED);
        assert!(
            choices[0].name.contains("Parakeet"),
            "switch choice must name the recommended model, got: {}",
            choices[0].name
        );
        assert!(
            !choices
                .iter()
                .any(|choice| choice.value == BUILTIN_DICTATION_MODEL_DOWNLOAD),
            "missing whisper must not offer the parakeet download under a whisper label"
        );
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_CANCEL);
    }

    /// Download-retry copy must name Parakeet (the only downloadable model)
    /// even when another model is selected in preferences.
    #[test]
    fn retry_copy_names_parakeet_even_when_whisper_selected() {
        let (_, _, choices) = ScriptListApp::build_dictation_model_prompt_for_model(
            crate::dictation::DictationModelStatus::DownloadFailed("network error".to_string()),
            crate::dictation::DictationModelId::WhisperMedium,
            None,
        );
        let retry = choices
            .iter()
            .find(|choice| choice.value == BUILTIN_DICTATION_MODEL_DOWNLOAD)
            .expect("failed state must offer retry");
        assert!(
            retry
                .description
                .as_deref()
                .is_some_and(|description| description.contains("Parakeet")),
            "retry description must name Parakeet, got: {:?}",
            retry.description
        );
    }

    #[test]
    fn downloading_prompt_shows_progress_bar_with_bytes_and_speed() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Downloading {
                percentage: 35,
                downloaded_bytes: 175_000_000,
                total_bytes: 500_000_000,
                speed_bytes_per_sec: 10_485_760,
                eta_seconds: Some(31),
            },
        );
        assert!(
            title.contains("35%"),
            "title must show percentage, got: {title}"
        );
        assert!(
            placeholder.contains("166.9 MB"),
            "placeholder must show downloaded bytes, got: {placeholder}"
        );
        assert!(
            placeholder.contains("10.0 MB/s"),
            "placeholder must show speed, got: {placeholder}"
        );
        assert!(
            placeholder.contains("ETA"),
            "placeholder must show ETA, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].name, "Cancel download");
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_CANCEL);
        assert_eq!(choices[1].name, "Hide");
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn downloading_prompt_prefers_hide_after_phase_change() {
        let index = ScriptListApp::preferred_dictation_model_prompt_index(
            &crate::dictation::DictationModelStatus::Downloading {
                percentage: 0,
                downloaded_bytes: 0,
                total_bytes: crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
            },
        );

        let (_, _, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Downloading {
                percentage: 0,
                downloaded_bytes: 0,
                total_bytes: crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE,
                speed_bytes_per_sec: 0,
                eta_seconds: None,
            },
        );

        assert_eq!(choices[index].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn failed_prompt_offers_retry() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::DownloadFailed("network timeout".to_string()),
        );
        assert_eq!(title, "Dictation model download failed");
        assert_eq!(placeholder, "network timeout");
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_CANCEL);
    }

    #[test]
    fn cancelled_prompt_offers_retry_and_done() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::DownloadFailed(
                "model download cancelled".to_string(),
            ),
        );
        assert_eq!(title, "Download cancelled");
        assert!(
            placeholder.contains("Partial download kept"),
            "cancelled placeholder must mention partial file, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].name, "Retry download");
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].name, "Done");
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn not_downloaded_prompt_offers_download_and_cancel() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::NotDownloaded,
        );
        assert!(
            title.starts_with("Download "),
            "title must name selected model, got: {title}"
        );
        assert!(
            placeholder.contains("Recommended")
                || placeholder.contains("Broadest language coverage")
                || placeholder.contains("resumable if interrupted"),
            "placeholder must mention recommendation/description or resumability, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_CANCEL);
    }

    #[test]
    fn partial_archive_prompt_offers_resume_copy() {
        let partial = crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE / 4;
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_model_prompt_for_model(
            crate::dictation::DictationModelStatus::NotDownloaded,
            crate::dictation::DictationModelId::ParakeetTdt06bV3,
            Some(partial),
        );
        assert!(
            title.contains("Resume download"),
            "title must mention resume, got: {title}"
        );
        assert!(choices[0].name.contains("already downloaded"));
    }

    #[test]
    fn setup_prompt_ready_starts_dictation_first() {
        let state = crate::dictation::DictationSetupState {
            model_status: crate::dictation::DictationModelStatus::Available,
            microphone_status: crate::dictation::DictationMicrophoneStatus::Ready {
                name: "MacBook Microphone".to_string(),
                using_system_default: true,
            },
            hotkey_status: crate::dictation::DictationHotkeyStatus::Ready(
                "cmd+shift+semicolon".to_string(),
            ),
            ready: true,
        };
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_setup_prompt(
            &state,
            crate::dictation::DictationModelId::ParakeetTdt06bV3,
        );
        assert_eq!(title, "Dictation ready");
        assert_eq!(choices[0].value, BUILTIN_DICTATION_SETUP_START);
    }

    #[test]
    fn setup_prompt_denied_microphone_opens_settings() {
        let state = crate::dictation::DictationSetupState {
            model_status: crate::dictation::DictationModelStatus::Available,
            microphone_status: crate::dictation::DictationMicrophoneStatus::PermissionNeeded(
                crate::dictation::DictationMicrophonePermissionStatus::Denied,
            ),
            hotkey_status: crate::dictation::DictationHotkeyStatus::Disabled,
            ready: false,
        };
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_setup_prompt(
            &state,
            crate::dictation::DictationModelId::ParakeetTdt06bV3,
        );
        assert_eq!(title, "Finish dictation setup");
        assert!(choices
            .iter()
            .any(|choice| choice.value == BUILTIN_DICTATION_SETUP_OPEN_MIC_SETTINGS));
    }

    #[test]
    fn extracting_prompt_offers_hide() {
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Extracting,
        );
        assert_eq!(title, "Installing local dictation model\u{2026}");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn available_prompt_offers_done() {
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Available,
        );
        assert!(
            title.ends_with(" ready"),
            "title must name selected ready model, got: {title}"
        );
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_HIDE);
    }
}
