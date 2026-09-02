//! Shared inline-popup window helpers.
//!
//! These helpers own the detached child-window mechanics used by any inline
//! popup surface (Agent Chat composer pickers, Agent Chat history popup, and the menu-syntax
//! `:`, `;`, and `!` trigger pickers). They are intentionally
//! neutral: no Agent Chat types, no menu-syntax types, no domain callbacks. Callers
//! layer their own row models and accept behavior on top.
//!
//! Every symbol that used to live in `src/ai/agent_chat/ui/popup_window.rs` under the
//! `DENSE_PICKER_*` / `dense_picker_*` / `popup_*` names has been renamed to a
//! neutral `INLINE_POPUP_*` / `inline_popup_*` form here. Agent Chat keeps a thin
//! compatibility facade via `pub(crate) use ... as old_name;` re-exports so
//! existing call sites and source-text audit tests continue to compile without
//! edits.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use crate::runtime_policy::{ExternalEffect, WindowHostPolicy};
use gpui::{
    px, AnyWindowHandle, App, AppContext, Bounds, DisplayId, FocusHandle, Pixels, Window,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};

/// Maximum rows a dense inline popup shows before scrolling kicks in.
pub const INLINE_POPUP_MAX_VISIBLE_ROWS: usize = 8;

/// Vertical padding applied above and below the popup's row list.
pub const INLINE_POPUP_VERTICAL_PADDING: f32 = 4.0;

/// Height used when the popup has zero rows (empty state).
pub const INLINE_POPUP_EMPTY_HEIGHT: f32 = 56.0;

/// Default popup width cap.
pub const INLINE_POPUP_DEFAULT_WIDTH: f32 = 320.0;

/// Minimum popup width — never goes narrower even when the parent is cramped.
pub const INLINE_POPUP_MIN_WIDTH: f32 = 168.0;

/// Gutter reserved on both sides of the parent window when fitting the popup.
pub const INLINE_POPUP_EDGE_GUTTER: f32 = 12.0;

/// Left margin used by callers that anchor the popup to the composer gutter.
pub const INLINE_POPUP_LEFT_MARGIN: f32 = 8.0;

#[cfg(target_os = "macos")]
const NS_WINDOW_ABOVE: i64 = 1;

static NEXT_INLINE_POPUP_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Exact identity for one native inline-popup lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize)]
pub struct InlinePopupGeneration(u64);

