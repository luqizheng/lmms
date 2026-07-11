<script setup lang="ts">
import { ref, computed, h } from "vue";
import { useI18n } from "vue-i18n";
import {
  listConfigItems,
  listNamespaces,
  createConfigItem,
  deleteConfigItem,
  type ConfigItem
} from "@/api/config";
import { toDisplayValue, reveal, hide, isRevealed } from "@/api/config/masking";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElTag } from "element-plus";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "ConfigItem" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<ConfigItem[]>([]);
const namespaces = ref<any[]>([]);
const selectedNs = ref<string>("");
const form = ref({ keyword: "" });

const columns = computed(() => [
  { label: t("menus.configColumnId"), prop: "id", minWidth: 100 },
  { label: t("menus.configColumnKey"), prop: "key", minWidth: 160 },
  {
    label: t("menus.configColumnValue"),
    prop: "value",
    minWidth: 220,
    cellRenderer: ({ row }: any) =>
      h("span", null, [
        toDisplayValue(row.id, row.value, row.is_secret),
        row.is_secret && hasPerms(["CFG_CONFIG_REVEAL"])
          ? h(
              ElButton,
              {
                type: "primary",
                link: true,
                onClick: () =>
                  isRevealed(row.id) ? hide(row.id) : reveal(row.id)
              },
              () =>
                isRevealed(row.id)
                  ? t("menus.configItemHideValue")
                  : t("menus.configItemShowValue")
            )
          : null
      ])
  },
  { label: t("menus.configColumnType"), prop: "value_type", width: 80 },
  {
    label: t("menus.configColumnSecret"),
    prop: "is_secret",
    width: 70,
    cellRenderer: ({ row }: any) =>
      h(ElTag, { type: row.is_secret ? "warning" : "info" }, () =>
        row.is_secret
          ? t("menus.configItemSecretYes")
          : t("menus.configItemSecretNo")
      )
  },
  {
    label: t("menus.userDelete"),
    width: 100,
    cellRenderer: ({ row }: any) =>
      row.id && hasPerms(["CFG_CONFIG_DELETE"])
        ? h(
            ElButton,
            { type: "danger", link: true, onClick: () => onDelete(row) },
            () => t("menus.userDelete")
          )
        : null
  }
]);

async function loadNamespaces() {
  const res = await listNamespaces({ page: 1, size: 1000 });
  namespaces.value = res.items || [];
  if (namespaces.value.length && !selectedNs.value) {
    selectedNs.value = namespaces.value[0].id;
  }
}

async function onSearch() {
  if (!selectedNs.value) {
    dataList.value = [];
    return;
  }
  loading.value = true;
  try {
    const res = await listConfigItems({
      namespace_id: selectedNs.value,
      key: form.value.keyword || undefined
    });
    dataList.value = res.items || [];
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
  if (!selectedNs.value) {
    message(t("menus.configItemSelectNsFirst"), { type: "warning" });
    return;
  }
  const key = window.prompt(t("menus.configItemPromptKey"));
  if (!key) return;
  const value = window.prompt(t("menus.configItemPromptValue"));
  if (value === null) return;
  const isSecret = confirm(t("menus.configItemSecretConfirm"));
  try {
    await createConfigItem({
      namespace_id: selectedNs.value,
      key,
      value,
      value_type: "string",
      is_secret: isSecret,
      description: null
    });
    message(t("menus.configCreated"), { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.configCreateFailFallback")), {
      type: "error"
    });
  }
}

async function onDelete(row: ConfigItem) {
  if (!confirm(t("menus.configItemConfirmDelete", { key: row.key }))) return;
  try {
    await deleteConfigItem(row.id);
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
      <el-form-item :label="`${t('menus.configSearchNamespace')}：`">
        <el-select
          v-model="selectedNs"
          :placeholder="t('menus.configPlaceholderNamespace')"
          class="w-50!"
          @change="onSearch"
        >
          <el-option
            v-for="n in namespaces"
            :key="n.id"
            :label="n.name"
            :value="n.id"
          />
        </el-select>
      </el-form-item>
      <el-form-item :label="`${t('menus.configSearchKey')}：`">
        <el-input
          v-model="form.keyword"
          :placeholder="t('menus.configPlaceholderKey')"
          clearable
          class="w-50!"
          @change="onSearch"
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
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch">{{
          t("menus.tenantRefresh")
        }}</el-button>
        <el-button
          v-if="hasPerms(['CFG_CONFIG_CREATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
          >{{ t("menus.configItemCreate") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.configColumnValue')" @refresh="onSearch">
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
