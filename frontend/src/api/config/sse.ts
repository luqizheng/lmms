// SSE 订阅：/v1/config/watch 配置变更推送
//
// 2026-07-02 激活：api-gateway 现支持 ws=true 路由的流式转发 + ?token= query
//   鉴权（详见 docs/superpowers/specs/2026-07-02-sse-gateway-streaming-design.md）。
//
// EventSource 限制（spec 限制）：
//   - 不能自定义 header（Authorization Bearer 不能塞 header）
//   - token 必须走 query: ?token=<jwt>
//   - api-gateway auth_middleware 现已支持 header 优先 / ?token= fallback。
//
// 用法：
//   import { watchConfig } from "@/api/config/sse";
//   const stop = watchConfig({
//     namespaceId: "global",
//     onEvent: (e) => console.log(e),
//     onError: (err) => console.error(err)
//   });
//   // 卸载：stop()

import { storageLocal } from "@pureadmin/utils";
import { TokenKey, formatToken } from "@/utils/auth";

export interface WatchConfigOptions {
  namespaceId: string;
  onEvent: (msg: { item_id: string; key: string; action: string }) => void;
  onError?: (err: unknown) => void;
  baseURL?: string;
}

function wsBaseURL(): string {
  if (typeof window === "undefined") return "";
  const base = (import.meta.env.VITE_API_BASE as string | undefined) ?? "/api";
  return `${location.protocol === "https:" ? "https:" : "http:"}//${location.host}${base}`;
}

export function watchConfig(opts: WatchConfigOptions): () => void {
  if (typeof window === "undefined") {
    return () => undefined;
  }
  const rawToken = storageLocal().getItem<string>(TokenKey);
  const tokenStr = formatToken(rawToken ?? "") ?? "";
  const url = `${wsBaseURL()}/v1/config/watch?namespace_id=${encodeURIComponent(
    opts.namespaceId
  )}&token=${encodeURIComponent(tokenStr)}`;

  const es = new EventSource(url);

  es.addEventListener("config.item.changed", ev => {
    try {
      const data = JSON.parse((ev as MessageEvent).data);
      opts.onEvent(data);
    } catch (e) {
      opts.onError?.(e);
    }
  });

  es.onerror = ev => {
    opts.onError?.(ev);
  };

  return () => {
    es.close();
  };
}

export function isSseReady(): boolean {
  return true;
}