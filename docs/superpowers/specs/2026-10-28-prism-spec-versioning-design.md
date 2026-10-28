# PRISM 规范纳入版本管理 — 设计文档

> 日期：2026-10-28
> 关联：`docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`（壳设计）、`frontend/src/styles/prism/tokens.css`（token 唯一真源）、`scripts/check-prism-tokens.js`（token 门禁）
> 范围：`docs/prism/`（新增）、`scripts/check-prism-spec-sync*.js`（新增）、`frontend/src/styles/prism/tokens.test.ts`（改为文档解析）、`docs/superpowers/{specs,plans}/*-prism-*.md`（追写路径注记）、`.gitlab-ci.yml`（接线）
> 风险：低（纯文档镜像 + 校验脚本；不动任何视觉数值）

---

## 1. 现状

### 1.1 具体行为

PRISM 的散文版规范在**版本库之外**：

```
D:\Workspace\web-concept\design\
├── phase1-visual-direction.md          # 主规范（Phase 1R v2.0）
├── phase2-ux-spec.md                   # 体验规范（Phase 2 v1.1）
└── phase1-visual-dark-v1.archived.md   # 暗色版（已废弃归档）
```

仓库内共 **6 处**引用这个仓库外绝对路径（`tokens.css` 头注释 1 处 + spec 1 处 + plan 3 处），而这些路径在 CI 机器上、换一台开发机、或换一个团队成员时**都不存在**。

### 1.2 根因

- 数值契约**在**版本库内：`tokens.test.ts` 把 Phase 1R 附录「CSS 变量速查」的每个值硬编码成 `APPENDIX_VALUES`（87 个变量），逐个与 `tokens.css` 比对。
- 散文契约**不在**：附录所在的文档、"为什么这么定"的推导、组件五态定义、响应式断点结论、Phase 3 衔接注意点，都只在仓库外。
- 硬编码副本是**手工誊抄**的：规范改了、测试里的副本没改，两边静默漂移，直到有人手工比对才发现。

### 1.3 影响

