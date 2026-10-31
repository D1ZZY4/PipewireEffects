PREFIX ?= /usr/local
DESTDIR ?=
CARGO ?= cargo
INSTALL ?= install
APPLICATION_ID = io.github.D1ZZY4.PipewireEffects

BINDIR = $(DESTDIR)$(PREFIX)/bin
DATADIR = $(DESTDIR)$(PREFIX)/share
APPDIR = $(DATADIR)/applications
ICONDIR = $(DATADIR)/icons/hicolor
PRESETDIR = $(DATADIR)/pipewire-effects/presets
LOCALEDIR = $(DATADIR)/locale
LINGUAS = $(shell grep -vE '^[[:space:]]*(#|$$)' po/LINGUAS)

.PHONY: all build install uninstall check test pot

all: build

build:
	$(CARGO) build --release --locked

check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
	$(CARGO) test --workspace

test: check

pot:
	scripts/update-po.sh

translations:
	@for lang in $(LINGUAS); do \
		msgfmt --check -o /dev/null po/$$lang.po || exit 1; \
	done

install: build translations
	$(INSTALL) -Dm755 target/release/pipewire-effects $(BINDIR)/pipewire-effects
	$(INSTALL) -Dm644 data/$(APPLICATION_ID).desktop $(APPDIR)/$(APPLICATION_ID).desktop
	$(INSTALL) -Dm644 data/icons/hicolor/scalable/apps/$(APPLICATION_ID).svg \
		$(ICONDIR)/scalable/apps/$(APPLICATION_ID).svg
	for size in 16 24 32 48 64 128 256; do \
		$(INSTALL) -d $(ICONDIR)/$${size}x$${size}/apps; \
	done
	for icon in data/icons/hicolor/symbolic/apps/*.svg; do \
		$(INSTALL) -Dm644 $$icon $(ICONDIR)/symbolic/apps/$$(basename $$icon); \
	done
	@for entry in \
		"Default:default" \
		"Podcast voice:podcast-voice" \
		"Music, speakers:music-speakers" \
		"Headphones:headphones" \
		"Bass boost:bass-boost" \
		"Clean microphone:clean-microphone"; do \
		name=$${entry%%:*}; \
		file=$${entry##*:}; \
		$(INSTALL) -Dm644 data/presets/$$file.json "$(PRESETDIR)/$$name.json"; \
	done
	for lang in $(LINGUAS); do \
		$(INSTALL) -d $(LOCALEDIR)/$$lang/LC_MESSAGES; \
		msgfmt -o $(LOCALEDIR)/$$lang/LC_MESSAGES/pipewire-effects.mo po/$$lang.po; \
	done
	@echo "Installed. Route an application to pipewire-effects-sink, or make it the default sink."

uninstall:
	rm -f $(BINDIR)/pipewire-effects
	rm -f $(APPDIR)/$(APPLICATION_ID).desktop
	rm -f $(ICONDIR)/scalable/apps/$(APPLICATION_ID).svg
	rm -f $(ICONDIR)/symbolic/apps/pipewire-effects-*.svg
	@for name in "Default" "Podcast voice" "Music, speakers" "Headphones" "Bass boost" "Clean microphone"; do \
		rm -f "$(PRESETDIR)/$$name.json"; \
	done
	for lang in $(LINGUAS); do \
		rm -f $(LOCALEDIR)/$$lang/LC_MESSAGES/pipewire-effects.mo; \
	done
