use super::Chunk;
use cheetah_media_backend_api::ByteSourceError;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;

pub(crate) async fn run(url: String, tx: mpsc::Sender<Chunk>) {
    match tokio_tungstenite::connect_async(&url).await {
        Ok((mut ws_stream, _)) => {
            // Drive the combined Stream+Sink. Tungstenite queues and flushes
            // automatic Pong replies while polling the stream, so we do not
            // split off the write half.
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        if tx.send(Chunk::Data(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Text(s)) => {
                        if tx.send(Chunk::Data(s.into_bytes())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx.send(Chunk::Eof).await;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => {
                        let _ = tx
                            .send(Chunk::Error(ByteSourceError::Fatal {
                                code: 40,
                                context: Some("ws_read_error"),
                            }))
                            .await;
                        break;
                    }
                }
            }
        }
        Err(_) => {
            let _ = tx
                .send(Chunk::Error(ByteSourceError::Retryable {
                    reason: "ws_connect_failed",
                    backoff_ms: 1000,
                }))
                .await;
        }
    }
}
