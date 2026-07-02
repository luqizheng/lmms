// SSE 占位：/v1/config/watch 配置变更订阅
//
// 状态（2026-07-01）：gateway 暂未把 /v1/config/watch 加入 config.router.toml。
// 原因：当前 api-gateway `forward_to_service` 全量 buffer 响应 +
//   RouteConfig.ws 字段无消费者 — SSE 端到端是 gateway 自身 feature gap，
//   需独立 plan 落地。前端本文件先把 EventSource + ?token= 用法钉下，
//   待 gateway 端修复后再激活。
//
// EventSource 限制（SPEC § API limitations）：
//   - 不能自定义 header（Authorization Bearer 不能塞 header）
//   - token 必须走 query: ?token=<jwt>
//   - api-gateway 当前 JWT middleware 仅检查 header，需扩 query-token 路径。
//
// 用法（占位，组件层暂不调用）：
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
  // ponytail: 占位 — gateway SSE 未发布前不调用 EventSource.open()。
  // 当 gateway /v1/config/watch 上线后，下方注释代码取消注释。
  //
  // const token = storageLocal().getItem<string>(TokenKey);
  // const url = `${wsBaseURL()}/v1/config/${opts.namespaceId}/watch?token=${encodeURIComponent(formatToken(token ?? "") ?? "")}`;
  // const es = new EventSource(url);
  // es.addEventListener("config.item.changed", ev => {
  //   try { opts.onEvent(JSON.parse((ev as MessageEvent).data)); } catch (e) { opts.onError?.(e); }
  // });
  // es.onerror = ev => { opts.onError?.(ev); };
  // return () => { es.close(); };

  // 当前 stub：no-op，记录调用以便排查
  if (typeof console !== "undefined") {
    console.warn(
      "[config/sse] watchConfig() called but disabled — gateway SSE not yet wired (see §3 frontend_todo.md)"
    );
  }
  return () => undefined;
}

export function isSseReady(): boolean {
  return false;
}
