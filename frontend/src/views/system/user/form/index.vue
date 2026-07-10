<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import ReCol from "@/components/ReCol";
import { formRules } from "../utils/rule";
import { FormProps } from "../utils/types";
import { usePublicHooks } from "../../hooks";

const { t } = useI18n();
const props = withDefaults(defineProps<FormProps>(), {
  formInline: () => ({
    title: "新增",
    higherDeptOptions: [],
    parentId: 0,
    nickname: "",
    username: "",
    password: "",
    phone: "",
    email: "",
    sex: "",
    status: "active",
    remark: ""
  })
});

const sexOptions = computed(() => [
  { value: 0, label: t("menus.userFormSexMale") },
  { value: 1, label: t("menus.userFormSexFemale") }
]);
const ruleFormRef = ref();
const { switchStyle } = usePublicHooks();
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
    <el-row :gutter="30">
      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item
          :label="t('menus.userFormLabelNickname')"
          prop="nickname"
        >
          <el-input
            v-model="newFormInline.nickname"
            clearable
            :placeholder="t('menus.userFormPlaceholderNickname')"
          />
        </el-form-item>
      </re-col>
      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item
          :label="t('menus.userFormLabelUsername')"
          prop="username"
        >
          <el-input
            v-model="newFormInline.username"
            clearable
            :placeholder="t('menus.userFormPlaceholderUsername')"
          />
        </el-form-item>
      </re-col>

      <re-col
        v-if="newFormInline.title === '新增'"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item
          :label="t('menus.userFormLabelPassword')"
          prop="password"
        >
          <el-input
            v-model="newFormInline.password"
            clearable
            :placeholder="t('menus.userFormPlaceholderPassword')"
          />
        </el-form-item>
      </re-col>
      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.userFormLabelPhone')" prop="phone">
          <el-input
            v-model="newFormInline.phone"
            clearable
            :placeholder="t('menus.userFormPlaceholderPhone')"
          />
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.userFormLabelEmail')" prop="email">
          <el-input
            v-model="newFormInline.email"
            clearable
            :placeholder="t('menus.userFormPlaceholderEmail')"
          />
        </el-form-item>
      </re-col>
      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.userFormLabelSex')">
          <el-select
            v-model="newFormInline.sex"
            :placeholder="t('menus.userFormPlaceholderSex')"
            class="w-full"
            clearable
          >
            <el-option
              v-for="(item, index) in sexOptions"
              :key="index"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.userFormLabelDept')">
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
            :placeholder="t('menus.userFormPlaceholderDept')"
          >
            <template #default="{ node, data }">
              <span>{{ data.name }}</span>
              <span v-if="!node.isLeaf"> ({{ data.children.length }}) </span>
            </template>
          </el-cascader>
        </el-form-item>
      </re-col>
      <re-col
        v-if="newFormInline.title === '新增'"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.userFormLabelStatus')">
          <el-switch
            v-model="newFormInline.status"
            inline-prompt
            active-value="active"
            inactive-value="disabled"
            :active-text="t('menus.userFormStatusActive')"
            :inactive-text="t('menus.userFormStatusInactive')"
            :style="switchStyle"
          />
        </el-form-item>
      </re-col>

      <re-col>
        <el-form-item :label="t('menus.userFormLabelRemark')">
          <el-input
            v-model="newFormInline.remark"
            :placeholder="t('menus.userFormPlaceholderRemark')"
            type="textarea"
          />
        </el-form-item>
      </re-col>
    </el-row>
  </el-form>
</template>