import vue from "@vitejs/plugin-vue";
import VueI18nPlugin from "@intlify/unplugin-vue-i18n/vite";
import { pathResolve } from "./utils";

/**
 * 构建插件。
 *
 * 刻意保持精简：PRISM 壳不引入 Tailwind / unplugin-icons / CDN 替换 / 代码检查器，
 * 样式一律是 scoped CSS + token 变量，图标一律 lucide-vue-next。
 */
export function getPluginsList() {
  return [
    vue(),
    // yaml 形式的 i18n 资源在构建期预编译
    VueI18nPlugin({ include: [pathResolve("locales/**")] })
  ];
}