impl InlinePopupGeneration {
    pub fn next() -> Self {
        Self(NEXT_INLINE_POPUP_GENERATION.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Legal phases for one attached interactive popup lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InlinePopupPhase {
    CreatedHidden,
    AttachPending,
    Open,
    Closing,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlinePopupAttachReceipt {
    pub generation: InlinePopupGeneration,
    pub attempt_count: u8,
    pub parent_window_number: i64,
    pub child_window_number: i64,
    pub parent_visible_at_attach: bool,
    pub child_visible_before_attach: bool,
    pub child_key_before_attach: bool,
    pub parent_child_relation_verified: bool,
    pub configured_after_attach: bool,
    pub child_visible_after_show: bool,
    pub child_key_after_show: bool,
    pub hidden_ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InlinePopupAttachFailure {
    PopupWindowGone,
    ChildNativeWindowMissing,
    ParentWindowGone,
    ParentNativeWindowMissing,
    ParentRuntimeHandleMismatch,
    ParentNotReady,
    SameNativeWindow,
    ChildVisibleBeforeAttach,
    ParentChildRelationRejected,
    ShowVerificationFailed,
    StaleGeneration,
    HostPolicyMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlinePopupAttachResult {
    Ready(InlinePopupAttachReceipt),
    Failed {
        generation: InlinePopupGeneration,
        failure: InlinePopupAttachFailure,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlinePopupCloseGate {
    Begin,
    AlreadyClosing,
    AlreadyClosed,
    StaleGeneration,
}

#[derive(Clone, Debug)]
pub struct InlinePopupLifecycle {
    generation: InlinePopupGeneration,
    phase: InlinePopupPhase,
    attach_receipt: Option<InlinePopupAttachReceipt>,
}

pub type InlinePopupLifecycleHandle = Arc<Mutex<InlinePopupLifecycle>>;

impl InlinePopupLifecycle {
    pub fn new() -> InlinePopupLifecycleHandle {
        Arc::new(Mutex::new(Self {
            generation: InlinePopupGeneration::next(),
            phase: InlinePopupPhase::CreatedHidden,
            attach_receipt: None,
        }))
    }

    pub fn generation(handle: &InlinePopupLifecycleHandle) -> InlinePopupGeneration {
        handle
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .generation
    }

    pub fn snapshot(
        handle: &InlinePopupLifecycleHandle,
    ) -> (
        InlinePopupGeneration,
        InlinePopupPhase,
        Option<InlinePopupAttachReceipt>,
    ) {
        let lifecycle = handle.lock().unwrap_or_else(|poison| poison.into_inner());
        (
            lifecycle.generation,
            lifecycle.phase,
            lifecycle.attach_receipt.clone(),
        )
    }

    pub fn begin_attach(
        handle: &InlinePopupLifecycleHandle,
        generation: InlinePopupGeneration,
    ) -> bool {
        let mut lifecycle = handle.lock().unwrap_or_else(|poison| poison.into_inner());
        if lifecycle.generation != generation || lifecycle.phase != InlinePopupPhase::CreatedHidden
        {
            return false;
        }
        lifecycle.phase = InlinePopupPhase::AttachPending;
        true
    }

    pub fn mark_ready(
        handle: &InlinePopupLifecycleHandle,
        receipt: InlinePopupAttachReceipt,
    ) -> bool {
        let mut lifecycle = handle.lock().unwrap_or_else(|poison| poison.into_inner());
        if lifecycle.generation != receipt.generation
            || lifecycle.phase != InlinePopupPhase::AttachPending
            || (receipt.hidden_ready
                && (receipt.parent_visible_at_attach
                    || receipt.child_visible_before_attach
                    || receipt.child_key_before_attach
                    || receipt.child_visible_after_show
                    || receipt.child_key_after_show
                    || receipt.configured_after_attach
                    || receipt.parent_child_relation_verified))
        {
            return false;
        }
        lifecycle.phase = InlinePopupPhase::Open;
        lifecycle.attach_receipt = Some(receipt);
        true
    }

    pub fn request_close(
        handle: &InlinePopupLifecycleHandle,
        generation: InlinePopupGeneration,
    ) -> InlinePopupCloseGate {
        let mut lifecycle = handle.lock().unwrap_or_else(|poison| poison.into_inner());
        if lifecycle.generation != generation {
            return InlinePopupCloseGate::StaleGeneration;
        }
        match lifecycle.phase {
            InlinePopupPhase::Closing => InlinePopupCloseGate::AlreadyClosing,
            InlinePopupPhase::Closed => InlinePopupCloseGate::AlreadyClosed,
            InlinePopupPhase::CreatedHidden
            | InlinePopupPhase::AttachPending
            | InlinePopupPhase::Open => {
                lifecycle.phase = InlinePopupPhase::Closing;
                InlinePopupCloseGate::Begin
            }
        }
    }

    pub fn mark_closed(
        handle: &InlinePopupLifecycleHandle,
        generation: InlinePopupGeneration,
    ) -> bool {
        let mut lifecycle = handle.lock().unwrap_or_else(|poison| poison.into_inner());
        if lifecycle.generation != generation || lifecycle.phase != InlinePopupPhase::Closing {
            return false;
        }
        lifecycle.phase = InlinePopupPhase::Closed;
        true
    }
}

/// Exact parent focus target captured before a popup lifetime starts.
#[derive(Clone)]
pub struct InlinePopupFocusReturn {
    pub generation: InlinePopupGeneration,
    pub parent_automation_id: String,
    pub parent_window_handle: AnyWindowHandle,
    pub focus_handle: FocusHandle,
    pub semantic_id: &'static str,
    pub parent_generation: u64,
    pub host_policy: WindowHostPolicy,
}

impl InlinePopupFocusReturn {
    pub fn restore(&self, expected_generation: InlinePopupGeneration, cx: &mut App) -> bool {
        if self.generation != expected_generation
            || crate::windows::get_runtime_window_handle_for_generation(
                &self.parent_automation_id,
                self.parent_generation,
            ) != Some(self.parent_window_handle)
        {
            return false;
        }
        if self.host_policy.validate().is_err() {
            return false;
        }

        cx.update_window(self.parent_window_handle, |_, window, cx| {
            if !self.host_policy.is_hidden() {
                window.activate_window();
            }
            window.focus(&self.focus_handle, cx);
            self.focus_handle.is_focused(window)
        })
        .unwrap_or(false)
    }
}

/// Whether the exact parent/child pair currently owns keyboard activation.
///
/// GPUI activation alone is insufficient for the nonactivating AppKit panels
/// used by attached popups: it can remain false while the parent is the native
/// key window. The native supplement lets consumers arm focus-loss dismissal
/// without treating the Accessory-mode false baseline as a real loss.
pub fn inline_popup_focus_pair_is_active(
    child_window: &mut Window,
    parent_window_handle: AnyWindowHandle,
    cx: &mut App,
) -> bool {
    if child_window.is_owned_hidden() {
        return cx
            .update_window(parent_window_handle, |_, parent, _| {
                parent.is_owned_hidden()
            })
            .unwrap_or(false);
    }
    if child_window.is_window_active() {
        return true;
    }

    let parent_gpui_active = cx
        .update_window(parent_window_handle, |_, parent_window, _cx| {
            parent_window.is_window_active()
        })
        .ok()
        .unwrap_or(false);
    if parent_gpui_active {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        let child_key = inline_popup_ns_window(child_window)
            .map(|ns_window| unsafe {
                let is_key: cocoa::base::BOOL = msg_send![ns_window, isKeyWindow];
                is_key != cocoa::base::NO
            })
            .unwrap_or(false);
        let parent_key = cx
            .update_window(parent_window_handle, |_, parent_window, _cx| {
                inline_popup_ns_window(parent_window)
                    .map(|ns_window| unsafe {
                        let is_key: cocoa::base::BOOL = msg_send![ns_window, isKeyWindow];
                        is_key != cocoa::base::NO
                    })
                    .unwrap_or(false)
            })
            .ok()
            .unwrap_or(false);
        child_key || parent_key
    }

    #[cfg(not(target_os = "macos"))]
    false
}

/// Compute popup height for a row count and row height.
///
/// Zero rows returns [`INLINE_POPUP_EMPTY_HEIGHT`] so an empty-state popup
/// still has a visible surface.
pub fn inline_popup_height_for_row_height(item_count: usize, row_height: f32) -> f32 {
    if item_count == 0 {
        return INLINE_POPUP_EMPTY_HEIGHT;
    }

    let visible_rows = item_count.min(INLINE_POPUP_MAX_VISIBLE_ROWS) as f32;
    (visible_rows * row_height) + (INLINE_POPUP_VERTICAL_PADDING * 2.0)
}

/// Clamp the popup width to the parent window, honoring the min/default caps
/// and the edge gutter on both sides.
pub fn inline_popup_width_for_window(window_width: f32) -> f32 {
    let max_width =
        (window_width - (INLINE_POPUP_EDGE_GUTTER * 2.0)).min(INLINE_POPUP_DEFAULT_WIDTH);
    max_width.max(INLINE_POPUP_MIN_WIDTH)
}

/// Top anchor for popups that prefer to sit above the mini-shell hint strip.
pub fn footer_anchored_inline_popup_top(parent_height: f32, popup_height: f32) -> f32 {
    let bottom_offset = crate::window_resize::main_layout::HINT_STRIP_HEIGHT + 4.0;
    (parent_height - bottom_offset - popup_height).max(0.0)
}

/// Build screen-relative popup bounds from `(left, top, width, height)`
/// offsets applied to the parent window's origin.
pub fn inline_popup_bounds(
    parent_bounds: Bounds<Pixels>,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
) -> Bounds<Pixels> {
    Bounds {
        origin: gpui::point(
            parent_bounds.origin.x + px(left),
            parent_bounds.origin.y + px(top),
        ),
        size: gpui::size(px(width), px(height)),
    }
}

/// Window options for a no-focus-steal popup. Theme-aware so vibrancy callers
/// get a blurred background and opaque callers get a solid one.
pub fn inline_popup_window_options(
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    host_policy: WindowHostPolicy,
) -> WindowOptions {
    let theme = crate::theme::get_cached_theme();
    let window_background = if !host_policy.is_hidden() && theme.is_vibrancy_enabled() {
        crate::platform::vibrancy_window_background()
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background,
        focus: false,
        // Interactive child popups remain hidden until the deferred AppKit
        // handshake proves they are attached to the exact live parent.
        show: false,
        kind: WindowKind::PopUp,
        // Popups size from row content via `set_inline_popup_window_bounds`; manual
        // edge resize would fight the left-drawer / dense-picker height contract.
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id,
        ..Default::default()
    }
}

/// Configure the newly-created popup NSWindow: dark-vibrancy + attach as a
/// child of `parent_window_handle` so it follows the parent.
pub fn configure_inline_popup_window<T: 'static>(
    handle: &WindowHandle<T>,
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    crate::runtime_policy::check(ExternalEffect::NativeVisibility)?;
    #[cfg(target_os = "macos")]
    {
        let is_dark_vibrancy = crate::theme::get_cached_theme().should_use_dark_vibrancy();
        handle
            .update(cx, move |_popup, window, cx| {
                window.defer(cx, move |window, cx| {
                    if let Some(ns_window) = inline_popup_ns_window(window) {
                        // SAFETY: `ns_window` comes from the live GPUI popup window on the
                        // main thread and is nil-checked before configuration.
                        unsafe {
                            crate::platform::configure_inline_dropdown_popup_window(
                                ns_window,
                                is_dark_vibrancy,
                            );
                        }
                        attach_inline_popup_to_parent_window(cx, parent_window_handle, ns_window);

                        tracing::info!(
                            target: "script_kit::inline_popup",
                            event = "inline_popup_attached",
                            dark = is_dark_vibrancy,
                            "Attached inline popup window to parent window"
                        );
                    }
                });
            })
            .map_err(|_| anyhow::anyhow!("failed to configure inline popup window"))?;
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (handle, cx, parent_window_handle);

    Ok(())
}

type InlinePopupAttachCallback =
    Rc<RefCell<Option<Box<dyn FnOnce(InlinePopupAttachResult, &mut App)>>>>;

/// Exact owning parent lifetime and host policy for a popup attach handshake.
pub struct InlinePopupParent {
    pub window_handle: AnyWindowHandle,
    pub automation_id: String,
    pub generation: u64,
    pub host_policy: WindowHostPolicy,
}

struct InlinePopupAttachAttempt {
    parent: InlinePopupParent,
    generation: InlinePopupGeneration,
    lifecycle: InlinePopupLifecycleHandle,
    attempt_count: u8,
    callback: InlinePopupAttachCallback,
}

/// Configure an interactive popup through a generation-scoped hidden
/// attach handshake. The result callback is the only point where consumers
/// may publish target identity or treat the popup as open.
pub fn configure_inline_popup_window_lifecycle<T: 'static>(
    handle: WindowHandle<T>,
    parent: InlinePopupParent,
    lifecycle: InlinePopupLifecycleHandle,
    cx: &mut App,
    on_result: impl FnOnce(InlinePopupAttachResult, &mut App) + 'static,
) -> anyhow::Result<()> {
    parent.host_policy.validate()?;
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(
            &parent.automation_id,
            parent.generation
        ) == Some(parent.window_handle),
        "inline_popup_parent_stale"
    );
    let generation = InlinePopupLifecycle::generation(&lifecycle);
    if !InlinePopupLifecycle::begin_attach(&lifecycle, generation) {
        anyhow::bail!("inline popup lifecycle rejected attach start");
    }

    let callback: InlinePopupAttachCallback = Rc::new(RefCell::new(Some(Box::new(on_result))));
    handle
        .update(cx, move |_popup, window, cx| {
            window.defer(cx, move |window, cx| {
                run_inline_popup_attach_attempt(
                    window,
                    cx,
                    InlinePopupAttachAttempt {
                        parent,
                        generation,
                        lifecycle,
                        attempt_count: 1,
                        callback,
                    },
                );
            });
        })
        .map_err(|_| anyhow::anyhow!("failed to schedule inline popup attach handshake"))?;

    Ok(())
}

fn run_inline_popup_attach_attempt(
    window: &mut Window,
    cx: &mut App,
    attempt: InlinePopupAttachAttempt,
) {
    let InlinePopupAttachAttempt {
        parent,
        generation,
        lifecycle,
        attempt_count,
        callback,
    } = attempt;
    if InlinePopupLifecycle::snapshot(&lifecycle).1 != InlinePopupPhase::AttachPending {
        finish_inline_popup_attach(
            InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::StaleGeneration,
            },
            lifecycle,
            callback,
            cx,
        );
        return;
    }

    if crate::windows::get_runtime_window_handle_for_generation(
        &parent.automation_id,
        parent.generation,
    ) != Some(parent.window_handle)
    {
        finish_inline_popup_attach(
            InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ParentRuntimeHandleMismatch,
            },
            lifecycle,
            callback,
            cx,
        );
        return;
    }

    let result = checked_attach_configure_and_show(
        window,
        cx,
        parent.window_handle,
        generation,
        attempt_count,
        parent.host_policy,
    );
    if matches!(
        result,
        InlinePopupAttachResult::Failed {
            failure: InlinePopupAttachFailure::ParentNotReady,
            ..
        }
    ) && attempt_count < 3
    {
        window.defer(cx, move |window, cx| {
            run_inline_popup_attach_attempt(
                window,
                cx,
                InlinePopupAttachAttempt {
                    parent,
                    generation,
                    lifecycle,
                    attempt_count: attempt_count + 1,
                    callback,
                },
            );
        });
        return;
    }

    finish_inline_popup_attach(result, lifecycle, callback, cx);
}

fn finish_inline_popup_attach(
    result: InlinePopupAttachResult,
    lifecycle: InlinePopupLifecycleHandle,
    callback: InlinePopupAttachCallback,
    cx: &mut App,
) {
    let callback = callback.borrow_mut().take();
    if let Some(callback) = callback {
        // Attach runs inside the child's Window::defer update. Registration and
        // failure cleanup both update that same handle, so release its borrow first.
        cx.defer(move |cx| {
            let result = match result {
                InlinePopupAttachResult::Ready(receipt) => {
                    let generation = receipt.generation;
                    if InlinePopupLifecycle::mark_ready(&lifecycle, receipt.clone()) {
                        InlinePopupAttachResult::Ready(receipt)
                    } else {
                        InlinePopupAttachResult::Failed {
                            generation,
                            failure: InlinePopupAttachFailure::StaleGeneration,
                        }
                    }
                }
                failed => failed,
            };
            callback(result, cx);
        });
    }
}

fn owned_hidden_popup_ready(
    child: &mut Window,
    cx: &mut App,
    parent: AnyWindowHandle,
    generation: InlinePopupGeneration,
    attempt_count: u8,
) -> InlinePopupAttachResult {
    let parent_hidden = cx
        .update_window(parent, |_, window, _| window.is_owned_hidden())
        .unwrap_or(false);
    if !child.is_owned_hidden() || !parent_hidden || child.window_handle() == parent {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::HostPolicyMismatch,
        };
    }
    InlinePopupAttachResult::Ready(InlinePopupAttachReceipt {
        generation,
        attempt_count,
        parent_window_number: 0,
        child_window_number: 0,
        parent_visible_at_attach: false,
        child_visible_before_attach: false,
        child_key_before_attach: false,
        parent_child_relation_verified: false,
        configured_after_attach: false,
        child_visible_after_show: false,
        child_key_after_show: false,
        hidden_ready: true,
    })
}

#[cfg(target_os = "macos")]
fn checked_attach_configure_and_show(
    child_window: &mut Window,
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    generation: InlinePopupGeneration,
    attempt_count: u8,
    host_policy: WindowHostPolicy,
) -> InlinePopupAttachResult {
    if host_policy.validate().is_err() {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::HostPolicyMismatch,
        };
    }
    if host_policy.is_hidden() {
        return owned_hidden_popup_ready(
            child_window,
            cx,
            parent_window_handle,
            generation,
            attempt_count,
        );
    }
    if crate::runtime_policy::check(ExternalEffect::NativeVisibility).is_err() {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::HostPolicyMismatch,
        };
    }
    use cocoa::base::{id, nil, NO};

    let Some(child_ns_window) = inline_popup_ns_window(child_window) else {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::ChildNativeWindowMissing,
        };
    };

