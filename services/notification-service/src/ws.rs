use crate::ws_hub::WsHub;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};

#[derive(Clone)]
pub struct WsState {
    pub hub: WsHub,
}

pub async fn upgrade(
    State(s): State<WsState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let user_id = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    ws.on_upgrade(move |socket| handle(socket, user_id, s.hub))
}

async fn handle(mut socket: WebSocket, user_id: Option<u64>, hub: WsHub) {
    let user_id = match user_id {
        Some(v) => v,
        None => {
            let _ = socket.close().await;
            return;
        }
    };
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
