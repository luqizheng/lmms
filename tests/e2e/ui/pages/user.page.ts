import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class UserPage {
  constructor(private page: Page) {}

  // 进入用户管理页（system/user）
  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/user/index`);
    await this.page.getByRole('button', { name: '新增用户' }).waitFor({ timeout: 15000 });
  }

  async createUser(params: { username: string }): Promise<void> {
    await this.goto();
    await this.page.getByRole('button', { name: '新增用户' }).click();
    // 弹窗表单：用户名称 输入框
    await this.page.getByPlaceholder('请输入用户名称').fill(params.username);
    await this.page.getByPlaceholder('请输入用户昵称').fill(params.username);
    // ReDialog footer 确定按钮
    await this.page.getByRole('button', { name: '确定' }).click();
  }

  // 读取 element-plus 成功提示文案
  async lastToastText(): Promise<string> {
    const toast = this.page.locator('.el-message').first();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
