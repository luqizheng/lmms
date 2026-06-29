# 65 — Config-Service Dogfood 手工验收

## 前置条件

- config-service 运行在 `http://localhost:6006`
- token-service 已编译（`cargo build -p token-service --features config-service`）

---

## 1. 建立 namespace 并写入配置

```bash
# 创建 namespace
curl -X POST http://localhost:6006/config/namespace \
  -H "Content-Type: application/json" \
  -d '{"id":"token-service-dev","name":"token-service-dev","description":"token-service dev config"}'

# 写入配置项（嵌套路径与 TokenServerConfig 对齐）
curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"server.host","value":"0.0.0.0"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"server.port","value":"4000"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"database.db_type","value":"mysql"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"database.url","value":"mysql://token_service:token_pwd@localhost:3306/token_service"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"database.max_connections","value":"16"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"issuer","value":"superpowers-token-service"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"jwt_private_key_path","value":"keys/private.pem"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"jwt_public_key_path","value":"keys/public.pem"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"default_access_ttl_sec","value":"3600"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"default_refresh_ttl_sec","value":"2592000"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"default_resource_ttl_sec","value":"300"}'

curl -X POST http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"user_service_url","value":"http://user-service:8080"}'

# 验证 export 端点
curl http://localhost:6006/config/export/token-service-dev | jq
```

**预期**：export 返回 12 个 key-value 对。

---

## 2. 启动验证（config-service 可用）

```bash
cd services/token-service
CONFIG_SERVICE_URL=http://localhost:6006 \
CONFIG_SERVICE_NAMESPACE=token-service-dev \
cargo run
```

**预期**：
- 启动日志显示 `server.host: 0.0.0.0` / `server.port: 4000`
- token-service 正常监听 4000 端口
- `curl http://localhost:4000/health` 返回 `{"status":"ok"}`

---

## 3. Fallback 测试（config-service 不可用）

```bash
# 停掉 config-service
# 然后启动 token-service（保留 config/config.yaml 本地文件）
cd services/token-service
CONFIG_SERVICE_URL=http://localhost:6006 \
CONFIG_SERVICE_NAMESPACE=token-service-dev \
cargo run
```

**预期**：
- 日志出现 `WARN config-service fetch failed, using local config`
- token-service 仍正常启动，使用本地 config/config.yaml 的值

---

## 4. 热更新测试（30s 轮询）

```bash
# 1. 启动 config-service + token-service
# 2. 修改 config-service 中的 default_access_ttl_sec
curl -X PUT http://localhost:6006/config/item \
  -H "Content-Type: application/json" \
  -d '{"namespace_id":"token-service-dev","key":"default_access_ttl_sec","value":"7200"}'

# 3. 等待 30s 轮询周期
# 4. 检查 token-service 日志
```

**预期**：
- 日志出现 `INFO hot config updated via config-service poll`
- token-service 使用新的 `default_access_ttl_sec=7200`
