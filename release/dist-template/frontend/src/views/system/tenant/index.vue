<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { http } from "@/utils/http";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "SystemTenant" });
const { t } = useI18n();
const loading = ref(false);
const dataList = ref<any[]>([]);

interface Tenant {
  id: string;
  slug: string;
  display_name: string;
  admin_user_id?: string;
  created_at?: string;
}

const columns = [
  { label: t("menus.tenantColumnId"), prop: "id", minWidth: 100 },
  { label: t("menus.tenantColumnSlug"), prop: "slug", minWidth: 160 },
  {
    label: t("menus.tenantColumnDisplayName"),
    prop: "display_name",
    minWidth: 160
  },
  {
    label: t("menus.tenantColumnAdmin"),
    prop: "admin_user_id",
    minWidth: 160
  },
  {
    label: t("menus.tenantColumnCreatedAt"),
    prop: "created_at",
    minWidth: 170
  }
];

async function onSearch() {
  loading.value = true;
  try {
    const res = await http.request<Tenant[]>("get", "/v1/iam/tenants");
    dataList.value = res || [];
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.tenantListPending")), {
      type: "warning"
    });
    dataList.value = [];
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  const slug = window.prompt(t("menus.tenantSlugPrompt"));
  if (!slug) return;
  const displayName = window.prompt(t("menus.tenantDisplayNamePrompt"));
  if (!displayName) return;
  const adminUsername = window.prompt(t("menus.tenantAdminUsernamePrompt"));
  if (!adminUsername) return;
  const adminPassword = window.prompt(t("menus.tenantPasswordPrompt"));
  if (!adminPassword) return;
  try {
    await http.request<{ tenant_id: string; admin_user_id: string }>(
      "post",
      "/v1/iam/tenants",
      {
        data: {
          tenant_slug: slug,
          display_name: displayName,
          admin_username: adminUsername,
          admin_password: adminPassword
        }
      }
    );
    message(t("menus.tenantCreateSuccess"), { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? t("menus.tenantCreateFail")), {
      type: "error"
    });
  }
}
</script>

<template>
  <div class="main">
    <el-form inline class="search-form pl-8 pt-3">
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Refresh)"
          :loading="loading"
          @click="onSearch"
          >{{ t("menus.tenantRefresh") }}</el-button
        >
        <el-button
          v-if="hasPerms(['IAM_TENANT_CREATE'])"
          type="success"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
          >{{ t("menus.tenantCreate") }}</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar :title="t('menus.systemTenant')" @refresh="onSearch">
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
