// window-id.swift — print the Dev Dashboard app's on-screen window id, or exit non-zero.
//
// Used by `dev-dash`'s screenshot commands (`shot`/`snap`) to capture ONLY the app window
// (via `screencapture -l <id>`) instead of the whole screen — so the macOS menu bar (top) and
// dock (bottom), which can leak other apps / notifications, are never in frame (opsec).
//
// Reads the CoreGraphics on-screen window list and prints the id of the largest normal-layer
// (layer 0) window owned by a process whose name contains "dashboard" (case-insensitive) — i.e.
// the app's main window, not a menu/overlay. The match is loose on purpose: the dev binary's
// process is "my-dev-dashboard" (from `cargo run`) but the PROD bundle's is "DevDashboard" (the
// bundle/launcher name, §14), so a strict "dev-dashboard" match would miss the prod app entirely
// and fall back to a full-screen grab. This needs only Screen Recording permission (same as any
// screencapture) — NOT Accessibility — because the window list, bounds, AND titles are readable
// with Screen Recording.
//
// A DEV_VIEW run titles its window "Dev Dashboard [DEV: …]" (see src/main.rs); the live app is
// plain "Dev Dashboard". The first argument selects which to match so the two never cross:
//   dev  → only windows whose title contains "[DEV"  (a `shot` of a mock — never the live data)
//   live → only windows whose title does NOT (a `snap` of the running app — never a stray mock)
// Default is "live". This is what lets `shot` and `snap` target the right window even when both a
// mock and the live app are on screen at once.
//
// Run: `swift static/scripts/window-id.swift [dev|live]` (invoked by `dev-dash`).

import CoreGraphics
import Foundation

let mode = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "live"

let opts = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
guard let list = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }

var best: (Int, Double)? = nil
for w in list {
    guard let owner = w[kCGWindowOwnerName as String] as? String, owner.lowercased().contains("dashboard") else { continue }
    guard let layer = w[kCGWindowLayer as String] as? Int, layer == 0 else { continue }
    guard let num = w[kCGWindowNumber as String] as? Int else { continue }
    guard let b = w[kCGWindowBounds as String] as? [String: Any],
          let width = b["Width"] as? Double, let height = b["Height"] as? Double else { continue }
    // Title-match so a mock shot and a live snap can't grab each other's window.
    let title = w[kCGWindowName as String] as? String ?? ""
    let isDev = title.contains("[DEV")
    if mode == "dev" && !isDev { continue }
    if mode == "live" && isDev { continue }
    let area = width * height
    if best == nil || area > best!.1 { best = (num, area) }
}
if let b = best { print(b.0); exit(0) }
exit(1)
