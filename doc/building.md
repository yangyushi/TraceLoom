# Building LLM Tracer

This document explains how to build the LLM Tracer desktop application from source on Linux, macOS, and Windows.

## Prerequisites

### All Platforms

| Tool     | Version | Purpose                          | Install Link                                      |
|----------|---------|----------------------------------|---------------------------------------------------|
| Node.js  | >= 18   | Frontend toolchain (Vite, Svelte)| https://nodejs.org/                               |
| npm      | >= 9    | Package manager                  | Bundled with Node.js                              |
| Rust     | >= 1.70 | Backend / Tauri runtime          | https://rustup.rs/                                |
| cargo    | latest  | Rust package manager             | Bundled with Rustup                               |

### Platform-Specific System Dependencies

#### Linux (Debian/Ubuntu)

```bash
sudo apt update
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

On older distributions the package may be `libwebkit2gtk-4.0-dev` instead of `4.1`.

#### Linux (Fedora / RHEL)

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf
```

#### macOS

No extra system packages are required. Xcode Command Line Tools are sufficient:

```bash
xcode-select --install
```

#### Windows

Install the **Microsoft C++ Build Tools** (Visual Studio Build Tools) with the **Desktop development with C++** workload.

Download: https://visualstudio.microsoft.com/visual-cpp-build-tools/

You also need **WebView2 Runtime** (usually pre-installed on Windows 10/11).

## Quick Start

### 1. Clone the repository

```bash
git clone <repo-url>
cd llm-tracer
```

### 2. Install frontend dependencies

```bash
npm install
```

### 3. Build and run

#### Linux / macOS

```bash
make dev      # Development mode with hot reload
make          # Release build (creates .deb / .AppImage / .dmg / .app)
make debug    # Debug build (faster, no installer bundling)
make test     # Run Rust unit tests
make clean    # Remove all build artifacts
```

#### Windows

```powershell
# Development mode
.\build.ps1 -Dev

# Release build
.\build.ps1

# Debug build
.\build.ps1 -Debug

# Run tests
.\build.ps1 -Test

# Clean artifacts
.\build.ps1 -Clean
```

## Build Outputs

After a successful release build (`make` or `.\build.ps1`), the native bundles are placed in:

```
src-tauri/target/release/bundle/
```

| Platform | Bundle formats                          |
|----------|-----------------------------------------|
| Linux    | `.deb`, `.rpm`, `.AppImage`             |
| macOS    | `.dmg`, `.app`                          |
| Windows  | `.msi`, `.nsis` (installer)             |

The standalone binary (without installer) is also available at:

```
src-tauri/target/release/llm-tracer        # Linux / macOS
src-tauri/target/release/llm-tracer.exe    # Windows
```

## Development Mode

Running `make dev` (Linux/macOS) or `.\build.ps1 -Dev` (Windows) starts:

1. **Vite dev server** on `http://localhost:5173` — serves the Svelte frontend with hot-module replacement.
2. **Tauri dev process** — compiles the Rust backend in debug mode and injects the Tauri API into the WebView.

The app window opens automatically. Frontend changes are reflected instantly; Rust changes trigger a recompile.

## Running Tests

### Rust backend tests

```bash
# Linux / macOS
make test

# Windows
.\build.ps1 -Test

# Or directly
cargo test --manifest-path src-tauri/Cargo.toml
```

The test suite includes:
- IR construction and serialization round-trips
- DAG validation (cycles, temporal violations, orphans)
- Source format detection (Claude, Codex, OpenClaw)
- Parser correctness against all sample files in `test/samples/`

## Troubleshooting

### Blank page / "Could not connect to localhost"

**Cause:** The production app window was defaulting to the development server URL instead of the bundled frontend assets.

**Fix:** Ensure `tauri.conf.json` contains an explicit `url` field in the window definition:

```json
"app": {
  "windows": [
    {
      "label": "main",
      "url": "index.html",
      ...
    }
  ]
}
```

This is already fixed in the repository. If you still see the issue, rebuild with `make clean && make`.

### `error while running tauri application: PluginInitialization("dialog", ...)`

**Cause:** The `plugins.dialog` entry in `tauri.conf.json` was an empty object `{}`, but the plugin expects no config (unit type).

**Fix:** Remove the `plugins` block entirely from `tauri.conf.json` if the plugin is initialized in Rust code (which it is). This is already fixed.

### Missing icons during build

**Cause:** The `icons/` directory must be located at `src-tauri/icons/`, not nested inside `src-tauri/src-tauri/icons/`.

**Fix:** Verify the directory structure matches:

```
src-tauri/
  icons/
    icon.png
  src/
  Cargo.toml
  tauri.conf.json
```

### WebKit errors on Linux

If you see errors about `org.a11y.Bus` or accessibility, they are harmless warnings and do not affect functionality. To suppress them:

```bash
export NO_AT_BRIDGE=1
./src-tauri/target/release/llm-tracer
```

### Very slow first build

Tauri compiles WebKitGTK on Linux, which can take several minutes on the first run. Subsequent builds are incremental and much faster. Use `make debug` during development to avoid the full release optimization pass.
