import CoreImage
import CoreMedia
import CoreVideo
import CryptoKit
import Foundation
import ImageIO
import ScreenCaptureKit
import AppKit

private struct Arguments {
    var windowID: CGWindowID?
    var pid: pid_t?
    var title: String?
    var maxWidth: Double?
    var outputDirectory: String?
    var readyPath: String?
    var durationMs = 250
    var frameRate = 120
    var displayStream = false
    var regionArmed = false
    var pinnedBounds: CGRect?
    var excludedWindowIDs: [CGWindowID] = []
    var ownerClass: String?
    var runID: String?
    var gitCommit: String?
    var binarySHA256: String?
}

private func parseArguments() -> Arguments {
    var result = Arguments()
    let values = Array(CommandLine.arguments.dropFirst())
    var index = 0
    while index < values.count {
        switch values[index] {
        case "--window-id":
            if index + 1 < values.count {
                result.windowID = CGWindowID(values[index + 1])
                index += 1
            }
        case "--pid":
            if index + 1 < values.count {
                result.pid = pid_t(values[index + 1])
                index += 1
            }
        case "--title":
            if index + 1 < values.count {
                result.title = values[index + 1]
                index += 1
            }
        case "--max-width":
            if index + 1 < values.count {
                result.maxWidth = Double(values[index + 1])
                index += 1
            }
        case "--out":
            if index + 1 < values.count {
                result.outputDirectory = values[index + 1]
                index += 1
            }
        case "--ready":
            if index + 1 < values.count {
                result.readyPath = values[index + 1]
                index += 1
            }
        case "--duration-ms":
            if index + 1 < values.count {
                result.durationMs = Int(values[index + 1]) ?? result.durationMs
                index += 1
            }
        case "--fps":
            if index + 1 < values.count {
                result.frameRate = Int(values[index + 1]) ?? result.frameRate
                index += 1
            }
        case "--display-stream":
            result.displayStream = true
        case "--region-armed":
            result.regionArmed = true
        case "--exclude-window-id":
            if index + 1 < values.count {
                if let id = CGWindowID(values[index + 1]) {
                    result.excludedWindowIDs.append(id)
                }
                index += 1
            }
        case "--owner-class":
            if index + 1 < values.count {
                result.ownerClass = values[index + 1]
                index += 1
            }
        case "--bounds":
            if index + 4 < values.count,
               let x = Double(values[index + 1]),
               let y = Double(values[index + 2]),
               let width = Double(values[index + 3]),
               let height = Double(values[index + 4]) {
                result.pinnedBounds = CGRect(
                    x: x,
                    y: y,
                    width: width,
                    height: height
                )
                index += 4
            }
        case "--run-id":
            if index + 1 < values.count {
                result.runID = values[index + 1]
                index += 1
            }
        case "--git-commit":
            if index + 1 < values.count {
                result.gitCommit = values[index + 1]
                index += 1
            }
        case "--binary-sha256":
            if index + 1 < values.count {
                result.binarySHA256 = values[index + 1]
                index += 1
            }
        default:
            break
        }
        index += 1
    }
    return result
}

private struct FrameReceipt: Codable {
    let sequence: Int
    let displayTime: UInt64
    let displayTimeNs: UInt64
    let windowBounds: CGRect?
    let windowAlpha: Double?
    let windowOnscreen: Bool?
    let expectedWindowID: UInt32
    let actualWindowID: UInt32?
    let geometrySource: String
    let path: String
    let sha256: String
}

private let machTimebase: mach_timebase_info_data_t = {
    var value = mach_timebase_info_data_t()
    mach_timebase_info(&value)
    return value
}()

private func hostTicksToNs(_ ticks: UInt64) -> UInt64 {
    let quotient = ticks / UInt64(machTimebase.denom)
    let remainder = ticks % UInt64(machTimebase.denom)
    return quotient * UInt64(machTimebase.numer)
        + remainder * UInt64(machTimebase.numer) / UInt64(machTimebase.denom)
}

