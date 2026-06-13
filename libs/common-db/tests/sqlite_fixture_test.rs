#![cfg(feature = "test-kit")]
use common_db::test_kit::SqliteFixture;
use sqlx::Row;
use std::path::Path;

#[tokio::test]
async fn fixture_isolates_per_test() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migration_path = crate_dir.parent().unwrap().parent().unwrap().join("migrations/_libs-fixture/sqlite");
    let fx = SqliteFixture::new(migration_path.to_str().unwrap()).await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('alice')")
        .execute(fx.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(fx.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 1);
}

#[tokio::test]
async fn fixture_parallel_does_not_collide() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let migration_path = crate_dir.parent().unwrap().parent().unwrap().join("migrations/_libs-fixture/sqlite");
    let a = SqliteFixture::new(migration_path.to_str().unwrap()).await.unwrap();
    let b = SqliteFixture::new(migration_path.to_str().unwrap()).await.unwrap();
    sqlx::query("INSERT INTO demo_user(name) VALUES('x')")
        .execute(a.pool()).await.unwrap();
    let row = sqlx::query("SELECT COUNT(*) AS c FROM demo_user")
        .fetch_one(b.pool()).await.unwrap();
    let c: i64 = row.get("c");
    assert_eq!(c, 0, "fixtures must not share state");
}