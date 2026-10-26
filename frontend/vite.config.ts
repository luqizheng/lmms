import { getPluginsList } from "./build/plugins";
import { include, exclude } from "./build/optimize";
import { type UserConfigExport, type ConfigEnv, loadEnv } from "vite";
import { root, alias, wrapperEnv, pathResolve } from "./build/utils";

/**
 * PRISM 壳的构建配置。
 *
 * 这是**独立的前端工程** —— 不依赖 pure-admin 的构建链：
 * 没有 Tailwind、没有 unplugin-icons、没有 CDN 替换、没有 element-plus。
 */
export default async ({ mode }: ConfigEnv): Promise<UserConfigExport> => {
  const { VITE_PORT, VITE_PUBLIC_PATH } = wrapperEnv(loadEnv(mode, root));

  return {
    base: VITE_PUBLIC_PATH,
    root,
    resolve: {
      alias
    },
    server: {
      port: VITE_PORT,
      host: "0.0.0.0",
      // 本地跨域代理 https://cn.vitejs.dev/config/server-options.html#server-proxy
      proxy: {
        "/api": {
          target: "http://127.0.0.1:3000",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/api/, "")
        },
        // 直连后端服务的 health/metrics 探测（绕开 api-gateway 的 JWT 鉴权），仅 dev 模式生效
        "/probe/api-gateway": {
          target: "http://127.0.0.1:3000",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/api-gateway/, "")
        },
        "/probe/token-service": {
          target: "http://127.0.0.1:8082",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/token-service/, "")
        },
        "/probe/iam-service": {
          target: "http://127.0.0.1:9081",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/iam-service/, "")
        },
        "/probe/notification-service": {
          target: "http://127.0.0.1:8083",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/notification-service/, "")
        },
        "/probe/audit-service": {
          target: "http://127.0.0.1:8084",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/audit-service/, "")
        },
        "/probe/file-service": {
          target: "http://127.0.0.1:8086",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/file-service/, "")
        },
        "/probe/config-service": {
          target: "http://127.0.0.1:8085",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/config-service/, "")
        },
        "/probe/prue-backend": {
          target: "http://127.0.0.1:8087",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/prue-backend/, "")
        }
      },
      // 预热文件以提前转换和缓存结果，降低启动期间的初始页面加载时长并防止转换瀑布
      warmup: {
        clientFiles: ["./index.html", "./src/shell/**/*"]
      }
    },
    plugins: await getPluginsList(),
    // https://cn.vitejs.dev/config/dep-optimization-options.html#dep-optimization-options
    optimizeDeps: {
      include,
      exclude
    },
    build: {
      // https://cn.vitejs.dev/guide/build.html#browser-compatibility
      target: "es2020",
      sourcemap: false,
      chunkSizeWarningLimit: 4000,
      rolldownOptions: {
        input: {
          index: pathResolve("index.html")
        },
        // 静态资源分类打包
        output: {
          chunkFileNames: "static/js/[name]-[hash].js",
          entryFileNames: "static/js/[name]-[hash].js",
          assetFileNames: "static/[ext]/[name]-[hash].[ext]"
        },
        checks: {
          pluginTimings: false,
          toleratedTransform: false
        }
      }
    },
    // vitest：部分测试用真实定时器，并行负载下 5s 默认超时不够
    test: {
      testTimeout: 20000,
      hookTimeout: 20000
    },
    define: {
      __INTLIFY_PROD_DEVTOOLS__: false
    }
  };
};
