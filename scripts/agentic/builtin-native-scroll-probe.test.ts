import { describe, expect, test } from "bun:test";
import { NATIVE_LIST_MATRIX, REQUIRED_NATIVE_LIST_FIELDS } from "./builtin-native-scroll-probe.ts";

describe("WP10 native list matrix", () => {
  test("covers Script List and every migrated built-in mode exactly once", () => {
    expect(NATIVE_LIST_MATRIX.map((entry) => entry.surface)).toEqual([
      "script_list", "app_launcher", "browser_tabs", "current_app_commands", "tips",
      "window_switcher", "clipboard_history", "process_manager", "kit_store_browse",
      "kit_store_installed", "browser_history", "notes_browse", "dictation_history",
      "agent_chat_history",
    ]);
    expect(new Set(NATIVE_LIST_MATRIX.map((entry) => entry.surface)).size).toBe(NATIVE_LIST_MATRIX.length);
  });

  test("declares empty short long and mixed-height fixture requirements for every row", () => {
    for (const entry of NATIVE_LIST_MATRIX) {
      expect(entry.fixtureProfiles).toEqual(["empty", "short", "long", "mixed-height"]);
    }
  });

  test("requires semantic, viewport, hover, focus, modality, source, and kind fields", () => {
    for (const field of [
      "selectedSemanticId", "scrollTopItem", "scrollTopOffsetPx", "firstVisibleSemanticId",
      "hoverSuppressedUntilPointerMove", "inputMode", "focusedSemanticId",
      "lastInteractionSource", "listKind",
    ]) expect(REQUIRED_NATIVE_LIST_FIELDS).toContain(field as never);
  });
});
