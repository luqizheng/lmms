import type { Page } from '@playwright/test';
import { config } from '../../shared/config';

// /#/file/upload 上传页（frontend/src/views/file/upload.vue）
export class FileUploadPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/file/upload`);
    // 提交按钮文案 menus.fileUploadSubmit=提交上传
    await this.page
      .getByRole('button', { name: '提交上传' })
      .waitFor({ timeout: 15000 });
  }

  // 选择本地文件并提交；el-upload 的 input[type=file] 隐藏但可 setInputFiles。
  // 成功 toast 形如 "成功 1，失败 0"（menus.fileUploadResultSummary）。
  async submitFile(filePath: string): Promise<string> {
    await this.page.locator('input[type="file"]').setInputFiles(filePath);
    await this.page.getByRole('button', { name: '提交上传' }).click();
    return this.lastToastText('失败');
  }

  // 以内存 buffer + 自定义文件名上传（删除闭环用唯一文件名，保证可重复跑）
  async submitBuffer(name: string, buffer: Buffer): Promise<string> {
    await this.page.locator('input[type="file"]').setInputFiles({
      name,
      mimeType: 'text/plain',
      buffer,
    });
    await this.page.getByRole('button', { name: '提交上传' }).click();
    return this.lastToastText('失败');
  }

  // 按子串过滤捕获 toast（避开 "登录成功" 残留提示）
  async lastToastText(filter: string): Promise<string> {
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: filter })
      .last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}

// /#/file/index 文件列表页（frontend/src/views/file/index.vue）
export class FileListPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/file/index`);
    await this.page
      .getByRole('button', { name: '搜索', exact: true })
      .waitFor({ timeout: 15000 });
  }

  // 列表页打开不自动加载，必须按文件名搜索触发 GET /v1/file/list
  async search(name: string): Promise<void> {
    const resp = this.page.waitForResponse(
      (r) => r.url().includes('/v1/file/list') && r.request().method() === 'GET',
    );
    await this.page.getByPlaceholder('按文件名筛选').fill(name);
    await this.page.getByRole('button', { name: '搜索', exact: true }).click();
    await resp;
  }

  rowByName(name: string) {
    return this.page.locator('.el-table__row').filter({ hasText: name });
  }

  async visibleCount(name: string): Promise<number> {
    return this.rowByName(name).count();
  }

  // 轮询等待匹配行数达到期望值（Vue 渲染滞后于响应，立即 count 不可靠）
  async waitRowCount(name: string, want: number, timeout = 8000): Promise<number> {
    const t0 = Date.now();
    for (;;) {
      const n = await this.rowByName(name).count();
      if (n === want || Date.now() - t0 > timeout) return n;
      await this.page.waitForTimeout(200);
    }
  }

  // 删除搜索到的第一行；行内删除无确认弹窗，成功 toast "已删除"（menus.configDeleted）
  async deleteFirst(name: string): Promise<string> {
    const row = this.rowByName(name).first();
    await row.waitFor({ timeout: 8000 });
    const delResp = this.page.waitForResponse(
      (r) =>
        /\/v1\/file\/[^/]+$/.test(r.url()) && r.request().method() === 'DELETE',
    );
    await row.getByRole('button', { name: '删除', exact: true }).click();
    await delResp;
    const toast = this.page
      .locator('.el-message')
      .filter({ hasText: '已删除' })
      .last();
    await toast.waitFor({ timeout: 15000 });
    return (await toast.textContent()) ?? '';
  }
}

// /#/file/presign 预签名页（frontend/src/views/file/presign.vue）
export class FilePresignPage {
  constructor(private page: Page) {}

  async goto(): Promise<void> {
    await this.page.goto(`${config.frontendURL}/#/file/presign`);
    // 页面标题 menus.presignTitle=预签名 URL
    await this.page
      .getByRole('heading', { name: '预签名 URL' })
      .waitFor({ timeout: 15000 });
  }

  async headingText(): Promise<string> {
    const h = this.page.getByRole('heading', { name: '预签名 URL' });
    return ((await h.textContent()) ?? '').trim();
  }
}
