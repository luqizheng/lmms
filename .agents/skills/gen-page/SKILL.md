---
name: gen-page
description: |
  按后端契约生成 PRISM 风格的业务页面。Use when the user asks to build a list page,
  a CRUD page, an admin page, or "生成列表页/编辑页" for a backend API.
  只有两种形态：列表页（DataTable）与弹框编辑页（FormDialog）。
---

# gen-page Skill

对着后端接口生成业务页面。**只生成两种形态**（用户明确过，别的都不要）：

| 形态 | 组件 | 何时用 |
|---|---|---|
| 列表页 | `shell/components/DataTable.vue` | 有 GET 列表接口 |
| 弹框编辑页 | `shell/components/FormDialog.vue` | 有 POST/PUT 接口，行内触发 |

## 开始前必读

1. **`.agents/skills/prism-design/SKILL.md`** —— 全部视觉与可访问性约束。
2. 后端契约：优先读 `services/<svc>/openapi.yaml`（路径 / 方法 / 字段）；
   其次读 `frontend/packages/contracts` 里的错误码与权限码。
3. 已有的同类页面作参考（`frontend/src/shell/views/`）。

## 第 1 步：确定接口与字段

从 openapi 里确认：

- 列表：`GET <path>` → 返回形状（数组 or `{items, total}`）与查询参数（分页参数名、关键字参数名）。
- 详情：`GET <path>/{id}`（可选；列表页若已返回全字段可省）。
- 写：`POST <path>` / `PUT <path>/{id}` → 请求体字段。
- 权限码：写操作按钮必须校验 `auth.hasPermission("<权限码>")`。

不确定就先问，不要猜字段名。

## 第 2 步：建视图文件

路径：`frontend/src/shell/views/<模块>/<页面名>View.vue`（模块目录用 kebab-case）。

骨架：

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { http } from "@/shell/api";
import { DataTable, type TableColumn, type QueryField } from "@/shell/components/DataTable";
import { FormDialog } from "@/shell/components/FormDialog";
import { useAuthStore } from "@/shell/store/auth";

interface Row { id: string; name: string; status: string; created_at: string }

const auth = useAuthStore();
const rows = ref<Row[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const dialogOpen = ref(false);
const submitting = ref(false);

const columns: TableColumn[] = [
  { label: "名称", prop: "name" },
  { label: "状态", prop: "status", slot: "status" },
  { label: "创建时间", prop: "created_at", numeric: true }
];

const query: QueryField[] = [{ prop: "keyword", placeholder: "搜索名称" }];

async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: Row[]; total: number }>("/v1/xxx/list", {
      page: page.value,
      page_size: pageSize,
      keyword: keyword.value
    });
    rows.value = result.items;
    total.value = result.total;
  } finally {
    loading.value = false;
  }
}

function onSearch(params: Record<string, string>) {
  keyword.value = params.keyword ?? "";
  page.value = 1;
  load();
}

function openCreate() {
  dialogOpen.value = true;
}

async function onSubmit() {
  submitting.value = true;
  try {
    await http.post("/v1/xxx", form.value);
    dialogOpen.value = false;
    await load();
  } finally {
    submitting.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">模块名</h1>
      <Button v-if="auth.hasPermission('XXX_CREATE')" @click="openCreate">新建</Button>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      caption="模块名列表"
      @search="onSearch"
      @update:page="page = $event; load()"
    >
      <template #status="{ row }">
        <Badge :variant="row.status === 'active' ? 'success' : 'neutral'">
          {{ row.status }}
        </Badge>
      </template>
    </DataTable>

    <FormDialog v-model:open="dialogOpen" title="新建" @submit="onSubmit" @cancel="dialogOpen = false">
      <FormField label="名称" required>
        <Input v-model="form.name" />
      </FormField>
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-page { display: flex; flex-direction: column; gap: var(--space-6); }
.ui-page__header { display: flex; align-items: center; justify-content: space-between; }
.ui-page__title { margin: 0; font: var(--text-h2); color: var(--text-primary); }
</style>
```

> 上面的 `.ui-page__title` 只是示意。**实际写法必须按 prism-design 的 token 族逐项写**
> （`font-size` / `line-height` / `weight` / `tracking` 四个属性分开写，不用 `font` 简写，
> 否则改字号会连带重置字重）。

## 第 3 步：注册菜单（DB 菜单驱动，不要写死路由）

新页面的路由来自 **DB 菜单**（`GET /v1/iam/menus`），路由守卫按菜单的 `component`
字段解析视图文件：`component: "system/user"` → `src/shell/views/system/user/index.vue`
或 `src/shell/views/system/user/index.vue`。

因此：

- 视图文件路径要与菜单的 `component` 一致（`index.vue` 或同名 `.vue`）。
- 解析不到就显示「该页面正在迁移」占位 —— **不要**为了绕过而伪造空页面。

## 第 4 步：写测试

`frontend/src/shell/views/<模块>/<页面名>View.test.ts`，至少 1 条：

- 断言**行为**：列表渲染出接口返回的数据、空态文案、加载态骨架、提交调用了正确的接口与参数。
- 用 `mountShell`（`@/shell/test-utils`）挂载 —— 它会装好 pinia 与 i18n。
- mock `@/shell/api` 的 `http`，不要真发请求。

## 第 5 步：跑门禁

```bash
cd frontend
pnpm vitest run src/shell/views/<模块>
pnpm exec eslint --fix --max-warnings 0 src/shell/views/<模块>
npx vue-tsc --noEmit --skipLibCheck
cd .. && node scripts/check-prism-tokens.js
```

## 第 6 步：文案 i18n

视图里**禁止硬编码中文**。把文案加进 `frontend/locales/zh-CN.yaml` 与 `en.yaml` 的
业务命名空间（不要塞进 `shell:` —— 那是壳自己的），然后用 `useI18n()` 取。

## 常见坑

| 坑 | 做法 |
|---|---|
| 分页参数名猜错 | 从 openapi 抄，别猜 `page`/`pageSize` |
| 写操作没权限校验 | 按钮 `v-if="auth.hasPermission('XXX')"` |
| 表格数值列没等宽 | `TableColumn.numeric = true` |
| 加载态直接清空旧数据 | 骨架只在 `loading` 时渲染，旧数据保留由 `DataTable` 处理 |
| 弹框提交后不刷新列表 | `onSubmit` 成功后 `await load()` |
| 在组件里写 hex | 用 token；实在缺值先报告，不要自造 |
