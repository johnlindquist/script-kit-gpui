#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AGY_BIN="${AGY_BIN:-/Users/johnlindquist/.local/bin/agy}"

SUBCOMMAND="${1:-help}"
if [[ "$SUBCOMMAND" == --* ]]; then
  SUBCOMMAND="run"
else
  shift || true
fi

PROMPT_TEXT=""
PROMPT_FILE_ARG=""
SURFACE_HINT="auto"
TARGET_HINT="auto"
MODE="inspect"
SESSION_NAME=""
EXPECTED_SESSION_PID=""
EXPECTED_SESSION_GENERATION=""
OUT_ROOT="${PROJECT_ROOT}/.test-output/agy-devtools"
RUN_ID=""
PRINT_TIMEOUT="5m"
TIMEOUT_MS=8000
DRY_RUN=0
FAST_MODE=0
TRUST_REPO=0
AGY_SANDBOX="on"
ALLOW_ACT=0
ALLOW_SUBMIT=0
ALLOW_NATIVE=0
ALLOW_MIC=0
ALLOW_REAL_DATA=0
KEEP_SESSION=0
START_APP=1
SHOW_APP=1
EXTRA_CONTEXT=()
RUN_DIR_ARG=""
if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]]; then
  START_APP=0
  SHOW_APP=0
fi

usage() {
  cat <<'EOF'
Usage:
  scripts/agentic/agy-devtools.sh <subcommand> [options]

Subcommands:
  run       Build prompt, invoke agy, save logs/results, and print compact output.
  infer     Infer surface, target, safety gates, and primitive stack as JSON.
  prompt    Build and print the exact agy prompt without invoking agy.
  compact   Print a compact summary for an existing run directory.
  cleanup   Stop a named wrapper-owned DevTools session.
  help      Show this help.

Compatibility:
  scripts/agentic/agy-devtools.sh --prompt <text> still behaves as `run`.

Required for run/infer/prompt:
  --prompt <text>          User bug report or inspection request.
  --prompt-file <path>     Read the user request from a file.

Core options:
  --surface <id>           auto, main, actions-dialog, notes, dictation, prompt, agent_chat, portal, theme, storybook.
  --target <target>        auto, main, focused, id:<automation-id>, kind:<target-kind>, title:<substring>.
  --mode <mode>            plan, inspect, act, full. Default: inspect.
  --session <name>         DevTools session hint. Default: agy-devtools-<timestamp>-<pid>.
  --expected-pid <pid>     Exact owned PID from an isolated-start receipt; required for cleanup.
  --expected-generation <generation>
                           Exact owned generation from that same receipt; required for cleanup.
  --out-dir <path>         Output root. Default: .test-output/agy-devtools.
  --run-id <id>            Stable run id. Default: timestamp + surface + prompt hash.
  --run-dir <path>         Existing run dir for compact/cleanup-oriented commands.
  --timeout-ms <n>         DevTools primitive timeout hint. Default: 8000.
  --print-timeout <dur>    agy --print-timeout value. Default: 5m.
  --agy-bin <path>         agy binary. Default: /Users/johnlindquist/.local/bin/agy.
  --add-dir <path>         Extra workspace directory for agy. Repeatable.

agy permission options:
  --agy-sandbox on|off     Default: on.
  --trust-repo             Pass --dangerously-skip-permissions to agy.
  --no-trust-repo          Default.
  --auto-approve           Alias for --trust-repo.

Safety gates:
  --allow-act              Permit safe protocol-first non-submit actions.
  --allow-actions          Alias for --allow-act.
  --allow-submit           Permit submit/Enter flows when explicitly needed.
  --allow-native           Permit native input escalation when explicitly needed.
  --allow-mic              Permit live microphone flows.
  --allow-real-data        Permit real-data inspection or mutation.

Session options:
  --start / --no-start     Hint whether a real app session may be started. Default for run: --start.
  --show / --no-show       Hint whether UI should be visible. Default for run: --show.
  --keep-session           Do not ask agy to clean up sessions it creates.
  --cleanup                Ask agy to clean up wrapper-created sessions. Default.
  --dry-run                For run: write input/inference/prompt, print next command, do not invoke agy.
  --fast                   Use a short command-budget prompt for known action flows.
  -h, --help               Show this help.
EOF
}

