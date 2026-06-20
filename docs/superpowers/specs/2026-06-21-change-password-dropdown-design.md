# 修改密码下拉项与弹窗设计

> 日期：2026-06-21
> 范围：纯前端（`frontend/src/**`）；后端零改动
> 状态：待用户审阅

---

## 1. 目标

在导航栏用户下拉菜单中新增「修改密码」入口，点击后弹出 `el-dialog` 形式的密码修改弹窗，提交成功后强制重新登录。

完成标志：
- 下拉菜单出现新条目「修改密码」（中英国际化）
- 弹窗可打开/关闭，三个字段可填写并校验
- 提交成功后跳转登录页，用新密码可重新登录
- 后端容器内 Rust 单元测试 + 前端 E2E 全部通过

---

## 2. 范围与非范围

**范围内**
- 前端 7 个文件改动
- 1 个 E2E 用例
- 1 个 Rust 集成测试

**不在范围**
- 改密码后强制踢出该用户所有设备
- 密码强度条 / 复杂度策略
- 找回密码 / 邮箱 / 手机验证流程
- 后端路由表改动（`iam.router.toml` 已有 `/v1/iam/auth/{rest}` 通配）

---

## 3. 架构与数据流

### 3.1 链路

```
用户点击下拉项「修改密码」
        ↓
ChangePasswordDialog.open()
        ↓ 提交表单（el-form validate）
useUserStore.changePassword(oldPassword, newPassword)
        ↓
changeIamPassword({ old_password, new_password })
        ↓
POST /v1/iam/auth/password
  (经 api-gateway 透传到 iam-service:8080/iam/auth/password)
        ↓ 200 OK
ElMessage.success("修改成功，请重新登录")
useUserStore.logOut()   // 清 token + 跳 /login
```

### 3.2 改动文件清单

| 文件 | 类型 | 作用 |
|---|---|---|
| `frontend/src/api/iam.ts` | 改 | 新增 `changeIamPassword` |
| `frontend/src/store/modules/user.ts` | 改 | 新增 `changePassword` action |
| `frontend/src/views/account-settings/components/ChangePasswordDialog.vue` | 新建 | 弹窗组件 |
| `frontend/src/layout/hooks/useNav.ts` | 改 | 暴露 `changePasswordRef` |
| `frontend/src/layout/components/lay-navbar/index.vue` | 改 | 新增下拉项 + 渲染弹窗 |
| `frontend/src/locales/zh-CN.yaml` | 改 | 新增 i18n 键 |
| `frontend/src/locales/en.yaml` | 改 | 新增 i18n 键 |
| `services/iam-service/tests/change_password_test.rs` | 新建 | 后端集成测试 |
| `tests/e2e/ui/cases/iam-change-password.yaml` | 新建 | 前端 UI E2E 用例（数据驱动） |
| `tests/e2e/ui/pages/change-password.page.ts` | 新建 | 弹窗 Page Object |
| `tests/e2e/ui/flows/index.ts` | 改 | 注册 `changePassword` flow |

### 3.3 关键决策

- **后端零改动**：`POST /v1/iam/auth/password` 已存在（`iam-service/src/routes/auth.rs:18`），DTO `ChangePasswordRequest` 已定义（`iam-service/src/dto/auth.rs:19`）
- **复用 logout**：使用现有 `useUserStore.logOut()`，自动清 token 并跳 `/login`
- **组件独立**：弹窗放在 `account-settings/components/` 与 `Profile.vue` 同级，便于未来复用

---

## 4. 组件细节

### 4.1 ChangePasswordDialog.vue

- **Props**：无
- **Exposes**：`open()`, `close()`
- **内部状态**：`passwordFormRef`、`form`（`{ oldPassword, newPassword, confirmPassword }`）、`submitting`

#### 表单验证（el-form rules）

| 字段 | 规则 |
|---|---|
| `oldPassword` | 必填，长度 6-128 |
| `newPassword` | 必填，长度 8-128，与 `oldPassword` 不同 |
| `confirmPassword` | 必填，自定义 validator：与 `newPassword` 一致 |

#### 提交流程

```ts
async function submitForm() {
  await passwordFormRef.value?.validate();
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
```

#### UI 元素

- `el-dialog` 标题 `t('changePassword.title')`，宽 480px
- 三个 `el-form-item`，`type="password"`，全部 `show-password`
- 底部"确定/取消"按钮（`确定` 按钮 `loading` 绑定 `submitting`）

### 4.2 useUserStore.changePassword action

```ts
async changePassword(oldPassword: string, newPassword: string) {
  await changeIamPassword({ old_password: oldPassword, new_password: newPassword });
}
```

### 4.3 changeIamPassword API

```ts
// frontend/src/api/iam.ts
export function changeIamPassword(data: { old_password: string; new_password: string }) {
  return http.request<void>("post", "/v1/iam/auth/password", { data });
}
```

### 4.4 lay-navbar/index.vue 改动

