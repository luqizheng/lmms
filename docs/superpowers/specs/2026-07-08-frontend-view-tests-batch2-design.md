# Frontend View Tests Batch 2 (F1)

## Status: 🟡 待 user review

「补 P1/P2 残留」首批（commit `60389ad` 9 个跨页 util 单测 + `de2132a` view test coverage linter + `c104aaa` pure-admin demo 豁免）已落。第二批目标**为核心业务 view 关联的 util / rule / 纯函数补单测**，闭合 `frontend/AGENTS.md §测试规范` 强制项。

---

## 1. 目标

`AGENTS.md §测试规范` 强制「任一新增 `src/views/**` view，至少交付一条 happy-path 单测；跨页通用 util、formatter、menu-adapter 之类的纯函数模块改动，必须伴随单测更新」。

**实际可行的测试面盘点**（2026-07-08）：

| view | utils 现状 | 可测内容 |
|---|---|---|
| `system/user` | `utils/rule.ts`（5 字段 + phone/email 自定义 validator） | rule 字段名 + validator 逻辑 |
| `system/role` | `utils/rule.ts`（2 字段声明式）+ `utils/hook.tsx` 内 `dataScopeText` / `dataScopeToNumber` 纯函数 | rule + dataScope 转换 |
| `system/dept` | `utils/rule.ts`（2 字段声明式） | rule 字段名 |
| `notification/admin` | `utils/rule.ts`（3 字段声明式） | rule 字段名 |
| `system/tenant` / `system/session` / `file/*` / `config/*` / `notification/inbox` | ❌ 业务逻辑直接写在 `index.vue`，**未抽 utils** | F1 不在范围（YAGNI 强行测需 jsdom 渲染测试，是另一档工作量） |
| `system/menu` / `notification/inbox` 的 utils | 已有 `read-state.test.ts` + `menu-adapter.test.ts` | 不重测 |

**4 个 view × 1-2 个 util = 5 个 .test.ts**（不强行凑 8-10 个）：
1. `system/user/utils/rule.test.ts`（phone/email validator + 字段名断言）
2. `system/role/utils/rule.test.ts`（字段名断言）
3. `system/role/utils/hook-data-scope.test.ts`（dataScopeText / dataScopeToNumber 纯函数 — **这是 role hook 里的纯函数**，单独抽出来测）
4. `system/dept/utils/rule.test.ts`（字段名断言）
5. `notification/admin/utils/rule.test.ts`（字段名断言）

合计 ~20-30 个 `it()`，~150-250 行 test 代码。

---

## 2. 不在范围（未来 wave）

| 主题 | 原因 |
|---|---|
| `system/user/utils/hook.tsx`（428 行）完整测试 | 大量 mock（api + element-plus + store + @pureadmin/utils），属第二批"大块 hook 测试" |
| `system/role/utils/hook.tsx`（330 行）完整测试 | 同上 |
| `system/dept/utils/hook.tsx`（240 行）完整测试 | 同上 |
| `notification/admin/utils/hook.tsx`（203 行）完整测试 | 同上 |
| simple view（tenant/session/file/config）组件挂载测试 | 需装 happy-dom / jsdom dev dep，独立立项 |
| `inbox/index.vue` / `menu/index.vue` view 本体渲染测试 | 同上 |
| 4 个 hook.tsx 里能抽的纯函数（columns / statusMap） | 抽离 = refactor，违反「bugfix Rule: fix minimally」 |

---

## 3. 设计

### 3.1 测试模式

参考 `views/system/menu/utils/menu-adapter.test.ts`（已存在）和 `views/notification/inbox/utils/read-state.test.ts`（已存在）：

```ts
import { describe, expect, it } from "vitest";
import { formRules } from "./rule";

describe("user-form-rule", () => {
  it("declares 5 required fields", () => {
    expect(Object.keys(formRules)).toEqual(
      expect.arrayContaining(["nickname", "username", "password", "phone", "email"])
    );
  });

  it("phone validator accepts valid + rejects invalid", () => {
    const phoneRule = formRules.phone[0];
    // ... call validator, assert callback
  });

  // ...
});
```

### 3.2 hook 里的纯函数（role 的 dataScope）

`role/utils/hook.tsx` 里的 `dataScopeText` / `dataScopeToNumber` **没有 `export`**，需要单独测。

**方案 A**：export 它们，写 test
**方案 B**：在 test 文件里直接 import 整个 hook 文件，断言副作用

