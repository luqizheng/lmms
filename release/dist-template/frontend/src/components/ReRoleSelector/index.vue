<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from "vue";
import { ElSelect, ElOption } from "element-plus";
import { listRoles, type Role } from "@/api/role";

export interface RoleValue {
  id: string;
  name: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: RoleValue | RoleValue[] | null;
    multiple?: boolean;
    placeholder?: string;
    disabled?: boolean;
  }>(),
  {
    multiple: true,
    placeholder: "搜索角色",
    disabled: false
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: RoleValue | RoleValue[] | null];
  change: [value: RoleValue | RoleValue[] | null];
}>();

const searchKeyword = ref("");
const loading = ref(false);
const roleList = ref<Role[]>([]);
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

const selectedId = computed({
  get() {
    if (!props.modelValue) return "";
    if (Array.isArray(props.modelValue)) {
      return props.modelValue.map(v => v.id);
    }
    return props.modelValue.id;
  },
  set(val: string | string[]) {
    handleChange(val);
  }
});

async function fetchRoles(keyword: string) {
  if (!keyword.trim()) {
    roleList.value = [];
    return;
  }

  loading.value = true;
  try {
    const response = await listRoles({
      keyword,
      page: 1,
      size: 20
    });
    roleList.value = response?.items ?? [];
  } catch {
    roleList.value = [];
  } finally {
    loading.value = false;
  }
}

function handleRemoteSearch(keyword: string) {
  searchKeyword.value = keyword;
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    fetchRoles(keyword);
  }, 300);
}

function handleChange(val: string | string[]) {
  if (!val) {
    emit("update:modelValue", null);
    emit("change", null);
    return;
  }

  const idArray = Array.isArray(val) ? val : [val];
  const newValue = idArray.map(id => {
    const role = roleList.value.find(r => r.id === Number(id));
    if (role) {
      return { id: String(role.id), name: role.name };
    }
    if (Array.isArray(props.modelValue)) {
      const existing = props.modelValue.find(v => v.id === id);
      return existing || { id, name: id };
    }
    return { id, name: id };
  });

  if (props.multiple) {
    emit("update:modelValue", newValue);
    emit("change", newValue);
  } else {
    emit("update:modelValue", newValue[0] || null);
    emit("change", newValue[0] || null);
  }
}

function handleClear() {
  emit("update:modelValue", null);
  emit("change", null);
}

onUnmounted(() => {
  if (debounceTimer) clearTimeout(debounceTimer);
});
</script>

<template>
  <ElSelect
    v-model="selectedId"
    :multiple="multiple"
    :placeholder="placeholder"
    :disabled="disabled"
    :loading="loading"
    :filterable="true"
    :remote="true"
    :remote-method="handleRemoteSearch"
    :clearable="!disabled"
    class="re-role-selector"
    @clear="handleClear"
  >
    <ElOption
      v-for="role in roleList"
      :key="role.id"
      :label="role.name"
      :value="String(role.id)"
    >
      <span class="option-name">{{ role.name }}</span>
      <span class="option-code">{{ role.code }}</span>
    </ElOption>
    <template #empty>
      {{ searchKeyword ? "未找到匹配角色" : "请输入关键词搜索" }}
    </template>
  </ElSelect>
</template>

<style scoped>
.re-role-selector {
  width: 100%;
}

.option-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.option-code {
  margin-left: 8px;
  font-size: 12px;
  color: #909399;
}
</style>
