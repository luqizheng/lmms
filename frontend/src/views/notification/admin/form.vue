<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { formRules } from "./utils/rule";
import type { FormProps } from "./utils/types";
import ReScopeSelector from "@/components/ReScopeSelector";

const props = defineProps<FormProps>();
const { t } = useI18n();

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
defineExpose({ getRef, newFormInline });
</script>

<template>
  <el-form
    ref="ruleFormRef"
    :model="newFormInline"
    :rules="formRules"
    label-width="100px"
  >
    <el-form-item :label="t('menus.announcementTitle')" prop="title">
      <el-input v-model="newFormInline.title" />
    </el-form-item>
    <el-form-item :label="t('menus.announcementContent')" prop="body">
      <el-input v-model="newFormInline.body" type="textarea" :rows="4" />
    </el-form-item>
    <el-form-item :label="t('menus.announcementPublishTime')" prop="publish_at">
      <el-date-picker
        v-model="newFormInline.publish_at"
        type="datetime"
        value-format="YYYY-MM-DDTHH:mm:ss[Z]"
      />
    </el-form-item>
    <el-form-item :label="t('menus.announcementExpireTime')">
      <el-date-picker
        v-model="newFormInline.expire_at"
        type="datetime"
        value-format="YYYY-MM-DDTHH:mm:ss[Z]"
      />
    </el-form-item>
    <el-form-item :label="t('menus.announcement')">
      <el-checkbox v-model="newFormInline.pinned">
        {{ t("menus.announcementPinned") }}
      </el-checkbox>
      <el-checkbox v-model="newFormInline.popover">
        {{ t("menus.announcementPopover") }}
      </el-checkbox>
    </el-form-item>
    <el-form-item :label="t('menus.announcementRecipients')">
      <ReScopeSelector
        v-model="newFormInline.recipients"
        :placeholder="t('menus.notificationPlaceholderRecipients')"
      />
    </el-form-item>
  </el-form>
</template>
