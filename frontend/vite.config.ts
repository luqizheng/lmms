import { getPluginsList } from "./build/plugins";
import { include, exclude } from "./build/optimize";
import { type UserConfigExport, type ConfigEnv, loadEnv } from "vite";
import {
  root,
  alias,
  wrapperEnv,
  pathResolve,
  __APP_INFO__
} from "./build/utils";

export default async ({ mode }: ConfigEnv): Promise<UserConfigExport> => {
  const { VITE_CDN, VITE_PORT, VITE_COMPRESSION, VITE_PUBLIC_PATH } =
    wrapperEnv(loadEnv(mode, root));
  return {
    base: VITE_PUBLIC_PATH,
    root,
    resolve: {
      alias
    },
    // 服务端渲染
    server: {
      // 端口号
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
        },
        "/probe/pm-platform": {
          target: "http://127.0.0.1:8088",
          changeOrigin: true,
          rewrite: (p: string) => p.replace(/^\/probe\/pm-platform/, "")
        },
        // PM Platform (apps/pm, Spring Boot 3) 直连，绕过 api-gateway（与 frontend/nginx.conf 对齐）
        "/pm-api": {
          target: "http://127.0.0.1:8088",
          changeOrigin: true
        }
      },
      // 预热文件以提前转换和缓存结果，降低启动期间的初始页面加载时长并防止转换瀑布
      warmup: {
        clientFiles: ["./index.html", "./src/{views,components}/*"]
      }
    },
    plugins: await getPluginsList(VITE_CDN, VITE_COMPRESSION),
    // https://cn.vitejs.dev/config/dep-optimization-options.html#dep-optimization-options
    optimizeDeps: {
      include,
      exclude
    },
    build: {
      // https://cn.vitejs.dev/guide/build.html#browser-compatibility
      target: "es2015",
      sourcemap: false,
      // 消除打包大小超过500kb警告
      chunkSizeWarningLimit: 4000,
      rolldownOptions: {
        input: {
          index: pathResolve("./index.html", import.meta.url)
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
    define: {
      __INTLIFY_PROD_DEVTOOLS__: false,
      __APP_INFO__: JSON.stringify(__APP_INFO__)
    }
  };
};
