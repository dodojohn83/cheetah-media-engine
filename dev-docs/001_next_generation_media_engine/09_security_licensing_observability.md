# 09. 安全、许可证与可观测性

## 1. Web 安全边界

播放器处理不可信网络媒体。所有 parser、demux、bitstream 和 decoder 输入必须：

- 增量解析且具有长度、深度、轨道数、NALU 数、box/segment 大小上限；
- 对整数溢出、offset 越界、时间戳极值和压缩炸弹返回错误；
- malformed input 不 panic、不无限循环、不申请输入声明的无界内存；
- 对连续错误有熔断和重连上限；
- 不将原始媒体 payload 写入默认日志或诊断包。

WASM 限制不能替代资源上限。codec pack OOM、trap 和 Worker crash 必须被宿主隔离并转换为可诊断失败。

## 2. CORS、COOP/COEP 与 CSP

### 2.1 基础部署

- stream、playlist、segment、WASM、Worker 和 codec pack 必须满足 CORS；
- WebSocket 服务端验证 Origin；
- snapshot/video canvas 使用要求服务端返回允许的跨域头；
- credentials 模式与通配 Origin 组合必须符合浏览器规则。

### 2.2 隔离优化档

启用 SharedArrayBuffer 时顶层应用配置：

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

或采用目标浏览器验证通过的等价 COEP 策略。所有跨域子资源必须提供 CORS/CORP。SDK 必须检测 `crossOriginIsolated`，不满足时降级而不是启动后崩溃。

### 2.3 CSP

- 支持 self-host Worker/WASM/codec pack；
- 避免依赖 `eval` 和运行时生成未授权脚本；
- 文档列出 worker-src、script-src、connect-src、media-src 等最小策略；
- CDN 制品提供内容哈希和 SRI；
- SDK 不从未配置的第三方域名自动加载代码。

## 3. 鉴权与敏感数据

- Fetch 支持调用方提供 credentials 和 headers；
- WebSocket 使用 cookie、query token 或 subprotocol，明确其泄漏风险；
- URL、Authorization、cookie、token、密钥和含 userinfo URI 必须脱敏；
- 诊断包默认只保留 origin、协议类别和 hash 后的 source identity；
- SM4/XOR/AES 等密钥通过 SecretProvider/回调取得，不长期保存在普通配置或 localStorage；
- snapshot/recording 文件名和 metadata 不包含未经清理的远端输入。

## 4. LGPL codec pack

### 4.1 边界

- engine/core 使用 MIT OR Apache-2.0；
- FFmpeg/libavcodec 等 LGPL 代码不静态链接进核心 WASM；
- 每个 codec pack 是独立、可替换、可禁用的构建产物；
- 分发包包含许可证、版权声明、构建脚本、准确 source offer 和修改记录；
- 应用允许用户用兼容 ABI 替换 codec pack；
- GPL/nonfree 构建与默认 LGPL 构建严格分离，不能误入正式制品。

### 4.2 专利与商标

开源许可证不授予 H.264/H.265/AAC 等专利权。每个发布地区和商业分发方式必须进行独立法律评估。文档不得把“开源 decoder”描述为“无需专利许可”。

第三方名称只用于兼容和参考，不暗示官方认证或合作关系。

## 5. 供应链

- Rust、npm、WASM toolchain、FFmpeg commit 和编译器版本全部锁定；
- 生成 SBOM、许可证清单和校验和；
- CI 执行依赖 advisory、license 和 provenance 检查；
- codec pack 构建可重复并保存构建参数；
- 发布 manifest 签名并包含 engine/ABI/version/hash；
- 运行时拒绝 hash、签名或 ABI 不匹配的可选模块。

## 6. 错误与诊断

稳定错误至少包括：

- Network、Cors、Auth、Timeout；
- UnsupportedProtocol、UnsupportedCodec、UnsupportedBackend；
- MalformedStream、MissingRandomAccessPoint；
- Decode、Render、Audio、MseQuota；
- OutOfMemory、QueueOverflow、WorkerCrashed；
- RecordingSink、PermissionDenied；
- Aborted、Destroyed、Internal。

错误对外包含 code、stage、retryable、backend、generation、correlation id 和安全 message。原始异常、URL query 和 payload 只允许进入显式、脱敏且有上限的 debug 诊断。

## 7. Metrics

每实例记录：

- 首字节、tracks ready、首个关键帧、首个解码帧和首个呈现帧；
- 直播延迟、demux/decode/render 耗时和 A/V 差；
- 输入、解码、呈现、丢弃和损坏帧；
- 网络、Packet、GOP、decoder、renderer、audio、MSE、record queue；
- JS heap、WASM arena、Frame/texture pool 水位；
- 显式复制次数/字节、GPU upload/readback；
- fallback、重连、quality switch 和后台恢复；
- codec pack、Worker 和 GPU device/context 故障。

多画面统计同时提供 wall 级总资源和每 tile 明细，避免只观察单实例。

## 8. Logging 与 tracing

- 默认日志级别为 warn/error，debug 使用固定大小环形缓冲；
- 不逐帧输出普通日志；
- trace span 至少覆盖 load generation、network request、probe、fallback、quality switch 和 recording；
- source URL、token、密钥和完整 codec payload 禁止记录；
- 高频 ID 不作为默认 metrics label；
- 下载诊断包包含版本、能力、统计、有限事件和脱敏配置，不包含媒体内容。

## 9. 遥测策略

SDK 默认不访问业务 stream 之外的任何远端遥测服务。业务可通过 DiagnosticsSink 订阅聚合指标并自行上报。遥测 callback 队列必须有界，慢 callback 只丢遥测，不能影响播放。

## 10. 安全测试

- parser/property/fuzz 覆盖任意切片、极端长度和结构嵌套；
- CORS/Origin、CSP、COOP/COEP 配置组合；
- token/secret 在 error、log、stats、URL 和诊断包中的泄漏扫描；
- 恶意 codec pack manifest、ABI 和 hash；
- Worker crash、WASM trap、OOM 和 GPU device lost；
- 录制文件名、下载和 Blob 内存上限；
- 第三方依赖 advisory、license、SBOM 和可重复构建。
