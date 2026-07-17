# ADR-002: Service Standard Baseline

> 日期：2026-06-25
> 状态：已采纳
> 关联：[P1.3 设计](../superpowers/specs/2026-06-25-service-standard-sweep-design.md) / [P1.3 实施](../superpowers/plans/2026-06-25-service-standard-sweep-plan.md)

## 上下文

随服务数量从 2 长到 8，各服务 AGENTS.md、端点接入、环境变量命名出现漂移。`scripts/check-service-standard.js` 早期规则只识别 6 项，且把"健康检查端点"判定为不存在的 `src/health.rs`，无法守护规范。

## 决议

1. **必需组件 8 项**（每个服务必须满足，豁免必须显式声明）：
   - Dockerfile / AGENTS.md / README.md / tests/ 目录
   - `/health` 端点 / `/metrics` 端点
   - `migrations/` 目录（无 DB 服务豁免）
   - Swagger 配置（`SWAGGER_ENABLED` 开关 + `utoipa-swagger-ui` 依赖）
2. **统一环境变量名**：`SERVER_HOST` / `SERVER_PORT` / `SERVER_ADDR` / `DATABASE_URL` / `RUST_LOG` / `SWAGGER_ENABLED` / `CONSUL_ADDR`
3. **豁免规则**：在 `AGENTS.md` 的 `## 豁免` 段每行一项写出，被检查脚本识别并跳过
4. **CI 强制**：`check:service-standard` / `check:spec` / `contracts:check` 三项 test 阶段闸门，任一红则阻断 build/deploy
5. **新服务**：必须从 `services/agents-template.md` 复制 AGENTS.md

## 后果

- 新服务上线必须先满足 8 项；CI 自动守护，避免人工 review 遗漏
- `check-service-standard.js` 与模板是绑定关系：模板改了脚本规则必须同步改
- Swagger 默认 `SWAGGER_ENABLED=false`；生产开启需在部署清单中显式声明
- 历史日志（build-*.log）禁止提交（`.gitignore` 拦截）
