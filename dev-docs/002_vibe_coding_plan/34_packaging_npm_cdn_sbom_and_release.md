# 34. 打包、npm/CDN、SBOM 与发布

## REL-001：npm 制品

- [ ] `@cheetah-media/web` 输出 tree-shakeable ESM 和类型声明；浏览器 IIFE 使用独立文件名和全局命名空间。
- [ ] `@cheetah-media/components` 输出 ESM/IIFE、类型和样式资产，只依赖公共 SDK 契约。
- [ ] `@cheetah-media/runtime` 保持内部 workspace 包，不作为用户直接入口承诺兼容。
- [ ] package exports、sideEffects、engines、files、license、repository 和 source map 策略经过安装测试。

## REL-002：WASM 和 codec pack 发现

- [ ] SDK 支持显式 `assetBaseUrl`/resolver，并提供同包、自托管和 CDN 三种文档化部署。
- [ ] core WASM、worker、codec packs 使用 manifest 关联 ABI/version/hash/features；禁止猜文件名加载混合版本。
- [ ] IIFE 不内联大型 WASM/codec pack；加载失败返回具体 URL 类型和可脱敏原因。
- [ ] 静态服务器示例配置正确 WASM MIME、缓存、CORS、CORP、COOP/COEP。

## REL-003：供应链制品

- [ ] 为 Rust、npm、WASM、FFmpeg 生成 SPDX 或 CycloneDX SBOM。
- [ ] 发布 source archive、NOTICE、FFmpeg source/configure manifest、hash/SRI 和 provenance。
- [ ] license/advisory/secret scan、API/ABI diff、bundle size 和可重复构建为发布阻塞项。
- [ ] 制品签名/attestation 与 changelog、git tag、npm version 一一对应。

## REL-004：发布、撤回和回滚

1. 发布 core release candidate tag。
2. server facade 和 engine 固定 RC，完成 contract/E2E/performance/soak。
3. 发布 core stable、npm prerelease，执行 clean install/CDN smoke。
4. 发布 npm stable 和 release notes，保存验收报告。

- [ ] 回滚文档覆盖 core tag、npm dist-tag、CDN immutable path 和 codec pack 撤回。
- [ ] 已发布版本不覆盖同 URL 内容；修复总是发布新版本。
- [ ] 安全撤回不删除取证和 SBOM，用户获得替代版本与影响说明。

