# 删 `# syntax=docker/dockerfile:1.7`：内网 registry 无 docker.io 镜像可拉，BuildKit 默认前端（1.24）已够本文件用到的 1.4+ 特性。
ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/node:22-alpine AS builder

WORKDIR /app

ENV HUSKY=0
ENV CI=true

RUN corepack enable && corepack prepare pnpm@latest --activate

# workspace 成员的 manifest 必须在 install 之前就位，否则 pnpm 解析 workspace:* 依赖会失败
COPY package.json pnpm-lock.yaml* .npmrc* ./
COPY packages/contracts/package.json packages/contracts/
COPY packages/http/package.json packages/http/

RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --ignore-scripts --fetch-timeout=300000 || \
    pnpm install --ignore-scripts --fetch-timeout=300000

COPY . .

RUN pnpm build

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/nginx:1.27-alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
