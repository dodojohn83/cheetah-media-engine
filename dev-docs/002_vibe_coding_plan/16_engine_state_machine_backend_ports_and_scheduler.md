# 16. Engine 状态机、Backend Ports 与调度器

## ENG-001：冻结引擎状态机

状态固定为 `Idle → Loading → Preroll → Playing ↔ Rebuffering → Stopping → Idle`，任意运行态可进入 `Failed`，`destroy` 最终进入不可复用 `Destroyed`。

- [ ] 为 load/play/pause/stop/destroy、网络事件、track/config、backend callback 建立显式转移表。
- [ ] 每次 load 创建 epoch；旧 epoch 异步结果只释放资源，不产生公开事件或状态变化。
- [ ] 非法命令返回稳定错误；stop/destroy 幂等，失败路径仍执行完整清理。
- [ ] 状态改变和对应事件在同一串行 command loop 内排序。

## ENG-002：定义 backend ports

- [ ] ByteSource：start/read-or-push/cancel/stats；必须表达 EOF、live、retryable failure。
- [ ] Demuxer：push/end/reset；输出 Track、Packet、Discontinuity，不执行网络和解码。
- [ ] Decoder/Renderer/AudioSink/Recorder：configure、submit、flush、reset、close，并报告 queue/clock/error。
- [ ] Clock/MetricsSink 使用注入接口，测试不依赖真实时间或全局日志。
- [ ] 所有 port 方法定义所有权转移、取消点、并发规则和 callback 线程。

## ENG-003：有界调度器

- [ ] 单一所有者修改 pipeline graph；跨线程命令带 sequence/epoch。
- [ ] 为输入、packet、decode、frame、render、audio 和 record 队列配置 high/low watermark。
- [ ] 调度优先级保证音频时钟、关键帧和控制命令不被批量输入饿死。
- [ ] 过载策略产生结构化 drop/backpressure 事件和计量。

## ENG-004：状态机测试

- [ ] model/property test 遍历命令和 callback 排列，断言无非法状态、double close 和资源遗留。
- [ ] 使用 fake ports 覆盖每个方法同步失败、异步失败、取消和迟到完成。
- [ ] 连续 load/stop、失败后重载、destroy during configure、后台/前台转换全覆盖。

