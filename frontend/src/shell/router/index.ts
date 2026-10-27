import {
  createRouter,
  createWebHashHistory,
  createWebHistory,
  type RouterHistory
} from "vue-router";
import { staticRoutes } from "./routes";
import { installAuthGuard } from "./guard";

/**
 * 解析 `VITE_ROUTER_HISTORY` 生成 history 实现。
 * 取值 `hash`（默认）或 `h5`（HTML5 history），可逗号附加 base，如 `h5,/sub`。
 */
function getHistoryMode(): RouterHistory {
  const raw = String(import.meta.env.VITE_ROUTER_HISTORY ?? "hash")
    .replace(/["']/g, "")
    .trim();
  const [mode, base = ""] = raw.split(",");
  return mode === "h5" ? createWebHistory(base) : createWebHashHistory(base);
}

export const router = createRouter({
  history: getHistoryMode(),
  routes: staticRoutes
});

installAuthGuard(router);

export { staticRoutes };
export default router;
