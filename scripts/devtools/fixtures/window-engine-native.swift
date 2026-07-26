// Window Engine native fixture.
//
// Creates deterministic PUBLIC AppKit windows for live AX verification:
// ordinary, minimum-size-constrained, titleless panel (dialog), sheet,
// duplicate-title pair, and a native tab group. Slow/clamp/destroy behaviors
// stay in the deterministic JSON provider — this fixture only proves real
// AX behavior against real windows.
//
// Compiled on demand by scripts/devtools/window-engine-foundation.ts via
// `xcrun swiftc`; never commit a binary. The process answers on stdin:
// "list" prints a JSON description of its windows; "quit" exits.

import AppKit
import Foundation

final class FixtureDelegate: NSObject, NSApplicationDelegate {
    var windows: [String: NSWindow] = [:]

    func applicationDidFinishLaunching(_ notification: Notification) {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)

        func makeWindow(
            key: String,
            title: String,
            frame: NSRect,
            style: NSWindow.StyleMask = [.titled, .closable, .miniaturizable, .resizable]
        ) -> NSWindow {
            let window = NSWindow(
                contentRect: frame,
                styleMask: style,
                backing: .buffered,
                defer: false
            )
            window.title = title
            window.isReleasedWhenClosed = false
            window.orderFrontRegardless()
            windows[key] = window
            return window
        }

        // Ordinary window.
        _ = makeWindow(
            key: "ordinary",
            title: "SK Native Fixture Ordinary",
            frame: NSRect(x: 80, y: 120, width: 900, height: 600)
        )

        // Minimum-size-constrained window.
        let constrained = makeWindow(
            key: "constrained",
            title: "SK Native Fixture Constrained",
            frame: NSRect(x: 1020, y: 120, width: 640, height: 480)
        )
        constrained.minSize = NSSize(width: 500, height: 400)

        // Titleless utility panel (observed as an untitled dialog-ish row).
        let panel = NSPanel(
            contentRect: NSRect(x: 300, y: 400, width: 420, height: 260),
            styleMask: [.titled, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        panel.title = ""
        panel.isReleasedWhenClosed = false
        panel.orderFrontRegardless()
        windows["panel"] = panel

        // Sheet attached to the ordinary window.
        if let host = windows["ordinary"] {
            let sheet = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 480, height: 220),
                styleMask: [.titled],
                backing: .buffered,
                defer: false
            )
            sheet.title = ""
            sheet.isReleasedWhenClosed = false
            windows["sheet"] = sheet
            host.beginSheet(sheet, completionHandler: nil)
        }

        // Duplicate-title pair.
        _ = makeWindow(
            key: "twin-a",
            title: "SK Native Fixture Twin",
            frame: NSRect(x: 120, y: 760, width: 500, height: 300)
        )
        _ = makeWindow(
            key: "twin-b",
            title: "SK Native Fixture Twin",
            frame: NSRect(x: 120, y: 760, width: 500, height: 300)
        )

        // Native tab group.
        let tabHost = makeWindow(
            key: "tab-one",
            title: "SK Native Fixture Tab One",
            frame: NSRect(x: 760, y: 760, width: 800, height: 500)
        )
        tabHost.tabbingMode = .preferred
        let tabTwo = makeWindow(
            key: "tab-two",
            title: "SK Native Fixture Tab Two",
            frame: NSRect(x: 760, y: 760, width: 800, height: 500)
        )
        tabTwo.tabbingMode = .preferred
        tabHost.addTabbedWindow(tabTwo, ordered: .above)

        FileHandle.standardOutput.write("READY\n".data(using: .utf8)!)

        DispatchQueue.global().async { [weak self] in
            while let line = readLine(strippingNewline: true) {
                switch line {
                case "list":
                    DispatchQueue.main.sync {
                        self?.printWindows()
                    }
                case "quit":
                    DispatchQueue.main.sync {
                        NSApplication.shared.terminate(nil)
                    }
                default:
                    FileHandle.standardOutput.write(
                        "UNKNOWN \(line)\n".data(using: .utf8)!)
                }
            }
        }
    }

    private func printWindows() {
        var rows: [[String: Any]] = []
        for (key, window) in windows {
            rows.append([
                "key": key,
                "title": window.title,
                "windowNumber": window.windowNumber,
                "x": Int(window.frame.origin.x),
                "y": Int(window.frame.origin.y),
                "width": Int(window.frame.size.width),
                "height": Int(window.frame.size.height),
                "isSheet": window.isSheet,
                "tabCount": window.tabbedWindows?.count ?? 0,
            ])
        }
        if let data = try? JSONSerialization.data(withJSONObject: rows),
           let json = String(data: data, encoding: .utf8) {
            FileHandle.standardOutput.write((json + "\n").data(using: .utf8)!)
        }
    }
}

let delegate = FixtureDelegate()
let app = NSApplication.shared
app.delegate = delegate
app.run()
