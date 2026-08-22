//! Pure admission policy for the existing Agent Chat entry request owner.
//!
//! `AgentChatEntryRequest` remains the production entry protocol. These values
//! describe its security-sensitive choices in a library-testable form, so a
//! disabled binary test target cannot silently turn clean-chat guarantees into
//! zero-test verification.

use serde::Serialize;
use sk_protocol::command_contract::{CommandIdentity, CommandSource};

/// The user-facing reason for entering an AI experience.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiEntryIntentKind {
    QuickQuestion,
    ContextualAnswer,
    OngoingConversation,
    ExplicitCommandHandoff,
    NoteHandoff,
    SelectedTextRewrite,
    ScriptAssistance,
    AutonomousTask,
    QuickAiPromotion,
}

/// Whether implicit source-surface data may become conversation context.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiContextAdmission {
    None,
    ExplicitOnly,
    AmbientOrFocused,
}

/// Host ownership is intentional, never inferred from whichever window wins
/// focus while a handoff is in flight.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiEntryHost {
    ExistingDetachedOrEmbedded,
    CurrentHostEmbedded,
    FreshEmbedded,
}

/// Opening or staging a composer can never implicitly submit a request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AiEntryDisposition {
    Open,
    Stage,
    Submit,
}

/// Small, pure projection of the existing production request's admission
/// choices. This is deliberately not another dispatch/request protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiEntryPolicy {
    pub intent: AiEntryIntentKind,
    pub context_admission: AiContextAdmission,
    pub host: AiEntryHost,
    pub disposition: AiEntryDisposition,
    pub composer_seed: Option<String>,
    pub selected_command: Option<CommandIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiEntryPolicyError {
    QuickQuestionMustBeEmpty,
    ExplicitCommandRequired,
    ImplicitContextForbidden,
    SubmissionRequiresText,
}

/// Content-free evidence suitable for automation and diagnostics. Neither
/// captured text nor durable command identifiers can leak through this shape.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEntryPolicyReceipt {
    pub intent: AiEntryIntentKind,
    pub context_admission: AiContextAdmission,
    pub host: AiEntryHost,
    pub disposition: AiEntryDisposition,
    pub composer_seed_present: bool,
    pub selected_command_source: Option<CommandSource>,
}

impl AiEntryPolicy {
    /// A quick question admits no implicit, ambient, focused, explicit, or
    /// profile-standing context and never sends a provider request.
    #[must_use]
    pub const fn quick_question() -> Self {
        Self {
            intent: AiEntryIntentKind::QuickQuestion,
            context_admission: AiContextAdmission::None,
            host: AiEntryHost::ExistingDetachedOrEmbedded,
            disposition: AiEntryDisposition::Open,
            composer_seed: None,
            selected_command: None,
        }
    }

    #[must_use]
    pub fn explicit_command_handoff(command: CommandIdentity) -> Self {
        Self {
            intent: AiEntryIntentKind::ExplicitCommandHandoff,
            context_admission: AiContextAdmission::ExplicitOnly,
            host: AiEntryHost::ExistingDetachedOrEmbedded,
            disposition: AiEntryDisposition::Stage,
            composer_seed: None,
            selected_command: Some(command),
        }
    }

    #[must_use]
    pub const fn note_handoff() -> Self {
        Self {
            intent: AiEntryIntentKind::NoteHandoff,
            context_admission: AiContextAdmission::ExplicitOnly,
            host: AiEntryHost::CurrentHostEmbedded,
            disposition: AiEntryDisposition::Stage,
            composer_seed: None,
            selected_command: None,
        }
    }

    #[must_use]
    pub fn quick_ai_promotion(seed: String) -> Self {
        Self {
            intent: AiEntryIntentKind::QuickAiPromotion,
            context_admission: AiContextAdmission::ExplicitOnly,
            host: AiEntryHost::FreshEmbedded,
            disposition: AiEntryDisposition::Stage,
            composer_seed: Some(seed),
            selected_command: None,
        }
    }

    pub fn validate(&self) -> Result<(), AiEntryPolicyError> {
        if self.intent == AiEntryIntentKind::QuickQuestion
            && (self.context_admission != AiContextAdmission::None
                || self.host != AiEntryHost::ExistingDetachedOrEmbedded
                || self.disposition != AiEntryDisposition::Open
                || self.composer_seed.is_some()
                || self.selected_command.is_some())
        {
            return Err(AiEntryPolicyError::QuickQuestionMustBeEmpty);
        }

        if self.intent == AiEntryIntentKind::ExplicitCommandHandoff
            && self.selected_command.is_none()
        {
            return Err(AiEntryPolicyError::ExplicitCommandRequired);
        }

        if matches!(
            self.intent,
            AiEntryIntentKind::ExplicitCommandHandoff
                | AiEntryIntentKind::NoteHandoff
                | AiEntryIntentKind::QuickAiPromotion
                | AiEntryIntentKind::SelectedTextRewrite
        ) && self.context_admission == AiContextAdmission::AmbientOrFocused
        {
            return Err(AiEntryPolicyError::ImplicitContextForbidden);
        }

        if self.disposition == AiEntryDisposition::Submit
            && self
                .composer_seed
                .as_deref()
                .is_none_or(|text| text.trim().is_empty())
        {
            return Err(AiEntryPolicyError::SubmissionRequiresText);
        }

        Ok(())
    }

