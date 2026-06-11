# scripts/run-e2e.ps1
# 一键启动 docker 全栈并跑 E2E 测试
# 用法: powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 [-Filter <name>] [-TestName <pattern>] [-NoUp] [-Rebuild] [-E2eDebug]

[CmdletBinding()]
param(
    [switch]$NoUp,
    [string]$Filter,
    [string]$TestName,
    [switch]$Rebuild,
    [switch]$E2eDebug
)

$ErrorActionPreference = 'Stop'
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $repoRoot

Write-Host '== E2E Docker Runner ==' -ForegroundColor Cyan
Write-Host "  NoUp    : $NoUp"
Write-Host "  Filter  : $Filter"
Write-Host "  TestName: $TestName"
Write-Host "  Rebuild : $Rebuild"
Write-Host "  E2eDebug: $E2eDebug"

Write-Host '[1/4] Checking e2e dependencies...' -ForegroundColor Yellow
if (-not (Test-Path 'tests/e2e/node_modules')) {
    Write-Host '  Installing pnpm packages...'
    Push-Location tests/e2e
    try {
        pnpm install
        if ($LASTEXITCODE -ne 0) { throw "pnpm install failed with code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
} else {
    Write-Host '  tests/e2e/node_modules exists, skip'
}

if (-not $NoUp) {
    Write-Host '[2/4] Starting docker compose stack...' -ForegroundColor Yellow
    $composeArgs = @('-f', 'infrastructure/docker-compose.yml', 'up', '-d')
    if ($Rebuild) { $composeArgs += '--build', '--pull', 'never' }
    docker compose @composeArgs
    if ($LASTEXITCODE -ne 0) { throw "docker compose up failed with code $LASTEXITCODE" }

    Write-Host '[2/4] Waiting for API gateway health...' -ForegroundColor Yellow
    $deadline = (Get-Date).AddSeconds(120)
    $healthy = $false
    while ((Get-Date) -lt $deadline) {
        try {
            $r = Invoke-WebRequest 'http://localhost:3000/api/v1/auth/setup-status' -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
            if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) {
                $healthy = $true
                break
            }
        } catch {}
        Start-Sleep -Seconds 2
    }
    if (-not $healthy) { throw 'API gateway not healthy after 120s. Run: docker compose -f infrastructure/docker-compose.yml logs' }
    Write-Host '  Gateway is healthy' -ForegroundColor Green
} else {
    Write-Host '[2/4] Skipped (NoUp)' -ForegroundColor Yellow
}
