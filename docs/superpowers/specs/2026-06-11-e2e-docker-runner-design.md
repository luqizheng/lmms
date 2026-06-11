# E2E Docker Runner 脚本设计

**日期**：2026-06-11
**状态**：待审
**作者**：brainstorming

## 背景

[2026-06-11-nodejs-web-api-e2e-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-11-nodejs-web-api-e2e-design.md) 已落 21 个 E2E case + 跑测脚本（`pnpm -C tests/e2e test`）。但用户起 docker 服务后还要：
- 记 `E2E_BASE_URL` 应该指向 3000（compose 网关端口），与 design 默认 8080 不一致
- 手动 `cd tests/e2e && pnpm install`（首次）
- 手动轮询服务健康

缺一个**一键脚本**，把这些串起来。

## 目标

- 一条命令：`scripts/run-e2e.ps1` 装包 + 起 docker + 等健康 + 跑测
- 透传 vitest 参数（单组/单 case/调试）
- 不自动 down（调试后可重跑）

## 非目标

- 不自动 down compose（用户决策）
- 不做 docker 镜像构建优化
- 不做日志/报告聚合
- 不替代 `test-orchestrator.ps1`（Rust 单测用）

## 架构

**单文件**：`scripts/run-e2e.ps1`

**流程图**：
```
参数解析
  ↓
e2e 依赖检查（tests/e2e/node_modules 存在？）
  ├─ 否 → pnpm install
  └─ 是 → 跳过
  ↓
-NoUp? → 跳过
  ├─ 否 → docker compose up -d
  │       ↓
  │      健康检查（GET http://localhost:3000，120s）
  │       ├─ 成功 → 继续
  │       └─ 失败 → exit 1
  └─ 是 → 跳过
  ↓
设 $env:E2E_BASE_URL='http://localhost:3000'
  ↓
cd tests/e2e && pnpm test <Filter 透传>
  ↓
打印 "手动 docker compose down" 提示
  ↓
exit <vitest 退出码>
```

## 关键设计

### 参数

```powershell
[switch]$NoUp         # 跳过 docker 启动（服务已起时）
[string]$Filter       # vitest 文件名过滤（例 'users'）
[string]$TestName     # vitest -t 过滤（例 'list users'）
[switch]$Rebuild      # docker compose up --build --pull never
[switch]$Debug        # 设 E2E_DEBUG=1
```

### 健康检查

```powershell
$timeout = (Get-Date).AddSeconds(120)
while ((Get-Date) -lt $timeout) {
  try {
    $r = Invoke-WebRequest 'http://localhost:3000/api/v1/auth/setup-status' -UseBasicParsing -TimeoutSec 3
    if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) { return }
  } catch {}
  Start-Sleep -Seconds 2
}
throw 'gateway not healthy after 120s'
```

**理由**：`/api/v1/auth/setup-status` 是 design 已验证的无 token 公开端点（auth.e2e.test.ts 测过）。

### 依赖检查

```powershell
if (-not (Test-Path 'tests/e2e/node_modules')) {
  Set-Location tests/e2e
  pnpm install
  Set-Location ..
}
```

### vitest 透传

```powershell
$args = @()
if ($Filter)   { $args += $Filter }
if ($TestName) { $args += '-t', $TestName }
if ($Debug)    { $env:E2E_DEBUG = '1' }
Set-Location tests/e2e
pnpm test @args
$code = $LASTEXITCODE
Set-Location ..
exit $code
```

### compose 命令

```powershell
$composeArgs = @('-f', 'infrastructure/docker-compose.yml', 'up', '-d')
if ($Rebuild) { $composeArgs += '--build', '--pull', 'never' }
docker compose @composeArgs
```

## 错误处理

| 场景 | 行为 |
|---|---|
| `docker` 命令不存在 | 抛 `command not found`，exit 1 |
| compose up 失败 | 透传 docker 退出码，exit 1 |
| 健康检查 120s 超时 | 抛错，exit 1，提示 `docker compose logs` |
| `pnpm install` 失败 | 透传 pnpm 退出码，exit 1 |
| vitest 失败 | 透传 vitest 退出码，exit 非 0（让 CI/用户感知） |

## YAGNI

- 不做自动 down
- 不做日志跟踪
- 不做 retry
- 不做镜像清理
- 不做多环境切换（dev/staging）

## 验收

- [ ] 干净环境：`scripts/run-e2e.ps1` 装包→起 docker→等健康→跑 21 case
- [ ] `-NoUp`：跳过 docker 启动
- [ ] `-Filter users`：仅 users
- [ ] `-TestName "list users"`：单 case
- [ ] `-Debug`：输出请求/响应体
- [ ] `-Rebuild`：强制重建镜像
- [ ] 不自动 down（脚本退出后容器仍在）
- [ ] vitest 失败时 exit 非 0
