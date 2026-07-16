# WP-47：局部文字 / 图片 / HTML 水印

## 1. 目标

为 `CheetahPlayerElement` 增加本地（client-side）水印覆盖能力，支持文字、图片、HTML 三种内容，并提供平铺、动态、幽灵三种效果，与 Jessibuca Pro 的局部水印能力对齐。

## 2. 范围

- 新增 `@cheetah-media/components` 水印模块 `packages/components/src/watermark.ts`：
  - `Watermark` / `TextWatermark` / `ImageWatermark` / `HtmlWatermark` 类型；
  - `parseWatermarks`：从 JSON 属性字符串解析并校验水印列表；
  - `createWatermarkOverlay`：创建水印 DOM 层并支持 `setWatermarks` / `clear`。
- `CheetahPlayerElement` 增加 `watermarks` 属性 / `setWatermarks` 方法；
- `packages/components/src/styles.ts` 增加 `.watermark-layer`、`.watermark-item`、`.watermark-tile-container`、动态与幽灵动画；
- 在 `attributeChangedCallback` 中监听 `watermarks` 属性变化并更新覆盖层；
- 单元测试覆盖解析、文本 / 图片 / HTML 渲染、平铺数量、动态 / 幽灵类名、清空与更新。
- 文档更新：`packages/components/README.md` 和 `dev-docs/003_web_pro_feature_parity.md`。

## 3. 实现要点

- 水印层位于 shadow DOM 的 video 表面之上、状态与控制层之下（`z-index: 1`），`pointer-events: none`，避免干扰播放控制；
- 平铺模式使用 `4x3` 网格容器生成 12 个副本，副本继承原水印的内容与效果；
- 动态模式通过 CSS `@keyframes watermark-move` 在容器边界内移动水印位置；
- 幽灵模式通过 CSS `@keyframes watermark-ghost` 交替变化透明度；
- 输入校验：丢弃未知 `type`、空 `content`、畸形 JSON；位置限定在 `0..100`，透明度限定在 `0..1`；
- 不读取外部资源或执行远程脚本，HTML 水印内容由调用方提供并由浏览器沙箱化渲染。

## 4. 完成定义

- [ ] `parseWatermarks` 通过单元测试，非法输入返回 `undefined`，合法列表返回验证后的对象；
- [ ] 文本、图片、HTML 三种水印在 `WatermarkOverlay` 中正确生成对应的 DOM；
- [ ] `tile`、`dynamic`、`ghost` 选项通过 CSS class / 网格布局体现；
- [ ] `CheetahPlayerElement` 可通过 `watermarks` 属性或 `setWatermarks()` 设置水印；
- [ ] `corepack pnpm typecheck`、`corepack pnpm test`、`corepack pnpm build` 通过；
- [ ] 无 `todo!()` / `unimplemented!()`，生产路径无裸 `unwrap()`（Rust 未改动）。

## 5. 后续

- WP-48：数字暗水印 / 截图水印集成（Extension）。
