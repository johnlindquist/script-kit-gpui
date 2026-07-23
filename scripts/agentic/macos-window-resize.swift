#!/usr/bin/env swift

import ApplicationServices
import Foundation

private func argument(_ name: String) -> String? {
    guard let index = CommandLine.arguments.firstIndex(of: name),
          index + 1 < CommandLine.arguments.count else { return nil }
    return CommandLine.arguments[index + 1]
}

guard let pidText = argument("--pid"),
      let pid = pid_t(pidText),
      let windowIDText = argument("--window-id"),
      let requestedWindowID = UInt32(windowIDText),
      let widthText = argument("--width"),
      let width = Double(widthText),
      let heightText = argument("--height"),
      let height = Double(heightText) else {
    fputs("usage: macos-window-resize --pid PID --window-id ID --width W --height H\n", stderr)
    exit(64)
}

let application = AXUIElementCreateApplication(pid)
var windowsValue: CFTypeRef?
let copyResult = AXUIElementCopyAttributeValue(
    application,
    kAXWindowsAttribute as CFString,
    &windowsValue
)
@_silgen_name("_AXUIElementGetWindow")
private func privateAXWindowNumber(
    _ element: AXUIElement,
    _ windowID: UnsafeMutablePointer<CGWindowID>
) -> AXError

guard copyResult == .success,
      let windows = windowsValue as? [AXUIElement],
      let window = windows.first(where: { candidate in
          var observed = CGWindowID(0)
          return privateAXWindowNumber(candidate, &observed) == .success
              && observed == requestedWindowID
      }) else {
    fputs("AX window unavailable for pid \(pid), error=\(copyResult.rawValue)\n", stderr)
    exit(1)
}

var requested = CGSize(width: width, height: height)
guard let requestedValue = AXValueCreate(.cgSize, &requested) else {
    fputs("failed to create AX size value\n", stderr)
    exit(1)
}
let setResult = AXUIElementSetAttributeValue(
    window,
    kAXSizeAttribute as CFString,
    requestedValue
)
guard setResult == .success else {
    fputs("AX size update failed, error=\(setResult.rawValue)\n", stderr)
    exit(1)
}

var observedValue: CFTypeRef?
let verifyResult = AXUIElementCopyAttributeValue(
    window,
    kAXSizeAttribute as CFString,
    &observedValue
)
var observed = CGSize.zero
if verifyResult == .success, let observedValue {
    let value = observedValue as! AXValue
    AXValueGetValue(value, .cgSize, &observed)
}

let receipt: [String: Any] = [
    "schemaVersion": 1,
    "pid": pid,
    "windowID": requestedWindowID,
    "requested": ["width": width, "height": height],
    "observed": ["width": observed.width, "height": observed.height],
    "setResult": setResult.rawValue,
]
let data = try JSONSerialization.data(withJSONObject: receipt, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
