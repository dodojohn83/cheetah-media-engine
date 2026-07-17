#include "test_cheetah_player_widget.h"
#include "../src/cheetah_player_widget.h"

#include <QSignalSpy>
#include <QString>
#include <QTest>

void TestCheetahPlayerWidget::testCreateDestroy() {
    CheetahPlayerWidget w;
    QVERIFY(!w.currentState().isEmpty());
}

void TestCheetahPlayerWidget::testLoadInvalidUrl() {
    CheetahPlayerWidget w;
    QVERIFY(!w.load("not-a-url"));
    QCOMPARE(w.currentState(), QString("idle"));
}

void TestCheetahPlayerWidget::testPlayBeforeLoad() {
    CheetahPlayerWidget w;
    QVERIFY(!w.play());
    QCOMPARE(w.currentState(), QString("idle"));
}

void TestCheetahPlayerWidget::testConfigure() {
    CheetahPlayerWidget w;
    QVERIFY(w.configure("{}"));
}

void TestCheetahPlayerWidget::testCallback() {
    CheetahPlayerWidget w;
    QSignalSpy spy(&w, &CheetahPlayerWidget::stateChanged);
    QVERIFY(w.load("http://example.com/test.flv"));
    QVERIFY(spy.count() >= 1);
}

QTEST_MAIN(TestCheetahPlayerWidget)
