import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

/**
 * 在线会话页 /#/system/session（smoke 覆盖：打开页面 + 表格容器渲染）。
 * 前端已知缺陷：session/index.vue 未给 PureTableBar 传 :columns，
 * el-table 只渲染空行无单元格，故 smoke 断言标题「会话管理」与表格容器可见。
 */
export class SessionPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/session`);
    await this.page.getByRole('button', { name: '刷新' }).waitFor({ timeout: 15000 });
  }

  /** 等标题与表格容器渲染，返回标题文本（会话管理） */
  async waitLoaded(): Promise<string> {
    const title = this.page.getByText('会话管理').first();
    await title.waitFor({ timeout: 10000 });
    await this.page.locator('.el-table__body-wrapper').waitFor({ timeout: 10000 });
    return ((await title.textContent()) ?? '').trim();
  }
}
