<script setup lang="ts">
import { ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FormField } from "@/components/ui/form-field";
import { useAuthStore } from "../../store/auth";
import { onUnauthorized } from "../../api/client";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();

const username = ref("");
const password = ref("");
const submitting = ref(false);
const errorMessage = ref("");

async function onSubmit() {
  if (submitting.value) return;
  errorMessage.value = "";

  if (!username.value.trim() || !password.value) {
    errorMessage.value = t("shell.login.missingFields");
    return;
  }

  submitting.value = true;
  try {
    await auth.signIn(username.value.trim(), password.value);
    const redirect = (route.query.redirect as string | undefined) ?? "/";
    await router.replace(redirect);
  } catch (error) {
    const code = (error as { code?: string }).code;
    errorMessage.value = code ? code : t("shell.login.failed");
  } finally {
    submitting.value = false;
  }
}

onUnauthorized(() => {
  auth.reset();
});
</script>

<template>
  <main class="ui-login">
    <div class="ui-login__glow" aria-hidden="true" />

    <section class="ui-login__card">
      <header class="ui-login__brand">
        <span class="ui-login__mark" aria-hidden="true" />
        <h1 class="ui-login__title">{{ t("shell.login.title") }}</h1>
        <p class="ui-login__subtitle">{{ t("shell.login.subtitle") }}</p>
      </header>

      <form class="ui-login__form" novalidate @submit.prevent="onSubmit">
        <FormField
          :label="t('shell.login.username')"
          html-for="login-username"
          required
        >
          <Input
            id="login-username"
            v-model="username"
            type="text"
            :placeholder="t('shell.login.usernamePlaceholder')"
            autocomplete="username"
          />
        </FormField>

        <FormField
          :label="t('shell.login.password')"
          html-for="login-password"
          required
        >
          <Input
            id="login-password"
            v-model="password"
            type="password"
            :placeholder="t('shell.login.passwordPlaceholder')"
            autocomplete="current-password"
          />
        </FormField>

        <p v-if="errorMessage" class="ui-login__error" role="alert">
          {{ errorMessage }}
        </p>

        <Button type="submit" class="ui-login__submit" :loading="submitting">
          {{ t("shell.login.submit") }}
        </Button>
      </form>
    </section>
  </main>
</template>

<style scoped>
/* 登录页 — PRISM 品牌第一门面：暖灰纸面 + 一条钴蓝焦点 */
.ui-login {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: var(--space-6);
  background: var(--surface-base);
  overflow: hidden;
}

/* Hero 光晕：径向钴蓝 6%（Phase 1R §5.4 允许项） */
.ui-login__glow {
  position: absolute;
  top: -20%;
  left: 50%;
  width: 720px;
  height: 720px;
  transform: translateX(-50%);
  background: radial-gradient(circle, var(--accent-wash) 0%, transparent 70%);
  pointer-events: none;
}

.ui-login__card {
  position: relative;
  width: 100%;
  max-width: 400px;
  padding: var(--space-8);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xs);
}

.ui-login__brand {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-6);
  text-align: center;
}

/* 棱镜几何标记 */
.ui-login__mark {
  width: 36px;
  height: 36px;
  background: var(--accent-gradient);
  clip-path: polygon(50% 0%, 100% 100%, 0% 100%);
  border-radius: var(--radius-xs);
}

.ui-login__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h3);
  color: var(--text-primary);
}

.ui-login__subtitle {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  color: var(--text-tertiary);
}

.ui-login__form {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.ui-login__error {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--danger);
}

.ui-login__submit {
  width: 100%;
}

@media (max-width: 640px) {
  .ui-login {
    padding: var(--space-4);
  }

  .ui-login__card {
    padding: var(--space-6);
  }
}
</style>
