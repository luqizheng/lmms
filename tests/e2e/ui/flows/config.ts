import { ConfigItemPage, ConfigNamespacePage } from '../pages/config.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const configFlows: Record<string, Flow> = {
  // 新建命名空间（默认 env=dev）；toast "已创建"
  createNamespace: async (page, params) => {
    await loginFirst(page);
    const ns = new ConfigNamespacePage(page);
    await ns.goto();
    const env = params.env ? String(params.env) : 'dev';
    return { toast: await ns.create(String(params.app), env) };
  },

  // 命名空间删除/编辑不可达（页面表格零列渲染，删除按钮不在 DOM）——见 pages/config.page.ts 注释，
  // 对应用例在 cases/config.yaml 中以 skip 记录。

  // 配置项页 smoke：页面可渲染；由于 loadNamespaces() 未被页面调用，
  // 命名空间下拉恒为空，点新建只会得到警告 toast "先选命名空间"。
  viewConfigItemPage: async (page) => {
    await loginFirst(page);
    const ip = new ConfigItemPage(page);
    await ip.goto();
    return { toast: await ip.clickCreateToast() };
  },
};
