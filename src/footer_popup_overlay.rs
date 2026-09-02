#[cfg(all(any(test, feature = "owned-ui-evaluation"), target_os = "macos"))]
pub(crate) const OWNED_FOOTER_FIXTURE_IDS: &[&str] = &["secondary.footer"];

fn close_footer_overlay_for_parent(parent: AnyWindowHandle, cx: &mut App) {
    let slot = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&parent.window_id()).and_then(|host| {
        host.fidelity = None;
        host.overlay.take()
    });
    if let Some(slot) = slot {
        if let Some(generation) = slot.info.generation {
            crate::windows::remove_runtime_window_instance(&slot.info.id, generation);
        }
        let _ = slot.handle.update(cx, |_, window, _| window.remove_window());
    }
}

pub(crate) fn retire_footer_owner(parent: AnyWindowHandle, cx: &mut App) {
    close_footer_overlay_for_parent(parent, cx);
    #[cfg(target_os = "macos")]
    if !crate::runtime_policy::is_owned_evaluation() {
        let native = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&parent.window_id()).map_or(0, |host| host.native_window);
        if native != 0 {
            // Only retained footer peers / weak container owners are touched;
            // the parent's NSWindow may already have been released.
            unsafe {
                remove_float_footer_child_window(native as id);
                remove_main_window_footer_glass_container(native as id);
            }
        }
    }
    FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).remove(&parent.window_id());
}

pub(crate) fn retire_closed_footer_owner(window_id: gpui::WindowId, cx: &mut App) {
    let parent = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&window_id).map(|host| host.handle);
    if let Some(parent) = parent { retire_footer_owner(parent, cx); }
}

fn open_or_sync_footer_overlay(
    parent: AnyWindowHandle, parent_bounds: Bounds<Pixels>, display_id: Option<DisplayId>,
    policy: crate::runtime_policy::WindowHostPolicy, cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    policy.validate()?;
    let parent_info = footer_owner_info(parent).ok_or_else(|| anyhow::anyhow!("footer_parent_unregistered"))?;
    let parent_generation = parent_info.generation.ok_or_else(|| anyhow::anyhow!("footer_parent_generation_missing"))?;
    anyhow::ensure!(crate::windows::runtime_window_host_policy(&parent_info.id, parent_generation)? == policy, "footer_parent_policy_mismatch");
    let (binding, config, existing) = {
        let hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        let host = hosts.get(&parent.window_id()).ok_or_else(|| anyhow::anyhow!("footer_owner_missing"))?;
        (host.binding.clone().ok_or_else(|| anyhow::anyhow!("footer_binding_missing"))?,
         host.config.clone().ok_or_else(|| anyhow::anyhow!("footer_config_missing"))?,
         host.overlay.as_ref().map(|slot| (slot.handle, slot.info.clone())))
    };
    anyhow::ensure!(footer_binding_is_live(&binding, parent), "footer_parent_stale");
    let bounds = gpui_footer_overlay_bounds(parent_bounds);
    let width = f32::from(bounds.size.width);
    if let Some((handle, info)) = existing {
        handle.update(cx, |overlay, window, cx| {
            overlay.set_config(config, binding, width);
            set_gpui_footer_overlay_window_bounds(window, bounds, cx);
            cx.notify();
        })?;
        if let Some(generation) = info.generation {
            crate::windows::set_automation_bounds_if_generation(&info.id, generation, Some(automation_bounds_from_gpui(bounds)));
        }
        if !policy.is_hidden() { park_overlay_during_glass_morph(handle, cx); }
        return Ok(info);
    }
    let handle = open_footer_overlay_window(config, binding, bounds, display_id, policy, cx)?;
    let publish = (|| -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
        if !policy.is_hidden() { configure_gpui_footer_overlay_window(&handle, cx, parent)?; }
        publish_footer_overlay(parent, &parent_info, handle, bounds, policy, cx)
    })();
    let info = match publish {
        Ok(info) => info,
        Err(error) => { let _ = handle.update(cx, |_, window, _| window.remove_window()); return Err(error); }
    };
    if !policy.is_hidden() { park_overlay_during_glass_morph(handle, cx); }
    Ok(info)
}

fn open_footer_overlay_window(
    config: MainWindowFooterConfig, binding: FooterBinding, bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>, policy: crate::runtime_policy::WindowHostPolicy, cx: &mut App,
) -> anyhow::Result<WindowHandle<GpuiFooterOverlay>> {
    policy.validate()?;
    let mut options = gpui_footer_overlay_window_options(bounds, display_id);
    options.show = !policy.is_hidden();
    cx.open_window(options, |_, cx| {
        cx.new(|_| GpuiFooterOverlay::new(config, binding, f32::from(bounds.size.width)))
    })
}

