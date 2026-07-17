# WP-57: 原生 HTTP/WS/TCP transport adapter（tokio）

## 1. 范围

在 `cheetah-media-backend-api` 的 `ByteSource` trait 基础上，建立 `cheetah-media-native-transport` crate，提供基于 tokio 的原生网络字节源：

- `TcpByteSource`：`tcp://host:port` 原始字节流。
- `HttpByteSource`：`http://` / `https://` 渐进式下载，通过 `reqwest` 流式读取响应体。
- `WebSocketByteSource`：`ws://` / `wss://` 二进制/文本帧，通过 `tokio-tungstenite`。
- 统一的 `NativeByteSource` 入口，按 URL scheme 选择实现，内部维护一个 tokio runtime 与 mpsc 通道。
- `ByteSource` 接口：`start` / `read_or_push` / `cancel` / `stats`。
- 单元测试：本地 TCP/HTTP/WS 回环服务器，验证数据往返、`Eof`、取消、未知 scheme 错误。

本 crate 只负责把网络字节喂到 `ByteSource` 接口；demux/decode/render 由后续 WP 集成。

## 2. 交付物

- `crates/cheetah-media-native-transport/Cargo.toml`
- `crates/cheetah-media-native-transport/src/lib.rs`
- `crates/cheetah-media-native-transport/src/tcp.rs`
- `crates/cheetah-media-native-transport/src/http.rs`（reqwest 流式）
- `crates/cheetah-media-native-transport/src/ws.rs`（tokio-tungstenite）
- `crates/cheetah-media-native-transport/src/error.rs`
- 单元测试覆盖三种 transport
- `dev-docs/004/57_native_transport.md` 与 baseline 状态更新

## 3. 接口草案

```rust
use cheetah_media_backend_api::ByteSource;

pub struct NativeByteSource;

impl NativeByteSource {
    pub fn new() -> Self;
}

impl ByteSource for NativeByteSource {
    fn start(&mut self, url: &str) -> Result<(), ByteSourceError>;
    fn read_or_push<'a>(&'a mut self, _buf: &mut [u8]) -> ByteSourceEvent<'a>;
    fn cancel(&mut self) -> Result<(), ByteSourceError>;
    fn stats(&self) -> SourceStats;
}
```

`start` 解析 URL scheme：
- `tcp://` -> `TcpByteSource`
- `http://` / `https://` -> `HttpByteSource`
- `ws://` / `wss://` -> `WebSocketByteSource`

`read_or_push` 从内部 tokio 任务 mpsc 拉取字节，返回 `Data(&[u8])`、`Live` 或 `Eof`。

## 4. 验证

```bash
cargo test -p cheetah-media-native-transport
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [x] crate 创建与依赖（tokio / reqwest / tokio-tungstenite）
- [x] `ByteSource` trait 实现与本地回环测试
- [x] Rust 全矩阵验证通过
- [ ] CI / Devin Review 通过并合并
