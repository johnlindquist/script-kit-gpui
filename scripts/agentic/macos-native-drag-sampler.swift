#!/usr/bin/env swift

import ApplicationServices
import Cocoa
import CoreImage
import CoreMedia
import CoreGraphics
import Darwin
import Foundation
import ImageIO
import ScreenCaptureKit

private let schemaVersion = 1
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
    let layer: Int
    let onscreen: Bool
    let boundsPt: Rect
}

private struct ControlFrame: Codable {
    let id: String
    let framePt: Rect?
    let mainFramePtAtMeasurement: Rect?
    let axWindowNumber: Int?
    let measurementSource: String
    let error: String?
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
}

private struct FilmstripFrame: Codable {
    let fraction: Double
    let tNs: UInt64
    let mainFramePt: Rect?
    let path: String
    let captureSucceeded: Bool
    let error: String?
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
    let mouseUpEventNs: UInt64?
    let samples: [Sample]
    let filmstripFrames: [FilmstripFrame]
    let errors: [String]
}

private struct Arguments {
    var pid: Int32?
    var trajectory = "fast-horizontal"
    var output: String?
    var filmstripDir: String?
    var filmstripPrefix: String?
    var dryRun = false
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
        default:
            break
        }
        index += 1
    }
    return result
}

private func monotonicNs() -> UInt64 {
    var info = mach_timebase_info_data_t()
    mach_timebase_info(&info)
    let ticks = mach_continuous_time()
    return UInt64(Double(ticks) * Double(info.numer) / Double(info.denom))
}

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
        let onscreen = (entry[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false
        return NativeWindow(
            windowNumber: windowNumber,
            layer: layer,
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
            layer: (entry[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0,
            onscreen: (entry[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue ?? false,
            boundsPt: bounds
        )
    }
}

private func relevantWindows(for pid: Int32) -> (main: NativeWindow?, footer: NativeWindow?, all: [NativeWindow]) {
    let all = windows(for: pid).filter {
        $0.onscreen && $0.boundsPt.width >= 300 && $0.boundsPt.height >= 24
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
    if let number = windowNumber(element) { return number }
    var windowID = CGWindowID(0)
    return privateAXWindowNumber(element, &windowID) == .success && windowID != 0
        ? Int(windowID)
        : nil
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

private func controlFrame(id: String, element: AXUIElement?) -> ControlFrame {
    guard let element else {
        return ControlFrame(
            id: id,
            framePt: nil,
            mainFramePtAtMeasurement: nil,
            axWindowNumber: nil,
            measurementSource: "accessibility",
            error: "AX identifier unresolved"
        )
    }
    guard
        let position = pointAttribute(element, kAXPositionAttribute as CFString),
        let size = sizeAttribute(element, kAXSizeAttribute as CFString)
    else {
        return ControlFrame(
            id: id,
            framePt: nil,
            mainFramePtAtMeasurement: nil,
            axWindowNumber: resolvedWindowNumber(element),
            measurementSource: "accessibility",
            error: "AX frame unavailable"
        )
    }
    return ControlFrame(
        id: id,
        framePt: Rect(x: position.x, y: position.y, width: size.width, height: size.height),
        mainFramePtAtMeasurement: nil,
        axWindowNumber: resolvedWindowNumber(element),
        measurementSource: "accessibility",
        error: nil
    )
}

private struct CachedControl {
    let id: String
    let element: AXUIElement
}

private struct LiveControlMeasurement {
    let cached: CachedControl?
    let tNs: UInt64
    let framePt: Rect?
    let ownerWindowNumber: Int?
}

private func cacheControl(
    id: String,
    element: AXUIElement?,
    windows: [NativeWindow]
) -> CachedControl? {
    guard let element else { return nil }
    let frame = controlFrame(id: id, element: element)
    guard let controlRect = frame.framePt else { return nil }
    let centre = CGPoint(
        x: controlRect.x + controlRect.width / 2,
        y: controlRect.y + controlRect.height / 2
    )
    let explicitWindow = frame.axWindowNumber.flatMap { number in
        windows.first { $0.windowNumber == number }
    }
    let containingWindow = windows
        .filter { candidate in
            let bounds = candidate.boundsPt
            return centre.x >= bounds.x && centre.x <= bounds.x + bounds.width
                && centre.y >= bounds.y && centre.y <= bounds.y + bounds.height
        }
        .min { lhs, rhs in
            lhs.boundsPt.width * lhs.boundsPt.height < rhs.boundsPt.width * rhs.boundsPt.height
        }
    guard explicitWindow != nil || containingWindow != nil else { return nil }
    return CachedControl(id: id, element: element)
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

private func measureLiveControl(_ cached: CachedControl?) -> LiveControlMeasurement {
    guard let cached else {
        return LiveControlMeasurement(cached: nil, tNs: monotonicNs(), framePt: nil, ownerWindowNumber: nil)
    }
    let frame = axRect(cached.element)
    let owner = resolvedWindowNumber(cached.element)
    return LiveControlMeasurement(
        cached: cached,
        tNs: monotonicNs(),
        framePt: frame,
        ownerWindowNumber: owner
    )
}

private func controlFrame(
    measurement: LiveControlMeasurement,
    fallbackID: String,
    mainBefore: (tNs: UInt64, frame: Rect?),
    mainAfter: (tNs: UInt64, frame: Rect?)
) -> ControlFrame {
    let id = measurement.cached?.id ?? fallbackID
    let interpolatedMain: Rect?
    if let before = mainBefore.frame, let after = mainAfter.frame {
        let denominator = max(1, mainAfter.tNs - mainBefore.tNs)
        let fraction = Double(measurement.tNs - mainBefore.tNs) / Double(denominator)
        interpolatedMain = interpolate(before, after, fraction: fraction)
    } else {
        interpolatedMain = nil
    }
    return ControlFrame(
        id: id,
        framePt: measurement.framePt,
        mainFramePtAtMeasurement: interpolatedMain,
        axWindowNumber: measurement.ownerWindowNumber,
        measurementSource: "live-ax+interpolated-main",
        error: measurement.cached == nil
            ? "cached AX control unavailable"
            : measurement.framePt == nil
                ? "live AX frame unavailable"
                : interpolatedMain == nil
                    ? "main AX frame unavailable around control query"
                    : measurement.ownerWindowNumber == nil
                        ? "live AX owning window unavailable"
                        : nil
    )
}

private func liveControlFrame(
    cached: CachedControl?,
    mainAXWindow: AXUIElement,
    mainSize: CGSize,
    fallbackID: String
) -> ControlFrame {
    let mainPositionBefore = pointAttribute(mainAXWindow, kAXPositionAttribute as CFString)
    let mainBefore = (
        tNs: monotonicNs(),
        frame: mainPositionBefore.map {
            Rect(x: $0.x, y: $0.y, width: mainSize.width, height: mainSize.height)
        }
    )
    let measurement = measureLiveControl(cached)
    let mainPositionAfter = pointAttribute(mainAXWindow, kAXPositionAttribute as CFString)
    let mainAfter = (
        tNs: monotonicNs(),
        frame: mainPositionAfter.map {
            Rect(x: $0.x, y: $0.y, width: mainSize.width, height: mainSize.height)
        }
    )
    return controlFrame(
        measurement: measurement,
        fallbackID: fallbackID,
        mainBefore: mainBefore,
        mainAfter: mainAfter
    )
}

private final class ControlFrameStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String: ControlFrame] = [:]

    func set(_ frame: ControlFrame, for key: String) {
        lock.lock(); defer { lock.unlock() }
        storage[key] = frame
    }

    func get(_ key: String) -> ControlFrame? {
        lock.lock(); defer { lock.unlock() }
        return storage[key]
    }
}

private final class NativeSnapshotStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: (main: NativeWindow?, footer: NativeWindow?, all: [NativeWindow])?

    func set(_ snapshot: (main: NativeWindow?, footer: NativeWindow?, all: [NativeWindow])) {
        lock.lock(); defer { lock.unlock() }
        storage = snapshot
    }

    func get() -> (main: NativeWindow?, footer: NativeWindow?, all: [NativeWindow])? {
        lock.lock(); defer { lock.unlock() }
        return storage
    }
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
    default: return (CGPoint(x: horizontal(-220), y: 0), 0.3)
    }
}

private final class SampleStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [Sample] = []
    private var phase = "pre"
    private var mouseUpNs: UInt64?

    func setPhase(_ phase: String) {
        lock.lock(); defer { lock.unlock() }
        self.phase = phase
    }

    func currentPhase() -> String {
        lock.lock(); defer { lock.unlock() }
        return phase
    }

    func markMouseUp(_ timestamp: UInt64) {
        lock.lock(); defer { lock.unlock() }
        mouseUpNs = timestamp
        phase = "mouseUp"
    }

    func mouseUpTimestamp() -> UInt64? {
        lock.lock(); defer { lock.unlock() }
        return mouseUpNs
    }

    func append(_ sample: Sample) {
        lock.lock(); defer { lock.unlock() }
        storage.append(sample)
    }

    func values() -> [Sample] {
        lock.lock(); defer { lock.unlock() }
        return storage
    }
}

private final class FilmstripStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [FilmstripFrame] = []

    func append(_ frame: FilmstripFrame) {
        lock.lock(); defer { lock.unlock() }
        storage.append(frame)
    }

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
        guard let content else {
            resultLock.lock()
            resultError = "ScreenCaptureKit content unavailable: \(error?.localizedDescription ?? "unknown error")"
            resultLock.unlock()
            semaphore.signal()
            return
        }
        guard let window = content.windows.first(where: { Int($0.windowID) == windowNumber }) else {
            resultLock.lock()
            resultError = "ScreenCaptureKit window \(windowNumber) unavailable"
            resultLock.unlock()
            semaphore.signal()
            return
        }
        resultLock.lock(); resultWindow = window; resultLock.unlock()
        semaphore.signal()
    }
    if semaphore.wait(timeout: .now() + 5) == .timedOut {
        return (nil, "ScreenCaptureKit content lookup timed out")
    }
    resultLock.lock(); defer { resultLock.unlock() }
    return (resultWindow, resultError)
}

