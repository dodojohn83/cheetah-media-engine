#pragma once

#include <QMainWindow>

class CheetahPlayerWidget;
class QLineEdit;

class MainWindow : public QMainWindow {
    Q_OBJECT
public:
    explicit MainWindow(QWidget *parent = nullptr);

private:
    CheetahPlayerWidget *player_widget_ = nullptr;
    QLineEdit *url_edit_ = nullptr;
};
