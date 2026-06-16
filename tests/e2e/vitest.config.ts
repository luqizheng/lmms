import { defineConfig } from 'vitest/config';
import ProgressReporter from './reporters/progress.reporter';

export default defineConfig({
  test: {
    globalSetup: ['./globalSetup.ts'],
    setupFiles: ['./setupFile.ts'],
    testTimeout: 15000,
    hookTimeout: 30000,
    reporters: process.env.E2E_DEBUG 
      ? ['verbose', ProgressReporter] 
      : [ProgressReporter],
  },
});
