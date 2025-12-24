#!/bin/bash

# Dev runner script for script-kit-gpui
# Uses cargo-watch to auto-rebuild on Rust file changes
# Clears screen between rebuilds for clean output

set -e

# Check if cargo-watch is installed
if ! command -v cargo-watch &> /dev/null; then
    echo "❌ cargo-watch is not installed"
    echo ""
    echo "Install it with:"
    echo "  cargo install cargo-watch"
    echo ""
    exit 1
fi

echo "🚀 Starting dev runner with cargo-watch..."
echo "   Watching for changes to .rs files"
echo "   Press Ctrl+C to stop"
echo ""

# Run cargo watch with auto-rebuild
# -x run: Execute 'cargo run' on file changes
# -c: Clear screen between runs for cleaner output
cargo watch -c -x run
