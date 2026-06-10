# Run Tests Script for coder-micro-services
# Usage: .\scripts\run_tests.ps1 [-Unit] [-Integration] [-All]

param(
    [switch]$Unit,
    [switch]$Integration,
    [switch]$All
)

$ErrorActionPreference = "Continue"
$ProjectRoot = "d:\projects\coder-micro-services"

function Write-TestHeader($service) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host " Testing: $service" -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
}

function Test-RustService($servicePath, $serviceName) {
    Write-TestHeader $serviceName

    Push-Location $servicePath

    Write-Host "[1/2] Running unit tests..." -ForegroundColor Yellow
    cargo test --lib 2>&1 | Select-Object -Last 10

    Write-Host "[2/2] Running integration tests..." -ForegroundColor Yellow
    cargo test --test '*' 2>&1 | Select-Object -Last 10

    Pop-Location
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Green
Write-Host "  coder-micro-services Test Suite" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Green

if ($All -or (-not $Unit -and -not $Integration)) {
    Write-Host "Running ALL tests (unit + integration)..." -ForegroundColor Magenta

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-service" "Token Service (Rust)"

} elseif ($Unit) {
    Write-Host "Running UNIT tests only..." -ForegroundColor Magenta

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-service" "Token Service (Rust)"

} elseif ($Integration) {
    Write-Host "Running INTEGRATION tests only..." -ForegroundColor Magenta
    Write-Host "Note: Ensure Docker Compose services are running:" -ForegroundColor Yellow
    Write-Host "  cd infrastructure; docker compose -f docker-compose.test-infra.yaml up -d" -ForegroundColor Yellow

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-service" "Token Service (Rust)"
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Green
Write-Host "  Test Run Complete" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Green
