// notification-service DTO 镜像。字段名以后端 serde 序列化为唯一事实源：
//   - services/notification-service/openapi.yaml + src/models.rs
// 成功响应为裸 JSON；错误体为 {error} / {error:"upstream",code,message}。
// 注意不对称：pinned/popover 创建时传 boolean，响应里是 int32（0/1）——照实镜像。
export {};
//# sourceMappingURL=types.js.map