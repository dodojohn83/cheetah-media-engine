# 13. HLS/LL-HLS Client 与直播策略

## HLS-001：Playlist 模型和解析

- [ ] 支持 master/media playlist、variant、media sequence、target duration、map、byterange、discontinuity、endlist。
- [ ] LL-HLS 支持 part、part-inf、server-control、preload-hint、skip 和 rendition-report。
- [ ] URI 解析遵循基准 URL；禁止自动携带跨域凭证，header/cookie 策略由调用方显式配置。
- [ ] 标签、行、URI、variant/segment 数和 playlist 总大小均有限制。

## HLS-002：Variant 选择和主子码流接口

- [ ] 将带宽、分辨率、codec、音频组和 URL 转换为稳定 `VariantInfo`。
- [ ] 初始选择服从用户约束和 capability；缺少精确信息时选择保守可解码 variant。
- [ ] 手动/自动切换输出原因，等待兼容边界，旧请求通过 epoch 取消。
- [ ] CheetahWall 的全局预算器可对每个实例提出目标 variant，不由 HLS client 自行争抢资源。

## HLS-003：live reload、取消和追赶

- [ ] reload 间隔、blocking reload、skip、part preload 均有超时、重试上限和 jitter。
- [ ] 去重 segment/part，检测窗口跳跃、sequence 回退和 discontinuity。
- [ ] 所有下载可取消；stop/destroy 后不得继续发请求或投递数据。
- [ ] live latency 超阈值时由策略选择跳过到独立解码点，不通过无限倍速追赶。

## HLS-004：网络和安全验证

- [ ] fake HTTP server 覆盖 200/206/304、重定向、超时、404/410/5xx、短读和错误 MIME。
- [ ] 测试 TS/fMP4、LL parts、byte range、playlist rollover 和 variant 切换。
- [ ] SSRF/混合内容/CORS/凭证策略在 SDK 配置中显式暴露和拒绝。
- [ ] 所有重试、下载并发、缓存字节和历史 ID 有可观测上限。

