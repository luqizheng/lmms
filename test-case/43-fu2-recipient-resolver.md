# 43-fu2 · role/dept 接收人解析端到端验证

Phase 4.3-fu2 端到端验证：发布公告给 role r1（10 人）→ 这 10 人 `notification/mine` 看到公告。

## 步骤

1. admin 创建 role r1，加入 10 个 user（u1..u10）
2. 创建 announcement，受众 `["r1"]` (10 个 user 都属于 r1)
3. publish： `POST /notification/announcements/:id/publish`
4. `mysql> SELECT count(*) FROM ntf_announcement_deliveries WHERE announcement_id = ?;`
   预期: 10
5. 任一 user (u1) 登录调 `GET /notification/mine`，预期: 看到该 announcement
