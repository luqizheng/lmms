<script setup lang="ts">
import { ref } from "vue";
import { formRules } from "./utils/rule";
import { FormProps } from "./utils/types";

const props = withDefaults(defineProps<FormProps>(), {
  formInline: () => ({
    isCreate: true,
    name: "",
    code: "",
    description: "",
    data_scope: 0,
    status: "active"
  })
});

const dataScopeOptions = [
  { value: 0, label: "仅本人" },
  { value: 1, label: "本部门" },
  { value: 2, label: "本部门及以下" },
  { value: 3, label: "全部" }
];

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
    label-width="82px"
  >
    <el-form-item label="角色名称" prop="name">
      <el-input
        v-model="newFormInline.name"
        clearable
        placeholder="请输入角色名称"
      />
    </el-form-item>

    <el-form-item label="角色标识" prop="code">
      <el-input
        v-model="newFormInline.code"
        clearable
        :disabled="!newFormInline.isCreate"
        placeholder="请输入角色标识"
      />
    </el-form-item>

    <el-form-item label="数据范围">
      <el-select
        v-model="newFormInline.data_scope"
        placeholder="请选择数据范围"
        class="w-full"
      >
        <el-option
          v-for="item in dataScopeOptions"
          :key="item.value"
          :label="item.label"
          :value="item.value"
        />
      </el-select>
    </el-form-item>

    <el-form-item label="状态">
      <el-radio-group v-model="newFormInline.status">
        <el-radio value="active">启用</el-radio>
        <el-radio value="disabled">停用</el-radio>
      </el-radio-group>
    </el-form-item>

    <el-form-item label="描述">
      <el-input
        v-model="newFormInline.description"
        placeholder="请输入描述信息"
        type="textarea"
      />
    </el-form-item>
  </el-form>
</template>
