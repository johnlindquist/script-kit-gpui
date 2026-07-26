//! Unified public display topology.
//!
//! `window_control` is the sole macOS display-information owner. Descriptors
//! come from NSScreen (`NSScreenNumber`, `localizedName`, `frame`,
//! `visibleFrame`, `backingScaleFactor`) plus the public
//! `CGDisplayCreateUUIDFromDisplayID`; the deterministic provider supplies
//! fixture displays. Coordinate conversion stays anchored to `screens[0]`
//! (the primary screen), never `mainScreen`.
//!
//! The topology generation advances only when identity-relevant facts change
//! (id, uuid, bounds, scale, primary status, legacy order) — transient
//! re-reads keep the same generation so plans can pin against it.

use anyhow::{Context, Result};

use super::identity::next_generation;
use super::types::{Bounds, Direction, DisplayDescriptor, DisplayId, DisplayOrientation, Insets};

/// Read-only snapshot of the current topology.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayTopologySnapshot {
    pub generation: u64,
    pub displays: Vec<DisplayDescriptor>,
}

struct DisplayTopologyState {
    generation: u64,
    descriptors: Vec<DisplayDescriptor>,
}

static TOPOLOGY: std::sync::LazyLock<parking_lot::Mutex<DisplayTopologyState>> =
    std::sync::LazyLock::new(|| {
        parking_lot::Mutex::new(DisplayTopologyState {
            generation: 0,
            descriptors: Vec::new(),
        })
    });

fn orientation_for(full: Bounds) -> DisplayOrientation {
    match full.width.cmp(&full.height) {
        std::cmp::Ordering::Greater => DisplayOrientation::Landscape,
        std::cmp::Ordering::Less => DisplayOrientation::Portrait,
        std::cmp::Ordering::Equal => DisplayOrientation::Square,
    }
}

fn insets_between(full: Bounds, visible: Bounds) -> Insets {
    Insets {
        top: (visible.y - full.y).max(0) as u32,
        left: (visible.x - full.x).max(0) as u32,
        right: ((full.x + full.width as i32) - (visible.x + visible.width as i32)).max(0) as u32,
        bottom: ((full.y + full.height as i32) - (visible.y + visible.height as i32)).max(0) as u32,
    }
}

/// Identity signature: facts whose change advances the topology generation.
fn identity_signature(
    descriptors: &[DisplayDescriptor],
) -> Vec<(u32, String, Bounds, Bounds, u64, bool, usize)> {
    descriptors
        .iter()
        .map(|display| {
            (
                display.id.0,
                display.uuid.clone(),
                display.full_bounds,
                display.visible_bounds,
                display.backing_scale_factor.to_bits(),
                display.is_primary,
                display.legacy_order,
            )
        })
        .collect()
}

/// Build a descriptor from raw facts (shared by live + provider paths).
#[allow(clippy::too_many_arguments)]
fn build_descriptor(
    id: u32,
    uuid: String,
    localized_name: String,
    full_bounds: Bounds,
    visible_bounds: Bounds,
    backing_scale_factor: f64,
    is_primary: bool,
    legacy_order: usize,
) -> DisplayDescriptor {
    DisplayDescriptor {
        id: DisplayId(id),
        uuid,
        localized_name,
        full_bounds,
        visible_bounds,
        usable_insets: insets_between(full_bounds, visible_bounds),
        backing_scale_factor,
        orientation: orientation_for(full_bounds),
        is_primary,
        legacy_order,
        topology_generation: 0, // stamped at publication
    }
}

fn publish(descriptors: Vec<DisplayDescriptor>) -> DisplayTopologySnapshot {
    let mut state = TOPOLOGY.lock();
    let changed = identity_signature(&state.descriptors) != identity_signature(&descriptors)
        || state.generation == 0;
    if changed {
        state.generation = next_generation(state.generation);
    }
    let generation = state.generation;
    state.descriptors = descriptors
        .into_iter()
        .map(|mut display| {
            display.topology_generation = generation;
            display
        })
        .collect();
    DisplayTopologySnapshot {
        generation,
        displays: state.descriptors.clone(),
    }
}

/// Stable UUID for a CG display id via the PUBLIC CoreGraphics API.
fn display_uuid(cg_display_id: u32) -> (String, bool) {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFUUIDCreateString(alloc: *const c_void, uuid: *const c_void) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    // SAFETY: public CG/CF calls; every returned object is null-checked and
    // released exactly once.
    unsafe {
        let uuid_ref = CGDisplayCreateUUIDFromDisplayID(cg_display_id);
        if uuid_ref.is_null() {
            return (format!("cg-display-{cg_display_id}"), false);
        }
        let string_ref = CFUUIDCreateString(std::ptr::null(), uuid_ref);
        CFRelease(uuid_ref);
        if string_ref.is_null() {
            return (format!("cg-display-{cg_display_id}"), false);
        }
        let uuid = CFString::wrap_under_create_rule(string_ref as _).to_string();
        (uuid, true)
    }
}

