#![allow(unexpected_cfgs)] // objc 0.2 macros still probe the retired `cargo-clippy` cfg.

//! Pixel-matched macOS 26 Spotlight detached-glass prototype.
//!
//! GPUI owns one 776 x 88 pt transparent window and the animation clock. Rust
//! hosts real `NSGlassEffectView`s in one `NSGlassEffectContainerView` beneath
//! GPUI's transparent Metal surface. The five visible pieces are therefore
//! sibling views in one native window, so AppKit drags them atomically.
//!
//! Run with:
//! `./scripts/agentic/agent-cargo.sh run --bin liquid-glass-demo`

use std::time::Instant;

use gpui::{
    div, prelude::*, px, size, AnimationExt, Bounds, Context, FocusHandle, KeyDownEvent, Render,
    Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use gpui_platform::application;

// Measured from the 1552 x 176 Retina Spotlight reference (points below).
const WINDOW_W: f32 = 776.0;
const WINDOW_H: f32 = 88.0;
const BAR_X: f32 = 41.0;
const BAR_Y: f32 = 20.0;
const ELEMENT_H: f32 = 56.0;
const EXPANDED_BAR_W: f32 = 640.0;
const DETACHED_BAR_W: f32 = 384.0;
const CIRCLE_DIAMETER: f32 = 56.0;
const CIRCLE_R: f32 = CIRCLE_DIAMETER / 2.0;
const ITEM_GAP: f32 = 8.0;
const CIRCLE_PITCH: f32 = CIRCLE_DIAMETER + ITEM_GAP;

const AUTO_SPLIT_DELAY_SECS: f32 = 0.303;
// Start the spring before the first visible lobe: its first ~40 ms remain
// sub-pixel, yielding the measured five-frame visual hold after contraction.
const CIRCLE_LAUNCH_DELAY_SECS: f32 = 0.040;
const BRIDGE_HOLD_SECS: f32 = 0.150;
const BRIDGE_RETRACT_SECS: f32 = 0.290;
const BRIDGE_SPACING: f32 = 18.0;
const SETTLED_SPACING: f32 = 4.0;

const CATEGORY_SYMBOLS: [&str; 4] = ["appstore", "folder", "square.3.layers.3d", "doc.on.doc"];

fn circle_target_x(index: usize) -> f32 {
    BAR_X + DETACHED_BAR_W + ITEM_GAP + CIRCLE_R + index as f32 * CIRCLE_PITCH
}

#[derive(Clone, Copy)]
struct Spring {
    position: f32,
    velocity: f32,
    target: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            position,
            velocity: 0.0,
            target: position,
            stiffness,
            damping,
        }
    }

    fn step(&mut self, dt: f32) {
        // Fixed-size substeps keep the spring stable through an occasional long
        // render interval without changing the intended 120 Hz choreography.
        let steps = ((dt / (1.0 / 240.0)).ceil() as usize).clamp(1, 32);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let acceleration =
                self.stiffness * (self.target - self.position) - self.damping * self.velocity;
            self.velocity += acceleration * h;
            self.position += self.velocity * h;
        }
    }
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

struct SpotlightGlassDemo {
    focus_handle: FocusHandle,
    started: Instant,
    last_frame: Instant,
    transition_started: Option<Instant>,
    detached: bool,
    auto_split_started: bool,
    bar_width: Spring,
    circle_progress: Spring,
    shadow_disabled: bool,
    #[cfg(target_os = "macos")]
    native: Option<native_glass::NativeGlass>,
}

