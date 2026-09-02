// Included by the ordinary stdin prompt handler.
fn set_main_window_input_text_for_batch(
    this: &gpui::WeakEntity<ScriptListApp>,
    main_window_handle: Option<gpui::AnyWindowHandle>,
    expected: Option<&protocol::AutomationTargetIdentitySnapshot>,
    guard: &crate::platform::gpui_event_simulator::DispatchTarget,
    text: &str,
    cx: &mut gpui::AsyncApp,
) -> anyhow::Result<()> {
    if let Some(handle) = main_window_handle.or_else(crate::get_main_window_handle) {
        return handle.update(cx, |_root, window, cx| {
            validate_batch_window_effect(expected, guard, this, window, cx)?;
            this.update(cx, |app, cx| app.set_input_text_in_window(text, window, cx))
        })??;
    }
    this.update(cx, |app, cx| {
        validate_batch_main_effect(app, expected, guard, cx)?;
        app.set_input_text(text, cx)
    })?
}

fn validate_batch_window_effect(
    expected: Option<&protocol::AutomationTargetIdentitySnapshot>,
    guard: &crate::platform::gpui_event_simulator::DispatchTarget,
    main: &gpui::WeakEntity<ScriptListApp>,
    window: &gpui::Window,
    cx: &gpui::App,
) -> anyhow::Result<()> {
    guard.validate().map_err(anyhow::Error::msg)?;
    if let Some(expected) = expected {
        let main = main.upgrade();
        let actual = live_gpui_target_identity(main.as_ref(), &guard.info, window, cx)?;
        validate_gpui_expected_identity(expected, &actual).map_err(anyhow::Error::msg)?;
    }
    Ok(())
}

fn validate_batch_app_effect(
    expected: Option<&protocol::AutomationTargetIdentitySnapshot>,
    guard: &crate::platform::gpui_event_simulator::DispatchTarget,
    main: &gpui::WeakEntity<ScriptListApp>,
    cx: &mut gpui::App,
) -> anyhow::Result<()> {
    guard.handle().update(cx, |_, window, cx| {
        validate_batch_window_effect(expected, guard, main, window, cx)
    })?
}

fn validate_batch_main_effect(
    app: &ScriptListApp,
    expected: Option<&protocol::AutomationTargetIdentitySnapshot>,
    guard: &crate::platform::gpui_event_simulator::DispatchTarget,
    cx: &mut gpui::Context<ScriptListApp>,
) -> anyhow::Result<()> {
    guard.validate().map_err(anyhow::Error::msg)?;
    let Some(expected) = expected else {
        return Ok(());
    };
    let entity_id = cx.entity_id();
    guard.handle().update(cx, |_, window, cx| {
        let actual =
            live_gpui_target_identity_from_main(Some((app, entity_id)), &guard.info, window, cx)?;
        validate_gpui_expected_identity(expected, &actual).map_err(anyhow::Error::msg)
    })?
}

struct TransactionTransportSession {
    id: String,
    issued: std::collections::HashSet<String>,
}
impl Default for TransactionTransportSession {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            issued: Default::default(),
        }
    }
}
impl gpui::Global for TransactionTransportSession {}

use protocol::transaction_executor::{MAX_BATCH_COMMANDS, MAX_WAIT_POLLS};

fn prepare_transaction_transport(
    request_id: &str,
    commands: &[protocol::BatchCommand],
    options: &protocol::BatchOptions,
    target: Option<&protocol::AutomationWindowTarget>,
    cx: &mut gpui::App,
) -> Result<
    Option<(
        String,
        crate::platform::gpui_event_simulator::DispatchTarget,
    )>,
    protocol::TransactionError,
> {
    cx.default_global::<TransactionTransportSession>();
    let session = cx.global_mut::<TransactionTransportSession>();
    if session.issued.contains(request_id) {
        return Ok(None);
    }
    if session.issued.len() >= 100_000 {
        return Err(protocol::TransactionError::action_failed(
            "Transaction request budget exhausted",
        ));
    }
    session.issued.insert(request_id.to_owned());
    validate_batch_budget(commands.len(), options)?;
    let target = target.unwrap_or(&protocol::AutomationWindowTarget::Main);
    let guard = crate::platform::gpui_event_simulator::DispatchTarget::resolve(Some(target))
        .map_err(|error| protocol::TransactionError::action_failed(error.message))?;
    let fingerprint = protocol::transaction_executor::scoped_transaction_fingerprint(
        commands,
        Some(options),
        &guard.info,
        &session.id,
    )
    .map_err(|error| protocol::TransactionError::action_failed(error.to_string()))?;
    Ok(Some((fingerprint, guard)))
}

fn validate_batch_budget(
    count: usize,
    options: &protocol::BatchOptions,
) -> Result<(), protocol::TransactionError> {
    if count > MAX_BATCH_COMMANDS || options.timeout == 0 || options.timeout > 600_000 {
        return Err(protocol::TransactionError::action_failed(
            "Batch command/deadline budget invalid",
        ));
    }
    if options.rollback_on_error {
        return Err(protocol::TransactionError::action_failed(
            "Batch rollback is unsupported; no commands executed",
        ));
    }
    Ok(())
}

fn bounded_batch_wait(
    timeout: Option<u64>,
    poll: Option<u64>,
    remaining: std::time::Duration,
) -> (std::time::Duration, std::time::Duration) {
    let timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000)).min(remaining);
    let poll = std::time::Duration::from_millis(poll.unwrap_or(25).clamp(1, 1_000)).min(timeout);
    (timeout, poll)
}

#[cfg(test)]
mod batch_transport_budget_tests {
    use super::*;

    #[test]
    fn wait_cannot_outlive_batch_or_spin_at_zero_interval() {
        let (timeout, poll) = bounded_batch_wait(
            Some(60_000),
            Some(60_000),
            std::time::Duration::from_millis(7),
        );
        assert_eq!(timeout, std::time::Duration::from_millis(7));
        assert_eq!(poll, timeout);
        let (_, poll) = bounded_batch_wait(None, Some(0), std::time::Duration::from_secs(1));
        assert_eq!(poll, std::time::Duration::from_millis(1));
    }

    #[test]
    fn unsupported_rollback_and_excessive_batch_fail_before_mutation() {
        let mut options = protocol::BatchOptions {
            stop_on_error: true,
            rollback_on_error: false,
            timeout: 5_000,
        };
        assert!(validate_batch_budget(MAX_BATCH_COMMANDS, &options).is_ok());
        assert!(validate_batch_budget(MAX_BATCH_COMMANDS + 1, &options).is_err());
        options.rollback_on_error = true;
        assert!(validate_batch_budget(1, &options).is_err());
    }
}
