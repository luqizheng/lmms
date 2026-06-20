# 修改密码下拉项与弹窗实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在导航栏用户下拉菜单中新增「修改密码」入口，点击后弹出 el-dialog 弹窗，提交后强制重新登录。

**Architecture:** 后端零改动（`POST /v1/iam/auth/password` 已存在）。前端新增独立弹窗组件 `ChangePasswordDialog.vue`，由 `useUserStore.changePassword` action 触发；store 内调用 `logOut()` 完成清 token + 跳登录页。

**Tech Stack:** Vue 3 + pure-admin + Element Plus + Pinia；Vitest + Playwright e2e；Rust 集成测试。

**Spec:** `docs/superpowers/specs/2026-06-21-change-password-dropdown-design.md`

---

## File Structure

**新增：**
- `frontend/src/views/account-settings/components/ChangePasswordDialog.vue` — 弹窗组件
- `services/iam-service/tests/change_password_test.rs` — Rust 集成测试
- `tests/e2e/ui/pages/change-password.page.ts` — E2E Page Object
- `tests/e2e/ui/cases/iam-change-password.yaml` — E2E 用例

**修改：**
- `frontend/src/api/iam.ts` — 新增 `changeIamPassword`
- `frontend/src/store/modules/user.ts` — 新增 `changePassword` action
- `frontend/src/layout/hooks/useNav.ts` — 暴露 `changePasswordRef` + `openChangePassword`
- `frontend/src/layout/components/lay-navbar/index.vue` — 新增下拉项 + 渲染弹窗
- `frontend/src/locales/zh-CN.yaml` — 新增 i18n 键
- `frontend/src/locales/en.yaml` — 新增 i18n 键
- `tests/e2e/ui/pages/login.page.ts` — 新增 `logout()` 方法
- `tests/e2e/ui/flows/index.ts` — 注册 `changeOwnPassword` flow

---

## Task 1: 新增 changeIamPassword API 封装

**Files:**
- Modify: `frontend/src/api/iam.ts:325-328`（文件末尾追加）

- [ ] **Step 1: 在 `frontend/src/api/iam.ts` 末尾追加**

```ts
// ============== 认证 ==============
export function changeIamPassword(data: {
  old_password: string;
  new_password: string;
}) {
  return http.request<void>("post", "/v1/iam/auth/password", { data });
}
```

- [ ] **Step 2: 提交**

```bash
git add frontend/src/api/iam.ts
git commit -m "feat(frontend): add changeIamPassword API"
```

---

## Task 2: 新增 useUserStore.changePassword action

**Files:**
- Modify: `frontend/src/store/modules/user.ts:1-15`（import）+ `frontend/src/store/modules/user.ts:107-122`（action 区）

- [ ] **Step 1: 在 import 区加入 `changeIamPassword`**

替换 `frontend/src/store/modules/user.ts:1-15` 的 import 块为：

```ts
import { defineStore } from "pinia";
import {
  type userType,
  store,
  router,
  resetRouter,
  routerArrays,
  storageLocal
} from "../utils";
import {
  login as loginApi,
  logout as logoutApi,
  refreshToken as refreshTokenApi,
  type LoginResult
} from "@/api/auth";
import { changeIamPassword } from "@/api/iam";
import { useMultiTagsStoreHook } from "./multiTags";
import { type DataInfo, setToken, removeToken, userKey } from "@/utils/auth";
```

- [ ] **Step 2: 在 `logOut` 之后新增 `changePassword` action**

在 `frontend/src/store/modules/user.ts` 的 `logOut` action 末尾（`router.push("/login");` 之后、`handRefreshToken` 之前）追加：

```ts
    /** 修改当前用户密码 */
    async changePassword(oldPassword: string, newPassword: string) {
      await changeIamPassword({
        old_password: oldPassword,
        new_password: newPassword
      });
    },
```

- [ ] **Step 3: 提交**