die() {
  printf 'agy-devtools error: %s\n' "$1" >&2
  exit "${2:-2}"
}

slugify() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-+/-/g' \
    | cut -c 1-48
}

prompt_hash() {
  printf '%s' "$1" | shasum -a 1 | awk '{print substr($1, 1, 8)}'
}

load_prompt() {
  if [[ -n "$PROMPT_FILE_ARG" ]]; then
    [[ -f "$PROMPT_FILE_ARG" ]] || die "prompt file not found: $PROMPT_FILE_ARG"
    PROMPT_TEXT="$(<"$PROMPT_FILE_ARG")"
  fi
  [[ -n "$PROMPT_TEXT" ]] || die "--prompt or --prompt-file is required"
}

parse_common_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --prompt) PROMPT_TEXT="${2:-}"; shift 2 ;;
      --prompt-file) PROMPT_FILE_ARG="${2:-}"; shift 2 ;;
      --surface) SURFACE_HINT="${2:-auto}"; shift 2 ;;
      --target) TARGET_HINT="${2:-auto}"; shift 2 ;;
      --mode) MODE="${2:-inspect}"; shift 2 ;;
      --session) SESSION_NAME="${2:-}"; shift 2 ;;
      --expected-pid) EXPECTED_SESSION_PID="${2:-}"; shift 2 ;;
      --expected-generation) EXPECTED_SESSION_GENERATION="${2:-}"; shift 2 ;;
      --out-dir) OUT_ROOT="${2:-}"; shift 2 ;;
      --run-id) RUN_ID="${2:-}"; shift 2 ;;
      --run-dir) RUN_DIR_ARG="${2:-}"; shift 2 ;;
      --timeout-ms) TIMEOUT_MS="${2:-8000}"; shift 2 ;;
      --print-timeout|--timeout) PRINT_TIMEOUT="${2:-5m}"; shift 2 ;;
      --agy-bin) AGY_BIN="${2:-}"; shift 2 ;;
      --add-dir) EXTRA_CONTEXT+=("${2:-}"); shift 2 ;;
      --agy-sandbox) AGY_SANDBOX="${2:-on}"; shift 2 ;;
      --trust-repo|--auto-approve) TRUST_REPO=1; shift ;;
      --no-trust-repo) TRUST_REPO=0; shift ;;
      --allow-act|--allow-actions) ALLOW_ACT=1; shift ;;
      --allow-submit) ALLOW_SUBMIT=1; shift ;;
      --allow-native) ALLOW_NATIVE=1; shift ;;
      --allow-mic) ALLOW_MIC=1; shift ;;
      --allow-real-data) ALLOW_REAL_DATA=1; shift ;;
      --start) START_APP=1; shift ;;
      --no-start) START_APP=0; shift ;;
      --show) SHOW_APP=1; shift ;;
      --no-show) SHOW_APP=0; shift ;;
      --keep-session) KEEP_SESSION=1; shift ;;
      --cleanup) KEEP_SESSION=0; shift ;;
      --dry-run) DRY_RUN=1; shift ;;
      --fast) FAST_MODE=1; shift ;;
      -h|--help) usage; exit 0 ;;
      *) die "unknown argument: $1" ;;
    esac
  if [[ "${SCRIPT_KIT_NONINTERACTIVE:-0}" == "1" ]] \
    && (( START_APP || SHOW_APP || ALLOW_NATIVE || ALLOW_MIC || ALLOW_REAL_DATA || ALLOW_SUBMIT )); then
    die 'noninteractive agy cannot claim ordinary startup, visible/native/device/private-data/live submission authority; use the owned evaluator client'
  fi
  done
}

make_run_dir() {
  local surface_slug prompt_slug hash timestamp
  timestamp="$(date +%Y%m%d-%H%M%S)"
  surface_slug="$(slugify "$SURFACE_HINT")"
  prompt_slug="$(slugify "$PROMPT_TEXT")"
  hash="$(prompt_hash "$PROMPT_TEXT")"
  [[ -n "$surface_slug" ]] || surface_slug="auto"
  [[ -n "$prompt_slug" ]] || prompt_slug="prompt"
  if [[ -z "$RUN_ID" ]]; then
    RUN_ID="${timestamp}-${surface_slug}-${hash}-${prompt_slug}"
  fi
  printf '%s/%s\n' "${OUT_ROOT%/}" "$RUN_ID"
}

