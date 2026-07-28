import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export interface CreateResult {
  toast: string;
  dialog: string;
}

export class TokenClientPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/token-client`);
    await this.page
      .getByRole('button', { name: '新增客户端' })
      .waitFor({ timeout: 15000 });
  }

  private waitListResponse<T = any>(trigger: () => Promise<void>): Promise<T> {
    return Promise.all([
      this.page.waitForResponse(
        (r) => r.url().includes('/admin/oauth-clients') && r.request().method() === 'GET',
      ),
      trigger(),
    ]).then(async ([resp]) => (await resp.json()) as T);
  }

  async searchByName(name: string): Promise<void> {
    const input = this.page.getByPlaceholder('Client ID / 名称');
    await input.fill(name);
    await this.waitListResponse(() =>
      this.page.getByRole('button', { name: '搜索', exact: true }).click(),
    );
  }

  rowByName(name: string) {
    return this.page.locator('.el-table__row').filter({ hasText: name });
  }

  async closeReDialog(dialog: ReturnType<Page['locator']>) {
    await dialog.getByRole('button', { name: '取消' }).click().catch(async () => {
      await this.page.keyboard.press('Escape');
    });
    await this.page.waitForTimeout(300);
  }

  async createClient(name: string): Promise<CreateResult> {
    await this.goto();
    const createResp = this.page.waitForResponse(
      (r) => r.url().includes('/admin/oauth-clients') && r.request().method() === 'POST',
    );
    const listResp = this.page.waitForResponse(
      (r) => r.url().includes('/admin/oauth-clients') && r.request().method() === 'GET',
    );
    await this.page.getByRole('button', { name: '新增客户端' }).click();
    const formDialog = this.page
      .locator('.el-dialog')
      .filter({ hasText: '客户端名称' });
    await formDialog.getByPlaceholder('请输入客户端名称').fill(name);
    await formDialog.getByRole('button', { name: '确定' }).click();
    await listResp;
    await createResp;
    const toast = await this.lastToastText();
    const secretDialog = this.page
      .locator('.el-dialog')
      .filter({ hasText: 'Client Secret' });
    await secretDialog.waitFor({ timeout: 8000 });
    const dialog = (
      (await secretDialog.locator('.el-dialog__title').textContent()) ?? ''
    ).trim();
    await this.closeReDialog(secretDialog);
    return { toast, dialog };
  }

  async resetSecret(name: string): Promise<{ dialog: string }> {
    await this.goto();
    await this.searchByName(name);
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '重置 Secret' }).click();
    await this.page
      .locator('.el-message-box')
      .filter({ hasText: '重置' })
      .getByRole('button', { name: '确定' })
      .click();
    const secretDialog = this.page
      .locator('.el-dialog')
      .filter({ hasText: 'Client Secret' });
    await secretDialog.waitFor({ timeout: 8000 });
    const dialog = (
      (await secretDialog.locator('.el-dialog__title').textContent()) ?? ''
    ).trim();
    await this.closeReDialog(secretDialog);
    return { dialog };
  }

  async toggleStatus(name: string): Promise<{ toast: string }> {
    await this.goto();
    await this.searchByName(name);
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    const disableBtn = row.getByRole('button', { name: '禁用' });
    const enableBtn = row.getByRole('button', { name: '启用' });
    const putResp = this.page.waitForResponse(
      (r) => r.url().includes('/admin/oauth-clients/') && r.request().method() === 'PUT',
    );
    if (await disableBtn.count()) {
      await disableBtn.click();
    } else {
      await enableBtn.click();
    }
    await putResp;
    return { toast: await this.lastToastText() };
  }

  async deleteClient(name: string): Promise<{ toast: string }> {
    await this.goto();
    await this.searchByName(name);
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '删除' }).click();
    // el-popconfirm 渲染在 body 末端；按主按钮（type=primary）定位
    const popPrimary = this.page
      .locator('.el-popconfirm')
      .locator('.el-popconfirm__action button.el-button--primary');
    await popPrimary.waitFor({ state: 'visible', timeout: 5000 });
    await popPrimary.click({ force: true });
    // handleDelete 内部还会弹 ElMessageBox 二次确认
    const msgBox = this.page.locator('.el-message-box');
    await msgBox.waitFor({ state: 'visible', timeout: 5000 });
    const delResp = this.page.waitForResponse(
      (r) =>
        /\/admin\/oauth-clients\/[^/]+$/.test(r.url()) &&
        r.request().method() === 'DELETE',
    );
    await msgBox.getByRole('button', { name: '确定' }).click();
    await delResp;
    return { toast: await this.lastToastText() };
  }

  async lastToastText(filter?: string): Promise<string> {
    const sel = filter
      ? this.page.locator('.el-message').filter({ hasText: filter })
      : this.page.locator('.el-message');
    const toast = sel.last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
