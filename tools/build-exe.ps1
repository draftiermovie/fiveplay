# Builds the single-file game executable players need: dist/FivePlay.exe.
#
# Assets are compiled into the binary and the C runtime is linked statically,
# so the .exe runs on a clean Windows PC with no other files.
#
#   ./tools/build-exe.ps1                              # no default server
#   ./tools/build-exe.ps1 -Server play.example.com     # PLAY ONLINE goes there by default
#
# Uses its own target directory so it does not invalidate the dev build cache.
# The first run compiles Bevy from scratch and can take a long time.
param([string]$Server = '')
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    $env:RUSTFLAGS = '-C target-feature=+crt-static --cfg fiveplay_dist'
    if ($Server) { $env:FIVEPLAY_DEFAULT_SERVER = $Server } else { Remove-Item Env:FIVEPLAY_DEFAULT_SERVER -ErrorAction SilentlyContinue }
    # Cargo reports progress on stderr; do not let PowerShell treat it as failure.
    $ErrorActionPreference = 'Continue'
    cargo build --release --locked --target-dir target/dist
    $ErrorActionPreference = 'Stop'
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with code $LASTEXITCODE" }
    New-Item -ItemType Directory -Force dist | Out-Null
    Copy-Item target/dist/release/fiveplay.exe dist/FivePlay.exe -Force
    $size = [math]::Round((Get-Item dist/FivePlay.exe).Length / 1MB, 1)
    Write-Host "Built dist/FivePlay.exe ($size MB). Send players just this file."
} finally {
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    Remove-Item Env:FIVEPLAY_DEFAULT_SERVER -ErrorAction SilentlyContinue
    Pop-Location
}
