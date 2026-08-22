import { describe, expect, test } from "bun:test";
import {
  auditFacadeMigrationScope,
  CONVERSATION_STYLE_FACADE,
  CONVERSATION_STYLE_OWNER,
  POPUP_AUTOMATION_POLICY,
  POPUP_WINDOW_FACADE,
  POPUP_WINDOW_OWNER,
  REQUIRED_FACADE_SOURCE_PATHS,
  REQUIRED_POPUP_CONSUMERS,
  SHARED_COMPONENTS_MODULE,
  stripFacadeMigrationRustTrivia,
  attachFacadeMigrationScope,
  validateCompleteFacadeMigrationScope,
  validateFacadeMigrationSourceIdentity,
  type FacadeMigrationSource,
} from "./facade-migrations";

const conversationConsumer = "src/prompts/chat.rs";

function passingSources(): FacadeMigrationSource[] {
  return [
    {
      path: SHARED_COMPONENTS_MODULE,
      content: "pub mod conversation_style;\npub mod inline_popup_window;\n",
    },
    { path: CONVERSATION_STYLE_FACADE, content: undefined },
    { path: CONVERSATION_STYLE_OWNER, content: "pub struct ConversationStyle;" },
    { path: POPUP_WINDOW_FACADE, content: undefined },
    { path: POPUP_WINDOW_OWNER, content: "pub struct InlinePopupWindow;" },
    {
      path: POPUP_AUTOMATION_POLICY,
      content: "pub(crate) fn agent_chat_popup_policy() {}",
    },
    {
      path: conversationConsumer,
      content: "use crate::components::conversation_style::ConversationStyle;",
    },
    ...REQUIRED_POPUP_CONSUMERS.map((path) => ({
      path,
      content: "use crate::components::inline_popup_window::InlinePopupWindow;",
    })),
  ];
}

const persistedTokens = [
  "crate::components::conversation_style::ASSISTANT_MESSAGE_PADDING",
];

function replaceSource(
  sources: FacadeMigrationSource[],
  path: string,
  content: string | undefined,
): FacadeMigrationSource[] {
  return sources.map((source) =>
    source.path === path ? { path, content } : source,
  );
}

