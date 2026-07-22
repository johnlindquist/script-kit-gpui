//! Source-level contract pinning the view-agnostic invariant of the
//! shared `triggerBuiltin` dispatcher — `dispatch_trigger_builtin_name`
//! + `apply_trigger_builtin` in `src/app_impl/trigger_builtin_dispatch.rs`.
//!
//! Run 9 Pass #16's A30 live-verified (fresh binary pid 30544) that
//! firing `triggerBuiltin browser-tabs` from INSIDE an active
//! `ThemeChooserView` session — with no intervening `hide`, `escape`,
//! or return-to-main — successfully flips the prompt to `browserTabs`
//! with `choiceCount:39, visible:39`. The cross-view in-place switch is
//! a documented design behavior: the single umbrella dispatcher does
//! NOT gate on `self.current_view` before routing the new trigger. It
//! unconditionally clears `self.opened_from_main_menu`, resolves the
//! canonical registry entry, and routes via the pure planner + the
//! `FilterableView` arms pinned by
//! `trigger_builtin_repeat_idempotency_contract.rs`.
//!
//! **Refactor threat** — a contributor, surprised that
//! `triggerBuiltin X` clobbers an already-active non-main view
//! (e.g. after reading Pass #12's receipt-only observation before
//! Pass #13's retraction), could add a guard at the top of
//! `dispatch_trigger_builtin_name`:
//!
//! ```ignore
//! if !matches!(self.current_view, AppView::ScriptList) {
//!     return None;
//! }
//! ```
//!
//! or equivalent inside `apply_trigger_builtin`. That change would
//! turn the observed compose-in-place behavior into a silent no-op,
//! breaking the Pass #16 A30 receipt and the automation flows that
//! rely on quick view-to-view jumps without an intervening `escape`.
//! This contract catches that edit at test time.
//!
//! Complements but does NOT overlap with the Run 2 Pass #55 contract
//! `trigger_builtin_repeat_idempotency_contract.rs`, which pins the
//! per-view assignment arms inside `show_filterable_view` (pure
//! struct-literal overwrites, no conditional wrappers). This one pins
//! the UMBRELLA functions one level up.

const DISPATCH: &str = include_str!("../src/app_impl/trigger_builtin_dispatch.rs");
const LIFECYCLE_RESET: &str = include_str!("../src/app_impl/lifecycle_reset.rs");

fn body_of<'a>(fn_name: &str, file: &'a str) -> &'a str {
    let start = file.find(fn_name).unwrap_or_else(|| {
        panic!(
            "expected `{}` to appear in trigger_builtin_dispatch.rs",
            fn_name
        )
    });
    // Skip past signature to the opening `{` of the body.
    let brace_rel = file[start..]
        .find(" {\n")
        .unwrap_or_else(|| panic!("expected body opener after `{}` signature", fn_name));
    let body_start = start + brace_rel + 3;
    // Walk to matching close-brace (depth-counted over `{` and `}`).
    let bytes = file.as_bytes();
    let mut depth = 1_i32;
    let mut i = body_start;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    &file[body_start..i]
}

#[test]
fn dispatch_trigger_builtin_name_body_has_no_current_view_gate() {
    let body = body_of("pub fn dispatch_trigger_builtin_name(", DISPATCH);
    assert!(
        !body.contains("current_view"),
        "`dispatch_trigger_builtin_name` body MUST NOT reference \
         `current_view` — adding a gate like `if matches!(self.current_view, \
         AppView::X) {{ return None }}` would break the Pass #16 A30 \
         compose-in-place receipt. Body was:\n{}",
        body
    );
    assert!(
        !body.contains(" if matches!"),
        "`dispatch_trigger_builtin_name` body MUST NOT contain any \
         `if matches!(…)` early-return guard — those are the shape a \
         view-gate refactor would take. Body was:\n{}",
        body
    );
}

#[test]
fn apply_trigger_builtin_body_has_no_current_view_gate() {
    let body = body_of("fn apply_trigger_builtin(", DISPATCH);
    assert!(
        !body.contains("current_view"),
        "`apply_trigger_builtin` body MUST NOT reference `current_view` — \
         the exhaustive `match plan_trigger_builtin_route(id)` dispatch \
         is view-agnostic by design. A refactor inserting \
         `if matches!(self.current_view, …)` before the match would \
         silently drop legitimate cross-view switches. Body was:\n{}",
        body
    );
}

#[test]
fn dispatch_clears_opened_from_main_menu_before_registry_resolve() {
    let body = body_of("pub fn dispatch_trigger_builtin_name(", DISPATCH);
    let clear_idx = body
        .find("self.mark_opened_directly(\"protocol\");")
        .expect(
            "`dispatch_trigger_builtin_name` must mark the launch origin as \
             direct — that unconditional clear is what lets ESC close the \
             window (not return to main) across all three legacy call sites \
             the dispatcher collapsed. Removing it would re-introduce the \
             Run 1-era ESC-returns-to-main bug for cross-view triggers.",
        );
    let resolve_idx = body
        .find("trigger_registry().resolve(")
        .expect("registry resolve call must exist in dispatch body");
    assert!(
        clear_idx < resolve_idx,
        "`self.mark_opened_directly(\"protocol\");` must appear BEFORE the \
         `trigger_registry().resolve(` call. Reordering (or making the \
         clear conditional on resolve success) would restore the \
         ESC-returns-to-main bug for unknown-name triggers. Body was:\n{}",
        body
    );
    assert!(
        LIFECYCLE_RESET.contains("pub(crate) fn mark_opened_directly")
            && LIFECYCLE_RESET.contains("self.opened_from_main_menu = false;"),
        "`mark_opened_directly` must remain the helper that clears \
         opened_from_main_menu"
    );
}
