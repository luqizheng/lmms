import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

/**
 * 权限管理页 /#/system/permission
 * 搜索占位符：code / name；搜索接口关键字同时匹配 code 与 name。
 */
export class PermissionPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/permission`);
    await this.page.getByRole('button', { name: '新增权限' }).waitFor({ timeout: 15000 });
  }

  /**
   * 按关键字搜索并断言结果行中包含该关键字；返回首条命中行的文本。
   * 无命中时抛错（供 flow 失败）。
   */
  async searchAndExpectRow(keyword: string): Promise<string> {
    await this.goto();
    await this.page.getByPlaceholder('code / name').fill(keyword);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    const row = this.page.locator('.el-table__row').filter({ hasText: keyword }).first();
    await row.waitFor({ timeout: 8000 });
    return ((await row.textContent()) ?? '').trim();
  }
}
