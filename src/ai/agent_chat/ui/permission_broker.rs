//! Agent Chat permission broker.
//!
//! Bridges the Agent Chat agent's permission requests to the GPUI UI thread.
//! Instead of a boolean allow/deny, the broker preserves the full set of
//! Agent Chat permission options and returns the exact user-selected `option_id`.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Context;

pub(crate) type ApprovalFn = Arc<
    dyn Fn(AgentChatApprovalRequestInput) -> anyhow::Result<Option<String>> + Send + Sync + 'static,
>;

/// Semantic category for a tool call approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AgentChatApprovalPreviewKind {
    /// Tool reads data (e.g., read file, open directory).
    Read,
    /// Tool writes or modifies data (e.g., write file, save, edit).
    Write,
    /// Tool runs a command or subprocess.
    Execute,
    /// Fallback for uncategorized tool calls.
    #[default]
    Generic,
}

impl AgentChatApprovalPreviewKind {
    /// Short label for the kind badge in the approval sheet.
    pub(crate) fn badge_label(self) -> &'static str {
        match self {
            Self::Read => "Read only",
            Self::Write => "Writes data",
            Self::Execute => "Runs command",
            Self::Generic => "Needs approval",
        }
    }
}

/// A single permission option presented to the user.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatApprovalOption {
    /// Agent Chat option ID (e.g., "allow-once", "allow-always", "deny").
    pub option_id: String,
    /// Human-readable name for the option.
    pub name: String,
    /// The kind of option (e.g., "AllowOnce", "AllowAlways", "RejectOnce").
    pub kind: String,
}

impl AgentChatApprovalOption {
    /// Canonical display label: `"Name (Kind)"`.
    ///
    /// Used in both the approval modal option list and the post-approval
    /// system message so they always match.
    pub(crate) fn summary_label(&self) -> String {
        format!("{} ({})", self.name, self.kind)
    }

    /// Whether this option represents a rejection/denial.
    pub(crate) fn is_reject(&self) -> bool {
        self.kind.starts_with("Reject")
    }

    /// Whether this option grants persistent (session-level) permission.
    pub(crate) fn is_persistent_allow(&self) -> bool {
        self.kind.contains("Always")
    }
}

/// A session-scoped permission grant recorded when the user picks a
/// persistent "Allow always" option.
///
/// Pi's runtime owns the actual approval cache; this record exists so the UI
/// can show the user which standing grants are active in the session instead
/// of letting "Allow always" decisions vanish invisibly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatStandingApproval {
    /// Human-readable tool name (e.g., "Write file", "bash").
    pub tool_title: String,
    /// Primary subject at grant time (file path, command), when known.
    pub subject: Option<String>,
    /// Semantic badge for the tool category (e.g., "Runs command").
    pub kind_badge: &'static str,
    /// The option the user chose, e.g. "Allow always (AllowAlways)".
    pub option_label: String,
}

/// Build summary labels for a slice of approval options.
pub(crate) fn summarize_approval_options(options: &[AgentChatApprovalOption]) -> Vec<String> {
    options
        .iter()
        .map(AgentChatApprovalOption::summary_label)
        .collect()
}

/// Structured preview data for an approval request.
///
/// Carries enough information for the UI to render a rich tool-call
/// preview instead of parsing a plain-text body blob.
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentChatApprovalPreview {
    /// Human-readable tool name (e.g., "Write file", "terminal/create").
    pub tool_title: String,
    /// Agent Chat tool call ID for traceability.
    pub tool_call_id: String,
    /// Primary subject of the tool call (e.g., a file path or command).
    pub subject: Option<String>,
    /// Short summary of what the tool call does.
    pub summary: Option<String>,
    /// Truncated preview of the tool call input payload.
    pub input_preview: Option<String>,
    /// Truncated preview of the tool call output payload.
    pub output_preview: Option<String>,
    /// Human-readable labels for each option (e.g., "Allow (AllowOnce)").
    pub option_summary: Vec<String>,
    /// Semantic category for the tool call.
    pub kind: AgentChatApprovalPreviewKind,
}

