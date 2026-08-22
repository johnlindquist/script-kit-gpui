// Name: SDK Test - fields() Date/Time Input Types
// Description: Exercises the real SDK request/response contract for date/time fields.

/**
 * SDK auto-submit verifies typed field definitions, ordered values, and defaults.
 * It does not claim native date-picker rendering or perform screen capture.
 */

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
	name: string,
	status: TestResult["status"],
	extra?: Partial<TestResult>,
) {
	console.log(
		JSON.stringify({
			test: name,
			status,
			timestamp: new Date().toISOString(),
			...extra,
		}),
	);
}

async function runTest(
	name: string,
	definitions: FieldDef[],
	expected: string[],
) {
	logTest(name, "running");
	const started = Date.now();

	try {
		const values = await fields(definitions);
		if (!Array.isArray(values)) {
			throw new Error(`Expected fields() to return an array, got ${typeof values}`);
		}
		if (JSON.stringify(values) !== JSON.stringify(expected)) {
			throw new Error(
				`Expected ordered values ${JSON.stringify(expected)}, got ${JSON.stringify(values)}`,
			);
		}
		logTest(name, "pass", {
			result: {
				fieldTypes: definitions.map((definition) => definition.type ?? "text"),
				values,
				proof: "sdk-auto-submit-request-response",
			},
			duration_ms: Date.now() - started,
		});
	} catch (error) {
		logTest(name, "fail", {
			error: String(error),
			duration_ms: Date.now() - started,
		});
	}
}

const dateTimeFields: FieldDef[] = [
	{ name: "birthday", label: "Birthday", type: "date", value: "2026-08-21" },
	{ name: "meeting", label: "Meeting Time", type: "time", value: "09:30" },
	{
		name: "appointment",
		label: "Appointment",
		type: "datetime-local",
		value: "2026-08-21T09:30",
	},
	{ name: "expiry", label: "Card Expiry", type: "month", value: "2026-08" },
	{ name: "week", label: "Week Number", type: "week", value: "2026-W34" },
];

await runTest("fields-implementation-check", [dateTimeFields[0]], ["2026-08-21"]);

for (const definition of dateTimeFields) {
	await runTest(`fields-${definition.type}`, [definition], [definition.value ?? ""]);
}

await runTest(
	"fields-all-datetime",
	dateTimeFields,
	dateTimeFields.map((definition) => definition.value ?? ""),
);

await runTest("fields-search-default", [
	{ name: "query", label: "Search", type: "search" },
], [""]);

console.error(
	"[TEST] fields() date/time SDK contracts completed; native picker rendering requires separate runtime proof.",
);
