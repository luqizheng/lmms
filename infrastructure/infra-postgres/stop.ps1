# infrastructure/infra-pg/stop.ps1 —— 本目录部署单元的停止入口
# 用法：
#   .\stop.ps1              # down（保留 volumes）
#   .\stop.ps1 -Volumes     # 连 named volume 一起删（数据清空，慎用）
# down 的定位依据是 project 名 + 服务清单，与启动时用的 overlay 片无关。
# ─────────────────────────────────────────────────────────────────────────────
param(
    [switch]$Volumes,
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Remaining = @()
)
$ErrorActionPreference = "Stop"

$Project = "lmms-infra"
$Base    = "docker-compose"
$Dir     = Split-Path -Parent $MyInvocation.MyCommand.Path

if ($Help) {
    Get-Content $MyInvocation.MyCommand.Path | Where-Object { $_ -match '^#' } | ForEach-Object { $_ -replace '^# ?','' }
    exit 0
}

Push-Location $Dir
try {
    Write-Host "[stop:$Project] down..."
    $args = @("-p", $Project, "-f", "$Base.yml", "-f", "$Base.prod.yml", "down")
    if ($Volumes) { $args += "--volumes" }
    foreach ($r in $Remaining) { $args += $r }
    & docker compose @args
} finally {
    Pop-Location
}
