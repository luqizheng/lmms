import { getIamUser } from "@/api/iam";

const MAX_CACHE_SIZE = 100;

interface CacheEntry {
  nickname: string;
  ts: number;
}

/**
 * 全局 LRU cache。
 * JavaScript Map 按插入顺序维护 key——遍历 keys() 时第一个 key 就是最旧的，
 * 天然支持 LRU 淘汰。
 */
const cache = new Map<string, CacheEntry>();

/** 并发请求去重：同一 userId 只发一个 HTTP 请求。 */
const pending = new Map<string, Promise<string>>();

/** 淘汰最旧的条目直到缓存大小回到上限内。 */
function evict(): void {
  while (cache.size > MAX_CACHE_SIZE) {
    const oldest = cache.keys().next().value as string;
    cache.delete(oldest);
  }
}

/**
 * 获取用户昵称。
 * - 缓存命中 → 直接返回并晋升 LRU 位置
 * - 有进行中的请求 → 复用同一个 Promise（防抖/去重）
 * - 缓存未命中 → 调用 API，写入缓存
 * - API 失败 → 返回 userId 本身作为 fallback，不写入缓存
 */
export async function getUserNickname(userId: string): Promise<string> {
  if (!userId) return "";

  // 1. 查 LRU 缓存
  const cached = cache.get(userId);
  if (cached) {
    // LRU 晋升：删除再插回末尾
    cache.delete(userId);
    cache.set(userId, cached);
    return cached.nickname;
  }

  // 2. 复用进行中的请求（防抖）
  const inflight = pending.get(userId);
  if (inflight) return inflight;

  // 3. 发起新请求
  const promise = getIamUser(userId)
    .then(user => {
      const nickname = user?.nickname || userId;
      cache.set(userId, { nickname, ts: Date.now() });
      evict();
      return nickname;
    })
    .catch(() => {
      // 失败回退：显示 userId 本身
      return userId;
    })
    .finally(() => {
      pending.delete(userId);
    });

  pending.set(userId, promise);
  return promise;
}

/** 清空全部缓存（例如在用户信息变更后调用）。 */
export function clearUserLabelCache(): void {
  cache.clear();
}

/** 清除单个 userId 的缓存。 */
export function invalidateUserLabel(userId: string): void {
  cache.delete(userId);
}
