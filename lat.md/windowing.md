# Windowing

Script Kit GPUI's sizing and blur behavior depend on a small set of concrete cross-layer rules: the gpui-component root sets the window rem size, Script Kit syncs theme font sizes into that root theme, and popup vibrancy goes through shared AppKit configuration with a special-case footer host.

## Key Facts

- `vendor/gpui-component/crates/ui/src/root.rs` calls `window.set_rem_size(cx.theme().font_size)` during `Root::render`, so rem-based sizing follows the current gpui-component theme on every render.
- `src/theme/gpui_integration.rs` pushes Script Kit's UI and mono font sizes into the global gpui-component theme, which is what ultimately drives rem sizing.
- Main and popup-adjacent overlay windows still use `WindowBackgroundAppearance::Blurred` across launcher-adjacent surfaces such as actions, confirm, ACP popup, ACP chat window, dictation, and notes.
- Shared popup vibrancy configuration uses recursive `NSVisualEffectView` setup with `BehindWindow` blending for detached popup-family windows.
- The native footer host is a special case: it uses an in-window `NSVisualEffectView` with `WithinWindow` blending and a custom `hitTest:` path that forwards non-button interaction back to the GPUI surface.
- Blur tint still depends on Script Kit opacity helpers. `theme.opacity.vibrancy_background` overrides the fallback; otherwise the defaults come from `VIBRANCY_DARK_OPACITY` and `VIBRANCY_LIGHT_OPACITY`.
- Script Kit still swizzles GPUI's `BlurredView.updateLayer` so the native tint layer survives instead of being flattened away.

## Key Files

- [vendor/gpui-component/crates/ui/src/root.rs](/Users/johnlindquist/dev/script-kit-gpui/vendor/gpui-component/crates/ui/src/root.rs) - Root wrapper that applies `window.set_rem_size`.
- [src/theme/gpui_integration.rs](/Users/johnlindquist/dev/script-kit-gpui/src/theme/gpui_integration.rs) - Syncs Script Kit fonts and theme into gpui-component.
- [src/platform/vibrancy_config.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/vibrancy_config.rs) - Recursive `NSVisualEffectView` configuration for blurred windows.
- [src/platform/secondary_window_config.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/secondary_window_config.rs) - Shared popup-family window vibrancy and ACP inline dropdown configuration.
- [src/platform/vibrancy_swizzle_materials.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/vibrancy_swizzle_materials.rs) - `BlurredView.updateLayer` swizzle.
- [src/footer_popup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/footer_popup.rs) - Native footer effect host, `WithinWindow` blending, and passthrough hit-testing.
- [src/ui_foundation/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/ui_foundation/mod.rs) - Vibrancy opacity fallback helpers.

## Source Documents

- [vendor/gpui-component/crates/ui/src/root.rs](/Users/johnlindquist/dev/script-kit-gpui/vendor/gpui-component/crates/ui/src/root.rs)
- [src/theme/gpui_integration.rs](/Users/johnlindquist/dev/script-kit-gpui/src/theme/gpui_integration.rs)
- [src/platform/vibrancy_config.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/vibrancy_config.rs)
- [src/platform/secondary_window_config.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/secondary_window_config.rs)
- [src/platform/vibrancy_swizzle_materials.rs](/Users/johnlindquist/dev/script-kit-gpui/src/platform/vibrancy_swizzle_materials.rs)
- [src/footer_popup.rs](/Users/johnlindquist/dev/script-kit-gpui/src/footer_popup.rs)
- [src/ui_foundation/mod.rs](/Users/johnlindquist/dev/script-kit-gpui/src/ui_foundation/mod.rs)

## Related Pages

- [design](./design.md)
- [architecture](./architecture.md)

## Operational Rules

- Any new top-level window that should participate in rem sizing needs the gpui-component `Root` wrapper.
- Detached popup-family windows should stay on the shared vibrancy path instead of inventing their own AppKit blur stack.
- The footer host is not interchangeable with detached popups; its `WithinWindow` blending and `hitTest:` passthrough are part of the behavior contract.