private final class WindowStreamCapture: NSObject, SCStreamOutput, @unchecked Sendable {
    private let lock = NSLock()
    private let context = CIContext(options: [.cacheIntermediates: false])
    private var latestImage: CGImage?

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard outputType == .screen,
              sampleBuffer.isValid,
              let pixelBuffer = sampleBuffer.imageBuffer else { return }
        let image = CIImage(cvPixelBuffer: pixelBuffer)
        guard let rendered = context.createCGImage(image, from: image.extent) else { return }
        lock.lock(); latestImage = rendered; lock.unlock()
    }

    func hasFrame() -> Bool {
        lock.lock(); defer { lock.unlock() }
        return latestImage != nil
    }

    func writeLatest(to path: String) -> String? {
        lock.lock(); let image = latestImage; lock.unlock()
        guard let image else { return "ScreenCaptureKit stream has no frame" }
        let url = URL(fileURLWithPath: path) as CFURL
        guard let destination = CGImageDestinationCreateWithURL(
            url,
            "public.png" as CFString,
            1,
            nil
        ) else { return "PNG destination creation failed" }
        CGImageDestinationAddImage(destination, image, nil)
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
    configuration.queueDepth = 5
    let stream = SCStream(
        filter: SCContentFilter(desktopIndependentWindow: window),
        configuration: configuration,
        delegate: nil
    )
    let outputQueue = DispatchQueue(label: "script-kit.native-drag-screen-stream", qos: .userInteractive)
    do {
        try stream.addStreamOutput(capture, type: .screen, sampleHandlerQueue: outputQueue)
    } catch {
        return (nil, nil, "ScreenCaptureKit output setup failed: \(error.localizedDescription)")
    }
    let semaphore = DispatchSemaphore(value: 0)
    var startError: Error?
    stream.startCapture { error in startError = error; semaphore.signal() }
    if semaphore.wait(timeout: .now() + 5) == .timedOut {
        return (nil, nil, "ScreenCaptureKit stream start timed out")
    }
    if let startError {
        return (nil, nil, "ScreenCaptureKit stream start failed: \(startError.localizedDescription)")
    }
    let deadline = Date(timeIntervalSinceNow: 2)
    while !capture.hasFrame() && Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
    return capture.hasFrame()
        ? (stream, capture, nil)
        : (nil, nil, "ScreenCaptureKit stream produced no initial frame")
}

