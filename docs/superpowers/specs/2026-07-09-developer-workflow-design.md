# Developer Workflow 总图（`docs-site/developer-workflow.md`）

## Status: 🟡 待 review

新人入职完成后（约 Day 3），日常开发工作（拿任务 → 改 → 测 → 推 → 出镜像 → 部署 → 观测）的活动动作链当前散落在多处：
- `docs-site/onboarding/`（9 篇课程，新人第 1–3 天跟读）
- `docs-site/guides/`（10 篇专题，例如 `testing.md`、`canary-rollout.md`）
- `scripts/` + `.gitlab-ci.yml`（自动化脚本和 CI 配置）

新人通读完 onboarding 后，对"日常怎么推一次代码"仍要靠口头交接；老员工离开 onboarding 几天后再回去查也费时。

---

## 1. 现状盘点

| 现有体系 | 定位 | 局限 |
|---|---|---|
| `docs-site/onboarding/01–09` | 线性入门课（Day 1–3 通读） | 一次性走完之后日常工作无单一入口 |
| `docs-site/guides/testing.md` 等 | 深度专题（每篇独立） | 没有一张总图串联 |
| `scripts/*.js` + `.gitlab-ci.yml` | 自动化脚本 | 命令分散，需记忆 |

尤其"出镜像"那条线（`scripts/build-docker.js`、CI build stages、release tag 打 `:sha` + `:latest` 两 tag 的逻辑），目前 onboarding/08 仅一句带过，新人首次出镜像要摸 3 个文件。

---

## 2. 范围

### 2.1 在范围

- 新建顶层 `docs-site/developer-workflow.md`（**单文件、索引型**），覆盖 10 个 Phase：
  - Phase 0 拿代码 → Phase 1 spec → Phase 2 实现 → Phase 3 测试 → Phase 4 CI Gate
  - → Phase 5 提交 → Phase 6 CI 自动 → Phase 7 出镜像 → Phase 8 部署/灰度 → Phase 9 观测
- 每 Phase 5–15 行：一句话 + 关键命令 + 详细文档跳转
- **顶部 ASCII pipeline 图** 让 5 分钟看完全局
- 与 `docs-site/onboarding/README.md` 互链：
  - `developer-workflow.md` 顶部 💡 "新人先走 onboarding 再回总图"
  - `onboarding/README.md` 末尾 "日常活动总图" 段落

### 2.2 不在范围

- ❌ 不重写 onboarding/01–09 现有内容
- ❌ 不动 guides、scripts、CI 任何代码
- ❌ 不替代任何深度文档（speed reference + cross-link）

---

## 3. 设计取舍

### 索引型 vs 手册型

**选索引型**（推荐）。理由：
- 已有 `testing.md`（496 行）、`canary-rollout.md`、`api-gateway-routing.md` 等深度文档
- 新增手册会重复维护负担，且新人 Day 1 看不懂、太长不看
- 索引型 + 跳转：5 分钟看完全局，30 秒跳到需要的那一页

### Phase 0 必须包含 `pnpm run setup`

Day 1 真正卡住的不是装工具（= `pnpm install`），而是**预拉基镜像**——Harbor `official/*` 是 proxy cache，部分 tag 为空，要走 mirror → source 回落链路。`pnpm run setup` 串好 install + sync + pull + check:env，正是 Day 1 正确起点。

### Phase 7 不重复造轮子

镜像发布那条线已经有：
- `scripts/build-docker.js`（123 行）—— 仅 build，输出 9 个服务的本地镜像
- `scripts/release-push.js`（87 行）—— 打 `:${sha}` + `:latest` tag、fail-fast push（**已于 2026-08 废弃，功能合并进 build-docker.js 的 `--push` / `--load-and-push`**）

总图里只描述**两条路径**（CI 自动 + 本地手动 hotfix），不重写脚本使用说明。Phase 7 顶部一句"前置"指向 `infrastructure/base-images.json` 是单一真相源。

---

## 4. 文件改动清单

| 文件 | 操作 | 说明 |
|---|---|---|
| `docs-site/developer-workflow.md` | 新建 | ~310 行（10 Phase + ASCII 图 + 速查表 + 关键路径索引） |
| `docs-site/onboarding/README.md` | 末尾新增"日常活动总图"段落 | 链 `../developer-workflow.md` |

无脚本、无 CI 改动。
