import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class NotificationAdminPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/notification/admin`);
    await this.page.getByRole('button', { name: '新建公告' }).waitFor({ timeout: 15000 });
  }

  async createAnnouncement(params: {
    title: string;
    body?: string;
    recipients?: { type: 'user' | 'role' | 'dept'; pickAll?: boolean }[];
  }): Promise<void> {
    await this.goto();
    await this.page.getByRole('button', { name: '新建公告' }).click();

    const dialog = this.page.locator('.el-dialog').filter({ hasText: '新建公告' });

    // 通过 form-item label 定位标题输入框
    const titleFormItem = dialog.locator('.el-form-item').filter({ hasText: '标题' });
    await titleFormItem.locator('.el-input__inner').fill(params.title);

    if (params.body) {
      const bodyFormItem = dialog.locator('.el-form-item').filter({ hasText: '正文' });
      await bodyFormItem.locator('.el-textarea__inner').fill(params.body);
    }

    if (params.recipients && params.recipients.length > 0) {
      for (const r of params.recipients) {
        const tabLabel = r.type === 'user' ? '用户' : r.type === 'role' ? '角色' : '部门';
        await dialog.getByRole('tab', { name: tabLabel }).click();
        if (r.pickAll) {
          await dialog.getByRole('button', { name: '添加全部' }).click();
        }
      }
    }

    await dialog.getByRole('button', { name: '确定' }).click();
  }

  async lastToastText(): Promise<string> {
    // 等待登录成功 toast 消失（最多等待 5 秒）
    const loginToast = this.page.locator('.el-message').filter({ hasText: '登录成功' });
    try {
      await loginToast.waitFor({ state: 'hidden', timeout: 5000 });
    } catch {
      // 登录 toast 可能已经消失，忽略
    }

    // 等待新的 toast 出现
    const toast = this.page.locator('.el-message').last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }

  async getAnnouncementCount(): Promise<number> {
    await this.goto();
    const rows = await this.page.locator('.el-table__body-wrapper .el-table__row').count();
    return rows;
  }

  async publishFirstAnnouncement(): Promise<void> {
    await this.goto();
    const firstRow = this.page.locator('.el-table__body-wrapper .el-table__row').first();
    await firstRow.getByRole('button', { name: '发布' }).click();
  }

  async revokeFirstAnnouncement(): Promise<void> {
    await this.goto();
    const firstRow = this.page.locator('.el-table__body-wrapper .el-table__row').first();
    await firstRow.getByRole('button', { name: '撤销' }).click();
    await this.page.getByRole('button', { name: '确定' }).click();
  }
}