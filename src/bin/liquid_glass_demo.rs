//! Liquid glass proof of concept: recreates the macOS 26 Tahoe Spotlight
//! glass morph (category circles gooey-splitting out of the search bar,
//! getting absorbed back in while typing, results panel flowing out below).
//!
//! GPUI owns its own Metal pixels, so the real SwiftUI `glassEffect`
//! refraction is not available. Instead:
//! - `WindowBackgroundAppearance::Blurred` gives a real frosted backdrop
//!   that follows the alpha of what we draw (desktop blurs only behind
//!   the shapes).
//! - The gooey merge is genuine: all glass shapes live in one signed
//!   distance field combined with a polynomial smooth-min, and the merged
//!   contour is extracted with marching squares each frame and painted as
//!   a single liquid path.
//! - Springs (underdamped) drive the choreography for the bouncy feel.
//!
//! Run with: cargo run --bin liquid-glass-demo

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use gpui::{
    canvas, div, hsla, linear_color_stop, linear_gradient, point, prelude::*, px, rgba, size,
    AnimationExt, Bounds, Context, FocusHandle, KeyDownEvent, PathBuilder, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use gpui_platform::application;
use script_kit_gpui::effects::BackgroundEffect;

// ---------------------------------------------------------------------------
// Layout constants (window coordinates)
// ---------------------------------------------------------------------------

const BAR_X: f32 = 40.0;
const BAR_Y: f32 = 50.0;
const BAR_H: f32 = 58.0;
const BAR_W_IDLE: f32 = 500.0;
const BAR_W_OPEN: f32 = 796.0;
const CIRCLE_R: f32 = 29.0;
const CIRCLE_PITCH: f32 = 74.0; // diameter 58 + gap 16
const PANEL_TOP: f32 = BAR_Y + BAR_H - 10.0; // overlaps the bar so the field merges them
const ROW_H: f32 = 44.0;
const MAX_ROWS: usize = 5;
const WINDOW_W: f32 = 880.0;
const WINDOW_H: f32 = 460.0;

const CATEGORY_ICONS: [&str; 4] = ["🏬", "📁", "🗂", "📄"];

const APPS: [(&str, &str); 12] = [
    ("Safari", "🧭"),
    ("Terminal", "🖥"),
    ("Notes", "📝"),
    ("Music", "🎵"),
    ("Mail", "✉️"),
    ("Messages", "💬"),
    ("Maps", "🗺"),
    ("Calendar", "📅"),
    ("Photos", "🌄"),
    ("Finder", "😀"),
    ("Settings", "⚙️"),
    ("Calculator", "🧮"),
];

fn circle_slot_x(i: usize) -> f32 {
    BAR_X + BAR_W_IDLE + 16.0 + CIRCLE_R + i as f32 * CIRCLE_PITCH
}

/// Where circles hide when absorbed: tucked inside the bar's right end.
const CIRCLE_HIDDEN_X: f32 = BAR_X + BAR_W_IDLE - 30.0;

// ---------------------------------------------------------------------------
// Springs
// ---------------------------------------------------------------------------

struct Spring {
    pos: f32,
    vel: f32,
    target: f32,
    stiffness: f32,
    damping: f32,
}

impl Spring {
    fn new(pos: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            pos,
            vel: 0.0,
            target: pos,
            stiffness,
            damping,
        }
    }

    fn step(&mut self, dt: f32) {
        let n = ((dt / 0.004).ceil() as usize).clamp(1, 32);
        let h = dt / n as f32;
        for _ in 0..n {
            let accel = self.stiffness * (self.target - self.pos) - self.damping * self.vel;
            self.vel += accel * h;
            self.pos += self.vel * h;
        }
    }
}

// ---------------------------------------------------------------------------
// Signed distance field for the merged glass
// ---------------------------------------------------------------------------

struct GlassField {
    bar: (f32, f32, f32, f32),     // center x, center y, half w, half h
    circles: Vec<(f32, f32, f32)>, // center x, center y, radius
    panel: Option<(f32, f32, f32, f32, f32)>, // cx, cy, half w, half h, corner radius
    blend: f32,
}

