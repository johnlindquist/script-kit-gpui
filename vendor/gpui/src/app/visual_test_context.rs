use crate::{
    Action, AnyView, AnyWindowHandle, App, AppCell, AppContext, AssetSource, BackgroundExecutor,
    Bounds, ClipboardItem, Context, Entity, ForegroundExecutor, Global, InputEvent, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Platform, Point,
    Render, Result, Size, Task, TestDispatcher, TextSystem, VisualTestPlatform, Window,
    WindowBounds, WindowHandle, WindowOptions, app::GpuiMode,
};
use anyhow::anyhow;
use image::RgbaImage;
use std::{future::Future, rc::Rc, sync::Arc, time::Duration};

/// A test context that uses real macOS rendering instead of mocked rendering.
/// This is used for visual tests that need to capture actual screenshots.
///
/// Unlike `TestAppContext` which uses `TestPlatform` with mocked rendering,
/// `VisualTestAppContext` uses the real `MacPlatform` to produce actual rendered output.
///
/// Windows created through this context are positioned off-screen (at coordinates like -10000, -10000)
/// so they are invisible to the user but still fully rendered by the compositor.
#[derive(Clone)]
pub struct VisualTestAppContext {
    /// The underlying app cell
    pub app: Rc<AppCell>,
    /// The background executor for running async tasks
    pub background_executor: BackgroundExecutor,
    /// The foreground executor for running tasks on the main thread
    pub foreground_executor: ForegroundExecutor,
    /// The test dispatcher for deterministic task scheduling
    dispatcher: TestDispatcher,
    platform: Rc<dyn Platform>,
    text_system: Arc<TextSystem>,
}

/// Actual bounded progress since one pump began. Pending timers are not called settled.
#[derive(Clone, Copy, Debug, Default)]
pub struct OwnedWorkProgress {
    /// Scheduler callbacks actually executed during this pump.
    pub scheduler_steps: usize,
    /// App effects actually processed during this pump.
    pub effects_executed: usize,
    /// Entity lifetimes actually released through their normal callbacks.
    pub entities_released: usize,
    /// Native frames completed during this pump.
    pub frames_completed: u64,
    /// Foreground callbacks still queued after the pump.
    pub pending_foreground_tasks: usize,
    /// Background callbacks still queued after the pump.
    pub pending_background_tasks: usize,
    /// App effects still queued after the pump.
    pub pending_effects: usize,
    /// Zero-reference entity lifetimes still awaiting release callbacks.
    pub pending_entity_releases: usize,
    /// Live windows still requiring a draw after the pump.
    pub pending_dirty_windows: usize,
    /// Whether queued work or future timers remain; not a settled assertion.
    pub has_pending_tasks_or_timers: bool,
    /// Whether runnable work remained when the step limit was reached.
    pub budget_exhausted: bool,
}

impl VisualTestAppContext {
    /// Creates a new `VisualTestAppContext` with real macOS platform rendering
    /// but deterministic task scheduling via TestDispatcher.
    ///
    /// This provides:
    /// - Real Metal/compositor rendering for accurate screenshots
    /// - Deterministic task scheduling via TestDispatcher
    /// - Controllable time via `advance_clock`
    ///
    /// Note: This uses a no-op asset source, so SVG icons won't render.
    /// Use `with_asset_source` to provide real assets for icon rendering.
    pub fn new(platform: Rc<dyn Platform>) -> Self {
        Self::with_asset_source(platform, Arc::new(()))
    }

