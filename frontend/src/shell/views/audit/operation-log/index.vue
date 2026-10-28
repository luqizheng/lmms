<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  UserLabel,
  type QueryField,
  type TableColumn
} from "@/shell/components";

const { t } = useI18n();
const auth = useAuthStore();

interface Row {
  id: string;
  user_id?: string;
  action?: string;
  resource?: string;
  result?: string;
  created_at: string;
}

const columns: TableColumn[] = [
  { label: t("system.audit.actor"), prop: "user_id", slot: "user" },
  { label: t("system.audit.action"), prop: "action" },
  { label: t("system.audit.target"), prop: "resource" },
  { label: t("system.audit.createdAt"), prop: "created_at", numeric: true }
];

const query: QueryField[] = [];

/** 表格插槽里的行是弱类型，取 userId 时只认字符串 */
function userIdOf(row: Record<string, unknown>): string | undefined {
  return typeof row.user_id === "string" ? row.user_id : undefined;
}

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
      "/v1/audit/operation",
      {
        page: page.value,
        size: pageSize
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
      <h1 class="ui-page__title">{{ t("system.audit.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.audit.caption')"
      @search="onSearch"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #user="{ row }">
        <UserLabel :user-id="userIdOf(row)" />
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
</style>
