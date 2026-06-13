use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

use crate::request_id::RequestId;

pub async fn request_id_layer(mut req: Request, next: Next) -> Response {
    let incoming = req
        .headers()
        .get("X-Request-Id")
        .and_then(|v| v.to_str().ok());
    let rid = RequestId::from_header(incoming);
    req.extensions_mut().insert(rid.clone());
    let mut resp = next.run(req).await;
    if let Ok(v) = HeaderValue::from_str(rid.as_str()) {
        resp.headers_mut().insert("X-Request-Id", v);
    }
    resp
}
