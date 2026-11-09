#!/usr/bin/env pwsh
# start.ps1 —— 全栈自包含部署单元（infra + observability + services）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./start.ps1             # prod 片（业务端口只留 gateway :3000）；仓内自动 --build
#   ./start.ps1 -Remote     # 跨主机分置片（保留业务端口供 consul 回拨）+ 自动 --no-build
#   ./start.ps1 -Logs       # 叠加日志栈（loki + promtail）
#   ./start.ps1 -Build      # 强制 build（覆盖自动检测）
#   ./start.ps1 -NoBuild    # 强制跳过 build（覆盖自动检测）
#   ./start.ps1 -ForceRecreate  # 透传其余 docker compose up 参数
#
# env 加载（后者覆盖前者，仅同目录）：
#   1. .env       —— 环境配置（IP / 口令 / 拓扑）
#   2. .env.prod  —— 运维覆盖层（可选；缺失不阻塞）
#   3. .image.env —— **镜像钉版**（平台镜像机器生成 + 基础镜像手工维护），最后加载 → 权威
#
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；缺变量直接报错。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Remote,
    [switch]$Prod,
    [switch]$Logs,
    [switch]$Build,
    [switch]$NoBuild,
    [switch]$ForceRecreate,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$Dir      = $PSScriptRoot
$Project  = "superpowers-full"
$Base     = "docker-compose-services"
$Overlay  = if ($Remote) { "remote" } else { "prod" }
$WithLogs = if ($Logs)   { 1 }         else { 0 }

# ── log / die ────────────────────────────────────────────────────────────────
function Write-Log {
    param([string]$Msg)
    Write-Host "[start:$Project] $Msg"
}

function Write-Fatal {
    param([string]$Msg)
    Write-Host "[start:$Project] FATAL: $Msg" -ForegroundColor Red
    exit 10
}

# ── 基础文件校验 ─────────────────────────────────────────────────────────────
if (-not (Test-Path "$Dir/$Base.yml")) {
    Write-Fatal "$Base.yml not found next to start.ps1"
}
if (-not (Test-Path "$Dir/$Base.$Overlay.yml")) {
    Write-Fatal "overlay $Base.$Overlay.yml not found"
}

$ImageEnv = "$Dir/.image.env"
if (-not (Test-Path $ImageEnv)) {
    Write-Fatal ".image.env not found in $Dir (generate: node scripts/update-image-env.js)"
}

# ── 自动 build / image 模式检测 ──────────────────────────────────────────────
if ($Build -and $NoBuild) {
    Write-Fatal "-Build and -NoBuild are mutually exclusive"
}
if (-not $Build -and -not $NoBuild) {
    $repoDockerfile = Join-Path $Dir "../../services/file-service/Dockerfile"
    if (Test-Path $repoDockerfile) { $Build = $true } else { $NoBuild = $true }
}
$BuildArg = if ($Build) { "--build" } elseif ($NoBuild) { "--no-build" } else { "" }
Write-Log "mode: $BuildArg (overlay: $Overlay, logs: $WithLogs)"

# ── env 分层加载 ─────────────────────────────────────────────────────────────
function Load-EnvFile {
    param([string]$File)
    if (-not (Test-Path $File)) { return }
    foreach ($line in (Get-Content $File)) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith("#")) { continue }
        $eq = $trimmed.IndexOf("=")
        if ($eq -lt 1) { continue }
        $key   = $trimmed.Substring(0, $eq).Trim()
        $value = $trimmed.Substring($eq + 1).Trim()
        if (($value.StartsWith('"') -and $value.EndsWith('"')) -or
            ($value.StartsWith("'") -and $value.EndsWith("'"))) {
            $value = $value.Substring(1, $value.Length - 2)
        }
        Set-Item -Path "env:$key" -Value $value
    }
    Write-Log "env: $File"
}

Load-EnvFile "$Dir/.env"
Load-EnvFile "$Dir/.env.prod"
Load-EnvFile $ImageEnv

# ── REQUIRED 校验 ────────────────────────────────────────────────────────────
$requiredEnv = @(
    "DATABASE_URL", "RABBITMQ_DEFAULT_PASS", "POSTGRES_PASSWORD",
    "IAM_DEV_BT", "TOKEN_DEV_BT", "NOTIFICATION_DEV_BT", "AUDIT_DEV_BT",
    "PRUE_DEV_BT", "GATEWAY_DEV_BT", "FILE_DEV_BT"
)
foreach ($v in $requiredEnv) {
    if (-not (Test-Path "env:$v")) { Write-Fatal "env var $v empty (set in .env)" }
    if ((Get-Item "env:$v").Value -eq "__SET_BY_OPS__") {
        Write-Fatal "env var $v still __SET_BY_OPS__ placeholder"
    }
}

$requiredImage = @(
    "IMAGE_API_GATEWAY", "IMAGE_TOKEN_SERVICE", "IMAGE_IAM_SERVICE",
    "IMAGE_NOTIFICATION_SERVICE", "IMAGE_FILE_SERVICE", "IMAGE_AUDIT_SERVICE",
    "IMAGE_PRUE_BACKEND", "IMAGE_CONSUL", "IMAGE_REDIS", "IMAGE_RABBITMQ", "IMAGE_POSTGRES",
    "IMAGE_PROMETHEUS", "IMAGE_ALERTMANAGER", "IMAGE_GRAFANA", "IMAGE_CONSUL_TEMPLATE",
    "IMAGE_OTEL_COLLECTOR", "IMAGE_JAEGER", "IMAGE_NGINX"
)
foreach ($v in $requiredImage) {
    if (-not (Test-Path "env:$v")) { Write-Fatal "env var $v empty (set in .image.env)" }
}
if ($WithLogs -eq 1) {
    foreach ($v in @("IMAGE_LOKI", "IMAGE_PROMTAIL")) {
        if (-not (Test-Path "env:$v")) {
            Write-Fatal "env var $v empty (set in .image.env; needed by -Logs)"
        }
    }
}

# ── 组装完整 docker compose 参数 ────────────────────────────────────────────
$dockerArgs = @("-p", $Project)

# env 文件
foreach ($f in @("$Dir/.env", "$Dir/.env.prod", $ImageEnv)) {
    if (Test-Path $f) { $dockerArgs += "--env-file"; $dockerArgs += $f }
}

# compose 文件链
$dockerArgs += "-f"; $dockerArgs += "docker-compose.yml"
$dockerArgs += "-f"; $dockerArgs += "$Base.$Overlay.yml"
if ($WithLogs -eq 1) {
    $dockerArgs += "-f"; $dockerArgs += "docker-compose-observability.logs.yml"
}

# ── 启动 ─────────────────────────────────────────────────────────────────────
# 创建 network（忽略已存在错误）
docker network create superpowers 2>$null | Out-Null

Set-Location $Dir

Write-Log "up (overlay: $Overlay, $BuildArg)..."

$upArgs = @("up", "-d")
if ($BuildArg) { $upArgs += $BuildArg }
if ($ForceRecreate) { $upArgs += "--force-recreate" }
if ($ExtraArgs) { $upArgs += $ExtraArgs }

& docker @dockerArgs @upArgs
if ($LASTEXITCODE -ne 0) { Write-Fatal "up failed (exit $LASTEXITCODE)" }

$n = (& docker @dockerArgs ps --services 2>$null | Measure-Object -Line).Lines
Write-Log "running: $n services — gateway :3000 | grafana :3001 | prometheus :9090 | jaeger :16686 | consul :8500"
