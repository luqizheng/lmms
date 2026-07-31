# Phase 2: config-service 配置中心 实施计划

**Goal:** 实现配置中心服务，支持多环境配置管理、版本控制、灰度发布、热更新推送。

**Architecture:** 微服务架构，使用 digua_common-db/digua_common-log/digua_common-cache。支持长轮询 / SSE 推送，敏感配置加密存储。

**Tech Stack:** Rust 2021 / axum 0.8 / sqlx 0.8 / tokio / tracing

参照设计：[2026-06-13-enterprise-base-libs-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-13-enterprise-base-libs-design.md) §4.4。

---

## 总体文件结构

```
services/config-service/
├─ Cargo.toml
├─ src/
│  ├─ lib.rs
│  ├─ main.rs
│  ├─ config.rs
│  ├─ error.rs
│  ├─ routes/
│  │  ├─ mod.rs
│  │  ├─ namespace.rs   # 命名空间管理
│  │  ├─ config.rs      # 配置项 CRUD
│  │  └─ watch.rs       # 长轮询/SSE 监听
│  ├─ models/
│  │  ├─ mod.rs
│  │  ├─ namespace.rs
│  │  ├─ config_item.rs
│  │  └─ version.rs
│  └─ services/
│     ├─ mod.rs
│     └─ config.rs
├─ migrations/
│  ├─ mysql/V001__init.sql
│  └─ sqlite/V001__init.sql
├─ config/config.yaml
├─ Dockerfile
├─ AGENTS.md
└─ README.md
```

---

## Task 1：项目骨架 + 数据库迁移

- [ ] **Step 1: 创建 Cargo.toml**
- [ ] **Step 2: 创建基础代码结构**
- [ ] **Step 3: 创建数据库迁移**
- [ ] **Step 4: 更新 Cargo workspace**

---

## Task 2：命名空间管理

- [ ] **Step 1: 创建命名空间模型**
- [ ] **Step 2: 实现命名空间服务**
- [ ] **Step 3: 创建命名空间路由**

---

## Task 3：配置项 CRUD

- [ ] **Step 1: 创建配置项模型**
- [ ] **Step 2: 实现配置项服务**
- [ ] **Step 3: 创建配置项路由**

---

## Task 4：版本管理

- [ ] **Step 1: 创建版本模型**
- [ ] **Step 2: 实现版本服务**

---

## Task 5：长轮询 / SSE 推送

- [ ] **Step 1: 创建 watch 路由**
- [ ] **Step 2: 实现事件广播机制**

---

## Task 6：服务入口 + 健康检查

- [ ] **Step 1: 创建 main.rs**
- [ ] **Step 2: 添加健康检查**

---

## Task 7：Docker + 服务规范

- [ ] **Step 1: 创建 Dockerfile**
- [ ] **Step 2: 创建 AGENTS.md**
- [ ] **Step 3: 创建 README.md**

---

## Task 8：单元测试

- [ ] **Step 1: 创建模型测试**

---

## API 端点清单

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /config/namespace | POST | 创建命名空间 |
| /config/namespace | GET | 查询命名空间列表 |
| /config/namespace/{id} | GET | 获取命名空间详情 |
| /config/namespace/{id} | PUT | 更新命名空间 |
| /config/namespace/{id} | DELETE | 删除命名空间 |
| /config/item | POST | 创建配置项 |
| /config/item | GET | 查询配置项列表 |
| /config/item/{id} | GET | 获取配置项详情 |
| /config/item/{id} | PUT | 更新配置项 |
| /config/item/{id} | DELETE | 删除配置项 |
| /config/watch | GET | 长轮询监听变更 |
| /config/sse | GET | SSE 推送 |
| /config/export/{namespace} | GET | 导出配置 |
| /config/import/{namespace} | POST | 导入配置 |
