# 32. 浏览器 E2E、兼容性与故障注入

## E2E-001：浏览器矩阵

| 环境 | 必验内容 |
| --- | --- |
| Chrome stable / Windows、macOS | WebCodecs、MSE、WASM threads/SIMD、WebGPU/WebGL |
| Edge stable / Windows | 硬解密度、MSE、企业常见安全策略 |
| Safari stable / macOS | WebCodecs/MSE 实际支持组合、WebGL、音频生命周期 |
| Firefox stable / Windows、macOS | MSE/软解回退、WebGL、非隔离路径 |

- [ ] 每次运行记录浏览器完整版本、OS、GPU、driver、isolation 和 capability snapshot。
- [ ] API 缺失不是测试跳过理由；必须断言预期下一路线或 Unsupported。

## E2E-002：协议/codec 核心矩阵

- [ ] HTTP-FLV、WS-FLV：H.264/H.265 × AAC/G.711A/U/MP3 的适用组合。
- [ ] HLS/LL-HLS：TS 与 fMP4、variant、discontinuity、窗口滑动和 parts。
- [ ] HTTP/WS-fMP4：init/media 跨任意 chunk、config change 和断流。
- [ ] 每条路径验证首帧、连续播放、A/V sync、stop/reload/destroy 和实际 backend。

## E2E-003：故障注入目录

- [ ] 网络：DNS/连接失败、HTTP 错误、stall、短读、断连、乱序脚本和重连。
- [ ] WebCodecs：support 误报、configure/decode/async error、queue stall。
- [ ] MSE：MIME 拒绝、append error、QuotaExceeded、media element error。
- [ ] WASM：pack 404/hash/ABI 错、OOM、worker crash、decoder error。
- [ ] 渲染/音频：device/context lost、AudioContext suspend、surface detach、后台恢复。
- [ ] 每次注入断言状态、事件顺序、下一后端、恢复耗时和资源 ledger。

## E2E-004：部署模式

- [ ] isolated demo 使用正确 COOP/COEP/CORP，证明 threads+SIMD。
- [ ] non-isolated demo 不使用 SharedArrayBuffer，证明 SIMD/baseline 和相同 SDK API。
- [ ] self-host/CDN、不同 base URL、严格 CSP、错误 WASM MIME 和跨域 codec pack 分别验证。
- [ ] Playwright trace/video 仅在失败保存并脱敏；flaky 重跑仍保留首次失败证据。

