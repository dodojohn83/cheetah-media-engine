# WP-46：双击局部全屏、拖拽排序、不规则布局

## 1. 目标

在 `CheetahWallElement` 上补齐视频监控墙的高级交互能力：

- 双击某个窗口实现局部全屏 / 退出全屏；
- 拖拽窗口重新排序；
- 通过 per-cell `data-grid` 属性支持不规则网格布局。

## 2. 范围

### 2.1 双击局部全屏

- `CheetahWallElement` 监听 `dblclick` 事件；
- 如果目标在 `cheetah-wall-cell` 内，则：
  - 当前 `fullscreen-cell` 已经是该 cell → 退出全屏；
  - 否则 → 进入该 cell 的全屏。
- 全屏行为复用现有 `fullscreen-cell` 属性与 `_updateGrid` 渲染逻辑。

### 2.2 拖拽排序

- 为 `cheetah-wall-cell` 设置 `draggable="true"`；
- `dragstart` 在 `dataTransfer` 中写入 cell id，并记录 `_dragSource`；
- `dragover` 阻止默认行为，根据鼠标在目标 cell 的垂直/水平位置决定插入到目标前还是后；
- `drop` 在 DOM 中移动 source cell，并触发 `wall:reorder` 自定义事件；
- 拖拽过程中不销毁内部 `cheetah-player`（只移动 DOM）；
- `MutationObserver` 已监听 childList，会触发预算重新注册。

### 2.3 不规则布局

- `layout` 属性新增 `custom` 值；
- `cheetah-wall-cell` 支持 `data-grid` JSON 属性：
  ```json
  { "col": 1, "row": 1, "colSpan": 2, "rowSpan": 2 }
  ```
  - `col` / `row` 起始位置从 1 开始；
  - `colSpan` / `rowSpan` 默认 1；
- 当 `layout="custom"` 时：
  - wall 遍历可见 cell，读取 `data-grid`；
  - 计算最大列数/行数；
  - 设置 `grid-template-columns` / `grid-template-rows`；
  - 为每个 cell 设置 `grid-column` / `grid-row`；
- 自定义布局下不启用拖拽排序（避免跨行/跨列语义混乱）。

## 3. 实现清单

- `packages/components/src/wall-element.ts`
  - 增加 `_onDblClick`、`_onDragStart`、`_onDragOver`、`_onDrop` 处理；
  - `_updateGrid` 支持 `layout="custom"` 和 `data-grid` 解析；
  - `connectedCallback` 注册 `dblclick`、`dragstart`、`dragover`、`drop`；
  - `disconnectedCallback` 移除这些监听。
- `packages/components/src/wall-element.test.ts`
  - 双击进入/退出全屏；
  - 拖拽排序并触发 `wall:reorder`；
  - 自定义 `data-grid` 布局设置 grid-column/row。
- `packages/components/README.md`
  - 补充 wall 的高级交互说明。
- `dev-docs/003_web_pro_feature_parity.md`
  - 记录 WP-46 状态。

## 4. 完成定义

- [x] 双击局部全屏/退出在单元测试中通过；
- [x] 拖拽排序后 DOM 顺序变化，`wall:reorder` 事件携带 `cellId`、`oldIndex`、`newIndex`；
- [x] `layout="custom"` + `data-grid` 正确渲染不规则网格；
- [x] 无 `todo!()` / `unimplemented!()`，生产路径无 `unwrap()`；
- [x] `corepack pnpm typecheck`、`corepack pnpm test`、`corepack pnpm build` 通过。

## 5. 后续

- WP-47：局部文字/图片/HTML 水印。
