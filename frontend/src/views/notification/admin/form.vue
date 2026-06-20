<script setup lang="ts">
import { ref, watch } from "vue";
import { formRules } from "./utils/rule";
import type { FormProps } from "./utils/types";
import type { ScopesResult } from "@/utils/iam-scopes-cache";

const props = withDefaults(
  defineProps<FormProps & { scopes: ScopesResult }>(),
  {
    scopes: () => ({ depts: [], roles: [], users: [] })
  }
);

const ruleFormRef = ref();
const newFormInline = ref({ ...props.formInline, recipients: [...props.formInline.recipients] });
const activeTab = ref<"user" | "role" | "dept">("user");
const keyword = ref("");

watch(
  () => props.formInline,
  v => {
    newFormInline.value = { ...v, recipients: [...v.recipients] };
  },
  { deep: true }
);

function getRef() {
  return ruleFormRef.value;
}
defineExpose({ getRef });

function filtered(items: { id: number; name: string }[]) {
  return !keyword.value
    ? items
    : items.filter(i => i.name.includes(keyword.value));
}

function isChecked(
  type: "user" | "role" | "dept",
  id: number
): boolean {
  return newFormInline.value.recipients.some(
    r => r.scope_type === type && r.scope_id === id
  );
}

function toggle(
  type: "user" | "role" | "dept",
  id: number,
  name: string
) {
  const arr = newFormInline.value.recipients;
  const idx = arr.findIndex(
    r => r.scope_type === type && r.scope_id === id
  );
  if (idx >= 0) arr.splice(idx, 1);
  else arr.push({ scope_type: type, scope_id: id, name });
}

function pickAll(
  items: { id: number; name: string }[],
  type: "user" | "role" | "dept"
) {
  for (const i of items) {
    if (!isChecked(type, i.id)) {
      newFormInline.value.recipients.push({
        scope_type: type,
        scope_id: i.id,
        name: i.name
      });
    }
  }
}

function removeOne(idx: number) {
  newFormInline.value.recipients.splice(idx, 1);
}
</script>

<template>
  <el-form
    ref="ruleFormRef"
    :model="newFormInline"
    :rules="formRules"
    label-width="100px"
  >
    <el-form-item label="标题" prop="title">
      <el-input v-model="newFormInline.title" />
    </el-form-item>
    <el-form-item label="正文" prop="body">
      <el-input
        v-model="newFormInline.body"
        type="textarea"
        :rows="4"
      />
    </el-form-item>
    <el-form-item label="发布时间" prop="publish_at">
      <el-date-picker
        v-model="newFormInline.publish_at"
        type="datetime"
        value-format="YYYY-MM-DDTHH:mm:ss[Z]"
      />
    </el-form-item>
    <el-form-item label="过期时间">
      <el-date-picker
        v-model="newFormInline.expire_at"
        type="datetime"
        value-format="YYYY-MM-DDTHH:mm:ss[Z]"
      />
    </el-form-item>
    <el-form-item label="选项">
      <el-checkbox v-model="newFormInline.pinned">置顶</el-checkbox>
      <el-checkbox v-model="newFormInline.popover">弹窗提示</el-checkbox>
    </el-form-item>
    <el-form-item label="接收人">
      <div class="w-full">
        <el-tag
          v-for="(r, idx) in newFormInline.recipients"
          :key="r.scope_type + '-' + r.scope_id"
          closable
          class="mr-1 mb-1"
          @close="removeOne(idx)"
        >
          {{ r.scope_type }}:{{ r.name ?? r.scope_id }}
        </el-tag>
        <el-tabs v-model="activeTab" class="mt-2">
          <el-tab-pane label="用户" name="user">
            <el-input
              v-model="keyword"
              placeholder="搜索"
              clearable
              class="mb-2"
            />
            <el-button
              size="small"
              @click="pickAll(filtered(scopes.users), 'user')"
            >
              添加全部
            </el-button>
            <div class="mt-2 max-h-48 overflow-auto">
              <el-checkbox
                v-for="u in filtered(scopes.users)"
                :key="'u' + u.id"
                :model-value="isChecked('user', u.id)"
                @change="toggle('user', u.id, u.name)"
              >
                {{ u.name }}
              </el-checkbox>
            </div>
          </el-tab-pane>
          <el-tab-pane label="角色" name="role">
            <el-input
              v-model="keyword"
              placeholder="搜索"
              clearable
              class="mb-2"
            />
            <el-button
              size="small"
              @click="pickAll(filtered(scopes.roles), 'role')"
            >
              添加全部
            </el-button>
            <div class="mt-2 max-h-48 overflow-auto">
              <el-checkbox
                v-for="r in filtered(scopes.roles)"
                :key="'r' + r.id"
                :model-value="isChecked('role', r.id)"
                @change="toggle('role', r.id, r.name)"
              >
                {{ r.name }}
              </el-checkbox>
            </div>
          </el-tab-pane>
          <el-tab-pane label="部门" name="dept">
            <el-input
              v-model="keyword"
              placeholder="搜索"
              clearable
              class="mb-2"
            />
            <el-button
              size="small"
              @click="pickAll(filtered(scopes.depts), 'dept')"
            >
              添加全部
            </el-button>
            <div class="mt-2 max-h-48 overflow-auto">
              <el-checkbox
                v-for="d in filtered(scopes.depts)"
                :key="'d' + d.id"
                :model-value="isChecked('dept', d.id)"
                @change="toggle('dept', d.id, d.name)"
              >
                {{ d.name }}
              </el-checkbox>
            </div>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-form-item>
  </el-form>
</template>
