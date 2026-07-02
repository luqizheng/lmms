use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::config::ResilienceConfig;
use crate::observer::{Outcome, ResilienceObserver};
use crate::retry::RetryPolicy;
use bytes::Bytes;
use dashmap::DashMap;
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, StatusCode};
use std::sync::Arc;
use std::time::Instant;
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
    pub fn new(inner: Client, cfg: ResilienceConfig, observer: Arc<dyn ResilienceObserver>) -> Self {
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
        let deadline = Instant::now(); // 决策 8C：栈上局部
        let breaker = self.breaker_for(service);
        let mut attempt = 0u32;

        loop {
            attempt += 1;

            // 决策 4A：overall deadline 封顶（含重试+退避）
            if deadline.elapsed() >= self.cfg.overall_timeout() {
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

            let result = timeout(self.cfg.timeout(), req.send()).await;

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
                        && deadline.elapsed() < self.cfg.overall_timeout()
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }

                    self.observer.on_outcome(
                        service,
                        if is_failure { Outcome::UpstreamError } else { Outcome::Success },
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
                        && deadline.elapsed() < self.cfg.overall_timeout()
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
                        && deadline.elapsed() < self.cfg.overall_timeout()
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
        let deadline = Instant::now();
        let breaker = self.breaker_for(service);
        let mut attempt = 0u32;

        loop {
            attempt += 1;

            if deadline.elapsed() >= self.cfg.overall_timeout() {
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

            let result = timeout(self.cfg.timeout(), req.send()).await;

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
                        if is_failure { Outcome::UpstreamError } else { Outcome::Success },
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
                        && deadline.elapsed() < self.cfg.overall_timeout()
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
                        && deadline.elapsed() < self.cfg.overall_timeout()
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