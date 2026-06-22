# frontend 前端服务规范

## 必需组件

- [x] Dockerfile
- [x] nginx.conf
- [x] package.json
- [x] README.md
- [x] 国际化配置（locales/）
- [x] 路由配置（router/）

## 配置选项

- NODE_ENV: 运行环境（development/production/staging）
- VITE_API_BASE: API 基础路径，默认 `/api`
- VITE_PUBLIC_PATH: 公共路径，默认 `/`
- VITE_ROUTER_HISTORY: 路由模式（hash/h5）

## 项目结构

### 目录说明

| 目录 | 说明 |
|------|------|
| src/api/ | API 客户端与类型定义 |
| src/components/ | 通用组件库 |
| src/layout/ | 布局组件 |
| src/router/ | 路由配置 |
| src/store/ | 状态管理 |
| src/views/ | 页面视图 |
| src/utils/ | 工具函数 |
| src/style/ | 全局样式 |
| locales/ | 国际化资源 |
| mock/ | Mock 数据 |
| build/ | 构建配置 |

### 路由模块

| 文件 | 路径前缀 | 说明 |
|------|----------|------|
| router/modules/home.ts | / | 首页 |
| router/modules/account.ts | /account-settings | 账户设置 |
| router/modules/system.ts | /system | 系统管理 |
| router/modules/notification.ts | /notification | 通知管理 |
| router/modules/monitor.ts | /monitor | 监控中心 |
| router/modules/permission.ts | /permission | 权限演示 |
| router/modules/error.ts | /error | 错误页面 |

### 布局组件

| 组件 | 说明 |
|------|------|
| lay-frame | 主框架容器 |
| lay-navbar | 顶部导航栏 |
| lay-sidebar | 侧边栏菜单 |
| lay-content | 内容区域 |
| lay-footer | 页脚 |

## API 客户端

前端通过 `src/utils/http/index.ts` 统一调用后端服务，所有接口遵循 `/api/v1/<service>/<resource>` 格式。

### 已集成服务

| 服务 | 路径前缀 | API 文件 |
|------|----------|----------|
| iam-service | /api/v1/iam | src/api/iam.ts |
| token-service | /api/v1/token | src/api/token.ts |
| audit-service | /api/v1/audit | src/api/audit.ts |
| notification-service | /api/v1/notification | src/api/notification.ts |

## 状态管理

基于 Pinia 的模块化状态管理：

| 模块 | 说明 |
|------|------|
| user | 用户信息 |
| permission | 权限路由 |
| app | 应用配置 |
| settings | 布局设置 |
| notification | 通知状态 |
| multiTags | 多标签页 |

## 指令系统

| 指令 | 说明 |
|------|------|
| v-auth | 页面鉴权 |
| v-perms | 按钮权限 |
| v-copy | 复制功能 |
| v-ripple | 涟漪效果 |
| v-longpress | 长按事件 |

## 构建命令

| 命令 | 说明 |
|------|------|
| pnpm install | 安装依赖 |
| pnpm run dev | 开发模式 |
| pnpm run build | 生产构建 |
| pnpm run build:staging | 预发布构建 |
| pnpm run lint | ESLint 检查 |
| pnpm run typecheck | TypeScript 检查 |

## 环境变量

开发环境 `.env.development`：
```bash
NODE_ENV=development
VITE_API_BASE=/api
VITE_PUBLIC_PATH=/
VITE_ROUTER_HISTORY=hash
```

生产环境 `.env.production`：
```bash
NODE_ENV=production
VITE_API_BASE=/api
VITE_PUBLIC_PATH=/
VITE_ROUTER_HISTORY=h5
```

## 身份认证规范

### 用户信息获取

前端**必须**从 `Authorization: Bearer <JWT>` 请求头中的 JWT payload 提取用户身份信息，**禁止**使用或读取 `X-User-Id`、`X-User-Roles`、`X-User-Dept` 等 X-User-* headers。

### JWT Payload 结构

从 JWT payload 中提取以下 claims：

| Claim | 说明 | 必填 |
|-------|------|------|
| `sub` | 用户 ID | 是 |
| `user_code` | 用户编码 | 否 |
| `dept_id` | 部门 ID | 否 |
| `roles` | 角色列表（JSON array） | 否 |
| `permissions` | 权限列表（JSON array） | 否 |

### 实现方式

使用 `@/utils/auth.ts` 中的工具函数解析 JWT：

```typescript
import { storageLocal, userKey, type DataInfo } from "@/utils/auth";

const userInfo = storageLocal().getItem<DataInfo<number>>(userKey);
const userId = userInfo?.userId;
const roles = userInfo?.roles;
```

### 安全原则

- 前端**不验证** JWT 签名，仅解码 payload
- 信任前提：api-gateway 已完成 RSA 验签和黑名单检查
- 所有 API 请求必须携带 `Authorization: Bearer <JWT>` header

### WebSocket 连接

WebSocket 连接也必须通过 `Authorization` header 传递 JWT：

```typescript
const socket = new WebSocket(wsUrl);
socket.onopen = () => {
  socket.send(JSON.stringify({
    token: localStorage.getItem("token")
  }));
};
```

### 违规处理

- 代码审查时发现使用 X-User-* headers 应拒绝合并
- 现有代码应逐步迁移到 JWT 解析方式

## 部署说明

前端构建产物通过 Docker 镜像部署，容器内使用 nginx 作为静态文件服务器。nginx 配置位于 `nginx.conf`，负责：
- 静态资源服务
- API 代理到 api-gateway
- SPA 路由 fallback