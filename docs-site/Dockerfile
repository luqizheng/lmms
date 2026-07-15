# docs-site Dockerfile -- VitePress 文档站镜像（nginx 托管）
#
# 构建模式：vitepress build 在宿主机/CI 完成（需要完整 git 仓库以生成 lastUpdated），
# 本 Dockerfile 只打包预构建的 docs-site/.vitepress/dist/ 到 nginx。
# build-docker.js 的 docs-site 分支会在 docker build 前自动跑 `pnpm docs:build`。
#
# 构建上下文必须是仓库根：
#   pnpm docs:build && docker build -t docs-site:latest -f docs-site/Dockerfile .
#
# 或通过项目编排器（自动预构建）：
#   pnpm run docker:build:docs-site
#   pnpm run docker:build -- docs-site -v 1.0.0 --push

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/nginx:1.27-alpine

LABEL org.opencontainers.image.title="coder-docs-site" \
      org.opencontainers.image.description="VitePress documentation site for coder micro services"

COPY docs-site/.vitepress/dist /usr/share/nginx/html
COPY docs-site/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=3s --retries=3 --start-period=5s \
  CMD wget -q --spider http://localhost:80/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
