# PRISM Admin UI — 实现路线图

> 日期：2026-10-26
> 设计文档：`docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`
> 状态：进行中

本路线图把设计文档拆成 **7 个独立可交付的计划**。每个计划结束时都产出一个可运行、可测试、自身闭环的交付物；后一个计划只依赖前面计划已交付的接口。

| # | 计划 | 交付物 | 依赖 | 状态 |
|---|---|---|---|---|
| 1 | [PRISM 设计系统基座](2026-10-26-prism-design-system-base.md) | token 层（含品牌覆盖层）+ 核心组件集 + 设计约束校验器（含自测） | — | 已完成 |
| 2 | PRISM 交互组件集 | dialog/select/dropdown/tabs/toast/command/sheet/switch 等 16 个组件 | 1 | 已完成 |
| 3 | 基础包发包 | `@digua/contracts`、`@digua/http` 两个 npm 包 + 发布前检查链路 | 1 | 已完成 |
| 4 | 壳骨架 | 布局（侧栏/顶栏/标签栏/命令面板）、路由、动态菜单、权限、登录页、i18n、响应式降级链 | 1,2,3 | 未开始 |
| 5 | 框架系统页 | 设计文档 §5 全部页面（认证/IAM/token/菜单/审计/配置/文件/通知/dashboard/account-settings） | 4 | 未开始 |
| 6 | 四个 skill | `prism-design` / `scaffold` / `update-shell` / `gen-page` | 1,2,4 | 未开始 |
| 7 | pm 迁移与退役 | project-manager 业务页面迁移 + 两仓 pure-admin 删除 | 5,6 | 未开始 |

## 切分原则

- **一个计划 = 一个可交付闭环**。计划 1 结束时，PRISM 组件可以在空页面里渲染并通过测试，但还没有任何业务页面——这本身就是一个完整里程碑。
- **纯增量，不破坏现状**。计划 1–3 只新增目录与依赖，不改动现有 pure-admin 前端的运行路径；切换发生在计划 4–5（壳就绪后）与计划 7（pm 迁移后）。任何时刻 `pnpm -C frontend dev` 都可运行。
- **计划 4 是唯一的切换点**。壳骨架完成后，旧页面下线、新页面上线；此前后的并行期由计划 4 的任务序列控制，不跨越到计划 5。

## 跨计划共享约束

以下约束对所有计划生效，各计划不再重复声明：

1. **视觉数值唯一来源** = PRISM Phase 1R 附录 CSS 变量（`D:\Workspace\web-concept\design\phase1-visual-direction.md`）。组件源码中禁止出现裸 hex 色值——由计划 1 的校验器强制。
2. **视觉交互唯一来源** = PRISM Phase 2 v1.1（`phase2-ux-spec.md`）。与 Phase 1R 冲突时以 Phase 1R 为准并回报。
3. **组件源码不得自造数值**：间距/字号/圆角/阴影/时长一律引用 token 或 Tailwind 主题映射。
4. **质量门禁延续现有政策**：每个视图 ≥1 条 vitest 用例；视图禁止硬编码中文；写操作按钮必须带权限校验。
5. **不引入暗色模式**（v1 仅浅色）。
6. **mobile-first**：所有壳级布局必须满足 PRISM Phase 2 §4 的五断点降级链。

## 执行方式

每个计划独立执行，可随时中断续接。计划 1 的任务清单见其文档；后续计划在开工前按同一格式（bite-sized checkbox 步骤 + 精确签名 + 验证命令）撰写。
