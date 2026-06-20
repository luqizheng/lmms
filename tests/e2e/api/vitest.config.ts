import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globalSetup: ['./api/globalSetup.ts'],
    include: ['api/runner/**/*.e2e.test.ts'],
    testTimeout: 20000,
    hookTimeout: 30000,
  },
});
