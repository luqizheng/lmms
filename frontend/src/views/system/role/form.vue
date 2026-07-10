<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
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

const { t } = useI18n();

const dataScopeOptions = [
  { value: 0, label: t("menus.roleFormDataScopeSelf") },
  { value: 1, label: t("menus.roleFormDataScopeDept") },
  { value: 2, label: t("menus.roleFormDataScopeDeptAndBelow") },
  { value: 3, label: t("menus.roleFormDataScopeAll") }
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
    <el-form-item :label="t('menus.roleFormLabelName')" prop="name">
      <el-input
        v-model="newFormInline.name"
        clearable
        :placeholder="t('menus.roleFormPlaceholderName')"
      />
    </el-form-item>

    <el-form-item :label="t('menus.roleFormLabelCode')" prop="code">
      <el-input
        v-model="newFormInline.code"
        clearable
        :disabled="!newFormInline.isCreate"
        :placeholder="t('menus.roleFormPlaceholderCode')"
      />
    </el-form-item>

    <el-form-item :label="t('menus.roleFormLabelDataScope')">
      <el-select
        v-model="newFormInline.data_scope"
        :placeholder="t('menus.roleFormPlaceholderDataScope')"
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

    <el-form-item :label="t('menus.roleFormLabelStatus')">
      <el-radio-group v-model="newFormInline.status">
        <el-radio value="active">
          {{ t("menus.deptStatusActive") }}
        </el-radio>
        <el-radio value="disabled">
          {{ t("menus.tokenClientStatusDisabled") }}
        </el-radio>
      </el-radio-group>
    </el-form-item>

    <el-form-item :label="t('menus.roleFormLabelDescription')">
      <el-input
        v-model="newFormInline.description"
        :placeholder="t('menus.roleFormPlaceholderDescription')"
        type="textarea"
      />
    </el-form-item>
  </el-form>
</template>