```bash
git add frontend/src/store/modules/user.ts
git commit -m "feat(frontend): add changePassword action to user store"
```

---

## Task 3: 新增 i18n 键

**Files:**
- Modify: `frontend/src/locales/zh-CN.yaml`
- Modify: `frontend/src/locales/en.yaml`

- [ ] **Step 1: 在 `frontend/src/locales/zh-CN.yaml` 的 `buttons:` 段下追加**

```yaml
  pureChangePassword: 修改密码
```

（注意：以 2 空格缩进，与同段下其他键对齐）

- [ ] **Step 2: 在 `frontend/src/locales/zh-CN.yaml` 末尾追加顶级 `changePassword` 段**

```yaml
changePassword:
  title: "修改密码"
  oldPassword: "旧密码"
  oldPasswordPlaceholder: "请输入旧密码"
  newPassword: "新密码"
  newPasswordPlaceholder: "请输入新密码（8-128 位）"
  confirmPassword: "确认密码"
  confirmPasswordPlaceholder: "请再次输入新密码"
  submit: "确定"
  cancel: "取消"
  passwordMismatch: "两次输入密码不一致"
  passwordSameAsOld: "新密码不能与旧密码相同"
  success: "修改成功，请重新登录"
  failure: "修改失败"
```

- [ ] **Step 3: 在 `frontend/src/locales/en.yaml` 的 `buttons:` 段下追加**

```yaml
  pureChangePassword: Change Password
```

- [ ] **Step 4: 在 `frontend/src/locales/en.yaml` 末尾追加顶级 `changePassword` 段**

```yaml
changePassword:
  title: "Change Password"
  oldPassword: "Current Password"
  oldPasswordPlaceholder: "Enter current password"
  newPassword: "New Password"
  newPasswordPlaceholder: "Enter new password (8-128 chars)"
  confirmPassword: "Confirm Password"
  confirmPasswordPlaceholder: "Re-enter new password"
  submit: "Submit"
  cancel: "Cancel"
  passwordMismatch: "Passwords do not match"
  passwordSameAsOld: "New password must differ from current"
  success: "Password changed, please log in again"
  failure: "Password change failed"
```

- [ ] **Step 5: 提交**

```bash
git add frontend/src/locales/zh-CN.yaml frontend/src/locales/en.yaml
git commit -m "feat(frontend): add changePassword i18n keys"
```

---

## Task 4: 创建 ChangePasswordDialog.vue 弹窗组件

**Files:**
- Create: `frontend/src/views/account-settings/components/ChangePasswordDialog.vue`

- [ ] **Step 1: 创建文件，写入完整组件**

```vue
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
          return cb(new Error(t("login.purePasswordRule") || "6-128 chars"));
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
      <el-button
        type="primary"
        :loading="submitting"
        @click="submitForm"
      >
        {{ t("changePassword.submit") }}
      </el-button>
    </template>
  </el-dialog>
</template>
```

- [ ] **Step 2: 提交**

```bash
git add frontend/src/views/account-settings/components/ChangePasswordDialog.vue
git commit -m "feat(frontend): add ChangePasswordDialog component"
```

---

## Task 5: 在 useNav.ts 暴露 changePasswordRef 与 openChangePassword

**Files:**
- Modify: `frontend/src/layout/hooks/useNav.ts:7-12`（import）+ `frontend/src/layout/hooks/useNav.ts:150-179`（return 段）

- [ ] **Step 1: 在 import 段加入 `ref` 与组件类型**

替换 `frontend/src/layout/hooks/useNav.ts:1-12` 为：

