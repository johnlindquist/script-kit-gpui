//! Unit-safe authored alpha bytes (GOV-003).
//!
//! An [`AlphaByte`] is an ALREADY-QUANTIZED 0..=255 alpha channel byte, the
//! byte domain that `(rgb << 8) | alpha` packers consume. It is deliberately
//! distinct from a normalized `f32` opacity in `0.0..=1.0`; a normalized
//! opacity stays `f32` until its ONE explicit quantization point, where the
//! chosen algorithm is named by the constructor:
//!
//! - [`AlphaByte::from_normalized`] — the canonical TRUNCATING quantization,
//!   byte-identical to `theme::types::opacity_to_alpha`
//!   (`(clamp * 255.0) as u8`).
//! - [`AlphaByte::from_normalized_rounded`] — round-to-nearest
//!   (`(clamp * 255.0).round() as u8`), for boundaries whose pre-existing
//!   algorithm rounds. The two algorithms are deliberately NOT unified.
//! - [`AlphaByte::from_authored_f32`] — transitional bridge for authored byte
//!   values that are still stored as `f32` in structs whose field-type flip is
//!   blocked on the design-contract integration owner (see
//!   `src/components/conversation_style.rs`). It preserves the historical
//!   `alpha.round() as u32` packer cast exactly and debug-asserts the byte
//!   range so a normalized opacity cannot silently slip through as ~0 or a
//!   giant value cannot truncate.
//!
//! `AlphaByte` intentionally derives NO serde traits: a serializer must state
//! its unit explicitly (`{ "value": n, "unit": "alphaByte" }`), so accidental
//! unitless serialization fails to compile instead of emitting a bare number.

/// An authored, already-quantized alpha channel byte.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AlphaByte(u8);

impl AlphaByte {
    /// A value authored directly in the byte domain (e.g. `0x32`, `0x7F`).
    pub const fn authored(value: u8) -> Self {
        Self(value)
    }

    /// Canonical truncating quantization of a normalized opacity — the exact
    /// `theme::types::opacity_to_alpha` algorithm (`(clamp * 255.0) as u8`).
    pub fn from_normalized(opacity: f32) -> Self {
        let clamped = opacity.clamp(0.0, 1.0);
        Self((clamped * 255.0) as u8)
    }

    /// Round-to-nearest quantization of a normalized opacity. Only for
    /// boundaries whose PRE-EXISTING algorithm rounds; never a drop-in
    /// replacement for [`Self::from_normalized`].
    pub fn from_normalized_rounded(opacity: f32) -> Self {
        let clamped = opacity.clamp(0.0, 1.0);
        Self((clamped * 255.0).round() as u8)
    }

    /// Transitional bridge for authored byte values still typed `f32` at their
    /// struct field (field flip blocked on the design-contract integration).
    /// Preserves the historical conversation packer cast (`alpha.round()`)
    /// exactly; debug-asserts the input is already in the byte domain.
    pub fn from_authored_f32(value: f32) -> Self {
        debug_assert!(
            (0.0..=255.0).contains(&value),
            "authored alpha byte out of range: {value}"
        );
        Self((value.round() as u32).min(255) as u8)
    }

    /// The raw byte.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The ONE typed `0xRRGGBB` + alpha-byte packer: `(rgb << 8) | alpha`.
/// Accepts only [`AlphaByte`] — an `f32` (normalized or authored) cannot reach
/// the packed word without naming its quantization first.
#[inline]
pub const fn pack_rgb_alpha(rgb: u32, alpha: AlphaByte) -> u32 {
    (rgb << 8) | alpha.get() as u32
}

#[cfg(test)]
mod alpha_byte_tests {
    use super::*;

    #[test]
    fn alpha_byte_is_one_byte() {
        assert_eq!(core::mem::size_of::<AlphaByte>(), 1);
    }

    #[test]
    fn authored_preserves_all_u8_values() {
        for value in 0..=u8::MAX {
            assert_eq!(AlphaByte::authored(value).get(), value);
        }
    }

    #[test]
    fn from_normalized_clamps() {
        assert_eq!(AlphaByte::from_normalized(-1.0).get(), 0);
        assert_eq!(AlphaByte::from_normalized(0.0).get(), 0);
        assert_eq!(AlphaByte::from_normalized(1.0).get(), 255);
        assert_eq!(AlphaByte::from_normalized(2.0).get(), 255);
        // An authored BYTE fed to the normalized constructor clamps to 0xFF —
        // the exact bug GOV-003 exists to make impossible for 0x32.
        assert_ne!(AlphaByte::from_normalized(50.0).get(), 0x32);
        assert_eq!(AlphaByte::from_normalized(50.0).get(), 0xFF);
    }

    #[test]
    fn from_normalized_preserves_existing_quantization() {
        // Must stay byte-identical to theme::types::opacity_to_alpha
        // (truncating) for every normalized input the theme produces.
        for step in 0..=1000 {
            let opacity = step as f32 / 1000.0;
            assert_eq!(
                u32::from(AlphaByte::from_normalized(opacity).get()),
                crate::theme::types::opacity_to_alpha(opacity),
                "diverged from opacity_to_alpha at {opacity}"
            );
        }
    }

    #[test]
    fn rounded_quantization_is_distinct_and_explicit() {
        // 0.72 * 255 = 183.6 — truncation and rounding disagree by one byte;
        // the two constructors must NOT be normalized into one algorithm.
        assert_eq!(AlphaByte::from_normalized(0.72).get(), 183);
        assert_eq!(AlphaByte::from_normalized_rounded(0.72).get(), 184);
        assert_eq!(AlphaByte::from_normalized_rounded(-1.0).get(), 0);
        assert_eq!(AlphaByte::from_normalized_rounded(2.0).get(), 255);
    }

    #[test]
    fn from_authored_f32_preserves_the_historical_round_cast() {
        // Historical conversation packer: `(rgb << 8) | alpha.round() as u32`.
        assert_eq!(AlphaByte::from_authored_f32(50.0).get(), 0x32);
        assert_eq!(AlphaByte::from_authored_f32(0x7f as f32).get(), 0x7F);
        assert_eq!(AlphaByte::from_authored_f32(0.0).get(), 0);
        assert_eq!(AlphaByte::from_authored_f32(255.0).get(), 0xFF);
    }

    #[test]
    fn pack_rgb_alpha_preserves_channel_order() {
        assert_eq!(
            pack_rgb_alpha(0xEF4444, AlphaByte::authored(0x32)),
            0xEF4444_32
        );
        assert_eq!(
            pack_rgb_alpha(0xFFFFFF, AlphaByte::authored(0x06)),
            0xFFFFFF_06
        );
        assert_eq!(pack_rgb_alpha(0x000000, AlphaByte::authored(0xFF)), 0xFF);
    }
}
