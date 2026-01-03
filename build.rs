// Build script for script-kit-gpui
//
// This script tells Cargo to rebuild when key files change.
// SDK deployment to ~/.sk/kit is now handled at runtime by setup::ensure_kit_setup()
// rather than at build time, ensuring the SDK is always in sync with the running binary.

fn main() {
    // Trigger rebuild when SDK source changes (it's embedded via include_str!)
    println!("cargo:rerun-if-changed=scripts/kit-sdk.ts");

    // Trigger rebuild when config template changes (also embedded)
    println!("cargo:rerun-if-changed=scripts/config-template.ts");

    // Trigger rebuild when theme example changes (also embedded)
    println!("cargo:rerun-if-changed=theme.example.json");

    // Trigger rebuild when bundled fonts change (embedded via include_bytes!)
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Regular.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Bold.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Italic.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-BoldItalic.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-Medium.ttf");
    println!("cargo:rerun-if-changed=assets/fonts/JetBrainsMono-SemiBold.ttf");
}