    let parent = cx.update_window(parent_window_handle, move |_, parent_window, _cx| {
        inline_popup_ns_window(parent_window)
    });
    let parent_ns_window = match parent {
        Ok(Some(window)) => window,
        Ok(None) => {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ParentNativeWindowMissing,
            };
        }
        Err(_) => {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ParentWindowGone,
            };
        }
    };

    if parent_ns_window == nil || child_ns_window == nil {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::ParentNativeWindowMissing,
        };
    }
    if parent_ns_window == child_ns_window {
        return InlinePopupAttachResult::Failed {
            generation,
            failure: InlinePopupAttachFailure::SameNativeWindow,
        };
    }

    // SAFETY: both pointers came from live GPUI windows on the AppKit main
    // thread. All state is observed before the child is ordered front.
    unsafe {
        let parent_visible: cocoa::base::BOOL = msg_send![parent_ns_window, isVisible];
        let child_visible_before: cocoa::base::BOOL = msg_send![child_ns_window, isVisible];
        let child_key_before: cocoa::base::BOOL = msg_send![child_ns_window, isKeyWindow];
        if parent_visible == NO {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ParentNotReady,
            };
        }
        if child_visible_before != NO || child_key_before != NO {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ChildVisibleBeforeAttach,
            };
        }

        let _: () = msg_send![
            parent_ns_window,
            addChildWindow: child_ns_window
            ordered: NS_WINDOW_ABOVE
        ];
        let actual_parent: id = msg_send![child_ns_window, parentWindow];
        if actual_parent != parent_ns_window {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ParentChildRelationRejected,
            };
        }

        crate::platform::configure_inline_dropdown_popup_window(
            child_ns_window,
            crate::theme::get_cached_theme().should_use_dark_vibrancy(),
        );

        let child_visible_after: cocoa::base::BOOL = msg_send![child_ns_window, isVisible];
        let child_key_after: cocoa::base::BOOL = msg_send![child_ns_window, isKeyWindow];
        if child_visible_after == NO {
            return InlinePopupAttachResult::Failed {
                generation,
                failure: InlinePopupAttachFailure::ShowVerificationFailed,
            };
        }

        let parent_window_number: i64 = msg_send![parent_ns_window, windowNumber];
        let child_window_number: i64 = msg_send![child_ns_window, windowNumber];
        InlinePopupAttachResult::Ready(InlinePopupAttachReceipt {
            generation,
            attempt_count,
            parent_window_number,
            child_window_number,
            parent_visible_at_attach: true,
            child_visible_before_attach: false,
            child_key_before_attach: false,
            parent_child_relation_verified: true,
            configured_after_attach: true,
            child_visible_after_show: true,
            child_key_after_show: child_key_after != NO,
            hidden_ready: false,
        })
    }
}