    /// Creates a new `VisualTestAppContext` with a custom asset source.
    ///
    /// Use this when you need SVG icons to render properly in visual tests.
    /// Pass the real `Assets` struct to enable icon rendering.
    pub fn with_asset_source(
        platform: Rc<dyn Platform>,
        asset_source: Arc<dyn AssetSource>,
    ) -> Self {
        // Use a seeded RNG for deterministic behavior
        let seed = std::env::var("SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Create a visual test platform that combines real Mac rendering
        // with controllable TestDispatcher for deterministic task scheduling
        let platform = Rc::new(VisualTestPlatform::new(platform, seed));
        Self::from_visual_platform(platform, asset_source)
    }

    /// Construct only around a native platform that installed its guard before initialization.
    pub fn with_owned_hidden_platform(
        platform: Rc<dyn Platform>,
        dispatcher: TestDispatcher,
        asset_source: Arc<dyn AssetSource>,
    ) -> Result<Self> {
        let platform = Rc::new(VisualTestPlatform::owned_hidden(platform, dispatcher)?);
        Ok(Self::from_visual_platform(platform, asset_source))
    }

    fn from_visual_platform(
        platform: Rc<VisualTestPlatform>,
        asset_source: Arc<dyn AssetSource>,
    ) -> Self {
        // Get the dispatcher and executors from the platform
        let dispatcher = platform.dispatcher().clone();
        let background_executor = platform.background_executor();
        let foreground_executor = platform.foreground_executor();

        let text_system = Arc::new(TextSystem::new(platform.text_system()));

        let http_client = if platform.owned_hidden_guard().is_some() {
            Arc::new(http_client::HttpClientWithUrl::new_url(
                Arc::new(http_client::BlockedHttpClient::new()),
                "",
                None,
            ))
        } else {
            http_client::FakeHttpClient::with_404_response()
        };

        let mut app = App::new_app(platform.clone(), asset_source, http_client);
        app.borrow_mut().mode = GpuiMode::test();

        Self {
            app,
            background_executor,
            foreground_executor,
            dispatcher,
            platform,
            text_system,
        }
    }

    /// Open the actual production root under explicit non-presenting window options.
    pub fn open_owned_hidden_window<V: Render + 'static>(
        &mut self,
        options: WindowOptions,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        anyhow::ensure!(
            self.platform.owned_hidden_guard().is_some(),
            "owned_hidden_context_required"
        );
        self.app.borrow_mut().open_window(options, build_root)
    }

