# GPUI Display and Mouse APIs Research

**Research Date:** December 24, 2025  
**Project:** Script Kit GPUI - Multi-display window positioning  
**Current Position Code:** `main.rs:731` - `let bounds = Bounds::centered(None, size(px(500.), px(700.0)), cx);`

---

## 1. Mouse Position API

### Getting Mouse Position Within Window

**Method:** `Window::mouse_position()`  
**Location:** `crates/gpui/src/window.rs`  
**Returns:** `Point<Pixels>` - position relative to the window

```rust
// In any Context<V> where you have access to Window
impl Window {
    /// The position of the mouse relative to the window.
    pub fn mouse_position(&self) -> Point<Pixels> {
        self.mouse_position
    }
}
```

**Usage Example:**
```rust
window.on_key_down(cx.listener(|_, _, window: &mut Window, cx| {
    let mouse_pos = window.mouse_position();  // Point<Pixels>
    println!("Mouse at: {:?}", mouse_pos);
}));
```

**Note:** This gives position relative to the window, not global screen coordinates. For positioning on a specific display, you'll need to calculate based on window bounds + mouse position.

### Getting Global Mouse Position

GPUI provides platform-level support but no built-in unified API for global cursor position. For the implementation:

**macOS:** Uses `NSEvent::mouseLocation()` or `CGEventGetLocation()` (via Cocoa)  
**Linux (X11/Wayland):** Platform-specific cursor APIs  
**Windows:** `GetCursorPos()` via Windows API  

**Recommendation:** Use `Window::mouse_position()` within the window context, which is available during event handling.

---

## 2. Displays API

### Enumerating Available Displays

**Method:** `App::displays()`  
**Location:** `crates/gpui/src/app.rs`  
**Returns:** `Vec<Rc<dyn PlatformDisplay>>`

```rust
pub fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
    self.platform.displays()
}
```

### Getting Primary Display

**Method:** `App::primary_display()`  
**Returns:** `Option<Rc<dyn PlatformDisplay>>`

```rust
pub fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
    self.platform.primary_display()
}
```

### Finding Specific Display by ID

**Method:** `App::find_display(id: DisplayId)`  
**Returns:** `Option<Rc<dyn PlatformDisplay>>`

```rust
pub fn find_display(&self, id: DisplayId) -> Option<Rc<dyn PlatformDisplay>> {
    self.displays()
        .iter()
        .find(|display| display.id() == id)
        .cloned()
}
```

### PlatformDisplay Trait Interface

**Location:** `crates/gpui/src/platform.rs`

```rust
pub trait PlatformDisplay: Send + Sync + Debug {
    /// Get the ID for this display
    fn id(&self) -> DisplayId;

    /// Returns a stable identifier for this display that can be persisted
    fn uuid(&self) -> Result<Uuid>;

    /// Get the bounds for this display (full screen area)
    fn bounds(&self) -> Bounds<Pixels>;

    /// Get the visible bounds for this display
    /// (excluding taskbar/dock areas, usable area for windows)
    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.bounds()
    }

    /// Get the default bounds for placing a window on this display
    fn default_bounds(&self) -> Bounds<Pixels> {
        let bounds = self.bounds();
        // ... centers a default-sized window
    }
}
```

### Usage Example

```rust
// Get all displays
let displays = cx.displays();
println!("Available displays: {}", displays.len());

// For each display
for display in displays {
    let bounds = display.bounds();
    println!("Display: {:?}", bounds);
    println!("Visible bounds: {:?}", display.visible_bounds());
    println!("Center: {:?}", bounds.center());
}

// Get primary display
if let Some(primary) = cx.primary_display() {
    let bounds = primary.bounds();
    println!("Primary display bounds: {:?}", bounds);
}
```

---

## 3. Display Detection (Point-in-Display)

### Bounds::contains() Method

**Method:** `Bounds<T>::contains(&Point<T>) -> bool`  
**Location:** `crates/gpui/src/geometry.rs`

```rust
pub fn contains(&self, point: &Point<T>) -> bool {
    point.x >= self.origin.x
        && point.x < self.origin.x.clone() + self.size.width.clone()
        && point.y >= self.origin.y
        && point.y < self.origin.y.clone() + self.size.height.clone()
}
```

### Finding Display Containing a Point

