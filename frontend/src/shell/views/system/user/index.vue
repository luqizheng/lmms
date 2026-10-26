<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { http } from "@/shell/api";
import { useAuthStore } from "@/shell/store/auth";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FormField } from "@/components/ui/form-field";
import { Badge } from "@/components/ui/badge";
import {
  DataTable,
  FormDialog,
  type QueryField,
  type TableColumn
} from "@/shell/components";

interface IamUser {
  id: string;
  username: string;
  nickname: string;
  email?: string;
  phone?: string;
  status: string;
  created_at: string;
}

interface UserListResponse {
  items: IamUser[];
  total: number;
  page: number;
  size: number;
}

const { t } = useI18n();
const auth = useAuthStore();

const rows = ref<IamUser[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 10;
const loading = ref(false);
const keyword = ref("");

const dialogOpen = ref(false);
const submitting = ref(false);
const editingId = ref<string | undefined>(undefined);
const form = ref({
  username: "",
  nickname: "",
  password: "",
  email: "",
  phone: ""
});

const columns: TableColumn[] = [
  { label: t("system.common.username"), prop: "username" },
  { label: t("system.common.nickname"), prop: "nickname" },
  { label: t("system.common.email"), prop: "email" },
  { label: t("system.common.status"), prop: "status", slot: "status" },
  { label: t("system.common.createdAt"), prop: "created_at", numeric: true }
];

const query: QueryField[] = [
  { prop: "keyword", placeholder: t("system.user.searchPlaceholder") },
  {
    prop: "status",
    type: "select",
    placeholder: t("system.user.statusPlaceholder"),
    options: [
      { label: t("system.user.statusActive"), value: "active" },
      { label: t("system.user.statusDisabled"), value: "disabled" }
    ]
  }
];

async function load() {
  loading.value = true;
  try {
    const result = await http.get<UserListResponse>("/v1/iam/users", {
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

function openCreate() {
  editingId.value = undefined;
  form.value = {
    username: "",
    nickname: "",
    password: "",
    email: "",
    phone: ""
  };
  dialogOpen.value = true;
}

function openEdit(row: IamUser) {
  editingId.value = row.id;
  form.value = {
    username: row.username,
    nickname: row.nickname,
    password: "",
    email: row.email ?? "",
    phone: row.phone ?? ""
  };
  dialogOpen.value = true;
}

function openEditByRow(row: Record<string, unknown>) {
  openEdit(row as unknown as IamUser);
}

async function onSubmit() {
  submitting.value = true;
  try {
    if (editingId.value) {
      await http.put(`/v1/iam/users/${editingId.value}`, {
        nickname: form.value.nickname,
        email: form.value.email || undefined,
        phone: form.value.phone || undefined
      });
    } else {
      await http.post("/v1/iam/users", {
        username: form.value.username,
        nickname: form.value.nickname,
        password: form.value.password,
        email: form.value.email || undefined,
        phone: form.value.phone || undefined
      });
    }
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
      <h1 class="ui-page__title">{{ t("system.user.title") }}</h1>
      <Button v-if="auth.hasPermission('IAM_USER_CREATE')" @click="openCreate">
        {{ t("system.user.create") }}
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
      :caption="t('system.user.caption')"
      @search="onSearch"
      @update:page="
        value => {
          page = value;
          load();
        }
      "
    >
      <template #status="{ row }">
        <Badge :variant="row.status === 'active' ? 'success' : 'neutral'">
          {{
            row.status === "active"
              ? t("system.user.statusActive")
              : t("system.user.statusDisabled")
          }}
        </Badge>
      </template>
      <template #username="{ row }">
        <button type="button" class="ui-page__link" @click="openEditByRow(row)">
          {{ row.username }}
        </button>
      </template>
    </DataTable>

    <FormDialog
      v-model:open="dialogOpen"
      :title="editingId ? t('system.user.edit') : t('system.user.create')"
      @submit="onSubmit"
      @cancel="dialogOpen = false"
    >
      <FormField :label="t('system.user.username')" required>
        <Input v-model="form.username" :disabled="Boolean(editingId)" />
      </FormField>
      <FormField :label="t('system.user.nickname')" required>
        <Input v-model="form.nickname" />
      </FormField>
      <FormField v-if="!editingId" :label="t('system.user.password')" required>
        <Input v-model="form.password" type="password" />
      </FormField>
      <FormField :label="t('system.user.email')">
        <Input v-model="form.email" type="email" />
      </FormField>
      <FormField :label="t('system.user.phone')">
        <Input v-model="form.phone" />
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
