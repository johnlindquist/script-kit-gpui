// Build script for script-kit-gpui
// This ensures the binary is rebuilt when the SDK changes

fn main() {
    // Ensure rebuild when SDK changes
    println!("cargo:rerun-if-changed=scripts/kit-sdk.ts");
}