#[cfg(not(target_os = "macos"))]
fn checked_attach_configure_and_show(
    _child_window: &mut Window,
    _cx: &mut App,
    _parent_window_handle: AnyWindowHandle,
    generation: InlinePopupGeneration,
    attempt_count: u8,
    host_policy: WindowHostPolicy,
) -> InlinePopupAttachResult {
    if host_policy.is_hidden() {
        return owned_hidden_popup_ready(
            _child_window,
            _cx,
            _parent_window_handle,
            generation,
            attempt_count,
        );
    }
    InlinePopupAttachResult::Ready(InlinePopupAttachReceipt {
        generation,
        attempt_count,
        parent_window_number: 0,
        child_window_number: 0,
        parent_visible_at_attach: true,
        child_visible_before_attach: false,
        child_key_before_attach: false,
        parent_child_relation_verified: true,
        child_visible_after_show: true,
        child_key_after_show: false,
        configured_after_attach: false,
        hidden_ready: false,
    })
}

#[cfg(target_os = "macos")]
fn ns_window_frame_from_screen_relative_bounds(
    bounds: Bounds<Pixels>,
    screen_frame: NSRect,
) -> NSRect {
    NSRect::new(
        NSPoint::new(
            screen_frame.origin.x + f32::from(bounds.origin.x) as f64,
            screen_frame.origin.y + screen_frame.size.height
                - f32::from(bounds.origin.y) as f64
                - f32::from(bounds.size.height) as f64,
        ),
        NSSize::new(
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        ),
    )
}

