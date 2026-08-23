#[cfg(test)]
mod tests {
    use super::{
        chat_window_open_route, clear_agent_chat_history_at, AgentChatHistoryDeletionError,
        AgentChatHistoryDeletionTarget, ChatWindowOpenRoute, ConfirmedAgentChatHistoryDeletion,
    };

    fn seed_agent_chat_history(root: &std::path::Path) {
        std::fs::create_dir_all(root.join("agent_chat-conversations"))
            .expect("conversation fixture directory");
        std::fs::write(root.join("agent_chat-history.jsonl"), "private index")
            .expect("history index fixture");
        std::fs::write(
            root.join("agent_chat-conversations").join("private.json"),
            "private conversation",
        )
        .expect("conversation fixture");
        std::fs::write(
            root.join("agent_chat-prompt-history.jsonl"),
            "private submitted prompt",
        )
        .expect("prompt history fixture");
        std::fs::create_dir(root.join("agent_chat-history-attachments"))
            .expect("history attachment fixture directory");
        std::fs::write(
            root.join("agent_chat-history-attachments")
                .join("private-transcript.md"),
            "private attached full transcript",
        )
        .expect("history attachment fixture");
    }

    #[test]
    fn bare_detached_open_never_routes_to_a_coming_soon_info_state() {
        assert_eq!(
            chat_window_open_route(false),
            ChatWindowOpenRoute::SpawnRealThread
        );
        assert_eq!(
            chat_window_open_route(true),
            ChatWindowOpenRoute::ActivateExisting
        );
    }

    #[test]
    fn agent_chat_history_deletion_request_never_mutates_any_private_store() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        seed_agent_chat_history(temp.path());

