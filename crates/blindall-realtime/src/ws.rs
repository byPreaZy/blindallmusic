use crate::hub::{Connection, Hub};
use crate::protocol::{ClientOp, PresenceKind, ServerOp};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ChannelAuthorizer: Send + Sync {
    async fn authorize(&self, user_id: i64, channel: &str) -> Result<Option<Value>, AuthzError>;
}

#[derive(Debug)]
pub struct AuthzError {
    pub code: &'static str,
}

pub async fn serve<A: ChannelAuthorizer>(socket: WebSocket, hub: Hub, authz: Arc<A>, user_id: i64) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (mut conn, mut outbound) = hub.connect(user_id);
    let conn_id = conn.id;

    let writer = tokio::spawn(async move {
        while let Some(op) = outbound.recv().await {
            let Ok(text) = serde_json::to_string(&op) else {
                continue;
            };
            if ws_tx.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(text) => {
                let Ok(op) = serde_json::from_str::<ClientOp>(&text) else {
                    conn.send(ServerOp::Error {
                        code: "bad_frame",
                        req_id: None,
                    });
                    continue;
                };
                handle_op(op, &hub, &authz, &mut conn).await;
            }
            Message::Close(_) => break,
            Message::Ping(p) => {
                let _ = conn.send(ServerOp::Pong { ts: None });
                let _ = p;
            }
            _ => {}
        }
    }

    drop(conn);
    writer.abort();
    tracing::debug!(conn_id, user_id, "ws closed");
}

async fn handle_op<A: ChannelAuthorizer>(
    op: ClientOp,
    hub: &Hub,
    authz: &Arc<A>,
    conn: &mut Connection,
) {
    match op {
        ClientOp::Subscribe { channel, req_id } => {
            match authz.authorize(conn.user_id, &channel).await {
                Err(e) => {
                    conn.send(ServerOp::Error {
                        code: e.code,
                        req_id,
                    });
                }
                Ok(member) => {
                    let snapshot = hub.subscribe(conn, &channel, member).await;
                    if let Some(users) = snapshot {
                        conn.send(ServerOp::Presence {
                            channel: channel.clone(),
                            kind: PresenceKind::Sync,
                            users,
                        });
                    }
                    conn.send(ServerOp::Ack { req_id });
                }
            }
        }
        ClientOp::Unsubscribe { channel, req_id } => {
            hub.unsubscribe(conn, &channel).await;
            conn.send(ServerOp::Ack { req_id });
        }
        ClientOp::Heartbeat { ts } => {
            conn.send(ServerOp::Pong { ts });
        }
    }
}
