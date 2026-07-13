# 26. Web SDK 公共 API、事件与错误

## SDK-001：公共包和导出面

公共包固定为 `@cheetah-media/web`，内部 runtime 不从公共入口泄漏。只导出稳定类型、工厂、player 接口、错误/事件/统计类型和版本信息。

- [ ] 提供 `createPlayer(config)`，返回单一 `CheetahPlayer` 实例；构造不自动发起网络。
- [ ] 方法至少包含 `load`、`play`、`pause`、`stop`、`destroy`、`snapshot`、`startRecording`、`switchVariant`、`getStats`、`exportDiagnostics`。
- [ ] 每个异步方法定义 resolve 时点、拒绝错误、取消、并发调用和 destroy 后行为。
- [ ] 禁止公开 WASM handle、worker、FFmpeg pointer、DOM backend 或可变内部 config。

## SDK-002：配置模型

- [ ] `PlayerConfig` 分 transport、latency、backend、memory、render、audio、recording、security、diagnostics 子对象。
- [ ] 默认值在一个版本化 schema 中生成；未知字段和越界值有确定验证错误。
- [ ] capability preference 允许禁用后端/软解，但不能强制使用实际不支持的路径。
- [ ] 凭证和自定义 header 作为敏感字段，toJSON/diagnostics 默认剔除。

## SDK-003：事件和顺序保证

- [ ] 事件至少包括 statechange、tracks、firstframe、backendchange、variantchange、buffering、stats、warning、error、recording。
- [ ] 所有事件携带 player id、epoch、sequence、monotonic time；错误事件携带 recoverability。
- [ ] statechange 先于该状态产生的业务事件；destroy 后不再触发任何用户 callback。
- [ ] 高频 stats/frame 事件节流并有上限，用户 handler 异常不能破坏 engine。

## SDK-004：错误和版本兼容

- [ ] TypeScript `CheetahMediaError` 一一映射 Rust code/stage/recoverability，并保留安全 cause chain。
- [ ] Unsupported 明确指出 protocol/codec/backend/capability 缺口和已尝试路径。
- [ ] npm 遵循 SemVer；公开类型、事件、默认行为变化进入 API report 和 changelog。
- [ ] contract test 在 mock runtime 和真实 WASM 上运行相同 API 行为矩阵。

