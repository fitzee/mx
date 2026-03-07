# m2c toolchain installer
#
# Usage:
#   make install          Install to ~/.m2c (or M2C_HOME)
#   make uninstall        Remove installed files
#   make build            Build compiler and tools
#   make clean            cargo clean
#   make check-deps       Verify build prerequisites

PREFIX ?= $(HOME)/.m2c

.PHONY: build install uninstall clean check-deps

build:
	cargo build --release

check-deps:
	@echo "Checking build prerequisites..."
	@command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found (install Rust: https://rustup.rs)"; exit 1; }
	@command -v cc >/dev/null 2>&1 || { echo "ERROR: cc not found (install Xcode CLT or gcc)"; exit 1; }
	@echo "  cargo: $$(cargo --version)"
	@echo "  cc:    $$(cc --version 2>&1 | head -1)"
	@command -v pkg-config >/dev/null 2>&1 && pkg-config --exists openssl 2>/dev/null \
		&& echo "  openssl: found" \
		|| echo "  openssl: not found (optional — needed for m2http, m2tls, m2auth)"
	@command -v pkg-config >/dev/null 2>&1 && pkg-config --exists sqlite3 2>/dev/null \
		&& echo "  sqlite3: found" \
		|| echo "  sqlite3: not found (optional — needed for m2sqlite)"
	@command -v pkg-config >/dev/null 2>&1 && pkg-config --exists zlib 2>/dev/null \
		&& echo "  zlib: found" \
		|| echo "  zlib: not found (optional — needed for m2zlib)"
	@echo "All required dependencies present."

install: build
	@echo "Installing m2c toolchain to $(PREFIX) ..."
	@mkdir -p "$(PREFIX)/bin"
	@mkdir -p "$(PREFIX)/lib"
	@mkdir -p "$(PREFIX)/doc"
	@# ── Binaries ──
	@cp target/release/m2c "$(PREFIX)/bin/m2c"
	@if [ -f target/release/m2pkg0 ]; then \
		cp target/release/m2pkg0 "$(PREFIX)/bin/m2pkg0"; \
	fi
	@cp tools/m2build "$(PREFIX)/bin/m2build"
	@chmod +x "$(PREFIX)/bin/m2build"
	@# ── m2sys (C runtime shim) ──
	@mkdir -p "$(PREFIX)/lib/m2sys"
	@cp libs/m2sys/m2sys.c "$(PREFIX)/lib/m2sys/m2sys.c"
	@cp libs/m2sys/m2sys.h "$(PREFIX)/lib/m2sys/m2sys.h"
	@# ── Libraries ──
	@for libdir in libs/m2*/; do \
		name=$$(basename "$$libdir"); \
		[ "$$name" = "m2sys" ] && continue; \
		[ ! -d "$$libdir/src" ] && continue; \
		dest="$(PREFIX)/lib/$$name/src"; \
		mkdir -p "$$dest"; \
		if [ -f "$$libdir/m2.toml" ]; then \
			cp "$$libdir/m2.toml" "$(PREFIX)/lib/$$name/m2.toml"; \
		fi; \
		for f in "$$libdir"/src/*.def "$$libdir"/src/*.mod "$$libdir"/src/*.c "$$libdir"/src/*.h; do \
			[ -f "$$f" ] && cp "$$f" "$$dest/"; \
		done; \
	done
	@# ── macOS SDK ──
	@if [ -d sdk/macos/v1 ]; then \
		mkdir -p "$(PREFIX)/sdk/macos"; \
		cp -R sdk/macos/v1 "$(PREFIX)/sdk/macos/v1"; \
	fi
	@# ── Documentation ──
	@if [ -d docs ]; then \
		cp -R docs/* "$(PREFIX)/doc/" 2>/dev/null || true; \
	fi
	@echo ""
	@echo "m2c toolchain installed to $(PREFIX)"
	@echo ""
	@echo "Add to your shell profile:"
	@echo '  export PATH="$(PREFIX)/bin:$$PATH"'
	@echo ""
	@if [ -f tools/vscode-m2plus/m2plus-0.1.0.vsix ]; then \
		echo "Install VS Code extension:"; \
		echo "  code --install-extension tools/vscode-m2plus/m2plus-0.1.0.vsix"; \
		echo ""; \
	fi
	@echo "Verify:"
	@echo "  m2c --version"

uninstall:
	@echo "Removing m2c toolchain from $(PREFIX) ..."
	rm -rf "$(PREFIX)"
	@echo "Done."

clean:
	cargo clean
