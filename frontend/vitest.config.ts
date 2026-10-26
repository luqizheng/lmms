import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

/**
 * vitest 配置。
 *
 * 单独成文件的原因：vite.config.ts 里的 build.rolldownOptions 是 vite 8 / rolldown
 * 的选项，vitest 2.x 的类型定义还不认识它，放进同一个文件会类型不兼容。
 *
 * 超时放宽到 20s：部分 legacy 测试用真实定时器（防抖 / 延时），
 * 并行负载下 5s 默认超时不够。
 */
const base =
  typeof viteConfig === "function"
    ? await viteConfig({ command: "serve", mode: "test" })
    : viteConfig;

export default mergeConfig(
  base,
  defineConfig({
    test: {
      testTimeout: 20000,
      hookTimeout: 20000
    }
  })
);
