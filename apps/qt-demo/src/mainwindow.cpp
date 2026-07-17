#include "mainwindow.h"
#include "cheetah_player_widget.h"

#include <QHBoxLayout>
#include <QLineEdit>
#include <QPushButton>
#include <QStatusBar>
#include <QVBoxLayout>
#include <QWidget>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
    auto *central = new QWidget(this);
    auto *layout = new QVBoxLayout(central);

    player_widget_ = new CheetahPlayerWidget(this);
    layout->addWidget(player_widget_);

    auto *controls = new QHBoxLayout();
    url_edit_ = new QLineEdit(this);
    url_edit_->setPlaceholderText(QStringLiteral("http://example.com/test.flv"));

    auto *loadBtn = new QPushButton(QStringLiteral("Load"), this);
    auto *playBtn = new QPushButton(QStringLiteral("Play"), this);
    auto *pauseBtn = new QPushButton(QStringLiteral("Pause"), this);
    auto *stopBtn = new QPushButton(QStringLiteral("Stop"), this);

    controls->addWidget(url_edit_);
    controls->addWidget(loadBtn);
    controls->addWidget(playBtn);
    controls->addWidget(pauseBtn);
    controls->addWidget(stopBtn);
    layout->addLayout(controls);

    setCentralWidget(central);
    statusBar()->showMessage(QStringLiteral("Idle"));

    connect(loadBtn, &QPushButton::clicked, this, [this]() {
        player_widget_->load(url_edit_->text(), false);
    });
    connect(playBtn, &QPushButton::clicked, this, [this]() {
        player_widget_->play();
    });
    connect(pauseBtn, &QPushButton::clicked, this, [this]() {
        player_widget_->pause();
    });
    connect(stopBtn, &QPushButton::clicked, this, [this]() {
        player_widget_->stop();
    });
    connect(player_widget_, &CheetahPlayerWidget::stateChanged, this, [this](const QString &state) {
        statusBar()->showMessage(QStringLiteral("State: ") + state);
    });
    connect(player_widget_, &CheetahPlayerWidget::errorOccurred, this, [this](int code, const QString &msg) {
        statusBar()->showMessage(QStringLiteral("Error %1: %2").arg(code).arg(msg));
    });
}
