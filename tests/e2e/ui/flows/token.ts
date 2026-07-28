import { TokenClientPage } from '../pages/token.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const tokenFlows: Record<string, Flow> = {
  createTokenClient: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    return await tp.createClient(String(params.name));
  },

  resetTokenClientSecret: async (page, params) => {
    await loginFirst(page);
    const tp = new TokenClientPage(page);
    const name = String(params.name);
    await tp.createClient(name);
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
