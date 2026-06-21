<script setup lang="ts">
import { ref, computed } from "vue";
import { ElTabs, ElTabPane, ElTag } from "element-plus";
import ReUserSelector, { type UserValue } from "@/components/ReUserSelector";
import ReRoleSelector, { type RoleValue } from "@/components/ReRoleSelector";
import ReOrgSelector, { type OrgValue } from "@/components/ReOrgSelector";

export interface ScopeValue {
  scope_type: "user" | "role" | "dept";
  scope_id: string;
  name: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: ScopeValue[];
    placeholder?: string;
    disabled?: boolean;
    showUser?: boolean;
    showRole?: boolean;
    showDept?: boolean;
  }>(),
  {
    placeholder: "请选择",
    disabled: false,
    showUser: true,
    showRole: true,
    showDept: true
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: ScopeValue[]];
  change: [value: ScopeValue[]];
}>();

const activeTab = ref<"user" | "role" | "dept">("user");

const selectedUsers = computed({
  get: () =>
    props.modelValue
      .filter(v => v.scope_type === "user")
      .map(v => ({ id: v.scope_id, name: v.name })),
  set: (val: UserValue[]) => {
    const newValue = [
      ...props.modelValue.filter(v => v.scope_type !== "user"),
      ...val.map(v => ({
        scope_type: "user" as const,
        scope_id: v.id,
        name: v.name
      }))
    ];
    emit("update:modelValue", newValue);
    emit("change", newValue);
  }
});

const selectedRoles = computed({
  get: () =>
    props.modelValue
      .filter(v => v.scope_type === "role")
      .map(v => ({ id: v.scope_id, name: v.name })),
  set: (val: RoleValue[]) => {
    const newValue = [
      ...props.modelValue.filter(v => v.scope_type !== "role"),
      ...val.map(v => ({
        scope_type: "role" as const,
        scope_id: v.id,
        name: v.name
      }))
    ];
    emit("update:modelValue", newValue);
    emit("change", newValue);
  }
});

const selectedDepts = computed({
  get: () =>
    props.modelValue
      .filter(v => v.scope_type === "dept")
      .map(v => ({ id: v.scope_id, name: v.name, path: [] })),
  set: (val: OrgValue[]) => {
    const newValue = [
      ...props.modelValue.filter(v => v.scope_type !== "dept"),
      ...val.map(v => ({
        scope_type: "dept" as const,
        scope_id: v.id,
        name: v.name
      }))
    ];
    emit("update:modelValue", newValue);
    emit("change", newValue);
  }
});

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

function getTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    user: "用户",
    role: "角色",
    dept: "部门"
  };
  return labels[type] || type;
}
</script>

<template>
  <div class="re-scope-selector" :class="{ disabled }">
    <div class="selector-input">
      <div class="selected-tags">
        <ElTag
          v-for="(item, index) in modelValue.slice(0, 3)"
          :key="`${item.scope_type}-${item.scope_id}`"
          closable
          size="small"
          @close="removeTag(index)"
        >
          <span class="tag-label">{{ getTypeLabel(item.scope_type) }}</span>
          <span class="tag-text">{{ item.name }}</span>
        </ElTag>
        <span v-if="modelValue.length > 3" class="tag tag-more">
          +{{ modelValue.length - 3 }}
        </span>
        <span v-if="modelValue.length === 0" class="placeholder">
          {{ placeholder }}
        </span>
      </div>
      <button
        v-if="modelValue.length > 0"
        class="clear-btn"
        title="清空"
        @click.stop="clearAll"
      >
        <svg viewBox="0 0 24 24" fill="none" width="16" height="16">
          <path
            d="M18 6L6 18M6 6l12 12"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
          />
        </svg>
      </button>
    </div>

    <ElTabs v-model="activeTab" class="selector-content">
      <ElTabPane v-if="showUser" label="用户" name="user">
        <ReUserSelector
          v-model="selectedUsers"
          :disabled="disabled"
          placeholder="搜索用户"
        />
      </ElTabPane>
      <ElTabPane v-if="showRole" label="角色" name="role">
        <ReRoleSelector
          v-model="selectedRoles"
          :disabled="disabled"
          placeholder="搜索角色"
        />
      </ElTabPane>
      <ElTabPane v-if="showDept" label="部门" name="dept">
        <ReOrgSelector
          v-model="selectedDepts"
          :disabled="disabled"
          placeholder="选择部门"
        />
      </ElTabPane>
    </ElTabs>
  </div>
</template>

<style scoped>
.re-scope-selector {
  background: #fff;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  transition: all 0.2s;
}

.re-scope-selector:hover:not(.disabled) {
  border-color: #c0c4cc;
}

.re-scope-selector.disabled {
  pointer-events: none;
  opacity: 0.6;
}

.selector-input {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
  min-height: 32px;
  padding: 4px 8px;
  border-bottom: 1px solid #e4e7ed;
}

.selected-tags {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  gap: 4px;
  align-items: center;
}

.tag-more {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  font-size: 12px;
  color: #909399;
  background: #f5f7fa;
  border: 1px solid #e4e7ed;
  border-radius: 4px;
}

.tag-label {
  padding-right: 4px;
  margin-right: 4px;
  font-size: 10px;
  color: #60a8ff;
  border-right: 1px solid #b3d8ff;
}

.tag-text {
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #409eff;
  white-space: nowrap;
}

.placeholder {
  font-size: 14px;
  color: #c0c4cc;
}

.clear-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  color: #909399;
  cursor: pointer;
  background: transparent;
  border: none;
  border-radius: 4px;
  transition: all 0.2s;
}

.clear-btn:hover {
  color: #606266;
  background: #f5f7fa;
}

.selector-content {
  border-top: none;
}
</style>
