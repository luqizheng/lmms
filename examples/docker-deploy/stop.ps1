#!/usr/bin/env pwsh
# stop.ps1 —— 停止本目录部署单元（services-only）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./stop.ps1              # down（保留 named volumes）
#   ./stop.ps1 -Volumes     # 连 named volume 一起删
#   ./stop.ps1 -Help       # 显示帮助
#
# 注意：down 的定位依据是 project 名 + service 名，与启动时用的 overlay 片无关。
# start.ps1 用的是 prod 还是 remote，stop.ps1 都能正确停止所有容器。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Volumes,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$Dir     = $PSScriptRoot
$Project = "superpowers-svc"
$Base    = "docker-compose-services"

Set-Location $Dir

$args = @()
if ($Volumes) { $args += "-v" }
if ($ExtraArgs) { $args += $ExtraArgs }

Write-Host "[stop:$Project] down..."
& docker compose -p $Project -f "$Base.yml" down @args
