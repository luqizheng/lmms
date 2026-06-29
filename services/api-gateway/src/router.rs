use crate::config::EgressTarget;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use common_http::{ResilientClient, ResilientError};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterConfig {
    pub resilient: Arc<ResilientClient>,
    pub egress_map: Arc<HashMap<String, EgressTarget>>,
}

/// 通过 sidecar egress 转发：URL 目标取自 `egress_map[service_name]`，
/// 由 sidecar 完成 mTLS 与对端选择。下游健康由 sidecar 的 EDS 维护，
/// 连接失败 / 超时透过 `ResilientClient` 喂熔断（决策 9A/14B 适用）。
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
) -> Response {
    let Some(egress) = cfg.egress_map.get(service_name) else {
        tracing::error!("no egress configured for service '{}'", service_name);
        return StatusCode::BAD_GATEWAY.into_response();
    };

    let target_url = format!(
        "http://{}:{}{}",
        egress.host, egress.port, target_path_query
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use common_http::ResilienceConfig;

    fn mk_cfg(map: HashMap<String, EgressTarget>) -> Arc<RouterConfig> {
        Arc::new(RouterConfig {
            resilient: Arc::new(ResilientClient::new(
                reqwest::Client::new(),
                ResilienceConfig::default(),
                Arc::new(common_http::NoopObserver),
            )),
            egress_map: Arc::new(map),
        })
    }

    #[tokio::test]
    async fn unknown_service_returns_bad_gateway() {
        let cfg = mk_cfg(HashMap::new());
        let req = Request::builder().uri("/x").body(Body::empty()).unwrap();
        let resp = forward_to_service(cfg, req, "no-such-service", "/x").await;
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }
}
