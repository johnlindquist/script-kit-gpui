//! System tray icon management for Script Kit
//!
//! Provides a TrayManager that creates a macOS menu bar icon with a context menu.
//! The icon uses the Script Kit logo rendered as a template image for proper
//! light/dark mode adaptation.

// --- merged from part_000.rs ---
use crate::updates::UpdateState;
use anyhow::{bail, Context, Result};
use std::sync::LazyLock;
use std::sync::{Arc, RwLock};
use tray_icon::{
    menu::{
        accelerator::{Accelerator, Code, Modifiers},
        ContextMenu, IconMenuItem, MenuEvent, MenuEventReceiver, MenuItem, NativeIcon,
        PredefinedMenuItem, Submenu,
    },
    Icon, TrayIcon, TrayIconBuilder,
};

/// URLs the tray menu opens. Centralised so callers and tests stay in sync.
pub use crate::branding::{LOGO_SVG, URL_DISCORD, URL_FOLLOW_US, URL_GITHUB};
pub const URL_FEEDBACK: &str = "https://github.com/johnlindquist/script-kit-next/issues/new";
const TRAY_MENU_SCHEMA_VERSION: u32 = 1;
/// Displayed at 18pt by tray-icon/muda; 44px covers 2x with 3x headroom.
const TRAY_ICON_RENDER_PX: u32 = 44;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuObservation {
    pub schema_version: u32,
    pub source: &'static str,
    pub owner: TrayMenuOwnerObservation,
    pub sections: Vec<TrayMenuSectionObservation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuOwnerObservation {
    pub kind: &'static str,
    pub name: &'static str,
    pub scope: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuSectionObservation {
    pub id: &'static str,
    pub label: &'static str,
    pub items: Vec<TrayMenuItemObservation>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuItemObservation {
    pub id: &'static str,
    pub title: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<TrayMenuShortcutObservation>,
    pub title_source: &'static str,
    pub destination_kind: &'static str,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuShortcutObservation {
    pub display: Option<String>,
    pub source: &'static str,
}

/// Accelerators mirrored onto tray rows so the menu advertises the user's
/// actual global hotkeys instead of hiding them (launcher, Notes, Agent Chat).
#[derive(Clone, Default)]
pub struct TrayShortcuts {
    pub main: Option<Accelerator>,
    pub notes: Option<Accelerator>,
    pub agent_chat: Option<Accelerator>,
}

/// Which tray rows currently display a configured shortcut — kept alongside
/// the observation source so DevTools/MCP snapshots stay in lockstep with the
/// real menu.
#[derive(Clone, Copy, Default)]
pub(crate) struct TrayShortcutFlags {
    pub launcher: bool,
    pub notes: bool,
    pub agent_chat: bool,
}

impl TrayShortcuts {
    fn flags(&self) -> TrayShortcutFlags {
        TrayShortcutFlags {
            launcher: self.main.is_some(),
            notes: self.notes.is_some(),
            agent_chat: self.agent_chat.is_some(),
        }
    }
}

#[derive(Clone)]
struct TrayMenuObservationSource {
    update_state: Arc<RwLock<UpdateState>>,
    shortcut_flags: TrayShortcutFlags,
}

static TRAY_MENU_OBSERVATION_SOURCE: LazyLock<RwLock<Option<TrayMenuObservationSource>>> =
    LazyLock::new(|| RwLock::new(None));
/// Renders an SVG string to RGBA pixel data with validation.
///
/// # Arguments
/// * `svg` - The SVG string to render
/// * `width` - Target width in pixels
/// * `height` - Target height in pixels
///
/// # Errors
/// Returns an error if:
/// - SVG parsing fails
/// - Pixmap creation fails
/// - The rendered output is completely transparent (likely a rendering failure)
///
/// # Returns
/// RGBA pixel data as a `Vec<u8>` (length = width * height * 4)
fn render_svg_to_rgba(svg: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    // Parse SVG
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &opts).context("Failed to parse SVG")?;

    // Create pixmap for rendering
    let mut pixmap = tiny_skia::Pixmap::new(width, height).context("Failed to create pixmap")?;

    // Calculate scale to fit SVG into target dimensions
    let size = tree.size();
    let scale_x = width as f32 / size.width();
    let scale_y = height as f32 / size.height();
    let scale = scale_x.min(scale_y);

    let transform = tiny_skia::Transform::from_scale(scale, scale);

    // Render SVG to pixmap
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Take ownership of pixel data
    let rgba = pixmap.take();

    // Validate: check that at least some pixels have non-zero alpha
    // This catches "failed silently" scenarios where nothing was rendered
    let has_visible_content = rgba.as_chunks::<4>().0.iter().any(|px| px[3] != 0);
    if !has_visible_content {
        bail!(
            "SVG rendered to fully transparent image ({}x{}) - likely a rendering failure",
            width,
            height
        );
    }

    Ok(rgba)
}
/// Brand glyphs for the social-section rows. They live in `assets/icons/` and
/// are inlined at compile time so the running binary has no filesystem
/// dependency on its bundle layout.
const ICON_X_SVG: &str = include_str!("../../assets/icons/x_twitter.svg");
const ICON_GITHUB_SVG: &str = include_str!("../../assets/icons/github.svg");
const ICON_DISCORD_SVG: &str = include_str!("../../assets/icons/discord.svg");
const ICON_NOTES_SVG: &str = include_str!("../../assets/icons/notes.svg");
const ICON_AGENT_CHAT_SVG: &str = include_str!("../../assets/icons/agent_chat.svg");
const ICON_SETTINGS_SVG: &str = include_str!("../../assets/icons/settings.svg");
const ICON_INFO_SVG: &str = include_str!("../../assets/icons/info.svg");
const ICON_BOOK_OPEN_SVG: &str = include_str!("../../assets/icons/book_open.svg");
const ICON_SHARE_SVG: &str = include_str!("../../assets/icons/share.svg");
const ICON_REFRESH_CW_SVG: &str = include_str!("../../assets/icons/refresh_cw.svg");
const ICON_CIRCLE_ARROW_UP_SVG: &str = include_str!("../../assets/icons/circle_arrow_up.svg");

/// Walk a muda `ContextMenu`'s underlying `NSMenu` and mark every menu-item
/// image as a template, so AppKit auto-tints it for light/dark/highlighted/
/// disabled states. Without this the brand glyphs (X, GitHub, Discord, the
/// Script Kit logo) render as raw white pixels — fine in dark menus, invisible
/// in light menus.
///
/// Implementation per Oracle session `tray-template-tint-refactor`: copies the
/// `NSImage` before mutating to avoid templating any shared/native images, and
/// recurses into submenus. Returns the number of items whose images were
/// touched so the caller can log a graceful-degrade warning if AppKit is
/// returning nil for every row (indicates a muda upgrade swapped in custom
/// views or otherwise hid the image API).
///
/// Re-run this whenever `tray_icon.set_menu(...)` swaps a freshly-built menu.
#[cfg(target_os = "macos")]
fn template_menu_items(menu: &dyn ContextMenu) -> usize {
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let is_main_thread: bool = msg_send![class!(NSThread), isMainThread];
        if !is_main_thread {
            tracing::warn!("tray.template_menu_items_not_main_thread");
            return 0;
        }
        template_ns_menu(menu.ns_menu() as id)
    }
}

#[cfg(target_os = "macos")]
unsafe fn template_ns_menu(ns_menu: cocoa::base::id) -> usize {
    use cocoa::base::{id, nil, YES};
    use objc::{msg_send, sel, sel_impl};
    if ns_menu == nil {
        return 0;
    }
    let items: id = msg_send![ns_menu, itemArray];
    if items == nil {
        return 0;
    }
    let count: usize = msg_send![items, count];
    let mut templated = 0usize;
    for index in 0..count {
        let item: id = msg_send![items, objectAtIndex: index];
        if item == nil {
            continue;
        }
        let image: id = msg_send![item, image];
        if image != nil {
            let already_template: bool = msg_send![image, isTemplate];
            if !already_template {
                // Copy first: shared/native NSImages must not be mutated globally.
                let image_copy: id = msg_send![image, copy];
                if image_copy != nil {
                    let _: () = msg_send![image_copy, setTemplate: YES];
                    let _: () = msg_send![item, setImage: image_copy];
                    let _: () = msg_send![image_copy, release];
                }
            }
            templated += 1;
        }
        let submenu: id = msg_send![item, submenu];
        if submenu != nil {
            templated += template_ns_menu(submenu);
        }
    }
    templated
}

/// Render an SVG glyph at menu-row resolution and turn it into a `muda::Icon`
/// suitable for `IconMenuItem::with_id`. The shared render size avoids
/// upscaling in the native 18pt menu slot.
///
/// `currentColor` in the source SVG is rewritten to **white** before render
/// so the rendered bitmap has full alpha where the glyph lives. Once
/// `template_menu_items()` flips the `NSImage` to a template, AppKit ignores
/// colour and uses the alpha mask — so any opaque fill works, white is just
/// the simplest currentColor → solid swap. If templating ever fails (logged
/// as `tray.menu_item_template_noop`), the white pixels are the
/// graceful-degrade fallback.
fn menu_icon_from_svg(svg: &str) -> Result<tray_icon::menu::Icon> {
    let recolored = svg.replace("currentColor", "white");
    let rgba = render_svg_to_rgba(&recolored, TRAY_ICON_RENDER_PX, TRAY_ICON_RENDER_PX)
        .context("render menu icon svg")?;
    tray_icon::menu::Icon::from_rgba(rgba, TRAY_ICON_RENDER_PX, TRAY_ICON_RENDER_PX)
        .context("create muda Icon from menu glyph rgba")
}
/// Menu item identifiers for matching events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayMenuAction {
    OpenScriptKit,
    OpenCurrentAppCommands,
    OpenNotes,
    OpenAgentChat,
    Settings,
    ReloadScripts,
    CheckForUpdates,
    OpenReleasePage,
    SendFeedback,
    FollowUs,
    OpenGitHub,
    JoinDiscord,
    OpenAbout,
    Quit,
}
impl TrayMenuAction {
    /// Returns a stable string ID for this action.
    /// Used with `with_id()` when creating menu items.
    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenScriptKit => "tray.open_script_kit",
            Self::OpenCurrentAppCommands => "tray.open_current_app_commands",
            Self::OpenNotes => "tray.open_notes",
            Self::OpenAgentChat => "tray.open_agent_chat",
            Self::Settings => "tray.settings",
            Self::ReloadScripts => "tray.reload_scripts",
            Self::CheckForUpdates => "tray.check_for_updates",
            Self::OpenReleasePage => "tray.open_release_page",
            Self::SendFeedback => "tray.send_feedback",
            Self::FollowUs => "tray.follow_us",
            Self::OpenGitHub => "tray.open_github",
            Self::JoinDiscord => "tray.join_discord",
            Self::OpenAbout => "tray.open_about",
            Self::Quit => "tray.quit",
        }
    }

    /// Looks up a TrayMenuAction from its string ID.
    /// Returns None if the ID is not recognized.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "tray.open_script_kit" => Some(Self::OpenScriptKit),
            "tray.open_current_app_commands" => Some(Self::OpenCurrentAppCommands),
            "tray.open_notes" => Some(Self::OpenNotes),
            "tray.open_agent_chat" => Some(Self::OpenAgentChat),
            "tray.settings" => Some(Self::Settings),
            "tray.reload_scripts" => Some(Self::ReloadScripts),
            "tray.check_for_updates" => Some(Self::CheckForUpdates),
            "tray.open_release_page" => Some(Self::OpenReleasePage),
            "tray.send_feedback" => Some(Self::SendFeedback),
            "tray.follow_us" => Some(Self::FollowUs),
            "tray.open_github" => Some(Self::OpenGitHub),
            "tray.join_discord" => Some(Self::JoinDiscord),
            "tray.open_about" => Some(Self::OpenAbout),
            "tray.quit" => Some(Self::Quit),
            _ => None,
        }
    }

    /// Returns all TrayMenuAction variants for iteration.
    #[cfg(test)]
    pub const fn all() -> &'static [Self] {
        &[
            Self::OpenScriptKit,
            Self::OpenCurrentAppCommands,
            Self::OpenNotes,
            Self::OpenAgentChat,
            Self::Settings,
            Self::ReloadScripts,
            Self::CheckForUpdates,
            Self::OpenReleasePage,
            Self::SendFeedback,
            Self::FollowUs,
            Self::OpenGitHub,
            Self::JoinDiscord,
            Self::OpenAbout,
            Self::Quit,
        ]
    }
}
/// Manages the system tray icon and menu
pub struct TrayManager {
    #[allow(dead_code)]
    tray_icon: TrayIcon,
    /// The current-app commands row. Its label follows the last tracked
    /// frontmost real app, refreshed from the tray dispatcher.
    current_app_commands_item: IconMenuItem,
    /// The "Version X.Y.Z" / "Update Available: ..." row — text + enabled state
    /// flip when `update_state` transitions.
    version_item: MenuItem,
    /// Shared update-checker state. Tray reads it whenever the menu refreshes.
    update_state: Arc<RwLock<UpdateState>>,
}
impl TrayManager {
    /// Creates a new TrayManager with the Script Kit logo and menu.
    ///
    /// # Errors
    /// Returns an error if:
    /// - SVG parsing fails
    /// - PNG rendering fails
    /// - Tray icon creation fails
    pub fn new(update_state: Arc<RwLock<UpdateState>>, shortcuts: TrayShortcuts) -> Result<Self> {
        let icon = Self::create_icon_from_svg()?;
        let shortcut_flags = shortcuts.flags();
        let (menu, current_app_commands_item, version_item) =
            Self::create_menu(&update_state, &shortcuts)?;

        // Mark every menu-item image as an NSImage template BEFORE handing
        // the menu off to muda. AppKit then auto-tints the icons for
        // light/dark/highlighted/disabled. Without this, brand glyphs (X,
        // GitHub, Discord, SK logo) render as raw white pixels and disappear
        // in light-mode menus.
        #[cfg(target_os = "macos")]
        {
            let templated = template_menu_items(menu.as_ref());
            if templated == 0 {
                tracing::warn!("tray.menu_item_template_noop");
            } else {
                tracing::info!(templated, "tray.menu_item_icons_templated");
            }
        }

        let mut builder = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Script Kit")
            .with_menu(menu);

        // Template mode is macOS-only; adapts icon to light/dark menu bar
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }

