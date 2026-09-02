/// Completion of one exact queued footer event. Enqueueing is not activation:
/// the evaluator must pump bounded owned work and poll this ticket.
pub(crate) struct FooterActionCompletion {
    receiver: async_channel::Receiver<Result<(), &'static str>>,
    completed: std::cell::Cell<bool>,
}

impl FooterActionCompletion {
    pub(crate) fn poll(&self) -> anyhow::Result<bool> {
        if self.completed.get() { return Ok(true); }
        match self.receiver.try_recv() {
            Ok(Ok(())) => { self.completed.set(true); Ok(true) }
            Ok(Err(reason)) => anyhow::bail!(reason),
            Err(async_channel::TryRecvError::Empty) => Ok(false),
            Err(async_channel::TryRecvError::Closed) => anyhow::bail!("footer_handler_did_not_complete"),
        }
    }
}

fn footer_fixture_handle(id: &str, generation: u64, cx: &App) -> anyhow::Result<WindowHandle<GpuiFooterOverlay>> {
    anyhow::ensure!(generation > 0, "footer_generation_missing");
    let exact_handle = crate::windows::get_runtime_window_handle_for_generation(id, generation)
        .ok_or_else(|| anyhow::anyhow!("footer_lifetime_stale"))?;
    let (handle, binding) = {
        let hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        hosts.values().find_map(|host| {
            let slot = host.overlay.as_ref()?;
            (slot.info.id == id && slot.info.generation == Some(generation) && AnyWindowHandle::from(slot.handle) == exact_handle)
                .then(|| host.binding.clone().map(|binding| (slot.handle, binding))).flatten()
        }).ok_or_else(|| anyhow::anyhow!("footer_overlay_missing"))?
    };
    let info = crate::windows::automation_window_by_id(id).ok_or_else(|| anyhow::anyhow!("footer_metadata_missing"))?;
    anyhow::ensure!(info.generation == Some(generation)
        && info.parent_window_id.as_deref() == Some(binding.window_id.as_str())
        && info.parent_window_generation == Some(binding.window_generation), "footer_parent_stale");
    let parent = crate::windows::get_runtime_window_handle_for_generation(&binding.window_id, binding.window_generation)
        .ok_or_else(|| anyhow::anyhow!("footer_parent_stale"))?;
    anyhow::ensure!(footer_binding_is_live(&binding, parent), "footer_parent_stale");
    let overlay = handle.read(cx)?;
    anyhow::ensure!(overlay.binding == binding && overlay.painted_binding.as_ref() == Some(&binding)
        && overlay.painted_frame_generation > 0, "footer_projection_unpainted_or_stale");
    Ok(handle)
}

fn footer_button_element(index: usize, button: &FooterButtonConfig, held: Option<FooterAction>) -> crate::protocol::ElementInfo {
    let enabled = button.enabled && button.disabled_reason.is_none();
    let mut element = crate::protocol::ElementInfo::button(index, button.label.as_ref());
    element.semantic_id = button.id.to_string();
    element.selected = Some(button.selected);
    element.selectable = Some(enabled);
    element.role = Some("footerAction".into());
    element.kind = Some(button.action.semantic_key().into());
    element.action_disabled = button.disabled_reason.as_ref().map(ToString::to_string)
        .or_else(|| (!enabled).then(|| "footer_action_disabled".into()));
    element.status_kind = Some(if !enabled { "disabled" } else if held == Some(button.action) { "held" } else if button.selected { "selected" } else { "ready" }.into());
    element
}

fn footer_elements(config: &MainWindowFooterConfig, held: Option<FooterAction>) -> Vec<crate::protocol::ElementInfo> {
    let mut elements: Vec<_> = config.buttons.iter().enumerate().map(|(index, button)| footer_button_element(index, button, held)).collect();
    if let Some(info) = &config.left_info {
        let mut element = if let Some(action) = info.action {
            let button = FooterButtonConfig::new(action, info.keycap.clone().unwrap_or_default(), info.model_name.clone()).selected(info.selected);
            footer_button_element(elements.len(), &button, held)
        } else {
            let mut element = crate::protocol::ElementInfo::panel("footer-left-info");
            element.text = Some(info.model_name.clone());
            element.selected = Some(info.selected);
            element.selectable = Some(false);
            element.role = Some("status".into());
            element
        };
        element.source = Some("footer-left-info".into());
        if !elements.iter().any(|existing| existing.semantic_id == element.semantic_id) { elements.push(element); }
    }
    elements
}

pub(crate) fn footer_fixture_elements(id: &str, generation: u64, cx: &App) -> anyhow::Result<Vec<crate::protocol::ElementInfo>> {
    let handle = footer_fixture_handle(id, generation, cx)?;
    let state = footer_runtime_state(id, generation).ok_or_else(|| anyhow::anyhow!("footer_owner_retired"))?;
    Ok(footer_elements(&handle.read(cx)?.config, state.held_action))
}

