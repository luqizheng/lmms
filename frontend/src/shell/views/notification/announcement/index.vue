<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { FormField } from "@/components/ui/form-field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  DataTable,
  FormDialog,
  type QueryField,
  type TableColumn
} from "@/shell/components";

interface Announcement {
  id: number;
  title: string;
  body: string;
  publish_at: string;
  status: string;
  pinned: boolean;
}

const { t } = useI18n();
const auth = useAuthStore();

const rows = ref<Announcement[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const dialogOpen = ref(false);
const submitting = ref(false);
const editingId = ref<number | undefined>(undefined);
const form = ref({ title: "", body: "", publish_at: "" });

const columns: TableColumn[] = [
  { label: t("system.notification.announcementTitle"), prop: "title" },
  { label: t("system.notification.status"), prop: "status", slot: "status" },
  { label: t("system.audit.createdAt"), prop: "publish_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.notification.searchPlaceholder") }
];

const statusVariant = (status: string) => {
  if (status === "published") return "success";
  if (status === "revoked") return "danger";
  return "neutral";
};

const statusLabel = (status: string) => {
  if (status === "published") return t("system.notification.published");
  if (status === "revoked") return t("system.notification.revoked");
  return t("system.notification.draft");
};

async function load() {
  loading.value = true;
  try {
    const result = await http.get<{ items: Announcement[]; total?: number }>(
      "/v1/notification/announcements"
    );
    rows.value = result.items ?? [];
    total.value = result.total ?? rows.value.length;
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
  editingId.value = undefined;
  form.value = { title: "", body: "", publish_at: "" };
  dialogOpen.value = true;
}

async function onSubmit() {
  submitting.value = true;
  try {
    const body = {
      title: form.value.title,
      body: form.value.body,
      publish_at: form.value.publish_at,
      recipients: []
    };
    if (editingId.value)
      await http.put(`/v1/notification/announcements/${editingId.value}`, body);
    else await http.post("/v1/notification/announcements", body);
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
      <h1 class="ui-page__title">{{ t("system.notification.title") }}</h1>
      <Button
        v-if="auth.hasPermission('NOTIFICATION_ANNOUNCEMENT_CREATE')"
        @click="openCreate"
      >
        {{ t("system.notification.create") }}
      </Button>
    </header>

    <DataTable
      :columns="columns"
      :rows="rows"
      :query="query"
      :loading="loading"
      :page="page"
      :page-size="pageSize"
      :total="total"
      :caption="t('system.notification.caption')"
      @search="onSearch"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #status="{ row }">
        <Badge :variant="statusVariant(row.status as string)">
          {{ statusLabel(row.status as string) }}
        </Badge>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="
        editingId
          ? t('system.notification.edit')
          : t('system.notification.create')
      "
      @submit="onSubmit"
      @cancel="dialogOpen = false"
    >
      <FormField :label="t('system.notification.announcementTitle')" required>
        <Input v-model="form.title" />
      </FormField>
      <FormField :label="t('system.notification.announcementBody')">
        <Textarea v-model="form.body" :rows="6" />
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
</style>
