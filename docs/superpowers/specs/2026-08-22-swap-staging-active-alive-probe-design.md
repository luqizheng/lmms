# 2026-08-22 swap-staging-active.sh alive-probe fix

## 触发

CI job **25062** `deploy:staging` 健康门 fail：

```text
[swap] ⚠ active 已指向 8221dc89，无操作
$ curl -fsS --max-time 10 "$STAGING_BASE_URL/health" | tee /tmp/post-deploy-health.txt
curl: (7) Failed to connect to 192.168.2.31 port 3000 after 0 ms: Couldn't connect to server
```

`0ms connection refused` 是「端口无人 listen」症状，不是慢启动。

## 根因

`infrastructure/scripts/swap-staging-active.sh` 原 48-53 行：

```bash
elif [ "${PREV}" = "${NEW_DIR}" ]; then
  echo "[swap] ⚠ active 已指向 ${NEW_SHA}，无操作"
  exit 0
fi
```

幂等分支**直接 exit 0，不重启容器**。若当前 active 容器栈因 OOM / server reboot / manual stop / 之前 deploy 中 `start.js --deploy` 部分失败 等原因停摆，再次 deploy 同一 SHA 都会过 swap 但撞 0ms refused。

叠加因素：`STAGING_BASE_URL` 在 GitLab Variables 被 override 到 `:3000`，但 staging 当前架构以 frontend `:8080` 为公共入口（infra/README 与 `.gitlab-ci.yml:38` 注释明确「不再依赖 3000 暴露」）。这次不在本次最小变更范围内。

## 修复（最小变更）

| 改 | 文件 / 行 | 内容 |
| --- | --- | --- |
| + | header 注释 | usage 加 `--force`，新增 Idempotent 处理说明 |
| + | arg parsing | `FORCE=0` + `SAME_DIR=0` + `--force)` case |
| ~ | elif (line 48-53) | `exit 0` 改为「cd + status.js 探针 (`regex \b(Up\b|healthy)`) + fall through OR `--force` 强制重启」 |
| + | dry-run | 多打一行 `same-dir=...` 给 ops 看 |
| ~ | step 2 (secrets 复制) | gate 加 `SAME_DIR=0`（同 dir 时 PREV==NEW_DIR，自复制 idempotent 浪费，skip） |
| ~ | step 3 (active 退到 staging-prev) | gate 加 `SAME_DIR=0`（同 dir 时 PREV==NEW_DIR，会覆盖上次 swap 的真 prev，skip 保护） |

`SAME_DIR=1` 的 fallthrough 路径确保走完 step 4 (`ln -sfn` idempotent) + step 5 (`stop.js` + `start.js --deploy`)，容器栈真正被刷新。

## 不动（用户显式排除在本次范围）

- `.gitlab-ci.yml` 的 `STAGING_BASE_URL` / health body 检查（独立 bug）
- `release/dist-template/bin/status.js`（已 dump 表格，够用）
- CI Variables（`:3000` override 是另一个独立问题）
- docs-site/runbooks/* （未沉淀 SOP）

## 验证

| 检查 | 命令 | 状态 |
| --- | --- | --- |
| bash 语法 | `bash -n infrastructure/scripts/swap-staging-active.sh` | ✓ pass |
| git 变更范围 | `git status --short infrastructure/` | 单文件 M，无其它污染 |
| 调用方兼容 | 复用现有 `deploy:staging` / `e2e:staging` rollback / 手动 rollback 三种入口，**都未传 `--force`**： | |

- `deploy:staging` (`.gitlab-ci.yml:508`)：命中 same-dir 分支 → 走 alive 探针分支（原 exit 0 行为被强化为先探针；alive 仍 exit 0，不动）
- `e2e:staging` rollback (`.gitlab-ci.yml:554`)：回退到 staging-prev，PREV≠NEW_DIR，不进 same-dir 分支
- 手动 rollback (`infrastructure/README.md:267`)：同 e2e rollback 路径

**blast radius 闭环**：三种调用方语义不变；新 flag `--force` 暂未在任何 CI 入口启用。

## staging 立即止血（不在本次 commit，操作者手动跑）

```bash
ssh kudi@192.168.2.31
cd /home/kudi/coder-micro-services/staging-active
node bin/stop.js || true
node bin/start.js --deploy
```

## 后续可选（独立 follow-up PR，不在本次）

1. `.gitlab-ci.yml:36` `:8080` 强制对齐（注释明确 + jq 健康判定）
2. `docs-site/runbooks/troubleshooting.md` 加「swap 无操作 + health 0ms refused」SOP
3. `release/dist-template/bin/status.js` 结构化输出（JSON），让 swap 探针用更严格 contract 而不是 regex 抓表格
4. 修 `:3000` CI Variables override（如果是项目自定，再评估）
