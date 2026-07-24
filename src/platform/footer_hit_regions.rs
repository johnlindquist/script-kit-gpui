//! Window-local geometry for GPUI-owned floating footer controls.
//!
//! Native glass is visual backing only. Pointer routing must use the GPUI
//! control bounds even when glass is unavailable, so this registry is kept
//! separate from `glass_button_host`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FooterHitRegion {
    pub(crate) group: &'static str,
    pub(crate) index: usize,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl FooterHitRegion {
    pub(crate) fn contains_with_guard(self, point: gpui::Point<gpui::Pixels>, guard: f32) -> bool {
        let x = point.x.as_f32();
        let y = point.y.as_f32();
        x >= self.x - guard
            && x <= self.x + self.width + guard
            && y >= self.y - guard
            && y <= self.y + self.height + guard
    }

    pub(crate) fn intersects_bottom_band(self, window_height: f32, band_height: f32) -> bool {
        self.y + self.height >= window_height - band_height && self.y <= window_height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FooterHitRegionSnapshot {
    pub(crate) native_window_number: i64,
    pub(crate) window_width: f32,
    pub(crate) window_height: f32,
    pub(crate) layout_generation: u64,
    pub(crate) regions: Vec<FooterHitRegion>,
}

#[derive(Default)]
struct WindowFooterHitRegions {
    native_window_number: i64,
    window_width: f32,
    window_height: f32,
    layout_generation: u64,
    groups: BTreeMap<&'static str, Vec<FooterHitRegion>>,
}

thread_local! {
    static REGIONS_BY_WINDOW: RefCell<HashMap<usize, WindowFooterHitRegions>> =
        RefCell::new(HashMap::new());
}

#[cfg(target_os = "macos")]
fn native_window_identity(window: &gpui::Window) -> Option<(usize, i64)> {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let view = appkit.ns_view.as_ptr() as id;
    // SAFETY: GPUI invokes prepaint and pointer handlers on the AppKit main
    // thread, and the raw handle belongs to this live window.
    unsafe {
        let native_window: id = msg_send![view, window];
        if native_window == nil {
            return None;
        }
        let number: i64 = msg_send![native_window, windowNumber];
        Some((native_window as usize, number))
    }
}

#[cfg(not(target_os = "macos"))]
fn native_window_identity(_window: &gpui::Window) -> Option<(usize, i64)> {
    None
}

pub(crate) fn sync_for_window(
    window: &gpui::Window,
    group: &'static str,
    bounds: &[gpui::Bounds<gpui::Pixels>],
) {
    let Some((window_key, native_window_number)) = native_window_identity(window) else {
        return;
    };
    let window_size = window.bounds().size;
    let regions = bounds
        .iter()
        .enumerate()
        .map(|(index, bounds)| FooterHitRegion {
            group,
            index,
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        })
        .collect::<Vec<_>>();

    REGIONS_BY_WINDOW.with(|registry| {
        let mut registry = registry.borrow_mut();
        let state = registry.entry(window_key).or_default();
        state.native_window_number = native_window_number;
        state.window_width = window_size.width.as_f32();
        state.window_height = window_size.height.as_f32();
        state.layout_generation = state.layout_generation.wrapping_add(1).max(1);
        state.groups.insert(group, regions);
    });
}

pub(crate) fn remove_group(window: &gpui::Window, group: &'static str) {
    let Some((window_key, _)) = native_window_identity(window) else {
        return;
    };
    REGIONS_BY_WINDOW.with(|registry| {
        let mut registry = registry.borrow_mut();
        let Some(state) = registry.get_mut(&window_key) else {
            return;
        };
        if state.groups.remove(group).is_some() {
            state.layout_generation = state.layout_generation.wrapping_add(1).max(1);
        }
    });
}

pub(crate) fn remove_for_window(window: &gpui::Window) {
    let Some((window_key, _)) = native_window_identity(window) else {
        return;
    };
    REGIONS_BY_WINDOW.with(|registry| {
        registry.borrow_mut().remove(&window_key);
    });
}

pub(crate) fn snapshot_for_window(window: &gpui::Window) -> Option<FooterHitRegionSnapshot> {
    let (window_key, _) = native_window_identity(window)?;
    let size = window.bounds().size;
    REGIONS_BY_WINDOW.with(|registry| {
        let registry = registry.borrow();
        let state = registry.get(&window_key)?;
        let width = size.width.as_f32();
        let height = size.height.as_f32();
        if (state.window_width - width).abs() > 0.5 || (state.window_height - height).abs() > 0.5 {
            return None;
        }
        Some(snapshot(state))
    })
}

pub(crate) fn snapshot_for_native_window_number(
    native_window_number: i64,
) -> Option<FooterHitRegionSnapshot> {
    REGIONS_BY_WINDOW.with(|registry| {
        registry
            .borrow()
            .values()
            .find(|state| state.native_window_number == native_window_number)
            .map(snapshot)
    })
}

fn snapshot(state: &WindowFooterHitRegions) -> FooterHitRegionSnapshot {
    FooterHitRegionSnapshot {
        native_window_number: state.native_window_number,
        window_width: state.window_width,
        window_height: state.window_height,
        layout_generation: state.layout_generation,
        regions: state
            .groups
            .values()
            .flat_map(|regions| regions.iter().copied())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_region_contains_only_its_interaction_box() {
        let region = FooterHitRegion {
            group: "notes",
            index: 1,
            x: 100.0,
            y: 250.0,
            width: 60.0,
            height: 28.0,
        };

        assert!(region.contains_with_guard(gpui::point(gpui::px(99.5), gpui::px(260.0)), 1.0));
        assert!(!region.contains_with_guard(gpui::point(gpui::px(98.0), gpui::px(260.0)), 1.0));
        assert!(region.intersects_bottom_band(280.0, 6.0));
        assert!(!region.intersects_bottom_band(320.0, 6.0));
    }
}
