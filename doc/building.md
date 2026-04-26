# Building TraceLoom

This document explains how to build TraceLoom from source on Linux, macOS, and Windows.

## Prerequisites

| Tool | Version | Purpose |
| --- | --- | --- |
| Node.js | >= 18 | Frontend toolchain |
| npm | >= 9 | Package manager |
| Rust | >= 1.80 | Backend and Tauri runtime |
| cargo | latest stable | Rust package manager |

## Platform Dependencies

Linux Debian/Ubuntu:

```bash
sudo apt update
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

Linux Fedora/RHEL:

```bash
sudo dnf install gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf
```

macOS requires Xcode Command Line Tools:

```bash
xcode-select --install
```

Windows requires Microsoft C++ Build Tools with the Desktop development with C++ workload. WebView2 Runtime is usually already installed on Windows 10/11.

## Quick Start

```bash
git clone <repo-url>
cd traceloom
npm install
```

Linux/macOS:

```bash
make dev      # Development mode
make          # Release build
make debug    # Debug Tauri build
make test     # Rust unit tests
```

Windows:

```powershell
.\build.ps1 -Dev
.\build.ps1
.\build.ps1 -Debug
.\build.ps1 -Test
```

## Build Outputs

Release bundles are written to:

```text
src-tauri/target/release/bundle/
```

Standalone binaries are written to:

```text
src-tauri/target/release/traceloom        # Linux/macOS
src-tauri/target/release/traceloom.exe    # Windows
```

## Development Checks

Run these before publishing or opening a pull request:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npx svelte-check --tsconfig ./tsconfig.json
npm run build
```

The Rust tests cover IR validation, parser detection, fixture parsing, fail-soft orphan behavior, and content block preservation.

## Tauri ACL Requirements

Tauri v2 commands are denied unless whitelisted. When adding a command:

1. Add a permission entry to `src-tauri/permissions/default.toml`.
2. Add that permission identifier to `src-tauri/capabilities/default.json`.

The app currently whitelists `load_trajectory`, `list_jsonl_files`, and `read_file_text`.

## Generated Schemas

`src-tauri/gen/schemas/*` is tracked intentionally. These files support editor validation for capabilities and permissions. Regenerate them with the Tauri CLI when Tauri configuration schema support changes:

```bash
npx tauri info
```

## Troubleshooting

### Blank Page or Localhost Error

The production window must load bundled frontend assets:

```json
{
  "label": "main",
  "url": "index.html"
}
```

This is configured in `src-tauri/tauri.conf.json`.

### Dialog Plugin Initialization Error

The dialog plugin is initialized in Rust code. Do not add an empty `plugins.dialog` object to `tauri.conf.json`.

### Missing Icons

Icons must live under `src-tauri/icons/`.

### WebKit Warnings on Linux

Warnings about `org.a11y.Bus` are usually harmless. To suppress accessibility bus warnings while running locally:

```bash
export NO_AT_BRIDGE=1
./src-tauri/target/release/traceloom
```
