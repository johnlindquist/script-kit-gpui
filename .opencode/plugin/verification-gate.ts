import type { Plugin } from "@opencode-ai/plugin"

/**
 * Verification Gate Plugin
 * 
 * Enforces quality gates before commits by:
 * 1. Detecting git commit attempts in tool calls
 * 2. Tracking whether verification was run in the session
 * 3. Adding reminders via system prompt
 * 
 * Uses documented hooks: tool.execute.before, tool.execute.after
 */

const VERIFICATION_COMMANDS = `cargo check && cargo clippy --all-targets -- -D warnings && cargo test`

const COMMIT_PATTERNS = [
  /git\s+commit/i,
  /git\s+add\s+.*&&\s*git\s+commit/i,
]

const VERIFICATION_PATTERNS = [
  /cargo\s+check/,
  /cargo\s+clippy/,
  /cargo\s+test/,
]

// Session state to track if verification was run
let verificationRanInSession = false
let lastVerificationTime = 0
const VERIFICATION_EXPIRY_MS = 5 * 60 * 1000 // 5 minutes

function isVerificationStale(): boolean {
  return Date.now() - lastVerificationTime > VERIFICATION_EXPIRY_MS
}

function containsCommitAttempt(text: string): boolean {
  return COMMIT_PATTERNS.some(pattern => pattern.test(text))
}

function containsVerificationCommand(text: string): boolean {
  // Must have all three verification commands
  return VERIFICATION_PATTERNS.every(pattern => pattern.test(text))
}

// Type for tool execution input
interface ToolInput {
  tool: string
  sessionID: string
  callID: string
  args?: Record<string, unknown>
  result?: Record<string, unknown>
}

export const VerificationGate: Plugin = async () => {
  // Reset state on plugin load
  verificationRanInSession = false
  lastVerificationTime = 0
  
  return {
    // Track when verification commands are run (after execution)
    "tool.execute.after": async (input: ToolInput) => {
      if (input.tool !== "bash") return
      
      const args = input.args || {}
      const result = input.result || {}
      const command = (args.command as string) || ""
      const output = (result.output as string) || ""
      
      // Check if this was a verification command
      if (containsVerificationCommand(command)) {
        // Only mark as verified if command succeeded (no error in result)
        const hasError = /error\[E\d+\]|^error:/im.test(output) ||
                        /warning:.*\n.*= help:/m.test(output) && /--\s*-D\s*warnings/.test(command)
        
        if (!hasError) {
          verificationRanInSession = true
          lastVerificationTime = Date.now()
        }
      }
    },

    // Intercept commit attempts and warn if verification not done (before execution)
    "tool.execute.before": async (input: ToolInput) => {
      if (input.tool !== "bash") return
      
      const args = input.args || {}
      const command = (args.command as string) || ""
      
      if (containsCommitAttempt(command)) {
        // Check if verification was run recently
        if (!verificationRanInSession || isVerificationStale()) {
          // Log warning - system prompt injection is the reliable approach for enforcement
          console.log("[VerificationGate] Commit attempted without recent verification")
        }
      }
    },

    // Add verification reminder to system prompt
    "experimental.chat.system.transform": async (_input, output) => {
      const verificationStatus = verificationRanInSession && !isVerificationStale()
        ? "✅ Verification recently passed"
        : "⏳ Verification pending"
      
      const reminder = `
<verification-gate-policy>
Before any git commit in this Rust project, ALWAYS run:
\`${VERIFICATION_COMMANDS}\`
All three must pass. Show evidence of passing before committing.

Current status: ${verificationStatus}
</verification-gate-policy>
`.trim()
      
      output.system.push(reminder)
    },

    // Preserve verification state through compaction
    "experimental.session.compacting": async (_input, output) => {
      const state = verificationRanInSession && !isVerificationStale()
        ? "Verification gate: PASSED (recent)"
        : "Verification gate: PENDING - run cargo check && cargo clippy && cargo test before commit"
      
      output.context.push(`<verification-state>${state}</verification-state>`)
    },
    
    // Reset state on new session
    event: async ({ event }) => {
      if (event.type === "session.created") {
        verificationRanInSession = false
        lastVerificationTime = 0
      }
    }
  }
}
