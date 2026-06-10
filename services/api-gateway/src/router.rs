use crate::discovery::ServiceDiscovery;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use reqwest::Client;
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterConfig {
    pub discovery: Arc<ServiceDiscovery>,
    pub http_client: Client,
}

/// 转发请求到 Consul 发现的后端服务
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
) -> Response {
    let instance = match cfg.discovery.discover(service_name).await {
        Ok(i) => i,
        Err(status) => return status.into_response(),
    };

    let target_url = format!(
        "http://{}:{}{}",
        instance.address,
        instance.port,
        request.uri().path_and_query().map(|pq| -> &str { pq.as_str() }).unwrap_or("")
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let req_builder = cfg.http_client.request(parts.method.clone(), &target_url);

    let resp = match req_builder.headers(parts.headers.clone()).body(bytes).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("forward to {} failed: {:?}", service_name, e);
            return (StatusCode::BAD_GATEWAY, "upstream error").into_response();
        }
    };

    let status = resp.status();
    let headers = resp.headers().clone();
    let body_bytes = resp.bytes().await.unwrap_or_default();

    let mut builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        if !["transfer-encoding", "connection"].contains(&key_str) {
            builder = builder.header(key_str, value.as_bytes());
        }
    }

    builder.body(Body::from(body_bytes)).unwrap_or_else(|_| {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    })
}
