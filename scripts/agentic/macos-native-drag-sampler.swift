#!/usr/bin/env swift

import ApplicationServices
import Cocoa
import CoreImage
import CoreMedia
import CoreGraphics
import CoreVideo
import Darwin
import Foundation
import ImageIO
import ScreenCaptureKit

private let schemaVersion = 2
private let eventTag: Int64 = 0x534B4D574E44
private let leftControlID = "script-kit-footer-left-info-hit-target"
private let rightControlCandidates = [
    "script-kit-footer-button-ai",
    "script-kit-footer-button-actions",
    "script-kit-footer-button-run",
]

private struct Point: Codable {
    let x: Double
    let y: Double
}

private struct Rect: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double

    var origin: CGPoint { CGPoint(x: x, y: y) }
}

private struct NativeWindow: Codable {
    let windowNumber: Int
    let ownerPID: Int32
    let layer: Int
    let alpha: Double
    let onscreen: Bool
    let boundsPt: Rect
}

private struct TimedRead<T: Codable>: Codable {
    let startNs: UInt64
    let endNs: UInt64
    let midpointNs: UInt64
    let value: T?
    let error: String?
}

private struct ControlFrame: Codable {
    let id: String
    let framePt: Rect?
    let mainFramePtAtMeasurement: Rect?
    let axWindowNumber: Int?
    let measurementSource: String
    let error: String?
    let frameRead: TimedRead<Rect>?
    let ownerRead: TimedRead<Int>?
    let alignmentUncertaintyPx: Double?
    let topologyFresh: Bool
    let displayIntervalIndex: Int
    let crossesEventBoundary: Bool

    init(
        id: String,
        framePt: Rect?,
        mainFramePtAtMeasurement: Rect?,
        axWindowNumber: Int?,
        measurementSource: String,
        error: String?,
        frameRead: TimedRead<Rect>? = nil,
        ownerRead: TimedRead<Int>? = nil,
        alignmentUncertaintyPx: Double? = nil,
        topologyFresh: Bool = false,
        displayIntervalIndex: Int = -1,
        crossesEventBoundary: Bool = false
    ) {
        self.id = id
        self.framePt = framePt
        self.mainFramePtAtMeasurement = mainFramePtAtMeasurement
        self.axWindowNumber = axWindowNumber
        self.measurementSource = measurementSource
        self.error = error
        self.frameRead = frameRead
        self.ownerRead = ownerRead
        self.alignmentUncertaintyPx = alignmentUncertaintyPx
        self.topologyFresh = topologyFresh
        self.displayIntervalIndex = displayIntervalIndex
        self.crossesEventBoundary = crossesEventBoundary
    }
}

private struct Sample: Codable {
    let tNs: UInt64
    let phase: String
    let mainWindowNumber: Int?
    let mainFramePt: Rect?
    let footerWindowNumber: Int?
    let footerFramePt: Rect?
    let relevantWindowCount: Int
    let relevantWindowNumbers: [Int]
    let controls: [ControlFrame]
    let packetStartNs: UInt64
    let packetEndNs: UInt64
    let displayTickNs: UInt64
    let displayIntervalIndex: Int
    let topologyStartNs: UInt64
    let topologyEndNs: UInt64
    let topologyFresh: Bool
    let topologyComplete: Bool

    init(
        tNs: UInt64,
        phase: String,
        mainWindowNumber: Int?,
        mainFramePt: Rect?,
        footerWindowNumber: Int?,
        footerFramePt: Rect?,
        relevantWindowCount: Int,
        relevantWindowNumbers: [Int],
        controls: [ControlFrame],
        packetStartNs: UInt64 = 0,
        packetEndNs: UInt64 = 0,
        displayTickNs: UInt64 = 0,
        displayIntervalIndex: Int = -1,
        topologyStartNs: UInt64 = 0,
        topologyEndNs: UInt64 = 0,
        topologyFresh: Bool = false,
        topologyComplete: Bool = false
    ) {
        self.tNs = tNs
        self.phase = phase
        self.mainWindowNumber = mainWindowNumber
        self.mainFramePt = mainFramePt
        self.footerWindowNumber = footerWindowNumber
        self.footerFramePt = footerFramePt
        self.relevantWindowCount = relevantWindowCount
        self.relevantWindowNumbers = relevantWindowNumbers
        self.controls = controls
        self.packetStartNs = packetStartNs
        self.packetEndNs = packetEndNs
        self.displayTickNs = displayTickNs
        self.displayIntervalIndex = displayIntervalIndex
        self.topologyStartNs = topologyStartNs
        self.topologyEndNs = topologyEndNs
        self.topologyFresh = topologyFresh
        self.topologyComplete = topologyComplete
    }
}

private struct FilmstripFrame: Codable {
    let fraction: Double
    let tNs: UInt64
    let actualFrameNs: UInt64?
    let markerEventNs: UInt64?
    let encodingCompletedNs: UInt64?
    let mainFramePt: Rect?
    let path: String
    let captureSucceeded: Bool
    let error: String?

    init(
        fraction: Double,
        tNs: UInt64,
        actualFrameNs: UInt64? = nil,
        markerEventNs: UInt64? = nil,
        encodingCompletedNs: UInt64? = nil,
        mainFramePt: Rect?,
        path: String,
        captureSucceeded: Bool,
        error: String?
    ) {
        self.fraction = fraction
        self.tNs = tNs
        self.actualFrameNs = actualFrameNs
        self.markerEventNs = markerEventNs
        self.encodingCompletedNs = encodingCompletedNs
        self.mainFramePt = mainFramePt
        self.path = path
        self.captureSucceeded = captureSucceeded
        self.error = error
    }
}

private struct EventRecord: Codable {
    let kind: String
    let sequence: Int
    let tag: Int64
    let intendedNs: UInt64
    let actualEventNs: UInt64
    let postStartNs: UInt64
    let postEndNs: UInt64
    var observedByEventTap: Bool
}

private struct Interference: Codable {
    let untaggedInputCount: Int
    let frontmostAppChanged: Bool
    let pointerDeviationPx: Double
    let targetMovedExternally: Bool
}

private struct ObserverHealth: Codable {
    var scheduledPackets: Int
    var completedPackets: Int
    var missedPackets: Int
    var axTimeoutCount: Int
    var topologyStaleCount: Int
    var displayTickIntervalsMs: [Double]
    var queueLatenessMs: [Double]
    var mainAXCallMs: [Double]
    var leftAXCallMs: [Double]
    var rightAXCallMs: [Double]
    var ownerAXCallMs: [Double]
    var cgInventoryCallMs: [Double]
}

private struct DisplayInfo: Codable {
    let displayID: UInt32
    let refreshHz: Double
    let backingScale: Double
    let boundsPt: Rect
}

private struct Output: Codable {
    let schemaVersion: Int
    let status: String
    let pid: Int32
    let trajectory: String
    let durationMs: Double
    let requestedDeltaPt: Point
    let accessibilityTrusted: Bool
    let display: DisplayInfo?
    let startedAt: String
    let finishedAt: String
    let sampleTargetHz: Double
    let mouseDownEventNs: UInt64?
    let mouseUpEventNs: UInt64?
    let events: [EventRecord]
    let interference: Interference
    let observerHealth: ObserverHealth
    let samples: [Sample]
    let filmstripFrames: [FilmstripFrame]
    let errors: [String]

