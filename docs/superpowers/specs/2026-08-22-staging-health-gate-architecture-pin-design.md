# 2026-08-22 staging health gate 架构钉死

## 触发

job 25062 `deploy:staging` 健康门 fail：
swap-staging-active.sh 幂等盲区（spec 2026-08-22-swap-staging-active-alive-probe-design.md）+ `STAGING_BASE_URL` 在 GitLab Variables 里被 override 到 `:3000` 叠加。

`swap-staging-active.sh` 那个幂等盲区在 #1 spec 处理。本 spec 处理 `STAGING_BASE_URL` env drift 风险，路径不同。

## 根因

yml:36 默认 `STAGING_BASE_URL: "http://192.168.2.31:8080"`。  
yml:34-35 注释 + infra/scripts/start-prod.sh:83 明确：「api-gateway:3000 在 prod 下不暴露 host」。但 GitLab Variables 仍可被 override 成 `http://192.168.2.31:3000`。

yml:510 health gate 形如：

```yaml
- 'curl -fsS --max-time 10 "$STAGING_BASE_URL/health" | tee /tmp/post-deploy-health.txt'
```

整串依赖 `$STAGING_BASE_URL`，drift = 0ms refused。  
对比 yml:541-542 e2e:api/ui 是显式 `--base-url=:8080`，对 env drift 免疫；yml:510 是**唯一**依赖 `$STAGING_BASE_URL` 整串的 gate。

## 修复

yml:510 健康门 URL 拼法从 `${STAGING_BASE_URL}` 改成 `${STAGING_HOST}:8080`：

| 项 | 改前 | 改后 |
| --- | --- | --- |
| URL | `http://${STAGING_BASE_URL}/health` | `http://${STAGING_HOST}:8080/health` |
| 拼出 | `http://192.168.2.31:3000`（drift 时）/ `:8080`（默认） | `http://192.168.2.31:8080`（恒定架构公共入口） |
| 依赖 vars | `STAGING_BASE_URL`（drift-prone） | `STAGING_HOST`（字面、几乎不动） |

`STAGING_HOST`（yml:29 `"192.168.2.31"`）远不如 `STAGING_BASE_URL` drift-prone；`${STAGING_HOST}:8080` 即「frontend 容器在 host 上的入口」= 架构公共入口。drift-proof：再有人改 GitLab Variables 改 STAGING_BASE_URL 都不影响该 gate。

## 不动

- 不动 `STAGING_BASE_URL` 变量本身（其他 job / MR url badge 仍用）
- 不动 `STAGING_HOST`（一直是字面值，未在 GitLab Variables 漂移）
- 不动 yml:541-542（已硬编码 :8080，本身就免疫）
- 不动 GitLab UI（operator 手动操作；本次 commit 已 gate 级免疫，仍建议清理 override 避免误导）

## 验证

| 检查 | 命令 | 期望 |
| --- | --- | --- |
| yml 语法 | `node -e "yaml.loadFile(...)"` 或 yml lint（CI `spec_check` 内置） | pass |
| 改动范围 | `git diff --stat` | 仅 .gitlab-ci.yml |
| URL 拼出等价 | 改前后在 default 下都拼出 `http://192.168.2.31:8080/health` | 等价 |
| drift 行为 | `STAGING_BASE_URL=http://192.168.2.31:3000` 注入下，gate 仍打 :8080 | 命中设计意图 |
| blast radius | 改的就是 yml:510 一行；其他 job 对 STAGING_BASE_URL 用法未动 | 闭环 |

## 配套（不在本 commit）

GitLab UI (Settings > CI/CD > Variables) 删 `STAGING_BASE_URL=:3000` override 后，yml 默认 :8080 生效，所有 job 都打架构入口。本次 commit 已让 health gate 免疫此 override，但仍建议清掉避免其他工具 / 文档引用错值。
