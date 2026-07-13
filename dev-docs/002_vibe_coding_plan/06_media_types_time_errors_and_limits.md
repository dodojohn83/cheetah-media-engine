# 06. 媒体类型、时间、错误与资源上限

## CORE-001：定义媒体标识和时间类型

**仓库**：core。**crate**：`cheetah-media-types`。**前置**：ARCH-001。

- [ ] 定义强类型 `TrackId`、`StreamEpoch`、`SequenceNumber`，禁止裸整数跨层混用。
- [ ] 定义约分且分母非零的 `TimeBase`，以及带 checked rescale/add/sub 的 `MediaTime`。
- [ ] Packet 同时保留 DTS、PTS、duration；未知值使用显式 Optional/Unknown，不使用魔数。
- [ ] 定义 timestamp wrap、discontinuity、回退和溢出的确定语义。
- [ ] 所有时间换算使用整数和明确舍入规则，热路径禁止浮点累积时钟。

**测试**：极值、负 CTS、33-bit wrap、不同 timebase 往返、溢出、单调性和 property test。

## CORE-002：定义 Track、Packet 和 Frame

- [ ] `TrackInfo` 包含 kind、codec、timebase、codec config、视频/音频格式和 generation。
- [ ] `MediaPacket` 包含共享 payload、track、epoch、DTS/PTS/duration、关键帧/损坏/discontinuity flags。
- [ ] `VideoFrame` 明确像素格式、coded/visible size、stride、color space、timestamp 和外部资源句柄。
- [ ] `AudioFrame` 明确 sample format、sample rate、channel layout、sample count、平面描述和 timestamp。
- [ ] codec config 变化必须递增 generation；旧 generation 的 frame 不得进入新 renderer/sink。

**禁止**：在核心类型中保存 DOM object、WebCodecs object、裸平台指针或模糊 `metadata: Map`。

## CORE-003：稳定错误模型

- [ ] 顶层错误分类固定为 InvalidInput、Unsupported、ResourceLimit、Timeout、Cancelled、BackendFailure、InternalInvariant。
- [ ] 错误携带稳定 code、stage、recoverability、可脱敏 context 和 source chain。
- [ ] parser 返回消费字节/需要更多数据/错误 offset；不得把截断等同格式错误。
- [ ] Rust、ABI、WASM 和 TypeScript 的错误码建立一对一表，并测试未知新码的前向兼容。

## CORE-004：统一资源限制

- [ ] 定义 `MediaLimits`：最大 track、box/tag/PES、参数集、分辨率、帧大小、缓存时长、队列深度。
- [ ] 默认值覆盖 v1 指标但拒绝明显恶意输入；用户只能在安全上下限内调整。
- [ ] 每次拒绝记录 limit 名、当前值和阈值，不记录 payload。
- [ ] 对所有上限补边界值、超限和长期重复攻击测试。

**完成命令**：fmt、clippy、all-features/no-default-features test、wasm32 build、property suite 全通过。

