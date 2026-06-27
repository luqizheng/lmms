//! 公告发布事件 → 本副本 WS 推送。
//!
//! ```text
//! events exchange ──(fanout copy 每副本)──▶ 本副本 ephemeral 队列
//!   handle(env): for uid in env.payload.user_ids:
//!                  hub.publish(uid, New)   // 仅本副本有本地 WS 连接的 uid 实际送达
//! ```
//! best-effort：无幂等去重（前端按 id 去重）；漏推由 list_for_user reload 兜底。
use common_event::events::NotificationAnnouncementPublished;
use common_event::{Envelope, HandlerError};

use crate::ws_hub::{WsHub, WsOut};

pub async fn handle(
    hub: &WsHub,
    env: &Envelope<NotificationAnnouncementPublished>,
) -> Result<(), HandlerError> {
    let p = &env.payload;
    let users: Vec<String> = serde_json::from_value(p.user_ids.clone()).unwrap_or_default();
    for uid in users {
        hub.publish(
            uid,
            WsOut::New(serde_json::json!({
                "id": p.announcement_id,
                "title": p.title,
                "pinned": p.pinned,
                "popover": p.popover,
            })),
        )
        .await;
    }
    Ok(())
}
