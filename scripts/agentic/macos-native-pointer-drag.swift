#!/usr/bin/env swift

import AppKit
import ApplicationServices
import CoreGraphics
import Foundation

private struct Rect: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

private struct Point: Codable {
    let x: Double
    let y: Double
}

private struct WindowSample: Codable {
    let sequence: Int
    let event: String
    let monotonicNs: UInt64
    let bounds: Rect?
}

private struct EventReceipt: Codable {
    let sequence: Int
    let event: String
    let point: Point
    let monotonicNs: UInt64
}

private struct Output: Codable {
    let schemaVersion: Int
    let status: String
    let pid: Int32
    let windowId: UInt32
    let eventTag: Int64
    let accessibilityTrusted: Bool
    let postEventAccess: Bool
    let initialBounds: Rect?
    let finalBounds: Rect?
    let events: [EventReceipt]
    let samples: [WindowSample]
    let taggedEventCount: Int
    let untaggedInputCount: Int
    let errors: [String]
}

private final class EventMonitor {
    private let lock = NSLock()
    private let expectedTag: Int64
    private var tagged = 0
    private var untagged = 0

    init(expectedTag: Int64) {
        self.expectedTag = expectedTag
    }

    func record(_ event: CGEvent) {
        lock.lock()
        defer { lock.unlock() }
        if event.getIntegerValueField(.eventSourceUserData) == expectedTag {
            tagged += 1
        } else {
            untagged += 1
        }
    }

    func counts() -> (tagged: Int, untagged: Int) {
        lock.lock()
        defer { lock.unlock() }
        return (tagged, untagged)
    }
}

private let eventTapCallback: CGEventTapCallBack = { _, type, event, context in
    guard type != .tapDisabledByTimeout, type != .tapDisabledByUserInput,
          let context
    else { return Unmanaged.passUnretained(event) }
    Unmanaged<EventMonitor>.fromOpaque(context).takeUnretainedValue().record(event)
    return Unmanaged.passUnretained(event)
}

private func monotonicNs() -> UInt64 {
    DispatchTime.now().uptimeNanoseconds
}