- 引入 `ChangePasswordDialog` 并渲染 `<ChangePasswordDialog ref="changePasswordRef" />`
- 在 `el-dropdown-menu` 中 `AccountSettings` 与 `LoginOut` 之间插入新项：
  ```vue
  <el-dropdown-item @click="openChangePassword">
    <IconifyIconOffline :icon="LockIcon" style="margin: 5px" />
    {{ t("buttons.pureChangePassword") }}
  </el-dropdown-item>
  ```
- `openChangePassword()` 调 `changePasswordRef.value?.open()`

### 4.5 useNav.ts 改动

```ts
const changePasswordRef = ref<InstanceType<typeof ChangePasswordDialog> | null>(null);
function openChangePassword() { changePasswordRef.value?.open(); }
// 导出
return { ..., changePasswordRef, openChangePassword };
```

---

## 5. 错误处理

| 场景 | 状态码 / 来源 | 前端处理 |
|---|---|---|
| 旧密码错误 | 401 `invalid credentials` | `ElMessage.error(e.message)`，弹窗不关 |
| 新密码长度 < 8 | 后端 validator 422 | 前端验证已拦截，理论上不触发 |
| 字段不一致 | 前端 validator | `el-form` 字段红字提示 |
| 网络/服务器错误 | 500 | `ElMessage.error("修改失败")`，弹窗不关 |
| 提交中网络断 | catch | 恢复按钮，提示"网络异常" |
| 修改成功但 logout 失败 | - | 仍提示成功（密码已改），不阻塞流程 |

`http.ts` 已统一处理业务码：响应 `code !== 0` 时抛错，`message` 来自响应体。弹窗在 catch 中读取 `error.message` 提示用户。

---

## 6. 国际化

### zh-CN.yaml 新增

为与现有命名风格保持一致，下拉项 key 加在 `buttons` 下，弹窗文案使用 `changePassword` 顶级键：

```yaml
buttons:
  pureChangePassword: 修改密码
changePassword:
  title: "修改密码"
  oldPassword: "旧密码"
  newPassword: "新密码"
  confirmPassword: "确认密码"
  submit: "确定"
  cancel: "取消"
  passwordMismatch: "两次输入密码不一致"
  passwordLengthError: "密码长度需 8-128 位"
  passwordSameAsOld: "新密码不能与旧密码相同"
  success: "修改成功，请重新登录"
  failure: "修改失败"
```

### en.yaml 同步提供英文。

---

## 7. 安全细节

- 三个输入框 `type="password"`，全部 `show-password` 可切换明文
- `autocomplete="new-password"`（浏览器防记忆）
- `close()` 时 `el-form.resetFields()` 重置整个表单
- 提交期间禁用按钮，防止重复点击

---

## 8. 测试

### 8.1 后端集成测试

文件：`services/iam-service/tests/change_password_test.rs`

| 用例 | 断言 |
|---|---|
| 登录后 PUT /iam/auth/password 旧密码正确 | 200；用新密码 POST /iam/auth/login 成功 |
| 旧密码错误 | 401 |
| 未登录访问 | 401 |
| 新密码长度 < 8 | 422（validator 拦截） |

跑测命令：`node scripts/run-tests-docker.js iam-service`

### 8.2 前端 UI E2E（数据驱动）

用例文件：`tests/e2e/ui/cases/iam-change-password.yaml`

```yaml
group: iam
cases:
  - caseId: UI-IAM-CHPW-001
    desc: 登录后修改密码，强制重新登录
    flow: changePassword
    params:
      oldPassword: ${adminPass}
      newPassword: NewP@ssw0rd!
    expect:
      - toast=修改成功，请重新登录
      - url=${frontendURL}/#/login
```

Flow（`tests/e2e/ui/flows/index.ts`）：

```ts
changePassword: async (page, params) => {
  await loginFirst(page);
  const cpp = new ChangePasswordPage(page);
  await cpp.openDialog();
  await cpp.submit(
    String(params.oldPassword),
    String(params.newPassword)
  );
  return { toast: await cpp.lastToastText() };
},
```

Page Object（`tests/e2e/ui/pages/change-password.page.ts`）负责：点击头像 → 点击下拉项「修改密码」→ 在弹窗内填写三个字段 → 点确定 → 等待 toast → 等待跳转 `/login`。

跑测命令：
```powershell
cd D:\projects\coder-micro-services\infrastructure
docker compose up -d
pnpm -C tests/e2e test:ui
```

### 8.3 手工验证清单（提交前自检）

- 弹窗打开/关闭动画正常
- 三个字段都为空时点提交：三条错误提示
- 旧密码错：弹窗不关，提示具体信息
- 修改成功：toast 提示 + 跳登录页
- 登录页用新密码可登入
- 切换中英文：下拉项与弹窗标题都国际化
- 三个输入框可切换明文 / 密文
- 关闭弹窗后再次打开：表单已清空
