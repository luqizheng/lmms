import {
  listDepts,
  listRoles,
  listUsers,
  type ScopeItem
} from "@/api/iam-scopes";

interface CacheEntry<T> {
  at: number;
  data: T;
}

const TTL = 5 * 60 * 1000;
const cache: Record<string, CacheEntry<unknown>> = {};

function get<T>(key: string): T | null {
  const e = cache[key];
  if (!e) return null;
  if (Date.now() - e.at > TTL) {
    delete cache[key];
    return null;
  }
  return e.data as T;
}
function set<T>(key: string, data: T) {
  cache[key] = { at: Date.now(), data };
}

function normalizeDept(d: any): ScopeItem {
  return {
    id: Number(d.id),
    name: d.name ?? d.dept_name ?? `dept-${d.id}`
  };
}
function normalizeUser(u: any): ScopeItem {
  return {
    id: Number(u.id),
    name: u.nickname || u.username || `user-${u.id}`
  };
}
function normalizeRole(r: any): ScopeItem {
  return {
    id: Number(r.id),
    name: r.name || r.code || `role-${r.id}`
  };
}

export interface ScopesResult {
  depts: ScopeItem[];
  roles: ScopeItem[];
  users: ScopeItem[];
}

export async function loadScopes(): Promise<ScopesResult> {
  const cached = get<ScopesResult>("scopes");
  if (cached) return cached;

  const [deptsRaw, rolesRaw, usersRaw] = await Promise.all([
    listDepts().catch(() => []),
    listRoles().catch(() => []),
    listUsers().catch(() => ({ list: [] }))
  ]);

  const depts = (
    Array.isArray(deptsRaw) ? deptsRaw : ((deptsRaw as any)?.list ?? [])
  ).map(normalizeDept);
  const roles = (Array.isArray(rolesRaw) ? rolesRaw : []).map(normalizeRole);
  const users = (
    Array.isArray(usersRaw) ? usersRaw : ((usersRaw as any)?.list ?? [])
  ).map(normalizeUser);

  const data: ScopesResult = { depts, roles, users };
  set("scopes", data);
  return data;
}

export function invalidateScopes() {
  delete cache["scopes"];
}
