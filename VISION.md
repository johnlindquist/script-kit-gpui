# Script Kit GPUI Vision

This document is the product and architecture vision for Script Kit GPUI. It
guides launch decisions, product tradeoffs, implementation direction, and
tool-facing documentation. Current source, tests, generated contracts, and repo
docs remain the source of truth for behavior.

When updating this document or making product-facing changes, start from
`AGENTS.md`, `README.md`, `GLOSSARY.md`, `.impeccable.md`, and the current
source/tests. `CLAUDE.md` is a protected tool-facing root document; changes to
`CLAUDE.md` should align with this vision instead of redefining the product
direction. If this document conflicts with current code, generated contracts, or
verified runtime receipts, update the document or the implementation
intentionally.

Owning context: this is a product architecture document. It owns launch vision,
product identity, decision rules, anti-goals, and verification expectations. It
does not replace `README.md`, `GLOSSARY.md`, `AGENTS.md`, `CLAUDE.md`,
generated contracts, tests, or source code.

## One-Sentence Vision

Script Kit GPUI is a native, keyboard-first programmable command center with a
local memory — everything the user captures, copies, says, and discusses
compounds into markdown they own — for people who want their desktop workflows
to be fast, local, inspectable, and owned by them.

## The Thread

Launchers are stateless: they answer "what do you want to do right now?" and
forget the user the moment they close. Script Kit GPUI is stateful: every
interaction leaves a deposit, and the deposits compound. The defining product
moment is not the launch — it is the return path, when the tool gives back
something the user had forgotten they gave it, at the moment they need it.

Stated as a thesis:

> One key. Everything you touch can become memory. Time is the structure,
> meaning is the index, and one gesture is the price.

The launcher, the Day Page, and Agent Chat are not three tools. They are three
pressures on the same key. See the Memory Layer section below and
`docs/adr/0002-one-key-gesture-grammar.md`.

## Launch Promise

Within the first few minutes, a new user should understand the promise:

- The launcher appears instantly and responds like a native Mac tool.
- Scripts are local TypeScript files the user can inspect, edit, and version.
- Prompt UIs feel coherent across scripts, built-ins, and Agent Chat.
- AI behavior is selected through explicit profiles, not hidden personas.
- Important product claims can be backed by semantic state, layout, logs, and
  receipts.

That promise is more important than feature breadth. A small number of coherent,
fast, ownable workflows beats a large surface area that feels fragmented or
opaque.

## Launch Workflows

Launch should be legible through a few concrete workflows, not only through
architecture language.

### First Five Minutes

In the first five minutes, a new user should be able to:

- Open Main Menu Search, find a runnable example, and understand where its local
  TypeScript file lives.
- Describe a workflow to Agent Chat, review the generated script, run a bounded
  verification step, and save the result as an owned local artifact.
- See which profile, cwd, model, tools, and context posture Agent Chat will use
  before trusting AI output.
- Inspect the receipt or local file path that proves what happened.

### Daily Loop

In the daily loop, a returning user should be able to:

- Type one query, choose a script, built-in, profile, capture target, or Action,
  then return to work without managing a dashboard.
- Attach current desktop context to Agent Chat with a resolution receipt that
  names attempted, resolved, and failed parts.
- Inspect or adopt a local plugin artifact by looking at its manifest and
  folders before running what it contains.
- Automate a prompt through semantic target identity, `getState`,
  `getElements`, `getLayoutInfo`, `waitFor`, `batch`, logs, and action receipts
  instead of timing guesses.

## Product Loop

Script Kit GPUI uses launcher speed as the front door, but the launcher is not
the whole product. The product is the loop:

```text
search -> choose -> prompt -> act -> automate -> verify -> return
```

Each step is a product lens:

- Search: find scripts, built-ins, profiles, commands, capture targets, and
  context through one keyboard-first command surface.
- Choose: make the right object obvious through shared rows, preview rules,
  source labels, and predictable focus.
- Prompt: turn workflows into native-feeling prompt UIs instead of one-off app
  panels.
- Act: run the selected workflow, switch profile, open Actions, or stage intent
  for Agent Chat.
- Automate: expose state and actions semantically so scripts and agents can
  operate without timing guesses or coordinate hacks.
- Verify: produce receipts for target identity, semantic state, layout,
  transcript, and runtime behavior.
- Return: finish quickly and get out of the user's way.

At launch, Script Kit GPUI should feel like one coherent native tool across
scripts, prompt UIs, built-ins, Actions, Agent Chat, profiles, shareable plugin
artifacts, and semantic automation receipts.

The launch workflows above are examples of that loop in practice: the product
should make owned creation, context-aware Agent Chat, local artifact inspection,
and semantic automation feel like one repeatable path.

