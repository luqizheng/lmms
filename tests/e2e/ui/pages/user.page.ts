import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class UserPage {
  constructor(private page: Page) {}

  // 进入用户管理页（hash 路由 /system/user）
  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/user`);
    await this.page.getByRole('button', { name: '新增用户' }).waitFor({ timeout: 15000 });
  }

  async createUser(params: { username: string; password?: string }): Promise<void> {
    await this.goto();
    await this.page.getByRole('button', { name: '新增用户' }).click();
    // 弹窗表单：限定在对话框范围内填写（避免与搜索栏同名输入冲突）
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '用户名称' });
    await dialog.getByPlaceholder('请输入用户名称').fill(params.username);
    await dialog.getByPlaceholder('请输入用户昵称').fill(params.username);
    await dialog.getByPlaceholder('请输入用户密码').fill(params.password ?? config.admin.pass);
    // ReDialog footer 确定按钮
    await dialog.getByRole('button', { name: '确定' }).click();
  }

  // 读取最近一条 element-plus 成功提示文案（排除登录残留提示）
  async lastToastText(): Promise<string> {
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: '用户' })
      .last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
