# infrastructure/lmms-pg-dev/start.ps1 —— lmms 业务服务组（PostgreSQL）本机开发启动入口
# ─────────────────────────────────────────────────────────────────────────────
# 前置：先启动 ../infra-pg（consul / postgres / redis / rabbitmq 都在那里）：
#     Push-Location ../infra-pg; .\start.ps1 -Dev; Pop-Location
#
# 用法：
#   .\start.ps1              # 叠加 dev 片（发布各服务调试端口）
#   .\start.ps1 -NoOverlay   # 只起基座（仅 api-gateway:3000 对外）
#   .\start.ps1 --build      # 其余未识别参数透传 docker compose up
#
# 环境变量：仅本目录 .env（cp .env.example .env），不向上回退。
# ─────────────────────────────────────────────────────────────────────────────
param(
    [switch]$NoOverlay,
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Remaining = @()
)
$ErrorActionPreference = "Stop"

$Project = "lmms-pg-dev"
$Base    = "docker-compose-lmms"
$Dir     = Split-Path -Parent $MyInvocation.MyCommand.Path

function Log($msg)   { Write-Host "[start:$Project] $msg" }
function Die($msg, $code=10) { Write-Host "[start:$Project] FATAL: $msg" -ForegroundColor Red; exit $code }

if ($Help) {
    Get-Content $MyInvocation.MyCommand.Path | Where-Object { $_ -match '^#' } | ForEach-Object { $_ -replace '^# ?','' }
    exit 0
}

# ---------- 文件存在性 ----------
if (-not (Test-Path "$Dir\$Base.yml"))              { Die "$Base.yml not found next to start.ps1" }
if (-not (Test-Path "$Dir\$Base.dev.yml"))          { Die "overlay $Base.dev.yml not found" }
if (-not (Test-Path "$Dir\.env"))                   { Die ".env not found (copy .env.example to .env)" }

# ---------- env 加载（本目录 .env 必需） ----------
function Load-EnvFile([string]$path) {
    if (-not (Test-Path $path)) { return }
    Get-Content $path | ForEach-Object {
        $line = $_.Trim()
        if (-not $line -or $line.StartsWith('#')) { return }
        $eq = $line.IndexOf('=')
        if ($eq -lt 1) { return }
        $key = $line.Substring(0, $eq).Trim()
        $val = $line.Substring($eq + 1).Trim()
        if (($val.StartsWith('"') -and $val.EndsWith('"')) -or
            ($val.StartsWith("'") -and $val.EndsWith("'"))) {
            $val = $val.Substring(1, $val.Length - 2)
        }
        [System.Environment]::SetEnvironmentVariable($key, $val, "Process")
    }
    Log "env: $path"
}

Load-EnvFile "$Dir\.env"

# ---------- REQUIRED 校验 ----------
$required = @("DATABASE_URL","RABBITMQ_DEFAULT_PASS",
              "IAM_DEV_BT","TOKEN_DEV_BT","NOTIFICATION_DEV_BT","AUDIT_DEV_BT",
              "PRUE_DEV_BT","GATEWAY_DEV_BT","FILE_DEV_BT")
foreach ($v in $required) {
    $val = [System.Environment]::GetEnvironmentVariable($v, "Process")
    if (-not $val) { Die "env var $v empty (set in .env)" }
    if ($val -eq "__SET_BY_OPS__") { Die "env var $v still __SET_BY_OPS__ placeholder" }
}

# ---------- 前置：infra 必须已启动 ----------
$netOk = (& docker network ls --format '{{.Name}}') | Where-Object { $_ -eq 'superpowers' }
if (-not $netOk) { Die "docker network 'superpowers' 不存在——请先启动 ../infra-pg" }

$consulOk = (& docker ps --filter "label=com.docker.compose.service=consul" --filter "status=running" -q)
if (-not $consulOk) { Die "consul 未运行——请先启动 ../infra-pg（.\start.ps1 -Dev）" }

$pgOk = (& docker ps --filter "label=com.docker.compose.service=postgres" --filter "status=running" -q)
if (-not $pgOk) { Die "postgres 未运行——请先启动 ../infra-pg（.\start.ps1 -Dev）" }

# ---------- up ----------
Push-Location $Dir
try {
    $args = @("-p", $Project, "-f", "$Base.yml")
    if (-not $NoOverlay) { $args += @("-f", "$Base.dev.yml") }
    $args += @("up", "-d")
    foreach ($r in $Remaining) { $args += $r }
    Log "up (overlay: $(if ($NoOverlay) { 'none' } else { 'dev' }))..."
    & docker compose @args
    if ($LASTEXITCODE -ne 0) { Die "up failed" 20 }
} finally {
    Pop-Location
}

Log "running — api-gateway :3000 (health: curl http://127.0.0.1:3000/health)"
if (-not $NoOverlay) {
    Log "debug ports — token:8082 iam:9081 ntf:8083 audit:8084 file:8086 prue:8087"
}
