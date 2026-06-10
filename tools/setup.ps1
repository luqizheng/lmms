# tools/setup.ps1
# 为 ring crate 准备 NASM 汇编器
# ring build.rs 会在 cargo 调用时查找 ./target/tools/windows/nasm/nasm
# 该路径相对 crate 源目录, 我们把仓库自带的 nasm 复制过去
# 兼容 ring 0.17.x 所有版本

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$nasmExe  = Join-Path $repoRoot "tools\nasm.exe"

if (-not (Test-Path $nasmExe)) {
    Write-Error "未找到 $nasmExe"
    exit 1
}

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME ".cargo" }
$srcRoot   = Join-Path $cargoHome "registry\src"

if (-not (Test-Path $srcRoot)) {
    Write-Host "cargo registry 缓存目录不存在, 跳过 (首次 cargo build 时会自动触发 ring build.rs)" -ForegroundColor Yellow
    exit 0
}

$count = 0
Get-ChildItem -Path $srcRoot -Directory -ErrorAction SilentlyContinue | Get-ChildItem -Directory -Filter "ring-*" -ErrorAction SilentlyContinue | ForEach-Object {
    $ringDir = $_.FullName
    $dst     = Join-Path $ringDir "target\tools\windows\nasm\nasm.exe"
    $dstDir  = Split-Path $dst -Parent
    if (-not (Test-Path $dstDir)) { New-Item -ItemType Directory -Force -Path $dstDir | Out-Null }
    Copy-Item $nasmExe $dst -Force
    Write-Host "[setup] $( $_.Name ) -> $dst" -ForegroundColor Green
    $count++
}

if ($count -eq 0) {
    Write-Host "未发现 ring-* 缓存目录, 跳过" -ForegroundColor Yellow
} else {
    Write-Host "[setup] 完成, 处理 $count 个 ring 版本" -ForegroundColor Green
}
