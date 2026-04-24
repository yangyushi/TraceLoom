# LLM Tracer — Build script for Windows
# Requires: Node.js, npm, Rust, cargo, Tauri CLI
# Usage:
#   .\build.ps1           # Release build
#   .\build.ps1 -Debug    # Debug build
#   .\build.ps1 -Dev      # Run in dev mode
#   .\build.ps1 -Clean    # Remove build artifacts
#   .\build.ps1 -Test     # Run Rust unit tests

param(
    [switch]$Debug,
    [switch]$Dev,
    [switch]$Clean,
    [switch]$Test
)

$ErrorActionPreference = "Stop"

function Remove-BuildArtifacts {
    $paths = @("dist", "src-tauri/target")
    foreach ($p in $paths) {
        if (Test-Path $p) {
            Remove-Item -Recurse -Force $p
            Write-Host "Removed $p"
        }
    }
    Write-Host "Cleaned build artifacts."
}

if ($Clean) {
    Remove-BuildArtifacts
    exit 0
}

if ($Test) {
    cargo test --manifest-path src-tauri/Cargo.toml
    exit $LASTEXITCODE
}

if ($Dev) {
    npx tauri dev
    exit $LASTEXITCODE
}

if ($Debug) {
    npx tauri build --debug
    exit $LASTEXITCODE
}

# Default: release build
npx tauri build
exit $LASTEXITCODE
