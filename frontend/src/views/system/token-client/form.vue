<script setup lang="ts">
import { ref } from "vue";
import { formRules, GRANT_TYPE_OPTIONS, STATUS_OPTIONS } from "./utils/rule";
import { FormProps } from "./utils/types";

const props = withDefaults(defineProps<FormProps>(), {
  formInline: () => ({
    client_id: "",
    name: "",
    permissions: [],
    grant_types: [],
    access_token_ttl_sec: 3600,
    refresh_token_ttl_sec: 2592000,
    status: "active"
  }),
  isEdit: false
});

const ruleFormRef = ref();
const newFormInline = ref(props.formInline);

function getRef() {
  return ruleFormRef.value;
}

defineExpose({ getRef });
</script>

<template>
  <el-form
    ref="ruleFormRef"
    :model="newFormInline"
    :rules="formRules"
    label-width="140px"
  >
    <el-form-item v-if="isEdit" label="Client ID">
      <el-input v-model="newFormInline.client_id" disabled />
    </el-form-item>

    <el-form-item label="客户端名称" prop="name">
      <el-input
        v-model="newFormInline.name"
        clearable
        placeholder="请输入客户端名称"
      />
    </el-form-item>

    <el-form-item label="权限 permissions">
      <el-select
        v-model="newFormInline.permissions"
        multiple
        filterable
        allow-create
        default-first-option
        placeholder="输入权限码后回车，多个回车追加"
        class="w-full"
      />
    </el-form-item>

    <el-form-item label="Grant Types">
      <el-select
        v-model="newFormInline.grant_types"
        multiple
        placeholder="选择支持的 grant_type"
        class="w-full"
      >
        <el-option
          v-for="o in GRANT_TYPE_OPTIONS"
          :key="o.value"
          :label="o.label"
          :value="o.value"
        />
      </el-select>
    </el-form-item>

    <el-form-item label="Access Token TTL(秒)" prop="access_token_ttl_sec">
      <el-input-number
        v-model="newFormInline.access_token_ttl_sec"
        :min="60"
        :max="86400 * 7"
        controls-position="right"
        class="w-full"
      />
    </el-form-item>

    <el-form-item label="Refresh Token TTL(秒)" prop="refresh_token_ttl_sec">
      <el-input-number
        v-model="newFormInline.refresh_token_ttl_sec"
        :min="60"
        :max="86400 * 90"
        controls-position="right"
        class="w-full"
      />
    </el-form-item>

    <el-form-item label="状态" prop="status">
      <el-select v-model="newFormInline.status" class="w-full">
        <el-option
          v-for="o in STATUS_OPTIONS"
          :key="o.value"
          :label="o.label"
          :value="o.value"
        />
      </el-select>
    </el-form-item>
  </el-form>
</template>
