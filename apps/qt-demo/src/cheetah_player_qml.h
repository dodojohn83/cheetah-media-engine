#pragma once

#include <QObject>
#include <QString>

extern "C" {
#include "cheetah_media.h"
}

class CheetahPlayerQml : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString state READ state NOTIFY stateChanged)
    Q_PROPERTY(QObject* videoSurface READ videoSurface CONSTANT)

public:
    Q_INVOKABLE CheetahPlayerQml(QObject *parent = nullptr);
    ~CheetahPlayerQml() override;

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

private:
    void onEvent(const CheetahEvent *event);
    static void eventCallback(const CheetahPlayer *player,
                              const CheetahEvent *event,
                              void *userdata);

    CheetahPlayer *player_ = nullptr;
};
