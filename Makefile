PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin
DATADIR = $(PREFIX)/share
APPID = io.github.caioquirino.CosmicWindowList

TARGET = target/release/cosmic-applet-window-list

.PHONY: all
all: build

.PHONY: build
build:
	cargo build --release --config 'net.git-fetch-with-cli = true'

.PHONY: install
install: build
	install -Dm755 $(TARGET) $(BINDIR)/cosmic-applet-window-list
	sed "s|Exec=cosmic-applet-window-list|Exec=$(BINDIR)/cosmic-applet-window-list|" res/com.system76.CosmicAppletWindowList.desktop > $(APPID).desktop.tmp
	# Also update Name in desktop file to be unique if needed, but for now just the filename
	install -Dm644 $(APPID).desktop.tmp $(DATADIR)/applications/$(APPID).desktop
	rm $(APPID).desktop.tmp

.PHONY: restart-panel
restart-panel:
	pkill cosmic-panel || true

.PHONY: restart-applet
restart-applet:
	pkill -f cosmic-applet-window-list || true

.PHONY: install-restart
install-restart: install restart-applet

.PHONY: uninstall
uninstall:
	rm -f $(BINDIR)/cosmic-applet-window-list
	rm -f $(DATADIR)/applications/$(APPID).desktop

.PHONY: clean
clean:
	cargo clean
