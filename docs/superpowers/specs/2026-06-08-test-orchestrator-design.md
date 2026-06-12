# 测试编排与 AI 协同修复 — 设计

日期：2026-06-08
状态：待审阅

## 目标

为 `coder-micro-services`（Rust + Go 多语言微服务）建立一套本地 CLI 工具链，覆盖：

1. 启动 Docker 中的 MySQL/Redis/Consul 与各服务容器
2. 重置数据库与缓存
3. 运行单元 + 集成测试
4. 生成结构化报告
5. 生成 AI 提示词（用于：基于 YAML 规格生成测试代码 / 修复失败测试）
6. 修复后重跑集成测试，验证未引入回归

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 编排工具 | 纯 PowerShell 脚本 | 与 Windows + PowerShell 环境一致；零外部依赖 |
| 触发模式 | 本地 CLI + 人工粘贴 AI 提示词 | 无需 LLM API Key；可控；可复盘 |
| AI 介入 | 自动修复（基于报告） | 闭环，减少人工排查 |
| 修复范围 | 仅测试代码 | 不动业务源码，安全边界 |
| 测试规格 | YAML 描述 + AI 生成代码 | 跨语言统一；非开发可读；版本可控 |
| DB 重置 | DROP/CREATE + FLUSHALL + 重启服务 | 由各服务启动时 AutoMigrate 重建表 |
| 重试上限 | 无（人工主导） | AI 不会自动无限循环修改 |

## 架构

```
开发者 ─写 YAML→ tests/specs/<service>/*.yaml
                              │
                              ▼
              generate-prompt（扫描 + 渲染模板）
                              │
                              ▼
            reports/spec-to-code-prompt.md ──复制粘贴──> AI
                              │
                              ▼
              AI 输出代码 → 按语言约定放回各服务
                Go:    services/<service>/<name>_test.go
                Rust:  services/<service>/tests/<name>.rs
                              │
                              ▼
   loop：start → reset-db → test-all → report → fix-prompt
                              │（失败时）
                              ▼
              reports/latest-fix-prompt.md ──复制粘贴──> AI
                              │
                              ▼
                       AI 改 services/<service>/tests/...
                              │
                              ▼
                reset-db → test-integration
```

## 组件

### 1. 入口脚本 `scripts/test-orchestrator.ps1`

子命令：

| 命令 | 作用 |
|------|------|
| `start` | `docker compose -f infrastructure/docker-compose.test.yaml up -d`，等待所有 healthcheck 通过（30s 轮询） |
| `stop` | `docker compose down -v` |
| `reset-db` | DROP/CREATE `superpowers`；`FLUSHALL`；`docker compose restart` 受影响服务 |
| `test-unit` | Rust：`cargo test --lib --message-format=json`；Go：`go test -short -json ./...` |
| `test-integration` | Rust：`cargo test --test '*' --message-format=json`；Go：`go test -json ./...` |
| `test-all` | 串联 test-unit + test-integration |
| `report` | 解析 JSON 日志 → `reports/run-{ts}/summary.md` |
| `fix-prompt` | 失败时生成 `reports/latest-fix-prompt.md`（指向最新 run） |
| `generate-prompt` | 扫描 `tests/specs/` → `reports/spec-to-code-prompt.md` |
| `validate-specs` | YAML 字段校验 |
| `loop` | 串联 `start → reset-db → test-all → fix-prompt` |

参数：`-Service <name>`（可选，限定单个服务）；`-Timestamp <yyyyMMdd-HHmmss>`（报告目录）

### 2. Docker 编排

复用 `infrastructure/docker-compose.test.yaml`（已含）：
- `mysql:8.0`、`redis:7-alpine`、`consul:1.15`
- 四个服务测试容器（api-gateway-test、token-server-test、auth-service-test、user-service-test）
- 网络 `test-superpowers`
- healthcheck 全部就绪

`start` 等待逻辑：循环 `docker inspect --format='{{.State.Health.Status}}'`，全绿后退出；30s 超时打印尾部 50 行日志后退出码 1。

### 3. 数据库重置

```
docker exec mysql mysql -uroot -prootpass -e \
  "DROP DATABASE IF EXISTS superpowers; \
   CREATE DATABASE superpowers CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;"

docker exec redis redis-cli FLUSHALL

docker compose -f infrastructure/docker-compose.test.yaml restart \
  auth-service-test user-service-test
```

表结构重建依赖各服务启动时执行的 GORM AutoMigrate / SQLx migrate。

### 4. 测试输出格式

- **Rust**：`cargo test --message-format=json` 输出 JSONL，自写 PS 解析器提取 `test` 事件。
- **Go**：`go test -json ./...` 已是 JSONL。

统一内部模型：
```
Run {
  service: string
  passed: int
  failed: int
  ignored: int
  cases: TestCase[]
}
TestCase {
  name: string
  status: pass | fail | ignored
  message?: string        // 失败信息
  location?: string       // 文件:行
  duration_ms: int
}
```

### 5. 报告生成器 `scripts/lib/Report-Builder.ps1`

输入：原始 JSON 日志
输出：`reports/run-{ts}/summary.md`

格式：
```markdown
# Test Run 2026-06-08 14:30:01

## 总览
| 服务 | 通过 | 失败 | 跳过 | 耗时 |
|------|------|------|------|------|
| api-gateway | 42 | 0 | 0 | 3.2s |
| token-server | 18 | 1 | 0 | 1.8s |
| ... |

## 失败详情
### token-server / issue_token_with_expired_key
- 位置：services/token-server/tests/issuer_test.rs:42
- 错误：assertion `left == right` failed
  left: 401
  right: 200
```

