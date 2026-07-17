use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use cheetah_media_backend_api::{ByteSource, ByteSourceError, ByteSourceEvent};

#[cfg(feature = "websocket")]
use futures_util::SinkExt;
#[cfg(feature = "websocket")]
use tokio::runtime::Runtime;

use super::NativeByteSource;

fn read_all(source: &mut NativeByteSource, timeout: Duration) -> Vec<u8> {
    let start = Instant::now();
    let mut out = Vec::new();
    while start.elapsed() < timeout {
        match source.read_or_push(&mut []) {
            ByteSourceEvent::Data(chunk) => out.extend_from_slice(chunk),
            ByteSourceEvent::Eof => break,
            ByteSourceEvent::Live => thread::sleep(Duration::from_millis(10)),
            ByteSourceEvent::Error(_) => break,
        }
    }
    out
}

fn tcp_server(body: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(&body).unwrap();
    });
    port
}

#[cfg(feature = "http")]
fn http_server(body: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
        stream.write_all(header.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
    });
    port
}

#[cfg(feature = "websocket")]
fn ws_server(body: Vec<u8>) -> u16 {
    let rt = Runtime::new().unwrap();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        rt.block_on(async {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws.send(tokio_tungstenite::tungstenite::protocol::Message::Binary(
                body,
            ))
            .await
            .unwrap();
        });
    });
    port
}

#[test]
fn tcp_roundtrip() {
    let port = tcp_server(b"hello world".to_vec());
    let mut src = NativeByteSource::new().unwrap();
    src.start(&format!("tcp://127.0.0.1:{}", port)).unwrap();
    let data = read_all(&mut src, Duration::from_secs(5));
    assert_eq!(data, b"hello world");
    assert!(src.stats().bytes_received >= 11);
}

#[test]
#[cfg(feature = "http")]
fn http_roundtrip() {
    let port = http_server(b"hello world".to_vec());
    let mut src = NativeByteSource::new().unwrap();
    src.start(&format!("http://127.0.0.1:{}/", port)).unwrap();
    let data = read_all(&mut src, Duration::from_secs(5));
    assert_eq!(data, b"hello world");
    assert!(src.stats().bytes_received >= 11);
}

#[test]
#[cfg(feature = "websocket")]
fn websocket_roundtrip() {
    let port = ws_server(b"hello world".to_vec());
    let mut src = NativeByteSource::new().unwrap();
    src.start(&format!("ws://127.0.0.1:{}/", port)).unwrap();
    let data = read_all(&mut src, Duration::from_secs(5));
    assert_eq!(data, b"hello world");
    assert!(src.stats().bytes_received >= 11);
}

#[test]
fn unsupported_scheme_rejected() {
    let mut src = NativeByteSource::new().unwrap();
    let err = src.start("ftp://example.com/").unwrap_err();
    assert!(matches!(err, ByteSourceError::Fatal { .. }));
}

#[test]
fn cancel_resets_source() {
    let mut src = NativeByteSource::new().unwrap();
    let port = tcp_server(b"hello world".to_vec());
    src.start(&format!("tcp://127.0.0.1:{}", port)).unwrap();
    src.cancel().unwrap();
    assert_eq!(
        src.read_or_push(&mut []),
        ByteSourceEvent::Error(super::ByteSourceError::NotStarted)
    );
    assert_eq!(src.stats().bytes_received, 0);
}
