/**
 * Show a non-blocking macOS banner before a visible DevTools phase.
 *
 * The banner is owned by Notification Center rather than the application
 * under test, so it cannot change Script Kit focus/window topology or appear
 * in exact-window Quartz captures. Set SCRIPT_KIT_TEST_STATUS=0 for CI.
 * Noninteractive verification never displays a banner, even when another
 * caller inherited SCRIPT_KIT_TEST_STATUS=1 from a visible-probe session.
 */
export async function announceTestStatus(
  phase: string,
  detail = "Please ignore automated window movement and input",
) {
  if (
    process.env.SCRIPT_KIT_NONINTERACTIVE === "1" ||
    process.platform !== "darwin" ||
    process.env.SCRIPT_KIT_TEST_STATUS === "0"
  ) return;
  const escapeAppleScript = (value: string) => value
    .replaceAll("\\", "\\\\")
    .replaceAll('"', '\\"')
    .replaceAll("\n", " ");
  const script = `display notification "${escapeAppleScript(detail)}" with title "Script Kit test" subtitle "${escapeAppleScript(phase)}"`;
  const child = Bun.spawn(["osascript", "-e", script], {
    stdout: "ignore",
    stderr: "ignore",
  });
  await child.exited;
}
