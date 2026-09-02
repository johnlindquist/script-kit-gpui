// Shared by ordinary queued input and native completed-frame identity observation.
pub(crate) fn live_gpui_target_identity(
    main: Option<&gpui::Entity<ScriptListApp>>,
    info: &protocol::AutomationWindowInfo,
    window: &gpui::Window,
    cx: &gpui::App,
) -> anyhow::Result<protocol::AutomationTargetIdentitySnapshot> {
    live_gpui_target_identity_from_main(
        main.map(|entity| (entity.read(cx), entity.entity_id())),
        info,
        window,
        cx,
    )
}

fn live_gpui_target_identity_from_main(
    main: Option<(&ScriptListApp, gpui::EntityId)>,
    info: &protocol::AutomationWindowInfo,
    window: &gpui::Window,
    cx: &gpui::App,
) -> anyhow::Result<protocol::AutomationTargetIdentitySnapshot> {
    use anyhow::Context as _;
    let generation = info.generation.context("window_generation_missing")?;
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(&info.id, generation)
            == Some(window.window_handle()),
        "target_runtime_handle_mismatch"
    );
    let (surface, data, presentation, applied_theme, variant) =
        if info.kind == protocol::AutomationWindowKind::Main {
            let (app, entity_id) = main.context("main_owner_missing")?;
            let root = Root::read(window, cx);
            anyhow::ensure!(root.view().entity_id() == entity_id, "main_owner_mismatch");
            let facts = app.owned_revision_facts();
            let data = facts
                .data_generation
                .checked_add(app.gpui_input_state.read(cx).revision())
                .and_then(|revision| revision.checked_add(app.arg_input.revision()))
                .and_then(|revision| revision.checked_add(app.owned_child_semantic_revision(cx)))
                .and_then(|revision| revision.checked_add(root.layer_revision()))
                .context("main_data_revision_exhausted")?;
            (
                facts.surface_generation,
                data,
                facts.presentation_revision,
                app.theme_revision_seen,
                app.current_view.app_view_variant().to_owned(),
            )
        } else if info.kind == protocol::AutomationWindowKind::PromptPopup
            && info.semantic_surface.as_deref() == Some("footerOverlay")
        {
            let state = crate::footer_popup::footer_runtime_state(&info.id, generation)
                .context("footer_owner_missing")?;
            (
                state.binding.host_generation,
                state.semantic_revision,
                state.presentation_revision,
                state.applied_theme_revision,
                "GpuiFooterOverlay".into(),
            )
        } else if info.id == "shortcut-recorder-popup" {
            let state = crate::shortcut_recorder::shortcut_fixture_observation(
                &info.id,
                generation,
                Some(window),
                cx,
            )?;
            (
                1,
                state.data_revision,
                state.presentation_revision,
                state
                    .applied_theme_revision
                    .context("shortcut_not_painted")?,
                "ShortcutRecorderPopupWindow".into(),
            )
        } else {
            let (surface, data, presentation, theme) =
                crate::windows::automation_surface_collector::surface_revision_facts(
                    info,
                    Some(window),
                    cx,
                )
                .context("owner_revision_unavailable")?;
            let variant = if info.kind == protocol::AutomationWindowKind::Notes {
                "NotesApp".into()
            } else {
                info.semantic_surface
                    .clone()
                    .unwrap_or_else(|| info.kind.as_camel_case().to_owned())
            };
            (surface, data, presentation, theme, variant)
        };
    Ok(protocol::AutomationTargetIdentitySnapshot {
        window_id: info.id.clone(),
        window_generation: Some(generation),
        app_view_variant: variant,
        target_generation: crate::windows::automation_registry::automation_target_revision(
            &info.id, generation,
        )
        .context("target_revision_missing")?,
        surface_generation: surface,
        data_generation: data,
        presentation_revision: Some(presentation),
        theme_revision: Some(applied_theme),
        frame_generation: Some(window.rendered_frame_generation()),
    })
}

fn validate_gpui_expected_identity(
    expected: &protocol::AutomationTargetIdentitySnapshot,
    actual: &protocol::AutomationTargetIdentitySnapshot,
) -> Result<(), String> {
    if actual.window_id != expected.window_id
        || actual.window_generation != expected.window_generation
        || actual.app_view_variant != expected.app_view_variant
        || actual.target_generation != expected.target_generation
        || actual.surface_generation != expected.surface_generation
        || actual.data_generation != expected.data_generation
        || actual.presentation_revision != expected.presentation_revision
        || actual.theme_revision != expected.theme_revision
        || expected.frame_generation.is_some_and(|expected| {
            actual
                .frame_generation
                .is_none_or(|actual| actual < expected)
        })
    {
        return Err("stale_target_identity".into());
    }
    Ok(())
}

fn gpui_dispatch_precondition(
    message: &mut protocol::Message,
    main: gpui::WeakEntity<ScriptListApp>,
) -> Option<crate::platform::gpui_event_simulator::GpuiDispatchPrecondition> {
    let protocol::Message::SimulateGpuiEvent {
        expected,
        expected_frame,
        event,
        ..
    } = message
    else {
        return None;
    };
    let coordinate_event = !matches!(event, protocol::SimulatedGpuiEvent::KeyDown { .. });
    let expected = expected.take();
    let expected_frame = expected_frame.take();
    if expected.is_none() && expected_frame.is_none() && !coordinate_event {
        return None;
    }
    Some(Box::new(move |info, window, cx| {
        if coordinate_event && window.is_owned_hidden() && expected_frame.is_none() {
            return Err("expected_frame_required".into());
        }
        if expected.is_none() && expected_frame.is_none() {
            return Ok(());
        }
        let main = main.upgrade();
        let actual = live_gpui_target_identity(main.as_ref(), info, window, cx)
            .map_err(|error| error.to_string())?;
        if let Some(expected) = expected.as_ref() {
            validate_gpui_expected_identity(expected, &actual)?;
        }
        if let Some(frame) = expected_frame.as_ref() {
            if expected
                .as_ref()
                .is_some_and(|expected| *expected != frame.target)
            {
                return Err("expected_frame_owner_mismatch".into());
            }
            #[cfg(target_os = "macos")]
            crate::computer_use::owned_render_capture::validate_owned_frame_for_input(
                frame, &actual, window,
            )
            .map_err(|error| error.to_string())?;
            #[cfg(not(target_os = "macos"))]
            return Err("owned_render_capture_unsupported".into());
        }
        Ok(())
    }))
}
