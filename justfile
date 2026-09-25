# TPT EM Design - task runner.
# The pipeline logic lives in build.ps1 (Windows) / build.sh (unix); these
# recipes are thin wrappers so `just` works everywhere.

set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

# Build the free WASM edition and report the gzip payload size.
build:
    pwsh -NoLogo -NoProfile -File build.ps1

# Build the Pro edition: antenna tools + field viewer + projects, trunk bundle, desktop exe.
pro:
    pwsh -NoLogo -NoProfile -File build.ps1 -Pro

# Re-run only wasm-bindgen + copy (after an existing release build).
bindgen:
    pwsh -NoLogo -NoProfile -File build.ps1 -SkipBuild

# Engine validation suite (closed-form + field-solver cross-checks), then the app.
test:
    cargo test -p tpt-em-design-engine
    cargo test -p tpt-em-design-engine --features pro
    cargo check --target wasm32-unknown-unknown -p tpt-app-em-design

# Type-check the wasm lib (free + pro) and the desktop shell.
check:
    cargo check --target wasm32-unknown-unknown -p tpt-app-em-design
    cargo check --target wasm32-unknown-unknown --features pro -p tpt-app-em-design
    cargo check -p tpt-em-design-desktop

# Live-reload standalone dev server (index.html mounts via #tpt-appfront-root).
dev:
    trunk serve

# Remove build outputs.
clean:
    cargo clean
    -rd /s /q dist 2>nul || rm -rf dist
