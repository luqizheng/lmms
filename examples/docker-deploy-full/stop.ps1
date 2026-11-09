#!/usr/bin/env pwsh
# stop.ps1 —— 停止全栈部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./stop.ps1              # down（保留 named volumes）
#   ./stop.ps1 -Volumes     # 连 named volume 一起删（清库/清监控数据）
#   ./stop.ps1 -Help
#
# 定位依据是 project 名，与启动时用的 overlay 片 / -Logs 无关。
# 本单元的 project 名是 superpowers-full（与 docker-deploy 的 superpowers-svc 区分）。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Volumes,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$Dir     = $PSScriptRoot
$Project = "superpowers-full"

Set-Location $Dir

$args = @()
if ($Volumes) { $args += "-v" }
if ($ExtraArgs) { $args += $ExtraArgs }

Write-Host "[stop:$Project] down..."
& docker compose -p $Project -f docker-compose.yml down @args
