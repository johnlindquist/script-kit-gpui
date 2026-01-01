use gpui::{px, size, App, AppContext as _, AsyncApp, Context, Focusable, Window, WindowHandle};

use crate::hotkeys;
use crate::notes;
use crate::platform::{calculate_eye_line_bounds_on_mouse_display, move_first_window_to_bounds};
use crate::window_resize::{initial_window_height, reset_resize_debounce};
use crate::{logging, platform, ScriptListApp, NEEDS_RESET, PANEL_CONFIGURED, WINDOW_VISIBLE};

/// A simple model that listens for hotkey triggers via async_channel (event-driven).
pub struct HotkeyPoller {
    window: WindowHandle<ScriptListApp>,
}

impl HotkeyPoller {
    pub fn new(window: WindowHandle<ScriptListApp>) -> Self {
        Self { window }
    }

    pub fn start_listening(&self, cx: &mut Context<Self>) {
        let window = self.window;
        // Event-driven: recv().await yields immediately when hotkey is pressed
        // No polling - replaces 100ms Timer::after loop
        cx.spawn(async move |_this, cx: &mut AsyncApp| {
            logging::log("HOTKEY", "Hotkey listener started (event-driven via async_channel)");

            while let Ok(()) = hotkeys::hotkey_channel().1.recv().await {
                logging::log("VISIBILITY", "");
                logging::log("VISIBILITY", "╔════════════════════════════════════════════════════════════╗");
                logging::log("VISIBILITY", "║  HOTKEY TRIGGERED - TOGGLE WINDOW                          ║");
                logging::log("VISIBILITY", "╚════════════════════════════════════════════════════════════╝");

                // Check current visibility state for toggle behavior
                let is_visible = WINDOW_VISIBLE.load(std::sync::atomic::Ordering::SeqCst);
                let needs_reset = NEEDS_RESET.load(std::sync::atomic::Ordering::SeqCst);
                logging::log(
                    "VISIBILITY",
                    &format!(
                        "State check: WINDOW_VISIBLE={}, NEEDS_RESET={}",
                        is_visible, needs_reset
                    ),
                );

                if is_visible {
                    logging::log("VISIBILITY", "Decision: HIDE (window is currently visible)");
                    // Update visibility state FIRST to prevent race conditions
                    // Even though the hide is async, we mark it as hidden immediately
                    WINDOW_VISIBLE.store(false, std::sync::atomic::Ordering::SeqCst);
                    logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

                    // Window is visible - check if in prompt mode
                    let window_clone = window;

                    // First check if we're in a prompt - if so, cancel and hide
                    let _ = cx.update(move |cx: &mut App| {
                        let _ = window_clone.update(
                            cx,
                            |view: &mut ScriptListApp,
                             _win: &mut Window,
                             ctx: &mut Context<ScriptListApp>| {
                                if view.is_in_prompt() {
                                    logging::log(
                                        "HOTKEY",
                                        "In prompt mode - canceling script before hiding",
                                    );
                                    view.cancel_script_execution(ctx);
                                }
                                // Reset UI state before hiding (clears selection, scroll position, filter)
                                logging::log("HOTKEY", "Resetting to script list before hiding");
                                view.reset_to_script_list(ctx);
                            },
                        );

                        // Always hide the window when hotkey pressed while visible
                        logging::log("HOTKEY", "Hiding window (toggle: visible -> hidden)");
                        // PERF: Measure window hide latency
                        let hide_start = std::time::Instant::now();
                        cx.hide();
                        let hide_elapsed = hide_start.elapsed();
                        logging::log(
                            "PERF",
                            &format!("Window hide took {:.2}ms", hide_elapsed.as_secs_f64() * 1000.0),
                        );
                        logging::log("HOTKEY", "Window hidden via cx.hide()");
                    });
                } else {
                    logging::log("VISIBILITY", "Decision: SHOW (window is currently hidden)");
                    // Update visibility state FIRST to prevent race conditions
                    WINDOW_VISIBLE.store(true, std::sync::atomic::Ordering::SeqCst);
                    logging::log("VISIBILITY", "WINDOW_VISIBLE set to: true");

                    let window_clone = window;
                    let _ = cx.update(move |cx: &mut App| {
                        // Step 0: CRITICAL - Set MoveToActiveSpace BEFORE any activation
                        // This MUST happen before move_first_window_to_bounds, cx.activate(),
                        // or win.activate_window() to prevent macOS from switching spaces
                        platform::ensure_move_to_active_space();

                        // Step 1: Calculate new bounds on display with mouse, at eye-line height
                        let window_size = size(px(750.), initial_window_height());
                        let new_bounds = calculate_eye_line_bounds_on_mouse_display(window_size);

                        logging::log(
                            "HOTKEY",
                            &format!(
                                "Calculated bounds: origin=({:.0}, {:.0}) size={:.0}x{:.0}",
                                f64::from(new_bounds.origin.x),
                                f64::from(new_bounds.origin.y),
                                f64::from(new_bounds.size.width),
                                f64::from(new_bounds.size.height)
                            ),
                        );

                        // Step 2: Move window (position only, no activation)
                        // Note: makeKeyAndOrderFront was removed - ordering happens via GPUI below
                        move_first_window_to_bounds(&new_bounds);
                        logging::log("HOTKEY", "Window repositioned to mouse display");

                        // Step 3: NOW activate the app (makes window visible at new position)
                        cx.activate(true);
                        logging::log("HOTKEY", "App activated (window now visible)");

                        // Step 3.5: Configure as floating panel on first show only
                        if !PANEL_CONFIGURED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                            platform::configure_as_floating_panel();
                            logging::log("HOTKEY", "Configured window as floating panel (first show)");
                        }

                        // Step 4: Activate the specific window and focus it
                        let _ = window_clone.update(
                            cx,
                            |view: &mut ScriptListApp, win: &mut Window, cx: &mut Context<ScriptListApp>| {
                                win.activate_window();
                                let focus_handle = view.focus_handle(cx);
                                win.focus(&focus_handle, cx);
                                logging::log("HOTKEY", "Window activated and focused");

                                // Step 5: Check if we need to reset to script list (after script completion)
                                // Reset debounce timer to allow immediate resize after window move
                                reset_resize_debounce();

                                if NEEDS_RESET
                                    .compare_exchange(
                                        true,
                                        false,
                                        std::sync::atomic::Ordering::SeqCst,
                                        std::sync::atomic::Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    logging::log(
                                        "VISIBILITY",
                                        "NEEDS_RESET was true - clearing and resetting to script list",
                                    );
                                    view.reset_to_script_list(cx);
                                } else {
                                    // Even without reset, ensure window is properly sized for current content
                                    view.update_window_size();
                                }
                            },
                        );

                        logging::log("VISIBILITY", "Window show sequence complete");
                    });
                }

                let final_visible = WINDOW_VISIBLE.load(std::sync::atomic::Ordering::SeqCst);
                let final_reset = NEEDS_RESET.load(std::sync::atomic::Ordering::SeqCst);
                logging::log(
                    "VISIBILITY",
                    &format!("Final state: WINDOW_VISIBLE={}, NEEDS_RESET={}", final_visible, final_reset),
                );
                logging::log("VISIBILITY", "═══════════════════════════════════════════════════════════════");
            }

            logging::log("HOTKEY", "Hotkey listener exiting (channel closed)");
        })
        .detach();
    }
}