/// Refresh topology from the live system (NSScreen-anchored).
fn refresh_from_system() -> Result<DisplayTopologySnapshot> {
    use core_graphics::display::{CGDisplay, CGRect};

    let mut descriptors = Vec::new();
    // SAFETY: objc messaging to NSScreen class methods; pointers null-checked.
    unsafe {
        use objc::runtime::{Class, Object};
        use objc::{msg_send, sel, sel_impl};

        let nsscreen_class = Class::get("NSScreen").context("Failed to get NSScreen class")?;
        let screens: *mut Object = msg_send![nsscreen_class, screens];
        anyhow::ensure!(!screens.is_null(), "Failed to get screens");
        let screen_count: usize = msg_send![screens, count];
        anyhow::ensure!(screen_count > 0, "No screens found");

        // Coordinate conversion anchor: screens[0], never mainScreen.
        let primary_screen: *mut Object = msg_send![screens, objectAtIndex: 0usize];
        let primary_frame: CGRect = msg_send![primary_screen, frame];
        let primary_height = primary_frame.size.height;
        let cg_main_id = CGDisplay::main().id;

        let to_cg = |frame: CGRect| -> Bounds {
            let cg_y = primary_height - (frame.origin.y + frame.size.height);
            Bounds::new(
                frame.origin.x as i32,
                cg_y as i32,
                frame.size.width as u32,
                frame.size.height as u32,
            )
        };

        for index in 0..screen_count {
            let screen: *mut Object = msg_send![screens, objectAtIndex: index];
            if screen.is_null() {
                continue;
            }
            // NSScreenNumber from deviceDescription — matched by key, never
            // by zipping unrelated arrays.
            let description: *mut Object = msg_send![screen, deviceDescription];
            let key: *mut Object = {
                let cls = Class::get("NSString").context("NSString class")?;
                msg_send![cls, stringWithUTF8String: c"NSScreenNumber".as_ptr()]
            };
            let number: *mut Object = msg_send![description, objectForKey: key];
            let cg_display_id: u32 = if number.is_null() {
                continue;
            } else {
                let value: u32 = msg_send![number, unsignedIntValue];
                value
            };

            let name: *mut Object = msg_send![screen, localizedName];
            let localized_name = if name.is_null() {
                format!("Display {index}")
            } else {
                let utf8: *const i8 = msg_send![name, UTF8String];
                if utf8.is_null() {
                    format!("Display {index}")
                } else {
                    std::ffi::CStr::from_ptr(utf8)
                        .to_str()
                        .unwrap_or("Display")
                        .to_string()
                }
            };

            let frame: CGRect = msg_send![screen, frame];
            let visible_frame: CGRect = msg_send![screen, visibleFrame];
            let scale: f64 = msg_send![screen, backingScaleFactor];
            let (uuid, _stable) = display_uuid(cg_display_id);

            descriptors.push(build_descriptor(
                cg_display_id,
                uuid,
                localized_name,
                to_cg(frame),
                to_cg(visible_frame),
                scale,
                cg_display_id == cg_main_id,
                index,
            ));
        }
    }
    anyhow::ensure!(!descriptors.is_empty(), "No display descriptors resolved");
    Ok(publish(descriptors))
}

/// Refresh topology from the deterministic provider fixture.
fn refresh_from_provider() -> Result<DisplayTopologySnapshot> {
    let displays = super::test_support::provider_displays()?;
    anyhow::ensure!(
        !displays.is_empty(),
        "provider fixture declares no displays"
    );
    let descriptors = displays
        .into_iter()
        .map(|display| {
            let full = Bounds::new(
                display.full_bounds.x.unwrap_or(0),
                display.full_bounds.y.unwrap_or(0),
                display.full_bounds.width.unwrap_or(1920),
                display.full_bounds.height.unwrap_or(1080),
            );
            let visible = Bounds::new(
                display.visible_bounds.x.unwrap_or(full.x),
                display.visible_bounds.y.unwrap_or(full.y),
                display.visible_bounds.width.unwrap_or(full.width),
                display.visible_bounds.height.unwrap_or(full.height),
            );
            build_descriptor(
                display.id,
                display.uuid,
                display.name,
                full,
                visible,
                display.scale_factor,
                display.is_primary,
                display.legacy_order,
            )
        })
        .collect();
    Ok(publish(descriptors))
}

