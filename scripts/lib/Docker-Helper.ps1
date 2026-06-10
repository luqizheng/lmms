# Docker-Helper.ps1
# 封装 docker compose 与 healthcheck 等待

$ErrorActionPreference = "Stop"
$ComposeFile = "d:\projects\coder-micro-services\infrastructure\docker-compose.test-infra.yaml"

function Get-ComposeArgs {
    @("-f", $ComposeFile)
}

function Invoke-PrePull {
    # 预先拉取基础镜像，避免 build 阶段网络抖动
    $images = @(
        "zhcoder-docker-registry.com:8000/official/alpine:3.21",
        "zhcoder-docker-registry.com:8000/official/golang:1.22-alpine",
        "zhcoder-docker-registry.com:8000/official/golang:1.25-alpine",
        "zhcoder-docker-registry.com:8000/official/rust:1.96-slim",
        "zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl",
        "zhcoder-docker-registry.com:8000/official/mysql:8.0",
        "zhcoder-docker-registry.com:8000/official/redis:7-alpine",
        "zhcoder-docker-registry.com:8000/official/consul:1.15"
    )
    foreach ($img in $images) {
        Write-Host "[pull] $img" -ForegroundColor Cyan
        $tmpOut = [System.IO.Path]::GetTempFileName()
        try {
            $p = Start-Process -FilePath "docker" -ArgumentList @("pull", $img) `
                -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err"
            Get-Content $tmpOut -ErrorAction SilentlyContinue | Select-Object -Last 3 | ForEach-Object { Write-Host $_ }
            Get-Content "$tmpOut.err" -ErrorAction SilentlyContinue | Select-Object -Last 3 | ForEach-Object { Write-Host $_ -ForegroundColor DarkGray }
            if ($p.ExitCode -ne 0) {
                Write-Host "[pull] WARNING: failed to pull $img (will retry during build)" -ForegroundColor Yellow
            }
        } finally {
            Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
            if (Test-Path "$tmpOut.err") { Remove-Item "$tmpOut.err" -Force -ErrorAction SilentlyContinue }
        }
    }
}

function Invoke-Compose {
    param([string[]]$ComposeArgs)
    $tmpOut = [System.IO.Path]::GetTempFileName()
    $tmpErr = "$tmpOut.err"
    $allArgs = @("compose") + $ComposeArgs
    try {
        $p = Start-Process -FilePath "docker" -ArgumentList $allArgs `
            -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError $tmpErr
        if (Test-Path $tmpErr) {
            Get-Content $tmpErr -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
            Remove-Item $tmpErr -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path $tmpOut) {
            Get-Content $tmpOut | ForEach-Object { Write-Host $_ }
        }
        return $p.ExitCode
    } finally {
        Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
        if (Test-Path $tmpErr) { Remove-Item $tmpErr -Force -ErrorAction SilentlyContinue }
    }
}

function Start-TestStack {
    Write-Host "[docker] pre-pulling base images..." -ForegroundColor Cyan
    Invoke-PrePull
    Write-Host "[docker] starting test stack (background)..." -ForegroundColor Cyan
    $tmpOut = [System.IO.Path]::GetTempFileName()
    $tmpErr = "$tmpOut.err"
    $logPath = "d:\projects\coder-micro-services\reports\docker-up.log"
    $allArgs = @("compose") + (Get-ComposeArgs) + @("up", "-d")
    # 后台启动 docker compose，避免 IDE 终端超时
    $p = Start-Process -FilePath "docker" -ArgumentList $allArgs `
        -NoNewWindow -PassThru -RedirectStandardOutput $logPath -RedirectStandardError "$logPath.err"
    Write-Host "[docker] PID=$($p.Id), log: $logPath" -ForegroundColor DarkGray
    try {
        if (-not (Wait-AllHealthy -TimeoutSec 1800)) {
            Get-Content "$logPath.err" -ErrorAction SilentlyContinue | Select-Object -Last 30 | ForEach-Object { Write-Host $_ }
            throw "test stack did not become healthy"
        }
    } finally {
        if (Test-Path "$logPath.err") { Remove-Item "$logPath.err" -Force -ErrorAction SilentlyContinue }
        Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
    }
}

function Stop-TestStack {
    Write-Host "[docker] stopping test stack..." -ForegroundColor Cyan
    $code = Invoke-Compose -ComposeArgs (@(Get-ComposeArgs) + @("down", "-v"))
    if ($code -ne 0) { throw "docker compose down failed (exit=$code)" }
}

function Restart-Services {
    param([string[]]$Services)
    Write-Host "[docker] restarting: $($Services -join ', ')" -ForegroundColor Cyan
    & docker compose @(Get-ComposeArgs) restart @Services 2>&1 | ForEach-Object { Write-Host $_ }
    Wait-AllHealthy
}

function Get-ContainerName {
    param([string]$Service)
    $tmpOut = [System.IO.Path]::GetTempFileName()
    try {
        $p = Start-Process -FilePath "docker" -ArgumentList @("compose", "-f", "d:\projects\coder-micro-services\infrastructure\docker-compose.test-infra.yaml", "ps", "-q", $Service) `
            -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err"
        $id = (Get-Content $tmpOut -Raw -ErrorAction SilentlyContinue).Trim()
        if ($id) { return $id }
        # fallback: ps -a + format
        $p2 = Start-Process -FilePath "docker" -ArgumentList @("ps", "-a", "--filter", "label=com.docker.compose.service=$Service", "--format", "{{.ID}}") `
            -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err"
        return (Get-Content $tmpOut -Raw -ErrorAction SilentlyContinue).Trim().Split("`n")[0].Trim()
    } finally {
        Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
        if (Test-Path "$tmpOut.err") { Remove-Item "$tmpOut.err" -Force -ErrorAction SilentlyContinue }
    }
}

