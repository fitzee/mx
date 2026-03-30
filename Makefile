# mx toolchain installer
#
# Usage:
#   make install          Install to ~/.mx (or MX_HOME)
#   make uninstall        Remove installed files
#   make build            Build compiler and tools
#   make clean            cargo clean + mxpkg artifacts

PREFIX ?= $(HOME)/.mx

.PHONY: build install uninstall clean check-deps

build: check-deps
	cargo build --release --workspace
	@echo "Bootstrapping mxpkg..."
	@cd tools/mxpkg && ../../target/release/mxpkg0 build
	@echo "Building m2dap..."
	@cd tools/m2dap && ../../target/release/mx build

check-deps:
	@echo "Checking build prerequisites..."
	@command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found (install Rust: https://rustup.rs)"; exit 1; }
	@command -v cc >/dev/null 2>&1 || { echo "ERROR: cc not found (install Xcode CLT or gcc)"; exit 1; }
	@echo "  cargo: $$(cargo --version)"
	@echo "  cc:    $$(cc --version 2>&1 | head -1)"
	@# ── OpenSSL (required) ──
	@OPENSSL_FOUND=0; \
	if command -v pkg-config >/dev/null 2>&1; then \
		if [ "$$(uname)" = "Darwin" ]; then \
			for p in /opt/homebrew/opt/openssl@3/lib/pkgconfig /usr/local/opt/openssl@3/lib/pkgconfig; do \
				if [ -d "$$p" ]; then \
					export PKG_CONFIG_PATH="$$p:$$PKG_CONFIG_PATH"; \
					break; \
				fi; \
			done; \
		fi; \
		if pkg-config --exists openssl 2>/dev/null; then \
			OPENSSL_FOUND=1; \
			echo "  openssl: $$(pkg-config --modversion openssl) ($$(pkg-config --variable=prefix openssl 2>/dev/null || echo unknown))"; \
		fi; \
	fi; \
	if [ "$$OPENSSL_FOUND" = "0" ]; then \
		if [ "$$(uname)" = "Darwin" ]; then \
			for d in /opt/homebrew/opt/openssl@3 /usr/local/opt/openssl@3; do \
				if [ -f "$$d/include/openssl/ssl.h" ]; then \
					OPENSSL_FOUND=1; \
					echo "  openssl: found ($$d)"; \
					break; \
				fi; \
			done; \
		else \
			if [ -f /usr/include/openssl/ssl.h ] || [ -f /usr/include/x86_64-linux-gnu/openssl/ssl.h ]; then \
				OPENSSL_FOUND=1; \
				echo "  openssl: found (system headers)"; \
			fi; \
		fi; \
	fi; \
	if [ "$$OPENSSL_FOUND" = "0" ]; then \
		echo ""; \
		echo "ERROR: openssl development headers not found"; \
		echo "  Required for: mxpkg, m2http, m2tls, m2auth"; \
		echo ""; \
		if [ "$$(uname)" = "Darwin" ]; then \
			echo "  Install with: brew install openssl@3"; \
		else \
			echo "  Install with: sudo apt install libssl-dev   (Debian/Ubuntu)"; \
			echo "            or: sudo dnf install openssl-devel (Fedora/RHEL)"; \
		fi; \
		exit 1; \
	fi
	@# ── clang for LLVM backend (optional) ──
	@if command -v clang >/dev/null 2>&1; then \
		CLANG_VER=$$(clang --version 2>&1 | head -1 | sed -n 's/.*version \([0-9]*\).*/\1/p'); \
		if [ -n "$$CLANG_VER" ] && [ "$$CLANG_VER" -ge 15 ] 2>/dev/null; then \
			echo "  clang:   $$(clang --version 2>&1 | head -1) (LLVM backend OK)"; \
		else \
			echo "  clang:   $$(clang --version 2>&1 | head -1) (LLVM backend requires clang 15+)"; \
		fi; \
	else \
		echo "  clang:   not found (optional — needed for --llvm backend)"; \
	fi
	@# ── Optional deps ──
	@command -v pkg-config >/dev/null 2>&1 && pkg-config --exists sqlite3 2>/dev/null \
		&& echo "  sqlite3: found" \
		|| echo "  sqlite3: not found (optional — needed for m2sqlite)"
	@command -v pkg-config >/dev/null 2>&1 && pkg-config --exists zlib 2>/dev/null \
		&& echo "  zlib: found" \
		|| echo "  zlib: not found (optional — needed for m2zlib)"
	@echo "All required dependencies present."

install: build
	@echo "Installing mx toolchain to $(PREFIX) ..."
	@mkdir -p "$(PREFIX)/bin"
	@mkdir -p "$(PREFIX)/lib"
	@mkdir -p "$(PREFIX)/docs"
	@# ── Binaries ──
	@cp target/release/mx "$(PREFIX)/bin/mx"
	@cp target/release/mxpkg0 "$(PREFIX)/bin/mxpkg0"
	@if [ -f tools/mxpkg/target/mxpkg ]; then \
		cp tools/mxpkg/target/mxpkg "$(PREFIX)/bin/mxpkg"; \
	fi
	@if [ -f tools/m2dap/.mx/bin/m2dap ]; then \
		cp tools/m2dap/.mx/bin/m2dap "$(PREFIX)/bin/m2dap"; \
	fi
	@# ── m2sys (C runtime shim) ──
	@mkdir -p "$(PREFIX)/lib/m2sys"
	@cp libs/m2sys/m2sys.c "$(PREFIX)/lib/m2sys/m2sys.c"
	@cp libs/m2sys/m2sys.h "$(PREFIX)/lib/m2sys/m2sys.h"
	@cp libs/m2sys/m2.toml "$(PREFIX)/lib/m2sys/m2.toml"
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
	@# ── Documentation (preserves libs/ structure for LSP discovery) ──
	@if [ -d docs ]; then \
		cp -R docs/* "$(PREFIX)/docs/" 2>/dev/null || true; \
	fi
	@echo ""
	@echo "mx toolchain installed to $(PREFIX)"
	@echo ""
	@echo "  mx      — Modula-2 compiler"
	@echo "  mxpkg0  — package manager (bootstrap)"
	@if [ -f "$(PREFIX)/bin/mxpkg" ]; then \
		echo "  mxpkg   — package manager"; \
	fi
	@if [ -f "$(PREFIX)/bin/m2dap" ]; then \
		echo "  m2dap   — debug adapter (DAP server)"; \
	fi
	@echo ""
	@echo "Add to your shell profile:"
	@echo '  export PATH="$(PREFIX)/bin:$$PATH"'
	@echo ""
	@VSIX=$$(ls tools/vscode-m2plus/m2plus-*.vsix 2>/dev/null | head -1); \
	if [ -n "$$VSIX" ]; then \
		echo "Install VS Code extension:"; \
		echo "  code --install-extension $$VSIX"; \
		echo ""; \
		echo "If VS Code can't find mx, set the full path in VS Code settings:"; \
		echo '  "mx.serverPath": "$(PREFIX)/bin/mx"'; \
		echo ""; \
	fi
	@echo "Verify:"
	@echo "  mx --version"
	@echo "  mxpkg version"

uninstall:
	@echo "Removing mx toolchain from $(PREFIX) ..."
	rm -rf "$(PREFIX)"
	@echo "Done."

clean:
	cargo clean
	@rm -rf tools/mxpkg/target
	@rm -rf tools/m2dap/.mx
