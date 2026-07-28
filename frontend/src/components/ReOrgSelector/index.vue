<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { ElTreeSelect } from "element-plus";
import { listIamOrgUnits, type IamOrgUnit } from "@/api/iam";

export interface OrgValue {
  id: string;
  name: string;
  path: string[];
}

const props = withDefaults(
  defineProps<{
    modelValue: OrgValue[];
    multiple?: boolean;
    placeholder?: string;
    disabled?: boolean;
  }>(),
  {
    multiple: true,
    placeholder: "选择部门",
    disabled: false
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: OrgValue[]];
  change: [value: OrgValue[]];
}>();

const loading = ref(false);
const orgTree = ref<IamOrgUnit[]>([]);
const treeProps = {
  children: "children",
  label: "name",
  value: "id"
};

const selectedIds = computed(() => {
  return props.modelValue.map(v => v.id);
});

interface IamOrgUnitWithPath extends IamOrgUnit {
  path?: string[];
}

const treeData = computed(() => {
  function convertTree(
    items: IamOrgUnit[],
    parentNames: string[] = []
  ): IamOrgUnitWithPath[] {
    return items.map(item => {
      const newNames = [item.name, ...parentNames];
      const node: IamOrgUnitWithPath = {
        ...item,
        path: newNames
      };
      if (item.children && item.children.length > 0) {
        node.children = convertTree(item.children, newNames) as any;
      }
      return node;
    });
  }
  return convertTree(orgTree.value);
});

async function loadOrgUnits() {
  loading.value = true;
  try {
    const response = await listIamOrgUnits();
    orgTree.value = response.items ?? [];
  } catch {
    orgTree.value = [];
  } finally {
    loading.value = false;
  }
}

function handleChange(ids: string[] | string) {
  const idArray = Array.isArray(ids) ? ids : [ids];
  const newValue: OrgValue[] = idArray.map(id => {
    const node = findNode(id, orgTree.value);
    if (node) {
      return {
        id: node.id,
        name: node.name,
        path: getFullPath(node)
      };
    }
    return { id, name: id, path: [] };
  });
  emit("update:modelValue", newValue);
  emit("change", newValue);
}

function findNode(id: string, tree: IamOrgUnit[]): IamOrgUnit | undefined {
  for (const node of tree) {
    if (node.id === id) return node;
    if (node.children) {
      const found = findNode(id, node.children);
      if (found) return found;
    }
  }
  return undefined;
}

function getFullPath(item: IamOrgUnit, parentNames: string[] = []): string[] {
  const names = [item.name, ...parentNames];
  if (item.parent_id) {
    const parent = findParent(item, orgTree.value);
    if (parent) {
      return getFullPath(parent, names);
    }
  }
  return names;
}

function findParent(
  item: IamOrgUnit,
  tree: IamOrgUnit[]
): IamOrgUnit | undefined {
  for (const node of tree) {
    if (node.id === item.parent_id) {
      return node;
    }
    if (node.children) {
      const found = findParent(item, node.children);
      if (found) return found;
    }
  }
  return undefined;
}

// 保留导出的 tag/清空能力：调用方若要自定义视觉可直接用 tag 或包一层容器；
// ElTreeSelect 的默认 tag 展示 + collapse-tags-tooltip 已能覆盖多选场景

onMounted(() => {
  loadOrgUnits();
});
</script>

<template>
  <ElTreeSelect
    :model-value="selectedIds"
    :multiple="multiple"
    :placeholder="placeholder"
    :disabled="disabled"
    :loading="loading"
    :props="treeProps"
    :data="treeData"
    :collapse-tags="true"
    :collapse-tags-tooltip="true"
    :max-collapse-tags="3"
    class="re-org-selector"
    @change="handleChange"
  />
</template>

<style scoped>
.re-org-selector {
  width: 100%;
}
</style>
