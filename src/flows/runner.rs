//! Spawns `md <flow> --events` runs and streams NDJSON into the registry.
//!
//! Contract (docs/ai/flow-ux-protocol.md §3):
//! - The `Starting` run is inserted into the registry BEFORE the process
//!   spawns, so the launch is acknowledged in the same frame it was
//!   requested.
//! - Each run gets its own process group (`process_group(0)`), so
//!   cancellation can kill the whole tree: SIGTERM to the group, bounded
//!   wait, then SIGKILL to the group. Cancel targets one run only.
//! - Reader threads never touch GPUI; they mutate the thread-safe registry,
//!   whose notify hook schedules the repaint.

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::catalog::mdflow_binary;
use super::model::{
    parse_event_line, EngagementMode, EventStreamValidator, FlowUxVariant, RunPhase,
};
use super::run_registry::flow_run_registry;
use super::session::{resolve_mdflow_turn_arg, MdflowTurnArg};

/// Bounded SIGTERM→SIGKILL escalation window (protocol §3).
const CANCEL_ESCALATION: Duration = Duration::from_secs(2);

/// Launch a flow run. Returns the registry-local run id immediately; all
/// process work happens on background threads. `input_overrides` are
/// (name, value) pairs passed as `--_<name> <value>` (collected natively —
/// `--events` runs never prompt). `capture_conversation` turns on the
/// append-only stdout accumulator for conversation turns (see
/// `FlowRun::conversation_stdout`).
#[allow(clippy::too_many_arguments)]
pub fn launch_flow(
    flow_id: &str,
    flow_name: &str,
    flow_path: &str,
    cwd: &str,
    variant: FlowUxVariant,
    engagement: EngagementMode,
    input_overrides: Vec<(String, String)>,
    launch_requested: Instant,
    capture_conversation: bool,
) -> u64 {
    let registry = flow_run_registry();
    let local_id =
        registry.insert_starting(flow_id, flow_name, flow_path, cwd, variant, engagement);
    registry.record_launch_ack(local_id, launch_requested.elapsed().as_millis() as u64);
    if capture_conversation {
        registry.enable_conversation_capture(local_id);
    }
    registry.record_override_names(
        local_id,
        input_overrides
            .iter()
            .map(|(name, _)| name.clone())
            .collect(),
    );

    let flow_path = flow_path.to_string();
    let cwd = cwd.to_string();
    std::thread::Builder::new()
        .name(format!("flow-run-{local_id}"))
        .spawn(move || {
            run_flow_process(
                local_id,
                &flow_path,
                &cwd,
                &input_overrides,
                capture_conversation,
            )
        })
        .ok();
    local_id
}

fn build_flow_command(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    overrides: &[(String, String)],
    turn_arg: Option<&MdflowTurnArg>,
) -> Command {
    let mut command = Command::new(binary);
    command.arg(flow_path);
    match turn_arg {
        Some(MdflowTurnArg::Positional(prompt)) => {
            command.arg(prompt);
        }
        Some(MdflowTurnArg::NamedTask(prompt)) => {
            command.arg("--_task").arg(prompt);
        }
        None => {}
    }
    command
        .arg("--events")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // stderr is out-of-band diagnostics (protocol §3) — captured, never
        // discarded: a flow crashing with only a stderr message must leave a
        // visible trace (2026-07-11 audit).
        .stderr(Stdio::piped());
    for (name, value) in overrides {
        // A Threadline turn arrives through the legacy `task` override slot;
        // its resolved positional/named shape was already appended above.
        if turn_arg.is_some() && name == "task" {
            continue;
        }
        command.arg(format!("--_{name}")).arg(value);
    }
    command
}