```ts
import { storeToRefs } from "pinia";
import { getConfig } from "@/config";
import { useRouter } from "vue-router";
import { emitter } from "@/utils/mitt";
import Avatar from "@/assets/user.jpg";
import { getTopMenu } from "@/router/utils";
import { useFullscreen } from "@vueuse/core";
import type { routeMetaType } from "../types";
import { transformI18n } from "@/plugins/i18n";
import { router, remainingPaths } from "@/router";
import { computed, ref, type CSSProperties } from "vue";
import { useAppStoreHook } from "@/store/modules/app";
import { useUserStoreHook } from "@/store/modules/user";
import { useGlobal, isAllEmpty } from "@pureadmin/utils";
import { useEpThemeStoreHook } from "@/store/modules/epTheme";
import { usePermissionStoreHook } from "@/store/modules/permission";
import ChangePasswordDialog from "@/views/account-settings/components/ChangePasswordDialog.vue";
import ExitFullscreen from "~icons/ri/fullscreen-exit-fill";
import Fullscreen from "~icons/ri/fullscreen-fill";
```

- [ ] **Step 2: 在 return 之前新增 changePasswordRef 与 openChangePassword**

在 `useNav` 函数内、`return { ... }` 之前（`function resolvePath` 之后、`return` 之前）追加：

```ts
  /** 修改密码弹窗 ref */
  const changePasswordRef =
    ref<InstanceType<typeof ChangePasswordDialog> | null>(null);
  function openChangePassword() {
    changePasswordRef.value?.open();
  }
```

- [ ] **Step 3: 在 return 对象中加入两个字段**

在 `frontend/src/layout/hooks/useNav.ts` 的 `return { ... }` 末尾追加：

```ts
    toAccountSettings,
    getDropdownItemStyle,
    getDropdownItemClass,
    changePasswordRef,
    openChangePassword
  };
```

- [ ] **Step 4: 提交**

```bash
git add frontend/src/layout/hooks/useNav.ts
git commit -m "feat(frontend): expose changePasswordRef from useNav"
```

---

## Task 6: 在 lay-navbar 加下拉项并渲染弹窗

**Files:**
- Modify: `frontend/src/layout/components/lay-navbar/index.vue:13-31`（import + destructure）+ `frontend/src/layout/components/lay-navbar/index.vue:101-127`（template）

- [ ] **Step 1: 加入 Lock 图标 import**

在 `frontend/src/layout/components/lay-navbar/index.vue:13-17` 区域追加：

```ts
import Lock from "~icons/ri/lock-password-line";
import ChangePasswordDialog from "@/views/account-settings/components/ChangePasswordDialog.vue";
```

- [ ] **Step 2: 从 useNav 解构 openChangePassword**

替换 `frontend/src/layout/components/lay-navbar/index.vue:18-31` 的 `useNav()` 解构为：

```ts
const {
  layout,
  device,
  logout,
  onPanel,
  pureApp,
  username,
  userAvatar,
  avatarsStyle,
  toggleSideBar,
  toAccountSettings,
  openChangePassword,
  getDropdownItemStyle,
  getDropdownItemClass
} = useNav();
```

- [ ] **Step 3: 在 dropdown menu 中插入新下拉项**

在 `frontend/src/layout/components/lay-navbar/index.vue` 第 109 行（`{{ t("buttons.pureAccountSettings") }}` 之后、`</el-dropdown-item>` 之后，下一个 `<el-dropdown-item @click="logout">` 之前）插入：

```vue
            <el-dropdown-item @click="openChangePassword">
              <IconifyIconOffline :icon="Lock" style="margin: 5px" />
              {{ t("buttons.pureChangePassword") }}
            </el-dropdown-item>
```

- [ ] **Step 4: 在模板底部渲染弹窗**

在 `frontend/src/layout/components/lay-navbar/index.vue` 倒数第 2 个 `<span ... @click="onPanel">` 之前，插入：

```vue
      <ChangePasswordDialog ref="useNav().changePasswordRef" />
```

实际写法（template 中不能直接调函数表达式，须先用 setup 暴露 ref）：

在 `<script setup>` 段加：

```ts
const { changePasswordRef } = useNav();
```

替换 `useNav()` 解构中的 `openChangePassword` 行不删，并补这一行。`getDropdownItemStyle` 解构保持原状。

