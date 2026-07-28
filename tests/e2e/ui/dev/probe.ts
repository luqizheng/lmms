import './env';
import { chromium } from '@playwright/test';

const url = process.argv[2] ?? 'http://localhost:8848/';
const browser = await chromium.launch({ headless: true, channel: 'chromium' });
const page = await browser.newPage();
await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 }).catch((e) => console.error('goto err:', String(e)));
await page.waitForTimeout(3000);
console.log('URL:', page.url());
console.log('TITLE:', await page.title());
const body = await page.locator('body').innerText().catch(() => '(no body)');
console.log('BODY (first 500):', body.slice(0, 500));
const inputs = await page.locator('input').evaluateAll((els) => els.map((e) => ({ ph: e.placeholder, type: e.type })));
console.log('INPUTS:', JSON.stringify(inputs));
await page.screenshot({ path: '/tmp/probe.png', fullPage: true });
await browser.close();