write_inference() {
  local output_path="$1"
  python3 - "$output_path" "$PROJECT_ROOT" "$PROMPT_TEXT" "$SURFACE_HINT" "$TARGET_HINT" "$MODE" "${SESSION_NAME}" "$TIMEOUT_MS" "$START_APP" "$SHOW_APP" "$KEEP_SESSION" "$ALLOW_ACT" "$ALLOW_SUBMIT" "$ALLOW_NATIVE" "$ALLOW_MIC" "$ALLOW_REAL_DATA" <<'PY'
import json
import re
import sys
from pathlib import Path

out, root, prompt, surface_hint, target_hint, mode, session, timeout_ms = sys.argv[1:9]
start, show, keep = [v == "1" for v in sys.argv[9:12]]
allow_act, allow_submit, allow_native, allow_mic, allow_real_data = [v == "1" for v in sys.argv[12:17]]
root_path = Path(root)
text = f"{surface_hint} {prompt}".lower()

def has(name: str) -> bool:
    return (root_path / "scripts" / "devtools" / name).is_file()

def norm(value: str) -> str:
    value = re.sub(r"([a-z])([A-Z])", r"\1-\2", value)
    return re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")

surface_map = {
    "actions-dialog": ("ActionsDialog", 0.95, ["explicit surface hint"]),
    "actions": ("ActionsDialog", 0.95, ["explicit surface hint"]),
    "main": ("ScriptList", 0.95, ["explicit surface hint"]),
    "launcher": ("ScriptList", 0.95, ["explicit surface hint"]),
    "notes": ("Notes", 0.95, ["explicit surface hint"]),
    "dictation": ("Dictation", 0.95, ["explicit surface hint"]),
    "prompt": ("PromptEntity", 0.95, ["explicit surface hint"]),
    "agent_chat": ("AgentChat", 0.90, ["explicit surface hint"]),
    "portal": ("Portal", 0.90, ["explicit surface hint"]),
    "theme": ("Theme", 0.90, ["explicit surface hint"]),
    "storybook": ("Storybook", 0.90, ["explicit surface hint"]),
}

hint = norm(surface_hint)
if hint in surface_map and hint != "auto":
    surface_id = "actions-dialog" if hint == "actions" else hint
    surface_kind, confidence, reasons = surface_map[hint]
elif re.search(r"\b(action|actions|cmd\+?k|command palette|popup|popover|route stack|deeplink|disabled action|clipped)\b", text):
    surface_id, surface_kind, confidence, reasons = "actions-dialog", "ActionsDialog", 0.88, ["matched actions/popup vocabulary"]
elif re.search(r"\b(note|notes|markdown|preview|autosize|dirty state|note editor|command bar)\b", text):
    surface_id, surface_kind, confidence, reasons = "notes", "Notes", 0.88, ["matched notes vocabulary"]
elif re.search(r"\b(dictation|mic|microphone|transcript|recording|audio|waveform|hotkey)\b", text):
    surface_id, surface_kind, confidence, reasons = "dictation", "Dictation", 0.88, ["matched dictation/media vocabulary"]
elif re.search(r"\b(arg prompt|form|fields|div|editor prompt|terminal prompt|prompt container)\b", text):
    surface_id, surface_kind, confidence, reasons = "prompt", "PromptEntity", 0.78, ["matched prompt-runtime vocabulary"]
elif re.search(r"\b(agent_chat|agent chat|composer|mention|slash command|thread)\b", text):
    surface_id, surface_kind, confidence, reasons = "agent_chat", "AgentChat", 0.78, ["matched agent_chat vocabulary"]
else:
    surface_id, surface_kind, confidence, reasons = "main", "ScriptList", 0.55, ["defaulted to main launcher"]

target_args = ["--main"]
target_selector = "main"
if target_hint == "focused":
    target_args, target_selector = ["--focused"], "focused"
elif target_hint.startswith("id:"):
    target_args, target_selector = ["--target-id", target_hint[3:]], target_hint
elif target_hint.startswith("kind:"):
    target_args, target_selector = ["--target-kind", target_hint[5:]], target_hint
elif target_hint.startswith("title:"):
    target_args, target_selector = ["--target-title", target_hint[6:]], target_hint
elif target_hint == "auto" and surface_id == "notes":
    target_args, target_selector = ["--target-kind", "notes"], "kind:notes"
elif target_hint == "auto" and surface_id == "dictation":
    target_args, target_selector = [], "dictation/passive"

commands = []
unavailable = []

def add(name, file_name, args, receipt):
    path = f"scripts/devtools/{file_name}"
    if has(file_name):
        commands.append({"name": name, "available": True, "command": args, "receipt": f"receipts/{receipt}"})
    else:
        unavailable.append({"name": name, "file": path, "reason": "missing devtools CLI file"})

if surface_id in {"main", "actions-dialog", "prompt", "agent_chat", "portal", "theme", "storybook"}:
    add("devtools.investigate", "investigate.ts", ["bun", "scripts/devtools/investigate.ts", "--surface", surface_kind, "--bug", "<prompt>"], "010-investigate.json")
    inspect_args = ["bun", "scripts/devtools/inspect.ts", "--session", session, "--bug", "<prompt>", "--surface", surface_kind, *target_args]
    if start:
        inspect_args.insert(4, "--start")
    if show:
        inspect_args.insert(5 if start else 4, "--show")
    add("devtools.inspect", "inspect.ts", inspect_args, "020-inspect.json")
    add("devtools.coverage", "coverage.ts", ["bun", "scripts/devtools/coverage.ts", "--surface", surface_id], "030-coverage.json")
elif surface_id == "notes":
    add("devtools.coverage", "coverage.ts", ["bun", "scripts/devtools/coverage.ts", "--surface", "notes"], "010-coverage.json")
    if "resize" in text or "autosize" in text or "shrink" in text or "grow" in text:
        add("devtools.notes.resize-compare", "notes.ts", ["bun", "scripts/devtools/notes.ts", "resize-compare", "--start", "--sandbox"], "020-notes-resize.json")
    else:
        add("devtools.notes.inspect", "notes.ts", ["bun", "scripts/devtools/notes.ts", "inspect", "--open"], "020-notes-inspect.json")
    add("devtools.inspect", "inspect.ts", ["bun", "scripts/devtools/inspect.ts", "--session", session, "--target-kind", "notes", "--bug", "<prompt>", "--surface", "Notes"], "030-inspect.json")
elif surface_id == "dictation":
    add("devtools.coverage", "coverage.ts", ["bun", "scripts/devtools/coverage.ts", "--surface", "dictation"], "010-coverage.json")
    add("devtools.dictation.inspect", "dictation.ts", ["bun", "scripts/devtools/dictation.ts", "inspect"], "020-dictation-inspect.json")
    add("devtools.media.inspect", "media.ts", ["bun", "scripts/devtools/media.ts", "--coverage", "receipts/010-coverage.json"], "030-media.json")
    if ("delivery" in text or "wrong target" in text or "transcript goes" in text) and not allow_mic:
        add("devtools.dictation.fixture", "dictation.ts", ["bun", "scripts/devtools/dictation.ts", "deliver-fixture", "--target", "mainWindowFilter", "--fixture-id", "short-phrase"], "040-dictation-fixture.json")

if re.search(r"\b(focus|keyboard|shortcut|tab|escape|enter|cmd|hotkey)\b", text):
    add("devtools.focus", "focus.ts", ["bun", "scripts/devtools/focus.ts", "inspect", "--session", session, *target_args], "050-focus.json")
    add("devtools.keyboard", "keyboard.ts", ["bun", "scripts/devtools/keyboard.ts", "inspect", "--session", session, *target_args], "060-keyboard.json")

if re.search(r"\b(clipped|overlap|too tall|too wide|blank|hidden|screenshot|unreadable|contrast|wrap|layout|scroll)\b", text):
    add("devtools.layout", "layout.ts", ["bun", "scripts/devtools/layout.ts", "measure", "--session", session, *target_args], "070-layout.json")
    add("devtools.text", "text.ts", ["bun", "scripts/devtools/text.ts", "measure", "--session", session, *target_args], "080-text.json")
    add("devtools.scroll", "scroll.ts", ["bun", "scripts/devtools/scroll.ts", "inspect", "--session", session, *target_args], "090-scroll.json")

add("devtools.events.tail", "events.ts", ["bun", "scripts/devtools/events.ts", "tail", "--session", session, "--limit", "80"], "900-events.json")

blocked = []
if not allow_act:
    blocked.append("protocol actions")
if not allow_submit:
    blocked.append("submit-like activation")
if not allow_native:
    blocked.append("native input")
if not allow_mic:
    blocked.append("live microphone")
if not allow_real_data:
    blocked.append("real-data mutation")

result = {
    "schemaVersion": 1,
    "tool": "agy-devtools.infer",
    "userPrompt": prompt,
    "mode": mode,
    "surface": {"requested": surface_hint, "id": surface_id, "surfaceKind": surface_kind, "confidence": confidence, "reason": reasons},
    "target": {"requested": target_hint, "selector": target_selector, "targetArgs": target_args},
    "session": {"name": session, "start": start, "show": show, "keepSession": keep},
    "safety": {
        "allowAct": allow_act,
        "allowSubmit": allow_submit,
        "allowNative": allow_native,
        "allowMic": allow_mic,
        "allowRealData": allow_real_data,
    },
    "primitiveStack": commands,
    "unavailablePrimitives": unavailable,
    "blockedByDefault": blocked,
    "warnings": ["command availability is discovered from scripts/devtools/*.ts", "agy must treat this inference as a starting plan, not proof"],
    "timeoutMs": int(timeout_ms),
}

encoded = json.dumps(result, indent=2)
if out == "-":
    print(encoded)
else:
    Path(out).write_text(encoded + "\n")
PY
}

