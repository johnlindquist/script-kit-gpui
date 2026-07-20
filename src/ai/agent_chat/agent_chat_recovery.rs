#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatWarmFailureKind {
    SidecarSpawn,
    Authentication,
    ProviderConfiguration,
    NoModels,
    Timeout,
    RuntimeClosed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatWarmFailure {
    pub kind: AgentChatWarmFailureKind,
    pub detail: String,
}

impl AgentChatWarmFailure {
    pub(crate) fn classify(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let normalized = detail.to_ascii_lowercase();
        let kind = if normalized.contains("failed to spawn")
            || normalized.contains("not found")
            || normalized.contains("permission denied")
        {
            AgentChatWarmFailureKind::SidecarSpawn
        } else if normalized.contains("auth")
            || normalized.contains("api key")
            || normalized.contains("unauthorized")
        {
            AgentChatWarmFailureKind::Authentication
        } else if normalized.contains("provider") || normalized.contains("config") {
            AgentChatWarmFailureKind::ProviderConfiguration
        } else if normalized.contains("no model") || normalized.contains("model array") {
            AgentChatWarmFailureKind::NoModels
        } else if normalized.contains("timed out") || normalized.contains("timeout") {
            AgentChatWarmFailureKind::Timeout
        } else if normalized.contains("stream closed") || normalized.contains("exited") {
            AgentChatWarmFailureKind::RuntimeClosed
        } else {
            AgentChatWarmFailureKind::Unknown
        };
        Self { kind, detail }
    }

    pub(crate) fn summary(&self) -> &'static str {
        match self.kind {
            AgentChatWarmFailureKind::SidecarSpawn => "The Pi sidecar could not start.",
            AgentChatWarmFailureKind::Authentication => "Pi needs provider authentication.",
            AgentChatWarmFailureKind::ProviderConfiguration => {
                "Pi could not load the provider configuration."
            }
            AgentChatWarmFailureKind::NoModels => "Pi did not report any available models.",
            AgentChatWarmFailureKind::Timeout => "Pi took too long to report available models.",
            AgentChatWarmFailureKind::RuntimeClosed => "Pi stopped before it became ready.",
            AgentChatWarmFailureKind::Unknown => "Pi Agent Chat could not become ready.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentChatRecoveryState {
    Failed {
        failure: AgentChatWarmFailure,
        attempts: u32,
    },
    Retrying {
        attempts: u32,
    },
    Succeeded,
    Dismissed,
}

impl AgentChatRecoveryState {
    pub(crate) fn failed(detail: impl Into<String>) -> Self {
        Self::Failed {
            failure: AgentChatWarmFailure::classify(detail),
            attempts: 0,
        }
    }

    pub(crate) fn retrying(&self) -> Self {
        let attempts = match self {
            Self::Failed { attempts, .. } | Self::Retrying { attempts } => attempts + 1,
            Self::Succeeded | Self::Dismissed => 1,
        };
        Self::Retrying { attempts }
    }

    pub(crate) fn repeated_failure(&self, detail: impl Into<String>) -> Self {
        let attempts = match self {
            Self::Retrying { attempts } | Self::Failed { attempts, .. } => *attempts,
            Self::Succeeded | Self::Dismissed => 0,
        };
        Self::Failed {
            failure: AgentChatWarmFailure::classify(detail),
            attempts,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_failures_are_typed_for_recovery_copy() {
        assert_eq!(
            AgentChatWarmFailure::classify("Pi setup required: authentication missing").kind,
            AgentChatWarmFailureKind::Authentication
        );
        assert_eq!(
            AgentChatWarmFailure::classify("model warm-up timed out").kind,
            AgentChatWarmFailureKind::Timeout
        );
        assert_eq!(
            AgentChatWarmFailure::classify("Failed to spawn Pi runtime").kind,
            AgentChatWarmFailureKind::SidecarSpawn
        );
    }

    #[test]
    fn recovery_tracks_repeated_attempts_and_terminal_states() {
        let failed = AgentChatRecoveryState::failed("provider config is invalid");
        let retrying = failed.retrying();
        assert_eq!(retrying, AgentChatRecoveryState::Retrying { attempts: 1 });
        let repeated = retrying.repeated_failure("provider config is still invalid");
        assert!(matches!(
            repeated,
            AgentChatRecoveryState::Failed { attempts: 1, .. }
        ));
        assert_ne!(
            AgentChatRecoveryState::Succeeded,
            AgentChatRecoveryState::Dismissed
        );
    }
}
