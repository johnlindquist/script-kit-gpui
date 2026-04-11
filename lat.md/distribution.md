# Distribution

Script Kit GPUI's current shipping path is a macOS app bundle built with `cargo-bundle`, verified locally with repo scripts, and published from GitHub Actions. The older cross-platform bundling roadmap is not the current durable contract.

## Local bundle path

The repo's explicit macOS bundle path is:

- `cargo build --release --bin script-kit-gpui`
- `cargo bundle --release --bin script-kit-gpui`
- `bash scripts/verify-macos-bundle.sh`

The resulting app lives at `target/release/bundle/osx/Script Kit.app`.

## Bundle metadata

The canonical bundle metadata lives in `Cargo.toml` under `[package.metadata.bundle.bin.script-kit-gpui]`.

That metadata currently defines the app name, bundle identifier, icons, minimum macOS version, URL scheme, bundled resources, and the `LSUIElement`-style agent-app plist extension.

## CI build artifact

The `CI` workflow on pushes to `main` builds the release binary, creates the macOS bundle, verifies bundle contents, ad-hoc signs the app, zips it, and uploads the archive as a short-lived artifact.

That is the current dev-build path. It is useful for download and testing, but it is not the notarized release path.

## Tagged release path

The `Release` workflow runs on `v*` tags and currently does this:

- validates the repo gates with `bash scripts/verify.sh --skip-bundle`
- builds the release binary and macOS bundle
- verifies the bundled app contents
- signs the app with the Developer ID certificate and `entitlements.plist`
- notarizes the zip with Apple's notary service
- staples the notarization ticket
- uploads the final `Script-Kit-macos.zip` to the GitHub release

This is the current production distribution contract.

## Human-only gate

`make ship-check` is the full local ship gate for humans. It runs the full validation path plus bundle sanity checks.

AI agents should not run `make ship-check`; they should use `make verify` or narrower checks unless a human explicitly asks for packaging validation.

## Source files

- [Cargo.toml](../Cargo.toml)
- [Makefile](../Makefile)
- [.github/workflows/ci.yml](../.github/workflows/ci.yml)
- [.github/workflows/release.yml](../.github/workflows/release.yml)
- [scripts/verify-macos-bundle.sh](../scripts/verify-macos-bundle.sh)
- [scripts/verify.sh](../scripts/verify.sh)
