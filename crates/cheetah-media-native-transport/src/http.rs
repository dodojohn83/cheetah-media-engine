use super::Chunk;
use cheetah_media_backend_api::ByteSourceError;
use futures_util::StreamExt;
use tokio::sync::mpsc;

pub(crate) async fn run(url: String, tx: mpsc::Sender<Chunk>) {
    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let mut stream = resp.bytes_stream();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(bytes) => {
                        if tx.send(Chunk::Data(bytes.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = tx
                            .send(Chunk::Error(ByteSourceError::Fatal {
                                code: 30,
                                context: Some("http_body_error"),
                            }))
                            .await;
                        break;
                    }
                }
            }
            let _ = tx.send(Chunk::Eof).await;
        }
        Ok(_) => {
            let _ = tx
                .send(Chunk::Error(ByteSourceError::Fatal {
                    code: 31,
                    context: Some("http_status_error"),
                }))
                .await;
        }
        Err(_) => {
            let _ = tx
                .send(Chunk::Error(ByteSourceError::Retryable {
                    reason: "http_request_failed",
                    backoff_ms: 1000,
                }))
                .await;
        }
    }
}
