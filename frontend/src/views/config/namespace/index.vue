<script setup lang="ts">
import { ref, h, computed, reactive, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { listNamespaces, createNamespace, deleteNamespace } from "@/api/config";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElTag, ElSelect, ElOption } from "element-plus";
import type { FormInstance, FormRules } from "element-plus";
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

const createDialogVisible = ref(false);
const createSubmitting = ref(false);
const createFormRef = ref<FormInstance>();
const createForm = reactive({
  app: "",
  env: "",
  profile: "",
  description: ""
});
const envOptions = [
  { value: "dev", label: "dev" },
  { value: "staging", label: "staging" },
  { value: "prod", label: "prod" }
];
const createRules: FormRules = {
  app: [
    {
      required: true,
      message: t("menus.configNamespacePromptApp"),
      trigger: "blur"
    }
  ],
  env: [
    {
      required: true,
      message: t("menus.configNamespacePromptEnv"),
      trigger: "change"
    }
  ]
};

function resetCreateForm() {
  createForm.app = "";
  createForm.env = "";
  createForm.profile = "";
  createForm.description = "";
  createFormRef.value?.clearValidate();
}

function openCreateDialog() {
  resetCreateForm();
  createDialogVisible.value = true;
}

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
  if (!createFormRef.value) return;
  await createFormRef.value.validate(async valid => {
    if (!valid) return;
    createSubmitting.value = true;
    try {
      await createNamespace({
        app: createForm.app,
        env: createForm.env,
        profile: createForm.profile || undefined,
        description: createForm.description || undefined
      } as any);
      message(t("menus.configCreated"), { type: "success" });
      createDialogVisible.value = false;
      onSearch();
    } catch (e: any) {
      message(translateError(e?.code ?? t("menus.configCreateFailFallback")), {
        type: "error"
      });
    } finally {
      createSubmitting.value = false;
    }
  });
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

onMounted(() => {
  onSearch();
});
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
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch">{{
          t("menus.tenantRefresh")
        }}</el-button>
        <el-button
          v-if="hasPerms(['CFG_NAMESPACE_CREATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openCreateDialog"
          >{{ t("menus.configNamespaceCreate") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.configColumnApp')" :columns="columns" @refresh="onSearch">
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

    <el-dialog
      v-model="createDialogVisible"
      :title="t('menus.configNamespaceDialogTitle')"
      width="480px"
      :close-on-click-modal="false"
    >
      <el-form
        ref="createFormRef"
        :model="createForm"
        :rules="createRules"
        label-width="90px"
      >
        <el-form-item :label="t('menus.configColumnApp')" prop="app">
          <el-input
            v-model="createForm.app"
            :placeholder="t('menus.configNamespacePromptApp')"
          />
        </el-form-item>
        <el-form-item :label="t('menus.configColumnEnv')" prop="env">
          <el-select
            v-model="createForm.env"
            :placeholder="t('menus.configNamespaceSelectEnv')"
          >
            <el-option
              v-for="opt in envOptions"
              :key="opt.value"
              :label="opt.label"
              :value="opt.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item
          :label="t('menus.configNamespaceFieldProfile')"
          prop="profile"
        >
          <el-input
            v-model="createForm.profile"
            :placeholder="t('menus.configNamespacePlaceholderProfile')"
          />
        </el-form-item>
        <el-form-item
          :label="t('menus.configNamespaceFieldDescription')"
          prop="description"
        >
          <el-input
            v-model="createForm.description"
            type="textarea"
            :placeholder="t('menus.configNamespacePlaceholderDescription')"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createDialogVisible = false">{{
          t("menus.configNamespaceCancel")
        }}</el-button>
        <el-button
          type="primary"
          :loading="createSubmitting"
          @click="onCreate"
          >{{ t("menus.configNamespaceConfirm") }}</el-button
        >
      </template>
    </el-dialog>
  </div>
</template>
