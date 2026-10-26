/**
 * token 持久化。
 *
 * 与 @digua/http 解耦：HTTP 客户端只通过回调取 token，
 * 因此这里不依赖任何 store，可在模块初始化阶段安全使用。
 */
const KEYS = {
  access: "prism_access_token",
  refresh: "prism_refresh_token",
  expiresAt: "prism_expires_at",
  user: "prism_user"
} as const;

function safeGet(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeSet(key: string, value: string | null) {
  try {
    if (value === null) window.localStorage.removeItem(key);
    else window.localStorage.setItem(key, value);
  } catch {
    // localStorage 不可用（隐私模式 / SSR）时忽略：内存态仍可用
  }
}

export interface StoredUser {
  id: string;
  username: string;
  nickname: string;
  email?: string | null;
  phone?: string | null;
  avatar?: string | null;
  roles: string[];
  permissions: string[];
}

export function getAccessToken(): string | undefined {
  return safeGet(KEYS.access) ?? undefined;
}

export function getRefreshToken(): string | undefined {
  return safeGet(KEYS.refresh) ?? undefined;
}

/** 过期时间（毫秒时间戳）；无记录时视为已过期 */
export function getExpiresAt(): number {
  const raw = safeGet(KEYS.expiresAt);
  const parsed = raw ? Number(raw) : 0;
  return Number.isFinite(parsed) ? parsed : 0;
}

export function isTokenExpired(): boolean {
  return getExpiresAt() <= Date.now();
}

export function getStoredUser(): StoredUser | null {
  const raw = safeGet(KEYS.user);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as StoredUser;
  } catch {
    return null;
  }
}

export function saveSession(params: {
  accessToken: string;
  refreshToken: string;
  expiresIn: number;
  user: StoredUser;
}) {
  safeSet(KEYS.access, params.accessToken);
  safeSet(KEYS.refresh, params.refreshToken);
  safeSet(KEYS.expiresAt, String(Date.now() + params.expiresIn * 1000));
  safeSet(KEYS.user, JSON.stringify(params.user));
}

export function clearSession() {
  safeSet(KEYS.access, null);
  safeSet(KEYS.refresh, null);
  safeSet(KEYS.expiresAt, null);
  safeSet(KEYS.user, null);
}
