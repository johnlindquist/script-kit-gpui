use crate::runtime_policy::{ExternalEffect, OwnedEvaluationPolicy};
use std::path::Path;

#[cfg(any(test, target_os = "windows"))]
fn escape_windows_cmd_open_target(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '^' | '&' | '|' | '<' | '>' | '(' | ')' | '%' | '!' | '"' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Open a file with the system default application
#[allow(dead_code)]
pub fn open_file(path: &str) -> Result<(), String> {
    use std::process::Command;

    crate::runtime_policy::check(ExternalEffect::OpenExternal).map_err(|e| e.to_string())?;

    // Brain activity journal: opening a file is a user decision the brain
    // should be able to answer questions about ("what was that png I just
    // opened?"). Single chokepoint — every open path (enter, mouse, action
    // dialog, automation) funnels through here. Off-thread, never blocks.
    crate::brain::record_activity("opened file", path);

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let escaped_path = escape_windows_cmd_open_target(path);
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(&escaped_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
        Ok(())
    }
}

/// Reveal a file in Finder (macOS) or file manager
#[allow(dead_code)]
pub fn reveal_in_finder(path: &str) -> Result<(), String> {
    use std::process::Command;

    crate::runtime_policy::check(ExternalEffect::OpenExternal).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", path])
            .spawn()
            .map_err(|e| format!("Failed to reveal file: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        // Try to get the parent directory and open it
        let parent = Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        Command::new("xdg-open")
            .arg(&parent)
            .spawn()
            .map_err(|e| format!("Failed to reveal file: {}", e))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", path])
            .spawn()
            .map_err(|e| format!("Failed to reveal file: {}", e))?;
        Ok(())
    }
}

pub(crate) fn terminal_working_directory(path: &str, is_dir: bool) -> String {
    if is_dir {
        return path.to_string();
    }

    Path::new(path)
        .parent()
        .and_then(|p| {
            let parent = p.to_string_lossy();
            if parent.is_empty() {
                None
            } else {
                Some(parent.to_string())
            }
        })
        .unwrap_or_else(|| ".".to_string())
}

fn move_destination_default_directory(path: &str, is_dir: bool) -> String {
    if is_dir {
        return Path::new(path)
            .parent()
            .and_then(|p| {
                let parent = p.to_string_lossy();
                if parent.is_empty() {
                    None
                } else {
                    Some(parent.to_string())
                }
            })
            .unwrap_or_else(|| ".".to_string());
    }

    terminal_working_directory(path, false)
}

/// Move a path to Trash.
pub fn move_to_trash(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let escaped_path = crate::utils::escape_applescript_string(path);
        let script = format!(
            r#"tell application "Finder"
                delete POSIX file "{}"
            end tell"#,
            escaped_path
        );

        crate::platform::run_osascript(&script, "file_search_move_to_trash")
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Move to Trash is currently only supported on macOS".to_string())
    }
}

/// Preview a file using Quick Look (macOS)
#[allow(dead_code)]
pub fn quick_look(path: &str) -> Result<(), String> {
    use std::process::Command;

    crate::runtime_policy::check(ExternalEffect::OpenExternal).map_err(|e| e.to_string())?;

    if !Path::new(path).exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("qlmanage")
            .arg("-p")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to preview file: {}", e))?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Quick Look is macOS-only; fall back to opening the file
        open_file(path)
    }
}

/// Show the "Open With" dialog for a file (macOS)
#[allow(dead_code)]
pub fn open_with(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Use AppleScript to trigger the "Open With" menu
        let script = format!(
            r#"tell application "Finder"
                activate
                set theFile to POSIX file "{}"
                open information window of theFile
            end tell"#,
            crate::utils::escape_applescript_string(path)
        );
        crate::platform::run_osascript(&script, "file_search_open_with")
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Open With is only supported on macOS".to_string())
    }
}

/// Show the Get Info window for a file in Finder (macOS)
#[allow(dead_code)]
pub fn show_info(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Use AppleScript to open the Get Info window
        let script = format!(
            r#"tell application "Finder"
                activate
                set theFile to POSIX file "{}"
                open information window of theFile
            end tell"#,
            crate::utils::escape_applescript_string(path)
        );
        crate::platform::run_osascript(&script, "file_search_show_info")
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Show Info is only supported on macOS".to_string())
    }
}

