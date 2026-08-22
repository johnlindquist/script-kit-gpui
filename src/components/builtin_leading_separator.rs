use gpui::{div, InteractiveElement, IntoElement, ParentElement, Styled};

#[allow(
    dead_code,
    reason = "separately compiled builtin renderers and paint measurement own this stable selector"
)]
pub(crate) const BUILTIN_LEADING_SEPARATOR_ID: &str = "builtin-leading-separator";

/// Shared persistent first row for builtin list browsers.
///
/// The row is always present. Transient status belongs in its label so status
/// changes cannot add floating chrome or shift the list below it (OF-15).
#[allow(
    dead_code,
    reason = "the separately compiled launcher calls this from seven builtin renderers"
)]
pub(crate) fn render_builtin_leading_separator(
    label: &str,
    status: Option<&str>,
    colors: crate::list_item::ListItemColors,
) -> impl IntoElement {
    let text = status.map_or_else(|| label.to_string(), |status| format!("{label} · {status}"));
    div()
        .debug_selector(|| BUILTIN_LEADING_SEPARATOR_ID.to_string())
        .w_full()
        .child(crate::list_item::render_section_header(
            &text, None, colors, true,
        ))
}

/// Semantic mirror of [`render_builtin_leading_separator`] for getElements.
#[allow(
    dead_code,
    reason = "the separately compiled launcher projects this separator in app_layout/collect_elements.rs"
)]
pub(crate) fn builtin_leading_separator_element(
    surface: &str,
    label: &str,
    status: Option<&str>,
) -> crate::protocol::ElementInfo {
    let mut element = crate::protocol::ElementInfo::panel(&format!("{surface}-leading-separator"));
    element.text =
        Some(status.map_or_else(|| label.to_string(), |status| format!("{label} · {status}")));
    element.role = Some("sectionHeader".to_string());
    element.kind = Some("leadingSeparator".to_string());
    element.selectable = Some(false);
    element.status_kind = status.map(str::to_string);
    element
}

#[cfg(test)]
mod tests {
    use gpui::AppContext;

    struct TestBuiltinLeadingSeparator;

    impl gpui::Render for TestBuiltinLeadingSeparator {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let theme = crate::theme::Theme::default();
            super::render_builtin_leading_separator(
                "Files",
                Some("Indexing files"),
                crate::list_item::ListItemColors::from_theme(&theme),
            )
        }
    }

    #[test]
    fn semantic_status_stays_inside_the_leading_separator() {
        let element = super::builtin_leading_separator_element(
            "file-search",
            "Files",
            Some("Indexing files"),
        );
        assert_eq!(element.role.as_deref(), Some("sectionHeader"));
        assert_eq!(element.kind.as_deref(), Some("leadingSeparator"));
        assert_eq!(element.text.as_deref(), Some("Files · Indexing files"));
        assert_eq!(element.status_kind.as_deref(), Some("Indexing files"));
        assert_eq!(element.selectable, Some(false));
    }

    #[gpui::test]
    fn rendered_leading_separator_records_non_zero_paint_bounds(cx: &mut gpui::TestAppContext) {
        use gpui::px;

        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(480.0), px(160.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestBuiltinLeadingSeparator))
                .unwrap()
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let measurement = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| entry.selector == super::BUILTIN_LEADING_SEPARATOR_ID)
                    .expect("builtin leading separator should record paint bounds");

                assert!(measurement.bounds.size.width > px(0.0));
                assert!(measurement.bounds.size.height > px(0.0));
            })
            .unwrap();
    }
}
