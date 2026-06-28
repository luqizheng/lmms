use crate::discovery::ServiceDiscovery;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use common_http::{ResilientClient, ResilientError};
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterConfig {
    pub discovery: Arc<ServiceDiscovery>,
    pub resilient: Arc<ResilientClient>,
}

/// 转发请求到 Consul 发现的后端服务（target_path_query 已由模板渲染好）。
///
/// 服务发现在韧性边界之外（决策 9A）：discover 在此完成、不进 ResilientClient、
/// 不喂 breaker —— breaker 信号只反映下游实例健康，不被 Consul 故障污染。
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
) -> Response {
    // --- 韧性边界之外：服务发现 ---
    let instance = match cfg.discovery.discover(service_name).await {
        Ok(i) => i,
        Err(status) => return status.into_response(),
    };

    let target_url = format!(
        "http://{}:{}{}",
        instance.address, instance.port, target_path_query
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // --- 韧性边界之内：超时/重试/熔断 ---
    match cfg
        .resilient
        .call(service_name, parts.method, &target_url, &parts.headers, bytes)
        .await
    {
        Ok(resp) => {
            let mut builder = Response::builder().status(resp.status);
            for (key, value) in resp.headers.iter() {
                let key_str = key.as_str();
                if !["transfer-encoding", "connection"].contains(&key_str) {
                    builder = builder.header(key_str, value.as_bytes());
                }
            }
            builder
                .body(Body::from(resp.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(ResilientError::CircuitOpen) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "circuit_open",
                "service": service_name
            })),
        )
            .into_response(),
        Err(ResilientError::Timeout) => {
            tracing::warn!("forward to {} timed out", service_name);
            (StatusCode::GATEWAY_TIMEOUT, "upstream timeout").into_response()
        }
        Err(ResilientError::Connect(e)) => {
            tracing::warn!("forward to {} connect error: {:?}", service_name, e);
            (StatusCode::BAD_GATEWAY, "upstream error").into_response()
        }
    }
}
