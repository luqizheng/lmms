# project-service

> Phase 9 验证 MVP 业务服务（Java + Spring Boot 3 + Java 25 + JPA + Flyway + springdoc-openapi）。
> 对应 spec：`docs/superpowers/specs/2026-07-11-pmd-java-mvp-design.md`。

## Quickstart

```bash
# 假设 MySQL / RabbitMQ 已启动；环境变量：
export DATABASE_URL='jdbc:mysql://127.0.0.1:3306/superpowers?useSSL=false&serverTimezone=UTC'
export PMD_LLM_PROVIDER=placeholder

# 本地 Maven 构建
cd services/project-service
./mvnw -o -DskipTests package          # 产 fat jar: target/project-service.jar
./mvnw test                          # 单测（后续 T6/T7 落地）

# 启动
java -jar target/project-service.jar
```

服务启动后：

| 路径 | 用途 |
|---|---|
| `http://localhost:8080/project/health` | 健康检查 |
| `http://localhost:8080/project/metrics` | Prometheus scrape |
| `http://localhost:8080/swagger-ui` | SpringDoc UI（dev 模式 `SWAGGER_ENABLED=true`） |
| `http://localhost:8080/api-docs/openapi.yaml` | OpenAPI yaml（gateway 抓取后 committed 到 `services/project-service/openapi.yaml`） |

## API Endpoints

| Method | Path | 说明 |
|---|---|---|
| `POST`   | `/project/projects` | 创建项目 |
| `GET`    | `/project/projects?page=&size=` | 分页列表 |
| `GET`    | `/project/projects/{id}` | 详情 |
| `PATCH`  | `/project/projects/{id}` | 编辑 |
| `DELETE` | `/project/projects/{id}` | 归档（软删，status='archived'，发 `pmd.project.archived`） |
| `POST`   | `/project/projects/{id}/milestones` | 创建 milestone |
| `POST`   | `/project/projects/{id}/tasks` | 创建 task |
| `GET`    | `/project/tasks?projectId=&status=` | 任务列表（按 tenant + status） |
| `GET`    | `/project/tasks/{id}` | 任务详情 |
| `PATCH`  | `/project/tasks/{id}` | 编辑（status 走状态机，`pmd.task.status_changed`） |
| `POST`   | `/project/sprints` | sprint 创建 |
| `GET`    | `/project/sprints` | sprint 列表 |
| `POST`   | `/project/llm/refine` | LLM 细化需求（同事务写 `pmd_llm_refinement_log` + `pmd_outbox_events`，发 `pmd.llm.refinement.requested`） |

## 部署

镜像由 `services/project-service/Dockerfile` 多阶段构建（`eclipse-temurin:25-jdk` → `eclipse-temurin:25-jre-alpine`）。
Compose 服务定义见 `infrastructure/docker-compose-services.yml` 的 `project-service` 段（端口 6008 → 8080）。

事件副作用由 `pmd-outbox-publisher`（Rust，独立 bin，同镜像多 CMD）轮询 `pmd_outbox_events` 发布到 RabbitMQ。审计消费由 `audit-service` 通过 `audit.pmd-events` 队列订阅 `pmd.#`。

## 相关文档

- spec：`docs/superpowers/specs/2026-07-11-pmd-java-mvp-design.md`
- 计划：`.omo/plans/project-mvp.md`（任务 T2/T6/T7/T9 关联本 README）
- DB 约定：`contracts/db-conventions.md`
- 合约：`contracts/permissions.yaml` / `error-codes.yaml` / `events.yaml`
