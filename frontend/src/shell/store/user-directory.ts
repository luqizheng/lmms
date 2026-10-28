import { ref } from "vue";
import { defineStore } from "pinia";
import { listUsersSimple } from "../api/users";

/** 分页拉取时的单页条数 */
const PAGE_SIZE = 200;

/**
 * 用户目录：userId → 昵称 缓存。
 *
 * - **懒加载**：首次调用 {@link useUserDirectory.ensureLoaded} 时拉取；
 * - **只拉一次**：全量拉取结果按会话缓存在内存里，后续调用直接命中；
 * - **单飞去重**：并发触发时复用同一个 in-flight 请求，不会重复打后端。
 *
 * 列表页里同一个 userId 出现多次、多行共用，也只会请求一次。
 */
export const useUserDirectory = defineStore("prism-user-directory", () => {
  const names = ref<Record<string, string>>({});
  const loaded = ref(false);
  let inflight: Promise<void> | null = null;

  async function fetchAll(): Promise<void> {
    const merged: Record<string, string> = {};
    let page = 1;
    for (;;) {
      const result = await listUsersSimple({ page, size: PAGE_SIZE });
      const items = result?.items ?? [];
      for (const user of items) {
        if (user?.id) merged[user.id] = user.nickname || user.id;
      }
      const total = result?.total ?? items.length;
      // 空页或已到末页即结束
      if (items.length === 0 || page * PAGE_SIZE >= total) break;
      page += 1;
    }
    names.value = merged;
  }

  /** 保证目录已加载；重复调用不会重复请求 */
  function ensureLoaded(): Promise<void> {
    if (loaded.value) return Promise.resolve();
    if (!inflight) {
      inflight = fetchAll()
        .then(() => {
          loaded.value = true;
        })
        .catch(() => {
          // 拉取失败：保持空目录，允许下次调用重试
        })
        .finally(() => {
          inflight = null;
        });
    }
    return inflight;
  }

  /** 查 userId 对应的昵称；未知或空 id 返回 undefined */
  function nameOf(userId?: string | null): string | undefined {
    if (!userId) return undefined;
    return names.value[userId];
  }

  return { names, loaded, ensureLoaded, nameOf };
});
/**
 * 用户 ID 可能带环境前缀（如 `dev__`，见 NoOpJwtVerifier 的占位值）。
 * 回退展示时只保留 `__` 之后的有效部分；没有前缀则原样返回。
 */
export function shortUserId(userId: string): string {
  const match = /^[a-z0-9]+__(.+)$/i.exec(userId);
  return match ? match[1] : userId;
}
