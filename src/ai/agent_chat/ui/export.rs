//! Shared Agent Chat conversation markdown serializer.
//!
//! Used by both the shared action handler (`agent_chat_export_markdown`,
//! `agent_chat_save_as_note`) and the detached Agent Chat Chat window export path.

use super::conversation_export::{AgentChatConversationExport, AgentChatExportPurpose};
use super::thread::{AgentChatThread, AgentChatThreadMessage, AgentChatThreadMessageRole};

const AGENT_CHAT_CONVERSATION_HEADING: &str = "# Agent Chat Conversation\n\n";

pub(crate) fn persist_private_agent_chat_export(
    directory: &std::path::Path,
    session_id: &str,
    markdown: &str,
) -> std::io::Result<std::path::PathBuf> {
    let safe_session_id = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if safe_session_id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Agent Chat export requires a session identifier",
        ));
    }

    crate::atomic_file::write_private_unique_named_file(
        directory,
        &format!("agent-chat-export-{safe_session_id}"),
        "md",
        markdown.as_bytes(),
    )
}

fn role_label(role: &AgentChatThreadMessageRole) -> &'static str {
    match role {
        AgentChatThreadMessageRole::User => "**You**",
        AgentChatThreadMessageRole::Assistant => "**Assistant**",
        AgentChatThreadMessageRole::Thought => "**Thinking**",
        AgentChatThreadMessageRole::Tool => "**Tool**",
        AgentChatThreadMessageRole::System => "**System**",
        AgentChatThreadMessageRole::Error => "**Error**",
    }
}

/// Build a markdown document from Agent Chat thread messages. Returns `None` if no
/// messages have non-empty renderable body text.
pub(crate) fn build_agent_chat_conversation_markdown(
    messages: &[AgentChatThreadMessage],
) -> Option<String> {
    let mut md = String::from(AGENT_CHAT_CONVERSATION_HEADING);
    let mut wrote_any = false;
    for msg in messages {
        let body = msg.body.trim();
        if body.is_empty() {
            continue;
        }
        md.push_str(role_label(&msg.role));
        md.push_str("\n\n");
        md.push_str(body);
        md.push_str("\n\n---\n\n");
        wrote_any = true;
    }
    wrote_any.then_some(md)
}

pub(crate) fn build_agent_chat_conversation_markdown_from_thread(
    thread: &AgentChatThread,
) -> Option<String> {
    let export = thread.export_conversation(AgentChatExportPurpose::CopyTranscript);
    build_agent_chat_conversation_markdown_from_export(&export)
}

pub(crate) fn build_agent_chat_conversation_markdown_from_export(
    export: &AgentChatConversationExport,
) -> Option<String> {
    let mut md = String::from(AGENT_CHAT_CONVERSATION_HEADING);
    let mut wrote_any = false;
    for msg in &export.messages {
        let body = msg.body.trim();
        if body.is_empty() {
            continue;
        }
        md.push_str(match msg.role.as_str() {
            "user" => "**You**",
            "assistant" => "**Assistant**",
            "thought" => "**Thinking**",
            "tool" => "**Tool**",
            "system" => "**System**",
            "error" => "**Error**",
            _ => "**Message**",
        });
        md.push_str("\n\n");
        md.push_str(body);
        md.push_str("\n\n---\n\n");
        wrote_any = true;
    }
    wrote_any.then_some(md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::SharedString;

    fn message(id: u64, role: AgentChatThreadMessageRole, body: &str) -> AgentChatThreadMessage {
        AgentChatThreadMessage {
            id,
            role,
            body: SharedString::from(body.to_string()),
            tool_call_id: None,
            tool_meta: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn agent_chat_markdown_export_labels_roles_and_preserves_fences() {
        let markdown = build_agent_chat_conversation_markdown(&[
            message(1, AgentChatThreadMessageRole::User, "show rust"),
            message(
                2,
                AgentChatThreadMessageRole::Assistant,
                "```rust\nfn main() {}\n```",
            ),
            message(3, AgentChatThreadMessageRole::System, "saved"),
        ])
        .expect("markdown");

        assert!(markdown.starts_with("# Agent Chat Conversation"));
        assert!(markdown.contains("**You**\n\nshow rust"));
        assert!(markdown.contains("**Assistant**\n\n```rust\nfn main() {}\n```"));
        assert!(markdown.contains("**System**\n\nsaved"));
    }

    #[cfg(unix)]
    #[test]
    fn agent_chat_export_integrity_creates_owner_only_private_markdown() {
        use std::os::unix::fs::PermissionsExt as _;

        let isolated = tempfile::tempdir().expect("isolated Agent Chat export root");
        let directory = isolated.path().join("private-exports");
        let path = persist_private_agent_chat_export(
            &directory,
            "private-session",
            "# private user and assistant conversation\n",
        )
        .expect("persist private conversation export");

        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "# private user and assistant conversation\n"
        );
    }

    #[test]
    fn agent_chat_export_integrity_preserves_each_repeated_conversation_export() {
        let isolated = tempfile::tempdir().expect("isolated Agent Chat export root");
        let first = persist_private_agent_chat_export(isolated.path(), "same-session", "first")
            .expect("persist first conversation export");
        let second = persist_private_agent_chat_export(isolated.path(), "same-session", "second")
            .expect("preserve earlier conversation export");

        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(first).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "second");
    }

    #[cfg(unix)]
    #[test]
    fn agent_chat_export_integrity_refuses_hostile_directories_and_destination_links() {
        use std::os::unix::fs::symlink;

        let isolated = tempfile::tempdir().expect("isolated hostile Agent Chat export root");
        let external_directory = isolated.path().join("another-owner");
        std::fs::create_dir(&external_directory).expect("seed unrelated export directory");
        let linked_directory = isolated.path().join("hostile-exports");
        symlink(&external_directory, &linked_directory).expect("plant hostile export directory");
        assert!(persist_private_agent_chat_export(
            &linked_directory,
            "private-session",
            "never expose private conversation",
        )
        .is_err());

        let directory = isolated.path().join("safe-exports");
        crate::atomic_file::ensure_private_directory(&directory)
            .expect("create private export directory");
        let foreign = isolated.path().join("foreign-private-document");
        std::fs::write(&foreign, "preserve another owner's private document")
            .expect("seed unrelated private document");
        symlink(
            &foreign,
            directory.join("agent-chat-export-private-session.md"),
        )
        .expect("plant hostile export destination");
        assert!(persist_private_agent_chat_export(
            &directory,
            "private-session",
            "never overwrite another owner",
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(&foreign).unwrap(),
            "preserve another owner's private document"
        );
    }

    #[test]
    fn agent_chat_export_integrity_contains_untrusted_session_identifiers() {
        let isolated = tempfile::tempdir().expect("isolated hostile session export root");
        let path = persist_private_agent_chat_export(
            isolated.path(),
            "../../foreign-owner\\session",
            "private conversation remains in the selected directory",
        )
        .expect("sanitize untrusted export session identity");

        assert_eq!(path.parent(), Some(isolated.path()));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("agent-chat-export-"));
    }
}
