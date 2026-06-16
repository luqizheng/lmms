<script setup lang="ts">
import { ref } from "vue";
import { formRules, CATEGORY_OPTIONS, STATUS_OPTIONS } from "./utils/rule";
import { PermissionFormProps } from "./utils/types";

const props = withDefaults(defineProps<PermissionFormProps>(), {
  formInline: () => ({
    code: "",
    name: "",
    description: "",
    category: 1,
    resource: "",
    action: "",
    sort_order: 0,
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
    label-width="120px"
  >
    <el-form-item label="权限码" prop="code">
      <el-input
        v-model="newFormInline.code"
        :disabled="isEdit"
        placeholder="如 user:read"
      />
    </el-form-item>
    <el-form-item label="名称" prop="name">
      <el-input v-model="newFormInline.name" placeholder="权限名称" />
    </el-form-item>
    <el-form-item label="类别" prop="category">
      <el-select v-model="newFormInline.category" class="w-full">
        <el-option
          v-for="o in CATEGORY_OPTIONS"
          :key="o.value"
          :label="o.label"
          :value="o.value"
        />
      </el-select>
    </el-form-item>
    <el-form-item label="资源">
      <el-input v-model="newFormInline.resource" placeholder="resource" />
    </el-form-item>
    <el-form-item label="动作">
      <el-input v-model="newFormInline.action" placeholder="action" />
    </el-form-item>
    <el-form-item label="排序">
      <el-input-number
        v-model="newFormInline.sort_order"
        :min="0"
        :max="9999"
        controls-position="right"
      />
    </el-form-item>
    <el-form-item label="描述">
      <el-input v-model="newFormInline.description" type="textarea" :rows="2" />
    </el-form-item>
    <el-form-item v-if="isEdit" label="状态">
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
