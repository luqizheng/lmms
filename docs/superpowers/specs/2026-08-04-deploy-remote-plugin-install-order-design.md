# deploy-remote.sh 插件安装顺序修复 — 设计

日期：2026-08-04
状态：待执行
范围：`infrastructure/scripts/deploy-remote.sh`（与 `.gitlab-ci.yml` 的 deploy job 配合）

## 背景与问题

Pipeline 6240 (commit fefd34c) deploy job (id 20736) 失败，trace 最后一行：

```
[deploy-remote] FATAL: docker CLI not found at all; cannot install compose plugin
```

来源：`infrastructure/scripts/deploy-remote.sh:46-50`。

### 矛盾点

`STAGING_DEPLOY_USER=kudi`、`STAGING_HOST=192.168.2.128`。同一 SSH 通道下：

- ping 命令 `docker --version` **成功**（输出 `Docker version 29.5.3, build d1c06ef`），说明 docker 二进制在 PATH 中
- 但 deploy-remote.sh 内部 `command -v docker` 失败 → FATAL

人工 SSH staging 端确认：

- `/usr/bin/docker` 存在
- `/usr/libexec/docker/cli-plugins/docker-compose` 存在（mtime 2026-05-21）
- PATH = `/usr/local/bin:/usr/bin:/bin:/usr/games`

理论上 deploy-remote.sh 跑时 plugin 探测循环（line 36-44）应该命中 plugin → break + `export PATH` → 新 PATH 第一项就是 plugin dir → `command -v docker-compose` 命中 → 整个 if 短路 → **不进 FATAL**。

但 trace 进 FATAL。唯一合理解释：**deploy 时刻 staging 端 plugin 文件不存在**，是 deploy 失败后手动装的。

### 根因

`deploy-remote.sh` 的 plugin 安装兜底逻辑（line 52-66）在 **line 46-50 之后**：

```bash
# line 46-50: 检查 docker + plugin 都在 PATH
if ! command -v docker-compose >/dev/null 2>&1 \
   && ! command -v docker >/dev/null 2>&1; then
  ...FATAL exit 10
fi

# line 52-66: plugin 不在就 apt install（兜底，永远跑不到）
if ! command -v docker-compose >/dev/null 2>&1 && ! docker compose version >/dev/null 2>&1; then
  sudo -n apt-get install -y docker-compose-plugin ...
fi
```

**任何让 line 46-50 进 FATAL 的情况，line 52-66 永远跑不到**。plugin install fallback 是死代码。

最近的 `fix(deploy)` commits 都建立在"line 46-50 不会 FATAL"的假设上：

- `e0db5b0 fix(deploy): prepend docker cli-plugins to PATH` — 假设只是 PATH 问题
- `4ad5219 fix(deploy): install docker-compose-plugin when missing` — fallback 跑不到

两者都未触及 line 46-50 的判断逻辑本身，导致连续 10+ 个 main pipeline deploy 失败。

## 设计决策

**重排 line 46-94 的检查顺序：先尝试安装，再决定是否 FATAL**。

### 新顺序（line 36-94 重写）

```
1. line 36-44（不变）: 探测 plugin dir 并 export PATH（已有 plugin 时）
2. 新增: 检查 docker 命令；不在则尝试 apt install docker.io
3. 新增: 检查 docker compose plugin（`command -v docker-compose` 或
   `docker compose version`）；不在则尝试 apt install docker-compose-plugin；
   装完后再 probe plugin dir 一次
4. 兜底: docker 命令仍然找不到 → FATAL exit 10（真问题，需要人工介入）
5. cd 到 PROJECT_ROOT（line 68，不变）
6. STEP 0 preflight（line 92-94，不变）: `docker compose version` 是最终防线
```

### 为什么这样改

- **不引入新逻辑分支**：保留 plugin dir 探测 + apt install 兜底，只是把它们**提前到 FATAL 之前**。
- **保留 STEP 0（line 92-94）的 `docker compose version` 检查**：作为最终防线，apt install 仍失败时这一行会 `die` 退出。
- **不破坏现有行为**：plugin 已经装好的 staging（用户当前 staging），line 36-44 + 新增的 `command -v` 都会立刻短路，跳过 apt install。
- **首次部署的 staging**（plugin 未装）：apt install 会跑一次（~30s），装完 probe 成功 → 继续。

### 修改的文件

- `infrastructure/scripts/deploy-remote.sh`：替换 line 36-94 的检查段

### 不修改的内容

- `infrastructure/scripts/start-prod.sh`、`stop-prod.sh`
- `.gitlab-ci.yml` 的 deploy job 自身（before_script 的 SSH/插件探测不变）
- staging 端的任何预装脚本

## 验证步骤

部署后人工验证：

1. **bash 语法检查**：`bash -n infrastructure/scripts/deploy-remote.sh`
2. **shellcheck**（如可用）：`shellcheck infrastructure/scripts/deploy-remote.sh`
3. **当前 staging 端 dry-run**（plugin 已装，验证不重复 install）：
   ```bash
   cd /home/kudi/coder-micro-services
   bash infrastructure/scripts/deploy-remote.sh 2>&1 | head -30
   ```
   期望：直接走到 STEP 0，不重复 apt install，不 FATAL。
4. **模拟 plugin 缺失场景**：临时把 plugin 文件挪开再跑，看 fallback 是否生效：
   ```bash
   sudo mv /usr/libexec/docker/cli-plugins/docker-compose{,.bak}
   bash infrastructure/scripts/deploy-remote.sh 2>&1 | head -30
   sudo mv /usr/libexec/docker/cli-plugins/docker-compose{.bak,}
   ```
   期望：log 含 `docker compose plugin not found; attempting apt install`，装完继续。
5. **下次 main pipeline 自动验证**：commit 触发 GitLab CI deploy job（runner 192.168.2.31，SHA 由 git push 决定）。

## 风险评估

- **高风险**：deploy 脚本一旦改错会阻塞下次 deploy。修复点只动 line 36-94 的检查段，不动后续 STEP 1-4（pull / rolling restart / healthcheck / prune），并且保留 STEP 0 兜底。
- **回滚方案**：单个 commit `fix(deploy): reorder docker compose plugin install before FATAL check`，可 revert 后重新 push。
- **人工门禁**：deploy 路径按 LOOP.md §Human Gates 仍属"all high-risk paths: human review required"；spec 在 commit 同周期内创建，让 PR reviewer 看得到变更理由。

## 不在范围内

- 不修 CI runner 端 apt-get install rsync 8 分钟慢的问题（fefd34c 之前的 commit chain）——属于运维/镜像预装范畴，独立 issue
- 不修 SSH_OPTS 变量未 export 的小 bug（独立 fix，可后续单独 commit）
- 不改 .gitlab-ci.yml 的 deploy job 整体结构