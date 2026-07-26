//! Source-level contract for keeping `StateResult` construction in lockstep.
//!
//! `Message::state_result(...)` is a narrow constructor wrapper around the
//! `Message::StateResult { ... }` variant, but its positional parameter list
//! is fragile because several adjacent fields share the same Rust type
//! (`prompt_id`/`placeholder`/`selected_value`/`screenshot_identity`,
//! `choice_count`/`visible_choice_count`, `is_focused`/`window_visible`).
//!
//! WHY this invariant exists: a caller passing 35 positional arguments cannot
//! be type-checked against transpositions of same-typed neighbors, so the
//! constructor signature and its forwarding literal must mirror the variant's
//! declaration order exactly — the variant declaration is the single order
//! authority.
//!
//! History note (pruning rule, AGENTS.md Source Audit Test Policy): the
//! original version of this lock hardcoded an EXPECTED field list and rotted
//! through 11 maintenance patches, ending several features stale (missing
//! `filter_input_diagnostics`, `active_list_scroll`, `day_page_state`,
//! `flow_ux`) so it failed on every compile of this rarely-built target
//! rather than on real desynchronization. It was rewritten structurally
//! (variant as authority) after the three-legitimate-refactor threshold
//! fired. Same two reader sites; no new source-audit surface.

const QUERY_OPS_VARIANTS: &str = include_str!("../src/protocol/message/variants/query_ops.rs");
const QUERY_OPS_CONSTRUCTORS: &str =
    include_str!("../src/protocol/message/constructors/query_ops.rs");

fn source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end after {start_pat}: {end_pat}"));
    &source[start..start + end_rel]
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Field/parameter names from a `name: Type,` declaration block, in order.
fn declared_field_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with("///") || trimmed.starts_with("//") {
                return None;
            }
            if !trimmed.ends_with(',') && !trimmed.ends_with('<') && !trimmed.ends_with(':') {
                return None;
            }
            let (name, _) = trimmed.split_once(':')?;
            let name = name.trim();
            if name == "crate" {
                return None;
            }
            is_ident(name).then(|| name.to_string())
        })
        .collect()
}

/// Shorthand-forwarded names (`name,`) from a struct literal body, in order.
fn forwarded_field_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let name = line.trim().strip_suffix(',')?;
            is_ident(name).then(|| name.to_string())
        })
        .collect()
}

#[test]
fn state_result_constructor_signature_and_forwarding_match_variant_field_order() {
    let variant = source_between(
        QUERY_OPS_VARIANTS,
        "#[serde(rename = \"stateResult\")]",
        "\n    // ============================================================\n    // ELEMENT QUERY",
    );
    let constructor = source_between(
        QUERY_OPS_CONSTRUCTORS,
        "pub fn state_result(",
        "\n    // ============================================================\n    // Constructor methods for element query",
    );
    let signature = source_between(constructor, "pub fn state_result(", ") -> Self");
    let literal = source_between(constructor, "Message::StateResult {", "\n        }\n    }");

    // The variant declaration is the one order authority.
    let authority = declared_field_names(variant);

    // Guard the extractor itself: if parsing rots, fail loudly rather than
    // vacuously comparing empty lists.
    assert!(
        authority.len() >= 30,
        "StateResult variant parser extracted only {} fields — the extraction \
         patterns no longer match the source layout",
        authority.len()
    );
    assert_eq!(
        authority.first().map(String::as_str),
        Some("request_id"),
        "StateResult variant parsing must start at request_id"
    );

    assert_eq!(
        declared_field_names(signature),
        authority,
        "Message::state_result parameter order must exactly match the StateResult \
         variant field order. Positional callers are too easy to desynchronize \
         otherwise — especially across the repeated-type slots \
         prompt_id/placeholder/selected_value/screenshot_identity, \
         choice_count/visible_choice_count, and is_focused/window_visible."
    );
    assert_eq!(
        forwarded_field_names(literal),
        authority,
        "Message::state_result must forward every parameter into Message::StateResult \
         in the same order as the variant fields (shorthand only, so a transposition \
         here silently swaps same-typed values)."
    );
}
