<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import ReCol from "@/components/ReCol";
import { formRules } from "./utils/rule";
import { FormProps } from "./utils/types";

const { t } = useI18n();
const props = withDefaults(defineProps<FormProps>(), {
  formInline: () => ({
    higherDeptOptions: [],
    id: undefined,
    parentId: "",
    name: "",
    code: "",
    sortOrder: 0,
    status: "active"
  })
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
    label-width="92px"
  >
    <el-row :gutter="30">
      <re-col>
        <el-form-item :label="t('menus.deptFormLabelParent')">
          <el-cascader
            v-model="newFormInline.parentId"
            class="w-full"
            :options="newFormInline.higherDeptOptions"
            :props="{
              value: 'id',
              label: 'name',
              emitPath: false,
              checkStrictly: true
            }"
            clearable
            filterable
            :placeholder="t('menus.deptFormPlaceholderParent')"
          >
            <template #default="{ node, data }">
              <span>{{ data.name }}</span>
              <span v-if="!node.isLeaf">
                ({{ data.children?.length || 0 }})
              </span>
            </template>
          </el-cascader>
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.deptFormLabelName')" prop="name">
          <el-input
            v-model="newFormInline.name"
            clearable
            :placeholder="t('menus.deptFormPlaceholderName')"
          />
        </el-form-item>
      </re-col>
      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.deptFormLabelCode')" prop="code">
          <el-input
            v-model="newFormInline.code"
            clearable
            :placeholder="t('menus.deptFormPlaceholderCode')"
          />
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.deptFormLabelSort')">
          <el-input-number
            v-model="newFormInline.sortOrder"
            class="w-full!"
            :min="0"
            :max="9999"
            controls-position="right"
          />
        </el-form-item>
      </re-col>
      <re-col v-if="newFormInline.id" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.deptFormLabelStatus')">
          <el-select v-model="newFormInline.status" class="w-full">
            <el-option :label="t('menus.deptStatusActive')" value="active" />
            <el-option
              :label="t('menus.deptStatusInactive')"
              value="inactive"
            />
          </el-select>
        </el-form-item>
      </re-col>
    </el-row>
  </el-form>
</template>