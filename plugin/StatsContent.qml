import QtQuick
import qs.Commons
import qs.Ui

Column {
  id: col

  property var host: null

  readonly property var s: host ? host.stats : null
  readonly property color fg: host.fg
  readonly property color dim: Qt.darker(fg, 1.5)
  readonly property color accent: host.accent
  readonly property string ff: host.font
  readonly property color gold: "#e8c25a"
  property string hint: ""

  spacing: Style.space(12)

  function tint(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }

  // ---------- Hero: level ring + headline numbers ----------
  Item {
    width: parent.width
    height: Style.space(84)

    Canvas {
      id: ring
      width: Style.space(80)
      height: width
      anchors.verticalCenter: parent.verticalCenter
      property real value: col.s ? col.s.progress : 0
      onValueChanged: requestPaint()
      onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        var r = width / 2 - 5
        var c = width / 2
        ctx.lineWidth = 7
        ctx.lineCap = "round"
        ctx.strokeStyle = col.tint(col.fg, 0.12)
        ctx.beginPath(); ctx.arc(c, c, r, 0, Math.PI * 2); ctx.stroke()
        if (value > 0.005) {
          ctx.strokeStyle = col.accent
          ctx.beginPath(); ctx.arc(c, c, r, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * value); ctx.stroke()
        }
      }
      Component.onCompleted: requestPaint()

      Column {
        anchors.centerIn: parent
        Text {
          anchors.horizontalCenter: parent.horizontalCenter
          textFormat: Text.PlainText
          text: col.s ? col.s.level : "—"
          color: col.fg
          font.family: col.ff
          font.pixelSize: Style.font.display
          font.bold: true
        }
        Text {
          anchors.horizontalCenter: parent.horizontalCenter
          text: "NÍVEL"
          color: col.dim
          font.family: col.ff
          font.pixelSize: Style.font.caption
          font.letterSpacing: 1.2
        }
      }
    }

    Column {
      anchors.left: ring.right
      anchors.leftMargin: Style.space(16)
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(3)

      Text {
        textFormat: Text.PlainText
        text: col.s ? col.s.zoneName : "Abyss"
        color: col.accent
        font.family: col.ff
        font.pixelSize: Style.font.heading
        font.bold: true
      }
      Text {
        textFormat: Text.PlainText
        text: col.s ? col.s.depth.toLocaleString(Qt.locale(), "f", 0) + " m de profundidade" : "sem dados ainda"
        color: col.fg
        font.family: col.ff
        font.pixelSize: Style.font.bodySmall
      }
      Text {
        textFormat: Text.PlainText
        text: col.s ? Math.round(col.s.xp) + " / " + Math.round(col.s.xpNext) + " XP" : ""
        color: col.dim
        font.family: col.ff
        font.pixelSize: Style.font.bodySmall
      }
      Row {
        spacing: Style.space(6)
        Chip { text: col.s ? "★ " + col.s.points + " pts" : "★ 0"; tone: col.gold }
        Chip { text: col.s ? "󰈸 " + col.s.streak + " d" : "󰈸 0"; tone: "#ee8a4a" }
      }
    }
  }

  // ---------- Depth gauge: six zones, marker at current depth ----------
  Column {
    width: parent.width
    spacing: Style.space(4)

    Item {
      width: parent.width
      height: Style.space(10)

      Row {
        anchors.fill: parent
        spacing: Style.space(2)
        Repeater {
          model: 6
          Rectangle {
            required property int index
            width: (parent.width - parent.spacing * 5) / 6
            height: parent.height
            radius: height / 2
            color: col.host.zoneColors[index]
            opacity: col.s && index <= col.s.zone ? 0.9 : 0.22
          }
        }
      }
      Rectangle {
        visible: col.s !== null
        width: Style.space(4)
        height: parent.height + Style.space(6)
        radius: width / 2
        color: col.fg
        anchors.verticalCenter: parent.verticalCenter
        x: col.s ? Math.min(1, col.s.zoneF / 6) * (parent.width - width) : 0
        Behavior on x { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
      }
    }

    Row {
      width: parent.width
      Repeater {
        model: ["Superf.", "Luz", "Crepúsc.", "Meia-noite", "Abissal", "Hadal"]
        Text {
          required property int index
          required property string modelData
          width: parent.width / 6
          horizontalAlignment: Text.AlignHCenter
          textFormat: Text.PlainText
          text: modelData
          color: col.s && index === col.s.zone ? col.fg : col.dim
          font.family: col.ff
          font.pixelSize: Style.font.caption - 1
          font.bold: col.s !== null && index === col.s.zone
          elide: Text.ElideRight
        }
      }
    }
  }

  // ---------- Screensaver switch ----------
  Item {
    width: parent.width
    height: Style.space(36)

    Rectangle {
      anchors.fill: parent
      radius: Style.space(10)
      color: col.tint(col.fg, 0.07)
    }

    Row {
      anchors.fill: parent
      anchors.margins: Style.space(3)
      spacing: Style.space(3)

      Repeater {
        model: [
          { key: "abyss", label: "󰈺  Abyss" },
          { key: "default", label: "󰹑  Padrão Omarchy" }
        ]
        Rectangle {
          required property var modelData
          readonly property bool on: col.host.mode === modelData.key
          width: (parent.width - parent.spacing) / 2
          height: parent.height
          radius: Style.space(8)
          color: on ? col.tint(col.accent, 0.28) : "transparent"
          border.width: on ? 1 : 0
          border.color: col.tint(col.accent, 0.8)
          Behavior on color { ColorAnimation { duration: 160 } }

          Text {
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: parent.modelData.label
            color: parent.on ? col.fg : col.dim
            font.family: col.ff
            font.pixelSize: Style.font.bodySmall
            font.bold: parent.on
          }
          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: col.host.setMode(parent.modelData.key)
          }
        }
      }
    }
  }

  // ---------- Resolution ----------
  Row {
    width: parent.width
    spacing: Style.space(8)

    Text {
      anchors.verticalCenter: parent.verticalCenter
      width: Style.space(78)
      textFormat: Text.PlainText
      text: "RESOLUÇÃO"
      color: col.dim
      font.family: col.ff
      font.pixelSize: Style.font.caption
      font.bold: true
      font.letterSpacing: 1.0
    }

    Row {
      width: parent.width - Style.space(86)
      spacing: Style.space(4)

      Repeater {
        model: [
          { key: "low", label: "Baixa" }, { key: "medium", label: "Média" },
          { key: "high", label: "Alta" }, { key: "ultra", label: "Ultra" }, { key: "max", label: "Máx" }
        ]
        Rectangle {
          required property var modelData
          readonly property bool on: col.host.quality === modelData.key
          width: (parent.width - parent.spacing * 4) / 5
          height: Style.space(26)
          radius: Style.space(7)
          color: on ? col.tint(col.accent, 0.28) : col.tint(col.fg, 0.07)
          border.width: on ? 1 : 0
          border.color: col.tint(col.accent, 0.8)

          Text {
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: parent.modelData.label
            color: parent.on ? col.fg : col.dim
            font.family: col.ff
            font.pixelSize: Style.font.caption
            font.bold: parent.on
          }
          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: col.host.setQuality(parent.modelData.key)
          }
        }
      }
    }
  }

  // ---------- Stat tiles ----------
  Grid {
    id: tiles
    width: parent.width
    columns: 3
    spacing: Style.space(8)
    readonly property real cell: (width - spacing * 2) / 3

    Tile { label: "TEMPO NO MAR"; value: col.s ? col.host.fmtDuration(col.s.totalSecs) : "—" }
    Tile { label: "SESSÕES"; value: col.s ? String(col.s.sessions) : "—" }
    Tile { label: "MAIOR SESSÃO"; value: col.s ? col.host.fmtDuration(col.s.bestSessionSecs) : "—" }
    Tile {
      label: "RECIFE"
      value: col.s ? col.s.reef + "/" + col.s.reefMax : "—"
      bar: col.s ? col.s.reef / col.s.reefMax : 0
    }
    Tile { label: "BALEIAS"; value: col.s ? String(col.s.whales) : "—" }
    Tile { label: "NOITE NO MAR"; value: col.s ? col.host.fmtDuration(col.s.nightSecs) : "—" }
  }

  // ---------- Activity: last 14 days ----------
  Column {
    width: parent.width
    spacing: Style.space(6)

    SectionLabel { text: "ATIVIDADE · 14 DIAS" }

    Row {
      id: bars
      width: parent.width
      height: Style.space(44)
      spacing: Style.space(3)
      readonly property var series: col.host.activity()
      readonly property real peak: {
        var m = 1
        for (var i = 0; i < series.length; i++) m = Math.max(m, series[i].secs)
        return m
      }

      Repeater {
        model: bars.series
        Item {
          required property var modelData
          required property int index
          width: (bars.width - bars.spacing * 13) / 14
          height: bars.height

          Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: Math.max(2, parent.height * (modelData.secs / bars.peak))
            radius: Style.space(2)
            color: modelData.secs > 0 ? col.accent : col.tint(col.fg, 0.12)
            opacity: index === 13 ? 1.0 : 0.75
          }
        }
      }
    }
  }

  // ---------- Trophies ----------
  Column {
    width: parent.width
    spacing: Style.space(8)

    Row {
      width: parent.width
      SectionLabel {
        text: "TROFÉUS  " + col.host.unlockedCount() + " / " + (col.s ? col.s.trophies.length : 0)
        width: parent.width / 2
      }
      Text {
        width: parent.width / 2
        horizontalAlignment: Text.AlignRight
        textFormat: Text.PlainText
        text: col.hint
        color: col.dim
        font.family: col.ff
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }

    Grid {
      id: badges
      width: parent.width
      columns: 4
      spacing: Style.space(6)
      readonly property real cell: (width - spacing * 3) / 4

      Repeater {
        model: col.s ? col.s.trophies : []
        Rectangle {
          id: badge
          required property var modelData
          width: badges.cell
          height: Style.space(64)
          radius: Style.space(8)
          color: modelData.unlocked ? col.tint(col.gold, 0.16) : col.tint(col.fg, 0.06)
          border.width: modelData.unlocked ? 1 : 0
          border.color: col.tint(col.gold, 0.55)

          Column {
            anchors.centerIn: parent
            width: parent.width - Style.space(8)
            spacing: Style.space(2)

            Text {
              anchors.horizontalCenter: parent.horizontalCenter
              textFormat: Text.PlainText
              text: badge.modelData.unlocked ? "★" : "☆"
              color: badge.modelData.unlocked ? col.gold : col.dim
              opacity: badge.modelData.unlocked ? 1 : 0.5
              font.family: col.ff
              font.pixelSize: Style.font.heading
            }
            Text {
              width: parent.width
              horizontalAlignment: Text.AlignHCenter
              textFormat: Text.PlainText
              text: badge.modelData.name
              color: badge.modelData.unlocked ? col.fg : col.dim
              font.family: col.ff
              font.pixelSize: Style.font.caption - 1
              wrapMode: Text.WordWrap
              maximumLineCount: 2
              elide: Text.ElideRight
              height: Math.ceil(font.pixelSize * 3.4)
              verticalAlignment: Text.AlignTop
            }
            Rectangle {
              visible: !badge.modelData.unlocked
              width: parent.width
              height: 2
              radius: 1
              color: col.tint(col.fg, 0.15)
              Rectangle {
                width: parent.width * badge.modelData.progress
                height: parent.height
                radius: 1
                color: col.accent
              }
            }
          }

          MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            onEntered: col.hint = badge.modelData.unlocked
              ? badge.modelData.name + " · " + col.host.fmtDay(badge.modelData.day)
              : badge.modelData.desc + " · " + Math.round(badge.modelData.progress * 100) + "%"
            onExited: col.hint = ""
          }
        }
      }
    }
  }

  component SectionLabel: Text {
    textFormat: Text.PlainText
    color: col.dim
    font.family: col.ff
    font.pixelSize: Style.font.caption
    font.bold: true
    font.letterSpacing: 1.0
  }

  component Chip: Rectangle {
    property string text: ""
    property color tone: col.fg
    implicitWidth: chipText.implicitWidth + Style.space(14)
    implicitHeight: chipText.implicitHeight + Style.space(6)
    radius: height / 2
    color: col.tint(tone, 0.16)
    Text {
      id: chipText
      anchors.centerIn: parent
      textFormat: Text.PlainText
      text: parent.text
      color: parent.tone
      font.family: col.ff
      font.pixelSize: Style.font.caption
      font.bold: true
    }
  }

  component Tile: Rectangle {
    property string label: ""
    property string value: ""
    property real bar: -1
    width: tiles.cell
    height: Style.space(58)
    radius: Style.space(8)
    color: col.tint(col.fg, 0.06)

    Column {
      anchors.fill: parent
      anchors.margins: Style.space(9)
      spacing: Style.space(3)
      Text {
        textFormat: Text.PlainText
        text: parent.parent.value
        color: col.fg
        font.family: col.ff
        font.pixelSize: Style.font.heading
        font.bold: true
      }
      Text {
        textFormat: Text.PlainText
        text: parent.parent.label
        color: col.dim
        font.family: col.ff
        font.pixelSize: Style.font.caption - 1
        font.letterSpacing: 0.8
      }
      Rectangle {
        visible: parent.parent.bar >= 0
        width: parent.width
        height: 3
        radius: 1.5
        color: col.tint(col.fg, 0.12)
        Rectangle {
          width: parent.width * Math.max(0, Math.min(1, parent.parent.parent.bar))
          height: parent.height
          radius: parent.radius
          color: col.accent
        }
      }
    }
  }
}