```rust
/// Utility: Find which display contains the given point
fn find_display_for_point(
    point: Point<Pixels>,
    cx: &App,
) -> Option<Rc<dyn PlatformDisplay>> {
    cx.displays()
        .into_iter()
        .find(|display| display.bounds().contains(&point))
}
```

### Usage Example

```rust
let mouse_pos = window.mouse_position();
let window_bounds = window.bounds();

// Calculate absolute mouse position on screen
let absolute_mouse = Point {
    x: window_bounds.origin.x + mouse_pos.x,
    y: window_bounds.origin.y + mouse_pos.y,
};

// Find which display contains the mouse
if let Some(display) = find_display_for_point(absolute_mouse, cx) {
    println!("Mouse is on display: {:?}", display.id());
    println!("Display bounds: {:?}", display.bounds());
}
```

---

## 4. Bounds::centered() - First Parameter Details

### Current Signature

**Location:** `crates/gpui/src/geometry.rs` and `crates/gpui/src/platform.rs`

```rust
impl Bounds<Pixels> {
    /// Generate a centered bounds for the given display or primary display if none is provided
    pub fn centered(
        display_id: Option<DisplayId>,  // <-- FIRST PARAMETER
        size: Size<Pixels>,
        cx: &App,
    ) -> Self {
        let display = display_id
            .and_then(|id| cx.find_display(id))
            .or_else(|| cx.primary_display());

        display
            .map(|display| Bounds::centered_at(display.bounds().center(), size))
            .unwrap_or_else(|| Bounds {
                origin: point(px(0.), px(0.)),
                size,
            })
    }
}
```

### Parameter Analysis

**First Parameter:** `display_id: Option<DisplayId>`

- **Type:** `Option<DisplayId>` (not a Display reference)
- **Purpose:** Specifies which display to center the window on
- **If `Some(id)`:** Finds the display with that ID and centers the window on it
- **If `None`:** Falls back to the primary display
- **Display Reference:** Internally uses `cx.find_display(id)` to get the actual `Rc<dyn PlatformDisplay>`

### Key Points

✅ **Does NOT take a Display reference** - it takes an optional DisplayId  
✅ **Centers window on display bounds center** using `display.bounds().center()`  
✅ **Falls back gracefully** if display not found  
✅ **Primary display handling** - uses primary if `None` is passed

### Implementation of Bounds::centered_at

```rust
impl<T> Bounds<T>
where
    T: Sub<T, Output = T> + Half + Clone + Debug + Default + PartialEq,
{
    /// Creates a new bounds centered at the given point.
    pub fn centered_at(center: Point<T>, size: Size<T>) -> Self {
        let origin = Point {
            x: center.x - size.width.half(),
            y: center.y - size.height.half(),
        };
        Self::new(origin, size)
    }
}
```

---

## 5. Real-World Example: Window Positioning Pattern

From `crates/ui/src/components/right_click_menu.rs` - positioning a menu at mouse:

```rust
*position.borrow_mut() = if let Some(child_bounds) = child_bounds {
    if let Some(attach) = attach {
        child_bounds.corner(attach)
    } else {
        // Position menu at mouse cursor
        window.mouse_position()
    }
} else {
    // Fallback to mouse position
    window.mouse_position()
};
window.refresh();
```

---

## 6. Implementation Pattern: Position Window on Display with Mouse

### Strategy for Eye-Line Positioning (Upper 1/3)

```rust
// Inside window callback or event handler
let mouse_pos = window.mouse_position();
let window_bounds = window.bounds();

// Convert to absolute screen coordinates
let absolute_mouse = Point {
    x: window_bounds.origin.x + mouse_pos.x,
    y: window_bounds.origin.y + mouse_pos.y,
};

// Find which display contains the mouse
let target_display = cx.displays()
    .into_iter()
    .find(|display| display.bounds().contains(&absolute_mouse));

if let Some(display) = target_display {
    let display_bounds = display.bounds();
    let window_size = size(px(500.), px(700.0));
    
    // Position at eye-line height (upper 1/3 of screen)
    let eye_line_y = display_bounds.origin.y 
        + (display_bounds.size.height / 3.0);
    
    // Center horizontally on display
    let centered_x = display_bounds.origin.x 
        + (display_bounds.size.width - window_size.width) / 2.0;
    
    let new_bounds = Bounds {
        origin: Point {
            x: centered_x,
            y: eye_line_y,
        },
        size: window_size,
    };
    
    window.set_bounds(WindowBounds::Windowed(new_bounds), cx);
}
```