    init(
        schemaVersion: Int,
        status: String,
        pid: Int32,
        trajectory: String,
        durationMs: Double,
        requestedDeltaPt: Point,
        accessibilityTrusted: Bool,
        display: DisplayInfo?,
        startedAt: String,
        finishedAt: String,
        sampleTargetHz: Double,
        mouseDownEventNs: UInt64? = nil,
        mouseUpEventNs: UInt64?,
        events: [EventRecord] = [],
        interference: Interference = Interference(
            untaggedInputCount: 0,
            frontmostAppChanged: false,
            pointerDeviationPx: 0,
            targetMovedExternally: false
        ),
        observerHealth: ObserverHealth = ObserverHealth(
            scheduledPackets: 0,
            completedPackets: 0,
            missedPackets: 0,
            axTimeoutCount: 0,
            topologyStaleCount: 0,
            displayTickIntervalsMs: [],
            queueLatenessMs: [],
            mainAXCallMs: [],
            leftAXCallMs: [],
            rightAXCallMs: [],
            ownerAXCallMs: [],
            cgInventoryCallMs: []
        ),
        samples: [Sample],
        filmstripFrames: [FilmstripFrame],
        errors: [String]
    ) {
        self.schemaVersion = schemaVersion
        self.status = status
        self.pid = pid
        self.trajectory = trajectory
        self.durationMs = durationMs
        self.requestedDeltaPt = requestedDeltaPt
        self.accessibilityTrusted = accessibilityTrusted
        self.display = display
        self.startedAt = startedAt
        self.finishedAt = finishedAt
        self.sampleTargetHz = sampleTargetHz
        self.mouseDownEventNs = mouseDownEventNs
        self.mouseUpEventNs = mouseUpEventNs
        self.events = events
        self.interference = interference
        self.observerHealth = observerHealth
        self.samples = samples
        self.filmstripFrames = filmstripFrames
        self.errors = errors
    }
}

private struct Arguments {
    var pid: Int32?
    var trajectory = "fast-horizontal"
    var output: String?
    var filmstripDir: String?
    var filmstripPrefix: String?
    var dryRun = false
    var mainWindowNumber: Int?
    var leftControlIdentifier = leftControlID
    var rightControlIdentifier: String?
}

private func parseArguments() -> Arguments {
    var result = Arguments()
    let args = Array(CommandLine.arguments.dropFirst())
    var index = 0
    while index < args.count {
        switch args[index] {
        case "--pid":
            if index + 1 < args.count { result.pid = Int32(args[index + 1]); index += 1 }
        case "--trajectory":
            if index + 1 < args.count { result.trajectory = args[index + 1]; index += 1 }
        case "--output":
            if index + 1 < args.count { result.output = args[index + 1]; index += 1 }
        case "--filmstrip-dir":
            if index + 1 < args.count { result.filmstripDir = args[index + 1]; index += 1 }
        case "--filmstrip-prefix":
            if index + 1 < args.count { result.filmstripPrefix = args[index + 1]; index += 1 }
        case "--dry-run":
            result.dryRun = true
        case "--observer-calibration":
            result.dryRun = true
        case "--main-window-number":
            if index + 1 < args.count { result.mainWindowNumber = Int(args[index + 1]); index += 1 }
        case "--left-control-id":
            if index + 1 < args.count { result.leftControlIdentifier = args[index + 1]; index += 1 }
        case "--right-control-id":
            if index + 1 < args.count { result.rightControlIdentifier = args[index + 1]; index += 1 }
        default:
            break
        }
        index += 1
    }
    return result
}

private enum HostClock {
    private static let timebase: mach_timebase_info_data_t = {
        var value = mach_timebase_info_data_t()
        mach_timebase_info(&value)
        return value
    }()

    // CVDisplayLink, CGEvent timestamps, and mach_wait_until all use the
    // mach_absolute_time host clock. Keep every observer surface in that
    // domain; mach_continuous_time has a suspend-adjusted offset.
    static func ticks() -> UInt64 { mach_absolute_time() }

    static func ns(_ ticks: UInt64 = ticks()) -> UInt64 {
        let quotient = ticks / UInt64(timebase.denom)
        let remainder = ticks % UInt64(timebase.denom)
        return quotient * UInt64(timebase.numer)
            + remainder * UInt64(timebase.numer) / UInt64(timebase.denom)
    }

    static func wait(until ticks: UInt64) {
        mach_wait_until(ticks)
    }

    static func ticks(forNanoseconds nanoseconds: UInt64) -> UInt64 {
        let quotient = nanoseconds / UInt64(timebase.numer)
        let remainder = nanoseconds % UInt64(timebase.numer)
        return quotient * UInt64(timebase.denom)
            + remainder * UInt64(timebase.denom) / UInt64(timebase.numer)
    }
}

private func monotonicNs() -> UInt64 { HostClock.ns() }

private func rect(_ dictionary: [String: Any]?) -> Rect? {
    guard let dictionary else { return nil }
    func number(_ key: String) -> Double? {
        if let value = dictionary[key] as? NSNumber { return value.doubleValue }
        if let value = dictionary[key] as? Double { return value }
        if let value = dictionary[key] as? Int { return Double(value) }
        return nil
    }
    guard
        let x = number("X"), let y = number("Y"),
        let width = number("Width"), let height = number("Height")
    else { return nil }
    return Rect(x: x, y: y, width: width, height: height)
}

