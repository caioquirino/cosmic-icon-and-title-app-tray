# COSMIC Window List Applet

A COSMIC desktop panel applet that shows open window icons and titles in a Windows 11-style taskbar. Built with [`libcosmic`](https://github.com/pop-os/libcosmic) and Wayland protocols.

## Features

- Window icons and titles for all open windows, updated in real time
- Windows 11-style pill button with accent-colored indicator for the active window, gray dot for background windows
- App pinning — pin any app to the tray so it stays visible even when closed
- Per-window context menu: desktop actions (New Window, etc.), window switcher, pin/unpin, Quit
- Workspace filtering — show only the current workspace or all workspaces
- Horizontal and vertical panel support
- High-quality icon rendering (SVG-preferred, correct resolution lookup)
- Persistent configuration via `cosmic-config`

---

## Installation

### From binaries / packages

Pre-built packages are attached to each [GitHub Release](https://github.com/caioquirino/cosmic-icon-and-title-app-tray/releases).

**Arch Linux** — install with the provided PKGBUILD:
```bash
# Download PKGBUILD from the release assets, then:
makepkg -si
```
Or install the pre-built `.pkg.tar.zst` directly:
```bash
sudo pacman -U cosmic-applet-window-list-*.pkg.tar.zst
```

**Debian / Ubuntu / Pop!_OS** — install the `.deb`:
```bash
sudo dpkg -i cosmic-applet-window-list_*.deb
```

**Fedora / openSUSE / RPM-based** — install the `.rpm`:
```bash
sudo rpm -i cosmic-applet-window-list-*.rpm
# or with dnf:
sudo dnf install cosmic-applet-window-list-*.rpm
```

After installing, register the applet's `.desktop` entry with COSMIC Panel and add it through **COSMIC Settings → Panel → Add Applet**.

---

### From source

**Prerequisites** — Rust toolchain via [rustup](https://rustup.rs), plus system libraries:

```bash
# Arch / EndeavourOS
sudo pacman -S wayland libxkbcommon

# Ubuntu / Pop!_OS / Debian
sudo apt install libwayland-dev libxkbcommon-dev
```

**Build and install** to `~/.local` (default):
```bash
make install
```

To install system-wide:
```bash
make install PREFIX=/usr/local
```

After updating a running applet:
```bash
make install-restart   # installs and kills the old process; COSMIC restarts it
```

To remove:
```bash
make uninstall
```

---

## Development Workflow

```bash
make dev        # debug build + restart running applet (fast iteration loop)
make run        # build release and run standalone, outside the panel
make log        # same as run but with RUST_LOG=debug
make check      # fast type-check without linking
make fmt        # cargo fmt
make clippy     # cargo clippy
make clean      # cargo clean
```

---

## Configuration

Configuration is stored by `cosmic-config` under app ID `io.github.caioquirino.CosmicWindowList` (version 1).

| Field | Default | Description |
|---|---|---|
| `show_all_workspaces` | `false` | Show windows from all workspaces, not just the active one |
| `context_menu_text_limit` | `25` | Max characters for context menu item labels |
| `pinned_apps` | `[]` | App IDs pinned to the tray |
| `expand_centered` | `true` | Expand items to fill the panel width, centered |
| `item_max_width` | `300.0` | Maximum width (px) of a single window item |

Pin/unpin and workspace filtering can be changed at runtime via the context menu. The other options currently require editing the config file directly.

---

## Project Structure

```
src/
  main.rs                 # App state, messages, update(), view()
  app_map.rs              # Desktop entry scanning and app_id → icon resolution
  config.rs               # Persistent config struct
  styles.rs               # Button styles and small utilities
  wayland_subscription.rs # Bridges the Wayland handler thread to iced
  wayland_handler.rs      # calloop event loop, cctk toplevel/workspace handling
res/
  com.system76.CosmicAppletWindowList.desktop
```

## License

MIT
