# test-orchestrator.ps1
# 入口：Docker 启动、DB 重置、测试运行、报告生成、AI 提示词
#
# 用法：
#   ./scripts/test-orchestrator.ps1 start
#   ./scripts/test-orchestrator.ps1 stop
#   ./scripts/test-orchestrator.ps1 reset-db
#   ./scripts/test-orchestrator.ps1 test-unit [-Service <name>]
#   ./scripts/test-orchestrator.ps1 test-integration [-Service <name>]
#   ./scripts/test-orchestrator.ps1 test-all [-Service <name>]
#   ./scripts/test-orchestrator.ps1 report
#   ./scripts/test-orchestrator.ps1 fix-prompt
#   ./scripts/test-orchestrator.ps1 generate-prompt
#   ./scripts/test-orchestrator.ps1 validate-specs
#   ./scripts/test-orchestrator.ps1 loop

param(
    [Parameter(Mandatory, Position = 0)][string]$Command,
    [string]$Service,
    [string]$Timestamp
)

$ErrorActionPreference = "Stop"
$Root = "d:\projects\coder-micro-services"
$LibDir = Join-Path $Root "scripts\lib"

# dot-source 所有 lib
Get-ChildItem -Path $LibDir -Filter "*.ps1" -File | ForEach-Object {
    . $_.FullName
}

function Get-RunDir {
    param([string]$Ts = $Timestamp)
    if (-not $Ts) { $Ts = Get-Date -Format "yyyyMMdd-HHmmss" }
    Join-Path $Root "reports\run-$Ts"
}

function Initialize-RunDir {
    param([string]$Dir)
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    return $Dir
}

function Run-TestPhase {
    param(
        [Parameter(Mandatory)][ValidateSet("unit", "integration")][string]$Type,
        [string]$Svc
    )
    $runDir = Initialize-RunDir -Dir (Get-RunDir)
    $services = if ($Svc) { @($Svc) } else { Get-ServiceList }
    foreach ($s in $services) {
        $json = Join-Path $runDir "$s.$Type.json"
        $log = Join-Path $runDir "$s.$Type.log"
        Write-Host "[test] $s ($Type) -> $json" -ForegroundColor Cyan
        try {
            Invoke-ServiceTests -Service $s -Type $Type -OutputJson $json 2>&1 | Tee-Object -FilePath $log | Out-Null
        } catch {
            Write-Host "[test] $s ($Type) FAILED to run: $_" -ForegroundColor Red
        }
    }
    return $runDir
}

switch ($Command) {
    "start"            { Start-TestStack }
    "stop"             { Stop-TestStack }
    "reset-db"         { Reset-Database }
    "test-unit"        { Run-TestPhase -Type "unit" -Svc $Service | Out-Null }
    "test-integration" { Run-TestPhase -Type "integration" -Svc $Service | Out-Null }
    "test-all" {
        $runDir = Run-TestPhase -Type "unit" -Svc $Service
        Run-TestPhase -Type "integration" -Svc $Service | Out-Null
        Write-Host "[done] logs: $runDir" -ForegroundColor Green
    }
    "report" {
        $runDir = if ($Timestamp) { Get-RunDir -Ts $Timestamp } else {
            Get-ChildItem -Path (Join-Path $Root "reports") -Directory -Filter "run-*" -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1 | ForEach-Object { $_.FullName }
        }
        if (-not $runDir -or -not (Test-Path $runDir)) { throw "no run dir (specify -Timestamp or run test-all first)" }
        $result = Build-Summary -RunDir $runDir
        Write-Host "[report] $($result.SummaryPath)" -ForegroundColor Green
        if ($result.Failing.Count -gt 0) {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[report] failures detected, fix-prompt: $fix" -ForegroundColor Yellow
            Write-Host "[report] latest: reports\latest-fix-prompt.md" -ForegroundColor Yellow
        }
    }
    "fix-prompt" {
        $runDir = if ($Timestamp) { Get-RunDir -Ts $Timestamp } else {
            Get-ChildItem -Path (Join-Path $Root "reports") -Directory -Filter "run-*" -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1 | ForEach-Object { $_.FullName }
        }
        if (-not $runDir -or -not (Test-Path $runDir)) { throw "no run dir (specify -Timestamp or run test-all first)" }
        $result = Build-Summary -RunDir $runDir
        if ($result.Failing.Count -eq 0) {
            Write-Host "[fix-prompt] no failures in latest run" -ForegroundColor Green
        } else {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[fix-prompt] $fix" -ForegroundColor Yellow
        }
    }
    "generate-prompt"  { Render-SpecToCodePrompt }
    "validate-specs"   { Invoke-SpecValidation }
    "loop" {
        Start-TestStack
        Reset-Database
        $runDir = Run-TestPhase -Type "unit" -Svc $Service
        Run-TestPhase -Type "integration" -Svc $Service | Out-Null
        $result = Build-Summary -RunDir $runDir
        Write-Host "[report] $($result.SummaryPath)" -ForegroundColor Green
        if ($result.Failing.Count -gt 0) {
            $fix = Build-FixPrompt -RunDir $runDir -Failing $result.Failing
            Write-Host "[loop] failures detected." -ForegroundColor Yellow
            Write-Host "[loop] open: $fix" -ForegroundColor Yellow
            Write-Host "[loop] also: reports\latest-fix-prompt.md" -ForegroundColor Yellow
            Write-Host "[loop] (paste to AI; after AI edits, re-run: ./scripts/test-orchestrator.ps1 reset-db && ./scripts/test-orchestrator.ps1 test-integration)" -ForegroundColor Yellow
        } else {
            Write-Host "[loop] all tests passed" -ForegroundColor Green
        }
    }
    default {
        Write-Host "unknown command: $Command" -ForegroundColor Red
        Write-Host "valid: start, stop, reset-db, test-unit, test-integration, test-all, report, fix-prompt, generate-prompt, validate-specs, loop"
        exit 1
    }
}
