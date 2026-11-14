import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts

ApplicationWindow {
    id: window
    visible: true
    width: 460
    height: 640
    minimumWidth: 420
    minimumHeight: 580
    title: "SoundThis"
    color: themeBackground

    Material.theme: Material.Dark
    Material.accent: themeAccent
    Material.roundedScale: themeRadius

    // Dynamic Theme Tokens (synced from OS /api/theme)
    property color themeBackground: "#1a1b26"
    property color themeDarkBackground: "#13141c"
    property color themeDarkerBackground: "#0e0e14"
    property color themeSurface: "#24283b"
    property color themeForeground: "#c0caf5"
    property color themeMuted: "#565f89"
    property color themeAccent: "#7aa2f7"
    property color themeGreen: "#9ece6a"
    property color themeRed: "#f7768e"
    property color themeYellow: "#e0af68"
    property int themeRadius: 12

    // Dynamic Server Telemetry
    property bool serverOnline: false
    property bool streamingActive: true
    property string lanIp: "127.0.0.1"
    property int port: 9697
    property string streamUrl: "http://127.0.0.1:9697"
    property string qrUrl: "http://127.0.0.1:9697/api/qr.svg"
    property var clients: []
    property int clientCount: 0
    property bool keepStreamingOnClose: false

    // Functions to talk to Rust Axum API
    function fetchTheme() {
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + port + "/api/theme");
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    var data = JSON.parse(xhr.responseText);
                    if (data.accent) themeAccent = data.accent;
                    if (data.background) themeBackground = data.background;
                    if (data.dark_background) themeDarkBackground = data.dark_background;
                    if (data.darker_background) themeDarkerBackground = data.darker_background;
                    if (data.lighter_background) themeSurface = data.lighter_background;
                    if (data.bright_foreground) themeForeground = data.bright_foreground;
                    if (data.muted) themeMuted = data.muted;
                    if (data.green) themeGreen = data.green;
                    if (data.red) themeRed = data.red;
                    if (data.yellow) themeYellow = data.yellow;
                } catch (e) {}
            }
        };
        xhr.send();
    }

    function fetchStatus() {
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + port + "/api/status");
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                if (xhr.status === 200) {
                    try {
                        var data = JSON.parse(xhr.responseText);
                        serverOnline = true;
                        streamingActive = !!data.enabled;
                        if (data.lan_ip) lanIp = data.lan_ip;
                        if (data.port) port = data.port;
                        streamUrl = "http://" + lanIp + ":" + port;
                        qrUrl = "http://127.0.0.1:" + port + "/api/qr.svg";
                        clients = data.clients || [];
                        clientCount = clients.length;
                    } catch (e) {
                        serverOnline = false;
                    }
                } else {
                    serverOnline = false;
                }
            }
        };
        xhr.send();
    }

    function toggleStreaming() {
        var xhr = new XMLHttpRequest();
        xhr.open("POST", "http://127.0.0.1:" + port + "/api/toggle");
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                fetchStatus();
            }
        };
        xhr.send("{}");
    }

    function copyUrl() {
        var xhr = new XMLHttpRequest();
        xhr.open("POST", "http://127.0.0.1:" + port + "/api/copy");
        xhr.send();
        copiedTimer.restart();
    }

    function openBrowser() {
        Qt.openUrlExternally(streamUrl);
    }

    function fetchKeepStreaming() {
        var xhr = new XMLHttpRequest();
        xhr.open("GET", "http://127.0.0.1:" + port + "/api/config/keep_streaming");
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    var data = JSON.parse(xhr.responseText);
                    if (data.keep !== undefined) {
                        keepStreamingOnClose = !!data.keep;
                    }
                } catch (e) {}
            }
        };
        xhr.send();
    }

    function saveKeepStreamingSetting(keep) {
        keepStreamingOnClose = keep;
        var xhr = new XMLHttpRequest();
        xhr.open("POST", "http://127.0.0.1:" + port + "/api/config/keep_streaming");
        xhr.setRequestHeader("Content-Type", "application/json");
        xhr.send(JSON.stringify({ "keep": keep }));
    }

    // Keyboard Shortcuts
    Shortcut {
        sequence: "Space"
        onActivated: toggleStreaming()
    }
    Shortcut {
        sequence: "Ctrl+C"
        onActivated: copyUrl()
    }
    Shortcut {
        sequence: "Ctrl+Q"
        onActivated: Qt.quit()
    }
    Shortcut {
        sequence: "Escape"
        onActivated: Qt.quit()
    }

    Timer {
        id: copiedTimer
        interval: 1800
        repeat: false
    }

    // Periodic Polling
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            fetchStatus();
            fetchTheme();
        }
    }

    Component.onCompleted: {
        fetchStatus();
        fetchTheme();
        fetchKeepStreaming();
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 16

        // 1. Header Bar
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Text {
                text: ""
                font.pixelSize: 22
                color: themeAccent
            }

            Text {
                text: "SoundThis"
                font.pixelSize: 20
                font.bold: true
                color: themeForeground
            }

            Item { Layout.fillWidth: true }

            // Status Badge
            Rectangle {
                implicitHeight: 28
                implicitWidth: statusText.implicitWidth + 20
                radius: Math.min(14, themeRadius)
                color: !serverOnline
                    ? Qt.rgba(0.97, 0.46, 0.56, 0.15)
                    : (streamingActive ? Qt.rgba(0.62, 0.81, 0.42, 0.15) : Qt.rgba(0.34, 0.37, 0.54, 0.2))
                border.width: 1
                border.color: !serverOnline
                    ? themeRed
                    : (streamingActive ? themeGreen : themeMuted)

                Text {
                    id: statusText
                    anchors.centerIn: parent
                    text: !serverOnline
                        ? "OFFLINE"
                        : (streamingActive ? "STREAM ACTIVE" : "MUTED")
                    font.pixelSize: 11
                    font.bold: true
                    color: !serverOnline
                        ? themeRed
                        : (streamingActive ? themeGreen : themeForeground)
                }
            }
        }

        // 2. Big Start / Stop Streaming Action Button
        Button {
            id: toggleBtn
            Layout.fillWidth: true
            Layout.preferredHeight: 52
            Material.background: streamingActive ? themeRed : themeAccent
            Material.roundedScale: themeRadius
            onClicked: toggleStreaming()

            contentItem: RowLayout {
                anchors.centerIn: parent
                spacing: 10

                Text {
                    text: streamingActive ? "⏸" : "▶"
                    font.pixelSize: 16
                    color: "#ffffff"
                }

                Text {
                    text: streamingActive ? "PAUSE STREAMING" : "START STREAMING"
                    font.pixelSize: 14
                    font.bold: true
                    color: "#ffffff"
                }
            }
        }

        // 3. QR Code & Stream URL Card
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 280
            radius: themeRadius
            color: themeSurface
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.08)

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 16
                spacing: 12

                // QR Code Image
                Rectangle {
                    Layout.alignment: Qt.AlignHCenter
                    width: 150
                    height: 150
                    radius: Math.min(8, themeRadius)
                    color: themeDarkerBackground
                    border.width: 1
                    border.color: Qt.rgba(1, 1, 1, 0.05)

                    Image {
                        anchors.fill: parent
                        anchors.margins: 6
                        source: qrUrl
                        fillMode: Image.PreserveAspectFit
                        smooth: false
                        cache: false
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: "Scan with phone or tablet to listen live"
                    font.pixelSize: 12
                    color: themeMuted
                }

                // Stream URL & Action Buttons
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 36
                        radius: Math.min(6, themeRadius)
                        color: themeDarkBackground
                        border.width: 1
                        border.color: Qt.rgba(1, 1, 1, 0.08)

                        Text {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            verticalAlignment: Text.AlignVCenter
                            text: streamUrl
                            font.pixelSize: 13
                            font.bold: true
                            color: themeForeground
                            elide: Text.ElideMiddle
                        }
                    }

                    Button {
                        Layout.preferredHeight: 36
                        Material.roundedScale: Math.min(6, themeRadius)
                        text: copiedTimer.running ? "Copied!" : "Copy"
                        onClicked: copyUrl()
                    }

                    Button {
                        Layout.preferredHeight: 36
                        Material.roundedScale: Math.min(6, themeRadius)
                        text: "Open"
                        onClicked: openBrowser()
                    }
                }
            }
        }

        // 4. Connected Listeners Section
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 8

            Text {
                text: "CONNECTED LISTENERS (" + clientCount + ")"
                font.pixelSize: 12
                font.bold: true
                color: themeMuted
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: themeRadius
                color: themeDarkBackground
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.06)

                ListView {
                    id: clientListView
                    anchors.fill: parent
                    anchors.margins: 8
                    clip: true
                    model: clients
                    spacing: 6

                    delegate: Rectangle {
                        width: clientListView.width
                        height: 38
                        radius: Math.min(6, themeRadius)
                        color: themeSurface

                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 8

                            Rectangle {
                                width: 8
                                height: 8
                                radius: 4
                                color: (modelData && modelData.listening) ? themeGreen : themeMuted
                            }

                            Text {
                                text: (modelData && modelData.name) ? modelData.name : "Web Listener"
                                font.pixelSize: 13
                                font.bold: true
                                color: themeForeground
                            }

                            Item { Layout.fillWidth: true }

                            Text {
                                text: (modelData && modelData.ip) ? modelData.ip : ""
                                font.pixelSize: 12
                                color: themeMuted
                            }
                        }
                    }

                    Text {
                        anchors.centerIn: parent
                        visible: clientCount === 0
                        text: "No devices currently connected"
                        font.pixelSize: 13
                        color: themeMuted
                    }
                }
            }
        }

        // 5. Footer Option: Keep streaming in bar when closed
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            CheckBox {
                id: keepStreamingCheck
                checked: keepStreamingOnClose
                onToggled: saveKeepStreamingSetting(checked)
                Material.accent: themeAccent
            }

            Text {
                Layout.fillWidth: true
                text: "Keep streaming in Omarchy bar when window is closed"
                font.pixelSize: 12
                color: themeForeground
                wrapMode: Text.WordWrap

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        keepStreamingCheck.checked = !keepStreamingCheck.checked;
                        saveKeepStreamingSetting(keepStreamingCheck.checked);
                    }
                }
            }
        }
    }
}
