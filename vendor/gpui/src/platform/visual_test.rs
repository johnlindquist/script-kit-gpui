//! Visual test platform that combines real rendering (macOs-only for now) with controllable TestDispatcher.
//!
//! This platform is used for visual tests that need:
//! - Real rendering (e.g. Metal/compositor) for accurate screenshots
//! - Deterministic task scheduling via TestDispatcher
//! - Controllable time via `advance_clock`

use crate::ScreenCaptureSource;
use crate::{
    AnyWindowHandle, BackgroundExecutor, ClipboardItem, CursorStyle, ForegroundExecutor, Keymap,
    Menu, MenuItem, OwnedMenu, PathPromptOptions, Platform, PlatformDisplay,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Task,
    TestDispatcher, WindowAppearance, WindowParams,
};
use anyhow::Result;
use futures::channel::oneshot;
use parking_lot::Mutex;

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

/// A platform that combines real Mac rendering with controllable TestDispatcher.
///
/// This allows visual tests to:
/// - Render real UI via Metal for accurate screenshots
/// - Control task scheduling deterministically via TestDispatcher
/// - Advance simulated time for testing time-based behaviors (tooltips, animations, etc.)
pub struct VisualTestPlatform {
    dispatcher: TestDispatcher,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    platform: Rc<dyn Platform>,
    clipboard: Mutex<Option<ClipboardItem>>,
    find_pasteboard: Mutex<Option<ClipboardItem>>,
    owned_hidden: Option<Arc<crate::OwnedHiddenGuard>>,
    owned_display: Option<Rc<crate::TestDisplay>>,
}

impl VisualTestPlatform {
    /// Creates a new VisualTestPlatform with the given random seed.
    ///
    /// The seed is used for deterministic random number generation in the TestDispatcher.
    pub fn new(platform: Rc<dyn Platform>, seed: u64) -> Self {
        assert!(
            platform.owned_hidden_guard().is_none(),
            "owned_hidden_platform_requires_shared_dispatcher"
        );
        let dispatcher = TestDispatcher::new(seed);
        Self::with_dispatcher(platform, dispatcher, None)
    }

    /// Bind the guarded native platform and GPUI to the same deterministic dispatcher.
    pub fn owned_hidden(platform: Rc<dyn Platform>, dispatcher: TestDispatcher) -> Result<Self> {
        let guard = platform
            .owned_hidden_guard()
            .ok_or_else(|| anyhow::anyhow!("native_hidden_guard_missing"))?;
        anyhow::ensure!(
            guard.observation().installed,
            "native_hidden_guard_not_installed"
        );
        validate_owned_dispatchers(
            &dispatcher,
            &platform.background_executor(),
            &platform.foreground_executor(),
        )?;
        Ok(Self::with_dispatcher(platform, dispatcher, Some(guard)))
    }

    fn with_dispatcher(
        platform: Rc<dyn Platform>,
        dispatcher: TestDispatcher,
        owned_hidden: Option<Arc<crate::OwnedHiddenGuard>>,
    ) -> Self {
        let arc_dispatcher = Arc::new(dispatcher.clone());

        let background_executor = BackgroundExecutor::new(arc_dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(arc_dispatcher);

        Self {
            dispatcher,
            background_executor,
            foreground_executor,
            platform,
            clipboard: Mutex::new(None),
            find_pasteboard: Mutex::new(None),
            owned_display: owned_hidden
                .as_ref()
                .map(|_| Rc::new(crate::TestDisplay::new())),
            owned_hidden,
        }
    }

    /// Returns a reference to the TestDispatcher for controlling task scheduling and time.
    pub fn dispatcher(&self) -> &TestDispatcher {
        &self.dispatcher
    }

    fn refusal(&self, operation: &str) -> Option<anyhow::Error> {
        self.owned_hidden
            .as_ref()
            .map(|guard| guard.refuse(operation))
    }
}

fn validate_owned_dispatchers(
    dispatcher: &TestDispatcher,
    background: &BackgroundExecutor,
    foreground: &ForegroundExecutor,
) -> Result<()> {
    anyhow::ensure!(
        background
            .dispatcher()
            .as_test()
            .is_some_and(|native| Arc::ptr_eq(native.scheduler(), dispatcher.scheduler()))
            && foreground
                .dispatcher()
                .as_test()
                .is_some_and(|native| Arc::ptr_eq(native.scheduler(), dispatcher.scheduler())),
        "owned_native_dispatcher_mismatch"
    );
    Ok(())
}

impl Platform for VisualTestPlatform {
    fn owned_hidden_guard(&self) -> Option<Arc<crate::OwnedHiddenGuard>> {
        self.owned_hidden.clone()
    }
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.platform.text_system()
    }

    fn run(&self, _on_finish_launching: Box<dyn 'static + FnOnce()>) {
        panic!("VisualTestPlatform::run should not be called in tests")
    }

    fn quit(&self) {}

    fn restart(&self, _binary_path: Option<PathBuf>) {
        let _ = self.refusal("restart");
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        let _ = self.refusal("activate");
    }

    fn hide(&self) {
        let _ = self.refusal("hide");
    }

    fn hide_other_apps(&self) {
        let _ = self.refusal("hide_other_apps");
    }

