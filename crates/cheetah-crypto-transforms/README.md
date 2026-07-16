# cheetah-crypto-transforms

Sans-I/O decryption transforms for the Cheetah media engine.

## Scope

This crate provides stateful, streaming decryption primitives intended to run
*before* a demuxer sees a media byte stream. It supports:

- `XorTransform` – byte-wise XOR with a repeating key.
- `Aes128CbcTransform` – AES-128 in CBC mode with PKCS#7 padding removal.
- `Sm4CbcTransform` – SM4 in CBC mode with PKCS#7 padding removal.

## Design

- Each transform implements the `Transform` trait with `update`/`finalize`.
- Inputs are processed incrementally and the result returned as a slice valid
  until the next call.
- CBC transforms retain one full block as a potential final padding block and
  only emit it after `finalize`.
- Secret key/IV bytes are not copied into error messages or returned slices.
- The crate is `no_std` + `alloc` by default. Enable the `std` feature for
  `std::error::Error` support.

## Allowed dependencies

- `aes`, `sm4` – RustCrypto block cipher primitives.
- `cipher` – RustCrypto trait glue (`Block`, `Key`, `BlockCipherDecrypt`, ...).

## Features

- `std` (default) – enables `std::error::Error`.

## Example

```rust
use cheetah_crypto_transforms::{Aes128CbcTransform, Transform};

let mut t = Aes128CbcTransform::new(key, iv).unwrap();
let mut plaintext = Vec::new();
for chunk in ciphertext.chunks(1024) {
    plaintext.extend_from_slice(t.update(chunk).unwrap());
}
plaintext.extend_from_slice(t.finalize().unwrap());
```

## No-std usage

Disable default features and provide an allocator:

```toml
cheetah-crypto-transforms = { default-features = false }
```
