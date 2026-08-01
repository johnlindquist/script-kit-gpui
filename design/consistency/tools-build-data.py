#!/usr/bin/env python3
"""Build the static consistency-explorer manifest from the reviewed fix ledger.

The generated JSON is checked in; the explorer itself has no build step.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / ".notes" / "CONSISTENCY-FIXES.md"
OUTPUT = ROOT / "design" / "consistency" / "data" / "groups.json"

GROUPS = [
    {
        "id": "proof-truth",
        "title": "Proof and truth",
        "shortTitle": "Proof",
        "question": "Can evidence earn its status without coverage, screenshots, or invalid observers being mistaken for proof?",
        "tasks": ["RPT-001", "PF-001", "PF-002", "PF-003", "PF-004", "PF-005", "PF-009", "PF-010", "PF-011", "PF-012", "GOV-006"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
    {
        "id": "cues-actions",
        "title": "Cues, shortcuts, and action promises",
        "shortTitle": "Cues",
        "question": "Can every keycap, trigger, syntax token, hint, toast, and footer verb tell the truth?",
        "tasks": ["UX-001", "UX-002", "UX-004", "UX-015", "UX-016"],
        "fixture": "../mockups/screens/actions-dialog/index.html?embed=story",
    },
    {
        "id": "context-identity",
        "title": "Context, identity, and destinations",
        "shortTitle": "Context",
        "question": "Can prompt context, configuration identity, and delivery destination look related without behaving alike?",
        "tasks": ["SAFE-001", "UX-005", "WF-001", "WF-002", "WF-003", "WF-004", "WF-008"],
        "fixture": "../mockups/screens/agent-chat/index.html?embed=story",
    },
    {
        "id": "rows-sections",
        "title": "Rows, sections, selection, and scroll",
        "shortTitle": "Rows",
        "question": "Can rows share state meaning while keeping family-specific anatomy, density, and geometry?",
        "tasks": ["UX-006", "UX-007", "UX-008", "UX-009", "UX-010", "GEO-009"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
    {
        "id": "inputs-popups",
        "title": "Inputs, Actions, and popup lifecycle",
        "shortTitle": "Inputs",
        "question": "Can Actions search, disabled reasons, forms, popup dismissal, and focus return use shared owners?",
        "tasks": ["UX-003", "UX-011", "UX-012", "UX-013", "UX-014", "GEO-008"],
        "fixture": "../mockups/screens/actions-dialog/index.html?embed=story",
    },
    {
        "id": "states-recovery",
        "title": "Empty, setup, recovery, and rich states",
        "shortTitle": "States",
        "question": "Can semantic states become recognizable while rich guidance and actionful recovery stay capable?",
        "tasks": ["UX-017", "UX-018", "GOV-001"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
    {
        "id": "conversations-flow",
        "title": "Conversation commands and Flow",
        "shortTitle": "Conversations",
        "question": "Can Send, Stop, Retry, Back, New, Delete, copy, identity, and retention mean the same thing across hosts?",
        "tasks": ["SAFE-003", "WF-005", "WF-006", "WF-007", "WF-009", "WF-010", "WF-011"],
        "fixture": "../mockups/screens/chat-prompt/index.html?embed=story",
    },
    {
        "id": "notes-today",
        "title": "Notes, Today, Browse, and Agent Chat",
        "shortTitle": "Notes & Today",
        "question": "Can shared editing and search feel consistent while each host names its real destination and preserves state?",
        "tasks": ["SAFE-004", "WF-012", "WF-013", "WF-014", "WF-015", "WF-016", "WF-017", "GEO-004", "GEO-005"],
        "fixture": "../mockups/screens/notes/index.html?embed=story",
    },
    {
        "id": "dictation",
        "title": "Dictation targets, delivery, and recovery",
        "shortTitle": "Dictation",
        "question": "Can Dictation aim safely, deliver explicitly, preserve transcripts, restore focus, and make History truthful?",
        "tasks": ["SAFE-002", "WF-018", "WF-019", "WF-020", "WF-021", "WF-022", "WF-023", "WF-024"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
    {
        "id": "geometry-settings",
        "title": "Geometry, Settings, and presentation modes",
        "shortTitle": "Geometry",
        "question": "Can reviewers compare like semantic layers and approve Settings language without flattening intentional geometry?",
        "tasks": ["GEO-001", "GEO-002", "GEO-003", "GEO-006", "GEO-007"],
        "fixture": "../mockups/screens/settings/index.html?embed=story",
    },
    {
        "id": "accessibility-semantics",
        "title": "Accessibility, focus, text fit, and scroll proof",
        "shortTitle": "Accessibility",
        "question": "Can visual review expose AX parity, keyboard order, clipping, occlusion, and safe selected-row visibility?",
        "tasks": ["PF-006", "PF-007", "PF-008"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
    {
        "id": "governance-contracts",
        "title": "Tokens, ownership, conflicts, and locked glass",
        "shortTitle": "Governance",
        "question": "Can contracts distinguish compatibility debt, unit types, stale owner maps, conflict lifecycles, and protected calibration?",
        "tasks": ["GOV-002", "GOV-003", "GOV-004", "GOV-005", "GOV-007"],
        "fixture": "../mockups/screens/main-menu/index.html?embed=story",
    },
]

FIELD_RE = re.compile(r"^- \*\*(.+?):\*\*\s*(.*)$")
HEADING_RE = re.compile(r"^###\s+([A-Z]+-\d+)\s+[—:-]\s+(.+)$")


def normalize(value: str) -> str:
    value = value.replace("`", "")
    return re.sub(r"\s+", " ", value).strip()


def parse_tasks() -> dict[str, dict]:
    lines = SOURCE.read_text().splitlines()
    tasks: dict[str, dict] = {}
    current: dict | None = None
    for line in lines:
        heading = HEADING_RE.match(line)
        if heading:
            current = {"id": heading.group(1), "title": normalize(heading.group(2)), "fields": {}}
            tasks[current["id"]] = current
            continue
        if current is None:
            continue
        field = FIELD_RE.match(line)
        if field:
            current["fields"][normalize(field.group(1))] = normalize(field.group(2))

    for task in tasks.values():
        fields = task.pop("fields")
        task["status"] = fields.get("Priority / status", "Reviewed proposal")
        task["owners"] = fields.get("Owner / paths", fields.get("Owners", "See .notes/CONSISTENCY-FIXES.md"))
        task["before"] = next(
            (
                fields[key]
                for key in (
                    "Observation",
                    "Current",
                    "Current gap",
                    "Current behavior",
                    "Problem",
                    "Risk",
                )
                if key in fields
            ),
            "Current behavior is spread across local owners, implicit conventions, or incomplete proof, so the same cue can make different promises.",
        )
        task["after"] = next(
            (
                fields[key]
                for key in ("Change", "Decision rule", "Labels", "Invariants", "Acceptance")
                if key in fields
            ),
            task["title"],
        )
        task["acceptance"] = fields.get("Acceptance", task["after"])
        task["proof"] = fields.get("Proof", "Focused source, behavior, and runtime proof at the owning layer.")
        task["guardrail"] = fields.get("Guardrail", fields.get("Protected behavior", "Preserve intentional host and component divergences."))
        task["recommendation"] = recommendation(task["id"])
    return tasks


def recommendation(task_id: str) -> str:
    special = {
        "WF-014": "Revise the literal Ask wording; approve the Add/Attach stage-only proposal.",
        "GEO-002": "Approve the presentation contract after the strict runtime geometry receipt.",
        "GEO-003": "Approve model parity; product pixels are intentionally unchanged.",
        "GEO-005": "Approve the shared target envelope; exact pixels remain proof-gated.",
        "GOV-002": "Approve the lifecycle rule; delete only after a verified zero-caller receipt.",
        "GOV-005": "Approve the lifecycle taxonomy; revise individual classifications as needed.",
        "GOV-007": "Approve documentation-only reconciliation after unchanged anti-drift proof.",
        "UX-007": "Approve the launcher-family marker concept after the zero-pixel state consolidation.",
    }
    return special.get(task_id, "Approve the contract; keep implementation proof and product changes separate from this review.")


def main() -> None:
    tasks = parse_tasks()
    assigned: list[str] = []
    rendered_groups = []
    for group in GROUPS:
        assigned.extend(group["tasks"])
        rendered_groups.append({**group, "taskRecords": [tasks[task_id] for task_id in group["tasks"]]})

    expected = set(tasks)
    actual = set(assigned)
    duplicates = sorted(task_id for task_id in actual if assigned.count(task_id) > 1)
    if expected != actual or duplicates:
        raise SystemExit(
            f"coverage mismatch missing={sorted(expected - actual)} extra={sorted(actual - expected)} duplicates={duplicates}"
        )

    output = {
        "schemaVersion": 1,
        "title": "Script Kit consistency proposals",
        "summary": "Truthful before/after visual proposals for every task in .notes/CONSISTENCY-FIXES.md.",
        "truthLabels": {
            "before": "CURRENT · SOURCE-DERIVED",
            "after": "PROPOSAL · NOT IMPLEMENTED",
            "reference": "CURRENT FIXTURE · UNMODIFIED",
            "negative": "NEGATIVE CONTROL",
        },
        "groupCount": len(rendered_groups),
        "taskCount": len(tasks),
        "groups": rendered_groups,
    }
    OUTPUT.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {OUTPUT.relative_to(ROOT)} with {len(rendered_groups)} groups and {len(tasks)} tasks")


if __name__ == "__main__":
    main()
