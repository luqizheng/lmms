# 升级指南

## 在线升级

```bash
# 拉取新版本镜像
make upgrade TAG=v1.1.0

# 验证
make verify
```

`make upgrade` 会：
1. 从 registry 拉取新 tag 的所有镜像
2. 重启所有容器（保留 volumes）
3. 不影响数据卷

## 离线升级

```bash
# 1. 在有网环境下载并导出新版本镜像
docker pull zhcoder-docker-registry.com:8000/micro-server/api-gateway:v1.1.0
docker save \
  zhcoder-docker-registry.com:8000/micro-server/api-gateway:v1.1.0 \
  -o images/api-gateway-v1.1.0.tar

# 对所有服务重复以上步骤...

# 2. 传输镜像 tar 到目标机器
scp images/*.tar user@target:/opt/coder-micro-services-v1.0.0/images/

# 3. 加载镜像
bash bin/load-images.sh

# 4. 修改 IMAGE_TAG 并重启
sed -i 's/IMAGE_TAG=v1.0.0/IMAGE_TAG=v1.1.0/' .env
make down && make up
```

## 滚动重启

如需在不更新镜像的情况下重启所有服务：

```bash
make down && make up
```

## 回滚

```bash
# 改回旧 IMAGE_TAG
sed -i 's/IMAGE_TAG=v1.1.0/IMAGE_TAG=v1.0.0/' .env
make down && make up
```

> 注意：数据库迁移（MySQL init scripts）仅在数据目录为空时执行。降级后如果 schema 不兼容可能需要 `make reset-data`（会清除数据）。
