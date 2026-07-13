# 31. Testkit、Fixture、Property、Fuzz 与 Contract

## QA-001：共享 testkit

**仓库**：core 为 fixture 真源，engine 提供 `cheetah-media-testkit` adapter，server 只消费固定版本。

- [ ] manifest loader 校验 schema、hash、license、protocol/codec、预期 tracks/packets/timeline/errors。
- [ ] fake clock、chunk splitter、bounded sink、failing backend、scripted transport 可在 native/WASM 复用。
- [ ] comparison 按字段和时间容差输出首个差异，不依赖 Debug 字符串或 HashMap 顺序。
- [ ] 大 fixture 下载到内容寻址 cache；离线缺失时明确 skip 仅限非 Required 套件。

## QA-002：Golden 和 contract suite

- [ ] parser golden 覆盖正常、所有 chunk 边界、截断、损坏、超限、config/timestamp discontinuity。
- [ ] server/core/engine 输出统一 canonical manifest，比较 Track、Packet metadata、payload hash 和 timeline。
- [ ] backend contract 对 configure/submit/flush/reset/close 的成功、失败、取消和迟到 callback 使用同一套测试。
- [ ] SDK contract 在 mock runtime 和真实 WASM 上验证状态、事件、错误和幂等。

## QA-003：Property 和状态模型

- [ ] 任意字节输入不得 panic/越界/无限循环；成功消费必须推进或明确 NeedMoreData。
- [ ] timebase rescale、NAL 转换、demux/mux 往返和 buffer ownership 建立 property。
- [ ] engine command/callback 序列用 model test 验证状态合法、epoch 隔离和资源 ledger 归零。
- [ ] 生成器的大小和运行时间受限，失败 case 固化为 regression fixture。

## QA-004：Fuzz 运维

- [ ] targets 覆盖 FLV、TS PSI/PES、ISOBMFF box/sample、H264/H265 config、AAC/MP3、ABI exports。
- [ ] 每 PR 运行短 smoke；nightly 使用固定预算；release 前运行扩展 corpus。
- [ ] crash、OOM、timeout、excess allocation 分别归类；修复必须加入最小 corpus。
- [ ] corpus 不包含受限媒体或秘密，归档时记录工具链和 commit。

