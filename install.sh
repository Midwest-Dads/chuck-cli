#!/bin/bash

# Chuck Installation Script
# Builds and installs Chuck to /usr/local/bin

set -e

echo "🧔 Chuck Installation Script"
echo "Building Chuck from source..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check if GitHub CLI is installed
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI is not installed. Please install it first:"
    echo "   brew install gh"
    echo "   # or visit: https://cli.github.com/"
    exit 1
fi

# Build Chuck
echo "🔨 Building Chuck..."
cargo build --release

# Install to /usr/local/bin
echo "📦 Installing Chuck to /usr/local/bin..."
sudo cp target/release/chuck /usr/local/bin/

# Verify installation
if command -v chuck &> /dev/null; then
    echo "✅ Chuck installed successfully!"
    echo "🧔 Run 'chuck --help' to get started"
    chuck --version
else
    echo "❌ Installation failed. Please check your PATH."
    exit 1
fi
