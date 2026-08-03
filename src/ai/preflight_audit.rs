use super::message_parts::{
    ContextResolutionFailure, PreparedMessageDecision, PreparedMessageReceipt,
};
use super::model::ChatId;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const AI_PREFLIGHT_AUDIT_SCHEMA_VERSION: u32 = 3;
pub const AI_PREFLIGHT_AUDIT_MAX_BYTES: u64 = 5 * 1024 * 1024;
const AI_PREFLIGHT_AUDIT_COMPACT_KEEP: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionableContextFailure {
    pub part_id: String,
    pub source_kind: super::message_parts::ContextSourceKind,
    pub role: super::message_parts::ContextPreparationRole,
    pub code: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiPreflightAudit {
    pub schema_version: u32,
    pub correlation_id: String,
    #[serde(default)]
    pub preflight_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_fingerprint: Option<String>,
    pub chat_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub decision: PreparedMessageDecision,
    #[serde(default)]
    pub raw_content_chars: usize,
    #[serde(default)]
    pub authored_content_chars: usize,
    pub has_pending_image: bool,
    pub has_context_parts: bool,
    pub receipt: PreparedMessageReceipt,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actionable_failures: Vec<ActionableContextFailure>,
    pub created_at: String,
}

impl AiPreflightAudit {
    pub fn new(
        chat_id: &ChatId,
        raw_content: &str,
        authored_content: &str,
        has_pending_image: bool,
        has_context_parts: bool,
        receipt: PreparedMessageReceipt,
    ) -> Self {
        let created_at = Utc::now();
        let correlation_id = format!(
            "preflight-{}-{}",
            chat_id.as_str(),
            created_at.timestamp_micros()
        );

        let actionable_failures = receipt
            .outcomes
            .iter()
            .filter(|outcome| {
                outcome.kind == super::message_parts::ContextPartPreparationOutcomeKind::Failed
            })
            .map(|outcome| {
                actionable_context_fields(&outcome.part_id, outcome.source_kind, outcome.role)
            })
            .collect();

        Self {
            schema_version: AI_PREFLIGHT_AUDIT_SCHEMA_VERSION,
            correlation_id,
            preflight_generation: 0,
            draft_fingerprint: Some(stable_draft_fingerprint(raw_content, authored_content)),
            chat_id: chat_id.as_str(),
            message_id: None,
            decision: receipt.decision.clone(),
            raw_content_chars: raw_content.chars().count(),
            authored_content_chars: authored_content.chars().count(),
            has_pending_image,
            has_context_parts,
            receipt,
            actionable_failures,
            created_at: created_at.to_rfc3339(),
        }
    }

    pub fn attach_message_id(&mut self, message_id: &str) {
        self.message_id = Some(message_id.to_string());
    }
}

fn stable_draft_fingerprint(raw_content: &str, authored_content: &str) -> String {
    format!(
        "raw:{}:authored:{}",
        raw_content.len(),
        authored_content.len()
    )
}

pub fn default_preflight_audit_log_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".scriptkit")
        .join("logs")
        .join("ai-preflight-audits.jsonl")
}

pub fn append_preflight_audit(
    path: Option<&Path>,
    audit: &AiPreflightAudit,
) -> anyhow::Result<PathBuf> {
    if audit.schema_version != AI_PREFLIGHT_AUDIT_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported preflight audit schema version {}",
            audit.schema_version
        );
    }

    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(default_preflight_audit_log_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    compact_preflight_audits_if_needed(&path)?;

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&path)?;
    ensure_jsonl_append_boundary(&mut file)?;
    writeln!(file, "{}", serde_json::to_string(audit)?)?;
    Ok(path)
}

fn ensure_jsonl_append_boundary(file: &mut fs::File) -> anyhow::Result<()> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    file.seek(SeekFrom::End(0))?;
    if last[0] != b'\n' {
        writeln!(file)?;
    }
    Ok(())
}

