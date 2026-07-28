import {
  MonitorLoginLogPage,
  MonitorOperationLogPage,
  MonitorRetentionPage,
} from '../pages/monitor.page';
import { UserPage } from '../pages/user.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const monitorFlows: Record<string, Flow> = {
  // 登录日志：loginFirst 后进入页面，断言列表非空并返回首行文本。
  // 注：页面无过滤 UI，且数据源 audit_login_logs 只收 demo 种子
  // （真实登录写 iam_login_logs），无法断言出现 e2eadmin 记录。
  loginLogList: async (page) => {
    await loginFirst(page);
    const mp = new MonitorLoginLogPage(page);
    await mp.goto();
    return { toast: await mp.firstRowText() };
  },

  // 操作日志：先用 UI 建用户，再进页面执行一次过滤查询。
  // 注：audit_operation_logs 只收 demo 种子（reset 后恒空），建用户不会出现在本页，
  // 因此断言「页面渲染 + 过滤查询无报错」，不要求行数。
  operationLogList: async (page, params) => {
    await loginFirst(page);
    await new UserPage(page).createUser({ username: String(params.username) });
    const mp = new MonitorOperationLogPage(page);
    await mp.goto();
    await mp.search({ resource: 'iam.user' });
    await mp.waitTableReady();
    return { toast: 'operation-log-queried' };
  },

  // 数据保留页 smoke：打开即渲染「保留策略」卡片
  retentionSmoke: async (page) => {
    await loginFirst(page);
    const mp = new MonitorRetentionPage(page);
    await mp.goto();
    return { toast: await mp.policyTitleText() };
  },
};
