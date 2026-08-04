# C03 Agent Chat Entry Caller Contract

This table is the checked-in authority for WF-002/WF-008 entry verbs. `Open`, `Add`, and `Continue` never submit. `Ask` and `Send` require non-empty explicit text and may produce exactly one submitted turn.

| Caller / user affordance | Visible verb | Typed intent | Target | Text source | Context policy | Return route | Submission expectation | Source-consumption rule |
|---|---|---|---|---|---|---|---|---|
| Standard launcher / tray / stdin open | Open | `Open` | existing detached or embedded | optional draft | ambient or focused | launcher | none | never consume authored source from open alone |
| Double-tap Quick Question | Open | `Open` | existing detached or embedded | none | suppress focused | launcher | none | no source to consume |
| Clean external open (Brain Inbox staging, Spine bootstrap) | Open | `Open` | existing detached or embedded | none | suppress focused | source | none | caller consumes only after its own context staging succeeds |
| Launcher Quick AI with non-empty query | Ask | `Ask` | fresh embedded | launcher query | suppress focused | launcher | exactly one accepted turn | accepted submission only |
| Generic Quick AI switch without query | Open | `Open` | fresh embedded | none | suppress focused | launcher | none | no source to consume |
| Quick AI result promotion | Continue | `Continue` | fresh embedded | bounded result | suppress focused | Quick AI | none | exact result staged in destination composer |
| ScriptList plain-prose Cmd+Enter | Ask | `Ask` | existing detached or embedded | launcher prose | suppress focused | launcher | exactly one accepted turn | accepted submission only |
| Migrate-v1 / script-issues “Fix in Agent” | Ask | `Ask` | existing detached or embedded | generated diagnostic prompt | suppress focused | source | exactly one accepted turn | accepted submission only |
| Dictation “send to Agent Chat” | Send | `Send` | existing detached or embedded | final transcript | suppress focused | ScriptList/MainFilter | exactly one accepted turn | accepted submission only |
| Dictation composer delivery | Add | `Add` | existing detached or embedded | final transcript | suppress focused | source | none | exact transcript staged in composer |
| Preserve-return prompt handoff | Send | `Send` | existing detached or embedded | explicit prompt | ambient or focused | source | exactly one accepted turn | accepted submission only |
| Screen/window/selection/browser capture commands and quick-submit plans | Send | `Send` | selected harness destination | planner submission text | explicit capture / ambient | source | exactly one accepted turn | accepted submission only |
| Launcher/File Search selected row Cmd+Enter | Add | `Add` | existing detached or embedded | none | focused row, or suppressed default row | source | none | required context item staged |
| Actions selected payload | Add | `Add` | existing detached or embedded | none | explicit actions payload | source | none | required target staged |
| Notes selected note + cart | Add | `Add` | current main-host embedded | canonical note mention | explicit Notes handoff | Notes | none | primary note staged; cart consumption waits for final success |
| Plugin skill selection | Add | `Add` | existing detached or embedded | canonical slash command | specialized plugin-skill staging | source | none | slash text and thread-bound skill context both staged |
| Large paste / terminal output / explicit context part | Add | `Add` | embedded | optional draft | one explicit part | source | none | explicit part staged |
| Focused Text mini | Open | `Open` | fresh embedded | none | suppress focused | captured app | none until the mini submits explicitly | capture snapshot/failure staged by focused-text owner |

## Outcome contract

Every dispatch is `Complete(AgentChatEntryOutcome)` or `Pending(AgentChatEntryTicket)`. The ticket is non-cloneable and exposes one completion receiver. Fire-and-forget callers attach `observe_agent_chat_entry_dispatch`. Receipts include request ID, verb, disposition, destination host/thread/generation, text/context staging facts, submission state, blocked reason, and return route; they never contain authored text or raw context.
