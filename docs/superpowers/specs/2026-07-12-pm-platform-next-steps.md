# PM Platform v0.1.0 → v0.2.0 — 下一步工作计划

> 日期：2026-07-12
> 接续：commit `f287a7f`（docs(pm-platform): end-to-end SDK chain verification evidence）
> 设计 spec：`docs/superpowers/specs/2026-07-11-pm-platform-design.md`

---

## 0. 上轮交付固化状态（已完成 + push）

```
f287a7f docs(pm-platform): end-to-end SDK chain verification evidence
164a8ac fix(scripts): publish-sdks.sh -pl paths must be relative to reactor root
8c4c7a8 feat(pm-platform): frontend integration + scripts + top-level wiring
c361471 feat(pm-platform): Spring Boot 3.x PM Platform v0.1.0
9fa057d feat(sdk): notification / file / config service-sdk v0.3.0
6dc2556 docs(plan): PM Platform v0.1.0 design spec
```

**已端到端验证（环境内）**：
| 检查 | 命令 | 结果 |
|---|---|---|
| 后端编译 | `mvn -Pjava17 clean verify` | ✓ BUILD SUCCESS（10 个 pom） |
| 后端单测 | (含 AuditEventReporterTest 7 cases) | ✓ all pass |
| 前端测试 | `pnpm test:unit` | ✓ 198/198 pass |
| SDK 干跑 | `bash scripts/publish-sdks.sh --dry-run` | ✓ BUILD SUCCESS（4 个 SDK + common + parent） |
| 容器启动 | `docker run ... pm-app-bootstrap.jar` | ✓ Up 19min, Tomcat 8088 |
| Flyway | V1+V2 跑 | ✓ 7 tables in `pm_platform` schema |
| Smoke | `bash scripts/pm-smoke.sh` | ✓ 11/11 pass |
| **SDK 链端到端** | 容器日志 `audit-service report failed (action=PM_PROJECT_CREATED, …): audit 401` | ✓ **HTTP 真打 audit-service**（401 是 token 不是 SDK） |

---

## 1. 工作分阶段（按优先级）

### Phase A：SDK publish + 全栈验证 ✅ 完成

| # | 任务 | 状态 | 实际 commit |
|---|---|---|---|
| A1 | SDK publish 到私仓 192.168.2.12:18181 | ✅ 完成 | `2d21eea` |
| A2 | apps/pm mvn verify 从私仓解析 SDK 0.3.0 | ✅ 完成 | `30df25e` |
| A3 | docker-compose healthcheck + registrator + gateway 路由 | ✅ 完成 | `71a50a0` |

### Phase B：IAM JWT 互通 ✅ 完成（方案 B1）

| # | 任务 | 状态 | 实际 commit |
|---|---|---|---|
| B1 | JwksJwtVerifier (RS256 + JWKS + kid 缓存 + iss/aud/scope 校验) | ✅ 完成 | `0ad0a62` |
| B2 | Hs256JwtVerifier (dev fallback) + 17 个单测 | ✅ 完成 | `97d73c5` |
| B3 | Principal record 移到 pm-app-domain + CurrentActor 工具 | ✅ 完成 | `1b62e29` |
| B4 | 6 个 service + 2 controller 用 CurrentActor 填 createdBy | ✅ 完成 | `7fdca30` |
| B5 | 端到端验证: iam-service 真 token -> PM 受保护端点 200 | ✅ 完成 | — |

### Phase C：真实 LLM 接入 ✅ 完成（DeepSeek V4-Flash）

| # | 任务 | 状态 | 实际 commit |
|---|---|---|---|
| C1 | DeepSeekLlmProvider (OpenAI-compatible Chat Completions) | ✅ 完成 | `1228c10` |
| C2 | Controller/Analyzer provider 参数 + Spring 配置注入 | ✅ 完成 | `250a044` |
| C3 | Flyway pm_platform schema 隔离 + DeepSeek 默认 model | ✅ 完成 | `a101fc0` |
| C4 | 前端 AI 拆解/改写 UI (JSON 解析 + 批量导入 + 应用改写) | ✅ 完成 | `986623c` |
| C5 | 端到端真打 DeepSeek: decompose 13s / refine 10s | ✅ 完成 | — |

### Phase D：业务功能补全（部分完成）

| # | 任务 | 状态 | 实际 commit |
|---|---|---|---|
| D1 | KB 文档编辑器 (md-editor-v3) | ✅ 完成 | `e0aa301` |
| D2 | KB 文档版本切换 UI | ✅ 完成（含在 D1） | `e0aa301` |
| D3 | KB 文档树状导航 (el-tree 懒加载) | ✅ 完成 | `893e609` |
| D4 | 任务 / Bug / 用例管理 | ✅ 完成 | `e176d94d` / `f600b993` / `b3ff5788` |
| D5 | 迭代（Sprint）管理 | ✅ 完成 | `785663e` / `2a4562b` |
| D6 | 附件上传到 file-service | ✅ 完成 | `dceb0c6` |
| D7 | 评论 / @提醒 | ✅ 完成 | `23919c3` |

