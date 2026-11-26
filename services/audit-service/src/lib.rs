pub mod config;
pub mod consumer;
pub mod db;
pub mod error;
pub mod models;
pub mod openapi;
pub mod redis_cli;
pub mod risk;
pub mod routes;
pub mod services;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use routes::{create_state, routes};

// 显式引用（匿名 use）：本 crate 的方言 feature 需要转发 `digua_common` 的
// 同名 feature（见 Cargo.toml `[features]`），而 cargo 的 unused_dependencies
// lint 不把 feature 转发计为使用，需在代码里留一个真实引用点。
use digua_common as _;
