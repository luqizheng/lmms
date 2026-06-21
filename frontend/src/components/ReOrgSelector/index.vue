<script setup lang="ts">
import { ref, computed, watch, onMounted } from "vue";
import { ElTreeSelect, ElTag, ElLoading } from "element-plus";
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
    showAllLevels?: boolean;
  }>(),
  {
    multiple: true,
    placeholder: "选择部门",
    disabled: false,
    showAllLevels: true
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

const treeData = computed(() => {
  function convertTree(items: IamOrgUnit[], parentNames: string[] = []): IamOrgUnit[] {
    return items.map(item => {
      const newNames = [item.name, ...parentNames];
      const node: IamOrgUnit = {
        ...item,
        path: newNames
      };
      if (item.children && item.children.length > 0) {
        node.children = convertTree(item.children, newNames);
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

function findParent(item: IamOrgUnit, tree: IamOrgUnit[]): IamOrgUnit | undefined {
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

function removeTag(index: number) {
  if (props.disabled) return;
  const newValue = props.modelValue.filter((_, idx) => idx !== index);
  emit("update:modelValue", newValue);
  emit("change", newValue);
}

function clearAll() {
  if (props.disabled) return;
  emit("update:modelValue", []);
  emit("change", []);
}

function getTagLabel(item: OrgValue): string {
  return props.showAllLevels ? item.path.join("/") : item.name;
}

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
    :tree-props="treeProps"
    :data="treeData"
    :collapse-tags="true"
    :max-collapse-tags="3"
    @change="handleChange"
    class="re-org-selector"
  >
    <template #default>
      <div class="selector-input">
        <div class="selected-tags">
          <ElTag
            v-for="(item, index) in modelValue.slice(0, 3)"
            :key="item.id"
            closable
            size="small"
            @close="removeTag(index)"
          >
            {{ getTagLabel(item) }}
          </ElTag>
          <span v-if="modelValue.length > 3" class="tag tag-more">
            +{{ modelValue.length - 3 }}
          </span>
        </div>
        <button
          v-if="modelValue.length > 0"
          class="clear-btn"
          @click.stop="clearAll"
          title="清空"
        >
          <svg viewBox="0 0 24 24" fill="none" width="16" height="16">
            <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          </svg>
        </button>
      </div>
    </template>
  </ElTreeSelect>
</template>

<style scoped>
.re-org-selector {
  width: 100%;
}

.tag-more {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  background: #f5f7fa;
  border: 1px solid #e4e7ed;
  border-radius: 4px;
  font-size: 12px;
  color: #909399;
}

.clear-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: #909399;
  cursor: pointer;
  border-radius: 4px;
  transition: all 0.2s;
  margin-left: 4px;
}

.clear-btn:hover {
  background: #f5f7fa;
  color: #606266;
}
</style>