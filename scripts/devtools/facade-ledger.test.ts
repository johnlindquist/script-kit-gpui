import { beforeAll, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  collectConversationMigrationSources,
  CANONICAL_MIGRATION_SOURCE_PATHS,
  inspectConversationFacadeMigration,
  inspectCurrentConversationFacadeMigration,
  inspectCurrentFacadeMigrations,
  PERSISTED_CONVERSATION_CONTRACT,
  type RustMigrationSource,
} from "./facade-ledger.ts";

let sources: RustMigrationSource[];
let bundle: Record<string, unknown>;

beforeAll(() => {
  sources = collectConversationMigrationSources();
  bundle = JSON.parse(readFileSync(PERSISTED_CONVERSATION_CONTRACT, "utf8"));
});

function clonedSources() {
  return sources.map((entry) => ({ ...entry }));
}

describe("real conversation-style compatibility facade migration", () => {
  test("bounded named-source discovery never invokes an injected external runner", () => {
    let externalProcessStarts = 0;
    const inventory = collectConversationMigrationSources({
      refresh: true,
      externalRunner: () => {
        externalProcessStarts += 1;
        throw new Error("external source discovery must never execute");
      },
    });
    expect(externalProcessStarts).toBe(0);
    expect(inventory.length).toBeLessThanOrEqual(
      CANONICAL_MIGRATION_SOURCE_PATHS.length,
    );
    expect(inventory.map((source) => source.path)).toContain(
      "src/components/conversation_text.rs",
    );
  });

  test("both retired facades are present in one truthful canonical-owner ledger", async () => {
    const ledger = await inspectCurrentFacadeMigrations();
    expect(ledger.pass).toBe(true);
    expect(ledger.evidenceClass).toBe("STATIC_INVENTORY");
    expect(ledger.provesRuntimeBehavior).toBe(false);
    expect(ledger.provesExporterByteEquality).toBe(false);
    expect(ledger.sourceCorpus.coverageMode).toBe(
      "BOUNDED_CANONICAL_OWNERS_AND_CONSUMERS",
    );
    expect(ledger.sourceCorpus.sourceGraphExhaustive).toBe(false);
    expect(ledger.sourceCorpus.externalProcessesStarted).toBe(0);
    expect(ledger.facades.map((facade) => facade.facadePath)).toEqual([
      "src/ai/agent_chat/ui/style_contract.rs",
      "src/ai/agent_chat/ui/popup_window.rs",
    ]);
    expect(ledger.facadeMigrations.popupAutomationPolicyPreserved).toBe(true);
    expect(ledger.facadeMigrations.sourceGraphExhaustive).toBe(false);
    expect(
      ledger.facadeMigrations.facades.find(
        (facade) => facade.id === "conversation-style",
      )?.canonicalConsumerPaths,
    ).toContain("src/prompts/chat/render_turns.rs");
    expect(
      ledger.facadeMigrations.facades.find(
        (facade) => facade.id === "popup-window",
      )?.canonicalConsumerPaths,
    ).toContain("src/menu_syntax/object_selector.rs");
    expect(
      ledger.facadeMigrations.facades.every(
        (facade) =>
          facade.facadeExists === false &&
          facade.canonicalOwnerDefinesImplementation === true &&
          facade.directCallerPaths.length === 0,
      ),
    ).toBe(true);
  });

  test("actual production, test, exporter, and persisted consumers share one owner", () => {
    const ledger = inspectCurrentConversationFacadeMigration();
    expect(ledger.pass).toBe(true);
    expect(ledger.disposition).toBe("EVALUABLE_PASS");
    expect(ledger.evidenceClass).toBe("STATIC_INVENTORY");
    expect(ledger.provesRuntimeBehavior).toBe(false);
    expect(Object.values(ledger.assertions).every(Boolean)).toBe(true);
    expect(ledger.sourceCorpus.canonicalProductionConsumers).toContain(
      "src/ai/agent_chat/ui/components/transcript.rs",
    );
    expect(ledger.sourceCorpus.canonicalProductionConsumers).toContain(
      "src/prompts/chat/render_turns.rs",
    );
    expect(ledger.sourceCorpus.canonicalTestConsumers.length).toBeGreaterThan(0);
    expect(ledger.persistedNames.canonicalTokenCount).toBeGreaterThan(0);
    expect(ledger.persistedNames.legacyTokenIds).toEqual([]);
  });

  test("a canonical local style_contract alias is legitimate, not a resurrected facade", () => {
    const ledger = inspectConversationFacadeMigration(sources, bundle);
    expect(ledger.sourceCorpus.unboundLocalAliases).toEqual([]);
    expect(ledger.sourceCorpus.staleProductionReferences).toEqual([]);
  });

  test("a reintroduced facade file, module, or qualified import fails migration", () => {
    expect(inspectConversationFacadeMigration(sources, bundle, { facadeExists: true }).pass)
      .toBe(false);

    for (const injected of [
      "mod style_contract;",
      "use crate::ai::agent_chat::ui::style_contract;",
    ]) {
      const mutated = clonedSources();
      mutated.find((entry) => entry.path === "src/components/mod.rs")!.source +=
        `\n${injected}\n`;
      expect(inspectConversationFacadeMigration(mutated, bundle).pass).toBe(false);
    }
  });

  test("local alias usage must resolve to the canonical imported module", () => {
    const mutated = clonedSources();
    const transcript = mutated.find((entry) =>
      entry.path === "src/ai/agent_chat/ui/components/transcript.rs"
    )!;
    transcript.source = transcript.source.replace(
      "self as style_contract,",
      "self as migrated_contract,",
    );
    const ledger = inspectConversationFacadeMigration(mutated, bundle);
    expect(ledger.pass).toBe(false);
    expect(ledger.sourceCorpus.unboundLocalAliases).toEqual([transcript.path]);
  });

  test("a missing migrated surface or legacy persisted token blocks acceptance", () => {
    const absent = sources.filter((entry) =>
      entry.path !== "src/prompts/chat/render_turns.rs"
    );
    const missingConsumer = inspectConversationFacadeMigration(absent, bundle);
    expect(missingConsumer.pass).toBe(false);
    expect(missingConsumer.sourceCorpus.missingProductionConsumers).toContain(
      "src/prompts/chat/render_turns.rs",
    );

    const mutatedBundle = structuredClone(bundle);
    const token = Object.values(mutatedBundle.tokens as Record<string, any>)
      .find((record) => String(record.rustPath ?? "").includes("conversation_style::"));
    token.rustPath = String(token.rustPath).replace(
      "conversation_style::",
      "style_contract::",
    );
    const legacyToken = inspectConversationFacadeMigration(sources, mutatedBundle);
    expect(legacyToken.pass).toBe(false);
    expect(legacyToken.persistedNames.legacyTokenIds.length).toBe(1);
  });

  test("legacy names in documentation and strings do not masquerade as Rust imports", () => {
    const mutated = clonedSources();
    const owner = mutated.find((entry) =>
      entry.path === "src/components/conversation_style.rs"
    )!;
    owner.source +=
      '\n// crate::ai::agent_chat::ui::style_contract::old\n' +
      'const EXAMPLE: &str = "crate::ai::agent_chat::ui::style_contract::old";\n';
    expect(inspectConversationFacadeMigration(mutated, bundle).pass).toBe(true);
  });
});
