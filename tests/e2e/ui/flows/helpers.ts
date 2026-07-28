import type { Page } from '@playwright/test';
import { LoginPage } from '../pages/login.page';

export async function loginFirst(page: Page): Promise<void> {
  await new LoginPage(page).login();
}
