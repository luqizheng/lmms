import { LoginPage } from '../pages/login.page';
import { config } from '../../shared/config';
import type { Flow } from './types';

// 登录页自身的用例：不能使用 loginFirst（那是其它域的前置助手），直接操作登录页。
export const authFlows: Record<string, Flow> = {
  // 错误密码 → 捕获错误提示（login.pureLoginFail=登录失败，见 views/login/index.vue）
  loginBadPassword: async (page, params) => {
    await page.goto(config.frontendURL);
    await page.getByPlaceholder('账号').fill(String(params.username ?? config.admin.user));
    await page.getByPlaceholder('密码').fill(String(params.password));
    await page.getByRole('button', { name: '登录', exact: true }).click();
    const toast = page.locator('.el-message').filter({ hasText: '登录失败' }).last();
    await toast.waitFor({ timeout: 15000 });
    return { toast: (await toast.textContent()) ?? '' };
  },

  // 正确账号 → 跳转 /welcome/（LoginPage.login 内置 waitForURL），捕获成功 toast
  loginSuccess: async (page) => {
    await new LoginPage(page).login();
    const toast = page.locator('.el-message').filter({ hasText: '登录成功' }).last();
    await toast.waitFor({ timeout: 15000 });
    return { toast: (await toast.textContent()) ?? '' };
  },

  // 登录后登出 → 跳回 /login/，返回登录页登录按钮文本作为到达标记
  logoutFlow: async (page) => {
    const lp = new LoginPage(page);
    await lp.login();
    await lp.logout();
    const btn = page.getByRole('button', { name: '登录', exact: true });
    await btn.waitFor({ timeout: 15000 });
    return { toast: (await btn.textContent()) ?? '' };
  },
};
