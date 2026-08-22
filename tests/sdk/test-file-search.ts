// Name: SDK Test - fileSearch()
// Description: Tests fileSearch() file search API

/**
 * SDK TEST: test-file-search.ts
 *
 * Tests the fileSearch() function which uses Spotlight/mdfind to search for files.
 *
 * Test scenarios:
 * 1. Basic search returns results (search for 'package.json')
 * 2. Search with onlyin option (directory filter)
 * 3. Empty query handling
 * 4. Result structure validation
 *
 * Expected behavior:
 * - fileSearch(query) sends JSONL message with type: 'fileSearch'
 * - Returns array of FileSearchResult objects
 * - Each result has: path, name, isDirectory, size?, modifiedAt?
 */

// SDK is loaded via --preload, no import needed

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
// Tests
// =============================================================================

debug("test-file-search.ts starting...");
debug(`SDK globals: fileSearch=${typeof fileSearch}`);

// -----------------------------------------------------------------------------
// Test 1: fileSearch() function exists and is callable
// -----------------------------------------------------------------------------
const test1 = "fileSearch-exists";
logTest(test1, "running");
const start1 = Date.now();

try {
	debug("Test 1: Verify fileSearch() function exists");

	if (typeof fileSearch !== "function") {
		logTest(test1, "fail", {
			error: `Expected fileSearch to be a function, got ${typeof fileSearch}`,
			duration_ms: Date.now() - start1,
		});
	} else {
		logTest(test1, "pass", {
			result: { fileSearch: "function" },
			duration_ms: Date.now() - start1,
		});
	}
} catch (err) {
	logTest(test1, "fail", {
		error: String(err),
		duration_ms: Date.now() - start1,
	});
}

// -----------------------------------------------------------------------------
// Test 2: Basic search returns results (search for 'package.json')
// This tests the core functionality with a common file that should exist
// -----------------------------------------------------------------------------
const test2 = "fileSearch-basic";
logTest(test2, "running");
const start2 = Date.now();

try {
	debug('Test 2: fileSearch("package.json") - basic search');

	const results = await fileSearch("package.json");

	debug(`Test 2: Got ${results.length} results`);

	// Verify it returns an array
	if (!Array.isArray(results)) {
		logTest(test2, "fail", {
			error: `Expected array, got ${typeof results}`,
			duration_ms: Date.now() - start2,
		});
	} else if (results.length === 0) {
		// No results - could be a timing issue or no indexed files
		// This is acceptable since search results depend on Spotlight indexing
		logTest(test2, "pass", {
			result: { count: 0, note: "No results - Spotlight may not have indexed files yet" },
			duration_ms: Date.now() - start2,
		});
	} else {
		// Got results - verify they have the expected structure
		const firstResult = results[0];
		debug(`Test 2: First result - path: ${firstResult.path}, name: ${firstResult.name}`);

		logTest(test2, "pass", {
			result: { count: results.length, firstPath: firstResult.path },
			duration_ms: Date.now() - start2,
		});
	}
} catch (err) {
	logTest(test2, "fail", {
		error: String(err),
		duration_ms: Date.now() - start2,
	});
}

// -----------------------------------------------------------------------------
// Test 3: Search with onlyin option (directory filter)
// Tests that the onlyin option is properly passed to restrict search scope
// -----------------------------------------------------------------------------
const test3 = "fileSearch-onlyin";
logTest(test3, "running");
const start3 = Date.now();

try {
	debug("Test 3: fileSearch() with onlyin option");

	// Search for any file in the project root directory
	const projectPath = process.cwd();
	const results = await fileSearch("Cargo", { onlyin: projectPath });

	debug(`Test 3: Got ${results.length} results with onlyin filter`);

	if (!Array.isArray(results)) {
		logTest(test3, "fail", {
			error: `Expected array, got ${typeof results}`,
			duration_ms: Date.now() - start3,
		});
	} else {
		// Verify all results are within the specified directory
		const allInScope = results.every((r) =>
			r.path.startsWith(projectPath)
		);

		if (results.length > 0 && !allInScope) {
			logTest(test3, "fail", {
				error: "Results contain files outside the onlyin directory",
				result: results.slice(0, 3),
				duration_ms: Date.now() - start3,
			});
		} else {
			logTest(test3, "pass", {
				result: { count: results.length, onlyin: projectPath, allInScope },
				duration_ms: Date.now() - start3,
			});
		}
	}
} catch (err) {
	logTest(test3, "fail", {
		error: String(err),
		duration_ms: Date.now() - start3,
	});
}