fn sd_round_rect(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - (hw - r);
    let qy = (y - cy).abs() - (hh - r);
    let ax = qx.max(0.0);
    let ay = qy.max(0.0);
    (ax * ax + ay * ay).sqrt() + qx.max(qy).min(0.0) - r
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b * (1.0 - h) + a * h - k * h * (1.0 - h)
}

impl GlassField {
    fn eval(&self, x: f32, y: f32) -> f32 {
        let (bx, by, bhw, bhh) = self.bar;
        let mut d = sd_round_rect(x, y, bx, by, bhw, bhh, bhh); // capsule
        for &(cx, cy, r) in &self.circles {
            let dc = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r;
            d = smin(d, dc, self.blend);
        }
        if let Some((pcx, pcy, phw, phh, pr)) = self.panel {
            let dp = sd_round_rect(x, y, pcx, pcy, phw, phh, pr);
            d = smin(d, dp, self.blend);
        }
        d
    }
}

// ---------------------------------------------------------------------------
// Marching squares: extract iso-contour loops of the field
// ---------------------------------------------------------------------------

type EdgeKey = (usize, usize, u8); // (i, j, 0 = horizontal edge, 1 = vertical edge)

fn contour_loops(field: &GlassField, w: f32, h: f32, cell: f32) -> Vec<Vec<(f32, f32)>> {
    let nx = (w / cell).ceil() as usize + 1;
    let ny = (h / cell).ceil() as usize + 1;
    let mut grid = vec![0.0f32; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            grid[j * nx + i] = field.eval(i as f32 * cell, j as f32 * cell);
        }
    }

    let mut pts: HashMap<EdgeKey, (f32, f32)> = HashMap::new();
    let mut adj: HashMap<EdgeKey, Vec<EdgeKey>> = HashMap::new();
    let interp = |a: f32, b: f32| (a / (a - b)).clamp(0.0, 1.0);

    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let v00 = grid[j * nx + i];
            let v10 = grid[j * nx + i + 1];
            let v01 = grid[(j + 1) * nx + i];
            let v11 = grid[(j + 1) * nx + i + 1];
            let mut case = 0u8;
            if v00 < 0.0 {
                case |= 1
            }
            if v10 < 0.0 {
                case |= 2
            }
            if v11 < 0.0 {
                case |= 4
            }
            if v01 < 0.0 {
                case |= 8
            }
            if case == 0 || case == 15 {
                continue;
            }
            let x0 = i as f32 * cell;
            let y0 = j as f32 * cell;
            let top = ((i, j, 0u8), (x0 + interp(v00, v10) * cell, y0));
            let bottom = ((i, j + 1, 0u8), (x0 + interp(v01, v11) * cell, y0 + cell));
            let left = ((i, j, 1u8), (x0, y0 + interp(v00, v01) * cell));
            let right = ((i + 1, j, 1u8), (x0 + cell, y0 + interp(v10, v11) * cell));

            let mut link = |a: (EdgeKey, (f32, f32)), b: (EdgeKey, (f32, f32))| {
                pts.insert(a.0, a.1);
                pts.insert(b.0, b.1);
                adj.entry(a.0).or_default().push(b.0);
                adj.entry(b.0).or_default().push(a.0);
            };

            match case {
                1 | 14 => link(left, top),
                2 | 13 => link(top, right),
                3 | 12 => link(left, right),
                4 | 11 => link(right, bottom),
                6 | 9 => link(top, bottom),
                7 | 8 => link(left, bottom),
                5 => {
                    link(left, top);
                    link(right, bottom);
                }
                10 => {
                    link(top, right);
                    link(left, bottom);
                }
                _ => unreachable!(),
            }
        }
    }

    let ordered = |a: EdgeKey, b: EdgeKey| if a <= b { (a, b) } else { (b, a) };
    let mut used: HashSet<(EdgeKey, EdgeKey)> = HashSet::new();
    let mut loops = Vec::new();
    let keys: Vec<EdgeKey> = adj.keys().copied().collect();
    for start in keys {
        let neighbors = adj[&start].clone();
        for first in neighbors {
            if used.contains(&ordered(start, first)) {
                continue;
            }
            used.insert(ordered(start, first));
            let mut chain = vec![start, first];
            let (mut prev, mut cur) = (start, first);
            loop {
                let next = adj[&cur]
                    .iter()
                    .copied()
                    .find(|&n| n != prev && !used.contains(&ordered(cur, n)));
                match next {
                    Some(n) => {
                        used.insert(ordered(cur, n));
                        prev = cur;
                        cur = n;
                        if n == start {
                            break;
                        }
                        chain.push(n);
                    }
                    None => break,
                }
            }
            if chain.len() >= 4 {
                loops.push(chain.iter().map(|k| pts[k]).collect::<Vec<_>>());
            }
        }
    }
    loops
}

