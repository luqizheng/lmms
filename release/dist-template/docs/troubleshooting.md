# 故障排查

## 服务启动失败

### 症状：容器一直 Restarting

```bash
# 查看容器日志
docker logs coder-micro-services-<service-name>

# 常见原因：
# 1. 依赖的服务未就绪 → 检查 consul / mysql / redis 是否 healthy
make status

# 2. 密码错误（MySQL / Redis）→ 检查 .env 中密码是否正确
# 3. 端口冲突 → docker ps 看哪些端口被占用
netstat -tlnp | grep -E "8080|8081|8082|8083|8084"
```

### 症状：MySQL 连接失败

```bash
# 检查 MySQL 是否就绪
docker exec coder-micro-services-mysql mysqladmin ping -h localhost -uroot -p

# 查看 MySQL 日志
docker logs coder-micro-services-mysql
```

### 症状：Redis 连接失败

```bash
# 测试 Redis 连接
docker exec coder-micro-services-redis redis-cli -a <password> ping
```

## 前端无法访问

```bash
# 检查 nginx 是否正常运行
docker logs coder-micro-services-api-gateway

# 检查 API 网关是否 healthy
curl http://localhost:3000/health

# 检查前端配置是否正确
```

## 镜像加载失败（离线环境）

```bash
# 确认镜像已加载
docker images | grep micro-server

# 如果加载失败，手动逐个加载：
docker load -i images/<service-name>.tar
```

## JWT 登录失败

```bash
# 1. 确认 token-service 健康
curl http://localhost:8082/health

# 2. 确认公钥可获取
curl http://localhost:8082/.well-known/jwks.json

# 3. 确认 API 网关的 GW_JWKS_URL 指向正确地址
#    检查 docker-compose.yml 中 api-gateway 的 GW_JWKS_URL 环境变量
```

## 性能问题

```bash
# 查看资源使用
docker stats --no-stream

# 查看 Prometheus 指标
curl http://localhost:9090/api/v1/query?query=up

# 查看 Jaeger trace
# 访问 http://localhost:16686
```

## 数据重置

⚠️ 此操作会清除所有数据：

```bash
make reset-data
```

## 日志查看

```bash
# 所有服务实时日志
make logs

# 指定服务
make logs api-gateway
make logs token-service
make logs iam-service

# 最近 200 行
docker logs --tail 200 coder-micro-services-api-gateway
```
