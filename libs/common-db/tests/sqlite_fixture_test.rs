#![cfg(feature = "test-kit")]
use common_db::test_kit::SqliteFixture;
use sqlx::Row;

#[tokio::test]
async fn fixture_isolates_per_test() {
    let fx = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('alice')")
        .execute(fx.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(fx.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 1);
}

#[tokio::test]
async fn fixture_parallel_does_not_collide() {
    let a = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    let b = SqliteFixture::new("migrations/_libs-fixture/sqlite").await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('x')")
        .execute(a.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(b.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 0, "fixtures must not share state");
}