    /// Mount a fallible production root without a placeholder or panic on preparation failure.
    pub fn open_owned_hidden_window_fallible<V: Render + 'static>(
        &mut self,
        options: WindowOptions,
        build_root: impl FnOnce(&mut Window, &mut App) -> Result<Entity<V>>,
    ) -> Result<WindowHandle<V>> {
        anyhow::ensure!(
            self.platform.owned_hidden_guard().is_some(),
            "owned_hidden_context_required"
        );
        self.app
            .borrow_mut()
            .open_window_fallible(options, build_root)
    }
    /// Bound scheduler, effects and completed frames. No unbounded dispatcher drain or sleep.
    /// A zero frame budget still advances timers, scheduler tasks and app effects.
    pub fn pump_owned_work(
        &mut self,
        max_steps: usize,
        advance: Duration,
        max_frames: u64,
    ) -> Result<OwnedWorkProgress> {
        anyhow::ensure!(
            self.platform.owned_hidden_guard().is_some(),
            "owned_hidden_context_required"
        );
        anyhow::ensure!(
            max_steps <= 4096 && advance <= Duration::from_secs(1),
            "owned_work_budget_invalid"
        );
        let before = self.owned_hidden_observation();
        self.dispatcher.advance_clock_without_running(advance);
        let mut progress = OwnedWorkProgress::default();
        // A frame callback may invalidate its own window for the next frame.
        // Keep that work queued for the next pump instead of rendering the same
        // animation repeatedly at one clock instant and starving other windows.
        let mut drawn_windows = smallvec::SmallVec::<[crate::WindowId; 8]>::new();
        let mut steps = 0;
        while steps < max_steps {
            let mut worked = false;
            if self.dispatcher.run_bounded(1) != 0 {
                progress.scheduler_steps += 1;
                steps += 1;
                worked = true;
            }
            if steps < max_steps {
                let effects = self.app.borrow_mut().flush_owned_effects(1)?;
                progress.effects_executed += effects.effects_executed;
                progress.entities_released += effects.entities_released;
                let work = effects.effects_executed + effects.entities_released;
                steps += work;
                worked |= work != 0;
            }
            if steps < max_steps
                && self.owned_hidden_observation().completed_frames - before.completed_frames
                    < max_frames
            {
                let dirty = self
                    .app
                    .borrow()
                    .windows
                    .values()
                    .filter_map(|window| window.as_deref())
                    .find(|window| {
                        window.invalidator.is_dirty()
                            && !drawn_windows.contains(&window.handle.window_id())
                    })
                    .map(|window| window.handle);
                if let Some(handle) = dirty {
                    self.update_window(handle, |_, window, cx| {
                        window.draw_scheduled_owned_frame(cx)
                    })??;
                    drawn_windows.push(handle.window_id());
                    steps += 1;
                    worked = true;
                }
            }
            if !worked {
                break;
            }
        }
        let (foreground, background) = self.dispatcher.pending_task_counts();
        let app = self.app.borrow();
        progress.pending_foreground_tasks = foreground;
        progress.pending_background_tasks = background;
        progress.pending_effects = app.pending_effects.len();
        progress.pending_entity_releases = app.entities.pending_dropped_count();
        progress.pending_dirty_windows = app
            .windows
            .values()
            .filter_map(|window| window.as_deref())
            .filter(|window| window.invalidator.is_dirty())
            .count();
        progress.has_pending_tasks_or_timers = self.dispatcher.has_pending_work();
        progress.frames_completed =
            self.owned_hidden_observation().completed_frames - before.completed_frames;
        progress.budget_exhausted = (steps == max_steps
            && (foreground
                + background
                + progress.pending_effects
                + progress.pending_entity_releases
                != 0))
            || (progress.pending_dirty_windows != 0
                && (steps == max_steps || progress.frames_completed >= max_frames));
        Ok(progress)
    }

    /// Snapshot the installed native authority without progressing any work.
    pub fn owned_hidden_observation(&self) -> crate::OwnedHiddenObservation {
        self.platform
            .owned_hidden_guard()
            .map(|guard| guard.observation())
            .unwrap_or_default()
    }

    /// Opens a window positioned off-screen for invisible rendering.
    ///
    /// The window is positioned at (-10000, -10000) so it's not visible on any display,
    /// but it's still fully rendered by the compositor and can be captured via ScreenCaptureKit.
    ///
    /// # Arguments
    /// * `size` - The size of the window to create
    /// * `build_root` - A closure that builds the root view for the window
    pub fn open_offscreen_window<V: Render + 'static>(
        &mut self,
        size: Size<Pixels>,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{point, px};

        let bounds = Bounds {
            origin: point(px(-10000.0), px(-10000.0)),
            size,
        };

        let mut cx = self.app.borrow_mut();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: false,
                show: true,
                ..Default::default()
            },
            build_root,
        )
    }

    /// Opens an off-screen window with default size (1280x800).
    pub fn open_offscreen_window_default<V: Render + 'static>(
        &mut self,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Result<WindowHandle<V>> {
        use crate::{px, size};
        self.open_offscreen_window(size(px(1280.0), px(800.0)), build_root)
    }

    /// Returns whether screen capture is supported on this platform.
    pub fn is_screen_capture_supported(&self) -> bool {
        self.platform.is_screen_capture_supported()
    }

    /// Returns the text system used by this context.
    pub fn text_system(&self) -> &Arc<TextSystem> {
        &self.text_system
    }

    /// Returns the background executor.
    pub fn executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    /// Returns the foreground executor.
    pub fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    /// Runs all pending foreground and background tasks until there's nothing left to do.
    /// This is essential for processing async operations like tooltip timers.
    pub fn run_until_parked(&self) {
        assert!(
            self.platform.owned_hidden_guard().is_none(),
            "owned host requires pump_owned_work"
        );
        self.dispatcher.run_until_parked();
    }

    /// Advances the simulated clock by the given duration and processes any tasks
    /// that become ready. This is essential for testing time-based behaviors like
    /// tooltip delays.
    pub fn advance_clock(&self, duration: Duration) {
        assert!(
            self.platform.owned_hidden_guard().is_none(),
            "owned host requires pump_owned_work"
        );
        self.dispatcher.advance_clock(duration);
    }

    /// Updates the app state.
    pub fn update<R>(&mut self, f: impl FnOnce(&mut App) -> R) -> R {
        let mut app = self.app.borrow_mut();
        f(&mut app)
    }

    /// Reads from the app state.
    pub fn read<R>(&self, f: impl FnOnce(&App) -> R) -> R {
        let app = self.app.borrow();
        f(&app)
    }

    /// Updates a window.
    pub fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut lock = self.app.borrow_mut();
        lock.update_window(window, f)
    }

    /// Spawns a task on the foreground executor.
    pub fn spawn<F, R>(&self, f: F) -> Task<R>
    where
        F: Future<Output = R> + 'static,
        R: 'static,
    {
        self.foreground_executor.spawn(f)
    }

    /// Checks if a global of type G exists.
    pub fn has_global<G: Global>(&self) -> bool {
        let app = self.app.borrow();
        app.has_global::<G>()
    }

    /// Reads a global value.
    pub fn read_global<G: Global, R>(&self, f: impl FnOnce(&G, &App) -> R) -> R {
        let app = self.app.borrow();
        f(app.global::<G>(), &app)
    }

    /// Sets a global value.
    pub fn set_global<G: Global>(&mut self, global: G) {
        let mut app = self.app.borrow_mut();
        app.set_global(global);
    }

    /// Updates a global value.
    pub fn update_global<G: Global, R>(&mut self, f: impl FnOnce(&mut G, &mut App) -> R) -> R {
        let mut lock = self.app.borrow_mut();
        lock.update(|cx| {
            let mut global = cx.lease_global::<G>();
            let result = f(&mut global, cx);
            cx.end_global_lease(global);
            result
        })
    }

    /// Simulates a sequence of keystrokes on the given window.
    ///
    /// Keystrokes are specified as a space-separated string, e.g., "cmd-p escape".
    pub fn simulate_keystrokes(&mut self, window: AnyWindowHandle, keystrokes: &str) {
        for keystroke_text in keystrokes.split_whitespace() {
            let keystroke = Keystroke::parse(keystroke_text)
                .unwrap_or_else(|_| panic!("Invalid keystroke: {}", keystroke_text));
            self.dispatch_keystroke(window, keystroke);
        }
        self.run_until_parked();
    }

    /// Dispatches a single keystroke to a window.
    pub fn dispatch_keystroke(&mut self, window: AnyWindowHandle, keystroke: Keystroke) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .ok();
    }

    /// Simulates typing text input on the given window.
    pub fn simulate_input(&mut self, window: AnyWindowHandle, input: &str) {
        for char in input.chars() {
            let key = char.to_string();
            let keystroke = Keystroke {
                modifiers: Modifiers::default(),
                key: key.clone(),
                key_char: Some(key),
            };
            self.dispatch_keystroke(window, keystroke);
        }
        self.run_until_parked();
    }

    /// Simulates a mouse move event.
    pub fn simulate_mouse_move(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: impl Into<Option<MouseButton>>,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseMoveEvent {
                position,
                modifiers,
                pressed_button: button.into(),
            },
        );
    }

    /// Simulates a mouse down event.
    pub fn simulate_mouse_down(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseDownEvent {
                position,
                modifiers,
                button,
                click_count: 1,
                first_mouse: false,
            },
        );
    }

    /// Simulates a mouse up event.
    pub fn simulate_mouse_up(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    ) {
        self.simulate_event(
            window,
            MouseUpEvent {
                position,
                modifiers,
                button,
                click_count: 1,
            },
        );
    }

    /// Simulates a click (mouse down followed by mouse up).
    pub fn simulate_click(
        &mut self,
        window: AnyWindowHandle,
        position: Point<Pixels>,
        modifiers: Modifiers,
    ) {
        self.simulate_mouse_down(window, position, MouseButton::Left, modifiers);
        self.simulate_mouse_up(window, position, MouseButton::Left, modifiers);
    }

    /// Simulates an input event on the given window.
    pub fn simulate_event<E: InputEvent>(&mut self, window: AnyWindowHandle, event: E) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_event(event.to_platform_input(), cx);
        })
        .ok();
        self.run_until_parked();
    }

    /// Dispatches an action to the given window.
    pub fn dispatch_action(&mut self, window: AnyWindowHandle, action: impl Action) {
        self.update_window(window, |_, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx);
        })
        .ok();
        self.run_until_parked();
    }

    /// Writes to the clipboard.
    pub fn write_to_clipboard(&self, item: ClipboardItem) {
        self.platform.write_to_clipboard(item);
    }

    /// Reads from the clipboard.
    pub fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        self.platform.read_from_clipboard()
    }

    /// Waits for a condition to become true, with a timeout.
    pub async fn wait_for<T: 'static>(
        &mut self,
        entity: &Entity<T>,
        predicate: impl Fn(&T) -> bool,
        timeout: Duration,
    ) -> Result<()> {
        let start = web_time::Instant::now();
        loop {
            {
                let app = self.app.borrow();
                if predicate(entity.read(&app)) {
                    return Ok(());
                }
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("Timed out waiting for condition"));
            }

            self.run_until_parked();
            self.background_executor
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    /// Captures a screenshot of the specified window using direct texture capture.
    ///
    /// This renders the scene to a Metal texture and reads the pixels directly,
    /// which does not require the window to be visible on screen.
    #[cfg(any(test, feature = "test-support"))]
    pub fn capture_screenshot(&mut self, window: AnyWindowHandle) -> Result<RgbaImage> {
        self.update_window(window, |_, window, _cx| window.render_to_image())?
    }

    /// Waits for animations to complete by waiting a couple of frames.
    pub async fn wait_for_animations(&self) {
        self.background_executor
            .timer(Duration::from_millis(32))
            .await;
        self.run_until_parked();
    }
}

