import type { Page } from '@playwright/test';
import { config } from '../../shared/config';
import { loginFirst } from './helpers';
import type { Flow, FlowResult } from './types';

/**
 * smoke 域：只读页面渲染冒烟（打开页面 → 标志性选择器出现即算过）。
 * 统一返回 { toast: 'ok:<page-name>' } 固定标记，YAML 用 toast=ok:<page-name> 断言。
 *
 * 跳过项（路由不存在，见 frontend/src/router/modules/）：
 * - /pm/**      — 无 pm 路由模块、无 views/pm 目录
 * - /about      — views/about/index.vue 存在但无任何路由注册它
 */

async function expectVisible(page: Page, selector: string): Promise<void> {
  await page.locator(selector).first().waitFor({ state: 'visible' });
}

function smokeFlow(route: string, selector: string, pageName: string): Flow {
  return async (page): Promise<FlowResult> => {
    await loginFirst(page);
    await page.goto(`${config.frontendURL}/#${route}`);
    await expectVisible(page, selector);
    return { toast: `ok:${pageName}` };
  };
}

export const smokeFlows: Record<string, Flow> = {
  // 欢迎页：登录后默认落地页；标志 = 统计卡片 .line-card（welcome/index.vue）
  smokeWelcome: smokeFlow('/welcome', '.line-card', 'welcome'),
  // Dashboard：服务状态总览；标志 = 根容器 .dashboard（dashboard/index.vue）
  smokeDashboard: smokeFlow('/dashboard/index', '.dashboard', 'dashboard'),
  // 个人资料：只断言表单渲染，不提交不修改；标志 = el-form（Profile.vue）
  smokeAccountProfile: smokeFlow('/account-settings/profile', '.el-form', 'account-profile'),
  // 安全设置（安全日志表）：只断言表格容器渲染，不断言数据行（日志数据依赖审计服务）
  // 标志 = pure-table 渲染出的 .el-table（SecurityLog.vue）
  smokeAccountSecurity: smokeFlow('/account-settings/security', '.el-table', 'account-security'),
  // 引导页：标志 = 「打开引导页」按钮（guide/index.vue）
  smokeGuide: smokeFlow('/guide/index', 'button:has-text("打开引导页")', 'guide'),
};
