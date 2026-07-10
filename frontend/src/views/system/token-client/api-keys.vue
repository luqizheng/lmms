<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { createApiKey, listApiKeys, type ApiKeySummary } from "@/api/token";
import {
  recordApiKey,
  removeCachedApiKey,
  listCachedApiKeys,
  type CachedApiKey
} from "@/api/api-key-cache";
import { hasPerms } from "@/utils/auth";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";
import CopyDocument from "~icons/ep/copy-document";

defineOptions({ name: "SystemTokenApiKey" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<ApiKeySummary[]>([]);

const form = ref({ name: "", scopes: "" });

const columns = [
  { label: t("menus.apiKeyColumnId"), prop: "id", minWidth: 100 },
  { label: t("menus.apiKeyColumnName"), prop: "name", minWidth: 160 },
  {
    label: t("menus.apiKeyColumnScopes"),
    prop: "permissions",
    minWidth: 180,
    formatter: ({ permissions }: ApiKeySummary) =>
      permissions?.length ? permissions.join(", ") : "-"
  },
  { label: t("menus.apiKeyColumnStatus"), prop: "status", minWidth: 100 },
  {
    label: t("menus.apiKeyColumnCreatedAt"),
    prop: "created_at",
    minWidth: 170,
    formatter: ({ created_at }: ApiKeySummary) =>
      created_at ? new Date(created_at).toLocaleString() : "-"
  },
  {
    label: t("menus.apiKeyColumnOperation"),
    width: 100,
    slot: "operation"
  }
];

async function refresh() {
  loading.value = true;
  try {
    const res = await listApiKeys({ page: 1, size: 100 });
    dataList.value = res.items || [];
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.apiKeyListLoadFailFallback")), {
      type: "error"
    });
    dataList.value = [];
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  if (!form.value.name) {
    message(t("menus.apiKeyNameRequired"), { type: "warning" });
    return;
  }
  const scopes = form.value.scopes
    ? form.value.scopes
        .split(",")
        .map(s => s.trim())
        .filter(Boolean)
    : [];
  loading.value = true;
  try {
    const res = await createApiKey({ name: form.value.name, scopes });
    recordApiKey({
      id: res.id,
      api_key: res.api_key,
      name: form.value.name,
      scopes,
      created_at: Date.now()
    });
    message(t("menus.apiKeyCreatedHint"), { type: "success" });
    form.value = { name: "", scopes: "" };
    refresh();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.apiKeyCreateFailFallback")), {
      type: "error"
    });
  } finally {
    loading.value = false;
  }
}

function onCopy(row: CachedApiKey) {
  const cached = listCachedApiKeys().find(c => c.id === row.id);
  if (!cached) {
    message(t("menus.apiKeySecretNotCached"), { type: "warning" });
    return;
  }
  navigator.clipboard.writeText(cached.api_key);
  message(t("menus.apiKeyCopiedFull"), { type: "success" });
}

function onDelete(row: ApiKeySummary) {
  if (!confirm(t("menus.apiKeyConfirmClearCache", { name: row.name }))) return;
  removeCachedApiKey(row.id);
  message(t("menus.apiKeyCacheCleared"), { type: "success" });
}

onMounted(refresh);
</script>

<template>
  <div class="main">
    <el-form inline :model="form" class="search-form pl-8 pt-3">
      <el-form-item :label="`${t('menus.apiKeySearchName')}：`">
        <el-input
          v-model="form.name"
          :placeholder="t('menus.apiKeyPlaceholderName')"
          clearable
          class="w-40!"
        />
      </el-form-item>
      <el-form-item :label="`${t('menus.apiKeySearchScopes')}：`">
        <el-input
          v-model="form.scopes"
          :placeholder="t('menus.apiKeyPlaceholderScopes')"
          clearable
          class="w-40!"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          v-if="hasPerms(['SYS_SETTING_UPDATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          :loading="loading"
          @click="onCreate"
          >{{ t("menus.apiKeyCreate") }}</el-button
        >
        <el-button :icon="useRenderIcon(Refresh)" @click="refresh"
          >{{ t("menus.tenantRefresh") }}</el-button
        >
      </el-form-item>
    </el-form>
    <el-alert
      :title="t('menus.apiKeyNotice')"
      type="info"
      show-icon
      :closable="false"
      class="mx-8"
    />
    <PureTableBar :title="t('menus.apiKeyColumnName')" @refresh="refresh">
      <template v-slot="{ size: ts, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :data="dataList"
          :size="ts"
          :columns="dynamicColumns"
        >
          <template #operation="{ row }">
            <el-button
              v-if="hasPerms(['SYS_SETTING_READ'])"
              link
              type="primary"
              :icon="useRenderIcon(CopyDocument)"
              @click="onCopy(row)"
              >{{ t("menus.apiKeyCopy") }}</el-button
            >
            <el-button
              v-if="hasPerms(['SYS_SETTING_UPDATE'])"
              link
              type="danger"
              @click="onDelete(row)"
              >{{ t("menus.apiKeyClearCache") }}</el-button
            >
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>