//! Repo-boundary lint: shipped `kit-init/` guidance cannot silently call
//! SDK APIs that `kit://sdk-reference` still marks `Unsupported`. The
//! canonical list of unsupported APIs lives in
//! [`script_kit_gpui::mcp_resources::sdk_reference_entries_for_ui`];
//! this guard reads from there so it stays in lockstep as entries flip
//! between Supported / Unsupported without any manual test edits.
//!
//! History: the guard originally targeted `notify(...)` while `notify` was
//! flagged Unsupported. Once `notify` was implemented on top of
//! `notify-rust`, it was dropped from the Unsupported list and the guard
//! automatically retargeted at whatever remained (currently `menu`).
//!
//! Failure mode: any runnable line (code fence, `.ts`, `.tsx`, `.js`,
//! `.jsx`, or `.md`) in `kit-init/` that calls `name(...)` for any
//! currently-Unsupported SDK entry fails with a file + line number the
//! author can jump to. API-reference prose that merely names the function
//! inside a markdown table cell is tolerated — the test only flags the
//! presence of `name(` which implies a call-site.

use std::fs;
use std::path::{Path, PathBuf};

use script_kit_gpui::mcp_resources::{sdk_reference_entries_for_ui, SdkSupport};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn kit_init_root() -> PathBuf {
    repo_root().join("kit-init")
}

/// Pull the current Unsupported-in-GPUI SDK names from the live reference.
/// This is the single source of truth — a name is only fenced off in
/// kit-init guidance for as long as kit://sdk-reference also flags it.
fn currently_unsupported_sdk_names() -> Vec<String> {
    sdk_reference_entries_for_ui()
        .iter()
        .filter(|entry| entry.support == SdkSupport::Unsupported)
        .map(|entry| entry.name.clone())
        .collect()
}

fn collect_checked_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(root).unwrap_or_else(|e| panic!("read kit-init dir {}: {e}", root.display()));
    for entry in entries {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_checked_files(&path, out);
            continue;
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if matches!(ext, "md" | "ts" | "tsx" | "js" | "jsx") {
            out.push(path);
        }
    }
}

/// Return the first Unsupported SDK name whose `name(` call-shape appears
/// on `line`, or `None`.
fn contains_unsupported_call<'a>(line: &str, names: &'a [String]) -> Option<&'a str> {
    names
        .iter()
        .find(|name| {
            let needle = format!("{name}(");
            line.match_indices(&needle).any(|(offset, _)| {
                line[..offset].chars().next_back().is_none_or(|previous| {
                    previous != '.'
                        && previous != '_'
                        && previous != '$'
                        && !previous.is_ascii_alphanumeric()
                })
            })
        })
        .map(|s| s.as_str())
}

#[test]
fn unsupported_global_detection_does_not_confuse_array_methods_with_sdk_calls() {
    let names = vec!["find".to_string(), "widget".to_string()];

    assert_eq!(
        contains_unsupported_call("await find('file')", &names),
        Some("find")
    );
    assert_eq!(
        contains_unsupported_call("await widget('<div />')", &names),
        Some("widget")
    );
    assert_eq!(
        contains_unsupported_call("items.find(item => item.id)", &names),
        None
    );
    assert_eq!(
        contains_unsupported_call("items?.find(item => item.id)", &names),
        None
    );
    assert_eq!(
        contains_unsupported_call("helpers.widget('<div />')", &names),
        None
    );
    assert_eq!(contains_unsupported_call("refind('text')", &names), None);
}

/// Markdown-table prose allowlist. An API-reference row of the shape
/// `| `name(args)` | description | ... |` is allowed to mention `name(`
/// without being treated as a call-site, because the backticks +
/// surrounding pipes make the intent unambiguous.
fn is_markdown_table_row(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('|') && line.contains('`')
}

#[test]
fn kit_init_shipped_examples_do_not_call_unsupported_sdk_apis() {
    let names = currently_unsupported_sdk_names();
    if names.is_empty() {
        // Nothing to guard against — every reference entry is now Supported.
        // The balance assertion in mcp_resources keeps at least one
        // Unsupported entry in place, but if that ever changes, this test
        // becomes a no-op by design rather than a false positive.
        return;
    }

    let mut files = Vec::new();
    collect_checked_files(&kit_init_root(), &mut files);
    assert!(
        !files.is_empty(),
        "kit-init scan found no md/ts/tsx/js/jsx files — scope drifted or repo layout changed"
    );

    let mut violations = Vec::new();
    for path in &files {
        let body = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                violations.push(format!("{}: read error: {e}", path.display()));
                continue;
            }
        };
        for (line_number, line) in body.lines().enumerate() {
            let Some(api) = contains_unsupported_call(line, &names) else {
                continue;
            };
            if is_markdown_table_row(line) {
                continue;
            }
            violations.push(format!(
                "{}:{} calls unsupported `{}(`: {}",
                path.display(),
                line_number + 1,
                api,
                line.trim()
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "kit-init/ shipped guidance must not call APIs that kit://sdk-reference still marks Unsupported. Current Unsupported names: {names:?}. Violations:\n{}",
        violations.join("\n")
    );
}
