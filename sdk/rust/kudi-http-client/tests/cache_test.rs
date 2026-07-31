use digua_http_client::cache::ServiceTokenCache;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn info_with(expires_in: i64) -> TokenInfo {
    TokenInfo {
        access_token: "tok".into(),
        refresh_token: None,
        token_type: "Bearer".into(),
        expires_in,
    }
}

#[tokio::test]
async fn caches_token_per_credential_pair() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));

    let c1 = counter.clone();
    let t1 = cache
        .get_or_fetch("a", "s1", || {
            let c = c1.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info_with(3600))
            }
        })
        .await
        .unwrap();

    let c2 = counter.clone();
    let t2 = cache
        .get_or_fetch("a", "s1", || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info_with(3600))
            }
        })
        .await
        .unwrap();

    assert_eq!(t1, "tok");
    assert_eq!(t2, "tok");
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "fetcher must run only once"
    );
}

#[tokio::test]
async fn separate_credentials_have_separate_cache_entries() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));

    for cid in ["a", "b"] {
        let c2 = counter.clone();
        let _ = cache
            .get_or_fetch(cid, "s1", || {
                let c = c2.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, KudiHttpError>(info_with(3600))
                }
            })
            .await
            .unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert_eq!(cache.snapshot().await.size, 2);
}

#[tokio::test]
async fn invalidating_a_credential_refetches() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..2 {
        let c2 = counter.clone();
        let _ = cache
            .get_or_fetch("a", "s1", || {
                let c = c2.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, KudiHttpError>(info_with(3600))
                }
            })
            .await
            .unwrap();
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    cache.invalidate("a", "s1").await;

    let c2 = counter.clone();
    let _ = cache
        .get_or_fetch("a", "s1", || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info_with(3600))
            }
        })
        .await
        .unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}