/// Update the popup NSWindow bounds without animation. GPUI's bounds are
/// screen-relative; we resolve the popup's current NSScreen and convert back
/// into AppKit coords before calling `setFrame` so multi-monitor setups work.
#[cfg(target_os = "macos")]
pub fn set_inline_popup_window_bounds(window: &mut Window, bounds: Bounds<Pixels>, cx: &mut App) {
    if window.is_owned_hidden() {
        window.resize(bounds.size);
        window.bounds_changed(cx);
        return;
    }
    if crate::runtime_policy::check(ExternalEffect::NativeVisibility).is_err() {
        return;
    }
    if let Some(ns_window) = inline_popup_ns_window(window) {
        // SAFETY: `ns_window` comes from a live GPUI popup window on the AppKit
        // main thread. GPUI `window.bounds()` is screen-relative, so we resolve
        // the popup's current NSScreen and convert back into that screen's
        // AppKit coordinate space before calling `setFrame`.
        unsafe {
            use cocoa::appkit::NSScreen;
            use cocoa::base::nil;

            let screen: cocoa::base::id = msg_send![ns_window, screen];
            let screen_frame = if screen != nil {
                let frame: NSRect = msg_send![screen, frame];
                frame
            } else {
                let screens: cocoa::base::id = NSScreen::screens(nil);
                let primary_screen: cocoa::base::id = msg_send![screens, objectAtIndex: 0u64];
                let frame: NSRect = msg_send![primary_screen, frame];
                frame
            };
            let target_frame = ns_window_frame_from_screen_relative_bounds(bounds, screen_frame);
            let _: () = msg_send![
                ns_window,
                setFrame: target_frame
                display: true
                animate: false
            ];
        }
    }

    window.resize(bounds.size);
    window.bounds_changed(cx);
}

