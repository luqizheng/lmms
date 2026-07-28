import type { Locator, Page } from '@playwright/test';
import { config } from '../../shared/config';
import { throttlePublish } from '../flows/throttle';

type Recipient = { type: 'user' | 'role' | 'dept'; pickAll?: boolean };

// 公告管理页 /#/notification/admin（views/notification/admin）。
// 当前前端：「新建公告」对话框字段=公告标题/公告内容/接收人(必填≥1)，
// 底部「发布」按钮一步完成创建+发布（toast=已发布）；旧版「确定」+免接收人
// 的实现已随前端改版失效。接收人选择器为 ReUser/Role/OrgSelector，
// 「搜索xx」是 span 占位符，真实输入框是 .el-select__input（点开才可输入），
// 且用户下拉选项文案是昵称（users/simple 不返回 username）。
export class NotificationAdminPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/notification/admin`);
    await this.page.getByRole('button', { name: '新建公告' }).waitFor({ timeout: 15000 });
  }

  // 下拉选项按「可见」过滤：公告状态筛选器的 dropdown 也常驻 DOM（隐藏），
  // 不限定 visible 会误中它的 option
  private async pickUserRecipient(dialog: Locator): Promise<void> {
    await dialog.locator('.re-user-selector .el-select__wrapper').click();
    await dialog.locator('.re-user-selector .el-select__input').fill(config.admin.user);
    const option = this.page
      .locator('.el-select-dropdown__item:visible')
      .filter({ hasText: config.admin.nickname })
      .first();
    await option.waitFor({ timeout: 15000 });
    await option.click();
  }

  private async pickRoleRecipient(dialog: Locator): Promise<void> {
    await dialog.getByRole('tab', { name: '角色' }).click();
    await dialog.locator('.re-role-selector .el-select__wrapper').click();
    await dialog.locator('.re-role-selector .el-select__input').fill('admin');
    const option = this.page.locator('.el-select-dropdown__item:visible').first();
    await option.waitFor({ timeout: 15000 });
    await option.click();
  }

  private async pickDeptRecipient(dialog: Locator): Promise<void> {
    await dialog.getByRole('tab', { name: '部门' }).click();
    await dialog.locator('.re-org-selector .el-select__wrapper').click();
    const node = this.page.locator('.el-tree-node__label').first();
    await node.waitFor({ timeout: 15000 });
    await node.click();
  }

  // 创建（对话框「发布」一步创建+发布）；recipients 缺省时选当前登录用户。
  // 返回结果 toast 文本（成功=已发布），失败文案原样返回由断言判定。
  async createAnnouncement(params: {
    title: string;
    body?: string;
    recipients?: Recipient[];
  }): Promise<string> {
    await this.goto();
    await this.page.getByRole('button', { name: '新建公告' }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '新建公告' });
    await dialog
      .locator('.el-form-item')
      .filter({ hasText: '公告标题' })
      .locator('input')
      .fill(params.title);
    await dialog
      .locator('.el-form-item')
      .filter({ hasText: '公告内容' })
      .locator('textarea')
      .fill(params.body ?? params.title);

    const recipients = params.recipients?.length ? params.recipients : [{ type: 'user' as const }];
    for (const r of recipients) {
      if (r.type === 'role') await this.pickRoleRecipient(dialog);
      else if (r.type === 'dept') await this.pickDeptRecipient(dialog);
      else await this.pickUserRecipient(dialog);
    }

    await throttlePublish();
    await dialog.getByRole('button', { name: '发布' }).click();
    // 同 inbox.page.ts：AMQP 抖动导致发布偶发 5xx 时重试（对话框失败不关闭）
    let toast = await this.lastToastText('发布');
    for (let i = 0; i < 2 && toast.includes('失败'); i++) {
      await this.page.waitForTimeout(3000);
      await throttlePublish();
      await dialog.getByRole('button', { name: '发布' }).click();
      toast = await this.lastToastText('发布');
    }
    return toast;
  }

  private async searchByTitle(title: string): Promise<void> {
    await this.page.getByPlaceholder('按标题筛选').fill(title);
    const resp = this.page.waitForResponse(
      (r) =>
        r.url().includes('/v1/notification/announcements') && r.request().method() === 'GET',
    );
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await resp;
    await this.page
      .locator('.el-table__row')
      .filter({ hasText: title })
      .first()
      .waitFor({ timeout: 15000 });
  }

  async publishByTitle(title: string): Promise<string> {
    await this.goto();
    await this.searchByTitle(title);
    const row = this.page.locator('.el-table__row').filter({ hasText: title }).first();
    await throttlePublish();
    await row.getByRole('button', { name: '发布', exact: true }).click();
    return this.lastToastText('发布');
  }

  async revokeByTitle(title: string): Promise<string> {
    await this.goto();
    await this.searchByTitle(title);
    const row = this.page.locator('.el-table__row').filter({ hasText: title }).first();
    await row.getByRole('button', { name: '撤销', exact: true }).click();
    const box = this.page.locator('.el-message-box').filter({ hasText: '确认撤销' });
    await box.getByRole('button', { name: '确定' }).click();
    return this.lastToastText('撤销');
  }

  async lastToastText(filter: string): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: filter }).last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}
