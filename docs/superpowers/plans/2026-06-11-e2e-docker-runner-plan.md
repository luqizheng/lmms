# E2E Docker Runner 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 写一个 PowerShell 脚本 `scripts/run-e2e.ps1`，一键装包 + 起 docker + 等健康 + 跑 E2E 测试，支持参数透传

**Architecture:** 单 PS1 文件，按 spec 流程分段；不引入 Pester/额外依赖；用 `Invoke-WebRequest` 探活；`pnpm test` 透传 vitest 参数

**Tech Stack:** PowerShell 5、Docker Compose v2、pnpm、vitest

**Spec:** [2026-06-11-e2e-docker-runner-design.md](../specs/2026-06-11-e2e-docker-runner-design.md)

**前置假设：**
- 用户在真实 PowerShell（沙箱不限制 PS 脚本执行）
- Docker Desktop / Docker Engine 已装
- 服务全栈 compose 文件：`infrastructure/docker-compose.yml`（含 consul/redis/mysql/api-gateway/token-server/auth-service/user-service/notification-service）
- E2E 子包：`tests/e2e/`（上一 plan 已建好）

---

## 文件结构

**新增：**
- `scripts/run-e2e.ps1` — 主脚本

**修改：**
- `package.json`（根）— 加 `test:e2e:docker` 脚本

---

## Task 1: 写脚本骨架与参数解析

**Files:**
- Create: `scripts/run-e2e.ps1`

- [ ] **Step 1: 写文件**

```powershell
# scripts/run-e2e.ps1
# 一键启动 docker 全栈并跑 E2E 测试
# 用法: powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 [-Filter <name>] [-TestName <pattern>] [-NoUp] [-Rebuild] [-Debug]

[CmdletBinding()]
param(
    [switch]$NoUp,
    [string]$Filter,
    [string]$TestName,
    [switch]$Rebuild,
    [switch]$Debug
)

$ErrorActionPreference = 'Stop'
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $repoRoot

Write-Host '== E2E Docker Runner ==' -ForegroundColor Cyan
Write-Host "  NoUp    : $NoUp"
Write-Host "  Filter  : $Filter"
Write-Host "  TestName: $TestName"
Write-Host "  Rebuild : $Rebuild"
Write-Host "  Debug   : $Debug"
```

- [ ] **Step 2: 验证参数解析**

Run: `powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -Filter users -Debug`
Expected: 输出 `== E2E Docker Runner ==` 和 5 行参数

- [ ] **Step 3: 提交**

```bash
git add scripts/run-e2e.ps1
git commit -m "chore(scripts): run-e2e.ps1 scaffold and params"
```

---

## Task 2: 加依赖检查

**Files:**
- Modify: `scripts/run-e2e.ps1`（在 `Set-Location` 之后追加）

- [ ] **Step 1: 追加依赖检查块**

```powershell
Write-Host '[1/4] Checking e2e dependencies...' -ForegroundColor Yellow
if (-not (Test-Path 'tests/e2e/node_modules')) {
    Write-Host '  Installing pnpm packages...'
    Push-Location tests/e2e
    try {
        pnpm install
        if ($LASTEXITCODE -ne 0) { throw "pnpm install failed with code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
} else {
    Write-Host '  tests/e2e/node_modules exists, skip'
}
```

- [ ] **Step 2: 验证**

Run: `powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -NoUp`
Expected: 输出 `[1/4] Checking e2e dependencies...` + `tests/e2e/node_modules exists, skip`

- [ ] **Step 3: 提交**

```bash
git add scripts/run-e2e.ps1
git commit -m "feat(scripts): e2e runner dependency check"
```

---

## Task 3: 加 docker compose up 与健康检查

**Files:**
- Modify: `scripts/run-e2e.ps1`

- [ ] **Step 1: 追加 docker 启动块**

```powershell
if (-not $NoUp) {
    Write-Host '[2/4] Starting docker compose stack...' -ForegroundColor Yellow
    $composeArgs = @('-f', 'infrastructure/docker-compose.yml', 'up', '-d')
    if ($Rebuild) { $composeArgs += '--build', '--pull', 'never' }
    docker compose @composeArgs
    if ($LASTEXITCODE -ne 0) { throw "docker compose up failed with code $LASTEXITCODE" }

    Write-Host '[2/4] Waiting for API gateway health...' -ForegroundColor Yellow
    $deadline = (Get-Date).AddSeconds(120)
    $healthy = $false
    while ((Get-Date) -lt $deadline) {
        try {
            $r = Invoke-WebRequest 'http://localhost:3000/api/v1/auth/setup-status' -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
            if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) {
                $healthy = $true
                break
            }
        } catch {}
        Start-Sleep -Seconds 2
    }
    if (-not $healthy) { throw 'API gateway not healthy after 120s. Run: docker compose -f infrastructure/docker-compose.yml logs' }
    Write-Host '  Gateway is healthy' -ForegroundColor Green
} else {
    Write-Host '[2/4] Skipped (NoUp)' -ForegroundColor Yellow
}
```

