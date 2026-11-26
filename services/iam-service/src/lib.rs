pub mod auth;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod mail;
pub mod middleware;
pub mod models;
pub mod openapi;
pub use digua_common_event::outbox_archive;
pub mod routes;
pub mod service;
pub mod state;

pub use config::{load_config, load_config_with_env, Config};

// 显式引用（匿名 use）：本 crate 的方言 feature 需要转发 `digua_common` 的
// 同名 feature（见 Cargo.toml `[features]`），而 cargo 的 unused_dependencies
// lint 不把 feature 转发计为使用，需在代码里留一个真实引用点。
use digua_common as _;
