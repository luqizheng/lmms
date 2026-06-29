//! Gateway read-side perm-epoch check (Phase 4.4). Thin wrapper over
//! `common_event::perm_epoch`. A token is rejected when it was issued
//! (`iat`) before the user's last permission change (`perm_epoch`).
use redis::aio::MultiplexedConnection;

/// Pure decision: reject iff an epoch exists and the token predates it.
pub fn is_revoked(epoch: Option<i64>, iat: i64) -> bool {
    matches!(epoch, Some(e) if iat < e)
}

/// Returns true when the token must be rejected. Fail-open: on Redis error
/// or key miss, returns false (matching existing blacklist behavior).
pub async fn check_revoked(
    conn: &mut MultiplexedConnection,
    prefix: &str,
    sub: &str,
    iat: i64,
) -> bool {
    match common_event::perm_epoch::get_perm_epoch(conn, prefix, sub).await {
        Ok(epoch) => is_revoked(epoch, iat),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_revoked_some_iat_less_than_epoch_rejects() {
        assert!(is_revoked(Some(150), 100));
    }

    #[test]
    fn is_revoked_some_iat_ge_epoch_passes() {
        assert!(!is_revoked(Some(100), 100));
        assert!(!is_revoked(Some(100), 150));
    }

    #[test]
    fn is_revoked_none_passes() {
        assert!(!is_revoked(None, 100));
    }
}
