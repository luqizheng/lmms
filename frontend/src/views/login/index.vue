<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { message } from "@/utils/message";
import { useNav } from "@/layout/hooks/useNav";
import { useEventListener } from "@vueuse/core";
import type { FormInstance } from "element-plus";
import { $t, transformI18n } from "@/plugins/i18n";
import { useUserStoreHook } from "@/store/modules/user";
import { getSetupStatus } from "@/api/auth";
import { ref, reactive, watch, onMounted } from "vue";
import { useTranslationLang } from "@/layout/hooks/useTranslationLang";
import { useDataThemeChange } from "@/layout/hooks/useDataThemeChange";

import dayIcon from "@/assets/svg/day.svg?component";
import darkIcon from "@/assets/svg/dark.svg?component";
import globalization from "@/assets/svg/globalization.svg?component";
import Check from "~icons/ep/check";
import Lock from "~icons/ri/lock-fill";
import User from "~icons/ri/user-3-fill";
import { avatar } from "./utils/static";

defineOptions({
  name: "Login"
});

const router = useRouter();
const loading = ref(false);
const disabled = ref(false);
const ruleFormRef = ref<FormInstance>();
const setupRequired = ref(false);

const { t } = useI18n();
const { dataTheme, themeMode, dataThemeChange } = useDataThemeChange();
dataThemeChange(themeMode.value);
const { title } = useNav();
const { locale, translationCh, translationEn } = useTranslationLang();

const ruleForm = reactive({
  username: "",
  password: ""
});

onMounted(async () => {
  try {
    const r = await getSetupStatus();
    setupRequired.value = r.data.setup_required;
    if (setupRequired.value) {
      router.push("/register");
    }
  } catch (_e) {
    // ignore; user can still try login
  }
});

const onLogin = async (formEl: FormInstance | undefined) => {
  if (!formEl) return;
  await formEl.validate(async valid => {
    if (!valid) return;
    loading.value = true;
    disabled.value = true;
    try {
      await useUserStoreHook().loginByUsername({
        username: ruleForm.username,
        password: ruleForm.password
      });
      // /users/me 可能暂不可用（业务路由尚未全量上线），失败不阻塞跳转
      try {
        await useUserStoreHook().setUserInfo();
      } catch (_e) {
        // ignore
      }
      message(t("login.pureLoginSuccess"), { type: "success" });
      router.push("/welcome");
    } catch (_err) {
      message(t("login.pureLoginFail"), { type: "error" });
    } finally {
      loading.value = false;
      disabled.value = false;
    }
  });
};

useEventListener(document, "keydown", ({ code }) => {
  if (
    ["Enter", "NumpadEnter"].includes(code) &&
    !disabled.value &&
    !loading.value
  ) {
    onLogin(ruleFormRef.value);
  }
});

watch(
  () => ({}),
  () => {},
  { immediate: false }
);
</script>

<template>
  <div class="select-none">
    <div class="flex-c absolute right-5 top-3">
      <el-switch
        v-model="dataTheme"
        inline-prompt
        :active-icon="dayIcon"
        :inactive-icon="darkIcon"
        @change="dataThemeChange"
      />
      <el-dropdown trigger="click">
        <globalization
          class="hover:text-primary hover:bg-transparent! size-5 ml-1.5 cursor-pointer outline-hidden duration-300"
        />
        <template #dropdown>
          <el-dropdown-menu class="translation">
            <el-dropdown-item
              :class="['dark:text-white!']"
              @click="translationCh"
            >
              <Check v-show="locale === 'zh'" class="check-zh" />
              简体中文
            </el-dropdown-item>
            <el-dropdown-item
              :class="['dark:text-white!']"
              @click="translationEn"
            >
              <Check v-show="locale === 'en'" class="check-en" />
              English
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
    <div class="login-container">
      <div class="login-form">
        <avatar class="avatar" />
        <h2 class="outline-hidden">{{ title }}</h2>

        <el-form
          ref="ruleFormRef"
          :model="ruleForm"
          size="large"
          @submit.prevent
        >
          <el-form-item
            :rules="[
              {
                required: true,
                message: transformI18n($t('login.pureUsernameReg')),
                trigger: 'blur'
              }
            ]"
            prop="username"
          >
            <el-input
              v-model="ruleForm.username"
              clearable
              :placeholder="t('login.pureUsername')"
            >
              <template #prefix>
                <User />
              </template>
            </el-input>
          </el-form-item>

          <el-form-item
            :rules="[
              {
                required: true,
                message: transformI18n($t('login.purePasswordReg')),
                trigger: 'blur'
              }
            ]"
            prop="password"
          >
            <el-input
              v-model="ruleForm.password"
              clearable
              show-password
              :placeholder="t('login.purePassword')"
            >
              <template #prefix>
                <Lock />
              </template>
            </el-input>
          </el-form-item>

          <el-form-item>
            <el-button
              class="w-full"
              size="default"
              type="primary"
              :loading="loading"
              :disabled="disabled"
              @click="onLogin(ruleFormRef)"
            >
              {{ t("login.pureLogin") }}
            </el-button>
          </el-form-item>
        </el-form>
        <div v-if="setupRequired" class="mt-2 text-center">
          <router-link to="/register" class="text-primary">
            首次部署？注册管理员
          </router-link>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@import url("@/style/login.css");
</style>

<style lang="scss" scoped>
.translation {
  :deep(.el-dropdown-menu__item) {
    padding: 5px 40px;
  }

  .check-zh {
    position: absolute;
    left: 20px;
  }

  .check-en {
    position: absolute;
    left: 20px;
  }
}
</style>
