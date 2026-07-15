# 27. Web Components 与可访问性

## UI-001：`<cheetah-player>` 组件

- [x] 组件封装 SDK 实例和 shadow DOM，属性/JS property 与 PlayerConfig 映射有明确优先级。
- [x] connected/disconnected/adopted 生命周期不重复 load；可配置断开时 stop 或 destroy。
- [x] 提供 surface slot/part、状态覆盖层、错误提示和控制栏，不暴露内部 worker。
- [x] 自定义事件与 SDK 事件建立表格，保持 composed/bubbles/detail 类型稳定。

## UI-002：播放器控制和状态

- [x] 控件覆盖播放/暂停、静音/音量、全屏、截图、录制、延迟/性能状态（清晰度选择依赖 SDK variant API，v1 未提供 UI）。
- [x] Loading、Preroll、Rebuffering、Failed 显示可区分且提供安全重试动作。
- [x] 自动播放被拒绝时显示用户激活入口，不将其报告为 backend failure。
- [x] 控件隐藏不停止统计或资源治理；无控件模式仍可完全通过 SDK 操作。

## UI-003：主题、国际化和宿主隔离

- [x] 使用 CSS custom properties/parts 提供尺寸、颜色、间距和图标定制，默认样式不污染宿主页面。
- [x] v1 至少提供中英文消息表；错误 code 与显示文本分离。
- [x] 容器 resize 使用 ResizeObserver 并节流；DPR/全屏变化正确更新 surface。
- [x] CSP 下不依赖内联 eval、远程字体或隐式第三方资源。

## UI-004：无障碍和测试

- [x] 键盘可操作全部控件，焦点顺序、可见焦点、ARIA label/state 和快捷键文档完整。
- [x] 状态提示使用适当 live region，避免高频 stats 造成读屏轰炸。
- [x] Playwright 覆盖 DOM 生命周期、属性反射、键盘、resize；全屏/自动播放拒绝在单元/Playwright 中做了基础验证。
- [ ] axe 或等价检查无严重问题；颜色对比和触控目标达到项目规定阈值（待后续 CI 接入 axe-core）。
