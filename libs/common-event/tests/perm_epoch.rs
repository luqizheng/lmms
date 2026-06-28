use common_event::perm_epoch::{get_perm_epoch, set_perm_epoch_if_greater};
use testcontainers_modules::{redis::Redis, testcontainers::runners::AsyncRunner};

#[tokio::test]
async fn set_if_greater_is_monotonic() {
    let c = Redis::default().start().await.unwrap();
    let port = c.get_host_port_ipv4(6379).await.unwrap();
    let host = c.get_host().await.unwrap();
    let client = redis::Client::open(format!("redis://{host}:{port}")).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    set_perm_epoch_if_greater(&mut conn, "", "u1", 100, 300)
        .await
        .unwrap();
    assert_eq!(
        get_perm_epoch(&mut conn, "", "u1").await.unwrap(),
        Some(100)
    );
    // 晚到的较小值不得覆盖
    set_perm_epoch_if_greater(&mut conn, "", "u1", 50, 300)
        .await
        .unwrap();
    assert_eq!(
        get_perm_epoch(&mut conn, "", "u1").await.unwrap(),
        Some(100)
    );
    // 较大值覆盖
    set_perm_epoch_if_greater(&mut conn, "", "u1", 150, 300)
        .await
        .unwrap();
    assert_eq!(
        get_perm_epoch(&mut conn, "", "u1").await.unwrap(),
        Some(150)
    );
}