/// One round of Chaikin corner cutting (closed polygon) to soften grid stairs.
fn chaikin(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = points.len();
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        out.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
        out.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
    }
    out
}

// ---------------------------------------------------------------------------
// The demo view
// ---------------------------------------------------------------------------

struct LiquidGlassDemo {
    query: String,
    selection: usize,
    focus_handle: FocusHandle,
    started: Instant,
    last_frame: Instant,
    bar_w: Spring,
    circles: Vec<Spring>, // progress 0..1 per category circle
    panel_h: Spring,
    shadow_disabled: bool,
    effect: BackgroundEffect,
    last_key: Instant,
    #[cfg(target_os = "macos")]
    native: Option<native_glass::NativeGlass>,
}

#[cfg(target_os = "macos")]
fn ns_view_of(window: &Window) -> Option<cocoa::base::id> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    Some(appkit.ns_view.as_ptr() as cocoa::base::id)
}

/// The window is almost entirely transparent; macOS would draw a full-rect
/// drop shadow behind it (the fork keeps a 0.0001-alpha background to
/// preserve shadows), which reads as a dark veil. Kill the shadow entirely.
#[cfg(target_os = "macos")]
fn try_disable_window_shadow(window: &Window) -> bool {
    use cocoa::base::{id, NO};
    use objc::{msg_send, sel, sel_impl};
    let Some(ns_view) = ns_view_of(window) else {
        return false;
    };
    // SAFETY: ns_view belongs to the live GPUI window on the AppKit main
    // thread; `-[NSView window]` and `-[NSWindow setHasShadow:]` are standard.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return false;
        }
        let _: () = msg_send![ns_window, setHasShadow: NO];
        true
    }
}

#[cfg(not(target_os = "macos"))]
fn try_disable_window_shadow(_window: &Window) -> bool {
    true
}