失败时同步生成 `fix-prompt.md`。

### 6. 提示词模板

**`scripts/prompts/fix-tests.template.md`：**
```
# 任务
修复以下失败的测试。**仅修改各服务下的测试代码**，
禁止修改 services/、libs/、sdk/ 下的业务源码（非 tests/ 目录）。

## 失败用例
{{FAILING_TESTS}}

## 错误详情
{{ERROR_DETAILS}}

## 相关文件
{{RELATED_FILES}}

## 完成后
运行：
  ./scripts/test-orchestrator.ps1 reset-db
  ./scripts/test-orchestrator.ps1 test-integration
```

**`scripts/prompts/spec-to-code.template.md`：**
```
# 任务
根据以下 YAML 测试规格，生成对应语言的测试代码。
- Go 服务 → services/<service>/<name>_test.go（与被测代码同包）
- Rust 服务 → services/<service>/tests/<name>.rs

仅输出代码块，不要解释。

## 规格
{{SPECS}}
```

### 7. YAML 规格校验 `scripts/lib/Spec-Validator.ps1`

必填字段：
- `name`（字符串，符合 `^[a-z][a-z0-9_]*$`）
- `service`（枚举：auth-service / user-service / api-gateway / token-server）
- `type`（枚举：unit / integration）
- `description`（非空字符串）
- `steps`（非空数组，每项含 `method` + `path`）

可选字段：`tags`、`preconditions`、`expected`、`data_cleanup`

校验失败时输出文件路径 + 错误行，退出码 1。

## 数据流

```
1. start
   └─ docker compose up -d --wait
   └─ 轮询 healthcheck，全绿退出

2. reset-db
   └─ DROP/CREATE MySQL
   └─ FLUSHALL Redis
   └─ docker compose restart 受影响服务
   └─ 等待 healthcheck

3. test-unit
   └─ for each service:
        cargo/go test → reports/run-{ts}/{service}.unit.json
        重定向 stdout+stderr → reports/run-{ts}/{service}.unit.log

4. test-integration
   └─ for each service:
        cargo/go test → reports/run-{ts}/{service}.integration.json
        重定向 stdout+stderr → reports/run-{ts}/{service}.integration.log

5. report
   └─ 解析所有 *.json
   └─ 汇总 → reports/run-{ts}/summary.md
   └─ 若有失败 → reports/run-{ts}/fix-prompt.md

6. fix-prompt（顶层别名）
   └─ 拷贝最新 fix-prompt.md → reports/latest-fix-prompt.md
   └─ 用于：本次运行失败时粘贴给 AI

7. generate-prompt（独立命令，按需执行）
   └─ 扫描 tests/specs/ → reports/spec-to-code-prompt.md
   └─ 用于：新增/修改 YAML 规格后生成代码时粘贴给 AI

8. （人工）复制 reports/latest-fix-prompt.md → AI
   └─ AI 改各服务下的测试代码
   └─ （人工）回到步骤 2
```

## 错误处理

| 场景 | 行为 |
|------|------|
| `start` healthcheck 超时 | 打印各容器尾部 50 行日志，退出码 1 |
| `start` 端口冲突 | 提示已占用端口，建议 `stop` 后重试 |
| `reset-db` MySQL 不可达 | 重试 3 次（指数退避），仍失败则退出 |
| 测试编译失败 | 标记 `category=compile_error`，列入 fix-prompt |
| 部分服务失败 | 继续其他服务，summary 标注 `partial` |
| 集成测试非零退出 | summary 标红，引导重跑 `reset-db + test-integration` |
| `fix-prompt.md` 已存在 | 覆盖，旧 run 目录保留 |

## 安全约束

- PS 脚本只对 `infrastructure/` 与 `reports/` 写
- 不动 `services/`、`libs/`、`sdk/` 下的业务代码（非 tests/ 部分）
- 提示词明确禁止改业务源码

## 文件清单

新增：

```
scripts/
  test-orchestrator.ps1           # 入口
  lib/
    Report-Builder.ps1            # 报告生成
    Spec-Validator.ps1            # YAML 校验
    Docker-Helper.ps1             # compose / healthcheck 封装
    Test-Runner.ps1               # cargo/go 封装
  prompts/
    fix-tests.template.md
    spec-to-code.template.md
tests/
  specs/                          # YAML 规格（git 跟踪）
    auth-service/
    user-service/
    api-gateway/
    token-server/
```

无文件清单修改（生成代码放回各服务 tests/ 目录，进版本库）。

## 测试策略

工具链本身不做单元测试（一次性脚本）。验证方式：

1. **手测 A：链路冒烟**
   ```
   ./scripts/test-orchestrator.ps1 start
   ./scripts/test-orchestrator.ps1 reset-db
   ./scripts/test-orchestrator.ps1 test-all
   ./scripts/test-orchestrator.ps1 report
   ```
   期望：`reports/run-001/summary.md` 列出所有服务通过/失败状态

2. **手测 B：AI 修复闭环**
   - 人为引入一个失败用例
   - 跑 loop
   - 复制 fix-prompt 给 AI
   - AI 改 `services/<service>/tests/...`
   - 跑 `reset-db + test-integration`
   - 期望：失败用例通过，无新失败

3. **手测 C：YAML 校验**
   ```
   ./scripts/test-orchestrator.ps1 validate-specs
   ```
   故意写错字段，确认报错信息含文件路径

## 不在范围

- LLM API 集成（保持人主导粘贴）
- 跨平台支持（仅 Windows PowerShell）
- 性能基准测试
- 测试覆盖率统计
- 浏览器端到到端测试（仅后端服务）

## 开放问题

无。
