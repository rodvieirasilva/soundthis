import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Shapes
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Panel {
  id: root
  moduleName: "soundthis"
  ipcTarget: "soundthis"

  visible: root.serverOnline
  implicitWidth: root.serverOnline ? button.implicitWidth : 0
  implicitHeight: root.serverOnline ? button.implicitHeight : 0

  property bool serverOnline: false
  property bool serverEnabled: true
  property int clientCount: 0
  property var clients: []
  property string lanIp: "127.0.0.1"
  property int port: 9697
  property string version: "0.6.0"

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

  function openApp() {
    if (bar) bar.run("soundthis")
    root.close()
  }

  function exitApp() {
    if (bar) bar.run("soundthis stop")
    root.serverOnline = false
    root.close()
  }

  function stopServer() {
    exitApp()
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
    command: ["curl", "-fsS", "--max-time", "1", "http://127.0.0.1:9697/api/v1/status"]
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
          if (data.version) root.version = data.version
        } catch (e) {
          root.serverOnline = false
        }
      }
    }
  }

  Process {
    id: toggleProc
    command: ["curl", "-fsS", "-X", "POST", "http://127.0.0.1:9697/api/v1/toggle"]
    onExited: {
      root.refreshStatus()
    }
  }

  component SoundThisBarIcon: Item {
    id: barIconRoot
    property real iconSize: Style.font.icon
    property color color: Commons.Color.foreground

    width: iconSize
    height: iconSize
    implicitWidth: iconSize
    implicitHeight: iconSize

    Shape {
      preferredRendererType: Shape.CurveRenderer
      width: 24
      height: 24
      scale: barIconRoot.iconSize / 24
      transformOrigin: Item.TopLeft

      ShapePath {
        strokeColor: barIconRoot.color
        strokeWidth: 2.4
        fillColor: "transparent"
        capStyle: ShapePath.RoundCap
        joinStyle: ShapePath.RoundJoin
        PathSvg {
          path: "M6.5 17.5 C3.2 14.2 3.2 9.8 6.5 6.5 M17.5 6.5 C20.8 9.8 20.8 14.2 17.5 17.5"
        }
      }
    }

    Rectangle {
      x: (barIconRoot.width - width) / 2
      y: (barIconRoot.height - height) / 2
      width: barIconRoot.iconSize * (5.2 / 24.0)
      height: width
      radius: width / 2
      color: barIconRoot.color
    }
  }

  component SoundThisHeroIcon: Item {
    id: heroIconRoot
    property color color: Commons.Color.accent

    width: Style.space(36)
    height: Style.space(36)
    implicitWidth: width
    implicitHeight: height

    Shape {
      anchors.fill: parent
      preferredRendererType: Shape.CurveRenderer
      scale: parent.width / 40.0
      transformOrigin: Item.Center

      ShapePath {
        strokeColor: heroIconRoot.color
        strokeWidth: 3.2
        fillColor: "transparent"
        capStyle: ShapePath.RoundCap
        joinStyle: ShapePath.RoundJoin
        PathSvg {
          path: "M 8.2 31.8 C 1.7 25.3 1.7 14.7 8.2 8.2 M 13 27 C 9.2 23.2 9.2 16.8 13 13 M 27 13 C 30.8 16.8 30.8 23.2 27 27 M 31.8 8.2 C 38.3 14.7 38.3 25.3 31.8 31.8"
        }
      }

      ShapePath {
        strokeColor: "transparent"
        strokeWidth: 0
        fillColor: heroIconRoot.color
        PathSvg {
          path: "M 16.7 20 a 3.3 3.3 0 1 0 6.6 0 a 3.3 3.3 0 1 0 -6.6 0"
        }
      }
    }
  }

  Component.onCompleted: {
    root.refreshStatus()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    iconComponent: Component {
      SoundThisBarIcon {
        iconSize: button.iconSize
        color: (root.serverOnline && root.serverEnabled) ? Commons.Color.accent : root.foreground
        opacity: (root.serverOnline && root.serverEnabled) ? 1.0 : 0.35
      }
    }
    tooltipText: root.serverOnline
      ? (root.serverEnabled
          ? "SoundThis v" + root.version + ": Sharing on " + root.clientUrl + (root.clientCount > 0 ? " (" + root.clientCount + " listening)" : "")
          : "SoundThis v" + root.version + ": Sharing Disabled (Right-click to enable)")
      : "SoundThis v" + root.version + ": Server Offline (Click to open)"

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
          foreground: root.foreground

          iconComponent: Component {
            SoundThisHeroIcon {
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

        // Action Buttons: 60/40 visual pattern (Primary action larger, then secondary action)
        RowLayout {
          width: parent.width
          spacing: Style.space(8)

          Button {
            Layout.fillWidth: true
            Layout.preferredWidth: 60
            text: "Open App"
            iconText: "󰍹"
            bordered: true
            onClicked: root.openApp()
          }

          Button {
            Layout.fillWidth: true
            Layout.preferredWidth: 40
            text: "Copy Link"
            iconText: "󰆏"
            bordered: true
            onClicked: root.copyUrl()
          }
        }

        RowLayout {
          width: parent.width
          spacing: Style.space(8)

          Button {
            Layout.fillWidth: true
            Layout.preferredWidth: 60
            text: "Open Web UI"
            iconText: "󰖟"
            bordered: true
            onClicked: root.openBrowser()
          }

          Button {
            Layout.fillWidth: true
            Layout.preferredWidth: 40
            text: "Exit"
            iconText: "󰗼"
            bordered: true
            onClicked: root.exitApp()
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

        PanelSeparator { foreground: root.foreground }

        // Footer matching webpage
        RowLayout {
          width: parent.width
          Text {
            Layout.fillWidth: true
            text: "48kHz Stereo • 16-bit PCM"
            textFormat: Text.PlainText
            color: Qt.darker(root.foreground, 1.8)
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }
          Text {
            text: "v" + root.version
            textFormat: Text.PlainText
            color: Commons.Color.accent
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.bold: true
          }
        }
      }
    }
  }
}