/// Run an AppleScript and return the text result, or `None` if the user cancelled.
#[cfg(target_os = "macos")]
fn run_osascript_capture(script: &str) -> Result<Option<String>, String> {
    match crate::platform::run_osascript(script, "file_search_dialog") {
        Ok(stdout) => Ok(Some(stdout)),
        Err(error) => {
            let message = error.to_string();
            if message.contains("User canceled") || message.contains("(-128)") {
                Ok(None)
            } else {
                Err(message)
            }
        }
    }
}

/// Show a native rename dialog and return the user-entered new name, or `None` if cancelled.
pub fn prompt_rename_target_name(path: &str) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let current_name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Selected item has no filename".to_string())?;

        let escaped_default = crate::utils::escape_applescript_string(current_name);
        let script = format!(
            r#"tell application "System Events"
                activate
                display dialog "Rename selected item" default answer "{}" buttons {{"Cancel", "Rename"}} default button "Rename"
                return text returned of result
            end tell"#,
            escaped_default
        );
        run_osascript_capture(&script)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Rename is currently only supported on macOS".to_string())
    }
}

/// Rename a file or directory in-place and return the new full path.
pub fn rename_path(path: &str, new_name: &str) -> Result<String, String> {
    rename_path_with_policy(path, new_name, crate::runtime_policy::owned_evaluation())
}

fn rename_path_with_policy(
    path: &str,
    new_name: &str,
    policy: Option<&OwnedEvaluationPolicy>,
) -> Result<String, String> {
    let trimmed_name = new_name.trim();
    if trimmed_name.is_empty() {
        return Err("New name cannot be empty".to_string());
    }
    if trimmed_name.contains('/') {
        return Err("New name cannot contain '/'".to_string());
    }

    let current_path = Path::new(path);
    let parent = current_path
        .parent()
        .ok_or_else(|| "Cannot rename a root path".to_string())?;
    let target = parent.join(trimmed_name);
    if let Some(policy) = policy {
        validate_owned_tree(policy, current_path, &target)?;
    }

    if target == current_path {
        return Ok(path.to_string());
    }

    std::fs::rename(current_path, &target).map_err(|e| format!("Failed to rename item: {}", e))?;

    Ok(target.to_string_lossy().to_string())
}

/// Show a native move-destination dialog and return the user-entered directory, or `None` if cancelled.
pub fn prompt_move_destination_dir(path: &str, is_dir: bool) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        let default_dir = move_destination_default_directory(path, is_dir);
        let escaped_default = crate::utils::escape_applescript_string(&default_dir);
        let script = format!(
            r#"tell application "System Events"
                activate
                display dialog "Move selected item to folder" default answer "{}" buttons {{"Cancel", "Move"}} default button "Move"
                return text returned of result
            end tell"#,
            escaped_default
        );
        run_osascript_capture(&script)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        let _ = is_dir;
        Err("Move is currently only supported on macOS".to_string())
    }
}

/// Move a file or directory to a new parent folder and return the new full path.
pub fn move_path(path: &str, destination_dir: &str) -> Result<String, String> {
    move_path_with_policy(
        path,
        destination_dir,
        crate::runtime_policy::owned_evaluation(),
    )
}

fn move_path_with_policy(
    path: &str,
    destination_dir: &str,
    policy: Option<&OwnedEvaluationPolicy>,
) -> Result<String, String> {
    let current_path = Path::new(path);
    let filename = current_path
        .file_name()
        .ok_or_else(|| "Selected item has no filename".to_string())?;

    let expanded_destination = crate::file_search::expand_path(destination_dir)
        .unwrap_or_else(|| destination_dir.to_string());
    let destination_path = Path::new(&expanded_destination);
    require_owned_paths(policy, &[current_path, destination_path])?;

    if !destination_path.is_dir() {
        return Err(format!(
            "Destination is not a folder: {}",
            destination_path.display()
        ));
    }

    let target = destination_path.join(filename);
    if let Some(policy) = policy {
        validate_owned_tree(policy, current_path, &target)?;
    }
    if target == current_path {
        return Ok(path.to_string());
    }

    std::fs::rename(current_path, &target).map_err(|e| format!("Failed to move item: {}", e))?;

    Ok(target.to_string_lossy().to_string())
}

