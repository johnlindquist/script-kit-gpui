import type { Plugin } from "@opencode-ai/plugin"

/**
 * Project Reminders Plugin
 * 
 * Injects AI logging legend and autonomous testing reminders into:
 * 1. The system prompt (via experimental.chat.system.transform)
 * 2. Compaction context (via experimental.session.compacting)
 */

const LOG_LEGEND = `
## AI Log Format (SCRIPT_KIT_AI_LOG=1)
Format: \`SS.mmm|L|C|message\` | Levels: i/w/e/d/t | Categories: P=POSITION A=APP U=UI S=STDIN H=HOTKEY V=VISIBILITY E=EXEC K=KEY F=FOCUS T=THEME C=CACHE R=PERF W=WINDOW_MGR X=ERROR M=MOUSE_HOVER L=SCROLL_STATE Q=SCROLL_PERF D=DESIGN G=SCRIPT N=CONFIG Z=RESIZE
`.trim()

const TESTING_REMINDER = `
## Testing Protocol
- **Run app**: \`echo '{"type":"run","path":"..."}' | SCRIPT_KIT_AI_LOG=1 ./target/debug/script-kit-gpui 2>&1\`
- **Before commit**: \`cargo check && cargo clippy && cargo test\`
- **Visual changes**: MUST write test script using \`captureScreenshot()\`, save PNG to \`./test-screenshots/\`, read file to verify
- **If user provides image**: YOU MUST use visual testing - create test, capture screenshot, compare, fix issues shown in image
`.trim()

const COMBINED_REMINDER = `
<project-reminder>
${LOG_LEGEND}

${TESTING_REMINDER}
</project-reminder>
`.trim()

export const ProjectReminders: Plugin = async () => {
  return {
    // Inject into system prompt - runs on every conversation
    "experimental.chat.system.transform": async (_input, output) => {
      output.system.push(COMBINED_REMINDER)
    },

    // Preserve in compaction context
    "experimental.session.compacting": async (_input, output) => {
      output.context.push(COMBINED_REMINDER)
    }
  }
}
