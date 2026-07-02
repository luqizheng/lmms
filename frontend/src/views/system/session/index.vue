<script setup lang="ts">
import { ref, onMounted } from "vue";
import { listIamSessions, revokeIamSession, type IamSession } from "@/api/iam";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { ElButton, ElTag } from "element-plus";
import { h } from "vue";
import Refresh from "~icons/ep/refresh";
import Delete from "~icons/ep/delete";

defineOptions({ name: "SystemSession" });
const loading = ref(false);
const dataList = ref<IamSession[]>([]);

const columns = [
  { label: "Session ID", prop: "id", minWidth: 220 },
  { label: "用户", prop: "user_id", minWidth: 120 },
  { label: "IP", prop: "client_ip", minWidth: 140 },
  { label: "UA", prop: "user_agent", minWidth: 200 },
  { label: "创建时间", prop: "created_at", minWidth: 170 },
  { label: "过期时间", prop: "expires_at", minWidth: 170 },
  {
    label: "状态",
    width: 90,
    cellRenderer: ({ row }: { row: IamSession }) =>
      h(ElTag, { type: row.is_current ? "success" : "info" }, () =>
        row.is_current ? "当前" : "活跃"
      )
  },
  {
    label: "操作",
    width: 100,
    cellRenderer: ({ row }: { row: IamSession }) =>
      row.is_current
        ? null
        : h(
            ElButton,
            {
              type: "danger",
              link: true,
              onClick: () => onRevoke(row)
            },
            () => "踢下线"
          )
  }
];

async function refresh() {
  loading.value = true;
  try {
    const res = await listIamSessions();
    dataList.value = res || [];
  } catch (e: any) {
    message(translateError(e?.code ?? "加载失败"), { type: "error" });
  } finally {
    loading.value = false;
  }
}

async function onRevoke(row: IamSession) {
  if (!confirm(`确认踢下线 session ${row.id}?`)) return;
  try {
    await revokeIamSession(row.id);
    message("已踢下线", { type: "success" });
    refresh();
  } catch (e: any) {
    message(translateError(e?.code ?? "操作失败"), { type: "error" });
  }
}

onMounted(refresh);
</script>

<template>
  <div class="main">
    <el-form inline class="search-form pl-8 pt-3">
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Refresh)"
          :loading="loading"
          @click="refresh"
          >刷新</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar title="在线会话" @refresh="refresh">
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
