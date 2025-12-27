//! System tray icon management for Script Kit
//!
//! Provides a TrayManager that creates a macOS menu bar icon with a context menu.
//! The icon uses the Script Kit logo rendered as a template image for proper
//! light/dark mode adaptation.

use anyhow::{Context, Result};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuEventReceiver, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// SVG logo for Script Kit (32x32, monochrome)
/// This will be rendered as a template image on macOS for light/dark mode adaptation
const LOGO_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" fill="currentColor" viewBox="0 0 32 32">
  <path fill="currentColor" d="M14 25a2 2 0 0 1 2-2h14a2 2 0 1 1 0 4H16a2 2 0 0 1-2-2ZM0 7.381c0-1.796 1.983-2.884 3.498-1.92l13.728 8.736c1.406.895 1.406 2.946 0 3.84L3.498 26.775C1.983 27.738 0 26.649 0 24.854V7.38Z"/>
</svg>"#;

/// Menu item identifiers for matching events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    OpenScriptKit,
    Settings,
    Quit,
}

/// Manages the system tray icon and menu
pub struct TrayManager {
    #[allow(dead_code)]
    tray_icon: TrayIcon,
    open_script_kit_id: String,
    settings_id: String,
    quit_id: String,
}

impl TrayManager {
    /// Creates a new TrayManager with the Script Kit logo and menu
    ///
    /// # Errors
    /// Returns an error if:
    /// - SVG parsing fails
    /// - PNG rendering fails
    /// - Tray icon creation fails
    pub fn new() -> Result<Self> {
        let icon = Self::create_icon_from_svg()?;
        let (menu, open_id, settings_id, quit_id) = Self::create_menu()?;

        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Script Kit")
            .with_menu(Box::new(menu))
            .with_icon_as_template(true) // macOS: adapt to light/dark menu bar
            .build()
            .context("Failed to create tray icon")?;

        Ok(Self {
            tray_icon,
            open_script_kit_id: open_id,
            settings_id,
            quit_id,
        })
    }

    /// Converts the embedded SVG logo to a PNG icon
    fn create_icon_from_svg() -> Result<Icon> {
        // Parse SVG
        let opts = usvg::Options::default();
        let tree = usvg::Tree::from_str(LOGO_SVG, &opts).context("Failed to parse SVG")?;

        // Create pixmap for rendering (32x32)
        let size = tree.size();
        let width = size.width() as u32;
        let height = size.height() as u32;

        let mut pixmap =
            tiny_skia::Pixmap::new(width, height).context("Failed to create pixmap")?;

        // Render SVG to pixmap (white color for template image)
        // Fill with white since template images on macOS use the alpha channel
        // and the system will colorize based on menu bar appearance
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        // Convert to RGBA bytes
        let rgba = pixmap.take();

        // Create tray icon from RGBA data
        Icon::from_rgba(rgba, width, height).context("Failed to create icon from RGBA data")
    }

    /// Creates the tray menu with standard items
    fn create_menu() -> Result<(Menu, String, String, String)> {
        let menu = Menu::new();

        // Create menu items
        let open_item = MenuItem::new("Open Script Kit", true, None);
        let settings_item = MenuItem::new("Settings", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        // Store IDs for event matching
        let open_id = open_item.id().0.clone();
        let settings_id = settings_item.id().0.clone();
        let quit_id = quit_item.id().0.clone();

        // Add items to menu
        menu.append(&open_item)
            .context("Failed to add Open item")?;
        menu.append(&settings_item)
            .context("Failed to add Settings item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("Failed to add separator")?;
        menu.append(&quit_item)
            .context("Failed to add Quit item")?;

        Ok((menu, open_id, settings_id, quit_id))
    }

    /// Returns the menu event receiver for handling menu clicks
    ///
    /// Use this in your event loop to process menu events:
    /// ```ignore
    /// if let Ok(event) = tray_manager.menu_event_receiver().try_recv() {
    ///     match tray_manager.match_menu_event(&event) {
    ///         Some(TrayMenuAction::OpenScriptKit) => { /* handle */ }
    ///         Some(TrayMenuAction::Settings) => { /* handle */ }
    ///         Some(TrayMenuAction::Quit) => { /* handle */ }
    ///         None => { /* unknown menu item */ }
    ///     }
    /// }
    /// ```
    pub fn menu_event_receiver(&self) -> &MenuEventReceiver {
        MenuEvent::receiver()
    }

    /// Matches a menu event to a TrayMenuAction
    ///
    /// Returns `Some(action)` if the event matches a known menu item,
    /// or `None` if the event is from an unknown source.
    pub fn match_menu_event(&self, event: &MenuEvent) -> Option<TrayMenuAction> {
        let id = &event.id.0;
        if id == &self.open_script_kit_id {
            Some(TrayMenuAction::OpenScriptKit)
        } else if id == &self.settings_id {
            Some(TrayMenuAction::Settings)
        } else if id == &self.quit_id {
            Some(TrayMenuAction::Quit)
        } else {
            None
        }
    }

    /// Returns the menu item ID for "Open Script Kit"
    pub fn open_script_kit_id(&self) -> &str {
        &self.open_script_kit_id
    }

    /// Returns the menu item ID for "Settings"
    pub fn settings_id(&self) -> &str {
        &self.settings_id
    }

    /// Returns the menu item ID for "Quit"
    pub fn quit_id(&self) -> &str {
        &self.quit_id
    }
}
