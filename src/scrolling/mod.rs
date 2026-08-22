#[allow(
    dead_code,
    reason = "the separately compiled launcher binary owns boundary-affordance consumers"
)]
pub(crate) mod boundary_affordance;
#[cfg(test)]
mod boundary_affordance_fuzz_tests;
pub(crate) mod free_scroll;
#[allow(
    dead_code,
    reason = "the separately compiled launcher binary owns grouped-list geometry consumers"
)]
pub(crate) mod list_geometry;
#[allow(
    dead_code,
    reason = "the separately compiled launcher binary owns viewport interaction consumers"
)]
pub(crate) mod list_interaction;
#[cfg(test)]
mod native_script_list_scroll_tests;
