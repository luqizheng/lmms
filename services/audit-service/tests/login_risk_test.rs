//! 风控 consumer 集成测试：发布 threshold 条 iam.login.failed（同一 username），
//! 断言 Redis 计数到达 threshold（越线告警是副作用，靠计数验证窗口逻辑）。
use std::time::Duration;

use audit_service::redis_cli::RedisCli;
use audit_service::risk::{self, RiskOptions};
use digua_common_event::events::IamLoginFailed;
use digua_common_event::{Envelope, Publisher, RabbitMqPublisher};
use testcontainers_modules::{rabbitmq::RabbitMq, redis::Redis, testcontainers::runners::AsyncRunner};

#[tokio::test]
async fn risk_counter_reaches_threshold() {
    let rabbit = RabbitMq::default().start().await.unwrap();
    let amqp_port = rabbit.get_host_port_ipv4(5672).await.unwrap();
    let amqp_host = rabbit.get_host().await.unwrap();
    let amqp_url = format!("amqp://guest:guest@{amqp_host}:{amqp_port}/%2f");

    let redis_c = Redis::default().start().await.unwrap();
    let redis_port = redis_c.get_host_port_ipv4(6379).await.unwrap();
    let redis_host = redis_c.get_host().await.unwrap();
    let redis_url = format!("redis://{redis_host}:{redis_port}");

    let redis = RedisCli::connect(&redis_url).await.unwrap();

    let opts = RiskOptions { threshold: 5, window_secs: 300 };
    {
        let redis = redis.clone();
        let url = amqp_url.clone();
        tokio::spawn(async move { let _ = risk::run(redis, url, opts).await; });
    }
    // 等 consumer 订阅就绪
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 连接 publisher（AMQP listener 可能晚于端口开放，重试）
    let publisher = {
        let mut last = None;
        for _ in 0..20 {
            match RabbitMqPublisher::new(&amqp_url, "events").await {
                Ok(p) => { last = Some(p); break; }
                Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
        last.expect("publisher connect")
    };

    for _ in 0..5 {
        let env = Envelope::new("test", IamLoginFailed {
            username: "attacker".into(),
            user_id: None,
            ip: "9.9.9.9".into(),
            user_agent: None,
            reason: "bad_password".into(),
        });
        publisher.publish(&env).await.unwrap();
    }

    // 轮询 Redis 直到计数到 5（或超时）
    let mut conn = redis.conn.clone();
    use redis::AsyncCommands;
    let mut count: i64 = 0;
    for _ in 0..40 {
        count = conn.get::<_, Option<i64>>("audit:loginfail:user:attacker").await.unwrap().unwrap_or(0);
        if count >= 5 { break; }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(count, 5, "expected risk counter to reach threshold");
}
