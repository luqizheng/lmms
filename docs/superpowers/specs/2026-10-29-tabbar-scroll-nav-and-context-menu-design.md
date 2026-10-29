# 标签栏滚动导航回灌上游 + 右键批量关闭菜单 — 设计文档

> 日期：2026-10-29
> 关联：`docs/prism/phase2-ux-spec.md`（壳体验规范）、`frontend/src/styles/prism/tokens.css`（token 唯一真源）
> 范围：`frontend/src/shell/components/`（TabBar.vue、AppShell.vue、tab-close.ts 及测试）、`frontend/locales/{en,zh-CN}/shell.yaml`
> 风险：低（纯前端壳组件，无后端 / 契约 / 路由结构改动）

---

## 1. 现状

### 1.1 上游 TabBar 是骨架初版

`frontend/src/shell/components/TabBar.vue` 自 PRISM 骨架提交（`d9789e39`）后未再演进：

1. 标签溢出只能拖动原生滚动条，无翻页按钮，当前激活标签也不保证可见。
2. 没有任何批量关闭手段：关闭 N 个标签要逐个点 X。

### 1.2 下游已有更好的滚动实现

下游 project-manager 的 `6a50acd` 给 TabBar 加了滚动导航：隐藏原生滚动条、两端 Chevron 翻页按钮
（按一屏宽度平滑滚动、禁用态跟随）、激活标签滚入可见区、ResizeObserver 跟随宽度变化。
用户裁决（2026-10-29）：**下游版本更好，同步回上游**。壳组件由上游同步到下游
（update-shell skill），所以这版必须先落上游，否则下次同步会被旧版覆盖。

### 1.3 右键菜单两边都没有

关闭本页 / 左侧 / 右侧 / 其他 / 全部 是本次新增功能，上下游都缺。

## 2. 方案

### 2.1 滚动导航：以上游为基线吸收下游实现

上游与下游 TabBar 的唯一结构差异是 `TabItem` 的来源：下游从 `../store/tabs` 引入
（下游有独立 tabs store），上游的标签状态是 AppShell 局部状态、类型由 TabBar 自带导出。
移植时保留上游的 `export interface TabItem`，其余（滚动状态、翻页、可见性、观察器、样式）
逐字取下游版本。

### 2.2 右键菜单：TabBar 管交互，AppShell 管语义

- **打开**：`@contextmenu.prevent` 打开；菜单 `Teleport` 到 body + `position: fixed`
  （不 Teleport 会被 tabbar 的 overflow 滚动容器裁剪）；先落在光标处，`nextTick` 实测尺寸后
  向视口内收 8px。
- **关闭**：点击菜单外（`pointerdown` 捕获）、`Escape`、任意滚动（捕获）、窗口缩放、焦点移出。
  关闭后焦点还给触发右键的标签。
- **键盘无障碍**：`role="menu"` / `role="menuitem"`；打开时聚焦第一个可用项；`↑` `↓` 在可用项间
  循环；菜单项是原生 `button`，Enter/Space 原生激活。
- **五项与禁用矩阵**（`closable !== false` 才算可关，禁用项只是置灰、不可激活）：

| 动作 | 禁用条件 |
|---|---|
| 关闭 | 目标标签 `closable === false` |
| 关闭左侧 | 目标左侧没有可关标签 |
| 关闭右侧 | 目标右侧没有可关标签 |
| 关闭其他 | 除目标外没有可关标签 |
| 关闭全部 | 没有任何可关标签 |

- **事件边界**：TabBar 只发语义事件 `close` / `closeLeft` / `closeRight` / `closeOthers`
  （payload 为目标 path）与 `closeAll`（无 payload）；"关哪些、关完跳哪里"的裁决全部在 AppShell。

### 2.3 批量关闭的回退导航：抽纯函数

AppShell 现有单标签 `closeTab` 的回退是"左邻优先、其次原位置右邻"。批量关闭需要同一条规则，
但触发点可能是非激活标签（比如右键第 3 个标签"关闭右侧"关掉了正在看的第 5 个）。抽出
`tab-close.ts`：

```ts
planTabClose(tabs, shouldClose, activePath) → { remaining, fallback? }
```

- `remaining`：过滤后的剩余标签；
- `fallback`：仅当 `activePath` 命中 `shouldClose` 时，从其原位置**先向左再向右**找最近的幸存标签
  （与既有单标签回退顺序一致）；没被关就不跳转。

AppShell 的单标签 `closeTab` 改走同一函数（行为不变），四个批量 handler 各是一行谓词；
可关性保护（`closable !== false`）只加在批量谓词上，单标签关闭沿用 UI 层禁用。

### 2.4 下游同步方式

下游已有滚动导航与独立 tabs store，本次只把**右键菜单增量**带下去：TabBar 菜单 UI、
`tab-close.ts`、AppShell 接线、5 个 i18n 键。store 保持 `closeTab` 既有 API，
AppShell 批量操作直接用 `tab-close.ts`，不动 store。

## 3. 改动清单

| 文件 | 改动 | 原因 |
|---|---|---|
| `frontend/src/shell/components/TabBar.vue` | 滚动导航移植（commit 1）+ 右键菜单 UI（commit 2） | §2.1 / §2.2 |
| `frontend/src/shell/components/tab-close.ts` | 新增：批量关闭纯函数 | §2.3 |
| `frontend/src/shell/components/AppShell.vue` | `closeTab` 走 `planTabClose` + 4 个批量 handler | §2.3 |
| `frontend/src/shell/components/TabBar.test.ts` | 新增（上游此前无）：滚动 5 条 + 菜单若干 | 回归保护 |
| `frontend/src/shell/components/tab-close.test.ts` | 新增：回退顺序 / 可关保护 | 纯函数单测 |
| `frontend/src/shell/components/AppShell.test.ts` | +右键菜单事件清空标签的接线用例 | 接线保护 |
| `frontend/locales/{en,zh-CN}/shell.yaml` | `tabs.scrollPrev/scrollNext`（commit 1）+ `tabs.close/closeLeft/closeRight/closeOthers/closeAll`（commit 2） | 菜单与翻页文案 |

## 4. 测试计划

- [ ] 单测：`npx vitest run src/shell/components/TabBar.test.ts src/shell/components/tab-close.test.ts src/shell/components/AppShell.test.ts`
- [ ] 类型：`npx vue-tsc --noEmit --skipLibCheck`
- [ ] lint：`npx eslint --max-warnings 0 src/shell`
- [ ] token：`node scripts/check-prism-tokens.js`
- [ ] i18n：`node scripts/check-i18n-namespace.js && node scripts/check-i18n-hardcode.js`
- [ ] 构建：`pnpm build`

## 5. 回滚方案

两个提交分别 `git revert` 即可；无数据迁移、无契约变更、无路由结构调整。
