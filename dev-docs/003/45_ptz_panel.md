# WP-45：PTZ 操作盘与 GB28181 命令生成

## 1. 目标

在 Web SDK / Components 层提供云台（PTZ）操作盘和 GB28181 `PTZCmd` 命令生成能力。媒体引擎只负责命令编码和 UI 事件分发，不直接发送 SIP/HTTP 到设备；应用层把 `ptz` 事件转发到 `dodojohn83/cheetah-signaling` 或自有信令服务完成实际控制。

## 2. 依赖

- WP-26：Web SDK 公共 API、事件和错误（已完成）。
- WP-27：Web Components 与 accessibility（已完成）。

## 3. 交付物

### 3.1 GB28181 PTZ 命令编码

- `packages/web/src/ptz.ts`：
  - `PtzAction` 联合类型：方向（`up`/`down`/`left`/`right`/对角线/`stop`）、变焦（`zoomIn`/`zoomOut`）、聚焦/光圈（可选）、预置位（`presetSet`/`presetCall`/`presetDel`）。
  - `PtzSpeeds`：云台水平/垂直速度、镜头变倍速度（均 0~255，内部按需截断）。
  - `createGb28181PtzCmd(action, speeds, options)`：返回 8 字节十六进制字符串。
    - 移动/停止/变焦使用 8 字节格式：`A5 0F 01 <cmd> <hs> <vs> <z> <checksum>`，`<z>` 高 4 位为 zoom 速度、低 4 位固定 0。
    - 预置位使用 8 字节格式：`A5 0F <addr-low> <preset-cmd> 00 <point> 00 <checksum>`（`addr-low` 默认 0x00，由调用方透传地址时覆盖）。
  - 校验码为前 7 字节之和 mod 256。

### 3.2 公共 Player PTZ API

- `CheetahPlayer` 增加 `ptz(command: PtzCommand): Promise<void>`。
- `CheetahPlayerImpl` 内部调用 `createGb28181PtzCmd` 并分发 `ptz` 事件（事件类型加入 `CheetahPlayerEventType`）。
- 如果 `command` 不合法，返回稳定 `CheetahMediaError`。

### 3.3 PTZ 操作盘组件

- `packages/components/src/ptz-panel-element.ts`：`CheetahPtzPanelElement`。
  - 暴露 `target` 属性：指向 `cheetah-player` 元素；为空时只冒泡 `ptz` 事件。
  - 方向键（上/下/左/右/四个对角线）、zoom +/-、停止、预置位（设置/调用/删除/编号输入）。
  - 键盘快捷键：方向键、+/-、数字键。
  - 使用 `i18n.ts` 翻译和无障碍标签。
  - 每按下方向按钮生成一次开始动作，松开/失焦时自动发送 `stop`；也可以提供 `autoStop` 开关。
  - 事件：`ptz` CustomEvent，detail 包含 `protocol: 'gb28181'`，`ptzCmd` 十六进制字符串，`action`，`speeds`。

### 3.4 测试与文档

- `packages/web/src/ptz.test.ts`：校验 `createGb28181PtzCmd` 输出、非法参数、校验和、停止码、预置位。
- `packages/components/src/ptz-panel-element.test.ts`：按钮/键盘触发事件、目标绑定、auto-stop 行为。
- 更新 `packages/web/README.md` 和 `packages/components/README.md` 说明 PTZ 用法。

## 4. 完成定义

- [x] `createGb28181PtzCmd` 实现并测试，无 `NaN`/越界 panic。
- [x] `CheetahPlayer` 暴露 `ptz` 方法并触发 `ptz` 事件。
- [x] `CheetahPtzPanelElement` 渲染、可交互、可访问、emit `ptz` 事件。
- [x] 全部验证矩阵通过：
  - `corepack pnpm typecheck`
  - `corepack pnpm test`
  - `corepack pnpm build`
- [x] 无 `todo!()` / `unimplemented!()` / 生产路径 `unwrap()`。

## 5. 后续

- WP-46：双击局部全屏、拖拽排序、不规则布局。
