import QtQuick 2.15
import QtQuick.Controls 2.15
import CheetahMedia 1.0

ApplicationWindow {
    visible: true
    width: 640
    height: 480
    title: qsTr("Cheetah QML Demo")

    CheetahPlayer {
        id: player
        onStateChanged: state => statusLabel.text = qsTr("State: ") + state
        onErrorOccurred: (code, message) => statusLabel.text = qsTr("Error %1: %2").arg(code).arg(message)
    }

    Column {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        Label {
            id: statusLabel
            text: qsTr("Idle")
        }

        TextField {
            id: urlField
            text: "http://example.com/test.flv"
            placeholderText: qsTr("Media URL")
            width: parent.width
        }

        Row {
            spacing: 8
            Button { text: qsTr("Load"); onClicked: player.load(urlField.text, false) }
            Button { text: qsTr("Play"); onClicked: player.play() }
            Button { text: qsTr("Pause"); onClicked: player.pause() }
            Button { text: qsTr("Stop"); onClicked: player.stop() }
        }
    }
}
