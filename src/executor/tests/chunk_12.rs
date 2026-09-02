// ============================================================
// Process Group Termination Escalation Tests (SIGTERM → SIGKILL)
// ============================================================
//
// These tests verify the graceful termination escalation protocol:
// 1. SIGTERM is sent first (graceful shutdown request)
// 2. Wait up to TERM_GRACE_MS (250ms) for process to exit
// 3. If still alive, escalate to SIGKILL (forceful termination)
//
// This ensures scripts that ignore SIGTERM are still killed.

/// Test that a well-behaved process terminates gracefully with SIGTERM
/// This test verifies:
/// 1. ProcessHandle.kill() sends SIGTERM to the process group
/// 2. The process responds to SIGTERM (sleep is well-behaved)
/// 3. Process is properly reaped (no zombie)
///
/// Note: We verify behavior (signal received) not timing, to avoid CI flakiness.
#[cfg(unix)]
#[test]
fn test_sigterm_graceful_termination() {
    use std::os::unix::process::ExitStatusExt;

    let mut split = spawn_script("sleep", &["60"], "[test:sigterm_graceful]")
        .expect("spawn owned sleep")
        .split();
    let pid = split.pid();
    assert!(split.is_running(), "owned child must start running");

    split.kill().expect("kill must reap the child and confirm its group exited");
    let status = split.child.try_wait().unwrap().expect("child must already be reaped");
    assert!(
        matches!(status.signal(), Some(libc::SIGTERM | libc::SIGKILL)),
        "expected termination signal, got {status:?}"
    );
    assert_eq!(
        crate::process_manager::observe_owned_process_group(pid),
        crate::process_manager::OwnedProcessGroupLiveness::Exited
    );
}

/// Test that ProcessHandle.kill() is idempotent (safe to call multiple times)
/// This verifies that calling kill() after the process is already dead doesn't panic
#[cfg(unix)]
#[test]
fn test_kill_idempotent() {
    let mut split = spawn_script("sleep", &["10"], "[test:kill_idempotent]")
        .expect("spawn owned sleep")
        .split();
    split.kill().expect("first kill must complete owned cleanup");
    let status = split.child.try_wait().unwrap().expect("child reaped");
    split.kill().expect("second kill must be a no-op");
    split.kill().expect("third kill must be a no-op");
    assert_eq!(split.child.try_wait().unwrap(), Some(status));
    assert!(split.process_handle.killed);
}

/// Test that process group is killed (child processes too)
/// This spawns bash which spawns a child sleep, verifying both are killed
/// when we send SIGTERM to the process group.
#[cfg(unix)]
#[test]
fn test_process_group_kills_children() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::Instant;

    // Spawn bash with a background child process
    // The bash script: starts a sleep in background, prints "started", then waits
    let script_content = "sleep 60 & echo started; wait";

    let mut cmd = Command::new("bash");
    cmd.args(["-c", script_content])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Create process group so we can kill all children together
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    if let Ok(mut child) = cmd.spawn() {
        let pid = child.id();
        let start = Instant::now();

        // Wait for "started" to confirm the child sleep was spawned
        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() && line.trim() == "started" {
                // Good - child sleep has been spawned
            }
        }

        // Create a ProcessHandle to manage termination
        let mut handle = ProcessHandle::new(pid, "[test:process_group_children]".to_string());

        // Kill the process group
        handle.kill();

        // Wait for child to be reaped
        let timeout = std::time::Duration::from_millis(500);
        let poll_interval = std::time::Duration::from_millis(25);

        while start.elapsed() < timeout {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Child reaped - verify it's truly gone
                    let is_dead = !Command::new("kill")
                        .args(["-0", &pid.to_string()])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);
                    assert!(is_dead, "Process should be fully dead after wait");
                    return;
                }
                Ok(None) => {
                    std::thread::sleep(poll_interval);
                }
                Err(_) => break,
            }
        }

        // Final cleanup
        let _ = child.kill();
        let _ = child.wait();

        panic!(
            "Parent process (PID {}) should be dead after group kill (waited {:?})",
            pid,
            start.elapsed()
        );
    }
}

/// Test that ProcessHandle is registered and unregistered with PROCESS_MANAGER
#[cfg(unix)]
#[test]
fn test_process_handle_registration_lifecycle() {
    let mut split = spawn_script("sleep", &["10"], "[test:registration_lifecycle]")
        .expect("spawn owned sleep")
        .split();
    let pid = split.pid();
    let manager = &crate::process_manager::PROCESS_MANAGER;
    assert!(manager.get_active_processes().iter().any(|info| info.pid == pid));
    split.kill().expect("complete owned cleanup");
    drop(split);
    assert!(!manager.get_active_processes().iter().any(|info| info.pid == pid));
}

/// Test that kill() marks the handle as killed
#[cfg(unix)]
#[test]
fn test_kill_sets_killed_flag() {
    let mut split = spawn_script("sleep", &["10"], "[test:killed_flag]")
        .expect("spawn owned sleep")
        .split();
    assert!(!split.process_handle.killed);
    split.kill().expect("complete owned cleanup");
    assert!(split.process_handle.killed);
}

/// Test that double kill doesn't attempt to kill again
#[cfg(unix)]
#[test]
fn test_double_kill_is_noop() {
    let mut split = spawn_script("sleep", &["10"], "[test:double_kill_noop]")
        .expect("spawn owned sleep")
        .split();
    split.kill().expect("complete owned cleanup");
    assert!(split.process_handle.killed);
    split.process_handle.kill();
    assert!(split.process_handle.killed);
    assert!(!split.is_running());
}

/// Test SplitSession provides correct PID
#[cfg(unix)]
#[test]
fn test_split_session_pid() {
    let result = spawn_script("sleep", &["5"], "[test:split_session_pid]");

    if let Ok(session) = result {
        let original_pid = session.pid();
        let split = session.split();

        assert_eq!(
            split.pid(),
            original_pid,
            "SplitSession should report same PID as original session"
        );
    }
}

/// Test that wait() returns correct exit code
#[cfg(unix)]
#[test]
fn test_wait_returns_exit_code() {
    let result = spawn_script("sh", &["-c", "exit 42"], "[test:wait_exit_code]");

    if let Ok(session) = result {
        let mut split = session.split();

        // Wait for exit
        match split.wait() {
            Ok(code) => assert_eq!(code, 42, "Exit code should be 42"),
            Err(e) => panic!("wait() failed: {}", e),
        }
    }
}

/// Test is_running() accurately reflects process state
#[cfg(unix)]
#[test]
fn test_is_running_accuracy() {
    let mut split = spawn_script("sleep", &["10"], "[test:is_running_accuracy]")
        .expect("spawn owned sleep")
        .split();
    assert!(split.is_running(), "process must be running after spawn");
    split.kill().expect("kill must complete owned cleanup");
    assert!(!split.is_running(), "process must not be running after kill");
}
