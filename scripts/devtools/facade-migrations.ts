import { createHash } from "node:crypto";

/**
 * The GOV-002 migration has two independent retired facades.  A ledger that
 * proves the conversation-style move alone is incomplete even when every one
 * of its individual assertions is true.
 *
 * This module deliberately accepts source bytes rather than walking the
 * checkout.  Callers own their bounded inventory; tests can mutate real Rust
 * syntax without launching an application, cargo, or an external process.
 */

export const CONVERSATION_STYLE_FACADE =
  "src/ai/agent_chat/ui/style_contract.rs";
export const CONVERSATION_STYLE_OWNER =
  "src/components/conversation_style.rs";
export const POPUP_WINDOW_FACADE = "src/ai/agent_chat/ui/popup_window.rs";
export const POPUP_WINDOW_OWNER = "src/components/inline_popup_window.rs";
export const POPUP_AUTOMATION_POLICY =
  "src/ai/agent_chat/ui/popup_automation.rs";
export const SHARED_COMPONENTS_MODULE = "src/components/mod.rs";

export const REQUIRED_POPUP_CONSUMERS = [
  "src/ai/agent_chat/ui/view.rs",
  "src/ai/agent_chat/ui/chat_window.rs",
  "src/ai/agent_chat/ui/history_popup.rs",
  "src/menu_syntax/object_selector.rs",
] as const;

export const REQUIRED_FACADE_MIGRATIONS = [
  {
    id: "conversation-style",
    removedFacadePath: CONVERSATION_STYLE_FACADE,
    canonicalOwnerPath: CONVERSATION_STYLE_OWNER,
    canonicalModule: "conversation_style",
  },
  {
    id: "popup-window",
    removedFacadePath: POPUP_WINDOW_FACADE,
    canonicalOwnerPath: POPUP_WINDOW_OWNER,
    canonicalModule: "inline_popup_window",
  },
] as const;

/**
 * Every producer and receipt validator must inspect this exact minimum set.
 * Retired files remain present in the inventory with undefined bytes so
 * omission cannot masquerade as proven deletion.
 */
export const REQUIRED_FACADE_SOURCE_PATHS = [
  SHARED_COMPONENTS_MODULE,
  CONVERSATION_STYLE_FACADE,
  CONVERSATION_STYLE_OWNER,
  POPUP_WINDOW_FACADE,
  POPUP_WINDOW_OWNER,
  POPUP_AUTOMATION_POLICY,
  ...REQUIRED_POPUP_CONSUMERS,
] as const;

export interface FacadeMigrationSource {
  readonly path: string;
  /** An absent file is distinct from a real, empty file. */
  readonly content: string | undefined;
}

export interface FacadeMigrationFinding {
  readonly id: (typeof REQUIRED_FACADE_MIGRATIONS)[number]["id"];
  readonly facadePath: string;
  readonly canonicalOwner: string;
  readonly facadeExists: boolean;
  readonly canonicalOwnerExists: boolean;
  readonly canonicalOwnerDefinesImplementation: boolean;
  readonly canonicalModuleRegistered: boolean;
  readonly directCallerPaths: readonly string[];
  readonly canonicalConsumerPaths: readonly string[];
}

export interface FacadeMigrationSourceDigest {
  readonly path: string;
  readonly state: "PRESENT" | "ABSENT";
  readonly sha256: string | null;
  readonly byteLength: number | null;
}

export interface FacadeMigrationScope {
  readonly schemaVersion: 1;
  readonly evidenceType: "STATIC_INVENTORY";
  readonly provesRuntimeBehavior: false;
  readonly provesExporterByteEquality: false;
  /** This producer inventories named owners/callers; it is not a full graph scan. */
  readonly coverageMode: "BOUNDED_CANONICAL_CONSUMERS";
  readonly sourceGraphExhaustive: false;
  readonly inspectedSourcePaths: readonly string[];
  readonly sourceDigests: readonly FacadeMigrationSourceDigest[];
  readonly disposition: "REMOVED" | "INCOMPLETE";
  readonly facades: readonly FacadeMigrationFinding[];
  readonly popupAutomationPolicyPath: typeof POPUP_AUTOMATION_POLICY;
  readonly popupAutomationPolicyPreserved: boolean;
  readonly missingPopupConsumerPaths: readonly string[];
  readonly conversationPersistedTokenPaths: readonly string[];
  readonly popupPersistedTokenPaths: readonly string[];
  readonly failures: readonly string[];
}

