impl ScriptListApp {
    /// Project the canonical protocol layout model into the optional debug-grid overlay.
    ///
    /// The overlay is a visualization adapter, not an independent geometry owner.
    /// Keeping it downstream of `build_layout_info` removes the former 695-line
    /// duplicate AppView sizing table and ensures model/debug rectangles share one
    /// source and one active theme resolution.
    fn build_component_bounds(
        &mut self,
        window_size: gpui::Size<gpui::Pixels>,
        cx: &mut gpui::Context<Self>,
    ) -> Vec<debug_grid::ComponentBounds> {
        use debug_grid::{BoxModel, ComponentBounds, ComponentType};

        let layout = self.build_layout_info(
            Some((
                window_size.width.as_f32(),
                window_size.height.as_f32(),
            )),
            cx,
        );

        layout
            .components
            .into_iter()
            .map(|component| {
                let component_type = match component.component_type {
                    protocol::LayoutComponentType::Prompt => ComponentType::Prompt,
                    protocol::LayoutComponentType::Input => ComponentType::Input,
                    protocol::LayoutComponentType::Button => ComponentType::Button,
                    protocol::LayoutComponentType::List => ComponentType::List,
                    protocol::LayoutComponentType::ListItem => ComponentType::ListItem,
                    protocol::LayoutComponentType::Header => ComponentType::Header,
                    protocol::LayoutComponentType::Container
                    | protocol::LayoutComponentType::Panel => ComponentType::Container,
                    protocol::LayoutComponentType::Other
                    | protocol::LayoutComponentType::Unknown => ComponentType::Other,
                };
                let padding = component
                    .box_model
                    .as_ref()
                    .and_then(|model| model.padding.as_ref())
                    .map(|padding| BoxModel {
                        top: padding.top,
                        right: padding.right,
                        bottom: padding.bottom,
                        left: padding.left,
                    })
                    .unwrap_or_else(|| BoxModel::uniform(0.0));
                let margin = component
                    .box_model
                    .as_ref()
                    .and_then(|model| model.margin.as_ref())
                    .map(|margin| BoxModel {
                        top: margin.top,
                        right: margin.right,
                        bottom: margin.bottom,
                        left: margin.left,
                    })
                    .unwrap_or_else(|| BoxModel::uniform(0.0));

                ComponentBounds::new(
                    component.name,
                    gpui::Bounds {
                        origin: gpui::point(
                            px(component.bounds.x),
                            px(component.bounds.y),
                        ),
                        size: gpui::size(
                            px(component.bounds.width),
                            px(component.bounds.height),
                        ),
                    },
                )
                .with_type(component_type)
                .with_padding(padding)
                .with_margin(margin)
            })
            .collect()
    }
}
