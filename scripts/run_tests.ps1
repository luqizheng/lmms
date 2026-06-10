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

function Test-GoService($servicePath, $serviceName) {
    Write-TestHeader $serviceName

    Push-Location $servicePath

    Write-Host "[1/3] Running unit tests (short)..." -ForegroundColor Yellow
    go test -v -short ./... 2>&1 | Select-Object -Last 20

    Write-Host "[2/3] Running all tests..." -ForegroundColor Yellow
    go test -v ./... 2>&1 | Select-Object -Last 20

    Write-Host "[3/3] Running tests with race detector..." -ForegroundColor Yellow
    go test -race ./... 2>&1 | Select-Object -Last 10

    Pop-Location
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Green
Write-Host "  coder-micro-services Test Suite" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Green

if ($All -or (-not $Unit -and -not $Integration)) {
    Write-Host "Running ALL tests (unit + integration)..." -ForegroundColor Magenta

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-server" "Token Server (Rust)"
    Test-GoService "$ProjectRoot\services\auth-service" "Auth Service (Go)"
    Test-GoService "$ProjectRoot\services\user-service" "User Service (Go)"

} elseif ($Unit) {
    Write-Host "Running UNIT tests only..." -ForegroundColor Magenta

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-server" "Token Server (Rust)"
    Test-GoService "$ProjectRoot\services\auth-service" "Auth Service (Go)"
    Test-GoService "$ProjectRoot\services\user-service" "User Service (Go)"

} elseif ($Integration) {
    Write-Host "Running INTEGRATION tests only..." -ForegroundColor Magenta
    Write-Host "Note: Ensure Docker Compose services are running:" -ForegroundColor Yellow
    Write-Host "  cd infrastructure; docker compose -f docker-compose.test-infra.yaml up -d" -ForegroundColor Yellow

    Test-RustService "$ProjectRoot\services\api-gateway" "API Gateway (Rust)"
    Test-RustService "$ProjectRoot\services\token-server" "Token Server (Rust)"
    Test-GoService "$ProjectRoot\services\auth-service" "Auth Service (Go)"
    Test-GoService "$ProjectRoot\services\user-service" "User Service (Go)"
}

Write-Host ""
Write-Host "===============================================" -ForegroundColor Green
Write-Host "  Test Run Complete" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Green
