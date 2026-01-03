//! UI Transitions Module
//!
//! Provides transition helpers for smooth UI animations.

// Allow dead code as this module provides utility functions that may not all be used yet
#![allow(dead_code)]
//!
//! # Key Components
//!
//! - `TransitionColor`: Color value supporting Lerp for smooth transitions
//! - `Opacity`: Opacity value (0.0-1.0) for fade transitions
//! - `SlideOffset`: X/Y offset for slide animations
//! - `AppearTransition`: Combined opacity + slide for toast/notification animations
//! - `HoverState`: Background color transition for list item hover effects
//!
//!
//! # Easing Functions
//!
//! - `linear`: No easing (constant velocity)
//! - `ease_out_quad`: Fast start, slow end (good for enter animations)
//! - `ease_in_quad`: Slow start, fast end (good for exit animations)
//! - `ease_in_out_quad`: Slow start and end (good for continuous loops)

use gpui::Rgba;
use std::time::Duration;

// ============================================================================
// Lerp Trait
// ============================================================================

/// A value which can be linearly interpolated with another value of the same type.
///
/// The `delta` parameter is a value from 0.0 to 1.0 where:
/// - 0.0 returns `self`
/// - 1.0 returns `to`
/// - Values in between return a linear interpolation
pub trait Lerp {
    fn lerp(&self, to: &Self, delta: f32) -> Self;
}

// ============================================================================
// Standard Durations
// ============================================================================

/// Fast transition (100ms) - for hover effects, micro-interactions
pub const DURATION_FAST: Duration = Duration::from_millis(100);

/// Standard transition (150ms) - for selection changes, hover feedback
pub const DURATION_STANDARD: Duration = Duration::from_millis(150);

/// Medium transition (200ms) - for panel reveals, focus changes
pub const DURATION_MEDIUM: Duration = Duration::from_millis(200);

/// Slow transition (300ms) - for large UI changes, modal appearances
pub const DURATION_SLOW: Duration = Duration::from_millis(300);

// ============================================================================
// Easing Functions
// ============================================================================

/// Linear easing - constant velocity
#[inline]
pub fn linear(t: f32) -> f32 {
    t
}

/// Quadratic ease out - fast start, slow end
/// Good for elements entering the screen
#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Quadratic ease in - slow start, fast end
/// Good for elements leaving the screen
#[inline]
pub fn ease_in_quad(t: f32) -> f32 {
    t * t
}

/// Quadratic ease in-out - slow start and end
/// Good for continuous looping animations
#[inline]
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Cubic ease out - faster deceleration than quadratic
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease in - slower acceleration than quadratic
#[inline]
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

// ============================================================================
// Lerp Implementations for Primitives
// ============================================================================

impl Lerp for f32 {
    #[inline]
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        self + (to - self) * delta
    }
}

impl Lerp for f64 {
    #[inline]
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        self + (to - self) * (delta as f64)
    }
}

// ============================================================================
// Color Transition Helpers
// ============================================================================

/// A color value that supports linear interpolation for transitions
///
/// Wraps gpui::Rgba to provide smooth color transitions with alpha support.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl TransitionColor {
    /// Create from RGBA components (0.0-1.0 range)
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from a hex color with alpha
    pub fn from_hex_alpha(hex: u32, alpha: f32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as f32 / 255.0,
            g: ((hex >> 8) & 0xFF) as f32 / 255.0,
            b: (hex & 0xFF) as f32 / 255.0,
            a: alpha,
        }
    }

    /// Create a fully transparent color
    pub fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Convert to gpui::Rgba
    pub fn to_rgba(self) -> Rgba {
        Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

impl Lerp for TransitionColor {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            r: self.r + (to.r - self.r) * delta,
            g: self.g + (to.g - self.g) * delta,
            b: self.b + (to.b - self.b) * delta,
            a: self.a + (to.a - self.a) * delta,
        }
    }
}

impl From<Rgba> for TransitionColor {
    fn from(rgba: Rgba) -> Self {
        Self {
            r: rgba.r,
            g: rgba.g,
            b: rgba.b,
            a: rgba.a,
        }
    }
}

impl From<TransitionColor> for Rgba {
    fn from(tc: TransitionColor) -> Self {
        tc.to_rgba()
    }
}

impl Default for TransitionColor {
    fn default() -> Self {
        Self::transparent()
    }
}

// ============================================================================
// Opacity Transition Helper
// ============================================================================

/// Opacity value for fade transitions (0.0 = invisible, 1.0 = fully visible)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(pub f32);

