import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

/**
 * 登录日志页 /#/monitor/login-log。
 * 实测偏差（已核实后端）：
 * 1. 页面只有表格 + 导出按钮，没有过滤表单，无法按用户名过滤；
 * 2. 页面数据源 audit_login_logs 仅由 scripts/demo-data.js 种子写入，
 *    真实 UI 登录写入的是 iam_login_logs（iam-service），本页查询不到，
 *    因此「断言出现 e2eadmin 登录记录」在本环境不成立，只能断言列表非空。
 */
export class MonitorLoginLogPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/monitor/login-log`);
  }

  // 等待列表出现至少一行，返回首行（最新一条）整行文本；超时即列表为空
  async firstRowText(): Promise<string> {
    const row = this.page.locator('.el-table__row').first();
    await row.waitFor({ timeout: 15000 });
    return (await row.textContent()) ?? '';
  }
}

/** 操作日志页 /#/monitor/operation-log（有 user_id / action / resource 过滤表单） */
export class MonitorOperationLogPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/monitor/operation-log`);
    // hash 路由同文档切换不触发 reload，必须等本页独有的输入框出现，
    // 否则后续表格行断言会命中上一页的残留 DOM
    await this.page.getByPlaceholder('user_id').waitFor({ timeout: 15000 });
  }

  // 点击「搜索」按当前表单过滤；filters 键即输入框 placeholder（user_id/action/resource）
  async search(filters: { user_id?: string; action?: string; resource?: string }): Promise<void> {
    if (filters.user_id) await this.page.getByPlaceholder('user_id').fill(filters.user_id);
    if (filters.action) await this.page.getByPlaceholder('action').fill(filters.action);
    if (filters.resource) await this.page.getByPlaceholder('resource').fill(filters.resource);
    await this.page.getByRole('button', { name: '搜索' }).click();
  }

  // 等待列表出现至少一行，返回首行文本；超时即列表为空
  async waitFirstRow(): Promise<string> {
    const row = this.page.locator('.el-table__row').first();
    await row.waitFor({ timeout: 15000 });
    return (await row.textContent()) ?? '';
  }

  // 等待表格容器渲染完成（行数可为 0）。
  // 本页数据源 audit_operation_logs 仅 demo 种子写入，reset 后恒为空，
  // 因此重置环境下只能断言「页面渲染 + 查询无报错」，不能要求行数
  async waitTableReady(): Promise<void> {
    await this.page.locator('.el-table').first().waitFor({ state: 'visible', timeout: 15000 });
  }
}

/** 数据保留页 /#/monitor/retention（smoke：打开渲染） */
export class MonitorRetentionPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/monitor/retention`);
    // 保留策略卡片标题出现即视为渲染完成（menus.retentionPolicyTitle=保留策略）
    await this.page.getByText('保留策略').waitFor({ timeout: 15000 });
  }

  async policyTitleText(): Promise<string> {
    return (await this.page.getByText('保留策略').first().textContent()) ?? '';
  }
}
