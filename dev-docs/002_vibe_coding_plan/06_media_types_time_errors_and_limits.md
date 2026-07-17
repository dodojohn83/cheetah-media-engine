# 06. 媒体类型、时间、错误与资源上限

## CORE-001：定义媒体标识和时间类型

**仓库**：core。**crate**：`cheetah-media-types`。**前置**：ARCH-001。

- [x] 定义强类型 `TrackId`、`StreamEpoch`、`SequenceNumber`，禁止裸整数跨层混用。
- [x] 定义约分且分母非零的 `TimeBase`，以及带 checked rescale/add/sub 的 `MediaTime`。
- [x] Packet 同时保留 DTS、PTS、duration；未知值使用显式 Optional/Unknown，不使用魔数。
- [x] 定义 timestamp wrap、discontinuity、回退和溢出的确定语义。
- [x] 所有时间换算使用整数和明确舍入规则，热路径禁止浮点累积时钟。

**测试**：极值、负 CTS、33-bit wrap、不同 timebase 往返、溢出、单调性和 property test。

> 实现位置：
> - `crates/cheetah-media-types/src/time.rs`：`TimeBase`、`Timestamp`、`MediaDuration`、`MediaTime`
> - `crates/cheetah-media-types/src/track.rs`：`TrackId`、`StreamEpoch`、`SequenceNumber`
>
> 验证命令：
> ```bash
> cargo fmt --all --check
> cargo clippy --workspace --all-targets --all-features -- -D warnings
> cargo test --workspace --all-features
> cargo test --workspace --no-default-features
> cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
> cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
> cargo deny check
> ```
> 结果：全部通过。timestamp wrap 测试覆盖 33-bit；rescale 测试覆盖 1kHz/90kHz/29.97fps 往返和溢出；`MediaTime::checked_add` 在 `i64::MAX+1` 时返回 `None`。

## CORE-002：定义 Track、Packet 和 Frame

- [x] `TrackInfo` 包含 kind、codec、timebase、codec config、视频/音频格式和 generation。
- [x] `MediaPacket` 包含共享 payload、track、epoch、DTS/PTS/duration、关键帧/损坏/discontinuity flags。
- [x] `VideoFrame` 明确像素格式、coded/visible size、stride、color space、timestamp 和外部资源句柄。
- [x] `AudioFrame` 明确 sample format、sample rate、channel layout、sample count、平面描述和 timestamp。
- [x] codec config 变化必须递增 generation；旧 generation 的 frame 不得进入新 renderer/sink。

**禁止**：在核心类型中保存 DOM object、WebCodecs object、裸平台指针或模糊 `metadata: Map`。

> 实现位置：
> - `crates/cheetah-media-types/src/track.rs`：`TrackInfo`、`CodecConfig`
> - `crates/cheetah-media-types/src/packet.rs`：`MediaPacket`、`PacketFlags`
> - `crates/cheetah-media-types/src/frame.rs`：`VideoFrame`、`AudioFrame`
> - `crates/cheetah-media-types/src/format.rs`：`PixelFormat`、`ColorSpace`、`SampleFormat`、`ChannelLayout`、`VideoFormat`、`AudioFormat`
>
> 外部资源句柄使用 `ExternalFrameHandle(u64)`，值 `0` 表示无外部资源，避免裸指针。

## CORE-003：稳定错误模型

- [x] 顶层错误分类固定为 InvalidInput、Unsupported、ResourceLimit、Timeout、Cancelled、BackendFailure、InternalInvariant。
- [x] 错误携带稳定 code、stage、recoverability、可脱敏 context 和 source chain。
- [ ] parser 返回消费字节/需要更多数据/错误 offset；不得把截断等同格式错误。
- [ ] Rust、ABI、WASM 和 TypeScript 的错误码建立一对一表，并测试未知新码的前向兼容。

> 实现位置：
> - `crates/cheetah-media-types/src/error.rs`：`MediaError`、`Recoverability`
>
> `MediaError` 提供 `code()`、`stage()` 和 `is_recoverable()`；当前 context 使用 `&'static str`。
> parser offset 解析将在容器解析细化任务中补齐；跨语言错误码映射表在后续 ABI/WASM 任务补齐。

## CORE-004：统一资源限制

- [x] 定义 `MediaLimits`：最大 track、box/tag/PES、参数集、分辨率、帧大小、缓存时长、队列深度。
- [x] 默认值覆盖 v1 指标但拒绝明显恶意输入；用户只能在安全上下限内调整。
- [x] 每次拒绝记录 limit 名、当前值和阈值，不记录 payload。
- [ ] 对所有上限补边界值、超限和长期重复攻击测试。

> 实现位置：
> - `crates/cheetah-media-types/src/limits.rs`：`MediaLimits`
>
> 默认限制：16 tracks、16 MiB box/tag/PES、64 参数集、8K 分辨率、128 MiB 帧、30 s 缓存、256 队列深度、1 MiB 读取块。
> 攻击/压力测试在后续 fuzz/property 任务补齐。

**完成命令**：fmt、clippy、all-features/no-default-features test、wasm32 build、property suite 全通过。

> 验证结果：
> ```bash
> cargo fmt --all --check          # passed
> cargo clippy --workspace --all-targets --all-features -- -D warnings  # passed
> cargo test --workspace --all-features          # passed (53 Rust tests)
> cargo test --workspace --no-default-features    # passed
> cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release  # passed
> cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features  # passed
> cargo deny check                 # passed (仅 license-not-encountered 警告)
> source ~/.nvm/nvm.sh && nvm use && corepack pnpm install --frozen-lockfile  # passed
> corepack pnpm typecheck          # passed
> corepack pnpm test               # passed (Vitest + Playwright Chromium)
> corepack pnpm build              # passed
> ```
