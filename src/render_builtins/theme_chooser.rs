use crate::theme::gpui_integration::best_contrast_of_two;

use gpui_component::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    slider::{Slider, SliderEvent, SliderState, SliderValue},
    Colorize as _,
};

include!("theme_chooser_state.rs");
include!("theme_chooser_controls.rs");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChooserSliderApplyMode {
    LiveDrag,
    Commit,
}

impl ThemeChooserSliderApplyMode {
    fn notify_parent(self) -> bool {
        matches!(self, Self::Commit)
    }
}

#[derive(Clone, Copy, Debug)]
enum ThemeChooserColorBinding {
    Accent,
    Background,
    GradientFrom { layer_index: Option<usize> },
    GradientTo { layer_index: Option<usize> },
}

/// Right-panel mode for the Theme Designer.
/// `Preview` shows a large live preview plus theme facts; `Customize` shows
/// the full editing control stack. Both keep the gallery list on the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThemeChooserPanelMode {
    #[default]
    Preview,
    Customize,
}

impl ThemeChooserPanelMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Customize => "customize",
        }
    }

    fn toggled(self) -> Self {
        match self {
            Self::Preview => Self::Customize,
            Self::Customize => Self::Preview,
        }
    }
}

pub(crate) struct ThemeChooserGradientControls {
    from: ThemeChooserColorControls,
    to: ThemeChooserColorControls,
    angle: Entity<SliderState>,
    opacity: Entity<SliderState>,
}

pub(crate) struct ThemeChooserColorControls {
    picker: Entity<ColorPickerState>,
    hex_input: Entity<gpui_component::input::InputState>,
}

