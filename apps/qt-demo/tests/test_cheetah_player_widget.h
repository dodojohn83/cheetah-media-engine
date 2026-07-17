#pragma once

#include <QObject>

class TestCheetahPlayerWidget : public QObject {
    Q_OBJECT
private slots:
    void testCreateDestroy();
    void testLoadInvalidUrl();
    void testPlayBeforeLoad();
    void testConfigure();
    void testCallback();
};