impl SpotlightGlassDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            started: Instant::now(),
            last_frame: Instant::now(),
            transition_started: None,
            detached: false,
            auto_split_started: false,
            // Tuned to the source: fast front-loaded contraction, a restrained
            // 5-7% overshoot, then target lock inside roughly 500 ms.
            bar_width: Spring::new(EXPANDED_BAR_W, 320.0, 24.0),
            // The material train develops for roughly 300 ms after launch;
            // using the bar's faster spring makes the circles separate three
            // source frames too early and reveals their symbols prematurely.
            circle_progress: Spring::new(0.0, 130.0, 16.5),
            shadow_disabled: false,
            #[cfg(target_os = "macos")]
            native: None,
        }
    }

    fn set_detached(&mut self, detached: bool) {
        self.detached = detached;
        self.transition_started = Some(Instant::now());
        self.bar_width.target = if detached {
            DETACHED_BAR_W
        } else {
            EXPANDED_BAR_W
        };
        // The source contracts the capsule for five frames before the first
        // material lobe appears. `tick` launches the circles after that hold.
        self.circle_progress.target = 0.0;
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.05);
        self.last_frame = now;

        if !self.auto_split_started && self.started.elapsed().as_secs_f32() >= AUTO_SPLIT_DELAY_SECS
        {
            self.auto_split_started = true;
            self.set_detached(true);
        }

        if self.detached
            && self
                .transition_started
                .is_some_and(|started| started.elapsed().as_secs_f32() >= CIRCLE_LAUNCH_DELAY_SECS)
        {
            self.circle_progress.target = 1.0;
        }

        self.bar_width.step(dt);
        self.circle_progress.step(dt);
    }

    fn container_spacing(&self) -> f32 {
        if !self.detached {
            return BRIDGE_SPACING;
        }
        let elapsed = self
            .transition_started
            .map(|started| started.elapsed().as_secs_f32())
            .unwrap_or_default();
        let retract =
            smoothstep((elapsed - BRIDGE_HOLD_SECS) / BRIDGE_RETRACT_SECS.max(f32::EPSILON));
        BRIDGE_SPACING + (SETTLED_SPACING - BRIDGE_SPACING) * retract
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "enter" | "space" => self.set_detached(!self.detached),
            "escape" => cx.quit(),
            _ => return,
        }
        cx.notify();
    }
}

#[cfg(target_os = "macos")]
fn ns_view_of(window: &Window) -> Option<cocoa::base::id> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    Some(appkit.ns_view.as_ptr() as cocoa::base::id)
}

#[cfg(target_os = "macos")]
fn disable_outer_window_shadow(window: &Window) -> bool {
    use cocoa::base::{id, NO};
    use objc::{msg_send, sel, sel_impl};

    let Some(ns_view) = ns_view_of(window) else {
        return false;
    };
    // SAFETY: GPUI renders and invokes this on the live AppKit main thread.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return false;
        }
        let _: () = msg_send![ns_window, setHasShadow: NO];
        let _: () = msg_send![ns_window, setOpaque: NO];
        true
    }
}