pub(crate) struct ThemeChooserControls {
    accent: ThemeChooserColorControls,
    background: ThemeChooserColorControls,
    surface_opacity: Entity<SliderState>,
    secondary_text_opacity: Entity<SliderState>,
    focused_background_opacity: Entity<SliderState>,
    glass_veil_opacity: Entity<SliderState>,
    glass_tint_opacity: Entity<SliderState>,
    glass_morph_duration: Entity<SliderState>,
    glass_morph_inset: Entity<SliderState>,
    ui_font_size: Entity<SliderState>,
    gradient_base: ThemeChooserGradientControls,
    gradient_layers: Vec<ThemeChooserGradientControls>,
    subscriptions: Vec<Subscription>,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum ThemeChooserManagementStatus {
    #[default]
    Clean,
    Dirty,
    Saved {
        name: String,
    },
    DuplicateName {
        requested: String,
        resolved: String,
    },
    DeleteNeedsConfirmation {
        name: String,
    },
    DeletedCanRestore {
        name: String,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThemeChooserManagementState {
    pub(crate) selected_base: Option<ThemeChooserBase>,
    pub(crate) draft_name: Option<String>,
    pub(crate) last_saved: Option<ThemeChooserSaveReceipt>,
    pub(crate) pending_delete: Option<ThemeChooserDeleteCandidate>,
    pub(crate) last_deleted: Option<ThemeChooserDeletedTheme>,
    pub(crate) status: ThemeChooserManagementStatus,
}

const THEME_LIST_PAGE_SIZE: usize = 5;

fn render_theme_chooser_browser_main(
    filter: &str,
    filtered_count: usize,
    results_body: gpui::AnyElement,
    empty_body: gpui::AnyElement,
    preview_panel: gpui::AnyElement,
    list_colors: crate::list_item::ListItemColors,
) -> gpui::AnyElement {
    let browser_body = if filtered_count == 0 {
        empty_body
    } else {
        results_body
    };

    div()
        .flex_1()
        .overflow_hidden()
        .flex()
        .flex_row()
        .child(
            div()
                .w_1_2()
                .h_full()
                .py(px(4.0))
                .flex()
                .flex_col()
                .child(
                    crate::components::builtin_leading_separator::render_builtin_leading_separator(
                        if filter.trim().is_empty() {
                            "Themes"
                        } else {
                            "Results"
                        },
                        None,
                        list_colors,
                    ),
                )
                .child(browser_body),
        )
        .child(preview_panel)
        .into_any_element()
}

#[derive(Clone, Debug)]
pub(crate) enum ThemeChooserCatalogKind {
    BuiltIn(usize),
    User { slug: String },
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeChooserCatalogEntry {
    pub(crate) kind: ThemeChooserCatalogKind,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) is_dark: bool,
    pub(crate) theme: std::sync::Arc<crate::theme::Theme>,
    pub(crate) preview_colors: theme::presets::PresetPreviewColors,
}

fn theme_chooser_remix_seed() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
}

impl ScriptListApp {
    fn apply_theme_chooser_slider_change(
        &mut self,
        binding: ThemeChooserSliderBinding,
        value: SliderValue,
        cx: &mut Context<Self>,
    ) {
        self.apply_theme_chooser_slider_change_with_mode(
            binding,
            value,
            ThemeChooserSliderApplyMode::Commit,
            cx,
        );
    }

    fn apply_theme_chooser_slider_drag_change(
        &mut self,
        binding: ThemeChooserSliderBinding,
        value: SliderValue,
        cx: &mut Context<Self>,
    ) {
        self.apply_theme_chooser_slider_change_with_mode(
            binding,
            value,
            ThemeChooserSliderApplyMode::LiveDrag,
            cx,
        );
    }

    fn apply_theme_chooser_slider_change_with_mode(
        &mut self,
        binding: ThemeChooserSliderBinding,
        value: SliderValue,
        mode: ThemeChooserSliderApplyMode,
        cx: &mut Context<Self>,
    ) {
        let value = value.end();
        match binding {
            ThemeChooserSliderBinding::SurfaceOpacity => {
                let next =
                    Self::apply_surface_opacity_preset(self.theme.as_ref(), value.clamp(0.0, 1.0));
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_surface_opacity_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::GlassVeilOpacity => {
                let next = Self::apply_glass_veil_opacity_preset(
                    self.theme.as_ref(),
                    value.clamp(0.0, 1.0),
                );
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_glass_veil_opacity_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::GlassTintOpacity => {
                let next = Self::apply_glass_tint_opacity_preset(
                    self.theme.as_ref(),
                    value.clamp(0.0, 1.0),
                );
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_glass_tint_opacity_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::GlassMorphDuration => {
                let next = Self::apply_glass_morph_duration_preset(
                    self.theme.as_ref(),
                    value.clamp(0.0, 2.0),
                );
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_glass_morph_duration_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::GlassMorphInset => {
                let next = Self::apply_glass_morph_inset_preset(
                    self.theme.as_ref(),
                    value.clamp(0.0, 0.4),
                );
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_glass_morph_inset_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::SecondaryTextOpacity => {
                let next =
                    Self::apply_text_opacity_preset(self.theme.as_ref(), value.clamp(0.0, 1.0));
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_text_opacity_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::FocusedBackgroundOpacity => {
                let next = Self::apply_focused_background_opacity_preset(
                    self.theme.as_ref(),
                    value.clamp(0.0, 1.0),
                );
                self.apply_theme_chooser_slider_theme(
                    next,
                    "theme_chooser_focused_background_opacity_slider",
                    mode,
                    cx,
                );
            }
            ThemeChooserSliderBinding::UiFontSize => {
                let size = value.clamp(10.0, 32.0);
                self.mutate_theme_chooser_slider_theme(
                    "theme_chooser_ui_font_size_slider",
                    mode,
                    cx,
                    |theme| {
                        if let Some(fonts) = theme.fonts.as_mut() {
                            fonts.ui_size = size;
                        } else {
                            theme.fonts = Some(theme::FontConfig {
                                ui_size: size,
                                ..Default::default()
                            });
                        }
                    },
                );
            }
            ThemeChooserSliderBinding::GradientAngle { layer_index } => {
                let angle = value.rem_euclid(360.0);
                self.mutate_theme_chooser_slider_theme(
                    "theme_chooser_gradient_angle_slider",
                    mode,
                    cx,
                    |theme| {
                        let Some(gradient) = theme.background_gradient.as_mut() else {
                            return;
                        };
                        if let Some(index) = layer_index {
                            if let Some(layer) = gradient.layers.get_mut(index) {
                                layer.angle = angle;
                            }
                        } else {
                            gradient.angle = angle;
                        }
                    },
                );
            }
            ThemeChooserSliderBinding::GradientOpacity { layer_index } => {
                let opacity = value.clamp(0.0, 1.0);
                self.mutate_theme_chooser_slider_theme(
                    "theme_chooser_gradient_opacity_slider",
                    mode,
                    cx,
                    |theme| {
                        let Some(gradient) = theme.background_gradient.as_mut() else {
                            return;
                        };
                        if let Some(index) = layer_index {
                            if let Some(layer) = gradient.layers.get_mut(index) {
                                layer.opacity = opacity;
                            }
                        } else {
                            gradient.opacity = opacity;
                        }
                    },
                );
            }
        }
    }

    fn apply_theme_chooser_color_change(
        &mut self,
        binding: ThemeChooserColorBinding,
        color: gpui::Hsla,
        cx: &mut Context<Self>,
    ) {
        let Some(hex) = Self::theme_chooser_hsla_to_hex_rgb(color) else {
            return;
        };
        self.apply_theme_chooser_color_hex_change(binding, hex, "theme_chooser_color_picker", cx);
    }

    fn apply_theme_chooser_color_hex_change(
        &mut self,
        binding: ThemeChooserColorBinding,
        hex: u32,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        match binding {
            ThemeChooserColorBinding::Accent => {
                self.mutate_theme_chooser_theme(reason, cx, |theme| {
                    theme.colors.accent.selected = hex;
                    theme.colors.text.on_accent =
                        best_contrast_of_two(hex, 0xFFFFFF, theme.colors.background.main);
                });
            }
            ThemeChooserColorBinding::Background => {
                self.mutate_theme_chooser_theme(reason, cx, |theme| {
                    theme.colors.background.main = hex;
                    // Match `normalize_theme_primary_text` load semantics so the
                    // previewed theme renders exactly like the persisted one.
                    theme.colors.text.primary = theme::hard_readable_text_hex(hex);
                    theme.colors.text.on_accent =
                        best_contrast_of_two(theme.colors.accent.selected, 0xFFFFFF, hex);
                });
            }
            ThemeChooserColorBinding::GradientFrom { layer_index } => {
                self.mutate_theme_chooser_theme(reason, cx, |theme| {
                    let Some(gradient) = theme.background_gradient.as_mut() else {
                        return;
                    };
                    if let Some(index) = layer_index {
                        if let Some(layer) = gradient.layers.get_mut(index) {
                            layer.from = hex;
                        }
                    } else {
                        gradient.from = hex;
                    }
                });
            }
            ThemeChooserColorBinding::GradientTo { layer_index } => {
                self.mutate_theme_chooser_theme(reason, cx, |theme| {
                    let Some(gradient) = theme.background_gradient.as_mut() else {
                        return;
                    };
                    if let Some(index) = layer_index {
                        if let Some(layer) = gradient.layers.get_mut(index) {
                            layer.to = hex;
                        }
                    } else {
                        gradient.to = hex;
                    }
                });
            }
        }
    }

    fn apply_theme_chooser_hex_text_if_valid(
        &mut self,
        binding: ThemeChooserColorBinding,
        text: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(hex) = Self::parse_theme_chooser_hex_input(text) {
            self.apply_theme_chooser_color_hex_change(binding, hex, "theme_chooser_hex_input", cx);
        }
    }

    pub(crate) fn set_theme_chooser_control_from_devtools(
        &mut self,
        control: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        let control = control
            .strip_prefix("control:theme-chooser:")
            .unwrap_or(control);
        let float_value = || {
            value
                .trim()
                .trim_end_matches('%')
                .parse::<f32>()
                .map(|parsed| {
                    if value.trim().ends_with('%') {
                        parsed / 100.0
                    } else {
                        parsed
                    }
                })
                .map_err(|_| anyhow::anyhow!("invalid numeric value '{value}'"))
        };
        let bool_value = || match value.trim().to_ascii_lowercase().as_str() {
            "true" | "on" | "1" | "yes" => Ok(true),
            "false" | "off" | "0" | "no" => Ok(false),
            _ => Err(anyhow::anyhow!("invalid boolean value '{value}'")),
        };
        let hex_value = || {
            Self::parse_theme_chooser_hex_input(value)
                .ok_or_else(|| anyhow::anyhow!("invalid #RRGGBB value '{value}'"))
        };
        let ensure_layer = |index: usize| {
            let layer_count = self
                .theme
                .background_gradient
                .as_ref()
                .map(|gradient| gradient.layers.len())
                .unwrap_or(0);
            if index >= layer_count {
                Err(anyhow::anyhow!(
                    "gradient layer {} does not exist",
                    index + 1
                ))
            } else {
                Ok(())
            }
        };

        match control {
            "surface-opacity" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::SurfaceOpacity,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "glass-veil-opacity" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::GlassVeilOpacity,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "glass-tint-opacity" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::GlassTintOpacity,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "glass-morph-duration" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::GlassMorphDuration,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "glass-morph-inset" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::GlassMorphInset,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "secondary-text-opacity" | "typography-hint-opacity" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::SecondaryTextOpacity,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "focused-background-opacity" | "focused-row-opacity" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::FocusedBackgroundOpacity,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "ui-font-size" => {
                self.apply_theme_chooser_slider_change(
                    ThemeChooserSliderBinding::UiFontSize,
                    SliderValue::Single(float_value()?),
                    cx,
                );
            }
            "save-name" => {
                let next_name = value.trim();
                if next_name.is_empty() {
                    return Err(anyhow::anyhow!("theme save name cannot be empty"));
                }
                let resolution = theme::user_themes::resolve_user_theme_name(next_name);
                let is_dirty = self.theme_chooser_is_dirty();
                let state = self.theme_chooser_management_mut();
                state.draft_name = Some(next_name.to_string());
                state.status = if resolution.collision_count > 0 {
                    ThemeChooserManagementStatus::DuplicateName {
                        requested: resolution.requested_name,
                        resolved: resolution.display_name,
                    }
                } else if is_dirty {
                    ThemeChooserManagementStatus::Dirty
                } else {
                    ThemeChooserManagementStatus::Clean
                };
            }
            "accent-color" | "accent-color-hex" => {
                // Apply the parsed hex directly: a hex -> Hsla -> hex round trip
                // is lossy (f32 channels) and shifts values by one (0x30 -> 0x2F).
                self.apply_theme_chooser_color_hex_change(
                    ThemeChooserColorBinding::Accent,
                    hex_value()?,
                    "theme_chooser_devtools_accent_color",
                    cx,
                );
            }
            "background-color" | "background-color-hex" => {
                self.apply_theme_chooser_color_hex_change(
                    ThemeChooserColorBinding::Background,
                    hex_value()?,
                    "theme_chooser_devtools_background_color",
                    cx,
                );
            }
            "vibrancy-material" => {
                let requested = value.trim().to_ascii_lowercase();
                let material = Self::VIBRANCY_MATERIALS
                    .iter()
                    .find(|(_, label)| label.eq_ignore_ascii_case(&requested))
                    .map(|(material, _)| *material)
                    .ok_or_else(|| {
                        anyhow::anyhow!("unknown vibrancy material '{value}' (hud, popover, menu, sidebar, content)")
                    })?;
                self.set_theme_chooser_vibrancy_material(
                    material,
                    "theme_chooser_devtools_vibrancy_material",
                    cx,
                );
            }
            "appearance-mode" => {
                let mode = match value.trim().to_ascii_lowercase().as_str() {
                    "auto" => crate::theme::types::AppearanceMode::Auto,
                    "light" => crate::theme::types::AppearanceMode::Light,
                    "dark" => crate::theme::types::AppearanceMode::Dark,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "unknown appearance mode '{value}' (auto, light, dark)"
                        ))
                    }
                };
                self.set_theme_chooser_appearance_mode(
                    mode,
                    "theme_chooser_devtools_appearance_mode",
                    cx,
                );
            }
            "panel-mode" => {
                let mode = match value.trim().to_ascii_lowercase().as_str() {
                    "preview" => ThemeChooserPanelMode::Preview,
                    "customize" => ThemeChooserPanelMode::Customize,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "unknown panel mode '{value}' (preview, customize)"
                        ))
                    }
                };
                self.set_theme_chooser_panel_mode(mode, cx);
            }
            "vibrancy-enabled" => {
                let enabled = bool_value()?;
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_vibrancy_enabled",
                    cx,
                    |theme| {
                        if let Some(vibrancy) = theme.vibrancy.as_mut() {
                            vibrancy.enabled = enabled;
                        } else {
                            theme.vibrancy = Some(crate::theme::types::VibrancySettings {
                                enabled,
                                ..Default::default()
                            });
                        }
                    },
                );
            }
            "gradient-enabled" => {
                let enabled = bool_value()?;
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_gradient_enabled",
                    cx,
                    |theme| {
                        if let Some(gradient) = theme.background_gradient.as_mut() {
                            gradient.enabled = enabled;
                        } else {
                            theme.background_gradient = Some(theme::BackgroundGradient {
                                enabled,
                                ..Default::default()
                            });
                        }
                    },
                );
            }
            "gradient-base-from" | "gradient-base-from-hex" => {
                let color = hex_value()?;
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_gradient_base_from",
                    cx,
                    |theme| {
                        let gradient = theme
                            .background_gradient
                            .get_or_insert_with(theme::BackgroundGradient::default);
                        gradient.from = color;
                    },
                );
            }
            "gradient-base-to" | "gradient-base-to-hex" => {
                let color = hex_value()?;
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_gradient_base_to",
                    cx,
                    |theme| {
                        let gradient = theme
                            .background_gradient
                            .get_or_insert_with(theme::BackgroundGradient::default);
                        gradient.to = color;
                    },
                );
            }
            "gradient-base-angle" => {
                let angle = float_value()?.rem_euclid(360.0);
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_gradient_base_angle",
                    cx,
                    |theme| {
                        let gradient = theme
                            .background_gradient
                            .get_or_insert_with(theme::BackgroundGradient::default);
                        gradient.angle = angle;
                    },
                );
            }
            "gradient-base-opacity" => {
                let opacity = float_value()?.clamp(0.0, 1.0);
                self.mutate_theme_chooser_theme(
                    "theme_chooser_devtools_gradient_base_opacity",
                    cx,
                    |theme| {
                        let gradient = theme
                            .background_gradient
                            .get_or_insert_with(theme::BackgroundGradient::default);
                        gradient.opacity = opacity;
                    },
                );
            }
            _ => {
                if let Some(rest) = control.strip_prefix("gradient-layer-") {
                    let Some((index_text, field)) = rest.split_once('-') else {
                        return Err(anyhow::anyhow!("unknown theme control '{control}'"));
                    };
                    let layer_index = index_text
                        .parse::<usize>()
                        .map_err(|_| anyhow::anyhow!("invalid gradient layer in '{control}'"))?
                        .checked_sub(1)
                        .ok_or_else(|| anyhow::anyhow!("gradient layer indices are 1-based"))?;
                    ensure_layer(layer_index)?;
                    match field {
                        "from" | "from-hex" => {
                            self.apply_theme_chooser_color_hex_change(
                                ThemeChooserColorBinding::GradientFrom {
                                    layer_index: Some(layer_index),
                                },
                                hex_value()?,
                                "theme_chooser_devtools_gradient_layer_from",
                                cx,
                            );
                        }
                        "to" | "to-hex" => {
                            self.apply_theme_chooser_color_hex_change(
                                ThemeChooserColorBinding::GradientTo {
                                    layer_index: Some(layer_index),
                                },
                                hex_value()?,
                                "theme_chooser_devtools_gradient_layer_to",
                                cx,
                            );
                        }
                        "angle" => {
                            self.apply_theme_chooser_slider_change(
                                ThemeChooserSliderBinding::GradientAngle {
                                    layer_index: Some(layer_index),
                                },
                                SliderValue::Single(float_value()?),
                                cx,
                            );
                        }
                        "opacity" => {
                            self.apply_theme_chooser_slider_change(
                                ThemeChooserSliderBinding::GradientOpacity {
                                    layer_index: Some(layer_index),
                                },
                                SliderValue::Single(float_value()?),
                                cx,
                            );
                        }
                        _ => return Err(anyhow::anyhow!("unknown theme control '{control}'")),
                    }
                } else {
                    return Err(anyhow::anyhow!("unknown theme control '{control}'"));
                }
            }
        }

        Ok(format!("{control}={value}"))
    }

    fn sync_slider_entity_value(
        slider: &gpui::Entity<SliderState>,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        slider.update(cx, |slider, cx| {
            if slider.is_dragging() {
                return;
            }
            let current = slider.value().end();
            if (current - value).abs() > 0.000_1 {
                slider.set_value(value, window, cx);
            }
        });
    }

    fn sync_color_picker_entity_value(
        picker: &gpui::Entity<ColorPickerState>,
        hex: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = Self::theme_chooser_hex_to_hsla(hex);
        let next_hex = next.to_hex().to_string();
        picker.update(cx, |picker, cx| {
            let current_hex = picker.value().map(|value| value.to_hex().to_string());
            if current_hex.as_deref() != Some(next_hex.as_str()) {
                picker.set_value(next, window, cx);
            }
        });
    }

    fn sync_color_control_value(
        controls: &ThemeChooserColorControls,
        hex: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::sync_color_picker_entity_value(&controls.picker, hex, window, cx);
        let next = Self::canonical_theme_chooser_hex_label(hex);
        controls.hex_input.update(cx, |input, cx| {
            if input.focus_handle(cx).is_focused(window) {
                return;
            }
            if input.value().as_ref() != next.as_str() {
                input.set_value(next, window, cx);
            }
        });
    }

    fn sync_gradient_controls_from_theme(
        controls: &ThemeChooserGradientControls,
        from: u32,
        to: u32,
        angle: f32,
        opacity: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::sync_color_control_value(&controls.from, from, window, cx);
        Self::sync_color_control_value(&controls.to, to, window, cx);
        Self::sync_slider_entity_value(&controls.angle, angle.rem_euclid(360.0), window, cx);
        Self::sync_slider_entity_value(&controls.opacity, opacity.clamp(0.0, 1.0), window, cx);
    }

    fn sync_theme_chooser_control_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(controls) = self.theme_chooser_controls.as_ref() else {
            return;
        };
        let opacity = self.theme.get_opacity();
        let fonts = self.theme.get_fonts();

        Self::sync_color_control_value(
            &controls.accent,
            self.theme.colors.accent.selected,
            window,
            cx,
        );
        Self::sync_color_control_value(
            &controls.background,
            self.theme.colors.background.main,
            window,
            cx,
        );
        Self::sync_slider_entity_value(&controls.surface_opacity, opacity.main, window, cx);
        Self::sync_slider_entity_value(
            &controls.glass_veil_opacity,
            opacity
                .glass_veil_opacity
                .unwrap_or(crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP),
            window,
            cx,
        );
        Self::sync_slider_entity_value(
            &controls.glass_tint_opacity,
            opacity.glass_tint_opacity.unwrap_or(0.0),
            window,
            cx,
        );
        Self::sync_slider_entity_value(
            &controls.glass_morph_duration,
            opacity
                .glass_morph_duration
                .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
            window,
            cx,
        );
        Self::sync_slider_entity_value(
            &controls.glass_morph_inset,
            opacity
                .glass_morph_inset
                .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
            window,
            cx,
        );
        Self::sync_slider_entity_value(
            &controls.secondary_text_opacity,
            opacity.text_placeholder,
            window,
            cx,
        );
        Self::sync_slider_entity_value(
            &controls.focused_background_opacity,
            opacity.selected,
            window,
            cx,
        );
        Self::sync_slider_entity_value(&controls.ui_font_size, fonts.ui_size, window, cx);

        let gradient = self.theme.background_gradient.clone().unwrap_or_default();

        Self::sync_gradient_controls_from_theme(
            &controls.gradient_base,
            gradient.from,
            gradient.to,
            gradient.angle,
            gradient.opacity,
            window,
            cx,
        );

        for (index, layer) in gradient.layers.iter().enumerate() {
            if let Some(layer_controls) = controls.gradient_layers.get(index) {
                Self::sync_gradient_controls_from_theme(
                    layer_controls,
                    layer.from,
                    layer.to,
                    layer.angle,
                    layer.opacity,
                    window,
                    cx,
                );
            }
        }
    }

    fn clear_theme_chooser_controls(&mut self) {
        self.theme_chooser_controls = None;
    }

    fn render_theme_chooser_customize_section(
        title: &'static str,
        subtitle: Option<&'static str>,
        chrome: &theme::AppChromeColors,
        children: Vec<gpui::AnyElement>,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(12.0))
            .rounded(px(10.0))
            .border_1()
            .border_color(rgba(chrome.whisper_border_rgba))
            .bg(rgba(chrome.whisper_surface_rgba))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(chrome.text_dimmed_hex))
                            .child(title),
                    )
                    .when_some(subtitle, |this, subtitle| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(rgb(chrome.text_muted_hex))
                                .child(subtitle),
                        )
                    }),
            )
            .children(children)
            .into_any_element()
    }

    fn theme_chooser_overlay_token(rgb_hex: u32, alpha_source_rgba: u32) -> u32 {
        ((rgb_hex & 0x00ff_ffff) << 8) | (alpha_source_rgba & 0xff)
    }

    const THEME_CHOOSER_MANAGEMENT_HOVER_ALPHA_FLOOR: u32 = 0x5c;

    fn theme_chooser_rgba_alpha_floor(rgba_token: u32) -> u32 {
        let alpha = (rgba_token & 0xff).max(Self::THEME_CHOOSER_MANAGEMENT_HOVER_ALPHA_FLOOR);
        (rgba_token & 0xffff_ff00) | alpha
    }

    fn theme_chooser_management_hover_token(
        kind: ThemeChooserManagementButtonKind,
        hover_rgba: u32,
    ) -> u32 {
        match kind {
            ThemeChooserManagementButtonKind::Primary => hover_rgba,
            ThemeChooserManagementButtonKind::Neutral
            | ThemeChooserManagementButtonKind::Destructive => {
                Self::theme_chooser_rgba_alpha_floor(hover_rgba)
            }
        }
    }

    fn render_theme_chooser_management_button<F>(
        id: &'static str,
        label: &'static str,
        kind: ThemeChooserManagementButtonKind,
        disabled: bool,
        style: ThemeChooserManagementButtonStyle,
        on_click: F,
    ) -> gpui::AnyElement
    where
        F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        let ThemeChooserManagementButtonStyle {
            text_hex,
            bg_rgba,
            border_rgba,
            hover_rgba,
        } = style;
        let hover_rgba = Self::theme_chooser_management_hover_token(kind, hover_rgba);
        div()
            .id(id)
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(rgba(border_rgba))
            .bg(rgba(bg_rgba))
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgb(text_hex))
            .cursor_default()
            .child(label)
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .hover(move |button| button.bg(rgba(hover_rgba)))
            })
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                if disabled {
                    return;
                }
                on_click(event, window, cx);
            })
            .into_any_element()
    }

    fn render_theme_chooser_slider_row(
        label: &'static str,
        value_label: String,
        slider: &Entity<SliderState>,
        chrome: &theme::AppChromeColors,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_secondary_hex))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(chrome.text_primary_hex))
                            .child(value_label),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .child(Slider::new(slider).horizontal().into_any_element()),
            )
            .into_any_element()
    }

    fn render_theme_chooser_color_picker_row(
        label: &'static str,
        _hex: u32,
        controls: &ThemeChooserColorControls,
        chrome: &theme::AppChromeColors,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_secondary_hex))
                            .child(label),
                    )
                    .child(
                        gpui_component::input::Input::new(&controls.hex_input)
                            .w(px(96.0))
                            .h(px(24.0))
                            .line_height(px(16.0))
                            .px(px(6.0))
                            .py(px(0.0))
                            .with_size(Size::XSmall)
                            .appearance(true)
                            .bordered(true)
                            .focus_bordered(true)
                            .text_color(rgb(chrome.text_primary_hex)),
                    ),
            )
            .child(
                div().flex().flex_row().items_center().gap(px(8.0)).child(
                    ColorPicker::new(&controls.picker)
                        .featured_colors(Self::theme_chooser_featured_colors())
                        .with_size(Size::Small),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn submit_theme_chooser_from_input_enter(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        logging::log("KEY", "PressEnter routed to ThemeChooser");
        if self.theme_chooser_preview_revision.is_some_and(|revision| revision != crate::theme::service::theme_revision()) {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: "Theme changed elsewhere; preview was not saved".into(),
            };
            self.mark_main_data_changed();
            cx.notify();
            return;
        }
        if let Err(e) = crate::theme::service::persist_theme_and_sync_all_windows(
            cx,
            self.theme.as_ref(),
            "theme_chooser_done",
        ) {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: format!("Save failed: {e}"),
            };
            self.mark_main_data_changed();
            cx.notify();
            return;
        }
        self.record_return_to_script_list_submit(
            "theme_chooser", "submit_theme_chooser_from_input_enter", Some("theme_chooser_done"),
        );
        self.theme_chooser_preview_revision = None;
        self.update_theme(cx);
        self.theme_revision_seen = crate::theme::service::theme_revision();
        self.theme_before_chooser = None;
        self.clear_theme_chooser_controls();
        self.go_back_or_close(window, cx);
    }

    fn theme_chooser_match_summary(
        filtered_indices: &[usize],
        catalog: &[ThemeChooserCatalogEntry],
    ) -> ThemeChooserMatchSummary {
        let catalog_dark = catalog.iter().filter(|entry| entry.is_dark).count();
        let visible_dark = filtered_indices
            .iter()
            .filter(|&&idx| catalog[idx].is_dark)
            .count();
        ThemeChooserMatchSummary {
            catalog_total: catalog.len(),
            catalog_dark,
            catalog_light: catalog.len().saturating_sub(catalog_dark),
            visible_total: filtered_indices.len(),
            visible_dark,
            visible_light: filtered_indices.len().saturating_sub(visible_dark),
        }
    }

    fn render_theme_chooser_empty_state_body(
        &self,
        filter: &str,
        summary: ThemeChooserMatchSummary,
        chrome: &theme::AppChromeColors,
    ) -> AnyElement {
        // Pasted hex colors are previewed live as the accent — explain that
        // instead of pretending the query simply had no matches.
        let hex_preview = Self::theme_chooser_filter_hex(filter);

        let (swatch_hex, headline, advice) = if let Some(hex) = hex_preview {
            (
                hex,
                format!(
                    "Previewing accent {}",
                    Self::canonical_theme_chooser_hex_label(hex)
                ),
                "Press Enter to apply this accent, or Esc to undo.".to_string(),
            )
        } else {
            let query = if filter.is_empty() {
                "your search".to_string()
            } else {
                format!("\"{}\"", filter)
            };
            (
                chrome.accent_hex,
                format!("No themes match {}", query),
                "Try a family name like rose, github, nord, or light.".to_string(),
            )
        };

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w(px(360.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(56.0))
                            .h(px(10.0))
                            .rounded(px(5.0))
                            .bg(rgb(swatch_hex)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(chrome.text_primary_hex))
                            .child(headline),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_muted_hex))
                            .child(advice),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_dimmed_hex))
                            .child(format!(
                                "{} dark · {} light · {} total presets",
                                summary.catalog_dark, summary.catalog_light, summary.catalog_total
                            )),
                    ),
            )
            .into_any_element()
    }

    /// Parse a `#RRGGBB` search query so pasted hex colors can be previewed
    /// live as the accent color instead of producing an empty list.
    fn theme_chooser_filter_hex(filter: &str) -> Option<u32> {
        let trimmed = filter.trim();
        if !trimmed.starts_with('#') {
            return None;
        }
        Self::parse_theme_chooser_hex_input(trimmed)
    }

    /// Live-preview a `#RRGGBB` pasted into the search input as the accent.
    pub(crate) fn apply_theme_chooser_filter_hex_preview(
        &mut self,
        hex: u32,
        cx: &mut Context<Self>,
    ) {
        self.apply_theme_chooser_color_hex_change(
            ThemeChooserColorBinding::Accent,
            hex,
            "theme_chooser_filter_hex_preview",
            cx,
        );
    }

    /// Shared filter-change side effects for the Theme Designer: re-splice the
    /// list state, scroll to top, and live-preview either a pasted `#RRGGBB`
    /// accent or the first match. Called from both the GPUI input path
    /// (`handle_filter_input_change`) and the protocol `setFilter` path
    /// (`set_filter_text_immediate`) so automation sees the same behavior as
    /// real typing.
    pub(crate) fn apply_theme_chooser_filter_change_effects(&mut self, cx: &mut Context<Self>) {
        let (current_filter, current_selected_index) = match &self.current_view {
            AppView::ThemeChooserView {
                filter,
                selected_index,
            } => (filter.clone(), *selected_index),
            _ => return,
        };
        let catalog = Self::theme_chooser_catalog();
        let filtered = Self::theme_chooser_catalog_filtered_indices(&current_filter, &catalog);
        self.sync_theme_chooser_list_state(filtered.len());
        self.theme_chooser_list_state.scroll_to(gpui::ListOffset {
            item_ix: 0,
            offset_in_item: px(0.),
        });
        if let Some(hex) = Self::theme_chooser_filter_hex(&current_filter) {
            // Pasted #RRGGBB: preview it live as the accent color.
            self.apply_theme_chooser_filter_hex_preview(hex, cx);
        } else if filtered.is_empty() {
            cx.notify();
        } else {
            self.preview_theme_chooser_catalog_entry(
                &catalog,
                &filtered,
                current_selected_index,
                "theme_chooser_filter_preview",
                false,
                cx,
            );
        }
    }

    /// Protocol `simulateKey` support for the Theme Designer. Mirrors the
    /// navigation/commit/cancel subset of the live GPUI key handler in
    /// `render_theme_chooser` so hidden-window automation can drive the same
    /// user paths (arrow preview, Cmd+E panel toggle, Esc cancel, Enter
    /// commit) that real keyboards do.
    pub(crate) fn simulate_theme_chooser_key(
        &mut self,
        key_lower: &str,
        has_cmd: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if has_cmd {
            match key_lower {
                "e" => {
                    self.toggle_theme_chooser_panel_mode(cx);
                    return;
                }
                "k" => {
                    self.toggle_actions(cx, window);
                    return;
                }
                "w" => {
                    if let Some(original) = self.theme_before_chooser.take() {
                        if !self.restore_theme_chooser_theme(original, "theme_chooser_close_undo", cx) { return; }
                    }
                    self.clear_theme_chooser_controls();
                    self.close_and_reset_window(cx);
                    return;
                }
                _ => {}
            }
        }
        match key_lower {
            "escape" => {
                // Same contract as the live handler: clear the filter first;
                // a pure cancel restores the opening snapshot in memory only.
                if !self.clear_builtin_view_filter(cx) {
                    if let Some(original) = self.theme_before_chooser.take() {
                        if !self.restore_theme_chooser_theme(original, "theme_chooser_escape_undo", cx) { return; }
                    }
                    self.clear_theme_chooser_controls();
                    self.go_back_or_close(window, cx);
                }
            }
            "enter" | "return" => {
                self.submit_theme_chooser_from_input_enter(window, cx);
            }
            "up" | "arrowup" | "down" | "arrowdown" | "home" | "end" | "pageup" | "pagedown" => {
                let (current_filter, old_index) = match &self.current_view {
                    AppView::ThemeChooserView {
                        filter,
                        selected_index,
                    } => (filter.clone(), *selected_index),
                    _ => return,
                };
                let catalog = Self::theme_chooser_catalog();
                let filtered =
                    Self::theme_chooser_catalog_filtered_indices(&current_filter, &catalog);
                let count = filtered.len();
                if count == 0 {
                    return;
                }
                let mut new_index = old_index.min(count - 1);
                match key_lower {
                    "up" | "arrowup" => new_index = new_index.saturating_sub(1),
                    "down" | "arrowdown" => new_index = (new_index + 1).min(count - 1),
                    "home" => new_index = 0,
                    "end" => new_index = count - 1,
                    "pageup" => new_index = new_index.saturating_sub(THEME_LIST_PAGE_SIZE),
                    "pagedown" => new_index = (new_index + THEME_LIST_PAGE_SIZE).min(count - 1),
                    _ => unreachable!(),
                }
                if let AppView::ThemeChooserView { selected_index, .. } = &mut self.current_view {
                    *selected_index = new_index;
                }
                self.theme_chooser_list_state
                    .scroll_to_reveal_item(new_index);
                self.preview_theme_chooser_catalog_entry(
                    &catalog,
                    &filtered,
                    new_index,
                    "theme_chooser_keyboard_preview",
                    false,
                    cx,
                );
            }
            _ => {
                logging::log(
                    "STDIN",
                    &format!("SimulateKey: Unhandled key '{key_lower}' in ThemeChooserView"),
                );
            }
        }
    }

    fn theme_preview_colors_for_theme(
        theme: &crate::theme::Theme,
    ) -> theme::presets::PresetPreviewColors {
        theme::presets::PresetPreviewColors {
            bg: theme.colors.background.main,
            accent: theme.colors.accent.selected,
            text: theme.colors.text.primary,
            secondary: theme.colors.text.secondary,
            border: theme.colors.ui.border,
        }
    }

    fn theme_chooser_catalog() -> Vec<ThemeChooserCatalogEntry> {
        let presets = theme::presets::presets_cached();
        let preview_colors = theme::presets::preset_preview_colors_cached();
        let mut catalog = theme::user_themes::list_user_themes()
            .into_iter()
            .filter_map(|user_theme| {
                let theme = theme::user_themes::load_user_theme(&user_theme.slug)?;
                let preview_colors = Self::theme_preview_colors_for_theme(&theme);
                Some(ThemeChooserCatalogEntry {
                    kind: ThemeChooserCatalogKind::User {
                        slug: user_theme.slug.clone(),
                    },
                    name: user_theme.name,
                    description: "User theme saved in ~/.scriptkit/themes".to_string(),
                    is_dark: theme.has_dark_colors(),
                    theme: std::sync::Arc::new(theme),
                    preview_colors,
                })
            })
            .collect::<Vec<_>>();

        catalog.extend(presets.iter().enumerate().map(|(index, preset)| {
            ThemeChooserCatalogEntry {
                kind: ThemeChooserCatalogKind::BuiltIn(index),
                name: preset.name.to_string(),
                description: preset.description.to_string(),
                is_dark: preset.is_dark,
                theme: theme::presets::preset_theme_cached(index),
                preview_colors: preview_colors[index],
            }
        }));

        catalog
    }

    /// Find the catalog row that matches the given theme: user themes are
    /// matched by exact fingerprint first, then built-in presets by their
    /// background/accent key. Falls back to the first row.
    fn theme_chooser_catalog_index_for_theme(
        catalog: &[ThemeChooserCatalogEntry],
        theme: &crate::theme::Theme,
    ) -> usize {
        let fingerprint = Self::theme_chooser_theme_fingerprint(theme);
        if let Some(index) = catalog.iter().position(|entry| {
            matches!(entry.kind, ThemeChooserCatalogKind::User { .. })
                && Self::theme_chooser_theme_fingerprint(entry.theme.as_ref()) == fingerprint
        }) {
            return index;
        }
        let preset_index = theme::presets::find_current_preset_index(theme);
        catalog
            .iter()
            .position(|entry| {
                matches!(&entry.kind, ThemeChooserCatalogKind::BuiltIn(index) if *index == preset_index)
            })
            .unwrap_or(0)
    }

    fn theme_chooser_catalog_filtered_indices(
        filter: &str,
        catalog: &[ThemeChooserCatalogEntry],
    ) -> Vec<usize> {
        let filter = filter.trim().to_lowercase();
        if filter.is_empty() {
            return (0..catalog.len()).collect();
        }
        catalog
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.name.to_lowercase().contains(&filter)
                    || entry.description.to_lowercase().contains(&filter)
                    || match &entry.kind {
                        ThemeChooserCatalogKind::BuiltIn(index) => theme::presets::presets_cached()
                            .get(*index)
                            .map(|preset| preset.id.contains(&filter))
                            .unwrap_or(false),
                        ThemeChooserCatalogKind::User { slug } => {
                            slug.contains(&filter) || "user personal custom".contains(&filter)
                        }
                    }
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Cached original-theme preset classification for the theme chooser.
    /// Only rebuilds when the `Arc<Theme>` pointer changes (i.e. a new theme
    /// is set as the original). Avoids JSON-serialization-based comparison
    /// on every render frame.
    fn cached_theme_chooser_original_match(
        theme: Option<&std::sync::Arc<crate::theme::Theme>>,
    ) -> Option<theme::presets::PresetMatchResult> {
        use std::cell::RefCell;
        thread_local! {
            static ORIGINAL_MATCH: RefCell<Option<(usize, theme::presets::PresetMatchResult)>> =
                const { RefCell::new(None) };
        }
        let theme = theme?;
        let cache_key = std::sync::Arc::as_ptr(theme) as usize;
        ORIGINAL_MATCH.with(|slot| {
            if let Some((cached_key, cached_match)) = slot.borrow().as_ref() {
                if *cached_key == cache_key {
                    return Some(cached_match.clone());
                }
            }
            let built = theme::presets::classify_theme_preset_match(theme.as_ref());
            *slot.borrow_mut() = Some((cache_key, built.clone()));
            Some(built)
        })
    }

    /// Unified helper for all chooser-local theme mutations.
    /// Updates self.theme, syncs gpui-component + native vibrancy, and notifies.
    fn apply_theme_chooser_theme(
        &mut self,
        next_theme: crate::theme::Theme,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.apply_theme_chooser_theme_preview(next_theme, reason, true, true, cx);
    }

    fn apply_theme_chooser_slider_theme(
        &mut self,
        next_theme: crate::theme::Theme,
        reason: &'static str,
        mode: ThemeChooserSliderApplyMode,
        cx: &mut Context<Self>,
    ) {
        self.apply_theme_chooser_theme_preview(next_theme, reason, false, mode.notify_parent(), cx);
    }

    fn apply_theme_chooser_theme_preview(
        &mut self,
        next_theme: crate::theme::Theme,
        reason: &'static str,
        sync_native_vibrancy: bool,
        notify_parent: bool,
        cx: &mut Context<Self>,
    ) {
        let snapshot = crate::theme::get_theme_snapshot();
        let expected = self.theme_chooser_preview_revision.unwrap_or(snapshot.revision);
        let result = crate::theme::live_edit::prepare_theme(next_theme)
            .map_err(crate::theme::service::ThemePublishError::from)
            .and_then(|prepared| crate::theme::service::publish_runtime_theme(
                cx, expected, prepared,
                crate::theme::service::ThemePublicationSource::ChooserPreview { sync_native: sync_native_vibrancy },
            ));
        match result {
            Ok(receipt) => {
                self.theme_before_chooser.get_or_insert(snapshot.theme.clone());
                self.theme_chooser_preview_revision = Some(receipt.revision);
                self.theme = crate::theme::get_theme_snapshot().theme.clone();
                self.theme_revision_seen = receipt.revision;
                self.sync_open_actions_dialog_theme(cx);
                self.sync_open_terminal_theme(cx);
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Preview failed: {error}"),
                };
                tracing::warn!(%error, reason, "theme_chooser_preview_failed");
                cx.notify();
                return;
            }
        }
        if notify_parent {
            cx.notify();
        }
    }

    /// Apply a theme AND persist to disk.
    /// Theme Designer should call this only for explicit Apply/Done/Undo
    /// style commits, not ordinary preview or customization controls.
    fn apply_and_persist_theme(
        &mut self,
        next_theme: crate::theme::Theme,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let expected = self.theme_chooser_preview_revision.unwrap_or_else(crate::theme::service::theme_revision);
        if expected != crate::theme::service::theme_revision() {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: "Theme changed elsewhere; preview was not saved".into(),
            };
            self.mark_main_data_changed();
            cx.notify();
            return;
        }
        match crate::theme::service::persist_theme_and_sync_all_windows(cx, &next_theme, reason) {
            Ok(_) => {
                let snapshot = crate::theme::get_theme_snapshot();
                self.theme = snapshot.theme.clone();
                // An explicit Apply is the new cancel baseline.
                self.theme_before_chooser = Some(snapshot.theme.clone());
                self.theme_chooser_preview_revision = Some(snapshot.revision);
                self.theme_revision_seen = snapshot.revision;
                self.sync_open_actions_dialog_theme(cx);
                self.sync_open_terminal_theme(cx);
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Save failed: {error}"),
                };
                self.mark_main_data_changed();
            }
        }
        cx.notify();
    }

    /// Clone-and-mutate convenience: clones the current theme, applies a
    /// mutation closure, then routes through the unified preview pipeline.
    fn mutate_theme_chooser_theme(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut crate::theme::Theme),
    ) {
        let mut next = (*self.theme).clone();
        mutate(&mut next);
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn mutate_theme_chooser_slider_theme(
        &mut self,
        reason: &'static str,
        mode: ThemeChooserSliderApplyMode,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut crate::theme::Theme),
    ) {
        let mut next = (*self.theme).clone();
        mutate(&mut next);
        self.apply_theme_chooser_slider_theme(next, reason, mode, cx);
    }

    /// Restore a previously saved theme (escape/close paths).
    /// Routes through the same preview sync pipeline as mutations.
    fn restore_theme_chooser_theme(
        &mut self,
        original: std::sync::Arc<crate::theme::Theme>,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(expected) = self.theme_chooser_preview_revision else { return true; };
        let result = crate::theme::live_edit::prepare_theme((*original).clone())
            .map_err(crate::theme::service::ThemePublishError::from)
            .and_then(|prepared| crate::theme::service::publish_runtime_theme(
                cx, expected, prepared, crate::theme::service::ThemePublicationSource::Revert,
            ));
        match result {
            Ok(_) => {
                self.theme = crate::theme::get_theme_snapshot().theme.clone();
                self.theme_chooser_preview_revision = None;
                self.update_theme(cx);
                self.theme_revision_seen = crate::theme::service::theme_revision();
                self.sync_open_actions_dialog_theme(cx);
                self.sync_open_terminal_theme(cx);
                cx.notify();
                true
            }
            Err(error) => {
                self.theme_before_chooser = Some(original);
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Restore refused: {error}"),
                };
                tracing::warn!(%error, reason, "theme_chooser_restore_failed");
                cx.notify();
                false
            }
        }
    }

    fn preview_theme_chooser_catalog_entry(
        &mut self,
        catalog: &[ThemeChooserCatalogEntry],
        filtered_indices: &[usize],
        filtered_selected_index: usize,
        reason: &'static str,
        persist: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(&catalog_idx) = filtered_indices.get(filtered_selected_index) else {
            return;
        };
        let Some(entry) = catalog.get(catalog_idx) else {
            return;
        };
        self.theme_chooser_list_state
            .scroll_to_reveal_item(filtered_selected_index);
        let next_theme = (*entry.theme).clone();
        self.update_theme_chooser_selected_base(entry);
        if persist {
            self.apply_and_persist_theme(next_theme, reason, cx);
        } else {
            self.apply_theme_chooser_theme(next_theme, reason, cx);
        }
    }

    fn save_current_theme_as_user_theme(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let selected_entry = self.selected_theme_chooser_catalog_entry();
        let snapshot = self.theme_chooser_management_snapshot(selected_entry.as_ref());
        match theme::user_themes::save_theme_as_user_theme(&snapshot.save_name, self.theme.as_ref())
        {
            Ok(saved) => {
                tracing::info!(slug = %saved.slug, path = %saved.path.display(), reason, "theme_chooser_saved_user_theme");
                let fingerprint = Self::theme_chooser_theme_fingerprint(self.theme.as_ref());
                let state = self.theme_chooser_management_mut();
                state.selected_base = Some(ThemeChooserBase::User {
                    slug: saved.slug.clone(),
                    name: saved.name.clone(),
                    fingerprint,
                });
                state.draft_name = None;
                state.last_saved = Some(ThemeChooserSaveReceipt {
                    slug: saved.slug,
                    name: saved.name.clone(),
                    fingerprint,
                });
                state.pending_delete = None;
                state.status = ThemeChooserManagementStatus::Saved { name: saved.name };
                cx.notify();
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Save failed: {error}"),
                };
                tracing::warn!(%error, reason, "theme_chooser_save_user_theme_failed");
                cx.notify();
            }
        }
    }

    fn materialize_theme_chooser_text_edit_file(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<std::path::PathBuf> {
        let current_fingerprint = Self::theme_chooser_theme_fingerprint(self.theme.as_ref());

        if let Some(saved) = self
            .theme_chooser_management
            .as_ref()
            .and_then(|state| state.last_saved.as_ref())
            .filter(|saved| saved.fingerprint == current_fingerprint)
        {
            if let Some(theme) = theme::user_themes::find_user_theme(&saved.slug) {
                return Ok(theme.path);
            }
        }

        if let Some(ThemeChooserBase::User {
            slug, fingerprint, ..
        }) = self
            .theme_chooser_management
            .as_ref()
            .and_then(|state| state.selected_base.as_ref())
            .filter(|base| match base {
                ThemeChooserBase::User { fingerprint, .. } => *fingerprint == current_fingerprint,
                ThemeChooserBase::BuiltIn { .. } => false,
            })
        {
            if let Some(theme) = theme::user_themes::find_user_theme(slug) {
                return Ok(theme.path);
            }
            let _ = fingerprint;
        }

        let selected_entry = self.selected_theme_chooser_catalog_entry();
        let snapshot = self.theme_chooser_management_snapshot(selected_entry.as_ref());
        let saved =
            theme::user_themes::save_theme_as_user_theme(&snapshot.save_name, self.theme.as_ref())?;
        tracing::info!(
            slug = %saved.slug,
            path = %saved.path.display(),
            "theme_chooser_materialized_text_edit_file"
        );
        let path = saved.path.clone();
        let state = self.theme_chooser_management_mut();
        state.selected_base = Some(ThemeChooserBase::User {
            slug: saved.slug.clone(),
            name: saved.name.clone(),
            fingerprint: current_fingerprint,
        });
        state.draft_name = None;
        state.last_saved = Some(ThemeChooserSaveReceipt {
            slug: saved.slug,
            name: saved.name.clone(),
            fingerprint: current_fingerprint,
        });
        state.pending_delete = None;
        state.status = ThemeChooserManagementStatus::Saved { name: saved.name };
        cx.notify();
        Ok(path)
    }

    fn update_selected_user_theme(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let Some(ThemeChooserBase::User { slug, name, .. }) = self
            .theme_chooser_management
            .as_ref()
            .and_then(|state| state.selected_base.clone())
        else {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: "Built-in themes cannot be updated".to_string(),
            };
            cx.notify();
            return;
        };
        if !self.theme_chooser_is_dirty() {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Clean;
            cx.notify();
            return;
        }
        match theme::user_themes::save_theme_to_user_theme_slug(&slug, &name, self.theme.as_ref()) {
            Ok(saved) => {
                let fingerprint = Self::theme_chooser_theme_fingerprint(self.theme.as_ref());
                let state = self.theme_chooser_management_mut();
                state.selected_base = Some(ThemeChooserBase::User {
                    slug: saved.slug.clone(),
                    name: saved.name.clone(),
                    fingerprint,
                });
                state.last_saved = Some(ThemeChooserSaveReceipt {
                    slug: saved.slug,
                    name: saved.name.clone(),
                    fingerprint,
                });
                state.pending_delete = None;
                state.status = ThemeChooserManagementStatus::Saved { name: saved.name };
                tracing::info!(slug, reason, "theme_chooser_updated_user_theme");
                cx.notify();
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Update failed: {error}"),
                };
                tracing::warn!(slug, %error, reason, "theme_chooser_update_user_theme_failed");
                cx.notify();
            }
        }
    }

    fn delete_selected_user_theme(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let Some((filter, selected_index)) = self.current_theme_chooser_selection() else {
            return;
        };
        let catalog = Self::theme_chooser_catalog();
        let filtered = Self::theme_chooser_catalog_filtered_indices(&filter, &catalog);
        let Some(entry) = filtered
            .get(selected_index)
            .and_then(|catalog_idx| catalog.get(*catalog_idx))
        else {
            return;
        };
        if let ThemeChooserCatalogKind::User { slug } = &entry.kind {
            let candidate = ThemeChooserDeleteCandidate {
                slug: slug.clone(),
                name: entry.name.clone(),
            };
            let already_pending = self
                .theme_chooser_management
                .as_ref()
                .and_then(|state| state.pending_delete.as_ref())
                .map(|pending| pending.slug == candidate.slug)
                .unwrap_or(false);
            if already_pending {
                self.confirm_delete_selected_user_theme(reason, cx);
            } else {
                let state = self.theme_chooser_management_mut();
                state.pending_delete = Some(candidate.clone());
                state.status = ThemeChooserManagementStatus::DeleteNeedsConfirmation {
                    name: candidate.name,
                };
                cx.notify();
            }
        } else {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: "Built-in themes cannot be deleted".to_string(),
            };
            cx.notify();
        }
    }

    fn confirm_delete_selected_user_theme(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let Some(candidate) = self
            .theme_chooser_management
            .as_ref()
            .and_then(|state| state.pending_delete.clone())
        else {
            return;
        };
        match theme::user_themes::delete_user_theme_with_backup(&candidate.slug) {
            Ok(Some(deleted)) => {
                let state = self.theme_chooser_management_mut();
                state.pending_delete = None;
                state.last_deleted = Some(ThemeChooserDeletedTheme {
                    slug: deleted.slug,
                    name: deleted.name.clone(),
                    contents: deleted.contents,
                });
                state.selected_base = None;
                state.status =
                    ThemeChooserManagementStatus::DeletedCanRestore { name: deleted.name };
                tracing::info!(slug = %candidate.slug, reason, "theme_chooser_deleted_user_theme");
                cx.notify();
            }
            Ok(None) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: "Theme was already deleted".to_string(),
                };
                cx.notify();
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Delete failed: {error}"),
                };
                tracing::warn!(slug = %candidate.slug, %error, reason, "theme_chooser_delete_user_theme_failed");
                cx.notify();
            }
        }
    }

    fn restore_last_deleted_user_theme(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let Some(deleted) = self
            .theme_chooser_management
            .as_ref()
            .and_then(|state| state.last_deleted.clone())
        else {
            self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                message: "No deleted theme to restore".to_string(),
            };
            cx.notify();
            return;
        };
        let backup = theme::user_themes::DeletedUserThemeBackup {
            path: theme::user_themes::user_themes_dir().join(format!("{}.json", deleted.slug)),
            slug: deleted.slug,
            name: deleted.name,
            contents: deleted.contents,
        };
        match theme::user_themes::restore_user_theme_backup(&backup) {
            Ok(restored) => {
                let state = self.theme_chooser_management_mut();
                state.last_deleted = None;
                state.pending_delete = None;
                state.status = ThemeChooserManagementStatus::Saved {
                    name: restored.name.clone(),
                };
                tracing::info!(slug = %restored.slug, reason, "theme_chooser_restored_user_theme");
                cx.notify();
            }
            Err(error) => {
                self.theme_chooser_management_mut().status = ThemeChooserManagementStatus::Error {
                    message: format!("Restore failed: {error}"),
                };
                tracing::warn!(%error, reason, "theme_chooser_restore_user_theme_failed");
                cx.notify();
            }
        }
    }

    fn add_theme_chooser_gradient_layer(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            gradient
                .layers
                .push(crate::theme::types::GradientLayer::default());
        } else {
            next.background_gradient = Some(crate::theme::types::BackgroundGradient {
                enabled: true,
                from: 0x1e1e1e,
                to: 0x2d2d30,
                angle: 135.0,
                opacity: 0.35,
                layers: vec![crate::theme::types::GradientLayer::default()],
            });
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn remove_theme_chooser_gradient_layer(
        &mut self,
        index: usize,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            if index < gradient.layers.len() {
                gradient.layers.remove(index);
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn toggle_theme_chooser_gradient_layer(
        &mut self,
        index: Option<usize>,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            if let Some(idx) = index {
                if let Some(layer) = gradient.layers.get_mut(idx) {
                    layer.enabled = !layer.enabled;
                }
            } else {
                gradient.enabled = !gradient.enabled;
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn update_theme_chooser_gradient_layer_color(
        &mut self,
        layer_index: Option<usize>,
        is_to: bool,
        new_color: u32,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            if let Some(idx) = layer_index {
                if let Some(layer) = gradient.layers.get_mut(idx) {
                    if is_to {
                        layer.to = new_color;
                    } else {
                        layer.from = new_color;
                    }
                }
            } else if is_to {
                gradient.to = new_color;
            } else {
                gradient.from = new_color;
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn adjust_theme_chooser_gradient_layer_angle(
        &mut self,
        layer_index: Option<usize>,
        delta: f32,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            if let Some(idx) = layer_index {
                if let Some(layer) = gradient.layers.get_mut(idx) {
                    layer.angle = (layer.angle + delta).rem_euclid(360.0);
                }
            } else {
                gradient.angle = (gradient.angle + delta).rem_euclid(360.0);
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn adjust_theme_chooser_gradient_layer_opacity(
        &mut self,
        layer_index: Option<usize>,
        delta: f32,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut next = (*self.theme).clone();
        if let Some(ref mut gradient) = next.background_gradient {
            if let Some(idx) = layer_index {
                if let Some(layer) = gradient.layers.get_mut(idx) {
                    layer.opacity = (layer.opacity + delta).clamp(0.0, 1.0);
                }
            } else {
                gradient.opacity = (gradient.opacity + delta).clamp(0.0, 1.0);
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn cycle_theme_chooser_gradient(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        const GRADIENT_PRESETS: &[(u32, u32, f32, f32)] = &[
            (0x1e1e1e, 0x3b2f5f, 135.0, 0.28),
            (0x111827, 0x164e63, 135.0, 0.30),
            (0x1f2937, 0x7c2d12, 135.0, 0.24),
        ];

        let mut next = (*self.theme).clone();
        let current = next.background_gradient.clone().unwrap_or_default();
        if !current.enabled {
            let (from, to, angle, opacity) = GRADIENT_PRESETS[0];
            next.background_gradient = Some(theme::BackgroundGradient {
                enabled: true,
                from,
                to,
                angle,
                opacity,
                layers: Vec::new(),
            });
        } else {
            let current_index = GRADIENT_PRESETS
                .iter()
                .position(|(from, to, angle, opacity)| {
                    *from == current.from
                        && *to == current.to
                        && (*angle - current.angle).abs() < 0.5
                        && (*opacity - current.opacity).abs() < 0.01
                })
                .unwrap_or(usize::MAX);
            if current_index + 1 < GRADIENT_PRESETS.len() {
                let (from, to, angle, opacity) = GRADIENT_PRESETS[current_index + 1];
                next.background_gradient = Some(theme::BackgroundGradient {
                    enabled: true,
                    from,
                    to,
                    angle,
                    opacity,
                    layers: Vec::new(),
                });
            } else {
                next.background_gradient = None;
            }
        }
        self.apply_theme_chooser_theme(next, reason, cx);
    }

    fn sync_theme_chooser_list_state(&mut self, item_count: usize) {
        let old_count = self.theme_chooser_list_state.item_count();
        if old_count != item_count {
            self.theme_chooser_list_state
                .splice(0..old_count, item_count);
        }
    }

    /// Accent color palette for theme customization
    const ACCENT_PALETTE: &'static [(u32, &'static str)] = theme::ACCENT_PALETTE;

    /// Opacity presets for quick selection
    const OPACITY_PRESETS: &'static [(f32, &'static str)] = &[
        (0.00, "0%"),
        (0.10, "10%"),
        (0.25, "25%"),
        (0.50, "50%"),
        (0.75, "75%"),
        (1.00, "100%"),
    ];

    /// Secondary text opacity presets for placeholder/hint/description tiers.
    const TEXT_OPACITY_PRESETS: &'static [(f32, &'static str)] = &[
        (0.00, "0%"),
        (0.25, "25%"),
        (0.50, "50%"),
        (0.60, "60%"),
        (0.65, "65%"),
        (0.70, "70%"),
        (0.80, "80%"),
        (0.90, "90%"),
        (1.00, "100%"),
    ];

    /// Focused row/background opacity presets.
    const FOCUSED_BACKGROUND_OPACITY_PRESETS: &'static [(f32, &'static str)] = &[
        (0.00, "0%"),
        (0.05, "5%"),
        (0.07, "7%"),
        (0.10, "10%"),
        (0.15, "15%"),
        (0.20, "20%"),
        (0.25, "25%"),
        (0.50, "50%"),
        (0.75, "75%"),
        (1.00, "100%"),
    ];

    /// Find the closest accent palette index for a given accent color
    fn find_accent_palette_index(accent: u32) -> Option<usize> {
        Self::ACCENT_PALETTE.iter().position(|&(c, _)| c == accent)
    }

    /// Find the closest opacity preset index for a given opacity value
    fn find_opacity_preset_index(opacity: f32) -> usize {
        Self::closest_float_preset_index(Self::OPACITY_PRESETS, opacity)
    }

    fn closest_float_preset_index(presets: &[(f32, &'static str)], current: f32) -> usize {
        let mut best_index = 0;
        let mut best_distance = f32::INFINITY;
        for (index, &(value, _)) in presets.iter().enumerate() {
            let distance = (value - current).abs();
            if distance < best_distance {
                best_distance = distance;
                best_index = index;
            }
        }
        best_index
    }

    fn render_theme_chooser_gradient_layer_controls(
        &self,
        title: String,
        enabled: bool,
        values: ThemeChooserGradientValues,
        controls: &ThemeChooserGradientControls,
        chrome: &theme::AppChromeColors,
    ) -> gpui::AnyElement {
        let ThemeChooserGradientValues {
            from: from_hex,
            to: to_hex,
            angle,
            opacity,
        } = values;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(chrome.text_secondary_hex))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(chrome.text_muted_hex))
                            .child(if enabled { "Enabled" } else { "Disabled" }),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .child(Self::render_theme_chooser_color_picker_row(
                                "From Color",
                                from_hex,
                                &controls.from,
                                chrome,
                            )),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Self::render_theme_chooser_color_picker_row(
                                "To Color",
                                to_hex,
                                &controls.to,
                                chrome,
                            )),
                    ),
            )
            .child(Self::render_theme_chooser_slider_row(
                "Angle",
                format!("{:.0}°", angle.rem_euclid(360.0)),
                &controls.angle,
                chrome,
            ))
            .child(Self::render_theme_chooser_slider_row(
                "Opacity",
                format!("{:.0}%", opacity.clamp(0.0, 1.0) * 100.0),
                &controls.opacity,
                chrome,
            ))
            .into_any_element()
    }

    /// Small pill segment used by single-select rows (vibrancy material,
    /// appearance mode) and the right-panel mode tabs.
    fn render_theme_chooser_segment_button<F>(
        id: (&'static str, usize),
        label: String,
        selected: bool,
        chrome: &theme::AppChromeColors,
        on_click: F,
    ) -> gpui::AnyElement
    where
        F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        let (text_color, bg_rgba, border_rgba) = if selected {
            (
                chrome.accent_badge_text_hex,
                chrome.accent_badge_bg_rgba,
                chrome.accent_badge_border_rgba,
            )
        } else {
            (
                chrome.text_secondary_hex,
                chrome.whisper_surface_rgba,
                chrome.whisper_border_rgba,
            )
        };
        let hover_rgba = Self::theme_chooser_rgba_alpha_floor(chrome.whisper_border_rgba);
        div()
            .id(id)
            .px(px(8.0))
            .py(px(3.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(rgba(border_rgba))
            .bg(rgba(bg_rgba))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(text_color))
            .cursor_pointer()
            .when(!selected, |segment| {
                segment.hover(move |segment| segment.bg(rgba(hover_rgba)))
            })
            .child(label)
            .on_click(move |event, window, cx| {
                cx.stop_propagation();
                on_click(event, window, cx);
            })
            .into_any_element()
    }

    /// Label + segments single-select row for the customize panel.
    fn render_theme_chooser_segmented_row(
        label: &'static str,
        segments: Vec<gpui::AnyElement>,
        chrome: &theme::AppChromeColors,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(chrome.text_secondary_hex))
                    .child(label),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(6.0))
                    .children(segments),
            )
            .into_any_element()
    }

    /// One label/value line inside the Preview-mode theme facts card.
    fn render_theme_chooser_fact_row(label: &'static str,
    value: String,
    chrome: &theme::AppChromeColors,
    swatch: Option<u32>,) -> gpui::Div {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(chrome.text_muted_hex))
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .when_some(swatch, |row, hex| {
                        row.child(
                            div()
                                .w(px(12.0))
                                .h(px(12.0))
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(rgba(chrome.whisper_border_rgba))
                                .bg(rgb(hex)),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(rgb(chrome.text_primary_hex))
                            .child(value),
                    ),
            )
    }

    /// Truthful footer hint strip for the theme chooser.
    ///
    /// Capped at the three-affordance budget (`.impeccable.md`: footer ≤3
    /// keys, discovery lives in ⌘K). ⌘E Customize and ⌘J Remix still work as
    /// shortcuts and are discoverable in the ⌘K actions dialog
    /// (`theme_chooser_toggle_customize` / `theme_chooser_remix` in
    /// `render_builtins/actions.rs`); they no longer occupy footer slots
    /// (chaos OF-7: five hints shipped under a comment claiming three).
    fn theme_chooser_hint_items() -> Vec<gpui::SharedString> {
        vec![
            gpui::SharedString::from("↵ Done"),
            gpui::SharedString::from("⌘K Actions"),
            gpui::SharedString::from("Esc Undo"),
        ]
    }

    fn current_theme_chooser_selection(&self) -> Option<(String, usize)> {
        if let AppView::ThemeChooserView {
            filter,
            selected_index,
        } = &self.current_view
        {
            Some((filter.clone(), *selected_index))
        } else {
            None
        }
    }


    fn theme_chooser_base_from_entry(entry: &ThemeChooserCatalogEntry) -> ThemeChooserBase {
        let fingerprint = Self::theme_chooser_theme_fingerprint(entry.theme.as_ref());
        match &entry.kind {
            ThemeChooserCatalogKind::BuiltIn(index) => ThemeChooserBase::BuiltIn {
                index: *index,
                name: entry.name.clone(),
                fingerprint,
            },
            ThemeChooserCatalogKind::User { slug } => ThemeChooserBase::User {
                slug: slug.clone(),
                name: entry.name.clone(),
                fingerprint,
            },
        }
    }

    fn theme_chooser_management_mut(&mut self) -> &mut ThemeChooserManagementState {
        self.theme_chooser_management
            .get_or_insert_with(ThemeChooserManagementState::default)
    }

    fn theme_chooser_selected_entry<'a>(
        catalog: &'a [ThemeChooserCatalogEntry],
        filtered_indices: &[usize],
        selected_index: usize,
    ) -> Option<&'a ThemeChooserCatalogEntry> {
        filtered_indices
            .get(selected_index)
            .and_then(|catalog_index| catalog.get(*catalog_index))
    }

    fn selected_theme_chooser_catalog_entry(&self) -> Option<ThemeChooserCatalogEntry> {
        let (filter, selected_index) = self.current_theme_chooser_selection()?;
        let catalog = Self::theme_chooser_catalog();
        let filtered = Self::theme_chooser_catalog_filtered_indices(&filter, &catalog);
        Self::theme_chooser_selected_entry(&catalog, &filtered, selected_index).cloned()
    }

    fn update_theme_chooser_selected_base(&mut self, entry: &ThemeChooserCatalogEntry) {
        let base = Self::theme_chooser_base_from_entry(entry);
        let state = self.theme_chooser_management_mut();
        state.selected_base = Some(base);
        state.draft_name = None;
        state.pending_delete = None;
        state.status = ThemeChooserManagementStatus::Clean;
    }

    pub(crate) fn theme_chooser_is_dirty(&self) -> bool {
        let current = Self::theme_chooser_theme_fingerprint(self.theme.as_ref());
        let base = self.theme_chooser_management.as_ref().and_then(|state| {
            state.selected_base.as_ref().map(|base| match base {
                ThemeChooserBase::BuiltIn { fingerprint, .. }
                | ThemeChooserBase::User { fingerprint, .. } => fingerprint,
            })
        });
        base.map(|fingerprint| *fingerprint != current)
            .unwrap_or(false)
    }

    fn suggested_theme_chooser_save_name(
        &self,
        selected_entry: Option<&ThemeChooserCatalogEntry>,
    ) -> String {
        let base = selected_entry
            .map(|entry| entry.name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or("Theme");
        let suffix = if self.theme_chooser_is_dirty() {
            " Remix"
        } else {
            " Copy"
        };
        format!("{base}{suffix}")
    }

    pub(crate) fn theme_chooser_management_snapshot(
        &self,
        selected_entry: Option<&ThemeChooserCatalogEntry>,
    ) -> ThemeChooserManagementSnapshot {
        let fallback_base = selected_entry.map(Self::theme_chooser_base_from_entry);
        let management = self.theme_chooser_management.as_ref();
        let base = management
            .and_then(|state| state.selected_base.as_ref())
            .cloned()
            .or(fallback_base);
        let current = Self::theme_chooser_theme_fingerprint(self.theme.as_ref());
        let base_fingerprint = base.as_ref().map(|base| match base {
            ThemeChooserBase::BuiltIn { fingerprint, .. }
            | ThemeChooserBase::User { fingerprint, .. } => fingerprint,
        });
        let is_dirty = base_fingerprint
            .map(|fingerprint| *fingerprint != current)
            .unwrap_or(false);
        let suggested = management
            .and_then(|state| state.draft_name.clone())
            .unwrap_or_else(|| self.suggested_theme_chooser_save_name(selected_entry));
        let resolution = theme::user_themes::resolve_user_theme_name(&suggested);
        let duplicate_status_kind =
            (resolution.collision_count > 0).then_some("duplicate".to_string());

        let (base_name, base_slug, is_user_base) = match base {
            Some(ThemeChooserBase::BuiltIn { index, name, .. }) => {
                (Some(name), Some(format!("builtin:{index}")), false)
            }
            Some(ThemeChooserBase::User { slug, name, .. }) => (Some(name), Some(slug), true),
            None => (None, None, false),
        };

        let (mut status_label, mut status_value, mut status_kind) = if is_dirty {
            (
                "Unsaved edits".to_string(),
                "dirty".to_string(),
                "dirty".to_string(),
            )
        } else if is_user_base {
            (
                "User theme saved".to_string(),
                "clean-user".to_string(),
                "clean".to_string(),
            )
        } else {
            (
                "Built-in preset".to_string(),
                "clean-built-in".to_string(),
                "clean".to_string(),
            )
        };

        if resolution.collision_count > 0 {
            status_label = format!("Will save as {}", resolution.display_name);
            status_value = resolution.display_name.clone();
            status_kind = "duplicate".to_string();
        }

        if let Some(state) = management {
            match &state.status {
                ThemeChooserManagementStatus::Saved { name } => {
                    status_label = format!("Saved as {name}");
                    status_value = name.clone();
                    status_kind = "saved".to_string();
                }
                ThemeChooserManagementStatus::DuplicateName {
                    requested,
                    resolved,
                } => {
                    status_label = format!("Will save {requested} as {resolved}");
                    status_value = resolved.clone();
                    status_kind = "duplicate".to_string();
                }
                ThemeChooserManagementStatus::DeleteNeedsConfirmation { name } => {
                    status_label = format!("Press Delete again to remove {name}");
                    status_value = name.clone();
                    status_kind = "delete-confirm".to_string();
                }
                ThemeChooserManagementStatus::DeletedCanRestore { name } => {
                    status_label = format!("Deleted {name}. Restore is available");
                    status_value = name.clone();
                    status_kind = "deleted".to_string();
                }
                ThemeChooserManagementStatus::Error { message } => {
                    status_label = message.clone();
                    status_value = message.clone();
                    status_kind = "error".to_string();
                }
                ThemeChooserManagementStatus::Clean | ThemeChooserManagementStatus::Dirty => {}
            }
        }

        ThemeChooserManagementSnapshot {
            status_label,
            status_value,
            status_kind,
            is_dirty,
            save_name: suggested,
            resolved_save_name: resolution.display_name,
            duplicate_status_kind,
            base_name,
            base_slug,
            can_update: is_user_base && is_dirty,
            update_disabled: if !is_user_base {
                Some("built_in_theme".to_string())
            } else if !is_dirty {
                Some("no_changes".to_string())
            } else {
                None
            },
            delete_disabled: (!is_user_base).then_some("built_in_theme".to_string()),
            restore_disabled: management
                .and_then(|state| state.last_deleted.as_ref())
                .is_none()
                .then_some("no_deleted_theme".to_string()),
        }
    }

    fn preview_current_theme_chooser_preset(
        &mut self,
        reason: &'static str,
        persist: bool,
        cx: &mut Context<Self>,
    ) {
        let Some((filter, selected_index)) = self.current_theme_chooser_selection() else {
            return;
        };
        let catalog = Self::theme_chooser_catalog();
        let filtered = Self::theme_chooser_catalog_filtered_indices(&filter, &catalog);
        self.preview_theme_chooser_catalog_entry(
            &catalog,
            &filtered,
            selected_index,
            reason,
            persist,
            cx,
        );
    }

    fn cycle_theme_chooser_accent(
        &mut self,
        direction: isize,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let current = self.theme.colors.accent.selected;
        let idx = Self::find_accent_palette_index(current).unwrap_or(0);
        let len = Self::ACCENT_PALETTE.len();
        let new_idx = if direction < 0 {
            (idx + len - 1) % len
        } else {
            (idx + 1) % len
        };
        let (new_accent, _) = Self::ACCENT_PALETTE[new_idx];
        let mut modified = (*self.theme).clone();
        modified.colors.accent.selected = new_accent;
        modified.colors.text.on_accent =
            best_contrast_of_two(new_accent, 0xFFFFFF, modified.colors.background.main);
        self.apply_theme_chooser_theme(modified, reason, cx);
    }

    fn adjust_theme_chooser_opacity(
        &mut self,
        direction: isize,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let idx = Self::find_opacity_preset_index(self.theme.get_opacity().main);
        let next_idx = if direction < 0 {
            idx.checked_sub(1)
        } else {
            (idx + 1 < Self::OPACITY_PRESETS.len()).then_some(idx + 1)
        };
        if let Some(next_idx) = next_idx {
            let target = Self::OPACITY_PRESETS[next_idx].0;
            let modified = Self::apply_surface_opacity_preset(self.theme.as_ref(), target);
            self.apply_theme_chooser_theme(modified, reason, cx);
        }
    }

    fn toggle_theme_chooser_vibrancy(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let mut modified = (*self.theme).clone();
        let vibrancy = modified
            .vibrancy
            .get_or_insert_with(crate::theme::types::VibrancySettings::default);
        vibrancy.enabled = !vibrancy.enabled;
        self.apply_theme_chooser_theme(modified, reason, cx);
    }

    fn set_theme_chooser_vibrancy_material(
        &mut self,
        material: theme::VibrancyMaterial,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut modified = (*self.theme).clone();
        let vibrancy = modified
            .vibrancy
            .get_or_insert_with(crate::theme::types::VibrancySettings::default);
        vibrancy.material = material;
        self.apply_theme_chooser_theme(modified, reason, cx);
    }

    fn set_theme_chooser_appearance_mode(
        &mut self,
        mode: crate::theme::types::AppearanceMode,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let mut modified = (*self.theme).clone();
        modified.appearance = mode;
        self.apply_theme_chooser_theme(modified, reason, cx);
    }

    pub(crate) fn set_theme_chooser_panel_mode(
        &mut self,
        mode: ThemeChooserPanelMode,
        cx: &mut Context<Self>,
    ) {
        if self.theme_chooser_panel_mode != mode {
            self.theme_chooser_panel_mode = mode;
            cx.notify();
        }
    }

    fn toggle_theme_chooser_panel_mode(&mut self, cx: &mut Context<Self>) {
        self.set_theme_chooser_panel_mode(self.theme_chooser_panel_mode.toggled(), cx);
    }

    fn cycle_theme_chooser_material(&mut self, reason: &'static str, cx: &mut Context<Self>) {
        let current_material = self
            .theme
            .vibrancy
            .as_ref()
            .map(|v| v.material)
            .unwrap_or_default();
        let idx = Self::find_vibrancy_material_index(current_material);
        let new_idx = (idx + 1) % Self::VIBRANCY_MATERIALS.len();
        let (new_material, _) = Self::VIBRANCY_MATERIALS[new_idx];
        self.set_theme_chooser_vibrancy_material(new_material, reason, cx);
    }

    fn adjust_theme_chooser_font_size(
        &mut self,
        direction: isize,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let current = self.theme.get_fonts().ui_size;
        let idx = Self::FONT_SIZE_PRESETS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.0 - current)
                    .abs()
                    .partial_cmp(&(b.0 - current).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        let next_idx = if direction < 0 {
            idx.checked_sub(1)
        } else {
            (idx + 1 < Self::FONT_SIZE_PRESETS.len()).then_some(idx + 1)
        };
        if let Some(next_idx) = next_idx {
            let (size, _) = Self::FONT_SIZE_PRESETS[next_idx];
            let mut modified = (*self.theme).clone();
            if let Some(ref mut fonts) = modified.fonts {
                fonts.ui_size = size;
            } else {
                modified.fonts = Some(theme::FontConfig {
                    ui_size: size,
                    ..Default::default()
                });
            }
            self.apply_theme_chooser_theme(modified, reason, cx);
        }
    }

    pub(crate) fn execute_theme_chooser_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action_id {
            "theme_chooser_done" => self.submit_theme_chooser_from_input_enter(window, cx),
            "theme_chooser_undo_close" => {
                // Memory-only restore: a cancelled session never writes the theme override to disk.
                if let Some(original) = self.theme_before_chooser.take() {
                    if !self.restore_theme_chooser_theme(original, "theme_chooser_action_undo", cx) { return; }
                }
                self.clear_theme_chooser_controls();
                self.go_back_or_close(window, cx);
            }
            "theme_chooser_remix" => {
                let remixed = Self::build_theme_chooser_remix(
                    self.theme.as_ref(),
                    theme_chooser_remix_seed(),
                );
                self.apply_theme_chooser_theme(remixed, "theme_chooser_action_remix", cx);
            }
            "theme_chooser_reset" => {
                self.preview_current_theme_chooser_preset("theme_chooser_action_reset", false, cx);
            }
            "theme_chooser_save_as_user_theme" => {
                self.save_current_theme_as_user_theme(
                    "theme_chooser_action_save_as_user_theme",
                    cx,
                );
            }
            "theme_chooser_edit_theme_as_text" => {
                match self.materialize_theme_chooser_text_edit_file(cx) {
                    Ok(path) => {
                        if let Err(error) =
                            crate::script_creation::open_in_editor(path.as_path(), &self.config)
                        {
                            self.theme_chooser_management_mut().status =
                                ThemeChooserManagementStatus::Error {
                                    message: format!("Open editor failed: {error}"),
                                };
                            tracing::warn!(%error, path = %path.display(), "theme_chooser_open_text_editor_failed");
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        self.theme_chooser_management_mut().status =
                            ThemeChooserManagementStatus::Error {
                                message: format!("Save for text edit failed: {error}"),
                            };
                        tracing::warn!(%error, "theme_chooser_materialize_text_edit_failed");
                        cx.notify();
                    }
                }
            }
            "theme_chooser_update_user_theme" => {
                self.update_selected_user_theme("theme_chooser_action_update_user_theme", cx);
            }
            "theme_chooser_delete_user_theme" => {
                self.delete_selected_user_theme("theme_chooser_action_delete_user_theme", cx);
            }
            "theme_chooser_restore_deleted_user_theme" => {
                self.restore_last_deleted_user_theme(
                    "theme_chooser_action_restore_deleted_user_theme",
                    cx,
                );
            }
            "theme_chooser_toggle_customize" => {
                self.toggle_theme_chooser_panel_mode(cx);
            }
            "theme_chooser_gradient_cycle" => {
                self.cycle_theme_chooser_gradient("theme_chooser_action_gradient_cycle", cx);
            }
            "theme_chooser_gradient_layer_add" => {
                self.add_theme_chooser_gradient_layer(
                    "theme_chooser_action_gradient_layer_add",
                    cx,
                );
            }
            "theme_chooser_gradient_layer_remove" => {
                let layer_count = self
                    .theme
                    .background_gradient
                    .as_ref()
                    .map(|gradient| gradient.layers.len())
                    .unwrap_or(0);
                if layer_count > 0 {
                    self.remove_theme_chooser_gradient_layer(
                        layer_count - 1,
                        "theme_chooser_action_gradient_layer_remove",
                        cx,
                    );
                }
            }
            "theme_chooser_accent_previous" => {
                self.cycle_theme_chooser_accent(-1, "theme_chooser_action_accent_previous", cx);
            }
            "theme_chooser_accent_next" => {
                self.cycle_theme_chooser_accent(1, "theme_chooser_action_accent_next", cx);
            }
            "theme_chooser_opacity_decrease" => {
                self.adjust_theme_chooser_opacity(-1, "theme_chooser_action_opacity_decrease", cx);
            }
            "theme_chooser_opacity_increase" => {
                self.adjust_theme_chooser_opacity(1, "theme_chooser_action_opacity_increase", cx);
            }
            "theme_chooser_vibrancy_toggle" => {
                self.toggle_theme_chooser_vibrancy("theme_chooser_action_vibrancy_toggle", cx);
            }
            "theme_chooser_material_cycle" => {
                self.cycle_theme_chooser_material("theme_chooser_action_material_cycle", cx);
            }
            "theme_chooser_font_size_decrease" => {
                self.adjust_theme_chooser_font_size(
                    -1,
                    "theme_chooser_action_font_size_decrease",
                    cx,
                );
            }
            "theme_chooser_font_size_increase" => {
                self.adjust_theme_chooser_font_size(
                    1,
                    "theme_chooser_action_font_size_increase",
                    cx,
                );
            }
            _ => {
                tracing::warn!(
                    target: "script_kit::actions",
                    action_id,
                    "Unknown Theme Chooser action"
                );
            }
        }
    }

    /// Apply a surface opacity preset to all shell surfaces together,
    /// so the preview and the real app behave identically.
    fn apply_surface_opacity_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.main = value;
        opacity.title_bar = value;
        opacity.search_box = value;
        opacity.log_panel = value;
        opacity.dialog = value;
        opacity.input = value;
        opacity.panel = value;
        opacity.input_inactive = value;
        opacity.input_active = value;
        opacity.vibrancy_background = Some(value);
        next.opacity = Some(opacity);
        next
    }

    fn apply_glass_veil_opacity_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.glass_veil_opacity = Some(value);
        next.opacity = Some(opacity);
        next
    }

    fn apply_glass_tint_opacity_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.glass_tint_opacity = Some(value);
        next.opacity = Some(opacity);
        next
    }

    fn apply_glass_morph_duration_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.glass_morph_duration = Some(value);
        next.opacity = Some(opacity);
        next
    }

    fn apply_glass_morph_inset_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.glass_morph_inset = Some(value);
        next.opacity = Some(opacity);
        next
    }

    fn apply_text_opacity_preset(theme: &crate::theme::Theme, value: f32) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.text_placeholder = value.clamp(0.0, 1.0);
        opacity.text_hint = (value + 0.05).clamp(0.0, 1.0);
        opacity.text_muted_alpha = (value + 0.15).clamp(0.0, 1.0);
        next.opacity = Some(opacity);
        next
    }

    fn apply_focused_background_opacity_preset(
        theme: &crate::theme::Theme,
        value: f32,
    ) -> crate::theme::Theme {
        let mut next = theme.clone();
        let mut opacity = next.get_opacity();
        opacity.selected = value.clamp(0.0, 1.0);
        next.opacity = Some(opacity);
        next
    }

    /// Build a remixed theme by randomly combining accent, opacity, and vibrancy material.
    fn build_theme_chooser_remix(base: &crate::theme::Theme, seed: usize) -> crate::theme::Theme {
        let mut next = base.clone();

        // Use different bit ranges of the seed for each dimension to avoid correlation
        let accent_index = seed % Self::ACCENT_PALETTE.len();
        let opacity_index = (seed / 7) % Self::OPACITY_PRESETS.len();
        let material_index = (seed / 13) % Self::VIBRANCY_MATERIALS.len();

        let (accent_hex, _) = Self::ACCENT_PALETTE[accent_index];
        let (opacity_value, _) = Self::OPACITY_PRESETS[opacity_index];
        let (material, _) = Self::VIBRANCY_MATERIALS[material_index];

        next.colors.accent.selected = accent_hex;
        next.colors.text.on_accent =
            best_contrast_of_two(accent_hex, 0xFFFFFF, next.colors.background.main);
        next = Self::apply_surface_opacity_preset(&next, opacity_value);
        if let Some(ref mut vibrancy) = next.vibrancy {
            vibrancy.enabled = true;
            vibrancy.material = material;
        }

        next
    }

    /// Render a contrast-safe semantic status chip
    fn render_theme_chooser_semantic_chip(
        label: &'static str,
        colors: theme::SemanticChipColors,
    ) -> gpui::AnyElement {
        div()
            .px(px(8.0))
            .py(px(3.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(rgba(colors.border_rgba))
            .bg(rgba(colors.bg_rgba))
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(rgb(colors.text_hex))
            .child(label)
            .into_any_element()
    }

    /// Render a launcher-style live preview that matches the main menu shell
    fn render_theme_chooser_list_item_preview_rows(&self) -> gpui::AnyElement {
        let list_colors = crate::list_item::ListItemColors::from_theme(self.theme.as_ref());

        div()
            .flex()
            .flex_col()
            .child(
                crate::list_item::ListItem::new("Selected Item", list_colors)
                    .description("Description appears only on the focused row")
                    .selected(true),
            )
            .child(crate::list_item::ListItem::new("Regular Item", list_colors))
            .child(crate::list_item::ListItem::new("Another Item", list_colors).tool_badge("ts"))
            .into_any_element()
    }

    fn render_theme_chooser_live_preview(
        &self,
        preset_name: &str,
        accent_name: &str,
        chrome: &theme::AppChromeColors,
    ) -> gpui::AnyElement {
        let header = div().w_full().flex().flex_row().items_center().child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(chrome.text_primary_hex))
                        .child(preset_name.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(chrome.text_muted_hex))
                        .child(format!("{accent_name} accent · live launcher preview")),
                ),
        );

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .child(self.render_theme_chooser_list_item_preview_rows());

        // Use the shared minimal-list prompt shell so the preview inherits the
        // same spacing, divider, and footer contract as real launcher surfaces.
        crate::components::render_minimal_list_prompt_shell(
            8.0,
            crate::ui_foundation::get_vibrancy_background(self.theme.as_ref()),
            header,
            content,
            Self::theme_chooser_hint_items(),
            None,
        )
        .text_color(rgb(chrome.text_primary_hex))
        .into_any_element()
    }

    /// Render the theme chooser with search, live preview, and preview panel
    pub(crate) fn render_theme_chooser(
        &mut self,
        filter: &str,
        selected_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let chrome = theme::AppChromeColors::from_theme(self.theme.as_ref());
        let text_primary = chrome.text_primary_hex;
        let text_secondary = chrome.text_secondary_hex;
        let text_muted = chrome.text_muted_hex;
        let accent_color = chrome.accent_hex;
        let text_on_accent = self.theme.colors.text.on_accent;
        let ui_success = self.theme.colors.ui.success;
        let ui_error = self.theme.colors.ui.error;
        let ui_warning = self.theme.colors.ui.warning;
        let ui_info = self.theme.colors.ui.info;
        let divider_bg = rgba(chrome.divider_rgba);
        let badge_border_bg = rgba(chrome.badge_border_rgba);
        let catalog = std::sync::Arc::new(Self::theme_chooser_catalog());
        let first_light = catalog
            .iter()
            .position(|entry| {
                !entry.is_dark && matches!(entry.kind, ThemeChooserCatalogKind::BuiltIn(_))
            })
            .unwrap_or(0);
        let original_match =
            Self::cached_theme_chooser_original_match(self.theme_before_chooser.as_ref());
        let original_index = original_match
            .as_ref()
            .and_then(|m| {
                catalog.iter().position(|entry| {
                    matches!(&entry.kind, ThemeChooserCatalogKind::BuiltIn(index) if *index == m.preset_index)
                })
            })
            .unwrap_or(0);
        let original_is_exact = original_match
            .as_ref()
            .map(|m| m.is_exact())
            .unwrap_or(false);

        // Filter built-in and user themes by name or description.
        let filtered_indices = std::sync::Arc::new(Self::theme_chooser_catalog_filtered_indices(
            filter, &catalog,
        ));
        let filtered_count = filtered_indices.len();
        self.sync_theme_chooser_list_state(filtered_count);
        let filter_is_empty = filter.is_empty();

        let summary = Self::theme_chooser_match_summary(&filtered_indices, &catalog);
        // Visible/total match count surfaced beside the search input.
        let header_count_label = if filter_is_empty {
            format!("{} themes", summary.catalog_total)
        } else {
            format!("{} / {}", summary.visible_total, summary.catalog_total)
        };
        let entity_handle = cx.entity().downgrade();
        let catalog_for_keys = std::sync::Arc::clone(&catalog);

        // ── Keyboard handler ───────────────────────────────────────
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);
                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::ThemeChooser,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        tracing::debug!(
                            target: "script_kit::actions",
                            event = "builtin_view_actions_key_routed",
                            surface = "theme_chooser",
                            key = %key,
                        );
                        cx.stop_propagation();
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::ThemeChooser,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                        return;
                    }
                }

                // Escape: clear filter first if present, otherwise undo all changes and close.
                // Undo restores the opening snapshot in memory only — nothing was
                // persisted during the session, so a pure cancel never writes to disk.
                if is_key_escape(key) && !this.show_actions_popup {
                    if !this.clear_builtin_view_filter(cx) {
                        if let Some(original) = this.theme_before_chooser.take() {
                            if !this.restore_theme_chooser_theme(original, "theme_chooser_escape_undo", cx) {
                                cx.stop_propagation();
                                return;
                            }
                        }
                        this.clear_theme_chooser_controls();
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                // Cmd+W: undo all changes (memory-only restore) and close window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    if let Some(original) = this.theme_before_chooser.take() {
                        if !this.restore_theme_chooser_theme(original, "theme_chooser_close_undo", cx) {
                            cx.stop_propagation();
                            return;
                        }
                    }
                    this.clear_theme_chooser_controls();
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+E: toggle the right panel between Preview and Customize
                if has_cmd && key.eq_ignore_ascii_case("e") {
                    this.toggle_theme_chooser_panel_mode(cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+[ / Cmd+]: cycle accent colors
                if has_cmd && (key == "[" || key.eq_ignore_ascii_case("bracketleft")) {
                    let current = this.theme.colors.accent.selected;
                    let idx = Self::find_accent_palette_index(current).unwrap_or(0);
                    let new_idx = if idx == 0 {
                        Self::ACCENT_PALETTE.len() - 1
                    } else {
                        idx - 1
                    };
                    let (new_accent, _) = Self::ACCENT_PALETTE[new_idx];
                    let mut modified = (*this.theme).clone();
                    modified.colors.accent.selected = new_accent;
                    modified.colors.text.on_accent =
                        best_contrast_of_two(new_accent, 0xFFFFFF, modified.colors.background.main);
                    this.apply_theme_chooser_theme(modified, "theme_chooser_accent_cycle", cx);
                    cx.stop_propagation();
                    return;
                }
                if has_cmd && (key == "]" || key.eq_ignore_ascii_case("bracketright")) {
                    let current = this.theme.colors.accent.selected;
                    let idx = Self::find_accent_palette_index(current).unwrap_or(0);
                    let new_idx = (idx + 1) % Self::ACCENT_PALETTE.len();
                    let (new_accent, _) = Self::ACCENT_PALETTE[new_idx];
                    let mut modified = (*this.theme).clone();
                    modified.colors.accent.selected = new_accent;
                    modified.colors.text.on_accent =
                        best_contrast_of_two(new_accent, 0xFFFFFF, modified.colors.background.main);
                    this.apply_theme_chooser_theme(modified, "theme_chooser_accent_cycle", cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+- / Cmd+=: adjust surface opacity (all shell surfaces together)
                if has_cmd && key == "-" {
                    let current_main = this.theme.get_opacity().main;
                    let idx = Self::find_opacity_preset_index(current_main);
                    if idx > 0 {
                        let target = Self::OPACITY_PRESETS[idx - 1].0;
                        let modified =
                            Self::apply_surface_opacity_preset(this.theme.as_ref(), target);
                        this.apply_theme_chooser_theme(
                            modified,
                            "theme_chooser_opacity_decrease",
                            cx,
                        );
                    }
                    cx.stop_propagation();
                    return;
                }
                if has_cmd && (key == "=" || key == "+") {
                    let current_main = this.theme.get_opacity().main;
                    let idx = Self::find_opacity_preset_index(current_main);
                    if idx < Self::OPACITY_PRESETS.len() - 1 {
                        let target = Self::OPACITY_PRESETS[idx + 1].0;
                        let modified =
                            Self::apply_surface_opacity_preset(this.theme.as_ref(), target);
                        this.apply_theme_chooser_theme(
                            modified,
                            "theme_chooser_opacity_increase",
                            cx,
                        );
                    }
                    cx.stop_propagation();
                    return;
                }
                // Cmd+B: toggle vibrancy
                if has_cmd && key.eq_ignore_ascii_case("b") {
                    let mut modified = (*this.theme).clone();
                    if let Some(ref mut vibrancy) = modified.vibrancy {
                        vibrancy.enabled = !vibrancy.enabled;
                    }
                    this.apply_theme_chooser_theme(modified, "theme_chooser_vibrancy_toggle", cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+M: cycle vibrancy material
                if has_cmd && key.eq_ignore_ascii_case("m") {
                    let current_material = this
                        .theme
                        .vibrancy
                        .as_ref()
                        .map(|v| v.material)
                        .unwrap_or_default();
                    let idx = Self::find_vibrancy_material_index(current_material);
                    let new_idx = (idx + 1) % Self::VIBRANCY_MATERIALS.len();
                    let (new_material, _) = Self::VIBRANCY_MATERIALS[new_idx];
                    let mut modified = (*this.theme).clone();
                    if let Some(ref mut vibrancy) = modified.vibrancy {
                        vibrancy.material = new_material;
                    }
                    this.apply_theme_chooser_theme(
                        modified,
                        "theme_chooser_vibrancy_material_cycle",
                        cx,
                    );
                    cx.stop_propagation();
                    return;
                }
                // Cmd+J: surprise me / remix
                if has_cmd && key.eq_ignore_ascii_case("j") {
                    let remixed = Self::build_theme_chooser_remix(
                        this.theme.as_ref(),
                        theme_chooser_remix_seed(),
                    );
                    this.apply_theme_chooser_theme(remixed, "theme_chooser_surprise_me", cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+R: reset customizations to selected preset defaults
                if has_cmd && key.eq_ignore_ascii_case("r") {
                    let current_filter =
                        if let AppView::ThemeChooserView { ref filter, .. } = this.current_view {
                            filter.clone()
                        } else {
                            return;
                        };
                    let filtered = Self::theme_chooser_catalog_filtered_indices(
                        &current_filter,
                        &catalog_for_keys,
                    );
                    if let AppView::ThemeChooserView {
                        ref selected_index, ..
                    } = this.current_view
                    {
                        this.preview_theme_chooser_catalog_entry(
                            &catalog_for_keys,
                            &filtered,
                            *selected_index,
                            "theme_chooser_reset_shortcut",
                            false,
                            cx,
                        );
                    }
                    cx.stop_propagation();
                    return;
                }
                // Compute filtered indices from current filter
                let current_filter =
                    if let AppView::ThemeChooserView { ref filter, .. } = this.current_view {
                        filter.clone()
                    } else {
                        return;
                    };
                let filtered = Self::theme_chooser_catalog_filtered_indices(
                    &current_filter,
                    &catalog_for_keys,
                );
                let count = filtered.len();
                if count == 0 {
                    if is_key_up(key)
                        || is_key_down(key)
                        || key.eq_ignore_ascii_case("home")
                        || key.eq_ignore_ascii_case("end")
                        || key.eq_ignore_ascii_case("pageup")
                        || key.eq_ignore_ascii_case("pagedown")
                    {
                        cx.stop_propagation();
                    }
                    return;
                }

                if let AppView::ThemeChooserView {
                    ref mut selected_index,
                    ..
                } = this.current_view
                {
                    let page_size: usize = THEME_LIST_PAGE_SIZE;
                    match key {
                        _ if is_key_up(key) => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                            }
                        }
                        _ if is_key_down(key) => {
                            if *selected_index < count - 1 {
                                *selected_index += 1;
                            }
                        }
                        _ if key.eq_ignore_ascii_case("home") => {
                            *selected_index = 0;
                        }
                        _ if key.eq_ignore_ascii_case("end") => {
                            *selected_index = count - 1;
                        }
                        _ if key.eq_ignore_ascii_case("pageup") => {
                            *selected_index = selected_index.saturating_sub(page_size);
                        }
                        _ if key.eq_ignore_ascii_case("pagedown") => {
                            *selected_index = (*selected_index + page_size).min(count - 1);
                        }
                        _ => return,
                    }
                    // Copy index before calling &mut self method
                    let idx = *selected_index;
                    // Map to actual preset index and apply theme
                    this.preview_theme_chooser_catalog_entry(
                        &catalog_for_keys,
                        &filtered,
                        idx,
                        "theme_chooser_keyboard_preview",
                        false,
                        cx,
                    );
                    cx.stop_propagation();
                }
            },
        );

        // ── Pre-compute data for list closure ──────────────────────
        let selected = selected_index;
        let orig_idx = original_index;
        let orig_exact = original_is_exact;
        let first_light_idx = first_light;
        let filtered_indices_for_list = std::sync::Arc::clone(&filtered_indices);
        let catalog_for_list = std::sync::Arc::clone(&catalog);
        let entity_handle_for_customize = entity_handle.clone();
        let accent_badge_border = rgba(chrome.accent_badge_border_rgba);
        let accent_badge_bg = rgba(chrome.accent_badge_bg_rgba);
        let accent_badge_text = rgb(chrome.accent_badge_text_hex);
        let list_colors = crate::list_item::ListItemColors::from_theme(self.theme.as_ref());

        // ── Theme list (shared ListItem rows) ─────────────────────
        let list = list(
            self.theme_chooser_list_state.clone(),
            move |ix, _window, _cx| {
                let catalog_idx = filtered_indices_for_list[ix];
                let entry = &catalog_for_list[catalog_idx];
                let is_selected = ix == selected;
                let is_original = catalog_idx == orig_idx;
                let name = entry.name.as_str();
                let desc = entry.description.as_str();
                let colors = &entry.preview_colors;
                let is_first_light = filter_is_empty
                    && catalog_idx == first_light_idx
                    && first_light_idx > 0
                    && matches!(entry.kind, ThemeChooserCatalogKind::BuiltIn(_));

                // Compact color bar — thin horizontal strip showing theme palette
                let color_bar = div()
                    .flex()
                    .flex_row()
                    .w(px(40.0))
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .child(div().flex_1().bg(rgb(colors.bg)))
                    .child(div().flex_1().bg(rgb(colors.accent)))
                    .child(div().flex_1().bg(rgb(colors.text)))
                    .child(div().flex_1().bg(rgb(colors.secondary)))
                    .child(div().flex_1().bg(rgb(colors.border)));

                // Badge for original theme — "Saved" if exact match, "Modified" if remixed
                let is_user_theme = matches!(&entry.kind, ThemeChooserCatalogKind::User { .. });
                let saved_badge = if is_original || is_user_theme {
                    let label = if is_user_theme {
                        "User"
                    } else if orig_exact {
                        "Saved"
                    } else {
                        "Modified"
                    };
                    Some(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(accent_badge_border)
                            .bg(accent_badge_bg)
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(accent_badge_text)
                            .child(label)
                            .into_any_element(),
                    )
                } else {
                    None
                };

                // Section label for light themes (only when unfiltered)
                let section_label = if is_first_light {
                    Some(crate::list_item::render_section_header(
                        "LIGHT",
                        None,
                        list_colors,
                        false,
                    ))
                } else {
                    None
                };

                // Click handler: select + preview via captured Arc indices
                let click_entity = entity_handle.clone();
                let click_indices = std::sync::Arc::clone(&filtered_indices_for_list);
                let click_catalog = std::sync::Arc::clone(&catalog_for_list);
                let click_handler =
                    move |_event: &gpui::ClickEvent, _window: &mut Window, cx: &mut gpui::App| {
                        cx.stop_propagation();
                        if let Some(app) = click_entity.upgrade() {
                            let indices = std::sync::Arc::clone(&click_indices);
                            let catalog = std::sync::Arc::clone(&click_catalog);
                            app.update(cx, |this, cx| {
                                if let AppView::ThemeChooserView {
                                    ref mut selected_index,
                                    ..
                                } = this.current_view
                                {
                                    *selected_index = ix;
                                }
                                this.preview_theme_chooser_catalog_entry(
                                    &catalog,
                                    &indices,
                                    ix,
                                    "theme_chooser_mouse_click",
                                    false,
                                    cx,
                                );
                            });
                        }
                    };

                // Build shared ListItem row — matches main menu rendering
                let item = crate::list_item::ListItem::new(name.to_string(), list_colors)
                    .description(desc.to_string())
                    .selected(is_selected)
                    .index(ix)
                    .leading_accessory(color_bar)
                    .trailing_accessory_opt(saved_badge);

                let row = div()
                    .id(ix)
                    .cursor_pointer()
                    .on_click(click_handler)
                    .child(item);

                if let Some(label) = section_label {
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(label)
                        .child(row)
                        .into_any_element()
                } else {
                    row.into_any_element()
                }
            },
        )
        .h_full()
        .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
        .into_any_element();

        // ── Header with search input + summary strip ─────────────────
        // Resolve selected preset name for live preview header
        let selected_preset_name = filtered_indices
            .get(selected_index)
            .and_then(|idx| catalog.get(*idx))
            .map(|entry| entry.name.as_str())
            .unwrap_or("Theme Preview");
        let selected_catalog_entry =
            Self::theme_chooser_selected_entry(&catalog, &filtered_indices, selected_index);
        let management_snapshot = self.theme_chooser_management_snapshot(selected_catalog_entry);

        // Resolve contrast-safe semantic chip colors
        let success_chip = chrome.semantic_chip_colors(self.theme.as_ref(), ui_success);
        let error_chip = chrome.semantic_chip_colors(self.theme.as_ref(), ui_error);
        let warning_chip = chrome.semantic_chip_colors(self.theme.as_ref(), ui_warning);
        let info_chip = chrome.semantic_chip_colors(self.theme.as_ref(), ui_info);

        // ── Preview panel with customization controls (Sliders & ColorPickers) ──
        self.ensure_theme_chooser_controls(window, cx);
        self.sync_theme_chooser_control_values(window, cx);
        let Some(controls) = self.theme_chooser_controls.as_ref() else {
            tracing::error!("theme_chooser.controls_unavailable_after_initialization");
            return crate::components::render_simple_empty_state(
                "theme-chooser-controls-unavailable",
                "Theme controls could not be loaded",
                "alert-circle",
                None,
                &self.theme,
                cx,
            );
        };
        let opacity = self.theme.get_opacity();
        let fonts = self.theme.get_fonts();
        let accent_name_str = Self::accent_color_name(accent_color);

        let save_click_entity = entity_handle_for_customize.clone();
        let update_click_entity = entity_handle_for_customize.clone();
        let delete_click_entity = entity_handle_for_customize.clone();
        let restore_click_entity = entity_handle_for_customize.clone();
        let update_disabled = management_snapshot.update_disabled.is_some();
        let delete_disabled = management_snapshot.delete_disabled.is_some();
        let restore_disabled = management_snapshot.restore_disabled.is_some();
        let management_button_colors =
            crate::components::ButtonColors::from_theme(self.theme.as_ref());
        let save_copy_hover_rgba = crate::components::Button::hover_background_token(
            crate::components::ButtonVariant::Primary,
            management_button_colors,
        );
        let neutral_hover_rgba = crate::components::Button::hover_background_token(
            crate::components::ButtonVariant::Ghost,
            management_button_colors,
        );
        let destructive_hover_rgba =
            Self::theme_chooser_overlay_token(ui_error, management_button_colors.hover_overlay);
        let management_section = Self::render_theme_chooser_customize_section(
            "SAVE & MANAGE",
            Some("Library status and custom theme actions"),
            &chrome,
            vec![
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .id("status:theme-chooser-dirty-state")
                            .debug_selector(|| "status:theme-chooser-dirty-state".to_string())
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(if management_snapshot.is_dirty {
                                accent_color
                            } else {
                                text_secondary
                            }))
                            .child(management_snapshot.status_label.clone()),
                    )
                    .child(div().text_xs().text_color(rgb(text_muted)).child(format!(
                        "Save copy name: {}",
                        management_snapshot.resolved_save_name
                    )))
                    .into_any_element(),
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-save-copy-button",
                        "Save Copy",
                        ThemeChooserManagementButtonKind::Primary,
                        false,
                        ThemeChooserManagementButtonStyle {
                            text_hex: chrome.accent_badge_text_hex,
                            bg_rgba: chrome.accent_badge_bg_rgba,
                            border_rgba: chrome.accent_badge_border_rgba,
                            hover_rgba: save_copy_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = save_click_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.save_current_theme_as_user_theme(
                                        "theme_chooser_save_copy_click",
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-update-user-theme-button",
                        "Update",
                        ThemeChooserManagementButtonKind::Neutral,
                        update_disabled,
                        ThemeChooserManagementButtonStyle {
                            text_hex: if update_disabled {
                                text_muted
                            } else {
                                text_primary
                            },
                            bg_rgba: chrome.whisper_surface_rgba,
                            border_rgba: chrome.whisper_border_rgba,
                            hover_rgba: neutral_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = update_click_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.update_selected_user_theme(
                                        "theme_chooser_update_user_theme_click",
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-delete-user-theme-button",
                        "Delete",
                        ThemeChooserManagementButtonKind::Destructive,
                        delete_disabled,
                        ThemeChooserManagementButtonStyle {
                            text_hex: if delete_disabled {
                                text_muted
                            } else {
                                ui_error
                            },
                            bg_rgba: chrome.whisper_surface_rgba,
                            border_rgba: chrome.whisper_border_rgba,
                            hover_rgba: destructive_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = delete_click_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.delete_selected_user_theme(
                                        "theme_chooser_delete_user_theme_click",
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-restore-user-theme-button",
                        "Restore",
                        ThemeChooserManagementButtonKind::Neutral,
                        restore_disabled,
                        ThemeChooserManagementButtonStyle {
                            text_hex: if restore_disabled {
                                text_muted
                            } else {
                                text_primary
                            },
                            bg_rgba: chrome.whisper_surface_rgba,
                            border_rgba: chrome.whisper_border_rgba,
                            hover_rgba: neutral_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = restore_click_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.restore_last_deleted_user_theme(
                                        "theme_chooser_restore_deleted_user_theme_click",
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .into_any_element(),
            ],
        );

        // 1. COLORS SECTION
        let colors_section = Self::render_theme_chooser_customize_section(
            "COLORS",
            Some("Interactive base and selection colors"),
            &chrome,
            vec![
                Self::render_theme_chooser_color_picker_row(
                    "Accent Color",
                    accent_color,
                    &controls.accent,
                    &chrome,
                ),
                Self::render_theme_chooser_color_picker_row(
                    "Background Color",
                    self.theme.colors.background.main,
                    &controls.background,
                    &chrome,
                ),
            ],
        );

        // 2. OPACITY & VIBRANCY SECTION
        let main_opacity_slider_row = Self::render_theme_chooser_slider_row(
            "Surface Opacity",
            format!("{:.0}%", opacity.main * 100.0),
            &controls.surface_opacity,
            &chrome,
        );
        let text_opacity_slider_row = Self::render_theme_chooser_slider_row(
            "Typography Hint Opacity",
            format!("{:.0}%", opacity.text_placeholder * 100.0),
            &controls.secondary_text_opacity,
            &chrome,
        );
        let focused_opacity_slider_row = Self::render_theme_chooser_slider_row(
            "Focused Row Opacity",
            format!("{:.0}%", opacity.selected * 100.0),
            &controls.focused_background_opacity,
            &chrome,
        );
        let glass_slider_rows = crate::platform::tahoe_liquid_glass_available().then(|| {
            (
                Self::render_theme_chooser_slider_row(
                    "Glass Veil Opacity",
                    format!(
                        "{:.0}%",
                        opacity
                            .glass_veil_opacity
                            .unwrap_or(crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP)
                            * 100.0
                    ),
                    &controls.glass_veil_opacity,
                    &chrome,
                ),
                Self::render_theme_chooser_slider_row(
                    "Glass Tint Opacity",
                    format!("{:.0}%", opacity.glass_tint_opacity.unwrap_or(0.0) * 100.0),
                    &controls.glass_tint_opacity,
                    &chrome,
                ),
                Self::render_theme_chooser_slider_row(
                    "Glass Morph Duration",
                    format!(
                        "{:.0} ms",
                        opacity
                            .glass_morph_duration
                            .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION)
                            * 1000.0
                    ),
                    &controls.glass_morph_duration,
                    &chrome,
                ),
                Self::render_theme_chooser_slider_row(
                    "Glass Morph Inset",
                    format!(
                        "{:.0}%",
                        opacity
                            .glass_morph_inset
                            .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET)
                            * 100.0
                    ),
                    &controls.glass_morph_inset,
                    &chrome,
                ),
            )
        });

        let vibrancy_enabled = self
            .theme
            .vibrancy
            .as_ref()
            .map(|vibrancy| vibrancy.enabled)
            .unwrap_or(false);
        let vibrancy_click_entity = entity_handle_for_customize.clone();
        let vibrancy_row = div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(text_secondary))
                    .child("Vibrancy (macOS panel vibrancy)"),
            )
            .child(
                div()
                    .id("vibrancy-toggle-btn")
                    .w(px(32.0))
                    .h(px(18.0))
                    .rounded(px(9.0))
                    .cursor_pointer()
                    .bg(if vibrancy_enabled {
                        rgb(accent_color)
                    } else {
                        badge_border_bg
                    })
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(7.0))
                            .bg(if vibrancy_enabled {
                                rgb(text_on_accent)
                            } else {
                                rgb(text_primary)
                            })
                            .when(vibrancy_enabled, |d| d.ml(px(16.0)))
                            .when(!vibrancy_enabled, |d| d.ml(px(2.0))),
                    )
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        if let Some(app) = vibrancy_click_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                this.toggle_theme_chooser_vibrancy(
                                    "theme_chooser_vibrancy_click",
                                    cx,
                                );
                            });
                        }
                    }),
            )
            .into_any_element();

        let current_material = self.theme.get_vibrancy().material;
        let material_segments = Self::VIBRANCY_MATERIALS
            .iter()
            .enumerate()
            .map(|(ix, (material, label))| {
                let material = *material;
                let click_entity = entity_handle_for_customize.clone();
                Self::render_theme_chooser_segment_button(
                    ("theme-chooser-material-segment", ix),
                    (*label).to_string(),
                    current_material == material,
                    &chrome,
                    move |_, _, cx| {
                        if let Some(app) = click_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                this.set_theme_chooser_vibrancy_material(
                                    material,
                                    "theme_chooser_material_segment_click",
                                    cx,
                                );
                            });
                        }
                    },
                )
            })
            .collect::<Vec<_>>();
        let material_row = Self::render_theme_chooser_segmented_row(
            "Vibrancy Material",
            material_segments,
            &chrome,
        );

        let mut opacity_section_rows = vec![
            main_opacity_slider_row,
            text_opacity_slider_row,
            focused_opacity_slider_row,
        ];
        if let Some((
            glass_veil_row,
            glass_tint_row,
            glass_morph_duration_row,
            glass_morph_inset_row,
        )) = glass_slider_rows
        {
            opacity_section_rows.push(glass_veil_row);
            opacity_section_rows.push(glass_tint_row);
            opacity_section_rows.push(glass_morph_duration_row);
            opacity_section_rows.push(glass_morph_inset_row);
        }
        opacity_section_rows.push(vibrancy_row);
        opacity_section_rows.push(material_row);
        let opacity_section = Self::render_theme_chooser_customize_section(
            "OPACITY & VIBRANCY",
            Some("Vibrancy blend and layer transparency"),
            &chrome,
            opacity_section_rows,
        );

        // APPEARANCE SECTION — auto/light/dark resolution for the theme
        let current_appearance = self.theme.appearance;
        let appearance_segments = [
            ("Auto", crate::theme::types::AppearanceMode::Auto),
            ("Light", crate::theme::types::AppearanceMode::Light),
            ("Dark", crate::theme::types::AppearanceMode::Dark),
        ]
        .into_iter()
        .enumerate()
        .map(|(ix, (label, mode))| {
            let click_entity = entity_handle_for_customize.clone();
            Self::render_theme_chooser_segment_button(
                ("theme-chooser-appearance-segment", ix),
                label.to_string(),
                current_appearance == mode,
                &chrome,
                move |_, _, cx| {
                    if let Some(app) = click_entity.upgrade() {
                        app.update(cx, |this, cx| {
                            this.set_theme_chooser_appearance_mode(
                                mode,
                                "theme_chooser_appearance_segment_click",
                                cx,
                            );
                        });
                    }
                },
            )
        })
        .collect::<Vec<_>>();
        let appearance_section = Self::render_theme_chooser_customize_section(
            "APPEARANCE",
            Some("How the theme resolves against system light/dark"),
            &chrome,
            vec![Self::render_theme_chooser_segmented_row(
                "Appearance Mode",
                appearance_segments,
                &chrome,
            )],
        );

        // 3. BACKGROUNDS & GRADIENTS SECTION
        let gradient_enabled = self
            .theme
            .background_gradient
            .as_ref()
            .map(|gradient| gradient.enabled)
            .unwrap_or(false);
        let grad_click_entity = entity_handle_for_customize.clone();
        let mut gradient_children = vec![div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(text_secondary))
                    .child("Enable Backdrop Gradient"),
            )
            .child(
                div()
                    .id("gradient-toggle-btn")
                    .w(px(32.0))
                    .h(px(18.0))
                    .rounded(px(9.0))
                    .cursor_pointer()
                    .bg(if gradient_enabled {
                        rgb(accent_color)
                    } else {
                        badge_border_bg
                    })
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(7.0))
                            .bg(if gradient_enabled {
                                rgb(text_on_accent)
                            } else {
                                rgb(text_primary)
                            })
                            .when(gradient_enabled, |d| d.ml(px(16.0)))
                            .when(!gradient_enabled, |d| d.ml(px(2.0))),
                    )
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        if let Some(app) = grad_click_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                let mut modified = (*this.theme).clone();
                                if let Some(ref mut grad) = modified.background_gradient {
                                    grad.enabled = !grad.enabled;
                                } else {
                                    modified.background_gradient =
                                        Some(theme::BackgroundGradient {
                                            enabled: true,
                                            ..Default::default()
                                        });
                                }
                                this.apply_theme_chooser_theme(
                                    modified,
                                    "theme_chooser_gradient_toggle_click",
                                    cx,
                                );
                            });
                        }
                    }),
            )
            .into_any_element()];

        if gradient_enabled {
            gradient_children.push(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .border_t_1()
                    .border_color(divider_bg)
                    .pt(px(8.0))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(text_secondary))
                            .child("Gradient Base"),
                    )
                    .child(
                        self.render_theme_chooser_gradient_layer_controls(
                            "Base Layer".to_string(),
                            gradient_enabled,
                            ThemeChooserGradientValues {
                                from: self
                                    .theme
                                    .background_gradient
                                    .as_ref()
                                    .map(|gradient| gradient.from)
                                    .unwrap_or_default(),
                                to: self
                                    .theme
                                    .background_gradient
                                    .as_ref()
                                    .map(|gradient| gradient.to)
                                    .unwrap_or_default(),
                                angle: self
                                    .theme
                                    .background_gradient
                                    .as_ref()
                                    .map(|gradient| gradient.angle)
                                    .unwrap_or_default(),
                                opacity: self
                                    .theme
                                    .background_gradient
                                    .as_ref()
                                    .map(|gradient| gradient.opacity)
                                    .unwrap_or(1.0),
                            },
                            &controls.gradient_base,
                            &chrome,
                        ),
                    )
                    .into_any_element(),
            );
            if let Some(gradient) = self.theme.background_gradient.as_ref() {
                for (i, layer) in gradient.layers.iter().enumerate() {
                    let Some(layer_controls) = controls.gradient_layers.get(i) else {
                        continue;
                    };
                    gradient_children.push(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .border_t_1()
                            .border_color(divider_bg)
                            .pt(px(8.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(text_secondary))
                                    .child(format!("Gradient Layer {}", i + 1)),
                            )
                            .child(self.render_theme_chooser_gradient_layer_controls(
                                format!("Layer {}", i + 1),
                                layer.enabled,
                                ThemeChooserGradientValues {
                                    from: layer.from,
                                    to: layer.to,
                                    angle: layer.angle,
                                    opacity: layer.opacity,
                                },
                                layer_controls,
                                &chrome,
                            ))
                            .into_any_element(),
                    );
                }
            }
        }

        if gradient_enabled {
            let layer_count = self
                .theme
                .background_gradient
                .as_ref()
                .map(|gradient| gradient.layers.len())
                .unwrap_or(0);
            let add_layer_entity = entity_handle_for_customize.clone();
            let remove_layer_entity = entity_handle_for_customize.clone();
            let remove_disabled = layer_count == 0;
            gradient_children.push(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .border_t_1()
                    .border_color(divider_bg)
                    .pt(px(8.0))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-gradient-layer-add-button",
                        "Add Layer",
                        ThemeChooserManagementButtonKind::Neutral,
                        false,
                        ThemeChooserManagementButtonStyle {
                            text_hex: text_primary,
                            bg_rgba: chrome.whisper_surface_rgba,
                            border_rgba: chrome.whisper_border_rgba,
                            hover_rgba: neutral_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = add_layer_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.add_theme_chooser_gradient_layer(
                                        "theme_chooser_gradient_layer_add_click",
                                        cx,
                                    );
                                });
                            }
                        },
                    ))
                    .child(Self::render_theme_chooser_management_button(
                        "theme-chooser-gradient-layer-remove-button",
                        "Remove Layer",
                        ThemeChooserManagementButtonKind::Neutral,
                        remove_disabled,
                        ThemeChooserManagementButtonStyle {
                            text_hex: if remove_disabled {
                                text_muted
                            } else {
                                text_primary
                            },
                            bg_rgba: chrome.whisper_surface_rgba,
                            border_rgba: chrome.whisper_border_rgba,
                            hover_rgba: neutral_hover_rgba,
                        },
                        move |_, _, cx| {
                            cx.stop_propagation();
                            if let Some(app) = remove_layer_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    let layer_count = this
                                        .theme
                                        .background_gradient
                                        .as_ref()
                                        .map(|gradient| gradient.layers.len())
                                        .unwrap_or(0);
                                    if layer_count > 0 {
                                        this.remove_theme_chooser_gradient_layer(
                                            layer_count - 1,
                                            "theme_chooser_gradient_layer_remove_click",
                                            cx,
                                        );
                                    }
                                });
                            }
                        },
                    ))
                    .into_any_element(),
            );
        }

        let backgrounds_section = Self::render_theme_chooser_customize_section(
            "BACKDROP & GRADIENTS",
            Some("Layered backgrounds and linear blends"),
            &chrome,
            gradient_children,
        );

        // 4. TYPOGRAPHY SECTION
        let typography_section = Self::render_theme_chooser_customize_section(
            "TYPOGRAPHY",
            Some("Global interface scale and font sizing"),
            &chrome,
            vec![Self::render_theme_chooser_slider_row(
                "UI Font Size",
                format!("{:.1} px", fonts.ui_size),
                &controls.ui_font_size,
                &chrome,
            )],
        );

        let customizer_scroller = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .overflow_y_scrollbar()
            .child(colors_section)
            .child(opacity_section)
            .child(appearance_section)
            .child(backgrounds_section)
            .child(typography_section)
            .child(management_section);

        // ── Right panel mode tabs (Preview / Customize) ────────────
        let panel_mode = self.theme_chooser_panel_mode;
        let is_customize = panel_mode == ThemeChooserPanelMode::Customize;
        let mode_tabs = {
            let preview_entity = entity_handle_for_customize.clone();
            let customize_entity = entity_handle_for_customize.clone();
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(Self::render_theme_chooser_segment_button(
                    ("theme-chooser-panel-tab", 0),
                    "Preview".to_string(),
                    !is_customize,
                    &chrome,
                    move |_, _, cx| {
                        if let Some(app) = preview_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                this.set_theme_chooser_panel_mode(
                                    ThemeChooserPanelMode::Preview,
                                    cx,
                                );
                            });
                        }
                    },
                ))
                .child(Self::render_theme_chooser_segment_button(
                    ("theme-chooser-panel-tab", 1),
                    "Customize".to_string(),
                    is_customize,
                    &chrome,
                    move |_, _, cx| {
                        if let Some(app) = customize_entity.upgrade() {
                            app.update(cx, |this, cx| {
                                this.set_theme_chooser_panel_mode(
                                    ThemeChooserPanelMode::Customize,
                                    cx,
                                );
                            });
                        }
                    },
                ))
        };

        // ── Preview-mode theme facts card ──────────────────────────
        let facts_card = {
            let vibrancy = self.theme.get_vibrancy();
            let material_label = Self::VIBRANCY_MATERIALS
                .iter()
                .find(|(material, _)| *material == vibrancy.material)
                .map(|(_, label)| *label)
                .unwrap_or("HUD");
            let appearance_label = match self.theme.appearance {
                crate::theme::types::AppearanceMode::Auto => "Auto",
                crate::theme::types::AppearanceMode::Light => "Light",
                crate::theme::types::AppearanceMode::Dark => "Dark",
            };
            let source_label = match management_snapshot.base_slug.as_deref() {
                Some(slug) if !slug.starts_with("builtin:") => {
                    format!("User theme · {slug}.json")
                }
                _ => "Built-in preset".to_string(),
            };
            let gradient_label = match self.theme.background_gradient.as_ref() {
                Some(gradient) if gradient.enabled => {
                    format!("On · {} layer(s)", gradient.layers.len() + 1)
                }
                _ => "Off".to_string(),
            };
            Self::render_theme_chooser_customize_section(
                "THEME FACTS",
                Some("Live values for the previewed theme"),
                &chrome,
                vec![
                    Self::render_theme_chooser_fact_row(
                        "Status",
                        management_snapshot.status_label.clone(),
                        &chrome,
                        None,
                    )
                    .id("status:theme-chooser-dirty-state")
                    .debug_selector(|| "status:theme-chooser-dirty-state".to_string())
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row("Source", source_label, &chrome, None)
                        .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Accent",
                        format!(
                            "{} {}",
                            accent_name_str,
                            Self::canonical_theme_chooser_hex_label(accent_color)
                        ),
                        &chrome,
                        Some(accent_color),
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Background",
                        Self::canonical_theme_chooser_hex_label(self.theme.colors.background.main),
                        &chrome,
                        Some(self.theme.colors.background.main),
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Appearance",
                        format!(
                            "{} · {} colors",
                            appearance_label,
                            if self.theme.has_dark_colors() {
                                "dark"
                            } else {
                                "light"
                            }
                        ),
                        &chrome,
                        None,
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Vibrancy",
                        format!(
                            "{} · {}",
                            if vibrancy.enabled { "On" } else { "Off" },
                            material_label
                        ),
                        &chrome,
                        None,
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Surface Opacity",
                        format!("{:.0}%", opacity.main * 100.0),
                        &chrome,
                        None,
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "Backdrop Gradient",
                        gradient_label,
                        &chrome,
                        None,
                    )
                    .into_any_element(),
                    Self::render_theme_chooser_fact_row(
                        "UI Font Size",
                        format!("{:.1} px", fonts.ui_size),
                        &chrome,
                        None,
                    )
                    .into_any_element(),
                ],
            )
        };

        let chips_row = div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(Self::render_theme_chooser_semantic_chip("OK", success_chip))
            .child(Self::render_theme_chooser_semantic_chip("Err", error_chip))
            .child(Self::render_theme_chooser_semantic_chip(
                "Warn",
                warning_chip,
            ))
            .child(Self::render_theme_chooser_semantic_chip("Info", info_chip));

        let contrast_line = {
            let contrast_snapshot = cached_theme_chooser_contrast_snapshot(&self.theme);
            div()
                .mt(px(4.0))
                .text_xs()
                .text_color(rgb(text_muted))
                .child(format!(
                    "Contrast {}/{} pass · worst {} {:.2}:1",
                    contrast_snapshot.passing,
                    contrast_snapshot.total,
                    contrast_snapshot.worst_label,
                    contrast_snapshot.worst_ratio,
                ))
        };

        let preview_panel = div()
            .id("theme-chooser-preview-panel")
            .w_1_2()
            .h_full()
            .min_h(px(0.0))
            .border_l_1()
            .border_color(divider_bg)
            .px(px(design_spacing.padding_lg))
            .py(px(design_spacing.padding_md))
            .flex()
            .flex_col()
            .gap(px(design_spacing.padding_sm))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(mode_tabs)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(text_muted))
                            .child(format!("Base: {}", selected_preset_name)),
                    ),
            )
            .when(is_customize, |panel| panel.child(customizer_scroller))
            .when(!is_customize, |panel| {
                panel.child(facts_card).child(
                    // Launcher-style live preview fills the remaining space in
                    // Preview mode so theme browsing reads like the real shell.
                    div().flex_1().min_h(px(0.0)).flex().flex_col().child(
                        self.render_theme_chooser_live_preview(
                            selected_preset_name,
                            accent_name_str,
                            &chrome,
                        ),
                    ),
                )
            })
            // ── Semantic status chips + contrast audit (summary only) ──
            .child(chips_row)
            .child(contrast_line)
            // ── Compact launcher-style live preview while editing ──
            .when(is_customize, |panel| {
                panel.child(div().h(px(1.0)).bg(divider_bg)).child(
                    self.render_theme_chooser_live_preview(
                        selected_preset_name,
                        accent_name_str,
                        &chrome,
                    ),
                )
            });

        // ── Footer: three-affordance hint strip per .impeccable.md (Done /
        // Actions / Undo; Customize + Remix discovery lives in ⌘K) ──
        let hints = Self::theme_chooser_hint_items();
        crate::components::emit_surface_prompt_hint_audit(
            "theme_chooser",
            &hints,
            "done_actions_undo_footer",
        );
        let uses_native_footer = self.main_window_uses_native_footer();
        let footer = self.main_window_footer_slot(
            crate::components::prompt_layout_shell::render_simple_hint_strip(hints, None),
        );
        let native_footer_hover_blocker = uses_native_footer.then(|| {
            crate::components::prompt_layout_shell::render_native_main_window_footer_hover_blocker()
                .into_any_element()
        });
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        // ── Main layout: shared list region + preview panel ──────────────
        let results_body = div()
            .relative()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .child(list)
            .vertical_scrollbar(&self.theme_chooser_list_state)
            .into_any_element();
        let empty_body = self.render_theme_chooser_empty_state_body(filter, summary, &chrome);
        let main = render_theme_chooser_browser_main(
            filter,
            filtered_count,
            results_body,
            empty_body,
            preview_panel.into_any_element(),
            list_colors,
        );

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(text_primary))
                .font_family(self.theme_font_family())
                .key_context("theme_chooser")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![self.render_builtin_main_input_count_label(header_count_label)],
                    cx,
                ),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main,
                footer,
                overlays: native_footer_hover_blocker.into_iter().collect(),
            },
        )
    }
}

#[cfg(test)]
include!("theme_chooser_tests.rs");
