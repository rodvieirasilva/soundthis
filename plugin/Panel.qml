import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Panel {
  id: root
  moduleName: "soundthis"
  ipcTarget: "soundthis"

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  property bool serverOnline: false
  property bool serverEnabled: true
  property int clientCount: 0
  property var clients: []
  property string lanIp: "127.0.0.1"
  property int port: 9697

  readonly property color foreground: bar ? bar.barForeground : Commons.Color.foreground
  readonly property string clientUrl: "http://" + lanIp + ":" + port

  function toggleServer() {
    root.serverEnabled = !root.serverEnabled
    toggleProc.running = true
  }

  function refreshStatus() {
    if (!statusProc.running) statusProc.running = true
  }

  function copyUrl() {
    if (bar) bar.run("wl-copy " + root.clientUrl)
  }

  function openBrowser() {
    if (bar) bar.run("xdg-open " + root.clientUrl)
  }

  function startServer() {
    if (bar) bar.run("systemctl --user start soundthis || soundthis serve &")
    refreshTimer.interval = 500
    refreshStatus()
  }

  // Periodic status poll
  Timer {
    id: refreshTimer
    interval: 1500
    running: true
    repeat: true
    onTriggered: {
      refreshTimer.interval = 1500
      root.refreshStatus()
    }
  }

  Process {
    id: statusProc
    command: ["curl", "-fsS", "--max-time", "1", "http://127.0.0.1:9697/api/status"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var raw = String(text || "").trim()
        if (!raw) {
          root.serverOnline = false
          return
        }
        try {
          var data = JSON.parse(raw)
          root.serverOnline = true
          root.serverEnabled = !!data.enabled
          root.lanIp = data.lan_ip || "127.0.0.1"
          root.port = data.port || 9697
          root.clients = data.clients || []
          root.clientCount = root.clients.length
        } catch (e) {
          root.serverOnline = false
        }
      }
    }
  }

  Process {
    id: toggleProc
    command: ["curl", "-fsS", "-X", "POST", "http://127.0.0.1:9697/api/toggle"]
    onExited: {
      root.refreshStatus()
    }
  }

  Component.onCompleted: {
    root.refreshStatus()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: ""
    opacity: (root.serverOnline && root.serverEnabled) ? 1.0 : 0.35
    tooltipText: root.serverOnline
      ? (root.serverEnabled
          ? "SoundThis: Sharing on " + root.clientUrl + (root.clientCount > 0 ? " (" + root.clientCount + " listening)" : "")
          : "SoundThis: Sharing Disabled (Right-click to enable)")
      : "SoundThis: Server Offline (Click to open)"

    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton || buttonCode === Qt.MiddleButton) {
        root.toggleServer()
      } else {
        root.toggle()
      }
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: panel.loadedContent
    contentWidth: panel.fittedContentWidth(Style.space(360))
    contentHeight: panel.fittedContentHeight(panel.loadedContentHeight)

    content: PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      implicitHeight: column.implicitHeight
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // Header Hero
        PanelHero {
          title: "SoundThis"
          meta: !root.serverOnline
            ? "Server offline"
            : (root.serverEnabled ? "Broadcasting live" : "Audio sharing disabled")
          detail: root.serverOnline ? (":" + root.port) : ""
          foreground: root.foreground

          iconComponent: Component {
            Text {
              text: ""
              textFormat: Text.PlainText
              font.family: Style.font.family
              font.pixelSize: Style.font.display
              color: (root.serverOnline && root.serverEnabled) ? Commons.Color.accent : Qt.darker(root.foreground, 1.5)
            }
          }

          trailingControl: Component {
            ToggleSwitch {
              checked: root.serverEnabled && root.serverOnline
              busy: toggleProc.running
              onToggled: root.toggleServer()
            }
          }
        }

        // Stream URL pill
        Rectangle {
          width: parent.width
          height: Style.space(30)
          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.06)
          radius: Style.cornerRadius

          RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Style.space(10)
            anchors.rightMargin: Style.space(10)
            spacing: Style.space(8)

            Text {
              text: "󰖟"
              textFormat: Text.PlainText
              color: root.serverOnline ? Commons.Color.accent : Qt.darker(root.foreground, 1.5)
              font.family: Style.font.family
              font.pixelSize: Style.font.body
            }

            Text {
              Layout.fillWidth: true
              text: root.serverOnline ? root.clientUrl : "Server offline"
              textFormat: Text.PlainText
              color: root.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
              font.bold: true
              elide: Text.ElideRight
            }
          }
        }

        // Action Buttons Row
        RowLayout {
          width: parent.width
          spacing: Style.space(8)

          Button {
            Layout.fillWidth: true
            text: "Copy Link"
            iconText: "󰆏"
            bordered: true
            onClicked: root.copyUrl()
          }

          Button {
            Layout.fillWidth: true
            text: "Open Web UI"
            iconText: "󰖟"
            bordered: true
            onClicked: root.openBrowser()
          }
        }

        PanelSeparator { foreground: root.foreground }

        // Connected Listeners Section
        PanelSectionHeader {
          text: "CONNECTED LISTENERS (" + root.clientCount + ")"
          foreground: root.foreground
        }

        Item {
          width: parent.width
          implicitHeight: emptyText.visible ? emptyText.implicitHeight : clientListCol.implicitHeight

          Text {
            id: emptyText
            visible: root.clientCount === 0
            width: parent.width
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: Qt.darker(root.foreground, 1.6)
            font.family: Style.font.family
            font.pixelSize: Style.font.bodySmall
            text: root.serverOnline
              ? "No devices listening. Open the link above on your phone or computer to listen in real time."
              : "Server is not running. Click 'Start Server' below to begin."
          }

          Column {
            id: clientListCol
            visible: root.clientCount > 0
            width: parent.width
            spacing: Style.space(8)

            Repeater {
              model: root.clients

              Rectangle {
                required property var modelData
                width: parent.width
                height: Style.space(38)
                color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.05)
                radius: Style.cornerRadius

                RowLayout {
                  anchors.fill: parent
                  anchors.leftMargin: Style.space(10)
                  anchors.rightMargin: Style.space(10)
                  spacing: Style.space(8)

                  Text {
                    text: "󰏲"
                    textFormat: Text.PlainText
                    color: Commons.Color.accent
                    font.family: Style.font.family
                    font.pixelSize: Style.font.body
                  }

                  Text {
                    Layout.fillWidth: true
                    text: modelData.name || "Device"
                    textFormat: Text.PlainText
                    color: root.foreground
                    font.family: Style.font.family
                    font.pixelSize: Style.font.bodySmall
                    font.bold: true
                    elide: Text.ElideRight
                  }

                  Text {
                    text: modelData.ip || ""
                    textFormat: Text.PlainText
                    color: Qt.darker(root.foreground, 1.5)
                    font.family: Style.font.family
                    font.pixelSize: Style.font.caption
                  }
                }
              }
            }
          }
        }

        // Server offline helper
        Button {
          visible: !root.serverOnline
          width: parent.width
          text: "Start SoundThis Server"
          iconText: "󰐍"
          bordered: true
          onClicked: root.startServer()
        }
      }
    }
  }
}