**采用 A**：在 hook.tsx 加 2 行 `export`（minimal 改动），test 直接 import。理由：
- 函数本身已经是 pure，没有副作用
- 单独 export 让其他 view 也能复用（DRY）
- 不破坏现有 import（`import { useRole }` 不变）

**变更点**：`role/utils/hook.tsx:32` 加 `export` keyword，`role/utils/hook.tsx:37` 加 `export` keyword。

### 3.3 命名

`<util>.test.ts`，与 `views/system/menu/utils/menu-adapter.test.ts` 对齐。
特例：role 的 hook 里抽的纯函数 → `role/utils/hook-data-scope.test.ts`（明确表意，不是 `hook.test.tsx` 因 hook 主流程不测）。

### 3.4 0 配置改动

- 无 vitest.config.ts 新建（沿用）
- 无 happy-dom / jsdom 安装
- 无 package.json 改动
- 无 AGENTS.md 改动（已有测试规范）

### 3.5 commit 节奏

**1 commit**：5 个 .test.ts + 1 spec + role hook 2 行 `export` keyword

```
test(frontend): add rule + dataScope tests for 4 core business pages

补 4 个核心 view 的 rule / 纯函数单测：
- system/user/utils/rule.test.ts       (phone/email validator)
- system/role/utils/rule.test.ts       (字段名)
- system/role/utils/hook-data-scope.test.ts (dataScopeText/ToNumber)
- system/dept/utils/rule.test.ts       (字段名)
- notification/admin/utils/rule.test.ts (字段名)

role/utils/hook.tsx 加 2 行 export 关键字，让 dataScope 纯函数可被单独测。

闭合 frontend_todo.md §7.2 残留「核心 view 单测」第一批。
详见 docs/superpowers/specs/2026-07-08-frontend-view-tests-batch2-design.md
```

---

## 4. 测试矩阵（5 文件 ~25 个 it）

### 4.1 user/utils/rule.test.ts（6-8 it）

- `nickname` / `username` / `password` 字段名存在
- `phone` 字段含 validator 函数
- `phone` validator: 空值 → callback()
- `phone` validator: 合法手机 → callback()
- `phone` validator: 非法手机 → callback(new Error)
- `email` validator: 同上 3 路径

### 4.2 role/utils/rule.test.ts（2-3 it）

- `name` / `code` 字段名存在
- 两条规则都是 required + trigger=blur

### 4.3 role/utils/hook-data-scope.test.ts（8-10 it）

- `dataScopeText(0/1/2/3)` → "仅本人" / "本部门" / "本部门及以下" / "全部"
- `dataScopeText("self"/"dept"/"dept_and_below"/"all")` → 4 标签
- `dataScopeText(undefined/null)` → "-"
- `dataScopeText(unknown)` → String(unknown) fallback
- `dataScopeToNumber(0/1/2/3)` → 自身
- `dataScopeToNumber("self"/"dept"/"dept_and_below"/"all")` → 0/1/2/3
- `dataScopeToNumber("")/undefined/null` → 0
- `dataScopeToNumber("unknown")` → 0

### 4.4 dept/utils/rule.test.ts（2 it）

- `name` / `code` 字段名存在
- 都 required + trigger=blur

### 4.5 admin/utils/rule.test.ts（3 it）

- `title` / `body` 字段名存在
- `publish_at` 字段名存在 + trigger=change（**注意：唯一非 blur 的**）

---

## 5. 风险

| 风险 | 缓解 |
|---|---|
| rule.test 测的是「字段名 + trigger」声明式 | 这是最小验证；后续 view 改动 rule 时能立即感知 regression |
| `dataScopeText/ToNumber` 加 export 是 refactor | 改动极小（2 个 keyword），不破坏任何现有 import |
| 4 个 hook.tsx 完整测试未做 | 已在 §2 列为未来 wave（独立 spec + 独立 commit） |
| simple view（tenant/session 等）测试未做 | 需装 jsdom，独立立项；与 F1 目标不一致 |

---

## 6. 验收

- [ ] 5 个 .test.ts 存在 + 25+ 个 it() 全过
- [ ] `pnpm run test:unit` 71+ → 96+ 全过
- [ ] `pnpm run typecheck` 干净
- [ ] `node scripts/check-vitest-coverage.js` 0 误报
- [ ] 1 commit（5 test + 1 spec + role hook 2 行 export）

---

_最后更新：2026-07-08_
