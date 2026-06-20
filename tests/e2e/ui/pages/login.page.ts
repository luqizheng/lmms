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
}
