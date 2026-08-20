use digua_common_event::consumer::{Consumer, RabbitMqConsumer};
use digua_common_event::events::IamUserCreated;
use digua_common_event::{Envelope, Publisher, RabbitMqPublisher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::rabbitmq::RabbitMq;

async fn rabbit_url() -> (String, ContainerAsync<RabbitMq>) {
    let c = RabbitMq.start().await.unwrap();
    let host = c.get_host().await.unwrap();
    let port = c.get_host_port_ipv4(5672).await.unwrap();
    (format!("amqp://guest:guest@{}:{}/%2f", host, port), c)
}

async fn connect_publisher(url: &str) -> RabbitMqPublisher {
    let mut last = None;
    for _ in 0..30 {
        match RabbitMqPublisher::new(url, "events").await {
            Ok(p) => return p,
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("rabbitmq connect failed: {:?}", last);
}

fn sample_event() -> Envelope<IamUserCreated> {
    Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: None,
            org_unit_id: None,
        },
    )
}

/// Two EPHEMERAL subscribers binding to the same routing key must BOTH receive
/// the same published message — proving fanout-to-all (per-replica server-named
/// queues), NOT competing-consumers load balancing across a shared queue.
#[tokio::test]
async fn two_ephemeral_subscribers_both_receive() {
    let (url, _rmq) = rabbit_url().await;

    let recv_a = Arc::new(AtomicUsize::new(0));
    let recv_b = Arc::new(AtomicUsize::new(0));

    // Subscriber A — its own connection + ephemeral queue.
    let consumer_a = RabbitMqConsumer::new(&url)
        .await
        .expect("consumer a connect");
    let recv_a_h = recv_a.clone();
    consumer_a
        .subscribe_ephemeral::<IamUserCreated, _, _>(
            &["iam.#"],
            move |env: Envelope<IamUserCreated>| {
                let counter = recv_a_h.clone();
                async move {
                    assert_eq!(env.payload.username, "alice");
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("subscribe a");

    // Subscriber B — separate connection + its own ephemeral queue, same binding.
    let consumer_b = RabbitMqConsumer::new(&url)
        .await
        .expect("consumer b connect");
    let recv_b_h = recv_b.clone();
    consumer_b
        .subscribe_ephemeral::<IamUserCreated, _, _>(
            &["iam.#"],
            move |env: Envelope<IamUserCreated>| {
                let counter = recv_b_h.clone();
                async move {
                    assert_eq!(env.payload.username, "alice");
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .expect("subscribe b");

    // Give both ephemeral queues time to bind before publishing.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let publisher = connect_publisher(&url).await;
    publisher
        .publish(&sample_event())
        .await
        .expect("publish ok");

    // Wait until both subscribers have received the single message.
    let mut both = false;
    for _ in 0..50 {
        if recv_a.load(Ordering::SeqCst) >= 1 && recv_b.load(Ordering::SeqCst) >= 1 {
            both = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        both,
        "both ephemeral subscribers must receive the message (a={}, b={})",
        recv_a.load(Ordering::SeqCst),
        recv_b.load(Ordering::SeqCst)
    );

    // Key fanout assertion: each independent ephemeral queue got exactly one
    // copy — not one consumer stealing it from the other (competing-consumers).
    assert_eq!(recv_a.load(Ordering::SeqCst), 1);
    assert_eq!(recv_b.load(Ordering::SeqCst), 1);
}