## What Script Kit GPUI Is Trying To Be

Script Kit GPUI is a fast native GPUI launcher shell over a programmable local
automation runtime. The shell should appear instantly, accept intent, execute or
stage the right workflow, and disappear.

It is a Bun-powered TypeScript runtime where users can write and version their
own scripts, bring their own dependencies, and keep control of the code that runs
on their machine.

It is a prompt-first SDK. Prompt APIs are not incidental helpers; they are the UI
composition model for personal automation. Stable prompt surfaces should share
chrome, focus behavior, footer discipline, semantic IDs, and layout receipts.

It is a shared native UI system for scripts, prompts, built-ins, and Agent Chat.
The product should feel coherent because surfaces share main search, Actions,
list row language, prompt shells, footer affordances, theme tokens, and
automation semantics.

It is a local plugin workspace for scripts, scriptlets, skills, profiles, and
shareable plugin repos. The filesystem model is part of the product: users
should be able to inspect, edit, version, and migrate the artifacts they run.

It is an Agent Chat profile runtime. Profiles should make AI behavior
attributable, selectable, warmable, logged, and testable through explicit prompt,
model, tool, cwd, session, and ambient-resource posture.

It is a semantic automation surface with receipts. Script Kit should prefer
target identity, semantic IDs, layout info, deterministic `waitFor`/`batch`
transactions, transcripts, logs, and action receipts over sleeps, coordinates, or
screenshot-only proof.

It is a local memory layer — Script Kit Brain. Notes, captures, clipboard
sediment, dictation, and Agent Chat turns accumulate as markdown files the user
owns, indexed (never owned) by a local hybrid search brain that resurfaces them
in search, in chat context, and in curator observations.

## The Memory Layer (Script Kit Brain)

This section is direction, not shipped behavior. Where it conflicts with
current source, the gap is intentional and tracked in
`docs/adr/0002-one-key-gesture-grammar.md`,
`docs/adr/0003-markdown-files-as-memory-substrate.md`,
`docs/adr/0004-clipboard-sediment.md`, and `docs/specs/**`.

Naming: the platform stays Script Kit. The memory layer is Script Kit Brain.

### One Key, Three Pressures

The entire interaction model for the memory layer is the existing main hotkey
plus two established conventions (Cmd+Enter for Agent Chat, Esc to dismiss). No
new chords.

- Tap (closed): the launcher, visible on key-down, instant, unchanged.
- Tap (open): toggle launcher <-> Day Page. In-flight query text carries over
  as the start of a capture. Esc is the only dismiss.
- Hold (~250ms): the window deepens into the Day Page. (Dictation stays with
  the dedicated dictation shortcut and window.)
- Double-tap: Agent Chat, from closed or from either surface.

The disambiguation rule is "always open, then deepen": the launcher appears on
key-down in every case, and hold/double-tap escalate the already-open window.
No gesture ever waits on another. Surfaces morph inside one window; windows are
never swapped.

### The Day Page

The Day Page is one markdown file per day on disk. Humans think in time blocks
("what happened yesterday"), so time is the structure: captures append
to today, grabs and promoted clipboard entries append with provenance, and
Agent Chat threads leave one-line traces. The day page must stay human-readable
in any editor: long captures live as separate markdown fragment files with
provenance frontmatter, referenced from the day page with a short excerpt.

The brain indexes the files; it never owns them. Markdown on disk is canonical;
SQLite (FTS, embeddings, signals) is a derived, rebuildable index. Delete the
app, keep your life.

The floating Notes window survives for notes about something other than today.
The Day Page and a note must feel identical — same editor entity, hosted by
different shells.

### Clipboard Sediment

Copies become memory through earned importance, not filing decisions:

- URLs are kept automatically.
- Other content is kept when behavior proves value (re-paste), or when the user
  promotes it via the post-copy quick menu (tap Cmd shortly after Cmd+C) by
  adding the "why".
- Concealed pasteboard types, password-manager sources, and user-defined
  secret patterns are hard-rejected. This is a launch-facing privacy claim and
  needs receipts before strong language.
- Kept copies land in a low tier and promote through signals (re-paste,
  annotation, recall) so resurfacing stays high-signal.

### Memory Anti-Goals

- No cloud, no accounts. The brain is local files plus a local index.
- The launcher input does not silently keep plain queries. Capture requires
  expressed intent: a semicolon tag, the hold gesture, the toggle, a grab, or a
  clipboard rule.
- The brain must never become a second filing system the user has to manage.
  Importance is earned by behavior, not declared by the user.

## What It Is Not

