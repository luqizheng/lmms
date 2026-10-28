<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  UserLabel,
  type QueryField,
  type TableColumn
} from "@/shell/components";

interface Row {
  id: string;
  user_id: string;
  client_ip: string;
  user_agent?: string;
  created_at: string;
  expires_at: string;
  is_current: boolean;
}

const { t } = useI18n();

const rows = ref<Row[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const columns: TableColumn[] = [
  { label: t("system.session.user"), prop: "user_id", slot: "user" },
  { label: t("system.session.ip"), prop: "client_ip" },
  { label: t("system.session.current"), prop: "is_current", slot: "current" },
  { label: t("system.session.expiresAt"), prop: "expires_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.session.searchPlaceholder") }
];

/** 表格插槽里的行是弱类型，取 userId 时只认字符串 */
function userIdOf(row: Record<string, unknown>): string | undefined {
  return typeof row.user_id === "string" ? row.user_id : undefined;
}

async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: Row[]; total: number }>("__PATH__", {
      page: page.value,
      size: pageSize,
      keyword: keyword.value || undefined
    });
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
      <h1 class="ui-page__title">{{ t("system.session.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.session.caption')"
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

      <template #current="{ row }">
        <Badge :variant="row.is_current ? 'accent' : 'neutral'">
          {{
            row.is_current
              ? t("system.session.currentSession")
              : t("system.session.otherSession")
          }}
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
</style>
