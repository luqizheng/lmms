<script setup lang="ts">
import { ref, onMounted } from "vue";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { createApiKey, listApiKeys, type ApiKeySummary } from "@/api/token";
import {
  recordApiKey,
  removeCachedApiKey,
  listCachedApiKeys,
  type CachedApiKey
} from "@/api/api-key-cache";
import { hasPerms } from "@/utils/auth";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";
import CopyDocument from "~icons/ep/copy-document";

defineOptions({ name: "SystemTokenApiKey" });
const loading = ref(false);
const dataList = ref<ApiKeySummary[]>([]);

const form = ref({ name: "", scopes: "" });

const columns = [
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "名称", prop: "name", minWidth: 160 },
  {
    label: "Scopes",
    prop: "permissions",
    minWidth: 180,
    formatter: ({ permissions }: ApiKeySummary) =>
      permissions?.length ? permissions.join(", ") : "-"
  },
  { label: "状态", prop: "status", minWidth: 100 },
  {
    label: "创建时间",
    prop: "created_at",
    minWidth: 170,
    formatter: ({ created_at }: ApiKeySummary) =>
      created_at ? new Date(created_at).toLocaleString() : "-"
  },
  {
    label: "操作",
    width: 100,
    slot: "operation"
  }
];

async function refresh() {
  loading.value = true;
  try {
    const res = await listApiKeys({ page: 1, size: 100 });
    dataList.value = res.items || [];
  } catch (e: any) {
    message(translateError(e?.code ?? "列表加载失败"), { type: "error" });
    dataList.value = [];
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  if (!form.value.name) {
    message("请输入名称", { type: "warning" });
    return;
  }
  const scopes = form.value.scopes
    ? form.value.scopes.split(",").map(s => s.trim()).filter(Boolean)
    : [];
  loading.value = true;
  try {
    const res = await createApiKey({ name: form.value.name, scopes });
    recordApiKey({
      id: res.id,
      api_key: res.api_key,
      name: form.value.name,
      scopes,
      created_at: Date.now()
    });
    message("已创建（secret 已保存到本地缓存）", { type: "success" });
    form.value = { name: "", scopes: "" };
    refresh();
  } catch (e: any) {
    message(translateError(e?.code ?? "创建失败"), { type: "error" });
  } finally {
    loading.value = false;
  }
}

function onCopy(row: CachedApiKey) {
  // 列表来自后端，secret 不在表里 → 仅当本地缓存有此 id 时能复制
  const cached = listCachedApiKeys().find(c => c.id === row.id);
  if (!cached) {
    message("本浏览器未缓存该 key 的 secret；后端不返回明文", { type: "warning" });
    return;
  }
  navigator.clipboard.writeText(cached.api_key);
  message("已复制完整 key", { type: "success" });
}

function onDelete(row: ApiKeySummary) {
  if (!confirm(`从本地缓存删除 ${row.name}?`)) return;
  removeCachedApiKey(row.id);
  message("缓存已清除（后端记录未变）", { type: "success" });
}

onMounted(refresh);
</script>

<template>
  <div class="main">
    <el-form inline :model="form" class="search-form pl-8 pt-3">
      <el-form-item label="名称：">
        <el-input v-model="form.name" placeholder="API Key 名称" clearable class="w-40!" />
      </el-form-item>
      <el-form-item label="Scopes：">
        <el-input v-model="form.scopes" placeholder="逗号分隔" clearable class="w-40!" />
      </el-form-item>
      <el-form-item>
        <el-button v-if="hasPerms(['SYS_SETTING_UPDATE'])" type="primary" :icon="useRenderIcon(AddFill)" :loading="loading" @click="onCreate">新建</el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="refresh">刷新</el-button>
      </el-form-item>
    </el-form>
    <el-alert
      title="列表来自后端 GET /v1/api-keys；secret 仅创建时返回一次，本地缓存保存以便复制。"
      type="info"
      show-icon
      :closable="false"
      class="ml-8 mr-8"
    />
    <PureTableBar title="API Key 管理" @refresh="refresh">
      <template v-slot="{ size: ts, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :data="dataList"
          :size="ts"
          :columns="dynamicColumns"
        >
          <template #operation="{ row }">
            <el-button v-if="hasPerms(['SYS_SETTING_READ'])" link type="primary" :icon="useRenderIcon(CopyDocument)" @click="onCopy(row)">复制</el-button>
            <el-button v-if="hasPerms(['SYS_SETTING_UPDATE'])" link type="danger" @click="onDelete(row)">清除缓存</el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>