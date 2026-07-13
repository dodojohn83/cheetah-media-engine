# 15. Media Server 兼容门面迁移

## SRV-001：修复 server WASM/no_std 基线

**仓库**：server。**前置**：REPO-001。**范围**：仅修复已确认的 `alloc` 导入和相应测试。

- [ ] 在 no_std 路径显式导入 `alloc::vec::Vec`、`alloc::boxed::Box`，不得通过开启 std 绕过。
- [ ] 对 `cheetah-codec` no-default-features 和 `cheetah-http-flv-core` wasm32 顺序构建。
- [ ] 使用隔离 `CARGO_TARGET_DIR` 重跑，区分源码失败和并发缓存碰撞。
- [ ] 记录基线 commit、命令和结果，作为 core 迁移前门禁。

## SRV-002：把 `cheetah-codec` 改为兼容门面

- [ ] 保持现有 crate/package 名和常用导入路径，通过 re-export/薄适配消费 core 固定 tag。
- [ ] 对外类型尽量直接 re-export；需要转换时证明无 payload 复制，并标注弃用周期。
- [ ] server 专属 session、协议 driver、模块配置和业务事件不得迁入 core。
- [ ] facade feature 映射保持默认行为；新增 core feature 不得意外扩大 server 二进制。

## SRV-003：按能力批次迁移

迁移顺序固定为：媒体类型/时间 → bitstream → FLV → TS/ISOBMFF → HLS core → timeline。每批执行：

- [ ] 固定旧实现 fixture 输出。
- [ ] 引入 core tag 并建立旧/新双跑比较。
- [ ] 迁移调用方且运行 workspace/协议 targeted test。
- [ ] 记录性能、内存和依赖图变化。
- [ ] 一个发布周期后删除重复代码，后续 bugfix 只进入 core。

## SRV-004：跨仓 contract 和回滚

- [ ] server 与 engine 对共享 manifest 运行完全相同的 parser/timeline contract。
- [ ] 差异按字段输出，禁止只比较序列化字符串或播放器画面。
- [ ] 发布 server 时记录 core tag；回滚只需恢复上一个 facade 依赖版本。
- [ ] 若新 core 破坏线上协议行为，先回滚 tag，再在 core 修复并发布新 tag，禁止 server/core 双写补丁。

