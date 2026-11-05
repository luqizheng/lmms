# Changelog

## v1.7.0 (2026-11-05)

# v1.7.0

### Added
- 服务层全量 PostgreSQL 支持（S0~S6）：双方言运行时与迁移、PG 镜像变体与 PG compose 模板（`:<ver>-pg` / `:pg-latest`），transform-compose 支持方言 tag 后缀
- token-service 全部签发 JWT 新增 `purpose` claim：签发通道权威声明（封闭枚举 `user_login` / `api_key` / `auth_code` / `service` / `resource`），由通道决定、请求不可自报；OAuth refresh 继承通道，legacy 令牌缺失即旧推断（fail-closed 场景一律拒绝）；下游据此区分人类登录令牌与机器兑换令牌（ai-pm 正式确认 fail-closed 的签发侧前提）

### Fixed
- 发布链路：镜像 .env 驱动化、双方言镜像构建、compose 模板双变体发布、模板包排除凭据文件
- rust 发布按依赖方向排序，支持断点续推并复原 Cargo.lock

### Changed
- 版本账本 1.6.0 → 1.7.0（镜像统一 1.7.0；PG 变体 1.7.0-pg）

## v1.6.0 (2026-10-30)

### Added
- PRISM 前端壳：全新设计系统（token 层 / 排版基线 / 设计约束校验器接入 CI）替换 pure-admin
- PRISM 组件库：Button/Input/Textarea/Label/FormField/Card/Badge/Skeleton/Table（44px 行高/数值列等宽）/交互组件集（dialog/sheet/command 等 16 个）
- 框架系统页：用户/角色/菜单/审计日志/配置项/公告/权限/组织架构/会话/文件/令牌客户端
- shell 能力：tab 右键菜单（关闭当前/左侧/右侧/其他/全部）、tab 滚动导航、bot icon 菜单注册、用户目录与 UserLabel（审计/会话页显示昵称）、PageHeader + useToast、动态菜单/响应式/登录页/i18n
- `cms-release` 发布 CLI：发布镜像 / 打离线包 / 版本收口 / 下游升级统一入口
- `@digua/contracts` 与 `@digua/http` 前端基础包（含发布前检查）
- i18n 文案按命名空间分文件（locales/<locale>/<ns>.yaml），业务模块各占各的文件

### Fixed
- 分发包升级链路：`bin/upgrade.js <ver>` 现在真正改写 `.env` 的 IMAGE_TAG 并以其执行 compose pull/up（原先版本参数仅用于 echo，升级为空操作）
- 双角色镜像的健康检查按角色分流
- deep-link 刷新 404（router history 支持环境变量切换 h5 模式）
- 嵌套菜单子路由保持绝对路径
- http token 刷新死锁与无限重放；菜单协议对齐后端；壳可滚动
- 侧栏菜单节奏（分组间距与同级间距分离）

### Changed
- 版本账本收口：dist Makefile IMAGE_TAG 1.0.0 → 1.6.0，与 git tag 对齐
- 前端为源码分发（不含 node_modules），含 pnpm-lock.yaml（不变）

### Notes
- 镜像版本统一 1.6.0；支持原地升级（保留 volumes），升级入口 `make upgrade TAG=v1.6.0` 或分发包 `bin/upgrade.js`

## v1.0.0 (2026-07-12)

### Added
- 完整离线分发包，含 docker-compose.yml 统一入口
- 8 个 Rust 微服务 + Vue 3 前端 + 可观测性栈
- Docker 镜像预加载脚本（air-gap 支持）
- JWT RS256 密钥自动生成脚本
- 端到端冒烟测试脚本（verify.sh）
- MySQL/Redis/Consul/RabbitMQ 基础设施 compose
- Prometheus + Grafana + Jaeger + OTEL 可观测性栈
- 统一 Makefile 入口（secrets/up/down/logs/verify/upgrade）

### Infrastructure
- MySQL 8.0（含 init-mysql 初始化脚本）
- Redis 7（密码认证）
- Consul 2.0（服务注册发现）
- RabbitMQ 3.13（management + definitions.json）
- Registrator（自动注册容器到 Consul）

### Services
| 服务 | 端口 |
|------|------|
| api-gateway | 8080 |
| config-service | 8081 |
| file-service | 8082 |
| token-service | 8083 |
| iam-service | 8084 |
| notification-service | 8085 |
| audit-service | 8086 |
| prue-backend | 8087 |

### Observability
- Prometheus v2.51.0（15d retention）
- Grafana 10.4.0（provisioned dashboards + datasources）
- Jaeger 1.55（OTLP 接收 + spanmetrics）
- OTEL Collector 0.96（traces → Jaeger, metrics → Prometheus）
- Consul-template 0.32（nginx canary 路由）

### Security
- JWT RS256 密钥自动生成（2048-bit RSA）
- 随机密码自动生成（32-char hex）
- bootstrap token 每服务独立
- 所有 secrets 不写入镜像，通过 .env 挂载

### Notes
- 前端为源码分发（不含 node_modules），含 pnpm-lock.yaml
- 镜像版本统一 v1.0.0
- 支持原地升级（保留 volumes）
- 数据目录 secrets/ 不提交 git