    #[must_use]
    pub const fn requests_submission(&self) -> bool {
        matches!(self.disposition, AiEntryDisposition::Submit)
    }

    #[must_use]
    pub fn receipt(&self) -> AiEntryPolicyReceipt {
        AiEntryPolicyReceipt {
            intent: self.intent,
            context_admission: self.context_admission,
            host: self.host,
            disposition: self.disposition,
            composer_seed_present: self.composer_seed.is_some(),
            selected_command_source: self.selected_command.as_ref().map(CommandIdentity::source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_question_entry_suppresses_all_implicit_context() {
        let policy = AiEntryPolicy::quick_question();

        assert_eq!(policy.validate(), Ok(()));
        assert_eq!(policy.context_admission, AiContextAdmission::None);
        assert_eq!(policy.host, AiEntryHost::ExistingDetachedOrEmbedded);
        assert_eq!(policy.disposition, AiEntryDisposition::Open);
        assert!(policy.composer_seed.is_none());
        assert!(policy.selected_command.is_none());
        assert!(!policy.requests_submission());
    }

    #[test]
    fn quick_question_rejects_ambient_context_seed_selection_and_submission() {
        let variations = [
            AiEntryPolicy {
                context_admission: AiContextAdmission::AmbientOrFocused,
                ..AiEntryPolicy::quick_question()
            },
            AiEntryPolicy {
                composer_seed: Some("private text".to_string()),
                ..AiEntryPolicy::quick_question()
            },
            AiEntryPolicy {
                selected_command: Some(
                    CommandIdentity::new(CommandSource::Script, "private/script")
                        .expect("fixture identity"),
                ),
                ..AiEntryPolicy::quick_question()
            },
            AiEntryPolicy {
                disposition: AiEntryDisposition::Submit,
                ..AiEntryPolicy::quick_question()
            },
        ];

        for policy in variations {
            assert_eq!(
                policy.validate(),
                Err(AiEntryPolicyError::QuickQuestionMustBeEmpty)
            );
        }
    }

    #[test]
    fn explicit_handoffs_require_intentional_context_and_never_submit() {
        let command =
            CommandIdentity::new(CommandSource::Script, "chosen-script").expect("fixture identity");
        let mut explicit = AiEntryPolicy::explicit_command_handoff(command);
        assert_eq!(explicit.validate(), Ok(()));
        assert_eq!(explicit.context_admission, AiContextAdmission::ExplicitOnly);
        assert!(!explicit.requests_submission());

        explicit.selected_command = None;
        assert_eq!(
            explicit.validate(),
            Err(AiEntryPolicyError::ExplicitCommandRequired)
        );

        let note = AiEntryPolicy::note_handoff();
        assert_eq!(note.validate(), Ok(()));
        assert_eq!(note.host, AiEntryHost::CurrentHostEmbedded);
        assert!(!note.requests_submission());
    }

    #[test]
    fn ai_promotion_keeps_explicit_text_without_ambient_context_or_submission() {
        let mut policy = AiEntryPolicy::quick_ai_promotion("preserved answer".to_string());
        assert_eq!(policy.validate(), Ok(()));
        assert_eq!(policy.host, AiEntryHost::FreshEmbedded);
        assert!(!policy.requests_submission());

        policy.context_admission = AiContextAdmission::AmbientOrFocused;
        assert_eq!(
            policy.validate(),
            Err(AiEntryPolicyError::ImplicitContextForbidden)
        );
    }

    #[test]
    fn submission_never_starts_without_nonempty_user_text() {
        let mut policy = AiEntryPolicy {
            intent: AiEntryIntentKind::ContextualAnswer,
            context_admission: AiContextAdmission::ExplicitOnly,
            host: AiEntryHost::ExistingDetachedOrEmbedded,
            disposition: AiEntryDisposition::Submit,
            composer_seed: Some("   ".to_string()),
            selected_command: None,
        };
        assert_eq!(
            policy.validate(),
            Err(AiEntryPolicyError::SubmissionRequiresText)
        );

        policy.composer_seed = Some("intentional request".to_string());
        assert_eq!(policy.validate(), Ok(()));
        assert!(policy.requests_submission());
    }

    #[test]
    fn receipts_never_expose_private_composer_text_or_command_identifier() {
        let policy = AiEntryPolicy {
            composer_seed: Some("private-api-key-sk-secret".to_string()),
            ..AiEntryPolicy::explicit_command_handoff(
                CommandIdentity::new(CommandSource::File, "/Users/private/medical-record.txt")
                    .expect("fixture identity"),
            )
        };
        let json = serde_json::to_string(&policy.receipt()).expect("serialize safe receipt");

        assert!(json.contains("\"selectedCommandSource\":\"file\""));
        assert!(json.contains("\"composerSeedPresent\":true"));
        assert!(!json.contains("private-api-key"));
        assert!(!json.contains("medical-record"));
        assert!(!json.contains("/Users/"));
    }
}
