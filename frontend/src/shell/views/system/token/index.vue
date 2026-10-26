<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  type QueryField,
  type TableColumn
} from "@/shell/components";

interface ApiKey {
  id: string;
  name: string;
  permissions: string[];
  status: string;
  created_at: string;
}

const { t } = useI18n();

const rows = ref<ApiKey[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const columns: TableColumn[] = [
  { label: t("system.token.name"), prop: "name" },
  {
    label: t("system.token.permissions"),
    prop: "permissions",
    slot: "permissions"
  },
  { label: t("system.common.status"), prop: "status", slot: "status" },
  { label: t("system.common.createdAt"), prop: "created_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.token.searchPlaceholder") }
];

async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: ApiKey[]; total: number }>(
      "/v1/api-keys",
      {
        page: page.value,
        size: pageSize,
        keyword: keyword.value || undefined
      }
    );
    rows.value = result.items ?? [];
    total.value = result.total ?? 0;
  } finally {
    loading.value = false;
  }
}

function onSearch(params: Record<string, string>) {
  keyword.value = params.keyword ?? "";
  page.value = 1;
  load();
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.token.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.token.caption')"
      @search="onSearch"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #permissions="{ row }">
        <span class="ui-page__muted">{{
          (row.permissions as string[]).join(", ") || "-"
        }}</span>
      </template>
      <template #status="{ row }">
        <Badge :variant="row.status === 'active' ? 'success' : 'neutral'">{{
          row.status
        }}</Badge>
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

.ui-page__muted {
  color: var(--text-tertiary);
}
</style>
