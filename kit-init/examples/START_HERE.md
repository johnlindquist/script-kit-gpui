# Script Kit Example Starter

This examples plugin ships one runnable script: `scripts/todo-app.ts`.

## Script

Fastest local path: open Script Kit's **Script Template Catalog**, choose a starter, name it, and create a local TypeScript script in `~/.scriptkit/plugins/main/scripts/`.

Copy from: `scripts/todo-app.ts`
Write to: `~/.scriptkit/plugins/main/scripts/<name>.ts`

For Agent Chat script authoring, read `~/.scriptkit/plugins/scriptkit/skills/new-script/SKILL.md`. Writing the file is not enough. Syntax-check and run the script in the current terminal before reporting success.

Use `home(...)` for user-relative paths when you need to read or write files in the user's workspace.

## Prompt API Sequencing

Script Kit prompt APIs are stateful. Never use `Promise.all`, `Promise.race`, `Promise.any`, or `Promise.allSettled` with `arg()`, `fields()`, `select()`, `editor()`, `div()`, `path()`, or `confirm()`.

Do this:

```ts
const url1 = await arg("URL 1");
const url2 = await arg("URL 2");
const url3 = await arg("URL 3");
```

Do not start multiple prompts concurrently.

Regular TypeScript scripts receive the interactive SDK transport. TypeScript
scriptlets do not receive an interactive stdin pipe, so prompts such as
`arg()`, `fields()`, `mini()`, and `hotkey()` belong in a regular script. Shell
and Python scriptlets do not receive SDK globals. Search **SDK Reference** in
the launcher to inspect supported APIs, permissions, and migration alternatives
before choosing a starter.

For local files, use `readFile(path, encoding?)` and
`writeFile(path, contents, encoding?)` with explicit paths. For subprocesses,
prefer `exec(binary, [arg1, arg2])`: it never starts a shell and rejects shell
pipes, redirects, and command substitution. `paste()` reads clipboard text; it
does not inject a system-wide paste shortcut.

AI state readers such as `aiIsOpen()`, `aiGetActiveChat()`, and
`aiGetConversation()` do not request provider inference. `aiSendMessage()` does,
and `aiStartChat()` may unless response generation is explicitly disabled.

## Mandatory Script Verification

Use `bun build` for a non-running syntax/bundle check, then use `SK_VERIFY=1` for a non-interactive execution check.

```bash
bun build ~/.scriptkit/plugins/main/scripts/<name>.ts --target=bun --outfile ~/.scriptkit/tmp/test-scripts/<name>.verify.mjs
```

```bash
SK_VERIFY=1 bun ~/.scriptkit/plugins/main/scripts/<name>.ts
```

If the script normally needs UI or typed input, add an `SK_VERIFY=1` branch first so the Bun execution step is non-interactive. If either command fails, fix the script and rerun both commands. Do not report success until both commands pass.

## Copy Command

```bash
cp ~/.scriptkit/plugins/examples/scripts/todo-app.ts ~/.scriptkit/plugins/main/scripts/my-todo-app.ts
```