/** Remove comments/literals before interpreting module paths as Rust syntax. */
export function stripFacadeMigrationRustTrivia(source: string): string {
  let result = "";
  let index = 0;
  let blockDepth = 0;

  while (index < source.length) {
    const current = source[index];
    const next = source[index + 1];

    if (blockDepth > 0) {
      if (current === "/" && next === "*") {
        blockDepth++;
        result += "  ";
        index += 2;
      } else if (current === "*" && next === "/") {
        blockDepth--;
        result += "  ";
        index += 2;
      } else {
        result += current === "\n" ? "\n" : " ";
        index++;
      }
      continue;
    }

    if (current === "/" && next === "/") {
      while (index < source.length && source[index] !== "\n") {
        result += " ";
        index++;
      }
      continue;
    }

    if (current === "/" && next === "*") {
      blockDepth = 1;
      result += "  ";
      index += 2;
      continue;
    }

    // Raw Rust strings may legitimately document old module paths.
    if (
      (current === "r" ||
        ((current === "b" || current === "c") && next === "r")) &&
      !/[a-zA-Z0-9_]/.test(source[index - 1] ?? "")
    ) {
      const prefixLength = current === "r" ? 1 : 2;
      let delimiterIndex = index + prefixLength;
      while (source[delimiterIndex] === "#") delimiterIndex++;
      if (source[delimiterIndex] === '"') {
        const hashes = delimiterIndex - index - prefixLength;
        const terminator = '"' + "#".repeat(hashes);
        const ending = source.indexOf(terminator, delimiterIndex + 1);
        const end = ending === -1 ? source.length : ending + terminator.length;
        result += source
          .slice(index, end)
          .replace(/[^\n]/g, " ");
        index = end;
        continue;
      }
    }

    if (current === '"') {
      result += " ";
      index++;
      while (index < source.length) {
        const character = source[index];
        result += character === "\n" ? "\n" : " ";
        index++;
        if (character === "\\" && index < source.length) {
          result += source[index] === "\n" ? "\n" : " ";
          index++;
        } else if (character === '"') {
          break;
        }
      }
      continue;
    }

    // Preserve lifetimes ('a, 'static), but strip actual Rust character
    // literals so an escaped quote cannot hide the remainder of a source file.
    if (current === "'") {
      const characterLiteral = source
        .slice(index)
        .match(/^'(?:\\(?:u\{[a-fA-F0-9_]+\}|x[a-fA-F0-9]{2}|.)|[^'\\\n])'/);
      if (characterLiteral !== null) {
        result += " ".repeat(characterLiteral[0].length);
        index += characterLiteral[0].length;
        continue;
      }
    }

    result += current;
    index++;
  }

  return result;
}

function moduleRegistrationPattern(moduleName: string): RegExp {
  return new RegExp(`\\b(?:pub(?:\\s*\\([^)]*\\))?\\s+)?mod\\s+${moduleName}\\s*;`);
}

function legacyReferencePattern(moduleName: string): RegExp {
  return new RegExp(
    `(?:\\b(?:pub(?:\\s*\\([^)]*\\))?\\s+)?mod\\s+${moduleName}\\s*(?:;|\\{)|::\\s*${moduleName}\\b|::\\s*\\{[^}]*\\b${moduleName}\\b|\\b(?:use|pub\\s+use)\\s+${moduleName}\\b)`,
  );
}

