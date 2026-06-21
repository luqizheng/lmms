<script setup lang="ts">
import { ref, watch } from "vue";
import { formRules } from "./utils/rule";
import type { FormProps } from "./utils/types";
import ReScopeSelector from "@/components/ReScopeSelector";

const props = defineProps<FormProps>();

const ruleFormRef = ref();
const newFormInline = ref({
  ...props.formInline,
  recipients: [...props.formInline.recipients]
});

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
      <el-input v-model="newFormInline.body" type="textarea" :rows="4" />
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
      <ReScopeSelector
        v-model="newFormInline.recipients"
        placeholder="请选择接收人"
      />
    </el-form-item>
  </el-form>
</template>