#[cfg(not(target_os = "macos"))]
fn disable_outer_window_shadow(_window: &Window) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn start_native_window_drag(window: &Window) {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    let Some(ns_view) = ns_view_of(window) else {
        return;
    };
    // SAFETY: standard borderless-window drag handoff on the AppKit main thread.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        let app: id = cocoa::appkit::NSApp();
        if ns_window == nil || app == nil {
            return;
        }
        let event: id = msg_send![app, currentEvent];
        if event != nil {
            let _: () = msg_send![ns_window, performWindowDragWithEvent: event];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn start_native_window_drag(_window: &Window) {}

#[cfg(target_os = "macos")]
mod native_glass {
    use cocoa::base::{id, nil, NO, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
    use objc::runtime::Class;
    use objc::{class, msg_send, sel, sel_impl};

    use super::{BAR_X, BAR_Y, CATEGORY_SYMBOLS, CIRCLE_R, ELEMENT_H};

    pub struct NativeGlass {
        content_view: id,
        container: id,
        bar: id,
        circles: Vec<id>,
        search_icon: id,
        search_label: id,
        caret: id,
        category_icons: Vec<id>,
    }

    fn ns_string(value: &str) -> id {
        unsafe { NSString::alloc(nil).init_str(value) }
    }

    unsafe fn dynamic_secondary_label_color(alpha: f64) -> id {
        let color: id = msg_send![class!(NSColor), secondaryLabelColor];
        if color == nil {
            return nil;
        }
        msg_send![color, colorWithAlphaComponent: alpha]
    }

    unsafe fn make_symbol_view(symbol: &str, point_size: f64, weight: f64) -> id {
        let name = ns_string(symbol);
        let image: id = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: name
            accessibilityDescription: nil
        ];
        if image == nil {
            return nil;
        }
        let configuration: id = msg_send![
            class!(NSImageSymbolConfiguration),
            configurationWithPointSize: point_size
            weight: weight
        ];
        let configured: id = if configuration == nil {
            image
        } else {
            msg_send![image, imageWithSymbolConfiguration: configuration]
        };
        let view: id = msg_send![class!(NSImageView), alloc];
        let view: id = msg_send![view, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(point_size + 4.0, point_size + 4.0),
        )];
        if view != nil {
            let _: () = msg_send![view, setImage: configured];
            let _: () = msg_send![view, setImageScaling: 0usize];
            let color = dynamic_secondary_label_color(0.86);
            if color != nil {
                let _: () = msg_send![view, setContentTintColor: color];
            }
        }
        view
    }

    unsafe fn make_app_store_view() -> id {
        // `appstore` is not an exported SF Symbol on macOS. Spotlight uses the
        // familiar three-tool mark, so keep a tiny template vector beside the
        // other native symbols instead of substituting a storefront glyph.
        const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
<g fill="none" stroke="#000" stroke-width="5.2" stroke-linecap="round" stroke-linejoin="round">
<path d="M10 37 L25 10"/><path d="M20 10 L38 37"/><path d="M8 30 L40 30"/>
</g></svg>"##;
        let data: id = msg_send![
            class!(NSData),
            dataWithBytes: SVG.as_ptr() as *const std::ffi::c_void
            length: SVG.len()
        ];
        let image: id = msg_send![class!(NSImage), alloc];
        let image: id = msg_send![image, initWithData: data];
        if image == nil {
            return nil;
        }
        let _: () = msg_send![image, setTemplate: YES];
        let view: id = msg_send![class!(NSImageView), alloc];
        let view: id = msg_send![view, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(26.0, 26.0),
        )];
        if view != nil {
            let _: () = msg_send![view, setImage: image];
            let _: () = msg_send![view, setImageScaling: 0usize];
            let color = dynamic_secondary_label_color(0.86);
            if color != nil {
                let _: () = msg_send![view, setContentTintColor: color];
            }
        }
        view
    }

    unsafe fn make_label() -> id {
        let field: id = msg_send![class!(NSTextField), alloc];
        let field: id = msg_send![field, init];
        if field == nil {
            return nil;
        }
        let value = ns_string("Spotlight Search");
        let font: id = msg_send![class!(NSFont), systemFontOfSize: 24.0f64 weight: -0.4f64];
        let color = dynamic_secondary_label_color(0.78);
        let _: () = msg_send![field, setStringValue: value];
        let _: () = msg_send![field, setBezeled: NO];
        let _: () = msg_send![field, setBordered: NO];
        let _: () = msg_send![field, setDrawsBackground: NO];
        let _: () = msg_send![field, setEditable: NO];
        let _: () = msg_send![field, setSelectable: NO];
        let _: () = msg_send![field, setUsesSingleLineMode: YES];
        let _: () = msg_send![field, setFont: font];
        if color != nil {
            let _: () = msg_send![field, setTextColor: color];
        }
        let _: () = msg_send![field, sizeToFit];
        field
    }

    unsafe fn make_caret() -> id {
        let view: id = msg_send![class!(NSView), alloc];
        let view: id = msg_send![view, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(2.0, 30.0),
        )];
        if view == nil {
            return nil;
        }
        let _: () = msg_send![view, setWantsLayer: YES];
        let layer: id = msg_send![view, layer];
        let color = dynamic_secondary_label_color(0.86);
        if layer != nil && color != nil {
            let cg_color: id = msg_send![color, CGColor];
            let _: () = msg_send![layer, setBackgroundColor: cg_color];
            let _: () = msg_send![layer, setCornerRadius: 1.0f64];
        }
        view
    }

    unsafe fn make_glass(glass_class: &Class, view_class: &Class, parent: id) -> (id, id) {
        let glass: id = msg_send![glass_class, alloc];
        let glass: id = msg_send![glass, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(10.0, 10.0),
        )];
        if glass == nil {
            return (nil, nil);
        }

        // AppKit only guarantees foreground placement for a glass effect's
        // `contentView`. Arbitrary siblings may be sampled as background and
        // refracted, which turns text and symbols into unreadable smears.
        let chrome: id = msg_send![view_class, alloc];
        let chrome: id = msg_send![chrome, initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(10.0, 10.0),
        )];
        let resize_mask: u64 = (1 << 1) | (1 << 4);
        let _: () = msg_send![chrome, setAutoresizingMask: resize_mask];
        let _: () = msg_send![glass, setContentView: chrome];
        let _: () = msg_send![glass, setHidden: YES];
        let _: () = msg_send![parent, addSubview: glass];
        (glass, chrome)
    }

    impl NativeGlass {
        pub fn setup(gpui_view: id) -> Option<Self> {
            let glass_class = Class::get("NSGlassEffectView")?;
            let container_class = Class::get("NSGlassEffectContainerView")?;
            let view_class = Class::get("NSView")?;

            // SAFETY: all objects are created and installed on AppKit's main thread.
            unsafe {
                let ns_window: id = msg_send![gpui_view, window];
                if ns_window == nil {
                    return None;
                }
                let content_view: id = msg_send![ns_window, contentView];
                let bounds: NSRect = msg_send![content_view, bounds];
                let resize_mask: u64 = (1 << 1) | (1 << 4);

                let container: id = msg_send![container_class, alloc];
                let container: id = msg_send![container, initWithFrame: bounds];
                let _: () = msg_send![container, setAutoresizingMask: resize_mask];
                let _: () = msg_send![container, setSpacing: 18.0f64];

                let inner: id = msg_send![view_class, alloc];
                let inner: id = msg_send![inner, initWithFrame: bounds];
                let _: () = msg_send![inner, setAutoresizingMask: resize_mask];
                let _: () = msg_send![container, setContentView: inner];

                let (bar, bar_content) = make_glass(glass_class, view_class, inner);
                let circle_pairs = (0..4)
                    .map(|_| make_glass(glass_class, view_class, inner))
                    .collect::<Vec<_>>();
                let circles = circle_pairs
                    .iter()
                    .map(|(glass, _)| *glass)
                    .collect::<Vec<_>>();
                let circle_contents = circle_pairs
                    .iter()
                    .map(|(_, content)| *content)
                    .collect::<Vec<_>>();

                // Every chrome view belongs to its owning glass `contentView`.
                // This is the supported foreground hierarchy in macOS 26.
                let search_icon = make_symbol_view("magnifyingglass", 22.0, 0.23);
                let search_label = make_label();
                let caret = make_caret();
                for view in [search_icon, search_label, caret] {
                    if view != nil {
                        let _: () = msg_send![bar_content, addSubview: view];
                    }
                }
                let category_icons = CATEGORY_SYMBOLS
                    .iter()
                    .enumerate()
                    .map(|(index, symbol)| {
                        let view = if index == 0 {
                            make_app_store_view()
                        } else {
                            make_symbol_view(symbol, 22.0, 0.23)
                        };
                        if view != nil {
                            let _: () = msg_send![circle_contents[index], addSubview: view];
                        }
                        view
                    })
                    .collect();

                let _: () = msg_send![
                    content_view,
                    addSubview: container
                    positioned: -1i64
                    relativeTo: gpui_view
                ];

                Some(Self {
                    content_view,
                    container,
                    bar,
                    circles,
                    search_icon,
                    search_label,
                    caret,
                    category_icons,
                })
            }
        }

        fn content_height(&self) -> f64 {
            unsafe {
                let bounds: NSRect = msg_send![self.content_view, bounds];
                bounds.size.height
            }
        }

        unsafe fn place_view(
            view: id,
            content_height: f64,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            visible: bool,
        ) {
            if view == nil {
                return;
            }
            let _: () = msg_send![view, setHidden: if visible { NO } else { YES }];
            if visible {
                let frame = NSRect::new(
                    NSPoint::new(x, content_height - y - height),
                    NSSize::new(width.max(1.0), height.max(1.0)),
                );
                let _: () = msg_send![view, setFrame: frame];
            }
        }

        unsafe fn place_glass(
            view: id,
            content_height: f64,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            radius: f64,
            visible: bool,
        ) {
            Self::place_view(view, content_height, x, y, width, height, visible);
            if view != nil && visible {
                let _: () = msg_send![view, setCornerRadius: radius];
            }
        }

        pub fn sync(
            &self,
            bar_width: f32,
            circle_progress: f32,
            spacing: f32,
            caret_visible: bool,
        ) {
            let content_height = self.content_height();
            let progress = circle_progress.clamp(0.0, 1.08);
            let radius = CIRCLE_R * progress;
            let icon_alpha = super::smoothstep((progress - 0.86) / 0.14) as f64;

            unsafe {
                let _: () = msg_send![self.container, setSpacing: spacing as f64];
                Self::place_glass(
                    self.bar,
                    content_height,
                    BAR_X as f64,
                    BAR_Y as f64,
                    bar_width.max(ELEMENT_H) as f64,
                    ELEMENT_H as f64,
                    (ELEMENT_H / 2.0) as f64,
                    true,
                );

                // Material is emitted by the capsule's live trailing cap. This
                // is what keeps frames 24-29 connected while the bar is still
                // contracting; anchoring at the old expanded edge creates a
                // visibly incorrect transient gap.
                let material_origin_x = BAR_X + bar_width - CIRCLE_R;
                for index in 0..4 {
                    let center_x = material_origin_x
                        + (super::circle_target_x(index) - material_origin_x) * progress;
                    Self::place_glass(
                        self.circles[index],
                        content_height,
                        (center_x - radius) as f64,
                        (BAR_Y + CIRCLE_R - radius) as f64,
                        (radius * 2.0) as f64,
                        (radius * 2.0) as f64,
                        radius as f64,
                        radius > 1.0,
                    );

                    let icon_size = 26.0f32;
                    let circle_size = radius * 2.0;
                    Self::place_view(
                        self.category_icons[index],
                        circle_size as f64,
                        ((circle_size - icon_size) / 2.0) as f64,
                        ((circle_size - icon_size) / 2.0) as f64,
                        icon_size as f64,
                        icon_size as f64,
                        radius > 10.0,
                    );
                    if self.category_icons[index] != nil {
                        let _: () = msg_send![
                            self.category_icons[index],
                            setAlphaValue: icon_alpha
                        ];
                    }
                }

                Self::place_view(
                    self.search_icon,
                    ELEMENT_H as f64,
                    18.0,
                    15.0,
                    26.0,
                    26.0,
                    true,
                );
                Self::place_view(self.caret, ELEMENT_H as f64, 59.0, 13.0, 2.0, 30.0, true);
                let _: () = msg_send![
                    self.caret,
                    setAlphaValue: if caret_visible { 0.90f64 } else { 0.0f64 }
                ];

                let label_size: NSSize = msg_send![self.search_label, fittingSize];
                Self::place_view(
                    self.search_label,
                    ELEMENT_H as f64,
                    59.0,
                    ((ELEMENT_H - label_size.height as f32) / 2.0) as f64,
                    label_size.width,
                    label_size.height,
                    true,
                );
            }
        }
    }
}