function canonicalReferencePattern(moduleName: string): RegExp {
  return new RegExp(
    `(?:\\b(?:crate|super|self|[A-Za-z_][A-Za-z0-9_]*)::(?:components::)?${moduleName}\\b|\\b(?:crate|super|self|[A-Za-z_][A-Za-z0-9_]*)::components::\\{[^}]*\\b${moduleName}\\b)`,
  );
}

function canonicalOwnerDefinesImplementation(source: string): boolean {
  return /\bpub(?:\s*\([^)]*\))?\s+(?:(?:async|unsafe|const)\s+)*(?:fn|struct|enum|trait|const|static|type)\s+[A-Za-z_][A-Za-z0-9_]*/.test(
    source,
  );
}

function sortedUnique(values: Iterable<string>): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function digestFacadeMigrationSource(
  path: string,
  content: string | undefined,
): FacadeMigrationSourceDigest {
  if (content === undefined) {
    return { path, state: "ABSENT", sha256: null, byteLength: null };
  }
  return {
    path,
    state: "PRESENT",
    sha256: createHash("sha256").update(content, "utf8").digest("hex"),
    byteLength: Buffer.byteLength(content, "utf8"),
  };
}

/**
 * Fail closed on either removed facade. Popup policy intentionally remains:
 * deleting popup_automation.rs would remove Agent Chat-specific policy rather
 * than completing the shared-window migration.
 */
