#!/usr/bin/env swift

import AppKit
import CryptoKit
import Foundation

// Fail-closed fixture modes (glass-smoke-harness-max-info WP9): an unknown
// mode prints a bounded error and exits nonzero. A substitute mode is NEVER
// drawn — the old stringly default silently treated unknown modes as
// dark-terminal, which could certify a study against the wrong backdrop.
private enum FixtureMode: String, CaseIterable {
    case saturatedStripes = "saturated-stripes"
    case darkTerminal = "dark-terminal"
    case lightDocument = "light-document"
    case materialMatched = "material-matched"
}

private struct Arguments {
    var mode = FixtureMode.darkTerminal
    var receipt: String?
}

private func parseArguments() -> Arguments {
    var result = Arguments()
    let values = Array(CommandLine.arguments.dropFirst())
    var index = 0
    while index < values.count {
        switch values[index] {
        case "--mode":
            if index + 1 < values.count {
                guard let mode = FixtureMode(rawValue: values[index + 1]) else {
                    fputs(
                        "unknown fixture mode \"\(values[index + 1])\" — known modes: "
                            + FixtureMode.allCases.map(\.rawValue).joined(separator: ", ")
                            + "\n",
                        stderr
                    )
                    exit(64)
                }
                result.mode = mode
                index += 1
            }
        case "--receipt":
            if index + 1 < values.count {
                result.receipt = values[index + 1]
                index += 1
            }
        default:
            break
        }
        index += 1
    }
    return result
}

private final class PatternView: NSView {
    let mode: FixtureMode

    init(frame: NSRect, mode: FixtureMode) {
        self.mode = mode
        super.init(frame: frame)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        // Exhaustive over FixtureMode — no default arm, so a new mode cannot
        // silently reuse another mode's drawing.
        switch mode {
        case .lightDocument:
            NSColor(calibratedWhite: 0.94, alpha: 1).setFill()
            bounds.fill()
            NSColor(calibratedWhite: 0.80, alpha: 1).setFill()
            for row in 0..<24 {
                NSRect(
                    x: bounds.minX + 72,
                    y: bounds.maxY - 90 - CGFloat(row * 28),
                    width: max(160, bounds.width - 144),
                    height: 2
                ).fill()
            }
        case .materialMatched:
            NSColor(calibratedRed: 0.105, green: 0.105, blue: 0.125, alpha: 1).setFill()
            bounds.fill()
        case .saturatedStripes:
            let colors: [NSColor] = [
                .systemPink, .systemPurple, .systemBlue, .systemTeal,
                .systemGreen, .systemYellow, .systemOrange, .systemRed,
            ]
            let width = max(1, bounds.width / CGFloat(colors.count))
            for (index, color) in colors.enumerated() {
                color.setFill()
                NSRect(
                    x: bounds.minX + CGFloat(index) * width,
                    y: bounds.minY,
                    width: width + 1,
                    height: bounds.height
                ).fill()
            }
        case .darkTerminal:
            NSColor(calibratedWhite: 0.035, alpha: 1).setFill()
            bounds.fill()
            NSColor(calibratedWhite: 0.18, alpha: 1).setFill()
            for row in 0..<30 {
                let width = bounds.width * CGFloat(0.35 + Double((row * 17) % 55) / 100.0)
                NSRect(
                    x: bounds.minX + 42,
                    y: bounds.maxY - 64 - CGFloat(row * 22),
                    width: width,
                    height: 2
                ).fill()
            }
        }
    }
}

private struct VisualDiagnostics {
    let visualSha256: String
    let meanLuminance: Double
    let luminanceRange: Double
    let maximumSaturation: Double
    let distinctHueBucketCount: Int
    let pass: Bool
    let reasons: [String]

    var json: [String: Any] {
        [
            "visualSha256": visualSha256,
            "meanLuminance": meanLuminance,
            "luminanceRange": luminanceRange,
            "maximumSaturation": maximumSaturation,
            "distinctHueBucketCount": distinctHueBucketCount,
            "pass": pass,
            "reasons": reasons,
        ]
    }
}

