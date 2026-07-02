# syntax=docker/dockerfile:1.7
ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/node:22-alpine AS builder

WORKDIR /app

ENV HUSKY=0
ENV CI=true

RUN corepack enable && corepack prepare pnpm@latest --activate

COPY package.json pnpm-lock.yaml* .npmrc* ./

RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --ignore-scripts || pnpm install --ignore-scripts

COPY . .

# 测试环境可通过 build arg 禁用登录图形验证码（默认空=启用，生产构建不传则不受影响）
ARG VITE_DISABLE_CAPTCHA
ENV VITE_DISABLE_CAPTCHA=$VITE_DISABLE_CAPTCHA

RUN pnpm build

ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official
FROM ${BASE_REGISTRY}/nginx:1.27-alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