#[cfg(not(target_os = "macos"))]
pub fn set_inline_popup_window_bounds(window: &mut Window, bounds: Bounds<Pixels>, cx: &mut App) {
    let _ = cx;
    window.resize(bounds.size);
}

/// Return the native `NSWindow` handle backing a live GPUI window, or `None`
/// on non-AppKit platforms / failed raw-handle lookup.
#[cfg(target_os = "macos")]
pub fn inline_popup_ns_window(window: &mut Window) -> Option<cocoa::base::id> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use cocoa::base::nil;

            let ns_view = appkit.ns_view.as_ptr() as cocoa::base::id;
            // SAFETY: `ns_view` comes from the live GPUI window on the main thread.
            unsafe {
                let ns_window: cocoa::base::id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some(ns_window);
                }
            }
        }
    }

    None
}

/// Ask AppKit to close one exact live popup window as if its native close
/// affordance had been invoked. Automation callers must resolve and validate
/// the popup generation before passing the handle here.
#[cfg(target_os = "macos")]
pub fn request_native_inline_popup_close(
    handle: AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<i64> {
    crate::runtime_policy::check(ExternalEffect::NativeVisibility)?;
    let (ns_window_address, window_number) = cx
        .update_window(handle, |_entity, window, _cx| {
            let ns_window = inline_popup_ns_window(window)
                .ok_or_else(|| anyhow::anyhow!("popup native NSWindow is unavailable"))?;
            // SAFETY: the NSWindow comes from the exact live GPUI window on the
            // AppKit main thread and remains retained by GPUI through this turn.
            let window_number: i64 = unsafe { msg_send![ns_window, windowNumber] };
            Ok::<_, anyhow::Error>((ns_window as usize, window_number))
        })
        .map_err(|error| anyhow::anyhow!("popup GPUI window is unavailable: {error}"))??;

    cx.spawn(async move |_cx: &mut gpui::AsyncApp| {
        let ns_window = ns_window_address as cocoa::base::id;
        // SAFETY: the foreground executor runs this after the current GPUI
        // RefCell borrow has been released. Borderless popup windows omit the
        // closable mask, so temporarily add that behavior-only bit before
        // `performClose:`; AppKit then traverses GPUI's should-close delegate.
        unsafe {
            use cocoa::base::nil;
            const NS_WINDOW_STYLE_MASK_CLOSABLE: u64 = 1 << 1;
            let style_mask: u64 = msg_send![ns_window, styleMask];
            let _: () =
                msg_send![ns_window, setStyleMask: style_mask | NS_WINDOW_STYLE_MASK_CLOSABLE];
            let _: () = msg_send![ns_window, performClose: nil];
        }
    })
    .detach();
    Ok(window_number)
}

#[cfg(not(target_os = "macos"))]
pub fn request_native_inline_popup_close(
    _handle: AnyWindowHandle,
    _cx: &mut App,
) -> anyhow::Result<i64> {
    anyhow::bail!("native popup close is only available on macOS")
}

pub fn close_prompt_popup_target_natively(
    target: &crate::protocol::AutomationWindowTarget,
    cx: &mut App,
) -> anyhow::Result<(String, u64, i64)> {
    let crate::protocol::AutomationWindowTarget::Instance { id, generation } = target else {
        anyhow::bail!("native popup close requires an exact instance target")
    };
    let resolved = crate::windows::resolve_automation_window(Some(target))?;
    if resolved.kind != crate::protocol::AutomationWindowKind::PromptPopup
        || resolved.generation != Some(*generation)
    {
        anyhow::bail!("native popup close target is not the exact live PromptPopup instance")
    }
    let handle =
        crate::windows::get_valid_runtime_window_handle_for_generation(id, *generation, cx)
            .ok_or_else(|| {
                anyhow::anyhow!("native popup close runtime handle is stale or missing")
            })?;
    let native_window_number = request_native_inline_popup_close(handle, cx)?;
    Ok((id.clone(), *generation, native_window_number))
}

/// Attach the popup NSWindow as a child of the parent launcher/composer
/// window so it follows focus, space moves, and parent closes.
#[cfg(target_os = "macos")]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn attach_inline_popup_to_parent_window(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    child_ns_window: cocoa::base::id,
) {
    if crate::runtime_policy::check(ExternalEffect::NativeVisibility).is_err() {
        return;
    }
    let _ = cx.update_window(parent_window_handle, move |_, parent_window, _cx| {
        let Some(parent_ns_window) = inline_popup_ns_window(parent_window) else {
            return;
        };

        // SAFETY: both NSWindow pointers come from live GPUI windows on the main
        // thread, and nil/equality are guarded before AppKit receives them.
        unsafe {
            use cocoa::base::nil;

            if parent_ns_window == nil
                || child_ns_window == nil
                || parent_ns_window == child_ns_window
            {
                return;
            }

            let _: () = msg_send![
                parent_ns_window,
                addChildWindow: child_ns_window
                ordered: NS_WINDOW_ABOVE
            ];
            let _: () = msg_send![child_ns_window, orderFrontRegardless];
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        footer_anchored_inline_popup_top, inline_popup_bounds, inline_popup_height_for_row_height,
        inline_popup_width_for_window, InlinePopupAttachReceipt, InlinePopupCloseGate,
        InlinePopupLifecycle, InlinePopupPhase, INLINE_POPUP_DEFAULT_WIDTH, INLINE_POPUP_MIN_WIDTH,
    };

    #[test]
    fn inline_popup_height_uses_empty_state_when_zero_rows() {
        assert!(inline_popup_height_for_row_height(0, 36.0) > 0.0);
    }

    #[test]
    fn inline_popup_height_caps_at_max_visible_rows() {
        // 12 rows should be equivalent to 8 rows (the max visible cap).
        assert_eq!(
            inline_popup_height_for_row_height(12, 36.0),
            inline_popup_height_for_row_height(8, 36.0),
        );
    }

    #[test]
    fn inline_popup_height_accepts_custom_row_height() {
        assert!(
            inline_popup_height_for_row_height(8, 36.0)
                < inline_popup_height_for_row_height(8, 40.0)
        );
    }

    #[test]
    fn inline_popup_width_matches_window_constraints() {
        assert_eq!(
            inline_popup_width_for_window(900.0),
            INLINE_POPUP_DEFAULT_WIDTH
        );
        assert_eq!(inline_popup_width_for_window(180.0), INLINE_POPUP_MIN_WIDTH);
    }

    #[test]
    fn footer_anchor_keeps_popup_above_hint_strip() {
        assert!(footer_anchored_inline_popup_top(400.0, 80.0) >= 0.0);
    }

    #[test]
    fn inline_popup_window_options_start_hidden_nonactivating_and_fixed_size() {
        let options = super::inline_popup_window_options(
            gpui::Bounds::default(),
            None,
            crate::runtime_policy::WindowHostPolicy::Interactive,
        );
        assert!(!options.focus);
        assert!(!options.show);
        assert!(!options.is_movable);
        assert!(!options.is_resizable);
    }

    fn ready_receipt(generation: super::InlinePopupGeneration) -> InlinePopupAttachReceipt {
        InlinePopupAttachReceipt {
            generation,
            attempt_count: 1,
            parent_window_number: 1,
            child_window_number: 2,
            parent_visible_at_attach: true,
            child_visible_before_attach: false,
            child_key_before_attach: false,
            parent_child_relation_verified: true,
            configured_after_attach: true,
            child_visible_after_show: true,
            child_key_after_show: false,
            hidden_ready: false,
        }
    }

    #[test]
    fn inline_popup_lifecycle_follows_only_valid_transitions() {
        let lifecycle = InlinePopupLifecycle::new();
        let generation = InlinePopupLifecycle::generation(&lifecycle);
        assert_eq!(
            InlinePopupLifecycle::snapshot(&lifecycle).1,
            InlinePopupPhase::CreatedHidden
        );
        assert!(InlinePopupLifecycle::begin_attach(&lifecycle, generation));
        assert!(InlinePopupLifecycle::mark_ready(
            &lifecycle,
            ready_receipt(generation)
        ));
        assert_eq!(
            InlinePopupLifecycle::snapshot(&lifecycle).1,
            InlinePopupPhase::Open
        );
        assert_eq!(
            InlinePopupLifecycle::request_close(&lifecycle, generation),
            InlinePopupCloseGate::Begin
        );
        assert_eq!(
            InlinePopupLifecycle::request_close(&lifecycle, generation),
            InlinePopupCloseGate::AlreadyClosing
        );
        assert!(InlinePopupLifecycle::mark_closed(&lifecycle, generation));
        assert_eq!(
            InlinePopupLifecycle::request_close(&lifecycle, generation),
            InlinePopupCloseGate::AlreadyClosed
        );
    }

    #[test]
    fn inline_popup_lifecycle_rejects_open_before_attach_and_stale_callbacks() {
        let lifecycle = InlinePopupLifecycle::new();
        let generation = InlinePopupLifecycle::generation(&lifecycle);
        assert!(!InlinePopupLifecycle::mark_ready(
            &lifecycle,
            ready_receipt(generation)
        ));
        assert!(InlinePopupLifecycle::begin_attach(&lifecycle, generation));
        assert_eq!(
            InlinePopupLifecycle::request_close(&lifecycle, generation),
            InlinePopupCloseGate::Begin
        );
        assert!(!InlinePopupLifecycle::mark_ready(
            &lifecycle,
            ready_receipt(generation)
        ));
        assert_eq!(
            InlinePopupLifecycle::request_close(
                &lifecycle,
                super::InlinePopupGeneration(generation.get() + 1)
            ),
            InlinePopupCloseGate::StaleGeneration
        );
    }

    #[test]
    fn hidden_ready_never_claims_native_attachment_visibility_or_focus() {
        let lifecycle = InlinePopupLifecycle::new();
        let generation = InlinePopupLifecycle::generation(&lifecycle);
        assert!(InlinePopupLifecycle::begin_attach(&lifecycle, generation));
        let mut receipt = ready_receipt(generation);
        receipt.hidden_ready = true;
        assert!(!InlinePopupLifecycle::mark_ready(
            &lifecycle,
            receipt.clone()
        ));
        receipt.parent_visible_at_attach = false;
        receipt.parent_child_relation_verified = false;
        receipt.configured_after_attach = false;
        receipt.child_visible_after_show = false;
        receipt.child_key_after_show = true;
        assert!(!InlinePopupLifecycle::mark_ready(
            &lifecycle,
            receipt.clone()
        ));
        receipt.child_key_after_show = false;
        assert!(InlinePopupLifecycle::mark_ready(&lifecycle, receipt));
        assert_eq!(
            InlinePopupLifecycle::snapshot(&lifecycle).1,
            InlinePopupPhase::Open
        );
    }

    #[test]
    fn inline_popup_bounds_offset_from_parent_origin() {
        let parent = gpui::Bounds {
            origin: gpui::point(gpui::px(100.0), gpui::px(40.0)),
            size: gpui::size(gpui::px(600.0), gpui::px(400.0)),
        };

        let bounds = inline_popup_bounds(parent, 8.0, 16.0, 200.0, 80.0);
        assert_eq!(f32::from(bounds.origin.x), 108.0);
        assert_eq!(f32::from(bounds.origin.y), 56.0);
        assert_eq!(f32::from(bounds.size.width), 200.0);
        assert_eq!(f32::from(bounds.size.height), 80.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn screen_relative_bounds_convert_to_nswindow_frame_on_secondary_display() {
        let bounds = gpui::Bounds {
            origin: gpui::point(gpui::px(24.0), gpui::px(60.0)),
            size: gpui::size(gpui::px(320.0), gpui::px(84.0)),
        };
        let screen_frame = cocoa::foundation::NSRect::new(
            cocoa::foundation::NSPoint::new(1440.0, 0.0),
            cocoa::foundation::NSSize::new(1920.0, 1200.0),
        );

        let frame = super::ns_window_frame_from_screen_relative_bounds(bounds, screen_frame);

        assert_eq!(frame.origin.x, 1464.0);
        assert_eq!(frame.origin.y, 1056.0);
        assert_eq!(frame.size.width, 320.0);
        assert_eq!(frame.size.height, 84.0);
    }
}