Script Kit GPUI is not a drop-in replacement for old Script Kit. The spirit
carries forward: fast scripting, prompt APIs, keyboard-first automation, and
local control. The old giant global helper surface does not.

Script Kit GPUI is not a marketplace-first product with local scripts bolted on.
Discovery and adoption matter, but they should preserve local ownership,
inspectability, and editability.

Script Kit GPUI is not a web dashboard in native clothing. It should appear,
accept intent, perform the job, and disappear.

Script Kit GPUI is not an unrestricted AI agent shell. Agent Chat and profiles
must be attributable, scoped, inspectable, and honest about what the runtime can
actually enforce.

## Why It Is Different

Modern launchers prove that users value fast command surfaces, polished keyboard
flow, and native-feeling interactions. Script Kit GPUI should meet that quality
bar, but its center of gravity is different.

Script Kit GPUI is attractive because it lets users turn personal workflows into
local artifacts they can own:

| Dimension | Script Kit GPUI direction |
| --- | --- |
| Core unit | User-owned script, prompt, plugin artifact, or profile |
| Customization | TypeScript, Bun packages, prompt APIs, plugin folders, config/theme files |
| Sharing/adoption | Shareable plugin repos that remain local and inspectable after adoption |
| AI | Profile-scoped Agent Chat runtime with tools, cwd, prompt, session policy, and receipts |
| Automation | Semantic state, `getElements`, `getLayoutInfo`, `waitFor`, `batch`, logs, and receipts |
| Migration | Preserve prompt spirit, drop helper sprawl, convert durable AI behavior into explicit profiles |

The product should not win by copying another launcher feature-for-feature. It
should win by making the user's own workflows feel native, fast, programmable,
and verifiable.

## Audience And Jobs To Be Done

Script Kit GPUI should delight developers and power users first: people who
already live in editors, terminals, Git, local files, keyboard launchers, and
personal automation. Their daily pain is not lack of another command palette; it
is the gap between a fast idea and an owned workflow they can inspect, edit,
version, rerun, and prove.

The next audience is automation-minded Mac users who want useful examples,
scriptable workflows, and Agent Chat profiles without starting from a blank
file. They should be able to adopt a local artifact, understand what it will run,
and make small edits before they need to understand the whole architecture.

The launch wedge is deliberately narrower than "everyone who uses a Mac." Do not
dilute the programmable local core to behave like a generic consumer launcher,
a folder of shell scripts, or a generic AI chat tab. Onboarding can be friendly,
but the tradeoff should stay explicit: local ownership, keyboard speed, and
inspectable automation matter more than hiding the filesystem.

## Priority Ladder

When launch priorities conflict, use this ladder:

1. Native speed and confidence.
2. Prompt coherence across scripts, built-ins, and Agent Chat.
3. Local artifact ownership: scripts, plugins, skills, and profiles should be
   inspectable and editable.
4. Agent Chat/profile attribution: AI behavior should have an explicit runtime
   boundary.
5. Semantic receipts: target identity, state, layout, logs, and transcripts
   should prove important claims.

The ladder is not a rigid roadmap. It is a tie-breaker. A feature that improves
distribution but weakens local ownership should lose. A feature that adds AI
power but weakens attribution should wait. A feature that looks polished but
cannot be operated or verified semantically is not launch-ready.

## Launch Bets

The priority ladder turns into these launch bets for near-term execution. These
are not a permanent roadmap; they are the product moves that should win while
the launch promise is still being proven.

### Now

- Make Main Menu Search the command spine for scripts, built-ins, profiles,
  command grammar, capture targets, and Agent Chat intent.
- Make verified local artifact creation feel first-class: intent becomes a
  reviewable TypeScript script, profile, skill, scriptlet, or plugin artifact
  with a local path and proof status.
- Keep prompt UIs, built-ins, Actions, Agent Chat, and feedback surfaces inside
  one shared native surface language.
- Make Agent Chat and profile behavior attributable through explicit model,
  prompt, tool, cwd, session, context, log, and receipt posture.
- Treat semantic receipts as product evidence, not internal debug output.

### Next

- Deepen context attachment so desktop, file, clipboard, notes, browser,
  selected-text, and current-app context can be staged with honest resolution
  receipts.
- Polish CreationFeedback so generated artifacts clearly offer edit, reveal,
  rerun, attach-to-profile, and inspect-receipt actions.
- Broaden profile and local plugin artifact coverage without weakening
  inspectability or editability.
- Make receipt history visible enough that users and agents can audit important
  actions after the fact.

### Later

- Defer broad marketplace and distribution-system decisions until local
  ownership, reviewability, update, removal, and rollback expectations are
  proven.
