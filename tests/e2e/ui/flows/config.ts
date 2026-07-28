import { ConfigItemPage, ConfigNamespacePage } from '../pages/config.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const configFlows: Record<string, Flow> = {
  createNamespace: async (page, params) => {
    await loginFirst(page);
    const ns = new ConfigNamespacePage(page);
    await ns.goto();
    const env = params.env ? String(params.env) : 'dev';
    return { toast: await ns.create(String(params.app), env) };
  },

  deleteNamespace: async (page, params) => {
    await loginFirst(page);
    const ns = new ConfigNamespacePage(page);
    await ns.goto();
    const app = String(params.app);
    await ns.create(app);
    await ns.searchByApp(app);
    const toast = await ns.deleteByApp(app);
    return { toast };
  },

  viewConfigItemPage: async (page) => {
    await loginFirst(page);
    const ip = new ConfigItemPage(page);
    await ip.goto();
    return { toast: `config-item-page-open` };
  },

  createConfigItem: async (page, params) => {
    await loginFirst(page);
    // 前置：确保至少有一个 namespace（item 页 onMounted 会默认选中第一个）
    const ns = new ConfigNamespacePage(page);
    await ns.goto();
    await ns.create(`e2e-ns-${Date.now()}`);
    const ip = new ConfigItemPage(page);
    await ip.goto();
    return {
      toast: await ip.create({
        key: String(params.key),
        value: String(params.value ?? 'e2e-value'),
      }),
    };
  },
};