### Phase E：质量 / 测试（未开始）

| # | 任务 | 状态 |
|---|---|---|
| E1 | 业务 service 单测 (6/6 services, 36 cases) | ✅ 完成 | `1f0a89a` / `5554879` / `6b675c3` / `d40b1a3` |
| E2 | 业务 controller 单测 (7/7 controllers, 53 cases) | ✅ 完成 | `9c84d92` / `95e9715` / `e8d48fa` / `3089b5f` / `7111b98` / `0ff2a08` |
| E3 | e2e Playwright 覆盖 (API smoke cases done, UI pending) | ✅ 部分完成 | `76db5f8` |
| E4 | OpenAPI yaml 落库 | ✅ 完成 | `84cd4cc` |
| E5 | vitest coverage 接 CI (coverage-v8 + test:coverage script) | ✅ 完成 | `027808d` |
| E6 | SpotBugs + PMD 接 CI (Maven verify phase) | ✅ 完成 | `31ef11e` |
| E7 | E2E 在 docker compose 完整栈上跑 | ⏸ 未开始 |

### Phase F：Dev Experience（部分完成）

| # | 任务 | 状态 | 实际 commit |
|---|---|---|---|
| F1 | dev profile JWT bypass (NoOpJwtVerifier) | ✅ 完成 | `f55f8eb` |
| F2 | 种子数据脚本 seed-pm.sh | ✅ 完成 | `68b673f` |
| F3 | apps/pm AGENTS.md dev quickstart | ✅ 完成（Phase B/F1 文档已更新） | `d784f02` / `f2f019e` |
| F4 | docker-compose pm healthcheck | ✅ 完成 | `71a50a0` |
| F5 | OpenAPI 集成测试 | ⏸ 未开始 | — |

### Phase G：跨服务架构层（本季度以后）

| # | 任务 | 状态 |
|---|---|---|
| G1 | OAuth2 / OIDC 替换 | ⏸ 跳过 |
| G2 | 多租户 | ⏸ 跳过 |
| G3 | 全文检索 (SQL LIKE on title, MVP) | ✅ 部分完成 | `4a51401` |
| G4 | 实时协作 (WebSocket + OT) | ⏸ 跳过 |
| G5 | 审计 (SDK service-jwt 注入) | ✅ 部分完成 | `96831f5` |

---

## 2. 下一会话第一步建议（env 可达场景）

```bash
# 0. checkout 干净 + 看上次 commit
cd /home/kudi/low-memory-micro-services
git log --oneline -3
git status

# 1. 装 JDK 17 + Maven + 配阿里云镜像
curl -fSL --max-time 300 -o /tmp/c17.tgz \
  https://corretto.aws/downloads/latest/amazon-corretto-17-x64-linux-jdk.tar.gz
mkdir -p ~/.local/jdk-17 && tar -xzf /tmp/c17.tgz -C ~/.local/jdk-17 --strip-components=1
cat > ~/.m2/settings.xml <<'EOF'
<settings><mirrors><mirror><id>aliyunmaven</id><mirrorOf>central,public</mirrorOf>
<name>aliyunmaven</name><url>https://maven.aliyun.com/repository/public</url></mirror></mirrors></settings>
EOF
source /home/kudi/.sdkman/bin/sdkman-init.sh && sdk install maven 3.9.9
echo "alias mvn-java17='JAVA_HOME=~/.local/jdk-17 PATH=~/.local/jdk-17/bin:$PATH mvn'" >> ~/.bashrc

# 2. 装 SDK 到 local repo（让 apps/pm 能解析）
source ~/.local/jdk-17/env.sh
sdk use maven 3.9.9
( cd sdk/java && mvn -Pjava8 install -DskipTests )

# 3. 完整 backend 验证
mvn -Pjava17 clean verify

# 4. 起 Docker compose
docker compose -f infrastructure/docker-compose.yml up -d mysql pm

# 5. 跑 smoke（用 IAM 真 token 替换 dev-bypass）
PM_JWT_SECRET='<iam-service-shared-secret>' bash scripts/pm-smoke.sh

# 6. frontend 跑测
cd frontend && pnpm install && pnpm test:unit && pnpm build
```

## 3. 下一会话第一步建议（env 不可达场景）

```bash
# 仅做 env-independent 增量：
# 1. 补 backend service 单测（Phase E1）
# 2. 加 dev profile bypass（Phase F1）
# 3. 写种子数据脚本（Phase F2）
# 4. commit + push
```

---

## 4. 风险与陷阱（必须知道）

