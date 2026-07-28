import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

/**
 * 部门管理页 /#/system/dept
 * 文案依据 frontend/src/views/system/dept/ 与 locales/zh-CN.yaml：
 * - 新增按钮：新增部门；弹窗标题：新增部门 / 修改部门
 * - toast：您新增了部门名称为X的这条数据 / 您修改了… / 您删除了部门名称为X的这条数据
 */
export class DeptPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/dept`);
    await this.page.getByRole('button', { name: '新增部门' }).waitFor({ timeout: 15000 });
  }

  async searchByName(name: string): Promise<void> {
    await this.page.getByPlaceholder('请输入部门名称').fill(name);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await this.page.waitForTimeout(400); // filterTree 为前端过滤，轻微等待渲染
  }

  rowByName(name: string) {
    return this.page.locator('.el-table__row').filter({ hasText: name });
  }

  /**
   * 新增部门。parentName 提供时通过上级部门级联选择器挂到该节点下（覆盖树形创建）。
   */
  async createDept(params: { name: string; code: string; parentName?: string }): Promise<string> {
    await this.goto();
    await this.page.getByRole('button', { name: '新增部门' }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '部门名称' });
    await dialog.waitFor({ timeout: 8000 });
    if (params.parentName) {
      await dialog.getByPlaceholder('请选择上级部门').click();
      const node = this.page
        .locator('.el-cascader-node__label')
        .filter({ hasText: params.parentName })
        .first();
      await node.waitFor({ timeout: 8000 });
      await node.click();
      // 关闭级联面板
      await this.page.keyboard.press('Escape');
    }
    await dialog.getByPlaceholder('请输入部门名称').fill(params.name);
    await dialog.getByPlaceholder('请输入部门编码').fill(params.code);
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('您新增了部门');
  }

  async editDept(params: { name: string; newName: string }): Promise<string> {
    await this.goto();
    await this.searchByName(params.name);
    const row = this.rowByName(params.name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '修改', exact: true }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '部门名称' });
    await dialog.getByPlaceholder('请输入部门名称').fill(params.newName);
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('您修改了部门');
  }

  async deleteDept(name: string): Promise<string> {
    await this.goto();
    await this.searchByName(name);
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '删除', exact: true }).click();
    const popPrimary = this.page
      .locator('.el-popconfirm')
      .locator('.el-popconfirm__action button.el-button--primary');
    await popPrimary.waitFor({ state: 'visible', timeout: 5000 });
    await popPrimary.click({ force: true });
    return this.waitToast('您删除了部门');
  }

  // 等含 expected 的 toast 并等弹窗关闭（router.beforeEach 会 closeAllDialog，必须序列化；
  // expected 精确到动作前缀，避免串到上一步残留 toast）
  private async waitToast(expected: string): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: expected }).last();
    await toast.waitFor({ timeout: 8000 });
    await this.page
      .locator('.el-dialog')
      .first()
      .waitFor({ state: 'hidden', timeout: 8000 })
      .catch(() => undefined);
    return (await toast.textContent()) ?? '';
  }

  /** 读取最近一条含「部门」的 el-message 文案 */
  async lastToastText(): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: '部门' }).last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
