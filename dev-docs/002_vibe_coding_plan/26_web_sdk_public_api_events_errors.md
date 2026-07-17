# 26. Web SDK 公共 API、事件与错误

## SDK-001：公共包和导出面

公共包固定为 `@cheetah-media/web`，内部 runtime 不从公共入口泄漏。只导出稳定类型、工厂、player 接口、错误/事件/统计类型和版本信息。

- [x] 提供 `createPlayer(config)`，返回单一 `CheetahPlayer` 实例；构造不自动发起网络（`load` 才启动 Worker）。
- [x] 方法包含 `load`、`play`、`pause`、`stop`、`destroy`、`snapshot`、`startRecording`、`stopRecording`、`switchVariant`、`getStats`、`exportDiagnostics`。
- [x] 异步方法拒绝 `CheetahMediaError`；`destroy` 后调用抛错；`load`/`stop`/`snapshot`/`switchVariant`/`startRecording` 受 `guardDestroyed` 保护。
- [x] 公共包仅导出稳定类型、`createPlayer`、`CheetahMediaError`、`CheetahPlayer` 接口和 ABI 常量，不暴露 WASM handle、worker 或内部运行时。

## SDK-002：配置模型

- [x] `PlayerConfig` 包含 transport、latency、backend、memory、render、audio、recording、security、diagnostics 子对象。
- [x] `withDefaults` 生成完整默认配置；`validateConfig` 对越界/矛盾值抛出 `CheetahMediaError` (code 6001, stage config)。
- [x] `BackendConfig.preference` 支持 ordered preference；实际能力过滤仍由 runtime planner 处理，preference 不会强制选择不支持路径。
- [x] `transport.headers`、`security.token`、`security.credentials` 在 `exportDiagnostics()` 和 `redactConfig` 中被 redact。

## SDK-003：事件和顺序保证

- [x] 事件类型完整覆盖 statechange、tracks、firstframe、backendchange、variantchange、buffering、stats、warning、error、recording。
- [x] 每个 `CheetahPlayerEvent` 携带 `playerId`、`epoch`、`sequence`、`timestamp`；`error` 事件 details 携带 `CheetahMediaError.toJSON()` 含 `recoverable`。
- [x] `setState` 在 `emit` 业务事件之前触发 `statechange`；`destroy` 后 `listeners` 被清空且非 error 事件不派发。
- [x] `stats` 按 `diagnostics.statsIntervalMs` 节流；用户 listener 异常被 try/catch 捕获，不影响引擎。

## SDK-004：错误和版本兼容

- [x] `CheetahMediaError` 携带 `code`/`stage`/`message`/`recoverable`，`toJSON()` 不包含 cause 对象，避免泄露内部栈。
- [-] Unsupported 路径的 `reasonChain` 已在 runtime planner 中输出；web 层将在后续 PR 把 planner 结果暴露给用户错误详情。
- [x] 版本保持 SemVer 0.1.0；新增 `packages/web/API.md` 与根目录 `CHANGELOG.md` 记录公开 API 和变更。
- [x] `packages/web/src/index.test.ts` 使用 mock runtime 完成 load/play/pause/stop/destroy/event/error/config/diagnostics/snapshot/switchVariant 契约测试；与真实 WASM 的端到端行为矩阵待后续 testkit/browser harness 补全。

