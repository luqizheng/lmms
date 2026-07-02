<script setup lang="ts">
import { ref, h, computed } from "vue";
import {
  listNamespaces,
  createNamespace,
  deleteNamespace
} from "@/api/config";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElTag } from "element-plus";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "ConfigNamespace" });
const loading = ref(false);
const dataList = ref<any[]>([]);
const total = ref(0);
const form = ref({ keyword: "" });
const page = ref(1);
const size = ref(20);

const columns = computed(() => [
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "名称", prop: "name", minWidth: 160 },
  { label: "编码", prop: "code", width: 160 },
  {
    label: "操作",
    width: 100,
    cellRenderer: ({ row }: any) =>
      row.id && hasPerms(["CFG_NAMESPACE_DELETE"])
        ? h(
            ElButton,
            { type: "danger", link: true, onClick: () => onDelete(row) },
            () => "删除"
          )
        : null
  }
]);

async function onSearch() {
  loading.value = true;
  try {
    const res = await listNamespaces({
      keyword: form.value.keyword || undefined,
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

async function onCreate() {
  const name = window.prompt("命名空间名称");
  if (!name) return;
  const code = window.prompt("编码（小写字母数字下划线）");
  if (!code) return;
  try {
    await createNamespace({ name, code } as any);
    message("已创建", { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? "创建失败"), { type: "error" });
  }
}

async function onDelete(row: any) {
  if (!confirm(`确认删除命名空间 ${row.name}?`)) return;
  try {
    await deleteNamespace(row.id);
    message("已删除", { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? "删除失败"), { type: "error" });
  }
}
</script>

<template>
  <div class="main">
    <el-form inline :model="form" class="search-form pl-8 pt-3">
      <el-form-item label="名称：">
        <el-input v-model="form.keyword" placeholder="按名称筛选" clearable class="w-50!" />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" :icon="useRenderIcon(Search)" :loading="loading" @click="onSearch">搜索</el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch">刷新</el-button>
        <el-button
          v-if="hasPerms(['CFG_NAMESPACE_CREATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
        >新建</el-button>
      </el-form-item>
    </el-form>
    <PureTableBar title="配置命名空间" @refresh="onSearch">
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