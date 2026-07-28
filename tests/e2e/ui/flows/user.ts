import { UserPage } from '../pages/user.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const userFlows: Record<string, Flow> = {
  createUser: async (page, params) => {
    await loginFirst(page);
    const up = new UserPage(page);
    await up.createUser({
      username: String(params.username),
      password: params.password ? String(params.password) : undefined,
    });
    return { toast: await up.lastToastText() };
  },
};