- [ ] **Step 2: 验证 — 干跑路径（-NoUp）**

Run: `powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -NoUp`
Expected: 输出 `[2/4] Skipped (NoUp)`

- [ ] **Step 3: 提交**

```bash
git add scripts/run-e2e.ps1
git commit -m "feat(scripts): e2e runner docker up + health check"
```

---

## Task 4: 加 vitest 透传跑测

**Files:**
- Modify: `scripts/run-e2e.ps1`

- [ ] **Step 1: 追加 vitest 跑测块**

```powershell
Write-Host '[3/4] Running E2E tests...' -ForegroundColor Yellow
$env:E2E_BASE_URL = 'http://localhost:3000'
if ($Debug) { $env:E2E_DEBUG = '1' }

$vitestArgs = @()
if ($Filter)   { $vitestArgs += $Filter }
if ($TestName) { $vitestArgs += '-t', $TestName }

Push-Location tests/e2e
try {
    pnpm test @vitestArgs
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}

Write-Host '[4/4] Done.' -ForegroundColor Yellow
if ($code -ne 0) {
    Write-Host "  vitest exit: $code" -ForegroundColor Red
} else {
    Write-Host "  vitest exit: 0" -ForegroundColor Green
}
Write-Host ''
Write-Host 'To stop the docker stack:' -ForegroundColor Cyan
Write-Host '  docker compose -f infrastructure/docker-compose.yml down'
exit $code
```

- [ ] **Step 2: 验证 — 干跑（-NoUp, 期望 service 不可达）**

Run: `powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -NoUp`
Expected: 输出 `[3/4] Running E2E tests...` + vitest 跑（因服务未起 ECONNREFUSED 退出非 0）
脚本输出 `To stop the docker stack: ...`

- [ ] **Step 3: 提交**

```bash
git add scripts/run-e2e.ps1
git commit -m "feat(scripts): e2e runner vitest pass-through"
```

---

## Task 5: 加根 package.json 脚本

**Files:**
- Modify: `package.json:14-22`

- [ ] **Step 1: 追加脚本**

在 `test:e2e:permissions` 之后插入：

```json
    "test:e2e:docker": "powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1",
    "test:e2e:docker:users": "powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -Filter users",
    "test:e2e:docker:no-up": "powershell -ExecutionPolicy Bypass -File scripts/run-e2e.ps1 -NoUp",
```

- [ ] **Step 2: 验证**

Run: `pnpm run test:e2e:docker:no-up`
Expected: 同 Task 4 验证，ECONNREFUSED，exit 非 0

- [ ] **Step 3: 提交**

```bash
git add package.json
git commit -m "chore(scripts): root e2e:docker scripts"
```

---

## Task 6: 验收

**Files:** 无

- [ ] **Step 1: 干跑路径（-NoUp）**

Run: `pnpm run test:e2e:docker:no-up`
Expected: `[1/4]...[2/4] Skipped...[3/4]...[4/4] Done.` 顺序输出；vitest 跑；脚本退出码 ≠ 0（无服务）

- [ ] **Step 2: 参数透传**

Run: `pnpm run test:e2e:docker:no-up -- -Filter users -Debug`
Expected: vitest 跑 users，E2E_DEBUG=1，输出 `>> GET/POST ... << status` 调试行

- [ ] **Step 3: 完整路径（需要 docker）**

Run: `pnpm run test:e2e:docker`
Expected:
1. `[1/4] Checking e2e dependencies...` 跳过（node_modules 存在）
2. `[2/4] Starting docker compose stack...` 镜像若没缓存会 build
3. `[2/4] Waiting for API gateway health...` 120s 内 healthy
4. `[3/4] Running E2E tests...` 21 case
5. `[4/4] Done.` exit 0

---

## Self-Review

- **Spec 覆盖**：
  - 参数 5 个（NoUp/Filter/TestName/Rebuild/Debug）✓ Task 1
  - 依赖检查 ✓ Task 2
  - compose up + 健康检查 ✓ Task 3
  - vitest 透传 ✓ Task 4
  - 根 scripts ✓ Task 5
  - 验收 8 条 ✓ Task 6
- **占位**：0 个 TBD/TODO
- **类型/命令一致**：`pnpm test`、`docker compose`、`Invoke-WebRequest` 用法跨 task 一致
- **范围**：单 plan 可执行
- **错误处理**：4 场景（docker 不存在/up 失败/超时/vitest 失败）透传退出码 ✓
