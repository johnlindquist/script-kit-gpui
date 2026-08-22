//! Typed execution transitions shared by scripts, flows, actions, and AI.
//!
//! This is a projection contract: source-specific process groups, AI reducers,
//! and Flow registries retain ownership of their richer operational semantics.

use crate::command_contract::{CommandAvailability, CommandIdentity};
use serde::{Deserialize, Serialize};

pub const EXECUTION_RECEIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionPhase {
    Ready,
    Preparing,
    Running,
    Streaming,
    /// Cancellation has been requested, but owned work may still be alive.
    ///
    /// This deliberately remains nonterminal until an identity-matched
    /// cleanup observation confirms that the entire process group is gone.
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

impl ExecutionPhase {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionEvent {
    Prepare,
    Start,
    Stream,
    Complete,
    Fail,
    /// Request cancellation. This never claims that work has stopped.
    Cancel,
}

/// The owning runner's explicit observation of cleanup for one exact run.
///
/// A process-group id is optional for in-process commands, but resources must
/// be released in either case. A group-backed command can only settle once
/// its owning runner has observed that group as no longer alive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionCleanupProof {
    pub run_id: String,
    pub command_id: CommandIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_group_id: Option<u32>,
    pub process_group_alive: bool,
    pub resources_released: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionLifecycle {
    pub run_id: String,
    pub command_id: CommandIdentity,
    pub phase: ExecutionPhase,
    pub event_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    owned_process_group_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verified_cleanup: Option<ExecutionCleanupProof>,
}

impl ExecutionLifecycle {
    pub fn new(
        run_id: impl Into<String>,
        command_id: CommandIdentity,
    ) -> Result<Self, ExecutionError> {
        let run_id = run_id.into();
        if run_id.trim().is_empty() {
            return Err(ExecutionError::MissingRunIdentity);
        }
        Ok(Self {
            run_id,
            command_id,
            phase: ExecutionPhase::Ready,
            event_count: 0,
            owned_process_group_id: None,
            verified_cleanup: None,
        })
    }

    /// Bind the actual spawned process group once, before cancellation begins.
    /// An observed cleanup cannot subsequently substitute a different group.
    pub fn bind_process_group(&mut self, process_group_id: u32) -> Result<(), ExecutionError> {
        if !matches!(
            self.phase,
            ExecutionPhase::Running | ExecutionPhase::Streaming
        ) {
            return Err(ExecutionError::InvalidTransition);
        }
        if process_group_id == 0 {
            return Err(ExecutionError::ProcessGroupIdentityMismatch);
        }
        match self.owned_process_group_id {
            Some(existing) if existing != process_group_id => {
                Err(ExecutionError::ProcessGroupIdentityMismatch)
            }
            Some(_) => Ok(()),
            None => {
                self.owned_process_group_id = Some(process_group_id);
                Ok(())
            }
        }
    }

    pub fn apply(&mut self, event: ExecutionEvent) -> Result<ExecutionPhase, ExecutionError> {
        if self.phase.is_terminal() {
            return Err(ExecutionError::AlreadyTerminal);
        }
        let next = match (self.phase, event) {
            (ExecutionPhase::Ready, ExecutionEvent::Prepare) => ExecutionPhase::Preparing,
            (ExecutionPhase::Preparing, ExecutionEvent::Start) => ExecutionPhase::Running,
            (ExecutionPhase::Running | ExecutionPhase::Streaming, ExecutionEvent::Stream) => {
                ExecutionPhase::Streaming
            }
            (ExecutionPhase::Running | ExecutionPhase::Streaming, ExecutionEvent::Complete) => {
                ExecutionPhase::Completed
            }
            (
                ExecutionPhase::Preparing | ExecutionPhase::Running | ExecutionPhase::Streaming,
                ExecutionEvent::Fail,
            ) => ExecutionPhase::Failed,
            (
                ExecutionPhase::Preparing | ExecutionPhase::Running | ExecutionPhase::Streaming,
                ExecutionEvent::Cancel,
            ) => ExecutionPhase::Cancelling,
            _ => return Err(ExecutionError::InvalidTransition),
        };
        self.phase = next;
        self.event_count = self.event_count.saturating_add(1);
        Ok(next)
    }

    pub fn preflight(&self, availability: &CommandAvailability) -> Result<(), ExecutionError> {
        if self.phase != ExecutionPhase::Ready {
            return Err(ExecutionError::InvalidTransition);
        }
        if !availability.is_executable() {
            return Err(ExecutionError::Unavailable);
        }
        Ok(())
    }

    /// Settle a user cancellation only after the exact owned work is gone.
    ///
    /// Neither SIGTERM dispatch nor another run's cleanup is sufficient.
    pub fn confirm_cancellation(
        &mut self,
        cleanup: ExecutionCleanupProof,
    ) -> Result<ExecutionPhase, ExecutionError> {
        if self.phase != ExecutionPhase::Cancelling {
            return Err(ExecutionError::InvalidTransition);
        }
        if cleanup.run_id != self.run_id || cleanup.command_id != self.command_id {
            return Err(ExecutionError::CleanupIdentityMismatch);
        }
        if cleanup.process_group_id != self.owned_process_group_id {
            return Err(ExecutionError::ProcessGroupIdentityMismatch);
        }
        if cleanup.process_group_alive || !cleanup.resources_released {
            return Err(ExecutionError::CleanupNotVerified);
        }
        self.verified_cleanup = Some(cleanup);
        self.phase = ExecutionPhase::Cancelled;
        self.event_count = self.event_count.saturating_add(1);
        Ok(self.phase)
    }

    pub fn receipt(
        &self,
        elapsed_ms: u64,
        diagnostic_fingerprint: Option<String>,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        if !self.phase.is_terminal() {
            return Err(ExecutionError::MissingTerminalOutcome);
        }
        let cleanup_verified = self.verified_cleanup.is_some();
        if self.phase == ExecutionPhase::Cancelled && !cleanup_verified {
            return Err(ExecutionError::CleanupNotVerified);
        }
        Ok(ExecutionReceipt {
            schema_version: EXECUTION_RECEIPT_SCHEMA_VERSION,
            run_id: self.run_id.clone(),
            command_id: self.command_id.clone(),
            outcome: self.phase,
            elapsed_ms,
            diagnostic_fingerprint,
            cleanup_verified,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReceipt {
    pub schema_version: u32,
    pub run_id: String,
    pub command_id: CommandIdentity,
    pub outcome: ExecutionPhase,
    pub elapsed_ms: u64,
    /// Opaque vault reference only; no argument, transcript, stderr, or secret.
    pub diagnostic_fingerprint: Option<String>,
    pub cleanup_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionError {
    MissingRunIdentity,
    Unavailable,
    InvalidTransition,
    AlreadyTerminal,
    MissingTerminalOutcome,
    CleanupNotVerified,
    CleanupIdentityMismatch,
    ProcessGroupIdentityMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_contract::CommandSource;

    fn lifecycle() -> ExecutionLifecycle {
        ExecutionLifecycle::new(
            "run-1",
            CommandIdentity::new(CommandSource::Script, "main:hello").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn successful_run_has_exactly_one_terminal_receipt() {
        let mut run = lifecycle();
        run.preflight(&CommandAvailability::Ready).unwrap();
        assert_eq!(
            run.apply(ExecutionEvent::Prepare),
            Ok(ExecutionPhase::Preparing)
        );
        assert_eq!(
            run.apply(ExecutionEvent::Start),
            Ok(ExecutionPhase::Running)
        );
        assert_eq!(
            run.apply(ExecutionEvent::Stream),
            Ok(ExecutionPhase::Streaming)
        );
        assert_eq!(
            run.apply(ExecutionEvent::Complete),
            Ok(ExecutionPhase::Completed)
        );
        assert_eq!(
            run.apply(ExecutionEvent::Fail),
            Err(ExecutionError::AlreadyTerminal)
        );
        let receipt = run.receipt(15, None).unwrap();
        assert_eq!(receipt.outcome, ExecutionPhase::Completed);
        assert_eq!(receipt.command_id.as_str(), "script/main:hello");
    }

    #[test]
    fn user_cancellation_is_never_failure_and_requires_verified_cleanup() {
        let mut run = lifecycle();
        run.apply(ExecutionEvent::Prepare).unwrap();
        run.apply(ExecutionEvent::Start).unwrap();
        run.bind_process_group(42).unwrap();
        assert_eq!(
            run.apply(ExecutionEvent::Cancel),
            Ok(ExecutionPhase::Cancelling)
        );
        assert!(!run.phase.is_terminal());
        assert_eq!(
            run.receipt(8, None),
            Err(ExecutionError::MissingTerminalOutcome)
        );

        let mut cleanup = ExecutionCleanupProof {
            run_id: run.run_id.clone(),
            command_id: run.command_id.clone(),
            process_group_id: Some(42),
            process_group_alive: true,
            resources_released: false,
        };
        assert_eq!(
            run.confirm_cancellation(cleanup.clone()),
            Err(ExecutionError::CleanupNotVerified),
        );
        assert_eq!(run.phase, ExecutionPhase::Cancelling);

        cleanup.process_group_alive = false;
        cleanup.resources_released = true;
        assert_eq!(
            run.confirm_cancellation(cleanup),
            Ok(ExecutionPhase::Cancelled),
        );
        let receipt = run.receipt(8, None).unwrap();
        assert_eq!(receipt.outcome, ExecutionPhase::Cancelled);
        assert!(receipt.cleanup_verified);
    }

    #[test]
    fn another_runs_cleanup_can_never_settle_a_cancellation() {
        let mut run = lifecycle();
        run.apply(ExecutionEvent::Prepare).unwrap();
        run.apply(ExecutionEvent::Start).unwrap();
        run.apply(ExecutionEvent::Cancel).unwrap();

        let cleanup = ExecutionCleanupProof {
            run_id: "other-run".to_owned(),
            command_id: run.command_id.clone(),
            process_group_id: Some(42),
            process_group_alive: false,
            resources_released: true,
        };
        assert_eq!(
            run.confirm_cancellation(cleanup),
            Err(ExecutionError::CleanupIdentityMismatch),
        );
        assert_eq!(run.phase, ExecutionPhase::Cancelling);
    }

    #[test]
    fn another_process_groups_cleanup_can_never_settle_the_owned_run() {
        let mut run = lifecycle();
        run.apply(ExecutionEvent::Prepare).unwrap();
        run.apply(ExecutionEvent::Start).unwrap();
        run.bind_process_group(42).unwrap();
        assert_eq!(
            run.bind_process_group(7),
            Err(ExecutionError::ProcessGroupIdentityMismatch)
        );
        run.apply(ExecutionEvent::Cancel).unwrap();

        let mut cleanup = ExecutionCleanupProof {
            run_id: run.run_id.clone(),
            command_id: run.command_id.clone(),
            process_group_id: Some(7),
            process_group_alive: false,
            resources_released: true,
        };
        assert_eq!(
            run.confirm_cancellation(cleanup.clone()),
            Err(ExecutionError::ProcessGroupIdentityMismatch)
        );
        assert_eq!(run.phase, ExecutionPhase::Cancelling);

        cleanup.process_group_id = Some(42);
        assert_eq!(
            run.confirm_cancellation(cleanup),
            Ok(ExecutionPhase::Cancelled)
        );
    }

    #[test]
    fn in_process_cancellation_cannot_fabricate_a_process_group() {
        let mut run = lifecycle();
        run.apply(ExecutionEvent::Prepare).unwrap();
        run.apply(ExecutionEvent::Start).unwrap();
        run.apply(ExecutionEvent::Cancel).unwrap();

        let mut cleanup = ExecutionCleanupProof {
            run_id: run.run_id.clone(),
            command_id: run.command_id.clone(),
            process_group_id: Some(42),
            process_group_alive: false,
            resources_released: true,
        };
        assert_eq!(
            run.confirm_cancellation(cleanup.clone()),
            Err(ExecutionError::ProcessGroupIdentityMismatch)
        );

        cleanup.process_group_id = None;
        assert_eq!(
            run.confirm_cancellation(cleanup),
            Ok(ExecutionPhase::Cancelled)
        );
    }

    #[test]
    fn cancellation_cannot_complete_fail_or_restart_while_cleanup_is_pending() {
        let mut run = lifecycle();
        run.apply(ExecutionEvent::Prepare).unwrap();
        run.apply(ExecutionEvent::Start).unwrap();
        run.apply(ExecutionEvent::Cancel).unwrap();

        for event in [
            ExecutionEvent::Cancel,
            ExecutionEvent::Complete,
            ExecutionEvent::Fail,
            ExecutionEvent::Start,
        ] {
            assert_eq!(run.apply(event), Err(ExecutionError::InvalidTransition));
            assert_eq!(run.phase, ExecutionPhase::Cancelling);
        }
    }

    #[test]
    fn unavailable_commands_never_start_side_effects() {
        let run = lifecycle();
        assert_eq!(
            run.preflight(&CommandAvailability::MissingAuthentication),
            Err(ExecutionError::Unavailable)
        );
        assert_eq!(run.phase, ExecutionPhase::Ready);
    }

    #[test]
    fn terminal_receipts_cannot_be_fabricated_before_completion() {
        let mut run = lifecycle();
        assert_eq!(
            run.receipt(1, None),
            Err(ExecutionError::MissingTerminalOutcome)
        );
        assert_eq!(
            run.apply(ExecutionEvent::Complete),
            Err(ExecutionError::InvalidTransition)
        );
    }
}
