---
layout: home

hero:
  name: Coder Micro Services
  text: Rust + Axum 微服务基础框架
  tagline: 网关、鉴权、配置、可观测性、事件总线、多租户 —— 开箱即用的微服务底座
  image:
    src: /logo.svg
    alt: Coder Micro Services
  actions:
    - theme: brand
      text: 新人入门
      link: /onboarding/
    - theme: alt
      text: 用户手册
      link: /user-manual/
    - theme: alt
      text: AI 协作上下文
      link: /ai-context

features:
  - icon: 🦀
    title: Rust + Axum 全栈
    details: 8 个 Rust HTTP 服务 + 2 个独立 outbox publisher，基于 Axum 0.8 / Tokio，性能与类型安全兼顾。
  - icon: 🚪
    title: API Gateway
    details: JWT 验签、Redis 黑名单、集中限流、多文件路由配置、SIGHUP 热重载，统一对外入口。
  - icon: 🔐
    title: 端到端鉴权
    details: RS256 JWT + service-jwt 服务间认证；Principal extractor；禁止 X-User-* headers。
  - icon: 📡
    title: 事件总线
    details: RabbitMQ + transactional outbox，同事务入队、独立 publisher 轮询发布、消费幂等。
  - icon: 📊
    title: 全链路可观测
    details: OpenTelemetry traces（Jaeger）+ Prometheus 指标（Grafana）+ 结构化日志。
  - icon: 🏢
    title: 多租户
    details: 共享 MySQL + tenant_id 隔离 + cross_tenant 标志，lint 守门防止越权。
  - icon: 🐦
    title: 灰度发布
    details: Consul canary tag + Nginx split_clients，10% → 50% → 100% 渐进，秒级回滚。
  - icon: 📜
    title: 契约驱动
    details: error-codes / permissions / events YAML 单一真相源，代码生成 + CI drift 守门。
---
