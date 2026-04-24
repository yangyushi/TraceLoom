# LLM Tracer — Build orchestration for Linux and macOS
# Requires: Node.js, npm, Rust, cargo, Tauri CLI

.PHONY: all clean dev debug test install

# Default target: release build
all:
	npx tauri build

# Run the app in development mode (hot-reload frontend + Rust)
dev:
	npx tauri dev

# Build debug binary (faster compilation, no bundling)
debug:
	npx tauri build --debug

# Run Rust unit tests
test:
	cargo test --manifest-path src-tauri/Cargo.toml

# Install frontend dependencies
install:
	npm install

# Remove all build artifacts
clean:
	rm -rf dist
	rm -rf src-tauri/target
	rm -rf src-tauri/target/debug
	rm -rf src-tauri/target/release
	rm -rf src-tauri/target/test
	rm -rf src-tauri/target/.rustc_info.json
	rm -rf src-tauri/target/CACHEDIR.TAG
	find src-tauri/target -type d -name "bundle" -exec rm -rf {} + 2>/dev/null || true
	find . -name ".turbo" -type d -exec rm -rf {} + 2>/dev/null || true
	@echo "Cleaned build artifacts."
