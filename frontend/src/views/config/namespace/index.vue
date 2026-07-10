<script setup lang="ts">
import { ref, h, computed } from "vue";
import { useI18n } from "vue-i18n";
import { listNamespaces, createNamespace, deleteNamespace } from "@/api/config";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElTag } from "element-plus";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "ConfigNamespace" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<any[]>([]);
const total = ref(0);
const form = ref({ keyword: "" });
const page = ref(1);
const size = ref(20);

const columns = computed(() => [
  { label: t("menus.configColumnId"), prop: "id", minWidth: 100 },
  { label: t("menus.configColumnApp"), prop: "app", minWidth: 160 },
  { label: t("menus.configColumnEnv"), prop: "env", width: 160 },
  {
    label: t("menus.userDelete"),
    width: 100,
    cellRenderer: ({ row }: any) =>
      row.id && hasPerms(["CFG_NAMESPACE_DELETE"])
        ? h(
            ElButton,
            { type: "danger", link: true, onClick: () => onDelete(row) },
            () => t("menus.userDelete")
          )
        : null
  }
]);

async function onSearch() {
  loading.value = true;
  try {
    const res = await listNamespaces({
      app: form.value.keyword || undefined,
      page: page.value,
      size: size.value
    });
    dataList.value = res.items || [];
    total.value = res.total || 0;
  } catch (e: any) {
    message(
      t("menus.configLoadFailPrefix", {
        error: translateError(e?.code ?? "")
      }),
      { type: "error" }
    );
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  const app = window.prompt(t("menus.configNamespacePromptApp"));
  if (!app) return;
  const env = window.prompt(t("menus.configNamespacePromptEnv"));
  if (!env) return;
  try {
    await createNamespace({ app, env } as any);
    message(t("menus.configCreated"), { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.configCreateFailFallback")), {
      type: "error"
    });
  }
}

async function onDelete(row: any) {
  if (!confirm(t("menus.configNamespaceConfirmDelete", { app: row.app })))
    return;
  try {
    await deleteNamespace(row.id);
    message(t("menus.configDeleted"), { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.configDeleteFailFallback")), {
      type: "error"
    });
  }
}
</script>

<template>
  <div class="main">
    <el-form inline :model="form" class="search-form pl-8 pt-3">
      <el-form-item :label="`${t('menus.configSearchName')}：`">
        <el-input
          v-model="form.keyword"
          :placeholder="t('menus.configPlaceholderName')"
          clearable
          class="w-50!"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Search)"
          :loading="loading"
          @click="onSearch"
          >{{ t("menus.notificationSearch") }}</el-button
        >
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch"
          >{{ t("menus.tenantRefresh") }}</el-button
        >
        <el-button
          v-if="hasPerms(['CFG_NAMESPACE_CREATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
          >{{ t("menus.configNamespaceCreate") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.configColumnApp')" @refresh="onSearch">
      <template v-slot="{ size: ts, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="ts"
          :data="dataList"
          :columns="dynamicColumns"
        />
      </template>
    </PureTableBar>
  </div>
</template>