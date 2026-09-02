impl ScriptListApp {
    fn theme_chooser_hex_to_hsla(hex: u32) -> gpui::Hsla {
        rgb(hex).into()
    }

    fn theme_chooser_hsla_to_hex_rgb(color: gpui::Hsla) -> Option<u32> {
        let hex = color.to_hex().to_string();
        let trimmed = hex.trim_start_matches('#');
        if trimmed.len() < 6 {
            return None;
        }
        u32::from_str_radix(&trimmed[..6], 16).ok()
    }

    fn parse_theme_chooser_hex_input(value: &str) -> Option<u32> {
        let trimmed = value.trim().trim_start_matches('#');
        if trimmed.len() != 6 {
            return None;
        }
        u32::from_str_radix(trimmed, 16).ok()
    }

    fn canonical_theme_chooser_hex_label(hex: u32) -> String {
        format!("#{:06X}", hex)
    }

    fn theme_chooser_featured_colors() -> Vec<gpui::Hsla> {
        Self::ACCENT_PALETTE
            .iter()
            .map(|&(hex, _)| Self::theme_chooser_hex_to_hsla(hex))
            .collect()
    }

    fn new_theme_chooser_slider(
        &self,
        binding: ThemeChooserSliderBinding,
        range: ThemeChooserSliderRange,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> gpui::Entity<SliderState> {
        let ThemeChooserSliderRange {
            min,
            max,
            step,
            initial,
        } = range;
        let slider = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(initial)
        });
        subscriptions.push(cx.subscribe_in(
            &slider,
            window,
            move |this, _, event: &SliderEvent, _window, cx| match event {
                SliderEvent::Change(value) => {
                    this.apply_theme_chooser_slider_drag_change(binding, *value, cx);
                }
                SliderEvent::Release(value) => {
                    this.apply_theme_chooser_slider_change(binding, *value, cx);
                }
            },
        ));
        slider
    }

    fn new_theme_chooser_color_controls(
        &self,
        binding: ThemeChooserColorBinding,
        initial_hex: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> ThemeChooserColorControls {
        let initial = Self::theme_chooser_hex_to_hsla(initial_hex);
        let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(initial));
        subscriptions.push(cx.subscribe_in(
            &picker,
            window,
            move |this, _, event: &ColorPickerEvent, _window, cx| match event {
                ColorPickerEvent::Change(Some(color)) => {
                    this.apply_theme_chooser_color_change(binding, *color, cx);
                }
                ColorPickerEvent::Change(None) => {}
            },
        ));

        let hex_input = cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .tab_navigation(true)
                .placeholder("#RRGGBB")
                .default_value(Self::canonical_theme_chooser_hex_label(initial_hex))
        });
        subscriptions.push(cx.subscribe_in(
            &hex_input,
            window,
            move |this, input, event: &gpui_component::input::InputEvent, window, cx| match event {
                gpui_component::input::InputEvent::Change => {
                    let value = input.read(cx).value().to_string();
                    this.apply_theme_chooser_hex_text_if_valid(binding, &value, cx);
                }
                gpui_component::input::InputEvent::PressEnter { .. }
                | gpui_component::input::InputEvent::Blur => {
                    this.sync_theme_chooser_control_values(window, cx);
                }
                _ => {}
            },
        ));

        ThemeChooserColorControls { picker, hex_input }
    }

    fn new_theme_chooser_gradient_controls(
        &self,
        layer_index: Option<usize>,
        values: ThemeChooserGradientValues,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> ThemeChooserGradientControls {
        let ThemeChooserGradientValues {
            from,
            to,
            angle,
            opacity,
        } = values;
        ThemeChooserGradientControls {
            from: self.new_theme_chooser_color_controls(
                ThemeChooserColorBinding::GradientFrom { layer_index },
                from,
                window,
                cx,
                subscriptions,
            ),
            to: self.new_theme_chooser_color_controls(
                ThemeChooserColorBinding::GradientTo { layer_index },
                to,
                window,
                cx,
                subscriptions,
            ),
            angle: self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GradientAngle { layer_index },
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 360.0,
                    step: 1.0,
                    initial: angle.rem_euclid(360.0),
                },
                window,
                cx,
                subscriptions,
            ),
            opacity: self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GradientOpacity { layer_index },
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity.clamp(0.0, 1.0),
                },
                window,
                cx,
                subscriptions,
            ),
        }
    }

    fn ensure_theme_chooser_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let opacity = self.theme.get_opacity();
        let fonts = self.theme.get_fonts();
        let gradient = self.theme.background_gradient.clone().unwrap_or_default();
        let needs_init = self.theme_chooser_controls.is_none();
        if needs_init {
            let mut subscriptions = Vec::new();
            let accent = self.new_theme_chooser_color_controls(
                ThemeChooserColorBinding::Accent,
                self.theme.colors.accent.selected,
                window,
                cx,
                &mut subscriptions,
            );
            let background = self.new_theme_chooser_color_controls(
                ThemeChooserColorBinding::Background,
                self.theme.colors.background.main,
                window,
                cx,
                &mut subscriptions,
            );
            let surface_opacity = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::SurfaceOpacity,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity.main,
                },
                window,
                cx,
                &mut subscriptions,
            );
            let secondary_text_opacity = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::SecondaryTextOpacity,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity.text_placeholder,
                },
                window,
                cx,
                &mut subscriptions,
            );
            let focused_background_opacity = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::FocusedBackgroundOpacity,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity.selected,
                },
                window,
                cx,
                &mut subscriptions,
            );
            let glass_veil_opacity = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GlassVeilOpacity,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity
                        .glass_veil_opacity
                        .unwrap_or(crate::theme::opacity::OPACITY_GLASS_MODE_VEIL_CAP),
                },
                window,
                cx,
                &mut subscriptions,
            );
            let glass_tint_opacity = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GlassTintOpacity,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity.glass_tint_opacity.unwrap_or(0.0),
                },
                window,
                cx,
                &mut subscriptions,
            );
            let glass_morph_duration = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GlassMorphDuration,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                    initial: opacity
                        .glass_morph_duration
                        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_DURATION),
                },
                window,
                cx,
                &mut subscriptions,
            );
            let glass_morph_inset = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::GlassMorphInset,
                ThemeChooserSliderRange {
                    min: 0.0,
                    max: 0.4,
                    step: 0.01,
                    initial: opacity
                        .glass_morph_inset
                        .unwrap_or(crate::theme::opacity::GLASS_MORPH_DEFAULT_INSET),
                },
                window,
                cx,
                &mut subscriptions,
            );
            let ui_font_size = self.new_theme_chooser_slider(
                ThemeChooserSliderBinding::UiFontSize,
                ThemeChooserSliderRange {
                    min: 12.0,
                    max: 24.0,
                    step: 0.5,
                    initial: fonts.ui_size,
                },
                window,
                cx,
                &mut subscriptions,
            );
            let gradient_base = self.new_theme_chooser_gradient_controls(
                None,
                ThemeChooserGradientValues {
                    from: gradient.from,
                    to: gradient.to,
                    angle: gradient.angle,
                    opacity: gradient.opacity,
                },
                window,
                cx,
                &mut subscriptions,
            );
            let gradient_layers = gradient
                .layers
                .iter()
                .enumerate()
                .map(|(index, layer)| {
                    self.new_theme_chooser_gradient_controls(
                        Some(index),
                        ThemeChooserGradientValues {
                            from: layer.from,
                            to: layer.to,
                            angle: layer.angle,
                            opacity: layer.opacity,
                        },
                        window,
                        cx,
                        &mut subscriptions,
                    )
                })
                .collect();
            self.theme_chooser_controls = Some(ThemeChooserControls {
                accent,
                background,
                surface_opacity,
                secondary_text_opacity,
                focused_background_opacity,
                glass_veil_opacity,
                glass_tint_opacity,
                glass_morph_duration,
                glass_morph_inset,
                ui_font_size,
                gradient_base,
                gradient_layers,
                subscriptions,
            });
        }
        self.reconcile_theme_chooser_gradient_controls(window, cx);
    }

    fn reconcile_theme_chooser_gradient_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(controls) = self.theme_chooser_controls.as_ref() else {
            return;
        };
        let current_layer_count = self
            .theme
            .background_gradient
            .as_ref()
            .map(|gradient| gradient.layers.len())
            .unwrap_or(0);
        if controls.gradient_layers.len() == current_layer_count {
            return;
        }
        self.theme_chooser_controls = None;
        self.ensure_theme_chooser_controls(window, cx);
    }
}
