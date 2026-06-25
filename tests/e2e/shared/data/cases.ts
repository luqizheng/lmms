import { readFileSync } from 'node:fs';
import { parse } from 'yaml';

// ===== 用例类型 =====
export interface ApiCase {
  caseId: string;
  desc: string;
  method: string;
  path: string;
  auth?: boolean; // 默认 true，false 则不带 Authorization
  headers?: Record<string, string>;
  body?: unknown;
  expectStatus: number | string | Array<number | string>; // 单值或容忍数组
  expect?: string[]; // 断言表达式数组
  skip?: boolean;
  smoke?: boolean; // true → 纳入 smoke 子集（test:smoke）
  timeoutMs?: number; // 覆盖默认 testTimeout
}

export interface UiCase {
  caseId: string;
  desc: string;
  flow: string;
  params?: Record<string, unknown>;
  expect?: string[]; // 形如 ["toast=已新增用户"]
  skip?: boolean;
  smoke?: boolean; // true → 纳入 smoke 子集
  tags?: string[]; // 例如 ['@smoke'] 用于 playwright grep
}

interface CaseFile<T> {
  group: string;
  cases: T[];
}

// 读取单个 yaml 用例文件，返回 { group, cases }
export function readCases<T = ApiCase>(file: string): CaseFile<T> {
  const doc = parse(readFileSync(file, 'utf8')) as CaseFile<T>;
  if (!doc || !Array.isArray(doc.cases)) {
    throw new Error(`用例文件格式错误（缺 cases 数组）: ${file}`);
  }
  return { group: doc.group ?? file, cases: doc.cases };
}

// 占位符解析：递归处理对象/数组/字符串
// ${rand} → 唯一值；${CASE-ID.field} → 引用前序响应字段
export function resolve<T>(input: T, ctx: Record<string, unknown>): T {
  if (typeof input === 'string') {
    return resolveString(input, ctx) as T;
  }
  if (Array.isArray(input)) {
    return input.map((v) => resolve(v, ctx)) as T;
  }
  if (input && typeof input === 'object') {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(input)) {
      out[k] = resolve(v, ctx);
    }
    return out as T;
  }
  return input;
}

function resolveString(input: string, ctx: Record<string, unknown>): string {
  return input
    .replace(/\$\{rand\}/g, () => `${Date.now()}${Math.random().toString(36).slice(2, 6)}`)
    .replace(/\$\{([\w-]+)\.([\w.]+)\}/g, (_m, caseId: string, field: string) => {
      const resp = ctx[caseId] as Record<string, unknown> | undefined;
      if (!resp) throw new Error(`占位引用未产出的用例: ${caseId}`);
      const val = field.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], resp);
      return String(val ?? '');
    });
}

// 按 smoke 标记过滤；smokeOnly=false 返回全部
export function filterBySmoke<T extends { smoke?: boolean }>(
  cases: T[],
  smokeOnly: boolean,
): T[] {
  return smokeOnly ? cases.filter((c) => c.smoke === true) : cases;
}

// 状态码匹配：期望可为单值或数组（容忍多状态）
export function statusMatches(
  actual: number,
  expect: number | string | Array<number | string>,
): boolean {
  const list = Array.isArray(expect) ? expect : [expect];
  return list.some((e) => String(e) === String(actual));
}

// 断言：每条形如
//   "$.id exists" / "$.id not exists"
//   "$.username==alice"
//   "$.items is array"
//   "$.data contains E2E 菜单"
export function assertBody(
  exprs: string[] | undefined,
  body: unknown,
): { ok: boolean; reason?: string } {
  if (!exprs || exprs.length === 0) return { ok: true };
  for (const raw of exprs) {
    const clause = raw.trim();
    if (!clause) continue;
    const notExistsM = clause.match(/^\$\.([\w.]+)\s+not\s+exists$/);
    const existsM = clause.match(/^\$\.([\w.]+)\s+exists$/);
    const isArrayM = clause.match(/^\$\.?([\w.]*)\s+is\s+array$/);
    const containsM = clause.match(/^\$\.?([\w.]*)\s+contains\s+(.+)$/);
    const eqM = clause.match(/^\$\.([\w.]+)==(.+)$/);
    if (notExistsM) {
      const v = getPath(body, notExistsM[1]);
      if (v !== undefined && v !== null) return { ok: false, reason: `${clause} 实际存在` };
    } else if (existsM) {
      const v = getPath(body, existsM[1]);
      if (v === undefined || v === null) return { ok: false, reason: `${clause} 不存在` };
    } else if (isArrayM) {
      const v = isArrayM[1] ? getPath(body, isArrayM[1]) : body;
      if (!Array.isArray(v)) return { ok: false, reason: `${clause} 非数组` };
    } else if (containsM) {
      const v = containsM[1] ? getPath(body, containsM[1]) : body;
      if (!JSON.stringify(v ?? '').includes(containsM[2].trim())) {
        return { ok: false, reason: `${clause} 不包含` };
      }
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