    fn unhide_other_apps(&self) {
        let _ = self.refusal("unhide_other_apps");
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        if let Some(display) = &self.owned_display {
            return vec![display.clone()];
        }
        self.platform.displays()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        if let Some(display) = &self.owned_display {
            return Some(display.clone());
        }
        self.platform.primary_display()
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        if self.owned_hidden.is_some() {
            return None;
        }
        self.platform.active_window()
    }

    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        if self.owned_hidden.is_some() {
            return Some(Vec::new());
        }
        self.platform.window_stack()
    }

    fn is_screen_capture_supported(&self) -> bool {
        if self.owned_hidden.is_some() {
            return false;
        }
        self.platform.is_screen_capture_supported()
    }

    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        if let Some(error) = self.refusal("screen_capture") {
            let (tx, rx) = oneshot::channel();
            let _ = tx.send(Err(error));
            return rx;
        }
        self.platform.screen_capture_sources()
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        if let Some(guard) = &self.owned_hidden {
            guard.validate_window(&options)?;
        }
        self.platform.open_window(handle, options)
    }

    fn window_appearance(&self) -> WindowAppearance {
        if self.owned_hidden.is_some() {
            return WindowAppearance::Dark;
        }
        self.platform.window_appearance()
    }

    fn open_url(&self, url: &str) {
        if self.refusal("open_url").is_some() {
            return;
        }
        self.platform.open_url(url)
    }

    fn on_open_urls(&self, _callback: Box<dyn FnMut(Vec<String>)>) {}

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        if let Some(error) = self.refusal("register_url_scheme") {
            return Task::ready(Err(error));
        }
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(match self.refusal("prompt_for_paths") {
            Some(error) => Err(error),
            None => Ok(None),
        })
        .ok();
        rx
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(match self.refusal("prompt_for_new_path") {
            Some(error) => Err(error),
            None => Ok(None),
        })
        .ok();
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        true
    }

    fn reveal_path(&self, path: &Path) {
        if self.refusal("reveal_path").is_some() {
            return;
        }
        self.platform.reveal_path(path)
    }

    fn open_with_system(&self, path: &Path) {
        if self.refusal("open_with_system").is_some() {
            return;
        }
        self.platform.open_with_system(path)
    }

    fn on_quit(&self, _callback: Box<dyn FnMut()>) {}

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {}

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {}

    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn crate::Action)>) {}

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {}

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn crate::Action) -> bool>) {}

    fn app_path(&self) -> Result<PathBuf> {
        self.platform.app_path()
    }

    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf> {
        self.platform.path_for_auxiliary_executable(name)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        // Cursor style is process-local in hidden mode; never change the operator's cursor.
        if self.owned_hidden.is_some() {
            return;
        }
        self.platform.set_cursor_style(style)
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        if self.owned_hidden.is_some() {
            return true;
        }
        self.platform.should_auto_hide_scrollbars()
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.clipboard.lock().clone()
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        *self.clipboard.lock() = Some(item);
    }

    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        self.find_pasteboard.lock().clone()
    }

    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, item: ClipboardItem) {
        *self.find_pasteboard.lock() = Some(item);
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        if let Some(error) = self.refusal("write_credentials") {
            return Task::ready(Err(error));
        }
        Task::ready(Ok(()))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        if let Some(error) = self.refusal("read_credentials") {
            return Task::ready(Err(error));
        }
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        if let Some(error) = self.refusal("delete_credentials") {
            return Task::ready(Err(error));
        }
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        self.platform.keyboard_layout()
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        self.platform.keyboard_mapper()
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {}

    fn thermal_state(&self) -> super::ThermalState {
        super::ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {}
}

#[cfg(test)]
mod owned_dispatcher_tests {
    use super::*;

    #[test]
    fn native_callbacks_must_share_the_owned_pump_scheduler() {
        let pump = TestDispatcher::new(0);
        let other = TestDispatcher::new(0);
        let background = BackgroundExecutor::new(Arc::new(pump.clone()));
        let foreground = ForegroundExecutor::new(Arc::new(pump.clone()));
        assert!(validate_owned_dispatchers(&pump, &background, &foreground).is_ok());
        assert!(validate_owned_dispatchers(&other, &background, &foreground).is_err());
        let foreign_background = BackgroundExecutor::new(Arc::new(other.clone()));
        assert!(validate_owned_dispatchers(&pump, &foreign_background, &foreground).is_err());
        let foreign_foreground = ForegroundExecutor::new(Arc::new(other));
        assert!(validate_owned_dispatchers(&pump, &background, &foreign_foreground).is_err());
    }

    #[test]
    fn guarded_platform_cannot_downgrade_or_forge_installed_authority() {
        let dispatcher = TestDispatcher::new(0);
        let platform = crate::TestPlatform::new(
            BackgroundExecutor::new(Arc::new(dispatcher.clone())),
            ForegroundExecutor::new(Arc::new(dispatcher.clone())),
        );
        let guarded = Rc::new(VisualTestPlatform::with_dispatcher(
            platform,
            dispatcher.clone(),
            Some(Arc::new(crate::OwnedHiddenGuard::default())),
        ));
        assert!(VisualTestPlatform::owned_hidden(guarded.clone(), dispatcher).is_err());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                VisualTestPlatform::new(guarded, 0)
            }))
            .is_err()
        );
    }
}
