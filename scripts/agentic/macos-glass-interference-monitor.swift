#!/usr/bin/env swift

import AppKit
import CoreGraphics
import Foundation

private var readyPath: String?
private var stopPath: String?
private var outputPath: String?
private let args = Array(CommandLine.arguments.dropFirst())
private var index = 0
while index < args.count {
    switch args[index] {
    case "--ready":
        if index + 1 < args.count { readyPath = args[index + 1]; index += 1 }
    case "--stop":
        if index + 1 < args.count { stopPath = args[index + 1]; index += 1 }
    case "--out":
        if index + 1 < args.count { outputPath = args[index + 1]; index += 1 }
    default:
        break
    }
    index += 1
}

guard let readyPath, let stopPath, let outputPath else {
    fputs("usage: macos-glass-interference-monitor --ready PATH --stop PATH --out PATH\n", stderr)
    exit(64)
}

private let watched: [(String, CGEventType)] = [
    ("keyDown", .keyDown),
    ("leftMouseDown", .leftMouseDown),
    ("rightMouseDown", .rightMouseDown),
    ("otherMouseDown", .otherMouseDown),
    ("mouseMoved", .mouseMoved),
    ("leftMouseDragged", .leftMouseDragged),
    ("rightMouseDragged", .rightMouseDragged),
    ("scrollWheel", .scrollWheel),
]

private func frontmost() -> [String: Any] {
    let app = NSWorkspace.shared.frontmostApplication
    return [
        "pid": app?.processIdentifier ?? 0,
        "bundleId": app?.bundleIdentifier ?? "",
        "name": app?.localizedName ?? "",
    ]
}

private func point(_ value: NSPoint) -> [String: Double] {
    ["x": value.x, "y": value.y]
}

private func write(_ value: [String: Any], to path: String) throws {
    let data = try JSONSerialization.data(
        withJSONObject: value,
        options: [.prettyPrinted, .sortedKeys]
    )
    try data.write(to: URL(fileURLWithPath: path), options: .atomic)
}

let startedAt = ISO8601DateFormatter().string(from: Date())
let initialFrontmost = frontmost()
let initialPointer = NSEvent.mouseLocation
var previousAges = Dictionary(
    uniqueKeysWithValues: watched.map {
        ($0.0, CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: $0.1))
    }
)
var eventCounts = Dictionary(uniqueKeysWithValues: watched.map { ($0.0, 0) })
var maximumPointerDeviation = 0.0
var frontmostHistory: [[String: Any]] = [initialFrontmost]
var sampleCount = 0
// Timestamped interference events (glass-smoke-harness-max-info WP6): each
// counter mutation site records WHEN it fired so scenario intervals can
// attribute interference to the scenario that was capturing at that moment.
// atUnixMs shares the probes' Date.now() clock; atUptimeNs shares the
// lifecycle receipts' host clock. Bounded so a noisy session cannot grow the
// receipt without bound — drops are counted, never silent.
let maximumRecordedEvents = 2000
var interferenceEvents: [[String: Any]] = []
var droppedEventCount = 0
func recordInterferenceEvent(_ kind: String, _ detail: [String: Any] = [:]) {
    guard interferenceEvents.count < maximumRecordedEvents else {
        droppedEventCount += 1
        return
    }
    var event: [String: Any] = [
        "kind": kind,
        "atUnixMs": Date().timeIntervalSince1970 * 1000.0,
        "atUptimeNs": DispatchTime.now().uptimeNanoseconds,
        "sampleIndex": sampleCount,
    ]
    for (key, value) in detail { event[key] = value }
    interferenceEvents.append(event)
}
var pointerDeviationEventRecorded = false
try write([
    "schemaVersion": 1,
    "status": "ready",
    "startedAt": startedAt,
    "frontmost": initialFrontmost,
    "pointer": point(initialPointer),
], to: readyPath)

