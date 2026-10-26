<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { Badge } from "@/components/ui/badge";
import { DataTable, type TableColumn } from "@/shell/components";

interface OrgUnit {
  id: string;
  name: string;
  code: string;
  level: number;
  sort_order: number;
  status: string;
  created_at: string;
  children: OrgUnit[];
}

interface Row {
  id: string;
  name: string;
  code: string;
  level: number;
  status: string;
  created_at: string;
  depth: number;
}

const { t } = useI18n();

const rows = ref<Row[]>([]);
const loading = ref(false);

const columns: TableColumn[] = [
  { label: t("system.dept.name"), prop: "name" },
  { label: t("system.dept.code"), prop: "code" },
  { label: t("system.dept.level"), prop: "level", numeric: true },
  { label: t("system.common.status"), prop: "status", slot: "status" },
  { label: t("system.common.createdAt"), prop: "created_at", numeric: true }
];

function flatten(items: OrgUnit[], depth = 0): Row[] {
  const out: Row[] = [];
  for (const item of items ?? []) {
    out.push({
      id: item.id,
      name: item.name,
      code: item.code,
      level: item.level,
      status: item.status,
      created_at: item.created_at,
      depth
    });
    if (item.children?.length) out.push(...flatten(item.children, depth + 1));
  }
  return out;
}

async function load() {
  loading.value = true;
  try {
    const result = await http.get<OrgUnit[]>("/v1/iam/org-units");
    rows.value = flatten(result ?? []);
  } finally {
    loading.value = false;
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
      <h1 class="ui-page__title">{{ t("system.dept.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :caption="t('system.dept.caption')"
      @update:page="load"
    >
      <template #name="{ row }">
        <span class="ui-page__name" :style="indent(row.depth as number)">{{
          row.name
        }}</span>
      </template>
      <template #status="{ row }">
        <Badge :variant="row.status === 'enabled' ? 'success' : 'neutral'">
          {{ row.status }}
        </Badge>
      </template>
    </DataTable>
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

.ui-page__name {
  color: var(--text-primary);
}
</style>