        assert_eq!(
            clear_agent_chat_history_at(temp.path(), None),
            Err(AgentChatHistoryDeletionError::ConfirmationRequired),
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("agent_chat-history.jsonl"))
                .expect("index stays readable"),
            "private index",
        );
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("agent_chat-conversations")
                    .join("private.json"),
            )
            .expect("conversation stays readable"),
            "private conversation",
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("agent_chat-prompt-history.jsonl"))
                .expect("submitted prompts stay readable"),
            "private submitted prompt",
        );
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("agent_chat-history-attachments")
                    .join("private-transcript.md"),
            )
            .expect("attached transcripts stay readable"),
            "private attached full transcript",
        );
    }

    #[test]
    fn confirmed_agent_chat_history_deletion_removes_all_four_exact_owned_targets() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        seed_agent_chat_history(temp.path());
        let unrelated_file = temp.path().join("agent_chat-settings.json");
        let unrelated_directory = temp.path().join("agent_chat-conversations-backup");
        std::fs::write(&unrelated_file, "preserve me").expect("unrelated sibling fixture");
        std::fs::create_dir_all(&unrelated_directory).expect("unrelated directory fixture");
        std::fs::write(unrelated_directory.join("saved.json"), "preserve me")
            .expect("unrelated nested fixture");

        assert_eq!(
            clear_agent_chat_history_at(temp.path(), Some(ConfirmedAgentChatHistoryDeletion)),
            Ok(()),
        );
        assert!(!temp.path().join("agent_chat-history.jsonl").exists());
        assert!(!temp.path().join("agent_chat-conversations").exists());
        assert!(!temp.path().join("agent_chat-prompt-history.jsonl").exists());
        assert!(!temp.path().join("agent_chat-history-attachments").exists());
        assert_eq!(
            std::fs::read_to_string(unrelated_file).expect("unrelated file survives"),
            "preserve me",
        );
        assert_eq!(
            std::fs::read_to_string(unrelated_directory.join("saved.json"))
                .expect("unrelated nested file survives"),
            "preserve me",
        );
    }

    #[test]
    fn agent_chat_history_deletion_rejects_wrong_target_type_before_any_removal() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        seed_agent_chat_history(temp.path());
        let conversations = temp.path().join("agent_chat-conversations");
        std::fs::remove_dir_all(&conversations).expect("replace fixture directory");
        std::fs::write(&conversations, "wrong type").expect("wrong target type fixture");

        assert_eq!(
            clear_agent_chat_history_at(temp.path(), Some(ConfirmedAgentChatHistoryDeletion)),
            Err(AgentChatHistoryDeletionError::UnsafeTarget(
                AgentChatHistoryDeletionTarget::Conversations,
            )),
        );
        assert!(temp.path().join("agent_chat-history.jsonl").is_file());
        assert!(temp
            .path()
            .join("agent_chat-prompt-history.jsonl")
            .is_file());
        assert_eq!(
            std::fs::read_to_string(conversations).expect("wrong target remains untouched"),
            "wrong type",
        );
    }

    #[cfg(unix)]
    #[test]
    fn agent_chat_history_deletion_rejects_conversation_symlink_without_following_it() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir_all(&root).expect("isolated kit fixture");
        seed_agent_chat_history(&root);
        let external = temp.path().join("unrelated-private-conversations");
        std::fs::create_dir_all(&external).expect("external directory fixture");
        std::fs::write(external.join("private.json"), "never follow me")
            .expect("external private conversation fixture");

        let conversations = root.join("agent_chat-conversations");
        std::fs::remove_dir_all(&conversations).expect("replace fixture directory");
        std::os::unix::fs::symlink(&external, &conversations)
            .expect("conversation symlink fixture");

        assert_eq!(
            clear_agent_chat_history_at(&root, Some(ConfirmedAgentChatHistoryDeletion)),
            Err(AgentChatHistoryDeletionError::UnsafeTarget(
                AgentChatHistoryDeletionTarget::Conversations,
            )),
        );
        assert!(root.join("agent_chat-history.jsonl").is_file());
        assert!(root.join("agent_chat-prompt-history.jsonl").is_file());
        assert_eq!(
            std::fs::read_to_string(external.join("private.json"))
                .expect("symlink destination remains private"),
            "never follow me",
        );
    }

    #[cfg(unix)]
    #[test]
    fn agent_chat_history_deletion_preflights_prompt_symlink_before_earlier_targets() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir_all(&root).expect("isolated kit fixture");
        seed_agent_chat_history(&root);
        let external = temp.path().join("unrelated-private-prompts.jsonl");
        std::fs::write(&external, "external secrets").expect("external prompt fixture");
        let prompt_history = root.join("agent_chat-prompt-history.jsonl");
        std::fs::remove_file(&prompt_history).expect("replace prompt fixture");
        std::os::unix::fs::symlink(&external, &prompt_history).expect("prompt symlink fixture");

        assert_eq!(
            clear_agent_chat_history_at(&root, Some(ConfirmedAgentChatHistoryDeletion)),
            Err(AgentChatHistoryDeletionError::UnsafeTarget(
                AgentChatHistoryDeletionTarget::PromptHistory,
            )),
        );
        assert!(root.join("agent_chat-history.jsonl").is_file());
        assert!(root
            .join("agent_chat-conversations")
            .join("private.json")
            .is_file());
        assert_eq!(
            std::fs::read_to_string(external).expect("external prompts remain private"),
            "external secrets",
        );
    }

    #[cfg(unix)]
    #[test]
    fn agent_chat_history_deletion_preflights_attachment_symlink_before_any_removal() {
        let temp = tempfile::tempdir().expect("isolated history fixture");
        let root = temp.path().join("kit");
        std::fs::create_dir(&root).expect("isolated kit root");
        seed_agent_chat_history(&root);
        let external = temp.path().join("external-attachments");
        std::fs::create_dir(&external).expect("external directory");
        std::fs::write(external.join("secret.md"), "external private transcript")
            .expect("external transcript fixture");
        let attachments = root.join("agent_chat-history-attachments");
        std::fs::remove_dir_all(&attachments).expect("replace isolated attachment fixture");
        std::os::unix::fs::symlink(&external, &attachments).expect("attachment directory symlink");

        assert_eq!(
            clear_agent_chat_history_at(&root, Some(ConfirmedAgentChatHistoryDeletion)),
            Err(AgentChatHistoryDeletionError::UnsafeTarget(
                AgentChatHistoryDeletionTarget::HistoryAttachments,
            )),
        );
        assert!(root.join("agent_chat-history.jsonl").is_file());
        assert!(root.join("agent_chat-conversations").is_dir());
        assert!(root.join("agent_chat-prompt-history.jsonl").is_file());
        assert_eq!(
            std::fs::read_to_string(external.join("secret.md")).unwrap(),
            "external private transcript"
        );
    }

    #[test]
    fn confirmed_agent_chat_history_deletion_is_idempotent_when_targets_are_missing() {
        let temp = tempfile::tempdir().expect("isolated empty history fixture");
        for _ in 0..2 {
            assert_eq!(
                clear_agent_chat_history_at(temp.path(), Some(ConfirmedAgentChatHistoryDeletion)),
                Ok(()),
            );
        }
    }
}
