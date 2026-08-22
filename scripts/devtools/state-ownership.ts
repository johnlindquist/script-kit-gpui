import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { stripFacadeMigrationRustTrivia } from "./facade-migrations.ts";

/**
 * GOV-001 owns a deliberately bounded state graph, never a checkout scan.
 * Domain values, app adapters, binary-only presentation state, and sanctioned
 * compatibility owners are different responsibilities rather than duplicates.
 */
export const CANONICAL_STATE_OWNERS = [
  {
    id: "domain-command-contract",
    path: "crates/sk-protocol/src/command_contract.rs",
    symbols: ["CommandSource", "CommandIdentity", "CommandDescriptor", "CommandAvailability"],
  },
  {
    id: "domain-search-contract",
    path: "crates/sk-protocol/src/search_contract.rs",
    symbols: [
      "ProviderRequest",
      "ProviderGenerationFence",
      "RootOwnedProviderRefresh",
      "RootOwnedProviderRefreshLifecycle",
    ],
  },
  {
    id: "launcher-result-model",
    path: "src/scripts/types.rs",
    symbols: ["SearchResult", "MatchEvidence"],
  },
  {
    id: "launcher-command-adapter",
    path: "src/scripts/command_contract.rs",
    symbols: ["LauncherCommandReceipt"],
  },
  {
    id: "root-provider-coordinator",
    path: "crates/sk-protocol/src/search_contract.rs",
    symbols: ["RootProviderCoordinator"],
  },
  {
    id: "host-root-search-store",
    path: "src/main_sections/root_search_store.rs",
    symbols: ["RootSearchStore"],
  },
  {
    id: "host-root-passive-frame",
    path: "src/main_sections/app_state.rs",
    symbols: ["RootPassiveFrame"],
  },
  {
    id: "persisted-external-command-category",
    path: "src/config/command_ids.rs",
    symbols: ["CommandCategory"],
  },
  {
    id: "conversation-host-command",
    path: "src/components/conversation_actions.rs",
    symbols: ["ConversationCommandDescriptor"],
  },
  {
    id: "existing-main-list-rows",
    path: "src/list_item/mod.rs",
    symbols: ["GroupedListItem", "GroupedListState", "ListItem"],
  },
] as const;

export const REQUIRED_STATE_REGISTRIES = [
  "crates/sk-protocol/src/lib.rs",
  "src/scripts/root_search_contract.rs",
  "src/scripts/mod.rs",
  "src/config/mod.rs",
  "src/components/mod.rs",
  "src/components/unified_list_item/mod.rs",
  "src/main.rs",
] as const;

export const REQUIRED_STATE_CONSUMERS = [
  "src/main_window_preflight/build.rs",
  "src/app_render/focused_info.rs",
  "src/actions/builders/script_context.rs",
  "src/render_script_list/mod.rs",
  "src/app_impl/filtering_cache.rs",
] as const;

export const REQUIRED_STATE_OWNERSHIP_PATHS = [
  ...new Set([
    ...CANONICAL_STATE_OWNERS.map(({ path }) => path),
    ...REQUIRED_STATE_REGISTRIES,
    ...REQUIRED_STATE_CONSUMERS,
  ]),
].sort();

export const SANCTIONED_EXTERNAL_COMMAND_CATEGORIES = [
  "Builtin",
  "App",
  "Script",
  "Scriptlet",
  "PromptTarget",
  "PromptAction",
] as const;

export type StateOwnershipSource = {
  readonly path: string;
  readonly content: string | undefined;
};

export type StateOwnershipInventoryOptions = {
  readonly sourceExists?: (path: string) => boolean;
  readonly readSource?: (path: string) => string;
  /** Negative controls can poison this runner; collection must never call it. */
  readonly externalRunner?: (argv: readonly string[]) => unknown;
};

type StateOwnershipFinding = {
  readonly id: string;
  readonly path: string;
  readonly symbols: readonly string[];
  readonly declarations: Readonly<Record<string, readonly string[]>>;
};

type StateConsumerFinding = {
  readonly id: string;
  readonly path: string;
  readonly pass: boolean;
};

export type StateOwnershipAudit = {
  readonly schemaVersion: 1;
  readonly taskId: "GOV-001";
  readonly inventoryEvidenceClass: "STATIC_INVENTORY";
  readonly coverageMode: "BOUNDED_NAMED_OWNERS_AND_CONSUMERS";
  readonly sourceGraphExhaustive: false;
  readonly provesRuntimeBehavior: false;
  readonly externalProcessesStarted: 0;
  readonly inspectedSourcePaths: readonly string[];
  readonly sourceFingerprints: Readonly<Record<string, string>>;
  readonly owners: readonly StateOwnershipFinding[];
  readonly consumers: readonly StateConsumerFinding[];
  readonly sanctionedExceptions: Readonly<Record<string, boolean>>;
  readonly failures: readonly string[];
  readonly pass: boolean;
};

