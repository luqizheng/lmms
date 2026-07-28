import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

// /#/config/namespace 命名空间页（frontend/src/views/config/namespace/index.vue）
// 注意：该页 <PureTableBar> 未传 :columns，el-table 零列渲染（无表头/单元格/删除按钮），
// 因此列表行定位、编辑、删除均不可达——本 page object 只保留可用的新建能力。
export class ConfigNamespacePage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/config/namespace`);
    // 新建按钮 menus.configNamespaceCreate=新建
    await this.page
      .getByRole('button', { name: '新建', exact: true })
      .waitFor({ timeout: 15000 });
  }

  // 新建命名空间：dialog 填 App 名称 + el-select 选 env，成功 toast "已创建"
  async create(app: string, env = 'dev'): Promise<string> {
    await this.page.getByRole('button', { name: '新建', exact: true }).click();
    const dialog = this.page
      .locator('.el-dialog')
      .filter({ hasText: '新建命名空间' });
    await dialog.getByPlaceholder('App 名称').fill(app);
    // dialog 内唯一 el-select 为 env 字段（menus.configNamespaceSelectEnv=请选择）
    await dialog.locator('.el-select').click();
    await this.page
      .locator('.el-select-dropdown__item:visible')
      .filter({ hasText: env })
      .first()
      .click();
    const resp = this.page.waitForResponse(
      (r) =>
        r.url().includes('/v1/config/namespace') &&
        r.request().method() === 'POST',
    );
    await dialog.getByRole('button', { name: '确定', exact: true }).click();
    await resp;
    return this.lastToastText('已创建');
  }

  async lastToastText(filter: string): Promise<string> {
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: filter })
      .last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}

// /#/config/item 配置项页（frontend/src/views/config/item/index.vue）
// 注意：页面 loadNamespaces() 从未被调用（无 onMounted/watch），命名空间下拉恒为空；
// 表格同样未传 :columns 零列渲染。新建/编辑/删除/敏感值 reveal 均不可达，仅可 smoke。
export class ConfigItemPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/config/item`);
    // 新建按钮 menus.configItemCreate=新建
    await this.page
      .getByRole('button', { name: '新建', exact: true })
      .waitFor({ timeout: 15000 });
  }

  // 未选命名空间时点击新建，前端警告 toast "先选命名空间"（menus.configItemSelectNsFirst）
  async clickCreateToast(): Promise<string> {
    await this.page.getByRole('button', { name: '新建', exact: true }).click();
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: '先选命名空间' })
      .last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
