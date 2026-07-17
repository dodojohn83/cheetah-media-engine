# Cheetah Qt Demo

A minimal Qt5 QWidget example that consumes the `cheetah-media-c-bindings` C ABI.

## Build

```bash
cd apps/qt-demo
rm -rf build && mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
cmake --build . -- -j$(nproc)
```

This also builds the Rust `cheetah-media-c-bindings` crate as a shared library
and copies it next to the `cheetah-qt-demo` executable.

## Run

```bash
./cheetah-qt-demo
```

For a headless environment:

```bash
QT_QPA_PLATFORM=offscreen ./cheetah-qt-demo
```

## Test

```bash
QT_QPA_PLATFORM=offscreen ctest --output-on-failure
```

## Notes

- `CheetahPlayerWidget` wraps a `CheetahPlayer` handle and emits Qt signals from
the registered C callback. Callbacks arrive on the thread that calls the control
functions, which must be the Qt main thread for direct signal emission.
- Video rendering is intentionally out of scope for this work package (WP-55);
WP-59 will provide a native renderer surface and wire it to the widget.
