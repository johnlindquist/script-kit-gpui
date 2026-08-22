//! Compatibility facade for app-independent ASCII and fuzzy search helpers.

pub(crate) use sk_protocol::ascii_search::{
    contains_ignore_ascii_case, find_ignore_ascii_case, fuzzy_match_with_indices_ascii,
    is_ascii_pair, is_fuzzy_match, is_word_boundary_match, MIN_FUZZY_QUERY_LEN,
};
