#!/usr/bin/env pwsh
# start.ps1 —— 本目录 = 自包含部署单元（模式 C，services-only）
# ─────────────────────────────────────────────────────────────────────────────
# 用法：
#   ./start.ps1             # 默认 prod 片；自动检测：仓内 → --build，远程 → --no-build
#   ./start.ps1 -Remote     # 跨主机片 + 自动 --no-build
#   ./start.ps1 -Build      # 强制 build（覆盖自动检测）
#   ./start.ps1 -NoBuild    # 强制跳过 build（覆盖自动检测）
#   ./start.ps1 -ForceRecreate  # 透传其余 docker compose up 参数
#
# env 加载（后者覆盖前者，仅同目录）：
#   1. .env      —— 环境配置（IP / 口令 / 拓扑）
#   2. .env.prod —— 运维覆盖层（可选：服务器真实口令；缺失不阻塞）
#   3. .image.env —— **镜像钉版**（发布即固化，机器生成），最后加载 → 权威
# compose 的 image: 只引用 ${IMAGE_*}，值来自 .image.env；
# 缺变量时 compose 直接报错，本脚本也会在启动前拦下。
# ─────────────────────────────────────────────────────────────────────────────
[CmdletBinding()]
param(
    [switch]$Remote,
    [switch]$Prod,
    [switch]$Build,
    [switch]$NoBuild,
    [switch]$ForceRecreate,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ExtraArgs
)

$ErrorActionPreference = "Stop"

$Dir     = $PSScriptRoot
$Project = "superpowers-svc"
$Base    = "docker-compose-services"
$Overlay = if ($Remote) { "remote" } else { "prod" }

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

# ── 镜像钉版文件（.image.env）必须在 ─────────────────────────────────────────
$ImageEnv = "$Dir/.image.env"
if (-not (Test-Path $ImageEnv)) {
    Write-Fatal ".image.env not found in $Dir (generate: node scripts/update-image-env.js -v <ver>)"
}

# ── 自动 build / image 模式检测 ──────────────────────────────────────────────
# 判断依据：build.context 指向的服务 Dockerfile 是否存在。
# 仓内开发：$Dir/../../services/file-service/Dockerfile 存在 → --build
# 远程部署：该路径不存在（目录被拷贝出仓） → --no-build（用 registry 镜像）
if ($Build -and $NoBuild) {
    Write-Fatal "-Build and -NoBuild are mutually exclusive"
}
if (-not $Build -and -not $NoBuild) {
    $repoDockerfile = Join-Path $Dir "../../services/file-service/Dockerfile"
    if (Test-Path $repoDockerfile) { $Build = $true } else { $NoBuild = $true }
}
$BuildArg = if ($Build) { "--build" } elseif ($NoBuild) { "--no-build" } else { "" }
Write-Log "mode: $BuildArg (overlay: $Overlay)"

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

# 1) 环境配置
Load-EnvFile "$Dir/.env"
# 2) 运维覆盖层（可选）
Load-EnvFile "$Dir/.env.prod"
# 3) 镜像钉版（最后加载 → 权威，.env 不能改掉已发布版本）
Load-EnvFile $ImageEnv

# ── REQUIRED 环境变量校验 ────────────────────────────────────────────────────
$requiredEnv = @(
    "DATABASE_URL", "RABBITMQ_DEFAULT_PASS",
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
    "IMAGE_PRUE_BACKEND"
)
foreach ($v in $requiredImage) {
    if (-not (Test-Path "env:$v")) { Write-Fatal "env var $v empty (set in .image.env)" }
}

# ── 组装 docker compose 参数 ────────────────────────────────────────────────
# 进程环境已由上面的 Load-EnvFile 注入（优先级高于 --env-file），这里只是兜底：
# 让直接跑 `docker compose ... config` 时也能拿到同样的变量。
$dockerArgs = @("-p", $Project)

foreach ($f in @("$Dir/.env", "$Dir/.env.prod", $ImageEnv)) {
    if (Test-Path $f) { $dockerArgs += "--env-file"; $dockerArgs += $f }
}

# services-only 单元：直接用 $Base.yml + overlay
$dockerArgs += "-f"; $dockerArgs += "$Base.yml"
$dockerArgs += "-f"; $dockerArgs += "$Base.$Overlay.yml"

# ── 启动 ─────────────────────────────────────────────────────────────────────
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
Write-Log "running: $n services — api-gateway :3000 (health: curl http://<host>:3000/health)"
