//! Native tokio-based HTTP/WS/TCP byte source adapters.
//!
//! Implements `cheetah_media_backend_api::ByteSource` so the platform-neutral
//! engine can drive network I/O without taking a dependency on `tokio` itself.

use cheetah_media_backend_api::{ByteSource, ByteSourceError, ByteSourceEvent, SourceStats};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "websocket")]
mod ws;

/// A chunk delivered from the background transport task to the synchronous
/// `ByteSource` consumer.
enum Chunk {
    Data(Vec<u8>),
    Eof,
    Error(ByteSourceError),
}

/// Platform-native `ByteSource` supporting `tcp://`, `http(s)://` and `ws(s)://`.
pub struct NativeByteSource {
    runtime: Runtime,
    buffer: Vec<u8>,
    task: Option<JoinHandle<()>>,
    rx: Option<mpsc::Receiver<Chunk>>,
    stats: SourceStats,
}

impl Default for NativeByteSource {
    fn default() -> Self {
        Self::new().expect("default NativeByteSource should build a tokio runtime")
    }
}

impl NativeByteSource {
    /// Create a new source with its own tokio runtime.
    pub fn new() -> Result<Self, ByteSourceError> {
        let runtime = Runtime::new().map_err(|_| ByteSourceError::Fatal {
            code: 10,
            context: Some("runtime_creation_failed"),
        })?;
        Ok(Self {
            runtime,
            buffer: Vec::with_capacity(8192),
            task: None,
            rx: None,
            stats: SourceStats::default(),
        })
    }

    /// Parse a URL into `(scheme, host, port, path_or_query)`.
    ///
    /// This is intentionally minimal: the adapters only need scheme, host and
    /// the full path/query string. Ports default based on the scheme.
    fn parse_url(url: &str) -> Option<(&'static str, String, Option<u16>, String)> {
        let (scheme, rest) = url.split_once("://")?;
        let scheme = match scheme {
            "tcp" => "tcp",
            "http" => "http",
            "https" => "https",
            "ws" => "ws",
            "wss" => "wss",
            _ => return None,
        };

        let (authority, path) = match rest.split_once('/') {
            Some((auth, p)) => (auth, format!("/{}", p)),
            None => (rest, "/".into()),
        };

        let (host, port) = if let Some((h, p)) = authority.rsplit_once(':') {
            let port = p.parse::<u16>().ok()?;
            (h.to_string(), Some(port))
        } else {
            (authority.to_string(), None)
        };

        Some((scheme, host, port, path))
    }

    fn reset(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.rx = None;
        self.buffer.clear();
    }
}

impl ByteSource for NativeByteSource {
    fn start(&mut self, url: &str) -> Result<(), ByteSourceError> {
        self.cancel()?;

        let (scheme, host, port, path) = Self::parse_url(url).ok_or(ByteSourceError::Fatal {
            code: 1,
            context: Some("unsupported_url_scheme"),
        })?;

        #[cfg(not(any(feature = "http", feature = "websocket")))]
        let _ = &path;

        let default_port = match scheme {
            "http" | "ws" => 80,
            "https" | "wss" => 443,
            "tcp" => 0,
            _ => 0,
        };
        let port = port.unwrap_or(default_port);

        let (tx, rx) = mpsc::channel(16);

        let is_live = matches!(scheme, "ws" | "wss");
        self.stats = SourceStats {
            is_live,
            ..SourceStats::default()
        };

        let task = match scheme {
            "tcp" => {
                if port == 0 {
                    return Err(ByteSourceError::Fatal {
                        code: 2,
                        context: Some("missing_tcp_port"),
                    });
                }
                self.runtime.spawn(tcp::run(host, port, tx))
            }
            #[cfg(feature = "http")]
            "http" | "https" => {
                let full = if path.is_empty() || path == "/" {
                    format!("{}://{}:{}", scheme, host, port)
                } else {
                    format!("{}://{}:{}{}", scheme, host, port, path)
                };
                self.runtime.spawn(http::run(full, tx))
            }
            #[cfg(feature = "websocket")]
            "ws" | "wss" => {
                let full = if path.is_empty() || path == "/" {
                    format!("{}://{}:{}", scheme, host, port)
                } else {
                    format!("{}://{}:{}{}", scheme, host, port, path)
                };
                self.runtime.spawn(ws::run(full, tx))
            }
            _ => {
                return Err(ByteSourceError::Fatal {
                    code: 1,
                    context: Some("unsupported_url_scheme"),
                });
            }
        };

        self.task = Some(task);
        self.rx = Some(rx);
        Ok(())
    }

    fn read_or_push<'a>(&'a mut self, _buf: &mut [u8]) -> ByteSourceEvent<'a> {
        // The previous slice is invalidated now; count whatever was outstanding
        // as consumed per the ByteSource contract.
        if !self.buffer.is_empty() {
            self.stats.bytes_consumed += self.buffer.len() as u64;
            self.buffer.clear();
        }

        let Some(rx) = self.rx.as_mut() else {
            return ByteSourceEvent::Error(ByteSourceError::NotStarted);
        };

        match rx.try_recv() {
            Ok(Chunk::Data(data)) => {
                self.stats.bytes_received += data.len() as u64;
                self.buffer = data;
                ByteSourceEvent::Data(&self.buffer)
            }
            Ok(Chunk::Eof) => {
                self.rx = None;
                ByteSourceEvent::Eof
            }
            Ok(Chunk::Error(e)) => {
                self.rx = None;
                ByteSourceEvent::Error(e)
            }
            Err(mpsc::error::TryRecvError::Empty) => ByteSourceEvent::Live,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.rx = None;
                ByteSourceEvent::Eof
            }
        }
    }

    fn cancel(&mut self) -> Result<(), ByteSourceError> {
        self.reset();
        self.stats = SourceStats::default();
        Ok(())
    }

    fn stats(&self) -> SourceStats {
        self.stats
    }
}

#[cfg(test)]
mod tests;

mod tcp {
    use super::Chunk;
    use cheetah_media_backend_api::ByteSourceError;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpStream;
    use tokio::sync::mpsc;

    pub(crate) async fn run(host: String, port: u16, tx: mpsc::Sender<Chunk>) {
        match TcpStream::connect((host.as_str(), port)).await {
            Ok(mut stream) => {
                let mut buf = vec![0u8; 8192];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) => {
                            let _ = tx.send(Chunk::Eof).await;
                            break;
                        }
                        Ok(n) => {
                            if tx.send(Chunk::Data(buf[..n].to_vec())).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = tx
                                .send(Chunk::Error(ByteSourceError::Fatal {
                                    code: 20,
                                    context: Some("tcp_read_error"),
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
                        reason: "tcp_connect_failed",
                        backoff_ms: 1000,
                    }))
                    .await;
            }
        }
    }
}
