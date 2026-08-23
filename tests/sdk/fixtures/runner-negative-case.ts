// Private fixtures for runner-safety.test.ts. This file is never discovered by
// the SDK suite because discovery only reads test-*.ts in tests/sdk itself.

const mode = process.env.SDK_RUNNER_FAILURE_FIXTURE;

function result(test: string, status: string, error?: string): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), error }));
}

switch (mode) {
  case "timeout":
    result("completed-before-timeout", "pass");
    result("incomplete-before-timeout", "running");
    await new Promise((resolve) => setTimeout(resolve, 10_000));
    break;
  case "grandchild-timeout": {
    const child = Bun.spawn({
      cmd: ["/bin/sleep", "3"],
      stdout: "inherit",
      stderr: "inherit",
    });
    const pidPath = process.env.SDK_RUNNER_DESCENDANT_PID_PATH;
    if (!pidPath) throw new Error("owned descendant PID fixture path is required");
    await Bun.write(pidPath, String(child.pid));
    result("owned-descendant-before-timeout", "running");
    await new Promise((resolve) => setTimeout(resolve, 10_000));
    break;
  }
  case "nonzero":
    result("completed-before-exit", "pass");
    process.exit(9);
    break;
  case "invalid-status":
    result("stale-blocked-case", "running");
    result("stale-blocked-case", "blocked");
    break;
  case "missing-terminal":
    result("never-completed", "running");
    break;
  case "safety-env": {
    const safe =
      process.env.SDK_TEST_AUTOSUBMIT === "1" &&
      process.env.SCRIPT_KIT_ALLOW_SCREEN_TAKEOVER === "0" &&
      process.env.SCRIPT_KIT_ALLOW_VISIBLE_PROBES === "0" &&
      process.env.SCRIPT_KIT_ALLOW_NATIVE_INPUT === "0" &&
      process.env.SCRIPT_KIT_ALLOW_SCREEN_CAPTURE === "0" &&
      process.env.SCRIPT_KIT_ALLOW_LIVE_AI === "0" &&
      process.env.SCRIPT_KIT_ALLOW_ISOLATED_APP_LAUNCH === "0" &&
      process.env.SCRIPT_KIT_TEST_STATUS === "0" &&
      (
        !process.env.SDK_RUNNER_EXPECTED_CONCURRENCY ||
        process.env.SDK_TEST_CONCURRENCY === process.env.SDK_RUNNER_EXPECTED_CONCURRENCY
      ) &&
      process.env.INCLUDE_SYSTEM_INPUT === "0";
    result("noninteractive-child-environment", safe ? "pass" : "fail");
    break;
  }
  default:
    result("runner-fixture", "fail", `Unknown fixture mode: ${mode}`);
}