/// The current display list, refreshing from the active backend.
pub fn list_displays() -> Result<Vec<DisplayDescriptor>> {
    Ok(display_topology_snapshot()?.displays)
}

/// Refresh and snapshot the topology.
pub fn display_topology_snapshot() -> Result<DisplayTopologySnapshot> {
    if super::test_support::is_active() {
        refresh_from_provider()
    } else {
        refresh_from_system()
    }
}

/// The current topology generation without refreshing.
pub(crate) fn topology_generation() -> u64 {
    TOPOLOGY.lock().generation
}

/// The display containing a point, from the current cached topology.
pub fn display_for_point(x: i32, y: i32) -> Option<DisplayDescriptor> {
    let state = TOPOLOGY.lock();
    state
        .descriptors
        .iter()
        .find(|display| {
            let full = display.full_bounds;
            x >= full.x
                && x < full.x + full.width as i32
                && y >= full.y
                && y < full.y + full.height as i32
        })
        .cloned()
}

fn overlap_area(a: Bounds, b: Bounds) -> i64 {
    let left = a.x.max(b.x) as i64;
    let right = ((a.x + a.width as i32).min(b.x + b.width as i32)) as i64;
    let top = a.y.max(b.y) as i64;
    let bottom = ((a.y + a.height as i32).min(b.y + b.height as i32)) as i64;
    ((right - left).max(0)) * ((bottom - top).max(0))
}

/// The display with the greatest overlap against `bounds`.
pub fn dominant_display(bounds: Bounds) -> Option<DisplayDescriptor> {
    let state = TOPOLOGY.lock();
    state
        .descriptors
        .iter()
        .max_by_key(|display| overlap_area(display.full_bounds, bounds))
        .filter(|display| overlap_area(display.full_bounds, bounds) > 0)
        .cloned()
        .or_else(|| state.descriptors.first().cloned())
}

/// Legacy next/previous ordering: NSScreen order, wrapping.
pub fn cyclic_neighbor(
    current: DisplayId,
    next: bool,
    displays: &[DisplayDescriptor],
) -> Option<DisplayId> {
    if displays.is_empty() {
        return None;
    }
    let mut ordered: Vec<&DisplayDescriptor> = displays.iter().collect();
    ordered.sort_by_key(|display| display.legacy_order);
    let position = ordered.iter().position(|display| display.id == current)?;
    let target = if next {
        (position + 1) % ordered.len()
    } else if position == 0 {
        ordered.len() - 1
    } else {
        position - 1
    };
    Some(ordered[target].id)
}

fn center(display: &DisplayDescriptor) -> (i64, i64) {
    let full = display.full_bounds;
    (
        full.x as i64 + full.width as i64 / 2,
        full.y as i64 + full.height as i64 / 2,
    )
}

fn lies_in_half_plane(
    current: &DisplayDescriptor,
    candidate: &DisplayDescriptor,
    direction: Direction,
) -> bool {
    let (cx, cy) = center(current);
    let (px, py) = center(candidate);
    let dx = px - cx;
    let dy = py - cy;
    // Dominant-axis classification: a candidate counts as a horizontal
    // neighbor only when the displacement is predominantly horizontal —
    // unless its projection overlaps on the orthogonal axis, in which case
    // the half-plane test alone is sufficient (side-by-side monitors of
    // different sizes still count as left/right).
    let overlaps = projections_overlap(current, candidate, direction);
    match direction {
        Direction::Left => dx < 0 && (overlaps || dx.abs() >= dy.abs()),
        Direction::Right => dx > 0 && (overlaps || dx.abs() >= dy.abs()),
        Direction::Up => dy < 0 && (overlaps || dy.abs() >= dx.abs()),
        Direction::Down => dy > 0 && (overlaps || dy.abs() >= dx.abs()),
    }
}

fn axis_gap(
    current: &DisplayDescriptor,
    candidate: &DisplayDescriptor,
    direction: Direction,
) -> i64 {
    let a = current.full_bounds;
    let b = candidate.full_bounds;
    let gap = match direction {
        Direction::Left => a.x as i64 - (b.x as i64 + b.width as i64),
        Direction::Right => b.x as i64 - (a.x as i64 + a.width as i64),
        Direction::Up => a.y as i64 - (b.y as i64 + b.height as i64),
        Direction::Down => b.y as i64 - (a.y as i64 + a.height as i64),
    };
    gap.max(0)
}

