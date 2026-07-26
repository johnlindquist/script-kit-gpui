use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::cf::*;
use super::ffi::*;

/// Global window cache using LazyLock (std alternative to lazy_static)
pub(super) static WINDOW_CACHE: LazyLock<Mutex<HashMap<u32, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// An owned cached window reference retained while in use.
pub(super) struct OwnedCachedWindowRef {
    window_ref: AXUIElementRef,
}

impl OwnedCachedWindowRef {
    pub(super) fn as_ptr(&self) -> AXUIElementRef {
        self.window_ref
    }
}

impl Drop for OwnedCachedWindowRef {
    fn drop(&mut self) {
        cf_release(self.window_ref as CFTypeRef);
    }
}

/// Single-owner retained AX reference (RAII).
///
/// The registry owns exactly one `CachedAxRef` per live window; callers get
/// independently retained [`OwnedCachedWindowRef`]s via [`Self::retain_owned`].
/// Deliberately NOT `Clone`: cloning would double-release on drop.
pub(super) struct CachedAxRef {
    raw: usize,
}

// SAFETY: AXUIElementRef is a CoreFoundation object pointer; CF objects are
// thread-safe for retain/release, and all attribute access goes through the
// per-PID executors. The registry stores these behind a lock.
unsafe impl Send for CachedAxRef {}
unsafe impl Sync for CachedAxRef {}

impl CachedAxRef {
    /// Retain a borrowed reference (e.g. from `CFArrayGetValueAtIndex`).
    pub(super) fn from_borrowed(raw: AXUIElementRef) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        let retained = cf_retain(raw as CFTypeRef);
        (!retained.is_null()).then_some(Self {
            raw: retained as usize,
        })
    }

    /// Take ownership of an already-retained reference (e.g. from a Copy API).
    pub(super) fn from_owned(raw: AXUIElementRef) -> Option<Self> {
        (!raw.is_null()).then_some(Self { raw: raw as usize })
    }

    /// Produce an independently retained reference for a caller.
    pub(super) fn retain_owned(&self) -> Option<OwnedCachedWindowRef> {
        let retained = cf_retain(self.raw as CFTypeRef) as AXUIElementRef;
        (!retained.is_null()).then_some(OwnedCachedWindowRef {
            window_ref: retained,
        })
    }

    /// The raw pointer as usize, for identity comparison only.
    pub(super) fn ptr_usize(&self) -> usize {
        self.raw
    }
}

impl Drop for CachedAxRef {
    fn drop(&mut self) {
        cf_release(self.raw as CFTypeRef);
    }
}

impl std::fmt::Debug for CachedAxRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedAxRef").finish_non_exhaustive()
    }
}

/// Get the window cache.
pub(super) fn get_cache() -> &'static Mutex<HashMap<u32, usize>> {
    &WINDOW_CACHE
}

pub(super) fn cache_window(id: u32, window_ref: AXUIElementRef) {
    if let Ok(mut cache) = get_cache().lock() {
        if let Some(previous) = cache.insert(id, window_ref as usize) {
            // The cache owns retained window references. Replacing an entry must
            // release the previous retained pointer to avoid leaks.
            cf_release(previous as CFTypeRef);
        }
    }
}

pub(super) fn get_cached_window(id: u32) -> Option<OwnedCachedWindowRef> {
    let cache = get_cache().lock().ok()?;
    let ptr = *cache.get(&id)?;
    let retained = cf_retain(ptr as CFTypeRef) as AXUIElementRef;
    if retained.is_null() {
        None
    } else {
        Some(OwnedCachedWindowRef {
            window_ref: retained,
        })
    }
}

