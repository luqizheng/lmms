import type { Locator, Page } from '@playwright/test';
import { config } from '../../shared/config';

export class ConfigNamespacePage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/config/namespace`);
    await this.page
      .getByRole('button', { name: '新建', exact: true })
      .waitFor({ timeout: 15000 });
  }

  async create(app: string, env = 'dev'): Promise<string> {
    await this.page.getByRole('button', { name: '新建', exact: true }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '新建命名空间' });
    await dialog.getByPlaceholder('App 名称').fill(app);
    await dialog.locator('.el-select').click();
    await this.page
      .locator('.el-select-dropdown__item:visible')
      .filter({ hasText: env })
      .first()
      .click();
    const resp = this.page.waitForResponse(
      (r) => r.url().includes('/v1/config/namespace') && r.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: '确定', exact: true }).click();
    await resp;
    return this.lastToastText('已创建');
  }

  async searchByApp(app: string): Promise<void> {
    const resp = this.page.waitForResponse(
      (r) => r.url().includes('/v1/config/namespace') && r.request().method() === 'GET',
    );
    await this.page.getByPlaceholder('按名称筛选').fill(app);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await resp;
    await this.rowByApp(app).first().waitFor({ timeout: 15000 });
  }

  rowByApp(app: string): Locator {
    return this.page.locator('.el-table__row').filter({ hasText: app });
  }

  // window.confirm 需 dialog handler；删除请求路径 DELETE /v1/config/namespace/{id}
  async deleteByApp(app: string): Promise<string> {
    this.page.once('dialog', (d) => {
      void d.accept();
    });
    const delResp = this.page.waitForResponse(
      (r) => /\/v1\/config\/namespace\/[^/]+$/.test(r.url()) && r.request().method() === 'DELETE',
    );
    await this.rowByApp(app).first().getByRole('button', { name: '删除', exact: true }).click();
    await delResp;
    return this.lastToastText('已删除');
  }

  async lastToastText(filter: string): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: filter }).last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}

// /#/config/item 配置项页
export class ConfigItemPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/config/item`);
    await this.page
      .getByRole('button', { name: '新建', exact: true })
      .waitFor({ timeout: 15000 });
    // 等命名空间下拉加载完成（页面 onMounted 自动 loadNamespaces）
    await this.page
      .waitForResponse(
        (r) => r.url().includes('/v1/config/namespace') && r.request().method() === 'GET',
        { timeout: 15000 },
      )
      .catch(() => undefined);
  }

  // 前端 onCreate 通过 window.prompt/confirm 输入 key/value/isSecret；
  // 必须在点击「新建」前挂 dialog handler，按 [key, value, isSecret] 顺序应答
  async create(params: { key: string; value: string; secret?: boolean }): Promise<string> {
    const answers: (string | boolean)[] = [params.key, params.value, params.secret ?? false];
    const handler = async (d: import('@playwright/test').Dialog) => {
      const a = answers.shift();
      if (a === undefined) return d.dismiss();
      if (typeof a === 'string') return d.accept(a);
      return a ? d.accept() : d.dismiss();
    };
    this.page.on('dialog', handler);
    try {
      const resp = this.page.waitForResponse(
        (r) => r.url().includes('/v1/config/item') && r.request().method() === 'POST',
      );
      await this.page.getByRole('button', { name: '新建', exact: true }).click();
      await resp;
    } finally {
      this.page.off('dialog', handler);
    }
    return this.lastToastText('已创建');
  }

  async searchByKey(key: string): Promise<void> {
    await this.page.getByPlaceholder('按 key 筛选').fill(key);
    const resp = this.page.waitForResponse(
      (r) => r.url().includes('/v1/config/item') && r.request().method() === 'GET',
    );
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await resp;
  }

  rowByKey(key: string): Locator {
    return this.page.locator('.el-table__row').filter({ hasText: key });
  }

  async deleteByKey(key: string): Promise<string> {
    this.page.once('dialog', (d) => {
      void d.accept();
    });
    const resp = this.page.waitForResponse(
      (r) => /\/v1\/config\/item\/[^/]+$/.test(r.url()) && r.request().method() === 'DELETE',
    );
    await this.rowByKey(key).first().getByRole('button', { name: '删除', exact: true }).click();
    await resp;
    return this.lastToastText('已删除');
  }

  async lastToastText(filter: string): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: filter }).last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}