- Defer broad feature breadth that would add side-channel pickers, one-off
  chrome, or unsupported prompt promises.
- Defer media prompt launch claims until current source and runtime receipts
  prove them.
- Defer stronger AI filesystem-enforcement claims until native or wrapper-level
  enforcement exists and receipts prove it.

## Launch Gates

A launch-facing claim needs a named proof path before it becomes product copy.
Where the repo does not yet have stable numeric baselines, use receipts and
focused checks instead of fake precision.

| Gate | Launch-ready means | Proof shape |
| --- | --- | --- |
| Native speed and keyboard confidence | The launcher opens, filters, moves selection, stages intent, and dismisses without visible hesitation or focus ambiguity. | DevTools or focused test receipts for surface identity, input freshness, selection movement, and return-to-work behavior. |
| Time-to-first owned artifact | A new user can reach a runnable or generated local script/profile/skill/plugin artifact, inspect its path, and know whether verification passed. | CreationFeedback/source-audit checks plus a receipt or diff showing the local artifact path and proof status. |
| Prompt coherence | Stable prompts, built-ins, Actions, and Agent Chat share chrome, rows, footer discipline, focus behavior, and semantic IDs unless a documented exception exists. | Shared surface contract tests, footer ownership checks, `getElements`, and `getLayoutInfo` receipts. |
| Local artifact ownership | Scripts, scriptlets, skills, profiles, and plugin artifacts remain inspectable, editable, versionable, removable, and migratable after adoption. | Manifest/path checks, plugin/profile artifact validation, and source checks that expose the local folder model. |
| Agent Chat/profile attribution | AI behavior is tied to an explicit profile/runtime boundary and does not imply hidden agents or stronger enforcement than exists. | Profile selection, model/cwd/tool posture, transcript, log, and action receipts. |
| Semantic receipt coverage | Launch-facing automation can report target identity, state, semantic elements, layout, transactions, logs, and pass/fail status. | `getState`, `getElements`, `getLayoutInfo`, `waitFor`, `batch`, transcript, and action-receipt checks. |

## Product Pillars

### Native Speed And Confidence

Fast is not a nice-to-have. The launcher should feel immediate. Keyboard-first
behavior, minimal chrome, native macOS feel, predictable focus, and low-friction
dismissal are part of the product.

The design language is fast, focused, and minimal. Chrome should earn its place.
The UI should make the user feel in control, not like they are operating a web
app inside a panel.

### Prompts Are The App Primitive

Prompt APIs are Script Kit's UI composition model. Stable prompt surfaces such
as `arg`, `div`, `editor`, `fields`, `form`, `path`, `term`, `drop`, `hotkey`,
and `chat` should feel like parts of one native product.

Do not claim media prompts such as `mic()` or `webcam()` are launch-ready until
current source and runtime receipts prove they are implemented. The current
source treats them as explicit coming-soon stubs.

#### Prompt Launch Readiness

Prompt claims should be source-aligned and updated when source, tests, and
runtime receipts change. A README or SDK list is not enough by itself to make a
prompt launch-ready.

| Prompt surface | Launch posture | Claim rule |
| --- | --- | --- |
| `arg`, `div`, `editor`, `fields`, `form`, `path`, `term`, `drop`, `hotkey`, `chat` | Source-aligned prompt surfaces | Claim launch readiness only with current source ownership, tests, and prompt/runtime receipts. |
| `mic()` | Coming-soon media prompt | Do not call launch-ready while source routes it through `MicComingSoon` or lacks runtime media receipts. |
| `webcam()` | Coming-soon media prompt | Do not call launch-ready while source routes it through `WebcamComingSoon` or lacks runtime media receipts. |
| `ShowMicro` | Compact input implementation detail | Do not confuse micro input support with `mic()` audio capture readiness. |

### The Launcher Is The Spine

Main Menu Search is the entry point for scripts, built-ins, profiles, command
syntax, context staging, and Agent Chat intent. Avoid side-channel pickers unless
they are contextual Actions.

The launcher should not become a cluttered dashboard. It is the command spine:
type intent, choose the right object, run or stage the workflow, then return the
user to their work.

Search has one linked state chain: immutable query meaning and retrieval scope →
provider work and current consumer authority → committed rows → selection intent
and selected subject → preview/footer/action subject → completed GPUI frame.
Automatic selection follows the first eligible result; a deliberate choice anchors
its identity. Input focus and a user-scrolled viewport are separate state. Async
arrival may improve results without changing query meaning or redirecting a stale
action to the wrong object.

### Actions Are Discovery

Command+K / Actions should answer "what can I do here?" New persistent controls
should go into Actions before earning permanent chrome.

