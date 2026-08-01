//! Type definitions for UnifiedListItem.

// Allow dead_code - this is new code not yet integrated into the main app
#![allow(dead_code)]

use gpui::*;
use std::ops::Range;
use std::sync::Arc;

/// A precomputed text segment for highlighted content rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightFragment {
    pub text: SharedString,
    pub is_highlighted: bool,
}

// =============================================================================
// TextContent - Title/Subtitle with optional highlight ranges
// =============================================================================

/// Text content that can be plain or have highlight ranges (for fuzzy match display).
///
/// Ranges are **byte offsets** into the text. The fuzzy matcher must return valid
/// UTF-8 boundaries. In debug builds, we assert that ranges land on char boundaries.
///
/// Note: Custom variant prevents Clone; build fresh per render (normal for virtualized lists).
pub enum TextContent {
    /// Plain text with no highlighting.
    Plain(SharedString),

    /// Text with highlighted ranges (e.g., fuzzy match results).
    Highlighted {
        text: SharedString,
        ranges: Vec<Range<usize>>,
        fragments: Arc<[HighlightFragment]>,
    },

    /// Custom element (for special rendering needs).
    Custom(AnyElement),
}

impl TextContent {
    /// Create plain text content.
    pub fn plain(text: impl Into<SharedString>) -> Self {
        Self::Plain(text.into())
    }

    /// Create highlighted text content with byte ranges.
    pub fn highlighted(text: impl Into<SharedString>, ranges: Vec<Range<usize>>) -> Self {
        let text = text.into();
        let fragments = build_highlight_fragments(text.as_ref(), &ranges);

        #[cfg(debug_assertions)]
        {
            let s = text.as_ref();
            for range in &ranges {
                assert!(
                    s.is_char_boundary(range.start),
                    "Range start {} is not a char boundary in '{}'",
                    range.start,
                    s
                );
                assert!(
                    s.is_char_boundary(range.end),
                    "Range end {} is not a char boundary in '{}'",
                    range.end,
                    s
                );
            }
        }

        Self::Highlighted {
            text,
            ranges,
            fragments: fragments.into(),
        }
    }

    /// Create custom element content.
    pub fn custom(element: impl IntoElement) -> Self {
        Self::Custom(element.into_any_element())
    }

    /// Get the text string (for a11y labels). Returns None for Custom.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Plain(s) => Some(s.as_ref()),
            Self::Highlighted { text, .. } => Some(text.as_ref()),
            Self::Custom(_) => None,
        }
    }

    /// Get precomputed highlight fragments for render hot-path reuse.
    pub(crate) fn highlight_fragments(&self) -> Option<&[HighlightFragment]> {
        match self {
            Self::Highlighted { fragments, .. } => Some(fragments.as_ref()),
            _ => None,
        }
    }
}

fn build_highlight_fragments(text: &str, ranges: &[Range<usize>]) -> Vec<HighlightFragment> {
    if ranges.is_empty() {
        return vec![HighlightFragment {
            text: text.to_string().into(),
            is_highlighted: false,
        }];
    }

    let mut fragments = Vec::with_capacity(ranges.len().saturating_mul(2).saturating_add(1));
    let mut current_byte = 0;

    for range in ranges {
        if range.start > current_byte && range.start <= text.len() {
            let slice = &text[current_byte..range.start];
            if !slice.is_empty() {
                fragments.push(HighlightFragment {
                    text: slice.to_string().into(),
                    is_highlighted: false,
                });
            }
        }

        if range.end > range.start && range.start < text.len() && range.end <= text.len() {
            let slice = &text[range.start..range.end];
            if !slice.is_empty() {
                fragments.push(HighlightFragment {
                    text: slice.to_string().into(),
                    is_highlighted: true,
                });
            }
        }

        current_byte = range.end;
    }

    if current_byte < text.len() {
        let slice = &text[current_byte..];
        if !slice.is_empty() {
            fragments.push(HighlightFragment {
                text: slice.to_string().into(),
                is_highlighted: false,
            });
        }
    }

    fragments
}

// =============================================================================
// ItemState - Visual state (passed in, not owned)
// =============================================================================

/// Visual state for a list item.
#[derive(Clone, Copy, Default)]
pub struct ItemState {
    pub is_selected: bool,
    pub is_hovered: bool,
    pub is_disabled: bool,
}

// =============================================================================
// LeadingContent - Left-side content
// =============================================================================

/// Content displayed on the left side of the list item.
///
/// Note: Custom variant prevents Clone; use standard variants when possible.
pub enum LeadingContent {
    /// Emoji string (e.g., "📋").
    Emoji(SharedString),

    /// SVG icon by name with optional color override.
    Icon {
        name: SharedString,
        color: Option<u32>,
    },

    /// Pre-decoded app icon image.
    AppIcon(Arc<RenderImage>),

    /// Placeholder while app icon loads.
    AppIconPlaceholder,

