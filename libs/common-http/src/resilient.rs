use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::config::ResilienceConfig;
use crate::observer::{Outcome, ResilienceObserver};
use crate::retry::RetryPolicy;
use bytes::Bytes;
use dashmap::DashMap;
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, StatusCode};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// 收到任何 HTTP 响应（含 5xx）都读成 owned 结构（决策 5A），
/// 消除 reqwest Response body 流读一次即消费与重试/透传的所有权冲突。
#[derive(Debug)]
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// 流式上游响应：SSE / WebSocket 场景。
///
/// 仅记录 status / headers 后即返回，未消费 body。调用方负责把
/// `body.bytes_stream()` 接到 axum Body / WebSocket upgrade。
#[derive(Debug)]
pub struct UpstreamStream {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// 未消费 body 的 live reqwest::Response —— 调用方独占消费。
    pub body: reqwest::Response,
}

/// 仅表达「没拿到响应」的传输层失败（决策 5A）。
/// 5xx/429 不是 Err —— 它们作为 Ok(UpstreamResponse) 返回。
#[derive(Debug)]
pub enum ResilientError {
    CircuitOpen,
    Timeout,
    Connect(reqwest::Error),
}

/// outbound 韧性客户端：超时 + 重试 + 按 service 熔断（决策 1A/4A/15A）。
///
/// 服务发现在 call 之前完成、不进本结构、不喂 breaker（决策 9A）。
///
/// call 单次循环：
///   deadline 超 overall? -> Timeout
///   breaker 不可用? -> CircuitOpen
///   timeout(单次, send) -> Ok(resp 读 owned) / Connect / Timeout
///   按 status/err 决定重试(仅幂等) 或 返回
pub struct ResilientClient {
    inner: Client,
    cfg: ResilienceConfig,
    retry: RetryPolicy,
    breakers: DashMap<String, Arc<CircuitBreaker>>,
    observer: Arc<dyn ResilienceObserver>,
}

impl ResilientClient {
    pub fn new(
        inner: Client,
        cfg: ResilienceConfig,
        observer: Arc<dyn ResilienceObserver>,
    ) -> Self {
        let retry = cfg.retry_policy();
        Self {
            inner,
            cfg,
            retry,
            breakers: DashMap::new(),
            observer,
        }
    }

    // 决策 15A：读锁命中直接 clone Arc；未命中才写锁创建。
    fn breaker_for(&self, service: &str) -> Arc<CircuitBreaker> {
        if let Some(b) = self.breakers.get(service) {
            return b.clone();
        }
        self.breakers
            .entry(service.to_string())
            .or_insert_with(|| Arc::new(self.cfg.new_breaker()))
            .clone()
    }

    fn note_state(&self, service: &str, before: CircuitState, breaker: &CircuitBreaker) {
        let after = breaker.state();
        if after != before {
            self.observer.on_state_change(service, after);
        }
    }

