# 30. 可观测、诊断、安全与隐私

## OBS-001：指标和 trace

- [ ] 指标分 source、demux、timeline、decode、render、audio、record、memory、fallback 九类。
- [ ] 统一定义 counter/gauge/histogram 单位、聚合窗口和 reset 语义；p95 不从平均值推导。
- [ ] trace 以 player/epoch/sequence 关联状态、首帧、backend 切换、恢复和销毁。
- [ ] 默认采样避免高频逐帧日志；debug 模式也不得记录媒体 payload。

## OBS-002：诊断包

- [ ] 导出版本、ABI/pack manifest、capability、配置脱敏副本、事件尾环、统计摘要和资源 ledger。
- [ ] URL 仅保留 origin/脱敏 path，删除 query、fragment、Authorization、Cookie 和自定义秘密 header。
- [ ] 诊断包有大小/事件数/时间范围上限，并在生成前显示包含内容。
- [ ] 导出失败不影响播放；destroy 后不得保留可识别会话数据。

## SEC-001：Web 安全边界

- [ ] 文档给出 COOP/COEP/CORP 示例和非隔离回退，不强制业务站点错误开启跨域策略。
- [ ] CSP 覆盖 worker-src、script-src、connect-src、WASM 和 codec pack；禁止 eval。
- [ ] loader 校验 HTTPS/同源策略、manifest ABI、hash/SRI 和 MIME，拒绝混合版本资源。
- [ ] parser/ABI 对恶意输入采用统一 limits；WASM trap/worker crash 转为受控错误。

## SEC-002：供应链与发布审计

- [ ] cargo/npm advisory、license allowlist、锁文件差异和 provenance 成为 Required CI。
- [ ] FFmpeg source/configure/NOTICE/SBOM 与 pack 同版本发布。
- [ ] source map 发布策略不包含绝对路径、秘密或未授权源码。
- [ ] 安全问题定义报告入口、严重性、修复 SLA、撤回 codec pack/npm 版本和通知流程。