    /// Custom element (use sparingly).
    Custom(AnyElement),
}

// =============================================================================
// TrailingContent - Right-side content
// =============================================================================

/// Content displayed on the right side of the list item.
///
/// Note: Custom variant prevents Clone; use standard variants when possible.
pub enum TrailingContent {
    /// Keyboard shortcut badge with tokens cached at model construction.
    Shortcut {
        raw: SharedString,
        tokens: Arc<[String]>,
    },

    /// Navigation hint (e.g., "Enter").
    Hint(SharedString),

    /// Item count badge.
    Count(usize),

    /// Right chevron for navigation.
    Chevron,

    /// Checkmark for selected items.
    Checkmark,

    /// Custom element (use sparingly).
    Custom(AnyElement),
}

impl TrailingContent {
    pub fn shortcut(shortcut: impl Into<SharedString>) -> Self {
        let raw = shortcut.into();
        let tokens = crate::components::hint_strip::shortcut_tokens_from_hint(raw.as_ref());
        Self::Shortcut {
            raw,
            tokens: tokens.into(),
        }
    }
}

// =============================================================================
// Density - Single knob for layout sizing
// =============================================================================

/// Layout density for list items.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Density {
    /// Comfortable spacing (canonical list row height).
    #[default]
    Comfortable,

    /// Compact spacing (canonical list row height with tighter internals).
    Compact,
}

// =============================================================================
// ListItemLayout - Computed layout values
// =============================================================================

/// Pre-computed layout values based on density.
#[derive(Clone, Copy)]
pub struct ListItemLayout {
    pub height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub gap: f32,
    pub leading_size: f32,
    pub radius: f32,
}

impl ListItemLayout {
    /// Compute layout from density.
    pub fn from_density(density: Density) -> Self {
        let canonical_height = crate::list_item::LIST_ITEM_HEIGHT;
        match density {
            Density::Comfortable => Self {
                height: canonical_height,
                padding_x: 12.0,
                padding_y: 6.0,
                gap: 8.0,
                leading_size: 20.0,
                radius: 6.0,
            },
            Density::Compact => Self {
                height: canonical_height,
                padding_x: 8.0,
                padding_y: 4.0,
                gap: 6.0,
                leading_size: 16.0,
                radius: 4.0,
            },
        }
    }
}

// =============================================================================
// UnifiedListItemColors - Pre-computed colors
// =============================================================================

/// Pre-computed colors for UnifiedListItem rendering.
#[derive(Clone, Copy)]
pub struct UnifiedListItemColors {
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_dimmed: u32,
    pub text_highlight: u32,
    pub accent: u32,
    pub accent_subtle: u32,
    pub background: u32,
    pub selected_opacity: f32,
    pub hover_opacity: f32,
}

impl Default for UnifiedListItemColors {
    fn default() -> Self {
        Self::from_theme(&crate::theme::get_cached_theme())
    }
}

impl UnifiedListItemColors {
    /// Create from theme reference.
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        let opacity = theme.get_opacity();
        Self {
            text_primary: theme.colors.text.primary,
            text_secondary: theme.colors.text.secondary,
            text_muted: theme.colors.text.muted,
            text_dimmed: theme.colors.text.dimmed,
            text_highlight: theme.colors.accent.selected,
            accent: theme.colors.accent.selected,
            accent_subtle: theme.colors.accent.selected_subtle,
            background: theme.colors.background.main,
            selected_opacity: opacity.selected,
            hover_opacity: opacity.hover,
        }
    }

    pub(crate) fn row_state_palette(self) -> crate::theme::RowStatePalette {
        use crate::theme::{resolve_row_state_palette, RowForegroundColors, RowStateColorInputs};

        let opaque = |hex: u32| (hex << 8) | 0xFF;
        let background = |opacity: f32| {
            Some((self.accent_subtle << 8) | u32::from((opacity.clamp(0.0, 1.0) * 255.0) as u8))
        };
        let rest_foregrounds = RowForegroundColors {
            primary_rgba: opaque(self.text_primary),
            secondary_rgba: opaque(self.text_muted),
            icon_rgba: opaque(self.text_secondary),
            accessory_rgba: opaque(self.text_dimmed),
        };
        let selected_foregrounds = RowForegroundColors {
            primary_rgba: opaque(self.text_primary),
            secondary_rgba: opaque(self.text_muted),
            icon_rgba: opaque(self.text_primary),
            accessory_rgba: opaque(self.text_dimmed),
        };
        let disabled_foregrounds = RowForegroundColors {
            primary_rgba: opaque(self.text_dimmed),
            secondary_rgba: opaque(self.text_muted),
            icon_rgba: opaque(self.text_secondary),
            accessory_rgba: opaque(self.text_dimmed),
        };
        let disabled_selected_foregrounds = RowForegroundColors {
            primary_rgba: opaque(self.text_dimmed),
            secondary_rgba: opaque(self.text_muted),
            // Preserve the existing selected-location icon emphasis even when
            // the row itself is disabled; activation remains a separate model.
            icon_rgba: opaque(self.text_primary),
            accessory_rgba: opaque(self.text_dimmed),
        };

        resolve_row_state_palette(RowStateColorInputs {
            rest_background_rgba: None,
            hover_background_rgba: background(self.hover_opacity),
            selected_background_rgba: background(self.selected_opacity),
            active_background_rgba: background(self.selected_opacity),
            rest_foregrounds,
            selected_foregrounds,
            disabled_foregrounds,
            disabled_selected_foregrounds,
        })
    }
}

