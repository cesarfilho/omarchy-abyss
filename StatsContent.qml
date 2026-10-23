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
  property int tab: 0
  property string selectedTrophyId: ""
  property string hoveredTrophyId: ""
  property string selectedCreatureId: ""
  property string hoveredCreatureId: ""
  property int keyboardIndex: 0

  spacing: Style.space(12)

  function tint(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
  function trophyStatus(t) {
    return t.unlocked
      ? t.desc + " · Desbloqueado em " + host.fmtDay(t.day)
      : t.desc + " · " + Math.round(t.progress * 100) + "% concluído"
  }
  function creatureStatus(c) {
    return c.found
      ? c.desc
      : "Ainda não encontrada · zonas " + (c.zones[0] + 1) + "–" + (c.zones[1] + 1)
  }
  function findById(items, id) {
    if (!items || !id) return null
    for (var i = 0; i < items.length; i++) if (items[i].id === id) return items[i]
    return null
  }
  function changeTab(direction) {
    tab = (tab + direction + 5) % 5
    keyboardIndex = 0
  }
  function moveSelection(dx, dy) {
    var items = tab === 1 && s ? s.bestiary : tab === 2 && s ? s.trophies : null
    if (tab === 4) {
      keyboardIndex = Math.max(0, Math.min(1, keyboardIndex + dx))
      return
    }
    if (!items || items.length === 0) return
    var columns = tab === 2 ? 4 : Math.max(1, Math.floor((width + Style.space(6)) / (Style.space(30) + Style.space(6))))
    var delta = dx !== 0 ? dx : dy * columns
    keyboardIndex = Math.max(0, Math.min(items.length - 1, keyboardIndex + delta))
  }
  function activateSelection() {
    if (tab === 1 && s && s.bestiary[keyboardIndex]) selectedCreatureId = s.bestiary[keyboardIndex].id
    if (tab === 2 && s && s.trophies[keyboardIndex]) selectedTrophyId = s.trophies[keyboardIndex].id
    if (tab === 4 && keyboardIndex < 2) host.setMode(keyboardIndex === 0 ? "abyss" : "default")
  }

  // ---------- Missing program notice ----------
  Rectangle {
    visible: col.host.binaryMissing
    width: parent.width
    height: visible ? missingText.implicitHeight + Style.space(16) : 0
    radius: Style.space(8)
    color: col.tint("#ee8a4a", 0.16)
    border.width: 1
    border.color: col.tint("#ee8a4a", 0.5)
    Text {
      id: missingText
      anchors.verticalCenter: parent.verticalCenter
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: Style.space(10)
      textFormat: Text.PlainText
      text: "O programa abyss não está instalado. Na pasta deste plugin, rode ./install.sh --setup; veja o README."
      color: col.fg
      font.family: col.ff
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
    }
  }

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

  // ---------- Tabs ----------
  Row {
    width: parent.width
    spacing: Style.space(4)

    Repeater {
      model: ["Resumo", "Missões", "Troféus", "Cosméticos", "Ajustes"]
      Rectangle {
        required property var modelData
        required property int index
        readonly property bool on: col.tab === index
        Accessible.role: Accessible.PageTab
        Accessible.name: modelData
        Accessible.selected: on
        Accessible.description: on ? "Aba selecionada" : "Abrir aba"
        width: (parent.width - parent.spacing * 4) / 5
        height: Style.space(28)
        radius: Style.space(8)
        color: on ? col.tint(col.accent, 0.28) : col.tint(col.fg, 0.07)
        border.width: on ? 1 : 0
        border.color: col.tint(col.accent, 0.8)
        Text {
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: parent.modelData
          color: parent.on ? col.fg : col.dim
          font.family: col.ff
          font.pixelSize: Style.font.bodySmall
          font.bold: parent.on
        }
        MouseArea {
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: col.tab = parent.index
        }
      }
    }
  }

  // ---------- Cosmetic catalogue ----------
  Column {
    visible: col.tab === 3
    width: parent.width
    spacing: Style.space(8)
    SectionLabel { text: "PALETAS COSMÉTICAS · SEM VANTAGEM DE JOGO" }
    Repeater {
      model: col.s ? col.s.cosmetics : []
      Rectangle {
        required property var modelData
        width: parent.width
        height: Style.space(42)
        radius: Style.space(8)
        color: col.tint(col.fg, 0.06)
        border.width: modelData.selected ? 1 : 0
        border.color: col.accent
        Row {
          anchors.fill: parent
          anchors.margins: Style.space(9)
          spacing: Style.space(8)
          Text {
            width: parent.width - Style.space(90)
            anchors.verticalCenter: parent.verticalCenter
            text: modelData.name + (modelData.selected ? " · selecionada" : "")
            color: col.fg
            font.family: col.ff
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
          Rectangle {
            width: Style.space(72); height: Style.space(24); radius: Style.space(6)
            anchors.verticalCenter: parent.verticalCenter
            color: col.tint(col.accent, 0.2)
            opacity: modelData.unlocked || (col.s && col.s.points >= modelData.cost) ? 1.0 : 0.45
            Text {
              anchors.centerIn: parent
              text: modelData.unlocked ? (modelData.selected ? "Ativa" : "Usar") : modelData.cost + " pts"
              color: col.fg; font.family: col.ff; font.pixelSize: Style.font.caption
            }
            MouseArea {
              anchors.fill: parent
              enabled: !modelData.selected && (modelData.unlocked || (col.s && col.s.points >= modelData.cost))
              cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
              onClicked: col.host.cosmetic(modelData.unlocked ? "select" : "buy", modelData.id)
            }
          }
        }
      }
    }
  }

  // ---------- Depth gauge: six zones, marker at current depth ----------
  Column {
    visible: col.tab === 0
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
    visible: col.tab === 4
    width: parent.width
    height: modeSettings.implicitHeight

    Column {
      id: modeSettings
      width: parent.width
      spacing: Style.space(5)

      SectionLabel { text: "SCREENSAVER NA OCIOSIDADE" }
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "Escolha o que aparece quando você se afasta."
        color: col.dim
        font.family: col.ff
        font.pixelSize: Style.font.caption
      }

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
              required property int index
              readonly property bool on: col.host.mode === modelData.key
              width: (parent.width - parent.spacing) / 2
              height: parent.height
              radius: Style.space(8)
              Accessible.role: Accessible.RadioButton
              Accessible.name: modelData.key === "abyss" ? "Abyss" : "Screensaver padrão do Omarchy"
              Accessible.focused: col.keyboardIndex === index
              Accessible.checkable: true
              Accessible.checked: on
              color: on ? col.tint(col.accent, 0.28) : "transparent"
              border.width: col.keyboardIndex === index ? 2 : on ? 1 : 0
              border.color: col.keyboardIndex === index ? col.accent : col.tint(col.accent, 0.8)
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
                onClicked: {
                  col.keyboardIndex = parent.index
                  col.host.setMode(parent.modelData.key)
                }
              }
            }
          }
        }
      }
    }
  }

  // ---------- Resolution ----------
  Row {
    visible: col.tab === 4
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

  // ---------- Idle time ----------
  Row {
    visible: col.tab === 4
    width: parent.width
    spacing: Style.space(8)

    Text {
      anchors.verticalCenter: parent.verticalCenter
      width: Style.space(78)
      textFormat: Text.PlainText
      text: "INATIVIDADE"
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
          { secs: 60, label: "1 min" }, { secs: 150, label: "2:30" }, { secs: 300, label: "5 min" },
          { secs: 600, label: "10 min" }, { secs: 900, label: "15 min" }
        ]
        Rectangle {
          required property var modelData
          readonly property bool on: col.host.idleSecs === modelData.secs
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
            onClicked: col.host.setIdle(parent.modelData.secs)
          }
        }
      }
    }
  }

  // ---------- Actions ----------
  Row {
    visible: col.tab === 4
    width: parent.width
    spacing: Style.space(8)
    property bool confirmReset: false

    Rectangle {
      width: (parent.width - parent.spacing) / 2
      height: Style.space(30)
      radius: Style.space(8)
      color: col.tint(col.accent, 0.28)
      border.width: 1
      border.color: col.tint(col.accent, 0.8)
      Text {
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: "󰐊  Iniciar agora"
        color: col.fg
        font.family: col.ff
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: col.host.startNow()
      }
    }

    Rectangle {
      id: resetButton
      width: (parent.width - parent.spacing) / 2
      height: Style.space(30)
      radius: Style.space(8)
      color: parent.confirmReset ? col.tint("#c0506a", 0.35) : col.tint(col.fg, 0.07)
      Text {
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: resetButton.parent.confirmReset ? "Confirmar: apagar tudo" : "󰆴  Limpar estatísticas"
        color: resetButton.parent.confirmReset ? col.fg : col.dim
        font.family: col.ff
        font.pixelSize: Style.font.bodySmall
      }
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: {
          if (resetButton.parent.confirmReset) {
            col.host.resetStats()
            resetButton.parent.confirmReset = false
          } else {
            resetButton.parent.confirmReset = true
          }
        }
      }
    }
  }

  // ---------- Stat tiles ----------
  Grid {
    visible: col.tab === 0
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
    visible: col.tab === 0
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

  // ---------- Season banner ----------
  Rectangle {
    visible: col.tab === 0 && (col.s && col.s.season ? true : false)
    width: parent.width
    height: visible ? seasonText.implicitHeight + Style.space(12) : 0
    radius: Style.space(8)
    color: col.tint(col.accent, 0.14)
    Text {
      id: seasonText
      anchors.verticalCenter: parent.verticalCenter
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: Style.space(10)
      textFormat: Text.PlainText
      text: col.s && col.s.season ? "≈ " + col.s.season.name + " · " + col.s.season.desc : ""
      color: col.fg
      font.family: col.ff
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
    }
  }

  // ---------- Daily quests ----------
  Column {
    visible: col.tab === 1 && (col.s && col.s.quests && col.s.quests.length > 0 ? true : false)
    width: parent.width
    spacing: Style.space(6)

    SectionLabel {
      text: "MISSÕES DE HOJE  " + (col.s ? col.s.quests.filter(function(q) { return q.done }).length : 0) + " / " + (col.s ? col.s.quests.length : 0)
    }

    Repeater {
      model: col.s ? col.s.quests : []
      Item {
        id: quest
        required property var modelData
        width: parent.width
        height: Style.space(28)

        Text {
          id: questMark
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: quest.modelData.done ? "✔" : "○"
          color: quest.modelData.done ? col.gold : col.dim
          font.family: col.ff
          font.pixelSize: Style.font.body
        }
        Column {
          anchors.left: questMark.right
          anchors.leftMargin: Style.space(8)
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)
          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: quest.modelData.name + " — " + quest.modelData.desc
            color: quest.modelData.done ? col.dim : col.fg
            font.family: col.ff
            font.pixelSize: Style.font.caption
            elide: Text.ElideRight
          }
          Rectangle {
            width: parent.width
            height: 3
            radius: 1.5
            color: col.tint(col.fg, 0.12)
            Rectangle {
              width: parent.width * quest.modelData.progress
              height: parent.height
              radius: parent.radius
              color: quest.modelData.done ? col.gold : col.accent
            }
          }
        }
      }
    }
  }

  // ---------- Bestiary ----------
  Column {
    visible: col.tab === 1 && (col.s && col.s.bestiary ? true : false)
    width: parent.width
    spacing: Style.space(6)

    Row {
      width: parent.width
      SectionLabel {
        text: "BESTIÁRIO  " + (col.s ? col.s.bestiary.filter(function(c) { return c.found }).length : 0) + " / " + (col.s ? col.s.bestiary.length : 0)
      }
    }
    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "Setas navegam · clique ou Enter abre · Tab troca de aba"
      color: col.dim
      font.family: col.ff
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
    }

    Flow {
      width: parent.width
      spacing: Style.space(6)
      Repeater {
        model: col.s ? col.s.bestiary : []
        Rectangle {
          id: creature
          required property var modelData
          required property int index
          width: Style.space(30)
          height: Style.space(22)
          radius: Style.space(8)
          Accessible.role: Accessible.Button
          Accessible.name: modelData.found ? modelData.name : "Criatura ainda não encontrada"
          Accessible.focused: col.keyboardIndex === index
          Accessible.selected: col.selectedCreatureId === modelData.id
          Accessible.description: col.creatureStatus(modelData)
          color: modelData.found ? col.tint(col.accent, 0.18) : col.tint(col.fg, 0.06)
          border.width: col.keyboardIndex === index ? 2 : 0
          border.color: col.keyboardIndex === index ? col.accent : "transparent"
          Text {
            anchors.centerIn: parent
            textFormat: Text.PlainText
            text: creature.modelData.found ? "◆" : "?"
            color: creature.modelData.found ? col.accent : col.dim
            opacity: creature.modelData.found ? 1 : 0.5
            font.family: col.ff
            font.pixelSize: Style.font.body
          }
          MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            onEntered: col.hoveredCreatureId = creature.modelData.id
            onExited: col.hoveredCreatureId = ""
            onClicked: {
              col.keyboardIndex = creature.index
              col.selectedCreatureId = creature.modelData.id
            }
          }
        }
      }
    }

    Rectangle {
      readonly property var creature: col.findById(col.s ? col.s.bestiary : null, col.hoveredCreatureId || col.selectedCreatureId)
      visible: creature !== null
      width: parent.width
      height: visible ? creatureDetails.implicitHeight + Style.space(16) : 0
      radius: Style.space(8)
      color: col.tint(col.accent, 0.12)
      border.width: 1
      border.color: col.tint(col.accent, 0.35)

      Column {
        id: creatureDetails
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(8)
        spacing: Style.space(3)

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: parent.parent.creature
            ? (parent.parent.creature.found ? parent.parent.creature.name : "Descoberta pendente") : ""
          color: col.fg
          font.family: col.ff
          font.pixelSize: Style.font.bodySmall
          font.bold: true
        }
        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: parent.parent.creature ? col.creatureStatus(parent.parent.creature) : ""
          color: col.dim
          font.family: col.ff
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
        }
      }
    }
  }

  // ---------- Trophies ----------
  Column {
    visible: col.tab === 2
    width: parent.width
    spacing: Style.space(8)

    Row {
      width: parent.width
      SectionLabel {
        text: "TROFÉUS  " + col.host.unlockedCount() + " / " + (col.s ? col.s.trophies.length : 0)
      }
    }
    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "Setas navegam · clique ou Enter abre · Tab troca de aba"
      color: col.dim
      font.family: col.ff
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
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
          required property int index
          width: badges.cell
          height: Style.space(50)
          radius: Style.space(8)
          Accessible.role: Accessible.Button
          Accessible.name: modelData.name
          Accessible.focused: col.keyboardIndex === index
          Accessible.selected: col.selectedTrophyId === modelData.id
          Accessible.description: col.trophyStatus(modelData)
          color: modelData.unlocked ? col.tint(col.gold, 0.16) : col.tint(col.fg, 0.06)
          border.width: col.keyboardIndex === index ? 2 : modelData.unlocked ? 1 : 0
          border.color: col.keyboardIndex === index ? col.accent : col.tint(col.gold, 0.55)

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
            onEntered: col.hoveredTrophyId = badge.modelData.id
            onExited: col.hoveredTrophyId = ""
            onClicked: {
              col.keyboardIndex = badge.index
              col.selectedTrophyId = badge.modelData.id
            }
          }
        }
      }
    }

    Rectangle {
      readonly property var trophy: col.findById(col.s ? col.s.trophies : null, col.hoveredTrophyId || col.selectedTrophyId)
      visible: trophy !== null
      width: parent.width
      height: visible ? trophyDetails.implicitHeight + Style.space(16) : 0
      radius: Style.space(8)
      color: col.tint(trophy && trophy.unlocked ? col.gold : col.accent, 0.12)
      border.width: 1
      border.color: col.tint(trophy && trophy.unlocked ? col.gold : col.accent, 0.35)

      Column {
        id: trophyDetails
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: Style.space(8)
        spacing: Style.space(3)

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: parent.parent.trophy ? parent.parent.trophy.name : ""
          color: col.fg
          font.family: col.ff
          font.pixelSize: Style.font.bodySmall
          font.bold: true
        }
        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: parent.parent.trophy ? col.trophyStatus(parent.parent.trophy) : ""
          color: col.dim
          font.family: col.ff
          font.pixelSize: Style.font.caption
          wrapMode: Text.WordWrap
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
