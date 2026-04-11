# Design

Script Kit GPUI's design language stays keyboard-first, macOS-native, and deliberately quiet. The chrome should stay out of the way while still giving the user clear affordances.

## Launcher contract

The main launcher footer keeps at most three primary affordances: `Run`, `Actions`, and `AI`. Anything beyond that belongs in the `Actions` dialog or a more specific surface rather than in persistent chrome.

## Chrome style

The visual system uses whisper-thin borders, low-opacity fills, and stable spacing instead of card-heavy composition. Theme work should route through the shared opacity and chrome tokens in `src/theme/opacity.rs` and `src/theme/chrome.rs`.

The current theme layer also has a unified resolver path in `src/theme/color_resolver.rs` for colors, typography, and spacing. New theme-aware UI should prefer those resolver types instead of reintroducing ad hoc default-vs-design branching.

## Rem sizing

Window-wide rem sizing is driven by the gpui-component `Root` wrapper, which pushes `cx.theme().font_size` into `window.set_rem_size(...)` during render. Text and spacing that should scale with the UI should stay on rem-based helpers such as `text_sm()` and `rems(...)`; fixed chrome such as borders and exact icon boxes can stay in pixels.

## Vibrancy

Popup surfaces should stay translucent when vibrancy is enabled, not boxed in by opaque fills. The current stack depends on blurred GPUI windows, Script Kit's `BlurredView` swizzle, popup-specific `NSVisualEffectView` configuration, and low-opacity hover or selection overlays so the desktop blur remains visible.

## Overlay split

The main launcher window, footer strip, detached popups, and ACP inline dropdowns do not all use the same macOS blur recipe. The footer is an in-window `NSVisualEffectView` host with `WithinWindow` blending, general popups flow through `configure_secondary_window_vibrancy()`, and ACP inline dropdowns use `configure_inline_dropdown_popup_window()` so they feel attached instead of detached.

## Window levels

Popup-family windows should stay in GPUI's popup level contract instead of manually inventing new window levels. The current repo rules are explicit: `WindowKind::PopUp` windows already sit at the necessary popup level, `orderFrontRegardless` is the tool for resurfacing them, and child-window attachment is how confirm-style overlays stay visibly above the parent without breaking that level contract.

## Context portalling

Inline `@` mentions are designed as stable pointers into other context surfaces. Passive preview is allowed, but entering or replacing a mention must be explicit and must preserve a clear return path back to the original editor or chat surface.

## Popup behavior

Parent-relative popup windows, shared footer density, and consistent row heights keep the app feeling like one system instead of a stack of unrelated dialogs. That rule matters most for the main window, actions popup, and context-picker surfaces.

## Current sources

This page is justified by the live chrome, popup, and portal code plus the root repo contract:

- [CLAUDE.md](../CLAUDE.md)
- [AGENTS.md](../AGENTS.md)
- [src/footer_popup.rs](../src/footer_popup.rs)
- [src/app_impl/attachment_portal.rs](../src/app_impl/attachment_portal.rs)
- [src/actions/window.rs](../src/actions/window.rs)
- [src/confirm/window.rs](../src/confirm/window.rs)
- [src/ai/acp/popup_window.rs](../src/ai/acp/popup_window.rs)
- [src/platform/secondary_window_config.rs](../src/platform/secondary_window_config.rs)
- [src/platform/vibrancy_swizzle_materials.rs](../src/platform/vibrancy_swizzle_materials.rs)
- [src/theme/chrome.rs](../src/theme/chrome.rs)
- [src/theme/color_resolver.rs](../src/theme/color_resolver.rs)
- [src/theme/opacity.rs](../src/theme/opacity.rs)
- [src/ui_foundation/mod.rs](../src/ui_foundation/mod.rs)
- [vendor/gpui-component/crates/ui/src/root.rs](../vendor/gpui-component/crates/ui/src/root.rs)
- [vendor/gpui/src/window.rs](../vendor/gpui/src/window.rs)

## Related Pages

- [windowing](./windowing.md)
