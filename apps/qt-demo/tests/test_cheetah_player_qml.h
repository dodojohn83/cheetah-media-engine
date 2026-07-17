#pragma once

#include <QObject>

class TestCheetahPlayerQml : public QObject {
    Q_OBJECT

private slots:
    void testCreateDestroy();
    void testLoadInvalidUrl();
    void testPlayBeforeLoad();
    void testConfigure();
    void testCallback();
    void testQmlRegistration();
    void testQmlInstantiation();
};