impl Render for SpotlightGlassDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick();

        if !self.shadow_disabled {
            self.shadow_disabled = disable_outer_window_shadow(window);
        }

        #[cfg(target_os = "macos")]
        {
            if self.native.is_none() {
                if let Some(ns_view) = ns_view_of(window) {
                    self.native = native_glass::NativeGlass::setup(ns_view);
                }
            }
            if let Some(native) = &self.native {
                let caret_visible = (self.started.elapsed().as_secs_f32() * 2.0).fract() < 0.65;
                native.sync(
                    self.bar_width.position,
                    self.circle_progress.position,
                    self.container_spacing(),
                    caret_visible,
                );
            }
        }

        window.focus(&self.focus_handle, cx);

        div()
            .id("spotlight-detached-glass-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _: &gpui::MouseDownEvent, window, _| {
                    start_native_window_drag(window);
                }),
            )
            .size_full()
            .child(div().size_0().with_animation(
                "spotlight-detached-glass-tick",
                gpui::Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |element, _| element,
            ))
    }
}

fn main() {
    application().run(|cx| {
        let bounds = Bounds::centered(None, size(px(WINDOW_W), px(WINDOW_H)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: true,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: true,
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |_, cx| cx.new(SpotlightGlassDemo::new),
        )
        .expect("open the Spotlight detached-glass prototype");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
}
