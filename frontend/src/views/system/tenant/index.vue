<script setup lang="ts">
import { ref } from "vue";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { http } from "@/utils/http";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "SystemTenant" });
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
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "Slug", prop: "slug", minWidth: 160 },
  { label: "显示名", prop: "display_name", minWidth: 160 },
  { label: "管理员", prop: "admin_user_id", minWidth: 160 },
  { label: "创建时间", prop: "created_at", minWidth: 170 }
];

async function onSearch() {
  loading.value = true;
  try {
    const res = await http.request<Tenant[]>("get", "/v1/iam/tenants");
    dataList.value = res || [];
  } catch (e: any) {
    // 列表接口可能不存在；fallback 到空 + 仅提示
    message(translateError(e?.code ?? "列表接口待实现"), { type: "warning" });
    dataList.value = [];
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  const slug = window.prompt("租户 slug（小写字母数字下划线，长度 2-64）");
  if (!slug) return;
  const displayName = window.prompt("显示名");
  if (!displayName) return;
  const adminUsername = window.prompt("管理员用户名");
  if (!adminUsername) return;
  const adminPassword = window.prompt("管理员密码（≥8 位）");
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
    message("已创建租户", { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? "创建失败"), { type: "error" });
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
          >刷新</el-button
        >
        <el-button
          v-if="hasPerms(['IAM_TENANT_CREATE'])"
          type="success"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
          >新建租户</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar title="租户管理" @refresh="onSearch">
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
