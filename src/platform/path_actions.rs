use std::path::{Path, PathBuf};

fn resolve_existing_path(path: &Path, action: &str) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|error| {
        let error_message = format!(
            "platform_{}_failed: stage=canonicalize_path path={} error={}",
            action,
            path.display(),
            error
        );
        tracing::error!(
            action = action,
            stage = "canonicalize_path",
            path = %path.display(),
            error = %error,
            "platform path action failed"
        );
        error_message
    })
}

/// Reveal the path in Finder on macOS.
///
/// On non-macOS platforms, opens the containing folder with the system default handler.
pub fn reveal_in_finder(path: &Path) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal).map_err(|error| error.to_string())?;
    let resolved_path = resolve_existing_path(path, "reveal_in_finder")?;
    reveal_in_finder_impl(&resolved_path)
}

#[cfg(target_os = "macos")]
fn reveal_in_finder_impl(path: &Path) -> Result<(), String> {
    if require_main_thread("reveal_in_finder") {
        let error_message = format!(
            "platform_reveal_in_finder_failed: stage=main_thread_required path={} error=AppKit requires main thread",
            path.display()
        );
        tracing::error!(
            action = "reveal_in_finder",
            stage = "main_thread_required",
            path = %path.display(),
            "platform path action failed"
        );
        return Err(error_message);
    }

    let path_str = path.to_string_lossy();

    // SAFETY: Main thread is verified above. Objective-C objects are checked for nil
    // before use and the method calls are standard NSWorkspace/NSURL APIs.
    unsafe {
        let workspace: cocoa::base::id =
            objc::msg_send![objc::class!(NSWorkspace), sharedWorkspace];
        if workspace == cocoa::base::nil {
            let error_message = format!(
                "platform_reveal_in_finder_failed: stage=workspace_unavailable path={} error=NSWorkspace.sharedWorkspace returned nil",
                path.display()
            );
            tracing::error!(
                action = "reveal_in_finder",
                stage = "workspace_unavailable",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        let ns_path = cocoa::foundation::NSString::alloc(cocoa::base::nil).init_str(&path_str);
        if ns_path == cocoa::base::nil {
            let error_message = format!(
                "platform_reveal_in_finder_failed: stage=nsstring_creation path={} error=Failed to create NSString",
                path.display()
            );
            tracing::error!(
                action = "reveal_in_finder",
                stage = "nsstring_creation",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        let file_url: cocoa::base::id =
            objc::msg_send![objc::class!(NSURL), fileURLWithPath: ns_path];
        if file_url == cocoa::base::nil {
            let _: () = objc::msg_send![ns_path, release];
            let error_message = format!(
                "platform_reveal_in_finder_failed: stage=file_url_creation path={} error=Failed to create file URL",
                path.display()
            );
            tracing::error!(
                action = "reveal_in_finder",
                stage = "file_url_creation",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        let urls: cocoa::base::id =
            objc::msg_send![objc::class!(NSArray), arrayWithObject: file_url];
        if urls == cocoa::base::nil {
            let _: () = objc::msg_send![ns_path, release];
            let error_message = format!(
                "platform_reveal_in_finder_failed: stage=url_array_creation path={} error=Failed to create NSArray",
                path.display()
            );
            tracing::error!(
                action = "reveal_in_finder",
                stage = "url_array_creation",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        tracing::debug!(
            action = "reveal_in_finder",
            stage = "dispatch_finder_select",
            path = %path.display(),
            "dispatching NSWorkspace reveal request"
        );

        let _: () = objc::msg_send![workspace, activateFileViewerSelectingURLs: urls];
        let _: () = objc::msg_send![ns_path, release];

        tracing::info!(
            action = "reveal_in_finder",
            stage = "completed",
            path = %path.display(),
            "platform path action completed"
        );
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn reveal_in_finder_impl(path: &Path) -> Result<(), String> {
    let target = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };

    tracing::debug!(
        action = "reveal_in_finder",
        stage = "fallback_open_parent",
        path = %path.display(),
        target = %target.display(),
        "using non-macOS reveal fallback"
    );

    open::that(target).map_err(|error| {
        let error_message = format!(
            "platform_reveal_in_finder_failed: stage=fallback_open_parent path={} target={} error={}",
            path.display(),
            target.display(),
            error
        );
        tracing::error!(
            action = "reveal_in_finder",
            stage = "fallback_open_parent",
            path = %path.display(),
            target = %target.display(),
            error = %error,
            "platform path action failed"
        );
        error_message
    })?;

    tracing::info!(
        action = "reveal_in_finder",
        stage = "completed_fallback",
        path = %path.display(),
        target = %target.display(),
        "platform path action completed"
    );

    Ok(())
}

/// Open a path with the default application.
///
/// On macOS, this uses `NSWorkspace.openURL` with a `file://` URL.
pub fn open_in_default_app(path: &Path) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal).map_err(|error| error.to_string())?;
    let resolved_path = resolve_existing_path(path, "open_in_default_app")?;
    open_in_default_app_impl(&resolved_path)
}

#[cfg(target_os = "macos")]
fn open_in_default_app_impl(path: &Path) -> Result<(), String> {
    if require_main_thread("open_in_default_app") {
        let error_message = format!(
            "platform_open_in_default_app_failed: stage=main_thread_required path={} error=AppKit requires main thread",
            path.display()
        );
        tracing::error!(
            action = "open_in_default_app",
            stage = "main_thread_required",
            path = %path.display(),
            "platform path action failed"
        );
        return Err(error_message);
    }

    let path_str = path.to_string_lossy();

    // SAFETY: Main thread is verified above. Objective-C objects are checked for nil
    // before use and the method calls are standard NSWorkspace/NSURL APIs.
    unsafe {
        let workspace: cocoa::base::id =
            objc::msg_send![objc::class!(NSWorkspace), sharedWorkspace];
        if workspace == cocoa::base::nil {
            let error_message = format!(
                "platform_open_in_default_app_failed: stage=workspace_unavailable path={} error=NSWorkspace.sharedWorkspace returned nil",
                path.display()
            );
            tracing::error!(
                action = "open_in_default_app",
                stage = "workspace_unavailable",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        let ns_path = cocoa::foundation::NSString::alloc(cocoa::base::nil).init_str(&path_str);
        if ns_path == cocoa::base::nil {
            let error_message = format!(
                "platform_open_in_default_app_failed: stage=nsstring_creation path={} error=Failed to create NSString",
                path.display()
            );
            tracing::error!(
                action = "open_in_default_app",
                stage = "nsstring_creation",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        let file_url: cocoa::base::id =
            objc::msg_send![objc::class!(NSURL), fileURLWithPath: ns_path];
        if file_url == cocoa::base::nil {
            let _: () = objc::msg_send![ns_path, release];
            let error_message = format!(
                "platform_open_in_default_app_failed: stage=file_url_creation path={} error=Failed to create file URL",
                path.display()
            );
            tracing::error!(
                action = "open_in_default_app",
                stage = "file_url_creation",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        tracing::debug!(
            action = "open_in_default_app",
            stage = "dispatch_open_url",
            path = %path.display(),
            "dispatching NSWorkspace open request"
        );

        let did_open: bool = objc::msg_send![workspace, openURL: file_url];
        let _: () = objc::msg_send![ns_path, release];

        if !did_open {
            let error_message = format!(
                "platform_open_in_default_app_failed: stage=open_url path={} error=NSWorkspace.openURL returned false",
                path.display()
            );
            tracing::error!(
                action = "open_in_default_app",
                stage = "open_url",
                path = %path.display(),
                "platform path action failed"
            );
            return Err(error_message);
        }

        tracing::info!(
            action = "open_in_default_app",
            stage = "completed",
            path = %path.display(),
            "platform path action completed"
        );
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn open_in_default_app_impl(path: &Path) -> Result<(), String> {
    tracing::debug!(
        action = "open_in_default_app",
        stage = "fallback_open",
        path = %path.display(),
        "using non-macOS open fallback"
    );

    open::that(path).map_err(|error| {
        let error_message = format!(
            "platform_open_in_default_app_failed: stage=fallback_open path={} error={}",
            path.display(),
            error
        );
        tracing::error!(
            action = "open_in_default_app",
            stage = "fallback_open",
            path = %path.display(),
            error = %error,
            "platform path action failed"
        );
        error_message
    })?;

    tracing::info!(
        action = "open_in_default_app",
        stage = "completed_fallback",
        path = %path.display(),
        "platform path action completed"
    );

    Ok(())
}

/// Copy application text to its actual destination: the system clipboard in
/// normal mode, or the bounded process-local sink under the owned policy.
/// Local copies are not available to system clipboard reads or native paste.
pub fn copy_text(text: &str) -> Result<crate::runtime_policy::CopyReceipt, String> {
    crate::runtime_policy::route_text_copy(
        crate::runtime_policy::owned_evaluation(),
        text,
        copy_text_to_clipboard,
    )
}

/// Copy text to the system clipboard.
///
/// On macOS this uses `NSPasteboard`. On other platforms it falls back to `arboard`.
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard).map_err(|error| error.to_string())?;
    copy_text_to_clipboard_impl(text)
}

#[cfg(target_os = "macos")]
fn copy_text_to_clipboard_impl(text: &str) -> Result<(), String> {
    if require_main_thread("copy_text_to_clipboard") {
        let error_message =
            "platform_copy_text_to_clipboard_failed: stage=main_thread_required error=AppKit requires main thread"
                .to_string();
        tracing::error!(
            action = "copy_text_to_clipboard",
            stage = "main_thread_required",
            "platform clipboard action failed"
        );
        return Err(error_message);
    }

    // SAFETY: Main thread is verified above. Objective-C objects are checked for nil
    // before use and the method calls are standard NSPasteboard APIs.
    unsafe {
        let pasteboard: cocoa::base::id =
            objc::msg_send![objc::class!(NSPasteboard), generalPasteboard];
        if pasteboard == cocoa::base::nil {
            let error_message =
                "platform_copy_text_to_clipboard_failed: stage=pasteboard_unavailable error=NSPasteboard.generalPasteboard returned nil".to_string();
            tracing::error!(
                action = "copy_text_to_clipboard",
                stage = "pasteboard_unavailable",
                "platform clipboard action failed"
            );
            return Err(error_message);
        }

        let _: i64 = objc::msg_send![pasteboard, clearContents];

        let ns_text = cocoa::foundation::NSString::alloc(cocoa::base::nil).init_str(text);
        if ns_text == cocoa::base::nil {
            let error_message =
                "platform_copy_text_to_clipboard_failed: stage=nsstring_creation error=Failed to create NSString".to_string();
            tracing::error!(
                action = "copy_text_to_clipboard",
                stage = "nsstring_creation",
                "platform clipboard action failed"
            );
            return Err(error_message);
        }

        let objects: cocoa::base::id =
            objc::msg_send![objc::class!(NSArray), arrayWithObject: ns_text];
        if objects == cocoa::base::nil {
            let _: () = objc::msg_send![ns_text, release];
            let error_message =
                "platform_copy_text_to_clipboard_failed: stage=object_array_creation error=Failed to create NSArray".to_string();
            tracing::error!(
                action = "copy_text_to_clipboard",
                stage = "object_array_creation",
                "platform clipboard action failed"
            );
            return Err(error_message);
        }

        let did_write: bool = objc::msg_send![pasteboard, writeObjects: objects];
        let _: () = objc::msg_send![ns_text, release];

        if !did_write {
            let error_message =
                "platform_copy_text_to_clipboard_failed: stage=write_objects error=NSPasteboard.writeObjects returned false".to_string();
            tracing::error!(
                action = "copy_text_to_clipboard",
                stage = "write_objects",
                "platform clipboard action failed"
            );
            return Err(error_message);
        }

        tracing::info!(
            action = "copy_text_to_clipboard",
            stage = "completed",
            text_length = text.len(),
            "platform clipboard action completed"
        );
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
fn copy_text_to_clipboard_impl(text: &str) -> Result<(), String> {
    tracing::debug!(
        action = "copy_text_to_clipboard",
        stage = "fallback_arboard",
        text_length = text.len(),
        "using non-macOS clipboard fallback"
    );

    let mut clipboard = arboard::Clipboard::new().map_err(|error| {
        let error_message = format!(
            "platform_copy_text_to_clipboard_failed: stage=fallback_clipboard_init error={}",
            error
        );
        tracing::error!(
            action = "copy_text_to_clipboard",
            stage = "fallback_clipboard_init",
            error = %error,
            "platform clipboard action failed"
        );
        error_message
    })?;

    clipboard.set_text(text.to_string()).map_err(|error| {
        let error_message = format!(
            "platform_copy_text_to_clipboard_failed: stage=fallback_clipboard_set_text error={}",
            error
        );
        tracing::error!(
            action = "copy_text_to_clipboard",
            stage = "fallback_clipboard_set_text",
            error = %error,
            "platform clipboard action failed"
        );
        error_message
    })?;

    tracing::info!(
        action = "copy_text_to_clipboard",
        stage = "completed_fallback",
        text_length = text.len(),
        "platform clipboard action completed"
    );

    Ok(())
}

// ============================================================================
// Native File Drag-Out
// ============================================================================

/// Start a native macOS drag session carrying the given file path.
///
/// This allows users to drag files from the mini explorer directly into
/// Finder, other apps, or the Desktop. The function must be called from
/// the main thread during a mouse event (typically from an `on_drag` callback).
#[cfg(target_os = "macos")]
pub fn begin_native_file_drag(path: &str) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput).map_err(|error| error.to_string())?;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSArray, NSPoint, NSString};
    use objc::{class, msg_send, sel, sel_impl};

    if require_main_thread("begin_native_file_drag") {
        return Err("begin_native_file_drag requires main thread".to_string());
    }

    let resolved = resolve_existing_path(std::path::Path::new(path), "begin_native_file_drag")?;
    let path_str = resolved.to_string_lossy();

    // SAFETY: Main thread verified above. All ObjC objects are nil-checked.
    // We use standard NSURL, NSPasteboardItem, NSDraggingItem, and NSView APIs.
    // The dragging session is owned by AppKit and cleaned up automatically.
    unsafe {
        // Ensure the GPUIView class has NSDraggingSource support
        ensure_dragging_source_protocol();

        let window = crate::window_manager::get_main_window().ok_or_else(|| {
            "begin_native_file_drag: no main window available".to_string()
        })?;
        let content_view: id = msg_send![window, contentView];
        if content_view.is_null() {
            return Err("begin_native_file_drag: no content view".to_string());
        }

        // Create file URL
        let ns_path = NSString::alloc(nil).init_str(&path_str);
        let file_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
        if file_url.is_null() {
            return Err("begin_native_file_drag: failed to create NSURL".to_string());
        }

        // Create pasteboard item with file URL
        let pb_item: id = msg_send![class!(NSPasteboardItem), new];
        if pb_item.is_null() {
            return Err("begin_native_file_drag: failed to create NSPasteboardItem".to_string());
        }
        let url_string: id = msg_send![file_url, absoluteString];
        let uti = NSString::alloc(nil).init_str("public.file-url");
        let _ok: bool = msg_send![pb_item, setString: url_string forType: uti];

        // Create dragging item from pasteboard item
        let drag_item: id = msg_send![class!(NSDraggingItem), alloc];
        let drag_item: id = msg_send![drag_item, initWithPasteboardWriter: pb_item];
        if drag_item.is_null() {
            return Err("begin_native_file_drag: failed to create NSDraggingItem".to_string());
        }

        // Set the dragging frame (icon position) — use a small rect at the current mouse location
        let mouse_loc: NSPoint = msg_send![window, mouseLocationOutsideOfEventStream];
        let null_view: id = nil;
        let view_mouse: NSPoint = msg_send![content_view, convertPoint: mouse_loc fromView: null_view];
        let frame = cocoa::foundation::NSRect::new(
            NSPoint::new(view_mouse.x - 16.0, view_mouse.y - 16.0),
            cocoa::foundation::NSSize::new(32.0, 32.0),
        );
        let null_image: id = nil;
        let _: () = msg_send![drag_item, setDraggingFrame: frame contents: null_image];

        // Get the current event for the drag session
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let current_event: id = msg_send![app, currentEvent];
        if current_event.is_null() {
            return Err("begin_native_file_drag: no current event".to_string());
        }

        // Start the drag session
        let items = NSArray::arrayWithObject(nil, drag_item);
        let _session: id = msg_send![
            content_view,
            beginDraggingSessionWithItems: items
            event: current_event
            source: content_view
        ];

        tracing::info!(
            action = "begin_native_file_drag",
            path = %path_str,
            "native file drag session started"
        );
    }

    Ok(())
}

/// Register NSDraggingSource protocol methods on the GPUIView class at runtime.
///
/// This is called once lazily before the first drag. It adds the required
/// `draggingSession:sourceOperationMaskForDraggingContext:` method so that
/// AppKit accepts the view as a drag source.
#[cfg(target_os = "macos")]
fn ensure_dragging_source_protocol() {
    use std::sync::Once;
    static REGISTER: Once = Once::new();

    REGISTER.call_once(|| {
        // SAFETY: We use the ObjC runtime API to add a method to an existing class.
        // The function pointer signature matches the expected ObjC method signature.
        // This is safe because: (1) the class is already registered, (2) class_addMethod
        // is thread-safe for adding methods, (3) we only call this once via Once.
        unsafe {
            let cls = objc::runtime::Class::get("GPUIView");
            let cls = match cls {
                Some(c) => c,
                None => {
                    tracing::warn!("GPUIView class not found; skipping drag source registration");
                    return;
                }
            };

            // Add draggingSession:sourceOperationMaskForDraggingContext:
            // NSDragOperation is an NSUInteger (u64 on 64-bit)
            // NSDraggingContext is an NSInteger (i64 on 64-bit)
            extern "C" fn dragging_source_operation_mask(
                _this: &objc::runtime::Object,
                _sel: objc::runtime::Sel,
                _session: cocoa::base::id,
                _context: i64,
            ) -> u64 {
                // NSDragOperationCopy = 1
                1u64
            }

            let sel = objc::runtime::Sel::register(
                "draggingSession:sourceOperationMaskForDraggingContext:",
            );

            // Encoding: Q@:@q  (return=NSUInteger, self=@, _cmd=:, session=@, context=NSInteger)
            let encoding = c"Q@:@q";

            #[allow(clippy::missing_transmute_annotations)]
            let imp: objc::runtime::Imp = std::mem::transmute(
                dragging_source_operation_mask
                    as extern "C" fn(
                        &objc::runtime::Object,
                        objc::runtime::Sel,
                        cocoa::base::id,
                        i64,
                    ) -> u64,
            );

            // class_addMethod returns false if method already exists — that's fine
            let cls_ptr = cls as *const objc::runtime::Class as *mut objc::runtime::Class;
            let _added =
                objc::runtime::class_addMethod(cls_ptr, sel, imp, encoding.as_ptr());

            tracing::debug!("registered NSDraggingSource on GPUIView");
        }
    });
}

/// Non-macOS stub for native file drag.
#[cfg(not(target_os = "macos"))]
pub fn begin_native_file_drag(_path: &str) -> Result<(), String> {
    Err("Native file drag is only supported on macOS".to_string())
}
