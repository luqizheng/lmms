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
