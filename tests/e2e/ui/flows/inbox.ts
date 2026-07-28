import { InboxPage } from '../pages/inbox.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const inboxFlows: Record<string, Flow> = {
  // 前置：创建并发布公告（InboxPage.createAndPublishAnnouncement），
  // 再到收件箱按标题筛选，断言该公告出现在列表，返回行文本。
  inboxReceiveAnnouncement: async (page, params) => {
    await loginFirst(page);
    const title = String(params.title);
    const inbox = new InboxPage(page);
    await inbox.createAndPublishAnnouncement({
      title,
      body: params.body ? String(params.body) : `${title} 正文`,
    });
    await inbox.goto();
    return { toast: await inbox.searchTitle(title) };
  },

  // 点开收件箱行 → 右侧抽屉显示详情，进入即标记已读（openDrawer 调 markRead）。
  // 返回抽屉标题文本。
  inboxOpenDetail: async (page, params) => {
    await loginFirst(page);
    const title = String(params.title);
    const inbox = new InboxPage(page);
    await inbox.createAndPublishAnnouncement({
      title,
      body: params.body ? String(params.body) : `${title} 正文`,
    });
    await inbox.goto();
    await inbox.searchTitle(title);
    return { toast: await inbox.openDetailDrawer(title) };
  },
};
