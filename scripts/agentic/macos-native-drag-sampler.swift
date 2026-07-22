#!/usr/bin/env swift

import ApplicationServices
import Cocoa
import CoreGraphics
import Darwin
import Foundation

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
    let controls: [ControlFrame]
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
    let samples: [Sample]
    let errors: [String]
}

private struct Arguments {
    var pid: Int32?
    var trajectory = "fast-horizontal"
    var output: String?
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
    guard let raw = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as? [[String: Any]] else {
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

private func children(_ element: AXUIElement) -> [AXUIElement] {
    guard let values = copyAttribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] else {
        return []
    }
    return values
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
            axWindowNumber: windowNumber(element),
            measurementSource: "accessibility",
            error: "AX frame unavailable"
        )
    }
    return ControlFrame(
        id: id,
        framePt: Rect(x: position.x, y: position.y, width: size.width, height: size.height),
        axWindowNumber: windowNumber(element),
        measurementSource: "accessibility",
        error: nil
    )
}

private struct CachedControl {
    let id: String
    let element: AXUIElement
    let hostWindowNumber: Int
    let localFramePt: Rect
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
    guard let host = explicitWindow ?? containingWindow else { return nil }
    return CachedControl(
        id: id,
        element: element,
        hostWindowNumber: host.windowNumber,
        localFramePt: Rect(
            x: controlRect.x - host.boundsPt.x,
            y: controlRect.y - host.boundsPt.y,
            width: controlRect.width,
            height: controlRect.height
        )
    )
}

private func projectedControl(_ cached: CachedControl?, windows: [NativeWindow], fallbackID: String) -> ControlFrame {
    guard let cached else {
        return ControlFrame(
            id: fallbackID,
            framePt: nil,
            axWindowNumber: nil,
            measurementSource: "cached-ax-local+cgwindow",
            error: "cached AX control unavailable"
        )
    }
    guard let host = windows.first(where: { $0.windowNumber == cached.hostWindowNumber }) else {
        return ControlFrame(
            id: cached.id,
            framePt: nil,
            axWindowNumber: cached.hostWindowNumber,
            measurementSource: "cached-ax-local+cgwindow",
            error: "owning native window unavailable"
        )
    }
    return ControlFrame(
        id: cached.id,
        framePt: Rect(
            x: host.boundsPt.x + cached.localFramePt.x,
            y: host.boundsPt.y + cached.localFramePt.y,
            width: cached.localFramePt.width,
            height: cached.localFramePt.height
        ),
        axWindowNumber: host.windowNumber,
        measurementSource: "cached-ax-local+cgwindow",
        error: nil
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
    default: return (CGPoint(x: horizontal(-220), y: 0), 0.3)
    }
}

private final class SampleStore: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [Sample] = []
    private var phase = "pre"

    func setPhase(_ phase: String) {
        lock.lock(); defer { lock.unlock() }
        self.phase = phase
    }

    func currentPhase() -> String {
        lock.lock(); defer { lock.unlock() }
        return phase
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
        samples: [],
        errors: errors + ["main native window unresolved"]
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
    let trackedWindowNumbers = [
        initialMain.windowNumber,
        initial.footer?.windowNumber,
    ].compactMap { $0 }

    let display = displayInfo(for: initialMain.boundsPt)
    let targetHz = max(120, (display?.refreshHz ?? 60) * 2)
    let interval = 1.0 / targetHz
    let store = SampleStore()
    let sampler = DispatchQueue(label: "script-kit.native-drag-sampler", qos: .userInteractive)
    let timer = DispatchSource.makeTimerSource(queue: sampler)
    timer.schedule(deadline: .now(), repeating: interval, leeway: .microseconds(250))
    timer.setEventHandler {
    let trackedWindows = windows(numbers: trackedWindowNumbers)
    let mainWindow = trackedWindows.first { $0.windowNumber == initialMain.windowNumber }
    let footerWindow = initial.footer.flatMap { initialFooter in
        trackedWindows.first { $0.windowNumber == initialFooter.windowNumber }
    }
    let controls = [
        projectedControl(cachedLeft, windows: trackedWindows, fallbackID: leftControlID),
        projectedControl(cachedRight, windows: trackedWindows, fallbackID: rightControlCandidates[0]),
    ]
    store.append(Sample(
        tNs: monotonicNs(),
        phase: store.currentPhase(),
        mainWindowNumber: mainWindow?.windowNumber,
        mainFramePt: mainWindow?.boundsPt,
        footerWindowNumber: footerWindow?.windowNumber,
        footerFramePt: footerWindow?.boundsPt,
        relevantWindowCount: trackedWindows.filter(\.onscreen).count,
        controls: controls
    ))
    }

    let (delta, duration) = trajectory(
        arguments.trajectory,
        frame: initialMain.boundsPt,
        display: display
    )
    let start = CGPoint(
    x: initialMain.boundsPt.x + initialMain.boundsPt.width * 0.50,
    y: initialMain.boundsPt.y + 12
    )

    timer.resume()
    Thread.sleep(forTimeInterval: 0.12)
    if !arguments.dryRun {
    store.setPhase("mouseDown")
    postMouseEvent(type: .leftMouseDown, point: start)
    Thread.sleep(forTimeInterval: 0.025)
    store.setPhase("dragged")
    let steps = max(72, Int(duration * targetHz))
    for step in 1...steps {
        let progress = Double(step) / Double(steps)
        let eased = progress * progress * (3 - 2 * progress)
        let point = CGPoint(x: start.x + delta.x * eased, y: start.y + delta.y * eased)
        postMouseEvent(type: .leftMouseDragged, point: point)
        Thread.sleep(forTimeInterval: duration / Double(steps))
    }
    store.setPhase("mouseUp")
    postMouseEvent(type: .leftMouseUp, point: CGPoint(x: start.x + delta.x, y: start.y + delta.y))
    }
    store.setPhase("settling")
    Thread.sleep(forTimeInterval: 0.16)
    timer.cancel()
    sampler.sync {}

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
    samples: store.values(),
    errors: errors
    )
    try write(output, to: arguments.output)
    exit(errors.isEmpty ? 0 : 1)
}

try main()
