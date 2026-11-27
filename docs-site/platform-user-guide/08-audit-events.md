# 08. 审计与领域事件

## 8.1 审计服务

audit-service 用于记录和查询操作日志、登录日志，并消费 IAM 等服务产生的领域事件。审计数据是合规和问题追踪依据，不应作为业务状态的唯一数据源。

管理端展示审计记录时，应保留服务端返回的操作者、动作、对象、时间、来源和 request/trace 标识（仅展示 API 实际提供字段）。日志查询和导出应受专门权限控制，例如权限字典中的 `AUD_LOG_READ`、`AUD_LOG_EXPORT`。

## 8.2 事件总线

服务间异步领域事件采用 RabbitMQ + transactional outbox：

```text
Business DB transaction
  |-- write business row
  |-- write *_outbox_events row
          |
          v
    outbox publisher
          |
          v
       RabbitMQ
          |
          v
    consumer service
```

outbox 保证业务写入与待发布事件在同一数据库事务内提交，避免业务成功但事件完全丢失。publisher 会异步发布，消费端必须考虑重复投递、重试和幂等。

## 8.3 主要事件示例

实际事件以 `contracts/events.yaml` 为准，当前包括：

- `iam.user.created` / `iam.user.deleted`
- `iam.user.role.assigned` / `iam.user.role.unassigned`
- `iam.user.dept.changed`
- `iam.login.succeeded` / `iam.login.failed`
- `iam.permission.changed`
- `notification.announcement.published`
- `file.uploaded` / `file.deleted`

事件字段、schema_version、必填字段和 routing key 必须从契约读取。不要根据事件名称猜测 payload。

## 8.4 消费者设计

- 以事件 ID 或业务唯一键实现幂等。
- 处理 schema_version，未知版本应安全拒绝或进入隔离队列，不能静默错误解析。
- 区分可重试错误与永久错误。
- 不假设事件严格全局有序；按业务键处理乱序。
- 事件消费失败要有可观测性和重试/死信处理。
- 不把 RabbitMQ broker 暴露给浏览器或普通客户端。

## 8.5 审计与事件的区别

| 审计日志 | 领域事件 |
|---|---|
| 回答“谁在什么时候做了什么” | 通知其他服务“某个业务事实发生了” |
| 面向追责、查询、留存 | 面向异步协作和投影更新 |
| 通常不应被业务消费者当命令 | 消费端可更新本地投影/触发后续工作 |
| 不替代业务表 | 不替代业务表 |

## 8.6 AI-PM 使用建议

AI-PM 可以把 LMMS 用户身份、文件对象 ID、通知公告和审计记录作为平台基础设施集成；但 AI-PM 自己的任务、审批、评论、Agent 执行等领域事件仍应由 AI-PM 自己定义。不要将 LMMS IAM 或公告模型扩展成 AI-PM 领域模型的替代品。
