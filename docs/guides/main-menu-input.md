# Main Menu Input

The launcher input is a search box first, but a small set of sigils turns it into a prompt builder, capture composer, or filtered search — without leaving the main list. The parser is conservative: plain text, URLs, `localhost:3000`, `hello@world.com`, and unknown heads all stay normal search.

## Quick Reference

| Input | Mode |
| --- | --- |
| `deploy` | Normal fuzzy search across scripts, built-ins, apps, and passive sources |
| `@selection`, `@file:readme` | Attach context to the prompt you're building |
| `/rewrite` | Slash command |
| `.professional` | Writing style |
| `\|creative` | AI profile |
| `>` , `>:dev` | Working-directory (cwd) picker |
| `todo; buy milk` | Capture (canonical postfix form) |
| `;todo buy milk`, `+todo buy milk` | Capture (prefix form; `+` is a legacy alias) |
| `note: Decision made` | Capture via keyword head (known targets only) |
| `:type:script deploy` | Filtered search |
| `~` / `~/Downloads` | Exit to File Search |
| `!` | Exit to Quick Terminal |
| `?` | Actions help |

## Prompt-Builder Sigils

`@`, `/`, `|`, `.`, and `>` each claim the sigil plus one word; everything else stays free text, so you can compose:

```text
>:dev @file:README.md /rewrite .concise explain the setup section
```

- **`@` context mentions** — `@selection`, `@clipboard`, `@file:notes.md`. The `@type:query` form sub-searches inside one source. Available sub-search prefixes include `file`, `project`, `clipboard`, `browser-history`, `notes`, `history` (Agent Chat history), `scripts`, `scriptlets`, `skills`, `dictation`, and `calendar`.
- **`/` slash commands** — pick a command from the slash catalog.
- **`.` styles** — built-in styles are `.professional`, `.concise`, `.friendly`, `.direct`; user-defined styles extend the catalog.
- **`|` profiles** — switch which AI profile handles the prompt.
- **`>` project cwd** — `>` alone opens the cwd picker rows (recents + browse); `>:dev` filters them; `>:~/dev/` browses a directory. The resolved cwd shows as a footer chip. `>name field:value -- args` also invokes a registered command head with argv passed after `--`.

A sigil only counts at a word boundary — `hello@world.com` is plain search.

## Keyboard Behavior

- `Enter` — run the current eligible selected row, or accept the highlighted picker row when a picker is open. A stale query or missing row is refused; Enter never silently substitutes a neighboring result.
- `Tab` — with typed text: **Quick AI** (opens the AI window with your query for review). With empty input: the **cwd picker**. Directory-browse queries keep `Tab` for path completion.
- `Shift+Tab` — open the agent **model/profile picker**.
- `⌘K` — actions for the selected row or current mode.
- `Escape` — closes the active picker/popup first, then clears/dismisses the launcher.

### Selection while results arrive

- Before you deliberately choose a row, selection follows the **first eligible result**. An asynchronous result that ranks above the old first row becomes selected automatically.
- Arrow navigation, clicking a row, or an agent selecting its semantic identity anchors that row. New results can change its position without changing your choice. Deliberately choosing the already-first row also anchors it.
- Changing the query restores automatic selection. If an anchored row disappears, selection falls back to the first eligible result; an empty or status-only list has no selected result.
- Empty `@source:` sub-searches are initially unarmed: the first Down arrow selects the first eligible row, not the second. Headers and layout-only slots are never choices.
- Unavailable explanatory choices may remain selectable for inspection, but cannot run.
- Result selection does not take keyboard focus away from the input. Refreshes do not undo deliberate scrolling. The preview, footer, actions, and Enter refer to the same current subject.
- `~` File Search keeps its own automatic-first/user-locked-path policy; it is not the launcher's query state.

## Capture

Capture turns the input into a structured local-data composer. The canonical spelling is postfix — type the target, then `;`:

```text
todo; Renew passport #errands p1
note; "Decision: ship parser first" #project
link; https://zed.dev #rust
cal; Design review start:"friday 2pm" for:45m
snippet; update @snippet:fetch-json -- const value = 1
```

Prefix (`;todo ...`), legacy `+todo ...`, and keyword (`todo: ...`) forms parse the same way. A bare `;` (or `+`) opens the **capture target picker** (the trigger picker), which suggests targets and can scaffold a handler script for a new target.

Built-in targets: `todo`, `note`, `link`, `snippet`, `cal`, `social`. Todo aliases: `reminder`, `snooze`, `defer`. `mcal` (macOS Calendar) is parser-known but hidden from the picker. Unknown heads like `github;` stay plain search until a script registers that target through metadata.

Capture body fields:

| Token | Meaning |
| --- | --- |
| plain words | Body text |
| `#tag` | Tags |
| `p1`–`p4` | Priority |
| `https://...` or `url:<value>` | URL |
| `due:`, `at:`, `start:`, `end:` | Date phrases (quoted phrases allowed: `due:"tomorrow 3pm"`) |
| `for:45m` | Duration |
| `key=value` | Custom metadata |
| `@name` / `@todo:id` | Reference an existing object (e.g. update a snippet) |

Incomplete captures don't run: `todo;` needs a body, `link;` needs a URL, `cal;` needs a body and a date. The hint row tells you what's missing.

## Filtered Search

Type `:` to open the filter picker, or type a complete filter directly. Filters compose with search words:

```text
:type:script deploy
:tag:work notes
:shortcut:any
:has:shortcut
:source:main inbox
:plugin:main.todo
:name:deploy  :desc:database  :alias:db
:meta.domain.kind:calendar
:#work            (sugar for tag:work)
:-type:app triage (leading - negates)
```

Qualifier heads: `type:`/`kind:`, `tag:`, `shortcut:` (`any`, `none`, or a literal like `cmd+k`), `source:`, `plugin:`, `name:`, `desc:`/`description:`, `alias:`, `has:`, and `meta.<path>:`. `type:` values cover scripts, scriptlets, skills, builtins, apps, windows, files, notes, todos, clipboard, tabs, history, conversations, vault, dictation, fallbacks, and issues.

A top-level `#work` (without `:`) remains normal search; inside a capture body it becomes a tag.

> **Planned, not shipped:** unified source heads like `files:`/`f:`, `notes:`/`n:`, `clipboard:`/`c:` are defined in the grammar but marked *planned* in the code. Don't rely on them yet — use `:type:` filters or the dedicated built-ins (File Search, Notes, Clipboard History) instead.

## Mode Exits

Three sigils leave the main-list grammar entirely:

- `~` or `~/path` — open **File Search** rooted at home/that path.
- `!` — open **Quick Terminal** for a shell command.
- `?` — open **Actions Help**.

## Safe Boundaries

- `localhost:3000`, `C#`, `hello!` — normal search.
- `hello world; not a capture` — a `;` after multiple words is not a capture head.
- `https://example.com; nope` — URLs disqualify the capture head.
- Unknown `;target` / `+target` / `target:` heads stay searchable.

When in doubt, type normally first; opt into a mode with a sigil only when you want it.

## Related

- [Feature Tour](./feature-tour.md) — where these modes fit in the app
- [SDK Scripting](./sdk-scripting.md) — registering capture handlers and command heads from scripts
