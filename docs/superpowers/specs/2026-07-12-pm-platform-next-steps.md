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

### Phase A：把上轮"差最后一脚"补上（env 可达时，< 1 天）

| # | 任务 | 触发命令 / 文件 | 卡点 | 估时 |
|---|---|---|---|---|
| A1 | **SDK 真正 publish 到内网** | `bash scripts/puperate-sdks.sh` | `~/.m2/settings.xml` 缺 `<id>kudi-maven</id>` 凭证 | 10 min（拿到凭证后） |
| A2 | **apps/pm 全栈真验证** | `mvn -Pjava17 verify && docker compose -f infrastructure/docker-compose.yml up -d pm && bash scripts/pm-smoke.sh` | 需 Docker daemon + JDK 17（我这边 OK）+ SDK publish 已 done | 30 min |
| A3 | **frontend pnpm install + build** | `cd frontend && pnpm install && pnpm build` | 需 Node + pnpm（我这边 OK） | 5 min |

A1 → A2 → A3 是直线路径。本会话里**已经验证 SDK 链可通**（audit HTTP 真打 401），所以 A1/A2 不会暴露新 bug，只会暴露 IAM token 问题（见 Phase B）。

### Phase B：IAM JWT 互通（架构决策 + 1-2 天）

**根因**（commit f287a7f 文档化）：apps/pm 用 HS256+自己 `PM_JWT_SECRET`，audit-service 期望 iam-service 的 RS256 token。两者 secret 算法不共享。

**方案候选**（需用户决策）：

| 方案 | 改动 | 优劣 |
|---|---|---|
| B1. apps/pm 也改 RS256，共享 iam-service 的 JWK | apps/pm 验签用 iam-service JWK endpoint；自身不再签 token | 单密钥源，但 apps/pm 完全依赖 iam-service |
| B2. 留 HS256，在 audit-service 端加 HS256 验签 | audit-service 配 apps/pm 共享 secret | 双密钥源，但 apps/pm 仍可独立 |
| B3. apps/pm 调用 audit 时用 iam-service token 代发（service-to-service 委托） | apps/pm 先调 iam-service 换 token | 单 token，但每次 SDK call 多一跳 |
| B4. 短期：apps/pm 启动时从 iam-service 拉 service token 缓存 | apps/pm 持 service account | 类似 B3 但加缓存 | 

**推荐 B1**（一致身份源），最干净但需要和 iam-service 团队协作。

**具体步骤**（按 B1 方案）：
1. apps/pm 改 `JwtConfig`：拿掉 `io.jsonwebtoken.Jwts.SIG.HS256`，改成 RS256 验签
2. 加 `JWKS_URL` 配置项指向 iam-service 的 `.well-known/jwks.json`（`${JWKS_URL}` 已有）
3. 在 `JwtAuthFilter` 用 Jwts.parser().verifyWith(JwkProvider).build() 验签
4. apps/pm 自己不再签 token（user 登录通过 iam-service `/oauth/token` 拿）
5. 新增 `POST /pm-api/v1/auth/login` 端点委托给 iam-service
6. pm-smoke.sh 改用 iam-service 发行的 token

### Phase C：真实 LLM 接入（1 天）

**当前**：NoopLlmProvider 唯一实现，UI 按钮 disabled，文案："LLM not configured"。

**步骤**：
1. 用户决策：选 OpenAI / Doubao（volcengine）/ Ollama / 其他
2. 新建 `OpenAiLlmProvider`（implements `LlmProvider`），调用 OpenAI Chat Completions API
   - 文件：`apps/pm/pm-app-infrastructure/src/main/java/com/coder/pm/infra/llm/provider/OpenAiLlmProvider.java`
   - properties：`pm.llm.openai.api-key`, `pm.llm.openai.base-url`, `pm.llm.openai.model`
