import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "io.github.cesarfilho.abyss"

  readonly property string stateDir: Quickshell.env("HOME") + "/.local/state/abyss"
  readonly property string configDir: Quickshell.env("HOME") + "/.config/abyss"

  property var stats: null
  property string mode: "abyss"
  property string quality: "ultra"
  property bool opened: false
  // The plugin is only the front end; the `abyss` program must be installed too (see the README).
  property bool binaryMissing: false

  readonly property color fg: bar ? bar.barForeground : Color.foreground
  readonly property string font: bar ? bar.fontFamily : Style.font.family
  readonly property var zoneColors: ["#6fd3ea", "#3d8ed0", "#4a6fc4", "#6a55b8", "#9a4fb0", "#c0506a"]
  readonly property color accent: stats ? zoneColors[Math.min(5, stats.zone)] : zoneColors[0]

  function close() { opened = false }
  function toggle() { opened = !opened }
  onOpenedChanged: if (opened) probe.running = true

  function setMode(next) {
    root.mode = next
    modeProcess.command = ["abyss", "mode", next]
    modeProcess.running = true
  }

  function setQuality(next) {
    root.quality = next
    qualityProcess.command = ["abyss", "quality", next]
    qualityProcess.running = true
  }

  function fmtDuration(secs) {
    var s = Math.floor(Number(secs) || 0)
    var h = Math.floor(s / 3600)
    var m = Math.floor((s % 3600) / 60)
    if (h > 0) return h + " h " + (m < 10 ? "0" : "") + m
    if (m > 0) return m + " min"
    return s + " s"
  }

  function fmtDay(day) {
    if (day === null || day === undefined) return ""
    var d = new Date(Number(day) * 86400000)
    return d.getUTCDate() + "/" + (d.getUTCMonth() + 1)
  }

  function unlockedCount() {
    if (!stats) return 0
    var n = 0
    for (var i = 0; i < stats.trophies.length; i++) if (stats.trophies[i].unlocked) n++
    return n
  }

  // Last 14 days as [{day, secs}], filling gaps with zero.
  function activity() {
    var out = []
    if (!stats) return out
    var byDay = {}
    for (var i = 0; i < stats.daily.length; i++) byDay[stats.daily[i].day] = stats.daily[i].secs
    var today = stats.today
    for (var d = today - 13; d <= today; d++) out.push({ day: d, secs: byDay[d] || 0 })
    return out
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // The Abyss binary rewrites these on every save; the shell only watches, never polls.
  FileView {
    id: statsFile
    path: root.stateDir + "/stats.json"
    watchChanges: true
    printErrors: false
    onLoaded: { try { root.stats = JSON.parse(text()) } catch (e) {} }
    onFileChanged: reload()
  }

  // A file that does not exist yet (before the first run) cannot be watched: poll until it appears.
  Timer {
    interval: 3000
    repeat: true
    running: root.stats === null
    onTriggered: statsFile.reload()
  }

  // `abyss mode|quality` write this file; the panel switches only ask the CLI and follow the file.
  FileView {
    path: root.configDir + "/config.json"
    watchChanges: true
    printErrors: false
    onLoaded: {
      try {
        var c = JSON.parse(text())
        root.mode = c.mode === "default" ? "default" : "abyss"
        if (c.quality) root.quality = c.quality
      } catch (e) {}
    }
    onFileChanged: reload()
  }

  Process {
    id: probe
    command: ["bash", "-lc", "command -v abyss >/dev/null"]
    running: true
    onExited: function(exitCode, exitStatus) { root.binaryMissing = exitCode !== 0 }
  }

  Process { id: modeProcess }
  Process { id: qualityProcess }

  IpcHandler {
    target: "io.github.cesarfilho.abyss"

    function open(): void { root.opened = true }
    function close(): void { root.close() }
    function toggle(): void { root.toggle() }
    function mode(next: string): void { root.setMode(next === "default" ? "default" : "abyss") }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.mode === "abyss" ? "󰈺" : "󰹑"
    tooltipText: (root.mode === "abyss" ? "Abyss" : "Screensaver padrão")
      + (root.stats ? " · nível " + root.stats.level + " · " + root.stats.depth + " m" : "")
    active: root.opened
    onPressed: function(b) {
      if (b === Qt.RightButton) root.setMode(root.mode === "abyss" ? "default" : "abyss")
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keys
    contentWidth: panel.fittedContentWidth(Style.space(440))
    contentHeight: panel.fittedContentHeight(content.height)

    PanelKeyCatcher {
      id: keys
      anchors.fill: parent
      onCloseRequested: root.close()

      // Loaded only while the panel is open, so a closed panel costs nothing.
      Loader {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        active: root.opened
        height: item ? item.implicitHeight : 0
        sourceComponent: StatsContent { host: root; width: content.width }
      }
    }
  }
}
