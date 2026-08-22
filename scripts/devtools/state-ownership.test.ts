import { describe, expect, test } from "bun:test";
import {
  auditStateOwnership,
  CANONICAL_STATE_OWNERS,
  collectCurrentStateOwnershipSources,
  inspectCurrentStateOwnership,
  REQUIRED_STATE_CONSUMERS,
  REQUIRED_STATE_OWNERSHIP_PATHS,
  type StateOwnershipSource,
} from "./state-ownership.ts";

function passingSources(): StateOwnershipSource[] {
  const sources: Record<string, string> = {
    "crates/sk-protocol/src/ascii_search.rs":
      "pub fn contains_ignore_ascii_case() {} pub fn find_ignore_ascii_case() {} pub fn fuzzy_match_with_indices_ascii() {} pub fn is_fuzzy_match() {} pub fn is_word_boundary_match() {}",
    "crates/sk-protocol/src/command_contract.rs":
      "pub enum CommandSource {} pub struct CommandIdentity; pub struct CommandDescriptor; pub enum CommandAvailability {}",
    "crates/sk-protocol/src/filter_coalescer.rs":
      "pub struct FilterCoalescer;",
    "crates/sk-protocol/src/query_prefix.rs":
      "pub struct ParsedQuery; pub fn parse_query_prefix() {} pub fn builtin_passes_prefix_filter() {} pub fn app_passes_prefix_filter() {} pub fn window_passes_prefix_filter() {} pub fn flow_passes_prefix_filter() {} pub fn skill_passes_prefix_filter() {} pub fn should_search_scripts() {} pub fn should_search_scriptlets() {}",
    "crates/sk-protocol/src/search_contract.rs":
      "pub struct ProviderRequest; pub struct ProviderGenerationFence; pub struct RootOwnedProviderRefresh; pub struct RootOwnedProviderRefreshLifecycle; pub struct RootProviderCoordinator;",
    "crates/sk-protocol/src/sentence_search.rs":
      "pub struct LongTextQuery; pub struct LongTextMatchEvidence;",
    "crates/sk-protocol/src/lib.rs":
      "pub mod ascii_search; pub mod command_contract; pub mod filter_coalescer; pub mod query_prefix; pub mod search_contract; pub mod sentence_search;",
    "src/scripts/types.rs":
      "pub enum SearchResult {} pub struct MatchEvidence;",
    "src/scripts/command_contract.rs":
      "use sk_protocol::command_contract::{CommandIdentity, CommandDescriptor, CommandSource}; pub struct LauncherCommandReceipt; impl SearchResult { pub fn command_descriptor(&self) {} }",
    "src/scripts/root_search_contract.rs":
      "pub(crate) use sk_protocol::search_contract::{RootProviderCoordinator, RootOwnedProviderRefresh, RootOwnedProviderRefreshLifecycle};",
    "src/scripts/search.rs":
      "mod ascii; mod prefix_filters; pub(crate) use ascii::{contains_ignore_ascii_case, find_ignore_ascii_case, fuzzy_match_with_indices_ascii, is_fuzzy_match, is_word_boundary_match}; pub(crate) use prefix_filters::{parse_query_prefix, builtin_passes_prefix_filter, app_passes_prefix_filter, window_passes_prefix_filter, skill_passes_prefix_filter, should_search_scripts, should_search_scriptlets};",
    "src/scripts/search/ascii.rs":
      "pub(crate) use sk_protocol::ascii_search::{contains_ignore_ascii_case, find_ignore_ascii_case, fuzzy_match_with_indices_ascii, is_fuzzy_match, is_word_boundary_match};",
    "src/scripts/search/prefix_filters.rs":
      "pub(crate) use sk_protocol::query_prefix::{ParsedQuery, parse_query_prefix, builtin_passes_prefix_filter, app_passes_prefix_filter, window_passes_prefix_filter, flow_passes_prefix_filter, skill_passes_prefix_filter, should_search_scripts, should_search_scriptlets};",
    "src/scripts/search/sentence.rs":
      "pub(crate) use sk_protocol::sentence_search::*;",
    "src/filter_coalescer.rs":
      "pub use sk_protocol::filter_coalescer::FilterCoalescer;",
    "src/scripts/mod.rs":
      "mod command_contract; pub(crate) mod root_search_contract; mod types; pub use self::types::{SearchResult, MatchEvidence};",
    "src/main_sections/root_search_store.rs":
      "pub(crate) struct RootSearchStore { provider: crate::scripts::root_search_contract::RootProviderCoordinator, root_file_result_cache: Vec<u8> }",
    "src/main_sections/app_state.rs":
      "pub(crate) struct RootPassiveFrame; struct App { root_search: RootSearchStore, cached_filtered_results: Vec<u8> }",
    "src/config/command_ids.rs":
      "use sk_protocol::command_contract::{CommandIdentity, CommandSource}; pub enum CommandCategory {} pub const SUPPORTED_COMMAND_CATEGORIES: &[CommandCategory] = &[CommandCategory::Builtin, CommandCategory::App, CommandCategory::Script, CommandCategory::Scriptlet, CommandCategory::PromptTarget, CommandCategory::PromptAction];",
    "src/config/mod.rs":
      "pub mod command_ids; pub use command_ids::{CommandCategory};",
    "src/components/conversation_actions.rs":
      "pub(crate) struct ConversationCommandDescriptor; impl ConversationCommandDescriptor { fn command_action(&self) -> sk_protocol::command_contract::CommandAction {} }",
    "src/components/mod.rs":
      "pub mod unified_list_item; pub(crate) mod footer_chrome;",
    "src/components/unified_list_item/mod.rs":
      "pub use crate::list_item::{GroupedListItem, GroupedListState};",
    "src/list_item/mod.rs":
      "pub enum GroupedListItem {} pub struct GroupedListState; pub struct ListItem;",
    "src/main.rs":
      "mod root_search_store; use root_search_store::RootSearchStore; mod filter_coalescer; use crate::filter_coalescer::FilterCoalescer; mod list_item; mod footer_popup;",
    "src/main_window_preflight/build.rs":
      "fn preflight() { app.redacted_command_receipt(); app.root_search.root_passive_frame(); }",
    "src/app_render/focused_info.rs":
      "fn info() { result.command_descriptor(); info.with_command_descriptor(value); }",
    "src/actions/builders/script_context.rs":
      "fn action() { script.command_descriptor.primary_action(); }",
    "src/render_script_list/mod.rs":
      "fn render() { crate::list_item::ListItem::new(); app.main_window_primary_action_label(); crate::components::footer_chrome::render(); }",
    "src/app_impl/filtering_cache.rs":
      "fn filter() { app.root_search.begin_provider_request(sk_protocol::command_contract::CommandSource::BrowserTab); app.root_search.invalidate_provider_request(source); }",
    "src/app_impl/filter_input_updates.rs":
      "fn filter() { app.filter_coalescer.queue(value); app.filter_coalescer.take_latest(); app.filter_coalescer.reset(); }",
    "src/scripts/search/unified.rs":
      "fn search() { parse_query_prefix(query); builtin_passes_prefix_filter(parsed); app_passes_prefix_filter(parsed); window_passes_prefix_filter(parsed); flow_passes_prefix_filter(parsed); skill_passes_prefix_filter(parsed); should_search_scripts(parsed); should_search_scriptlets(parsed); }",
  };
  return REQUIRED_STATE_OWNERSHIP_PATHS.map((path) => ({
    path,
    content: sources[path],
  }));
}

