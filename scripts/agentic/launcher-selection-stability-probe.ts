#!/usr/bin/env bun

import {
  Driver,
  type ActiveListScrollReceipt,
  type Json,
} from "../devtools/driver";

const RUN_ID = Date.now().toString(36);
const ASYNC_QUERY = `zzlauncherselectionstability-${RUN_ID}.test`;
const NEW_QUERY = `zzlauncherselectionnewquery-${RUN_ID}.test`;
const PAINTED_STABLE_KEY = "fallback/root-file-search-handoff/global";
const PAINTED_SEMANTIC_ID = `main-list-row:${PAINTED_STABLE_KEY}`;
const PROVIDER_RESULT_URL = `https://${ASYNC_QUERY}/inserted`;
const TIMEOUT_MS = 8_000;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function asFiniteNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function elementsFromReceipt(receipt: Json): Json[] {
  for (const candidate of [
    receipt.elements,
    receipt.nodes,
    receipt.elementSnapshot?.nodes,
  ]) {
    if (Array.isArray(candidate)) return candidate as Json[];
  }
  return [];
}

function choiceElements(receipt: Json): Json[] {
  return elementsFromReceipt(receipt).filter(
    (element) =>
      element?.elementType === "choice" ||
      element?.type === "choice" ||
      (element?.role === "row" && Object.hasOwn(element, "selected")),
  );
}

function selectedChoiceElements(receipt: Json): Json[] {
  return choiceElements(receipt).filter(
    (element) => element?.selected === true || element?.selected === "true",
  );
}

function requireSelectedReceipt(
  receipt: ActiveListScrollReceipt,
  label: string,
): {
  selectedIndex: number;
  selectedSemanticId: string;
  selectedStableKey: string;
  itemCount: number;
} {
  const selectedIndex = asFiniteNumber(receipt.selectedIndex);
  const itemCount = asFiniteNumber(receipt.itemCount);
  const selectedSemanticId = receipt.selectedSemanticId;
  const selectedStableKey = receipt.selectedStableKey;
  if (
    selectedIndex === null ||
    itemCount === null ||
    typeof selectedSemanticId !== "string" ||
    typeof selectedStableKey !== "string"
  ) {
    throw new Error(
      `${label} did not expose a complete selected-row receipt: ${JSON.stringify(receipt)}`,
    );
  }
  return {
    selectedIndex,
    selectedSemanticId,
    selectedStableKey,
    itemCount,
  };
}

async function typeQuery(driver: Driver, text: string): Promise<void> {
  await driver.setFilterAndWait(text, { timeoutMs: TIMEOUT_MS });
}

async function waitForSelectedStableKey(
  driver: Driver,
  stableKey: string,
  timeoutMs = TIMEOUT_MS,
): Promise<ActiveListScrollReceipt> {
  const started = performance.now();
  let last: ActiveListScrollReceipt | null = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.getActiveListScroll({ timeoutMs: TIMEOUT_MS });
    if (last.selectedStableKey === stableKey) return last;
  }
  throw new Error(
    `Timed out waiting for selected stable key '${stableKey}'; last=${JSON.stringify(last)}`,
  );
}

async function waitForLogEvidence(
  driver: Driver,
  requiredFragments: string[],
  timeoutMs = TIMEOUT_MS,
): Promise<void> {
  const started = performance.now();
  let last: Json | null = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.getLogs({ limit: 500 }, { timeoutMs: TIMEOUT_MS });
    const serialized = JSON.stringify(last);
    if (requiredFragments.every((fragment) => serialized.includes(fragment)))
      return;
    await Bun.sleep(10);
  }
  throw new Error(
    `Timed out waiting for log evidence ${JSON.stringify(requiredFragments)}; last=${JSON.stringify(last)}`,
  );
}

async function waitForPublishedItemCount(
  driver: Driver,
  previousItemCount: number,
  timeoutMs = TIMEOUT_MS,
): Promise<ActiveListScrollReceipt> {
  const started = performance.now();
  let last: ActiveListScrollReceipt | null = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.getActiveListScroll({ timeoutMs: TIMEOUT_MS });
    if (Number(last.itemCount) > previousItemCount) return last;
  }
  throw new Error(
    `Timed out waiting for provider publication above itemCount ${previousItemCount}; last=${JSON.stringify(last)}`,
  );
}