Actions should feel like a natural extension of the main list, not a separate UI
system. The invariant is shared language, keyboard-first behavior, native feel,
contextual relevance, and no footer bloat.

### Three-Affordance Footer Discipline

The mental model should stay small:

```text
Primary action. Actions. Agent.
```

Contextual primary labels are fine. Persistent footer bloat is launch debt. If a
surface needs more operations, they belong in Actions unless there is a tested,
documented exception.

### Plugins Are Local Artifact Containers

A plugin can carry scripts, scriptlets, skills, profiles, and compatibility agent
artifacts. The local filesystem model is a product contract, not an
implementation detail.

Users should be able to inspect the plugin folder, understand what will run,
version it, edit it, and migrate it. Future sharing and distribution mechanics
should support that ownership model rather than obscure it.

#### Plugin Ownership Standards

Plugin work should preserve local ownership before it optimizes discovery.

- The local plugin root should be inspectable before launch-facing code runs
  from it.
- Manifest data should be visible and explainable. Prefer `plugin.json`; a
  `package.json` fallback can identify legacy or imported roots, but it should
  not hide what the local folder contains.
- `scripts`, `scriptlets`, `skills`, `profiles`, and compatibility `agents`
  should remain normal local artifacts that can be edited, versioned, removed,
  or migrated.
- Update, remove, and rollback flows should make the affected local folder and
  manifest clear before they change user-owned files.
- Distribution and broader marketplace mechanics are deferred until they can
  preserve inspectability, editability, rollback, and removal instead of
  obscuring them.

### Profiles Are Runtime Boundaries

A profile is the selected AI runtime boundary. It defines the prompt, model,
tools, cwd, session behavior, logs, and receipts for Agent Chat.

A profile is not a hidden agent, magic persona, or vague preset. Keep launch
profile selection explicit, attributable, and testable. Durable AI behavior
should become explicit local profile artifacts rather than hidden configuration
blobs.

If future work adds profile handoffs or subagent-like flows, the UI, logs, and
receipts must show who acted and why.

#### AI Claim Levels

AI trust language should distinguish what the app actually enforces from what it
validates, prompts, or hopes to support later.

| Claim level | Meaning | Product language rule |
| --- | --- | --- |
| Enforced | Source, runtime, native, or wrapper-level behavior prevents the action and receipts prove it. | Use strong safety language only at this level. |
| Validated | Config, schema, or artifact checks reject invalid inputs before launch or runtime selection. | Say the artifact or configuration is validated, not sandboxed. |
| Prompt-policy or advisory | The profile prompt, tool posture, path policy, or blocked-action message guides behavior without filesystem enforcement. | Be explicit that this is policy posture, not native enforcement. |
| Receipted | Logs, transcripts, state, or action receipts show what happened and who acted. | Use receipts to support attribution and auditability. |
| Future aspiration | A claim depends on future Pi, native, or wrapper-level enforcement work. | Keep it out of launch promises until source and receipts prove it. |

### Skills Are Recipes, Not Ambient Magic

Skills should be explicit task recipes that can be invoked inside Agent Chat or
profile flows. They should not silently change runtime behavior without UI/log
attribution.

Schema and runtime design should keep skills understandable: a skill says how to
do a task; a profile says what runtime boundary the task runs inside.

### Automation Must Be Semantic And Receipted

Prefer `getState`, `getElements`, `getLayoutInfo`, `waitFor`, `batch`, semantic
IDs, action receipts, transcripts, and logs over sleeps, coordinates,
screenshot-only proof, or unrestricted keyboard/mouse injection.

Automation should be legible to humans and agents through the same state model.
If a launch-facing surface cannot report what it is, what is focused, what is
selected, and what changed, it is not agent-ready.

Exploratory prototypes can use lighter proof while ideas are still forming. Once
a claim becomes launch-facing, receipts are non-negotiable.

The search agent loop is discover → inspect → act against the observed revision →
wait for an attributed change → inspect its completed frame → diagnose. An action
must retain the caller's expectation, not refresh away its stale-target guard.
Inspection does not advance a controlled fixture's logical clock. Provider gates
substitute only external IO; production grouping, input dispatch, selection,
rendering, and activation policy remain the exercised application path.
An accepted dispatch is not a completed effect. Report completion only when the
owning operation confirms success; a refused or unobserved effect must remain
distinct, even when the UI also shows an explanatory message.

Owned hidden-window evidence is the default for launcher search contracts: no
desktop activation, global input, or screen capture. Semantic state, naturally
scheduled drawing, and retained framebuffer samples are distinct proof levels.
Forced captures cannot prove notification causality. Live provider correctness,
WindowServer composition, native focus, and external-app effects require separate
authorized evidence and are not implied by an owned search receipt.

