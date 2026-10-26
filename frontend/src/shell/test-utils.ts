import { mount, type ComponentMountingOptions } from "@vue/test-utils";
import { createPinia } from "pinia";
import type { Component } from "vue";
import { i18n } from "./i18n";

/**
 * 壳组件的测试挂载：统一装上 pinia 与 i18n。
 *
 * 壳的组件用 `useI18n()` 取文案，没有 i18n 插件会直接抛
 * "Need to install with `app.use` function"。
 */
export function mountShell<T extends Component>(
  component: T,
  options: ComponentMountingOptions<T> = {} as ComponentMountingOptions<T>
) {
  const plugins = options.global?.plugins ?? [];
  return mount(component, {
    ...options,
    global: {
      ...options.global,
      plugins: [createPinia(), i18n, ...plugins]
    }
  } as ComponentMountingOptions<T>);
}
