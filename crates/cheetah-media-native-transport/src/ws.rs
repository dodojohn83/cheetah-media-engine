use std::time::Duration;

use super::Chunk;
use cheetah_media_backend_api::ByteSourceError;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::Message;

pub(crate) async fn run(url: String, tx: mpsc::Sender<Chunk>) {
    match timeout(
        Duration::from_secs(30),
        tokio_tungstenite::connect_async(&url),
    )
    .await
    {
        Ok(Ok((mut ws_stream, _))) => {
            // Drive the combined Stream+Sink. Tungstenite queues and flushes
            // automatic Pong replies while polling the stream, so we do not
            // split off the write half.
            let mut errored = false;
            let mut eof_sent = false;
            loop {
                match timeout(Duration::from_secs(60), ws_stream.next()).await {
                    Ok(Some(Ok(Message::Binary(data)))) => {
                        if tx.send(Chunk::Data(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(Ok(Message::Text(s)))) => {
                        if tx.send(Chunk::Data(s.into_bytes())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(Ok(Message::Close(_)))) => {
                        let _ = tx.send(Chunk::Eof).await;
                        eof_sent = true;
                        break;
                    }
                    Ok(Some(Ok(_))) => {}
                    Ok(Some(Err(_))) => {
                        let _ = tx
                            .send(Chunk::Error(ByteSourceError::Fatal {
                                code: 40,
                                context: Some("ws_read_error"),
                            }))
                            .await;
                        errored = true;
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {
                        let _ = tx
                            .send(Chunk::Error(ByteSourceError::Retryable {
                                reason: "ws_read_timeout",
                                backoff_ms: 1000,
                            }))
                            .await;
                        errored = true;
                        break;
                    }
                }
            }
            if !errored && !eof_sent {
                let _ = tx.send(Chunk::Eof).await;
            }
        }
        Ok(Err(_)) => {
            let _ = tx
                .send(Chunk::Error(ByteSourceError::Retryable {
                    reason: "ws_connect_failed",
                    backoff_ms: 1000,
                }))
                .await;
        }
        Err(_) => {
            let _ = tx
                .send(Chunk::Error(ByteSourceError::Retryable {
                    reason: "ws_connect_timeout",
                    backoff_ms: 1000,
                }))
                .await;
        }
    }
}