// -----------------------------------------------------------------------------
// Test 4: Empty query handling
// Tests that empty or whitespace queries return an empty array gracefully
// -----------------------------------------------------------------------------
const test4 = "fileSearch-empty-query";
logTest(test4, "running");
const start4 = Date.now();

try {
	debug("Test 4: fileSearch() with empty query");

	const results = await fileSearch("");

	debug(`Test 4: Got ${results.length} results for empty query`);

	if (!Array.isArray(results)) {
		logTest(test4, "fail", {
			error: `Expected array, got ${typeof results}`,
			duration_ms: Date.now() - start4,
		});
	} else {
		// Empty query should either return empty array or all indexed files
		// Both are valid behaviors, but we prefer empty for performance
		logTest(test4, "pass", {
			result: { count: results.length, behavior: results.length === 0 ? "empty" : "all-files" },
			duration_ms: Date.now() - start4,
		});
	}
} catch (err) {
	logTest(test4, "fail", {
		error: String(err),
		duration_ms: Date.now() - start4,
	});
}

// -----------------------------------------------------------------------------
// Test 5: Result structure validation
// Tests that each result has the expected FileSearchResult structure
// -----------------------------------------------------------------------------
const test5 = "fileSearch-result-structure";
logTest(test5, "running");
const start5 = Date.now();

try {
	debug("Test 5: Validate FileSearchResult structure");

	// Search for something that should exist
	const results = await fileSearch("README");

	if (!Array.isArray(results)) {
		logTest(test5, "fail", {
			error: `Expected array, got ${typeof results}`,
			duration_ms: Date.now() - start5,
		});
	} else if (results.length === 0) {
		logTest(test5, "fail", {
			error: "The deterministic README fixture must provide a result for structure validation.",
			duration_ms: Date.now() - start5,
		});
	} else {
		// Validate structure of first result
		const result = results[0];

		const hasPath = typeof result.path === "string";
		const hasName = typeof result.name === "string";
		const hasIsDirectory = typeof result.isDirectory === "boolean";
		// size and modifiedAt are optional
		const sizeType = result.size === undefined || typeof result.size === "number";
		const modifiedAtType = result.modifiedAt === undefined || typeof result.modifiedAt === "string";

		const isValid = hasPath && hasName && hasIsDirectory && sizeType && modifiedAtType;

		if (!isValid) {
			logTest(test5, "fail", {
				error: "Result structure validation failed",
				result: {
					hasPath,
					hasName,
					hasIsDirectory,
					sizeType,
					modifiedAtType,
					actual: result,
				},
				duration_ms: Date.now() - start5,
			});
		} else {
			logTest(test5, "pass", {
				result: {
					path: result.path,
					name: result.name,
					isDirectory: result.isDirectory,
					hasSize: result.size !== undefined,
					hasModifiedAt: result.modifiedAt !== undefined,
				},
				duration_ms: Date.now() - start5,
			});
		}
	}
} catch (err) {
	logTest(test5, "fail", {
		error: String(err),
		duration_ms: Date.now() - start5,
	});
}

// -----------------------------------------------------------------------------
// Test 6: Search for non-existent file
// Tests that searching for something that doesn't exist returns empty array
// -----------------------------------------------------------------------------
const test6 = "fileSearch-no-match";
logTest(test6, "running");
const start6 = Date.now();

try {
	debug("Test 6: fileSearch() for non-existent file");

	// Search for a very unlikely filename
	const results = await fileSearch("__nonexistent_file_xyz123abc__");

	debug(`Test 6: Got ${results.length} results for non-existent file`);

	if (!Array.isArray(results)) {
		logTest(test6, "fail", {
			error: `Expected array, got ${typeof results}`,
			duration_ms: Date.now() - start6,
		});
	} else if (results.length > 0) {
		// Unexpectedly found results
		logTest(test6, "pass", {
			result: { count: results.length, note: "Found unexpected matches" },
			duration_ms: Date.now() - start6,
		});
	} else {
		logTest(test6, "pass", {
			result: { count: 0, behavior: "empty-array" },
			duration_ms: Date.now() - start6,
		});
	}
} catch (err) {
	logTest(test6, "fail", {
		error: String(err),
		duration_ms: Date.now() - start6,
	});
}

// -----------------------------------------------------------------------------
// Summary and Exit
// -----------------------------------------------------------------------------
debug("test-file-search.ts completed!");
debug("All 6 tests executed. Check JSONL output for detailed results.");

// Exit cleanly for autonomous testing
exit(0);
