<script setup lang="ts">
import { ref, computed, h } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { listFiles, deleteFile, fileUrl } from "@/api/file";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElImage, ElTag } from "element-plus";
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

const selectedRows = ref<any[]>([]);
const isDragOver = ref(false);
let dragCounter = 0;

const IMAGE_EXT = /\.(png|jpe?g|gif|webp|bmp|svg)$/i;
function isImage(name: string | undefined): boolean {
  return !!name && IMAGE_EXT.test(name);
}

const columns = [
  { label: t("menus.fileColumnId"), prop: "id", minWidth: 100 },
  {
    label: t("menus.fileColumnFilename"),
    prop: "filename",
    minWidth: 220,
    cellRenderer: ({ row }: any) => {
      if (isImage(row.filename)) {
        const imgSrc = fileUrl(row.url || row.preview_url);
        return h("div", { class: "flex items-center gap-2" }, [
          h(ElImage, {
            src: imgSrc,
            alt: row.filename,
            class: "file-thumb",
            lazy: true,
            "preview-src-list": imgSrc ? [imgSrc] : [],
            "hide-on-click-modal": true
          }),
          h("span", { class: "truncate" }, row.filename)
        ]);
      }
      return h("span", { class: "truncate" }, row.filename);
    }
  },
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

function onSelectionChange(rows: any[]) {
  selectedRows.value = rows;
}

const selectedCount = computed(() => selectedRows.value.length);

async function bulkDelete() {
  if (selectedCount.value === 0) return;
  if (!confirm(t("menus.fileBulkDeleteConfirm", { n: selectedCount.value })))
    return;
  let ok = 0;
  let fail = 0;
  for (const row of selectedRows.value) {
    try {
      await deleteFile(row.id);
      ok++;
    } catch {
      fail++;
    }
  }
  const summary =
    fail > 0
      ? t("menus.fileBulkDeletePartial", { ok, fail })
      : t("menus.fileBulkDeleteSuccess", { n: ok });
  message(summary, { type: fail > 0 ? "warning" : "success" });
  selectedRows.value = [];
  await onSearch();
}

function onDragEnter(e: DragEvent) {
  e.preventDefault();
  dragCounter++;
  isDragOver.value = true;
}

function onDragLeave(e: DragEvent) {
  e.preventDefault();
  dragCounter--;
  if (dragCounter <= 0) {
    isDragOver.value = false;
    dragCounter = 0;
  }
}

function onDragOver(e: DragEvent) {
  e.preventDefault();
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  isDragOver.value = false;
  dragCounter = 0;
  // 携带文件元数据跳上传页；上传页支持接收 ?files= 预填（若实现）
  router.push("/file/upload");
}
</script>

<template>
  <div
    class="main drop-zone-wrap"
    :class="{ 'is-drag-over': isDragOver }"
    @dragenter="onDragEnter"
    @dragleave="onDragLeave"
    @dragover="onDragOver"
    @drop="onDrop"
  >
    <div v-if="isDragOver" class="drop-overlay">
      <div class="drop-hint">
        <el-icon :size="48"><Upload /></el-icon>
        <div class="text-lg mt-2">{{ t("menus.fileDropHint") }}</div>
      </div>
    </div>

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
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch">{{
          t("menus.tenantRefresh")
        }}</el-button>
        <el-button
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="success"
          :icon="useRenderIcon(Upload)"
          @click="goUpload"
          >{{ t("menus.fileNew") }}</el-button
        >
        <el-button
          v-if="hasPerms(['FILE_DELETE']) && selectedCount > 0"
          type="danger"
          @click="bulkDelete"
        >
          {{ t("menus.fileBulkDelete") }} ({{ selectedCount }})
        </el-button>
      </el-form-item>
    </el-form>

    <div v-if="selectedCount > 0" class="selected-banner">
      <el-tag effect="dark" type="info">
        {{ t("menus.fileSelected", { n: selectedCount }) }}
      </el-tag>
    </div>

    <PureTableBar :title="t('menus.fileColumnFilename')" @refresh="onSearch" :columns="columns" >
      <template v-slot="{ size: ts, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="ts"
          :data="dataList"
          :columns="dynamicColumns"
          @selection-change="onSelectionChange"
        />
      </template>
    </PureTableBar>
  </div>
</template>

<style lang="scss" scoped>
.drop-zone-wrap {
  position: relative;
  min-height: 400px;
}

.is-drag-over {
  outline: 2px dashed var(--el-color-primary);
  outline-offset: -8px;
}

.drop-overlay {
  position: absolute;
  inset: 0;
  background: rgba(64, 158, 255, 0.08);
  z-index: 999;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.drop-hint {
  text-align: center;
  color: var(--el-color-primary);
  font-weight: 600;
}

.file-thumb {
  width: 32px;
  height: 32px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
}

.selected-banner {
  padding: 4px 12px;
  background: var(--el-color-info-light-9);
  border-radius: 4px;
  margin: 8px 0;
}
</style>
