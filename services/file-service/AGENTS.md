# file-service 服务规范

## 必需组件

- Dockerfile ✓
- Swagger 配置（可配置启用/禁用）✓
- 健康检查端点（/health、/ready、/metrics）✓
- 数据库迁移文件（migrations/）✓
- README.md ✓
- tests/ 目录 ✓

## 配置选项

- swagger_enabled: 环境变量 SWAGGER_ENABLED 或默认 true
- health_check_required: 环境变量 HEALTH_CHECK_REQUIRED 或默认 true
- migrations_required: 环境变量 MIGRATIONS_REQUIRED 或默认 true

## package.json 脚本

- build:docker
- build:cargo

## AI 检查时机