fn require_owned_paths(
    policy: Option<&OwnedEvaluationPolicy>,
    paths: &[&Path],
) -> Result<(), String> {
    if let Some(policy) = policy {
        for path in paths {
            policy.require_owned_path(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// Preflight the entire tree before the first mutation. Never follow symlinks,
// including links below a directory being renamed or moved to a new parent.
fn validate_owned_tree(
    policy: &OwnedEvaluationPolicy,
    src: &Path,
    dst: &Path,
) -> Result<(), String> {
    require_owned_paths(Some(policy), &[src, dst])?;
    let metadata = std::fs::symlink_metadata(src)
        .map_err(|e| format!("Failed to inspect source '{}': {}", src.display(), e))?;
    if metadata.is_dir() {
        for entry in
            std::fs::read_dir(src).map_err(|e| format!("Failed to read source folder: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read folder entry: {}", e))?;
            validate_owned_tree(policy, &entry.path(), &dst.join(entry.file_name()))?;
        }
    } else if !metadata.is_file() {
        return Err(format!("Unsupported owned file type: {}", src.display()));
    }
    Ok(())
}

fn duplicate_target_path(
    path: &Path,
    policy: Option<&OwnedEvaluationPolicy>,
) -> Result<std::path::PathBuf, String> {
    require_owned_paths(policy, &[path])?;
    let parent = path
        .parent()
        .ok_or_else(|| "Cannot duplicate a root path".to_string())?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Selected item has no filename".to_string())?;
    let is_dir = path.is_dir();
    let (base, ext) = if is_dir {
        (file_name.to_string(), None)
    } else {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name)
            .to_string();
        (stem, ext)
    };
    for index in 1..=999 {
        let candidate_name = match (&ext, index) {
            (Some(ext), 1) => format!("{} copy.{}", base, ext),
            (Some(ext), n) => format!("{} copy {}.{}", base, n, ext),
            (None, 1) => format!("{} copy", base),
            (None, n) => format!("{} copy {}", base, n),
        };
        let candidate = parent.join(candidate_name);
        require_owned_paths(policy, &[&candidate])?;
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not find an available duplicate name".to_string())
}

fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    policy: Option<&OwnedEvaluationPolicy>,
) -> Result<(), String> {
    require_owned_paths(policy, &[src, dst])?;
    std::fs::create_dir(dst).map_err(|e| format!("Failed to create duplicate folder: {}", e))?;
    for entry in
        std::fs::read_dir(src).map_err(|e| format!("Failed to read source folder: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read folder entry: {}", e))?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        require_owned_paths(policy, &[&entry_path, &target_path])?;
        let metadata = std::fs::symlink_metadata(&entry_path)
            .map_err(|e| format!("Failed to inspect folder entry: {}", e))?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            copy_symlink(&entry_path, &target_path)?;
        } else if file_type.is_dir() {
            copy_dir_recursive(&entry_path, &target_path, policy)?;
        } else {
            std::fs::copy(&entry_path, &target_path)
                .map_err(|e| format!("Failed to copy '{}': {}", entry_path.display(), e))?;
        }
    }
    Ok(())
}

#[cfg(target_family = "unix")]
fn copy_symlink(src: &Path, dst: &Path) -> Result<(), String> {
    let target = std::fs::read_link(src)
        .map_err(|e| format!("Failed to read symlink '{}': {}", src.display(), e))?;
    std::os::unix::fs::symlink(&target, dst)
        .map_err(|e| format!("Failed to duplicate symlink '{}': {}", src.display(), e))
}

#[cfg(not(target_family = "unix"))]
fn copy_symlink(src: &Path, _dst: &Path) -> Result<(), String> {
    Err(format!(
        "Duplicating symlinks is not currently supported on this platform: {}",
        src.display()
    ))
}

/// Duplicate a file or directory and return the new path.
pub fn duplicate_path(path: &str) -> Result<String, String> {
    duplicate_path_with_policy(path, crate::runtime_policy::owned_evaluation())
}