pub(crate) fn footer_fixture_select(
    id: &str, generation: u64, semantic_id: &str, submit: bool, cx: &mut App,
) -> anyhow::Result<Option<FooterActionCompletion>> {
    crate::runtime_policy::WindowHostPolicy::OwnedHidden.validate()?;
    if !submit {
        return Err(crate::protocol::TransactionError {
            code: crate::protocol::TransactionErrorCode::UnsupportedCommand,
            message: "selection_only_unsupported".into(),
            suggestion: Some("Use explicit submit:true to activate a footer action".into()),
        }.into());
    }
    anyhow::ensure!(crate::windows::runtime_window_host_policy(id, generation)? == crate::runtime_policy::WindowHostPolicy::OwnedHidden, "footer_owned_host_required");
    let handle = footer_fixture_handle(id, generation, cx)?;
    let overlay = handle.read(cx)?;
    let action = if let Some(button) = overlay.config.buttons.iter().find(|button| button.id.as_ref() == semantic_id) {
        anyhow::ensure!(button.enabled && button.disabled_reason.is_none(), "{}", button.disabled_reason.as_ref().map(|reason| reason.as_str()).unwrap_or("footer_action_disabled"));
        button.action
    } else if let Some(action) = overlay.config.left_info.as_ref().and_then(|info| info.action)
        .filter(|action| semantic_id == format!("footer-action:{}", action.semantic_key())) {
        action
    } else { return Ok(None); };
    let (sender, receiver) = async_channel::bounded(1);
    enqueue_bound_footer_action(&overlay.binding, action, Some(sender)).map_err(anyhow::Error::msg)?;
    Ok(Some(FooterActionCompletion { receiver, completed: std::cell::Cell::new(false) }))
}

pub(crate) fn footer_fixture_layout(id: &str, generation: u64, cx: &mut App) -> anyhow::Result<crate::protocol::LayoutInfo> {
    use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
    let handle = footer_fixture_handle(id, generation, cx)?;
    handle.update(cx, |overlay, window, _| {
        let frame = window.rendered_frame_generation();
        anyhow::ensure!(frame > 0 && frame == overlay.painted_frame_generation, "footer_layout_unpainted_or_stale");
        let mut selectors = vec![(GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID.to_string(), LayoutComponentType::Panel, None)];
        selectors.extend(overlay.config.buttons.iter().map(|button| (
            format!("agent-chat.footer-overlay.{}", button.id), LayoutComponentType::Button, Some(button.id.to_string()),
        )));
        if let Some(info) = &overlay.config.left_info {
            selectors.push(("agent-chat.footer-overlay.profile".into(),
                if info.action.is_some() { LayoutComponentType::Button } else { LayoutComponentType::Container },
                Some(info.action.map(|action| format!("footer-action:{}", action.semantic_key())).unwrap_or_else(|| "panel:footer-left-info".into()))));
            if !info.model_name.trim().is_empty() { selectors.push(("agent-chat.footer-overlay.model".into(), LayoutComponentType::Other, None)); }
            if info.spinner_glyph.as_ref().is_some_and(|glyph| !glyph.trim().is_empty()) { selectors.push(("agent-chat.footer-overlay.spinner".into(), LayoutComponentType::Other, None)); }
        }
        let mut components = Vec::with_capacity(selectors.len());
        for (selector, component_type, semantic_id) in selectors {
            let entry = window.debug_bounds_entries().iter().rev().find(|entry| entry.selector == selector)
                .ok_or_else(|| anyhow::anyhow!("footer_layout_unavailable:{selector}"))?;
            let bounds = entry.bounds;
            let visible = entry.visible_bounds;
            let clip = entry.clip_bounds;
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            anyhow::ensure!(width.is_finite() && height.is_finite() && width >= 0.0 && height >= 0.0, "footer_layout_invalid:{selector}");
            let mut component = LayoutComponentInfo::new(&selector, component_type)
                .with_bounds(f32::from(bounds.origin.x), f32::from(bounds.origin.y), width, height)
                .with_measurement("paint-time", "window")
                .with_measurement_frame(frame)
                .with_paint_visibility(f32::from(visible.origin.x), f32::from(visible.origin.y), f32::from(visible.size.width), f32::from(visible.size.height), f32::from(clip.origin.x), f32::from(clip.origin.y), f32::from(clip.size.width), f32::from(clip.size.height));
            component.semantic_id = semantic_id;
            if selector != GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID { component = component.with_parent(GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID); }
            components.push(component);
        }
        let viewport = window.viewport_size();
        Ok(LayoutInfo { window_width: f32::from(viewport.width), window_height: f32::from(viewport.height), prompt_type: "footerOverlay".into(), components, timestamp: chrono::Utc::now().to_rfc3339(), ..Default::default() })
    })?
}

fn record_footer_held_action(binding: &FooterBinding, action: Option<FooterAction>) -> bool {
    let Some(handle) = crate::windows::get_runtime_window_handle_for_generation(&binding.window_id, binding.window_generation) else { return false; };
    let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let Some(host) = hosts.get_mut(&handle.window_id()) else { return false; };
    if host.binding.as_ref() != Some(binding) || host.held_action == action { return false; }
    host.held_action = action;
    host.interaction_revision += 1;
    true
}
