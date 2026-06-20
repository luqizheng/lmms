import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as XLSX from 'xlsx';

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface CaseResult {
  caseId: string;
  desc: string;
  result: 'PASS' | 'FAIL' | 'SKIP';
  status: number | string;
  durationMs: number;
  error?: string;
}

export interface ErrorDetail {
  caseId: string;
  request: unknown;
  response: unknown;
  expect: unknown;
  timestamp: string;
  screenshot?: string;
}

const reportDir = __dirname;
const errorsDir = join(__dirname, 'errors');

// 写单个失败用例 <caseId>.json
export function writeErrorJson(detail: ErrorDetail): string {
  mkdirSync(errorsDir, { recursive: true });
  const file = join(errorsDir, `${detail.caseId}.json`);
  writeFileSync(file, JSON.stringify(detail, null, 2), 'utf8');
  return file;
}

// 汇总写 report-<时间戳>.xlsx，返回路径
export function writeReport(results: CaseResult[]): string {
  const ts = new Date().toISOString().replace(/[:.]/g, '-');
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet(results), 'detail');
  const summary = {
    total: results.length,
    pass: results.filter((r) => r.result === 'PASS').length,
    fail: results.filter((r) => r.result === 'FAIL').length,
    skip: results.filter((r) => r.result === 'SKIP').length,
  };
  XLSX.utils.book_append_sheet(wb, XLSX.utils.json_to_sheet([summary]), 'summary');
  const file = join(reportDir, `report-${ts}.xlsx`);
  XLSX.writeFile(wb, file);
  return file;
}
