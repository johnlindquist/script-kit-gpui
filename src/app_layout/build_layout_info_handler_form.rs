impl ScriptListApp {
    fn build_handler_form_layout_info(
        &self,
        content_top: f32,
        content_height: f32,
        window_width: f32,
    ) -> Option<serde_json::Value> {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.menu_syntax_capture_form_owns_input()
        {
            return None;
        }
        let form = self
            .menu_syntax_main_hint_snapshot(&self.filter_text, false)?
            .form?;

        const OUTER_PADDING_X: f32 = 18.0;
        const OUTER_PADDING_TOP: f32 = 12.0;
        const TITLE_BLOCK_HEIGHT: f32 = 72.0;
        const FIELD_HEIGHT: f32 = 68.0;
        const FIELD_GAP: f32 = 8.0;
        const SIDEBAR_WIDTH: f32 = 180.0;
        const SIDEBAR_GAP: f32 = 8.0;

        let offset = self.menu_syntax_main_hint_scroll_handle.offset();
        let max_offset = self.menu_syntax_main_hint_scroll_handle.max_offset();
        let scroll_offset_y = (-offset.y.as_f32()).max(0.0);
        let viewport = serde_json::json!({
            "x": OUTER_PADDING_X,
            "y": content_top + OUTER_PADDING_TOP,
            "width": (window_width - (OUTER_PADDING_X * 2.0)).max(0.0),
            "height": (content_height - OUTER_PADDING_TOP).max(0.0),
        });
        let viewport_top = content_top + OUTER_PADDING_TOP;
        let viewport_bottom = content_top + content_height;
        let form_top = content_top + OUTER_PADDING_TOP + TITLE_BLOCK_HEIGHT;
        let content_width = (window_width - (OUTER_PADDING_X * 2.0)).max(0.0);
        let sidebar_field_id = self.menu_syntax_form_suggestion_field_id.as_deref();
        let field_width = content_width;

        let mut focused_visibility = serde_json::Value::Null;
        let fields = form
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let y = form_top + (index as f32 * (FIELD_HEIGHT + FIELD_GAP)) - scroll_offset_y;
                let bottom = y + FIELD_HEIGHT;
                let visible_top = y.max(viewport_top);
                let visible_bottom = bottom.min(viewport_bottom);
                let visible_height = (visible_bottom - visible_top).clamp(0.0, FIELD_HEIGHT);
                let visible_ratio = if FIELD_HEIGHT > 0.0 {
                    visible_height / FIELD_HEIGHT
                } else {
                    0.0
                };
                let fully_visible = visible_ratio >= 0.999;
                let semantic_id = format!("handler-form:{}:{}", form.target, field.id);
                let bounds = serde_json::json!({
                    "x": OUTER_PADDING_X,
                    "y": y,
                    "width": field_width,
                    "height": FIELD_HEIGHT,
                });
                let field_info = serde_json::json!({
                    "semanticId": semantic_id,
                    "fieldId": field.id,
                    "index": index,
                    "focused": field.focused && self.menu_syntax_form_input_active,
                    "fullyVisible": fully_visible,
                    "visibleRatio": visible_ratio,
                    "handlerFormFieldBounds": bounds,
                });
                if field.focused && self.menu_syntax_form_input_active {
                    focused_visibility = serde_json::json!({
                        "semanticId": format!("handler-form:{}:{}", form.target, field.id),
                        "index": index,
                        "fullyVisible": fully_visible,
                        "visibleRatio": visible_ratio,
                        "bounds": bounds,
                    });
                }
                field_info
            })
            .collect::<Vec<_>>();

        let suggestions = sidebar_field_id.and_then(|field_id| {
            form.fields
                .iter()
                .any(|field| field.id == field_id)
                .then(|| {
                    let field_count = form.fields.len() as f32;
                    let sidebar_height = (field_count * FIELD_HEIGHT
                        + (field_count - 1.0).max(0.0) * FIELD_GAP)
                        .min((content_height - OUTER_PADDING_TOP - TITLE_BLOCK_HEIGHT).max(0.0));
                    serde_json::json!({
                        "ownerFieldId": field_id,
                        "role": "listbox",
                        "surface": "menuSyntaxTriggerPicker",
                        "bounds": {
                            "x": OUTER_PADDING_X + field_width + SIDEBAR_GAP,
                            "y": form_top - scroll_offset_y,
                            "width": SIDEBAR_WIDTH,
                            "height": sidebar_height,
                        }
                    })
                })
        });

        Some(serde_json::json!({
            "target": form.target,
            "source": "menuSyntaxMainHint.form",
            "scrollContainerId": "menu-syntax-main-hint-scroll",
            "scrollOffsetY": scroll_offset_y,
            "maxScrollOffsetY": max_offset.y.as_f32().max(0.0),
            "viewport": viewport,
            "focusedSemanticId": if self.menu_syntax_form_input_active {
                form.fields
                    .iter()
                    .find(|field| field.focused)
                    .map(|field| format!("handler-form:{}:{}", form.target, field.id))
            } else {
                None
            },
            "focusedIndex": if self.menu_syntax_form_input_active {
                Some(form.focused_index)
            } else {
                None
            },
            "handlerFormFocusedVisibility": focused_visibility,
            "fields": fields,
            "suggestions": suggestions,
        }))
    }
}
