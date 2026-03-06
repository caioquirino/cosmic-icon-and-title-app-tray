PREFIX  ?= $(HOME)/.local
BINDIR   = $(PREFIX)/bin
DATADIR  = $(PREFIX)/share
APPID    = io.github.caioquirino.CosmicWindowList
TARGET   = target/release/cosmic-applet-window-list

# ── Build ──────────────────────────────────────────────────────────────────────

.PHONY: all build check fmt clippy

all: build

build:
	cargo build --release --config 'net.git-fetch-with-cli = true'

## Faster syntax/type check without producing a binary
check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy

# ── Development ────────────────────────────────────────────────────────────────

.PHONY: dev run log

## Debug build + hot-restart the applet (fast iteration loop)
dev:
	cargo build && $(MAKE) restart-applet

## Run standalone (outside the panel, useful for quick visual testing)
run: build
	$(TARGET)

## Run standalone with debug logging
log: build
	RUST_LOG=debug $(TARGET)

# ── Install / Uninstall ────────────────────────────────────────────────────────

.PHONY: install install-restart uninstall

install: build
	install -Dm755 $(TARGET) $(BINDIR)/cosmic-applet-window-list
	sed "s|Exec=cosmic-applet-window-list|Exec=$(BINDIR)/cosmic-applet-window-list|" \
		res/com.system76.CosmicAppletWindowList.desktop > $(APPID).desktop.tmp
	install -Dm644 $(APPID).desktop.tmp $(DATADIR)/applications/$(APPID).desktop
	rm $(APPID).desktop.tmp

## Build, install, then restart the running applet instance
install-restart: install restart-applet

uninstall:
	rm -f $(BINDIR)/cosmic-applet-window-list
	rm -f $(DATADIR)/applications/$(APPID).desktop

# ── Panel / Applet lifecycle ───────────────────────────────────────────────────

.PHONY: restart-applet restart-panel

restart-applet:
	pkill -f cosmic-applet-window-list || true

## Full panel restart (use when the applet is not responding to restart-applet)
restart-panel:
	pkill cosmic-panel || true

# ── Misc ───────────────────────────────────────────────────────────────────────

.PHONY: clean

clean:
	cargo clean
