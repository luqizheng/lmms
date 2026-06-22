<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from "vue";
import { ElSelect, ElOption } from "element-plus";
import { listIamUsersSimple, type IamUserSimple } from "@/api/iam";

export interface UserValue {
  id: string;
  name: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: UserValue | UserValue[] | null;
    multiple?: boolean;
    placeholder?: string;
    disabled?: boolean;
    remoteMethod?: (keyword: string) => Promise<IamUserSimple[]>;
  }>(),
  {
    multiple: true,
    placeholder: "搜索用户",
    disabled: false
  }
);

const emit = defineEmits<{
  "update:modelValue": [value: UserValue | UserValue[] | null];
  change: [value: UserValue | UserValue[] | null];
}>();

const searchKeyword = ref("");
const loading = ref(false);
const userList = ref<IamUserSimple[]>([]);
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

async function fetchUsers(keyword: string) {
  if (!keyword.trim()) {
    userList.value = [];
    return;
  }

  loading.value = true;
  try {
    if (props.remoteMethod) {
      userList.value = await props.remoteMethod(keyword);
    } else {
      const response = await listIamUsersSimple({
        keyword,
        page: 1,
        size: 20
      });
      userList.value = response?.items ?? [];
    }
  } catch {
    userList.value = [];
  } finally {
    loading.value = false;
  }
}

function handleRemoteSearch(keyword: string) {
  searchKeyword.value = keyword;
  if (debounceTimer) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    fetchUsers(keyword);
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
    const user = userList.value.find(u => u.id === id);
    if (user) {
      return { id: user.id, name: user.nickname };
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
    class="re-user-selector"
    @clear="handleClear"
  >
    <ElOption
      v-for="user in userList"
      :key="user.id"
      :label="user.nickname"
      :value="user.id"
    >
      <span class="option-name">{{ user.nickname }}</span>
    </ElOption>
    <template #empty>
      {{ searchKeyword ? "未找到匹配用户" : "请输入关键词搜索" }}
    </template>
  </ElSelect>
</template>

<style scoped>
.re-user-selector {
  width: 100%;
}

.option-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.option-extra {
  margin-left: 8px;
  font-size: 12px;
  color: #909399;
}
</style>