private func windows(for pid: Int32) -> [NativeWindow] {
    guard let raw = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        return []
    }
    return raw.compactMap { entry in
        guard (entry[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid else {
            return nil
        }
        guard let bounds = rect(entry[kCGWindowBounds as String] as? [String: Any]) else {
            return nil
        }
        let windowNumber = (entry[kCGWindowNumber as String] as? NSNumber)?.intValue ?? 0
        let layer = (entry[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0
        let alpha = (entry[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 0
        let onscreen = (entry[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false
        return NativeWindow(
            windowNumber: windowNumber,
            ownerPID: pid,
            layer: layer,
            alpha: alpha,
            onscreen: onscreen,
            boundsPt: bounds
        )
    }
}

private func windows(numbers: [Int]) -> [NativeWindow] {
    return numbers.compactMap { number in
        guard let raw = CGWindowListCopyWindowInfo(
            [.optionIncludingWindow, .excludeDesktopElements],
            CGWindowID(number)
        ) as? [[String: Any]], let entry = raw.first else {
            return nil
        }
        guard let bounds = rect(entry[kCGWindowBounds as String] as? [String: Any]) else {
            return nil
        }
        return NativeWindow(
            windowNumber: (entry[kCGWindowNumber as String] as? NSNumber)?.intValue ?? number,
            ownerPID: (entry[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value ?? 0,
            layer: (entry[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0,
            alpha: (entry[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 0,
            onscreen: (entry[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false,
            boundsPt: bounds
        )
    }
}

private func relevantWindows(for pid: Int32) -> (main: NativeWindow?, footer: NativeWindow?, all: [NativeWindow]) {
    let all = windows(for: pid).filter {
        $0.onscreen && $0.alpha > 0 && $0.boundsPt.width > 1 && $0.boundsPt.height > 1
    }
    let main = all
        .filter { $0.boundsPt.height >= 120 }
        .max { lhs, rhs in lhs.boundsPt.width * lhs.boundsPt.height < rhs.boundsPt.width * rhs.boundsPt.height }
    let footer = all.first { candidate in
        guard let main else { return false }
        return candidate.windowNumber != main.windowNumber
            && abs(candidate.boundsPt.width - main.boundsPt.width) <= 1
            && candidate.boundsPt.height >= 28
            && candidate.boundsPt.height <= 40
    }
    return (main, footer, all)
}

private func copyAttribute(_ element: AXUIElement, _ name: CFString) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name, &value) == .success else { return nil }
    return value
}

private func stringAttribute(_ element: AXUIElement, _ name: CFString) -> String? {
    copyAttribute(element, name) as? String
}

private func pointAttribute(_ element: AXUIElement, _ name: CFString) -> CGPoint? {
    guard let value = copyAttribute(element, name), CFGetTypeID(value) == AXValueGetTypeID() else {
        return nil
    }
    var point = CGPoint.zero
    guard AXValueGetValue(value as! AXValue, .cgPoint, &point) else { return nil }
    return point
}

private func sizeAttribute(_ element: AXUIElement, _ name: CFString) -> CGSize? {
    guard let value = copyAttribute(element, name), CFGetTypeID(value) == AXValueGetTypeID() else {
        return nil
    }
    var size = CGSize.zero
    guard AXValueGetValue(value as! AXValue, .cgSize, &size) else { return nil }
    return size
}

private func windowNumber(_ element: AXUIElement) -> Int? {
    if let number = copyAttribute(element, "AXWindowNumber" as CFString) as? NSNumber {
        return number.intValue
    }
    if let owner = copyAttribute(element, kAXWindowAttribute as CFString) {
        let window = unsafeBitCast(owner, to: AXUIElement.self)
        if let number = copyAttribute(window, "AXWindowNumber" as CFString) as? NSNumber {
            return number.intValue
        }
    }
    return nil
}

@_silgen_name("_AXUIElementGetWindow")
private func privateAXWindowNumber(
    _ element: AXUIElement,
    _ windowID: UnsafeMutablePointer<CGWindowID>
) -> AXError

private func resolvedWindowNumber(_ element: AXUIElement) -> Int? {
    var windowID = CGWindowID(0)
    if privateAXWindowNumber(element, &windowID) == .success && windowID != 0 {
        return Int(windowID)
    }
    return windowNumber(element)
}

private func children(_ element: AXUIElement) -> [AXUIElement] {
    guard let values = copyAttribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] else {
        return []
    }
    return values
}

private func axRect(_ element: AXUIElement) -> Rect? {
    let attributes = [kAXPositionAttribute as CFString, kAXSizeAttribute as CFString] as CFArray
    var copiedValues: CFArray?
    guard AXUIElementCopyMultipleAttributeValues(element, attributes, [], &copiedValues) == .success,
          let values = copiedValues as? [CFTypeRef],
          values.count == 2,
          CFGetTypeID(values[0]) == AXValueGetTypeID(),
          CFGetTypeID(values[1]) == AXValueGetTypeID()
    else { return nil }
    var position = CGPoint.zero
    var size = CGSize.zero
    guard AXValueGetValue(values[0] as! AXValue, .cgPoint, &position),
          AXValueGetValue(values[1] as! AXValue, .cgSize, &size)
    else { return nil }
    return Rect(x: position.x, y: position.y, width: size.width, height: size.height)
}

private func findWindowElement(root: AXUIElement, windowNumber target: Int) -> AXUIElement? {
    guard let windowElements = copyAttribute(root, kAXWindowsAttribute as CFString) as? [AXUIElement] else {
        return nil
    }
    return windowElements.first { resolvedWindowNumber($0) == target }
}

private func findElement(
    root: AXUIElement,
    identifiers: Set<String>,
    maxDepth: Int = 14
) -> (String, AXUIElement)? {
    var queue: [(AXUIElement, Int)] = [(root, 0)]
    var cursor = 0
    while cursor < queue.count {
        let (element, depth) = queue[cursor]
        cursor += 1
        let identifier = stringAttribute(element, kAXIdentifierAttribute as CFString)
            ?? stringAttribute(element, "AXDOMIdentifier" as CFString)
        if let identifier, identifiers.contains(identifier) {
            return (identifier, element)
        }
        if depth < maxDepth {
            queue.append(contentsOf: children(element).map { ($0, depth + 1) })
        }
    }
    return nil
}

private struct CachedControl {
    let id: String
    let element: AXUIElement
}

private func cacheControl(id: String, element: AXUIElement?) -> CachedControl? {
    guard let element else { return nil }
    return CachedControl(id: id, element: element)
}

private func timedRect(_ element: AXUIElement) -> TimedRead<Rect> {
    let start = monotonicNs()
    let value = axRect(element)
    let end = monotonicNs()
    return TimedRead(
        startNs: start,
        endNs: end,
        midpointNs: start + (end - start) / 2,
        value: value,
        error: value == nil ? "AX frame unavailable" : nil
    )
}

private func timedPinnedWindowRect(_ windowNumber: Int) -> TimedRead<Rect> {
    let start = monotonicNs()
    let value = windows(numbers: [windowNumber]).first?.boundsPt
    let end = monotonicNs()
    return TimedRead(
        startNs: start,
        endNs: end,
        midpointNs: start + (end - start) / 2,
        value: value,
        error: value == nil ? "pinned CGWindow frame unavailable" : nil
    )
}

private func timedOwner(_ element: AXUIElement) -> TimedRead<Int> {
    let start = monotonicNs()
    let value = resolvedWindowNumber(element)
    let end = monotonicNs()
    return TimedRead(
        startNs: start,
        endNs: end,
        midpointNs: start + (end - start) / 2,
        value: value,
        error: value == nil ? "AX owning window unavailable" : nil
    )
}

private func interpolate(_ before: Rect, _ after: Rect, fraction: Double) -> Rect {
    let t = min(1, max(0, fraction))
    return Rect(
        x: before.x + (after.x - before.x) * t,
        y: before.y + (after.y - before.y) * t,
        width: before.width + (after.width - before.width) * t,
        height: before.height + (after.height - before.height) * t
    )
}

private func crossesBoundary(startNs: UInt64, endNs: UInt64, boundaries: [UInt64]) -> Bool {
    boundaries.contains { $0 >= startNs && $0 <= endNs }
}

private func measuredControl(
    _ cached: CachedControl,
    frameRead: TimedRead<Rect>,
    ownerRead: TimedRead<Int>,
    mainBefore: TimedRead<Rect>,
    mainAfter: TimedRead<Rect>,
    commandedVelocityPtPerSecond: Double,
    backingScale: Double,
    topologyFresh: Bool,
    displayIntervalIndex: Int,
    eventBoundaries: [UInt64]
) -> ControlFrame {
    let interpolatedMain: Rect?
    if let before = mainBefore.value, let after = mainAfter.value {
        let denominator = max(1, mainAfter.midpointNs - mainBefore.midpointNs)
        let numerator = frameRead.midpointNs > mainBefore.midpointNs
            ? frameRead.midpointNs - mainBefore.midpointNs
            : 0
        interpolatedMain = interpolate(
            before,
            after,
            fraction: Double(numerator) / Double(denominator)
        )
    } else {
        interpolatedMain = nil
    }
    let denominator = max(1, mainAfter.midpointNs - mainBefore.midpointNs)
    let numerator = frameRead.midpointNs > mainBefore.midpointNs
        ? frameRead.midpointNs - mainBefore.midpointNs
        : 0
    let interpolationFraction = min(1, max(0, Double(numerator) / Double(denominator)))
    let mainReadUncertaintyNs = UInt64(
        (1 - interpolationFraction) * Double((mainBefore.endNs - mainBefore.startNs) / 2)
            + interpolationFraction * Double((mainAfter.endNs - mainAfter.startNs) / 2)
    )
    let controlReadUncertaintyNs = (frameRead.endNs - frameRead.startNs) / 2
    let timingUncertaintySeconds = Double(mainReadUncertaintyNs + controlReadUncertaintyNs) / 1_000_000_000
    let alignmentUncertaintyPx = commandedVelocityPtPerSecond
        * timingUncertaintySeconds
        * backingScale
    let boundaryCrossed = crossesBoundary(
        startNs: min(mainBefore.startNs, frameRead.startNs),
        endNs: max(mainAfter.endNs, frameRead.endNs),
        boundaries: eventBoundaries
    )
    let error = frameRead.error ?? ownerRead.error ?? mainBefore.error ?? mainAfter.error
    return ControlFrame(
        id: cached.id,
        framePt: frameRead.value,
        mainFramePtAtMeasurement: interpolatedMain,
        axWindowNumber: ownerRead.value,
        measurementSource: "live-ax+bracketed-main-v2",
        error: error,
        frameRead: frameRead,
        ownerRead: ownerRead,
        alignmentUncertaintyPx: alignmentUncertaintyPx,
        topologyFresh: topologyFresh,
        displayIntervalIndex: displayIntervalIndex,
        crossesEventBoundary: boundaryCrossed
    )
}

private func displayInfo(for frame: Rect?) -> DisplayInfo? {
    guard let frame else { return nil }
    let centre = CGPoint(x: frame.x + frame.width / 2, y: frame.y + frame.height / 2)
    for screen in NSScreen.screens {
        let screenFrame = screen.frame
        if screenFrame.contains(centre) {
            let displayID = (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value ?? 0
            let mode = CGDisplayCopyDisplayMode(displayID)
            let refresh = mode?.refreshRate ?? 0
            return DisplayInfo(
                displayID: displayID,
                refreshHz: refresh > 1 ? refresh : 60,
                backingScale: screen.backingScaleFactor,
                boundsPt: Rect(
                    x: screenFrame.origin.x,
                    y: screenFrame.origin.y,
                    width: screenFrame.width,
                    height: screenFrame.height
                )
            )
        }
    }
    return nil
}

private func trajectory(
    _ name: String,
    frame: Rect,
    display: DisplayInfo?
) -> (delta: CGPoint, duration: TimeInterval) {
    let screen = display?.boundsPt ?? Rect(x: 0, y: 0, width: 1512, height: 982)
    func horizontal(_ preferred: Double) -> Double {
        if preferred > 0, frame.x + frame.width + preferred > screen.x + screen.width - 20 {
            return -preferred
        }
        if preferred < 0, frame.x + preferred < screen.x + 20 {
            return -preferred
        }
        return preferred
    }
    switch name {
    case "slow-horizontal": return (CGPoint(x: horizontal(280), y: 0), 0.9)
    case "diagonal":
        let y = frame.y + frame.height + 120 > screen.y + screen.height - 20 ? -120.0 : 120.0
        return (CGPoint(x: horizontal(240), y: y), 0.7)
    default: return (CGPoint(x: horizontal(-240), y: 0), 0.33)
    }
}

private struct TopologySnapshot {
    let startNs: UInt64
    let endNs: UInt64
    let windows: [NativeWindow]
    var midpointNs: UInt64 { startNs + (endNs - startNs) / 2 }
}

private func timedTopology(pid: Int32) -> TopologySnapshot {
    let start = monotonicNs()
    let inventory = windows(for: pid).filter {
        $0.onscreen && $0.alpha > 0 && $0.boundsPt.width > 1 && $0.boundsPt.height > 1
    }
    let end = monotonicNs()
    return TopologySnapshot(startNs: start, endNs: end, windows: inventory)
}

private final class EvidenceStore: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [Sample] = []
    private var events: [EventRecord] = []
    private var observedSequences = Set<Int>()
    private var untaggedInputCount = 0
    private var mouseDownNs: UInt64?
    private var mouseUpNs: UInt64?
    private var stopped = false
    private var health = ObserverHealth(
        scheduledPackets: 0, completedPackets: 0, missedPackets: 0,
        axTimeoutCount: 0, topologyStaleCount: 0,
        displayTickIntervalsMs: [], queueLatenessMs: [], mainAXCallMs: [],
        leftAXCallMs: [], rightAXCallMs: [], ownerAXCallMs: [], cgInventoryCallMs: []
    )

    func append(_ sample: Sample) {
        lock.lock(); samples.append(sample); lock.unlock()
    }

    func record(_ event: EventRecord) {
        lock.lock(); defer { lock.unlock() }
        events.append(event)
        if event.kind == "mouseDown" { mouseDownNs = event.actualEventNs }
        if event.kind == "mouseUp" { mouseUpNs = event.actualEventNs }
    }

    func observed(sequence: Int, tagged: Bool) {
        lock.lock(); defer { lock.unlock() }
        if tagged { observedSequences.insert(sequence) }
        else if mouseDownNs != nil && mouseUpNs == nil { untaggedInputCount += 1 }
    }

    func eventBoundaries() -> [UInt64] {
        lock.lock(); defer { lock.unlock() }
        return [mouseDownNs, mouseUpNs].compactMap { $0 }
    }

    func phase(at timestamp: UInt64) -> String {
        lock.lock(); defer { lock.unlock() }
        guard let down = mouseDownNs else { return "pre" }
        if timestamp < down { return "pre" }
        guard let up = mouseUpNs else { return "dragged" }
        return timestamp < up ? "dragged" : "settling"
    }

    func setStopped() { lock.lock(); stopped = true; lock.unlock() }
    func isStopped() -> Bool { lock.lock(); defer { lock.unlock() }; return stopped }

    func recordTickInterval(_ milliseconds: Double) {
        lock.lock(); health.displayTickIntervalsMs.append(milliseconds); lock.unlock()
    }
    func recordScheduled() { lock.lock(); health.scheduledPackets += 2; lock.unlock() }
    func recordCompleted(_ count: Int) { lock.lock(); health.completedPackets += count; lock.unlock() }
    func recordMissed(_ count: Int = 2) { lock.lock(); health.missedPackets += count; lock.unlock() }
    func recordQueueLateness(_ value: Double) { lock.lock(); health.queueLatenessMs.append(value); lock.unlock() }
    func recordTopologyDuration(_ value: Double) { lock.lock(); health.cgInventoryCallMs.append(value); lock.unlock() }
    func recordMainDuration(_ value: Double) { lock.lock(); health.mainAXCallMs.append(value); lock.unlock() }
    func recordControlDuration(_ value: Double, left: Bool) {
        lock.lock()
        if left { health.leftAXCallMs.append(value) } else { health.rightAXCallMs.append(value) }
        lock.unlock()
    }
    func recordOwnerDuration(_ value: Double) { lock.lock(); health.ownerAXCallMs.append(value); lock.unlock() }
    func recordAXTimeout() { lock.lock(); health.axTimeoutCount += 1; lock.unlock() }
    func recordTopologyStale() { lock.lock(); health.topologyStaleCount += 1; lock.unlock() }

    func values() -> [Sample] { lock.lock(); defer { lock.unlock() }; return samples }
    func mouseDownTimestamp() -> UInt64? { lock.lock(); defer { lock.unlock() }; return mouseDownNs }
    func mouseUpTimestamp() -> UInt64? { lock.lock(); defer { lock.unlock() }; return mouseUpNs }
    func eventValues() -> [EventRecord] {
        lock.lock(); defer { lock.unlock() }
        return events.map { value in
            var copy = value
            copy.observedByEventTap = observedSequences.contains(value.sequence)
            return copy
        }
    }
    func interference(frontmostChanged: Bool) -> Interference {
        lock.lock(); defer { lock.unlock() }
        return Interference(
            untaggedInputCount: untaggedInputCount,
            frontmostAppChanged: frontmostChanged,
            pointerDeviationPx: 0,
            targetMovedExternally: false
        )
    }
    func observerHealth() -> ObserverHealth { lock.lock(); defer { lock.unlock() }; return health }
}

private struct DisplayTick {
    let index: Int
    let hostTicks: UInt64
    let ns: UInt64
}

private final class DisplayTimeline: @unchecked Sendable {
    private let lock = NSLock()
    private let semaphore = DispatchSemaphore(value: 0)
    private var pending: DisplayTick?
    private var nextIndex = 0
    private var previousNs: UInt64?
    private let evidence: EvidenceStore

    init(evidence: EvidenceStore) { self.evidence = evidence }

    func push(hostTicks: UInt64) {
        let ns = HostClock.ns(hostTicks)
        lock.lock()
        if let prior = previousNs, ns > prior {
            evidence.recordTickInterval(Double(ns - prior) / 1_000_000)
        }
        previousNs = ns
        let tick = DisplayTick(index: nextIndex, hostTicks: hostTicks, ns: ns)
        nextIndex += 1
        let shouldSignal = pending == nil
        if !shouldSignal { evidence.recordMissed() }
        pending = tick
        lock.unlock()
        if shouldSignal { semaphore.signal() }
    }

    func next(timeout: DispatchTime) -> DisplayTick? {
        guard semaphore.wait(timeout: timeout) == .success else { return nil }
        lock.lock(); defer { lock.unlock() }
        let value = pending
        pending = nil
        return value
    }
}

private let displayLinkCallback: CVDisplayLinkOutputCallback = { _, _, outputTime, _, _, context in
    guard let context else { return kCVReturnError }
    let timeline = Unmanaged<DisplayTimeline>.fromOpaque(context).takeUnretainedValue()
    timeline.push(hostTicks: outputTime.pointee.hostTime)
    return kCVReturnSuccess
}

private final class EventTapMonitor: @unchecked Sendable {
    let evidence: EvidenceStore
    init(evidence: EvidenceStore) { self.evidence = evidence }

    func handle(type: CGEventType, event: CGEvent) {
        guard [.leftMouseDown, .leftMouseDragged, .leftMouseUp, .keyDown].contains(type) else { return }
        let tag = event.getIntegerValueField(.eventSourceUserData)
        let sequence = Int(event.getIntegerValueField(.eventSourceUserID))
        evidence.observed(sequence: sequence, tagged: tag == eventTag)
    }
}

private let eventTapCallback: CGEventTapCallBack = { _, type, event, context in
    guard let context else { return Unmanaged.passUnretained(event) }
    let monitor = Unmanaged<EventTapMonitor>.fromOpaque(context).takeUnretainedValue()
    monitor.handle(type: type, event: event)
    return Unmanaged.passUnretained(event)
}

private final class FilmstripStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [FilmstripFrame] = []
    func append(_ frame: FilmstripFrame) { lock.lock(); storage.append(frame); lock.unlock() }
    func values() -> [FilmstripFrame] {
        lock.lock(); defer { lock.unlock() }
        return storage.sorted { $0.fraction < $1.fraction }
    }
}

private func resolveShareableWindow(_ windowNumber: Int) -> (window: SCWindow?, error: String?) {
    let semaphore = DispatchSemaphore(value: 0)
    let resultLock = NSLock()
    var resultError: String?
    var resultWindow: SCWindow?
    SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, error in
        resultLock.lock()
        if let content {
            resultWindow = content.windows.first { Int($0.windowID) == windowNumber }
            if resultWindow == nil { resultError = "ScreenCaptureKit window \(windowNumber) unavailable" }
        } else {
            resultError = "ScreenCaptureKit content unavailable: \(error?.localizedDescription ?? "unknown error")"
        }
        resultLock.unlock(); semaphore.signal()
    }
    if semaphore.wait(timeout: .now() + 5) == .timedOut { return (nil, "ScreenCaptureKit content lookup timed out") }
    resultLock.lock(); defer { resultLock.unlock() }
    return (resultWindow, resultError)
}

private struct CapturedBuffer {
    let pixelBuffer: CVPixelBuffer
    let actualFrameNs: UInt64
}

private final class WindowStreamCapture: NSObject, SCStreamOutput, @unchecked Sendable {
    private let lock = NSLock()
    private var latest: CapturedBuffer?

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard outputType == .screen, sampleBuffer.isValid, let pixelBuffer = sampleBuffer.imageBuffer else { return }
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false)
            as? [[SCStreamFrameInfo: Any]]
        let displayTicks = (attachments?.first?[.displayTime] as? NSNumber)?.uint64Value
            ?? HostClock.ticks()
        lock.lock()
        latest = CapturedBuffer(pixelBuffer: pixelBuffer, actualFrameNs: HostClock.ns(displayTicks))
        lock.unlock()
    }

    func snapshot() -> CapturedBuffer? { lock.lock(); defer { lock.unlock() }; return latest }

    func write(_ captured: CapturedBuffer, to path: String) -> String? {
        let context = CIContext(options: [.cacheIntermediates: false])
        let image = CIImage(cvPixelBuffer: captured.pixelBuffer)
        guard let rendered = context.createCGImage(image, from: image.extent) else { return "CI render failed" }
        guard let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, "public.png" as CFString, 1, nil) else {
            return "PNG destination creation failed"
        }
        CGImageDestinationAddImage(destination, rendered, nil)
        return CGImageDestinationFinalize(destination) ? nil : "PNG finalization failed"
    }
}

private func startWindowStream(_ window: SCWindow?) -> (stream: SCStream?, capture: WindowStreamCapture?, error: String?) {
    guard let window else { return (nil, nil, "ScreenCaptureKit window unavailable") }
    let capture = WindowStreamCapture()
    let configuration = SCStreamConfiguration()
    configuration.width = Int(window.frame.width * 2)
    configuration.height = Int(window.frame.height * 2)
    configuration.showsCursor = false
    configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)
    configuration.queueDepth = 3
    let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: window), configuration: configuration, delegate: nil)
    let outputQueue = DispatchQueue(label: "script-kit.native-drag-screen-stream", qos: .utility)
    do { try stream.addStreamOutput(capture, type: .screen, sampleHandlerQueue: outputQueue) }
    catch { return (nil, nil, "ScreenCaptureKit output setup failed: \(error.localizedDescription)") }
    let semaphore = DispatchSemaphore(value: 0)
    var startError: Error?
    stream.startCapture { error in startError = error; semaphore.signal() }
    if semaphore.wait(timeout: .now() + 5) == .timedOut { return (nil, nil, "ScreenCaptureKit stream start timed out") }
    if let startError { return (nil, nil, "ScreenCaptureKit stream start failed: \(startError.localizedDescription)") }
    let deadline = Date(timeIntervalSinceNow: 2)
    while capture.snapshot() == nil && Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
    return capture.snapshot() != nil ? (stream, capture, nil) : (nil, nil, "ScreenCaptureKit stream produced no initial frame")
}

private func postMouseEvent(
    type: CGEventType,
    point: CGPoint,
    intendedNs: UInt64,
    sequence: Int,
    kind: String
) -> EventRecord? {
    guard let event = CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: .left) else { return nil }
    event.setIntegerValueField(.eventSourceUserData, value: eventTag)
    event.setIntegerValueField(.eventSourceUserID, value: Int64(sequence))
    let start = monotonicNs()
    event.post(tap: .cghidEventTap)
    let end = monotonicNs()
    return EventRecord(
        kind: kind, sequence: sequence, tag: eventTag, intendedNs: intendedNs,
        actualEventNs: start + (end - start) / 2, postStartNs: start, postEndNs: end,
        observedByEventTap: false
    )
}

private func write(_ output: Output, to path: String?) throws {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    let data = try encoder.encode(output)
    if let path {
        try data.write(to: URL(fileURLWithPath: path), options: .atomic)
    }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
}

private struct CaptureMarker {
    let fraction: Double
    let markerEventNs: UInt64
    let path: String
    let captured: CapturedBuffer?
}

private final class CaptureMarkerStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [CaptureMarker] = []
    func append(_ marker: CaptureMarker) { lock.lock(); storage.append(marker); lock.unlock() }
    func values() -> [CaptureMarker] { lock.lock(); defer { lock.unlock() }; return storage }
}

private func main() throws {
    let arguments = parseArguments()
    guard let pid = arguments.pid else {
        fputs("missing --pid\n", stderr)
        exit(2)
    }

    let startedAt = ISO8601DateFormatter().string(from: Date())
    let trusted = AXIsProcessTrusted()
    let app = AXUIElementCreateApplication(pid)
    AXUIElementSetMessagingTimeout(app, 0.05)
    var errors: [String] = []
    if !trusted { errors.append("accessibility permission is not trusted") }

    let initial = relevantWindows(for: pid)
    let pinnedMainNumber = arguments.mainWindowNumber ?? initial.main?.windowNumber
    guard let mainNumber = pinnedMainNumber,
          let initialMain = initial.all.first(where: { $0.windowNumber == mainNumber }) else {
        let output = Output(
            schemaVersion: schemaVersion, status: "invalid", pid: pid,
            trajectory: arguments.trajectory, durationMs: 0,
            requestedDeltaPt: Point(x: 0, y: 0), accessibilityTrusted: trusted,
            display: nil, startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            sampleTargetHz: 0, mouseUpEventNs: nil, samples: [],
            filmstripFrames: [], errors: errors + ["pinned main native window unresolved"]
        )
        try write(output, to: arguments.output)
        exit(1)
    }
    guard let mainAXWindow = findWindowElement(root: app, windowNumber: mainNumber) else {
        let output = Output(
            schemaVersion: schemaVersion, status: "invalid", pid: pid,
            trajectory: arguments.trajectory, durationMs: 0,
            requestedDeltaPt: Point(x: 0, y: 0), accessibilityTrusted: trusted,
            display: nil, startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            sampleTargetHz: 0, mouseUpEventNs: nil, samples: [],
            filmstripFrames: [], errors: errors + ["pinned main AX window unresolved"]
        )
        try write(output, to: arguments.output)
        exit(1)
    }
    AXUIElementSetMessagingTimeout(mainAXWindow, 0.05)

    let leftMatch = findElement(root: app, identifiers: [arguments.leftControlIdentifier])
    let rightIdentifiers = arguments.rightControlIdentifier.map { [$0] } ?? rightControlCandidates
    let rightMatch = rightIdentifiers.lazy.compactMap { findElement(root: app, identifiers: [$0]) }.first
    guard let leftMatch, let rightMatch else {
        if leftMatch == nil { errors.append("left AX control unresolved: \(arguments.leftControlIdentifier)") }
        if rightMatch == nil { errors.append("right AX control unresolved: \(rightIdentifiers.joined(separator: ","))") }
        let output = Output(
            schemaVersion: schemaVersion, status: "invalid", pid: pid,
            trajectory: arguments.trajectory, durationMs: 0,
            requestedDeltaPt: Point(x: 0, y: 0), accessibilityTrusted: trusted,
            display: displayInfo(for: initialMain.boundsPt), startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            sampleTargetHz: 0, mouseUpEventNs: nil, samples: [],
            filmstripFrames: [], errors: errors
        )
        try write(output, to: arguments.output)
        exit(1)
    }
    AXUIElementSetMessagingTimeout(leftMatch.1, 0.05)
    AXUIElementSetMessagingTimeout(rightMatch.1, 0.05)
    let cachedLeft = cacheControl(id: leftMatch.0, element: leftMatch.1)!
    let cachedRight = cacheControl(id: rightMatch.0, element: rightMatch.1)!

    guard let display = displayInfo(for: initialMain.boundsPt) else {
        errors.append("display timeline could not be resolved")
        let output = Output(
            schemaVersion: schemaVersion, status: "invalid", pid: pid,
            trajectory: arguments.trajectory, durationMs: 0,
            requestedDeltaPt: Point(x: 0, y: 0), accessibilityTrusted: trusted,
            display: nil, startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            sampleTargetHz: 0, mouseUpEventNs: nil, samples: [], filmstripFrames: [], errors: errors
        )
        try write(output, to: arguments.output)
        exit(1)
    }
    let targetHz = max(120, display.refreshHz * 2)
    let refreshPeriodNs = UInt64((1_000_000_000 / max(1, display.refreshHz)).rounded())
    let (delta, duration) = trajectory(arguments.trajectory, frame: initialMain.boundsPt, display: display)
    let commandedVelocity = arguments.dryRun
        ? 0
        : hypot(delta.x, delta.y) / max(0.001, duration)
    let evidence = EvidenceStore()
    let timeline = DisplayTimeline(evidence: evidence)
    let markerStore = CaptureMarkerStore()
    let filmstripStore = FilmstripStore()
    let initialFrontmostPID = NSWorkspace.shared.frontmostApplication?.processIdentifier

    let monitor = EventTapMonitor(evidence: evidence)
    let eventMask = (1 << CGEventType.leftMouseDown.rawValue)
        | (1 << CGEventType.leftMouseDragged.rawValue)
        | (1 << CGEventType.leftMouseUp.rawValue)
        | (1 << CGEventType.keyDown.rawValue)
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
        errors.append("tagged input event tap unavailable")
    }
    let globalEventMonitor = NSEvent.addGlobalMonitorForEvents(
        matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp, .keyDown]
    ) { event in
        guard let cgEvent = event.cgEvent else { return }
        monitor.handle(type: cgEvent.type, event: cgEvent)
    }

    var displayLink: CVDisplayLink?
    let displayLinkCreate = CVDisplayLinkCreateWithCGDisplay(display.displayID, &displayLink)
    if displayLinkCreate != kCVReturnSuccess || displayLink == nil {
        errors.append("CVDisplayLink creation failed: \(displayLinkCreate)")
    }
    if let displayLink {
        CVDisplayLinkSetOutputCallback(
            displayLink,
            displayLinkCallback,
            Unmanaged.passUnretained(timeline).toOpaque()
        )
        let startResult = CVDisplayLinkStart(displayLink)
        if startResult != kCVReturnSuccess { errors.append("CVDisplayLink start failed: \(startResult)") }
    }

    if let directory = arguments.filmstripDir {
        try FileManager.default.createDirectory(
            at: URL(fileURLWithPath: directory), withIntermediateDirectories: true
        )
    }
    let shareable = arguments.filmstripDir == nil
        ? (window: Optional<SCWindow>.none, error: Optional<String>.none)
        : resolveShareableWindow(mainNumber)
    if let error = shareable.error { errors.append(error) }
    let streamCapture = arguments.filmstripDir == nil
        ? (stream: Optional<SCStream>.none, capture: Optional<WindowStreamCapture>.none, error: Optional<String>.none)
        : startWindowStream(shareable.window)
    if let error = streamCapture.error { errors.append(error) }

    let geometryDone = DispatchSemaphore(value: 0)
    let geometryQueue = DispatchQueue(label: "script-kit.native-drag.geometry", qos: .userInitiated)
    geometryQueue.async {
        var previousTopology: TopologySnapshot?
        while !evidence.isStopped() {
            guard let tick = timeline.next(timeout: .now() + 0.1) else { continue }
            evidence.recordScheduled()
            let packetStarted = monotonicNs()
            evidence.recordQueueLateness(Double(packetStarted > tick.ns ? packetStarted - tick.ns : 0) / 1_000_000)
            let topologyBefore = previousTopology ?? timedTopology(pid: pid)
            if previousTopology == nil {
                evidence.recordTopologyDuration(Double(topologyBefore.endNs - topologyBefore.startNs) / 1_000_000)
            }

            let main0 = timedPinnedWindowRect(mainNumber)
            let left0Frame = timedRect(cachedLeft.element)
            let left0Owner = timedOwner(cachedLeft.element)
            let right0Frame = timedRect(cachedRight.element)
            let right0Owner = timedOwner(cachedRight.element)
            let main1 = timedPinnedWindowRect(mainNumber)
            let right1Frame = timedRect(cachedRight.element)
            let left1Frame = timedRect(cachedLeft.element)
            let main2 = timedPinnedWindowRect(mainNumber)
            let topologyAfter = timedTopology(pid: pid)
            previousTopology = topologyAfter
            evidence.recordTopologyDuration(Double(topologyAfter.endNs - topologyAfter.startNs) / 1_000_000)

            [main0, main1, main2].forEach {
                evidence.recordMainDuration(Double($0.endNs - $0.startNs) / 1_000_000)
                if $0.error != nil { evidence.recordAXTimeout() }
            }
            [(left0Frame, true), (left1Frame, true), (right0Frame, false), (right1Frame, false)].forEach {
                evidence.recordControlDuration(Double($0.0.endNs - $0.0.startNs) / 1_000_000, left: $0.1)
                if $0.0.error != nil { evidence.recordAXTimeout() }
            }
            [left0Owner, right0Owner].forEach {
                evidence.recordOwnerDuration(Double($0.endNs - $0.startNs) / 1_000_000)
                if $0.error != nil { evidence.recordAXTimeout() }
            }

            let beforeNumbers = topologyBefore.windows.map(\.windowNumber).sorted()
            let afterNumbers = topologyAfter.windows.map(\.windowNumber).sorted()
            let topologyGap = topologyAfter.midpointNs > topologyBefore.midpointNs
                ? topologyAfter.midpointNs - topologyBefore.midpointNs
                : 0
            let topologyFresh = beforeNumbers == afterNumbers
                && beforeNumbers.contains(mainNumber)
                && topologyGap <= refreshPeriodNs + 1_000_000
            if !topologyFresh { evidence.recordTopologyStale() }
            let topologyWindows = topologyAfter.windows
            let mainNative = topologyWindows.first { $0.windowNumber == mainNumber }
            let footerNative = topologyWindows.first { candidate in
                candidate.windowNumber != mainNumber
                    && abs(candidate.boundsPt.width - initialMain.boundsPt.width) <= 1
                    && candidate.boundsPt.height >= 24 && candidate.boundsPt.height <= 48
            }
            let boundaries = evidence.eventBoundaries()
            let left0 = measuredControl(
                cachedLeft, frameRead: left0Frame, ownerRead: left0Owner,
                mainBefore: main0, mainAfter: main1,
                commandedVelocityPtPerSecond: commandedVelocity,
                backingScale: display.backingScale, topologyFresh: topologyFresh,
                displayIntervalIndex: tick.index, eventBoundaries: boundaries
            )
            let right0 = measuredControl(
                cachedRight, frameRead: right0Frame, ownerRead: right0Owner,
                mainBefore: main0, mainAfter: main1,
                commandedVelocityPtPerSecond: commandedVelocity,
                backingScale: display.backingScale, topologyFresh: topologyFresh,
                displayIntervalIndex: tick.index, eventBoundaries: boundaries
            )
            let right1 = measuredControl(
                cachedRight, frameRead: right1Frame, ownerRead: right0Owner,
                mainBefore: main1, mainAfter: main2,
                commandedVelocityPtPerSecond: commandedVelocity,
                backingScale: display.backingScale, topologyFresh: topologyFresh,
                displayIntervalIndex: tick.index, eventBoundaries: boundaries
            )
            let left1 = measuredControl(
                cachedLeft, frameRead: left1Frame, ownerRead: left0Owner,
                mainBefore: main1, mainAfter: main2,
                commandedVelocityPtPerSecond: commandedVelocity,
                backingScale: display.backingScale, topologyFresh: topologyFresh,
                displayIntervalIndex: tick.index, eventBoundaries: boundaries
            )
            let inventory = topologyWindows.map(\.windowNumber).sorted()
            let common = (
                mainWindowNumber: mainNative?.windowNumber,
                mainFramePt: mainNative?.boundsPt,
                footerWindowNumber: footerNative?.windowNumber,
                footerFramePt: footerNative?.boundsPt
            )
            let firstMid = (left0Frame.midpointNs + right0Frame.midpointNs) / 2
            let secondMid = (right1Frame.midpointNs + left1Frame.midpointNs) / 2
            evidence.append(Sample(
                tNs: main1.endNs, phase: evidence.phase(at: firstMid),
                mainWindowNumber: common.mainWindowNumber, mainFramePt: common.mainFramePt,
                footerWindowNumber: common.footerWindowNumber, footerFramePt: common.footerFramePt,
                relevantWindowCount: inventory.count, relevantWindowNumbers: inventory,
                controls: [left0, right0], packetStartNs: packetStarted,
                packetEndNs: main1.endNs, displayTickNs: tick.ns,
                displayIntervalIndex: tick.index, topologyStartNs: topologyBefore.startNs,
                topologyEndNs: topologyAfter.endNs, topologyFresh: topologyFresh,
                topologyComplete: true
            ))
            evidence.append(Sample(
                tNs: topologyAfter.endNs, phase: evidence.phase(at: secondMid),
                mainWindowNumber: common.mainWindowNumber, mainFramePt: common.mainFramePt,
                footerWindowNumber: common.footerWindowNumber, footerFramePt: common.footerFramePt,
                relevantWindowCount: inventory.count, relevantWindowNumbers: inventory,
                controls: [left1, right1], packetStartNs: main1.startNs,
                packetEndNs: topologyAfter.endNs, displayTickNs: tick.ns,
                displayIntervalIndex: tick.index, topologyStartNs: topologyBefore.startNs,
                topologyEndNs: topologyAfter.endNs, topologyFresh: topologyFresh,
                topologyComplete: true
            ))
            evidence.recordCompleted(2)
        }
        geometryDone.signal()
    }

    let startPoint = CGPoint(
        x: initialMain.boundsPt.x + initialMain.boundsPt.width * 0.50,
        y: initialMain.boundsPt.y + 12
    )
    let driverDone = DispatchSemaphore(value: 0)
    DispatchQueue.global(qos: .userInitiated).async {
        if arguments.dryRun {
            Thread.sleep(forTimeInterval: 0.7)
            driverDone.signal()
            return
        }
        let epochTicks = HostClock.ticks() + HostClock.ticks(forNanoseconds: 220_000_000)
        HostClock.wait(until: epochTicks)
        let downNs = HostClock.ns(epochTicks)
        if let event = postMouseEvent(
            type: .leftMouseDown, point: startPoint, intendedNs: downNs,
            sequence: 1, kind: "mouseDown"
        ) { evidence.record(event) }
        // Input follows the real display cadence. Geometry independently
        // captures two observations per display interval; posting at that
        // doubled rate only starves WindowServer and does not add motion.
        let steps = max(36, Int(ceil(duration * display.refreshHz)))
        let captureFractions = [0.25, 0.5, 0.75]
        var nextCapture = 0
        for step in 1...steps {
            let progress = Double(step) / Double(steps)
            let intendedOffsetNs = UInt64((duration * progress * 1_000_000_000).rounded())
            let intendedTicks = epochTicks + HostClock.ticks(forNanoseconds: intendedOffsetNs)
            HostClock.wait(until: intendedTicks)
            let eased = progress * progress * (3 - 2 * progress)
            let point = CGPoint(x: startPoint.x + delta.x * eased, y: startPoint.y + delta.y * eased)
            if let event = postMouseEvent(
                type: .leftMouseDragged, point: point,
                intendedNs: HostClock.ns(intendedTicks), sequence: step + 1,
                kind: "mouseDragged"
            ) { evidence.record(event) }
            if nextCapture < captureFractions.count,
               progress >= captureFractions[nextCapture],
               let directory = arguments.filmstripDir {
                let fraction = captureFractions[nextCapture]
                let prefix = arguments.filmstripPrefix ?? arguments.trajectory
                let path = URL(fileURLWithPath: directory)
                    .appendingPathComponent("\(prefix)-filmstrip-\(nextCapture + 1).png").path
                markerStore.append(CaptureMarker(
                    fraction: fraction, markerEventNs: monotonicNs(), path: path,
                    captured: streamCapture.capture?.snapshot()
                ))
                nextCapture += 1
            }
        }
        let upTicks = epochTicks + HostClock.ticks(forNanoseconds: UInt64((duration * 1_000_000_000).rounded()))
        HostClock.wait(until: upTicks)
        let endPoint = CGPoint(x: startPoint.x + delta.x, y: startPoint.y + delta.y)
        if let event = postMouseEvent(
            type: .leftMouseUp, point: endPoint, intendedNs: HostClock.ns(upTicks),
            sequence: steps + 2, kind: "mouseUp"
        ) { evidence.record(event) }
        Thread.sleep(forTimeInterval: 0.22)
        driverDone.signal()
    }

    while driverDone.wait(timeout: .now()) != .success {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.01))
    }
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.08))
    evidence.setStopped()
    if let displayLink, CVDisplayLinkIsRunning(displayLink) { CVDisplayLinkStop(displayLink) }
    _ = geometryDone.wait(timeout: .now() + 2)

    if let stream = streamCapture.stream {
        let stop = DispatchSemaphore(value: 0)
        stream.stopCapture { _ in stop.signal() }
        _ = stop.wait(timeout: .now() + 2)
    }
    let samples = evidence.values()
    for marker in markerStore.values() {
        let nearestMain = marker.captured.flatMap { captured in
            samples.min { lhs, rhs in
                let lhsDistance = lhs.tNs > captured.actualFrameNs ? lhs.tNs - captured.actualFrameNs : captured.actualFrameNs - lhs.tNs
                let rhsDistance = rhs.tNs > captured.actualFrameNs ? rhs.tNs - captured.actualFrameNs : captured.actualFrameNs - rhs.tNs
                return lhsDistance < rhsDistance
            }?.mainFramePt
        }
        let captureError: String?
        if let captured = marker.captured, let capture = streamCapture.capture {
            captureError = capture.write(captured, to: marker.path)
        } else {
            captureError = "ScreenCaptureKit frame unavailable at marker"
        }
        filmstripStore.append(FilmstripFrame(
            fraction: marker.fraction,
            tNs: marker.captured?.actualFrameNs ?? marker.markerEventNs,
            actualFrameNs: marker.captured?.actualFrameNs,
            markerEventNs: marker.markerEventNs,
            encodingCompletedNs: monotonicNs(), mainFramePt: nearestMain,
            path: marker.path, captureSucceeded: captureError == nil, error: captureError
        ))
    }

    if let eventTapSource { CFRunLoopRemoveSource(CFRunLoopGetCurrent(), eventTapSource, .commonModes) }
    if let eventTap { CGEvent.tapEnable(tap: eventTap, enable: false) }
    if let globalEventMonitor { NSEvent.removeMonitor(globalEventMonitor) }
    let finalFrontmostPID = NSWorkspace.shared.frontmostApplication?.processIdentifier
    let frontmostChanged = initialFrontmostPID != nil && finalFrontmostPID != nil
        && initialFrontmostPID != finalFrontmostPID
    let output = Output(
        schemaVersion: schemaVersion,
        status: errors.isEmpty ? "ok" : "invalid",
        pid: pid, trajectory: arguments.trajectory,
        durationMs: arguments.dryRun ? 700 : duration * 1000,
        requestedDeltaPt: Point(x: arguments.dryRun ? 0 : delta.x, y: arguments.dryRun ? 0 : delta.y),
        accessibilityTrusted: trusted, display: display, startedAt: startedAt,
        finishedAt: ISO8601DateFormatter().string(from: Date()),
        sampleTargetHz: targetHz,
        mouseDownEventNs: evidence.mouseDownTimestamp(),
        mouseUpEventNs: evidence.mouseUpTimestamp(),
        events: evidence.eventValues(),
        interference: evidence.interference(frontmostChanged: frontmostChanged),
        observerHealth: evidence.observerHealth(),
        samples: samples, filmstripFrames: filmstripStore.values(), errors: errors
    )
    try write(output, to: arguments.output)
    exit(errors.isEmpty ? 0 : 1)
}

try main()