function assertNewQuerySelectedFirst(
  scroll: ActiveListScrollReceipt,
  elementsReceipt: Json,
): {
  firstSelectableIndex: number;
  selectedChoiceSemanticId: string | null;
} {
  const selected = requireSelectedReceipt(scroll, "new query");
  if (selected.selectedStableKey !== PAINTED_STABLE_KEY) {
    throw new Error(
      `new query selected stable key ${selected.selectedStableKey}, expected ${PAINTED_STABLE_KEY}`,
    );
  }
  const choices = choiceElements(elementsReceipt)
    .map((element) => ({ element, index: asFiniteNumber(element.index) }))
    .filter(
      (entry): entry is { element: Json; index: number } =>
        entry.index !== null,
    );
  if (choices.length === 0) {
    throw new Error(
      `new query exposed no indexed choice elements: ${JSON.stringify(elementsReceipt)}`,
    );
  }
  const firstSelectableIndex = Math.min(
    ...choices.map((choice) => choice.index),
  );
  const selectedChoices = selectedChoiceElements(elementsReceipt);
  if (selectedChoices.length !== 1) {
    throw new Error(
      `new query expected exactly one selected choice, got ${selectedChoices.length}: ${JSON.stringify(elementsReceipt)}`,
    );
  }
  const selectedChoiceIndex = asFiniteNumber(selectedChoices[0]?.index);
  if (selectedChoiceIndex !== firstSelectableIndex) {
    throw new Error(
      `new query selected choice index ${selectedChoiceIndex}, expected first selectable choice index ${firstSelectableIndex}`,
    );
  }
  if (
    scroll.scrollTopItem !== 0 ||
    Number(scroll.scrollTopOffsetPx ?? 0) !== 0
  ) {
    throw new Error(
      `new query did not reset the viewport to the top: ${JSON.stringify(scroll)}`,
    );
  }
  const selectedChoiceSemanticId =
    typeof selectedChoices[0]?.semanticId === "string"
      ? selectedChoices[0].semanticId
      : typeof selectedChoices[0]?.semantic_id === "string"
        ? selectedChoices[0].semantic_id
        : null;
  return { firstSelectableIndex, selectedChoiceSemanticId };
}

