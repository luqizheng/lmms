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
