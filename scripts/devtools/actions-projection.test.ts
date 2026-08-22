import { describe, expect, test } from "bun:test";
import {
  auditActionProjection,
  groupActionSections,
  visibleActionRows,
} from "./actions.ts";

describe("capture-free production action projection", () => {
  test("an authoritative empty filtered result never resurrects stale sample actions", () => {
    const rows = visibleActionRows({
      visibleActions: [],
      actions: {
        visibleSample: [{ id: "stale:run", title: "Stale action", enabled: true }],
      },
    });
    expect(rows).toEqual([]);
    expect(auditActionProjection(rows).complete).toBe(true);
  });

  test("legacy samples remain available only when no authoritative list exists", () => {
    const rows = visibleActionRows({
      actions: {
        visibleSample: [{
          id: "action:open",
          title: "Open",
          section: "Primary",
          shortcut: "⌘+O",
          enabled: true,
        }],
      },
    });
    expect(rows[0]).toMatchObject({
      id: "action:open",
      label: "Open",
      enabled: true,
      activatable: true,
      shortcutTokens: ["⌘", "O"],
    });
    expect(auditActionProjection(rows).activatableActionIds).toEqual(["action:open"]);
    expect(groupActionSections(rows)).toEqual([{
      title: "Primary",
      rowCount: 1,
      firstIndex: 0,
      lastIndex: 0,
    }]);
  });

  test("disabled reasons override stale enabled claims and remain explicit contradictions", () => {
    const rows = visibleActionRows({
      visibleActions: [{
        id: "action:retry",
        label: "Retry",
        enabled: true,
        actionDisabled: "MissingProvider",
      }],
    });
    expect(rows[0]).toMatchObject({
      enabled: false,
      disabledReason: "MissingProvider",
      stateConsistent: false,
      activatable: false,
    });
    const projection = auditActionProjection(rows);
    expect(projection.complete).toBe(false);
    expect(projection.contradictoryAvailabilityIds).toEqual(["action:retry"]);
    expect(projection.activatableActionIds).toEqual([]);
  });

  test("unknown availability and missing or duplicate identities fail closed", () => {
    const rows = visibleActionRows({
      visibleActions: [
        { id: "duplicate", label: "First", enabled: true },
        { id: "duplicate", label: "Second", enabled: true },
        { label: "Missing identity", enabled: true },
        { id: "unknown", label: "Unknown availability" },
      ],
    });
    const projection = auditActionProjection(rows);
    expect(projection.complete).toBe(false);
    expect(projection.duplicateActionIds).toEqual(["duplicate"]);
    expect(projection.missingActionIds).toEqual([2]);
    expect(projection.unknownAvailabilityIds).toEqual(["unknown"]);
    expect(rows[2].activatable).toBe(false);
    expect(rows[3].activatable).toBe(false);
  });
});
