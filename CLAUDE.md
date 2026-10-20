# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Abyss: a gamified deep-sea screensaver for Omarchy (Hyprland + Quickshell). The repository root is itself the Omarchy plugin (`manifest.json`, `BarWidget.qml`, `Service.qml`, `StatsContent.qml`: marketplace layout, `omarchy plugin validate .` must pass) and a Rust crate. A Rust binary (`abyss`) renders a procedural pixel-art ocean in a terminal, earns XP/trophies while the machine is idle, and a Quickshell plugin (the QML at the root) triggers it and shows the stats. User-facing text is Portuguese.

## Commands

```bash
cargo build --release
cargo test                       # unit tests in state, world, gfx, ipc, config, theme, power
cargo test streak_and_wilt       # single test by name
cargo clippy --all-targets --locked -- -D warnings   # CI gate
cargo test --release bench_frame -- --ignored --nocapture   # per-stage frame timings
./install.sh [--setup]           # build + install binaries/launchers/abyss-setup/plugin; --setup also wires Omarchy
./uninstall.sh [--purge]         # abyss-setup disable + remove files (+ data)
scripts/e2e-idle.sh [--real]     # real plugin -> launcher -> windows -> wake run on this session
abyss run [--screensaver] [--no-intro] [--monitor NAME]
abyss status [--json] | mode | quality | config | wake | stop | doctor [--fix] | snapshot
```

Env knobs: `ABYSS_XP_MULT` (faster XP), `ABYSS_SEASON=halloween|xmas|oceans`, `ABYSS_NO_INTRO`, `ABYSS_STATE_DIR`, `ABYSS_CONFIG_DIR`, `ABYSS_THEME_FILE`. Env vars do NOT reach windows started through `abyss-launch` (Hyprland exec), so test XP/state changes by running `foot --fullscreen -e env ABYSS_XP_MULT=… abyss run --no-intro` or by seeding `~/.local/state/abyss/save.json`. The user works on this machine while you test: any key press closes a running screensaver and `grim` then captures their desktop. Prefer `abyss snapshot ZONE REEF --ppm out.ppm --t EPOCH_SECS [--size WxH] [--season ID]` (renders one frame without a terminal; convert with `magick`) and use `grim` only when the real terminal path is the thing under test, never keeping or showing an image that is not the sea.

## Architecture

**Rendering (`gfx.rs`, `world.rs`, `sprites.rs`)**: the scene is drawn into a `Frame` of square pixels, 2 wide × 4 tall per terminal cell. `Presenter::present` resamples each cell to a 2×3 sextant (U+1FB00 block) and fits two colours per cell (`split_two`), writing only changed cells with truecolor escapes. HUD and toasts are terminal text via `Overlay` (cells), not frame pixels; text size therefore equals the cell size, which the `quality` setting (terminal font size, see `QUALITIES` in `main.rs`) controls. Creatures are procedural (`sprites.rs`), drawn from a length in screen pixels so detail scales with resolution; sizes derive from a height-based `refw`, never from the frame width.

**World is a pure function of the wall clock (`world.rs`, `path.rs`)**: fish swim along `path::Path` routes (a wandering heading, periodic over 30 min, integrated once into a table and read with a cubic, so no per-frame state; each school or loner has its own start heading, followers trail the leader by a time delay, the sprite flips on `vx` and pitches by vertical shear in `sprites::blit`); jellies use the same type. `shark.rs` is a visit per 240 s window (45 %, zones 0-3) with a `Path::calm` route, a body of circles laid along the route it has swum, and `SharkRun::push` shoving nearby fish away as a pure function of the clock (fish headings are taken from `fish_xy` at t±0.25 s so fleeing fish face away). Fish, jellies, bubbles, snow, whale and bloom positions come from fixed seeds plus `now_secs()`, with no per-frame state. Every monitor runs the same world and draws only its slice (`Layout { pano_w, off }`, computed in `app.rs::layout` from `hyprctl monitors -j`), so a fish leaving one screen appears on the next. Seabed content lives in `src/world/scenery.rs` (props per plane by zone weights, ground texture, caustics, surface, far schools, crabs) and `src/world/landmarks.rs` (`KINDS`: wreck, liner, sub, plane, ruins, anchor, treasure, amphorae, whale skeleton, vents, each also a `BESTIARY` entry; a test keeps zones in sync; `abyss snapshot --landmark ID` forces one). Both are child modules of `world` so they can use its private fields; use `floor_row(slot)` (once per plane and frame) rather than `floor_y` in per-column loops. Anything added to the world must stay clock-deterministic and use panorama coordinates (subtract `self.off` when drawing); `World::at(t)` freezes the clock for tests, and `world::tests::two_monitors_form_one_panorama` guards the seam. The population follows the save's target zone (`tz`), not the eased `zf`, so monitors repopulate together. `World::update` emits `WorldEvent::Spotted(id)` for creatures in view (bestiary) and seasons/palette are set with `set_season`/`set_palette`. The water pass is table-driven (see `draw_water`); keep per-pixel work out of hot loops, a frame must stay near 8 ms at 640x340. Depth is 2.5D: creatures have `z` (0 near .. 1 far), the seabed is 5 parallax planes (`PLANES`), and `World::draw` interleaves planes and creatures far to near.