impl AgentChatApprovalPreview {
    /// Start building a preview with the required fields.
    pub(crate) fn new(tool_title: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Self {
            tool_title: tool_title.into(),
            tool_call_id: tool_call_id.into(),
            subject: None,
            summary: None,
            input_preview: None,
            output_preview: None,
            option_summary: Vec::new(),
            kind: AgentChatApprovalPreviewKind::Generic,
        }
    }

    /// Set the primary subject (e.g., file path or command). Blank values are ignored.
    pub(crate) fn with_subject(mut self, subject: Option<String>) -> Self {
        self.subject = subject.filter(|v| !v.trim().is_empty());
        self
    }

    /// Set a short summary of the tool call action. Blank values are ignored.
    pub(crate) fn with_summary(mut self, summary: Option<String>) -> Self {
        self.summary = summary.filter(|v| !v.trim().is_empty());
        self
    }

    /// Set a truncated preview of the tool call input payload. Blank values are ignored.
    pub(crate) fn with_input_preview(mut self, input_preview: Option<String>) -> Self {
        self.input_preview = input_preview.filter(|v| !v.trim().is_empty());
        self
    }

    /// Set a truncated preview of the tool call output payload. Blank values are ignored.
    pub(crate) fn with_output_preview(mut self, output_preview: Option<String>) -> Self {
        self.output_preview = output_preview.filter(|v| !v.trim().is_empty());
        self
    }

    /// Set option summary labels from approval options via `summarize_approval_options`.
    pub(crate) fn with_options(mut self, options: &[AgentChatApprovalOption]) -> Self {
        self.option_summary = summarize_approval_options(options);
        self
    }

    /// Set the semantic kind explicitly.
    pub(crate) fn with_kind(mut self, kind: AgentChatApprovalPreviewKind) -> Self {
        self.kind = kind;
        self
    }

    /// Build a deterministic plain-text body from the structured preview.
    ///
    /// Lets callers summarize once, then reuse the same data for both
    /// rich overlay rendering and any plain-text fallback surface.
    /// Blank sections are omitted.
    pub(crate) fn fallback_body(&self) -> String {
        let mut parts = Vec::with_capacity(7);
        parts.push(format!("Tool: {}", self.tool_title));
        parts.push(format!("Tool call ID: {}", self.tool_call_id));
        if let Some(subject) = self.subject.as_deref() {
            parts.push(format!("Subject: {subject}"));
        }
        if let Some(summary) = self.summary.as_deref() {
            parts.push(summary.to_string());
        }
        if let Some(input_preview) = self.input_preview.as_deref() {
            parts.push(format!("Input:\n{input_preview}"));
        }
        if let Some(output_preview) = self.output_preview.as_deref() {
            parts.push(format!("Output:\n{output_preview}"));
        }
        if !self.option_summary.is_empty() {
            parts.push(format!("Options: {}", self.option_summary.join(", ")));
        }
        parts.join("\n\n")
    }

    /// Infer the semantic kind from the tool title.
    pub(crate) fn infer_kind(mut self) -> Self {
        let lowered = self.tool_title.to_ascii_lowercase();
        self.kind =
            if lowered.contains("write") || lowered.contains("save") || lowered.contains("edit") {
                AgentChatApprovalPreviewKind::Write
            } else if lowered.contains("terminal")
                || lowered.contains("command")
                || lowered.contains("exec")
            {
                AgentChatApprovalPreviewKind::Execute
            } else if lowered.contains("read") || lowered.contains("open") {
                AgentChatApprovalPreviewKind::Read
            } else {
                AgentChatApprovalPreviewKind::Generic
            };
        self
    }
}

/// Input for constructing an approval request (before assigning an ID).
#[derive(Debug, Clone)]
pub(crate) struct AgentChatApprovalRequestInput {
    /// Title for the approval dialog.
    pub title: String,
    /// Body text describing the action requiring approval.
    pub body: String,
    /// Structured preview for rich UI rendering. `None` falls back to body text.
    pub preview: Option<AgentChatApprovalPreview>,
    /// Available permission options from the Agent Chat agent.
    pub options: Vec<AgentChatApprovalOption>,
}

