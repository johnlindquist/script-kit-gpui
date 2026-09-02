#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterBinding {
    pub window_id: String,
    pub window_generation: u64,
    pub host_generation: u64,
    pub config_revision: u64,
    pub presentation_revision: u64,
    pub theme_revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct FooterRuntimeState {
    pub binding: FooterBinding,
    pub config: MainWindowFooterConfig,
    pub host: MainWindowFooterHostSnapshot,
    pub semantic_revision: u64,
    pub presentation_revision: u64,
    pub applied_theme_revision: u64,
    pub completed_action_count: u64,
    pub held_action: Option<FooterAction>,
}

struct FooterHost {
    handle: AnyWindowHandle,
    binding: Option<FooterBinding>,
    config: Option<MainWindowFooterConfig>,
    host_generation: u64,
    config_revision: u64,
    presentation_revision: u64,
    applied_theme_revision: u64,
    snapshot: MainWindowFooterHostSnapshot,
    sender: async_channel::Sender<FooterActionEnvelope>,
    receiver: async_channel::Receiver<FooterActionEnvelope>,
    sequence: u64,
    accepted_sequence: u64,
    completed_sequence: u64,
    completed_action_count: u64,
    held_action: Option<FooterAction>,
    interaction_revision: u64,
    refresh_signature: Option<MainWindowFooterRefreshSignature>,
    native_window: usize,
    native_token: u64,
    native_view: usize,
    overlay: Option<GpuiFooterOverlaySlot>,
    fidelity: Option<crate::protocol::FidelityPaintTargetSnapshot>,
}

static FOOTER_HOSTS: std::sync::LazyLock<Mutex<std::collections::HashMap<gpui::WindowId, FooterHost>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));
static FOOTER_LIFETIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_footer_lifetime() -> u64 {
    FOOTER_LIFETIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl FooterHost {
    fn new(handle: AnyWindowHandle) -> Self {
        let (sender, receiver) = async_channel::bounded(32);
        Self {
            handle, binding: None, config: None, host_generation: next_footer_lifetime(),
            config_revision: 0, presentation_revision: 0, applied_theme_revision: 0,
            refresh_signature: None, native_window: 0, native_view: 0, native_token: 0, overlay: None, fidelity: None,
            sequence: 0, accepted_sequence: 0, completed_sequence: 0, completed_action_count: 0,
            snapshot: MainWindowFooterHostSnapshot::default(), sender, receiver,
            held_action: None, interaction_revision: 0,
        }
    }

    fn accepts(&self, binding: &FooterBinding, action: FooterAction) -> bool {
        self.binding.as_ref() == Some(binding)
            && binding.theme_revision == crate::theme::get_theme_snapshot().revision
            && self.config.as_ref().is_some_and(|config| matches!(
                config.action_dispatch_authorization(action, false),
                FooterActionDispatchAuthorization::PresentedButton
                    | FooterActionDispatchAuthorization::PresentedLeftAffordance
            ))
    }
}

fn footer_owner_info(handle: AnyWindowHandle) -> Option<crate::protocol::AutomationWindowInfo> {
    crate::windows::list_automation_windows().into_iter().find(|info| {
        info.generation.is_some_and(|generation|
            crate::windows::get_runtime_window_handle_for_generation(&info.id, generation) == Some(handle))
    })
}

fn footer_binding_is_live(binding: &FooterBinding, handle: AnyWindowHandle) -> bool {
    crate::windows::get_runtime_window_handle_for_generation(&binding.window_id, binding.window_generation) == Some(handle)
        && crate::windows::automation_window_by_id(&binding.window_id)
            .is_some_and(|info| info.generation == Some(binding.window_generation))
}

/// Register a receiver against the real GPUI window, even during construction
/// before metadata is published. Actions remain refused until exact binding.
pub(crate) fn footer_action_receiver(window: &Window) -> async_channel::Receiver<FooterActionEnvelope> {
    let handle = window.window_handle();
    FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner())
        .entry(handle.window_id()).or_insert_with(|| FooterHost::new(handle)).receiver.clone()
}

#[derive(Debug)]
pub(crate) struct FooterActionEnvelope {
    pub semantic_id: String,
    pub binding: FooterBinding,
    pub action: FooterAction,
    sequence: u64,
    owner_handle: AnyWindowHandle,
    accepted: std::sync::atomic::AtomicBool,
    completed: std::sync::atomic::AtomicBool,
    completion: Option<async_channel::Sender<Result<(), &'static str>>>,
}

