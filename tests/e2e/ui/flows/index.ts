import type { Page } from '@playwright/test';
import { LoginPage } from '../pages/login.page';
import { UserPage } from '../pages/user.page';

export interface FlowResult {
  toast?: string;
}

export type Flow = (page: Page, params: Record<string, unknown>) => Promise<FlowResult>;

export const flows: Record<string, Flow> = {
  createUser: async (page, params) => {
    await new LoginPage(page).login();
    const up = new UserPage(page);
    await up.createUser({ username: String(params.username) });
    return { toast: await up.lastToastText() };
  },
};
