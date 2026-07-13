# cheetah-media-backend-api

Platform-neutral backend port definitions for the Cheetah media engine.

## Responsibility

- `CapabilityProbe`
- `TransportSource` and `TransportError`
- Future ports: `VideoDecoder`, `AudioDecoder`, `VideoRenderer`, `AudioSink`, `Clock`, `RecorderSink`, `DiagnosticsSink`

## Constraints

- Does not depend on DOM, Qt, JNI, WebCodecs, MSE, or any platform runtime.
- Forbids `unsafe_code`.