fn run_flow_process(
    local_id: u64,
    flow_path: &str,
    cwd: &str,
    overrides: &[(String, String)],
    capture_conversation: bool,
) {
    let registry = flow_run_registry();
    // Cancel-before-spawn: a run cancelled while still queued must never
    // spawn a process at all (a leaked process here would be invisible to
    // cancellation, which only signals recorded pgids).
    if registry
        .get(local_id)
        .is_none_or(|run| run.phase.is_terminal())
    {
        return;
    }
    let Some(binary) = mdflow_binary() else {
        registry.mark_failed(local_id, "mdflow CLI not found on PATH (npm i -g mdflow)");
        return;
    };

    let turn_arg = if capture_conversation {
        let Some((_, prompt)) = overrides.iter().find(|(name, _)| name == "task") else {
            registry.mark_failed(local_id, "Threadline run missing its turn prompt");
            return;
        };
        match resolve_mdflow_turn_arg(binary, flow_path, cwd, prompt) {
            Ok(arg) => Some(arg),
            Err(err) => {
                registry.mark_failed(local_id, &err);
                return;
            }
        }
    } else {
        None
    };

    let mut command = build_flow_command(binary, flow_path, cwd, overrides, turn_arg.as_ref());
    // Own process group so cancel can kill the full descendant tree without
    // touching sibling runs.
    command.process_group(0);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            registry.mark_failed(local_id, &format!("failed to spawn {binary}: {err}"));
            return;
        }
    };
    let pid = child.id();
    // set_pid returns the phase observed under the registry lock: if a
    // cancel landed between spawn and pid publication, the phase is already
    // Cancelled and WE must deliver the signal cancel_run could not (it had
    // no pgid to target yet).
    let observed = registry.set_pid(local_id, pid);
    if observed.is_some_and(|phase| phase.is_terminal()) {
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGTERM);
        }
        spawn_kill_escalation(local_id, pid);
    }
    // Orphan hygiene only — run lifecycle state lives in the flow registry.
    crate::process_manager::PROCESS_MANAGER.register_process(pid, flow_path);

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            registry.mark_failed(local_id, "child stdout unavailable");
            let _ = child.kill();
            return;
        }
    };
    // Drain raw stderr on its own thread (out-of-band diagnostics; protocol
    // stdout stays pure). Retained on the run's tails so failures have a
    // visible trace.
    if let Some(stderr) = child.stderr.take() {
        let stderr_registry = registry.clone();
        std::thread::Builder::new()
            .name(format!("flow-run-{local_id}-stderr"))
            .spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    stderr_registry.push_raw_stderr(local_id, &line);
                }
            })
            .ok();
    }

    // Strict protocol consumption (docs/ai/flow-ux-protocol.md §3): any
    // parse error or ordering violation fails the run CLOSED — a stream
    // that lies about identity or ordering cannot be trusted to report the
    // outcome, so the process is also killed rather than left running
    // unsupervised.
    let mut validator = EventStreamValidator::default();
    let mut protocol_failure: Option<String> = None;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let verdict = parse_event_line(&line)
            .map_err(|err| err.to_string())
            .and_then(|envelope| {
                validator
                    .validate(&envelope)
                    .map(|()| envelope)
                    .map_err(|violation| violation.to_string())
            });
        match verdict {
            Ok(envelope) => registry.apply_event(local_id, &envelope),
            Err(detail) => {
                protocol_failure = Some(detail);
                break;
            }
        }
    }

    if let Some(detail) = &protocol_failure {
        crate::logging::log(
            "FLOWS",
            &format!("run {local_id}: protocol violation — {detail}"),
        );
        // A cancel already in flight keeps its outcome; otherwise the run
        // fails closed. Either way the untrustworthy process is killed.
        if registry.get(local_id).map(|run| run.phase) == Some(RunPhase::Cancelling) {
            registry.mark_cancelled(local_id);
        } else {
            registry.mark_failed(local_id, &format!("protocol violation: {detail}"));
        }
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGTERM);
        }
        spawn_kill_escalation(local_id, pid);
    }

    let status = child.wait();
    crate::process_manager::PROCESS_MANAGER.unregister_process(pid);
    if !validator.saw_terminal() && protocol_failure.is_none() {
        // Process died without a terminal event (killed, crashed, legacy
        // binary). Preserve cancellation if we initiated it, and surface
        // the last stderr diagnostic when one exists.
        let phase = registry.get(local_id).map(|r| r.phase);
        match phase {
            Some(RunPhase::Cancelled) => {
                // cancel_run already settled it.
            }
            Some(RunPhase::Cancelling) => {
                // We initiated the kill and the process is now confirmed
                // dead — the cancel receipt becomes truthful here.
                registry.mark_cancelled(local_id);
            }
            _ => {
                let stderr_note = registry
                    .get(local_id)
                    .and_then(|run| run.stderr_tail.last_line().map(str::to_string))
                    .map(|line| format!(" — stderr: {line}"))
                    .unwrap_or_default();
                match status {
                    Ok(status) if status.success() => registry.mark_failed(
                        local_id,
                        &format!("run ended without a terminal protocol event{stderr_note}"),
                    ),
                    Ok(status) => registry.mark_failed(
                        local_id,
                        &format!("process exited {status} without terminal event{stderr_note}"),
                    ),
                    Err(err) => {
                        registry.mark_failed(local_id, &format!("wait failed: {err}{stderr_note}"))
                    }
                }
            }
        }
    }
}

