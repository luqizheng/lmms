# ADR-006: API Gateway 长期技术策略

> 日期：2026-07-29
> 状态：已采纳
> 关联：
> - [ADR-001 Gateway 路由配置多文件拆分](./ADR-001-api-gateway-multi-file-router-config.md)
> - [可观测性指南](../guides/observability.md)
> - [灰度发布指南](../guides/canary-rollout.md)

## 上下文

`api-gateway` 是自研 Axum 服务，当前承载以下 6+ 职责：

| 职责 | 当前实现 | 风险 |
|------|---------|------|
| 路由转发 | `routes/*.router.toml` + SIGHUP 热重载 | 低 |
| JWT 验签（用户） | RSA 公钥验签 + exp/iat 检查 | 中（CVE 自修） |
| Service JWT 签发与分发 | 为下游服务生成临时 JWT | 中 |
| 限流 | Token bucket（per route, `rate-limits.toml`） | 低 |
| Trace 注入 | W3C TraceContext propagation | 低 |
| Consul 服务发现 | 动态 upstream 解析 | 中（依赖 Consul 可用性） |
| Canary 路由 | `consul-template` + nginx 配置渲染 | 中 |
| Perm epoch 检查 | Redis 权限版本号 | 低 |

网关是**每一个外部请求都经过的 SPOF**。它的稳定性和吞吐量直接决定全平台可用性。随着服务数量增长（当前 8 个），网关的复杂度持续上升。

## 决议

### 短期（1.0 阶段）：继续自研，加固现有网关

**决定**：1.0 之前不自研换网关，但做以下加固：

1. **多副本 + Nginx L4 前置**：
   - 已有多副本能力（`docker-compose scale`）
   - `infrastructure/config/nginx/nginx.conf.template` 已在项目中，评估将其扶正为生产入口 L4/L7 代理
   - Nginx 前置可提供：连接池复用、静态文件服务、更成熟的限流

2. **剥离非核心职责**：
   - `consul-template` 已独立运行（canary 路由），路由决策不经过网关代码
   - Service JWT 签发逻辑评估迁移到 `digua_common-jwt` lib，减少网关代码行数

3. **Gateway 专属 SLO**：99.9%（见 [SLO 文档](../slo.md)），错误预算驱动变更节奏

### 中期（2.0 阶段）：评估 Envoy 或 Pingora 作为 sidecar/ingress

| 方案 | 优点 | 缺点 | 适合场景 |
|------|------|------|---------|
| **继续自研** | 完全可控、Rust 生态、零外部依赖 | 自修所有 CVE、性能优化靠自己 | 团队有 Rust 能力，接受维护成本 |
| **Envoy** | 行业标准、成熟度极高、xDS 动态配置 | 学习曲线陡、C++ 调试门槛高、内存占用较大 | K8s/Istio 生态，需要高级流量管理 |
| **Pingora** | Rust 编写、Cloudflare 开源、性能优异 | 生态不成熟（2024 新项目）、社区较小 | 想保留 Rust 栈但不想自研 |

**2.0 决策点**：当以下任一条件触发时，正式启动网关替换评估：

- K3s 化部署上线（Envoy 在 K8s 生态中天然优势明显）
- 自研网关出现无法快速修复的严重 CVE 或性能瓶颈
- 业务需要 Istio 级别的流量管理（mTLS、fault injection、circuit breaking）

### 为什么不是现在切

| 因素 | 评估 |
|------|------|
| 团队带宽 | 1.0 上线前有 P0 安全项、CI 补齐优先于网关重写 |
| 风险 | 换网关是高风险操作，1.0 之前不该引入这种级别的变更 |
| 当前可用性 | 自研网关在 8 服务规模下运行稳定，无已知严重问题 |
| 学习成本 | Envoy xDS 配置体系与当前 router.toml DSL 差异巨大 |

## 后果

### 好处

- 1.0 交付路径不被网关迁移阻塞
- 自研网关的维护成本可预见（Rust + Axum，团队成员已熟悉）
- 2.0 决策有明确触发条件，不会无限期"评估"

### 代价

- 网关复杂度持续增长，直到 2.0 评估启动
- 自研网关每新增一个 CVE 需自行修复
- Envoy/Pingora 的迁移成本随时间推移不会降低

### 已知未做

- `infrastructure/config/nginx/` 到生产入口的正式扶正方案
- 网关性能基线（需 P1-5 负载摸底完成后记录到 `docs-site/perf-baseline.md`）
- Service JWT 签发逻辑从网关剥离的具体时间

## 变更时机

- **1.0 阶段**：不切网关，仅加固（多副本 + Nginx L4 前置评估）
- **2.0 启动信号**：K3s 部署就绪 OR 网关出现无法自修的严重 CVE
- **方案选择**：2.0 启动时另出 ADR-007 做 Envoy vs Pingora 的最终选型
