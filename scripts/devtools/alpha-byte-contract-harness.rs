//! Dependency-free harness for the actual production AlphaByte module.
//!
//! The production module's existing tests compare against the canonical theme
//! opacity conversion. Supplying that one pure arithmetic dependency locally
//! avoids linking GPUI or compiling the application while still running the
//! unmodified tests from `src/theme/alpha.rs`.

mod theme {
    pub mod types {
        pub fn opacity_to_alpha(opacity: f32) -> u32 {
            (opacity.clamp(0.0, 1.0) * 255.0) as u32
        }
    }
}

#[path = "../../src/theme/alpha.rs"]
mod production_alpha;

pub use production_alpha::{pack_rgb_alpha, AlphaByte};

#[cfg(test)]
mod isolated_contract_tests {
    use super::{pack_rgb_alpha, AlphaByte};

    #[test]
    fn authored_byte_cannot_be_confused_with_a_normalized_opacity() {
        assert_eq!(AlphaByte::authored(50).get(), 50);
        assert_eq!(AlphaByte::from_normalized(50.0).get(), 255);
        assert_ne!(AlphaByte::authored(50), AlphaByte::from_normalized(50.0));
    }

    #[test]
    fn canonical_packer_accepts_only_the_explicit_byte_domain() {
        assert_eq!(pack_rgb_alpha(0xEF4444, AlphaByte::authored(0x32)), 0xEF444432);
    }
}