**Game state (`state.rs`)**: `save.json` (XP, streak, reef, trophies, daily history) in `~/.local/state/abyss/`. Only one instance (the one that wins the `flock` on `lock`, which the kernel releases on any death) earns XP and writes. It also serves a Unix socket (`ipc.rs`, `$XDG_RUNTIME_DIR/abyss.sock`; inside the state dir when `ABYSS_STATE_DIR` is set); the other monitors subscribe and receive the full `Save` every 2 s (immediately on zone/reef change), every `GameEvent` as it happens, and `wake`/`stop`. `abyss wake|stop` talk to the same socket (SIGUSR1/SIGTERM only as a fallback). Each `Save::store` also writes `stats.json`, which the plugin watches. `Session` is what the exit summary shows.

Game content lives in `state.rs`: `TROPHIES` (22), `BESTIARY` (every `sprites::SPECIES` id plus jelly/whale/bloom; a test keeps them in sync), `QUESTS` (3 per day, rolled in `roll_quests`), `SEASONS`. `Save.schema` + `Save::migrate` version the file; an unreadable `save.json` is renamed `save.json.corrupt-<ts>`, never overwritten. A new persisted field needs `#[serde(default)]` coverage (the struct already has it) and, if old saves should get a value, a branch in `migrate`.

**Config** is `~/.config/abyss/config.json` (`config.rs`: mode, quality, fps, battery_fps, palette, seasons). The plugin's `BarWidget.qml` watches that file and the CLI writes it; legacy `mode`/`quality` files in the state dir are migrated once. **Other state files** in `~/.local/state/abyss/`: `last_session.json`, `lock`, `crash.log`; `upstream.sha256` (hash of the stock launcher `abyss-launch` was copied from) and `setup.json` (previous `idle.screensaver`) live in `~/.config/abyss/` so wiping the game data does not lose them.

**Power (`power.rs`)**: every 5 s the app reads DPMS state from `hyprctl monitors -j` (drawing pauses while the monitor sleeps) and battery status from sysfs (`battery_fps`). **Theme (`theme.rs`)**: `Palette` is built from `~/.local/state/omarchy/current/theme/colors.toml` and used by the world (water, plankton) and HUD/toasts.

**Terminal-hangup trap**: crossterm's `poll`/`read` spin forever (ignoring EIO) once the terminal has hung up, so a closed window left `abyss` at 100% CPU and blocked `abyss-launch`. `app::run` starts a watchdog thread that exits the process 1.5 s after the stop flag is set, and `cutscene::play` honours the flag and a 25 s cap. Keep both when touching the loop. A panic hook restores the terminal and writes `crash.log`.

**ttfx** (`cutscene.rs`): only used for the exit summary (`waves`) and the optional manual intro; it blanks the screen, so achievements use in-scene toasts (`toast.rs`) instead.

## Omarchy integration (non-obvious)

- Never edit `/usr/share/omarchy/` (package-owned). The stock idle service always calls `omarchy-launch-screensaver`, so this project sets `idle.screensaver` to `0` in `~/.config/omarchy/shell.json` and the plugin's `Service.qml` (an `IdleMonitor`) starts `abyss-screensaver`, which picks Abyss or the stock screensaver from `abyss mode`. The menu entry `system.screensaver` is overridden in `~/.config/omarchy/extensions/omarchy-menu.jsonc` (between `abyss-menu-*` markers) to call the same launcher. All of that wiring is `scripts/abyss-setup enable|disable` (idempotent, backs up, tested against a throw-away `HOME` in CI); edit that script rather than documenting manual steps.
- `bin/abyss-launch` is a copy of `omarchy-launch-screensaver` with font size and `--monitor` added. `abyss doctor` compares the stock script's hash with `upstream.sha256`; when it differs run `scripts/diff-launch.sh`, port the change, then `abyss-setup record-upstream`. Windows must keep class/app-id `org.omarchy.screensaver`: `abyss run --screensaver` quits when that window loses focus, and `abyss wake` (SIGUSR1) is how the plugin ends it with the summary.
- `stay-awake` (`~/.local/state/omarchy/indicators/stay-awake`) suppresses the idle launch; remember this when a test "does nothing".
- The shell caches compiled QML: after editing the plugin's `*.qml` (repository root) (copy to `~/.config/omarchy/plugins/io.github.cesarfilho.abyss/`), `omarchy-shell shell rescanPlugins` is not enough, run `omarchy restart shell`. Property names like `data` and `implicitHeight` are read-only on QtQuick items.

## Testing by hand

Run the screensaver, then `grim -o DP-1 file.png` (or `grim file.png` for all outputs) and read the image. A leftover `abyss` process blocks the next launch (`abyss-launch` exits when it finds one): `abyss stop`, then `pkill -x abyss`; `abyss doctor --fix` also removes a stale socket and un-hides the pointer. Delete `~/.local/state/abyss` after tests so the user starts at level 0, and leave `~/.config/abyss/config.json` at its defaults (`abyss quality ultra`, fps 30/15).

Packaging: `packaging/PKGBUILD` (release: builds the GitHub tag tarball, `sha256sums` updated after each tag) and `packaging/PKGBUILD.dev` (builds the checkout); licence is MIT (`LICENSE`) and `.github/workflows/ci.yml` (clippy `-D warnings`, tests, shellcheck, an `abyss-setup` round trip). The QML plugin is copied by `abyss-setup plugin` from `~/.local/share/abyss/plugin` (which `install.sh` refreshes from the repository root), so after editing the plugin's `*.qml` (repository root) run `./install.sh` and `omarchy restart shell`, not just `abyss-setup plugin`. The stats FileView cannot watch a file that does not exist yet, hence the polling Timer in `BarWidget.qml`.