/// A model that listens for script hotkey triggers via async_channel.
pub struct ScriptHotkeyPoller {
    window: WindowHandle<ScriptListApp>,
}

impl ScriptHotkeyPoller {
    pub fn new(window: WindowHandle<ScriptListApp>) -> Self {
        Self { window }
    }

    pub fn start_listening(&self, cx: &mut Context<Self>) {
        let window = self.window;
        cx.spawn(async move |_this, cx: &mut AsyncApp| {
            logging::log("HOTKEY", "Script hotkey listener started");

            while let Ok(script_path) = hotkeys::script_hotkey_channel().1.recv().await {
                logging::log(
                    "HOTKEY",
                    &format!("Script shortcut received: {}", script_path),
                );

                let path_clone = script_path.clone();
                let _ = cx.update(move |cx: &mut App| {
                    let _ = window.update(
                        cx,
                        |view: &mut ScriptListApp,
                         _win: &mut Window,
                         ctx: &mut Context<ScriptListApp>| {
                            // Find and execute the script by path
                            view.execute_script_by_path(&path_clone, ctx);
                        },
                    );
                });
            }

            logging::log("HOTKEY", "Script hotkey listener exiting");
        })
        .detach();
    }
}

/// A model that listens for notes hotkey triggers via async_channel.
pub struct NotesHotkeyPoller;

impl NotesHotkeyPoller {
    pub fn new() -> Self {
        Self
    }

    pub fn start_listening(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |_this, cx: &mut AsyncApp| {
            logging::log("HOTKEY", "Notes hotkey listener started");

            while let Ok(()) = hotkeys::notes_hotkey_channel().1.recv().await {
                logging::log("HOTKEY", "Notes hotkey triggered - opening notes window");

                let _ = cx.update(move |cx: &mut App| {
                    if let Err(e) = notes::open_notes_window(cx) {
                        logging::log("HOTKEY", &format!("Failed to open notes window: {}", e));
                    }
                });
            }

            logging::log("HOTKEY", "Notes hotkey listener exiting");
        })
        .detach();
    }
}

pub(crate) fn start_hotkey_event_handler(cx: &mut App, window: WindowHandle<ScriptListApp>) {
    // Start main hotkey listener (for app show/hide toggle)
    let handler = cx.new(|_| HotkeyPoller::new(window));
    handler.update(cx, |p, cx| {
        p.start_listening(cx);
    });

    // Start script hotkey listener (for direct script execution via shortcuts)
    let script_handler = cx.new(|_| ScriptHotkeyPoller::new(window));
    script_handler.update(cx, |p, cx| {
        p.start_listening(cx);
    });

    // Start notes hotkey listener (for opening notes window)
    let notes_handler = cx.new(|_| NotesHotkeyPoller::new());
    notes_handler.update(cx, |p, cx| {
        p.start_listening(cx);
    });
}
