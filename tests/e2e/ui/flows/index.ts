import type { Page } from '@playwright/test';
import { LoginPage } from '../pages/login.page';
import { UserPage } from '../pages/user.page';
import { TokenClientPage } from '../pages/token.page';

export interface FlowResult {
  toast?: string;
  dialog?: string;
}

export type Flow = (page: Page, params: Record<string, unknown>) => Promise<FlowResult>;

async function loginFirst(page: Page) {
  await new LoginPage(page).login();
}

export const flows: Record<string, Flow> = {
  createUser: async (page, params) => {
    await loginFirst(page);
    const up = new UserPage(page);
    await up.createUser({
      username: String(params.username),
      password: params.password ? String(params.password) : undefined,
    });
    return { toast: await up.lastToastText() };
  },

  createTokenClient: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    return await tp.createClient(String(params.name));
  },

  resetTokenClientSecret: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    const name = String(params.name);
    // 准备数据：先创建客户端（返回结果不向测试暴露）
    await tp.createClient(name);
    // 执行动作：重置 secret
    return await tp.resetSecret(name);
  },

  toggleTokenClient: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    const name = String(params.name);
    await tp.createClient(name);
    return await tp.toggleStatus(name);
  },

  deleteTokenClient: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    const name = String(params.name);
    await tp.createClient(name);
    return await tp.deleteClient(name);
  },
};