        let tray_icon = builder.build().context("Failed to create tray icon")?;

        store_tray_menu_observation_source(Arc::clone(&update_state), shortcut_flags);

        Ok(Self {
            tray_icon,
            current_app_commands_item,
            version_item,
            update_state,
        })
    }

    /// Converts the embedded SVG logo to a tray icon.
    ///
    /// Uses `render_svg_to_rgba` for validated rendering.
    fn create_icon_from_svg() -> Result<Icon> {
        // Parse the SVG first so invalid logo data still fails before render.
        let opts = usvg::Options::default();
        usvg::Tree::from_str(LOGO_SVG, &opts).context("Failed to parse logo SVG")?;

        // Render with validation
        let rgba = render_svg_to_rgba(LOGO_SVG, TRAY_ICON_RENDER_PX, TRAY_ICON_RENDER_PX)
            .context("Failed to render tray logo SVG")?;

        // Create tray icon from RGBA data
        Icon::from_rgba(rgba, TRAY_ICON_RENDER_PX, TRAY_ICON_RENDER_PX)
            .context("Failed to create tray icon from RGBA data")
    }

    /// Creates the tray menu with standard items.
    ///
    /// Sections (Raycast-style):
    ///   • Open Script Kit (configured launcher hotkey) / Current App Commands… / Open Notes / Open Agent Chat
    ///   • Manual / Send Feedback…
    ///   • Follow Us / GitHub / Discord
    ///   • Settings ⌘, / Reload Scripts / Check for Updates… / Version (or Update Available) / About
    ///   • Quit Script Kit
    fn create_menu(
        update_state: &Arc<RwLock<UpdateState>>,
        shortcuts: &TrayShortcuts,
    ) -> Result<(Box<dyn ContextMenu>, IconMenuItem, MenuItem)> {
        let menu = Submenu::with_id("tray.root", "Script Kit", true);

        // Use the embedded Script Kit logo for the headline row. Falls back
        // to NativeIcon::Home if SVG rendering ever fails.
        let open_item = match menu_icon_from_svg(LOGO_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenScriptKit.id(),
                "Open Script Kit",
                true,
                Some(icon),
                shortcuts.main,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.open_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::OpenScriptKit.id(),
                    "Open Script Kit",
                    true,
                    Some(NativeIcon::Home),
                    shortcuts.main,
                )
            }
        };
        // Current App Commands — lucide book-open glyph; NativeIcon variants
        // render at their own intrinsic size and look thin next to the
        // menu_icon_from_svg rows, so only the fallback uses one.
        let open_current_app_commands_item = match menu_icon_from_svg(ICON_BOOK_OPEN_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenCurrentAppCommands.id(),
                current_app_commands_label(),
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.current_app_commands_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::OpenCurrentAppCommands.id(),
                    current_app_commands_label(),
                    true,
                    Some(NativeIcon::Bookmarks),
                    None,
                )
            }
        };
        // Open Notes — lucide notepad-text glyph, white-on-template via menu_icon_from_svg.
        let open_notes_item = match menu_icon_from_svg(ICON_NOTES_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenNotes.id(),
                "Open Notes",
                true,
                Some(icon),
                shortcuts.notes,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.notes_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::OpenNotes.id(),
                    "Open Notes",
                    true,
                    Some(NativeIcon::Bookmarks),
                    shortcuts.notes,
                )
            }
        };
        // Open Agent Chat — lucide bot-message-square glyph.
        let open_agent_chat_item = match menu_icon_from_svg(ICON_AGENT_CHAT_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenAgentChat.id(),
                "Open AI",
                true,
                Some(icon),
                shortcuts.agent_chat,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.agent_chat_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::OpenAgentChat.id(),
                    "Open AI",
                    true,
                    Some(NativeIcon::Bookmarks),
                    shortcuts.agent_chat,
                )
            }
        };

        // Send Feedback — lucide share glyph, sized like the other SVG rows.
        let feedback_item = match menu_icon_from_svg(ICON_SHARE_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::SendFeedback.id(),
                "Send Feedback…",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.feedback_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::SendFeedback.id(),
                    "Send Feedback…",
                    true,
                    Some(NativeIcon::Share),
                    None,
                )
            }
        };

        // Brand-correct social glyphs rendered from `assets/icons/`. Falling
        // back to `with_id_and_native_icon` keeps the row alive even if the
        // SVG render path errors out (e.g. asset corruption).
        let follow_us_item = match menu_icon_from_svg(ICON_X_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::FollowUs.id(),
                "Follow Us",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.follow_us_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::FollowUs.id(),
                    "Follow Us",
                    true,
                    Some(NativeIcon::User),
                    None,
                )
            }
        };
        let github_item = match menu_icon_from_svg(ICON_GITHUB_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenGitHub.id(),
                "GitHub",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.github_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::OpenGitHub.id(),
                    "GitHub",
                    true,
                    Some(NativeIcon::FollowLinkFreestanding),
                    None,
                )
            }
        };
        let discord_item = match menu_icon_from_svg(ICON_DISCORD_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::JoinDiscord.id(),
                "Discord",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.discord_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::JoinDiscord.id(),
                    "Discord",
                    true,
                    Some(NativeIcon::UserGroup),
                    None,
                )
            }
        };

        let settings_item = match menu_icon_from_svg(ICON_SETTINGS_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::Settings.id(),
                "Settings",
                true,
                Some(icon),
                Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.settings_icon_fallback");
                IconMenuItem::with_id(
                    TrayMenuAction::Settings.id(),
                    "Settings",
                    true,
                    None,
                    Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
                )
            }
        };
        // Reload Scripts — lucide refresh-cw glyph, sized like the other SVG rows.
        let reload_scripts_item = match menu_icon_from_svg(ICON_REFRESH_CW_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::ReloadScripts.id(),
                "Reload Scripts",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.reload_scripts_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::ReloadScripts.id(),
                    "Reload Scripts",
                    true,
                    Some(NativeIcon::Refresh),
                    None,
                )
            }
        };
        // Check for Updates — lucide circle-arrow-up glyph.
        let check_for_updates_item = match menu_icon_from_svg(ICON_CIRCLE_ARROW_UP_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::CheckForUpdates.id(),
                "Check for Updates…",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.check_for_updates_icon_fallback");
                IconMenuItem::with_id_and_native_icon(
                    TrayMenuAction::CheckForUpdates.id(),
                    "Check for Updates…",
                    true,
                    Some(NativeIcon::RefreshFreestanding),
                    None,
                )
            }
        };

        // Version row — id matches OpenReleasePage so AppKit dispatches a click
        // to the release page when an update is available. Label/enabled flip
        // via `refresh_version_label`.
        let version_snapshot = update_state
            .read()
            .map(|state| state.clone())
            .unwrap_or_else(|_| UpdateState::Error {
                message: "Update state unavailable".to_string(),
                failure: crate::updates::UpdateFailure::InvalidResponse,
            });
        let (version_label, version_enabled) = version_label_and_enabled(&version_snapshot);
        let version_item = MenuItem::with_id(
            TrayMenuAction::OpenReleasePage.id(),
            version_label,
            version_enabled,
            None,
        );

        let about_item = match menu_icon_from_svg(ICON_INFO_SVG) {
            Ok(icon) => IconMenuItem::with_id(
                TrayMenuAction::OpenAbout.id(),
                "About Script Kit",
                true,
                Some(icon),
                None,
            ),
            Err(e) => {
                tracing::warn!(error = %e, "tray.about_icon_fallback");
                IconMenuItem::with_id(
                    TrayMenuAction::OpenAbout.id(),
                    "About Script Kit",
                    true,
                    None,
                    None,
                )
            }
        };

        let quit_item = MenuItem::with_id(TrayMenuAction::Quit.id(), "Quit Script Kit", true, None);

        menu.append(&open_item).context("Failed to add Open item")?;
        menu.append(&open_current_app_commands_item)
            .context("Failed to add Current App Commands item")?;
        menu.append(&open_notes_item)
            .context("Failed to add Open Notes item")?;
        menu.append(&open_agent_chat_item)
            .context("Failed to add Open Agent Chat item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("Failed to add separator")?;
        menu.append(&feedback_item)
            .context("Failed to add Feedback item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("Failed to add separator")?;
        menu.append(&follow_us_item)
            .context("Failed to add Follow Us item")?;
        menu.append(&github_item)
            .context("Failed to add GitHub item")?;
        menu.append(&discord_item)
            .context("Failed to add Discord item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("Failed to add separator")?;
        menu.append(&settings_item)
            .context("Failed to add Settings item")?;
        menu.append(&reload_scripts_item)
            .context("Failed to add Reload Scripts item")?;
        menu.append(&check_for_updates_item)
            .context("Failed to add Check for Updates item")?;
        menu.append(&version_item)
            .context("Failed to add Version item")?;
        menu.append(&about_item)
            .context("Failed to add About item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("Failed to add separator")?;
        menu.append(&quit_item).context("Failed to add Quit item")?;
        tracing::info!(layout = "raycast_style_v2", "tray.menu_built");

        Ok((Box::new(menu), open_current_app_commands_item, version_item))
    }

    /// Returns the menu event receiver for handling menu clicks.
    pub fn menu_event_receiver(&self) -> &MenuEventReceiver {
        MenuEvent::receiver()
    }

    /// Converts a menu event to a `TrayMenuAction` (pure function).
    ///
    /// Returns `Some(action)` if the event matches a known menu item,
    /// or `None` if the event is from an unknown source.
    ///
    /// This is a pure function with no side effects - use `handle_action()`
    /// separately to perform the associated action.
    pub fn action_from_event(event: &MenuEvent) -> Option<TrayMenuAction> {
        TrayMenuAction::from_id(&event.id.0)
    }

    /// Handles any side effects for a menu action. Currently no tray action
    /// has side effects internal to `TrayManager` — Launch at Login was
    /// removed because it duplicates macOS System Settings → Login Items.
    ///
    /// # Errors
    /// Reserved for future side effects; never returns an error today.
    pub fn handle_action(&self, _action: TrayMenuAction) -> Result<()> {
        Ok(())
    }

    /// Refreshes the current-app commands row to match the frontmost-app
    /// tracker. The tray dispatcher calls this before handling every event so
    /// the next menu open sees the latest localized app name.
    pub fn refresh_current_app_label(&self) {
        self.current_app_commands_item
            .set_text(current_app_commands_label());
    }

    /// Re-render the Version row from the current `UpdateState`. Call after
    /// `updates::check_now` completes — flips the row to enabled "Update
    /// Available: vX.Y.Z" when a newer release is found.
    pub fn refresh_version_label(&self) {
        let snapshot = self.update_state_snapshot();
        let (label, enabled) = version_label_and_enabled(&snapshot);
        self.version_item.set_text(&label);
        self.version_item.set_enabled(enabled);
    }

    /// Snapshot of the shared update state. Callers use this to decide whether
    /// clicking the Version row should open the release page.
    pub fn update_state_snapshot(&self) -> UpdateState {
        self.update_state
            .read()
            .map(|state| state.clone())
            .unwrap_or_else(|_| UpdateState::Error {
                message: "Update state unavailable".to_string(),
                failure: crate::updates::UpdateFailure::InvalidResponse,
            })
    }

    /// Shared `Arc<RwLock<UpdateState>>` so the dispatcher can hand it to
    /// `updates::check_now`.
    pub fn update_state_handle(&self) -> Arc<RwLock<UpdateState>> {
        Arc::clone(&self.update_state)
    }

    pub fn observation_snapshot(&self) -> TrayMenuObservation {
        tray_menu_observation_snapshot(&self.update_state_snapshot(), self.shortcut_flags())
    }

    fn shortcut_flags(&self) -> TrayShortcutFlags {
        TRAY_MENU_OBSERVATION_SOURCE
            .read()
            .ok()
            .and_then(|source| source.as_ref().map(|source| source.shortcut_flags))
            .unwrap_or_default()
    }
}

fn store_tray_menu_observation_source(
    update_state: Arc<RwLock<UpdateState>>,
    shortcut_flags: TrayShortcutFlags,
) {
    if let Ok(mut source) = TRAY_MENU_OBSERVATION_SOURCE.write() {
        *source = Some(TrayMenuObservationSource {
            update_state,
            shortcut_flags,
        });
    }
}

pub fn current_tray_menu_observation_snapshot() -> TrayMenuObservation {
    let source = TRAY_MENU_OBSERVATION_SOURCE
        .read()
        .ok()
        .and_then(|source| source.clone());

    match source {
        Some(source) => {
            let update_state = source
                .update_state
                .read()
                .map(|state| state.clone())
                .unwrap_or_else(|_| UpdateState::Error {
                    message: "Update state unavailable".to_string(),
                    failure: crate::updates::UpdateFailure::InvalidResponse,
                });
            tray_menu_observation_snapshot(&update_state, source.shortcut_flags)
        }
        None => {
            let mut snapshot =
                tray_menu_observation_snapshot(&UpdateState::Idle, TrayShortcutFlags::default());
            snapshot
                .warnings
                .push("tray menu manager has not been initialized".to_string());
            snapshot
        }
    }
}

pub(crate) fn tray_menu_observation_snapshot(
    update_state: &UpdateState,
    shortcut_flags: TrayShortcutFlags,
) -> TrayMenuObservation {
    let (version_label, version_enabled) = version_label_and_enabled(update_state);

    TrayMenuObservation {
        schema_version: TRAY_MENU_SCHEMA_VERSION,
        source: "scriptKitTrayMenuModel",
        owner: TrayMenuOwnerObservation {
            kind: "scriptKitStatusItem",
            name: "Script Kit",
            scope: "ownTrayMenuOnly",
        },
        sections: vec![
            TrayMenuSectionObservation {
                id: "open",
                label: "Open",
                items: vec![
                    tray_menu_item(
                        TrayMenuAction::OpenScriptKit,
                        "Open Script Kit".to_string(),
                        true,
                        shortcut_flags
                            .launcher
                            .then_some(TrayMenuShortcutObservation {
                                display: None,
                                source: "configuredLauncherHotkey",
                            }),
                        "static",
                        "scriptKitWindow",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenCurrentAppCommands,
                        current_app_commands_label(),
                        true,
                        None,
                        "frontmostAppTracker",
                        "scriptKitWindow",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenNotes,
                        "Open Notes".to_string(),
                        true,
                        shortcut_flags.notes.then_some(TrayMenuShortcutObservation {
                            display: None,
                            source: "configuredNotesHotkey",
                        }),
                        "static",
                        "scriptKitWindow",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenAgentChat,
                        "Open AI".to_string(),
                        true,
                        shortcut_flags
                            .agent_chat
                            .then_some(TrayMenuShortcutObservation {
                                display: None,
                                source: "configuredAgentChatHotkey",
                            }),
                        "static",
                        "scriptKitWindow",
                    ),
                ],
            },
            TrayMenuSectionObservation {
                id: "help",
                label: "Help",
                items: vec![tray_menu_item(
                    TrayMenuAction::SendFeedback,
                    "Send Feedback…".to_string(),
                    true,
                    None,
                    "static",
                    "externalUrl",
                )],
            },
            TrayMenuSectionObservation {
                id: "social",
                label: "Social",
                items: vec![
                    tray_menu_item(
                        TrayMenuAction::FollowUs,
                        "Follow Us".to_string(),
                        true,
                        None,
                        "static",
                        "externalUrl",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenGitHub,
                        "GitHub".to_string(),
                        true,
                        None,
                        "static",
                        "externalUrl",
                    ),
                    tray_menu_item(
                        TrayMenuAction::JoinDiscord,
                        "Discord".to_string(),
                        true,
                        None,
                        "static",
                        "externalUrl",
                    ),
                ],
            },
            TrayMenuSectionObservation {
                id: "system",
                label: "System",
                items: vec![
                    tray_menu_item(
                        TrayMenuAction::Settings,
                        "Settings".to_string(),
                        true,
                        None,
                        "static",
                        "scriptKitWindow",
                    ),
                    tray_menu_item(
                        TrayMenuAction::ReloadScripts,
                        "Reload Scripts".to_string(),
                        true,
                        None,
                        "static",
                        "scriptKitRuntime",
                    ),
                    tray_menu_item(
                        TrayMenuAction::CheckForUpdates,
                        "Check for Updates…".to_string(),
                        true,
                        None,
                        "static",
                        "updateCheck",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenReleasePage,
                        version_label,
                        version_enabled,
                        None,
                        "updateState",
                        "externalUrl",
                    ),
                    tray_menu_item(
                        TrayMenuAction::OpenAbout,
                        "About Script Kit".to_string(),
                        true,
                        None,
                        "static",
                        "appInfo",
                    ),
                ],
            },
            TrayMenuSectionObservation {
                id: "exit",
                label: "Exit",
                items: vec![tray_menu_item(
                    TrayMenuAction::Quit,
                    "Quit Script Kit".to_string(),
                    true,
                    None,
                    "static",
                    "appLifecycle",
                )],
            },
        ],
        warnings: Vec::new(),
    }
}

fn tray_menu_item(
    action: TrayMenuAction,
    title: String,
    enabled: bool,
    shortcut: Option<TrayMenuShortcutObservation>,
    title_source: &'static str,
    destination_kind: &'static str,
) -> TrayMenuItemObservation {
    TrayMenuItemObservation {
        id: action.id(),
        title,
        enabled,
        shortcut,
        title_source,
        destination_kind,
    }
}

/// Convert the launcher's `HotkeyConfig` (modifiers + key) into a muda
/// `Accelerator` so the tray's "Open Script Kit" row mirrors the user's
/// global hotkey. Returns `None` if the config's key/modifier strings can't
/// be recognised — caller falls back to no key equivalent.
pub fn main_shortcut_accelerator(hk: &crate::config::HotkeyConfig) -> Option<Accelerator> {
    let mut mods = Modifiers::empty();
    for modifier in &hk.modifiers {
        match modifier.as_str() {
            "meta" | "cmd" | "command" => mods |= Modifiers::META,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            _ => return None,
        }
    }
    Some(Accelerator::new(Some(mods), hotkey_code(&hk.key)?))
}

fn hotkey_code(key: &str) -> Option<Code> {
    Some(match key {
        "KeyA" => Code::KeyA,
        "KeyB" => Code::KeyB,
        "KeyC" => Code::KeyC,
        "KeyD" => Code::KeyD,
        "KeyE" => Code::KeyE,
        "KeyF" => Code::KeyF,
        "KeyG" => Code::KeyG,
        "KeyH" => Code::KeyH,
        "KeyI" => Code::KeyI,
        "KeyJ" => Code::KeyJ,
        "KeyK" => Code::KeyK,
        "KeyL" => Code::KeyL,
        "KeyM" => Code::KeyM,
        "KeyN" => Code::KeyN,
        "KeyO" => Code::KeyO,
        "KeyP" => Code::KeyP,
        "KeyQ" => Code::KeyQ,
        "KeyR" => Code::KeyR,
        "KeyS" => Code::KeyS,
        "KeyT" => Code::KeyT,
        "KeyU" => Code::KeyU,
        "KeyV" => Code::KeyV,
        "KeyW" => Code::KeyW,
        "KeyX" => Code::KeyX,
        "KeyY" => Code::KeyY,
        "KeyZ" => Code::KeyZ,
        "Digit0" => Code::Digit0,
        "Digit1" => Code::Digit1,
        "Digit2" => Code::Digit2,
        "Digit3" => Code::Digit3,
        "Digit4" => Code::Digit4,
        "Digit5" => Code::Digit5,
        "Digit6" => Code::Digit6,
        "Digit7" => Code::Digit7,
        "Digit8" => Code::Digit8,
        "Digit9" => Code::Digit9,
        "Space" => Code::Space,
        "Enter" => Code::Enter,
        "Tab" => Code::Tab,
        "Escape" => Code::Escape,
        "Backspace" => Code::Backspace,
        "Delete" => Code::Delete,
        "ArrowUp" => Code::ArrowUp,
        "ArrowDown" => Code::ArrowDown,
        "ArrowLeft" => Code::ArrowLeft,
        "ArrowRight" => Code::ArrowRight,
        "Home" => Code::Home,
        "End" => Code::End,
        "PageUp" => Code::PageUp,
        "PageDown" => Code::PageDown,
        "Insert" => Code::Insert,
        "Semicolon" => Code::Semicolon,
        "Quote" => Code::Quote,
        "Comma" => Code::Comma,
        "Period" => Code::Period,
        "Slash" => Code::Slash,
        "Backslash" => Code::Backslash,
        "BracketLeft" => Code::BracketLeft,
        "BracketRight" => Code::BracketRight,
        "Minus" => Code::Minus,
        "Equal" => Code::Equal,
        "Backquote" => Code::Backquote,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        _ => return None,
    })
}

fn current_app_commands_label() -> String {
    crate::frontmost_app_tracker::get_last_real_app()
        .map(|app| format!("{} Commands", app.name))
        .unwrap_or_else(|| "Current App Commands…".to_string())
}

fn version_label_and_enabled(state: &UpdateState) -> (String, bool) {
    let current = env!("CARGO_PKG_VERSION");
    match state {
        UpdateState::Available { release } => {
            (format!("Update Available: v{}", release.version), true)
        }
        UpdateState::ReleaseNotReady {
            version, reason, ..
        } => (
            format!("Update v{version} not ready ({})", reason.label()),
            true,
        ),
        UpdateState::Checking { .. } => (format!("Checking for updates… (v{current})"), false),
        UpdateState::UpToDate => (format!("Version {current} — up to date"), false),
        UpdateState::Error { .. } => (format!("Version {current} (update check failed)"), false),
        UpdateState::Idle => (format!("Version {current}"), false),
    }
}

// --- merged from part_001.rs ---
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_menu_action_id_roundtrip() {
        // Every action should roundtrip through id() and from_id()
        for action in TrayMenuAction::all() {
            let id = action.id();
            let recovered = TrayMenuAction::from_id(id);
            assert_eq!(
                recovered,
                Some(*action),
                "Action {:?} with id '{}' should roundtrip",
                action,
                id
            );
        }
    }

    #[test]
    fn test_tray_menu_action_ids_are_unique() {
        let all = TrayMenuAction::all();
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        a.id(),
                        b.id(),
                        "Actions {:?} and {:?} have duplicate IDs",
                        a,
                        b
                    );
                }
            }
        }
    }

    #[test]
    fn test_tray_menu_action_ids_are_prefixed() {
        // All IDs should start with "tray." for namespacing
        for action in TrayMenuAction::all() {
            assert!(
                action.id().starts_with("tray."),
                "Action {:?} ID '{}' should start with 'tray.'",
                action,
                action.id()
            );
        }
    }

    #[test]
    fn test_tray_menu_action_from_id_unknown() {
        assert_eq!(TrayMenuAction::from_id("unknown"), None);
        assert_eq!(TrayMenuAction::from_id(""), None);
        assert_eq!(TrayMenuAction::from_id("tray.nonexistent"), None);
    }

    /// Every menu row ships an SVG glyph rendered via `menu_icon_from_svg`;
    /// a render failure silently falls back to an AppKit NativeIcon whose
    /// intrinsic size doesn't match the 44px SVG rows (the 2026-07-11 tray
    /// icon size mismatch). Keep every glyph renderable so the fallback
    /// never fires.
    #[test]
    fn tray_menu_svg_glyphs_all_render() {
        for (name, svg) in [
            ("logo", LOGO_SVG),
            ("book_open", ICON_BOOK_OPEN_SVG),
            ("notes", ICON_NOTES_SVG),
            ("agent_chat", ICON_AGENT_CHAT_SVG),
            ("share", ICON_SHARE_SVG),
            ("x_twitter", ICON_X_SVG),
            ("github", ICON_GITHUB_SVG),
            ("discord", ICON_DISCORD_SVG),
            ("settings", ICON_SETTINGS_SVG),
            ("refresh_cw", ICON_REFRESH_CW_SVG),
            ("circle_arrow_up", ICON_CIRCLE_ARROW_UP_SVG),
            ("info", ICON_INFO_SVG),
        ] {
            menu_icon_from_svg(svg)
                .unwrap_or_else(|e| panic!("tray glyph '{name}' failed to render: {e}"));
        }
    }

    #[test]
    fn test_tray_menu_action_all_count() {
        // Verify all() returns all variants — bump when adding TrayMenuAction variants.
        assert_eq!(TrayMenuAction::all().len(), 14);
    }

    fn observed_items(snapshot: &TrayMenuObservation) -> Vec<&TrayMenuItemObservation> {
        snapshot
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .collect()
    }

    #[test]
    fn tray_menu_observation_contains_all_tray_actions() {
        let snapshot = tray_menu_observation_snapshot(
            &UpdateState::Idle,
            TrayShortcutFlags {
                launcher: true,
                notes: true,
                agent_chat: true,
            },
        );
        let ids: Vec<&str> = observed_items(&snapshot)
            .into_iter()
            .map(|item| item.id)
            .collect();

        for action in TrayMenuAction::all() {
            assert!(
                ids.contains(&action.id()),
                "tray observation missing {}",
                action.id()
            );
        }
        assert_eq!(ids.len(), TrayMenuAction::all().len());
    }

    #[test]
    fn tray_menu_observation_sections_match_create_menu_order() {
        let snapshot =
            tray_menu_observation_snapshot(&UpdateState::Idle, TrayShortcutFlags::default());
        let section_ids: Vec<&str> = snapshot.sections.iter().map(|section| section.id).collect();
        assert_eq!(
            section_ids,
            vec!["open", "help", "social", "system", "exit"]
        );

        let ids: Vec<&str> = observed_items(&snapshot)
            .into_iter()
            .map(|item| item.id)
            .collect();
        assert_eq!(
            ids,
            vec![
                TrayMenuAction::OpenScriptKit.id(),
                TrayMenuAction::OpenCurrentAppCommands.id(),
                TrayMenuAction::OpenNotes.id(),
                TrayMenuAction::OpenAgentChat.id(),
                TrayMenuAction::SendFeedback.id(),
                TrayMenuAction::FollowUs.id(),
                TrayMenuAction::OpenGitHub.id(),
                TrayMenuAction::JoinDiscord.id(),
                TrayMenuAction::Settings.id(),
                TrayMenuAction::ReloadScripts.id(),
                TrayMenuAction::CheckForUpdates.id(),
                TrayMenuAction::OpenReleasePage.id(),
                TrayMenuAction::OpenAbout.id(),
                TrayMenuAction::Quit.id(),
            ]
        );
    }

    #[test]
    fn tray_menu_observation_ids_are_unique() {
        let snapshot =
            tray_menu_observation_snapshot(&UpdateState::Idle, TrayShortcutFlags::default());
        let ids: Vec<&str> = observed_items(&snapshot)
            .into_iter()
            .map(|item| item.id)
            .collect();

        for (index, id) in ids.iter().enumerate() {
            assert!(
                !ids.iter()
                    .enumerate()
                    .any(|(other_index, other)| other_index != index && other == id),
                "duplicate tray observation id: {id}"
            );
        }
    }

    #[test]
    fn tray_menu_observation_current_app_title_uses_frontmost_tracker_fallback() {
        let snapshot =
            tray_menu_observation_snapshot(&UpdateState::Idle, TrayShortcutFlags::default());
        let current_app = observed_items(&snapshot)
            .into_iter()
            .find(|item| item.id == TrayMenuAction::OpenCurrentAppCommands.id())
            .expect("current app commands row");

        assert!(!current_app.title.trim().is_empty());
        assert_eq!(current_app.title_source, "frontmostAppTracker");
    }

    #[test]
    fn tray_menu_observation_version_row_reflects_update_state() {
        let snapshot = tray_menu_observation_snapshot(
            &UpdateState::Available {
                release: crate::updates::VerifiedRelease {
                    version: semver::Version::parse("9.9.9").unwrap(),
                    tag: "v9.9.9".to_string(),
                    release_page_url: "https://example.com/release".to_string(),
                    manifest_url: "https://example.com/release-manifest.json".to_string(),
                    artifact: crate::updates::VerifiedArtifact {
                        name: "Script-Kit-macos.zip".to_string(),
                        download_url: "https://example.com/Script-Kit-macos.zip".to_string(),
                        size_bytes: Some(123),
                        sha256: "022689519147819b7eb0ef2dba102e03677e53eb550eddd5f3b10f78aa5d3427"
                            .to_string(),
                        github_digest: None,
                    },
                },
            },
            TrayShortcutFlags::default(),
        );
        let version = observed_items(&snapshot)
            .into_iter()
            .find(|item| item.id == TrayMenuAction::OpenReleasePage.id())
            .expect("version row");

        assert_eq!(version.title, "Update Available: v9.9.9");
        assert!(version.enabled);
        assert_eq!(version.title_source, "updateState");
    }

    #[test]
    fn tray_menu_observation_has_no_click_or_execute_fields() {
        let snapshot = tray_menu_observation_snapshot(
            &UpdateState::Idle,
            TrayShortcutFlags {
                launcher: true,
                notes: true,
                agent_chat: true,
            },
        );
        let text = serde_json::to_string(&snapshot).expect("serialize tray observation");

        for forbidden in ["\"click\"", "\"execute\"", "\"action\"", "\"event\""] {
            assert!(
                !text.contains(forbidden),
                "tray observation must not expose executable fields; found {forbidden}"
            );
        }
    }

    // ========================================================================
    // SVG rendering tests
    // ========================================================================

    #[test]
    fn test_render_svg_to_rgba_valid_svg() {
        // A simple valid SVG with visible content
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
            <rect x="0" y="0" width="16" height="16" fill="white"/>
        </svg>"#;

        let result = render_svg_to_rgba(svg, 16, 16);
        assert!(result.is_ok(), "Valid SVG should render: {:?}", result);

        let rgba = result.unwrap();
        assert_eq!(
            rgba.len(),
            16 * 16 * 4,
            "RGBA data should be width*height*4 bytes"
        );
    }

    #[test]
    fn test_render_svg_to_rgba_invalid_svg() {
        let svg = "not valid svg at all";

        let result = render_svg_to_rgba(svg, 16, 16);
        assert!(result.is_err(), "Invalid SVG should fail");
        assert!(
            result.unwrap_err().to_string().contains("parse"),
            "Error should mention parsing"
        );
    }

    #[test]
    fn test_render_svg_to_rgba_empty_svg() {
        // An SVG with no visible content (all transparent)
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"></svg>"#;

        let result = render_svg_to_rgba(svg, 16, 16);
        assert!(result.is_err(), "Empty SVG should fail validation");
        assert!(
            result.unwrap_err().to_string().contains("transparent"),
            "Error should mention transparency"
        );
    }

    #[test]
    fn test_render_svg_to_rgba_logo_renders() {
        // Test that our actual logo SVG renders successfully
        let result = render_svg_to_rgba(LOGO_SVG, 32, 32);
        assert!(result.is_ok(), "Logo SVG should render: {:?}", result);
    }

    #[test]
    fn test_create_menu_uses_native_menu_icons() {
        // Only template-style NativeIcon variants are wired up. Manual
        // intentionally has NO icon because the AppKit equivalents
        // render as full-colour status images that clash with a Raycast-style menu.
        // Settings and About use custom SVG templates.
        // Follow Us / GitHub / Discord ship brand-correct SVG glyphs and
        // only fall back to NativeIcon if the SVG render fails.
        const TRAY_SOURCE: &str = include_str!("mod.rs");
        let implementation_source = TRAY_SOURCE
            .split("// --- merged from part_001.rs ---")
            .next()
            .unwrap_or(TRAY_SOURCE);
        for native_icon in [
            "NativeIcon::Home",                   // Open Script Kit fallback
            "NativeIcon::Bookmarks",              // Current App Commands
            "NativeIcon::Share",                  // Send Feedback
            "NativeIcon::User",                   // Follow Us fallback
            "NativeIcon::FollowLinkFreestanding", // GitHub fallback
            "NativeIcon::UserGroup",              // Discord fallback
            "NativeIcon::Refresh",                // Reload Scripts
            "NativeIcon::RefreshFreestanding",    // Check for Updates
        ] {
            assert!(
                implementation_source.contains(native_icon),
                "Tray menu should use {native_icon}"
            );
        }
        // Affirm the deliberate exclusions stay excluded.
        for forbidden in [
            "NativeIcon::Info",
            "NativeIcon::PreferencesGeneral",
            "NativeIcon::StopProgress",
        ] {
            assert!(
                !implementation_source.contains(forbidden),
                "Tray menu must not use {forbidden} (renders full-colour, not template)"
            );
        }
    }

    #[test]
    fn test_brand_icons_render() {
        // The Script Kit logo + X / GitHub / Discord glyphs must all
        // round-trip through SVG → RGBA → muda::Icon without falling back
        // to NativeIcon at runtime.
        for (label, svg) in [
            ("logo", LOGO_SVG),
            ("x_twitter", ICON_X_SVG),
            ("github", ICON_GITHUB_SVG),
            ("discord", ICON_DISCORD_SVG),
            ("notes", ICON_NOTES_SVG),
            ("agent_chat", ICON_AGENT_CHAT_SVG),
            ("settings", ICON_SETTINGS_SVG),
            ("info", ICON_INFO_SVG),
        ] {
            let icon = menu_icon_from_svg(svg);
            assert!(
                icon.is_ok(),
                "{label} brand icon failed to render: {icon:?}"
            );
        }
    }

    #[test]
    fn test_main_shortcut_accelerator_default() {
        use crate::config::HotkeyConfig;
        // Default launcher hotkey is meta+Semicolon. The accelerator must
        // parse, so the tray Open row shows the user's real shortcut.
        let acc = main_shortcut_accelerator(&HotkeyConfig {
            modifiers: vec!["meta".into()],
            key: "Semicolon".into(),
        });
        assert!(acc.is_some(), "default meta+Semicolon must convert");

        for (modifiers, key, label) in [
            (vec!["meta"], "Semicolon", "meta+Semicolon"),
            (vec!["meta"], "Space", "meta+Space"),
            (vec!["meta", "shift"], "KeyK", "meta+shift+KeyK"),
            (vec!["meta"], "Comma", "meta+Comma"),
            (vec!["meta"], "Slash", "meta+Slash"),
        ] {
            let acc = main_shortcut_accelerator(&HotkeyConfig {
                modifiers: modifiers.into_iter().map(str::to_string).collect(),
                key: key.into(),
            });
            assert!(acc.is_some(), "{label} must convert");
        }

        // Unknown modifier returns None instead of silently dropping it.
        let acc_bad = main_shortcut_accelerator(&HotkeyConfig {
            modifiers: vec!["hyper".into()],
            key: "Space".into(),
        });
        assert!(
            acc_bad.is_none(),
            "unknown modifier must not silently parse"
        );
    }

    #[test]
    fn test_tray_urls_are_https_and_pinned() {
        for url in [URL_FOLLOW_US, URL_GITHUB, URL_DISCORD, URL_FEEDBACK] {
            assert!(url.starts_with("https://"), "tray URL must be https: {url}");
        }
        assert!(URL_FOLLOW_US.contains("scriptkitapp"));
        assert!(URL_GITHUB.contains("github.com/johnlindquist/script-kit-next"));
        assert!(URL_FEEDBACK.contains("github.com/johnlindquist/script-kit-next/issues/new"));
    }
}
