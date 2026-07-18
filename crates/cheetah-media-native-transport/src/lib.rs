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

/// Lifecycle state of the native byte source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceState {
    /// `start()` has not been called (or was cancelled).
    Idle,
    /// A transport task is running.
    Running,
    /// The transport task has ended; further reads are terminal.
    Finished,
}

/// Platform-native `ByteSource` supporting `tcp://`, `http(s)://` and `ws(s)://`.
pub struct NativeByteSource {
    runtime: Runtime,
    buffer: Vec<u8>,
    task: Option<JoinHandle<()>>,
    rx: Option<mpsc::Receiver<Chunk>>,
    stats: SourceStats,
    state: SourceState,
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
            state: SourceState::Idle,
        })
    }

    /// Parse a URL into `(scheme, host, port, path_or_query)`.
    ///
    /// This is intentionally minimal: the adapters only need scheme, host and
    /// the full path/query string. Ports default based on the scheme.
    fn parse_url(url: &str) -> Option<(&'static str, String, Option<u16>, String)> {
        let (scheme, rest) = url.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        let scheme = match scheme.as_str() {
            "tcp" => "tcp",
            "http" => "http",
            "https" => "https",
            "ws" => "ws",
            "wss" => "wss",
            _ => return None,
        };

        let (authority, path) = Self::split_authority_and_path(rest);
        let (host, port) = Self::parse_authority(authority)?;

        Some((scheme, host, port, path))
    }

    fn split_authority_and_path(rest: &str) -> (&str, String) {
        if let Some(idx) = rest.find(&['/', '?', '#'][..]) {
            let authority = &rest[..idx];
            let path = if rest.as_bytes()[idx] == b'/' {
                rest[idx..].to_string()
            } else {
                format!("/{}", &rest[idx..])
            };
            (authority, path)
        } else {
            (rest, "/".into())
        }
    }

    fn parse_authority(authority: &str) -> Option<(String, Option<u16>)> {
        if authority.is_empty() {
            return None;
        }
        // Strip optional userinfo (e.g. user:pass@) before host/port parsing.
        let authority = authority
            .rsplit_once('@')
            .map(|(_, host)| host)
            .unwrap_or(authority);

        if let Some(end) = authority.find(']') {
            if !authority.starts_with('[') {
                return None;
            }
            let host = authority[..=end].to_string();
            let rest = &authority[end + 1..];
            if rest.is_empty() {
                return Some((host, None));
            }
            let port = rest.strip_prefix(':')?.parse::<u16>().ok()?;
            return Some((host, Some(port)));
        }

        if let Some((h, p)) = authority.rsplit_once(':') {
            if h.is_empty() {
                return None;
            }
            // Reject unbracketed IPv6 addresses (e.g. 2001:db8::1) that would
            // otherwise be parsed as a host:port pair.
            if !authority.contains('@') && h.contains(':') {
                return None;
            }
            let port = p.parse::<u16>().ok()?;
            return Some((h.to_string(), Some(port)));
        }

        Some((authority.to_string(), None))
    }

    fn reset(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.rx = None;
        self.buffer.clear();
        self.state = SourceState::Idle;
    }
}

impl Drop for NativeByteSource {
    fn drop(&mut self) {
        // Abort any running transport task so dropping the runtime does not
        // block on a stuck network connection.
        self.reset();
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

        if scheme == "tcp" && port == 0 {
            return Err(ByteSourceError::Fatal {
                code: 2,
                context: Some("missing_tcp_port"),
            });
        }

        let (tx, rx) = mpsc::channel(16);

        let is_live = matches!(scheme, "ws" | "wss");
        self.stats = SourceStats {
            is_live,
            ..SourceStats::default()
        };

        let task = match scheme {
            "tcp" => self.runtime.spawn(tcp::run(host, port, tx)),
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
        self.state = SourceState::Running;
        Ok(())
    }

    fn read_or_push<'a>(&'a mut self, _buf: &mut [u8]) -> ByteSourceEvent<'a> {
        // The previous slice is invalidated now; count whatever was outstanding
        // as consumed per the ByteSource contract.
        if !self.buffer.is_empty() {
            self.stats.bytes_consumed = self
                .stats
                .bytes_consumed
                .saturating_add(self.buffer.len() as u64);
            self.buffer.clear();
        }

        if self.rx.is_none() {
            return match self.state {
                SourceState::Idle => ByteSourceEvent::Error(ByteSourceError::NotStarted),
                SourceState::Finished => ByteSourceEvent::Eof,
                SourceState::Running => {
                    // We are Running but the channel is gone; treat as terminal EOF.
                    self.state = SourceState::Finished;
                    ByteSourceEvent::Eof
                }
            };
        }

        let Some(rx) = self.rx.as_mut() else {
            // The rx was cleared after the non-None check above; treat as a
            // fatal transport inconsistency rather than panicking.
            return ByteSourceEvent::Error(ByteSourceError::Fatal {
                code: 12,
                context: Some("transport_receiver_missing"),
            });
        };

        match rx.try_recv() {
            Ok(Chunk::Data(data)) => {
                self.stats.bytes_received =
                    self.stats.bytes_received.saturating_add(data.len() as u64);
                self.buffer = data;
                ByteSourceEvent::Data(&self.buffer)
            }
            Ok(Chunk::Eof) => {
                self.rx = None;
                self.state = SourceState::Finished;
                ByteSourceEvent::Eof
            }
            Ok(Chunk::Error(e)) => {
                self.rx = None;
                self.state = SourceState::Finished;
                ByteSourceEvent::Error(e)
            }
            Err(mpsc::error::TryRecvError::Empty) => ByteSourceEvent::Live,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.rx = None;
                self.state = SourceState::Finished;
                ByteSourceEvent::Error(ByteSourceError::Fatal {
                    code: 11,
                    context: Some("transport_task_disconnected"),
                })
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
