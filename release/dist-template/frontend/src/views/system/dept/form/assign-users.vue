<script setup lang="ts">
import { ref, watch } from "vue";
import { listIamUsers, assignOrgUnitUsers, type IamUser } from "@/api/iam";
import { ElMessage } from "element-plus";
import { hasPerms } from "@/utils/auth";

interface Props {
  orgUnitId: string;
  orgUnitName: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ close: [] }>();

const loading = ref(false);
const users = ref<IamUser[]>([]);
const selected = ref<string[]>([]);
const searchKeyword = ref("");

async function load() {
  loading.value = true;
  try {
    const res = await listIamUsers({ page: 1, size: 1000 });
    users.value = res.items || [];
  } finally {
    loading.value = false;
  }
}

const filtered = (kw: string) =>
  !kw
    ? users.value
    : users.value.filter(u =>
        (u.username + (u.nickname ?? ""))
          .toLowerCase()
          .includes(kw.toLowerCase())
      );

async function onSubmit() {
  try {
    await assignOrgUnitUsers(props.orgUnitId, selected.value);
    ElMessage.success("已保存");
    emit("close");
  } catch (e: any) {
    ElMessage.error(`保存失败：${e?.message ?? e}`);
  }
}

watch(() => props.orgUnitId, load, { immediate: true });
</script>

<template>
  <el-dialog
    :model-value="true"
    :title="`分配成员 - ${orgUnitName}`"
    width="640px"
    @close="emit('close')"
  >
    <el-input
      v-model="searchKeyword"
      placeholder="按 username/nickname 过滤"
      clearable
      class="mb-3"
    />
    <el-checkbox-group v-model="selected" v-loading="loading">
      <el-table :data="filtered(searchKeyword)" max-height="380" border>
        <el-table-column width="56" label="选">
          <template #default="{ row }">
            <el-checkbox :value="row.id" />
          </template>
        </el-table-column>
        <el-table-column prop="username" label="用户名" min-width="120" />
        <el-table-column prop="nickname" label="昵称" min-width="120" />
        <el-table-column prop="email" label="邮箱" min-width="180" />
      </el-table>
    </el-checkbox-group>
    <template #footer>
      <el-button @click="emit('close')">取消</el-button>
      <el-button
        v-if="hasPerms(['IAM_ORG_UPDATE'])"
        type="primary"
        @click="onSubmit"
        >保存</el-button
      >
    </template>
  </el-dialog>
</template>
