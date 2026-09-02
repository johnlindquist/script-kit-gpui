//! Pure decisions shared by the production launcher filter pipeline and library stories.

/// Sigils switch the launcher from fuzzy rows to a structurally distinct list.
const LIST_STRUCTURE_SIGILS: &[char] = &['@', '/', ';', ':', '!', '~', '|', '>', '+', '.'];

#[derive(Debug, PartialEq, Eq)]
enum FilterListFamily {
    Default,
    Sigil(char),
    QualifierOwned,
    Plain,
}

fn filter_list_family(text: &str) -> FilterListFamily {
    if text.is_empty() {
        return FilterListFamily::Default;
    }
    if let Some(head) = text.trim_start().chars().next() {
        if LIST_STRUCTURE_SIGILS.contains(&head) {
            return FilterListFamily::Sigil(head);
        }
    }
    if crate::menu_syntax::active_filter_head_owns_main_list(text) {
        FilterListFamily::QualifierOwned
    } else {
        FilterListFamily::Plain
    }
}

/// Crossing list families must compute synchronously rather than showing a stale
/// list during the production filter coalescer's defer.
pub(crate) fn filter_change_flips_list_structure(old: &str, new: &str) -> bool {
    filter_list_family(old) != filter_list_family(new)
}

pub(crate) fn menu_syntax_filter_only_escape_should_clear(
    raw: &str,
    mode: &crate::menu_syntax::MenuSyntaxMode,
) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }

    if let Some(rest) = trimmed.strip_prefix(':') {
        return !rest.chars().any(char::is_whitespace);
    }

    if crate::menu_syntax::active_filter_head_owns_main_list(trimmed) {
        return true;
    }

    trimmed.split_whitespace().count() == 1
        && mode
            .advanced_query_for(trimmed)
            .is_some_and(|query| query.free_text.trim().is_empty() && query.has_predicates())
}
