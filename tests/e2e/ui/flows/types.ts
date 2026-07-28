import type { Page } from '@playwright/test';

export interface FlowResult {
  toast?: string;
  dialog?: string;
}

export type Flow = (page: Page, params: Record<string, unknown>) => Promise<FlowResult>;
