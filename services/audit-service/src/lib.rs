pub mod config;
pub mod consumer;
pub mod error;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod services;

pub use config::AppConfig;
pub use error::{AppError, AppResult};
pub use routes::{create_state, routes};
