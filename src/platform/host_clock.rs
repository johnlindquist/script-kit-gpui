//! One process-wide macOS host-time domain for native lifecycle receipts.
//!
//! ScreenCaptureKit's `displayTime` is expressed in Mach absolute host ticks.
//! Product events that are compared with captured frames must use the same
//! origin and timebase rather than independent `Instant` origins.

#[cfg(target_os = "macos")]
#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

#[cfg(target_os = "macos")]
pub(crate) fn host_time_ns() -> u64 {
    static TIMEBASE: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
    let (numer, denom) = *TIMEBASE.get_or_init(|| {
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: `info` is a valid writable Mach timebase structure.
        let status = unsafe { mach_timebase_info(&mut info) };
        assert_eq!(status, 0, "mach_timebase_info failed");
        (u64::from(info.numer), u64::from(info.denom.max(1)))
    });
    // SAFETY: `mach_absolute_time` has no preconditions.
    let ticks = unsafe { mach_absolute_time() };
    let quotient = ticks / denom;
    let remainder = ticks % denom;
    quotient
        .saturating_mul(numer)
        .saturating_add(remainder.saturating_mul(numer) / denom)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn host_time_ns() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64
}
