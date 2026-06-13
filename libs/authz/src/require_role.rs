use axum::extract::{FromRequestParts, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use futures::future::BoxFuture;
use jwt_ctx::Principal;

pub fn require_role(
    role: &'static str,
) -> impl Fn(Request, Next) -> BoxFuture<'static, Result<Response, StatusCode>>
       + Clone
       + Send
       + Sync
       + 'static {
    move |req: Request, next: Next| {
        let role = role;
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let p = match Principal::from_request_parts(&mut parts, &()).await {
                Ok(p) => p,
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            };
            if !p.has_role(role) {
                return Err(StatusCode::FORBIDDEN);
            }
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        })
    }
}
