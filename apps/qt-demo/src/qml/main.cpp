#include <QGuiApplication>
#include <QQmlApplicationEngine>

#include "../cheetah_player_qml.h"

int main(int argc, char **argv) {
    QGuiApplication app(argc, argv);

    qmlRegisterType<CheetahPlayerQml>("CheetahMedia", 1, 0, "CheetahPlayer");

    QQmlApplicationEngine engine;
    engine.load(QUrl(QStringLiteral("qrc:/main.qml")));

    if (engine.rootObjects().isEmpty()) {
        return -1;
    }

    return app.exec();
}
