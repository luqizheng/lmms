<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  cloneMenuConfig,
  listMenuConfigs,
  publishMenuConfig,
  type MenuConfigItem
} from "@/shell/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { DataTable, type TableColumn } from "@/shell/components";

const { t } = useI18n();

const rows = ref<MenuConfigItem[]>([]);
const loading = ref(false);
const busyId = ref<string | undefined>(undefined);

const columns: TableColumn[] = [
  { label: t("system.menu.name"), prop: "name" },
  { label: t("system.menu.version"), prop: "version", numeric: true },
  { label: t("system.menu.status"), prop: "is_active", slot: "status" },
  { label: t("system.user.createdAt"), prop: "created_at", numeric: true }
];

async function load() {
  loading.value = true;
  try {
    const result = await listMenuConfigs();
    rows.value = result.items ?? [];
  } finally {
    loading.value = false;
  }
}

async function onPublish(row: Record<string, unknown>) {
  const id = row.id as string;
  busyId.value = id;
  try {
    await publishMenuConfig(id);
    await load();
  } finally {
    busyId.value = undefined;
  }
}

async function onClone(row: Record<string, unknown>) {
  const id = row.id as string;
  busyId.value = id;
  try {
    await cloneMenuConfig(id);
    await load();
  } finally {
    busyId.value = undefined;
  }
}

onMounted(load);
</script>

<template>
  <section class="ui-page">
    <header class="ui-page__header">
      <h1 class="ui-page__title">{{ t("system.menu.title") }}</h1>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :loading="loading"
      :caption="t('system.menu.caption')"
      @update:page="load"
    >
      <template #status="{ row }">
        <Badge :variant="row.is_active ? 'success' : 'neutral'">
          {{ row.is_active ? t("system.menu.active") : t("system.menu.draft") }}
        </Badge>
      </template>
      <template #name="{ row }">
        <span class="ui-page__name">{{ row.name }}</span>
        <span class="ui-page__actions">
          <Button
            v-if="!row.is_active"
            variant="secondary"
            size="sm"
            :loading="busyId === row.id"
            @click="onPublish(row)"
          >
            {{ t("system.menu.publish") }}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            :loading="busyId === row.id"
            @click="onClone(row)"
          >
            {{ t("system.menu.clone") }}
          </Button>
        </span>
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

.ui-page__actions {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  margin-left: var(--space-3);
}
</style>
