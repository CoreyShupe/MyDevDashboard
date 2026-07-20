// window-id.swift — print the Dev Dashboard app's on-screen window id, or exit non-zero.
//
// Used by `dev-dash`'s screenshot commands (`shot`/`snap`) to capture ONLY the app window
// (via `screencapture -l <id>`) instead of the whole screen — so the macOS menu bar (top) and
// dock (bottom), which can leak other apps / notifications, are never in frame (opsec).
//
// Reads the CoreGraphics on-screen window list and prints the id of the largest normal-layer
// (layer 0) window owned by a process whose name contains "dev-dashboard" — i.e. the app's main
// window, not a menu/overlay. This needs only Screen Recording permission (same as any
// screencapture) — NOT Accessibility — because the window list + bounds are readable without it.
//
// Run: `swift static/scripts/window-id.swift` (invoked by `dev-dash`; not meant to be run by hand).

import CoreGraphics
import Foundation

let opts = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }

var best: (Int, Double)? = nil
for w in list {
    guard let owner = w[kCGWindowOwnerName as String] as? String, owner.contains("dev-dashboard") else { continue }
    guard let layer = w[kCGWindowLayer as String] as? Int, layer == 0 else { continue }
    guard let num = w[kCGWindowNumber as String] as? Int else { continue }
    guard let b = w[kCGWindowBounds as String] as? [String: Any],
          let width = b["Width"] as? Double, let height = b["Height"] as? Double else { continue }
    let area = width * height
    if best == nil || area > best!.1 { best = (num, area) }
}
if let b = best { print(b.0); exit(0) }
exit(1)