impl FooterActionEnvelope {
    pub(crate) fn accept(&self, window: &Window) -> Option<FooterAction> {
        let handle = window.window_handle();
        if handle != self.owner_handle || !footer_binding_is_live(&self.binding, handle) {
            return self.refuse("footer_action_stale");
        }
        let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        let Some(host) = hosts.get_mut(&handle.window_id()) else { return self.refuse("footer_owner_retired"); };
        if !host.accepts(&self.binding, self.action) { return self.refuse("footer_action_stale_or_disabled"); }
        if self.sequence <= host.accepted_sequence || self.accepted.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return None;
        }
        host.accepted_sequence = self.sequence;
        Some(self.action)
    }

    /// Called after the real owner handler returns; enqueue/accept is not completion.
    pub(crate) fn complete(&self, window: &Window) {
        if window.window_handle() != self.owner_handle
            || !self.accepted.load(std::sync::atomic::Ordering::Acquire)
            || self.completed.swap(true, std::sync::atomic::Ordering::AcqRel) { return; }
        if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&self.owner_handle.window_id()) {
            if host.host_generation == self.binding.host_generation && self.sequence > host.completed_sequence {
                host.completed_sequence = self.sequence;
                host.completed_action_count += 1;
            }
        }
        // An action may retire its own footer. Its exact accepted envelope still
        // acknowledges the real owner handler returning, independently of maps.
        if let Some(sender) = &self.completion { let _ = sender.try_send(Ok(())); }
    }

    fn refuse(&self, reason: &'static str) -> Option<FooterAction> {
        if !self.accepted.load(std::sync::atomic::Ordering::Acquire) {
            if let Some(sender) = &self.completion { let _ = sender.try_send(Err(reason)); }
        }
        None
    }
}

pub(crate) fn dispatch_bound_footer_action(binding: &FooterBinding, action: FooterAction) -> bool {
    enqueue_bound_footer_action(binding, action, None).is_ok()
}

fn enqueue_bound_footer_action(
    binding: &FooterBinding, action: FooterAction,
    completion: Option<async_channel::Sender<Result<(), &'static str>>>,
) -> Result<(), &'static str> {
    let handle = crate::windows::get_runtime_window_handle_for_generation(&binding.window_id, binding.window_generation).ok_or("footer_action_stale")?;
    if !footer_binding_is_live(binding, handle) { return Err("footer_action_stale"); }
    let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let host = hosts.get_mut(&handle.window_id()).ok_or("footer_owner_retired")?;
    if !host.accepts(binding, action) { return Err("footer_action_stale_or_disabled"); }
    host.sequence += 1;
    let semantic_id = host.config.as_ref().and_then(|config| config.descriptor_for_action(action)).map(|button| button.id.to_string()).unwrap_or_else(|| format!("footer-action:{}", action.semantic_key()));
    host.sender.try_send(FooterActionEnvelope {
        binding: binding.clone(), action, semantic_id, sequence: host.sequence, owner_handle: handle,
        accepted: std::sync::atomic::AtomicBool::new(false), completed: std::sync::atomic::AtomicBool::new(false), completion,
    }).map_err(|error| match error {
        async_channel::TrySendError::Full(_) => "footer_action_queue_full",
        async_channel::TrySendError::Closed(_) => "footer_owner_disconnected",
    })
}

pub(crate) fn footer_config_for_window(handle: AnyWindowHandle) -> Option<MainWindowFooterConfig> {
    FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&handle.window_id())?.config.clone()
}

pub(crate) fn footer_config_matches(handle: AnyWindowHandle, config: Option<&MainWindowFooterConfig>) -> bool {
    FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&handle.window_id()).and_then(|host| host.config.as_ref()) == config
}

pub(crate) fn footer_host_snapshot(handle: AnyWindowHandle) -> MainWindowFooterHostSnapshot {
    FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get(&handle.window_id()).map(|host| host.snapshot).unwrap_or_default()
}

pub(crate) fn footer_runtime_state(id: &str, generation: u64) -> Option<FooterRuntimeState> {
    let handle = crate::windows::get_runtime_window_handle_for_generation(id, generation)?;
    let hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let host = hosts.values().find(|host| host.handle == handle || host.overlay.as_ref().is_some_and(|overlay| AnyWindowHandle::from(overlay.handle) == handle))?;
    let binding = host.binding.clone()?;
    if !footer_binding_is_live(&binding, host.handle) { return None; }
    let overlay = host.overlay.as_ref().filter(|overlay| AnyWindowHandle::from(overlay.handle) == handle);
    Some(FooterRuntimeState {
        semantic_revision: binding.config_revision,
        presentation_revision: overlay.map_or(host.presentation_revision, |overlay| overlay.presentation_revision).saturating_add(host.interaction_revision),
        applied_theme_revision: overlay.map_or(host.applied_theme_revision, |overlay| overlay.applied_theme_revision),
        binding, config: host.config.clone()?, host: host.snapshot,
        completed_action_count: host.completed_action_count,
        held_action: host.held_action,
    })
}