describe("GOV-002 complete facade-migration scope", () => {
  test("requires both retired facades and preserves narrow popup automation", () => {
    const ledger = auditFacadeMigrationScope(passingSources(), persistedTokens);

    expect(ledger.disposition).toBe("REMOVED");
    expect(ledger.failures).toEqual([]);
    expect(ledger.facades.map((facade) => facade.facadePath)).toEqual([
      CONVERSATION_STYLE_FACADE,
      POPUP_WINDOW_FACADE,
    ]);
    expect(ledger.popupAutomationPolicyPreserved).toBe(true);
    expect(ledger.popupPersistedTokenPaths).toEqual([]);
    expect(ledger.provesRuntimeBehavior).toBe(false);
    expect(ledger.provesExporterByteEquality).toBe(false);
    expect(ledger.coverageMode).toBe("BOUNDED_CANONICAL_CONSUMERS");
    expect(ledger.sourceGraphExhaustive).toBe(false);
    for (const path of REQUIRED_FACADE_SOURCE_PATHS) {
      expect(ledger.inspectedSourcePaths).toContain(path);
    }
    expect(
      ledger.sourceDigests.find((source) =>
        source.path === CONVERSATION_STYLE_FACADE,
      ),
    ).toMatchObject({ state: "ABSENT", sha256: null, byteLength: null });
    const popupOwnerDigest = ledger.sourceDigests.find(
      (source) => source.path === POPUP_WINDOW_OWNER,
    );
    expect(popupOwnerDigest?.state).toBe("PRESENT");
    expect(popupOwnerDigest?.sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(validateFacadeMigrationSourceIdentity(ledger, passingSources())).toEqual([]);
  });

  test.each([
    CONVERSATION_STYLE_FACADE,
    POPUP_WINDOW_FACADE,
  ])("rejects restoring retired facade %s", (path) => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(passingSources(), path, "pub use crate::components::*;"),
      persistedTokens,
    );
    expect(ledger.disposition).toBe("INCOMPLETE");
    expect(ledger.failures).toContain(`facade-still-exists:${path}`);
  });

  test.each([
    [CONVERSATION_STYLE_OWNER, "conversation_style"],
    [POPUP_WINDOW_OWNER, "inline_popup_window"],
  ])("rejects deleting canonical owner %s", (path) => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(passingSources(), path, undefined),
      persistedTokens,
    );
    expect(ledger.failures).toContain(`missing-canonical-owner:${path}`);
  });

  test.each([
    CONVERSATION_STYLE_OWNER,
    POPUP_WINDOW_OWNER,
  ])("rejects a canonical owner reduced to forwarding re-exports: %s", (path) => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        path,
        "pub use crate::components::some_other_module::*;",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toContain(
      "canonical-owner-has-no-owned-implementation:" + path,
    );
  });

  test.each(["style_contract", "popup_window"])(
    "rejects legacy qualified imports for %s",
    (legacyModule) => {
      const path = REQUIRED_POPUP_CONSUMERS[0];
      const ledger = auditFacadeMigrationScope(
        replaceSource(
          passingSources(),
          path,
          `use super::${legacyModule}::OldFacade;\nuse crate::components::inline_popup_window::InlinePopupWindow;`,
        ),
        persistedTokens,
      );
      expect(ledger.failures.some((failure) => failure.includes(`:${path}`))).toBe(true);
      expect(ledger.disposition).toBe("INCOMPLETE");
    },
  );

  test("rejects a restored popup module declaration", () => {
    const path = REQUIRED_POPUP_CONSUMERS[0];
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        path,
        "mod popup_window;\nuse crate::components::inline_popup_window::InlinePopupWindow;",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toContain(`legacy-caller:popup-window:${path}`);
  });

  test("rejects a stale popup facade hidden in a grouped Rust import", () => {
    const path = REQUIRED_POPUP_CONSUMERS[0];
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        path,
        "use super::{other, popup_window::OldPopup};\nuse crate::components::inline_popup_window::InlinePopupWindow;",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toContain(`legacy-caller:popup-window:${path}`);
  });

  test("retains the documented local canonical self-as-style_contract alias", () => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        conversationConsumer,
        "use crate::components::conversation_style::{self as style_contract, ConversationStyle};",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toEqual([]);
  });

  test("rejects deleting either canonical module registration", () => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        SHARED_COMPONENTS_MODULE,
        "pub mod conversation_style;",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toContain(
      "canonical-module-not-registered:inline_popup_window",
    );
  });

  test.each(REQUIRED_POPUP_CONSUMERS)(
    "requires the real popup consumer %s to import the shared owner",
    (path) => {
      const ledger = auditFacadeMigrationScope(
        replaceSource(passingSources(), path, "pub fn unrelated_popup() {}"),
        persistedTokens,
      );
      expect(ledger.failures).toContain(`missing-popup-canonical-consumer:${path}`);
    },
  );

  test("does not mistake retained popup automation policy for the old facade", () => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(
        passingSources(),
        POPUP_AUTOMATION_POLICY,
        "pub(crate) fn close_history_popup_window() {}",
      ),
      persistedTokens,
    );
    expect(ledger.failures).toEqual([]);
  });

  test("refuses deleting sanctioned Agent Chat popup automation", () => {
    const ledger = auditFacadeMigrationScope(
      replaceSource(passingSources(), POPUP_AUTOMATION_POLICY, undefined),
      persistedTokens,
    );
    expect(ledger.failures).toContain(
      `missing-popup-automation-policy:${POPUP_AUTOMATION_POLICY}`,
    );
  });

  test("rejects old facade names persisted in generated token paths", () => {
    const ledger = auditFacadeMigrationScope(passingSources(), [
      ...persistedTokens,
      "crate::ai::agent_chat::ui::popup_window::PADDING",
    ]);
    expect(ledger.failures).toContain(
      "legacy-persisted-token-path:crate::ai::agent_chat::ui::popup_window::PADDING",
    );
  });

  test("rejects persisted references to a deleted facade source filename", () => {
    const path = "src/ai/agent_chat/ui/style_contract.rs";
    const ledger = auditFacadeMigrationScope(passingSources(), [
      ...persistedTokens,
      path,
    ]);
    expect(ledger.failures).toContain("legacy-persisted-token-path:" + path);
  });

  test("comments and ordinary/raw strings cannot counterfeit legacy imports", () => {
    const path = REQUIRED_POPUP_CONSUMERS[0];
    const source = [
      "// use super::popup_window::OldFacade;",
      "/* mod popup_window; /* nested */ */",
      'let documented = "super::popup_window::OldFacade";',
      'let raw = r#"mod popup_window;"#;',
      "let quote = '\\\"';",
      "use crate::components::inline_popup_window::InlinePopupWindow;",
    ].join("\n");
    const ledger = auditFacadeMigrationScope(
      replaceSource(passingSources(), path, source),
      persistedTokens,
    );
    expect(ledger.failures).toEqual([]);
    expect(stripFacadeMigrationRustTrivia(source)).not.toContain("OldFacade");
  });

  test("duplicate source inventory cannot obscure a restored facade", () => {
    const ledger = auditFacadeMigrationScope(
      [
        ...passingSources(),
        { path: POPUP_WINDOW_FACADE, content: "pub mod legacy;" },
      ],
      persistedTokens,
    );
    expect(ledger.failures).toContain(`duplicate-source:${POPUP_WINDOW_FACADE}`);
    expect(ledger.disposition).toBe("INCOMPLETE");
  });

  test.each([
    "../src/components/conversation_style.rs",
    "/tmp/popup_window.rs",
    "src/components/../popup_window.rs",
  ])("rejects noncanonical source path %s", (path) => {
    const ledger = auditFacadeMigrationScope(
      [...passingSources(), { path, content: "pub mod harmless;" }],
      persistedTokens,
    );
    expect(ledger.failures).toContain("invalid-source-path:" + path);
  });

  test("accepts canonical external-crate and grouped component imports", () => {
    const popupPath = REQUIRED_POPUP_CONSUMERS[0];
    const sources = replaceSource(
      replaceSource(
        passingSources(),
        conversationConsumer,
        "use script_kit_gpui::components::conversation_style::ConversationStyle;",
      ),
      popupPath,
      "use crate::components::{inline_popup_window::InlinePopupWindow, other};",
    );
    expect(auditFacadeMigrationScope(sources, persistedTokens).failures).toEqual([]);
  });

  test("structural ledger validation rejects omitted popup migration", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const incomplete = { ...complete, facades: complete.facades.slice(0, 1) };
    expect(validateCompleteFacadeMigrationScope(incomplete)).toContain(
      "incomplete-required-facade-migration-set",
    );
  });

  test("rejects a forged canonical owner digest against current source bytes", () => {
    const sources = passingSources();
    const complete = auditFacadeMigrationScope(sources, persistedTokens);
    const forged = {
      ...complete,
      sourceDigests: complete.sourceDigests.map((digest) =>
        digest.path === POPUP_WINDOW_OWNER
          ? { ...digest, sha256: "a".repeat(64) }
          : digest,
      ),
    };
    expect(validateFacadeMigrationSourceIdentity(forged, sources)).toContain(
      "facade-source-identity-drift:" + POPUP_WINDOW_OWNER,
    );
  });

  test("rejects canonical source drift even when byte length is unchanged", () => {
    const sources = passingSources();
    const complete = auditFacadeMigrationScope(sources, persistedTokens);
    const drifted = replaceSource(
      sources,
      POPUP_WINDOW_OWNER,
      "pub struct InLinePopupWindow;",
    );
    expect(validateFacadeMigrationSourceIdentity(complete, drifted)).toContain(
      "facade-source-identity-drift:" + POPUP_WINDOW_OWNER,
    );
  });

  test("rejects a ledger that marks a removed facade as a present source", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const restored = {
      ...complete,
      sourceDigests: complete.sourceDigests.map((digest) =>
        digest.path === POPUP_WINDOW_FACADE
          ? {
              ...digest,
              state: "PRESENT",
              sha256: "a".repeat(64),
              byteLength: 0,
            }
          : digest,
      ),
    };
    expect(validateCompleteFacadeMigrationScope(restored)).toContain(
      "retired-facade-source-digest-is-present:" + POPUP_WINDOW_FACADE,
    );
  });

  test("structural ledger validation rejects swapped facade/owner mappings", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const swapped = {
      ...complete,
      facades: complete.facades.map((facade) => ({
        ...facade,
        canonicalOwner:
          facade.id === "popup-window"
            ? CONVERSATION_STYLE_OWNER
            : POPUP_WINDOW_OWNER,
      })),
    };
    expect(validateCompleteFacadeMigrationScope(swapped)).toContain(
      "incorrect-canonical-facade-owner:popup-window",
    );
  });

  test("rejects a ledger claiming owner existence without owned implementation", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const forwarded = {
      ...complete,
      facades: complete.facades.map((facade) =>
        facade.id === "popup-window"
          ? { ...facade, canonicalOwnerDefinesImplementation: false }
          : facade,
      ),
    };
    expect(validateCompleteFacadeMigrationScope(forwarded)).toContain(
      "canonical-facade-owner-has-no-owned-implementation:popup-window",
    );
  });

  test("structural ledger validation rejects duplicate migration records", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const duplicated = {
      ...complete,
      facades: [complete.facades[0], complete.facades[0]],
    };
    expect(validateCompleteFacadeMigrationScope(duplicated)).toContain(
      "missing-or-duplicate-facade-migration:popup-window",
    );
  });

  test.each(["provesRuntimeBehavior", "provesExporterByteEquality"])(
    "rejects falsely claiming %s from static migration evidence",
    (key) => {
      const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
      expect(validateCompleteFacadeMigrationScope({ ...complete, [key]: true }).length)
        .toBeGreaterThan(0);
      expect(() =>
        attachFacadeMigrationScope({ taskId: "GOV-002", [key]: true }, complete),
      ).toThrow();
    },
  );

  test("rejects pretending a bounded source inventory was exhaustive", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    expect(
      validateCompleteFacadeMigrationScope({ ...complete, sourceGraphExhaustive: true }),
    ).toContain("facade-source-graph-exhaustiveness-misrepresented");
  });

  test("rejects hiding a required popup consumer from the source inventory", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const missingPath = REQUIRED_POPUP_CONSUMERS[0];
    expect(
      validateCompleteFacadeMigrationScope({
        ...complete,
        inspectedSourcePaths: complete.inspectedSourcePaths.filter(
          (path) => path !== missingPath,
        ),
      }),
    ).toContain(`missing-inspected-facade-source:${missingPath}`);
  });

  test("forged auxiliary ledger paths cannot escape the bounded Rust source tree", () => {
    const complete = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const forged = {
      ...complete,
      inspectedSourcePaths: [...complete.inspectedSourcePaths, "/etc/passwd"],
      sourceDigests: [
        ...complete.sourceDigests,
        {
          path: "/etc/passwd",
          state: "PRESENT",
          sha256: "a".repeat(64),
          byteLength: 1,
        },
      ],
    };
    const errors = validateCompleteFacadeMigrationScope(forged);
    expect(errors).toContain("unsafe-inspected-facade-source-path:/etc/passwd");
    expect(errors).toContain("unsafe-facade-source-digest-path:/etc/passwd");
  });

  test("does not attach GOV-002 source evidence to a different task", () => {
    const scope = auditFacadeMigrationScope(passingSources(), persistedTokens);
    expect(() =>
      attachFacadeMigrationScope({ taskId: "GOV-001" }, scope),
    ).toThrow("GOV-002");
  });

  test("refuses replacing preexisting or falsely runtime-classified evidence", () => {
    const scope = auditFacadeMigrationScope(passingSources(), persistedTokens);
    expect(() =>
      attachFacadeMigrationScope(
        { taskId: "GOV-002", facadeMigrations: { forged: true } },
        scope,
      ),
    ).toThrow("overwrite");
    expect(() =>
      attachFacadeMigrationScope(
        { taskId: "GOV-002", evidenceType: "DIRECT_RUNTIME" },
        scope,
      ),
    ).toThrow("STATIC_INVENTORY");
  });

  test("attaches both structured records without erasing legacy ledger fields", () => {
    const scope = auditFacadeMigrationScope(passingSources(), persistedTokens);
    const ledger = attachFacadeMigrationScope(
      { taskId: "GOV-002", assertions: { sourceOwnersPass: true } },
      scope,
    );
    expect(ledger.taskId).toBe("GOV-002");
    expect(ledger.facades).toHaveLength(2);
    expect(ledger.facadeMigrations).toBe(scope);
    expect(validateCompleteFacadeMigrationScope(ledger.facadeMigrations)).toEqual([]);
  });
});
