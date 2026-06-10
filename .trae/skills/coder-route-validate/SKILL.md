---
name: coder-route-validate
description: 校验/热重载 api-gateway 路由配置（routes/*.router.toml）。用于「校验路由」「reload」「改路由后验证」等意图。
---

# coder-route-validate

## 何时用

- 新增/修改 `docker/config/api-gateway/routes/*.router.toml` 后
- 部署前 CI 检查
- 用户说「validate 路由」「reload gateway」时

## 步骤

### 1. 离线校验（不启 gateway）

```bash
cd d:\projects\coder-micro-services
cargo run -p api-gateway -- validate --routes docker/config/api-gateway/routes
```

退出码 0 = 通过；1 = 失败，stderr 列出原因。

### 2. 启动 gateway

```bash
cargo run -p api-gateway -- serve --routes docker/config/api-gateway/routes --admin-port 9090
```

启动日志打印 `N routes` 数量。

### 3. 热重载（Windows 走 HTTP，Linux 走 SIGHUP）

**Windows / 跨平台**：

```powershell
curl -X POST http://localhost:9090/__admin/reload
```

成功：`200 OK`，body `{"reloaded": N}`。
失败：`500`，body `{"error": "..."}`，旧表保持。

**Linux**：

```bash
pid=$(pgrep -f "api-gateway serve")
kill -HUP "$pid"
```

### 4. 烟测

```bash
curl -i http://localhost:3000/health
```

期望 `200 OK`。

## 路由字段

详见 `docs/guides/api-gateway-routing.md`。

冲突检测：两个文件声明相同 `match` → 启动/重载报错 `duplicate match pattern: /xxx/**`。

## rewrite 规则类型

`strip_prefix` / `redirect` / `header_set` / `header_add` / `path_param`

未知类型 → 启动/重载报错 `unknown variant unknown_op`。