pub fn read_preflight_audits(path: Option<&Path>) -> anyhow::Result<Vec<AiPreflightAudit>> {
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(default_preflight_audit_log_path);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = OpenOptions::new().read(true).open(&path)?;
    let reader = BufReader::new(file);
    let mut seen = BTreeSet::new();
    let mut audits = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let audit: AiPreflightAudit = match serde_json::from_str(&line) {
            Ok(audit) => audit,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::ai_preflight",
                    %error,
                    "Skipping malformed preflight audit log entry"
                );
                continue;
            }
        };
        if audit.schema_version != AI_PREFLIGHT_AUDIT_SCHEMA_VERSION {
            tracing::warn!(
                target: "script_kit::ai_preflight",
                schema_version = audit.schema_version,
                "Skipping unsupported preflight audit schema version"
            );
            continue;
        }
        if seen.insert(audit.correlation_id.clone()) {
            audits.push(audit);
        }
    }

    Ok(audits)
}

pub fn compact_preflight_audits_if_needed(path: &Path) -> anyhow::Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() <= AI_PREFLIGHT_AUDIT_MAX_BYTES {
        return Ok(());
    }

    let audits = read_preflight_audits(Some(path))?;
    let start = audits.len().saturating_sub(AI_PREFLIGHT_AUDIT_COMPACT_KEEP);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for audit in audits.into_iter().skip(start) {
        writeln!(file, "{}", serde_json::to_string(&audit)?)?;
    }
    Ok(())
}

pub fn actionable_context_failure(failure: &ContextResolutionFailure) -> ActionableContextFailure {
    actionable_context_fields(&failure.part_id, failure.source_kind, failure.role)
}

fn actionable_context_fields(
    part_id: &str,
    source_kind: super::message_parts::ContextSourceKind,
    role: super::message_parts::ContextPreparationRole,
) -> ActionableContextFailure {
    let (code, message, remediation) = match source_kind {
        super::message_parts::ContextSourceKind::Resource => (
            "context_resource_unavailable",
            "A desktop context resource could not be prepared.",
            "Refocus the target app and retry, or remove this context item.",
        ),
        super::message_parts::ContextSourceKind::File
        | super::message_parts::ContextSourceKind::Skill => (
            "attachment_unavailable",
            "An attachment could not be prepared.",
            "Verify the source still exists and retry, or remove this attachment.",
        ),
        super::message_parts::ContextSourceKind::FocusedTarget => (
            "focused_context_unavailable",
            "The focused item could not be prepared.",
            "Select the item again and retry, or remove this context item.",
        ),
        super::message_parts::ContextSourceKind::Ambient
        | super::message_parts::ContextSourceKind::Text => (
            "context_unavailable",
            "A context item could not be prepared.",
            "Retry or remove this context item before sending.",
        ),
    };
    ActionableContextFailure {
        part_id: part_id.to_string(),
        source_kind,
        role,
        code: code.to_string(),
        message: message.to_string(),
        remediation: remediation.to_string(),
    }
}

