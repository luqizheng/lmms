# PRISM Admin UI — 设计文档

> 日期：2026-10-25
> 状态：已评审定稿（builder check 修正 + 业主两项裁决已并入），待批准后进入实现计划
> 上游输入：`D:\Workspace\web-concept\design\phase1-visual-direction.md`（PRISM v2.0 视觉规范）、`phase2-ux-spec.md`（Phase 2 v1.1 体验规范）
> 范围：替换 `frontend/` 现有 vue-pure-admin 前端，建立公司自有 admin-ui 壳 + 基础包 + AI 开发 skill 体系

---

## 1. 意图与动机

用自有设计系统 **PRISM** 重写 admin 前端，替换 vue-pure-admin（完整版）。三个驱动力：

1. **白标/品牌**：商户与竞争对手不应识别出我们在用开源免费模板。pure-admin 默认视觉辨识度过高（登录页、布局、演示页）。PRISM 是公司私有设计系统，从未开源，天然不可识别。业务系统级白标通过覆盖 accent 色板 + logo + 产品名实现。
2. **AI 可改性**：pure-admin 样式层 SCSS/Tailwind 混用、layout 深且回收了大量演示代码，AI 改主体风格成本高。新壳薄、样式全走 PRISM CSS 变量 token，AI 只面对自有代码。
3. **AI 驱动开发流水线**：先做好 skill，业务系统（当前为 project-manager，未来有更多）由 AI 通过 skill 生成。admin-ui 是这条流水线的地基。

**绿地重建**：系统仍在开发阶段，无遗留数据、无兼容负担。pure-admin（上游 `frontend/` 与 pm 仓库 `frontend/` 两份）最终删除。

## 2. 目标

1. **壳**：基于 Vue 3 + Vite + TS + shadcn-vue（reka-ui）+ Tailwind 的轻量 admin 壳，视觉严格实现 PRISM v2.0 token 与组件语言。
2. **框架绑定功能全保留**：现有前端消费 low-memory-micro-services 后端的所有页面与机制原样重做（清单见 §5）。
3. **基础库发包**：业务无关的 http 封装、contracts 生成物以私有 npm 包分发，减少下游拷贝体积。
4. **四个 skill**：`scaffold` / `update-shell` / `gen-page` / `prism-design`（§7）。
5. **pm 迁移**：project-manager 前端全部业务页面（`views/pm/` 等）迁移到新壳，pure-admin 删除。

## 3. 非目标 / 反目标

- **不做暗色模式**（v1）。PRISM v1 深色版 OBSIDIAN 已被明确废弃，v2 仅浅色。
- **不做展示页 / Dashboard 概念页**（那是 web-concept 概念项目的交付物，本次只取规范文本）。
- **不做主题/插件配置 API**。壳以拷贝分发、直接改源码定制——不做 pure-admin 式高度配置化（那是它复杂难改的根源，也是反目标）。
- **不做巨石前端/微前端聚合**。各业务系统独立仓库、独立构建、独立部署（air-gap 要求）。
- **不保留任何 pure-admin 演示页**（able/list/table/tabs 演示/menuoverflow/nested/guide/result/error/empty/about/welcome/permission 权限演示）。
- **不自造任何视觉数值**。所有颜色/间距/字号/圆角/时长/阴影以 PRISM Phase 1R 附录 CSS 变量为唯一来源；PRISM 未覆盖的新组件（如多标签栏）按 PRISM 语言推导，并在实现中标注"PRISM 补充设计"。

## 4. 约束

- **部署环境**：内网 + air-gap；内网 npm registry 时有失联（需 WireGuard），发包与装包链路必须有 registry 不可用时的兜底（pnpm 支持 file:/tarball/git 依赖，或离线 store）。
- **包管理**：pnpm workspace（上游仓库已建立，`frontend` 是成员）。
- **上游-下游结构**：low-memory-micro-services（框架上游）↔ project-manager（业务下游）。壳的真源在上游仓库；下游通过 skill 拷贝获得。现 cms-sync 同步 pure-admin 的职责由 `update-shell` skill 接管（对象从"整份肥壳"变为"薄壳 diff"）。
- **后端 API 固定**：壳消费的后端接口面不变（见 §5），含菜单协议 `/v1/pure/get-async-routes`（当前激活的动态路由树）与 `/v1/pure/menu-configs` 的 CRUD/publish/clone 生命周期。
  > **2026-10-26 修正**：本文早期版本写的是 `/v1/iam/menus` —— 该接口**在后端不存在**（`iam-service` 无 menus 路由，且 `swagger_test.rs` 明确断言其不存在）。真实协议在 prue_backend：`GET /v1/pure/get-async-routes` → `{ code, message, data: RouteNode[] }`，`data` 是已激活菜单配置版本里的路由树；菜单的可见性由 `meta.showLink` 决定（**不是** `visible` / `status` 字段）。
