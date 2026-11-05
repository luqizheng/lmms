use crate::ws_hub::WsHub;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::{Principal, PrincipalStateProvider};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;

#[derive(Clone)]
pub struct WsState {
    pub hub: WsHub,
    /// 2026-11-05 review (P0-7): required by the `Principal` extractor. The
    /// WS identity now comes from the verified JWT (`sub` claim); the old
    /// `x-user-id` header path let anyone subscribe to any user's pushes
    /// (the gateway forwards client headers verbatim).
    pub jwks: Arc<JWKSCache>,
}

impl PrincipalStateProvider for WsState {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

pub async fn upgrade(
    State(s): State<WsState>,
    principal: Principal,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle(socket, principal.user_id, s.hub))
}

async fn handle(socket: WebSocket, user_id: String, hub: WsHub) {
    let mut rx = hub.subscribe(user_id).await;
    let (mut tx, mut rx_in) = socket.split();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Ok(s) = msg {
                    if tx.send(Message::Text(Utf8Bytes::from(s))).await.is_err() { break; }
                }
            }
            inbound = rx_in.next() => {
                match inbound {
                    Some(Ok(Message::Text(t))) if &*t == "{\"type\":\"ping\"}" => {
                        let p = serde_json::json!({"type":"pong"}).to_string();
                        if tx.send(Message::Text(Utf8Bytes::from(p))).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