fn sync_footer_owner(window: &Window, config: Option<&MainWindowFooterConfig>) -> Option<FooterBinding> {
    let binding = sync_footer_binding(window.window_handle(), config)?;
    #[cfg(target_os = "macos")]
    if !window.is_owned_hidden() {
        let native_window = window_gpui_view_and_ns_window(window).map_or(0, |(_, window)| window as usize);
        if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&window.window_handle().window_id()) {
            host.native_window = native_window;
        }
    }
    Some(binding)
}

fn sync_footer_binding(handle: AnyWindowHandle, config: Option<&MainWindowFooterConfig>) -> Option<FooterBinding> {
    let info = footer_owner_info(handle);
    let theme_revision = crate::theme::get_theme_snapshot().revision;
    let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let host = hosts.entry(handle.window_id()).or_insert_with(|| FooterHost::new(handle));
    let changed = host.config.as_ref() != config;
        if host.config.is_none() && config.is_some() {
            host.host_generation = next_footer_lifetime();
            host.refresh_signature = None;
        }
    if changed {
        if host.held_action.take().is_some() { host.interaction_revision += 1; }
        host.config = config.cloned();
        host.config_revision += 1;
        host.presentation_revision += 1;
        host.native_token = next_footer_lifetime();
    }
    if host.binding.as_ref().is_some_and(|binding| binding.theme_revision != theme_revision) {
        host.presentation_revision += 1;
        host.native_token = next_footer_lifetime();
    }
    host.snapshot.requested_surface = config.map(|config| config.surface);
    if config.is_none() {
        host.snapshot = MainWindowFooterHostSnapshot::default();
        host.refresh_signature = None;
        host.binding = None;
        return None;
    }
    let info = info?;
    let generation = info.generation?;
    let identity_changed = host.binding.as_ref().is_some_and(|binding| binding.window_id != info.id || binding.window_generation != generation);
    if identity_changed {
        host.host_generation = next_footer_lifetime();
        host.refresh_signature = None;
    }
    let binding = FooterBinding { window_id: info.id, window_generation: generation,
        host_generation: host.host_generation, config_revision: host.config_revision,
        presentation_revision: host.presentation_revision, theme_revision };
    host.binding = Some(binding.clone());
    Some(binding)
}

fn mark_footer_installed(handle: AnyWindowHandle, native: bool) {
    if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&handle.window_id()) {
        host.snapshot.installed_surface = host.config.as_ref().map(|config| config.surface);
        host.snapshot.native_host_installed = native;
        host.applied_theme_revision = crate::theme::get_theme_snapshot().revision;
    }
}

fn main_footer_handle() -> Option<AnyWindowHandle> { crate::get_main_window_handle() }

fn clear_footer_refresh_signature(handle: AnyWindowHandle) {
    if let Some(host) = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner()).get_mut(&handle.window_id()) { host.refresh_signature = None; }
}

/// Publish only a completed native refresh for the exact owner/config/theme
/// captured before painting. Failed or superseded work leaves the cache cold.
fn commit_footer_refresh(
    handle: AnyWindowHandle,
    binding: &FooterBinding,
    signature: MainWindowFooterRefreshSignature,
) -> bool {
    if !footer_binding_is_live(binding, handle)
        || binding.theme_revision != crate::theme::get_theme_snapshot().revision
        || signature.theme_revision != binding.theme_revision
    {
        return false;
    }
    let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let Some(host) = hosts.get_mut(&handle.window_id()) else { return false; };
    if host.handle != handle || host.binding.as_ref() != Some(binding)
        || host.config.as_ref() != Some(&signature.config)
    {
        return false;
    }
    host.snapshot.installed_surface = Some(signature.config.surface);
    host.snapshot.native_host_installed = true;
    host.applied_theme_revision = signature.theme_revision;
    host.refresh_signature = Some(signature);
    true
}


pub(crate) fn footer_owner_subscription(window: &Window, cx: &App) -> gpui::Subscription {
    let handle = window.window_handle();
    cx.on_window_closed(move |cx, closed| {
        if closed == handle.window_id() { retire_footer_owner(handle, cx); }
    })
}
#[cfg(target_os = "macos")]
fn native_footer_binding(ns_window: id) -> Option<(AnyWindowHandle, FooterBinding, u64)> {
    let hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
    let host = hosts.values().find(|host| host.native_window == ns_window as usize)?;
    let binding = host.binding.clone()?;
    footer_binding_is_live(&binding, host.handle).then_some((host.handle, binding, host.native_token))
}