- **图标**：Lucide（PRISM 规范点名）。
- **移动端是 v1 必须项**（商户会用手机/平板访问后台）。Phase 2 响应式降级链全量落地为绑定契约：侧边栏 <640 抽屉式 / ≥768 图标栏 / ≥1024 完整展开（用户折叠状态 localStorage 持久化）；列表页 <640 表格卡片化、≥640 横向滚动；顶栏搜索 + ⌘K 命令面板按 Phase 2 §3.2 实现。**多标签栏为桌面专属，<768 隐藏**（移动端靠返回与面包屑）——PRISM 补充设计。
- **质量门禁延续现有 AGENTS.md 政策**：每个视图 ≥1 条 vitest 用例、视图禁止硬编码中文、写操作按钮必须带权限校验；门禁脚本（check-vitest-coverage / check-i18n-hardcode / check-v-perms）重写适配 shadcn-vue 组件形态，`gen-page` 产物默认必须过门禁。
- **i18n**：中文为主，vue-i18n + yaml（沿用现有 `locales/zh-CN.yaml`、`en.yaml` 双文件形态）。
- **无障碍基线**：键盘走通全部功能、焦点环可见、axe 扫描 0 严重违规（承 Phase 2 §9.4）。

## 5. 壳的功能清单（必须保留，从现有代码盘点核实）

| 能力 | 内容 | 后端接口面 |
|---|---|---|
| 认证 | 登录/登出/刷新/注册/改密/会话管理、首次初始化状态引导 | `/v1/iam/auth/*`、setup-status |
| IAM 管理 | 用户、角色、权限、部门（组织单元嵌在部门表单内管理，不设独立页）、租户、菜单管理页、会话 | `/v1/iam/users·roles·permissions·dept·org-units·tenants·menus` |
| Token 管理 | access / api_key / refresh / resource / revoke + OAuth 客户端（同在 token-client 页管理，与 api-keys 同表） | `/v1/tokens/*`、`/v1/admin/oauth-clients` |
| 动态菜单 | 后端下发的已激活路由树装载、按 `meta.showLink` 控可见性、菜单配置版本 CRUD/publish/clone | `/v1/pure/get-async-routes`、`/v1/pure/menu-configs` |
| 审计 | 登录日志、操作日志、保留策略、合规导出 CSV/JSON | `/v1/audit/*` |
| 配置中心 | namespace / item 两级管理页 | `/v1/config/*` |
| 文件 | 上传 / 预签名 / 列表 | `/v1/file/*` |
| 通知 | 公告、未读数、顶栏铃铛 | `/v1/notification/*` |
| 个人 | account-settings、dashboard（现有定制版的信息结构） | `/mine`、`/mine-logs` |
| 多标签页 | tabs（keepalive、关闭/批量操作）——桌面专属，<768 隐藏；PRISM 未覆盖，按其语言补设计（白底、1px 暖灰边、当前项 accent-subtle 底 + accent-deep 字 + 底缘 2px accent 指示条） | — |
| 登录页 | 完全重做——白标第一门面，PRISM 语言 | — |

**重写而非保留的部分**：pure-admin 的路由/权限/keepalive 机制、换肤面板（PRISM token 取代）、element-plus 全家桶（shadcn-vue 取代）。

## 6. 架构与分发

**判断标准：业务会不会想改它。想改的拷贝，不想改的发包。**

| 层 | 分发方式 | 内容 |
|---|---|---|
| 基础库 | **npm 包**（内网 registry，失联兜底见 §4） | `@scope/http`（axios 封装：token 刷新、错误码→文案映射、统一错误处理）、`@scope/contracts`（后端契约生成的 TS 类型 + 错误码 + 权限常量，真源在上游契约，生成流水线属上游） |
| 壳 | **skill 拷贝**（`scaffold` 生成、`update-shell` 升级） | 布局（侧栏 240/64px、顶栏 56px、tabs）、登录页、PRISM token 层（tokens.css + Tailwind 映射）、动态菜单装载、权限路由、i18n、§5 全部框架页面 |
| 业务组件 | skill 拷贝 | 基于 shadcn-vue 的业务表格（44px 行高、骨架/空/错态、排序分页）、表单/弹框模式、Badge/状态展示 |
| 业务页面 | skill 生成 | 各业务系统自有页面（pm 的 `views/pm/` 等） |

**壳必须薄**：目标量级为几十个源文件（对照 pure-admin 数千文件），这是 `update-shell` diff 可行的前提。

**真源位置**：上游仓库 `frontend/`（重写 pure-admin 原地，pnpm workspace 成员不变）。上游 `frontend/` 本身可运行（框架系统页的活样例与视觉验收载体）。

**pm 迁移模式**：功能等价迁移。专业三方组件**保留原样继续使用**（dhtmlx-gantt、视频播放器、PDF、高德地图、AI 对话、markdown 编辑器、mqtt、vxe-table 重型表格），PRISM 只约束其外围壳与页面框架；标准列表页用共享业务表格，重型表格页保留 vxe-table。按模块**分阶段迁移切换**，不做一次性 cutover。gantt 商业授权按现状延续，如授权状态变化再评估替代。无障碍门禁（axe/键盘）适用于壳与标准页，三方组件内嵌页以可用性为准、不追组件内部合规。