function sourceDigest(source: string): string {
  return createHash("sha256").update(source).digest("hex");
}

function escapeExpression(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function declarationSites(
  sources: ReadonlyMap<string, string>,
  symbol: string,
): string[] {
  const pattern = new RegExp(
    `\\b(?:pub(?:\\s*\\([^)]*\\))?\\s+)?(?:struct|enum|type)\\s+${escapeExpression(symbol)}\\b`,
    "g",
  );
  const sites: string[] = [];
  for (const [path, code] of sources) {
    for (const _match of code.matchAll(pattern)) sites.push(path);
  }
  return sites.sort();
}

function hasModule(code: string, moduleName: string): boolean {
  return new RegExp(
    `\\b(?:pub(?:\\s*\\([^)]*\\))?\\s+)?mod\\s+${escapeExpression(moduleName)}\\s*;`,
  ).test(code);
}

function hasGroupedSymbol(code: string, owner: string, symbol: string): boolean {
  const path = owner.split("::").map(escapeExpression).join("\\s*::\\s*");
  return new RegExp(
    `\\b(?:pub\\s+)?use\\s+${path}\\s*::\\s*(?:${escapeExpression(symbol)}\\b|\\{[^;]*\\b${escapeExpression(symbol)}\\b)`,
  ).test(code);
}

function exactCategories(code: string): string[] {
  const declaration = code.match(
    /\bSUPPORTED_COMMAND_CATEGORIES\b[^=]*=\s*&?\s*\[([\s\S]*?)\]/,
  );
  if (!declaration) return [];
  return [...declaration[1]!.matchAll(/\bCommandCategory\s*::\s*([A-Za-z_][A-Za-z_0-9]*)/g)]
    .map((match) => match[1]!);
}

export function collectCurrentStateOwnershipSources(
  options: StateOwnershipInventoryOptions = {},
): StateOwnershipSource[] {
  const sourceExists = options.sourceExists ?? existsSync;
  const readSource = options.readSource ?? ((path: string) => readFileSync(path, "utf8"));
  // `externalRunner` is deliberately not read or invoked: the exact inventory
  // is canonical, and rg/find/cargo/app discovery would violate its boundary.
  return REQUIRED_STATE_OWNERSHIP_PATHS.map((path) => ({
    path,
    content: sourceExists(path) ? readSource(path) : undefined,
  }));
}

export function auditStateOwnership(
  inventory: readonly StateOwnershipSource[],
): StateOwnershipAudit {
  const failures: string[] = [];
  const provided = new Map<string, string>();
  for (const source of inventory) {
    if (!REQUIRED_STATE_OWNERSHIP_PATHS.includes(source.path)) {
      failures.push(`unexpected-source:${source.path}`);
      continue;
    }
    if (provided.has(source.path)) {
      failures.push(`duplicate-source:${source.path}`);
      continue;
    }
    if (typeof source.content !== "string") {
      failures.push(`missing-source:${source.path}`);
      continue;
    }
    provided.set(source.path, source.content);
  }
  for (const path of REQUIRED_STATE_OWNERSHIP_PATHS) {
    if (!provided.has(path) && !failures.includes(`missing-source:${path}`)) {
      failures.push(`missing-source:${path}`);
    }
  }

  const codes = new Map(
    [...provided].map(([path, content]) => [path, stripFacadeMigrationRustTrivia(content)]),
  );
  const code = (path: string) => codes.get(path) ?? "";

  const owners: StateOwnershipFinding[] = CANONICAL_STATE_OWNERS.map((owner) => {
    const declarations = Object.fromEntries(
      owner.symbols.map((symbol) => {
        const sites = declarationSites(codes, symbol);
        if (!sites.includes(owner.path)) {
          failures.push(`missing-canonical-owner:${symbol}:${owner.path}`);
        }
        for (const site of sites) {
          if (site !== owner.path) failures.push(`duplicate-state-owner:${symbol}:${site}`);
        }
        if (sites.filter((site) => site === owner.path).length > 1) {
          failures.push(`ambiguous-canonical-owner:${symbol}:${owner.path}`);
        }
        return [symbol, sites];
      }),
    );
    return { id: owner.id, path: owner.path, symbols: owner.symbols, declarations };
  });

  const domainRegistry = code("crates/sk-protocol/src/lib.rs");
  for (const module of ["command_contract", "search_contract"]) {
    if (!hasModule(domainRegistry, module)) {
      failures.push(`missing-domain-module:${module}`);
    }
  }
  const scriptsRegistry = code("src/scripts/mod.rs");
  for (const module of ["command_contract", "root_search_contract", "types"]) {
    if (!hasModule(scriptsRegistry, module)) {
      failures.push(`missing-launcher-module:${module}`);
    }
  }
  const configRegistry = code("src/config/mod.rs");
  if (!hasModule(configRegistry, "command_ids")) {
    failures.push("missing-config-module:command_ids");
  }

  const adapter = code("src/scripts/command_contract.rs");
  const projectionSites = [...codes]
    .filter(([, source]) => /\bfn\s+command_descriptor\s*\(/.test(source))
    .map(([path]) => path);
  if (!projectionSites.includes("src/scripts/command_contract.rs")) {
    failures.push("missing-canonical-search-result-projection");
  }
  for (const path of projectionSites) {
    if (path !== "src/scripts/command_contract.rs") {
      failures.push(`duplicate-search-result-projection:${path}`);
    }
  }
  if (!/\bimpl\s+SearchResult\s*\{/.test(adapter)) {
    failures.push("launcher-adapter-does-not-extend-search-result");
  }
  for (const symbol of ["CommandIdentity", "CommandDescriptor", "CommandSource"]) {
    if (!hasGroupedSymbol(adapter, "sk_protocol::command_contract", symbol)) {
      failures.push(`launcher-adapter-missing-domain-import:${symbol}`);
    }
  }

  const coordinatorAdapter = code("src/scripts/root_search_contract.rs");
  for (const symbol of [
    "RootProviderCoordinator",
    "RootOwnedProviderRefresh",
    "RootOwnedProviderRefreshLifecycle",
  ]) {
    if (!hasGroupedSymbol(coordinatorAdapter, "sk_protocol::search_contract", symbol)) {
      failures.push(`coordinator-adapter-missing-domain-owner:${symbol}`);
    }
  }
  const hostStore = code("src/main_sections/root_search_store.rs");
  if (
    !/\bcrate\s*::\s*scripts\s*::\s*root_search_contract\s*::\s*RootProviderCoordinator\b/
      .test(hostStore)
  ) {
    failures.push("host-search-store-does-not-use-canonical-coordinator");
  }
  const mainRegistry = code("src/main.rs");
  if (!hasModule(mainRegistry, "root_search_store")) {
    failures.push("missing-binary-root-search-module");
  }
  if (!hasGroupedSymbol(mainRegistry, "root_search_store", "RootSearchStore")) {
    failures.push("missing-binary-root-search-import");
  }

  for (const [path, source] of codes) {
    for (const match of source.matchAll(
      /\bas\s+(CommandSource|CommandIdentity|CommandDescriptor|CommandAvailability|ProviderRequest|ProviderGenerationFence|SearchResult|MatchEvidence|RootProviderCoordinator|RootSearchStore)\b/g,
    )) {
      failures.push(`unsanctioned-canonical-alias:${match[1]}:${path}`);
    }
  }
  for (const domainPath of [
    "crates/sk-protocol/src/command_contract.rs",
    "crates/sk-protocol/src/search_contract.rs",
  ]) {
    if (/\b(?:script_kit_gpui|crate\s*::\s*(?:scripts|components|main_sections))\b/.test(code(domainPath))) {
      failures.push(`domain-imports-application-owner:${domainPath}`);
    }
  }

  const categoryOwner = code("src/config/command_ids.rs");
  const categories = exactCategories(categoryOwner);
  const categorySet = new Set(categories);
  const categorySubsetPass =
    categories.length === SANCTIONED_EXTERNAL_COMMAND_CATEGORIES.length &&
    SANCTIONED_EXTERNAL_COMMAND_CATEGORIES.every((category) => categorySet.has(category)) &&
    hasGroupedSymbol(categoryOwner, "sk_protocol::command_contract", "CommandIdentity") &&
    hasGroupedSymbol(categoryOwner, "sk_protocol::command_contract", "CommandSource");

  const conversationOwner = code("src/components/conversation_actions.rs");
  const conversationBridgePass =
    /\bfn\s+command_action\s*\(/.test(conversationOwner) &&
    /\bsk_protocol\s*::\s*command_contract\s*::\s*CommandAction\b/.test(conversationOwner);
  const searchResultReexportPass =
    hasGroupedSymbol(scriptsRegistry, "self::types", "SearchResult") &&
    hasGroupedSymbol(scriptsRegistry, "self::types", "MatchEvidence");
  const categoryReexportPass = hasGroupedSymbol(configRegistry, "command_ids", "CommandCategory");

  const componentsRegistry = code("src/components/mod.rs");
  const unifiedRows = code("src/components/unified_list_item/mod.rs");
  const dualRowOwnersPass =
    hasModule(componentsRegistry, "unified_list_item") &&
    hasModule(mainRegistry, "list_item") &&
    hasGroupedSymbol(unifiedRows, "crate::list_item", "GroupedListItem");
  const nativeFooterPass =
    hasModule(mainRegistry, "footer_popup") &&
    hasModule(componentsRegistry, "footer_chrome") &&
    /\bcrate\s*::\s*components\s*::\s*footer_chrome\b/
      .test(code("src/render_script_list/mod.rs"));
  const localCachesPass =
    /\bcached_filtered_results\b/.test(code("src/main_sections/app_state.rs")) &&
    /\broot_file_result_cache\b/.test(hostStore);

  const sanctionedExceptions = {
    externalCommandCategoryRemainsExactSupportedSubset: categorySubsetPass,
    existingConversationDescriptorBridgesDomainAction: conversationBridgePass,
    binaryOnlyRootSearchStoreConsumesLibraryCoordinator:
      /\bRootProviderCoordinator\b/.test(hostStore) && /\broot_search\s*:\s*RootSearchStore\b/
        .test(code("src/main_sections/app_state.rs")),
    launcherSearchResultCompatibilityReexportsPreserved: searchResultReexportPass,
    persistedCommandCategoryCompatibilityReexportPreserved: categoryReexportPass,
    bothExistingRowSystemsAndCanonicalBridgePreserved: dualRowOwnersPass,
    nativeFooterAndSharedFooterExceptionPreserved: nativeFooterPass,
    surfaceLocalPresentationCachesPreserved: localCachesPass,
  } as const;
  for (const [exception, pass] of Object.entries(sanctionedExceptions)) {
    if (!pass) failures.push(`missing-sanctioned-exception:${exception}`);
  }

  const consumerRequirements: Array<{
    id: string;
    path: (typeof REQUIRED_STATE_CONSUMERS)[number];
    patterns: RegExp[];
  }> = [
    {
      id: "main-preflight-command-and-passive-frame",
      path: "src/main_window_preflight/build.rs",
      patterns: [/\.redacted_command_receipt\s*\(/, /\broot_search\s*\.\s*root_passive_frame\s*\(/],
    },
    {
      id: "focused-info-shared-command-descriptor",
      path: "src/app_render/focused_info.rs",
      patterns: [/\.command_descriptor\s*\(/, /\.with_command_descriptor\s*\(/],
    },
    {
      id: "actions-shared-command-primary-action",
      path: "src/actions/builders/script_context.rs",
      patterns: [/\.command_descriptor\b/, /\.primary_action\s*\(/],
    },
    {
      id: "main-list-existing-rows-and-host-footer",
      path: "src/render_script_list/mod.rs",
      patterns: [
        /\bcrate\s*::\s*list_item\b/,
        /\bmain_window_primary_action_label\s*\(/,
        /\bcrate\s*::\s*components\s*::\s*footer_chrome\b/,
      ],
    },
    {
      id: "filtering-cache-canonical-provider-coordinator",
      path: "src/app_impl/filtering_cache.rs",
      patterns: [
        /\broot_search\s*\.\s*begin_provider_request\s*\(/,
        /\broot_search\s*\.\s*invalidate_provider_request\s*\(/,
        /\bsk_protocol\s*::\s*command_contract\s*::\s*CommandSource\b/,
      ],
    },
  ];
  const consumers = consumerRequirements.map(({ id, path, patterns }) => {
    const pass = patterns.every((pattern) => pattern.test(code(path)));
    if (!pass) failures.push(`missing-canonical-consumer:${id}:${path}`);
    return { id, path, pass };
  });

  return {
    schemaVersion: 1,
    taskId: "GOV-001",
    inventoryEvidenceClass: "STATIC_INVENTORY",
    coverageMode: "BOUNDED_NAMED_OWNERS_AND_CONSUMERS",
    sourceGraphExhaustive: false,
    provesRuntimeBehavior: false,
    externalProcessesStarted: 0,
    inspectedSourcePaths: [...provided.keys()].sort(),
    sourceFingerprints: Object.fromEntries(
      [...provided].sort(([left], [right]) => left.localeCompare(right))
        .map(([path, source]) => [path, sourceDigest(source)]),
    ),
    owners,
    consumers,
    sanctionedExceptions,
    failures: [...new Set(failures)].sort(),
    pass: failures.length === 0,
  };
}

export function inspectCurrentStateOwnership(): StateOwnershipAudit {
  return auditStateOwnership(collectCurrentStateOwnershipSources());
}
