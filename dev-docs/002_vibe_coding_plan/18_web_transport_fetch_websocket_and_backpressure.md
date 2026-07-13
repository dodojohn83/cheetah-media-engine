# 18. Fetch/WebSocket Transport 与背压

## NET-001：统一 transport 配置

- [ ] 配置包括 URL、method、headers、credentials、referrer、timeout、retry、redirect 和最大响应字节。
- [ ] 默认拒绝 URL 用户信息、危险 scheme 和从 HTTPS 页面加载明文媒体。
- [ ] Authorization/Cookie 不写入事件、日志、diagnostics 或错误 context。
- [ ] transport 只输出 byte chunks 和响应 metadata，不识别 FLV/MP4/TS。

## NET-002：Fetch streaming

- [ ] 使用 ReadableStream reader，支持 AbortSignal、短读、EOF、Content-Length 不一致和无 body。
- [ ] reader 服从下游 high watermark，不能在 JS 堆无限缓存。
- [ ] 对 200/206、重定向、CORS、opaque response 和非成功状态定义稳定错误。
- [ ] HTTP 重试仅在尚未产生不可安全拼接的数据或由上层创建新 epoch 时执行。

## NET-003：WebSocket streaming

- [ ] `binaryType=arraybuffer`，拒绝或受限处理 text message；message 边界不等于容器边界。
- [ ] 连接、open、close code/reason、error、用户 stop 的状态明确区分。
- [ ] 浏览器 WS 无真正 read backpressure 时，以输入预算和受控 close 防止内存失控。
- [ ] 自动重连使用上限、指数退避和 jitter；成功重连创建新 epoch/preroll。

## NET-004：fake server 验证

- [ ] 可脚本化 chunk 大小、间隔、短读、断连、stall、重定向、状态码和错误 Content-Type。
- [ ] 验证 stop/destroy 后零网络活动，迟到 chunk 不进入新会话。
- [ ] 高速发送下内存保持上限且产生 backpressure/drop 指标。
- [ ] 网络恢复首帧和延迟重新收敛时间进入 E2E 报告。

