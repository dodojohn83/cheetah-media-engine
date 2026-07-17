#pragma once

#include <QWidget>
#include <QString>

extern "C" {
#include "cheetah_media.h"
}

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