/// Build an approval request input from a structured preview without forcing
/// each caller to separately rebuild a parallel plain-text body.
pub(crate) fn approval_request_input(
    title: impl Into<String>,
    preview: AgentChatApprovalPreview,
    options: Vec<AgentChatApprovalOption>,
) -> AgentChatApprovalRequestInput {
    let body = preview.fallback_body();
    AgentChatApprovalRequestInput {
        title: title.into(),
        body,
        preview: Some(preview),
        options,
    }
}

/// A fully-formed approval request ready for the UI.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatApprovalRequest {
    /// Unique request identifier.
    pub id: u64,
    /// Title for the approval dialog.
    pub title: String,
    /// Body text describing the action requiring approval.
    pub body: String,
    /// Structured preview for rich UI rendering. `None` falls back to body text.
    pub preview: Option<AgentChatApprovalPreview>,
    /// Available permission options from the Agent Chat agent.
    pub options: Vec<AgentChatApprovalOption>,
    /// Channel to send the user's selected option ID (or `None` for cancel).
    pub reply_tx: async_channel::Sender<Option<String>>,
}

/// Broker that manages permission request flow between the Agent Chat worker
/// thread and the GPUI UI thread.
///
/// The broker lives on the Agent Chat worker thread and sends requests to the
/// UI via a bounded channel. The UI sends back the selected option ID
/// (or `None` for cancellation) through a per-request reply channel.
#[derive(Clone)]
pub(crate) struct AgentChatPermissionBroker {
    tx: async_channel::Sender<AgentChatApprovalRequest>,
    next_id: Arc<AtomicU64>,
}

impl AgentChatPermissionBroker {
    /// Create a new broker and its corresponding receiver.
    ///
    /// The receiver should be consumed by the UI thread to present
    /// approval dialogs.
    pub(crate) fn new() -> (Self, async_channel::Receiver<AgentChatApprovalRequest>) {
        let (tx, rx) = async_channel::bounded(32);
        (
            Self {
                tx,
                next_id: Arc::new(AtomicU64::new(1)),
            },
            rx,
        )
    }

    /// Nonblocking producer used by controlled sources; the real permission
    /// listener and approval handler still own presentation and completion.
    pub(crate) fn try_request(
        &self,
        input: AgentChatApprovalRequestInput,
    ) -> anyhow::Result<async_channel::Receiver<Option<String>>> {
        let (reply_tx, reply_rx) = async_channel::bounded(1);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.tx
            .try_send(AgentChatApprovalRequest {
                id,
                title: input.title,
                body: input.body,
                preview: input.preview,
                options: input.options,
                reply_tx,
            })
            .map_err(|_| anyhow::anyhow!("approval_queue_full_or_closed"))?;
        Ok(reply_rx)
    }

    /// Submit a permission request and block until the UI responds.
    ///
    /// Returns `Some(option_id)` if the user selected an option, or
    /// `None` if they cancelled.
    pub(crate) fn request(
        &self,
        input: AgentChatApprovalRequestInput,
    ) -> anyhow::Result<Option<String>> {
        let (reply_tx, reply_rx) = async_channel::bounded(1);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        self.tx
            .send_blocking(AgentChatApprovalRequest {
                id,
                title: input.title,
                body: input.body,
                preview: input.preview,
                options: input.options,
                reply_tx,
            })
            .context("failed to send Agent Chat approval request to UI")?;

        reply_rx
            .recv_blocking()
            .context("Agent Chat approval reply channel closed")
    }