/// Cancel one run: SIGTERM its process group, escalate to SIGKILL after a
/// bounded wait if the group survives. Never touches other runs.
///
/// Truthful receipts: the run enters `Cancelling` when the signal is sent
/// and only becomes `Cancelled` once the outcome is known — the process's
/// own `run.cancelled` event, the reader-thread EOF fallback, or the
/// escalation watcher confirming the group is dead.
pub fn cancel_run(local_id: u64) {
    let registry = flow_run_registry();
    let Some(run) = registry.get(local_id) else {
        return;
    };
    if run.phase.is_terminal() || run.phase == RunPhase::Cancelling {
        return;
    }
    let Some(pid) = run.pid else {
        // Not spawned yet: nothing is running, so Cancelled is immediately
        // truthful (the runner thread's cancel-before-spawn guard prevents
        // a later spawn).
        registry.mark_cancelled(local_id);
        return;
    };

    registry.mark_cancelling(local_id);
    // `process_group(0)` made the child its own group leader → pgid == pid.
    let killpg_result = unsafe { libc::killpg(pid as libc::pid_t, libc::SIGTERM) };
    if killpg_result != 0 {
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error() == Some(libc::ESRCH) {
            // Group already gone: dead is dead — settle as Cancelled unless
            // a terminal event already recorded the real outcome.
            registry.mark_cancelled(local_id);
            return;
        }
        // Signal actually failed (e.g. EPERM): the process may still be
        // running — stay in Cancelling and let escalation resolve it.
        crate::logging::log(
            "FLOWS",
            &format!("run {local_id}: killpg({pid}, SIGTERM) failed: {errno} — escalating"),
        );
    }
    spawn_kill_escalation(local_id, pid);
}

/// Bounded SIGTERM→SIGKILL escalation watcher for one process group. Once
/// the group is confirmed dead it settles a still-`Cancelling` run as
/// `Cancelled` (the reader thread usually does this first via EOF; this is
/// the backstop for pathological pipes).
fn spawn_kill_escalation(local_id: u64, pgid: u32) {
    std::thread::Builder::new()
        .name(format!("flow-cancel-{local_id}"))
        .spawn(move || {
            let registry = flow_run_registry();
            let finalize = |registry: &crate::flows::run_registry::FlowRunRegistry| {
                if registry.get(local_id).map(|run| run.phase) == Some(RunPhase::Cancelling) {
                    registry.mark_cancelled(local_id);
                }
            };
            // Poll the group (not just the leader) so descendants count,
            // with the protocol's 2s bound before SIGKILL.
            let deadline = Instant::now() + CANCEL_ESCALATION;
            while Instant::now() < deadline {
                if !process_group_alive(pgid) {
                    finalize(&registry);
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if process_group_alive(pgid) {
                unsafe {
                    libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
                }
                crate::logging::log(
                    "FLOWS",
                    &format!("run {local_id}: escalated cancel to SIGKILL for pgid {pgid}"),
                );
                // SIGKILL cannot be caught; give the kernel a beat to reap.
                for _ in 0..20 {
                    if !process_group_alive(pgid) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            finalize(&registry);
        })
        .ok();
}

/// True while any process in the group is alive (signal 0 probe).
pub fn process_group_alive(pgid: u32) -> bool {
    unsafe { libc::killpg(pgid as libc::pid_t, 0) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::run_registry::flow_run_registry;

    /// Launch with a missing binary path exercises the spawn-failure path
    /// end to end without depending on mdflow being installed.
    #[test]
    fn cancel_before_spawn_marks_cancelled() {
        let registry = flow_run_registry();
        let id = registry.insert_starting(
            "project:t",
            "t",
            "/nonexistent/flows/t.md",
            "/tmp",
            FlowUxVariant::Dispatch,
            EngagementMode::Background,
        );
        cancel_run(id);
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Cancelled);
        cancel_run(id); // idempotent on terminal runs
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Cancelled);
    }

    fn command_args(turn_arg: MdflowTurnArg) -> Vec<String> {
        build_flow_command(
            "/tmp/md",
            "flows/example.md",
            "/tmp",
            &[("task".to_string(), "ignored legacy slot".to_string())],
            Some(&turn_arg),
        )
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
    }

    #[test]
    fn threadline_ordinary_flow_passes_prompt_as_first_positional_arg() {
        assert_eq!(
            command_args(MdflowTurnArg::Positional("hello ordinary".into())),
            ["flows/example.md", "hello ordinary", "--events"]
        );
    }

    #[test]
    fn threadline_named_task_flow_uses_task_flag_only_when_resolved() {
        assert_eq!(
            command_args(MdflowTurnArg::NamedTask("hello named".into())),
            ["flows/example.md", "--_task", "hello named", "--events"]
        );
    }

    #[test]
    fn process_group_probe_reports_dead_group() {
        // PID 1 exists but we cannot signal its group as a normal user —
        // either way this must not panic; a definitely-unused pgid reports
        // dead.
        let _ = process_group_alive(1);
        assert!(!process_group_alive(4_000_000));
    }
}
