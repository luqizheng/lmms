// 本地 API Key 缓存（仅记录本浏览器内创建过的 key）
// 后端 token-service 暂未提供 GET /v1/tokens/api_key 全量列表，
// 创建后只显示一次。该 cache 让用户能回看自己创建过的 key。
//
// 注意：localStorage 是明文存 API Key，**仅适合一次性创建场景**；
// 后续如后端补 list 端点，应改为读后端。

const KEY = "token_api_key_local_cache";

export interface CachedApiKey {
  id: string;
  api_key: string;
  name: string;
  scopes?: string[];
  created_at: number;
}

function load(): CachedApiKey[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

function save(list: CachedApiKey[]) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // quota — skip
  }
}

export function listCachedApiKeys(): CachedApiKey[] {
  return load().sort((a, b) => b.created_at - a.created_at);
}

export function recordApiKey(key: CachedApiKey): void {
  const list = load();
  list.unshift(key);
  save(list);
}

export function removeCachedApiKey(id: string): void {
  const list = load().filter(k => k.id !== id);
  save(list);
}