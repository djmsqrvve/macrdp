.PHONY: help build build-cli build-ui build-ui-rust build-ui-frontend \
       dev dev-cli dev-ui clean clean-cli clean-ui \
       install run-cli run-ui check release release-fast link-cli

# Default target
help:
	@echo "macrdp Makefile"
	@echo ""
	@echo "  Development:"
	@echo "    make install          Install frontend dependencies"
	@echo "    make build-cli        Build CLI"
	@echo "    make dev-ui           Start UI dev mode (Tauri)"
	@echo "    make dev-front        Start frontend dev server only"
	@echo "    make run-cli          Run CLI (port 3389)"
	@echo "    make run-cli-ipc      Run CLI + IPC socket"
	@echo "    make check            Check all builds"
	@echo ""
	@echo "  Release:"
	@echo "    make release          Full release build (CLI+UI, optimized, dmg)"
	@echo "    make release-fast     Fast release build (no optimization, dmg)"
	@echo ""
	@echo "  Cleanup:"
	@echo "    make clean            Clean all build artifacts"
	@echo "    make clean-cli        Clean CLI only"
	@echo "    make clean-ui         Clean UI only"

# === CLI ===

link-cli:
	@ln -sf ../../target/debug/macrdp-server macrdp-ui/src-tauri/macrdp-server
	@mkdir -p macrdp-ui/src-tauri/target/debug
	@ln -sf ../../../../target/debug/macrdp-server macrdp-ui/src-tauri/target/debug/macrdp-server

build-cli: link-cli
	cargo build -p macrdp-server

run-cli:
	cargo run -p macrdp-server

run-cli-ipc:
	cargo run -p macrdp-server -- --ipc-socket /tmp/macrdp.sock

clean-cli:
	cargo clean

# === UI ===

install:
	cd macrdp-ui && npm install

build-ui-rust:
	cargo build --manifest-path macrdp-ui/src-tauri/Cargo.toml

build-ui-front:
	cd macrdp-ui && npm run build

dev-ui: build-cli
	cd macrdp-ui && npm run tauri dev

dev-front:
	cd macrdp-ui && npm run dev

clean-ui:
	rm -rf macrdp-ui/dist macrdp-ui/src-tauri/target

# === Release ===

# Full release: fully optimized build + package dmg
release:
	@echo "=== Full release build (CLI release + UI release) ==="
	MACRDP_CLI_PROFILE=release cd macrdp-ui && npm run tauri build
	@echo ""
	@echo "=== Build complete ==="
	@ls -lh macrdp-ui/src-tauri/target/release/bundle/dmg/*.dmg
	@ls -lh macrdp-ui/src-tauri/target/release/bundle/macos/*.app

# Fast release: disable optimization for faster build, still package dmg
release-fast:
	@echo "=== Fast release build (CLI debug + UI no optimization) ==="
	MACRDP_CLI_PROFILE=debug cd macrdp-ui && npm run tauri build
	@echo ""
	@echo "=== Build complete ==="
	@ls -lh macrdp-ui/src-tauri/target/release/bundle/dmg/*.dmg
	@ls -lh macrdp-ui/src-tauri/target/release/bundle/macos/*.app

# === Global ===

build: build-cli build-ui-front build-ui-rust

clean: clean-cli clean-ui

check:
	@echo "Checking CLI..."
	@cargo build -p macrdp-server 2>&1 | tail -1
	@echo "Checking UI Rust..."
	@cargo build --manifest-path macrdp-ui/src-tauri/Cargo.toml 2>&1 | tail -1
	@echo "Checking UI frontend..."
	@cd macrdp-ui && npm run build 2>&1 | tail -1
	@echo "All checks passed."