### Alternative Using DisplayId

```rust
// If you have a DisplayId from find_display:
if let Some(display) = cx.find_display(display_id) {
    let window_size = size(px(500.), px(700.0));
    let display_bounds = display.bounds();
    
    // Calculate eye-line position
    let eye_line_y = display_bounds.origin.y + display_bounds.size.height / 3.0;
    let center_x = display_bounds.center().x - window_size.width / 2.0;
    
    let bounds = Bounds {
        origin: Point { x: center_x, y: eye_line_y },
        size: window_size,
    };
    
    window.set_bounds(WindowBounds::Windowed(bounds), cx);
}
```

---

## 7. Code Example: Complete Usage

### In Window Callback

```rust
use gpui::{
    App, Context, Window, Bounds, Point, Size, 
    WindowBounds, px, point, size,
};

fn position_window_on_display(
    window: &mut Window,
    cx: &mut App,
) {
    let window_size = size(px(500.), px(700.0));
    
    // Get all displays
    let displays = cx.displays();
    
    if displays.is_empty() {
        // Fallback to centered on primary
        let bounds = Bounds::centered(None, window_size, cx);
        window.set_bounds(WindowBounds::Windowed(bounds), cx);
        return;
    }
    
    // Get primary or use first available
    let target_display = cx.primary_display()
        .or_else(|| cx.displays().into_iter().next());
    
    if let Some(display) = target_display {
        let bounds = display.bounds();
        
        // Position at eye-line (upper 1/3)
        let eye_line = bounds.origin.y + bounds.size.height / 3.0;
        
        // Create positioned bounds
        let positioned = Bounds::centered_at(
            Point {
                x: bounds.center().x,
                y: eye_line,
            },
            window_size,
        );
        
        window.set_bounds(WindowBounds::Windowed(positioned), cx);
    }
}
```

---

## 8. Key Types

### DisplayId
```rust
/// An opaque identifier for a hardware display
pub struct DisplayId {
    // Platform-specific ID
}
```

### Bounds<Pixels>
```rust
pub struct Bounds<T> {
    pub origin: Point<T>,  // Top-left corner
    pub size: Size<T>,     // Width and height
}
```

### Point<Pixels>
```rust
pub struct Point<T> {
    pub x: T,
    pub y: T,
}
```

---

## 9. Summary of APIs Needed

| API | Method | Purpose |
|-----|--------|---------|
| **List displays** | `App::displays()` | Get all available displays |
| **Primary display** | `App::primary_display()` | Get main display |
| **Find display** | `App::find_display(id)` | Get specific display by ID |
| **Display bounds** | `display.bounds()` | Get full bounds of display |
| **Visible bounds** | `display.visible_bounds()` | Get usable bounds (no taskbar) |
| **Display center** | `bounds.center()` | Get center point of display |
| **Point in bounds** | `bounds.contains(&point)` | Check if point is in display |
| **Window mouse pos** | `window.mouse_position()` | Get mouse position in window |
| **Create bounds** | `Bounds::centered()` | Create centered bounds on display |
| **Set window bounds** | `window.set_bounds()` | Move/resize window |

---

## 10. Recommendations for Implementation

1. **Get Display ID from Mouse:** Convert `window.mouse_position()` to absolute coordinates and use `contains()` to find which display
2. **Center Window:** Use `Bounds::centered(Some(display_id), size, cx)` to center on selected display
3. **Eye-Line Offset:** Calculate Y position as `display.bounds().origin.y + display.bounds().size.height / 3.0`
4. **Horizontal Center:** Use `display.bounds().center().x` for X positioning
5. **Fallback:** Always have fallback to primary display if mouse display can't be determined
6. **Call During Window Open:** Do positioning in the window callback passed to `cx.open_window()`

---

## References

- **GPUI Crate:** https://docs.rs/gpui/latest/gpui/
- **Zed Repository:** https://github.com/zed-industries/zed (crates/gpui/src/)
- **Key Files:**
  - `crates/gpui/src/app.rs` - `displays()`, `primary_display()`, `find_display()`
  - `crates/gpui/src/geometry.rs` - `Bounds`, `Point`, `contains()`, `centered()`
  - `crates/gpui/src/window.rs` - `mouse_position()`
  - `crates/gpui/src/platform.rs` - `PlatformDisplay` trait

