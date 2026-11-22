//! Real connectivity smoke test for the Dameng MySQL-wire compatibility path.
//!
//! The test is skipped unless TEST_DAMENG_URL is explicitly provided. CI supplies
//! a URL using the public dameng:// scheme; common-db normalizes it for SQLx.

#![cfg(feature = "dameng")]

use digua_common_db::{init_pool, DbPool};

#[tokio::test]
async fn dameng_mysql_wire_connection_and_ping() {
    let Ok(url) = std::env::var("TEST_DAMENG_URL") else {
        eprintln!("[dameng_test] TEST_DAMENG_URL not set — skipped");
        return;
    };

    let pool = init_pool(&url)
        .await
        .expect("connect to Dameng through its MySQL-compatible wire protocol");

    match pool {
        DbPool::Mysql(_) => pool.ping().await.expect("Dameng SELECT 1 health probe"),
        _ => panic!("Dameng must be represented by the MySQL transport pool"),
    }
}