fn duplicate_path_with_policy(
    path: &str,
    policy: Option<&OwnedEvaluationPolicy>,
) -> Result<String, String> {
    let src = Path::new(path);
    require_owned_paths(policy, &[src])?;
    if !src.exists() {
        return Err(format!("Path does not exist: {}", src.display()));
    }
    let target = duplicate_target_path(src, policy)?;
    if let Some(policy) = policy {
        validate_owned_tree(policy, src, &target)?;
    }
    if src.is_dir() {
        copy_dir_recursive(src, &target, policy)?;
    } else {
        std::fs::copy(src, &target).map_err(|e| format!("Failed to duplicate item: {}", e))?;
    }
    Ok(target.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        duplicate_path, duplicate_path_with_policy, escape_windows_cmd_open_target,
        move_destination_default_directory, move_path_with_policy, quick_look,
        rename_path_with_policy, terminal_working_directory, OwnedEvaluationPolicy,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn owned_fixture() -> (tempfile::TempDir, OwnedEvaluationPolicy) {
        let temp = tempdir().expect("tempdir");
        let root = temp.path().join("owned");
        fs::create_dir(&root).expect("owned root");
        let policy = OwnedEvaluationPolicy::new(
            &root.canonicalize().expect("canonical root"),
            "file-actions".to_string(),
            "test-generation".to_string(),
        )
        .expect("owned policy");
        (temp, policy)
    }

    #[test]
    fn owned_file_actions_preserve_contents_and_duplicate_numbering() {
        let (_temp, policy) = owned_fixture();
        let source = policy.root().join("note.txt");
        let destination = policy.root().join("destination");
        fs::write(&source, "owned contents").unwrap();
        fs::create_dir(&destination).unwrap();
        let duplicate =
            duplicate_path_with_policy(source.to_str().unwrap(), Some(&policy)).unwrap();
        assert_eq!(
            PathBuf::from(&duplicate),
            policy.root().join("note copy.txt")
        );
        let second = duplicate_path_with_policy(source.to_str().unwrap(), Some(&policy)).unwrap();
        assert_eq!(PathBuf::from(second), policy.root().join("note copy 2.txt"));
        let renamed = rename_path_with_policy(&duplicate, "renamed.txt", Some(&policy)).unwrap();
        assert!(!PathBuf::from(duplicate).exists());
        let moved =
            move_path_with_policy(&renamed, destination.to_str().unwrap(), Some(&policy)).unwrap();
        assert!(!PathBuf::from(renamed).exists());
        assert_eq!(fs::read_to_string(&source).unwrap(), "owned contents");
        assert_eq!(fs::read_to_string(&moved).unwrap(), "owned contents");
        assert_eq!(
            rename_path_with_policy(&moved, "renamed.txt", Some(&policy)).unwrap(),
            moved
        );
        assert_eq!(
            move_path_with_policy(&moved, destination.to_str().unwrap(), Some(&policy)).unwrap(),
            moved
        );
    }

    #[test]
    fn owned_directory_actions_preserve_nested_contents() {
        let (_temp, policy) = owned_fixture();
        let source = policy.root().join("folder");
        let destination = policy.root().join("destination");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("nested/note.txt"), "nested contents").unwrap();
        let duplicate =
            duplicate_path_with_policy(source.to_str().unwrap(), Some(&policy)).unwrap();
        let renamed = rename_path_with_policy(&duplicate, "renamed", Some(&policy)).unwrap();
        let moved =
            move_path_with_policy(&renamed, destination.to_str().unwrap(), Some(&policy)).unwrap();
        assert_eq!(
            fs::read_to_string(PathBuf::from(moved).join("nested/note.txt")).unwrap(),
            "nested contents"
        );
        assert_eq!(
            fs::read_to_string(source.join("nested/note.txt")).unwrap(),
            "nested contents"
        );
    }

    #[test]
    fn owned_actions_reject_outside_sources_before_noop_or_existence_checks() {
        let (temp, policy) = owned_fixture();
        let outside = temp.path().canonicalize().unwrap().join("operator.txt");
        fs::write(&outside, "operator contents").unwrap();
        for source in [&outside, &outside.with_file_name("missing.txt")] {
            let path = source.to_str().unwrap();
            assert_eq!(
                duplicate_path_with_policy(path, Some(&policy)).unwrap_err(),
                "evaluation_path_outside_owner"
            );
            assert_eq!(
                rename_path_with_policy(
                    path,
                    source.file_name().unwrap().to_str().unwrap(),
                    Some(&policy),
                )
                .unwrap_err(),
                "evaluation_path_outside_owner"
            );
            assert_eq!(
                move_path_with_policy(
                    path,
                    source.parent().unwrap().to_str().unwrap(),
                    Some(&policy)
                )
                .unwrap_err(),
                "evaluation_path_outside_owner"
            );
        }
        assert_eq!(fs::read_to_string(&outside).unwrap(), "operator contents");
        assert!(!outside.with_file_name("operator copy.txt").exists());
    }

    #[test]
    fn owned_actions_reject_outside_or_parent_destinations_without_mutation() {
        let (temp, policy) = owned_fixture();
        let source = policy.root().join("note.txt");
        fs::write(&source, "owned contents").unwrap();
        let outside = temp
            .path()
            .canonicalize()
            .unwrap()
            .join("missing-destination");
        assert_eq!(
            move_path_with_policy(
                source.to_str().unwrap(),
                outside.to_str().unwrap(),
                Some(&policy)
            )
            .unwrap_err(),
            "evaluation_path_outside_owner"
        );
        let parent_escape = policy.root().join("..");
        assert_eq!(
            move_path_with_policy(
                source.to_str().unwrap(),
                parent_escape.to_str().unwrap(),
                Some(&policy),
            )
            .unwrap_err(),
            "evaluation_path_not_normalized"
        );
        assert_eq!(
            rename_path_with_policy(source.to_str().unwrap(), "..", Some(&policy)).unwrap_err(),
            "evaluation_path_not_normalized"
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "owned contents");
        assert!(!outside.exists());
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn owned_actions_reject_symlink_sources_and_ancestors() {
        let (temp, policy) = owned_fixture();
        let outside = temp.path().canonicalize().unwrap().join("operator");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("note.txt"), "operator contents").unwrap();
        let directory_link = policy.root().join("directory-link");
        let file_link = policy.root().join("file-link");
        let dangling_link = policy.root().join("dangling-link");
        std::os::unix::fs::symlink(&outside, &directory_link).unwrap();
        std::os::unix::fs::symlink(outside.join("note.txt"), &file_link).unwrap();
        std::os::unix::fs::symlink(outside.join("missing"), &dangling_link).unwrap();
        for source in [
            directory_link.join("note.txt"),
            directory_link,
            file_link,
            dangling_link,
        ] {
            let path = source.to_str().unwrap();
            assert_eq!(
                duplicate_path_with_policy(path, Some(&policy)).unwrap_err(),
                "evaluation_path_symlink"
            );
            assert_eq!(
                rename_path_with_policy(path, "renamed", Some(&policy)).unwrap_err(),
                "evaluation_path_symlink"
            );
            assert_eq!(
                move_path_with_policy(path, policy.root().to_str().unwrap(), Some(&policy))
                    .unwrap_err(),
                "evaluation_path_symlink"
            );
        }
        assert_eq!(
            fs::read_to_string(outside.join("note.txt")).unwrap(),
            "operator contents"
        );
        assert!(!policy.root().join("renamed").exists());
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn owned_tree_preflight_rejects_nested_symlink_before_creating_or_moving_anything() {
        let (temp, policy) = owned_fixture();
        let outside = temp.path().canonicalize().unwrap().join("operator.txt");
        fs::write(&outside, "operator contents").unwrap();
        let source = policy.root().join("folder");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("safe.txt"), "owned contents").unwrap();
        std::os::unix::fs::symlink(&outside, source.join("nested/link")).unwrap();
        let destination = policy.root().join("destination");
        fs::create_dir(&destination).unwrap();
        let path = source.to_str().unwrap();
        assert_eq!(
            duplicate_path_with_policy(path, Some(&policy)).unwrap_err(),
            "evaluation_path_symlink"
        );
        assert_eq!(
            rename_path_with_policy(path, "renamed", Some(&policy)).unwrap_err(),
            "evaluation_path_symlink"
        );
        assert_eq!(
            move_path_with_policy(path, destination.to_str().unwrap(), Some(&policy)).unwrap_err(),
            "evaluation_path_symlink"
        );
        assert!(!policy.root().join("folder copy").exists());
        assert!(!policy.root().join("renamed").exists());
        assert!(!destination.join("folder").exists());
        assert_eq!(
            fs::read_to_string(source.join("safe.txt")).unwrap(),
            "owned contents"
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), "operator contents");
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn owned_actions_reject_symlink_destinations_including_dangling_duplicate_names() {
        let (temp, policy) = owned_fixture();
        let outside = temp.path().canonicalize().unwrap().join("operator");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("note.txt"), "operator contents").unwrap();
        let source = policy.root().join("note.txt");
        fs::write(&source, "owned contents").unwrap();
        let destination = policy.root().join("destination");
        fs::create_dir(&destination).unwrap();
        std::os::unix::fs::symlink(
            outside.join("missing.txt"),
            policy.root().join("note copy.txt"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.join("note.txt"), policy.root().join("renamed.txt"))
            .unwrap();
        std::os::unix::fs::symlink(outside.join("note.txt"), destination.join("note.txt")).unwrap();
        let directory_link = policy.root().join("directory-link");
        std::os::unix::fs::symlink(&outside, &directory_link).unwrap();
        let path = source.to_str().unwrap();
        assert_eq!(
            duplicate_path_with_policy(path, Some(&policy)).unwrap_err(),
            "evaluation_path_symlink"
        );
        assert_eq!(
            rename_path_with_policy(path, "renamed.txt", Some(&policy)).unwrap_err(),
            "evaluation_path_symlink"
        );
        for directory in [&destination, &directory_link] {
            assert_eq!(
                move_path_with_policy(path, directory.to_str().unwrap(), Some(&policy))
                    .unwrap_err(),
                "evaluation_path_symlink"
            );
        }
        assert!(!outside.join("missing.txt").exists());
        assert_eq!(
            fs::read_to_string(outside.join("note.txt")).unwrap(),
            "operator contents"
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "owned contents");
    }

    #[test]
    fn quick_look_missing_path_returns_error_without_panic() {
        let missing = format!(
            "/tmp/script-kit-gpui-missing-quick-look-{}",
            std::process::id()
        );

        let error = quick_look(&missing).expect_err("missing path should return an error");
        assert!(error.contains("Path does not exist"));
    }

    #[test]
    fn test_terminal_working_directory_returns_parent_for_file_paths() {
        let resolved = terminal_working_directory("/tmp/a/b/file.txt", false);
        assert_eq!(resolved, "/tmp/a/b");
    }

    #[test]
    fn test_move_destination_default_directory_returns_parent_for_directories() {
        let resolved = move_destination_default_directory("/tmp/a/b/folder", true);
        assert_eq!(resolved, "/tmp/a/b");
    }

    #[test]
    fn test_escape_windows_cmd_open_target_escapes_shell_metacharacters() {
        let escaped = escape_windows_cmd_open_target(r#"C:\tmp\a&b|c<d>e(f)g^h%i!j"k.txt"#);
        assert_eq!(escaped, r#"C:\tmp\a^&b^|c^<d^>e^(f^)g^^h^%i^!j^"k.txt"#);
    }

    #[test]
    fn test_duplicate_path_copies_regular_file() {
        let dir = tempdir().expect("tempdir should be created");
        let source = dir.path().join("note.txt");
        std::fs::write(&source, "hello").expect("source file should be written");

        let duplicated = duplicate_path(source.to_str().expect("utf8 path")).expect("duplicate");
        let duplicated_path = std::path::PathBuf::from(duplicated);

        assert!(duplicated_path.exists());
        assert_eq!(
            std::fs::read_to_string(duplicated_path).expect("duplicate file should be readable"),
            "hello"
        );
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn test_duplicate_path_preserves_directory_symlinks() {
        let dir = tempdir().expect("tempdir should be created");
        let source = dir.path().join("folder");
        let nested = source.join("nested");
        std::fs::create_dir_all(&nested).expect("source directory should be created");

        let target_dir = dir.path().join("linked-target");
        std::fs::create_dir(&target_dir).expect("symlink target dir should be created");
        let symlink_path = source.join("link");
        std::os::unix::fs::symlink(&target_dir, &symlink_path)
            .expect("directory symlink should be created");

        let duplicated = duplicate_path(source.to_str().expect("utf8 path")).expect("duplicate");
        let duplicated_link = std::path::PathBuf::from(duplicated).join("link");
        let metadata =
            std::fs::symlink_metadata(&duplicated_link).expect("duplicate symlink metadata");

        assert!(metadata.file_type().is_symlink());
        assert_eq!(
            std::fs::read_link(&duplicated_link).expect("duplicate symlink target"),
            target_dir
        );
    }
}
