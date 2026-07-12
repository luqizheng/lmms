# 备份与恢复

## 备份数据卷

Docker volumes 数据保存在 `/var/lib/docker/volumes/`（Linux）或通过 `docker volume inspect` 查看。

### 方式一：直接备份 volumes

```bash
# 查看所有 volumes
docker volume ls

# 备份指定 volume
docker run --rm \
  -v coder-micro-services_mysql_data:/src/mysql \
  -v $(pwd):/dest \
  alpine \
  tar -czf /dest/mysql_backup_$(date +%Y%m%d).tar.gz -C /src/mysql .
```

### 方式二：使用 docker-compose

```bash
# 停止所有容器
make down

# 备份整个数据目录（Docker 默认路径）
sudo rsync -avz /var/lib/docker/volumes/ /path/to/backup/volumes/

# 重启
make up
```

## 备份配置和密钥

```bash
# 备份 .env 和 secrets 目录
tar -czf config_backup_$(date +%Y%m%d).tar.gz \
  .env \
  secrets/
```

## 恢复

```bash
# 1. 停止服务
make down

# 2. 恢复 MySQL volume
docker volume rm coder-micro-services_mysql_data
docker volume create coder-micro-services_mysql_data
docker run --rm \
  -v coder-micro-services_mysql_data:/dest \
  -v $(pwd)/mysql_backup_20260101.tar.gz:/src/backup.tar.gz \
  alpine \
  tar -xzf /src/backup.tar.gz -C /dest

# 3. 恢复 .env 和密钥
tar -xzf config_backup_20260101.tar.gz

# 4. 重启
make up
```

## 定时备份建议

```bash
# crontab -e，每天凌晨 3 点备份
0 3 * * * cd /opt/coder-micro-services && make backup
```

## 重要文件清单

| 文件/目录 | 备份优先级 | 说明 |
|-----------|-----------|------|
| `secrets/jwt_private.pem` | **高** | JWT 私钥，丢失后所有 token 需重新签发 |
| `.env` | **高** | 所有密码和密钥 |
| `docker volumes` | **高** | MySQL/Redis/RabbitMQ 数据 |
| `config/` | 中 | 路由/限流配置 |
| `init-mysql/` | 低 | 仅首次初始化需要 |
