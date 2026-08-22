// Name: SDK Test - form() specialized inputs
// Description: Verifies specialized fields/form SDK responses without screen capture.

import type { FieldDef } from "../../scripts/kit-sdk";

interface TestResult {
  test: string;
  status: "running" | "pass" | "fail";
  timestamp: string;
  result?: unknown;
  error?: string;
  duration_ms?: number;
}

function logTest(
  test: string,
  status: TestResult["status"],
  extra: Partial<TestResult> = {},
): void {
  console.log(JSON.stringify({ test, status, timestamp: new Date().toISOString(), ...extra }));
}

async function runFieldsTest(test: string, definitions: FieldDef[]): Promise<void> {
  logTest(test, "running");
  const started = Date.now();

  try {
    const values = await fields(definitions);
    const expected = definitions.map((definition) => definition.value ?? "");
    if (JSON.stringify(values) !== JSON.stringify(expected)) {
      throw new Error(
        `Expected ordered values ${JSON.stringify(expected)}, received ${JSON.stringify(values)}`,
      );
    }
    logTest(test, "pass", {
      result: { fieldTypes: definitions.map((definition) => definition.type), values },
      duration_ms: Date.now() - started,
    });
  } catch (error) {
    logTest(test, "fail", { error: String(error), duration_ms: Date.now() - started });
  }
}

await runFieldsTest("fields-url", [
  { name: "website", label: "Website URL", type: "url", value: "https://example.com" },
]);

await runFieldsTest("fields-search", [
  { name: "query", label: "Search Query", type: "search", value: "needle" },
]);

await runFieldsTest("fields-tel", [
  { name: "phone", label: "Phone Number", type: "tel", value: "303-555-0100" },
]);

await runFieldsTest("fields-color", [
  { name: "favoriteColor", label: "Favorite Color", type: "color", value: "#ff0000" },
]);

const formTest = "form-textarea";
logTest(formTest, "running");
const formStarted = Date.now();

try {
  const result = await form(
    '<form><label for="bio">Biography</label><textarea name="bio" id="bio"></textarea></form>',
  );
  if (typeof result !== "object" || result === null || Array.isArray(result)) {
    throw new Error(`Expected an object result from form(), got ${JSON.stringify(result)}`);
  }
  logTest(formTest, "pass", { result, duration_ms: Date.now() - formStarted });
} catch (error) {
  logTest(formTest, "fail", { error: String(error), duration_ms: Date.now() - formStarted });
}

await runFieldsTest("fields-combined", [
  { name: "website", label: "Website", type: "url", value: "https://example.com" },
  { name: "phone", label: "Phone", type: "tel", value: "303-555-0100" },
  { name: "themeColor", label: "Theme Color", type: "color", value: "#4f46e5" },
  { name: "email", label: "Email", type: "email", value: "hello@example.com" },
  { name: "age", label: "Age", type: "number", value: "25" },
]);

console.error(
  "[TEST] Specialized field SDK response contracts verified; native control rendering requires separate runtime proof.",
);
