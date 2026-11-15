// Creed login screen for SDDM (Qt 6). One file, three layouts chosen by theme.conf:
// layout=creed (centred card), windows (time bottom-left, sign-in centre), macos (time on top).
import QtQuick 2.15
import QtQuick.Shapes 1.15

Rectangle {
    id: root
    width: 1920; height: 1080
    readonly property string layout: config.layout || "creed"
    readonly property color accent: config.accent || "#e8905a"
    readonly property color fg: "#f5f1ee"
    readonly property string fontFamily: config.font || "Inter"
    property int sessionIndex: sessionModel.lastIndex >= 0 ? sessionModel.lastIndex : 0
    property string failMessage: ""

    gradient: Gradient {
        GradientStop { position: 0.0; color: config.bgTop || "#2a1d1a" }
        GradientStop { position: 1.0; color: config.bgBottom || "#3a2620" }
    }

    Image {
        anchors.fill: parent
        source: config.background || ""
        fillMode: Image.PreserveAspectCrop
        visible: status === Image.Ready
        asynchronous: true
    }
    // Soft glows (radial gradients, so the theme works without any image).
    Repeater {
        model: [[0.78, 0.25, 0.45, config.glow1 || "#e8905a"], [0.2, 0.78, 0.42, config.glow2 || "#c25a6c"]]
        Shape {
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeWidth: -1
                fillGradient: RadialGradient {
                    centerX: root.width * modelData[0]; centerY: root.height * modelData[1]
                    focalX: centerX; focalY: centerY
                    centerRadius: root.width * modelData[2]; focalRadius: 0
                    GradientStop { position: 0.0; color: Qt.alpha(modelData[3], 0.45) }
                    GradientStop { position: 0.55; color: Qt.alpha(modelData[3], 0.16) }
                    GradientStop { position: 1.0; color: Qt.alpha(modelData[3], 0.0) }
                }
                startX: 0; startY: 0
                PathLine { x: root.width; y: 0 }
                PathLine { x: root.width; y: root.height }
                PathLine { x: 0; y: root.height }
                PathLine { x: 0; y: 0 }
            }
        }
    }
    Rectangle { anchors.fill: parent; color: "#000000"; opacity: root.layout === "windows" ? 0.25 : 0.18 }

    Connections {
        target: sddm
        function onLoginFailed() { root.failMessage = "Wrong password - try again"; password.text = ""; password.focus = true }
        function onLoginSucceeded() { root.failMessage = "" }
    }

    function doLogin() {
        root.failMessage = ""
        sddm.login(userField.text, password.text, root.sessionIndex)
    }

    Timer { interval: 1000; running: true; repeat: true; onTriggered: clock.now = new Date() }

    // Clock + date
    Column {
        id: clock
        property date now: new Date()
        spacing: 4
        anchors.horizontalCenter: root.layout === "windows" ? undefined : parent.horizontalCenter
        anchors.left: root.layout === "windows" ? parent.left : undefined
        anchors.leftMargin: 64
        anchors.top: root.layout === "macos" ? parent.top : undefined
        anchors.topMargin: 90
        anchors.bottom: root.layout === "windows" ? parent.bottom : (root.layout === "creed" ? card.top : undefined)
        anchors.bottomMargin: root.layout === "windows" ? 110 : 36
        Text {
            text: Qt.formatTime(clock.now, "hh:mm")
            color: root.fg; font.family: root.fontFamily
            font.pixelSize: root.layout === "creed" ? 120 : 132
            font.weight: root.layout === "windows" ? Font.Light : Font.Bold
            anchors.horizontalCenter: root.layout === "windows" ? undefined : parent.horizontalCenter
        }
        Text {
            text: Qt.formatDate(clock.now, "dddd, d MMMM")
            color: root.layout === "creed" ? root.accent : root.fg
            font.family: root.fontFamily; font.pixelSize: root.layout === "windows" ? 30 : 24
            anchors.horizontalCenter: root.layout === "windows" ? undefined : parent.horizontalCenter
        }
    }

    // Sign-in card
    Rectangle {
        id: card
        width: 380; height: inner.implicitHeight + 48
        radius: root.layout === "windows" ? 8 : 26
        color: root.layout === "creed" ? "#2a2220" : "transparent"
        opacity: 0.96
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: root.layout === "macos" ? undefined : parent.verticalCenter
        anchors.verticalCenterOffset: root.layout === "creed" ? 90 : 40
        anchors.bottom: root.layout === "macos" ? parent.bottom : undefined
        anchors.bottomMargin: 110

        Column {
            id: inner
            anchors.centerIn: parent
            width: parent.width - 48
            spacing: 14

            Rectangle {
                width: 84; height: 84; radius: 42
                color: root.accent
                anchors.horizontalCenter: parent.horizontalCenter
                Text {
                    anchors.centerIn: parent
                    text: (userField.text || "?").charAt(0).toUpperCase()
                    color: "#ffffff"; font.pixelSize: 38; font.bold: true; font.family: root.fontFamily
                }
            }
            TextInput {
                id: userField
                width: parent.width
                text: userModel.lastUser
                color: root.fg; font.family: root.fontFamily; font.pixelSize: 20; font.weight: Font.DemiBold
                horizontalAlignment: TextInput.AlignHCenter
                KeyNavigation.tab: password
                Accessible.name: "User name"
                Text {
                    anchors.centerIn: parent
                    visible: !userField.text
                    text: "User name"
                    color: root.fg; opacity: 0.55; font.family: root.fontFamily; font.pixelSize: 18
                }
            }
            Item {
                width: parent.width; height: 46
                Rectangle {
                    anchors.fill: parent
                    radius: root.layout === "windows" ? 4 : 23
                    color: "#ffffff"; opacity: 0.14
                }
                Rectangle {
                    anchors.fill: parent
                    radius: root.layout === "windows" ? 4 : 23
                    color: "transparent"
                    border.color: password.activeFocus ? root.accent : "transparent"; border.width: 2
                }
                TextInput {
                    id: password
                    anchors.fill: parent; anchors.leftMargin: 20; anchors.rightMargin: 20
                    echoMode: TextInput.Password
                    passwordCharacter: "•"
                    verticalAlignment: TextInput.AlignVCenter
                    horizontalAlignment: TextInput.AlignHCenter
                    color: root.fg; font.family: root.fontFamily; font.pixelSize: 18
                    focus: true
                    Keys.onReturnPressed: root.doLogin()
                    Keys.onEnterPressed: root.doLogin()
                    Accessible.name: "Password"
                }
                Text {
                    anchors.centerIn: parent
                    visible: !password.text && !password.activeFocus
                    text: root.layout === "windows" ? "PIN or password" : "Enter Password"
                    color: root.fg; opacity: 0.55; font.family: root.fontFamily; font.pixelSize: 16
                }
            }
            Text {
                width: parent.width
                text: root.failMessage
                visible: text.length > 0
                color: "#ff8a80"; font.family: root.fontFamily; font.pixelSize: 14
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

    // Bottom row: session switcher + power
    Row {
        anchors.right: parent.right; anchors.bottom: parent.bottom; anchors.margins: 28
        spacing: 12
        Repeater {
            model: [
                { label: sessionModel.data(sessionModel.index(root.sessionIndex, 0), Qt.UserRole + 4) || "Session", act: "session" },
                { label: "Sleep", act: "suspend" },
                { label: "Restart", act: "reboot" },
                { label: "Shut down", act: "poweroff" }
            ]
            Rectangle {
                width: label.implicitWidth + 32; height: 38; radius: 19
                color: mouse.containsMouse ? Qt.rgba(1, 1, 1, 0.2) : Qt.rgba(1, 1, 1, 0.1)
                Text { id: label; anchors.centerIn: parent; text: modelData.label; color: root.fg; font.family: root.fontFamily; font.pixelSize: 14 }
                Accessible.role: Accessible.Button
                Accessible.name: modelData.label
                MouseArea {
                    id: mouse; anchors.fill: parent; hoverEnabled: true
                    onClicked: {
                        if (modelData.act === "session") root.sessionIndex = (root.sessionIndex + 1) % Math.max(1, sessionModel.rowCount())
                        else if (modelData.act === "suspend") sddm.suspend()
                        else if (modelData.act === "reboot") sddm.reboot()
                        else sddm.powerOff()
                    }
                }
            }
        }
    }
}
