import { NotificationAdminPage } from '../pages/notification.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const notificationFlows: Record<string, Flow> = {
  createAnnouncement: async (page, params) => {
    await loginFirst(page);
    const np = new NotificationAdminPage(page);
    const toast = await np.createAnnouncement({
      title: String(params.title),
      body: params.body ? String(params.body) : undefined,
      recipients: params.recipients as { type: 'user' | 'role' | 'dept' }[] | undefined,
    });
    return { toast };
  },
};
