# 侧栏菜单节奏 — 设计文档

> 日期：2026-10-28
> 关联：`docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`（壳设计）、`frontend/src/styles/prism/tokens.css`（token 唯一真源）
> 范围：`frontend/src/shell/components/SidebarMenuItem.vue`（样式 + 一个 class）、`Sidebar.test.ts`（新增 1 条用例）
> 风险：低（纯 CSS 节奏调整，不动结构、不动后端）

---

## 1. 现状

### 1.1 度量

侧栏菜单项行高 36px（`--size-control-md`），但兄弟节点之间一律 `margin-top: var(--space-6)`（24px）：

```css
.ui-sidebar__group + .ui-sidebar__group { margin-top: var(--space-6); }
```

实测（真实 token + 真实组件样式，17 个菜单项的典型树）：

| 指标 | 值 |
|---|---|
| 相邻项垂直间距 | **60px**（36px 行高 + 24px 间距） |
| 菜单内容总高 | **948px** |
| 760px 视口下是否需要滚动 | **需要**（一级菜单放不完） |

### 1.2 根因

24px 本意是"分组之间的分隔"，但选择器 `.ui-sidebar__group + .ui-sidebar__group` 命中**任意层级**的相邻兄弟节点——同一分组里的普通子项也拿到了 24px。后果有两个：

1. **密度**：项节奏 60px，约为常规 admin 侧栏（36–40px）的 1.5 倍，15 项就要滚一屏。
2. **语义丢失**：分组之间与同级项之间间距完全相同（都 24px），间距不再承载分组信息，每一项看上去都像一个独立分组。

## 2. 目标

1. 同级项紧凑，项节奏回到 40px（36px 行高 + 4px 间距）。
2. 分组（带子列表的节点）前后仍有清晰分隔，让层级可读。
3. 只用 PRISM 已有 token，不引入新数值（遵守"不自造数值"约束）。

## 3. 方案

把"同级间距"和"分组间距"拆成两个语义：

| 关系 | 旧 | 新 | token |
|---|---|---|---|
| 同级项之间 | 24px | **4px** | `--space-1` |
| 分组（带子项）前后 | 24px | **20px** | `--space-5` |
| 父项 → 其子列表 | 4px | **8px** | `--space-2` |

实现方式：给带子项的 `li` 加 `is-group` 修饰类，用两条相邻兄弟规则区分：

```css
.ui-sidebar__group + .ui-sidebar__group { margin-top: var(--space-1); }
.ui-sidebar__group + .ui-sidebar__group.is-group,
.ui-sidebar__group.is-group + .ui-sidebar__group { margin-top: var(--space-5); }
```

**为什么用 class 而不是 `:has()`**：`:has(> .ui-sidebar__sublist)` 能少一个修饰类，但 `:has()` 的浏览器基线（Chrome 105+/Safari 15.4+/FF 121+）高于本项目内网环境可控范围；显式 class 在任何浏览器上行为一致，也不依赖结构推断。

## 4. 效果

同一棵菜单树、同一套 token，改前改后实测：

| 指标 | 改前 | 改后 | 变化 |
|---|---|---|---|
| 相邻项垂直间距 | 60px | **40px** | −33% |
| 菜单内容总高 | 948px | **724px** | −224px（−23.6%） |
| 760px 视口下 | 需滚动 | **一屏放下** | — |

## 5. 验证

```bash
cd frontend
npx vitest run src/shell/components/Sidebar.test.ts   # 含新增的 is-group 用例
npx vue-tsc --noEmit --skipLibCheck                   # 类型
npx eslint --max-warnings 0 src/shell                 # lint
cd .. && node scripts/check-prism-tokens.js           # token 校验
```

**视觉自审**：用真实 `tokens.css` + 真实组件样式生成静态预览页（`chrome-headless-shell --screenshot`），在 420×760 视口下改前 / 改后各截一张，并改后追加测量脚本输出菜单总高与项间距。预览页由脚本从 `.vue` 源码抽取 `<style scoped>` 生成，不手抄 CSS，避免与组件漂移。

## 6. 影响面

- 侧栏展开态与收起态（图标栏）同时受益——两者共用同一套项间距。
- 顶栏 / 标签栏 / 内容区不受影响。
- 下游业务系统拷贝壳后同样生效。

## 7. 后续待办

1. 菜单项 36px 行高沿用 `--size-control-md`；若后续要更紧凑的"高密度模式"，应加 token 变量而不是就地改数值。
2. 收起态（64px 图标栏）目前同样按 4px 节奏堆叠，未单独评估密度——如需图标栏更紧凑，可单独定 token。