build_prompt() {
  local inference_path="$1"
  local output_path="$2"
  python3 - "$inference_path" "$output_path" "$PROJECT_ROOT" "$PROMPT_TEXT" "$SESSION_NAME" "$RUN_DIR" "$RECEIPTS_DIR" "$FAST_MODE" <<'PY'
import json
import sys
from pathlib import Path

inference_path, output_path, root, prompt, session, run_dir, receipts_dir, fast_mode = sys.argv[1:9]
inference = json.loads(Path(inference_path).read_text())
fast = fast_mode == "1"
primitive_lines = []
for item in inference.get("primitiveStack", []):
    command = " ".join(item.get("command", []))
    primitive_lines.append(f"- {item['name']}: `{command}` -> `{item['receipt']}`")
unavailable = []
for item in inference.get("unavailablePrimitives", []):
    unavailable.append(f"- {item['name']}: unavailable because {item['reason']} ({item['file']})")

if fast:
    agent_chat_fast_path = ""
    theme_fast_path = ""
    if inference.get("surface", {}).get("id") == "agent_chat":
        agent_chat_fast_path = f"""
Agent Chat inspection path:
1. Inspect the explicitly supplied existing session without --start or --show.
2. Query current semantic elements and exact target generations; never hardcode a row index or guess a semantic ID.
3. Exercise provider-free failure/retry/Stop only through the owned evaluator's declared conversation fixture controls.
4. Do not type or submit a live AI prompt, start a sidecar, or escalate to native input.
5. Retain correlated terminal receipts and detach from borrowed sessions.
"""
    if inference.get("surface", {}).get("id") == "theme":
        theme_fast_path = """
Theme inspection path:
1. Inspect the existing target without visible startup or native escalation.
2. Use the owned design client's revision-bound theme preview/revert transaction for a requested change.
3. Preserve locked glass/motion values and report the actual publication revision and completed frame.
"""

    text = f"""You are agy running in FAST Script Kit GPUI DevTools mode.

User request:
{prompt}

Repo root: {root}
Run directory: {run_dir}
Receipts directory: {receipts_dir}

Hard constraints:
- Do not read source files, skill files, docs, or tests unless one of the commands below fails.
- Do not broaden into a general investigation.
- Execute at most 8 shell commands before writing the result files.
- Use protocol-first DevTools commands only. No native input.
- Safety gates: allowAct={inference['safety']['allowAct']}, allowSubmit={inference['safety']['allowSubmit']}, allowNative={inference['safety']['allowNative']}, allowMic={inference['safety']['allowMic']}, allowRealData={inference['safety']['allowRealData']}.
- If a command is blocked, stop, write result.json with that classification, and do not invent a workaround unless it is listed here.

Wrapper inference:
```json
{json.dumps(inference, indent=2)}
```
{agent_chat_fast_path}
{theme_fast_path}
Fallback primitive stack if this is not the Agent Chat path:
{chr(10).join(primitive_lines) if primitive_lines else "- none"}

Required final files:
- `{run_dir}/result.json`
- `{run_dir}/result.md`
- `{run_dir}/compact.md`

Final result.json must include: classification, surface, target, primitiveStack, findings, missingPrimitives, likelyOwners, warnings, cleanup.

Print a compact Markdown result after writing files.
"""
else:
    text = f"""You are agy running as a fast Script Kit GPUI DevTools inspector.

User request:
{prompt}

Repo root: {root}
Run directory: {run_dir}
Receipts directory: {receipts_dir}

Wrapper inference:
```json
{json.dumps(inference, indent=2)}
```

Use this inference as a starting plan, not as proof. Your job is to inspect the real app through the existing Script Kit DevTools layer and report honest classifications.

Primitive menu selected for this prompt:
{chr(10).join(primitive_lines) if primitive_lines else "- none"}

Unavailable primitive candidates:
{chr(10).join(unavailable) if unavailable else "- none"}

Rules:
1. Prefer protocol-first DevTools commands under `scripts/devtools/` and `scripts/agentic/devtools-session.sh`.
2. Write every JSON receipt you create under `{receipts_dir}`.
3. Keep the run fail-closed. If target identity, app visibility, strict bounds, or a required primitive is missing, classify the result precisely.
4. Default to inspect-only. Do not submit forms, press Enter to launch selected items, trigger destructive built-ins, read/mutate real private data, use live microphone, or use native input unless the matching safety gate in inference.safety is true.
5. Do not use screenshots as a substitute for semantic/layout/focus proof.
6. Do not call a recipe green proof unless direct DevTools receipts prove the same user-path symptom.
7. Do not edit source files unless the user request explicitly asks for implementation.
8. If you create a DevTools session and keepSession is false, clean up only the session you created. Do not stop dev-watch or another user's existing session.

Create these final files when possible:
- `{run_dir}/result.json`
- `{run_dir}/result.md`
- `{run_dir}/compact.md`

`result.json` schema:
```json
{{
  "schemaVersion": 1,
  "tool": "agy-devtools.result",
  "classification": "reproduced | not-reproduced | fixed | needs-user-info | blocked-by-missing-primitive | blocked-by-target-identity | blocked-by-user-interruption | blocked-by-unsafe-operation | blocked-by-timeout | failed",
  "surface": {{"requested": "...", "inferred": "...", "resolved": "..."}},
  "target": {{"strict": false, "automationId": null, "targetKind": null, "surfaceKind": null}},
  "primitiveStack": [{{"name": "...", "command": "...", "receipt": "receipts/NNN-name.json", "classification": "..."}}],
  "findings": [],
  "missingPrimitives": [],
  "likelyOwners": [],
  "warnings": [],
  "cleanup": {{"createdSession": false, "command": null}}
}}
```

Final Markdown response format:

# agy DevTools Result

- classification:
- inferred_surface:
- target:
- commands_run:
- receipt_paths:
- key_findings:
- missing_primitives:
- likely_owner_files:
- next_verification:
- cleanup:

Keep the final answer compact. Include exact command lines and receipt paths.
"""

if output_path == "-":
    print(text)
else:
    Path(output_path).write_text(text)
PY
}