| 谁受影响 | 后果 |
|---|---|
| CI / 新机器 | 拿不到散文契约，token 门禁只能校验"代码自洽"，校验不了"代码是否忠于规范" |
| 接手的人 | 只能靠口述 + 翻 `D:\` 下的散文档才能知道设计意图 |
| 设计侧 | 规范改动没有 diff、没有评审、没有历史 |

## 2. 目标与非目标

### 目标

1. 三份规范进入版本管理，仓库内成为**唯一可提交、可评审、可回溯**的副本。
2. **散文 ↔ token 的漂移可被机器发现**：不再靠手工誊抄的 `APPENDIX_VALUES`，改为直接解析规范文档的附录。
3. 保留"设计侧工作副本"这个现实：外部目录继续存在，但降级为**草稿区**，改动通过显式回灌进入仓库。

### 非目标

- 不改规范内容——镜像**逐字**照搬，一个字都不动（含已废弃的暗色版，作为决策留痕一并归档）。
- 不改任何视觉数值、token、组件。
- 不删除外部目录——那是设计侧自己的工作区。

## 3. 方案

### 3.1 目录落位

```
docs/prism/
├── README.md                            # 真源关系 + 改动流程 + 回灌命令
├── phase1-visual-direction.md           # 镜像（逐字）
├── phase2-ux-spec.md                    # 镜像（逐字）
└── phase1-visual-dark-v1.archived.md    # 镜像（逐字，已废弃）
```

选 `docs/prism/` 而不是 `docs-site/`：`docs-site/` 是给下游/部署者看的对外站点，设计规范是**内部真源**，与 `docs/superpowers/`（研发过程文档）并列更合适。

### 3.2 真源关系

| 位置 | 角色 | 谁能改 |
|---|---|---|
| `docs/prism/*.md`（仓库内） | **权威副本**：唯一可提交、可评审、有 diff 有历史 | 所有人，改完直接提 PR |
| `D:\Workspace\web-concept\design\`（仓库外） | 设计侧**草稿区**：写作用 | 设计侧，改完需回灌 |

选"仓库内为权威"而不是"外部为权威 + 仓库内为镜像"，是因为后者在 CI 上无法校验（CI 机器没有那个目录），散文契约等于仍游离在版本管理之外。

### 3.3 漂移检测（两道）

**① 散文 ↔ token（CI 必跑，最关键）**

改造 `frontend/src/styles/prism/tokens.test.ts`：删掉手工誊抄的 `APPENDIX_VALUES`（87 行硬编码），改为**解析 `docs/prism/phase1-visual-direction.md` 附录里的 `:root { ... }`**：

```
附录 ──解析──> 期望值 ──比对──> tokens.css 实际值
```

规范附录改了而 `tokens.css` 没改 → 测试红。**这是本次改动的核心价值**：把散文和代码焊在一起。

解析规则（附录是标准 `:root {}` 块，机器可解析）：
- 定位「## 附录：CSS 变量速查」小节 → 取其 ```css 围栏块
- 正则 `/--([a-z0-9-]+)\s*:\s*([^;]+);/g` 逐变量取值（附录里多个变量写在同一行也要能取到）
- 值比较忽略全部空白（prettier 会规范化 `rgba()` 内外空格）

**② 仓库内副本 ↔ 外部草稿（本地跑，CI 无外部目录时跳过）**

新增 `scripts/check-prism-spec-sync.js`：
- 默认：比对 `docs/prism/*.md` 与外部目录同名文件，逐文件报告"一致 / 仓库落后 / 外部落后"
- `--sync`：把外部文件**回灌**到仓库（单向：外部 → 仓库），并提示"回灌后请提交"
- 外部目录不存在（CI / 别人机器）→ 打印说明并 **exit 0**，不阻断 CI
- 附 `--mirror-root <dir>` 便于自测与 CI 指向其他副本

按仓库既有惯例（`check-prism-tokens.js` 有 `check-prism-tokens.test.js`），同步脚本必须有自测 `scripts/check-prism-spec-sync.test.js`——校验器自身的洞不能静默存在。

### 3.4 引用改写

| 位置 | 改法 |
|---|---|
| `frontend/src/styles/prism/tokens.css` 头注释 | 权威来源改为仓库内路径（这是活文档，必须准） |
| `docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md` | **追写**路径注记（不改原正文） |
| `docs/superpowers/plans/2026-10-26-prism-*.md`（3 个） | **追写**路径注记（不改原正文） |

历史 spec/plan 采用"追写一行注记"而非改正文：历史文档记录的是当时的事实，改写等于篡改；注记说明"该路径已迁入本仓库 `docs/prism/`"即可。

### 3.5 CI 接线

`prism_tokens_check` job 增加一行 `node scripts/check-prism-spec-sync.js`（外部目录不存在时自动跳过），并把 `docs/prism/**` 加进该 job 的 `rules.changes`——改规范镜像时必须跑门禁。

## 4. 影响面

- 视觉：**零影响**。token、组件、页面都不动。
- 测试：`tokens.test.ts` 的期望值来源从"硬编码副本"变成"解析文档"，断言数量与语义不变（附录变量存在性 + 值等价）。
- 新增 `docs/prism/` 约 85KB 文档（散文），进入版本管理后可 diff、可评审。
- 外部目录继续可用；`--sync` 是唯一的回灌通道。

## 5. 验证

```bash
node scripts/check-prism-spec-sync.js            # 副本 vs 外部草稿（应报一致）
node --test scripts/check-prism-spec-sync.test.js # 脚本自测
node scripts/check-prism-tokens.js               # token 门禁（应仍通过）
node scripts/check-doc-links.js                   # 文档链接门禁
cd frontend && npx vitest run src/styles/prism    # token 契约（含文档解析版）
```

## 6. 后续待办

1. `docs/prism/README.md` 里应写清"编辑入口 = 仓库内那份"；若设计侧更习惯在外部写，靠 `--sync` 回灌。
2. `phase1-visual-dark-v1.archived.md` 归档进仓库后，应确认没人再拿它当现行规范（README 里标注"已废弃"）。
3. 规范迁入后，PRISM 路线图里"计划 7（pm 迁移）"仍可按原计划推进；本次不改变任何计划状态。