private func windowBounds(pid: Int32, windowId: UInt32) -> Rect? {
    guard let rows = CGWindowListCopyWindowInfo(
        [.optionIncludingWindow],
        CGWindowID(windowId)
    ) as? [[String: Any]] else { return nil }
    guard let row = rows.first,
          (row[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
          let rawBounds = row[kCGWindowBounds as String]
    else { return nil }
    let dict = rawBounds as! CFDictionary
    guard let rect = CGRect(dictionaryRepresentation: dict) else { return nil }
    return Rect(
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height
    )
}

private func write(_ output: Output, path: String?) throws {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    let data = try encoder.encode(output)
    if let path {
        try data.write(to: URL(fileURLWithPath: path), options: .atomic)
    }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
}

private struct Arguments {
    var pid: Int32?
    var windowId: UInt32?
    var start: CGPoint?
    var end: CGPoint?
    var durationMs = 600
    var steps = 48
    var eventTag = Int64.random(in: 1_000_000...Int64.max / 4)
    var output: String?
}

private func parseArguments() -> Arguments {
    var result = Arguments()
    let args = Array(CommandLine.arguments.dropFirst())
    var index = 0
    func value() -> String? {
        guard index + 1 < args.count else { return nil }
        index += 1
        return args[index]
    }
    while index < args.count {
        switch args[index] {
        case "--pid":
            result.pid = value().flatMap(Int32.init)
        case "--window-id":
            result.windowId = value().flatMap(UInt32.init)
        case "--start":
            if let x = value().flatMap(Double.init),
               let y = value().flatMap(Double.init)
            {
                result.start = CGPoint(x: x, y: y)
            }
        case "--end":
            if let x = value().flatMap(Double.init),
               let y = value().flatMap(Double.init)
            {
                result.end = CGPoint(x: x, y: y)
            }
        case "--duration-ms":
            result.durationMs = value().flatMap(Int.init) ?? result.durationMs
        case "--steps":
            result.steps = value().flatMap(Int.init) ?? result.steps
        case "--event-user-data":
            result.eventTag = value().flatMap(Int64.init) ?? result.eventTag
        case "--out":
            result.output = value()
        default:
            break
        }
        index += 1
    }
    return result
}

private func makeEvent(
    type: CGEventType,
    point: CGPoint,
    tag: Int64,
    sequence: Int
) -> CGEvent? {
    guard let event = CGEvent(
        mouseEventSource: CGEventSource(stateID: .hidSystemState),
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: .left
    ) else { return nil }
    event.setIntegerValueField(.eventSourceUserData, value: tag)
    event.setIntegerValueField(.eventSourceUserID, value: Int64(sequence))
    return event
}

private func main() throws {
    let args = parseArguments()
    guard let pid = args.pid, let windowId = args.windowId,
          let start = args.start, let end = args.end
    else {
        fputs(
            "usage: macos-native-pointer-drag --pid PID --window-id ID --start X Y --end X Y [--duration-ms N] [--steps N] [--event-user-data N] [--out PATH]\n",
            stderr
        )
        exit(64)
    }

    let accessibilityTrusted = AXIsProcessTrusted()
    let postEventAccess = CGPreflightPostEventAccess()
    let initialBounds = windowBounds(pid: pid, windowId: windowId)
    var errors: [String] = []
    if !accessibilityTrusted { errors.append("accessibility permission unavailable") }
    if !postEventAccess { errors.append("post-event permission unavailable") }
    if initialBounds == nil { errors.append("exact PID/window identity unresolved") }

    let monitor = EventMonitor(expectedTag: args.eventTag)
    let eventMask = (1 << CGEventType.keyDown.rawValue)
        | (1 << CGEventType.flagsChanged.rawValue)
        | (1 << CGEventType.leftMouseDown.rawValue)
        | (1 << CGEventType.leftMouseDragged.rawValue)
        | (1 << CGEventType.leftMouseUp.rawValue)
        | (1 << CGEventType.rightMouseDown.rawValue)
        | (1 << CGEventType.otherMouseDown.rawValue)
        | (1 << CGEventType.scrollWheel.rawValue)
    let eventTap = CGEvent.tapCreate(
        tap: .cghidEventTap,
        place: .headInsertEventTap,
        options: .listenOnly,
        eventsOfInterest: CGEventMask(eventMask),
        callback: eventTapCallback,
        userInfo: Unmanaged.passUnretained(monitor).toOpaque()
    )
    var eventTapSource: CFRunLoopSource?
    if let eventTap {
        eventTapSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0)
        CFRunLoopAddSource(CFRunLoopGetCurrent(), eventTapSource, .commonModes)
        CGEvent.tapEnable(tap: eventTap, enable: true)
    } else {
        errors.append("input event tap unavailable")
    }

    var events: [EventReceipt] = []
    var samples: [WindowSample] = []
    let resultLock = NSLock()
    let done = DispatchSemaphore(value: 0)

    if errors.isEmpty {
        DispatchQueue.global(qos: .userInteractive).async {
            let steps = max(4, args.steps)
            let intervalUs = useconds_t(
                max(1, args.durationMs * 1_000 / steps)
            )

            func post(_ type: CGEventType, _ name: String, _ point: CGPoint, _ sequence: Int) {
                let timestamp = monotonicNs()
                if let event = makeEvent(
                    type: type,
                    point: point,
                    tag: args.eventTag,
                    sequence: sequence
                ) {
                    event.post(tap: .cghidEventTap)
                }
                usleep(1_000)
                let sample = WindowSample(
                    sequence: sequence,
                    event: name,
                    monotonicNs: timestamp,
                    bounds: windowBounds(pid: pid, windowId: windowId)
                )
                resultLock.lock()
                events.append(EventReceipt(
                    sequence: sequence,
                    event: name,
                    point: Point(x: point.x, y: point.y),
                    monotonicNs: timestamp
                ))
                samples.append(sample)
                resultLock.unlock()
            }

            post(.leftMouseDown, "mouseDown", start, 0)
            for step in 1...steps {
                let progress = Double(step) / Double(steps)
                let point = CGPoint(
                    x: start.x + (end.x - start.x) * progress,
                    y: start.y + (end.y - start.y) * progress
                )
                post(.leftMouseDragged, "mouseDragged", point, step)
                usleep(intervalUs)
            }
            post(.leftMouseUp, "mouseUp", end, steps + 1)
            done.signal()
        }

        while done.wait(timeout: .now()) != .success {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.01))
        }
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.08))
    }

    if let eventTap {
        CGEvent.tapEnable(tap: eventTap, enable: false)
    }
    if let eventTapSource {
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), eventTapSource, .commonModes)
    }
    let counts = monitor.counts()
    let finalBounds = windowBounds(pid: pid, windowId: windowId)
    let status = errors.isEmpty ? "ok" : "invalid"
    try write(
        Output(
            schemaVersion: 1,
            status: status,
            pid: pid,
            windowId: windowId,
            eventTag: args.eventTag,
            accessibilityTrusted: accessibilityTrusted,
            postEventAccess: postEventAccess,
            initialBounds: initialBounds,
            finalBounds: finalBounds,
            events: events,
            samples: samples,
            taggedEventCount: counts.tagged,
            untaggedInputCount: counts.untagged,
            errors: errors
        ),
        path: args.output
    )
    exit(status == "ok" ? 0 : 2)
}

try main()