    pub async fn call(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<UpstreamResponse, ResilientError> {
        self.call_inner(service, method, url, headers, body, None)
            .await
    }

    /// Per-call 超时覆盖：当 `timeout_opt = Some(t)` 时，该次调用的单次发送超时与
    /// 整体超时都用 `t`（POST 非幂等不重试，单次=整体语义对齐）。缺省 None 走
    /// 全局 `ResilienceConfig` 默认值。调用方应使用带名字的常量/路由配置传入。
    pub async fn call_with_timeout(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
        timeout: Duration,
    ) -> Result<UpstreamResponse, ResilientError> {
        self.call_inner(service, method, url, headers, body, Some(timeout))
            .await
    }

    async fn call_inner(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
        timeout_opt: Option<Duration>,
    ) -> Result<UpstreamResponse, ResilientError> {
        let deadline = Instant::now(); // 决策 8C：栈上局部
        let breaker = self.breaker_for(service);
        let mut attempt = 0u32;

        // Per-call override：单次超时与整体超时同步放宽（POST 不重试，单次=整体语义对齐）
        let per_attempt_timeout = timeout_opt.unwrap_or_else(|| self.cfg.timeout());
        let overall_timeout = timeout_opt.unwrap_or_else(|| self.cfg.overall_timeout());

        loop {
            attempt += 1;

            // 决策 4A：overall deadline 封顶（含重试+退避）
            if deadline.elapsed() >= overall_timeout {
                self.observer.on_outcome(service, Outcome::Timeout);
                return Err(ResilientError::Timeout);
            }

            let st_before = breaker.state();
            if !breaker.is_available() {
                self.note_state(service, st_before, &breaker);
                self.observer.on_outcome(service, Outcome::CircuitOpen);
                return Err(ResilientError::CircuitOpen);
            }
            self.note_state(service, st_before, &breaker);

            // 决策 16A：headers/body 入口准备一次，每轮用引用 + Bytes O(1) clone 重建
            let req = self
                .inner
                .request(method.clone(), url)
                .headers(headers.clone())
                .body(body.clone());

            let result = timeout(per_attempt_timeout, req.send()).await;

            match result {
                // 拿到响应（含 5xx）—— 决策 5A：读成 owned
                Ok(Ok(resp)) => {
                    let status = resp.status();
                    let resp_headers = resp.headers().clone();
                    // 决策 review#2：body 流读失败时显式打 warn，不再静默退化为空
                    // body（否则调用方拿到 200 + 空 body 与合法空响应无法区分）。
                    // 返回类型保持 Ok(UpstreamResponse) —— status 仍可达，四态映射不变。
                    let body_bytes = resp.bytes().await.unwrap_or_else(|e| {
                        tracing::warn!(
                            service = service,
                            status = status.as_u16(),
                            error = %e,
                            "failed to read upstream response body; returning empty bytes"
                        );
                        bytes::Bytes::new()
                    });

                    // breaker 计失败口径（spec §5.3）：502/503/429 计失败，其余成功
                    let is_failure = matches!(status.as_u16(), 502 | 503 | 429);
                    let st = breaker.state();
                    if is_failure {
                        breaker.record_failure();
                    } else {
                        breaker.record_success();
                    }
                    self.note_state(service, st, &breaker);

                    // 决策 6A：status 是否重试
                    if self.retry.should_retry_status(attempt, status, &method)
                        && deadline.elapsed() < overall_timeout
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }

                    self.observer.on_outcome(
                        service,
                        if is_failure {
                            Outcome::UpstreamError
                        } else {
                            Outcome::Success
                        },
                    );
                    return Ok(UpstreamResponse {
                        status,
                        headers: resp_headers,
                        body: body_bytes,
                    });
                }
                // reqwest 传输错误（请求未送达）
                Ok(Err(e)) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Connect(e);
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < overall_timeout
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::ConnectError);
                    return Err(err);
                }
                // 单次超时
                Err(_) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Timeout;
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < overall_timeout
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::Timeout);
                    return Err(err);
                }
            }
        }
    }

    /// 流式版本：复用 breaker / connect-timeout 重试，但**不读 body**、
    /// 不做 status-based retry（SSE / WS 一旦开始流不可 replay）。
    /// 调用方拿到 Ok 后立刻消费 `body.bytes_stream()`。
    pub async fn call_streaming(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<UpstreamStream, ResilientError> {
        self.call_streaming_inner(service, method, url, headers, body, None)
            .await
    }

    /// 流式 per-call 超时覆盖（参见 `call_with_timeout`）。
    pub async fn call_streaming_with_timeout(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
        timeout: Duration,
    ) -> Result<UpstreamStream, ResilientError> {
        self.call_streaming_inner(service, method, url, headers, body, Some(timeout))
            .await
    }

    async fn call_streaming_inner(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
        timeout_opt: Option<Duration>,
    ) -> Result<UpstreamStream, ResilientError> {
        let deadline = Instant::now();
        let breaker = self.breaker_for(service);
        let mut attempt = 0u32;

        let per_attempt_timeout = timeout_opt.unwrap_or_else(|| self.cfg.timeout());
        let overall_timeout = timeout_opt.unwrap_or_else(|| self.cfg.overall_timeout());

        loop {
            attempt += 1;

            if deadline.elapsed() >= overall_timeout {
                self.observer.on_outcome(service, Outcome::Timeout);
                return Err(ResilientError::Timeout);
            }

            let st_before = breaker.state();
            if !breaker.is_available() {
                self.note_state(service, st_before, &breaker);
                self.observer.on_outcome(service, Outcome::CircuitOpen);
                return Err(ResilientError::CircuitOpen);
            }
            self.note_state(service, st_before, &breaker);

            let req = self
                .inner
                .request(method.clone(), url)
                .headers(headers.clone())
                .body(body.clone());

            let result = timeout(per_attempt_timeout, req.send()).await;

            match result {
                Ok(Ok(resp)) => {
                    let status = resp.status();
                    let resp_headers = resp.headers().clone();

                    // 5xx / 429 / 5xx-then-ok 都计入 breaker
                    let is_failure = matches!(status.as_u16(), 502 | 503 | 429);
                    let st = breaker.state();
                    if is_failure {
                        breaker.record_failure();
                    } else {
                        breaker.record_success();
                    }
                    self.note_state(service, st, &breaker);

                    // 流式不重试 status —— 已订立的流不可 replay
                    self.observer.on_outcome(
                        service,
                        if is_failure {
                            Outcome::UpstreamError
                        } else {
                            Outcome::Success
                        },
                    );

                    return Ok(UpstreamStream {
                        status,
                        headers: resp_headers,
                        body: resp,
                    });
                }
                Ok(Err(e)) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Connect(e);
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < overall_timeout
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::ConnectError);
                    return Err(err);
                }
                Err(_) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Timeout;
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < overall_timeout
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::Timeout);
                    return Err(err);
                }
            }
        }
    }
}

