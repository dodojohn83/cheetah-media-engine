#include "test_cheetah_player_qml.h"
#include "../src/cheetah_player_qml.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QString>
#include <QTest>

void TestCheetahPlayerQml::testCreateDestroy() {
    CheetahPlayerQml player;
    QVERIFY(!player.state().isEmpty());
    QCOMPARE(player.state(), QString("idle"));
}

void TestCheetahPlayerQml::testLoadInvalidUrl() {
    CheetahPlayerQml player;
    QVERIFY(!player.load("not-a-url", false));
    QCOMPARE(player.state(), QString("idle"));
}

void TestCheetahPlayerQml::testPlayBeforeLoad() {
    CheetahPlayerQml player;
    QVERIFY(!player.play());
    QCOMPARE(player.state(), QString("idle"));
}

void TestCheetahPlayerQml::testConfigure() {
    CheetahPlayerQml player;
    QVERIFY(player.configure("{}"));
}

void TestCheetahPlayerQml::testCallback() {
    CheetahPlayerQml player;
    QSignalSpy spy(&player, &CheetahPlayerQml::stateChanged);
    QVERIFY(player.load("http://example.com/test.flv", false));
    QVERIFY(spy.count() >= 1);
}

void TestCheetahPlayerQml::testQmlRegistration() {
    const int id = qmlRegisterType<CheetahPlayerQml>("TestCheetah", 1, 0, "CheetahPlayer");
    QVERIFY(id >= 0);
}

void TestCheetahPlayerQml::testQmlInstantiation() {
    qmlRegisterType<CheetahPlayerQml>("TestCheetah", 1, 0, "CheetahPlayer");

    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(
        "import QtQuick 2.15\n"
        "import TestCheetah 1.0\n"
        "Item { CheetahPlayer { id: p; objectName: \"player\" } }",
        QUrl());

    QObject *obj = component.create();
    QVERIFY(obj != nullptr);
    auto *player = obj->findChild<CheetahPlayerQml *>("player");
    QVERIFY(player != nullptr);
    QCOMPARE(player->state(), QString("idle"));
    delete obj;
}

QTEST_MAIN(TestCheetahPlayerQml)
