//! Compatibility facade for the app-independent private persistence domain.
//!
//! Run focused storage behavior without GPUI, Metal, Whisper, or ONNX:
//! `./scripts/agentic/agent-cargo.sh test -p sk-storage <reviewed-filter>`.

pub use sk_storage::write_atomic;
pub(crate) use sk_storage::{
    append_private_jsonl_record, append_private_observability_record, ensure_private_directory,
    inspect_private_file, read_private_file, write_private_atomic, write_private_unique_named_file,
};