fn projections_overlap(
    current: &DisplayDescriptor,
    candidate: &DisplayDescriptor,
    direction: Direction,
) -> bool {
    let a = current.full_bounds;
    let b = candidate.full_bounds;
    match direction {
        Direction::Left | Direction::Right => {
            let top = a.y.max(b.y);
            let bottom = (a.y + a.height as i32).min(b.y + b.height as i32);
            bottom > top
        }
        Direction::Up | Direction::Down => {
            let left = a.x.max(b.x);
            let right = (a.x + a.width as i32).min(b.x + b.width as i32);
            right > left
        }
    }
}

fn orthogonal_center_distance(
    current: &DisplayDescriptor,
    candidate: &DisplayDescriptor,
    direction: Direction,
) -> i64 {
    let (cx, cy) = center(current);
    let (px, py) = center(candidate);
    match direction {
        Direction::Left | Direction::Right => (py - cy).abs(),
        Direction::Up | Direction::Down => (px - cx).abs(),
    }
}

fn squared_center_distance(current: &DisplayDescriptor, candidate: &DisplayDescriptor) -> i64 {
    let (cx, cy) = center(current);
    let (px, py) = center(candidate);
    (px - cx) * (px - cx) + (py - cy) * (py - cy)
}

/// Physically directional neighbor selection over rectangle topology.
pub fn directional_neighbor(
    current: DisplayId,
    direction: Direction,
    displays: &[DisplayDescriptor],
) -> Option<DisplayId> {
    let current_display = displays.iter().find(|display| display.id == current)?;
    displays
        .iter()
        .filter(|candidate| candidate.id != current)
        .filter(|candidate| lies_in_half_plane(current_display, candidate, direction))
        .min_by_key(|candidate| {
            let overlap_penalty = if projections_overlap(current_display, candidate, direction) {
                0
            } else {
                10_000
            };
            let primary_gap = axis_gap(current_display, candidate, direction);
            let orthogonal = orthogonal_center_distance(current_display, candidate, direction);
            let tiebreak = squared_center_distance(current_display, candidate);
            (
                overlap_penalty + primary_gap * 16 + orthogonal,
                tiebreak,
                candidate.legacy_order,
            )
        })
        .map(|display| display.id)
}

