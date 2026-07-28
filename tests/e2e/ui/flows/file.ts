import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FileListPage, FilePresignPage, FileUploadPage } from '../pages/file.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

const FIXTURE_PATH = fileURLToPath(
  new URL('../dev/fixtures/e2e-upload.txt', import.meta.url),
);

export const fileFlows: Record<string, Flow> = {
  // 上传 fixture 文件；toast 形如 "成功 1，失败 0"
  uploadFile: async (page) => {
    await loginFirst(page);
    const up = new FileUploadPage(page);
    await up.goto();
    return { toast: await up.submitFile(FIXTURE_PATH) };
  },

  // 删除闭环：上传唯一文件名 → 列表搜索可见 → 删除 → 列表不可见
  deleteFile: async (page, params) => {
    await loginFirst(page);
    const filename = String(params.filename);
    const up = new FileUploadPage(page);
    await up.goto();
    await up.submitBuffer(filename, readFileSync(FIXTURE_PATH));

    const list = new FileListPage(page);
    await list.goto();
    await list.search(filename);
    if ((await list.waitRowCount(filename, 1)) !== 1) {
      throw new Error(`上传后列表未搜索到 ${filename}`);
    }
    const toast = await list.deleteFirst(filename);
    await list.search(filename);
    if ((await list.waitRowCount(filename, 0)) !== 0) {
      throw new Error(`删除后列表仍可见 ${filename}`);
    }
    return { toast };
  },

  // 预签名页 smoke：打开验证渲染
  viewPresignPage: async (page) => {
    await loginFirst(page);
    const pp = new FilePresignPage(page);
    await pp.goto();
    return { dialog: await pp.headingText() };
  },
};
