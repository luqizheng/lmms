<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { listFiles, deleteFile } from "@/api/file";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { checkFileSize, FILE_MAX_SIZE_MB } from "@/api/file/limits";
import { ElButton } from "element-plus";
import { h } from "vue";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import Upload from "~icons/ep/upload-filled";

defineOptions({ name: "FileList" });
const loading = ref(false);
const dataList = ref<any[]>([]);
const total = ref(0);
const form = ref({ keyword: "" });
const page = ref(1);
const size = ref(20);

const columns = [
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "文件名", prop: "filename", minWidth: 200 },
  { label: "类型", prop: "content_type", width: 140 },
  {
    label: "大小",
    prop: "size",
    width: 100,
    formatter: ({ size }: any) => (size ? `${(size / 1024).toFixed(1)} KB` : "-")
  },
  { label: "上传时间", prop: "created_at", width: 170 },
  {
    label: "操作",
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
            () => "删除"
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
    message(`加载失败：${translateError(e?.code ?? "")}`, { type: "error" });
  } finally {
    loading.value = false;
  }
}

async function onDelete(row: any) {
  try {
    await deleteFile(row.id);
    message("已删除", { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? "删除失败"), { type: "error" });
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
      <el-form-item label="文件名：">
        <el-input v-model="form.keyword" placeholder="按文件名筛选" clearable class="w-50!" />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" :icon="useRenderIcon(Search)" :loading="loading" @click="onSearch">
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch">刷新</el-button>
        <el-button
          v-if="hasPerms(['FILE_UPLOAD'])"
          type="success"
          :icon="useRenderIcon(Upload)"
          @click="goUpload"
        >
          上传
        </el-button>
      </el-form-item>
    </el-form>
    <PureTableBar title="文件列表" @refresh="onSearch">
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