## 7. Skills（四个）

1. **`scaffold`**：新业务系统初始化——从上游壳生成前端骨架（拷贝壳 + 装基础包 + 品牌化：accent 色板/logo/产品名），产出可登录可导航的空业务壳。品牌 accent 色阶（约 10 个互锁 token：hover/pressed/deep/subtle/wash/渐变/阴影/焦点光晕）由**文档化固定算法**从品牌主色推导，scaffold 内置 WCAG 对比度断言，不达标即失败——「不自造数值」约束针对 PRISM 默认色板，推导算法是固定规则而非自由发挥。
2. **`update-shell`**：上游壳改动同步到下游——diff 上游壳与下游副本，生成可评审的变更集（排除下游已定制区域，定制区域列出来人工裁决）。
3. **`gen-page`**：对着后端契约/OpenAPI 生成页面，**两种形态**：① 列表页（查询 + 表格 + 分页 + 操作，含 <640 卡片化变体）；② 编辑页（弹框 + 表单）。覆盖 PRISM 的 loading 150ms/骨架 300ms/空态/错误态/文案语气规范；产物默认满足质量门禁（vitest ≥1、i18n 键、写操作权限校验）。
4. **`prism-design`**：设计规则约束——token 唯一值、tint 上用 accent-deep、hover 加灰不提亮、ink 占比 <10%、reduced-motion 降级清单等，供 AI 写任何前端代码时加载，防止发明数值。

skill 归属上游仓库的 skill 目录体系（与现有 skills-lock/catalog 机制一致），下游安装使用。

## 8. 技术栈（已定 + 实现者自由度）

**已定**：Vue 3 + Vite + TypeScript + shadcn-vue（reka-ui 内核）+ Tailwind + pinia + vue-i18n（yaml）+ Lucide 图标 + axios（`@scope/http` 内）。

**留给实现计划的决定**（不改变交付物）：Tailwind 大版本（v4 与 shadcn-vue 当前推荐对齐）、表单校验库（vee-validate/zod 方向）、表格底层（tanstack-vue-table 方向）、图表库（ECharts 方向，dashboard 需要面积图/环形进度，PRISM 对图表有 token 级要求：轴线用 `--border-data`）、包 scope 命名。

## 9. 交付物与验收

| # | 交付物 | 验收标准 |
|---|---|---|
| 1 | 上游 `frontend/` 新壳 | §5 清单全部页面可用；视觉符合 PRISM 唯一值；响应式降级链落地（≥375px 手机可完成核心操作：抽屉导航、卡片化表格、命令面板）；axe 0 严重违规；键盘走通 |
| 2 | `@scope/http`、`@scope/contracts` 包 | 可从内网 registry 安装；registry 失联时有兜底装法 |
| 3 | pm 前端迁移 | 全部业务页面在新壳运行（专业组件按 §6 迁移模式：保留原样、外壳 PRISM 化）；pm 仓库 pure-admin 删除 |
| 4 | 四个 skill | `scaffold` 从零到可登录空壳（含品牌化与对比度断言）；`gen-page` 对现有 API 产出可用列表/编辑页（含移动端变体、过质量门禁）；`update-shell` 对模拟上游改动产出正确 diff |
| 5 | pure-admin 退役 | 上游与 pm 两处 `frontend/` 的 pure-admin 代码、依赖、构建脚本清除 |

**成功的画面**：竞争对手打开任意业务系统后台，看到的是一套查不到出处的私有设计语言；公司新起一个业务系统，AI 用 skill 在一天内铺出带完整 IAM/审计/配置能力的前端骨架和第一批业务页面；改全局品牌只动一个 token 文件。

## 10. 风险与备注

- **移动端抬高 gen-page 与壳的复杂度**：每张列表页两套形态（表格 + 卡片化）、抽屉侧栏、命令面板都是新增面。实现计划应把桌面形态先行、移动变体紧随同页交付，避免积压到收尾。
- **Phase 2 从"概念页规范"升格为绑定契约**（响应式链、命令面板、反馈节奏）：如与 Phase 1R 数值冲突，仍以 Phase 1R 为准并回报。
- **shadcn-vue 是社区移植**，组件覆盖少于 React 版。所需均为核心组件（button/input/select/dialog/table/tabs 等），风险可控；缺的按 PRISM 语言自写。
- **菜单协议是核心资产**：动态路由树装载 + `meta.showLink` 可见性 + publish/clone 是最近投入的功能，迁移时按现协议重写装载逻辑，接口不变。
- **两份 pure-admin 的删除时机**：新壳就绪 → pm 迁移 → 删除，按此顺序，避免中间态双轨过长。
- **PRISM 规范原文按 React 概念项目撰写**：token 与视觉语言与框架无关，按 Vue 实现不构成冲突；如实现中发现规范数值互相矛盾，以 Phase 1R（视觉规范）为准并回报。
