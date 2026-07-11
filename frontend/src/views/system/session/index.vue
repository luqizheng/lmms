<script setup lang="ts">
import { ref, onMounted, h } from "vue";
import { useI18n } from "vue-i18n";
import { listIamSessions, revokeIamSession, type IamSession } from "@/api/iam";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { ElButton, ElTag } from "element-plus";
import Refresh from "~icons/ep/refresh";
import Delete from "~icons/ep/delete";

defineOptions({ name: "SystemSession" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<IamSession[]>([]);

const columns = [
  {
    label: t("menus.sessionColumnId"),
    prop: "id",
    minWidth: 220
  },
  { label: t("menus.sessionColumnUser"), prop: "user_id", minWidth: 120 },
  { label: t("menus.sessionColumnIp"), prop: "client_ip", minWidth: 140 },
  { label: t("menus.sessionColumnUa"), prop: "user_agent", minWidth: 200 },
  {
    label: t("menus.sessionColumnCreatedAt"),
    prop: "created_at",
    minWidth: 170
  },
  {
    label: t("menus.sessionColumnExpiresAt"),
    prop: "expires_at",
    minWidth: 170
  },
  {
    label: t("menus.sessionColumnStatus"),
    width: 90,
    cellRenderer: ({ row }: { row: IamSession }) =>
      h(ElTag, { type: row.is_current ? "success" : "info" }, () =>
        row.is_current
          ? t("menus.sessionStatusCurrent")
          : t("menus.sessionStatusActive")
      )
  },
  {
    label: t("menus.sessionColumnOperation"),
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
            () => t("menus.sessionRevoke")
          )
  }
];

async function refresh() {
  loading.value = true;
  try {
    const res = await listIamSessions();
    dataList.value = res || [];
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.sessionLoadFail")), {
      type: "error"
    });
  } finally {
    loading.value = false;
  }
}

async function onRevoke(row: IamSession) {
  if (!confirm(t("menus.sessionConfirmRevoke", { id: row.id }))) return;
  try {
    await revokeIamSession(row.id);
    message(t("menus.sessionRevokeSuccess"), { type: "success" });
    refresh();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.sessionActionFail")), {
      type: "error"
    });
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
          >{{ t("menus.tenantRefresh") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.systemSession')" @refresh="refresh">
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