function Reset-Database {
    Write-Host "[db] dropping and recreating superpowers_test..." -ForegroundColor Cyan
    $mysqlC = Get-ContainerName -Service "mysql"
    $redisC = Get-ContainerName -Service "redis"
    if (-not $mysqlC) { throw "mysql container not found" }
    if (-not $redisC) { throw "redis container not found" }
    Write-Host "[db] mysql=$mysqlC  redis=$redisC" -ForegroundColor DarkGray

    $tmpOut = [System.IO.Path]::GetTempFileName()
    try {
        Start-Process -FilePath "docker" -ArgumentList @("exec", "-e", "MYSQL_PWD=rootpass", $mysqlC, "mysql", "-uroot", "--show-warnings=0", "-e", "DROP DATABASE IF EXISTS superpowers_test; CREATE DATABASE superpowers_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;") `
            -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err" | Out-Null
    } finally {
        Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
        if (Test-Path "$tmpOut.err") { Remove-Item "$tmpOut.err" -Force -ErrorAction SilentlyContinue }
    }

    Write-Host "[db] flushing redis..." -ForegroundColor Cyan
    $tmpOut = [System.IO.Path]::GetTempFileName()
    try {
        Start-Process -FilePath "docker" -ArgumentList @("exec", $redisC, "redis-cli", "FLUSHALL") `
            -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err" | Out-Null
    } finally {
        Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
        if (Test-Path "$tmpOut.err") { Remove-Item "$tmpOut.err" -Force -ErrorAction SilentlyContinue }
    }

    Write-Host "[db] reset complete (services run on host, no restart needed)" -ForegroundColor Green
}

function Wait-AllHealthy {
    param([int]$TimeoutSec = 120)
    $services = @("mysql", "redis", "consul")
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $start = Get-Date
    while ((Get-Date) -lt $deadline) {
        $allHealthy = $true
        $allRunning = $true
        $tmpOut = [System.IO.Path]::GetTempFileName()
        try {
            $p = Start-Process -FilePath "docker" -ArgumentList (@("compose") + (Get-ComposeArgs) + @("ps", "--format", "{{.Service}}|{{.State}}|{{.Health}}")) `
                -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err"
            $rows = @(Get-Content $tmpOut -ErrorAction SilentlyContinue | Where-Object { $_ -match "\|" })
            foreach ($s in $services) {
                $row = $rows | Where-Object { $_ -like "$s|*" } | Select-Object -First 1
                if (-not $row) { $allRunning = $false; $allHealthy = $false; continue }
                $parts = $row -split "\|"
                $state = $parts[1].Trim()
                $health = $parts[2].Trim()
                if ($state -ne "running") { $allRunning = $false; $allHealthy = $false; continue }
                if ($health -and $health -ne "healthy") { $allHealthy = $false }
            }
        } finally {
            Remove-Item $tmpOut -Force -ErrorAction SilentlyContinue
            if (Test-Path "$tmpOut.err") { Remove-Item "$tmpOut.err" -Force -ErrorAction SilentlyContinue }
        }
        if ($allHealthy -and $allRunning) {
            Write-Host "[docker] all services healthy" -ForegroundColor Green
            return $true
        }
        $elapsed = [math]::Round((New-TimeSpan -Start $start -End (Get-Date)).TotalSeconds)
        Write-Host "[docker] waiting... (${elapsed}s elapsed, healthy=$allHealthy running=$allRunning)" -ForegroundColor DarkGray
        Start-Sleep -Seconds 5
    }
    Write-Host "[docker] TIMEOUT after ${TimeoutSec}s" -ForegroundColor Red
    return $false
}
