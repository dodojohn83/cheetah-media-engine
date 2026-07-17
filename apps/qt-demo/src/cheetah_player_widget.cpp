#include "cheetah_player_widget.h"

#include <QByteArray>
#include <QStringList>

CheetahPlayerWidget::CheetahPlayerWidget(QWidget *parent) : QWidget(parent) {
    if (cheetah_player_create(&player_) != CHEETAH_RESULT_OK) {
        player_ = nullptr;
    } else if (player_ != nullptr) {
        cheetah_player_set_event_callback(player_, &CheetahPlayerWidget::eventCallback, this);
    }
}

CheetahPlayerWidget::~CheetahPlayerWidget() {
    if (player_ != nullptr) {
        cheetah_player_destroy(&player_);
    }
}

bool CheetahPlayerWidget::configure(const QString &json) {
    if (player_ == nullptr) {
        return false;
    }
    const QByteArray bytes = json.toUtf8();
    return cheetah_player_configure(player_, bytes.constData()) == CHEETAH_RESULT_OK;
}

bool CheetahPlayerWidget::load(const QString &url, bool isLive) {
    if (player_ == nullptr) {
        return false;
    }
    const QByteArray bytes = url.toUtf8();
    return cheetah_player_load(player_, bytes.constData(), isLive) == CHEETAH_RESULT_OK;
}

bool CheetahPlayerWidget::play() {
    if (player_ == nullptr) {
        return false;
    }
    return cheetah_player_play(player_) == CHEETAH_RESULT_OK;
}

bool CheetahPlayerWidget::pause() {
    if (player_ == nullptr) {
        return false;
    }
    return cheetah_player_pause(player_) == CHEETAH_RESULT_OK;
}

bool CheetahPlayerWidget::stop() {
    if (player_ == nullptr) {
        return false;
    }
    return cheetah_player_stop(player_) == CHEETAH_RESULT_OK;
}

QString CheetahPlayerWidget::currentState() const {
    if (player_ == nullptr) {
        return QString();
    }
    const char *s = cheetah_player_state(player_);
    return QString::fromUtf8(s);
}

void CheetahPlayerWidget::onEvent(const CheetahEvent *event) {
    const QString type = QString::fromUtf8(event->event_type);
    const QString trackId = event->track_id ? QString::fromUtf8(event->track_id) : QString();
    const QString message = event->message ? QString::fromUtf8(event->message) : QString();

    if (type == QLatin1String("state_changed")) {
        const QStringList parts = message.split(", ");
        const QString toState = parts.size() >= 2 ? parts.last() : message;
        emit stateChanged(toState);
    } else if (type == QLatin1String("error")) {
        emit errorOccurred(static_cast<int>(event->error_code), message);
    } else if (type == QLatin1String("track_added")) {
        emit trackAdded(trackId, message);
    } else if (type == QLatin1String("track_config_changed")) {
        emit trackConfigChanged(trackId, message);
    } else if (type == QLatin1String("eof")) {
        emit eof();
    }
}

void CheetahPlayerWidget::eventCallback(const CheetahPlayer * /*player*/,
                                        const CheetahEvent *event,
                                        void *userdata) {
    auto *w = static_cast<CheetahPlayerWidget *>(userdata);
    if (w != nullptr && event != nullptr) {
        w->onEvent(event);
    }
}
