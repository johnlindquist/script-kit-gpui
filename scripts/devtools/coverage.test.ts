import { describe, expect, test } from "bun:test";
import {
  buildCoverageReport,
  coverageProfileById,
  coverageProfiles,
  validateCoverageProfiles,
  type CoverageProfile,
} from "./coverage.ts";

function mainProfile(changes: Partial<CoverageProfile> = {}): CoverageProfile {
  const profile = coverageProfileById("main");
  if (!profile) throw new Error("main coverage profile is required");
  return { ...profile, ...changes };
}

describe("fail-closed coverage ownership registry", () => {
  test("every tracked coverage owner resolves inside the repository", () => {
    expect(validateCoverageProfiles(coverageProfiles)).toEqual([]);
    expect(mainProfile().sourceFiles).toEqual([
      "src/main_sections/render_impl.rs",
      "src/main_sections/app_state.rs",
      "src/main_entry/app_run_setup.rs",
      "src/main_entry/app_run_setup_startup_helpers.rs",
      "src/render_script_list/mod.rs",
      "src/app_execute/builtin_execution.rs",
      "src/app_execute/builtin_execution_support.rs",
      "src/app_execute/builtin_execution_ai_capture.rs",
      "src/app_impl/ui_window.rs",
      "src/app_impl/ui_window_footer_helpers.rs",
      "src/app_impl/ui_window_context_chips.rs",
      "src/app_impl/ui_window_interaction_helpers.rs",
      "src/footer_popup.rs",
      "src/footer_popup_fidelity.rs",
      "src/footer_popup_glass_geometry.rs",
      "src/footer_popup_native_dispatch.rs",
      "src/footer_popup_native_layout.rs",
      "src/platform/secondary_window_config.rs",
      "src/platform/secondary_window_glass_animation.rs",
      "src/platform/secondary_window_glass_backdrop.rs",
      "src/platform/secondary_window_glass_lifecycle.rs",
      "src/platform/secondary_window_glass_style.rs",
      "src/platform/secondary_window_resize_policy.rs",
      "src/platform/secondary_window_vibrancy_impl.rs",
    ]);
    expect(coverageProfileById("dictation-history")?.sourceFiles).toContain(
      "src/mcp_resources/mod.rs",
    );
  });

  test("a stale source owner invalidates its profile", () => {
    const errors = validateCoverageProfiles([
      mainProfile({ sourceFiles: ["src/does-not-exist.rs"] }),
    ]);
    expect(errors).toContain(
      "profile main references missing source owner: src/does-not-exist.rs",
    );
  });

  test("empty, duplicate, absolute, and escaping owners fail closed", () => {
    expect(validateCoverageProfiles([mainProfile({ sourceFiles: [] })])).toContain(
      "profile main has no source owners",
    );
    expect(validateCoverageProfiles([mainProfile({ sourceFiles: [""] })])).toContain(
      "profile main has an empty source owner",
    );
    expect(
      validateCoverageProfiles([
        mainProfile({
          sourceFiles: [
            "src/main_sections/render_impl.rs",
            "src/main_sections/../main_sections/render_impl.rs",
          ],
        }),
      ]),
    ).toContain(
      "profile main lists source owner twice: src/main_sections/../main_sections/render_impl.rs",
    );
    expect(
      validateCoverageProfiles([mainProfile({ sourceFiles: ["../outside.rs"] })]),
    ).toContain("profile main source owner escapes repository: ../outside.rs");
    expect(
      validateCoverageProfiles([mainProfile({ sourceFiles: ["/tmp/outside.rs"] })]),
    ).toContain("profile main source owner escapes repository: /tmp/outside.rs");
  });

  test("source owner validation can be deterministically fault-injected", () => {
    const errors = validateCoverageProfiles([mainProfile()], {
      ownerExists: (path) => !path.endsWith("render_script_list/mod.rs"),
    });
    expect(errors).toContain(
      "profile main references missing source owner: src/render_script_list/mod.rs",
    );
  });

  test("Flow and Notes ownership inventories include extracted runtime, persistence, and automation", () => {
    for (const profileId of ["flow-ux-view", "flow-session-view"]) {
      expect(coverageProfileById(profileId)?.sourceFiles).toEqual(
        expect.arrayContaining([
          "src/render_builtins/flow_ux_session_runtime.rs",
          "src/render_builtins/flow_ux_session_navigation.rs",
          "src/flows/session_persistence.rs",
          "src/app_layout/collect_elements_flow_surfaces.rs",
          "src/app_layout/collect_elements_projection_primitives.rs",
        ]),
      );
    }

    expect(coverageProfileById("notes")?.sourceFiles).toEqual(
      expect.arrayContaining([
        "src/notes/window/window_ops_automation.rs",
        "src/notes/window/window_ops_mcp.rs",
      ]),
    );
  });

  test("Agent Chat ownership includes reliability, context, branching, and fixture state", () => {
    expect(coverageProfileById("agent-chat")?.sourceFiles).toEqual(
      expect.arrayContaining([
        "src/ai/agent_chat/ui/thread.rs",
        "src/ai/agent_chat/ui/thread_recovery.rs",
        "src/ai/agent_chat/ui/thread_context_resolution.rs",
        "src/ai/agent_chat/ui/thread_fork_models.rs",
        "src/ai/agent_chat/ui/thread_fixtures.rs",
        "src/ai/agent_chat/ui/view_automation_geometry.rs",
        "src/ai/agent_chat/ui/view_footer_ownership.rs",
        "src/ai/agent_chat/ui/view_focused_text_variations.rs",
        "src/ai/agent_chat/ui/view_history_navigation.rs",
        "src/ai/agent_chat/ui/view_permission_actions.rs",
        "src/ai/agent_chat/ui/view_spine_rich_results.rs",
        "src/ai/agent_chat/ui/view_recovery_and_transient.rs",
        "src/app_layout/collect_elements_projection_primitives.rs",
      ]),
    );
  });

  test("Dictation ownership includes visual state, recovery actions, and automation layout", () => {
    expect(coverageProfileById("dictation")?.sourceFiles).toEqual(
      expect.arrayContaining([
        "src/dictation/window.rs",
        "src/dictation/window_visual_primitives.rs",
        "src/dictation/window_actions.rs",
        "src/dictation/window_automation.rs",
      ]),
    );
  });

  test("static Direct bindings never masquerade as runtime proof", () => {
    const report = buildCoverageReport({ surface: "agent-chat" });
    expect(report.surfaces[0]?.bindingSelectors[0]?.relation).toBe("Direct");
    expect(report.evidenceClass).toBe("STATIC_INVENTORY");
    expect(report.runtimeProof.disposition).toBe("NOT_EVALUATED");
    expect(report.runtimeProof.provenSurfaceCount).toBe(0);
    expect(report.registryValidation.errors).toEqual([]);
  });

  test("launcher and Dictation History advertise their real direct owners without claiming runtime proof", () => {
    const launcher = coverageProfileById("main");
    const dictationHistory = coverageProfileById("dictation-history");

    expect(launcher?.bindingSelectors).toContainEqual({
      relation: "Direct",
      priority: 100,
      contractKinds: ["ScriptList"],
      appViewVariants: ["ScriptList"],
      hostKinds: ["MainWindow"],
    });
    expect(dictationHistory?.status).toBe("partial");
    expect(dictationHistory?.sourceFiles).toEqual(
      expect.arrayContaining([
        "src/render_builtins/dictation_history.rs",
        "src/render_builtins/common.rs",
        "src/app_layout/collect_elements.rs",
        "src/app_layout/build_layout_info.rs",
      ]),
    );
    expect(dictationHistory?.availablePrimitiveIds).toEqual(
      expect.arrayContaining([
        "devtools.elements.snapshot",
        "devtools.focus.inspect",
        "devtools.keyboard.inspect",
        "devtools.layout.measure",
        "devtools.scroll.inspect",
        "devtools.act",
      ]),
    );

    const report = buildCoverageReport({ surface: "dictation-history" });
    expect(report.evidenceClass).toBe("STATIC_INVENTORY");
    expect(report.runtimeProof.provenSurfaceCount).toBe(0);
    expect(report.surfaces[0]?.missingRuntimePrimitives).toContain(
      "redacted transcript fingerprint",
    );
  });

  test("high-traffic launcher surfaces name their real renderer and semantic owners without claiming runtime proof", () => {
    const expectedOwners = [
      ["clipboard-history", "ClipboardHistoryView", "src/render_builtins/clipboard.rs"],
      ["browser-history", "BrowserHistoryView", "src/render_builtins/browser_history.rs"],
      ["notes-browse", "NotesBrowseView", "src/render_builtins/notes_browse.rs"],
      ["file-search", "FileSearchView", "src/render_builtins/file_search.rs"],
      ["day-page", "DayPage", "src/main_sections/day_page_view.rs"],
      ["current-app-commands", "CurrentAppCommandsView", "src/render_builtins/current_app_commands.rs"],
      ["agent-chat-history", "AgentChatHistoryView", "src/render_builtins/agent_chat_history.rs"],
      ["webcam", "WebcamView", "src/prompts/webcam.rs"],
    ] as const;

    for (const [id, variant, renderer] of expectedOwners) {
      const profile = coverageProfileById(id);
      expect(profile?.status).toBe("partial");
      expect(profile?.sourceFiles).toEqual([
        renderer,
        "src/app_layout/collect_elements.rs",
        "src/app_layout/collect_elements_surface_rows.rs",
        "src/app_layout/collect_elements_projection_primitives.rs",
        "src/app_layout/build_layout_info.rs",
      ]);
      expect(profile?.bindingSelectors).toContainEqual({
        relation: "Direct",
        priority: 100,
        appViewVariants: [variant],
        hostKinds: ["MainWindow"],
      });
      const report = buildCoverageReport({ surface: id });
      expect(report.evidenceClass).toBe("STATIC_INVENTORY");
      expect(report.runtimeProof.provenSurfaceCount).toBe(0);
    }
    expect(validateCoverageProfiles(coverageProfiles)).toEqual([]);
  });
});
