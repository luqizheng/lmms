<script setup lang="ts">
import { ref, computed } from "vue";
import {
  listConfigItems,
  listNamespaces,
  createConfigItem,
  deleteConfigItem,
  type ConfigItem
} from "@/api/config";
import { toDisplayValue, reveal, hide, isRevealed } from "@/api/config/masking";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import { message } from "@/utils/message";
import { translateError } from "@/utils/error-translate";
import { hasPerms } from "@/utils/auth";
import { ElButton, ElTag } from "element-plus";
import { h } from "vue";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";
import AddFill from "~icons/ri/add-circle-line";

defineOptions({ name: "ConfigItem" });
const loading = ref(false);
const dataList = ref<ConfigItem[]>([]);
const namespaces = ref<any[]>([]);
const selectedNs = ref<string>("");
const form = ref({ keyword: "" });

const columns = computed(() => [
  { label: "ID", prop: "id", minWidth: 100 },
  { label: "Key", prop: "key", minWidth: 160 },
  {
    label: "Value",
    prop: "value",
    minWidth: 220,
    cellRenderer: ({ row }: any) =>
      h("span", null, [
        toDisplayValue(row.id, row.value, row.is_secret),
        row.is_secret && hasPerms(["CFG_CONFIG_REVEAL"])
          ? h(
              ElButton,
              {
                type: "primary",
                link: true,
                onClick: () =>
                  isRevealed(row.id) ? hide(row.id) : reveal(row.id)
              },
              () => (isRevealed(row.id) ? "隐藏" : "显示")
            )
          : null
      ])
  },
  { label: "类型", prop: "value_type", width: 80 },
  {
    label: "敏感",
    prop: "is_secret",
    width: 70,
    cellRenderer: ({ row }: any) =>
      h(ElTag, { type: row.is_secret ? "warning" : "info" }, () =>
        row.is_secret ? "是" : "否"
      )
  },
  {
    label: "操作",
    width: 100,
    cellRenderer: ({ row }: any) =>
      row.id && hasPerms(["CFG_CONFIG_DELETE"])
        ? h(
            ElButton,
            { type: "danger", link: true, onClick: () => onDelete(row) },
            () => "删除"
          )
        : null
  }
]);

async function loadNamespaces() {
  const res = await listNamespaces({ page: 1, size: 1000 });
  namespaces.value = res.items || [];
  if (namespaces.value.length && !selectedNs.value) {
    selectedNs.value = namespaces.value[0].id;
  }
}

async function onSearch() {
  if (!selectedNs.value) {
    dataList.value = [];
    return;
  }
  loading.value = true;
  try {
    const res = await listConfigItems({
      namespace_id: selectedNs.value,
      key: form.value.keyword || undefined
    });
    dataList.value = res.items || [];
  } catch (e: any) {
    message(`加载失败：${translateError(e?.code ?? "")}`, { type: "error" });
  } finally {
    loading.value = false;
  }
}

async function onCreate() {
  if (!selectedNs.value) {
    message("先选命名空间", { type: "warning" });
    return;
  }
  const key = window.prompt("配置 Key");
  if (!key) return;
  const value = window.prompt("配置 Value");
  if (value === null) return;
  const isSecret = confirm("标记为敏感（敏感值默认 UI 掩码）?");
  try {
    await createConfigItem({
      namespace_id: selectedNs.value,
      key,
      value,
      value_type: "string",
      is_secret: isSecret,
      description: null
    });
    message("已创建", { type: "success" });
    onSearch();
  } catch (e: any) {
    message(translateError(e?.code ?? "创建失败"), { type: "error" });
  }
}

async function onDelete(row: ConfigItem) {
  if (!confirm(`删除 ${row.key}?`)) return;
  try {
    await deleteConfigItem(row.id);
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
      <el-form-item label="命名空间：">
        <el-select
          v-model="selectedNs"
          placeholder="选择"
          class="w-50!"
          @change="onSearch"
        >
          <el-option
            v-for="n in namespaces"
            :key="n.id"
            :label="n.name"
            :value="n.id"
          />
        </el-select>
      </el-form-item>
      <el-form-item label="Key：">
        <el-input
          v-model="form.keyword"
          placeholder="按 key 筛选"
          clearable
          class="w-50!"
          @change="onSearch"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Search)"
          :loading="loading"
          @click="onSearch"
          >搜索</el-button
        >
        <el-button :icon="useRenderIcon(Refresh)" @click="onSearch"
          >刷新</el-button
        >
        <el-button
          v-if="hasPerms(['CFG_CONFIG_CREATE'])"
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="onCreate"
          >新建</el-button
        >
      </el-form-item>
    </el-form>
    <PureTableBar title="配置项" @refresh="onSearch">
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