然后在模板对应位置改为：

```vue
      <ChangePasswordDialog ref="changePasswordRef" />
```

- [ ] **Step 5: 提交**

```bash
git add frontend/src/layout/components/lay-navbar/index.vue
git commit -m "feat(frontend): wire change-password dropdown and dialog"
```

---

## Task 7: 扩展 LoginPage 加 logout 方法

**Files:**
- Modify: `tests/e2e/ui/pages/login.page.ts:1-15`

- [ ] **Step 1: 追加 logout 方法**

替换 `tests/e2e/ui/pages/login.page.ts` 文件为：

```ts
import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class LoginPage {
  constructor(private page: Page) {}

  async login(user = config.admin.user, pass = config.admin.pass): Promise<void> {
    await this.page.goto(config.frontendURL);
    // 账号 / 密码 占位符见 frontend locales：pureUsername=账号, purePassword=密码
    await this.page.getByPlaceholder('账号').fill(user);
    await this.page.getByPlaceholder('密码').fill(pass);
    await this.page.getByRole('button', { name: '登录', exact: true }).click();
    // 登录成功跳转首页 welcome
    await this.page.waitForURL(/welcome/, { timeout: 15000 });
  }

  /**
   * 退出当前登录态：点用户头像下拉 → 退出系统。
   * 等待跳回 /#/login。
   */
  async logout(): Promise<void> {
    await this.page.locator('.el-dropdown-link').click();
    await this.page.getByText('退出系统').click();
    await this.page.waitForURL(/\/login/, { timeout: 15000 });
  }
}
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/ui/pages/login.page.ts
git commit -m "test(e2e): add LoginPage.logout helper"
```

---

## Task 8: 新增 ChangePasswordPage Page Object

**Files:**
- Create: `tests/e2e/ui/pages/change-password.page.ts`

- [ ] **Step 1: 创建文件**

```ts
import type { Page } from '@playwright/test';

export class ChangePasswordPage {
  constructor(private page: Page) {}

  /** 点击用户头像下拉 → 修改密码 → 等待弹窗出现 */
  async openDialog(): Promise<void> {
    await this.page.locator('.el-dropdown-link').click();
    await this.page.getByText('修改密码').click();
    await this.page
      .locator('.el-dialog')
      .filter({ hasText: '旧密码' })
      .waitFor({ timeout: 10000 });
  }

  /**
   * 在弹窗内填三个字段并点确定；等待跳转到 /login。
   * 若未跳转则抛错（被 expect.toast 抓住前先 fail）。
   */
  async submit(oldPassword: string, newPassword: string): Promise<void> {
    const dialog = this.page
      .locator('.el-dialog')
      .filter({ hasText: '旧密码' });
    await dialog.getByPlaceholder('请输入旧密码').fill(oldPassword);
    await dialog.getByPlaceholder('请输入新密码（8-128 位）').fill(newPassword);
    await dialog.getByPlaceholder('请再次输入新密码').fill(newPassword);
    await dialog.getByRole('button', { name: '确定' }).click();
    // 成功后强制 logout + 跳 /login
    await this.page.waitForURL(/\/login/, { timeout: 15000 });
  }

  /** 读取最近一条 el-message 文本 */
  async lastToastText(): Promise<string> {
    const toast = this.page.locator('.el-message').last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/ui/pages/change-password.page.ts
git commit -m "test(e2e): add ChangePasswordPage page object"
```

---

## Task 9: 注册 E2E flow

**Files:**
- Modify: `tests/e2e/ui/flows/index.ts:1-12`（import）+ `tests/e2e/ui/flows/index.ts:55-58`（flow 列表末尾）

- [ ] **Step 1: 在 import 区加入 ChangePasswordPage**

替换 `tests/e2e/ui/flows/index.ts:1-12` 为：