pub fn build_actionable_preflight_error(audit: &AiPreflightAudit) -> Option<String> {
    if audit.actionable_failures.is_empty() {
        return None;
    }

    Some(
        audit
            .actionable_failures
            .iter()
            .map(|failure| format!("{} {}", failure.message, failure.remediation))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn log_preflight_audit(audit: &AiPreflightAudit, stage: &str) {
    tracing::info!(
        target: "script_kit::ai_preflight",
        event = "ai_preflight_audit",
        stage = stage,
        correlation_id = %audit.correlation_id,
        chat_id = %audit.chat_id,
        message_id = ?audit.message_id,
        decision = ?audit.decision,
        attempted = audit.receipt.context.attempted,
        resolved = audit.receipt.context.resolved,
        failure_count = audit.receipt.context.failed,
        has_pending_image = audit.has_pending_image,
        has_context_parts = audit.has_context_parts,
        final_user_content_len = audit.receipt.final_content_chars,
        "ai_preflight_audit"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::message_parts::{
        ContextPreparationRole, ContextPreparationSummary, ContextResolutionFailure,
        ContextSourceKind, PreparedMessageDecision, PreparedMessageReceipt,
    };

    fn failed_context(part_id: &str, source_kind: ContextSourceKind) -> ContextResolutionFailure {
        ContextResolutionFailure {
            part_id: part_id.to_string(),
            source_kind,
            role: ContextPreparationRole::Primary,
            failure: crate::ai::reliability::context_unavailable_failure("TEST_ERROR_CANARY"),
        }
    }

    fn receipt_with_failure() -> PreparedMessageReceipt {
        PreparedMessageReceipt {
            schema_version: crate::ai::message_parts::AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
            decision: PreparedMessageDecision::Blocked,
            authored_content_chars: 19,
            final_content_chars: 19,
            context: ContextPreparationSummary {
                attempted: 1,
                resolved: 0,
                failed: 1,
                primary_failed: 1,
                supplemental_failed: 0,
            },
            assembly: None,
            outcomes: Vec::new(),
            user_error: Some(
                "This context could not be prepared. Retry or remove it before sending."
                    .to_string(),
            ),
        }
    }

    #[test]
    fn test_build_actionable_preflight_error_for_browser_failure() {
        let failure = failed_context("context-0000", ContextSourceKind::Resource);
        let receipt = receipt_with_failure();

        let audit = AiPreflightAudit {
            schema_version: AI_PREFLIGHT_AUDIT_SCHEMA_VERSION,
            correlation_id: "corr-1".to_string(),
            preflight_generation: 1,
            draft_fingerprint: Some("raw:19:authored:19".to_string()),
            chat_id: "chat-1".to_string(),
            message_id: None,
            decision: PreparedMessageDecision::Blocked,
            raw_content_chars: 19,
            authored_content_chars: 19,
            has_pending_image: false,
            has_context_parts: true,
            actionable_failures: vec![actionable_context_failure(&failure)],
            receipt,
            created_at: "2026-03-21T18:32:13Z".to_string(),
        };

        let error = build_actionable_preflight_error(&audit).expect("expected actionable error");
        assert!(
            error.contains("Refocus the target app and retry"),
            "Expected remediation guidance in error, got: {error}"
        );
        assert!(
            error.contains("desktop context resource could not be prepared"),
            "Expected user-facing message in error, got: {error}"
        );
        assert!(!error.contains("TEST_ERROR_CANARY"));
    }

    #[test]
    fn test_actionable_failure_codes_for_safe_source_kinds() {
        let cases = vec![
            (ContextSourceKind::Resource, "context_resource_unavailable"),
            (ContextSourceKind::File, "attachment_unavailable"),
            (ContextSourceKind::Skill, "attachment_unavailable"),
            (
                ContextSourceKind::FocusedTarget,
                "focused_context_unavailable",
            ),
            (ContextSourceKind::Text, "context_unavailable"),
        ];

        for (source_kind, expected_code) in cases {
            let failure = failed_context("context-0000", source_kind);
            let actionable = actionable_context_failure(&failure);
            assert_eq!(actionable.code, expected_code);
            let serialized = serde_json::to_string(&actionable).expect("serialize safe failure");
            assert!(!serialized.contains("TEST_ERROR_CANARY"));
        }
    }

    #[test]
    fn test_no_actionable_error_when_no_failures() {
        let receipt = PreparedMessageReceipt {
            schema_version: crate::ai::message_parts::AI_MESSAGE_PREPARATION_SCHEMA_VERSION,
            decision: PreparedMessageDecision::Ready,
            authored_content_chars: 5,
            final_content_chars: 5,
            context: ContextPreparationSummary::default(),
            assembly: None,
            outcomes: Vec::new(),
            user_error: None,
        };

        let audit = AiPreflightAudit {
            schema_version: AI_PREFLIGHT_AUDIT_SCHEMA_VERSION,
            correlation_id: "corr-2".to_string(),
            preflight_generation: 1,
            draft_fingerprint: Some("raw:5:authored:5".to_string()),
            chat_id: "chat-2".to_string(),
            message_id: None,
            decision: PreparedMessageDecision::Ready,
            raw_content_chars: 5,
            authored_content_chars: 5,
            has_pending_image: false,
            has_context_parts: false,
            actionable_failures: Vec::new(),
            receipt,
            created_at: "2026-03-21T18:32:13Z".to_string(),
        };

        assert!(build_actionable_preflight_error(&audit).is_none());
    }

    #[test]
    fn test_serde_roundtrip_camel_case() {
        let failure = ActionableContextFailure {
            part_id: "context-0000".to_string(),
            source_kind: ContextSourceKind::Resource,
            role: ContextPreparationRole::Primary,
            code: "context_resource_unavailable".to_string(),
            message: "A context resource could not be prepared.".to_string(),
            remediation: "Retry or remove it.".to_string(),
        };

        let json = serde_json::to_string(&failure).expect("serialize");
        assert!(json.contains("\"partId\""), "fields should be camelCase");
        assert!(!json.contains("kit://"));

        let deserialized: ActionableContextFailure =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, failure);
    }
}
