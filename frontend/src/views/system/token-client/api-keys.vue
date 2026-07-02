<script setup lang="ts">
import { ref, onMounted } from "vue";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { createApiKey } from "@/api/token";
import {
  listCachedApiKeys,
  recordApiKey,
  removeCachedApiKey,
  type CachedApiKey
} from "@/api/api-key-cache";
import { hasPerms } from "@/utils/auth";
import Refresh from "~icons/ep/refresh";
import AddFill from "~icons/ri/add-circle-line";
import CopyDocument from "~icons/ep/copy-document";

defineOptions({ name: "SystemTokenApiKey" });
const loading = ref(false);
const dataList = ref<CachedApiKey[]>([]);

const form = ref({ name: "", scopes: "" });

const columns = [
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "名称", prop: "name", minWidth: 160 },
  { label: "Scopes", prop: "scopes", minWidth: 160 },
  {
    label: "Key",
    prop: "api_key",
    minWidth: 240,
    formatter: ({ api_key }: CachedApiKey) =>
      api_key ? api_key.slice(0, 12) + "********" : "-"
  },
  {
    label: "创建时间",
    prop: "created_at",
    minWidth: 170,
    formatter: ({ created_at }: CachedApiKey) =>
      new Date(created_at).toLocaleString()
  },
  {
    label: "操作",
    width: 100,
    slot: "operation"
  }
];

function refresh() {
  dataList.value = listCachedApiKeys();
}

async function onCreate() {
  if (!form.value.name) {
    message("请输入名称", { type: "warning" });
    return;
  }
  const scopes = form.value.scopes
    ? form.value.scopes.split(",").map(s => s.trim()).filter(Boolean)
    : undefined;
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
    message("已创建并保存到本地缓存", { type: "success" });
    form.value = { name: "", scopes: "" };
    refresh();
  } catch (e: any) {
    message(`创建失败：${e?.message ?? e}`, { type: "error" });
  } finally {
    loading.value = false;
  }
}

function onCopy(row: CachedApiKey) {
  navigator.clipboard.writeText(row.api_key);
  message("已复制完整 key", { type: "success" });
}

function onDelete(row: CachedApiKey) {
  if (!confirm(`从本地缓存删除 ${row.name}?`)) return;
  removeCachedApiKey(row.id);
  refresh();
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
      title="后端暂无 list 端点，本页仅展示本浏览器创建过的 key（localStorage）。"
      type="warning"
      show-icon
      :closable="false"
      class="ml-8 mr-8"
    />
    <PureTableBar title="API Key 管理" @refresh="refresh">
      <template v-slot="{ size: ts, dynamicColumns }">
        <pure-table
          align-whole="center"
          :data="dataList"
          :size="ts"
          :columns="dynamicColumns"
        >
          <template #operation="{ row }">
            <el-button v-if="hasPerms(['SYS_SETTING_READ'])" link type="primary" :icon="useRenderIcon(CopyDocument)" @click="onCopy(row)">复制</el-button>
            <el-button v-if="hasPerms(['SYS_SETTING_UPDATE'])" link type="danger" @click="onDelete(row)">删除</el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>