impl Opacity {
    pub const INVISIBLE: Self = Self(0.0);
    pub const VISIBLE: Self = Self(1.0);
    pub const HALF: Self = Self(0.5);

    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Lerp for Opacity {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self(self.0 + (to.0 - self.0) * delta)
    }
}

impl Default for Opacity {
    fn default() -> Self {
        Self::VISIBLE
    }
}

// ============================================================================
// Transform Values for Slide Transitions
// ============================================================================

/// Vertical offset in pixels for slide animations
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SlideOffset {
    pub x: f32,
    pub y: f32,
}

impl SlideOffset {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Slide from bottom
    pub fn from_bottom(amount: f32) -> Self {
        Self { x: 0.0, y: amount }
    }

    /// Slide from top
    pub fn from_top(amount: f32) -> Self {
        Self { x: 0.0, y: -amount }
    }
}

impl Lerp for SlideOffset {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            x: self.x + (to.x - self.x) * delta,
            y: self.y + (to.y - self.y) * delta,
        }
    }
}

// ============================================================================
// Combined Transitions for Common Patterns
// ============================================================================

/// Combined opacity and slide for toast/notification animations
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppearTransition {
    pub opacity: Opacity,
    pub offset: SlideOffset,
}

impl AppearTransition {
    /// Initial hidden state (invisible, offset down)
    pub fn hidden() -> Self {
        Self {
            opacity: Opacity::INVISIBLE,
            offset: SlideOffset::from_bottom(20.0),
        }
    }

    /// Visible state (fully visible, no offset)
    pub fn visible() -> Self {
        Self {
            opacity: Opacity::VISIBLE,
            offset: SlideOffset::ZERO,
        }
    }

    /// Dismiss state (invisible, offset up)
    pub fn dismissed() -> Self {
        Self {
            opacity: Opacity::INVISIBLE,
            offset: SlideOffset::from_top(10.0),
        }
    }
}

impl Lerp for AppearTransition {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            opacity: self.opacity.lerp(&to.opacity, delta),
            offset: self.offset.lerp(&to.offset, delta),
        }
    }
}

impl Default for AppearTransition {
    fn default() -> Self {
        Self::hidden()
    }
}

// ============================================================================
// Hover State for List Items
// ============================================================================

/// Hover state for list item background transitions
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverState {
    /// Background color (transitions between normal/hovered/selected)
    pub background: TransitionColor,
}

impl HoverState {
    pub fn normal() -> Self {
        Self {
            background: TransitionColor::transparent(),
        }
    }

    pub fn with_background(color: TransitionColor) -> Self {
        Self { background: color }
    }
}

impl Lerp for HoverState {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            background: self.background.lerp(&to.background, delta),
        }
    }
}

