// Name: SDK Test - fields() Basic Input Types
// Description: Tests fields() with text, password, email, number input types

/**
 * SDK TEST: test-fields-basic.ts
 *
 * Tests the fields() SDK function with basic input types.
 * 
 * Verifies the SDK request/response contract, ordered result values, and field
 * defaults. Native styling and password masking require separate runtime proof.
 *
 * Test cases:
 * 1. fields-string-labels: Simple string labels (fields(["Name", "Email"]))
 * 2. fields-text-type: Typed fields with text type
 * 3. fields-password-type: Password field masking
 * 4. fields-email-type: Email field with placeholder
 * 5. fields-number-type: Number field with placeholder
 * 6. fields-prefilled-values: Pre-filled default values
 *
 * Expected behavior:
 * - fields() returns array of strings matching number of fields
 * - Pre-filled values remain in field-definition order
 */

import "../../scripts/kit-sdk.ts";

// =============================================================================
// Test Infrastructure
// =============================================================================

interface TestResult {
	test: string;
	status: "running" | "pass" | "fail" | "skip";
	timestamp: string;
	result?: unknown;
	error?: string;
	duration_ms?: number;
}

function logTest(
	name: string,
	status: TestResult["status"],
	extra?: Partial<TestResult>,
) {
	const result: TestResult = {
		test: name,
		status,
		timestamp: new Date().toISOString(),
		...extra,
	};
	console.log(JSON.stringify(result));
}

function debug(msg: string) {
	console.error(`[TEST] ${msg}`);
}

// =============================================================================
// Test Definitions - All field configurations to test
// =============================================================================

const testCases = [
	{
		name: "fields-string-labels",
		description: "Simple string labels",
		fields: ["Name", "Email"],
	},
	{
		name: "fields-text-type",
		description: "Typed fields with text type",
		fields: [
			{ name: "firstName", label: "First Name", type: "text" as const, placeholder: "Enter first name" },
			{ name: "lastName", label: "Last Name", type: "text" as const, placeholder: "Enter last name" },
		],
	},
	{
		name: "fields-password-type",
		description: "Password field masking",
		fields: [
			{ name: "username", label: "Username", type: "text" as const },
			{ name: "password", label: "Password", type: "password" as const, placeholder: "Enter password" },
			{ name: "confirmPassword", label: "Confirm Password", type: "password" as const },
		],
	},
	{
		name: "fields-email-type",
		description: "Email field with placeholder",
		fields: [
			{ name: "personalEmail", label: "Personal Email", type: "email" as const, placeholder: "you@example.com" },
			{ name: "workEmail", label: "Work Email", type: "email" as const, placeholder: "you@company.com" },
		],
	},
	{
		name: "fields-number-type",
		description: "Number field with placeholder",
		fields: [
			{ name: "age", label: "Age", type: "number" as const, placeholder: "Enter your age" },
			{ name: "quantity", label: "Quantity", type: "number" as const, placeholder: "0" },
		],
	},
	{
		name: "fields-prefilled-values",
		description: "Pre-filled default values",
		fields: [
			{ name: "name", label: "Name", type: "text" as const, value: "John Doe" },
			{ name: "email", label: "Email", type: "email" as const, value: "john@example.com" },
			{ name: "age", label: "Age", type: "number" as const, value: "30" },
			{ name: "website", label: "Website", type: "text" as const, value: "https://example.com", placeholder: "URL" },
		],
	},
];

// =============================================================================
// Run Tests
// =============================================================================

debug("test-fields-basic.ts starting...");
debug(`SDK globals: fields=${typeof fields}`);
debug(`Running ${testCases.length} test cases`);

// Run all tests sequentially; prompts share one host and must not overlap.

for (let i = 0; i < testCases.length; i++) {
	const tc = testCases[i];
	const testName = tc.name;

	logTest(testName, "running");
	const startTime = Date.now();

	debug(`\n--- Test ${i + 1}/${testCases.length}: ${tc.description} ---`);
	debug(`Field count: ${tc.fields.length}`);
	debug(`Fields: ${JSON.stringify(tc.fields)}`);

	try {
		const result = await fields(tc.fields);
		const expected = tc.fields.map((field) =>
			typeof field === "string" ? "" : field.value ?? "",
		);
		if (!Array.isArray(result) || JSON.stringify(result) !== JSON.stringify(expected)) {
			throw new Error(
				`Expected ordered values ${JSON.stringify(expected)}, got ${JSON.stringify(result)}`,
			);
		}

		logTest(testName, "pass", {
			result: {
				description: tc.description,
				fieldCount: tc.fields.length,
				values: result,
				proof: "sdk-auto-submit-request-response",
			},
			duration_ms: Date.now() - startTime,
		});

		debug(`Test ${testName} passed - ordered field response verified`);
	} catch (err) {
		logTest(testName, "fail", {
			error: String(err),
			duration_ms: Date.now() - startTime,
		});
		debug(`Test ${testName} failed: ${err}`);
	}
}

// =============================================================================
// Summary
// =============================================================================

debug("\n=== Test Summary ===");
debug(`Ran ${testCases.length} test cases for fields() SDK function`);
debug("All tests verify that the SDK correctly sends Fields messages.");
debug("SDK auto-submit verifies ordered response values and field defaults without GPUI interaction.");
debug("The GPUI Fields handler uses the shared form prompt; native styling and password masking require separate runtime proof.");
debug("");
debug("test-fields-basic.ts completed!");

process.exit(0);