impl AppContext for VisualTestAppContext {
    fn new<T: 'static>(&mut self, build_entity: impl FnOnce(&mut Context<T>) -> T) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.new(build_entity)
    }

    fn reserve_entity<T: 'static>(&mut self) -> crate::Reservation<T> {
        let mut app = self.app.borrow_mut();
        app.reserve_entity()
    }

    fn insert_entity<T: 'static>(
        &mut self,
        reservation: crate::Reservation<T>,
        build_entity: impl FnOnce(&mut Context<T>) -> T,
    ) -> Entity<T> {
        let mut app = self.app.borrow_mut();
        app.insert_entity(reservation, build_entity)
    }

    fn update_entity<T: 'static, R>(
        &mut self,
        handle: &Entity<T>,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        let mut app = self.app.borrow_mut();
        app.update_entity(handle, update)
    }

    fn as_mut<'a, T>(&'a mut self, _: &Entity<T>) -> crate::GpuiBorrow<'a, T>
    where
        T: 'static,
    {
        panic!("Cannot use as_mut with a visual test app context. Try calling update() first")
    }

    fn read_entity<T, R>(&self, handle: &Entity<T>, read: impl FnOnce(&T, &App) -> R) -> R
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_entity(handle, read)
    }

    fn update_window<T, F>(&mut self, window: AnyWindowHandle, f: F) -> Result<T>
    where
        F: FnOnce(AnyView, &mut Window, &mut App) -> T,
    {
        let mut lock = self.app.borrow_mut();
        lock.update_window(window, f)
    }

    fn read_window<T, R>(
        &self,
        window: &WindowHandle<T>,
        read: impl FnOnce(Entity<T>, &App) -> R,
    ) -> Result<R>
    where
        T: 'static,
    {
        let app = self.app.borrow();
        app.read_window(window, read)
    }

    fn background_spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        self.background_executor.spawn(future)
    }

    fn read_global<G, R>(&self, callback: impl FnOnce(&G, &App) -> R) -> R
    where
        G: Global,
    {
        let app = self.app.borrow();
        callback(app.global::<G>(), &app)
    }
}
