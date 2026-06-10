use crate::config::RewriteSpec;
use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Request, StatusCode, Uri};
use axum::response::Response;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RewriteError {
    #[error("prefix not in path: {0}")]
    PrefixMissing(String),
    #[error("invalid uri: {0}")]
    InvalidUri(String),
    #[error("invalid header name: {0}")]
    InvalidHeaderName(String),
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(String),
}

pub fn apply(rules: &[RewriteSpec], req: &mut Request<Body>) -> Result<(), Response> {
    for rule in rules {
        match rule {
            RewriteSpec::StripPrefix { prefix } => {
                let p = req.uri().path();
                let stripped = p.strip_prefix(prefix.as_str()).map(str::to_string);
                match stripped {
                    Some(rest) => {
                        let new_path = if rest.is_empty() { "/".to_string() } else { rest };
                        *req.uri_mut() = build_uri(req.uri(), &new_path);
                    }
                    None => {
                        return Err(err_response(
                            StatusCode::BAD_REQUEST,
                            &RewriteError::PrefixMissing(prefix.clone()).to_string(),
                        ));
                    }
                }
            }
            RewriteSpec::HeaderSet { name, value } => {
                let n: HeaderName = name.parse().map_err(|_| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &RewriteError::InvalidHeaderName(name.clone()).to_string(),
                    )
                })?;
                let v: HeaderValue = value.parse().map_err(|_| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &RewriteError::InvalidHeaderValue(value.clone()).to_string(),
                    )
                })?;
                req.headers_mut().insert(n, v);
            }
            RewriteSpec::HeaderAdd { name, value } => {
                let n: HeaderName = name.parse().map_err(|_| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &RewriteError::InvalidHeaderName(name.clone()).to_string(),
                    )
                })?;
                let v: HeaderValue = value.parse().map_err(|_| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &RewriteError::InvalidHeaderValue(value.clone()).to_string(),
                    )
                })?;
                req.headers_mut().append(n, v);
            }
            RewriteSpec::PathParam { from, to } => {
                let mut parts = req.uri().clone().into_parts();
                if let Some(pq) = parts.path_and_query.as_mut() {
                    let new_path = pq.path().replace(from.as_str(), to.as_str());
                    let new_pq: axum::http::uri::PathAndQuery = new_path
                        .parse()
                        .map_err(|_| {
                            err_response(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                &RewriteError::InvalidUri(new_path.clone()).to_string(),
                            )
                        })?;
                    *pq = new_pq;
                }
                *req.uri_mut() = Uri::from_parts(parts).map_err(|_| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &RewriteError::InvalidUri(req.uri().to_string()).to_string(),
                    )
                })?;
            }
            RewriteSpec::Redirect { status, to } => {
                let s = StatusCode::from_u16(*status).unwrap_or(StatusCode::MOVED_PERMANENTLY);
                let v: HeaderValue = to.parse().unwrap_or(HeaderValue::from_static("/"));
                let resp = Response::builder()
                    .status(s)
                    .header("location", v)
                    .body(Body::empty())
                    .unwrap();
                return Err(resp);
            }
        }
    }
    Ok(())
}

fn build_uri(orig: &Uri, new_path: &str) -> Uri {
    let mut parts = orig.clone().into_parts();
    let pq = parts
        .path_and_query
        .get_or_insert_with(|| "/".parse().unwrap());
    *pq = new_path.parse().unwrap();
    Uri::from_parts(parts).unwrap()
}

fn err_response(status: StatusCode, msg: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(msg.to_string()))
        .unwrap()
}