```ts
import type { Page } from '@playwright/test';
import { LoginPage } from '../pages/login.page';
import { UserPage } from '../pages/user.page';
import { TokenClientPage } from '../pages/token.page';
import { ChangePasswordPage } from '../pages/change-password.page';

export interface FlowResult {
  toast?: string;
  dialog?: string;
}

export type Flow = (page: Page, params: Record<string, unknown>) => Promise<FlowResult>;

async function loginFirst(page: Page) {
  await new LoginPage(page).login();
}
```

- [ ] **Step 2: 在 flows 对象末尾追加 changeOwnPassword**

在 `tests/e2e/ui/flows/index.ts` 的 `flows` 对象最后一个 flow 之后追加：

```ts
  changeOwnPassword: async (page, params) => {
    const username = String(params.username);
    const oldPassword = String(params.oldPassword);
    const newPassword = String(params.newPassword);

    // 1. 以 admin 登录建新用户
    const login = new LoginPage(page);
    await login.login();
    await new UserPage(page).createUser({ username, password: oldPassword });

    // 2. 退出 admin
    await login.logout();

    // 3. 以新用户登录
    await login.login(username, oldPassword);

    // 4. 打开修改密码弹窗并提交
    const cpp = new ChangePasswordPage(page);
    await cpp.openDialog();
    await cpp.submit(oldPassword, newPassword);

    return { toast: await cpp.lastToastText() };
  },
```

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/ui/flows/index.ts
git commit -m "test(e2e): register changeOwnPassword flow"
```

---

## Task 10: 新增 E2E 用例 YAML

**Files:**
- Create: `tests/e2e/ui/cases/iam-change-password.yaml`

- [ ] **Step 1: 创建文件**

```yaml
group: iam
cases:
  - caseId: UI-IAM-CHPW-001
    desc: 新用户登录后修改密码，强制跳回登录页
    flow: changeOwnPassword
    params:
      username: u${rand}
      oldPassword: ${adminPass}
      newPassword: NewP@ssw0rd!
    expect:
      - toast=修改成功
