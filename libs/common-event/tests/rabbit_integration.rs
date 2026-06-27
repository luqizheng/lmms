use common_event::events::IamUserCreated;
use common_event::{Envelope, Publisher, RabbitMqPublisher};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::rabbitmq::RabbitMq;

async fn make_publisher() -> (RabbitMqPublisher, ContainerAsync<RabbitMq>) {
    let c = RabbitMq::default().start().await.unwrap();
    let host = c.get_host().await.unwrap();
    let port = c.get_host_port_ipv4(5672).await.unwrap();
    let url = format!("amqp://guest:guest@{}:{}/%2f", host, port);

    // RabbitMQ may report its port open before the AMQP listener fully accepts
    // connections — retry the initial connect for up to ~30s.
    let mut last_err = None;
    for _ in 0..30 {
        match RabbitMqPublisher::new(&url, "events").await {
            Ok(pub_) => return (pub_, c),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    panic!("could not connect to rabbitmq: {:?}", last_err);
}

#[tokio::test]
async fn publish_sends_to_topic_exchange() {
    let (pub_, _container) = make_publisher().await;
    let env = Envelope::new(
        "iam-service",
        IamUserCreated {
            user_id: "u1".to_string(),
            username: "alice".to_string(),
            email: None,
            org_unit_id: None,
        },
    );
    pub_.publish(&env).await.expect("publish ok");
}

