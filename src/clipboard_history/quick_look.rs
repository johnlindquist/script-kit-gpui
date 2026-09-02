#![allow(dead_code)]
//! Quick Look preview helpers for clipboard history entries.
//!
//! Uses macOS `qlmanage -p` to preview text/images. For non-macOS targets,
//! falls back to opening the generated file with the default app.

use std::fs;
use std::path::{Path, PathBuf};

use super::{content_to_png_bytes, get_entry_content, ClipboardEntryMeta, ContentType};

/// Preview a clipboard history entry with Quick Look (macOS) or open fallback.
pub fn quick_look_entry(entry: &ClipboardEntryMeta) -> Result<(), String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::OpenExternal)
        .map_err(|error| error.to_string())?;
    let content = get_entry_content(&entry.id)
        .ok_or_else(|| "Failed to load clipboard entry content".to_string())?;

    let preview_path = match entry.content_type {
        ContentType::Text | ContentType::Link | ContentType::File | ContentType::Color => {
            write_text_preview(&entry.id, &content)?
        }
        ContentType::Image => resolve_image_preview_path(&entry.id, &content)?,
    };

    quick_look_path(&preview_path)
}

fn quick_look_path(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        Command::new("qlmanage")
            .args(["-p", path_str.as_ref()])
            .spawn()
            .map_err(|e| format!("Failed to preview file: {}", e))?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        crate::file_search::open_file(path_str.as_ref())
    }
}

fn quicklook_dir() -> Result<PathBuf, String> {
    let kit_dir = PathBuf::from(shellexpand::tilde("~/.scriptkit").as_ref());
    let quicklook_dir = kit_dir.join("clipboard").join("quicklook");

    if !quicklook_dir.exists() {
        fs::create_dir_all(&quicklook_dir)
            .map_err(|e| format!("Failed to create Quick Look directory: {}", e))?;
    }

    Ok(quicklook_dir)
}

fn sanitize_id(entry_id: &str) -> String {
    entry_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn write_text_preview(entry_id: &str, content: &str) -> Result<PathBuf, String> {
    let dir = quicklook_dir()?;
    let filename = format!("{}.txt", sanitize_id(entry_id));
    let path = dir.join(filename);

    fs::write(&path, content.as_bytes())
        .map_err(|e| format!("Failed to write Quick Look text: {}", e))?;

    Ok(path)
}

fn resolve_image_preview_path(entry_id: &str, content: &str) -> Result<PathBuf, String> {
    if let Some(blob_path) = blob_path_from_content(content) {
        if blob_path.exists() {
            return Ok(blob_path);
        }
    }

    let png_bytes = content_to_png_bytes(content)
        .ok_or_else(|| "Failed to decode clipboard image".to_string())?;

    let dir = quicklook_dir()?;
    let filename = format!("{}.png", sanitize_id(entry_id));
    let path = dir.join(filename);

    fs::write(&path, png_bytes).map_err(|e| format!("Failed to write Quick Look image: {}", e))?;

    Ok(path)
}

fn blob_path_from_content(content: &str) -> Option<PathBuf> {
    let hash = content.strip_prefix("blob:")?;
    let blob_dir = super::blob_store::get_blob_dir().ok()?;
    Some(blob_dir.join(format!("{}.png", hash)))
}