## AI Terminology

Use one product language layer even when implementation and compatibility names
remain in the source.

- Agent Chat is the user-facing AI chat product.
- A profile is the explicit Agent Chat runtime boundary: prompt, model, tools,
  cwd, session policy, context posture, logs, and receipts.
- A skill is a repeatable task recipe that runs inside an Agent Chat/profile
  runtime.
- A plugin is a local artifact container for scripts, scriptlets, skills,
  profiles, and compatibility agent artifacts.
- A legacy agent is an import or compatibility source. Durable behavior should
  become an explicit profile or skill before it is treated as launch-facing.
- Pi is backend/runtime implementation detail unless the UI is intentionally
  showing model, provider, or backend attribution.
- Agent Chat and `tab_ai` are compatibility or implementation names. They should not
  create a second user-facing AI chat concept.

## Source-Of-Truth Map

- Product/process docs: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GLOSSARY.md`,
  `.impeccable.md`. These define the public promise, agent process, UI map, and
  visual taste. They should align with this vision.
- Surface model: `src/main_sections/app_view_state.rs`. This backs surface
  families, contracts, footer ownership, and view identity.
- Main launcher: `src/render_script_list/mod.rs`. This owns the primary command
  spine and its keyboard/footer behavior.
- View routing/root shell: `src/main_sections/render_impl.rs`. This keeps
  surfaces inside one coherent main-window family.
- App state and launcher context: `src/main_sections/app_state.rs`. This backs
  launcher context, passive frames, and app-level state.
- Prompt protocol: `src/main_sections/prompt_messages.rs`. This defines prompt
  display, state, elements, layout, waits, batches, and inspection messages.
- Semantic elements/layout receipts: `src/app_layout/collect_elements.rs`,
  `src/app_layout/build_layout_info.rs`,
  `src/app_layout/build_component_bounds.rs`. These make the UI inspectable.
- Shared chrome/components: `src/components/main_view_chrome.rs`,
  `src/components/prompt_layout_shell.rs`,
  `src/components/minimal_prompt_shell.rs`,
  `src/components/prompt_container.rs`, `src/components/prompt_footer.rs`,
  `src/components/hint_strip.rs`. These keep surfaces visually and behaviorally
  aligned.
- Actions: `src/actions/**`, `src/render_builtins/actions.rs`. These back the
  universal discovery layer.
- Built-ins: `src/render_builtins/**`. Built-ins should model first-party use of
  the shared surface language.
- Agent Chat/profiles: `src/ai/agent_chat/**`, `src/ai/agent_chat/ui/**`,
  `src/app_impl/agent_handoff/**`. Product language should say Agent Chat even
  where compatibility implementation names remain.
- Plugin artifacts: `src/plugins/**`. Plugin profile artifacts are parsed and
  validated by the plugin profile module.
- Menu syntax: `src/menu_syntax/**`. This backs command/capture grammar.
- DevTools/automation receipts: `scripts/devtools/**`,
  `src/agentic_protocol_bus.rs`. These back target-scoped proof.

Keep these references source-aware but not brittle. Prefer subsystem ownership
over freezing exact type placement forever.

## Product Loop Proof Matrix

Use this matrix as the checklist for making a launch-facing claim falsifiable.
Source areas are intentionally subsystem-level; exact type placement can move.

| Loop step | Owning surfaces | Source areas | Representative checks | Runtime receipts |
| --- | --- | --- | --- | --- |
| Search | Main Menu Search, catalog sources, command grammar | `src/app_impl/filter_input_*`, `src/app_impl/filtering_cache.rs`, `src/main_sections/root_search_store.rs`, `crates/sk-protocol/src/search_contract.rs` | `./scripts/agentic/agent-cargo.sh test --locked -p sk-protocol --lib search_contract::tests`; owned `design run --scenario launcher-ranking-provider` with an immutable artifact | query/scope revision, exact provider run and publication policy, committed row content, complete finite schedule accounting |
| Choose | ScriptList rows, built-ins, profile rows, Actions rows | `src/main_sections/app_state.rs`, `src/components/unified_list_item/**`, `src/render_script_list/**`, semantic dispatch | shared `scripts/agentic/launcher-selection-stability-probe.ts` recipe through the owned launcher journey; surface-specific Actions/profile checks remain separate | automatic/anchored intent, selected semantic identity, actual GPUI input focus, preview/footer subject, natural completed frame and matching retained pixels |
| Prompt | Prompt APIs and prompt shells | `src/main_sections/prompt_messages.rs`, `src/render_prompts/**`, `src/components/prompt_*` | `./scripts/agentic/agent-cargo.sh test --test surface_contract_matrix_artifact_contract`, prompt-specific source checks | prompt state, focus owner, footer hints, `getLayoutInfo` |
| Act | Enter, Cmd+K Actions, Cmd+Enter Agent Chat, direct action execution | `src/actions/**`, `src/app_actions/**`, `src/app_impl/**` | `./scripts/agentic/agent-cargo.sh test --test actions_dialog_enter_routing_contract`, `./scripts/agentic/agent-cargo.sh test --test main_window_footer_surface_owner_contract` | action receipt, route stack, target identity, focus restoration |
| Automate | Protocol, MCP, DevTools, semantic transactions | `src/agentic_protocol_bus.rs`, `src/app_layout/**`, `scripts/devtools/**` | `./scripts/agentic/agent-cargo.sh test --test devtools_coverage_contract`, protocol wait/batch contracts | `getState`, `getElements`, `getLayoutInfo`, `waitFor`, `batch` |
| Verify | Receipts, transcripts, logs, generated contracts | `docs/ai/contracts/**`, `scripts/devtools/**`, `src/ai/**`, test contracts | `./scripts/agentic/agent-cargo.sh test --test pi_profile_artifact_contract`, `./scripts/agentic/agent-cargo.sh test --test pi_profile_launch_contract` | transcript export, logs, profile selection receipts, pass/fail status |
| Return | Dismissal, close/reopen, focus recovery, cleanup | `src/main_sections/**`, `src/app_impl/**`, `scripts/agentic/session.sh` | focused close/reopen and lifecycle checks, `git status --short` after proof | window target status, input freshness, previous surface, cleanup proof |

## Decision Rules

Build as a script when the user should own the TypeScript, dependencies, and
workflow.

Build as a prompt when the workflow needs structured input, selection, editing,
terminal output, forms, paths, drops, or chat-like interaction.

Build as a built-in when the feature requires native integration, privileged app
state, app-wide indexing, tight performance control, or a first-party reference
implementation.

Build as a plugin when the feature should be shareable and carry scripts,
scriptlets, skills, profiles, or compatibility agent artifacts together.

Build as a profile when the main distinction is AI runtime identity: prompt,
tools, cwd, model, session policy, ambient-resource posture, or warm/runtime
behavior.

Build as a skill when it is a repeatable AI task recipe that should run inside an
existing profile instead of defining a new runtime.

Expose through Main Menu Search when the object is first-class and
runnable/selectable: scripts, built-ins, profiles, command grammar heads, or
capture targets.

Expose through Actions when the user is asking what can be done with the current
context.

Do not add persistent chrome for every feature.

Use mini mode when the row name is enough to choose confidently. Use expanded
mode when preview is required.

Route AI product language through Agent Chat. Do not create a second user-facing
AI chat concept because compatibility implementation names exist.

New launcher-family surfaces should join the shared surface contract, shared
chrome, footer ownership, semantic elements, and layout receipt model.

### Decision Examples

- Build as a script when the user wants to own the TypeScript and dependencies,
  such as a personal deploy helper, report generator, or clipboard formatter.
- Build as a prompt when the workflow needs structured native input, selection,
  editing, terminal output, forms, paths, drops, hotkeys, or chat-like
  interaction.
- Build as a built-in when the feature owns privileged app state, global
  indexing, native integration, or a first-party reference surface such as
  Clipboard History, App Launcher, Window Switcher, or Notes.
- Build as a plugin when scripts, scriptlets, skills, profiles, and compatibility
  agent artifacts should travel together as local files.
- Build as a profile when the difference is AI runtime identity: prompt, model,
  tools, cwd, session policy, context posture, or warm behavior.
- Build as a skill when the value is a repeatable task recipe that should run
  inside an existing profile instead of defining a new runtime boundary.
- Expose through Main Menu Search when the object is first-class, runnable, or
  selectable.
- Expose through Actions when the operation depends on the currently selected
  row, prompt, profile, built-in, Agent Chat thread, or artifact.
- Anti-example: do not add persistent footer chrome for every feature just
  because the operation is useful.
- Anti-example: do not hide profile handoffs, call prompt-policy path posture a
  sandbox, or turn Agent Chat/`tab_ai` compatibility names into new product concepts.

## Migration Model

Old Script Kit users should not expect drop-in compatibility. What carries
forward is the spirit: fast scripting, prompt APIs, keyboard-first automation,
and local control.

What changes is the SDK shape. Script Kit GPUI should keep the core prompt model
sharp and let users bring dependencies through Bun instead of restoring a giant
bundled helper global.

Migration ladder:

- Existing script idea -> TypeScript script under the plugin workspace.
- Old helper dependency -> explicit Bun package import.
- Reusable UI flow -> prompt APIs.
- Reusable capture or text grammar -> menu syntax or scriptlet artifact.
- Reusable AI task -> skill.
- Distinct AI runtime or policy -> profile artifact.
- Shareable package -> plugin repo.
- Legacy agent -> compatibility/import input that becomes an explicit profile
  when durable Agent Chat behavior is needed.

## Launch Standards

Fast is not optional.

Footer bloat is launch debt.

One-off UI is launch debt.

Mismatched prompt, built-in, and Agent Chat chrome is launch debt.

Any feature that cannot be discovered through Main Menu Search or Actions needs
a reason.

Any launch-facing surface that cannot produce semantic elements and layout
receipts is not agent-ready.

Any AI/profile claim that cannot be backed by runtime receipts should be
softened until it is true.

Screenshots can support visual debugging, but they are not proof by themselves.
Target identity, semantic state, layout state, and interaction receipts come
first.

## Risks

- Category drift: the product can collapse into "just another launcher" if local
  scripts, prompts, profiles, and receipts stop being the center.
- Distribution drift: shareable plugins can become opaque if install flows hide
  local files instead of making them inspectable.
- AI overclaim: profiles can sound safer than they are if docs imply sandboxing
  beyond current enforcement.
- Hidden-agent drift: profile handoffs and subagent-like flows can become
  untrustworthy if attribution, logs, and receipts are missing.
- Source-map staleness: exact paths will move. Keep subsystem intent current.
- UI islands: every one-off renderer weakens the native product family.
- Receipt gaps: launch-facing claims without semantic proof will erode trust.

## Anti-Goals

Do not become a launcher clone with scripts bolted on. The goal is programmable
local automation with native command speed.

Do not become a marketplace-first product whose local scripts, profiles, and
skills are secondary implementation details. Distribution should preserve local
ownership.

Do not recreate the old giant helper SDK just to reduce migration pain. Bring
dependencies through Bun and keep the prompt model sharp.

Do not become a web dashboard in native clothing. Script Kit should appear,
accept intent, act, and disappear.

Do not let Agent Chat become multiple user-facing chat products. Keep product
language singular even where compatibility implementation names remain.

Do not introduce hidden background subagents without explicit profile handoff UI,
logs, attribution, and receipts.

Do not claim filesystem sandboxing until native or wrapper-level enforcement
exists and receipts prove it. Current profile `pathPolicy` language should stay
honest about advisory/validation/prompt-policy boundaries unless enforcement
changes.

Do not make coordinate-based automation or unrestricted keyboard/mouse injection
the primary automation path before semantic contracts are complete.

Do not let built-ins drift into bespoke UI islands.

## Verification Expectations

Use the smallest check that can fail. Route cargo through
`./scripts/agentic/agent-cargo.sh`, not bare `cargo`.

Representative source checks:

```bash
./scripts/agentic/agent-cargo.sh check --lib
./scripts/agentic/agent-cargo.sh test --lib render_script_list_footer_tests
./scripts/agentic/agent-cargo.sh test --lib actions_dialog
./scripts/agentic/agent-cargo.sh test --test simulate_key_cmd_enter_scriptlist_contract
./scripts/agentic/agent-cargo.sh test --test pi_profile_artifact_contract
./scripts/agentic/agent-cargo.sh test --test pi_profile_launch_contract
./scripts/agentic/agent-cargo.sh test --test agent_chat_profile_selector_contract
./scripts/agentic/agent-cargo.sh test --test devtools_coverage_contract
```

Representative runtime receipts:

- Target identity through automation window inspection.
- Surface contract evidence from `getState`.
- Semantic IDs, focus, selected item, footer buttons, warnings, and truncation
  from `getElements`.
- Header/input/main/footer/preview geometry from `getLayoutInfo`.
- Deterministic `waitFor` + `batch` transactions instead of sleeps.
- Agent Chat state receipts, composer identity, profile selection receipts,
  allowed/blocked transcripts, and export receipts where relevant.
- Visual proof only after target, semantic, and layout identity agree.

## The Product Test

When evaluating a launch-facing change, ask:

- Does this strengthen the loop of search, choose, prompt, act, automate,
  verify, and return?
- Does it preserve launcher speed and keyboard confidence?
- Does it reuse the shared surface language instead of creating a new island?
- Is discovery in Main Menu Search or Actions?
- Can a human inspect the local artifact or understand the runtime boundary?
- Can an agent inspect the semantic state and produce a receipt?
- Are the claims honest about what the current runtime can enforce?

If the answer is no, the change may still be worth making, but it needs an
explicit product reason and a verification plan.