async function main(): Promise<void> {
  let driver: Driver | null = null;
  const receipt: Json = {
    schemaVersion: 1,
    probe: "launcher-selection-stability",
    status: "fail",
    failure: null,
    asyncRefresh: null,
    newQuery: null,
    cleanup: {
      attempted: false,
      closed: false,
      aliveAfterClose: null,
      failure: null,
    },
    stats: null,
  };

  try {
    driver = await Driver.launch({
      sessionName: `launcher-selection-stability-${RUN_ID}`,
      sandboxHome: true,
      sharedModels: false,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1",
        RUST_LOG: "script_kit::selection=debug,warn",
        SCRIPT_KIT_BROWSER_TABS_TEST_PROVIDER: JSON.stringify({
          delayMs: 2_000,
          tabs: [
            {
              browser_name: "Google Chrome",
              browser_bundle_id: "com.google.Chrome",
              window_index: 1,
              tab_index: 1,
              title: `${ASYNC_QUERY} fixture tab`,
              url: PROVIDER_RESULT_URL,
            },
          ],
        }),
      },
      readyTimeoutMs: 15_000,
      defaultTimeoutMs: TIMEOUT_MS,
    });

    await driver.waitForState(
      { promptType: "scriptList" },
      { timeoutMs: TIMEOUT_MS, pollIntervalMs: 5 },
    );
    await typeQuery(driver, ASYNC_QUERY);

    const beforeReceipt = await driver.getActiveListScroll({
      timeoutMs: TIMEOUT_MS,
    });
    const before = requireSelectedReceipt(beforeReceipt, "pre-publish");
    if (before.selectedStableKey !== PAINTED_STABLE_KEY) {
      throw new Error(
        `pre-publish selected stable key ${before.selectedStableKey} did not match ${PAINTED_STABLE_KEY}: ${JSON.stringify(beforeReceipt)}`,
      );
    }
    if (before.selectedSemanticId !== PAINTED_SEMANTIC_ID) {
      throw new Error(
        `pre-publish semantic id ${before.selectedSemanticId} did not match ${PAINTED_SEMANTIC_ID}`,
      );
    }

    const afterReceipt = await waitForPublishedItemCount(
      driver,
      before.itemCount,
    );
    const after = requireSelectedReceipt(afterReceipt, "post-publish");
    await waitForLogEvidence(driver, [
      "main_menu_async_refresh_selection_reconciled",
      "browser_tabs_refresh_complete",
    ]);
    if (after.selectedSemanticId !== before.selectedSemanticId) {
      throw new Error(
        `provider publication swapped selected semantic id from ${before.selectedSemanticId} to ${after.selectedSemanticId}`,
      );
    }
    if (after.selectedStableKey !== before.selectedStableKey) {
      throw new Error(
        `provider publication swapped selected stable key from ${before.selectedStableKey} to ${after.selectedStableKey}`,
      );
    }
    if (after.selectedIndex <= before.selectedIndex) {
      throw new Error(
        `provider fixture did not insert a row above the painted target: before=${before.selectedIndex} after=${after.selectedIndex}`,
      );
    }

    await driver.setFilterAndWait("", { timeoutMs: TIMEOUT_MS });
    await typeQuery(driver, NEW_QUERY);
    const newQueryScroll = await waitForSelectedStableKey(
      driver,
      PAINTED_STABLE_KEY,
    );
    const newQueryElements = await driver.getElements(
      { limit: 100 },
      { timeoutMs: TIMEOUT_MS },
    );
    const newQueryProof = assertNewQuerySelectedFirst(
      newQueryScroll,
      newQueryElements,
    );
    const newQuerySelection = requireSelectedReceipt(
      newQueryScroll,
      "new query",
    );
    if (newQuerySelection.selectedIndex >= after.selectedIndex) {
      throw new Error(
        `new query did not reset from the shifted async index ${after.selectedIndex} to its first match ${newQuerySelection.selectedIndex}`,
      );
    }

    receipt.asyncRefresh = {
      query: ASYNC_QUERY,
      provider: "browser-tabs",
      providerResultUrl: PROVIDER_RESULT_URL,
      before,
      after,
      insertedAbove: after.selectedIndex > before.selectedIndex,
      semanticIdStable: after.selectedSemanticId === before.selectedSemanticId,
      stableKeyStable: after.selectedStableKey === before.selectedStableKey,
      providerPublishLogObserved: true,
      reconciliationLogObserved: true,
    };
    receipt.newQuery = {
      query: NEW_QUERY,
      selectedIndex: newQuerySelection.selectedIndex,
      firstSelectableIndex: newQueryProof.firstSelectableIndex,
      selectedSemanticId: newQuerySelection.selectedSemanticId,
      selectedStableKey: newQuerySelection.selectedStableKey,
      selectedChoiceSemanticId: newQueryProof.selectedChoiceSemanticId,
      scrollTopItem: newQueryScroll.scrollTopItem,
      scrollTopOffsetPx: newQueryScroll.scrollTopOffsetPx,
    };
    receipt.stats = driver.stats;
    receipt.status = "pass";
  } catch (error) {
    receipt.failure = errorMessage(error);
  } finally {
    receipt.cleanup.attempted = driver !== null;
    if (driver) {
      try {
        await driver.close();
        receipt.cleanup.closed = true;
        receipt.cleanup.aliveAfterClose = driver.alive;
        if (driver.alive) {
          throw new Error(
            `owned driver process ${driver.pid ?? "unknown"} survived close`,
          );
        }
      } catch (error) {
        receipt.cleanup.failure = errorMessage(error);
        receipt.failure ??= `cleanup: ${receipt.cleanup.failure}`;
        receipt.status = "fail";
      }
    }
    console.log(JSON.stringify(receipt));
  }

  if (receipt.status !== "pass") process.exitCode = 1;
}

await main();
