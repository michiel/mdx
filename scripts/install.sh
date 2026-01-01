#!/bin/bash
# MDX installer for Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/michiel/mdx/main/install.sh | bash

set -e

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux*)
        case "$ARCH" in
            x86_64)
                ASSET="mdx-linux-x86_64"
                ;;
            aarch64|arm64)
                ASSET="mdx-linux-aarch64"
                ;;
            *)
                echo "Error: Unsupported architecture $ARCH for Linux"
                exit 1
                ;;
        esac
        ;;
    Darwin*)
        case "$ARCH" in
            x86_64)
                ASSET="mdx-macos-x86_64"
                ;;
            arm64)
                ASSET="mdx-macos-aarch64"
                ;;
            *)
                echo "Error: Unsupported architecture $ARCH for macOS"
                exit 1
                ;;
        esac
        ;;
    *)
        echo "Error: Unsupported OS $OS"
        echo "For Windows, use: iwr -useb https://raw.githubusercontent.com/michiel/mdx/main/install.ps1 | iex"
        exit 1
        ;;
esac

echo "Detected platform: $OS $ARCH"
echo "Downloading $ASSET..."

# Get latest release URL
LATEST_URL="https://api.github.com/repos/michiel/mdx/releases/latest"
RELEASE_JSON=$(curl -fsSL "$LATEST_URL" || true)
DOWNLOAD_URL=$(printf "%s" "$RELEASE_JSON" | grep "browser_download_url.*$ASSET" | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
    if printf "%s" "$RELEASE_JSON" | grep -q "API rate limit exceeded"; then
        echo "Error: GitHub API rate limit exceeded. Try again later or use a GitHub token:"
        echo "  export GITHUB_TOKEN=...   # then re-run the installer"
    elif printf "%s" "$RELEASE_JSON" | grep -q "\"message\""; then
        echo "Error: GitHub API response error:"
        printf "%s\n" "$RELEASE_JSON" | sed -n '1,5p'
    else
        echo "Error: Could not find download URL for $ASSET"
    fi
    exit 1
fi

# Determine install directory
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ -d "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

# Download and install
TEMP_FILE=$(mktemp)
trap 'rm -f "$TEMP_FILE"' EXIT

echo "Downloading from $DOWNLOAD_URL..."
curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_FILE"

echo "Installing to $INSTALL_DIR/mdx..."
chmod +x "$TEMP_FILE"
mv "$TEMP_FILE" "$INSTALL_DIR/mdx"

echo ""
echo "✓ mdx installed successfully to $INSTALL_DIR/mdx"
echo ""

# Check if install dir is in PATH
case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        echo "Run 'mdx --help' to get started"
        ;;
    *)
        echo "Add $INSTALL_DIR to your PATH to use mdx:"
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
        echo "Add this line to your ~/.bashrc, ~/.zshrc, or equivalent"
        ;;
esac