```

- [ ] **Step 2: 提交**

```bash
git add tests/e2e/ui/cases/iam-change-password.yaml
git commit -m "test(e2e): add iam change-password UI case"
```

---

## Task 11: 新增 Rust 集成测试

**Files:**
- Create: `services/iam-service/tests/change_password_test.rs`

- [ ] **Step 1: 创建文件**

```rust
//! 集成测试：覆盖 /iam/auth/password 修改密码端点。
//! 由 `node scripts/run-tests-docker.js iam-service` 在容器内执行。

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use std::process::{Command, Stdio};

    /// 启动并登录一个新注册用户，返回 access_token。
    async fn register_and_login(base: &str, username: &str, password: &str) -> String {
        // 注册（仅当系统无用户时 register 才允许；此处用 IAM_REGISTER_ENABLED 或首次注册）
        let client = reqwest::Client::new();
        let _ = client
            .post(format!("{}/v1/iam/auth/register", base))
            .json(&json!({
                "username": username,
                "password": password,
                "nickname": username,
            }))
            .send()
            .await
            .expect("register http");
        // 登录
        let resp = client
            .post(format!("{}/v1/iam/auth/login", base))
            .json(&json!({
                "username": username,
                "password": password,
            }))
            .send()
            .await
            .expect("login http");
        assert!(resp.status().is_success(), "login status {}", resp.status());
        let body: Value = resp.json().await.expect("login json");
        body["access_token"]
            .as_str()
            .expect("access_token missing")
            .to_string()
    }

    /// 旧密码错误 → 401
    #[tokio::test]
    async fn change_password_wrong_old_returns_401() {
        let base = std::env::var("E2E_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let client = reqwest::Client::new();
        let token = register_and_login(&base, "chpw_wrong", "OldP@ssw0rd!").await;
        let resp = client
            .post(format!("{}/v1/iam/auth/password", base))
            .bearer_auth(&token)
            .json(&json!({
                "old_password": "WrongPassword",
                "new_password": "NewP@ssw0rd!"
            }))
            .send()
            .await
            .expect("change http");
        assert_eq!(resp.status().as_u16(), 401);
    }

    /// 未登录访问 → 401
    #[tokio::test]
    async fn change_password_without_auth_returns_401() {
        let base = std::env::var("E2E_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/v1/iam/auth/password", base))
            .json(&json!({
                "old_password": "x",
                "new_password": "NewP@ssw0rd!"
            }))
            .send()
            .await
            .expect("change http");
        assert_eq!(resp.status().as_u16(), 401);
    }

    /// 新密码长度 < 8 → 422
    #[tokio::test]
    async fn change_password_short_new_returns_422() {
        let base = std::env::var("E2E_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());
        let client = reqwest::Client::new();
        let token = register_and_login(&base, "chpw_short", "OldP@ssw0rd!").await;
        let resp = client
            .post(format!("{}/v1/iam/auth/password", base))
            .bearer_auth(&token)
            .json(&json!({
                "old_password": "OldP@ssw0rd!",
                "new_password": "short"
            }))
            .send()
            .await
            .expect("change http");
        assert_eq!(resp.status().as_u16(), 422);
    }

    // 占位以保留 Command / Stdio 引用（项目其他 test 常用）
    #[allow(dead_code)]
    fn _unused() {
        let _ = Command::new("true").stdout(Stdio::null()).status();
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add services/iam-service/tests/change_password_test.rs
git commit -m "test(iam-service): add change_password integration test"
```

---

## Task 12: 跑通 E2E 验证

**Files:** 无（只跑命令）

- [ ] **Step 1: 启动 infrastructure**

```powershell
cd D:\projects\coder-micro-services\infrastructure
docker compose up -d
```

- [ ] **Step 2: 跑新用例**

```powershell
$env:E2E_DEBUG = "1"
pnpm -C tests/e2e test:ui
```

预期：UI-IAM-CHPW-001 PASS；toast 含"修改成功"；URL 落到 `/#/login`。

- [ ] **Step 3: 跑 Rust 集成测试**

需在真实 PowerShell（vcvarsall x64 已加载）执行：

```powershell
cd D:\projects\coder-micro-services
node scripts/run-tests-docker.js iam-service
```

预期：`change_password_test::tests::*` 三个用例全过。

- [ ] **Step 4: 手工自检**

按 spec 第 8.3 节清单逐项过一遍：弹窗动画、三字段提示、旧密码错、成功跳转、中英切换、明密文切换、关闭后清空。

---

## Self-Review

**1. Spec 覆盖**
- §3 架构：Task 1/2/4/5/6 ✓
- §4 组件细节：Task 1/2/4/5/6 ✓
- §5 错误处理：Task 4（catch + ElMessage.error）✓
- §6 i18n：Task 3 ✓
- §7 安全细节：Task 4（show-password / autocomplete / resetFields）✓
- §8.1 后端测试：Task 11 ✓
- §8.2 E2E：Task 7/8/9/10/12 ✓
- §8.3 手工清单：Task 12 Step 4 ✓

**2. 占位符扫描**
- 无 TBD / TODO
- 无 "implement later"

**3. 类型一致**
- `changeIamPassword({ old_password, new_password })` ↔ Task 1 定义、Task 2 调用、Task 11 测试一致
- `useUserStore.changePassword(old, new)` ↔ Task 2 定义、Task 4 调用一致
- `changePasswordRef` ↔ Task 5 定义、Task 6 模板 ref 一致

**4. 范围合理**
- 单一子系统（前端弹窗 + 后端测试），无需拆分

---

## 执行选项

计划已保存至 `docs/superpowers/plans/2026-06-21-change-password-dropdown.md`。两种执行方式：

1. **Subagent 驱动（推荐）** — 每任务派一个全新子代理，任务间有 review
2. **内联执行** — 在当前会话中用 executing-plans 跑，批量执行带 checkpoint

请选择执行方式。
