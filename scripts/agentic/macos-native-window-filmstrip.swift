import CoreImage
import CoreMedia
import CoreVideo
import Foundation
import ImageIO
import ScreenCaptureKit
import AppKit

private struct Arguments {
    var windowID: CGWindowID?
    var pid: pid_t?
    var title: String?
    var outputDirectory: String?
    var readyPath: String?
    var durationMs = 250
    var frameRate = 120
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

private func windowState(_ windowID: CGWindowID) -> (
    bounds: CGRect?,
    alpha: Double?,
    onscreen: Bool?
) {
    guard let info = CGWindowListCopyWindowInfo(
        [.optionIncludingWindow, .excludeDesktopElements],
        windowID
    ) as? [[String: Any]], let row = info.first else {
        return (nil, nil, nil)
    }
    let bounds = (row[kCGWindowBounds as String] as? NSDictionary)
        .flatMap { CGRect(dictionaryRepresentation: $0) }
    let alpha = (row[kCGWindowAlpha as String] as? NSNumber)?.doubleValue
    let onscreen = (row[kCGWindowIsOnscreen as String] as? NSNumber)?.boolValue
    return (bounds, alpha, onscreen)
}

private struct Receipt: Codable {
    let schemaVersion: Int
    let status: String
    let windowID: UInt32
    let requestedDurationMs: Int
    let requestedFrameRate: Int
    let startedAt: String
    let finishedAt: String
    let frames: [FrameReceipt]
    let errors: [String]
}

private func sha256(_ path: String) -> String {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/shasum")
    process.arguments = ["-a", "256", path]
    let pipe = Pipe()
    process.standardOutput = pipe
    try? process.run()
    process.waitUntilExit()
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    return String(data: data, encoding: .utf8)?.split(separator: " ").first.map(String.init) ?? ""
}

private final class Capture: NSObject, SCStreamOutput, @unchecked Sendable {
    private let lock = NSLock()
    private let outputDirectory: URL
    private let windowID: CGWindowID
    private var receipts: [FrameReceipt] = []
    private var errors: [String] = []
    private let context = CIContext(options: [.cacheIntermediates: false])

    init(outputDirectory: URL, windowID: CGWindowID) {
        self.outputDirectory = outputDirectory
        self.windowID = windowID
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard outputType == .screen, sampleBuffer.isValid,
              let pixelBuffer = sampleBuffer.imageBuffer else { return }
        let attachments = CMSampleBufferGetSampleAttachmentsArray(
            sampleBuffer,
            createIfNecessary: false
        ) as? [[SCStreamFrameInfo: Any]]
        guard let status = attachments?.first?[.status] as? NSNumber,
              status.intValue == SCFrameStatus.complete.rawValue else { return }
        let displayTime = (attachments?.first?[.displayTime] as? NSNumber)?.uint64Value ?? 0
        let state = windowState(windowID)
        lock.lock()
        let sequence = receipts.count
        lock.unlock()
        let path = outputDirectory.appendingPathComponent(String(format: "frame-%04d.png", sequence)).path
        let image = CIImage(cvPixelBuffer: pixelBuffer)
        guard let rendered = context.createCGImage(image, from: image.extent),
              let destination = CGImageDestinationCreateWithURL(
                URL(fileURLWithPath: path) as CFURL,
                "public.png" as CFString,
                1,
                nil
              ) else {
            lock.lock()
            errors.append("frame \(sequence) render failed")
            lock.unlock()
            return
        }
        CGImageDestinationAddImage(destination, rendered, nil)
        guard CGImageDestinationFinalize(destination) else {
            lock.lock()
            errors.append("frame \(sequence) PNG finalization failed")
            lock.unlock()
            return
        }
        let receipt = FrameReceipt(
            sequence: sequence,
            displayTime: displayTime,
            displayTimeNs: hostTicksToNs(displayTime),
            windowBounds: state.bounds,
            windowAlpha: state.alpha,
            windowOnscreen: state.onscreen,
            path: path,
            sha256: sha256(path)
        )
        lock.lock()
        receipts.append(receipt)
        lock.unlock()
    }

    func snapshot() -> (frames: [FrameReceipt], errors: [String]) {
        lock.lock()
        defer { lock.unlock() }
        return (receipts, errors)
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
            return pidMatches && titleMatches
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
        do {
            let window = try await resolveWindow(arguments)
            resolvedWindowID = window.windowID
            let receiver = Capture(outputDirectory: directory, windowID: window.windowID)
            let configuration = SCStreamConfiguration()
            configuration.width = max(1, Int(window.frame.width * 2))
            configuration.height = max(1, Int(window.frame.height * 2))
            configuration.showsCursor = false
            configuration.queueDepth = 8
            configuration.minimumFrameInterval = CMTime(
                value: 1,
                timescale: CMTimeScale(max(1, arguments.frameRate))
            )
            let createdStream = SCStream(
                filter: SCContentFilter(desktopIndependentWindow: window),
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
                    "windowID": Int(window.windowID),
                    "startedAt": ISO8601DateFormatter().string(from: Date()),
                ] as [String: Any]
                let data = try JSONSerialization.data(
                    withJSONObject: ready,
                    options: [.prettyPrinted, .sortedKeys]
                )
                try data.write(to: URL(fileURLWithPath: readyPath), options: .atomic)
            }
            try await Task.sleep(for: .milliseconds(arguments.durationMs))
            try await createdStream.stopCapture()
        } catch {
            errors.append(error.localizedDescription)
            if let stream {
                try? await stream.stopCapture()
            }
        }
        let snapshot = capture?.snapshot() ?? (frames: [], errors: [])
        errors.append(contentsOf: snapshot.errors)
        if snapshot.frames.isEmpty {
            errors.append("ScreenCaptureKit produced no complete frames")
        }
        let receipt = Receipt(
            schemaVersion: 1,
            status: errors.isEmpty ? "ok" : "invalid",
            windowID: resolvedWindowID,
            requestedDurationMs: arguments.durationMs,
            requestedFrameRate: arguments.frameRate,
            startedAt: startedAt,
            finishedAt: ISO8601DateFormatter().string(from: Date()),
            frames: snapshot.frames,
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
        exit(errors.isEmpty ? 0 : 1)
    }
}
