// Principal 已通过 jwt_ctx 实现 FromRequestParts，可直接在 handler 中提取
// 无需额外中间件

pub use jwt_ctx::Principal;