/// Render the fixture's OWN view to a bitmap and validate mode-specific
/// invariants, so a structurally correct receipt cannot certify a blank or
/// wrong fixture. This is prompt-free (no screen-capture TCC): it proves the
/// drawn content; on-screen composited verification stays with the probes'
/// background-reference captures.
private func validateVisual(view: NSView, mode: FixtureMode) -> VisualDiagnostics {
    guard let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
        return VisualDiagnostics(
            visualSha256: "",
            meanLuminance: -1,
            luminanceRange: -1,
            maximumSaturation: -1,
            distinctHueBucketCount: 0,
            pass: false,
            reasons: ["could not allocate validation bitmap"]
        )
    }
    view.cacheDisplay(in: view.bounds, to: bitmap)
    guard let data = bitmap.tiffRepresentation else {
        return VisualDiagnostics(
            visualSha256: "",
            meanLuminance: -1,
            luminanceRange: -1,
            maximumSaturation: -1,
            distinctHueBucketCount: 0,
            pass: false,
            reasons: ["could not encode validation bitmap"]
        )
    }
    let sha = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()

    // Sample a coarse grid of pixels for cheap, deterministic diagnostics.
    let columns = 32
    let rows = 18
    var luminances: [Double] = []
    var maxSaturation = 0.0
    var hueBuckets = Set<Int>()
    for row in 0..<rows {
        for column in 0..<columns {
            let x = Int(
                (Double(column) + 0.5) / Double(columns) * Double(bitmap.pixelsWide)
            )
            let y = Int(
                (Double(row) + 0.5) / Double(rows) * Double(bitmap.pixelsHigh)
            )
            guard let color = bitmap.colorAt(x: x, y: y)?
                .usingColorSpace(.deviceRGB) else { continue }
            let r = Double(color.redComponent)
            let g = Double(color.greenComponent)
            let b = Double(color.blueComponent)
            luminances.append(0.2126 * r + 0.7152 * g + 0.0722 * b)
            let maxC = max(r, g, b)
            let minC = min(r, g, b)
            let saturation = maxC == 0 ? 0 : (maxC - minC) / maxC
            maxSaturation = max(maxSaturation, saturation)
            if saturation > 0.35 {
                var hue: CGFloat = 0
                var s: CGFloat = 0
                var brightness: CGFloat = 0
                var alpha: CGFloat = 0
                color.getHue(&hue, saturation: &s, brightness: &brightness, alpha: &alpha)
                hueBuckets.insert(Int((hue * 12).rounded()) % 12)
            }
        }
    }
    let mean = luminances.isEmpty
        ? -1 : luminances.reduce(0, +) / Double(luminances.count)
    let range = luminances.isEmpty
        ? -1 : (luminances.max()! - luminances.min()!)

    var reasons: [String] = []
    switch mode {
    case .saturatedStripes:
        if hueBuckets.count < 5 {
            reasons.append(
                "expected >= 5 distinct saturated hue buckets, found \(hueBuckets.count)"
            )
        }
        if maxSaturation < 0.5 {
            reasons.append("expected saturated stripes, max saturation \(maxSaturation)")
        }
    case .darkTerminal:
        if mean < 0 || mean > 0.15 {
            reasons.append("expected dark background, mean luminance \(mean)")
        }
        if maxSaturation > 0.2 {
            reasons.append("expected achromatic terminal, max saturation \(maxSaturation)")
        }
    case .lightDocument:
        if mean < 0.75 {
            reasons.append("expected light document, mean luminance \(mean)")
        }
    case .materialMatched:
        if range < 0 || range > 0.05 {
            reasons.append("expected uniform low-contrast field, luminance range \(range)")
        }
        // Device-profile conversion shifts the calibrated 0.105/0.105/0.125
        // fill noticeably (measured 0.191 on the reference display); the
        // load-bearing invariants are uniformity + not-light, so the mean
        // gate stays deliberately wide.
        if mean < 0.05 || mean > 0.3 {
            reasons.append("expected material-matched luminance, mean \(mean)")
        }
    }
    if luminances.isEmpty {
        reasons.append("no pixels sampled")
    }
    return VisualDiagnostics(
        visualSha256: sha,
        meanLuminance: mean,
        luminanceRange: range,
        maximumSaturation: maxSaturation,
        distinctHueBucketCount: hueBuckets.count,
        pass: reasons.isEmpty,
        reasons: reasons
    )
}

