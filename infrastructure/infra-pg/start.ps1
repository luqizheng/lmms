# infrastructure/infra-pg/start.ps1 —— 本目录 = 自包含部署单元（模式 C，docs/deploy-topologies.md）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   .\start.ps1             # 默认叠加 prod 片（收口端口、加 restart）
#   .\start.ps1 -Remote     # 跨主机模式：以 remote 片发布宿主端口供他机访问
#   .\start.ps1 -Dev        # 本地开发：以 dev 片发布宿主端口（供宿主进程连）
#
# 环境变量来源：仅本目录下的 .env（必需）与可选的 .env.prod（生产口令覆盖），
# 不再向上回退 infrastructure/.env 或仓库根 .env，确保本部署单元可完全独立。
#
# 其余未识别 args 透传到 docker compose up（如 --force-recreate）。
# ─────────────────────────────────────────────────────────────────────────────
param(
    [switch]$Remote,
    [switch]$Dev,
    [switch]$Prod,
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Remaining = @()
)
$ErrorActionPreference = "Stop"

$Project  = "lmms-infra"
$Base     = "docker-compose"
$Overlay  = if ($Remote) { "remote" } elseif ($Dev) { "dev" } elseif ($Prod) { "prod" } else { "prod" }
$Dir      = Split-Path -Parent $MyInvocation.MyCommand.Path

function Log($msg)   { Write-Host "[start:$Project] $msg" }
function Die($msg, $code=10) { Write-Host "[start:$Project] FATAL: $msg" -ForegroundColor Red; exit $code }

if ($Help) {
    Get-Content $MyInvocation.MyCommand.Path | Where-Object { $_ -match '^#' } | ForEach-Object { $_ -replace '^# ?','' }
    exit 0
}

# ---------- 文件存在性 ----------
if (-not (Test-Path "$Dir\$Base.yml"))      { Die "$Base.yml not found next to start.ps1" }
if (-not (Test-Path "$Dir\$Base.$Overlay.yml")) { Die "overlay $Base.$Overlay.yml not found" }
if (-not (Test-Path "$Dir\.env"))           { Die ".env not found in infra dir (copy .env.example to .env)" }

# ---------- env 加载 ----------
# .env 必需，.env.prod 可选覆盖；与 bash 版 start.sh 顺序一致：后者覆盖前者。
function Load-EnvFile([string]$path) {
    if (-not (Test-Path $path)) { return }
    Get-Content $path | ForEach-Object {
        $line = $_.Trim()
        if (-not $line -or $line.StartsWith('#')) { return }
        $eq = $line.IndexOf('=')
        if ($eq -lt 1) { return }
        $key = $line.Substring(0, $eq).Trim()
        $val = $line.Substring($eq + 1).Trim()
        # 去掉首尾引号（如果有）
        if (($val.StartsWith('"') -and $val.EndsWith('"')) -or
            ($val.StartsWith("'") -and $val.EndsWith("'"))) {
            $val = $val.Substring(1, $val.Length - 2)
        }
        [System.Environment]::SetEnvironmentVariable($key, $val, "Process")
    }
    Log "env: $path"
}

Load-EnvFile "$Dir\.env"
Load-EnvFile "$Dir\.env.prod"

# ---------- REQUIRED 校验（infra-pg） ----------
$required = @("POSTGRES_PASSWORD","RABBITMQ_DEFAULT_PASS")
foreach ($v in $required) {
    $val = [System.Environment]::GetEnvironmentVariable($v, "Process")
    if (-not $val -or $val -eq "") { Die "env var $v empty (set in .env)" }
    if ($val -eq "__SET_BY_OPS__") { Die "env var $v still __SET_BY_OPS__ placeholder" }
}

# ---------- 网络 ----------
try { docker network create superpowers *> $null } catch {}

# ---------- up ----------
Push-Location $Dir
try {
    Log "up (overlay: $Overlay)..."
    $args = @("-p", $Project,
              "-f", "$Base.yml",
              "-f", "$Base.$Overlay.yml",
              "up", "-d")
    foreach ($r in $Remaining) { $args += $r }
    & docker compose @args
    if ($LASTEXITCODE -ne 0) { Die "up failed" 20 }
} finally {
    Pop-Location
}

$n = (& docker compose -p $Project ps --services 2>$null | Measure-Object -Line).Lines
Log "running: $n services — consul :8500 / postgres :5432 / redis :6379 / rabbitmq :5672"
