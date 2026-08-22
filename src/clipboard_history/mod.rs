//! Clipboard History Module
//!
//! Provides SQLite-backed clipboard history with background monitoring.
//!
//! ## Features
//! - Stores text and base64-encoded images
//! - Background polling every 500ms
//! - Time-based retention (default 30 days)
//! - Pin/unpin entries to prevent deletion
//! - Pagination support for lazy loading
//! - Time-based grouping (Today, Yesterday, This Week, etc.)
//! - OCR text storage for image entries
//!
//! ## Module Structure
//! - `types`: Core types (ContentType, TimeGroup, ClipboardEntry)
//! - `config`: Retention and text length configuration
//! - `cache`: LRU caching for images and entries
//! - `database`: SQLite operations (CRUD, migrations)
//! - `image`: Image encoding/decoding (PNG, RGBA)
//! - `monitor`: Background clipboard polling and maintenance
//! - `clipboard`: System clipboard operations

mod blob_store;
mod cache;
pub(crate) mod change_detection;
mod clipboard;
mod config;
mod database;
mod db_worker;
mod exclusions;
mod image;
mod macos_paste;
mod monitor;
pub mod ocr;
mod open_with;
pub mod paste_sequential;
mod quick_look;
mod rejection;
mod sediment;
pub use sediment::keep_entry_in_today;
mod temp_file;
mod types;
mod uri;

// Re-export public API
// These exports form the public API of the clipboard_history module.
// Some may appear unused in this crate but are used by external consumers.

// Types
#[allow(unused_imports)]
pub use types::{
    classify_timestamp, group_entries_by_time, root_clipboard_entry_is_eligible,
    root_clipboard_history_query_is_eligible, ClipboardEntry, ClipboardEntryMeta, ContentType,
    RootClipboardHistorySectionOptions, TimeGroup,
};

// Clipboard source exclusions

// DB Worker (new architecture - message passing instead of global mutex)
#[allow(unused_imports)]
pub use db_worker::{get_db_sender, start_db_worker, DbRequest};

// Config
#[allow(unused_imports)]
pub use config::{
    get_max_text_content_len, get_retention_days, set_max_text_content_len, set_retention_days,
};

// Cache
#[allow(unused_imports)]
pub use cache::{
    cache_image, get_cached_entries, get_cached_image, search_root_clipboard_history_meta_cached,
};
pub(crate) use cache::{
    discard_root_clipboard_history_refresh, finish_root_clipboard_history_refresh,
    read_root_clipboard_history_snapshot, root_clipboard_history_cache_is_fresh,
    try_begin_root_clipboard_history_refresh, RootClipboardHistorySnapshot,
};

// Database operations
#[allow(unused_imports)]
pub use database::{
    add_entry, clear_history, clear_unpinned_history, get_clipboard_history,
    get_clipboard_history_meta, get_clipboard_history_page, get_entry_by_id, get_entry_content,
    get_total_entry_count, pin_entry, remove_entry, search_root_clipboard_history_meta,
    search_root_clipboard_history_meta_direct, trim_oversize_text_entries, unpin_entry,
    update_ocr_text,
};

// Image operations
#[allow(unused_imports)]
pub use image::{
    content_to_png_bytes, decode_to_render_image, decode_to_rgba_bytes, encode_image_as_png,
};
#[allow(unused_imports)]
pub use quick_look::quick_look_entry;

// Temp file helpers
#[allow(unused_imports)]
pub use temp_file::save_entry_to_temp_file;

// Open With helpers (macOS Launch Services)
#[allow(unused_imports)]
pub use open_with::{get_apps_for_file, open_file_with_app, AppInfo};

// Secret rejection
pub use rejection::{configure_secret_rejection, SecretRejectionConfig};

// Monitor/Init
#[allow(unused_imports)]
pub use monitor::{
    init_clipboard_history, inject_clipboard_capture_fixture, stop_clipboard_monitoring,
    ClipboardCaptureFixtureOutcome, ClipboardCaptureFixturePayload,
};

// OCR worker
#[allow(unused_imports)]
pub use ocr::{enqueue_ocr, start_ocr_worker, stop_ocr_worker};

// Clipboard operations
pub use clipboard::copy_entry_to_clipboard;
#[allow(unused_imports)]
pub use clipboard::ClipboardWriteError;
#[allow(unused_imports)]
pub use uri::{entry_resource_uri, parse_entry_resource_uri};

// Sequential paste state machine
pub use paste_sequential::{
    advance_paste_sequence, commit_paste_sequence, enqueue_sequential_paste, EnqueuePasteError,
    PasteSequentialOutcome, PasteSequentialState,
};

// Test-only exports
#[cfg(test)]
#[allow(unused_imports)]
pub use types::classify_timestamp_with_now;
