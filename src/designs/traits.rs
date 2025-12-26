#![allow(dead_code)]
//! Design Renderer Trait
//!
//! This module defines the trait that all design variants must implement.
//! The trait provides a consistent interface for rendering the script list
//! while allowing each design to have its own unique visual style.

use gpui::*;

use super::DesignVariant;

/// Trait for design renderers
///
/// Each design variant implements this trait to provide its own rendering
/// of the script list UI. The trait is designed to work with GPUI's
/// component model and follows the existing patterns in the codebase.
///
/// # Type Parameters
///
/// * `App` - The application type that this renderer works with
///
/// # Implementation Notes
///
/// - Use `AnyElement` as the return type to allow flexible element trees
/// - Access app state through the provided app reference
/// - Follow the project's theme system
/// - Use `LIST_ITEM_HEIGHT` (52.0) for consistent item sizing
pub trait DesignRenderer<App>: Send + Sync {
    /// Render the script list in this design's style
    ///
    /// This method should return a complete script list UI element
    /// that can be composed into the main application view.
    ///
    /// # Arguments
    ///
    /// * `app` - Reference to the app for accessing state
    /// * `cx` - GPUI context for creating elements and handling events
    ///
    /// # Returns
    ///
    /// An `AnyElement` containing the rendered script list.
    fn render_script_list(
        &self,
        app: &App,
        cx: &mut Context<App>,
    ) -> AnyElement;

    /// Get the variant this renderer implements
    fn variant(&self) -> DesignVariant;

    /// Get the display name for this design
    fn name(&self) -> &'static str {
        self.variant().name()
    }

    /// Get a description of this design
    fn description(&self) -> &'static str {
        self.variant().description()
    }
}

/// Type alias for boxed design renderers
///
/// Use this when storing or passing design renderers as trait objects.
pub type DesignRendererBox<App> = Box<dyn DesignRenderer<App>>;
