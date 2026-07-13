# 05. 迁移清单、Fixture 与许可证

## MIG-001：盘点 media server 可复用能力

- [ ] 以固定 server commit 为基线，逐模块登记所有者、依赖、feature、测试、许可证和 Web/no_std 可用性。
- [ ] 优先盘点 `crates/foundation/cheetah-codec` 与 `crates/protocols/hls/core` 的类型、parser、player、pacer 和 fixture。
- [ ] HTTP-FLV/fMP4 仅提取纯容器和媒体能力；server session、driver、module、HTTP handler 留在 server。
- [ ] 为每项标注 `Move`、`Adapt`、`Keep`、`Replace`，并说明理由和目标 crate。

## MIG-002：建立 fixture 资产台账

每个 fixture manifest 必须记录：稳定 ID、来源 URL/仓库/commit、许可证、hash、脱敏、协议、codec、分辨率、帧率、时长、异常特征和预期输出摘要。

- [ ] 建立正常最小流、真实长流、边界流、损坏流、时间戳跳变和 codec config 变化集合。
- [ ] 大文件存对象存储并固定 hash；仓库只保留最小可审查 fixture 和 manifest。
- [ ] 禁止提交含凭证、设备标识、地理信息、人员画面或未经授权的监控录像。
- [ ] core/server/engine 必须读取同一 fixture manifest，禁止各自维护不一致副本。

## MIG-003：许可证和 FFmpeg 边界

- [ ] core/engine 自有 Rust 代码采用 MIT OR Apache-2.0；第三方依赖进入 allowlist。
- [ ] FFmpeg 固定 8.1.2，配置关闭 `--enable-gpl` 和 `--enable-nonfree`，只链接所需 LGPL decoder/util/resampler。
- [ ] codec pack 与主 SDK 独立下载、独立 manifest、独立 NOTICE，可被用户替换或完全移除。
- [ ] G.711A/U 使用 Rust 实现，不为其引入 FFmpeg。
- [ ] CI 验证 configure 输出、库列表、许可证文本、source offer/SBOM 和制品 hash。

## MIG-004：迁移一致性和删除门禁

1. 固定原始实现 commit 和 fixture。
2. 在 core 实现并运行 golden/contract。
3. server facade 双跑新旧实现，比较结构化输出和错误。
4. engine 使用同一 core tag 验证 WASM。
5. 观察一个发布周期后再删除重复实现。

**验收**：差异必须有明确兼容决策；不得以“浏览器能播”替代时间戳、参数集和错误语义的一致性证明。

