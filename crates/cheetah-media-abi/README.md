# cheetah-media-abi

Stable ABI and platform-neutral ports for media operations.

## Responsibility

- Define `Decoder`, `Renderer`, `AudioSink`, `Clock`, `ByteSource`, `MetricsSink` ports.
- Provide `Input`/`Output` sample descriptors and `Error` enum.

## Constraints

- Forbids `unsafe_code`.
- Depends only on `cheetah-media-types`.
- No platform-specific types (DOM, JNI, Qt, ArkTS).
