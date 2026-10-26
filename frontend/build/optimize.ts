/**
 * 依赖预构建配置。
 *
 * reka-ui 是无头原语库（含大量副作用模块），预构建反而更慢也更易出问题，
 * 直接排除交给 Vite 按需编译。
 */
export const include = [
  "vue",
  "vue-router",
  "pinia",
  "vue-i18n",
  "lucide-vue-next"
];

export const exclude = ["reka-ui"];
