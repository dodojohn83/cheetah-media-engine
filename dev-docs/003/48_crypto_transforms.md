# WP-48: SM4 / XOR / AES-128-CBC 解密 Transform

## 1. 目标

交付 `crates/cheetah-crypto-transforms`：一组 Sans-I/O 解密 transform，供 HLS/私有流在 demux 前对负载进行有界、零拷贝友好地解密。支持 XOR、AES-128-CBC（含 PKCS#7 去填充）和 SM4-CBC（含 PKCS#7 去填充）。

## 2. 交付物

- `cheetah-crypto-transforms` crate，README 说明职责、允许依赖和 feature。
- `Transform` trait：增量 `update` / `finalize` 接口，输出解密后的字节，适用于流式传输。
- `XorTransform`：循环密钥字节异或，无块对齐要求。
- `Aes128CbcTransform`：AES-128-CBC 解密 + PKCS#7 去填充；支持 16 字节 IV。
- `Sm4CbcTransform`：SM4-CBC 解密 + PKCS#7 去填充；支持 16 字节 IV。
- 错误类型：`CryptoError`（密钥/IV 长度错误、块未对齐、填充错误等）。

## 3. 完成定义

- `cargo fmt/clippy/test --workspace --all-features` 通过。
- `no_std + alloc` 编译通过（`default-features = false`）。
- 无 `todo!()`/`unimplemented()`；对外部输入无 `unwrap()`/`expect()`。
- 测试覆盖 NIST/国密已知向量、PKCS#7 去填充（正常/错误/空输入）、增量分片边界、密钥长度错误、XOR 循环密钥。

## 4. 边界

- 加密路径本次不实现；只提供解密 transform。
- 密钥不进入日志、错误消息或诊断输出。
- 不处理 HLS 密钥 URI 获取；只接收已提供的密钥/IV 字节。
