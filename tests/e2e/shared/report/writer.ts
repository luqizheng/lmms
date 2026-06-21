import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as XLSX from 'xlsx';

const __dirname = dirname(fileURLToPath(import.meta.url));

export interface CaseResult {
  caseId: string;
  desc: string;
  result: 'PASS' | 'FAIL';
  status: string | number;
  durationMs: number;
  error: string;
}

export interface ErrorDetail {
  caseId: string;
  request: unknown;
  response: unknown;
  expect?: unknown;
  timestamp: string;
  screenshot?: string;
}

const errorsDir = join(__dirname, 'errors');

function ensureErrorsDir(): void {
  if (!existsSync(errorsDir)) {
    mkdirSync(errorsDir, { recursive: true });
  }
}

export function writeReport(results: CaseResult[]): string {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
  const filename = `report-${timestamp}.xlsx`;
  const filepath = join(__dirname, filename);

  // detail sheet
  const detailData = results.map((r) => ({
    caseId: r.caseId,
    desc: r.desc,
    result: r.result,
    status: r.status,
    durationMs: r.durationMs,
    error: r.error,
  }));

  // summary sheet
  const total = results.length;
  const passed = results.filter((r) => r.result === 'PASS').length;
  const failed = results.filter((r) => r.result === 'FAIL').length;
  const skipped = results.filter((r) => r.result === 'SKIP').length;
  const summaryData = [
    { metric: 'total', value: total },
    { metric: 'pass', value: passed },
    { metric: 'fail', value: failed },
    { metric: 'skip', value: skipped },
  ];

  const wb = XLSX.utils.book_new();
  const detailSheet = XLSX.utils.json_to_sheet(detailData);
  const summarySheet = XLSX.utils.json_to_sheet(summaryData);
  XLSX.utils.book_append_sheet(wb, detailSheet, 'detail');
  XLSX.utils.book_append_sheet(wb, summarySheet, 'summary');
  XLSX.writeFile(wb, filepath);

  return filepath;
}

export function writeErrorJson(detail: ErrorDetail): void {
  ensureErrorsDir();
  const filepath = join(errorsDir, `${detail.caseId}.json`);
  writeFileSync(filepath, JSON.stringify(detail, null, 2), 'utf8');
}