function mutateSource(
  path: string,
  mutate: (source: string) => string | undefined,
): StateOwnershipSource[] {
  return passingSources().map((source) =>
    source.path === path
      ? { path, content: mutate(source.content ?? "") }
      : source,
  );
}

describe("GOV-001 canonical state ownership and sanctioned exceptions", () => {
  test("proves bounded named owners, real consumer edges, and every approved exception", () => {
    const audit = auditStateOwnership(passingSources());

    expect(audit.pass).toBe(true);
    expect(audit.failures).toEqual([]);
    expect(audit.owners.map(({ id }) => id)).toEqual(
      CANONICAL_STATE_OWNERS.map(({ id }) => id),
    );
    expect(audit.consumers.map(({ path }) => path)).toEqual(
      [...REQUIRED_STATE_CONSUMERS],
    );
    expect(Object.values(audit.sanctionedExceptions).every(Boolean)).toBe(true);
    expect(audit.coverageMode).toBe("BOUNDED_NAMED_OWNERS_AND_CONSUMERS");
    expect(audit.sourceGraphExhaustive).toBe(false);
    expect(audit.provesRuntimeBehavior).toBe(false);
    expect(audit.externalProcessesStarted).toBe(0);
  });

  test("current exact production owners and consumers pass without scanning or launching", () => {
    const audit = inspectCurrentStateOwnership();

    expect(audit.failures).toEqual([]);
    expect(audit.pass).toBe(true);
    expect(audit.inspectedSourcePaths).toEqual(REQUIRED_STATE_OWNERSHIP_PATHS);
    for (const path of REQUIRED_STATE_OWNERSHIP_PATHS) {
      expect(audit.sourceFingerprints[path]).toMatch(/^[a-f0-9]{64}$/);
    }
  });

  test("launcher filter scheduling is owned by the testable domain and consumed through its adapter", () => {
    const audit = auditStateOwnership(passingSources());

    expect(audit.owners).toContainEqual(
      expect.objectContaining({
        id: "domain-filter-coalescer",
        path: "crates/sk-protocol/src/filter_coalescer.rs",
      }),
    );
    expect(audit.consumers).toContainEqual(
      expect.objectContaining({
        id: "filter-updates-canonical-coalescer-lifecycle",
        path: "src/app_impl/filter_input_updates.rs",
        pass: true,
      }),
    );
  });

  test("launcher ASCII matching has one GPUI-free domain owner and compatibility adapter", () => {
    const audit = auditStateOwnership(passingSources());

    expect(audit.pass).toBe(true);
    expect(audit.owners).toContainEqual(
      expect.objectContaining({
        id: "domain-ascii-search",
        path: "crates/sk-protocol/src/ascii_search.rs",
        symbols: expect.arrayContaining([
          "contains_ignore_ascii_case",
          "find_ignore_ascii_case",
          "fuzzy_match_with_indices_ascii",
          "is_fuzzy_match",
          "is_word_boundary_match",
        ]),
      }),
    );
  });

  test("structured launcher queries have one domain owner, adapter, and real search consumer", () => {
    const audit = auditStateOwnership(passingSources());

    expect(audit.pass).toBe(true);
    expect(audit.owners).toContainEqual(
      expect.objectContaining({
        id: "domain-query-prefix",
        path: "crates/sk-protocol/src/query_prefix.rs",
        symbols: expect.arrayContaining([
          "ParsedQuery",
          "parse_query_prefix",
          "should_search_scripts",
          "should_search_scriptlets",
        ]),
      }),
    );
    expect(audit.consumers).toContainEqual(
      expect.objectContaining({
        id: "launcher-search-canonical-query-prefix-routing",
        path: "src/scripts/search/unified.rs",
        pass: true,
      }),
    );
  });

  test("refuses a launcher prefix adapter that no longer imports the domain parser", () => {
    const audit = auditStateOwnership(
      mutateSource("src/scripts/search/prefix_filters.rs", (source) =>
        source.replace("parse_query_prefix, ", ""),
      ),
    );

    expect(audit.failures).toContain(
      "query-prefix-adapter-missing-domain-owner:parse_query_prefix",
    );
    expect(audit.pass).toBe(false);
  });

  test("refuses a launcher search that skips a canonical structured category", () => {
    const audit = auditStateOwnership(
      mutateSource("src/scripts/search/unified.rs", (source) =>
        source.replace("flow_passes_prefix_filter(parsed); ", ""),
      ),
    );

    expect(audit.failures).toContain(
      "missing-canonical-consumer:launcher-search-canonical-query-prefix-routing:src/scripts/search/unified.rs",
    );
    expect(audit.pass).toBe(false);
  });

  test("bounded collection cannot invoke an injected external discovery runner", () => {
    const sources = new Map(
      passingSources().map(({ path, content }) => [path, content!]),
    );
    let externalRuns = 0;

    const inventory = collectCurrentStateOwnershipSources({
      sourceExists: (path) => sources.has(path),
      readSource: (path) => sources.get(path)!,
      externalRunner: () => {
        externalRuns += 1;
        throw new Error("must not scan, spawn, or launch");
      },
    });

    expect(externalRuns).toBe(0);
    expect(auditStateOwnership(inventory).pass).toBe(true);
  });

  test.each(CANONICAL_STATE_OWNERS)(
    "rejects missing canonical owner $id",
    ({ path }) => {
      const audit = auditStateOwnership(mutateSource(path, () => undefined));

      expect(audit.pass).toBe(false);
      expect(audit.failures).toContain(`missing-source:${path}`);
    },
  );

  test.each([
    ["CommandDescriptor", "src/app_render/focused_info.rs"],
    ["CommandIdentity", "src/config/command_ids.rs"],
    ["SearchResult", "src/app_impl/filtering_cache.rs"],
    ["RootProviderCoordinator", "src/main_sections/root_search_store.rs"],
    ["RootSearchStore", "src/scripts/root_search_contract.rs"],
    ["ConversationCommandDescriptor", "src/actions/builders/script_context.rs"],
    ["GroupedListItem", "src/components/unified_list_item/mod.rs"],
  ])("rejects duplicate state owner %s at %s", (symbol, path) => {
    const audit = auditStateOwnership(
      mutateSource(path, (source) => `${source}\npub struct ${symbol};`),
    );

    expect(audit.failures).toContain(`duplicate-state-owner:${symbol}:${path}`);
    expect(audit.pass).toBe(false);
  });

  test("rejects ambiguous duplicate declarations at the canonical owner", () => {
    const path = "crates/sk-protocol/src/command_contract.rs";
    const audit = auditStateOwnership(
      mutateSource(path, (source) => `${source}\npub struct CommandIdentity;`),
    );

    expect(audit.failures).toContain(`ambiguous-canonical-owner:CommandIdentity:${path}`);
  });

  test("rejects an application dependency from an app-independent domain owner", () => {
    const path = "crates/sk-protocol/src/command_contract.rs";
    const audit = auditStateOwnership(
      mutateSource(path, (source) => `${source}\nuse crate::scripts::SearchResult;`),
    );

    expect(audit.failures).toContain(`domain-imports-application-owner:${path}`);
  });

  test("rejects a second launcher command projection at a consumer", () => {
    const path = "src/main_window_preflight/build.rs";
    const audit = auditStateOwnership(
      mutateSource(path, (source) => `${source}\nfn command_descriptor() {}`),
    );

    expect(audit.failures).toContain(`duplicate-search-result-projection:${path}`);
  });

  test.each(["command_contract", "query_prefix", "search_contract"])(
    "rejects missing domain registry %s",
    (module) => {
      const audit = auditStateOwnership(
        mutateSource("crates/sk-protocol/src/lib.rs", (source) =>
          source.replace(`pub mod ${module};`, ""),
        ),
      );

      expect(audit.failures).toContain(`missing-domain-module:${module}`);
    },
  );

  test.each(["command_contract", "root_search_contract", "types"])(
    "rejects missing launcher registry %s",
    (module) => {
      const audit = auditStateOwnership(
        mutateSource("src/scripts/mod.rs", (source) =>
          source.replace(new RegExp(`(?:pub\\(crate\\)\\s+)?mod ${module};`), ""),
        ),
      );

      expect(audit.failures).toContain(`missing-launcher-module:${module}`);
    },
  );

  test("rejects an unsanctioned alias of the domain descriptor", () => {
    const path = "src/app_render/focused_info.rs";
    const audit = auditStateOwnership(
      mutateSource(path, (source) =>
        `${source}\nuse crate::components::WrongDescriptor as CommandDescriptor;`,
      ),
    );

    expect(audit.failures).toContain(`unsanctioned-canonical-alias:CommandDescriptor:${path}`);
  });

  test("ignores counterfeit owners hidden in comments and string literals", () => {
    const path = "src/app_render/focused_info.rs";
    const audit = auditStateOwnership(
      mutateSource(path, (source) =>
        `${source}\n// pub struct CommandDescriptor;\nconst LABEL: &str = "pub struct CommandDescriptor;";`,
      ),
    );

    expect(audit.failures).toEqual([]);
  });

  test("preserves all six persisted categories but rejects promoting a passive source", () => {
    const path = "src/config/command_ids.rs";
    const missing = auditStateOwnership(
      mutateSource(path, (source) => source.replace("CommandCategory::App,", "")),
    );
    const expanded = auditStateOwnership(
      mutateSource(path, (source) =>
        source.replace("CommandCategory::App,", "CommandCategory::App, CommandCategory::Flow,"),
      ),
    );

    expect(missing.failures).toContain(
      "missing-sanctioned-exception:externalCommandCategoryRemainsExactSupportedSubset",
    );
    expect(expanded.failures).toContain(
      "missing-sanctioned-exception:externalCommandCategoryRemainsExactSupportedSubset",
    );
  });

  test("allows existing compatibility reexports and rejects deleting either", () => {
    expect(auditStateOwnership(passingSources()).pass).toBe(true);

    const launcher = auditStateOwnership(
      mutateSource("src/scripts/mod.rs", (source) => source.replace("SearchResult, ", "")),
    );
    const category = auditStateOwnership(
      mutateSource("src/config/mod.rs", (source) =>
        source.replace("pub use command_ids::{CommandCategory};", ""),
      ),
    );

    expect(launcher.failures).toContain(
      "missing-sanctioned-exception:launcherSearchResultCompatibilityReexportsPreserved",
    );
    expect(category.failures).toContain(
      "missing-sanctioned-exception:persistedCommandCategoryCompatibilityReexportPreserved",
    );
  });

  test("preserves both row owners and the existing backwards-compatible bridge", () => {
    const unified = auditStateOwnership(
      mutateSource("src/components/mod.rs", (source) =>
        source.replace("pub mod unified_list_item;", ""),
      ),
    );
    const legacy = auditStateOwnership(
      mutateSource("src/main.rs", (source) => source.replace("mod list_item;", "")),
    );
    const bridge = auditStateOwnership(
      mutateSource("src/components/unified_list_item/mod.rs", (source) =>
        source.replace("GroupedListItem, ", ""),
      ),
    );

    for (const audit of [unified, legacy, bridge]) {
      expect(audit.failures).toContain(
        "missing-sanctioned-exception:bothExistingRowSystemsAndCanonicalBridgePreserved",
      );
    }
  });

  test("preserves host-local conversation commands, native footer, and surface caches", () => {
    const conversation = auditStateOwnership(
      mutateSource("src/components/conversation_actions.rs", (source) =>
        source.replace("fn command_action", "fn detached_action"),
      ),
    );
    const footer = auditStateOwnership(
      mutateSource("src/main.rs", (source) => source.replace("mod footer_popup;", "")),
    );
    const cache = auditStateOwnership(
      mutateSource("src/main_sections/app_state.rs", (source) =>
        source.replace("cached_filtered_results", "detached_results"),
      ),
    );

    expect(conversation.failures).toContain(
      "missing-sanctioned-exception:existingConversationDescriptorBridgesDomainAction",
    );
    expect(footer.failures).toContain(
      "missing-sanctioned-exception:nativeFooterAndSharedFooterExceptionPreserved",
    );
    expect(cache.failures).toContain(
      "missing-sanctioned-exception:surfaceLocalPresentationCachesPreserved",
    );
  });

  test.each(REQUIRED_STATE_CONSUMERS)(
    "requires actual canonical consumer %s",
    (path) => {
      const audit = auditStateOwnership(mutateSource(path, () => "fn unrelated() {}"));

      expect(
        audit.failures.some((failure) =>
          failure.startsWith("missing-canonical-consumer:") && failure.endsWith(`:${path}`),
        ),
      ).toBe(true);
    },
  );

  test("rejects omitted, duplicate, or undeclared inventory sources", () => {
    const sources = passingSources();
    const path = REQUIRED_STATE_OWNERSHIP_PATHS[0]!;

    expect(
      auditStateOwnership(sources.filter((source) => source.path !== path)).failures,
    ).toContain(`missing-source:${path}`);
    expect(auditStateOwnership([...sources, sources[0]!]).failures).toContain(
      `duplicate-source:${sources[0]!.path}`,
    );
    expect(
      auditStateOwnership([
        ...sources,
        { path: "src/not-a-reviewed-owner.rs", content: "pub struct Intruder;" },
      ]).failures,
    ).toContain("unexpected-source:src/not-a-reviewed-owner.rs");
  });
});
