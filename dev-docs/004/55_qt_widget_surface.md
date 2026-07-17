# WP-55: Qt QWidget 接入与窗口生命周期

## 1. 范围

在 WP-54 的 C ABI 控制面基础上，建立 `apps/qt-demo`：一个最小 Qt5 QWidget 示例，演示如何：

- 加载 `libcheetah_media_c_bindings.so` 并创建 `CheetahPlayer`。
- 在 `QWidget` 中注册 C 事件回调，把 `CheetahEvent` 转换为 Qt 信号。
- 通过 UI 按钮触发 `configure` / `load` / `play` / `pause` / `stop`。
- 处理窗口生命周期：`show`/`close` 创建/销毁 player；resize 事件占位。
- 提供 CMake 构建脚本和本地无头运行（`QT_QPA_PLATFORM=offscreen`）的 smoke 测试。

本工作包不实现真实视频渲染（WP-59），只建立 widget 骨架、C ABI 链接与生命周期映射。

## 2. 交付物

- `apps/qt-demo/CMakeLists.txt`
- `apps/qt-demo/src/main.cpp`
- `apps/qt-demo/src/mainwindow.{h,cpp}`
- `apps/qt-demo/src/cheetah_player_widget.{h,cpp}`
- `apps/qt-demo/tests/test_cheetah_player_widget.{h,cpp}`（QTest 无头 smoke）
- `apps/qt-demo/README.md`

## 3. 接口草案

```cpp
class CheetahPlayerWidget : public QWidget {
    Q_OBJECT
public:
    explicit CheetahPlayerWidget(QWidget *parent = nullptr);
    ~CheetahPlayerWidget() override;
    bool configure(const QString &json);
    bool load(const QString &url, bool isLive = false);
    bool play();
    bool pause();
    bool stop();
    QString currentState() const;
signals:
    void stateChanged(const QString &state);
    void errorOccurred(int code, const QString &message);
    void trackAdded(const QString &trackId, const QString &message);
    void trackConfigChanged(const QString &trackId, const QString &message);
    void eof();
private:
    void onEvent(const CheetahEvent *event);
    static void eventCallback(const CheetahPlayer *player,
                              const CheetahEvent *event,
                              void *userdata);
    CheetahPlayer *player_ = nullptr;
};
```

- 回调由 Qt 主线程触发（控制函数在同一线程调用）。
- `CheetahEvent` 中的字符串仅在回调存续期内使用，转换为 `QString` 后不再保留 C 指针。

## 4. 验证

```bash
# 构建 c-bindings
(cd /home/ubuntu/repos/cheetah-media-engine && cargo build -p cheetah-media-c-bindings --release)
# 构建 Qt demo
cd apps/qt-demo
rm -rf build && mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
cmake --build . -- -j$(nproc)
# 无头 smoke 测试
QT_QPA_PLATFORM=offscreen ctest --output-on-failure
```

同时运行 Rust 验证矩阵：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --no-default-features
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --release
cargo build -p cheetah-media-web-bindings --target wasm32-unknown-unknown --no-default-features
cargo deny check
```

## 5. 状态

- [x] Qt5 dev 环境可用
- [x] QWidget 示例骨架与 CMake 构建
- [x] 无头 smoke 测试通过
- [x] CI 不依赖 Qt 构建（Qt demo 为可选产物）