/// Height for section headers in grouped lists.
pub const SECTION_HEADER_HEIGHT: f32 = crate::list_item::SECTION_HEADER_HEIGHT;

#[cfg(test)]
mod tests {
    use super::UnifiedListItemColors;
    use crate::theme::RowStateFlags;

    #[test]
    fn test_unified_list_item_colors_default_uses_cached_theme_tokens() {
        let resolved = UnifiedListItemColors::default();
        let expected = UnifiedListItemColors::from_theme(&crate::theme::get_cached_theme());

        assert_eq!(resolved.text_primary, expected.text_primary);
        assert_eq!(resolved.text_secondary, expected.text_secondary);
        assert_eq!(resolved.text_muted, expected.text_muted);
        assert_eq!(resolved.text_dimmed, expected.text_dimmed);
        assert_eq!(resolved.text_highlight, expected.text_highlight);
        assert_eq!(resolved.accent, expected.accent);
        assert_eq!(resolved.accent_subtle, expected.accent_subtle);
        assert_eq!(resolved.background, expected.background);
        assert_eq!(resolved.selected_opacity, expected.selected_opacity);
        assert_eq!(resolved.hover_opacity, expected.hover_opacity);
    }

    #[test]
    fn unified_row_palette_preserves_legacy_state_bytes() {
        let palette = UnifiedListItemColors {
            text_primary: 0x112233,
            text_secondary: 0x223344,
            text_muted: 0x334455,
            text_dimmed: 0x445566,
            text_highlight: 0x556677,
            accent: 0x667788,
            accent_subtle: 0x778899,
            background: 0x010203,
            selected_opacity: 0.23,
            hover_opacity: 0.06,
        }
        .row_state_palette();

        assert_eq!(palette.rest.background_rgba, None);
        assert_eq!(palette.hovered.background_rgba, Some(0x7788990F));
        assert_eq!(palette.selected.background_rgba, Some(0x7788993A));
        assert_eq!(palette.active, palette.selected);
        assert_eq!(palette.rest.primary_foreground_rgba, 0x112233FF);
        assert_eq!(palette.rest.secondary_foreground_rgba, 0x334455FF);
        assert_eq!(palette.rest.icon_foreground_rgba, 0x223344FF);
        assert_eq!(palette.rest.accessory_foreground_rgba, 0x445566FF);
        assert_eq!(palette.selected.icon_foreground_rgba, 0x112233FF);
        assert_eq!(palette.disabled.primary_foreground_rgba, 0x445566FF);
        assert_eq!(
            palette.disabled_selected.icon_foreground_rgba, 0x112233FF,
            "selected-disabled icon location feedback is preserved"
        );
    }

    #[test]
    fn unified_dark_and_light_palettes_follow_theme_bytes_and_state_order() {
        for theme in [
            crate::theme::Theme::dark_default(),
            crate::theme::Theme::light_default(),
        ] {
            let colors = UnifiedListItemColors::from_theme(&theme);
            let palette = colors.row_state_palette();
            let opacity = theme.get_opacity();
            let hover_alpha = (opacity.hover * 255.0) as u32;
            let selected_alpha = (opacity.selected * 255.0) as u32;

            assert_eq!(
                palette.hovered.background_rgba,
                Some((theme.colors.accent.selected_subtle << 8) | hover_alpha)
            );
            assert_eq!(
                palette.selected.background_rgba,
                Some((theme.colors.accent.selected_subtle << 8) | selected_alpha)
            );
            assert!(selected_alpha >= hover_alpha);
            assert_eq!(palette.active, palette.selected);
            assert_eq!(
                palette.disabled.primary_foreground_rgba,
                (theme.colors.text.dimmed << 8) | 0xFF
            );
        }
    }

    #[test]
    fn unified_disabled_hover_resolves_to_disabled_paint() {
        let palette = UnifiedListItemColors::default().row_state_palette();
        assert_eq!(
            palette.for_flags(RowStateFlags {
                selected: false,
                hovered: true,
                active: false,
                disabled: true,
            }),
            palette.disabled
        );
        assert_eq!(
            palette.for_flags(RowStateFlags {
                selected: true,
                hovered: true,
                active: true,
                disabled: false,
            }),
            palette.active,
            "active must outrank selection and hover"
        );
    }
}
