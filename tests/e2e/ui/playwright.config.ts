import './dev/env';
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './runner',
  globalSetup: './globalSetup.ts',
  timeout: 120000,
  retries: process.env.CI ? 2 : 1,
  use: {
    baseURL: process.env.E2E_FRONTEND_URL ?? 'http://localhost:8080',
    screenshot: 'only-on-failure',
    headless: true,
    // 使用完整 chromium 构建（new headless），避免依赖 chromium-headless-shell
    channel: 'chromium',
  },
  reporter: [['list']],
});
