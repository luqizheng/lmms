/**
 * ui/dev/run-flow.ts — 独立执行单个 flow，不触发 runner/globalSetup/DB 重置。
 *
 * 用途：开发期验证 Page Object 选择器与 flow 编排，多个会话可并发跑（互不清库）。
 * 注意：正式套件仍走 `pnpm -C tests/e2e test:ui`（runner 会在 beforeAll resetBusiness）。
 *
 * 用法：
 *   E2E_FRONTEND_URL=http://localhost:8848 npx tsx dev/run-flow.ts <flowName> ['<paramsJson>']
 * 示例：
 *   E2E_FRONTEND_URL=http://localhost:8848 npx tsx dev/run-flow.ts createUser '{"username":"u${rand}"}'
 */
import './env';
import { chromium } from '@playwright/test';
import { flows } from '../flows';
import { resolve } from '../../shared/data/cases';
import { config } from '../../shared/config';

const [, , flowName, paramsJson] = process.argv;
const registered = Object.keys(flows);
if (!flowName || !flows[flowName]) {
  console.error(`未知 flow: ${flowName ?? '(空)'}\n已注册:\n  ${registered.join('\n  ')}`);
  process.exit(1);
}
const params = paramsJson ? resolve(JSON.parse(paramsJson), {}) : {};

const browser = await chromium.launch({ headless: true, channel: 'chromium' });
const page = await browser.newPage({ baseURL: config.frontendURL });
page.setDefaultTimeout(15000);
try {
  const result = await flows[flowName](page, params);
  console.log(`FLOW_OK ${flowName}`, JSON.stringify(result));
} catch (e) {
  const shot = new URL('./last-error.png', import.meta.url).pathname;
  await page.screenshot({ path: shot, fullPage: true }).catch(() => undefined);
  console.error(`FLOW_FAIL ${flowName}: ${String(e)}\n截图: ${shot}`);
  process.exitCode = 1;
} finally {
  await browser.close();
}
