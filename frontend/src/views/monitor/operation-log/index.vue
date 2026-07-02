<script setup lang="ts">
import { ref } from "vue";
import { useOperationLog } from "./utils/hook";
import {
  exportOperationLogsCsv,
  exportOperationLogsJson
} from "@/api/audit";
import { downloadRaw, timestampedFilename } from "@/utils/download";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";

import View from "~icons/ep/view";
import Refresh from "~icons/ep/refresh";
import Download from "~icons/ep/download";

defineOptions({
  name: "MonitorOperationLog"
});

const formRef = ref();

const {
  form,
  loading,
  columns,
  dataList,
  pagination,
  onSearch,
  resetForm,
  showDetail,
  handleSizeChange,
  handleCurrentChange,
  handleSelectionChange
} = useOperationLog();

const exporting = ref(false);

async function onExportCsv() {
  exporting.value = true;
  try {
    const content = await exportOperationLogsCsv();
    downloadRaw(
      content,
      timestampedFilename("operation_log", "csv"),
      "text/csv;charset=utf-8"
    );
    message("已下载 CSV", { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? "导出失败"), { type: "error" });
  } finally {
    exporting.value = false;
  }
}

async function onExportJson() {
  exporting.value = true;
  try {
    const content = await exportOperationLogsJson();
    downloadRaw(
      content,
      timestampedFilename("operation_log", "json"),
      "application/json;charset=utf-8"
    );
    message("已下载 JSON", { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? "导出失败"), { type: "error" });
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto"
    >
      <el-form-item label="用户ID：" prop="user_id">
        <el-input
          v-model="form.user_id"
          placeholder="user_id"
          clearable
          class="w-50!"
        />
      </el-form-item>
      <el-form-item label="操作：" prop="action">
        <el-input
          v-model="form.action"
          placeholder="action"
          clearable
          class="w-40!"
        />
      </el-form-item>
      <el-form-item label="资源：" prop="resource">
        <el-input
          v-model="form.resource"
          placeholder="resource"
          clearable
          class="w-40!"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon('ri/search-line')"
          :loading="loading"
          @click="onSearch"
        >
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="resetForm(formRef)">
          重置
        </el-button>
        <el-button
          v-if="hasPerms(['AUD_LOG_EXPORT'])"
          type="success"
          :icon="useRenderIcon(Download)"
          :loading="exporting"
          @click="onExportCsv"
        >
          合规导出 CSV
        </el-button>
        <el-button
          v-if="hasPerms(['AUD_LOG_EXPORT'])"
          type="warning"
          :icon="useRenderIcon(Download)"
          :loading="exporting"
          @click="onExportJson"
        >
          合规导出 JSON
        </el-button>
      </el-form-item>
    </el-form>

    <PureTableBar title="操作日志" :columns="columns" @refresh="onSearch">
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center"
          showOverflowTooltip
          table-layout="auto"
          :loading="loading"
          :size="size"
          adaptive
          :adaptiveConfig="{ offsetBottom: 108 }"
          :data="dataList"
          :columns="dynamicColumns"
          :pagination="{ ...pagination, size }"
          :header-cell-style="{
            background: 'var(--el-fill-color-light)',
            color: 'var(--el-text-color-primary)'
          }"
          @selection-change="handleSelectionChange"
          @page-size-change="handleSizeChange"
          @page-current-change="handleCurrentChange"
        >
          <template #operation="{ row }">
            <el-button
              class="reset-margin"
              link
              type="primary"
              :size="size"
              :icon="useRenderIcon(View)"
              @click="showDetail(row)"
            >
              详情
            </el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>

<style lang="scss" scoped>
.search-form {
  :deep(.el-form-item) {
    margin-bottom: 12px;
  }
}
</style>
