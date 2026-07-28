import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

/**
 * 角色管理页 /#/system/role
 * 文案依据 frontend/src/views/system/role/ 与 locales/zh-CN.yaml：
 * - 新增按钮：新增角色；弹窗标题：新增角色 / 修改角色
 * - toast：已新增角色 X / 已修改角色 X / 已删除角色 X / 角色 X 的权限已更新
 */
export class RolePage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/role`);
    await this.page.getByRole('button', { name: '新增角色' }).waitFor({ timeout: 15000 });
  }

  /** 按角色名称搜索（搜索栏与弹窗共用占位符，搜索在弹窗打开前调用） */
  async searchByName(name: string): Promise<void> {
    await this.page.getByPlaceholder('请输入角色名称').fill(name);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    // 等加载圈消失，保证表格已刷新
    await this.page.locator('.el-table__row').first().waitFor({ timeout: 8000 }).catch(() => undefined);
  }

  rowByName(name: string) {
    return this.page.locator('.el-table__row').filter({ hasText: name });
  }

  async createRole(params: { name: string; code: string; description?: string }): Promise<string> {
    await this.goto();
    await this.page.getByRole('button', { name: '新增角色' }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '角色名称' });
    await dialog.getByPlaceholder('请输入角色名称').fill(params.name);
    await dialog.getByPlaceholder('请输入角色标识').fill(params.code);
    if (params.description) {
      await dialog.getByPlaceholder('请输入描述信息').fill(params.description);
    }
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('已新增角色');
  }

  async editRole(params: { name: string; newName: string }): Promise<string> {
    await this.goto();
    await this.searchByName(params.name);
    const row = this.rowByName(params.name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '修改', exact: true }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '角色名称' });
    await dialog.getByPlaceholder('请输入角色名称').fill(params.newName);
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('已修改角色');
  }

  async deleteRole(name: string): Promise<string> {
    await this.goto();
    await this.searchByName(name);
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '删除', exact: true }).click();
    // el-popconfirm 渲染在 body 末端；点主按钮确认
    const popPrimary = this.page
      .locator('.el-popconfirm')
      .locator('.el-popconfirm__action button.el-button--primary');
    await popPrimary.waitFor({ state: 'visible', timeout: 5000 });
    await popPrimary.click({ force: true });
    return this.waitToast('已删除角色');
  }

  /**
   * 打开「权限」弹窗并勾选一个权限后保存。
   * checkText 缺省时勾选第一个权限复选框。
   */
  async assignPermissions(params: { name: string; checkText?: string }): Promise<string> {
    await this.goto();
    await this.searchByName(params.name);
    const row = this.rowByName(params.name).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '权限', exact: true }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '分配权限' });
    await dialog.waitFor({ timeout: 8000 });
    // 权限列表异步加载，等第一个复选框出现
    const firstBox = dialog.locator('.el-checkbox').first();
    await firstBox.waitFor({ timeout: 8000 });
    if (params.checkText) {
      await dialog.locator('.el-checkbox').filter({ hasText: params.checkText }).first().click();
    } else {
      await firstBox.click();
    }
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('权限已更新');
  }

  /**
   * 等包含 expected 的 toast 出现并等弹窗关闭；返回 toast 文案。
   * 必须等 beforeSure 全部落定再返回——App.vue 在 router.beforeEach 里 closeAllDialog()，
   * 后续 goto 若赶在异步保存完成前触发会清掉 dialogStore 使 done() 抛错。
   * expected 精确到动作前缀（已新增/已修改/已删除），避免串到上一步残留 toast。
   */
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

  /** 读取最近一条含「角色」的 el-message 文案（规避登录成功残留 toast） */
  async lastToastText(): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: '角色' }).last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
