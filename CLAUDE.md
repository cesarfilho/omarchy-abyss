# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Abyss: a gamified deep-sea screensaver for Omarchy (Hyprland + Quickshell). A Rust binary (`abyss`) renders a procedural pixel-art ocean in a terminal, earns XP/trophies while the machine is idle, and a Quickshell plugin (`plugin/`) triggers it and shows the stats. User-facing text is Portuguese.

## Commands

```bash
cargo build --release
cargo test                       # unit tests live only in src/state.rs
cargo test streak_and_wilt       # single test by name
./install.sh                     # build + install abyss, abyss-launch, abyss-screensaver, plugin QML
abyss run [--screensaver] [--no-intro] [--monitor NAME]
abyss status [--json] | mode [abyss|default|toggle] | quality [low|medium|high|ultra|max] | wake | snapshot
```

Env knobs: `ABYSS_XP_MULT` (faster XP), `ABYSS_NO_INTRO`, `ABYSS_STATE_DIR`. Env vars do NOT reach windows started through `abyss-launch` (Hyprland exec), so test XP/state changes by running `foot ... -e env ABYSS_XP_MULT=… abyss run` or by seeding `~/.local/state/abyss/save.json`.

## Architecture

**Rendering (`gfx.rs`, `world.rs`, `sprites.rs`)**: the scene is drawn into a `Frame` of square pixels, 2 wide × 4 tall per terminal cell. `Presenter::present` resamples each cell to a 2×3 sextant (U+1FB00 block) and fits two colours per cell (`split_two`), writing only changed cells with truecolor escapes. HUD and toasts are terminal text via `Overlay` (cells), not frame pixels; text size therefore equals the cell size, which the `quality` setting (terminal font size, see `QUALITIES` in `main.rs`) controls. Creatures are procedural (`sprites.rs`), drawn from a length in screen pixels so detail scales with resolution; sizes derive from a height-based `refw`, never from the frame width.

**World is a pure function of the wall clock (`world.rs`)**: fish, jellies, bubbles, snow, whale and bloom positions come from fixed seeds plus `now_secs()`, with no per-frame state. Every monitor runs the same world and draws only its slice (`Layout { pano_w, off }`, computed in `app.rs::layout` from `hyprctl monitors -j`), so a fish leaving one screen appears on the next. Anything added to the world must stay clock-deterministic and use panorama coordinates (subtract `self.off` when drawing). Depth is 2.5D: creatures have `z` (0 near .. 1 far), the seabed is 5 parallax planes (`PLANES`), and `World::draw` interleaves planes and creatures far to near.

**Game state (`state.rs`)**: `save.json` (XP, streak, reef, trophies, daily history) in `~/.local/state/abyss/`. Only one instance (the one that wins the `lock` file) earns XP and writes; the others reload the save every 5 s. Each `Save::store` also writes `stats.json`, which the plugin watches. `Session` is what the exit summary shows. Achievements are `GameEvent`s; the primary publishes them to `events.json` (`toast::Feed`) and secondaries poll it, so banners appear on every monitor.

**Other state files** in `~/.local/state/abyss/`: `mode` (abyss/default), `quality`, `last_session.json`, `lock`, `events.json`. The Rust side and the QML plugin communicate only through these files and the `abyss` CLI.

**ttfx** (`cutscene.rs`): only used for the exit summary (`waves`) and the optional manual intro; it blanks the screen, so achievements use in-scene toasts (`toast.rs`) instead.

## Omarchy integration (non-obvious)

- Never edit `/usr/share/omarchy/` (package-owned). The stock idle service always calls `omarchy-launch-screensaver`, so this project sets `idle.screensaver` to `0` in `~/.config/omarchy/shell.json` and the plugin's `Service.qml` (an `IdleMonitor`) starts `abyss-screensaver`, which picks Abyss or the stock screensaver from `abyss mode`. The menu entry `system.screensaver` is overridden in `~/.config/omarchy/extensions/omarchy-menu.jsonc` (between `abyss-menu-*` markers) to call the same launcher.
- `plugin/abyss-launch` is a copy of `omarchy-launch-screensaver` with font size and `--monitor` added; re-derive it if upstream changes. Windows must keep class/app-id `org.omarchy.screensaver`: `abyss run --screensaver` quits when that window loses focus, and `abyss wake` (SIGUSR1) is how the plugin ends it with the summary.
- `stay-awake` (`~/.local/state/omarchy/indicators/stay-awake`) suppresses the idle launch; remember this when a test "does nothing".
- The shell caches compiled QML: after editing `plugin/*.qml` (copy to `~/.config/omarchy/plugins/io.github.cesarfilho.abyss/`), `omarchy-shell shell rescanPlugins` is not enough, run `omarchy restart shell`. Property names like `data` and `implicitHeight` are read-only on QtQuick items.

## Testing by hand

Run the screensaver, then `grim -o DP-1 file.png` (or `grim file.png` for all outputs) and read the image. A leftover `abyss` process blocks the next launch (`abyss-launch` exits when it finds one): `pkill -x abyss` first. Delete `~/.local/state/abyss` after tests so the user starts at level 0.
