# TPT EM Design - build pipeline (Windows).
#
# Free edition (default):  ./build.ps1
#   cargo build --release --target wasm32-unknown-unknown (no pro feature)
#   wasm-bindgen --target web  ->  web repo public/apps/em-design/ when
#   TPT_WEB_REPO is set (glue named em-design.js, wasm keeps its _bg.wasm
#   name - the glue resolves it via import.meta.url), otherwise dist\web.
# Pro desktop edition:     ./build.ps1 -Pro
#   trunk build --release --features pro (its own independent wasm build,
#   into dist\ - NOT the wasm-bindgen path above) + cargo build the native
#   webview exe. Deliberately does not touch public/apps/em-design/: that is
#   the free in-browser demo's bundle, and it must never be built with
#   --features pro, or the site would give away every paid feature (antenna
#   design tools, 2D field visualization, saved project library) for free.
#   The two editions are separate cargo builds writing to separate
#   directories on purpose - do not try to "reuse" one build for both.
#
# Env:
#   TPT_WEB_REPO  path to the tptsolutions.co.nz web repo; the app is copied
#                 to <repo>\public\apps\em-design\ for the hub runner.
#                 Defaults to the known local checkout below if unset.

[CmdletBinding()]
param(
    # Build the Pro desktop bundle (--features pro) instead of the free one.
    # Never writes to the free edition's $Dest - see header comment.
    [switch]$Pro,
    # Free-edition only: skip the cargo build and re-run wasm-bindgen against
    # an existing release build.
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-Location -LiteralPath $PSScriptRoot

$Crate = "tpt-app-em-design"
$Target = "wasm32-unknown-unknown"
$OutName = "em-design"
$Slug = "em-design"
$DefaultWebRepo = "D:\Programming\2 WIP\TPT Electrician\tpt-electrician-nz-2"

Write-Host "== TPT EM Design build ==" -ForegroundColor Cyan

if ($Pro) {
    Write-Host " edition: pro (antenna tools + 2D field viewer + project library)"
    Write-Host "-- trunk build --release --features pro (standalone bundle for the desktop shell)" -ForegroundColor Cyan
    trunk build --release --features pro
    if ($LASTEXITCODE -ne 0) { throw "trunk build failed" }

    Write-Host "-- cargo build --release -p tpt-em-design-desktop (webview exe)" -ForegroundColor Cyan
    cargo build --release -p tpt-em-design-desktop
    if ($LASTEXITCODE -ne 0) { throw "desktop build failed" }
    Write-Host "   exe: target\release\tpt-em-design-pro.exe (serves dist\)" -ForegroundColor Cyan
    Write-Host "-- done" -ForegroundColor Green
    Write-Host "   (public/apps/em-design/ was not touched - run ./build.ps1 with no flags to refresh the free demo)"
    exit 0
}

# Where the browser-loadable free bundle goes.
$WebRepo = if ($env:TPT_WEB_REPO) { $env:TPT_WEB_REPO } elseif (Test-Path $DefaultWebRepo) { $DefaultWebRepo } else { $null }
$Dest = if ($WebRepo) {
    Join-Path $WebRepo "public\apps\em-design"
} else {
    Join-Path $PSScriptRoot "dist\web"
}

Write-Host " edition: free (trace impedance calculator)"
Write-Host " dest:    $Dest"

if (-not $SkipBuild) {
    Write-Host "-- cargo build --release --target $Target" -ForegroundColor Cyan
    cargo build --release --target $Target -p $Crate
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$Wasm = Join-Path $PSScriptRoot "target\$Target\release\tpt_app_em_design.wasm"
if (-not (Test-Path $Wasm)) { throw "missing $Wasm - run the cargo build step first" }

Write-Host "-- wasm-bindgen --target web --out-name $OutName" -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
wasm-bindgen --target web --out-name $OutName --out-dir $Dest $Wasm
if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen failed" }

$Glue = Join-Path $Dest "$OutName.js"
$BgWasm = Join-Path $Dest "$($OutName)_bg.wasm"
foreach ($artifact in @($Glue, $BgWasm)) {
    if (-not (Test-Path $artifact)) { throw "wasm-bindgen did not produce $artifact" }
}

# Payload size report (todo Phase 4: budget ~1.5 MB gzip; closed-form engine,
# no meshing or sparse solver).
$raw = (Get-Item $BgWasm).Length
Add-Type -AssemblyName System.IO.Compression
$rawBytes = [System.IO.File]::ReadAllBytes($BgWasm)
$memoryStream = [System.IO.MemoryStream]::new()
$gzipStream = [System.IO.Compression.GZipStream]::new($memoryStream, [System.IO.Compression.CompressionLevel]::Optimal)
$gzipStream.Write($rawBytes, 0, $rawBytes.Length)
$gzipStream.Dispose()
$gzipped = $memoryStream.ToArray().Length
$memoryStream.Dispose()
$gzKb = [math]::Round($gzipped / 1KB, 1)
Write-Host "-- $(Split-Path $BgWasm -Leaf): $raw bytes raw, $gzipped bytes gzipped ($gzKb KB gz)" -ForegroundColor Cyan
if ($gzipped -gt 1536KB) {
    Write-Warning "payload exceeds the ~1.5 MB gzip budget for the free build"
}

Write-Host "-- done" -ForegroundColor Green
if (-not $WebRepo) {
    Write-Host "   (set TPT_WEB_REPO to copy the bundle into the hub web repo)"
} else {
    Write-Host ""
    Write-Host "Paste into src/lib/tpt-apps-registry.ts (TPT_APPS_REGISTRY), then flip status to 'live':" -ForegroundColor Cyan
    Write-Host @"
  {
    slug: '$Slug',
    name: 'TPT EM Design',
    description: 'PCB trace impedance, antenna design, and 2D field visualization for RF work.',
    category: 'engineering',
    status: 'coming-soon',
    wasm: { entry: '/apps/$Slug/$OutName.js' },
    // gumroadUrl: 'https://gum.co/...',  // add once the Pro exe is listed
  },
"@
}
