# config-service 使用指南

> **状态**：草案 · 待评审
> **关联**：ai-context.md §4.2 config-service 使用方式缺失
> **适用范围**：config-service 配置中心的使用、集成与运维

---

## 0. 一句话

> config-service 是平台的配置中心，提供命名空间隔离的 key-value 配置存储，支持热更新（watch）和敏感配置掩码。

---

## 1. 概述

### 1.1 什么是 config-service

config-service 是一个轻量级的配置管理服务，核心功能：
- **命名空间隔离**：不同租户/环境使用不同 namespace
- **Key-Value 存储**：配置项以 namespace + key 唯一标识
- **热更新**：通过 SSE endpoint 订阅配置变更，无需重启服务
- **访问控制**：通过 `CFG_CONFIG_REVEAL` 权限控制敏感配置访问

### 1.2 架构

```
应用服务 → GET/PUT /v1/cfg/namespaces/{ns}/configs/{key}
              ↓
         config-service (MySQL)
              ↓
         SSE /v1/cfg/namespaces/{ns}/watch
```

### 1.3 当前接入状态

| 服务 | 接入状态 | 用途 |
|------|----------|------|
| token-service | ✅ 已接入（Phase 6-B dogfooding） | 存储 JWT 配置、限流配置 |
| 其他 Rust 服务 | ❌ 待接入 | 存储各自配置 |
| Java 服务 | ❌ 待接入 | 存储各自配置 |

---

## 2. API 参考

### 2.1 基础信息

- **Base URL**：`http://localhost:8087`（本地 cargo run）或 `http://config-service:8080`（容器内）
- **认证**：通过 api-gateway 的 JWT 鉴权
- **响应格式**：JSON

### 2.2 端点列表

| 方法 | 路径 | 说明 | 认证 |
|------|------|------|------|
| GET | `/v1/cfg/namespaces` | 列出所有命名空间 | ✅ |
| POST | `/v1/cfg/namespaces` | 创建命名空间 | ✅ |
| GET | `/v1/cfg/namespaces/{ns}/configs` | 列出配置项 | ✅ |
| POST | `/v1/cfg/namespaces/{ns}/configs` | 创建配置项 | ✅ |
| GET | `/v1/cfg/namespaces/{ns}/configs/{key}` | 获取配置项 | ✅ |
| PUT | `/v1/cfg/namespaces/{ns}/configs/{key}` | 更新配置项 | ✅ |
| DELETE | `/v1/cfg/namespaces/{ns}/configs/{key}` | 删除配置项 | ✅ |
| GET | `/v1/cfg/namespaces/{ns}/watch` | SSE 热更新订阅 | ✅ |
| GET | `/health` | 健康检查 | ❌ |
| GET | `/metrics` | Prometheus 指标 | ❌ |

### 2.3 请求/响应示例

#### 创建命名空间

```bash
curl -X POST http://localhost:8087/v1/cfg/namespaces \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "production",
    "description": "生产环境配置"
  }'
```

#### 创建配置项

```bash
curl -X POST http://localhost:8087/v1/cfg/namespaces/production/configs \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "key": "jwt.access_token_ttl",
    "value": "3600",
    "value_type": "integer",
    "description": "Access token 有效期（秒）",
    "is_secret": false
  }'
```

#### 获取配置项（含敏感掩码）

```bash
# 获取普通配置
curl http://localhost:8087/v1/cfg/namespaces/production/configs/jwt.access_token_ttl \
  -H "Authorization: Bearer <token>"
# 响应：
# {"key":"jwt.access_token_ttl","value":"3600","is_secret":false,...}

# 获取敏感配置（需要 CFG_CONFIG_REVEAL 权限）
curl http://localhost:8087/v1/cfg/namespaces/production/configs/db.password \
  -H "Authorization: Bearer <token>"
# 有权限：{"key":"db.password","value":"actual_password","is_secret":true,...}
# 无权限：{"key":"db.password","value":"********","is_secret":true,...}
```

#### 更新配置项

```bash
curl -X PUT http://localhost:8087/v1/cfg/namespaces/production/configs/jwt.access_token_ttl \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "value": "7200",
    "description": "更新为 2 小时"
  }'
```

---

## 3. 命名空间隔离

### 3.1 命名空间设计

推荐命名空间按环境 + 租户划分：

```
production        # 生产环境共享配置
production-tenant-a  # 租户 A 生产配置
production-tenant-b  # 租户 B 生产配置
staging           # 预发环境
development       # 开发环境
```

### 3.2 Namespace 权限

```bash
# 创建 namespace 时指定 owner
POST /v1/cfg/namespaces
{
  "name": "production-tenant-a",
  "owner": "tenant-a-id",
  "description": "租户 A 生产配置"
}
```

---

## 4. 敏感配置处理（is_secret）

### 4.1 什么是 is_secret

`is_secret: true` 的配置项在 API 响应中会被掩码为 `********`，除非调用者持有 `CFG_CONFIG_REVEAL` 权限。

### 4.2 权限要求

| 操作 | 所需权限 |
|------|----------|
| 读取普通配置 | `CFG_CONFIG_READ` |
| 读取敏感配置（掩码） | `CFG_CONFIG_READ` |
| 读取敏感配置（明文） | `CFG_CONFIG_READ` + `CFG_CONFIG_REVEAL` |
| 创建/更新/删除配置 | `CFG_CONFIG_WRITE` |

### 4.3 权限码注册

