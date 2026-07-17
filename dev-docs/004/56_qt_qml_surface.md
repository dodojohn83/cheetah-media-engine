# WP-56: Qt QML / Qt Quick surface 接入

## 1. 范围

在 WP-55 的 `apps/qt-demo` 基础上，增加一个 QML / Qt Quick 入口：

- 定义 `CheetahPlayerQml`：一个 `QObject` 派生类型，通过 `qmlRegisterType` 暴露给 QML。
- 从 QML 调用 `configure` / `load` / `play` / `pause` / `stop`。
- 把 C ABI 回调事件转换为 QML 可监听信号：
  `stateChanged`, `errorOccurred`, `trackAdded`, `trackConfigChanged`, `eof`。
- 提供 `main.qml` 与 `cheetah-qml-demo` 可执行文件。
- 提供无头 smoke 测试，验证 QML 类型加载与控制面调用。

真实视频渲染仍不在本工作包范围内；`CheetahPlayerQml` 的 `videoSurface` 属性为占位
（返回 `QQuickItem` 父对象自身），WP-59 会替换为真实渲染 surface。

## 2. 交付物

- `apps/qt-demo/CMakeLists.txt`：条件构建 `cheetah-qml-demo`（需要 Qt5::Quick 与 Qt5::QuickControls2）。
- `apps/qt-demo/src/cheetah_player_qml.{h,cpp}`
- `apps/qt-demo/qml/main.qml`
- `apps/qt-demo/qml/qml.qrc`
- `apps/qt-demo/src/qml/main.cpp`
- `apps/qt-demo/tests/test_cheetah_player_qml.{h,cpp}`
- 本文档

## 3. 接口草案

```qml
import CheetahMedia 1.0

CheetahPlayerQml {
    id: player
    onStateChanged: state => statusLabel.text = "State: " + state
    onErrorOccurred: (code, message) => statusLabel.text = "Error " + code + ": " + message
}

Button { text: "Play"; onClicked: player.play() }
Button { text: "Load"; onClicked: player.load("http://example.com/test.flv", false) }
```

C++ 类：

```cpp
class CheetahPlayerQml : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(QObject* videoSurface READ videoSurface CONSTANT)
public:
    explicit CheetahPlayerQml(QObject *parent = nullptr);
    ~CheetahPlayerQml();
    Q_INVOKABLE bool configure(const QString &json);
    Q_INVOKABLE bool load(const QString &url, bool isLive);
    Q_INVOKABLE bool play();
    Q_INVOKABLE bool pause();
    Q_INVOKABLE bool stop();
    QString state() const;
    QObject *videoSurface() const;
signals:
    void stateChanged(const QString &state);
    void errorOccurred(int code, const QString &message);
    void trackAdded(const QString &trackId, const QString &message);
    void trackConfigChanged(const QString &trackId, const QString &message);
    void eof();
};
```

## 4. 验证

```bash
cd apps/qt-demo
rm -rf build && mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
cmake --build . -- -j$(nproc)
QT_QPA_PLATFORM=offscreen ctest --output-on-failure
```

如果系统没有 Qt5 Quick，CMake 会自动跳过 QML demo，仅构建 Widget 版本。

## 5. 状态

- [x] Qt5 Quick dev 环境可用
- [x] CheetahPlayerQml C++/QML 类型与信号
- [x] cheetah-qml-demo 与 main.qml
- [x] 无头 smoke 测试通过
- [x] Rust 验证矩阵通过