// WindowServer synthesizes hover-update mouseMoved events when the window
// under a STATIONARY cursor changes — inherent to a window-lifecycle probe
// (2026-07-26: 840/1064 study attempts were invalidated by exactly one
// zero-displacement mouseMoved). A mouseMoved that arrives while the cursor
// has not displaced since the previous 120 Hz sample cannot be human input:
// record it as a visible phantom event, but do not count it as interference.
// All other event types (clicks, keys, scroll, drags) have no synthetic
// stationary source and keep failing closed exactly as before.
var phantomStationaryMouseMovedCount = 0
var previousPointer = initialPointer
while !FileManager.default.fileExists(atPath: stopPath) {
    Thread.sleep(forTimeInterval: 1.0 / 120.0)
    sampleCount += 1
    let pointerNow = NSEvent.mouseLocation
    let movedSinceLastSample = hypot(
        pointerNow.x - previousPointer.x,
        pointerNow.y - previousPointer.y
    ) > 0.5
    previousPointer = pointerNow
    for (name, eventType) in watched {
        let age = CGEventSource.secondsSinceLastEventType(
            .combinedSessionState,
            eventType: eventType
        )
        if let previous = previousAges[name], age + 0.004 < previous {
            if name == "mouseMoved" && !movedSinceLastSample {
                phantomStationaryMouseMovedCount += 1
                recordInterferenceEvent(
                    "phantomStationaryMouseMoved",
                    ["eventType": name]
                )
            } else {
                eventCounts[name, default: 0] += 1
                recordInterferenceEvent("untaggedInput", ["eventType": name])
            }
        }
        previousAges[name] = age
    }
    let pointer = pointerNow
    let pointerDeviation = hypot(
        pointer.x - initialPointer.x,
        pointer.y - initialPointer.y
    )
    maximumPointerDeviation = max(maximumPointerDeviation, pointerDeviation)
    if pointerDeviation > 1.0 && !pointerDeviationEventRecorded {
        pointerDeviationEventRecorded = true
        recordInterferenceEvent(
            "pointerDeviationExceeded",
            ["deviationPx": pointerDeviation]
        )
    }
    if sampleCount % 12 == 0 {
        let current = frontmost()
        let last = frontmostHistory.last
        if (last?["pid"] as? Int32) != (current["pid"] as? Int32) {
            frontmostHistory.append(current)
            recordInterferenceEvent(
                "frontmostAppChanged",
                ["bundleId": current["bundleId"] ?? ""]
            )
        }
    }
}

let finalFrontmost = frontmost()
if (frontmostHistory.last?["pid"] as? Int32) != (finalFrontmost["pid"] as? Int32) {
    frontmostHistory.append(finalFrontmost)
    recordInterferenceEvent(
        "frontmostAppChanged",
        ["bundleId": finalFrontmost["bundleId"] ?? ""]
    )
}
let finalPointer = NSEvent.mouseLocation
let untaggedInputCount = eventCounts.values.reduce(0, +)
let frontmostChanged = (initialFrontmost["pid"] as? Int32)
    != (finalFrontmost["pid"] as? Int32)
let receipt: [String: Any] = [
    "schemaVersion": 1,
    "status": "ok",
    "startedAt": startedAt,
    "finishedAt": ISO8601DateFormatter().string(from: Date()),
    "sampleRateHz": 120,
    "sampleCount": sampleCount,
    "frontmostBefore": initialFrontmost,
    "frontmostHistory": frontmostHistory,
    "frontmostAfter": finalFrontmost,
    "frontmostAppChanged": frontmostChanged,
    "pointerBefore": point(initialPointer),
    "pointerAfter": point(finalPointer),
    "pointerDeviationPx": maximumPointerDeviation,
    "intendedPointerPath": "stationary",
    "taggedInputCount": 0,
    "untaggedInputCount": untaggedInputCount,
    "phantomStationaryMouseMovedCount": phantomStationaryMouseMovedCount,
    "eventCounts": eventCounts,
    "events": interferenceEvents,
    "droppedEventCount": droppedEventCount,
    "eventTimestampsSupported": true,
    "targetMovedExternally": false,
    "pass": untaggedInputCount == 0
        && !frontmostChanged
        && maximumPointerDeviation <= 1.0,
]
try write(receipt, to: outputPath)
FileHandle.standardOutput.write(
    try JSONSerialization.data(withJSONObject: receipt, options: [.sortedKeys])
)
FileHandle.standardOutput.write(Data("\n".utf8))