    /// Create an `ApprovalFn` that routes through this broker.
    ///
    /// This is the bridge between the old `ApprovalFn` signature and
    /// the new broker-based flow. The returned function captures the
    /// broker and forwards all permission options.
    pub(crate) fn approval_fn(&self) -> ApprovalFn {
        let broker = self.clone();
        Arc::new(move |input: AgentChatApprovalRequestInput| broker.request(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_assigns_sequential_ids() {
        let (broker, _rx) = AgentChatPermissionBroker::new();
        assert_eq!(broker.next_id.load(Ordering::SeqCst), 1);
        // Simulate two increments
        broker.next_id.fetch_add(1, Ordering::SeqCst);
        broker.next_id.fetch_add(1, Ordering::SeqCst);
        assert_eq!(broker.next_id.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn broker_request_completes_when_reply_sent() {
        let (broker, rx) = AgentChatPermissionBroker::new();

        // Spawn a thread to answer the request
        let handle = std::thread::spawn(move || {
            let request = rx.recv_blocking().expect("should receive request");
            assert_eq!(request.id, 1);
            assert_eq!(request.options.len(), 2);
            request
                .reply_tx
                .send_blocking(Some("allow-once".to_string()))
                .expect("reply should send");
        });

        let result = broker
            .request(AgentChatApprovalRequestInput {
                title: "Test".to_string(),
                body: "Test body".to_string(),
                preview: None,
                options: vec![
                    AgentChatApprovalOption {
                        option_id: "allow-once".to_string(),
                        name: "Allow once".to_string(),
                        kind: "AllowOnce".to_string(),
                    },
                    AgentChatApprovalOption {
                        option_id: "deny".to_string(),
                        name: "Deny".to_string(),
                        kind: "RejectOnce".to_string(),
                    },
                ],
            })
            .expect("request should succeed");

        assert_eq!(result, Some("allow-once".to_string()));
        handle.join().expect("responder thread should finish");
    }

    #[test]
    fn broker_request_returns_none_on_cancel() {
        let (broker, rx) = AgentChatPermissionBroker::new();

        let handle = std::thread::spawn(move || {
            let request = rx.recv_blocking().expect("should receive request");
            request
                .reply_tx
                .send_blocking(None)
                .expect("reply should send");
        });

        let result = broker
            .request(AgentChatApprovalRequestInput {
                title: "Test".to_string(),
                body: "Cancel test".to_string(),
                preview: None,
                options: vec![AgentChatApprovalOption {
                    option_id: "allow".to_string(),
                    name: "Allow".to_string(),
                    kind: "AllowOnce".to_string(),
                }],
            })
            .expect("request should succeed");

        assert_eq!(result, None);
        handle.join().expect("responder thread should finish");
    }

    #[test]
    fn summary_label_formats_name_and_kind() {
        let option = AgentChatApprovalOption {
            option_id: "allow".to_string(),
            name: "Allow".to_string(),
            kind: "AllowOnce".to_string(),
        };
        assert_eq!(option.summary_label(), "Allow (AllowOnce)");
    }

    #[test]
    fn summarize_approval_options_maps_all() {
        let options = vec![
            AgentChatApprovalOption {
                option_id: "allow".to_string(),
                name: "Allow".to_string(),
                kind: "AllowOnce".to_string(),
            },
            AgentChatApprovalOption {
                option_id: "deny".to_string(),
                name: "Deny".to_string(),
                kind: "RejectOnce".to_string(),
            },
        ];
        assert_eq!(
            summarize_approval_options(&options),
            vec!["Allow (AllowOnce)", "Deny (RejectOnce)"],
        );
    }

    #[test]
    fn preview_builder_sets_all_fields() {
        let options = vec![
            AgentChatApprovalOption {
                option_id: "allow".to_string(),
                name: "Allow".to_string(),
                kind: "AllowOnce".to_string(),
            },
            AgentChatApprovalOption {
                option_id: "deny".to_string(),
                name: "Deny".to_string(),
                kind: "RejectOnce".to_string(),
            },
        ];
        let preview = AgentChatApprovalPreview::new("terminal/create", "client-terminal-create")
            .with_subject(Some("bun run dev".to_string()))
            .with_summary(Some(
                "Spawn a subprocess owned by the Agent Chat client".to_string(),
            ))
            .with_input_preview(Some("{ \"command\": \"bun\" }".to_string()))
            .with_output_preview(Some("ok".to_string()))
            .with_options(&options);

        assert_eq!(preview.tool_title, "terminal/create");
        assert_eq!(preview.tool_call_id, "client-terminal-create");
        assert_eq!(preview.subject.as_deref(), Some("bun run dev"));
        assert_eq!(
            preview.summary.as_deref(),
            Some("Spawn a subprocess owned by the Agent Chat client"),
        );
        assert_eq!(
            preview.input_preview.as_deref(),
            Some("{ \"command\": \"bun\" }"),
        );
        assert_eq!(preview.output_preview.as_deref(), Some("ok"));
        assert_eq!(
            preview.option_summary,
            vec!["Allow (AllowOnce)", "Deny (RejectOnce)"],
        );
    }

    #[test]
    fn fallback_body_populated_preview() {
        let options = vec![
            AgentChatApprovalOption {
                option_id: "allow".to_string(),
                name: "Allow".to_string(),
                kind: "AllowOnce".to_string(),
            },
            AgentChatApprovalOption {
                option_id: "deny".to_string(),
                name: "Deny".to_string(),
                kind: "RejectOnce".to_string(),
            },
        ];
        let preview = AgentChatApprovalPreview::new("write_text_file", "client-fs-write")
            .with_subject(Some("/tmp/demo.txt".to_string()))
            .with_summary(Some("Write 24 bytes".to_string()))
            .with_input_preview(Some("hello world content".to_string()))
            .with_output_preview(Some("ok".to_string()))
            .with_options(&options);

        let body = preview.fallback_body();
        assert!(body.contains("Tool: write_text_file"));
        assert!(body.contains("Tool call ID: client-fs-write"));
        assert!(body.contains("Subject: /tmp/demo.txt"));
        assert!(body.contains("Write 24 bytes"));
        assert!(body.contains("Input:\nhello world content"));
        assert!(body.contains("Output:\nok"));
        assert!(body.contains("Options: Allow (AllowOnce), Deny (RejectOnce)"));
    }

    #[test]
    fn fallback_body_omits_blank_sections() {
        let preview = AgentChatApprovalPreview::new("read_file", "id-123")
            .with_subject(None)
            .with_summary(None)
            .with_input_preview(Some("  ".to_string())) // blank, filtered by builder
            .with_output_preview(None);

        let body = preview.fallback_body();
        assert!(body.contains("Tool: read_file"));
        assert!(body.contains("Tool call ID: id-123"));
        assert!(!body.contains("Subject:"));
        assert!(!body.contains("Input:"));
        assert!(!body.contains("Output:"));
        assert!(!body.contains("Options:"));
    }

    #[test]
    fn fallback_body_option_summary_formatting() {
        let options = vec![
            AgentChatApprovalOption {
                option_id: "a".to_string(),
                name: "Allow".to_string(),
                kind: "AllowOnce".to_string(),
            },
            AgentChatApprovalOption {
                option_id: "b".to_string(),
                name: "Allow always".to_string(),
                kind: "AllowAlways".to_string(),
            },
            AgentChatApprovalOption {
                option_id: "c".to_string(),
                name: "Deny".to_string(),
                kind: "RejectOnce".to_string(),
            },
        ];
        let preview =
            AgentChatApprovalPreview::new("terminal/create", "tc-1").with_options(&options);
        let body = preview.fallback_body();
        assert!(body
            .contains("Options: Allow (AllowOnce), Allow always (AllowAlways), Deny (RejectOnce)"));
    }

    #[test]
    fn approval_request_input_uses_fallback_body() {
        let options = vec![AgentChatApprovalOption {
            option_id: "allow".to_string(),
            name: "Allow".to_string(),
            kind: "AllowOnce".to_string(),
        }];
        let preview = AgentChatApprovalPreview::new("write_text_file", "w-1")
            .with_subject(Some("/tmp/out.txt".to_string()))
            .with_summary(Some("Write 128 bytes".to_string()))
            .with_options(&options);

        let expected_body = preview.fallback_body();
        let input = approval_request_input("Agent Chat file write request", preview, options);

        assert_eq!(input.title, "Agent Chat file write request");
        assert_eq!(input.body, expected_body);
        assert!(input.preview.is_some());
        assert_eq!(input.options.len(), 1);
    }

    #[test]
    fn preview_builder_filters_blank_strings() {
        let preview = AgentChatApprovalPreview::new("test", "id")
            .with_subject(Some("  ".to_string()))
            .with_summary(Some(String::new()))
            .with_input_preview(None);

        assert!(preview.subject.is_none());
        assert!(preview.summary.is_none());
        assert!(preview.input_preview.is_none());
    }
}
