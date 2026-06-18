# Build script for Supply Chain Provenance contract
$ErrorActionPreference = "Stop"

Write-Host "Building Supply Chain Provenance contract..." -ForegroundColor Green
Write-Host ""

# Step 1: Check prerequisites
$hasRust = $null -ne (Get-Command "rustc" -ErrorAction SilentlyContinue)
$hasWasmTarget = rustup target list --installed 2>$null | Select-String "wasm32v1-none"

if (-not $hasRust) {
    Write-Host "Error: Rust is not installed." -ForegroundColor Red
    Write-Host "Install it from: https://rustup.rs"
    exit 1
}

if (-not $hasWasmTarget) {
    Write-Host "Installing wasm32-unknown-unknown target..." -ForegroundColor Yellow
    rustup target add wasm32v1-none
}

# Step 2: Run tests
Write-Host "Running tests..." -ForegroundColor Cyan
cargo test
if ($LASTEXITCODE -ne 0) {
    Write-Host "Tests failed!" -ForegroundColor Red
    exit 1
}
Write-Host "All tests passed!" -ForegroundColor Green
Write-Host ""

# Step 3: Build contract
Write-Host "Building contract (release)..." -ForegroundColor Cyan
cargo build --target wasm32v1-none --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

$wasmPath = "target\wasm32v1-none\release\provenance.wasm"
if (Test-Path $wasmPath) {
    $size = (Get-Item $wasmPath).Length
    Write-Host "Build successful!" -ForegroundColor Green
    Write-Host "WASM file: $wasmPath ($([math]::Round($size / 1024, 1)) KB)" -ForegroundColor Green
} else {
    Write-Host "WASM file not found at expected path: $wasmPath" -ForegroundColor Red
}