impl Default for HoverState {
    fn default() -> Self {
        Self::normal()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Easing Function Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_linear_easing() {
        assert!((linear(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((linear(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((linear(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ease_out_quad() {
        // t=0 should give 0
        assert!((ease_out_quad(0.0) - 0.0).abs() < f32::EPSILON);
        // t=1 should give 1
        assert!((ease_out_quad(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 should be > 0.5 (fast start)
        assert!(ease_out_quad(0.5) > 0.5);
    }

    #[test]
    fn test_ease_in_quad() {
        assert!((ease_in_quad(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((ease_in_quad(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 should be < 0.5 (slow start)
        assert!(ease_in_quad(0.5) < 0.5);
    }

    #[test]
    fn test_ease_in_out_quad() {
        assert!((ease_in_out_quad(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((ease_in_out_quad(1.0) - 1.0).abs() < f32::EPSILON);
        // t=0.5 should be exactly 0.5 (symmetric)
        assert!((ease_in_out_quad(0.5) - 0.5).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // Opacity Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_opacity_lerp_start_end() {
        let from = Opacity::INVISIBLE;
        let to = Opacity::VISIBLE;

        // At delta=0, should be start value
        let result = from.lerp(&to, 0.0);
        assert!((result.0 - 0.0).abs() < f32::EPSILON);

        // At delta=1, should be end value
        let result = from.lerp(&to, 1.0);
        assert!((result.0 - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_opacity_lerp_midpoint() {
        let from = Opacity::INVISIBLE;
        let to = Opacity::VISIBLE;

        // At delta=0.5, should be halfway
        let result = from.lerp(&to, 0.5);
        assert!((result.0 - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_opacity_clamp() {
        let clamped = Opacity::new(1.5);
        assert!((clamped.0 - 1.0).abs() < f32::EPSILON);

        let clamped = Opacity::new(-0.5);
        assert!((clamped.0 - 0.0).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // TransitionColor Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_transition_color_lerp() {
        let from = TransitionColor::transparent();
        let to = TransitionColor::new(1.0, 1.0, 1.0, 1.0);

        // At delta=0
        let result = from.lerp(&to, 0.0);
        assert!((result.a - 0.0).abs() < f32::EPSILON);

        // At delta=1
        let result = from.lerp(&to, 1.0);
        assert!((result.r - 1.0).abs() < f32::EPSILON);
        assert!((result.g - 1.0).abs() < f32::EPSILON);
        assert!((result.b - 1.0).abs() < f32::EPSILON);
        assert!((result.a - 1.0).abs() < f32::EPSILON);

        // At delta=0.5
        let result = from.lerp(&to, 0.5);
        assert!((result.r - 0.5).abs() < f32::EPSILON);
        assert!((result.a - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_transition_color_from_hex() {
        let color = TransitionColor::from_hex_alpha(0xFF8800, 0.5);
        assert!((color.r - 1.0).abs() < 0.01); // FF = 255 = 1.0
        assert!((color.g - 0.533).abs() < 0.01); // 88 = 136 = 0.533
        assert!((color.b - 0.0).abs() < f32::EPSILON); // 00 = 0 = 0.0
        assert!((color.a - 0.5).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // SlideOffset Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_slide_offset_lerp() {
        let from = SlideOffset::from_bottom(20.0);
        let to = SlideOffset::ZERO;

        // At delta=0
        let result = from.lerp(&to, 0.0);
        assert!((result.y - 20.0).abs() < f32::EPSILON);

        // At delta=1
        let result = from.lerp(&to, 1.0);
        assert!((result.y - 0.0).abs() < f32::EPSILON);

        // At delta=0.5
        let result = from.lerp(&to, 0.5);
        assert!((result.y - 10.0).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // AppearTransition Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_appear_transition_hidden_to_visible() {
        let from = AppearTransition::hidden();
        let to = AppearTransition::visible();

        // Check hidden state values
        assert!((from.opacity.0 - 0.0).abs() < f32::EPSILON);
        assert!((from.offset.y - 20.0).abs() < f32::EPSILON);

        // Check visible state values
        assert!((to.opacity.0 - 1.0).abs() < f32::EPSILON);
        assert!((to.offset.y - 0.0).abs() < f32::EPSILON);

        // At delta=0.5
        let result = from.lerp(&to, 0.5);
        assert!((result.opacity.0 - 0.5).abs() < f32::EPSILON);
        assert!((result.offset.y - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_appear_transition_visible_to_dismissed() {
        let from = AppearTransition::visible();
        let to = AppearTransition::dismissed();

        // At delta=1
        let result = from.lerp(&to, 1.0);
        assert!((result.opacity.0 - 0.0).abs() < f32::EPSILON);
        assert!((result.offset.y - (-10.0)).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // HoverState Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_hover_state_lerp() {
        let from = HoverState::normal();
        let to = HoverState::with_background(TransitionColor::from_hex_alpha(0xFFFFFF, 0.2));

        // At delta=0, should be transparent
        let result = from.lerp(&to, 0.0);
        assert!((result.background.a - 0.0).abs() < f32::EPSILON);

        // At delta=1, should be target color
        let result = from.lerp(&to, 1.0);
        assert!((result.background.a - 0.2).abs() < f32::EPSILON);
    }

    // -------------------------------------------------------------------------
    // Duration Constants Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_duration_ordering() {
        assert!(DURATION_FAST < DURATION_STANDARD);
        assert!(DURATION_STANDARD < DURATION_MEDIUM);
        assert!(DURATION_MEDIUM < DURATION_SLOW);
    }

    #[test]
    fn test_duration_values() {
        assert_eq!(DURATION_FAST.as_millis(), 100);
        assert_eq!(DURATION_STANDARD.as_millis(), 150);
        assert_eq!(DURATION_MEDIUM.as_millis(), 200);
        assert_eq!(DURATION_SLOW.as_millis(), 300);
    }

    // -------------------------------------------------------------------------
    // Primitive Lerp Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_f32_lerp() {
        assert!((0.0_f32.lerp(&1.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((0.0_f32.lerp(&1.0, 0.5) - 0.5).abs() < f32::EPSILON);
        assert!((0.0_f32.lerp(&1.0, 1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_f64_lerp() {
        assert!((0.0_f64.lerp(&1.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((0.0_f64.lerp(&1.0, 0.5) - 0.5).abs() < f64::EPSILON);
        assert!((0.0_f64.lerp(&1.0, 1.0) - 1.0).abs() < f64::EPSILON);
    }
}