compact_print() {
  local run_dir="$1"
  python3 - "$run_dir" <<'PY'
import json
import sys
from pathlib import Path

run = Path(sys.argv[1])
result_json = run / "result.json"
result_md = run / "result.md"
agy_stdout = run / "agy.stdout.md"
agy_log = run / "agy.log"
receipts_dir = run / "receipts"

def load(path):
    try:
        return json.loads(path.read_text())
    except Exception as exc:
        return {"classification": "blocked-by-parse-error", "path": str(path), "error": str(exc)}

result = load(result_json) if result_json.exists() else {}
receipts = sorted(receipts_dir.glob("*.json")) if receipts_dir.exists() else []
loaded = [load(path) for path in receipts]
classes = {}
for item in loaded:
    cls = item.get("classification") or item.get("status") or "unknown"
    classes[cls] = classes.get(cls, 0) + 1
missing = []
for item in loaded + ([result] if result else []):
    for key in ("missingFieldDetails", "missingPrimitives"):
        values = item.get(key) if isinstance(item, dict) else None
        if isinstance(values, list):
            for value in values:
                if isinstance(value, dict):
                    missing.append((value.get("field") or value.get("primitive") or "unknown", value.get("nextPrimitive") or value.get("reason") or ""))
warnings = []
for item in loaded + ([result] if result else []):
    values = item.get("warnings") if isinstance(item, dict) else None
    if isinstance(values, list):
        warnings.extend(str(value) for value in values)

classification = result.get("classification") if result else "unknown"
if not result_json.exists() and agy_stdout.exists():
    classification = "agy-output-without-result-json"
elif not result_json.exists():
    classification = "blocked-by-missing-result-json"

lines = [f"agy-devtools: {classification}", f"run: {run}"]
surface = result.get("surface") if isinstance(result.get("surface"), dict) else {}
target = result.get("target") if isinstance(result.get("target"), dict) else {}
if surface:
    requested = surface.get("requested") or surface.get("id")
    inferred = surface.get("inferred") or surface.get("surfaceKind")
    resolved = surface.get("resolved") or surface.get("surfaceKind") or surface.get("id")
    lines.append(f"surface: requested={requested} inferred={inferred} resolved={resolved}")
if target:
    selector = target.get("selector")
    target_args = " ".join(target.get("targetArgs") or []) if isinstance(target.get("targetArgs"), list) else None
    target_detail = target.get("automationId") or selector or target_args
    lines.append(f"target: strict={target.get('strict')} automationId={target.get('automationId')} selector={target_detail} targetKind={target.get('targetKind')} surfaceKind={target.get('surfaceKind')}")
if classes:
    lines.append("receipts: " + ", ".join(f"{key}={value}" for key, value in sorted(classes.items())))
if missing:
    lines.append("missing:")
    for field, next_primitive in missing[:8]:
        lines.append(f" - {field} -> {next_primitive}")
if warnings:
    lines.append("warnings:")
    for warning in warnings[:5]:
        lines.append(f" - {warning}")
lines.extend([
    f"result: {result_md}",
    f"json: {result_json}",
    f"agy stdout: {agy_stdout}",
    f"agy log: {agy_log}",
    f"receipts: {receipts_dir}",
])
text = "\n".join(lines) + "\n"
(run / "compact.md").write_text(text)
print(text, end="")
PY
}