#[cfg(test)]
mod timeout_override_tests {
    use super::*;
    use crate::NoopObserver;
    use axum::{routing::post, Json, Router};
    use bytes::Bytes;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    async fn spawn_slow_server(delay: Duration) -> (SocketAddr, Arc<AtomicU64>) {
        let hits = Arc::new(AtomicU64::new(0));
        let hits_clone = hits.clone();
        let app = Router::new().route(
            "/slow",
            post(move || {
                let hits = hits_clone.clone();
                let delay = delay;
                async move {
                    tokio::time::sleep(delay).await;
                    hits.fetch_add(1, Ordering::SeqCst);
                    Json(serde_json::json!({"ok": true}))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, hits)
    }

    fn build_client() -> ResilientClient {
        let cfg = ResilienceConfig {
            timeout_ms: 3000,
            overall_timeout_ms: 3000,
            retry_max_attempts: 1,
            ..Default::default()
        };
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        ResilientClient::new(http, cfg, Arc::new(NoopObserver))
    }

    #[tokio::test]
    async fn call_with_timeout_overrides_default_for_slow_upstream() {
        let (addr, hits) = spawn_slow_server(Duration::from_secs(5)).await;
        let client = build_client();
        let url = format!("http://{addr}/slow");

        let started = std::time::Instant::now();
        let result = client
            .call_with_timeout(
                "test",
                reqwest::Method::POST,
                &url,
                &reqwest::header::HeaderMap::new(),
                Bytes::new(),
                Duration::from_millis(500),
            )
            .await;
        let elapsed = started.elapsed();

        assert!(matches!(result, Err(ResilientError::Timeout)));
        // POST 不重试 → 只 1 次 attempt，500ms override 应触发立即 Timeout
        assert!(elapsed < Duration::from_secs(2), "elapsed={elapsed:?}");
        assert_eq!(
            hits.load(Ordering::SeqCst),
            0,
            "server should not have completed"
        );
    }

    #[tokio::test]
    async fn call_with_timeout_succeeds_for_fast_upstream() {
        let (addr, hits) = spawn_slow_server(Duration::from_millis(50)).await;
        let client = build_client();
        let url = format!("http://{addr}/slow");

        let result = client
            .call_with_timeout(
                "test",
                reqwest::Method::POST,
                &url,
                &reqwest::header::HeaderMap::new(),
                Bytes::new(),
                Duration::from_secs(5),
            )
            .await;

        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn call_without_override_uses_default_timeout() {
        let (addr, hits) = spawn_slow_server(Duration::from_secs(5)).await;
        let client = build_client();
        let url = format!("http://{addr}/slow");

        let result = client
            .call(
                "test",
                reqwest::Method::POST,
                &url,
                &reqwest::header::HeaderMap::new(),
                Bytes::new(),
            )
            .await;

        // 默认 3000ms timeout, 5s 上游 → Timeout
        assert!(matches!(result, Err(ResilientError::Timeout)));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn call_streaming_with_timeout_overrides_default() {
        let (addr, hits) = spawn_slow_server(Duration::from_secs(5)).await;
        let client = build_client();
        let url = format!("http://{addr}/slow");

        let started = std::time::Instant::now();
        let result = client
            .call_streaming_with_timeout(
                "test",
                reqwest::Method::POST,
                &url,
                &reqwest::header::HeaderMap::new(),
                Bytes::new(),
                Duration::from_millis(500),
            )
            .await;
        let elapsed = started.elapsed();

        assert!(matches!(result, Err(ResilientError::Timeout)));
        assert!(elapsed < Duration::from_secs(2), "elapsed={elapsed:?}");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
}