/// Hand the in-flight mouse-down to AppKit as a native window drag so the
/// borderless window can be moved from anywhere.
#[cfg(target_os = "macos")]
fn start_native_window_drag(window: &Window) {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};
    let Some(ns_view) = ns_view_of(window) else {
        return;
    };
    // SAFETY: main thread; `-[NSWindow performWindowDragWithEvent:]` with the
    // application's current event is the standard borderless-drag pattern.
    unsafe {
        let ns_window: id = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        let app: id = cocoa::appkit::NSApp();
        if app == nil {
            return;
        }
        let event: id = msg_send![app, currentEvent];
        if !event.is_null() {
            let _: () = msg_send![ns_window, performWindowDragWithEvent: event];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn start_native_window_drag(_window: &Window) {}

/// Real liquid glass on macOS 26: AppKit `NSGlassEffectView`s (genuine
/// refraction of whatever is behind the window) hosted in an
/// `NSGlassEffectContainerView` (native gooey shape merging), inserted
/// *below* GPUI's Metal view — the same slot the fork uses for its blur
/// view — so GPUI text/icons composite on top, unrefracted. Classes are
/// resolved at runtime; on older macOS we return None and the demo keeps
/// its CPU-side SDF glass.
#[cfg(target_os = "macos")]
mod native_glass {
    use cocoa::base::{id, NO, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::runtime::Class;
    use objc::{msg_send, sel, sel_impl};

    pub struct NativeGlass {
        content_view: id,
        pub bar: id,
        pub circles: Vec<id>,
        pub panel: id,
    }

    pub fn setup(gpui_view: id) -> Option<NativeGlass> {
        let glass_cls = Class::get("NSGlassEffectView")?;
        let container_cls = Class::get("NSGlassEffectContainerView")?;
        let nsview_cls = Class::get("NSView")?;
        // SAFETY: main thread; standard AppKit alloc/init and view hierarchy
        // calls; `gpui_view` is the live GPUI Metal view inside contentView.
        unsafe {
            let ns_window: id = msg_send![gpui_view, window];
            if ns_window.is_null() {
                return None;
            }
            let content_view: id = msg_send![ns_window, contentView];
            let bounds: NSRect = msg_send![content_view, bounds];
            let resize_mask: u64 = (1 << 1) | (1 << 4); // width + height sizable

            let container: id = msg_send![container_cls, alloc];
            let container: id = msg_send![container, initWithFrame: bounds];
            let _: () = msg_send![container, setAutoresizingMask: resize_mask];
            let _: () = msg_send![container, setSpacing: 24.0f64];

            let inner: id = msg_send![nsview_cls, alloc];
            let inner: id = msg_send![inner, initWithFrame: bounds];
            let _: () = msg_send![inner, setAutoresizingMask: resize_mask];
            let _: () = msg_send![container, setContentView: inner];

            let make_glass = || -> id {
                let v: id = msg_send![glass_cls, alloc];
                let v: id = msg_send![
                    v,
                    initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0))
                ];
                let _: () = msg_send![v, setHidden: YES];
                let _: () = msg_send![inner, addSubview: v];
                v
            };
            let bar = make_glass();
            let circles = (0..4).map(|_| make_glass()).collect();
            let panel = make_glass();

            let below: i64 = -1; // NSWindowBelow
            let _: () = msg_send![content_view, addSubview: container positioned: below relativeTo: gpui_view];

            Some(NativeGlass {
                content_view,
                bar,
                circles,
                panel,
            })
        }
    }

    impl NativeGlass {
        pub fn content_height(&self) -> f64 {
            // SAFETY: main thread, standard accessor.
            unsafe {
                let bounds: NSRect = msg_send![self.content_view, bounds];
                bounds.size.height
            }
        }

        /// x/y are top-left in GPUI (y-down) coordinates; AppKit is y-up.
        pub fn place(
            view: id,
            content_h: f64,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
            radius: f64,
            visible: bool,
        ) {
            // SAFETY: main thread, standard NSView setters plus
            // NSGlassEffectView's cornerRadius property.
            unsafe {
                let _: () = msg_send![view, setHidden: if visible { NO } else { YES }];
                if !visible {
                    return;
                }
                let frame = NSRect::new(
                    NSPoint::new(x, content_h - y - h),
                    NSSize::new(w.max(1.0), h.max(1.0)),
                );
                let _: () = msg_send![view, setFrame: frame];
                let _: () = msg_send![view, setCornerRadius: radius];
            }
        }
    }
}

impl LiquidGlassDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            query: String::new(),
            selection: 0,
            focus_handle: cx.focus_handle(),
            started: Instant::now(),
            last_frame: Instant::now(),
            bar_w: Spring::new(BAR_W_IDLE, 320.0, 24.0),
            circles: (0..4)
                .map(|i| Spring::new(0.0, 260.0 + i as f32 * 15.0, 16.5))
                .collect(),
            panel_h: Spring::new(0.0, 340.0, 26.0),
            shadow_disabled: false,
            effect: BackgroundEffect::Aurora,
            last_key: Instant::now(),
            #[cfg(target_os = "macos")]
            native: None,
        }
    }

    fn results(&self) -> Vec<(&'static str, &'static str)> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let q = self.query.to_lowercase();
        APPS.iter()
            .filter(|(name, _)| name.to_lowercase().contains(&q))
            .copied()
            .take(MAX_ROWS)
            .collect()
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.05);
        self.last_frame = now;

        let t = self.started.elapsed().as_secs_f32();
        let searching = !self.query.is_empty();
        let rows = self.results().len();

        for (i, spring) in self.circles.iter_mut().enumerate() {
            spring.target = if searching {
                0.0
            } else if t > 0.35 + i as f32 * 0.09 {
                1.0
            } else {
                0.0
            };
            spring.step(dt);
        }
        self.bar_w.target = if searching { BAR_W_OPEN } else { BAR_W_IDLE };
        self.bar_w.step(dt);
        self.panel_h.target = if rows > 0 {
            rows as f32 * ROW_H + 34.0
        } else {
            0.0
        };
        self.panel_h.step(dt);
    }

    fn glass_field(&self) -> GlassField {
        let bar_w = self.bar_w.pos.max(BAR_H);
        let bar = (
            BAR_X + bar_w / 2.0,
            BAR_Y + BAR_H / 2.0,
            bar_w / 2.0,
            BAR_H / 2.0,
        );
        let circles = self
            .circles
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let p = s.pos;
                let r = CIRCLE_R * p.clamp(0.0, 1.08);
                if r < 1.5 {
                    return None;
                }
                let x = CIRCLE_HIDDEN_X + (circle_slot_x(i) - CIRCLE_HIDDEN_X) * p;
                Some((x, BAR_Y + BAR_H / 2.0, r))
            })
            .collect();
        let panel = {
            let h = self.panel_h.pos;
            if h > 3.0 {
                let hw = self.bar_w.pos.max(BAR_H) / 2.0;
                Some((
                    BAR_X + self.bar_w.pos.max(BAR_H) / 2.0,
                    PANEL_TOP + h / 2.0,
                    hw,
                    h / 2.0,
                    26.0f32.min(h / 2.0),
                ))
            } else {
                None
            }
        };
        GlassField {
            bar,
            circles,
            panel,
            blend: 16.0,
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.last_key = Instant::now();
        let key = event.keystroke.key.as_str();
        match key {
            "tab" => {
                self.effect = if event.keystroke.modifiers.shift {
                    self.effect.prev()
                } else {
                    self.effect.next()
                };
            }
            "escape" => {
                if self.query.is_empty() {
                    cx.quit();
                } else {
                    self.query.clear();
                    self.selection = 0;
                }
            }
            "backspace" => {
                self.query.pop();
                self.selection = 0;
            }
            "enter" => {
                self.query.clear();
                self.selection = 0;
            }
            "up" => self.selection = self.selection.saturating_sub(1),
            "down" => {
                let rows = self.results().len();
                if rows > 0 {
                    self.selection = (self.selection + 1).min(rows - 1);
                }
            }
            "space" => self.query.push(' '),
            _ => {
                if let Some(ch) = &event.keystroke.key_char {
                    if !ch.is_empty() && !ch.chars().any(|c| c.is_control()) {
                        self.query.push_str(ch);
                        self.selection = 0;
                    }
                } else if key.chars().count() == 1 {
                    self.query.push_str(key);
                    self.selection = 0;
                }
            }
        }
        cx.notify();
    }
}