private final class FixtureDelegate: NSObject, NSApplicationDelegate {
    let arguments: Arguments
    var window: NSWindow?

    init(arguments: Arguments) {
        self.arguments = arguments
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let screen = NSScreen.main else {
            fputs("No main display\n", stderr)
            NSApp.terminate(nil)
            return
        }
        let window = NSWindow(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false,
            screen: screen
        )
        // Script Kit's PopUp owners render at level 101. Keep this fixture one
        // level below the product so it is visible through the glass while
        // remaining above normal applications such as terminals and editors.
        let backingLevel = NSWindow.Level(
            rawValue: NSWindow.Level.popUpMenu.rawValue - 1
        )
        window.level = backingLevel
        window.collectionBehavior = [.canJoinAllSpaces, .stationary]
        window.ignoresMouseEvents = true
        window.isOpaque = true
        window.backgroundColor = .black
        let pattern = PatternView(frame: screen.frame, mode: arguments.mode)
        window.contentView = pattern
        window.orderFrontRegardless()
        self.window = window

        // Validate the fixture's own rendered content BEFORE declaring ready.
        let diagnostics = validateVisual(view: pattern, mode: arguments.mode)

        let palettes: [FixtureMode: [String]] = [
            .darkTerminal: ["white:0.035", "white:0.18"],
            .lightDocument: ["white:0.94", "white:0.80"],
            .materialMatched: ["rgb:0.105,0.105,0.125"],
            .saturatedStripes: [
                "systemPink", "systemPurple", "systemBlue", "systemTeal",
                "systemGreen", "systemYellow", "systemOrange", "systemRed",
            ],
        ]
        let configuration: [String: Any] = [
            "mode": arguments.mode.rawValue,
            "displayID": screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32 ?? 0,
            "backingScale": screen.backingScaleFactor,
            "frame": [
                "x": screen.frame.origin.x,
                "y": screen.frame.origin.y,
                "width": screen.frame.width,
                "height": screen.frame.height,
            ],
            "palette": palettes[arguments.mode]!,
        ]
        let configurationData = try! JSONSerialization.data(
            withJSONObject: configuration,
            options: [.sortedKeys]
        )
        let configurationSHA256 = SHA256.hash(data: configurationData)
            .map { String(format: "%02x", $0) }
            .joined()
        let receipt: [String: Any] = [
            "schemaVersion": 2,
            "status": diagnostics.pass ? "ready" : "invalid-visual",
            "pid": ProcessInfo.processInfo.processIdentifier,
            "mode": arguments.mode.rawValue,
            "windowNumber": window.windowNumber,
            "displayID": screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32 ?? 0,
            "backingScale": screen.backingScaleFactor,
            "ignoresMouseEvents": window.ignoresMouseEvents,
            "windowLevel": window.level.rawValue,
            "orderingContract": "one-level-below-popup-owner",
            "configuration": configuration,
            "configurationSha256": configurationSHA256,
            "visualDiagnostics": diagnostics.json,
            "frame": [
                "x": screen.frame.origin.x,
                "y": screen.frame.origin.y,
                "width": screen.frame.width,
                "height": screen.frame.height,
            ],
            "startedAt": ISO8601DateFormatter().string(from: Date()),
        ]
        let data = try! JSONSerialization.data(withJSONObject: receipt, options: [.prettyPrinted, .sortedKeys])
        if let path = arguments.receipt {
            try! data.write(to: URL(fileURLWithPath: path), options: .atomic)
        }
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data("\n".utf8))
        if !diagnostics.pass {
            fputs(
                "fixture visual validation failed: "
                    + diagnostics.reasons.joined(separator: "; ") + "\n",
                stderr
            )
            exit(70)
        }
    }
}

private let arguments = parseArguments()
private let application = NSApplication.shared
application.setActivationPolicy(.accessory)
private let delegate = FixtureDelegate(arguments: arguments)
application.delegate = delegate
application.run()
