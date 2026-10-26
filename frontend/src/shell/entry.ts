import { createApp } from "vue";
import { createPinia } from "pinia";
import { useI18n } from "./i18n";
import { router } from "./router";
import ShellApp from "./ShellApp.vue";
import { onUnauthorized } from "./api/client";
import { useAuthStore } from "./store/auth";
import "@/styles/prism/index.css";

/**
 * PRISM 壳的应用入口。
 *
 * 与 legacy 的 `src/main.ts` 并存：Plan 7 删除 pure-admin 时，连带删除 legacy 入口。
 */
const app = createApp(ShellApp);
const pinia = createPinia();

app.use(pinia);
useI18n(app);
app.use(router);

// token 彻底失效的全局兜底：必须在这里注册，
// 不能挂在登录页上 —— 用户带着 localStorage 会话直接进业务页时从未挂载过登录页。
onUnauthorized(() => {
  useAuthStore(pinia).reset();
  if (router.currentRoute.value.name !== "login") {
    router.replace({
      name: "login",
      query: { redirect: router.currentRoute.value.fullPath }
    });
  }
});

// 页面标题随路由切换（Phase 2 §5.3）
router.afterEach(to => {
  const title = (to.meta?.title as string | undefined) ?? "";
  document.title = title ? `${title} · PRISM 控制台` : "PRISM 控制台";
});

router.isReady().then(() => {
  app.mount("#app");
});
