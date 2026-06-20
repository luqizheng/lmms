import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as XLSX from 'xlsx';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

function write(file, sheets) {
  mkdirSync(dirname(file), { recursive: true });
  const wb = XLSX.utils.book_new();
  for (const [name, rows] of Object.entries(sheets)) {
    XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet(rows), name);
  }
  XLSX.writeFile(wb, file);
  console.log('written', file);
}

write(join(root, 'api/cases/iam-users.xlsx'), {
  users: [
    { caseId: 'IAM-USER-001', desc: '创建用户', method: 'POST', path: '/v1/iam/users', headers: '', body: '{"username":"u${rand}","password":"P@ssw0rd!","email":"u${rand}@t.local","nickname":"N"}', expectStatus: '200', expectBody: '$.id exists', skip: '' },
    { caseId: 'IAM-USER-002', desc: '查详情', method: 'GET', path: '/v1/iam/users/${IAM-USER-001.id}', headers: '', body: '', expectStatus: '200', expectBody: '$.username exists', skip: '' },
    { caseId: 'IAM-USER-003', desc: '删除', method: 'DELETE', path: '/v1/iam/users/${IAM-USER-001.id}', headers: '', body: '', expectStatus: '204', expectBody: '', skip: '' },
  ],
});

write(join(root, 'ui/cases/user-flow.xlsx'), {
  user: [
    { caseId: 'UI-USER-001', desc: '登录后建用户', flow: 'createUser', params: '{"username":"u${rand}"}', expect: 'toast=创建成功', skip: '' },
  ],
});
