<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import {
  DataTable,
  FormDialog,
  type QueryField,
  type TableColumn
} from "@/shell/components";

interface IamMenu {
  id: string;
  parent_id?: string;
  name: string;
  code: string;
  path: string;
  component?: string;
  icon?: string;
  sort_order: number;
  visible: boolean;
  status: string;
  children: IamMenu[];
}

/** 展平成带层级的行，前端表格没有树形控件 */
interface Row {
  id: string;
  name: string;
  path: string;
  component: string;
  icon: string;
  sort_order: number;
  visible: boolean;
  depth: number;
}

const { t } = useI18n();
const auth = useAuthStore();

const rows = ref<Row[]>([]);
const loading = ref(false);
const dialogOpen = ref(false);
const submitting = ref(false);
const editingId = ref<string | undefined>(undefined);
const form = ref({
  name: "",
  code: "",
  path: "",
  component: "",
  icon: "",
  sort_order: 0
});

const columns: TableColumn[] = [
  { label: t("system.menu.name"), prop: "name" },
  { label: t("system.menu.path"), prop: "path" },
  { label: t("system.menu.component"), prop: "component" },
  { label: t("system.menu.icon"), prop: "icon" },
  { label: t("system.menu.sortOrder"), prop: "sort_order", numeric: true },
  { label: t("system.menu.visible"), prop: "visible", slot: "visible" }
];

const query: QueryField[] = [];

function flatten(items: IamMenu[], depth = 0): Row[] {
  const out: Row[] = [];
  for (const item of items ?? []) {
    out.push({
      id: item.id,
      name: item.name,
      path: item.path ?? "",
      component: item.component ?? "",
      icon: item.icon ?? "",
      sort_order: item.sort_order ?? 0,
      visible: item.visible !== false,
      depth
    });
    if (item.children?.length) out.push(...flatten(item.children, depth + 1));
  }
  return out;
}

async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: IamMenu[] }>("/v1/iam/menus");
    rows.value = flatten(result.items ?? []);
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  editingId.value = undefined;
  form.value = {
    name: "",
    code: "",
    path: "",
    component: "",
    icon: "",
    sort_order: 0
  };
  dialogOpen.value = true;
}

function openEdit(row: Record<string, unknown>) {
  editingId.value = row.id as string;
  form.value = {
    name: (row.name as string) ?? "",
    code: "",
    path: (row.path as string) ?? "",
    component: (row.component as string) ?? "",
    icon: (row.icon as string) ?? "",
    sort_order: (row.sort_order as number) ?? 0
  };
  dialogOpen.value = true;
}

async function onSubmit() {
  submitting.value = true;
  try {
    const body = {
      name: form.value.name,
      code: form.value.code || form.value.name,
      path: form.value.path,
      component: form.value.component || undefined,
      icon: form.value.icon || undefined,
      sort_order: form.value.sort_order
    };
    if (editingId.value)
      await http.put(`/v1/iam/menus/${editingId.value}`, body);
    else await http.post("/v1/iam/menus", body);
    dialogOpen.value = false;
    await load();
  } finally {
    submitting.value = false;
  }
}

const indent = (depth: number) => ({
  paddingLeft: `calc(var(--space-4) + ${depth} * var(--space-5))`
});

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.menu.title") }}</h1>
      <Button v-if="auth.hasPermission('IAM_MENU_CREATE')" @click="openCreate">
        {{ t("system.menu.create") }}
      </Button>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :caption="t('system.menu.caption')"
      @update:page="load"
    >
      <template #name="{ row }">
        <button
          type="button"
          class="ui-page__link"
          :style="indent(row.depth as number)"
          @click="openEdit(row)"
        >
          {{ row.name }}
        </button>
      </template>
      <template #visible="{ row }">
        <Badge :variant="row.visible ? 'success' : 'neutral'">
          {{
            row.visible
              ? t("system.user.statusActive")
              : t("system.user.statusDisabled")
          }}
        </Badge>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="editingId ? t('system.menu.edit') : t('system.menu.create')"
      @submit="onSubmit"
      @cancel="dialogOpen = false"
    >
      <FormField :label="t('system.menu.name')" required>
        <Input v-model="form.name" />
      </FormField>
      <FormField :label="t('system.menu.path')">
        <Input v-model="form.path" />
      </FormField>
      <FormField :label="t('system.menu.component')">
        <Input v-model="form.component" />
      </FormField>
      <FormField :label="t('system.menu.icon')">
        <Input v-model="form.icon" />
      </FormField>
    </FormDialog>
  </section>
</template>

<style scoped>
.ui-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-page__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-page__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

.ui-page__link {
  padding: 0;
  border: 0;
  background: transparent;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  color: var(--accent);
  cursor: pointer;
}

.ui-page__link:hover {
  color: var(--accent-hover);
}

.ui-page__link:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
  border-radius: var(--radius-xs);
}
</style>