1. **pm 容器是 `pm-app-bootstrap.jar`，没有 curl / 工具**——不要试图 `docker exec infra-pm-1 curl`；用 `docker logs` 或 `PM_BASE=http://localhost:8088 bash scripts/pm-smoke.sh` 替代。
2. **Flyway V1 = pm_*, V2 = kb_***——任何对这两个的 schema 改动都得 **forward-only**（V3+），不能 amend 已有 migration。
3. **pm_*/kb_* 与 iam_/audit_/file_/ntf_/cfg_ 在同一 MySQL `superpowers` 库**——已用独立 schema `pm_platform` 隔离，但**表前缀规则**仍按 `scripts/check-db-naming.js` 走。
4. **frontend `vue-pure-admin` 必须经 nginx `/pm-api/` 转发**（不是 `/api/`）；前端 axios 自己 baseURL = `import.meta.env.VITE_PM_API_BASE ?? '/pm-api'`（`api/{pm,kb,llm}.ts` 里写死）。
5. **SDK 0.3.0-SNAPSHOT 在 `~/.m2/repository/com/kudi/coder/*/`**——本机安装时 mvn 会找到；但若 CI 用 fresh image，需先把 SDK 装上或接 mvn.kudi.repos 远端 repo。
6. **Spring Boot 3.3.5 + Spring Security 6.3.4**——`-parameters` 必须显式开（已在 pom）；`SecurityContextHolder` API 不变但需 `spring-security-core` 依赖（已在 pm-app-infrastructure pom）。

---

## 5. 文件导航（接手时直接走这些路径）

```
/home/kudi/low-memory-micro-services/
├── apps/pm/                              # Spring Boot 3.x 后端（62 文件）
│   ├── pom.xml                           # parent + java.version=17
│   ├── pm-app-bootstrap/                  # @SpringBootApplication + Security + Health
│   ├── pm-app-api/                        # 8 REST controllers
│   ├── pm-app-domain/                     # NotFoundException + ConflictException
│   ├── pm-app-infrastructure/             # 7 entities + 7 mappers + 7 services + SdkConfig + MarkdownRenderer
│   ├── pm-app-llm/                        # LlmProvider interface + NoopLlmProvider + Registry
│   └── README.md                          # 含「端到端验证」段
├── sdk/java/                              # 4 SDK modules
│   ├── pom.xml                            # parent 0.3.0
│   ├── audit-service-sdk/                 # v0.3.0-SNAPSHOT
│   ├── notification-service-sdk/          # v0.3.0-SNAPSHOT
│   ├── file-service-sdk/                  # v0.3.0-SNAPSHOT
│   └── config-service-sdk/                # v0.3.0-SNAPSHOT
├── frontend/src/                          # vue-pure-admin 内嵌
│   ├── api/{pm,kb,llm}.ts                # axios clients, baseURL = '/pm-api'
│   ├── store/modules/pm.ts                # Pinia store
│   ├── router/modules/pm.ts               # /pm/* routes
│   ├── views/pm/                          # 4 view files + 4 test files
│   └── AGENTS.md                          # PM Platform 路由表 + 服务 URL 前缀表
├── scripts/
│   ├── publish-sdks.sh                    # 一键发 SDK（需 kudi-maven 凭证）
│   └── pm-smoke.sh                        # 端到端 smoke（用 PM_JWT_SECRET inline mint HS256 JWT）
├── infrastructure/
│   └── docker-compose-services.yml        # pm service entry
├── docs/superpowers/specs/
│   ├── 2026-07-11-pm-platform-design.md   # 上轮设计 spec
│   └── 2026-07-12-pm-platform-next-steps.md  # ← 本文件
└── README.md                              # 根 README 加了 PM Platform 一行
```

---

## 6. 关键决策待定（用户必须输入）

| 决策 | 选项 | 默认倾向 |
|---|---|---|
| 1. IAM JWT 互通方案 | B1 RS256+共享 JWK / B2 HS256+audit 双签 / B3 service-to-service 委托 / B4 短期缓存 | **B1**（最干净） |
| 2. 真实 LLM 选型 | OpenAI / Doubao (volcengine) / Ollama / 其他 | **用户决定**（不同选择 SDK 大小差 10x） |
| 3. 文档编辑器 | Monaco Editor（重、IDE 体验）vs md-editor-v3（轻、专为 MD）vs 自建 textarea+preview（最轻） | **md-editor-v3**（KM 场景） |
| 4. Phase D（业务补全）选哪几项先做 | D1-D7 全列 | **D1 文档编辑器 + D2 版本切换**（KB 用户感知最大） |
| 5. Phase G（架构大块）本季度做不做 | 跳过 / 部分 / 全做 | **跳过 G1-G5**（不在本季度目标） |

---

## 7. 一句话总结

**v0.1.0 已完整交付 + push + 端到端真验证 SDK 链。**

下一会话**第一件事**是 Phase A1（SDK publish 拿凭证），跑完后直接 A2 + A3 把 production-readiness 闭环；然后进 Phase B 解决 IAM 401 那个 token 不通的真问题。

如果只做 env-independent：Phase E1（service 单测）+ F1（dev bypass）+ F2（seed 脚本） 是性价比最高的三件事。