`CFG_CONFIG_REVEAL` 权限码已在 `contracts/permissions.yaml` 注册。

### 4.4 使用场景

适合存储：
- 数据库密码
- API 密钥
- 第三方服务凭证
- 任何不希望在日志中明文出现的配置

---

## 5. 热更新（Watch）

### 5.1 SSE 订阅

```bash
# 订阅 production namespace 的配置变更
curl -N http://localhost:8087/v1/cfg/namespaces/production/watch \
  -H "Authorization: Bearer <token>"
```

### 5.2 响应格式

```text
event: config_changed
data: {"key":"jwt.access_token_ttl","action":"update","value":"7200","changed_at":"2026-07-13T10:30:00Z"}

event: config_deleted
data: {"key":"legacy_config","action":"delete","changed_at":"2026-07-13T10:31:00Z"}
```

### 5.3 Rust 代码示例

```rust
use reqwest::EventSource;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct ConfigChange {
    key: String,
    action: String,
    value: String,
    changed_at: String,
}

async fn watch_configs(ns: &str, token: &str) {
    let url = format!(
        "http://config-service:8080/v1/cfg/namespaces/{}/watch",
        ns
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    let mut es = EventSource::new(response);
    while let Some(event) = es.next().await {
        match event {
            Ok(event) => {
                if let Some(data) = event.data().parse::<ConfigChange>().ok() {
                    println!("Config changed: {:?}", data);
                    // 重新加载配置
                }
            }
            Err(e) => {
                eprintln!("Watch error: {}", e);
            }
        }
    }
}
```

---

## 6. 服务集成

### 6.1 token-service 集成示例

token-service 已在 Phase 6-B 接入 config-service：

```rust
// services/token-service/src/config.rs
use digua_common_config::{ConfigServiceClient, Namespace};

// 初始化配置服务客户端
let config_client = ConfigServiceClient::new(
    "http://config-service:8080",
    token.clone(),
);

// 读取配置
let access_token_ttl: i64 = config_client
    .get(Namespace::Production, "jwt.access_token_ttl")
    .await?
    .parse()
    .unwrap_or(3600);
```

### 6.2 新服务接入

#### 1. 添加依赖

```toml
# Cargo.toml
digua_common-config = { path = "../../libs/digua_common-config" }
```

#### 2. 初始化客户端

```rust
// src/config.rs
use digua_common_config::ConfigServiceClient;

pub struct AppConfig {
    pub client: ConfigServiceClient,
    pub namespace: String,
}

impl AppConfig {
    pub async fn new(base_url: &str, token: &str, namespace: &str) -> Self {
        Self {
            client: ConfigServiceClient::new(base_url, token),
            namespace: namespace.to_string(),
        }
    }

    pub async fn get<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.client
            .get(&self.namespace, key)
            .await
            .ok()?
            .parse()
            .ok()
    }
}
```

#### 3. 启动时加载 + Watch 热更新

```rust
// src/main.rs
use digua_common_config::ConfigWatcher;

#[tokio::main]
async fn main() {
    let config = AppConfig::new(
        "http://config-service:8080",
        token,
        "production",
    )
    .await;

    // 启动时加载配置
    let jwt_ttl: i64 = config.get("jwt.access_token_ttl").await.unwrap_or(3600);

    // 启动 Watch 热更新
    let config_clone = config.clone();
    tokio::spawn(async move {
        let mut watcher = ConfigWatcher::new(config_clone).await;
        watcher.watch().await;
    });

    // 启动 HTTP 服务器...
}
```

---

## 7. SDK / 客户端

### 7.1 HTTP 客户端（任何语言）

```bash
# 获取配置
curl http://config-service:8080/v1/cfg/namespaces/{ns}/configs/{key} \
  -H "Authorization: Bearer <token>"

# 更新配置
curl -X PUT http://config-service:8080/v1/cfg/namespaces/{ns}/configs/{key} \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"value":"new_value"}'
```

### 7.2 Java SDK（待实现）

```java
// 计划中的 Java SDK
import com.coder.config.ConfigClient;

ConfigClient client = ConfigClient.builder()
    .baseUrl("http://config-service:8080")
    .token(System.getenv("BEARER_TOKEN"))
    .build();

// 读取
String value = client.get("production", "jwt.access_token_ttl");

// Watch
client.watch("production", event -> {
    System.out.println("Config changed: " + event.getKey());
});
```

---

## 8. 运维

### 8.1 健康检查

```bash
curl http://localhost:8087/health
# 响应：{"status":"ok"}
```

### 8.2 指标

```bash
curl http://localhost:8087/metrics
# 暴露 Prometheus 格式指标
```

### 8.3 迁移状态

```bash
curl http://localhost:8087/internal/migrations/state
```

### 8.4 常见问题

| 问题 | 原因 | 解决 |
|------|------|------|
| 401 Unauthorized | token 无效或过期 | 刷新 token |
| 403 Forbidden | 缺少 `CFG_CONFIG_READ` 权限 | 检查角色权限 |
| 配置读取不到 | namespace 或 key 不存在 | 检查创建是否成功 |
| Watch 不推送 | SSE 连接断开 | 重连 SSE |

---

## 9. 关联

- `services/config-service/` — config-service 源码
- `libs/digua_common-config/` — 公共配置客户端库
- `docs/superpowers/specs/2026-06-29-phase6b-config-service-dogfood-design.md` — dogfooding spec
- `contracts/permissions.yaml` — 权限码定义

---

> **最后更新**：2026-07-13
