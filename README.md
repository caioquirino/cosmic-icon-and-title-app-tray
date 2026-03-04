# COSMIC Window List Applet

A COSMIC applet that shows both window icons and titles in the panel.

## Features
- Shows icons and titles for all open windows.
- Automatically updates when windows are opened, closed, or renamed.
- Built using `libcosmic` and Wayland protocols.

## Prerequisites
- Rust and Cargo.
- `libcosmic` dependencies (e.g., `libxkbcommon-dev`, `wayland-client`).

## Building
```bash
cargo build --release
```

## Running
To test it as a standalone applet:
```bash
./target/release/cosmic-applet-window-list
```

## Installation
To integrate it with COSMIC Panel:
1. Copy the binary to `/usr/local/bin/` or `~/.local/bin/`.
2. Copy `res/com.system76.CosmicAppletWindowList.desktop` to `~/.local/share/applications/`.
3. Add the applet to your panel using COSMIC Settings.
