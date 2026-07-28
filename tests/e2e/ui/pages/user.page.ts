import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

export class UserPage {
  constructor(private page: Page) {}

  // 进入用户管理页（hash 路由 /system/user）
  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/system/user`);
    await this.page.getByRole('button', { name: '新增用户' }).waitFor({ timeout: 15000 });
  }

  async createUser(params: { username: string; password?: string }): Promise<void> {
    await this.goto();
    await this.page.getByRole('button', { name: '新增用户' }).click();
    // 弹窗表单：限定在对话框范围内填写（避免与搜索栏同名输入冲突）
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '用户名称' });
    await dialog.getByPlaceholder('请输入用户名称').fill(params.username);
    await dialog.getByPlaceholder('请输入用户昵称').fill(params.username);
    await dialog.getByPlaceholder('请输入用户密码').fill(params.password ?? config.admin.pass);
    // ReDialog footer 确定按钮
    await dialog.getByRole('button', { name: '确定' }).click();
    await this.waitToast('已新增用户');
  }

  // 等含 expected 的 toast 并等弹窗关闭（router.beforeEach 会 closeAllDialog，后续 goto 必须等保存落定；
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

  /** 按用户名称搜索（须在弹窗打开前调用，搜索栏与弹窗共用占位符） */
  async searchByUsername(username: string): Promise<void> {
    await this.page.getByPlaceholder('请输入用户名称').fill(username);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await this.page
      .locator('.el-table__row')
      .first()
      .waitFor({ timeout: 8000 })
      .catch(() => undefined);
  }

  rowByName(username: string) {
    return this.page.locator('.el-table__row').filter({ hasText: username });
  }

  /** 编辑用户昵称 → toast「已修改用户 X」 */
  async editUser(params: { username: string; newNickname: string }): Promise<string> {
    await this.goto();
    await this.searchByUsername(params.username);
    const row = this.rowByName(params.username).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '修改', exact: true }).click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '用户昵称' });
    await dialog.getByPlaceholder('请输入用户昵称').fill(params.newNickname);
    await dialog.getByRole('button', { name: '确定' }).click();
    return this.waitToast('已修改用户');
  }

  /** 删除用户 → 行内 el-popconfirm 确认 → toast「已删除用户 X」 */
  async deleteUser(username: string): Promise<string> {
    await this.goto();
    await this.searchByUsername(username);
    const row = this.rowByName(username).first();
    await row.waitFor({ timeout: 8000 });
    await row.getByRole('button', { name: '删除', exact: true }).click();
    // el-popconfirm 渲染在 body 末端；点主按钮确认
    const popPrimary = this.page
      .locator('.el-popconfirm')
      .locator('.el-popconfirm__action button.el-button--primary');
    await popPrimary.waitFor({ state: 'visible', timeout: 5000 });
    await popPrimary.click({ force: true });
    return this.waitToast('已删除用户');
  }

  /**
   * 给用户分配角色：行尾「更多」下拉（hover 触发）→ 分配角色 → 多选角色 → 确定。
   * toast「角色已更新」。
   */
  async assignRole(params: { username: string; roleName: string }): Promise<string> {
    await this.goto();
    await this.searchByUsername(params.username);
    const row = this.rowByName(params.username).first();
    await row.waitFor({ timeout: 8000 });
    // 行尾 More 图标按钮（el-dropdown 默认 hover 触发）
    await row.locator('.el-dropdown').first().hover();
    const menuItem = this.page
      .locator('.el-dropdown-menu')
      .getByRole('button', { name: '分配角色' });
    await menuItem.waitFor({ state: 'visible', timeout: 5000 });
    await menuItem.click();
    const dialog = this.page.locator('.el-dialog').filter({ hasText: '角色列表' });
    await dialog.waitFor({ timeout: 8000 });
    // 新版 el-select 占位符是 span 而非 input placeholder，直接点 select 容器展开
    await dialog.locator('.el-select').click();
    const option = this.page
      .locator('.el-select-dropdown__item')
      .filter({ hasText: params.roleName })
      .first();
    await option.waitFor({ timeout: 8000 });
    await option.click();
    await this.page.keyboard.press('Escape'); // 收起下拉
    await dialog.getByRole('button', { name: '确定' }).click();
    // 分配角色 toast 为「角色已更新」，不含「用户」，需单独等
    const toast = this.page.locator('.el-message').filter({ hasText: '角色已更新' }).last();
    await toast.waitFor({ timeout: 8000 });
    await dialog.waitFor({ state: 'hidden', timeout: 8000 }).catch(() => undefined);
    return (await toast.textContent()) ?? '';
  }

  // 读取最近一条 element-plus 成功提示文案（排除登录残留提示）
  async lastToastText(): Promise<string> {
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: '用户' })
      .last();
    await toast.waitFor({ timeout: 8000 });
    return (await toast.textContent()) ?? '';
  }
}