export function auditFacadeMigrationScope(
  sources: readonly FacadeMigrationSource[],
  persistedTokenPaths: readonly string[],
): FacadeMigrationScope {
  const sourceByPath = new Map<string, string | undefined>();
  const failures: string[] = [];

  for (const source of sources) {
    if (
      source.path.startsWith("/") ||
      source.path.includes("\\") ||
      source.path.split("/").some((segment) =>
        segment === "" || segment === "." || segment === "..",
      )
    ) {
      failures.push("invalid-source-path:" + source.path);
      continue;
    }
    if (sourceByPath.has(source.path)) {
      failures.push(`duplicate-source:${source.path}`);
      continue;
    }
    sourceByPath.set(source.path, source.content);
  }

  const componentModule = sourceByPath.get(SHARED_COMPONENTS_MODULE);
  if (componentModule === undefined) {
    failures.push(`missing-source:${SHARED_COMPONENTS_MODULE}`);
  }

  const syntaxByPath = new Map<string, string>();
  for (const [path, content] of sourceByPath) {
    if (content !== undefined) {
      syntaxByPath.set(path, stripFacadeMigrationRustTrivia(content));
    }
  }

  const componentSyntax =
    componentModule === undefined
      ? ""
      : stripFacadeMigrationRustTrivia(componentModule);

  const facades = REQUIRED_FACADE_MIGRATIONS.map((migration) => {
    const legacyModule = migration.removedFacadePath
      .split("/")
      .at(-1)!
      .replace(/\.rs$/, "");
    const facadeExists = sourceByPath.get(migration.removedFacadePath) !== undefined;
    const canonicalOwnerExists =
      sourceByPath.get(migration.canonicalOwnerPath) !== undefined;
    const canonicalOwnerDefinesImplementationResult =
      canonicalOwnerExists &&
      canonicalOwnerDefinesImplementation(
        syntaxByPath.get(migration.canonicalOwnerPath) ?? "",
      );
    const canonicalModuleRegistered = moduleRegistrationPattern(
      migration.canonicalModule,
    ).test(componentSyntax);
    const directCallerPaths = sortedUnique(
      [...syntaxByPath]
        .filter(([path, syntax]) =>
          path !== migration.removedFacadePath &&
          legacyReferencePattern(legacyModule).test(
            // This is a documented, sanctioned local alias to the canonical
            // owner, not the removed Agent Chat style_contract module.
            legacyModule === "style_contract"
              ? syntax.replace(/\bself\s+as\s+style_contract\b/g, "")
              : syntax,
          ),
        )
        .map(([path]) => path),
    );
    const canonicalConsumerPaths = sortedUnique(
      [...syntaxByPath]
        .filter(([path, syntax]) =>
          path !== migration.canonicalOwnerPath &&
          canonicalReferencePattern(migration.canonicalModule).test(syntax),
        )
        .map(([path]) => path),
    );

    if (facadeExists) failures.push(`facade-still-exists:${migration.removedFacadePath}`);
    if (!canonicalOwnerExists) {
      failures.push(`missing-canonical-owner:${migration.canonicalOwnerPath}`);
    }
    if (canonicalOwnerExists && !canonicalOwnerDefinesImplementationResult) {
      failures.push(
        "canonical-owner-has-no-owned-implementation:" +
          migration.canonicalOwnerPath,
      );
    }
    if (!canonicalModuleRegistered) {
      failures.push(`canonical-module-not-registered:${migration.canonicalModule}`);
    }
    for (const path of directCallerPaths) {
      failures.push(`legacy-caller:${migration.id}:${path}`);
    }
    if (canonicalConsumerPaths.length === 0) {
      failures.push(`missing-canonical-consumer:${migration.id}`);
    }

    return {
      id: migration.id,
      facadePath: migration.removedFacadePath,
      canonicalOwner: migration.canonicalOwnerPath,
      facadeExists,
      canonicalOwnerExists,
      canonicalOwnerDefinesImplementation:
        canonicalOwnerDefinesImplementationResult,
      canonicalModuleRegistered,
      directCallerPaths,
      canonicalConsumerPaths,
    } satisfies FacadeMigrationFinding;
  });

  const popupAutomationPolicyPreserved =
    sourceByPath.get(POPUP_AUTOMATION_POLICY) !== undefined;
  if (!popupAutomationPolicyPreserved) {
    failures.push(`missing-popup-automation-policy:${POPUP_AUTOMATION_POLICY}`);
  }

  const popupMigration = facades.find((facade) => facade.id === "popup-window")!;
  const popupConsumers = new Set(popupMigration.canonicalConsumerPaths);
  const missingPopupConsumerPaths = REQUIRED_POPUP_CONSUMERS.filter(
    (path) => !popupConsumers.has(path),
  );
  for (const path of missingPopupConsumerPaths) {
    failures.push(`missing-popup-canonical-consumer:${path}`);
  }

  const conversationPersistedTokenPaths = sortedUnique(
    persistedTokenPaths.filter((path) => path.includes("conversation_style::")),
  );
  const popupPersistedTokenPaths = sortedUnique(
    persistedTokenPaths.filter((path) => path.includes("inline_popup_window::")),
  );
  if (conversationPersistedTokenPaths.length === 0) {
    failures.push("missing-conversation-persisted-token-paths");
  }
  for (const path of persistedTokenPaths) {
    if (
      /(?:^|::)(?:style_contract|popup_window)::/.test(path) ||
      /(?:^|\/)(?:style_contract|popup_window)\.rs(?:$|[:#])/.test(path)
    ) {
      failures.push(`legacy-persisted-token-path:${path}`);
    }
  }

  return {
    schemaVersion: 1,
    evidenceType: "STATIC_INVENTORY",
    provesRuntimeBehavior: false,
    provesExporterByteEquality: false,
    coverageMode: "BOUNDED_CANONICAL_CONSUMERS",
    sourceGraphExhaustive: false,
    inspectedSourcePaths: sortedUnique(sourceByPath.keys()),
    sourceDigests: sortedUnique(sourceByPath.keys()).map((path) =>
      digestFacadeMigrationSource(path, sourceByPath.get(path)),
    ),
    disposition: failures.length === 0 ? "REMOVED" : "INCOMPLETE",
    facades,
    popupAutomationPolicyPath: POPUP_AUTOMATION_POLICY,
    popupAutomationPolicyPreserved,
    missingPopupConsumerPaths,
    conversationPersistedTokenPaths,
    // Popup geometry is not currently persisted in generated tokens. An empty
    // list is an honest inventory, never manufactured evidence.
    popupPersistedTokenPaths,
    failures: sortedUnique(failures),
  };
}

/**
 * Structural negative controls are deliberately separate from source
 * auditing: consistency.ts can reject a swapped, truncated, or counterfeit
 * ledger even if somebody bypasses the producer's bounded source preflight.
 */
export function validateCompleteFacadeMigrationScope(value: unknown): string[] {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return ["invalid-facade-migration-scope"];
  }

  const scope = value as Record<string, unknown>;
  const failures: string[] = [];
  if (scope.schemaVersion !== 1) {
    failures.push("invalid-facade-migration-schema-version");
  }
  if (scope.evidenceType !== "STATIC_INVENTORY") {
    failures.push("invalid-facade-evidence-type");
  }
  if (scope.provesRuntimeBehavior !== false) {
    failures.push("facade-runtime-proof-misrepresented");
  }
  if (scope.provesExporterByteEquality !== false) {
    failures.push("facade-exporter-proof-misrepresented");
  }
  if (scope.coverageMode !== "BOUNDED_CANONICAL_CONSUMERS") {
    failures.push("facade-source-coverage-misrepresented");
  }
  if (scope.sourceGraphExhaustive !== false) {
    failures.push("facade-source-graph-exhaustiveness-misrepresented");
  }
  if (
    !Array.isArray(scope.inspectedSourcePaths) ||
    scope.inspectedSourcePaths.some((path) => typeof path !== "string")
  ) {
    failures.push("missing-inspected-facade-source-inventory");
  } else {
    const inspectedPaths = new Set(scope.inspectedSourcePaths);
    for (const path of scope.inspectedSourcePaths) {
      if (
        !path.startsWith("src/") ||
        !path.endsWith(".rs") ||
        path.includes("\\") ||
        path.split("/").some((segment) =>
          segment === "" || segment === "." || segment === "..",
        )
      ) {
        failures.push("unsafe-inspected-facade-source-path:" + path);
      }
    }
    if (inspectedPaths.size !== scope.inspectedSourcePaths.length) {
      failures.push("duplicate-inspected-facade-source-paths");
    }
    for (const path of REQUIRED_FACADE_SOURCE_PATHS) {
      if (!inspectedPaths.has(path)) {
        failures.push(`missing-inspected-facade-source:${path}`);
      }
    }
  }
  if (!Array.isArray(scope.sourceDigests)) {
    failures.push("missing-facade-source-digests");
  } else {
    const digestedPaths = new Set<string>();
    for (const candidate of scope.sourceDigests) {
      if (
        candidate === null ||
        typeof candidate !== "object" ||
        Array.isArray(candidate)
      ) {
        failures.push("invalid-facade-source-digest");
        continue;
      }
      const digest = candidate as Record<string, unknown>;
      if (typeof digest.path !== "string" || digestedPaths.has(digest.path)) {
        failures.push("duplicate-or-invalid-facade-source-digest-path");
        continue;
      }
      if (
        !digest.path.startsWith("src/") ||
        !digest.path.endsWith(".rs") ||
        digest.path.includes("\\") ||
        digest.path.split("/").some((segment) =>
          segment === "" || segment === "." || segment === "..",
        )
      ) {
        failures.push("unsafe-facade-source-digest-path:" + digest.path);
        continue;
      }
      digestedPaths.add(digest.path);
      if (digest.state === "PRESENT") {
        if (
          typeof digest.sha256 !== "string" ||
          !/^[a-f0-9]{64}$/.test(digest.sha256) ||
          typeof digest.byteLength !== "number" ||
          !Number.isSafeInteger(digest.byteLength) ||
          digest.byteLength < 0
        ) {
          failures.push("invalid-present-facade-source-digest:" + digest.path);
        }
      } else if (
        digest.state !== "ABSENT" ||
        digest.sha256 !== null ||
        digest.byteLength !== null
      ) {
        failures.push("invalid-absent-facade-source-digest:" + digest.path);
      }
      if (
        (digest.path === CONVERSATION_STYLE_FACADE ||
          digest.path === POPUP_WINDOW_FACADE) &&
        digest.state !== "ABSENT"
      ) {
        failures.push("retired-facade-source-digest-is-present:" + digest.path);
      }
      if (
        (digest.path === CONVERSATION_STYLE_OWNER ||
          digest.path === POPUP_WINDOW_OWNER ||
          digest.path === POPUP_AUTOMATION_POLICY) &&
        digest.state !== "PRESENT"
      ) {
        failures.push("required-facade-source-digest-is-absent:" + digest.path);
      }
    }
    if (Array.isArray(scope.inspectedSourcePaths)) {
      for (const path of scope.inspectedSourcePaths) {
        if (typeof path === "string" && !digestedPaths.has(path)) {
          failures.push("missing-facade-source-digest:" + path);
        }
      }
      for (const path of digestedPaths) {
        if (!scope.inspectedSourcePaths.includes(path)) {
          failures.push("uninspected-facade-source-digest:" + path);
        }
      }
    }
  }
  if (scope.disposition !== "REMOVED") {
    failures.push("facade-migration-not-removed");
  }
  if (scope.popupAutomationPolicyPath !== POPUP_AUTOMATION_POLICY) {
    failures.push("incorrect-popup-automation-policy-path");
  }
  if (scope.popupAutomationPolicyPreserved !== true) {
    failures.push("popup-automation-policy-not-preserved");
  }
  if (!Array.isArray(scope.failures) || scope.failures.length !== 0) {
    failures.push("facade-migration-has-unresolved-failures");
  }
  if (
    !Array.isArray(scope.conversationPersistedTokenPaths) ||
    scope.conversationPersistedTokenPaths.length === 0 ||
    scope.conversationPersistedTokenPaths.some(
      (path) => typeof path !== "string" || !path.includes("conversation_style::"),
    )
  ) {
    failures.push("missing-canonical-conversation-token-evidence");
  }
  if (
    !Array.isArray(scope.popupPersistedTokenPaths) ||
    scope.popupPersistedTokenPaths.some(
      (path) =>
        typeof path !== "string" || !path.includes("inline_popup_window::"),
    )
  ) {
    failures.push("invalid-popup-persisted-token-evidence");
  }
  if (!Array.isArray(scope.missingPopupConsumerPaths)) {
    failures.push("missing-popup-consumer-inventory");
  } else if (scope.missingPopupConsumerPaths.length !== 0) {
    failures.push("popup-consumer-migration-incomplete");
  }

  if (
    !Array.isArray(scope.facades) ||
    scope.facades.length !== REQUIRED_FACADE_MIGRATIONS.length
  ) {
    failures.push("incomplete-required-facade-migration-set");
    return sortedUnique(failures);
  }

  for (const required of REQUIRED_FACADE_MIGRATIONS) {
    const matches = scope.facades.filter(
      (candidate): candidate is Record<string, unknown> =>
        candidate !== null &&
        typeof candidate === "object" &&
        !Array.isArray(candidate) &&
        (candidate as Record<string, unknown>).id === required.id,
    );
    if (matches.length !== 1) {
      failures.push(`missing-or-duplicate-facade-migration:${required.id}`);
      continue;
    }

    const migration = matches[0];
    if (migration.facadePath !== required.removedFacadePath) {
      failures.push(`incorrect-retired-facade-path:${required.id}`);
    }
    if (migration.canonicalOwner !== required.canonicalOwnerPath) {
      failures.push(`incorrect-canonical-facade-owner:${required.id}`);
    }
    if (migration.facadeExists !== false) {
      failures.push(`retired-facade-still-present:${required.id}`);
    }
    if (migration.canonicalOwnerExists !== true) {
      failures.push(`canonical-facade-owner-missing:${required.id}`);
    }
    if (migration.canonicalOwnerDefinesImplementation !== true) {
      failures.push(
        "canonical-facade-owner-has-no-owned-implementation:" + required.id,
      );
    }
    if (migration.canonicalModuleRegistered !== true) {
      failures.push(`canonical-facade-module-unregistered:${required.id}`);
    }
    if (
      !Array.isArray(migration.directCallerPaths) ||
      migration.directCallerPaths.length !== 0
    ) {
      failures.push(`legacy-facade-callers-remain:${required.id}`);
    }
    if (
      !Array.isArray(migration.canonicalConsumerPaths) ||
      migration.canonicalConsumerPaths.length === 0 ||
      migration.canonicalConsumerPaths.some((path) => typeof path !== "string")
    ) {
      failures.push(`canonical-facade-consumers-missing:${required.id}`);
    }
    if (required.id === "popup-window") {
      const consumers = new Set(
        Array.isArray(migration.canonicalConsumerPaths)
          ? migration.canonicalConsumerPaths
          : [],
      );
      for (const requiredConsumer of REQUIRED_POPUP_CONSUMERS) {
        if (!consumers.has(requiredConsumer)) {
          failures.push(`missing-popup-canonical-consumer:${requiredConsumer}`);
        }
      }
    }
  }

  return sortedUnique(failures);
}

/** Reconcile ledger hashes against provided current bytes, without disk access. */
export function validateFacadeMigrationSourceIdentity(
  scope: FacadeMigrationScope,
  sources: readonly FacadeMigrationSource[],
): string[] {
  const failures = validateCompleteFacadeMigrationScope(scope);
  const currentByPath = new Map(
    sources.map((source) => [
      source.path,
      digestFacadeMigrationSource(source.path, source.content),
    ]),
  );
  if (currentByPath.size !== sources.length) {
    failures.push("duplicate-current-facade-source");
  }

  for (const recorded of scope.sourceDigests) {
    const current = currentByPath.get(recorded.path);
    if (current === undefined) {
      failures.push("missing-current-facade-source:" + recorded.path);
      continue;
    }
    if (
      current.state !== recorded.state ||
      current.sha256 !== recorded.sha256 ||
      current.byteLength !== recorded.byteLength
    ) {
      failures.push("facade-source-identity-drift:" + recorded.path);
    }
  }
  for (const path of currentByPath.keys()) {
    if (!scope.inspectedSourcePaths.includes(path)) {
      failures.push("unexpected-current-facade-source:" + path);
    }
  }
  return sortedUnique(failures);
}

/** Preserve existing ledger fields while attaching the complete migration. */
export function attachFacadeMigrationScope<T extends Record<string, unknown>>(
  ledger: T,
  scope: FacadeMigrationScope,
): T & {
  facadeMigrations: FacadeMigrationScope;
  facades: readonly FacadeMigrationFinding[];
} {
  const failures = validateCompleteFacadeMigrationScope(scope);
  if (failures.length > 0) {
    throw new Error(`GOV-002 facade migration is incomplete: ${failures.join(", ")}`);
  }
  if (ledger.taskId !== "GOV-002") {
    throw new Error("Complete facade migration evidence belongs only to GOV-002");
  }
  if (
    ledger.evidenceType !== undefined &&
    ledger.evidenceType !== "STATIC_INVENTORY"
  ) {
    throw new Error("Facade ledger evidence must remain STATIC_INVENTORY");
  }
  if (
    ledger.evidenceClass !== undefined &&
    ledger.evidenceClass !== "STATIC_INVENTORY"
  ) {
    throw new Error("Facade ledger evidence class must remain STATIC_INVENTORY");
  }
  if (ledger.facades !== undefined || ledger.facadeMigrations !== undefined) {
    throw new Error("Refusing to overwrite an existing facade migration scope");
  }
  if (ledger.provesRuntimeBehavior === true || ledger.provesExporterByteEquality === true) {
    throw new Error("GOV-002 static migration evidence cannot certify runtime or exporter bytes");
  }
  return {
    ...ledger,
    facadeMigrations: scope,
    facades: scope.facades,
  };
}
