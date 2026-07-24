#!/usr/bin/env swift

import AppKit
import CryptoKit
import Foundation

private struct Arguments {
    var mode = "dark-terminal"
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
                result.mode = values[index + 1]
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
    let mode: String

    init(frame: NSRect, mode: String) {
        self.mode = mode
        super.init(frame: frame)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        switch mode {
        case "light-document":
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
        case "material-matched":
            NSColor(calibratedRed: 0.105, green: 0.105, blue: 0.125, alpha: 1).setFill()
            bounds.fill()
        case "saturated-stripes":
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
        default:
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
        window.level = .normal
        window.collectionBehavior = [.canJoinAllSpaces, .stationary]
        window.ignoresMouseEvents = true
        window.isOpaque = true
        window.backgroundColor = .black
        window.contentView = PatternView(frame: screen.frame, mode: arguments.mode)
        window.orderFrontRegardless()
        self.window = window

        let configuration: [String: Any] = [
            "mode": arguments.mode,
            "displayID": screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32 ?? 0,
            "backingScale": screen.backingScaleFactor,
            "frame": [
                "x": screen.frame.origin.x,
                "y": screen.frame.origin.y,
                "width": screen.frame.width,
                "height": screen.frame.height,
            ],
            "palette": [
                "dark-terminal": ["white:0.035", "white:0.18"],
                "light-document": ["white:0.94", "white:0.80"],
                "material-matched": ["rgb:0.105,0.105,0.125"],
                "saturated-stripes": [
                    "systemPink", "systemPurple", "systemBlue", "systemTeal",
                    "systemGreen", "systemYellow", "systemOrange", "systemRed",
                ],
            ][arguments.mode] ?? ["white:0.035", "white:0.18"],
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
            "status": "ready",
            "pid": ProcessInfo.processInfo.processIdentifier,
            "mode": arguments.mode,
            "windowNumber": window.windowNumber,
            "displayID": screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? UInt32 ?? 0,
            "backingScale": screen.backingScaleFactor,
            "ignoresMouseEvents": window.ignoresMouseEvents,
            "configuration": configuration,
            "configurationSha256": configurationSHA256,
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
    }
}

private let arguments = parseArguments()
private let application = NSApplication.shared
application.setActivationPolicy(.accessory)
private let delegate = FixtureDelegate(arguments: arguments)
application.delegate = delegate
application.run()
