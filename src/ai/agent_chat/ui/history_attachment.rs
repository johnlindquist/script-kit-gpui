//! Agent Chat history attachment artifacts.
//!
//! Writes a deterministic markdown file under `~/.scriptkit/agent_chat-history-attachments/`
//! that can be attached to a new Agent Chat chat via the existing `AiContextPart::FilePath` path.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Whether to attach a short summary or the full transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatHistoryAttachMode {
    Summary,
    Transcript,
}

impl AgentChatHistoryAttachMode {
    pub(crate) fn file_stem(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Transcript => "transcript",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::Transcript => "Transcript",
        }
    }
}

fn attachments_dir_at(kit_root: &Path) -> PathBuf {
    kit_root.join("agent_chat-history-attachments")
}

fn ensure_private_attachments_dir(kit_root: &Path) -> Result<PathBuf> {
    let dir = attachments_dir_at(kit_root);
    if !super::history::inspect_conversation_directory(&dir)? {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder
            .create(&dir)
            .with_context(|| format!("create private attachment directory {}", dir.display()))?;
    }
    if !super::history::inspect_conversation_directory(&dir)? {
        bail!("Agent Chat history attachment directory is unsafe");
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY);
    }
    let directory = options
        .open(&dir)
        .with_context(|| format!("open private attachment directory {}", dir.display()))?;
    let metadata = directory
        .metadata()
        .context("inspect attachment directory")?;
    if !metadata.is_dir() {
        bail!("Agent Chat history attachment target is not a directory");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o700 {
            directory
                .set_permissions(std::fs::Permissions::from_mode(0o700))
                .context("repair private attachment directory permissions")?;
        }
    }

    Ok(dir)
}

