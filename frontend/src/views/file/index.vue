<script setup lang="ts">
import { ref, h } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { listFiles, deleteFile } from "@/api/file";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { checkFileSize, FILE_MAX_SIZE_MB } from "@/api/file/limits";
import { ElButton } from "element-plus";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import Upload from "~icons/ep/upload-filled";

defineOptions({ name: "FileList" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<any[]>([]);
const total = ref(0);
const form = ref({ keyword: "" });
const page = ref(1);
const size = ref(20);

const columns = [
  { label: t("menus.fileColumnId"), prop: "id", minWidth: 100 },
  { label: t("menus.fileColumnFilename"), prop: "filename", minWidth: 200 },
  { label: t("menus.fileColumnType"), prop: "content_type", width: 140 },
  {
    label: t("menus.fileColumnSize"),
    prop: "size",
    width: 100,
    formatter: ({ size }: any) =>
      size ? `${(size / 1024).toFixed(1)} KB` : "-"
  },
  { label: t("menus.fileColumnUploadedAt"), prop: "created_at", width: 170 },
  {
    label: t("menus.userDelete"),
    width: 100,
    cellRenderer: ({ row }: any) =>
      row.id && hasPerms(["FILE_DELETE"])
        ? h(
            ElButton,
            {
              type: "danger",
              link: true,
              onClick: () => onDelete(row)
            },
            () => t("menus.userDelete")
          )
        : null
  }
];

async function onSearch() {
  loading.value = true;
  try {
    const res = await listFiles({
      filename: form.value.keyword || undefined,
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

async function onDelete(row: any) {
  try {
    await deleteFile(row.id);
    message(t("menus.configDeleted"), { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.configDeleteFailFallback")), {
      type: "error"
    });
  }
}

const router = useRouter();

function goUpload() {
  router.push("/file/upload");
}

function onBeforeUpload(file: File) {
  return checkFileSize(file.size);
}
</script>

<template>
  <div class="main">
    <el-form inline :model="form" class="search-form pl-8 pt-3">
      <el-form-item :label="`${t('menus.fileSearchFilename')}：`">
        <el-input
          v-model="form.keyword"
          :placeholder="t('menus.filePlaceholderFilename')"
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
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="success"
          :icon="useRenderIcon(Upload)"
          @click="goUpload"
          >{{ t("menus.fileNew") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.fileColumnFilename')" @refresh="onSearch">
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