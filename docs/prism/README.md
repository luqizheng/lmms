# PRISM 设计规范（仓库内权威副本）

> 本目录是 PRISM 设计系统的**散文版规范**在版本库中的权威副本。
> 2026-10-28 起，规范不再只存在于某台开发机的本地目录里。
>
> ⚠️ **2026-10-30：框架仓库已移除 PRISM console 前端**（`frontend/` 目录），
> 下文「与代码的关系」中指向 `frontend/src/styles/prism/tokens.css` 等路径的
> 焊点已随之上移到各业务仓——各业务仓自己的 `styles/prism/tokens.css` 是
> 该仓的 token 唯一真源。本目录保留为设计规范文档本身。

## 文件

| 文件 | 内容 | 状态 |
|---|---|---|
| `phase1-visual-direction.md` | PRISM v2.0 · Phase 1R 视觉方向规范（色彩 / 字体 / 布局 / 材质 / 组件五态 / 动效），末尾「附录：CSS 变量速查」是前端 token 的直接来源 | **现行** |
| `phase2-ux-spec.md` | Phase 2 v1.1 体验规范（信息架构 / 响应式五断点 / 无障碍 / 交互状态 / 反馈节奏 / 文案语气） | **现行** |
| `phase1-visual-dark-v1.archived.md` | Phase 1 v1.0 OBSIDIAN 深色版 | **已废弃**，仅作决策留痕，不得当作现行规范 |

三份文件均为**逐字镜像**：不做任何改写。修改规范请在源文件改完再回灌（见下）。

## 与代码的关系

```
phase1-visual-direction.md
        │  「附录：CSS 变量速查」的 :root 块
        ▼
frontend/src/styles/prism/tokens.css        ← token 唯一真源（逐字转录）
        │  组件只能引用这里的变量
        ▼
frontend/src/components/ui/ · frontend/src/shell/
```

**焊点**：`frontend/src/styles/prism/tokens.test.ts` 直接**解析本目录的规范文档**，逐个变量比对 `tokens.css`；规范附录改了而 `tokens.css` 没改，测试立刻变红。历史上这份期望值是手工誊抄在测试里的副本——誊抄本来就会漂移（已经漂过一次：`0.30` 被写成 `0.3`），现在改成从规范解析，漂移无处藏身。

## 与设计侧草稿区的关系

| 位置 | 角色 | 改动方式 |
|---|---|---|
| `docs/prism/`（本目录） | **权威副本**：唯一可提交、可评审、有 diff、有历史 | 直接改，提交 |
| `D:\Workspace\web-concept\design\` | 设计侧**草稿区**：写作用 | 改完回灌 |

比对与回灌：

```bash
node scripts/check-prism-spec-sync.js            # 比对：副本 vs 草稿区，报漂移
node scripts/check-prism-spec-sync.js --sync     # 回灌：草稿区 → 仓库（单向，不删仓库侧文件）
```

CI 机器上没有草稿区时，脚本会打印说明并以 0 退出，不阻断流水线；仓库内副本与 `tokens.css` 的一致性由前端单测保证。

## 改动规范的标准动作

1. **先改规范**（本目录里的文件，或草稿区改完 `--sync` 回灌）。
2. 同步改 `frontend/src/styles/prism/tokens.css`（附录变量逐字转录；正文内联数值进「PRISM 补充 token」段并标注来源章节）。
3. 跑门禁：

   ```bash
   node scripts/check-prism-spec-sync.js
   node scripts/check-prism-tokens.js
   cd frontend && npx vitest run src/styles/prism
   ```

4. 提交。正文改了还要回头检查 `phase2-ux-spec.md` 是否需要同步（Phase 1R 优先，冲突以 Phase 1R 为准并回报）。

## 为什么这样定

- **单一权威源**：代码里的 token、规范里的数值，必须来自同一份文档；此前它们分居两处，靠人同步。
- **散文契约也要版本管理**：设计意图（为什么是暖灰、为什么 accent 只干三件事）比数值更难在口耳相传中保真，必须留在 git 里才有 diff 和评审。
- **机器优先**：能机器校验的就不靠文档喊——数值比对、裸色值拦截、WCAG 对比度，都已经是 CI 门禁。
