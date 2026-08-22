//! Compatibility path for the shared GPUI-free natural-language search owner.
//!
//! Clipboard, Dictation, conversations, and launcher projections keep their
//! original application imports while the matcher and its behavior tests live
//! in the independently testable `sk-protocol` domain.

pub(crate) use sk_protocol::sentence_search::*;
