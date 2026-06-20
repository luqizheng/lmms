<script setup lang="ts">
import { ref, reactive } from "vue";
import { ElMessage } from "element-plus";
import { useTranslationLang } from "@/layout/hooks/useTranslationLang";
import { useUserStoreHook } from "@/store/modules/user";

const { t } = useTranslationLang();

const visible = ref(false);
const submitting = ref(false);
const passwordFormRef = ref();

const form = reactive({
  oldPassword: "",
  newPassword: "",
  confirmPassword: ""
});

const rules = {
  oldPassword: [
    {
      required: true,
      validator: (_: unknown, v: string, cb: (e?: Error) => void) => {
        if (!v || v.length < 6 || v.length > 128) {
          return cb(new Error("6-128"));
        }
        cb();
      },
      trigger: "blur"
    }
  ],
  newPassword: [
    {
      required: true,
      validator: (_: unknown, v: string, cb: (e?: Error) => void) => {
        if (!v || v.length < 8 || v.length > 128) {
          return cb(new Error(t("changePassword.newPasswordPlaceholder")));
        }
        if (v === form.oldPassword) {
          return cb(new Error(t("changePassword.passwordSameAsOld")));
        }
        cb();
      },
      trigger: "blur"
    }
  ],
  confirmPassword: [
    {
      required: true,
      validator: (_: unknown, v: string, cb: (e?: Error) => void) => {
        if (v !== form.newPassword) {
          return cb(new Error(t("changePassword.passwordMismatch")));
        }
        cb();
      },
      trigger: "blur"
    }
  ]
};

function open() {
  visible.value = true;
}

function close() {
  visible.value = false;
  passwordFormRef.value?.resetFields();
  form.oldPassword = "";
  form.newPassword = "";
  form.confirmPassword = "";
}

async function submitForm() {
  try {
    await passwordFormRef.value?.validate();
  } catch {
    return;
  }
  submitting.value = true;
  try {
    await useUserStoreHook().changePassword(form.oldPassword, form.newPassword);
    ElMessage.success(t("changePassword.success"));
    await useUserStoreHook().logOut();
    close();
  } catch (e: any) {
    ElMessage.error(e?.message ?? t("changePassword.failure"));
  } finally {
    submitting.value = false;
  }
}

defineExpose({ open, close });
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="t('changePassword.title')"
    width="480px"
    :close-on-click-modal="false"
    :show-close="!submitting"
    @close="close"
  >
    <el-form
      ref="passwordFormRef"
      :model="form"
      :rules="rules"
      label-width="100px"
      label-position="right"
      @submit.prevent
    >
      <el-form-item
        :label="t('changePassword.oldPassword')"
        prop="oldPassword"
      >
        <el-input
          v-model="form.oldPassword"
          type="password"
          show-password
          :placeholder="t('changePassword.oldPasswordPlaceholder')"
          autocomplete="current-password"
        />
      </el-form-item>
      <el-form-item
        :label="t('changePassword.newPassword')"
        prop="newPassword"
      >
        <el-input
          v-model="form.newPassword"
          type="password"
          show-password
          :placeholder="t('changePassword.newPasswordPlaceholder')"
          autocomplete="new-password"
        />
      </el-form-item>
      <el-form-item
        :label="t('changePassword.confirmPassword')"
        prop="confirmPassword"
      >
        <el-input
          v-model="form.confirmPassword"
          type="password"
          show-password
          :placeholder="t('changePassword.confirmPasswordPlaceholder')"
          autocomplete="new-password"
        />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button :disabled="submitting" @click="close">
        {{ t("changePassword.cancel") }}
      </el-button>
      <el-button type="primary" :loading="submitting" @click="submitForm">
        {{ t("changePassword.submit") }}
      </el-button>
    </template>
  </el-dialog>
</template>
