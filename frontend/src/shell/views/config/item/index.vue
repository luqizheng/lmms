<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  type QueryField,
  type TableColumn
} from "@/shell/components";

const { t } = useI18n();
const auth = useAuthStore();

interface Row {
  id: string;
  namespace_id?: string;
  key: string;
  value?: string;
  description?: string;
  updated_at: string;
}

const columns: TableColumn[] = [
  { label: t("system.config.key"), prop: "key" },
  { label: t("system.config.value"), prop: "value" },
  { label: t("system.config.namespaceName"), prop: "namespace_id" },
  { label: t("system.user.createdAt"), prop: "updated_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.config.searchPlaceholder") }
];

const rows = ref<Row[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");
async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: Row[]; total: number }>(
      "/v1/config/item",
      {
        page: page.value,
        size: pageSize,
        key: keyword.value || undefined
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
      <h1 class="ui-page__title">{{ t("system.config.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.config.caption')"
      @search="onSearch"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    />
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
</style>