#[cfg(test)]
pub(super) fn reset_topology_for_tests() {
    let mut state = TOPOLOGY.lock();
    state.generation = 0;
    state.descriptors.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    static TOPOLOGY_TEST_LOCK: std::sync::LazyLock<parking_lot::Mutex<()>> =
        std::sync::LazyLock::new(|| parking_lot::Mutex::new(()));

    fn display(
        id: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        order: usize,
    ) -> DisplayDescriptor {
        build_descriptor(
            id,
            format!("uuid-{id}"),
            format!("Display {id}"),
            Bounds::new(x, y, width, height),
            Bounds::new(x, y + 25, width, height - 25),
            2.0,
            order == 0,
            order,
        )
    }

    #[test]
    fn insets_derive_from_full_and_visible_bounds() {
        let insets = insets_between(Bounds::new(0, 0, 1920, 1080), Bounds::new(4, 25, 1912, 985));
        assert_eq!(
            insets,
            Insets {
                top: 25,
                left: 4,
                right: 4,
                bottom: 70
            }
        );
    }

    #[test]
    fn orientation_follows_aspect() {
        assert_eq!(
            orientation_for(Bounds::new(0, 0, 1920, 1080)),
            DisplayOrientation::Landscape
        );
        assert_eq!(
            orientation_for(Bounds::new(0, 0, 1080, 1920)),
            DisplayOrientation::Portrait
        );
        assert_eq!(
            orientation_for(Bounds::new(0, 0, 1000, 1000)),
            DisplayOrientation::Square
        );
    }

    #[test]
    fn horizontal_arrangement_selects_the_physically_right_display() {
        let displays = vec![
            display(1, 0, 0, 1920, 1080, 0),
            display(2, 1920, 0, 1920, 1080, 1),
        ];
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Right, &displays),
            Some(DisplayId(2))
        );
        assert_eq!(
            directional_neighbor(DisplayId(2), Direction::Left, &displays),
            Some(DisplayId(1))
        );
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Left, &displays),
            None
        );
    }

    #[test]
    fn vertical_arrangement_selects_up_and_down() {
        let displays = vec![
            display(1, 0, 0, 1920, 1080, 0),
            display(2, 200, -1440, 2560, 1440, 1),
        ];
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Up, &displays),
            Some(DisplayId(2))
        );
        assert_eq!(
            directional_neighbor(DisplayId(2), Direction::Down, &displays),
            Some(DisplayId(1))
        );
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Right, &displays),
            None
        );
    }

    #[test]
    fn l_shaped_topology_prefers_overlapping_projection() {
        // Current at origin; one display directly right, one diagonal
        // down-right (no vertical overlap).
        let displays = vec![
            display(1, 0, 0, 1000, 1000, 0),
            display(2, 1000, 0, 1000, 1000, 1),
            display(3, 1000, 1200, 1000, 1000, 2),
        ];
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Right, &displays),
            Some(DisplayId(2))
        );
        // From the diagonal display, moving up prefers its overlapping column.
        assert_eq!(
            directional_neighbor(DisplayId(3), Direction::Up, &displays),
            Some(DisplayId(2))
        );
    }

    #[test]
    fn overlapping_topologies_use_center_distance_tiebreak() {
        let displays = vec![
            display(1, 0, 0, 1000, 1000, 0),
            display(2, 900, 0, 1000, 1000, 1),
            display(3, 2100, 0, 1000, 1000, 2),
        ];
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Right, &displays),
            Some(DisplayId(2))
        );
    }

    #[test]
    fn portrait_negative_origin_displays_work() {
        let displays = vec![
            display(1, 0, 0, 1920, 1080, 0),
            display(2, -1080, -200, 1080, 1920, 1),
        ];
        assert_eq!(
            directional_neighbor(DisplayId(1), Direction::Left, &displays),
            Some(DisplayId(2))
        );
        assert_eq!(displays[1].orientation, DisplayOrientation::Portrait);
    }

    #[test]
    fn cyclic_order_follows_legacy_order_and_wraps() {
        let displays = vec![
            display(5, 0, 0, 1000, 1000, 0),
            display(9, 1000, 0, 1000, 1000, 1),
            display(7, 2000, 0, 1000, 1000, 2),
        ];
        assert_eq!(
            cyclic_neighbor(DisplayId(5), true, &displays),
            Some(DisplayId(9))
        );
        assert_eq!(
            cyclic_neighbor(DisplayId(7), true, &displays),
            Some(DisplayId(5))
        );
        assert_eq!(
            cyclic_neighbor(DisplayId(5), false, &displays),
            Some(DisplayId(7))
        );
    }

    #[test]
    fn dominant_display_uses_overlap_area() {
        let _lock = TOPOLOGY_TEST_LOCK.lock();
        reset_topology_for_tests();
        publish(vec![
            display(1, 0, 0, 1000, 1000, 0),
            display(2, 1000, 0, 1000, 1000, 1),
        ]);
        let dominant = dominant_display(Bounds::new(800, 100, 600, 400)).expect("dominant");
        assert_eq!(dominant.id, DisplayId(2));
        reset_topology_for_tests();
    }

    #[test]
    fn topology_generation_is_stable_until_identity_changes() {
        let _lock = TOPOLOGY_TEST_LOCK.lock();
        reset_topology_for_tests();
        let first = publish(vec![display(1, 0, 0, 1000, 1000, 0)]);
        let second = publish(vec![display(1, 0, 0, 1000, 1000, 0)]);
        assert_eq!(first.generation, second.generation);
        let third = publish(vec![
            display(1, 0, 0, 1000, 1000, 0),
            display(2, 1000, 0, 1000, 1000, 1),
        ]);
        assert!(third.generation > second.generation);
        reset_topology_for_tests();
    }

    #[test]
    fn provider_and_native_paths_share_one_descriptor_type() {
        let _lock = TOPOLOGY_TEST_LOCK.lock();
        let _env = super::super::test_support::test_env::EnvGuard::set(
            r#"{
                "windows": [{"app":"A","title":"T"}],
                "displays": [
                    {"id": 1, "uuid": "fixture-primary", "name": "Main",
                     "fullBounds": {"x":0,"y":0,"width":1920,"height":1080},
                     "visibleBounds": {"x":0,"y":25,"width":1920,"height":1055},
                     "isPrimary": true},
                    {"id": 2, "uuid": "fixture-portrait", "name": "Portrait",
                     "fullBounds": {"x":-1080,"y":-200,"width":1080,"height":1920},
                     "visibleBounds": {"x":-1080,"y":-200,"width":1080,"height":1920},
                     "legacyOrder": 1}
                ]
            }"#,
        );
        reset_topology_for_tests();
        let snapshot = display_topology_snapshot().expect("snapshot");
        assert_eq!(snapshot.displays.len(), 2);
        assert!(snapshot.displays[0].is_primary);
        assert_eq!(
            snapshot.displays[1].orientation,
            DisplayOrientation::Portrait
        );
        assert_eq!(
            snapshot.displays[0].usable_insets.top, 25,
            "insets derive from full/visible"
        );
        reset_topology_for_tests();
    }
}
