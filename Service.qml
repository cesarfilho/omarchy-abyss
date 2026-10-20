import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

// Replaces the built-in screensaver launch. After the idle timeout it starts either Abyss or
// the stock Omarchy screensaver, depending on `abyss mode`, and wakes it on activity.
// Keep `idle.screensaver` at 0 in shell.json so the built-in timer stays out of the way.
Item {
  id: root

  property var shell: null

  // `abyss idle SECONDS` (or the panel) writes this to ~/.config/abyss/config.json.
  property int timeoutSeconds: 150
  property bool launched: false

  function launch() {
    root.launched = true
    launchProcess.command = ["bash", "-lc",
      "[[ -e \"$HOME/.local/state/omarchy/indicators/stay-awake\" ]] && exit 0; " +
      "[[ $(omarchy-shell lock isLocked 2>/dev/null) == \"true\" ]] && exit 0; " +
      "abyss-screensaver"]
    launchProcess.running = true
  }

  function wake() {
    if (!root.launched) return
    root.launched = false
    // Abyss shows its summary on SIGUSR1; the stock screensaver just gets closed.
    wakeProcess.command = ["bash", "-lc",
      "abyss wake; pkill -x ttfx; pkill -f '[o]rg.omarchy.screensaver'; true"]
    wakeProcess.running = true
  }

  FileView {
    path: Quickshell.env("HOME") + "/.config/abyss/config.json"
    watchChanges: true
    printErrors: false
    onLoaded: {
      try {
        var n = Number(JSON.parse(text()).idle_secs)
        if (n >= 10) root.timeoutSeconds = n
      } catch (e) {}
    }
    onFileChanged: reload()
  }

  IdleMonitor {
    enabled: true
    timeout: root.timeoutSeconds
    respectInhibitors: true
    onIsIdleChanged: isIdle ? root.launch() : root.wake()
  }

  // Manual triggers: `quickshell ipc -p $OMARCHY_PATH/shell call abyss.service launch|wake`.
  IpcHandler {
    target: "abyss.service"

    function launch(): void { root.launch() }
    function wake(): void { root.wake() }
  }

  Process { id: launchProcess }
  Process { id: wakeProcess }
}
