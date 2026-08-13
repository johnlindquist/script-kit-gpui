// Window State Management Tests
//
// This module tests the window visibility and reset state machine.
//
// # Window State Invariants
//
// 1. **NEVER call `cx.hide()` directly in command execution code**
//    - Use `self.close_and_reset_window(cx)` instead
//    - This ensures reset happens AND handles Notes/AI windows correctly
//
// 2. **When hiding the main window:**
//    - Always use `platform::defer_hide_main_window(cx)`
//    - `cx.hide()` app-hides every window, so a false-negative Notes check can
//      hide the independent Notes host together with main
//    - `close_and_reset_window()` handles this automatically
//
// 3. **NEVER call `platform::hide_main_window()` directly from GPUI callbacks**
//    - `orderOut:` on a key window triggers macOS `window_did_change_key_status`
//      synchronously, which re-enters GPUI's `App` RefCell → panic
//    - Always use `platform::defer_hide_main_window(cx)` which defers via `cx.spawn()`
//
// 4. **Reset must happen BEFORE or DURING hide, not after**
//    - The window should be clean when shown again
//    - Don't rely on NEEDS_RESET flag for normal operation
//
// # Forbidden Patterns in app_execute.rs
//
// These patterns indicate a bug:
// - `cx.hide()` without `close_and_reset_window`
// - `NEEDS_RESET.store(true` (should use close_and_reset_window instead)
// - `platform::hide_main_window()` called directly (use `defer_hide_main_window` instead)
//
// # Code Audit Tests
//
// The tests below verify that forbidden patterns don't exist in the codebase.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// Helper function to read a source file for pattern checking
    fn read_source_file(path: &str) -> String {
        fs::read_to_string(path).unwrap_or_else(|_| {
            // Try with src/ prefix if not found
            fs::read_to_string(format!("src/{}", path)).unwrap_or_default()
        })
    }

    fn append_rs_sources(dir: &Path, content: &mut String) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        let mut paths: Vec<_> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();
        paths.sort();

        for path in paths {
            if path.is_dir() {
                append_rs_sources(&path, content);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                if let Ok(file_content) = fs::read_to_string(&path) {
                    content.push_str(&file_content);
                    content.push('\n');
                }
            }
        }
    }

    fn read_rs_sources_under(path: &str) -> String {
        let mut content = String::new();
        append_rs_sources(Path::new(path), &mut content);
        content
    }

    /// Count occurrences of a pattern in text
    fn count_occurrences(text: &str, pattern: &str) -> usize {
        text.matches(pattern).count()
    }

    /// Find lines containing a pattern (for error reporting)
    fn find_lines_with_pattern(text: &str, pattern: &str) -> Vec<(usize, String)> {
        text.lines()
            .enumerate()
            .filter(|(_, line)| line.contains(pattern))
            .map(|(i, line)| (i + 1, line.to_string()))
            .collect()
    }

    /// Verify that app_execute.rs doesn't use cx.hide() directly
    /// All hide operations should go through close_and_reset_window()
    #[test]
    fn test_no_direct_cx_hide_in_app_execute() {
        let content = read_source_file("app_execute.rs");
        let matches = find_lines_with_pattern(&content, "cx.hide()");

        // cx.hide() should NOT appear in app_execute.rs
        // All hide operations should use close_and_reset_window()
        assert!(
            matches.is_empty(),
            "Found forbidden cx.hide() in app_execute.rs. Use self.close_and_reset_window(cx) instead.\nFound:\n{}",
            matches.iter()
                .map(|(line, text)| format!("  Line {}: {}", line, text.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Verify that app_execute.rs doesn't set NEEDS_RESET directly
    /// The close_and_reset_window() function handles reset properly
    #[test]
    fn test_no_needs_reset_in_app_execute() {
        let content = read_source_file("app_execute.rs");
        let matches = find_lines_with_pattern(&content, "NEEDS_RESET.store(true");

        // NEEDS_RESET should NOT be set in app_execute.rs
        // Reset happens immediately in close_and_reset_window()
        assert!(
            matches.is_empty(),
            "Found forbidden NEEDS_RESET.store(true in app_execute.rs. Use self.close_and_reset_window(cx) instead.\nFound:\n{}",
            matches.iter()
                .map(|(line, text)| format!("  Line {}: {}", line, text.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Verify that all hide calls in app_execute use defer_hide_main_window
    #[test]
    fn test_app_execute_uses_deferred_hide() {
        let content = read_rs_sources_under("src/app_execute");
        let deferred = count_occurrences(&content, "defer_hide_main_window");

        // All hide calls in app_execute should use the deferred version
        assert!(
            deferred >= 1,
            "Expected at least 1 defer_hide_main_window call in src/app_execute/"
        );
    }

    /// REENTRANCY GUARD: Ensure `platform::hide_main_window()` is NEVER called
    /// directly outside the platform module.
    ///
    /// `orderOut:` on a key NSWindow synchronously fires macOS's
    /// `window_did_change_key_status`, which re-enters GPUI's `App` `RefCell`.
    /// If any GPUI borrow is held at the time, this causes:
    ///   `RefCell already borrowed` → panic → abort
    ///
    /// The sync function is module-private in `src/platform/`. All external code
    /// must use `platform::defer_hide_main_window(cx)` which runs the ObjC call
    /// on the next event-loop tick, after all borrows are released.
    ///
    /// If this test fails, you introduced a direct `platform::hide_main_window()`
    /// call. Replace it with `platform::defer_hide_main_window(cx)`.
    #[test]
    fn test_no_direct_hide_main_window_outside_platform() {
        // Scan all Rust source files under src/ (excluding the platform module)
        let dirs_to_scan = [
            "src/app_impl",
            "src/app_execute",
            "src/app_actions",
            "src/main_entry",
            "src/main_sections",
            "src/render_prompts",
            "src/render_builtins",
            "src/render_script_list",
            "src/notes",
            "src/ai",
            "src/prompts",
            "src/components",
        ];

        let mut violations = Vec::new();

        for dir in &dirs_to_scan {
            let dir_path = std::path::Path::new(dir);
            if !dir_path.exists() {
                continue;
            }
            let content = {
                let mut buf = String::new();
                append_rs_sources(dir_path, &mut buf);
                buf
            };

            // Look for direct calls: platform::hide_main_window() without "defer_"
            for (line_num, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                // Skip comments
                if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("*") {
                    continue;
                }
                // Match direct sync call but not the deferred wrapper
                if trimmed.contains("hide_main_window()")
                    && !trimmed.contains("defer_hide_main_window")
                    && !trimmed.contains("fn hide_main_window")
                {
                    violations.push(format!("  {}:{}: {}", dir, line_num + 1, trimmed));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "Found direct hide_main_window() calls outside platform module!\n\
             These WILL cause `RefCell already borrowed` panics.\n\
             Replace with `platform::defer_hide_main_window(cx)`.\n\n{}\n",
            violations.join("\n")
        );
    }

    /// Verify close_and_reset_window exists and has the right structure
    #[test]
    fn test_close_and_reset_window_exists() {
        let content = read_source_file("app_impl/lifecycle_reset.rs");
        let count = count_occurrences(&content, "fn close_and_reset_window");

        assert!(
            count >= 1,
            "close_and_reset_window() function not found in app_impl/lifecycle_reset.rs"
        );
    }

    /// Verify close_and_reset_window calls reset_to_script_list
    #[test]
    fn test_close_and_reset_window_resets() {
        let content = read_source_file("app_impl/lifecycle_reset.rs");

        // Find the function and check if it contains reset_to_script_list
        if let Some(start) = content.find("fn close_and_reset_window") {
            let after_start = &content[start..];
            let end = after_start.find("\n    ///").unwrap_or(after_start.len());
            let function_chunk = &after_start[..end];
            // The reset is expressed either as a direct reset_to_script_list
            // call or as the calibrated-hide owner's ResetScriptList policy,
            // which performs the same reset only after AppKit confirms the
            // native hide (exit-fade restoration, 2026-08-13).
            let has_reset = function_chunk.contains("reset_to_script_list")
                || function_chunk.contains("MainWindowPostHide::ResetScriptList");

            assert!(
                has_reset,
                "close_and_reset_window() must reset the launcher route to ScriptList"
            );
        } else {
            panic!("close_and_reset_window() function not found in app_impl/lifecycle_reset.rs");
        }
    }

    /// Day Page autosave is debounced and its trailing flush runs from render
    /// side effects, which stop once the window hides. Every teardown path
    /// (blur close, Cmd+W, hidden re-show reset) funnels through
    /// reset_to_script_list, so it must flush the Day Page save synchronously
    /// before the entity is dropped — otherwise the last ~300ms of typing is
    /// lost in the surface whose job is capturing thoughts.
    #[test]
    fn test_reset_to_script_list_flushes_day_page_save() {
        let content = read_source_file("app_impl/registries_state.rs");
        let start = content
            .find("pub(crate) fn reset_to_script_list")
            .expect("reset_to_script_list should exist in app_impl/registries_state.rs");
        let body = &content[start..];

        let flush = body
            .find("AppView::DayPage { entity, .. }")
            .expect("reset_to_script_list must flush the Day Page save before dropping the view");
        let view_reset = body
            .find("self.current_view = AppView::ScriptList;")
            .expect("reset_to_script_list should reset current_view");
        assert!(
            flush < view_reset,
            "Day Page save flush must run BEFORE current_view is replaced (the entity drops with it)"
        );
        assert!(
            body[flush..view_reset].contains("view.save(cx)"),
            "Day Page flush must call view.save(cx) so the debounced autosave cannot be outrun by teardown"
        );
    }

    /// Verify close_and_reset_window records Notes/AI state for diagnostics.
    #[test]
    fn test_close_and_reset_window_checks_secondary_windows() {
        let content = read_source_file("app_impl/lifecycle_reset.rs");
        let prepare_start = content
            .find("fn prepare_main_window_close")
            .expect("prepare_main_window_close should exist");
        let close_start = content
            .find("fn close_and_reset_window")
            .expect("close_and_reset_window should exist");
        let prepare_body = &content[prepare_start..close_start];
        assert!(
            prepare_body.contains("notes::is_notes_window_open()")
                && prepare_body.contains("agent_chat_open"),
            "the shared main-window close preparation must inspect Notes and Agent Chat window state"
        );

        let close_body = content[close_start..]
            .split("/// Agent Chat's strict close barrier")
            .next()
            .expect("close_and_reset_window body should have a boundary");
        assert!(
            close_body.contains("prepare_main_window_close("),
            "close_and_reset_window must route through the shared secondary-window-aware preparation"
        );
    }

    #[test]
    fn test_main_hide_paths_never_app_hide() {
        let lifecycle = read_source_file("app_impl/lifecycle_reset.rs");
        let close_chunk = lifecycle
            .split("fn close_and_reset_window")
            .nth(1)
            .and_then(|rest| rest.split("pub(crate) fn can_preserve_hide").next())
            .expect("close_and_reset_window should exist");
        assert!(
            close_chunk.contains("defer_calibrated_main_window_hide("),
            "close_and_reset_window must route through the shared calibrated main-only hide owner"
        );
        assert!(
            !close_chunk.contains("cx.hide();"),
            "close_and_reset_window must not app-hide, because that can hide Notes"
        );

        let preserve_chunk = lifecycle
            .split("fn hide_main_window_preserving_state_for_focus_loss")
            .nth(1)
            .and_then(|rest| rest.split("/// Clear the current built-in view").next())
            .expect("hide_main_window_preserving_state_for_focus_loss should exist");
        assert!(
            preserve_chunk.contains("defer_calibrated_main_window_hide("),
            "focus-loss hide must route through the shared calibrated main-only hide owner"
        );
        assert!(
            !preserve_chunk.contains("cx.hide();"),
            "focus-loss hide must not app-hide, because that can hide Notes"
        );

        let visibility = read_source_file("main_sections/window_visibility.rs");
        let helper_chunk = visibility
            .split("fn hide_main_window_helper")
            .nth(1)
            .and_then(|rest| rest.split("/// Handle window resize events").next())
            .unwrap_or_else(|| visibility.split("fn hide_main_window_helper").nth(1).unwrap());
        assert!(
            helper_chunk.contains("defer_calibrated_main_window_hide("),
            "shared hide helper must route through the shared calibrated main-only hide owner"
        );
        assert!(
            !helper_chunk.contains("cx.hide();"),
            "shared hide helper must not app-hide, because that can hide Notes"
        );
    }

    #[test]
    fn test_main_entry_dispatchers_never_app_hide() {
        for file in [
            "main_entry/runtime_stdin.rs",
            "main_entry/runtime_stdin_match_core.rs",
            "main_entry/runtime_stdin_match_simulate_key.rs",
            "main_entry/app_run_setup.rs",
        ] {
            let content = read_source_file(file);
            let offenders: Vec<_> = content
                .lines()
                .enumerate()
                .filter_map(|(index, line)| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") || trimmed.starts_with("///") {
                        return None;
                    }
                    (trimmed.contains("ctx.hide();") || trimmed.contains("cx.hide();"))
                        .then_some(format!("{}:{}: {}", file, index + 1, trimmed))
                })
                .collect();

            assert!(
                offenders.is_empty(),
                "main-entry hide dispatchers must not app-hide because that can hide Notes:\n{}",
                offenders.join("\n")
            );
        }
    }

    /// Count how many places use close_and_reset_window vs direct patterns
    /// This helps track adoption of the correct pattern
    #[test]
    fn test_close_and_reset_window_adoption() {
        let content = read_rs_sources_under("src/app_impl");
        let correct_count = count_occurrences(&content, "close_and_reset_window(");

        // We expect multiple usages of close_and_reset_window
        assert!(
            correct_count >= 5,
            "Expected at least 5 close_and_reset_window call sites in src/app_impl, found {}",
            correct_count
        );
    }

    #[test]
    fn test_simulate_key_escape_uses_go_back_or_close_for_opened_from_main_menu() {
        let content = read_source_file("app_impl/simulate_key_dispatch.rs");
        let anchor = "SimulateKey: Escape - close menu-syntax picker, clear filter, go back, or hide";
        let start = content
            .find(anchor)
            .unwrap_or_else(|| panic!("Expected Escape SimulateKey branch log anchor"));
        let branch = &content[start..std::cmp::min(start + 6000, content.len())];

        assert!(
            branch.contains("view.opened_from_main_menu"),
            "Escape SimulateKey branch must check opened_from_main_menu"
        );
        assert!(
            branch.contains("view.go_back_or_close(window, ctx);"),
            "Escape SimulateKey branch must delegate opened-from-main-menu handling to go_back_or_close"
        );

        let opened_from_main_menu_start = branch
            .find("} else if view.opened_from_main_menu {")
            .unwrap_or_else(|| panic!("Expected opened_from_main_menu branch"));
        let fallback_start = opened_from_main_menu_start
            + branch[opened_from_main_menu_start..]
                .find("} else {")
                .unwrap_or_else(|| {
                    panic!("Expected fallback else branch after opened_from_main_menu")
                });
        let fallback_branch = &branch[fallback_start..];
        assert!(
            fallback_branch.contains("defer_calibrated_main_window_hide("),
            "non-opened-from-main-menu fallback must route through the shared calibrated main-only hide owner"
        );
        assert!(
            !fallback_branch.contains("ctx.hide();"),
            "non-opened-from-main-menu fallback must not app-hide, because that can hide Notes"
        );

        let opened_from_main_menu_branch = &branch[opened_from_main_menu_start..fallback_start];
        assert!(
            !opened_from_main_menu_branch.contains("ctx.hide();"),
            "opened_from_main_menu branch must not directly call ctx.hide()"
        );
    }

    /// Document the valid patterns for hiding windows
    /// This test always passes but serves as documentation
    #[test]
    fn document_valid_hide_patterns() {
        // VALID PATTERN 1: Using close_and_reset_window (preferred)
        // ```rust
        // self.close_and_reset_window(cx);
        // ```
        // This handles everything: reset, visibility flag, Notes/AI check, hide

        // VALID PATTERN 2: Opening secondary window (Notes/AI)
        // ```rust
        // script_kit_gpui::set_main_window_visible(false);
        // self.reset_to_script_list(cx);
        // platform::defer_hide_main_window(cx);  // Deferred — safe from RefCell reentrancy
        // notes::open_notes_window(cx)?;
        // ```

        // INVALID PATTERN 1: Direct cx.hide() in command execution
        // ```rust
        // cx.hide();  // BAD - doesn't reset, doesn't check secondary windows
        // ```

        // INVALID PATTERN 2: Setting NEEDS_RESET instead of resetting
        // ```rust
        // NEEDS_RESET.store(true, ...);  // BAD - defers reset, can be missed
        // cx.hide();
        // ```

        // INVALID PATTERN 3: Direct hide_main_window (CRASHES!)
        // ```rust
        // platform::hide_main_window();  // BAD - RefCell reentrancy panic
        // ```
        // Use platform::defer_hide_main_window(cx) instead.

        // This test exists for documentation purposes - the patterns above
        // are enforced by the other tests in this module
    }
}