private func postMouseEvent(type: CGEventType, point: CGPoint) {
    guard let event = CGEvent(
        mouseEventSource: nil,
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: .left
    ) else { return }
    event.post(tap: .cghidEventTap)
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

private func main() throws {
    let arguments = parseArguments()
    guard let pid = arguments.pid else {
        fputs("missing --pid\n", stderr)
        exit(2)
    }

    let startedAt = ISO8601DateFormatter().string(from: Date())
    let trusted = AXIsProcessTrusted()
    let app = AXUIElementCreateApplication(pid)
    let left = findElement(root: app, identifiers: [leftControlID])
    let right = rightControlCandidates.lazy.compactMap { candidate in
        findElement(root: app, identifiers: [candidate])
    }.first
    var errors: [String] = []
    if !trusted { errors.append("accessibility permission is not trusted") }
    if left == nil { errors.append("left AX control unresolved: \(leftControlID)") }
    if right == nil { errors.append("right AX control unresolved: \(rightControlCandidates.joined(separator: ","))") }

    let initial = relevantWindows(for: pid)
    guard let initialMain = initial.main else {
        let output = Output(
        schemaVersion: schemaVersion,
        status: "invalid",
        pid: pid,
        trajectory: arguments.trajectory,
        durationMs: 0,
        requestedDeltaPt: Point(x: 0, y: 0),
        accessibilityTrusted: trusted,
        display: nil,
        startedAt: startedAt,
        finishedAt: ISO8601DateFormatter().string(from: Date()),
        sampleTargetHz: 0,
        mouseUpEventNs: nil,
        samples: [],
        filmstripFrames: [],
        errors: errors + ["main native window unresolved"]
    )
        try write(output, to: arguments.output)
        exit(1)
    }
    guard let mainAXWindow = findWindowElement(root: app, windowNumber: initialMain.windowNumber) else {
        let output = Output(
            schemaVersion: schemaVersion,
            status: "invalid",
            pid: pid,
            trajectory: arguments.trajectory,
            durationMs: 0,
            requestedDeltaPt: Point(x: 0, y: 0),
            accessibilityTrusted: trusted,
            display: nil,
            startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            sampleTargetHz: 0,
            mouseUpEventNs: nil,
            samples: [],
            filmstripFrames: [],
            errors: errors + ["main AX window unresolved"]
        )
        try write(output, to: arguments.output)
        exit(1)
    }

    let cachedLeft = cacheControl(
        id: left?.0 ?? leftControlID,
        element: left?.1,
        windows: initial.all
    )
    let cachedRight = cacheControl(
        id: right?.0 ?? rightControlCandidates[0],
        element: right?.1,
        windows: initial.all
    )
    if cachedLeft == nil { errors.append("left AX control could not be bound to a native window") }
    if cachedRight == nil { errors.append("right AX control could not be bound to a native window") }
    let display = displayInfo(for: initialMain.boundsPt)
    let targetHz = max(120, (display?.refreshHz ?? 60) * 2)
    let interval = 1.0 / targetHz
    let store = SampleStore()
    let filmstripStore = FilmstripStore()
    let captureGroup = DispatchGroup()
    if let directory = arguments.filmstripDir {
        try FileManager.default.createDirectory(
            at: URL(fileURLWithPath: directory),
            withIntermediateDirectories: true
        )
    }
    let shareableWindowLookup = arguments.filmstripDir == nil
        ? (window: Optional<SCWindow>.none, error: Optional<String>.none)
        : resolveShareableWindow(initialMain.windowNumber)
    if let captureError = shareableWindowLookup.error {
        errors.append(captureError)
    }
    let streamCapture = arguments.filmstripDir == nil
        ? (stream: Optional<SCStream>.none, capture: Optional<WindowStreamCapture>.none, error: Optional<String>.none)
        : startWindowStream(shareableWindowLookup.window)
    if let captureError = streamCapture.error {
        errors.append(captureError)
    }
    let appendSample: () -> Void = {
        let frameStore = ControlFrameStore()
        let snapshotStore = NativeSnapshotStore()
        let measurementGroup = DispatchGroup()
        let mainSize = CGSize(width: initialMain.boundsPt.width, height: initialMain.boundsPt.height)
        measurementGroup.enter()
        DispatchQueue.global(qos: .userInteractive).async {
            snapshotStore.set(relevantWindows(for: pid))
            measurementGroup.leave()
        }
        measurementGroup.enter()
        DispatchQueue.global(qos: .userInteractive).async {
            frameStore.set(
                liveControlFrame(
                    cached: cachedLeft,
                    mainAXWindow: mainAXWindow,
                    mainSize: mainSize,
                    fallbackID: leftControlID
                ),
                for: "left"
            )
            measurementGroup.leave()
        }
        measurementGroup.enter()
        DispatchQueue.global(qos: .userInteractive).async {
            frameStore.set(
                liveControlFrame(
                    cached: cachedRight,
                    mainAXWindow: mainAXWindow,
                    mainSize: mainSize,
                    fallbackID: rightControlCandidates[0]
                ),
                for: "right"
            )
            measurementGroup.leave()
        }
        measurementGroup.wait()
        let snapshot = snapshotStore.get() ?? (main: nil, footer: nil, all: [])
        let mainWindow = snapshot.all.first { $0.windowNumber == initialMain.windowNumber }
        let footerWindow = snapshot.footer
        let controls = [
            frameStore.get("left"),
            frameStore.get("right"),
        ].compactMap { $0 }
        store.append(Sample(
            tNs: monotonicNs(),
            phase: store.currentPhase(),
            mainWindowNumber: mainWindow?.windowNumber,
            mainFramePt: mainWindow?.boundsPt,
            footerWindowNumber: footerWindow?.windowNumber,
            footerFramePt: footerWindow?.boundsPt,
            relevantWindowCount: snapshot.all.count,
            relevantWindowNumbers: snapshot.all.map(\.windowNumber).sorted(),
            controls: controls
        ))
    }
    let timer = Timer(timeInterval: interval, repeats: true) { _ in appendSample() }
    timer.tolerance = 0
    RunLoop.current.add(timer, forMode: .common)
    RunLoop.current.add(timer, forMode: RunLoop.Mode("NSEventTrackingRunLoopMode"))

    let (delta, duration) = trajectory(
        arguments.trajectory,
        frame: initialMain.boundsPt,
        display: display
    )
    let start = CGPoint(
    x: initialMain.boundsPt.x + initialMain.boundsPt.width * 0.50,
    y: initialMain.boundsPt.y + 12
    )

    let driverDone = DispatchSemaphore(value: 0)
    DispatchQueue.global(qos: .userInteractive).async {
        Thread.sleep(forTimeInterval: 0.12)
        if !arguments.dryRun {
            store.setPhase("mouseDown")
            postMouseEvent(type: .leftMouseDown, point: start)
            Thread.sleep(forTimeInterval: 0.025)
            store.setPhase("dragged")
            let steps = max(72, Int(duration * targetHz))
            var nextCaptureIndex = 0
            let captureFractions = [0.25, 0.5, 0.75]
            for step in 1...steps {
                let progress = Double(step) / Double(steps)
                let eased = progress * progress * (3 - 2 * progress)
                let point = CGPoint(x: start.x + delta.x * eased, y: start.y + delta.y * eased)
                postMouseEvent(type: .leftMouseDragged, point: point)
                if nextCaptureIndex < captureFractions.count,
                   progress >= captureFractions[nextCaptureIndex],
                   let directory = arguments.filmstripDir {
                    let fraction = captureFractions[nextCaptureIndex]
                    let index = nextCaptureIndex
                    let prefix = arguments.filmstripPrefix ?? arguments.trajectory
                    let path = URL(fileURLWithPath: directory)
                        .appendingPathComponent("\(prefix)-filmstrip-\(index + 1).png").path
                    let captureTime = monotonicNs()
                    let captureMainFrame = windows(numbers: [initialMain.windowNumber]).first?.boundsPt
                    captureGroup.enter()
                    DispatchQueue.global(qos: .userInitiated).async {
                        let captureError: String? = streamCapture.capture == nil
                            ? "ScreenCaptureKit stream capture unavailable"
                            : streamCapture.capture!.writeLatest(to: path)
                        filmstripStore.append(FilmstripFrame(
                            fraction: fraction,
                            tNs: captureTime,
                            mainFramePt: captureMainFrame,
                            path: path,
                            captureSucceeded: captureError == nil,
                            error: captureError
                        ))
                        captureGroup.leave()
                    }
                    nextCaptureIndex += 1
                }
                Thread.sleep(forTimeInterval: duration / Double(steps))
            }
            let mouseUpTime = monotonicNs()
            store.markMouseUp(mouseUpTime)
            postMouseEvent(type: .leftMouseUp, point: CGPoint(x: start.x + delta.x, y: start.y + delta.y))
            DispatchQueue.main.sync { appendSample() }
        }
        store.setPhase("settling")
        Thread.sleep(forTimeInterval: 0.16)
        driverDone.signal()
    }
    while driverDone.wait(timeout: .now()) != .success {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.01))
    }
    timer.invalidate()
    captureGroup.wait()
    if let stream = streamCapture.stream {
        let stopSemaphore = DispatchSemaphore(value: 0)
        stream.stopCapture { _ in stopSemaphore.signal() }
        _ = stopSemaphore.wait(timeout: .now() + 2)
    }

    let output = Output(
    schemaVersion: schemaVersion,
    status: errors.isEmpty ? "ok" : "invalid",
    pid: pid,
    trajectory: arguments.trajectory,
    durationMs: duration * 1000,
    requestedDeltaPt: Point(x: delta.x, y: delta.y),
    accessibilityTrusted: trusted,
    display: display,
    startedAt: startedAt,
    finishedAt: ISO8601DateFormatter().string(from: Date()),
    sampleTargetHz: targetHz,
    mouseUpEventNs: store.mouseUpTimestamp(),
    samples: store.values(),
    filmstripFrames: filmstripStore.values(),
    errors: errors
    )
    try write(output, to: arguments.output)
    exit(errors.isEmpty ? 0 : 1)
}

try main()
