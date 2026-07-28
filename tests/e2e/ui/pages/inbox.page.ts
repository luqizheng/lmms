import { expect, type Page } from '@playwright/test';
import { config } from '../../shared/config';
import { throttlePublish } from '../flows/throttle';

// 公告收件箱 /#/notification/inbox
export class InboxPage {
  constructor(private page: Page) {}

  // 前置数据：在公告管理页创建并发布一条公告。
  // 复刻自实现（不复用 notification.page.ts）避免收件箱测试受 admin page 变动影响
  async createAndPublishAnnouncement(params: { title: string; body: string }): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/notification/admin`);
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
      .fill(params.body);
    // 接收人必填：默认用户 tab，远程搜索并选中当前登录用户。
    // 「搜索用户」是 span 占位符，真实输入框是 .el-select__input（点开才可输入）
    await dialog.locator('.re-user-selector .el-select__wrapper').click();
    await dialog.locator('.re-user-selector .el-select__input').fill(config.admin.user);
    // 下拉选项文案是昵称（users/simple 只返回 nickname），按昵称匹配
    const option = this.page
      .locator('.el-select-dropdown__item')
      .filter({ hasText: config.admin.nickname })
      .first();
    await option.waitFor({ timeout: 15000 });
    await option.click();
    await throttlePublish();
    await dialog.getByRole('button', { name: '发布' }).click();
    // 发布路径依赖 RabbitMQ outbox，环境中 AMQP 抖动会偶发 503；
    // 失败时对话框保持打开，等 3s 重试点「发布」（最多 2 次）
    let toastText = await this.readPublishToast();
    for (let i = 0; i < 2 && !toastText.includes('已发布'); i++) {
      await this.page.waitForTimeout(3000);
      await throttlePublish();
      await dialog.getByRole('button', { name: '发布' }).click();
      toastText = await this.readPublishToast();
    }
    if (!toastText.includes('已发布')) {
      throw new Error(`公告发布失败：${toastText}`);
    }
  }

  private async readPublishToast(): Promise<string> {
    const toast = this.page.locator('.el-message').filter({ hasText: '发布' }).last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/notification/inbox`);
    // hash 路由同文档切换不触发 reload，必须等本页独有元素出现防残留 DOM：
    // 「按标题筛选」与「搜索」按钮在公告管理页同样存在，只有 tab 是收件箱独有
    await this.page
      .locator('.el-tabs__item')
      .filter({ hasText: '未读' })
      .waitFor({ timeout: 15000 });
  }

  // 按标题筛选（placeholder=按标题筛选，客户端过滤）。
  // 标题带唯一后缀，过滤后列表只剩目标公告：断言 tab 显示「未读 (1)」。
  async searchTitle(title: string): Promise<string> {
    await this.page.getByPlaceholder('按标题筛选').fill(title);
    await Promise.all([
      this.page.waitForResponse(
        (r) => r.url().includes('/v1/notification/announcements') && r.request().method() === 'GET',
        { timeout: 15000 },
      ),
      this.page.getByRole('button', { name: '搜索' }).click(),
    ]);
    const unreadTab = this.page.locator('.el-tabs__item').filter({ hasText: '未读' });
    await expect(unreadTab).toContainText('(1)', { timeout: 15000 });
    return (await unreadTab.textContent()) ?? '';
  }

  // 点开过滤后按标题匹配的首行 → 右侧抽屉详情；进入即标记已读（openDrawer 调 markRead）
  async openDetailDrawer(title: string): Promise<string> {
    const row = this.page.locator('.el-table__row').filter({ hasText: title }).first();
    await row.waitFor({ timeout: 15000 });
    await row.click();
    const drawerTitle = this.page.locator('.el-drawer .drawer-title');
    await drawerTitle.waitFor({ timeout: 15000 });
    const text = (await drawerTitle.textContent()) ?? '';
    if (!text.includes(title)) {
      throw new Error(`抽屉标题不含 "${title}"，实际="${text}"`);
    }
    return text;
  }
}
