# 17. Web Worker Runtime、内存与生命周期

## WEB-001：Worker 拓扑

- [ ] 主线程只负责公开 API、DOM/surface 协调和少量事件聚合。
- [ ] media worker 运行 Rust/WASM、demux、planner 和软解控制；codec worker 是否拆分由 codec pack manifest 决定。
- [ ] AudioWorklet 只消费预格式化 PCM/ring descriptor，不解析容器或执行网络。
- [ ] 消息 envelope 固定 protocol version、instance、epoch、sequence、type 和 payload descriptor。

## WEB-002：隔离与非隔离启动

- [ ] 启动时检测 secure context、crossOriginIsolated、SharedArrayBuffer、Atomics、WASM SIMD/threads。
- [ ] isolated 优先 threads+SIMD；非隔离保持单 worker SIMD/baseline 可用。
- [ ] loader 校验 JS/WASM/codec pack ABI manifest 后才实例化，版本不匹配返回 Unsupported。
- [ ] CSP、worker URL、WASM MIME、跨域资源失败产生可操作诊断。

## WEB-003：内存增长和批处理

- [ ] 所有 TypedArray view 在 memory growth 后重建；不得跨 await 保存易失 view。
- [ ] push/poll/event/release 使用有界批量，限制单批条目和执行时间以免阻塞 worker loop。
- [ ] 监测 WASM pages、pool bytes、descriptor count、JS heap estimate 和 GC pause。
- [ ] 达到 hard limit 时先施加背压/实时丢弃，仍无法恢复则 ResourceLimit 失败。

## WEB-004：生命周期和崩溃

- [ ] stop 取消 fetch/ws、timer、pending promise、decoder callback、GPU/audio 任务。
- [ ] destroy 终止 worker 并解除所有 event listener、object URL、AudioContext/GPU 引用。
- [ ] worker crash 只允许一次受控重建；重复 crash 进入 Failed，防止重启风暴。
- [ ] 页面隐藏/冻结/恢复策略可配置，并验证恢复不播放旧 epoch 帧。

