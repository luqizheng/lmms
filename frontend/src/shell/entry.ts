import { createApp } from "vue";
import { createPinia } from "pinia";
import { useI18n } from "./i18n";
import { router } from "./router";
import ShellApp from "./ShellApp.vue";
import "@/styles/prism/index.css";

/**
 * PRISM 壳的应用入口。
 *
 * 与 legacy 的 `src/main.ts` 并存：Plan 7 删除 pure-admin 时，连带删除 legacy 入口。
 */
const app = createApp(ShellApp);

app.use(createPinia());
useI18n(app);
app.use(router);

router.isReady().then(() => {
  app.mount("#app");
});
