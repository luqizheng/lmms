.PHONY: help secrets up down status logs verify reset-data seed-demo build-frontend load-images upgrade

REGISTRY ?= zhcoder-docker-registry.com:8000
IMAGE_TAG ?= v1.0.0

help:
	@echo "coder-micro-services v1.0.0 — 可用命令："
	@echo ""
	@echo "  make secrets         生成 JWT 密钥 + 随机密码，初始化 .env"
	@echo "  make up              启动所有服务（自动构建 frontend 镜像）"
	@echo "  make down            停止所有容器（保留数据卷）"
	@echo "  make status          列出所有容器状态"
	@echo "  make logs [svc=]     查看日志（可指定服务名）"
	@echo "  make verify          端到端冒烟测试"
	@echo "  make reset-data      清除所有数据并重新初始化"
	@echo "  make seed-demo       写入 demo 数据"
	@echo "  make build-frontend 构建 frontend 镜像"
	@echo "  make load-images     从离线 tar 加载 Docker 镜像（air-gap）"
	@echo "  make upgrade TAG=v1.x 升级到指定版本"
	@echo ""
	@echo "环境变量：REGISTRY=${REGISTRY}  IMAGE_TAG=${IMAGE_TAG}"

secrets:
	@bash bin/generate-secrets.sh

up:
	@bash bin/start.sh

down:
	@bash bin/stop.sh

status:
	@bash bin/status.sh

logs:
	@bash bin/logs.sh $(svc)

verify:
	@bash bin/verify.sh

reset-data:
	@bash bin/reset-data.sh

seed-demo:
	@bash bin/seed-demo.sh

build-frontend:
	@bash bin/build-frontend.sh

load-images:
	@bash bin/load-images.sh

upgrade:
	@bash bin/upgrade.sh $(TAG)