3. `LlmController` 移除 `:disabled="decomposeLoading"` 启用按钮
4. `RequirementAnalyzer` 解析 `CompletionResponse.content` 提取 JSON 里的 stories/criteria，写入 `pm_user_stories` / `pm_acceptance_criteria`（此步要新加 controller/service）
5. e2e：录入需求 → 点 AI 拆解 → 后端真打 LLM → 自动建 stories + criteria

### Phase D：业务功能补全（按需，每项 0.5-1 天）

| # | 任务 | 影响 |
|---|---|---|
| D1 | **KB 文档编辑器**（Monaco / md-editor-v3 + 实时预览） | KB 实际可用性的最大提升 |
| D2 | **KB 文档版本切换 UI** | 复用现 backend `GET /versions` 端点 |
| D3 | **KB 文档树状导航**（space → folder → document） | 体验质变 |
| D4 | **任务 / Bug / 用例管理** | 完整 PM 三角，但 spec §11 列外（不在 MVP） |
| D5 | **迭代（Sprint）管理** | 看板视图前置 |
| D6 | **附件上传到 file-service** | 真实跨服务互通的第二个 demo（继 audit 之后） |
| D7 | **评论 / @提醒**（走 notification-service） | 实时协作前置 |

### Phase E：质量 / 测试（持续，每项 0.5 天）

| # | 任务 | 文件 |
|---|---|---|
| E1 | 业务 service 单测：ProjectService / RequirementService / KbDocumentService 等 | `apps/pm/pm-app-infrastructure/src/test/java/com/coder/pm/infra/service/` |
| E2 | 业务 controller 单测：用 `@WebMvcTest` | `apps/pm/pm-app-api/src/test/java/com/coder/pm/api/` |
| E3 | e2e Playwright 覆盖 4 个 view 关键路径 | `frontend/tests/e2e/` |
| E4 | OpenAPI yaml 落库（springdoc 生成） | `scripts/dump-openapi-specs.sh` 跑后 commit 到 `apps/pm/openapi.yaml` |
| E5 | `check-vitest-coverage` 接 CI：apps/pm 单测 60% line coverage 起步 |
| E6 | SpotBugs + PMD 检查接 CI（apps/pm 独有） | `.gitlab-ci.yml` 加 stage |
| E7 | E2E 在 docker compose 完整栈上跑 | `tests/e2e/cases/pm.yaml` |

### Phase F：Dev Experience（1 天打包）

| # | 任务 | 文件 |
|---|---|---|
| F1 | **dev profile bypass**：本地 dev 时关 JWT 校验（仅 `dev` profile 启 `@Profile("dev")`） | `apps/pm/.../config/SecurityConfig.java` |
| F2 | 种子数据脚本：`scripts/seed-pm.sh` 创建 demo project + requirements + stories | `apps/pm/` + `scripts/` |
| F3 | apps/pm `AGENTS.md` 加 dev quickstart（`mvn spring-boot:run` 一步起） | `apps/pm/AGENTS.md` |
| F4 | docker-compose-services.yml pm service 加 healthcheck | `infrastructure/docker-compose-services.yml` |
| F5 | OpenAPI 集成测试（springdoc-generated yaml 落库后做 schema 校验） | `tests/e2e/cases/openapi-pm.yaml` |

### Phase G：跨服务架构层（spec §11 大块，本季度以后）

| # | 任务 | 影响 |
|---|---|---|
| G1 | **OAuth2 / OIDC** 替换自定义 HS256 | 接入外部 IdP（Keycloak、Auth0） |
| G2 | **多租户** | SaaS 化前置 |
| G3 | **全文检索**（ES / Meilisearch 接管 KB 搜索） | KB 大规模后必做 |
| G4 | **实时协作**（WebSocket + 操作转换） | 多人编辑 KB 文档 |
| G5 | **审计**（公司合规级别，immutable log） | apps/pm 现有 audit 仅是 SDK demo，真实合规链路需重做 |

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