impl Render for LiquidGlassDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.tick();

        let field = self.glass_field();
        let results = self.results();
        let searching = !self.query.is_empty();
        let bar_w = self.bar_w.pos;
        let panel_alpha = (self.panel_h.pos / 90.0).clamp(0.0, 1.0);
        let t = self.started.elapsed().as_secs_f32();
        let caret_on = (t * 2.0).fract() < 0.65;

        let circle_overlays: Vec<_> = self
            .circles
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let p = s.pos;
                let x = CIRCLE_HIDDEN_X + (circle_slot_x(i) - CIRCLE_HIDDEN_X) * p;
                (x, p.clamp(0.0, 1.0))
            })
            .collect();

        if !self.shadow_disabled {
            self.shadow_disabled = try_disable_window_shadow(window);
        }

        // Native AppKit liquid glass (real refraction) when available: sync
        // the glass view frames to the springs every frame. The container
        // handles the gooey merging natively.
        #[cfg(target_os = "macos")]
        let native_active = {
            if self.native.is_none() {
                if let Some(ns_view) = ns_view_of(window) {
                    self.native = native_glass::setup(ns_view);
                }
            }
            if let Some(glass) = &self.native {
                use native_glass::NativeGlass;
                let ch = glass.content_height();
                NativeGlass::place(
                    glass.bar,
                    ch,
                    BAR_X as f64,
                    BAR_Y as f64,
                    self.bar_w.pos as f64,
                    BAR_H as f64,
                    (BAR_H / 2.0) as f64,
                    true,
                );
                for (i, spring) in self.circles.iter().enumerate() {
                    let p = spring.pos;
                    let r = CIRCLE_R * p.clamp(0.0, 1.08);
                    let x = CIRCLE_HIDDEN_X + (circle_slot_x(i) - CIRCLE_HIDDEN_X) * p;
                    let cy = BAR_Y + BAR_H / 2.0;
                    NativeGlass::place(
                        glass.circles[i],
                        ch,
                        (x - r) as f64,
                        (cy - r) as f64,
                        (r * 2.0) as f64,
                        (r * 2.0) as f64,
                        r as f64,
                        r > 1.5,
                    );
                }
                let ph = self.panel_h.pos;
                NativeGlass::place(
                    glass.panel,
                    ch,
                    BAR_X as f64,
                    PANEL_TOP as f64,
                    self.bar_w.pos as f64,
                    ph as f64,
                    26.0f64.min((ph / 2.0) as f64),
                    ph > 3.0,
                );
                true
            } else {
                false
            }
        };
        #[cfg(not(target_os = "macos"))]
        let native_active = false;

        window.focus(&self.focus_handle, cx);

        div()
            .id("liquid-glass-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _: &gpui::MouseDownEvent, window, _| {
                    start_native_window_drag(window);
                }),
            )
            .size_full()
            .relative()
            .font_family(".SystemUIFont")
            // Liquid glass slab: SDF -> marching squares -> one merged path.
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds: Bounds<gpui::Pixels>, _, window, _| {
                        // Real AppKit glass draws below GPUI; skip the CPU glass.
                        if native_active {
                            return;
                        }
                        let ox = f32::from(bounds.origin.x);
                        let oy = f32::from(bounds.origin.y);
                        let loops = contour_loops(&field, WINDOW_W, WINDOW_H, 4.0);
                        for lp in &loops {
                            let smooth = chaikin(&chaikin(lp));
                            if smooth.len() < 3 {
                                continue;
                            }
                            let build = |style: PathBuilder| {
                                let mut b = style;
                                b.move_to(point(px(smooth[0].0 + ox), px(smooth[0].1 + oy)));
                                for p in &smooth[1..] {
                                    b.line_to(point(px(p.0 + ox), px(p.1 + oy)));
                                }
                                b.close();
                                b.build()
                            };
                            // Frosted base tint.
                            if let Ok(path) = build(PathBuilder::fill()) {
                                window.paint_path(path, rgba(0x1c1e2ac9));
                            }
                            // Vertical sheen, brighter at the top like lit glass.
                            if let Ok(path) = build(PathBuilder::fill()) {
                                window.paint_path(
                                    path,
                                    linear_gradient(
                                        180.0,
                                        linear_color_stop(rgba(0xffffff2e), 0.0),
                                        linear_color_stop(rgba(0xffffff07), 1.0),
                                    ),
                                );
                            }
                            // Specular rim.
                            if let Ok(path) = build(PathBuilder::stroke(px(1.0))) {
                                window.paint_path(path, rgba(0xffffff6b));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            // Script Kit's procedural shader effects, playing inside each
            // glass shape (uv is per-quad, so every shape is its own little
            // scene). Tab / Shift-Tab cycles the roster.
            .children({
                // Faster clock + bright saturated palette so the effect reads
                // as a luminous scene inside the glass, not a faint tint.
                let shader_bg = |intensity: f32| {
                    gpui::shader_effect(
                        self.effect.shader_id(),
                        t * 1.4,
                        [
                            ((62.0 + self.query.len() as f32 * 13.0) / bar_w).clamp(0.05, 0.95),
                            0.5,
                        ],
                        (0.35 + 0.65 * (-(self.last_key.elapsed().as_secs_f32()) * 2.0).exp())
                            .clamp(0.0, 1.0),
                        hsla(0.55, 0.95, 0.72, intensity),
                        hsla(0.80, 0.90, 0.70, intensity),
                    )
                };
                let fx_quad = |x: f32, y: f32, w: f32, h: f32, r: f32, intensity: f32| {
                    div()
                        .absolute()
                        .left(px(x + 1.0))
                        .top(px(y + 1.0))
                        .w(px((w - 2.0).max(0.0)))
                        .h(px((h - 2.0).max(0.0)))
                        .rounded(px((r - 1.0).max(0.0)))
                        .bg(shader_bg(intensity))
                };
                let mut quads = vec![fx_quad(BAR_X, BAR_Y, bar_w, BAR_H, BAR_H / 2.0, 0.62)];
                for (i, spring) in self.circles.iter().enumerate() {
                    let p = spring.pos;
                    let r = CIRCLE_R * p.clamp(0.0, 1.08);
                    if r > 1.5 {
                        let x = CIRCLE_HIDDEN_X + (circle_slot_x(i) - CIRCLE_HIDDEN_X) * p;
                        let cy = BAR_Y + BAR_H / 2.0;
                        quads.push(fx_quad(x - r, cy - r, r * 2.0, r * 2.0, r, 0.5));
                    }
                }
                let ph = self.panel_h.pos;
                if ph > 3.0 {
                    quads.push(fx_quad(
                        BAR_X,
                        PANEL_TOP,
                        bar_w,
                        ph,
                        26.0f32.min(ph / 2.0),
                        0.62,
                    ));
                }
                quads
            })
            // Effect name hint above the bar.
            .child(
                div()
                    .absolute()
                    .left(px(BAR_X + 4.0))
                    .top(px(BAR_Y - 26.0))
                    .text_size(px(11.0))
                    .text_color(rgba(0xffffff66))
                    .child(format!("{} · ⇥ cycle effect", self.effect.name())),
            )
            // Search bar content (flex row so the caret rides the text).
            .child(
                div()
                    .absolute()
                    .left(px(BAR_X))
                    .top(px(BAR_Y))
                    .w(px(bar_w))
                    .h(px(BAR_H))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(22.0))
                    .child(div().text_size(px(19.0)).child("🔍"))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .text_color(if searching {
                                rgba(0xffffffee)
                            } else {
                                rgba(0xffffff73)
                            })
                            .child(if searching {
                                self.query.clone()
                            } else {
                                "Spotlight Search".to_string()
                            }),
                    )
                    .child(
                        div()
                            .w(px(2.0))
                            .h(px(26.0))
                            .rounded(px(1.0))
                            .bg(rgba(0xffffffcc))
                            .opacity(if caret_on { 1.0 } else { 0.0 }),
                    ),
            )
            // Category circle icons riding their springs.
            .children(circle_overlays.into_iter().enumerate().map(|(i, (x, p))| {
                div()
                    .absolute()
                    .left(px(x - CIRCLE_R))
                    .top(px(BAR_Y))
                    .w(px(CIRCLE_R * 2.0))
                    .h(px(BAR_H))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(20.0))
                    .opacity(p * p)
                    .child(CATEGORY_ICONS[i])
            }))
            // Results rows.
            .child(
                div()
                    .absolute()
                    .left(px(BAR_X + 12.0))
                    .top(px(BAR_Y + BAR_H + 6.0))
                    .w(px(bar_w - 24.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .opacity(panel_alpha)
                    .children(results.iter().enumerate().map(|(i, (name, icon))| {
                        let selected = i == self.selection;
                        div()
                            .h(px(ROW_H))
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .px(px(12.0))
                            .rounded(px(12.0))
                            .when(selected, |d| d.bg(rgba(0xffffff1f)))
                            .child(div().text_size(px(20.0)).child(*icon))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(15.0))
                                            .text_color(rgba(0xfffffff2))
                                            .child(*name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(rgba(0xffffff8c))
                                            .child("Application"),
                                    ),
                            )
                            .child(div().flex_grow())
                            .when(selected, |d| {
                                d.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(rgba(0xffffff73))
                                        .child("↩"),
                                )
                            })
                    })),
            )
            // Invisible repeating animation keeps frames flowing for the springs.
            .child(div().size_0().with_animation(
                "liquid-glass-tick",
                gpui::Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |el, _| el,
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
            |_, cx| cx.new(LiquidGlassDemo::new),
        )
        .unwrap();
        cx.on_window_closed(|cx, _| {
            cx.quit();
        })
        .detach();
        cx.activate(true);
    });
}
