<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
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
const { t } = useI18n();

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
    message(t("menus.monitorExportSuccess"), { type: "success" });
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.monitorExportFailFallback")), {
      type: "error"
    });
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
        {{ t("menus.monitorExportCsv") }}
      </el-button>
    </div>
    <PureTableBar
      :title="t('menus.loginMonitorLog')"
      :columns="columns"
      @refresh="onSearch"
    >
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