@MainActor
private func displayRefreshRate(_ displayID: CGDirectDisplayID, fallback: Double) -> Double {
    for screen in NSScreen.screens {
        let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")]
            as? NSNumber
        if number?.uint32Value == displayID {
            return Double(max(1, screen.maximumFramesPerSecond))
        }
    }
    if let mode = CGDisplayCopyDisplayMode(displayID), mode.refreshRate > 0 {
        return mode.refreshRate
    }
    return fallback
}

private func windowState(_ windowID: CGWindowID) -> (
    bounds: CGRect?,
    alpha: Double?,
    onscreen: Bool?
) {
    // `optionIncludingWindow` intermittently returns an empty array while a
    // floating panel is transitioning between ordered-out and ordered-front on
    // macOS 26. The complete inventory path used by the sibling window-query
    // helper remains authoritative across that transition, so enumerate it and
    // select the already-pinned CGWindowID explicitly.
    guard let info = CGWindowListCopyWindowInfo(
        [.optionAll],
        kCGNullWindowID
    ) as? [[String: Any]], let row = info.first(where: {
        ($0[kCGWindowNumber as String] as? NSNumber)?.uint32Value == windowID
    }) else {
        return (nil, nil, nil)
    }
    let bounds = (row[kCGWindowBounds as String] as? NSDictionary)
        .flatMap { CGRect(dictionaryRepresentation: $0) }
    let alpha = (row[kCGWindowAlpha as String] as? NSNumber)?.doubleValue
    let onscreen = (row[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue
    return (bounds, alpha, onscreen)
}

private struct RegionOwnerCandidate {
    let windowID: CGWindowID
    let bounds: CGRect
    let alpha: Double?
    let onscreen: Bool?
}

/// Resolve only windows created after the pre-arm inventory. This mirrors the
/// TypeScript topology classifier but runs on every delivered display frame so
/// the first composited pixels can be bound before the control process observes
/// the new automation target. Ambiguity is returned rather than sorted away.
private func regionOwnerCandidates(
    pid: pid_t,
    excludedWindowIDs: Set<CGWindowID>,
    ownerClass: String?,
    captureFrame: CGRect
) -> [RegionOwnerCandidate] {
    guard let info = CGWindowListCopyWindowInfo(
        [.optionAll],
        kCGNullWindowID
    ) as? [[String: Any]] else { return [] }
    return info.compactMap { row in
        guard (row[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid,
              let id = (row[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
              !excludedWindowIDs.contains(id),
              let dictionary = row[kCGWindowBounds as String] as? NSDictionary,
              let bounds = CGRect(dictionaryRepresentation: dictionary),
              bounds.width > 1,
              bounds.height > 1,
              bounds.intersects(captureFrame)
        else { return nil }
        let title = row[kCGWindowName as String] as? String ?? ""
        let layer = (row[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0
        switch ownerClass {
        case "Notes":
            guard title == "Notes" else { return nil }
        case "Actions":
            let normalizedTitle = title.lowercased()
            guard normalizedTitle.contains("actions")
                || normalizedTitle.contains("command palette")
                || (title.isEmpty && layer == 101 && bounds.height > 140)
            else { return nil }
        case .some:
            return nil
        case .none:
            break
        }
        return RegionOwnerCandidate(
            windowID: id,
            bounds: bounds,
            alpha: (row[kCGWindowAlpha as String] as? NSNumber)?.doubleValue,
            onscreen: (row[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue
        )
    }
}

private struct Receipt: Codable {
    let schemaVersion: Int
    let status: String
    let windowID: UInt32
    let requestedDurationMs: Int
    let requestedFrameRate: Int
    let runID: String?
    let gitCommit: String?
    let binarySHA256: String?
    let pid: Int32?
    let displayID: UInt32?
    let refreshRateHz: Double
    let captureScale: Double
    let captureBounds: [String: Double]
    let captureMode: String
    let pixelFormat: String
    let receivedSampleCount: Int
    let accountedSampleCount: Int
    let completeSampleCount: Int
    let copiedCompleteCount: Int
    let encodedCompleteCount: Int
    let incompleteSampleCount: Int
    let incompleteRenderableSampleCount: Int
    let missingDisplayTimeCount: Int
    let droppedCompleteCount: Int
    let duplicateDisplayTimeCount: Int
    let lateFrameCount: Int
    let maximumConsecutiveDisplayTimeGapNs: UInt64
    let maximumAllowedDisplayTimeGapNs: UInt64
    let screenDamageCadenceWithinOneDisplayPeriod: Bool
    let captureHealthPass: Bool
    let startedAt: String
    let finishedAt: String
    let frames: [FrameReceipt]
    let errors: [String]
}

private func sha256(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

/// SCStream owns a bounded IOSurface pool (queueDepth is capped at eight).
/// Retaining those sample buffers until finalization stalls capture after the
/// first eight frames. Clone BGRA bytes into caller-owned CVPixelBuffers so the
/// stream can immediately recycle every delivered surface.
private func clonePixelBuffer(_ source: CVPixelBuffer) -> CVPixelBuffer? {
    let width = CVPixelBufferGetWidth(source)
    let height = CVPixelBufferGetHeight(source)
    let pixelFormat = CVPixelBufferGetPixelFormatType(source)
    var destination: CVPixelBuffer?
    guard CVPixelBufferCreate(
        kCFAllocatorDefault,
        width,
        height,
        pixelFormat,
        nil,
        &destination
    ) == kCVReturnSuccess, let destination else {
        return nil
    }
    CVPixelBufferLockBaseAddress(source, .readOnly)
    CVPixelBufferLockBaseAddress(destination, [])
    defer {
        CVPixelBufferUnlockBaseAddress(destination, [])
        CVPixelBufferUnlockBaseAddress(source, .readOnly)
    }
    guard let sourceBase = CVPixelBufferGetBaseAddress(source),
          let destinationBase = CVPixelBufferGetBaseAddress(destination) else {
        return nil
    }
    let sourceStride = CVPixelBufferGetBytesPerRow(source)
    let destinationStride = CVPixelBufferGetBytesPerRow(destination)
    let copiedBytesPerRow = min(sourceStride, destinationStride)
    for row in 0..<height {
        memcpy(
            destinationBase.advanced(by: row * destinationStride),
            sourceBase.advanced(by: row * sourceStride),
            copiedBytesPerRow
        )
    }
    return destination
}

private final class Capture: NSObject, SCStreamOutput, @unchecked Sendable {
    private struct CapturedSample {
        let displayTime: UInt64
        let pixelBuffer: CVPixelBuffer
        let windowBounds: CGRect?
        let windowAlpha: Double?
        let windowOnscreen: Bool?
        let actualWindowID: CGWindowID?
        let geometrySource: String
    }

    private let lock = NSLock()
    private let outputDirectory: URL
    private var boundWindowID: CGWindowID
    private let captureFrame: CGRect
    private let captureScale: Double
    private let displayStream: Bool
    private let regionBindingPID: pid_t?
    private let excludedWindowIDs: Set<CGWindowID>
    private let ownerClass: String?
    private var captured: [CapturedSample] = []
    private var errors: [String] = []
    private var receivedSampleCount = 0
    private var completeSampleCount = 0
    private var incompleteSampleCount = 0
    private var incompleteRenderableSampleCount = 0
    private var missingDisplayTimeCount = 0
    private var receivedDisplayTimes: [UInt64] = []
    private var bindingAmbiguityReported = false
    private let context = CIContext(options: [.cacheIntermediates: false])

    init(
        outputDirectory: URL,
        windowID: CGWindowID,
        captureFrame: CGRect,
        captureScale: Double,
        displayStream: Bool,
        regionBindingPID: pid_t? = nil,
        excludedWindowIDs: Set<CGWindowID> = [],
        ownerClass: String? = nil
    ) {
        self.outputDirectory = outputDirectory
        self.boundWindowID = windowID
        self.captureFrame = captureFrame
        self.captureScale = captureScale
        self.displayStream = displayStream
        self.regionBindingPID = regionBindingPID
        self.excludedWindowIDs = excludedWindowIDs
        self.ownerClass = ownerClass
    }

    func resolvedWindowID() -> CGWindowID {
        lock.lock()
        defer { lock.unlock() }
        return boundWindowID
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard outputType == .screen else { return }
        let attachments = CMSampleBufferGetSampleAttachmentsArray(
            sampleBuffer,
            createIfNecessary: false
        ) as? [[SCStreamFrameInfo: Any]]
        let displayTime = (attachments?.first?[.displayTime] as? NSNumber)?.uint64Value ?? 0
        lock.lock()
        receivedSampleCount += 1
        if displayTime > 0 {
            receivedDisplayTimes.append(displayTime)
        } else {
            missingDisplayTimeCount += 1
        }
        lock.unlock()
        guard let status = attachments?.first?[.status] as? NSNumber,
              status.intValue == SCFrameStatus.complete.rawValue,
              sampleBuffer.isValid,
              let pixelBuffer = sampleBuffer.imageBuffer else {
            lock.lock()
            incompleteSampleCount += 1
            if sampleBuffer.isValid && sampleBuffer.imageBuffer != nil {
                incompleteRenderableSampleCount += 1
            }
            lock.unlock()
            return
        }
        guard let ownedPixelBuffer = clonePixelBuffer(pixelBuffer) else {
            lock.lock()
            errors.append("complete frame pixel-buffer clone failed")
            lock.unlock()
            return
        }
        let activeWindowID: CGWindowID
        if let regionBindingPID {
            lock.lock()
            let alreadyBound = boundWindowID
            lock.unlock()
            if alreadyBound > 0 {
                activeWindowID = alreadyBound
            } else {
                let candidates = regionOwnerCandidates(
                    pid: regionBindingPID,
                    excludedWindowIDs: excludedWindowIDs,
                    ownerClass: ownerClass,
                    captureFrame: captureFrame
                )
                if candidates.count == 1 {
                    activeWindowID = candidates[0].windowID
                    lock.lock()
                    boundWindowID = activeWindowID
                    lock.unlock()
                } else {
                    activeWindowID = 0
                    if candidates.count > 1 {
                        lock.lock()
                        if !bindingAmbiguityReported {
                            errors.append(
                                "region owner binding ambiguous: \(candidates.map(\.windowID))"
                            )
                            bindingAmbiguityReported = true
                        }
                        lock.unlock()
                    }
                }
            }
        } else {
            activeWindowID = boundWindowID
        }
        let state = activeWindowID > 0
            ? windowState(activeWindowID)
            : (bounds: nil, alpha: nil, onscreen: nil)
        let dirtyRects = (attachments?.first?[.dirtyRects] as? [CGRect])
            ?? (attachments?.first?[.dirtyRects] as? [NSValue])?.map(\.rectValue)
            ?? []
        let dirtyUnion = dirtyRects.reduce(CGRect.null) { partial, rect in
            partial.union(rect)
        }
        var renderedDamageBounds: CGRect?
        if displayStream
            && !dirtyUnion.isNull
            && dirtyUnion.width > 0
            && dirtyUnion.height > 0
        {
            let width = dirtyUnion.width / captureScale
            let height = dirtyUnion.height / captureScale
            renderedDamageBounds = CGRect(
                x: captureFrame.midX - width / 2,
                y: captureFrame.midY - height / 2,
                width: width,
                height: height
            )
        }
        let resolvedBounds = renderedDamageBounds ?? state.bounds
        lock.lock()
        completeSampleCount += 1
        captured.append(CapturedSample(
            displayTime: displayTime,
            pixelBuffer: ownedPixelBuffer,
            windowBounds: resolvedBounds,
            windowAlpha: state.alpha,
            windowOnscreen: state.onscreen,
            actualWindowID: state.bounds == nil ? nil : activeWindowID,
            geometrySource: renderedDamageBounds == nil
                ? "cg-window-inventory"
                : "screen-damage"
        ))
        lock.unlock()
    }

    func finalize(
        maximumAllowedGapNs: UInt64
    ) -> (
        frames: [FrameReceipt],
        errors: [String],
        received: Int,
        complete: Int,
        copied: Int,
        encoded: Int,
        incomplete: Int,
        incompleteRenderable: Int,
        missingDisplayTime: Int,
        dropped: Int,
        duplicates: Int,
        late: Int,
        maximumGapNs: UInt64
    ) {
        lock.lock()
        let samples = captured
        let received = receivedSampleCount
        let complete = completeSampleCount
        let incomplete = incompleteSampleCount
        let incompleteRenderable = incompleteRenderableSampleCount
        let missingDisplayTime = missingDisplayTimeCount
        let allDisplayTimes = receivedDisplayTimes
        let resolvedWindowID = boundWindowID
        var finalizeErrors = errors
        lock.unlock()
        if regionBindingPID != nil && resolvedWindowID == 0 {
            finalizeErrors.append("region capture never bound a unique native owner")
        }

        var frames: [FrameReceipt] = []
        for (sequence, sample) in samples.enumerated() {
            let path = outputDirectory
                .appendingPathComponent(String(format: "frame-%04d.png", sequence))
                .path
            let image = CIImage(cvPixelBuffer: sample.pixelBuffer)
            guard let rendered = context.createCGImage(image, from: image.extent) else {
                finalizeErrors.append("frame \(sequence) render failed")
                continue
            }
            let data = NSMutableData()
            guard let destination = CGImageDestinationCreateWithData(
                data,
                "public.png" as CFString,
                1,
                nil
            ) else {
                finalizeErrors.append("frame \(sequence) PNG destination failed")
                continue
            }
            CGImageDestinationAddImage(destination, rendered, nil)
            guard CGImageDestinationFinalize(destination) else {
                finalizeErrors.append("frame \(sequence) PNG finalization failed")
                continue
            }
            let png = data as Data
            do {
                try png.write(to: URL(fileURLWithPath: path), options: .atomic)
            } catch {
                finalizeErrors.append("frame \(sequence) write failed: \(error)")
                continue
            }
            frames.append(FrameReceipt(
                sequence: sequence,
                displayTime: sample.displayTime,
                displayTimeNs: hostTicksToNs(sample.displayTime),
                windowBounds: sample.windowBounds,
                windowAlpha: sample.windowAlpha,
                windowOnscreen: sample.windowOnscreen,
                expectedWindowID: resolvedWindowID,
                actualWindowID: sample.actualWindowID,
                geometrySource: sample.geometrySource,
                path: path,
                sha256: sha256(png)
            ))
        }
        let displayTimes = allDisplayTimes.sorted()
        let duplicateCount = displayTimes.count - Set(displayTimes).count
        let sampleGaps: [UInt64] = zip(
            displayTimes,
            displayTimes.dropFirst()
        ).map { pair in
            let (previous, current) = pair
            return hostTicksToNs(current) - hostTicksToNs(previous)
        }
        let maximumGap = sampleGaps.max() ?? 0
        let lateCount = sampleGaps.filter { $0 > maximumAllowedGapNs }.count
        return (
            frames,
            finalizeErrors,
            received,
            complete,
            samples.count,
            frames.count,
            incomplete,
            incompleteRenderable,
            missingDisplayTime,
            max(0, complete - samples.count),
            duplicateCount,
            lateCount,
            maximumGap
        )
    }
}

private func resolveWindow(_ arguments: Arguments) async throws -> SCWindow {
    for _ in 0..<100 {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        if let window = content.windows.first(where: { candidate in
            if let id = arguments.windowID {
                return candidate.windowID == id
            }
            guard let pid = arguments.pid else { return false }
            let pidMatches = candidate.owningApplication?.processID == pid
            let titleMatches = arguments.title == nil || candidate.title == arguments.title
            let widthMatches =
                arguments.maxWidth == nil
                || candidate.frame.width <= arguments.maxWidth!
            return pidMatches && titleMatches && widthMatches
        }) {
            return window
        }
        try await Task.sleep(for: .milliseconds(20))
    }
    throw NSError(
        domain: "macos-native-window-filmstrip",
        code: 1,
        userInfo: [
            NSLocalizedDescriptionKey:
                "ScreenCaptureKit window unavailable for id=\(arguments.windowID.map(String.init) ?? "nil") pid=\(arguments.pid.map(String.init) ?? "nil") title=\(arguments.title ?? "nil")"
        ]
    )
}

private func writeReceipt(_ receipt: Receipt, to path: URL) throws {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    try encoder.encode(receipt).write(to: path, options: .atomic)
}

@main
private enum Main {
    static func main() async {
        let arguments = parseArguments()
        guard arguments.windowID != nil || arguments.pid != nil,
              let outputDirectory = arguments.outputDirectory else {
            fputs("usage: macos-native-window-filmstrip (--window-id ID | --pid PID [--title TITLE]) --out DIR [--ready PATH] [--duration-ms N] [--fps N]\n", stderr)
            exit(64)
        }
        let directory = URL(fileURLWithPath: outputDirectory, isDirectory: true)
        try? FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: true
        )
        let startedAt = ISO8601DateFormatter().string(from: Date())
        var stream: SCStream?
        var capture: Capture?
        var errors: [String] = []
        var resolvedWindowID = arguments.windowID ?? 0
        var resolvedDisplayID: CGDirectDisplayID?
        var resolvedCaptureFrame = CGRect.zero
        var resolvedCaptureMode = "unresolved"
        var refreshRateHz = Double(max(1, arguments.frameRate))
        let captureScale = 2.0
        do {
            let windowID: CGWindowID
            let filter: SCContentFilter
            let captureFrame: CGRect
            let captureMode: String
            if arguments.displayStream {
                let pinnedWindowID: CGWindowID
                let pinnedBounds: CGRect
                if arguments.regionArmed {
                    guard let requestedBounds = arguments.pinnedBounds,
                          arguments.pid != nil,
                          arguments.ownerClass != nil
                    else {
                        throw NSError(
                            domain: "macos-native-window-filmstrip",
                            code: 5,
                            userInfo: [
                                NSLocalizedDescriptionKey:
                                    "region-armed capture requires --pid, --owner-class, and --bounds"
                            ]
                        )
                    }
                    pinnedWindowID = 0
                    pinnedBounds = requestedBounds
                } else if let requestedWindowID = arguments.windowID {
                    guard let retainedBounds =
                        arguments.pinnedBounds ?? windowState(requestedWindowID).bounds
                    else {
                        throw NSError(
                            domain: "macos-native-window-filmstrip",
                            code: 3,
                            userInfo: [
                                NSLocalizedDescriptionKey:
                                    "pinned window \(requestedWindowID) has no retained bounds"
                            ]
                        )
                    }
                    pinnedWindowID = requestedWindowID
                    pinnedBounds = retainedBounds
                } else {
                    let resolvedWindow = try await resolveWindow(arguments)
                    pinnedWindowID = resolvedWindow.windowID
                    pinnedBounds = resolvedWindow.frame
                }
                let content = try await SCShareableContent.excludingDesktopWindows(
                    false,
                    onScreenWindowsOnly: false
                )
                let center = CGPoint(x: pinnedBounds.midX, y: pinnedBounds.midY)
                guard let display = content.displays.first(where: {
                    $0.frame.contains(center)
                }) else {
                    throw NSError(
                        domain: "macos-native-window-filmstrip",
                        code: 4,
                        userInfo: [
                            NSLocalizedDescriptionKey:
                                "no ScreenCaptureKit display contains pinned bounds \(pinnedBounds)"
                        ]
                    )
                }
                windowID = pinnedWindowID
                captureFrame = pinnedBounds
                captureMode = arguments.regionArmed
                    ? "display-region-armed-before-owner"
                    : "display-pinned-window-bounds"
                resolvedDisplayID = display.displayID
                filter = SCContentFilter(display: display, excludingWindows: [])
            } else {
                let window = try await resolveWindow(arguments)
                windowID = window.windowID
                captureFrame = window.frame
                captureMode = "desktop-independent-window"
                let content = try await SCShareableContent.excludingDesktopWindows(
                    false,
                    onScreenWindowsOnly: false
                )
                let center = CGPoint(x: captureFrame.midX, y: captureFrame.midY)
                resolvedDisplayID = content.displays.first(where: {
                    $0.frame.contains(center)
                })?.displayID
                filter = SCContentFilter(desktopIndependentWindow: window)
            }
            resolvedCaptureFrame = captureFrame
            resolvedCaptureMode = captureMode
            if let displayID = resolvedDisplayID {
                refreshRateHz = displayRefreshRate(
                    displayID,
                    fallback: refreshRateHz
                )
            }
            resolvedWindowID = windowID
            let receiver = Capture(
                outputDirectory: directory,
                windowID: windowID,
                captureFrame: captureFrame,
                captureScale: captureScale,
                displayStream: arguments.displayStream,
                regionBindingPID: arguments.regionArmed ? arguments.pid : nil,
                excludedWindowIDs: Set(arguments.excludedWindowIDs),
                ownerClass: arguments.ownerClass
            )
            let configuration = SCStreamConfiguration()
            configuration.width = max(1, Int(captureFrame.width * captureScale))
            configuration.height = max(1, Int(captureFrame.height * captureScale))
            configuration.showsCursor = false
            configuration.queueDepth = 8
            configuration.pixelFormat = kCVPixelFormatType_32BGRA
            if arguments.displayStream {
                let displayFrame = filter.contentRect
                configuration.sourceRect = CGRect(
                    x: captureFrame.minX - displayFrame.minX,
                    y: captureFrame.minY - displayFrame.minY,
                    width: captureFrame.width,
                    height: captureFrame.height
                )
            }
            configuration.minimumFrameInterval = CMTime(
                value: 1,
                timescale: CMTimeScale(max(1, arguments.frameRate))
            )
            let createdStream = SCStream(
                filter: filter,
                configuration: configuration,
                delegate: nil
            )
            try createdStream.addStreamOutput(
                receiver,
                type: .screen,
                sampleHandlerQueue: DispatchQueue(
                    label: "script-kit.native-window-filmstrip",
                    qos: .userInitiated
                )
            )
            try await createdStream.startCapture()
            stream = createdStream
            capture = receiver
            if let readyPath = arguments.readyPath {
                let ready = [
                    "windowID": Int(windowID),
                    "pid": arguments.pid.map(Int.init) as Any,
                    "displayID": resolvedDisplayID.map(Int.init) as Any,
                    "refreshRateHz": refreshRateHz,
                    "captureScale": captureScale,
                    "startedAt": ISO8601DateFormatter().string(from: Date()),
                    "captureMode": captureMode,
                    "pixelFormat": "BGRA",
                    "captureBounds": [
                        "x": captureFrame.minX,
                        "y": captureFrame.minY,
                        "width": captureFrame.width,
                        "height": captureFrame.height,
                    ],
                ] as [String: Any]
                let data = try JSONSerialization.data(
                    withJSONObject: ready,
                    options: [.prettyPrinted, .sortedKeys]
                )
                try data.write(to: URL(fileURLWithPath: readyPath), options: .atomic)
            }
            try await Task.sleep(for: .milliseconds(arguments.durationMs))
            try await createdStream.stopCapture()
            resolvedWindowID = receiver.resolvedWindowID()
        } catch {
            errors.append(error.localizedDescription)
            if let stream {
                try? await stream.stopCapture()
            }
        }
        let maximumAllowedGapNs = UInt64(
            (1_000_000_000.0 / max(1.0, refreshRateHz)) + 1_000_000.0
        )
        let finalized = capture?.finalize(
            maximumAllowedGapNs: maximumAllowedGapNs
        ) ?? (
            frames: [],
            errors: [],
            received: 0,
            complete: 0,
            copied: 0,
            encoded: 0,
            incomplete: 0,
            incompleteRenderable: 0,
            missingDisplayTime: 0,
            dropped: 0,
            duplicates: 0,
            late: 0,
            maximumGapNs: 0
        )
        errors.append(contentsOf: finalized.errors)
        if finalized.frames.isEmpty {
            errors.append("ScreenCaptureKit produced no complete frames")
        }
        let firstOwnedFrame = finalized.frames.firstIndex {
            $0.actualWindowID == resolvedWindowID
        }
        let frameIdentitiesExact = finalized.frames.enumerated().allSatisfy {
            index, frame in
            guard frame.expectedWindowID == resolvedWindowID else { return false }
            if frame.actualWindowID == resolvedWindowID { return true }
            guard firstOwnedFrame != nil else { return false }
            // A pinned window stream remains the same stream while the
            // WindowServer inventory legitimately has no row before entry or
            // after orderOut. `nil` means absent, never a substituted ID.
            return frame.actualWindowID == nil && frame.windowBounds == nil
        }
        if !frameIdentitiesExact {
            errors.append("one or more frames changed exact CGWindowID")
        }
        if finalized.received != finalized.complete + finalized.incomplete {
            errors.append(
                "sample accounting mismatch received=\(finalized.received) complete=\(finalized.complete) incomplete=\(finalized.incomplete)"
            )
        }
        if finalized.missingDisplayTime > 0 {
            errors.append(
                "capture contains \(finalized.missingDisplayTime) samples without display time"
            )
        }
        if finalized.complete != finalized.copied
            || finalized.copied != finalized.encoded
            || finalized.dropped > 0 {
            errors.append(
                "capture accounting mismatch complete=\(finalized.complete) copied=\(finalized.copied) encoded=\(finalized.encoded) dropped=\(finalized.dropped)"
            )
        }
        if finalized.duplicates > 0 {
            errors.append(
                "capture contains \(finalized.duplicates) duplicate display times"
            )
        }
        let captureHealthPass = errors.isEmpty
        let receipt = Receipt(
            schemaVersion: 2,
            status: captureHealthPass ? "ok" : "invalid",
            windowID: resolvedWindowID,
            requestedDurationMs: arguments.durationMs,
            requestedFrameRate: arguments.frameRate,
            runID: arguments.runID,
            gitCommit: arguments.gitCommit,
            binarySHA256: arguments.binarySHA256,
            pid: arguments.pid,
            displayID: resolvedDisplayID,
            refreshRateHz: refreshRateHz,
            captureScale: captureScale,
            captureBounds: [
                "x": resolvedCaptureFrame.minX,
                "y": resolvedCaptureFrame.minY,
                "width": resolvedCaptureFrame.width,
                "height": resolvedCaptureFrame.height,
            ],
            captureMode: resolvedCaptureMode,
            pixelFormat: "BGRA",
            receivedSampleCount: finalized.received,
            accountedSampleCount: finalized.complete + finalized.incomplete,
            completeSampleCount: finalized.complete,
            copiedCompleteCount: finalized.copied,
            encodedCompleteCount: finalized.encoded,
            incompleteSampleCount: finalized.incomplete,
            incompleteRenderableSampleCount: finalized.incompleteRenderable,
            missingDisplayTimeCount: finalized.missingDisplayTime,
            droppedCompleteCount: finalized.dropped,
            duplicateDisplayTimeCount: finalized.duplicates,
            lateFrameCount: finalized.late,
            maximumConsecutiveDisplayTimeGapNs: finalized.maximumGapNs,
            maximumAllowedDisplayTimeGapNs: maximumAllowedGapNs,
            screenDamageCadenceWithinOneDisplayPeriod: finalized.late == 0,
            captureHealthPass: captureHealthPass,
            startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            frames: finalized.frames,
            errors: errors
        )
        do {
            try writeReceipt(receipt, to: directory.appendingPathComponent("receipt.json"))
            let data = try JSONEncoder().encode(receipt)
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data("\n".utf8))
        } catch {
            fputs("receipt write failed: \(error.localizedDescription)\n", stderr)
            exit(1)
        }
        exit(captureHealthPass ? 0 : 1)
    }
}