fn publish_footer_overlay(
    parent: AnyWindowHandle, parent_info: &crate::protocol::AutomationWindowInfo,
    handle: WindowHandle<GpuiFooterOverlay>, bounds: Bounds<Pixels>,
    policy: crate::runtime_policy::WindowHostPolicy, cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    let parent_generation = parent_info.generation.ok_or_else(|| anyhow::anyhow!("footer_parent_generation_missing"))?;
    anyhow::ensure!(crate::windows::runtime_window_host_policy(&parent_info.id, parent_generation)? == policy, "footer_parent_policy_mismatch");
    let binding = &handle.read(cx)?.binding;
    anyhow::ensure!(binding.window_id == parent_info.id && binding.window_generation == parent_generation
        && footer_binding_is_live(binding, parent), "footer_parent_stale");
    let info = crate::windows::register_runtime_window_instance(crate::protocol::AutomationWindowInfo {
        id: if parent_info.id == "main" { GPUI_FOOTER_OVERLAY_AUTOMATION_ID.into() } else { format!("footer-overlay:{}:{}", parent_info.id, parent_generation) },
        kind: crate::protocol::AutomationWindowKind::PromptPopup,
        title: Some(GPUI_FOOTER_OVERLAY_WINDOW_TITLE.into()),
        focused: false, visible: !policy.is_hidden(), semantic_surface: Some("footerOverlay".into()),
        bounds: Some(automation_bounds_from_gpui(bounds)), parent_window_id: Some(parent_info.id.clone()),
        parent_window_generation: Some(parent_generation), parent_kind: Some(parent_info.kind),
        pid: Some(std::process::id()), generation: None,
    }, handle.into(), cx)?;
    let close_id = info.id.clone();
    let generation = info.generation.ok_or_else(|| anyhow::anyhow!("footer_generation_missing"))?;
    let subscription = cx.on_window_closed(move |cx, window_id| {
        if window_id == parent.window_id() {
            retire_footer_owner(parent, cx);
        } else if window_id == handle.window_id() {
            crate::windows::remove_runtime_window_instance(&close_id, generation);
            if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&parent.window_id()) {
                if host.overlay.as_ref().is_some_and(|slot| slot.info.generation == Some(generation)) {
                    host.overlay = None;
                    host.fidelity = None;
                }
            }
        }
    });
    handle.update(cx, |overlay, _, _| overlay.close_subscription = Some(subscription))?;
    {
        let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        let host = hosts.get_mut(&parent.window_id()).ok_or_else(|| anyhow::anyhow!("footer_owner_retired"))?;
        host.overlay = Some(GpuiFooterOverlaySlot { handle, parent_window_handle: parent, info: info.clone(), presentation_revision: 0, applied_theme_revision: 0 });
    }
    Ok(info)
}

#[cfg(all(any(test, feature = "owned-ui-evaluation"), target_os = "macos"))]
pub(crate) fn mount_owned_footer_fixture(
    fixture_id: &str, parent: &crate::protocol::AutomationWindowInfo,
    parent_handle: AnyWindowHandle, cx: &mut App,
) -> anyhow::Result<crate::protocol::AutomationWindowInfo> {
    anyhow::ensure!(OWNED_FOOTER_FIXTURE_IDS.contains(&fixture_id), "unknown_footer_fixture");
    let generation = parent.generation.ok_or_else(|| anyhow::anyhow!("footer_parent_generation_missing"))?;
    anyhow::ensure!(crate::windows::get_runtime_window_handle_for_generation(&parent.id, generation) == Some(parent_handle), "footer_parent_stale");
    let (bounds, display) = parent_handle.update(cx, |_, window, cx| (window.bounds(), window.display(cx).map(|display| display.id())))?;
    open_or_sync_footer_overlay(parent_handle, bounds, display, crate::runtime_policy::WindowHostPolicy::OwnedHidden, cx)
}

pub(crate) fn close_owned_footer_fixture(id: &str, generation: u64, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(crate::windows::runtime_window_host_policy(id, generation)? == crate::runtime_policy::WindowHostPolicy::OwnedHidden, "footer_not_owned");
    let parent = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).values().find_map(|host| {
        host.overlay.as_ref().filter(|slot| slot.info.id == id && slot.info.generation == Some(generation)).map(|slot| slot.parent_window_handle)
    }).ok_or_else(|| anyhow::anyhow!("footer_lifetime_stale"))?;
    close_footer_overlay_for_parent(parent, cx);
    Ok(())
}

fn prepare_footer_overlay_render(overlay: &mut GpuiFooterOverlay, window: &Window) -> Option<AnyWindowHandle> {
    let parent = crate::windows::get_runtime_window_handle_for_generation(&overlay.binding.window_id, overlay.binding.window_generation)?;
    if !footer_binding_is_live(&overlay.binding, parent) { return None; }
    let theme_revision = crate::theme::get_theme_snapshot().revision;
    let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let host = hosts.get_mut(&parent.window_id())?;
    let binding = host.binding.as_mut()?;
    if binding.theme_revision != theme_revision {
        host.presentation_revision += 1;
        binding.theme_revision = theme_revision;
        binding.presentation_revision = host.presentation_revision;
        host.native_token = next_footer_lifetime();
    }
    overlay.binding = binding.clone();
    let config = host.config.as_ref()?;
    if overlay.config != *config { overlay.config = config.clone(); }
    if let Some(slot) = host.overlay.as_mut() {
        if AnyWindowHandle::from(slot.handle) != window.window_handle() { return None; }
        slot.presentation_revision = host.presentation_revision;
        slot.applied_theme_revision = theme_revision;
    }
    host.snapshot.installed_surface = Some(overlay.config.surface);
    Some(parent)
}


fn notify_changed_footer_overlay(parent: AnyWindowHandle, cx: &mut App) {
    let handle = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&parent.window_id()).and_then(|host| {
        host.overlay.as_ref().filter(|slot| slot.presentation_revision != host.presentation_revision).map(|slot| slot.handle)
    });
    if let Some(handle) = handle {
        cx.defer(move |cx| { let _ = handle.update(cx, |_, _, cx| cx.notify()); });
    }
}