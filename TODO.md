# TODO

（已清空：原 P1.1 prue_backend 草稿并发控制 与 P1.2 sanitize active routes 输出
已于 2026-06-25 完成，详见
`docs/superpowers/specs/2026-06-25-prue-backend-concurrency-design.md`
与对应实现 commit。）

---

## 新员工手册待补充（docs/onboarding/）

以下 TODO 项来自 `docs/onboarding/` 系列文档，需要人工补充：

### 基础设施与权限

- [ ] **内网仓库凭证申请**：Docker Registry (`zhcoder-docker-registry.com:8000`)、Maven (`http://mvn.kudi.repos`)、NuGet (`http://nuget.kudi.repos`) 的账号申请流程 — **待运维补充**
- [ ] **生产数据迁移 SOP**：生产环境数据库迁移的备份、回滚、灰度验证标准流程 — **待运维补充**
- [ ] **集中式日志收集**：生产环境日志收集方案（ELK / Loki / 阿里云 SLS）、查询入口、权限申请 — **待运维补充**

### 团队规范

- [ ] **求救渠道**：技术问题 IM 群号/频道名、on-call 值班表、紧急故障联系人、升级路径 — **待团队负责人补充**
- [ ] **commit message 规范**：是否使用 Conventional Commits、PR 标题格式、是否需关联 issue/ticket — **待规范人补充**

### 技术细节

- [ ] **config-service 使用方式**：如何通过 config-service API 读写配置、各服务如何集成、配置优先级 — **待熟悉 config-service 的开发者补充**
- [ ] **前端登录态刷新机制细节**：Token 刷新触发条件、并发刷新处理、自动登出策略的完整实现 — **待前端负责人确认（目前已基于代码写出基本逻辑，需确认边界情况）**