pub(super) fn clear_window_cache() {
    if let Ok(mut cache) = get_cache().lock() {
        // Release all retained window refs before clearing
        for &window_ptr in cache.values() {
            cf_release(window_ptr as CFTypeRef);
        }
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    fn cf_get_retain_count(cf: CFTypeRef) -> isize {
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFGetRetainCount(cf: CFTypeRef) -> isize;
        }

        // SAFETY: CFGetRetainCount only reads retain-count metadata from a live
        // Core Foundation object. The tests pass objects created earlier in the
        // same scope and keep them alive across this call, so `cf` is valid here.
        unsafe { CFGetRetainCount(cf) }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cached_ax_ref_from_borrowed_retains_once_and_releases_on_drop() {
        let object = try_create_cf_string("cached-ax-ref-borrowed").expect("cf string");
        let baseline = cf_get_retain_count(object);

        let cached = CachedAxRef::from_borrowed(object as AXUIElementRef).expect("non-null");
        let after_stage = cf_get_retain_count(object);
        assert_eq!(
            after_stage,
            baseline + 1,
            "from_borrowed must retain exactly once"
        );
        assert_eq!(cached.ptr_usize(), object as usize);

        drop(cached);
        let after_drop = cf_get_retain_count(object);
        assert_eq!(after_drop, baseline, "drop must release the staged retain");

        cf_release(object);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cached_ax_ref_retain_owned_adds_one_retain_released_on_drop() {
        let object = try_create_cf_string("cached-ax-ref-owned").expect("cf string");
        let cached = CachedAxRef::from_borrowed(object as AXUIElementRef).expect("non-null");
        let baseline = cf_get_retain_count(object);

        let owned = cached.retain_owned().expect("retain");
        assert_eq!(owned.as_ptr(), object as AXUIElementRef);
        assert_eq!(
            cf_get_retain_count(object),
            baseline + 1,
            "retain_owned must add exactly one retain"
        );

        drop(owned);
        assert_eq!(
            cf_get_retain_count(object),
            baseline,
            "dropping the owned ref must release its retain"
        );

        drop(cached);
        cf_release(object);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cached_ax_ref_from_owned_takes_ownership_without_extra_retain() {
        let object = try_create_cf_string("cached-ax-ref-from-owned").expect("cf string");
        let baseline = cf_get_retain_count(object);

        // Simulate a Copy-API result: retain here, hand ownership to CachedAxRef.
        let retained = cf_retain(object) as AXUIElementRef;
        let cached = CachedAxRef::from_owned(retained).expect("non-null");
        assert_eq!(
            cf_get_retain_count(object),
            baseline + 1,
            "from_owned must not add another retain"
        );

        drop(cached);
        assert_eq!(
            cf_get_retain_count(object),
            baseline,
            "drop must release the transferred retain"
        );

        cf_release(object);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn replacing_a_map_entry_releases_the_previous_cached_ref() {
        use std::collections::HashMap;

        let first = try_create_cf_string("cached-map-first").expect("cf string");
        let second = try_create_cf_string("cached-map-second").expect("cf string");
        let first_baseline = cf_get_retain_count(first);

        let mut map: HashMap<u32, CachedAxRef> = HashMap::new();
        map.insert(
            7,
            CachedAxRef::from_borrowed(first as AXUIElementRef).expect("non-null"),
        );
        assert_eq!(cf_get_retain_count(first), first_baseline + 1);

        map.insert(
            7,
            CachedAxRef::from_borrowed(second as AXUIElementRef).expect("non-null"),
        );
        assert_eq!(
            cf_get_retain_count(first),
            first_baseline,
            "replacing the entry must release the old ref"
        );

        drop(map);
        cf_release(first);
        cf_release(second);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn dropping_a_staged_map_releases_every_cached_ref() {
        use std::collections::HashMap;

        let a = try_create_cf_string("cached-map-drop-a").expect("cf string");
        let b = try_create_cf_string("cached-map-drop-b").expect("cf string");
        let a_baseline = cf_get_retain_count(a);
        let b_baseline = cf_get_retain_count(b);

        let mut map: HashMap<u32, CachedAxRef> = HashMap::new();
        map.insert(
            1,
            CachedAxRef::from_borrowed(a as AXUIElementRef).expect("non-null"),
        );
        map.insert(
            2,
            CachedAxRef::from_borrowed(b as AXUIElementRef).expect("non-null"),
        );

        drop(map);
        assert_eq!(cf_get_retain_count(a), a_baseline);
        assert_eq!(cf_get_retain_count(b), b_baseline);

        cf_release(a);
        cf_release(b);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_window_cache_releases_previous_pointer_on_overwrite() {
        clear_window_cache();

        let window_id = 0xCAFE_1000;
        let first_window =
            try_create_cf_string("window-cache-overwrite-first").expect("valid CFString literal");
        let second_window =
            try_create_cf_string("window-cache-overwrite-second").expect("valid CFString literal");

        cache_window(window_id, cf_retain(first_window) as AXUIElementRef);
        let first_after_insert = cf_get_retain_count(first_window);

        cache_window(window_id, cf_retain(second_window) as AXUIElementRef);
        let first_after_overwrite = cf_get_retain_count(first_window);

        assert_eq!(
            first_after_overwrite + 1,
            first_after_insert,
            "cache overwrite should release old retained window pointer"
        );

        clear_window_cache();
        cf_release(first_window);
        cf_release(second_window);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_window_cache_get_returns_owned_reference_and_releases_on_drop() {
        clear_window_cache();

        let window_id = 0xCAFE_2000;
        let window =
            try_create_cf_string("window-cache-owned-get").expect("valid CFString literal");
        cache_window(window_id, cf_retain(window) as AXUIElementRef);

        let before_get = cf_get_retain_count(window);
        let owned = get_cached_window(window_id).expect("window should exist in cache");
        assert_eq!(owned.as_ptr(), window as AXUIElementRef);

        let during_get = cf_get_retain_count(window);
        assert_eq!(
            during_get,
            before_get + 1,
            "get_cached_window should retain before returning"
        );

        drop(owned);
        let after_drop = cf_get_retain_count(window);
        assert_eq!(
            after_drop, before_get,
            "dropping owned cached window should release retained reference"
        );

        clear_window_cache();
        cf_release(window);
    }
}
