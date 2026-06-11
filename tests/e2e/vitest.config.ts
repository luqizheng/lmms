import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globalSetup: ['./globalSetup.ts'],
    testTimeout: 15000,
    hookTimeout: 30000,
    reporters: process.env.E2E_DEBUG ? 'verbose' : 'default',
  },
});