/// Validate every exact attachment before the conversation delete mutates
/// either its saved transcript or index. Missing directories are legitimate;
/// a symlink, foreign type, or hostile session identifier fails closed.
pub(super) fn existing_history_attachment_paths_at(
    kit_root: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>> {
    super::history::validate_agent_chat_session_id(session_id)?;
    let dir = attachments_dir_at(kit_root);
    if !super::history::inspect_conversation_directory(&dir)? {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for mode in [
        AgentChatHistoryAttachMode::Summary,
        AgentChatHistoryAttachMode::Transcript,
    ] {
        let path = dir.join(format!("{session_id}-{}.md", mode.file_stem()));
        if super::history::inspect_regular_history_file(&path)? {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn one_line(value: &str, max_chars: usize) -> String {
    let collapsed: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = collapsed.chars().take(max_chars).collect();
    if collapsed.chars().count() > max_chars {
        out.push('\u{2026}');
    }
    out
}

/// Format a conversation as a markdown attachment.
pub(crate) fn format_history_attachment_markdown(
    conversation: &super::history::SavedConversation,
    mode: AgentChatHistoryAttachMode,
) -> String {
    let entry = super::history::build_history_entry(conversation);
    let title = entry
        .as_ref()
        .map(|e| e.title_display().to_string())
        .unwrap_or_else(|| "Conversation".to_string());

    let mut out = String::new();
    out.push_str("# Agent Chat Conversation\n\n");
    out.push_str(&format!("- session_id: {}\n", conversation.session_id));
    out.push_str(&format!("- timestamp: {}\n", conversation.timestamp));
    out.push_str(&format!("- mode: {}\n\n", mode.label()));
    out.push_str(&format!("## Title\n{}\n\n", title));

    match mode {
        AgentChatHistoryAttachMode::Summary => {
            out.push_str("## Summary\n");
            for msg in conversation.messages.iter().take(6) {
                out.push_str(&format!(
                    "- **{}**: {}\n",
                    msg.role,
                    one_line(&msg.body, 220)
                ));
            }
            out.push('\n');
        }
        AgentChatHistoryAttachMode::Transcript => {
            out.push_str("## Transcript\n\n");
            for msg in &conversation.messages {
                out.push_str(&format!("### {}\n{}\n\n", msg.role, msg.body));
            }
        }
    }

    out
}

/// Write a markdown attachment to disk and return (path, label).
pub(crate) fn write_history_attachment(
    session_id: &str,
    mode: AgentChatHistoryAttachMode,
) -> Result<(PathBuf, String)> {
    let conversation = super::history::load_conversation(session_id)
        .with_context(|| format!("missing Agent Chat conversation {session_id}"))?;
    write_history_attachment_at(
        &crate::setup::get_kit_path(),
        session_id,
        &conversation,
        mode,
    )
}

fn write_history_attachment_at(
    kit_root: &Path,
    session_id: &str,
    conversation: &super::history::SavedConversation,
    mode: AgentChatHistoryAttachMode,
) -> Result<(PathBuf, String)> {
    super::history::validate_agent_chat_session_id(session_id)?;
    if conversation.session_id != session_id {
        bail!("Agent Chat history attachment conversation identity does not match");
    }
    let dir = ensure_private_attachments_dir(kit_root)?;

    let path = dir.join(format!("{session_id}-{}.md", mode.file_stem()));
    let markdown = format_history_attachment_markdown(conversation, mode);
    super::history::write_private_history_file_atomically(&path, &markdown)
        .with_context(|| format!("write private attachment {}", path.display()))?;

    let title = super::history::build_history_entry(conversation)
        .map(|e| e.title_display().to_string())
        .unwrap_or_else(|| session_id.to_string());
    let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
    let safe_title = crate::logging::log_private_user_value(&title);

    tracing::info!(
        target: "script_kit::tab_ai",
        event = "agent_chat_history_attachment_written",
        session_id = %session_id,
        mode = ?mode,
        path_bytes = safe_path.raw_bytes,
        path_sha256 = %safe_path.sha256,
        title_bytes = safe_title.raw_bytes,
        title_sha256 = %safe_title.sha256,
    );

    Ok((
        path,
        format!("History \u{00b7} {} \u{00b7} {}", mode.label(), title),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::ui::history::{SavedConversation, SavedMessage};

    fn test_conversation() -> SavedConversation {
        SavedConversation {
            session_id: "test-attach-1".to_string(),
            timestamp: "2026-04-05T12:00:00Z".to_string(),
            custom_title: None,
            messages: vec![
                SavedMessage {
                    role: "user".to_string(),
                    body: "help me fix login".to_string(),
                },
                SavedMessage {
                    role: "assistant".to_string(),
                    body: "The root cause is an expired OAuth redirect URI".to_string(),
                },
            ],
        }
    }

    #[test]
    fn summary_format_includes_title_and_messages() {
        let md = format_history_attachment_markdown(
            &test_conversation(),
            AgentChatHistoryAttachMode::Summary,
        );
        assert!(md.contains("# Agent Chat Conversation"));
        assert!(md.contains("help me fix login"));
        assert!(md.contains("mode: Summary"));
        assert!(md.contains("## Summary"));
    }

    #[test]
    fn transcript_format_includes_full_messages() {
        let md = format_history_attachment_markdown(
            &test_conversation(),
            AgentChatHistoryAttachMode::Transcript,
        );
        assert!(md.contains("## Transcript"));
        assert!(md.contains("### user"));
        assert!(md.contains("### assistant"));
        assert!(md.contains("expired OAuth redirect URI"));
    }

    #[test]
    fn one_line_truncates() {
        assert_eq!(one_line("hello world", 5), "hello\u{2026}");
        assert_eq!(one_line("hi", 10), "hi");
    }

    #[test]
    fn attach_mode_labels() {
        assert_eq!(AgentChatHistoryAttachMode::Summary.label(), "Summary");
        assert_eq!(AgentChatHistoryAttachMode::Transcript.label(), "Transcript");
        assert_eq!(AgentChatHistoryAttachMode::Summary.file_stem(), "summary");
        assert_eq!(
            AgentChatHistoryAttachMode::Transcript.file_stem(),
            "transcript"
        );
    }

    #[test]
    fn private_history_attachments_use_owner_only_atomic_files_and_directories() {
        let root = tempfile::tempdir().expect("isolated attachment root");
        let conversation = test_conversation();
        let (summary, _) = write_history_attachment_at(
            root.path(),
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Summary,
        )
        .expect("private summary attachment");
        let (transcript, _) = write_history_attachment_at(
            root.path(),
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .expect("private transcript attachment");
        assert_ne!(summary, transcript);
        assert!(std::fs::read_to_string(&transcript)
            .expect("private transcript contents")
            .contains("expired OAuth redirect URI"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            for file in [&summary, &transcript] {
                assert_eq!(
                    std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
            assert_eq!(
                std::fs::metadata(attachments_dir_at(root.path()))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
    }

    #[test]
    fn history_attachment_rejects_hostile_and_foreign_session_before_filesystem_changes() {
        let root = tempfile::tempdir().expect("isolated attachment root");
        let conversation = test_conversation();
        for session_id in ["", "..", "../escape", "a/b", "C:\\private"] {
            assert!(write_history_attachment_at(
                root.path(),
                session_id,
                &conversation,
                AgentChatHistoryAttachMode::Transcript,
            )
            .is_err());
        }
        assert!(write_history_attachment_at(
            root.path(),
            "foreign-session",
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .is_err());
        assert!(!attachments_dir_at(root.path()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn history_attachment_repairs_existing_directory_permissions_before_private_write() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("isolated attachment root");
        let dir = attachments_dir_at(root.path());
        std::fs::create_dir(&dir).expect("legacy attachment directory");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755))
            .expect("legacy readable directory mode");
        let conversation = test_conversation();

        write_history_attachment_at(
            root.path(),
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .expect("legacy directory becomes private before writing");

        assert_eq!(
            std::fs::metadata(dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[cfg(unix)]
    #[test]
    fn history_attachment_refuses_symlinked_directory_and_file_targets() {
        let root = tempfile::tempdir().expect("isolated attachment root");
        let kit = root.path().join("kit");
        let external = root.path().join("external");
        std::fs::create_dir(&kit).expect("kit root");
        std::fs::create_dir(&external).expect("external directory");
        let external_file = external.join("private.md");
        std::fs::write(&external_file, "preserve unrelated secret").unwrap();
        let conversation = test_conversation();
        let dir = attachments_dir_at(&kit);
        std::os::unix::fs::symlink(&external, &dir).expect("directory symlink");

        assert!(write_history_attachment_at(
            &kit,
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&external_file).unwrap(),
            "preserve unrelated secret"
        );

        std::fs::remove_file(&dir).expect("remove isolated symlink");
        std::fs::create_dir(&dir).expect("real attachment directory");
        let hostile = dir.join(format!("{}-transcript.md", conversation.session_id));
        std::os::unix::fs::symlink(&external_file, &hostile).expect("file symlink");
        assert!(write_history_attachment_at(
            &kit,
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&external_file).unwrap(),
            "preserve unrelated secret"
        );
    }

    #[test]
    fn history_attachment_atomic_replacement_preserves_the_complete_latest_transcript() {
        let root = tempfile::tempdir().expect("isolated attachment root");
        let mut conversation = test_conversation();
        let (first, _) = write_history_attachment_at(
            root.path(),
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .expect("first attachment");
        conversation.messages[1].body = "new complete private answer".into();
        let (second, _) = write_history_attachment_at(
            root.path(),
            &conversation.session_id,
            &conversation,
            AgentChatHistoryAttachMode::Transcript,
        )
        .expect("atomic replacement");
        assert_eq!(first, second);
        let contents = std::fs::read_to_string(second).unwrap();
        assert!(contents.contains("new complete private answer"));
        assert!(!contents.contains("expired OAuth redirect URI"));
    }
}