cmd_infer() {
  parse_common_args "$@"
  load_prompt
  [[ -n "$SESSION_NAME" ]] || SESSION_NAME="agy-devtools-$(date +%Y%m%d-%H%M%S)-$$"
  write_inference "-"
}

cmd_prompt() {
  parse_common_args "$@"
  load_prompt
  [[ -n "$SESSION_NAME" ]] || SESSION_NAME="agy-devtools-$(date +%Y%m%d-%H%M%S)-$$"
  RUN_DIR="${RUN_DIR_ARG:-$(make_run_dir)}"
  RECEIPTS_DIR="${RUN_DIR}/receipts"
  mkdir -p "$RECEIPTS_DIR"
  printf '%s\n' "$PROMPT_TEXT" >"${RUN_DIR}/input.txt"
  write_inference "${RUN_DIR}/inference.json"
  build_prompt "${RUN_DIR}/inference.json" "${RUN_DIR}/prompt.md"
  cat "${RUN_DIR}/prompt.md"
}

cmd_run() {
  parse_common_args "$@"
  load_prompt
  [[ -x "$AGY_BIN" ]] || die "agy binary is not executable: $AGY_BIN"
  [[ -n "$SESSION_NAME" ]] || SESSION_NAME="agy-devtools-$(date +%Y%m%d-%H%M%S)-$$"
  RUN_DIR="${RUN_DIR_ARG:-$(make_run_dir)}"
  RECEIPTS_DIR="${RUN_DIR}/receipts"
  mkdir -p "$RECEIPTS_DIR" "${RUN_DIR}/logs" "${RUN_DIR}/session"
  printf '%s\n' "$PROMPT_TEXT" >"${RUN_DIR}/input.txt"
  write_inference "${RUN_DIR}/inference.json"
  build_prompt "${RUN_DIR}/inference.json" "${RUN_DIR}/prompt.md"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf 'agy-devtools dry-run\n'
    printf 'run_dir=%s\n' "$RUN_DIR"
    printf 'input=%s\n' "${RUN_DIR}/input.txt"
    printf 'inference=%s\n' "${RUN_DIR}/inference.json"
    printf 'prompt=%s\n' "${RUN_DIR}/prompt.md"
    printf 'next=%s --print <prompt.md> --print-timeout %s --log-file %s --add-dir %s\n' "$AGY_BIN" "$PRINT_TIMEOUT" "${RUN_DIR}/agy.log" "$PROJECT_ROOT"
    python3 - "${RUN_DIR}/inference.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1]))
print(f"surface: {data['surface']['id']}")
print(f"target: {data['target']['selector']}")
print("safe defaults: no submit, no native, no mic, no real-data mutation")
print("primitiveStack:")
for item in data["primitiveStack"]:
    print(f" - {item['name']} -> {item['receipt']}")
if data["unavailablePrimitives"]:
    print("unavailable:")
    for item in data["unavailablePrimitives"]:
        print(f" - {item['name']} ({item['file']})")
PY
    exit 0
  fi

  AGY_ARGS=(
    "--print"
    "$(cat "${RUN_DIR}/prompt.md")"
    "--print-timeout" "$PRINT_TIMEOUT"
    "--log-file" "${RUN_DIR}/agy.log"
    "--add-dir" "$PROJECT_ROOT"
  )
  for context_dir in "${EXTRA_CONTEXT[@]}"; do
    [[ -n "$context_dir" ]] && AGY_ARGS+=("--add-dir" "$context_dir")
  done
  [[ "$AGY_SANDBOX" == "on" ]] && AGY_ARGS+=("--sandbox")
  [[ "$TRUST_REPO" -eq 1 ]] && AGY_ARGS+=("--dangerously-skip-permissions")

  set +e
  "$AGY_BIN" "${AGY_ARGS[@]}" >"${RUN_DIR}/agy.stdout.md" 2>"${RUN_DIR}/agy.stderr.log"
  AGY_STATUS=$?
  set -e

  cp "${RUN_DIR}/agy.stdout.md" "${RUN_DIR}/result.md"
  if [[ ! -f "${RUN_DIR}/result.json" ]]; then
    python3 - "${RUN_DIR}" "$AGY_STATUS" <<'PY'
import json, sys
from pathlib import Path
run = Path(sys.argv[1])
status = int(sys.argv[2])
result = {
    "schemaVersion": 1,
    "tool": "agy-devtools.wrapper-result",
    "classification": "agy-output-without-result-json" if status == 0 else "failed",
    "agyStatus": status,
    "surface": {},
    "target": {},
    "primitiveStack": [],
    "findings": ["agy did not create result.json; inspect agy.stdout.md for details"],
    "missingPrimitives": [],
    "likelyOwners": [],
    "warnings": ["wrapper-created fallback result.json"],
    "cleanup": {"createdSession": None, "command": None},
}
(run / "result.json").write_text(json.dumps(result, indent=2) + "\n")
PY
  fi

  compact_print "$RUN_DIR"
  if [[ "$AGY_STATUS" -ne 0 ]]; then
    exit "$AGY_STATUS"
  fi
}

cmd_compact() {
  parse_common_args "$@"
  [[ -n "$RUN_DIR_ARG" ]] || die "compact requires --run-dir <path>"
  compact_print "$RUN_DIR_ARG"
}

cmd_cleanup() {
  parse_common_args "$@"
  [[ -n "$SESSION_NAME" ]] || die "cleanup requires --session <name>"
  bash "${PROJECT_ROOT}/scripts/agentic/devtools-session.sh" cleanup --session "$SESSION_NAME" \
    --expected-pid "$EXPECTED_SESSION_PID" \
    --expected-generation "$EXPECTED_SESSION_GENERATION"
}

case "$SUBCOMMAND" in
  run) cmd_run "$@" ;;
  infer) cmd_infer "$@" ;;
  prompt) cmd_prompt "$@" ;;
  compact) cmd_compact "$@" ;;
  cleanup) cmd_cleanup "$@" ;;
  help|-h|--help|"") usage ;;
  *) die "unknown subcommand: $SUBCOMMAND" ;;
esac
