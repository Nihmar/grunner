#!/bin/bash
set -euo pipefail  # Exit on error, undefined variable, and pipe failures

echo "Building erunner in release mode..."
cargo build --release

# If we reach this point, the build succeeded.
# Create the destination directory if it doesn't exist.
mkdir -p "$HOME/.local/bin/grunner"

# Copy the built binary. Adjust the source path if your project structure differs.
# The typical path after 'cargo build --release' is './target/release/grunner'.
cp "./target/release/grunner" "$HOME/.local/bin/grunner"

echo "erunner installed to ~/.local/bin/grunner"
