import { readFileSync } from 'node:fs';
import * as XLSX from 'xlsx';

export interface ApiCase {
  caseId: string;
  desc: string;
  method: string;
  path: string;
  headers?: string;
  body?: string;
  expectStatus: string;
  expectBody?: string;
  skip?: string;
}

// 读取单个 xlsx 的所有 sheet，返回 { sheetName: rows }
export function readCases<T = Record<string, string>>(file: string): Record<string, T[]> {
  const wb = XLSX.read(readFileSync(file), { type: 'buffer' });
  const out: Record<string, T[]> = {};
  for (const name of wb.SheetNames) {
    out[name] = XLSX.utils.sheet_to_json<T>(wb.Sheets[name], { defval: '' });
  }
  return out;
}

// 占位符解析：${rand} 唯一值；${CASE-ID.field} 引用前序响应
export function resolvePlaceholders(
  input: string,
  ctx: Record<string, unknown>,
): string {
  if (!input) return input;
  return input
    .replace(/\$\{rand\}/g, () => `${Date.now()}${Math.random().toString(36).slice(2, 6)}`)
    .replace(/\$\{([\w-]+)\.([\w.]+)\}/g, (_m, caseId: string, field: string) => {
      const resp = ctx[caseId] as Record<string, unknown> | undefined;
      if (!resp) throw new Error(`占位引用未产出的用例: ${caseId}`);
      const val = field.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], resp);
      return String(val ?? '');
    });
}

// 极简断言：表达式如 "$.id exists" / "$.username==alice"
export function assertBody(expr: string, body: unknown): { ok: boolean; reason?: string } {
  if (!expr) return { ok: true };
  for (const clause of expr.split('&&').map((s) => s.trim()).filter(Boolean)) {
    const existsM = clause.match(/^\$\.([\w.]+)\s+exists$/);
    const eqM = clause.match(/^\$\.([\w.]+)==(.+)$/);
    if (existsM) {
      const v = getPath(body, existsM[1]);
      if (v === undefined || v === null) return { ok: false, reason: `${clause} 不存在` };
    } else if (eqM) {
      const v = getPath(body, eqM[1]);
      if (String(v) !== eqM[2]) return { ok: false, reason: `${clause} 实际=${String(v)}` };
    } else {
      return { ok: false, reason: `无法解析断言: ${clause}` };
    }
  }
  return { ok: true };
}

function getPath(obj: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], obj);
}
