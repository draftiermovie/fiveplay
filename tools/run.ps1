param([switch]$SmokeTest)
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if ($SmokeTest) { cargo run --locked -- --smoke-test }
    else { cargo run --locked --release }
    if ($LASTEXITCODE -ne 0) { throw "FivePlay exited with code $LASTEXITCODE" }
} finally { Pop-Location }
