import { createRouter, createWebHashHistory } from "vue-router";
import { staticRoutes } from "./routes";
import { installAuthGuard } from "./guard";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: staticRoutes
});

installAuthGuard(router);

export { staticRoutes };
export default router;
