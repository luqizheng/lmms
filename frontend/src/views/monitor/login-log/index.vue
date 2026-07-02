<script setup lang="ts">
import { ref } from "vue";
import { useLoginLog } from "./utils/hook";
import { exportLoginLogsCsv } from "@/api/audit";
import { downloadRaw, timestampedFilename } from "@/utils/download";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { PureTableBar } from "@/components/RePureTableBar";
import Download from "~icons/ep/download";

defineOptions({
  name: "MonitorLoginLog"
});

const exporting = ref(false);

const {
  loading,
  columns,
  dataList,
  pagination,
  onSearch,
  handleSizeChange,
  handleCurrentChange,
  handleSelectionChange
} = useLoginLog();

async function onExportCsv() {
  exporting.value = true;
  try {
    const content = await exportLoginLogsCsv();
    downloadRaw(
      content,
      timestampedFilename("login_log", "csv"),
      "text/csv;charset=utf-8"
    );
    message("已下载 CSV", { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? "导出失败"), { type: "error" });
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <div class="main">
    <div class="flex justify-end pr-8 pt-3">
      <el-button
        v-if="hasPerms(['AUD_LOG_EXPORT'])"
        type="success"
        :icon="useRenderIcon(Download)"
        :loading="exporting"
        @click="onExportCsv"
      >
        合规导出 CSV
      </el-button>
    </div>
    <PureTableBar title="登录日志" :columns="columns" @refresh="onSearch">
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
        />
      </template>
    </PureTableBar>
  </div>
</template>
