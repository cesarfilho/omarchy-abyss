# Changelog

## 0.5.0

- Regiões oceânicas com correntes, marés, clima e condições do mar exibidas no HUD, com efeitos visuais e movimento do panorama.
- Progressão revisada para tempo ativo, missões diárias de duração e paletas cosméticas compradas com pontos.
- Painel do Omarchy reorganizado em cinco abas, com navegação por teclado para bestiário e troféus.
- Instalação e remoção respeitam diretórios XDG e permitem escolher o diretório dos executáveis.

## 0.4.1

- Security: `scripts/e2e-idle.sh` now stores its diagnostic screenshot in an owner-only temporary directory with private file permissions and removes it on exit.

## 0.4.0

- The screensaver opens with a "retomando a jornada" toast (level, points, depth, streak, trophies) and the farewell summary shows total trophies and creatures.
- Panel split into tabs: Resumo, Missões, Troféus, Ajustes.
- New panel controls: start the screensaver now, idle time before it starts (1 to 15 min), and clear statistics and achievements (asks for confirmation).
- New CLI: `abyss idle [SECONDS]` (`idle_secs` in `config.json`, read live by the plugin), `abyss start`, `abyss reset`.

## 0.3.1

- The repository root is now the Omarchy plugin (`manifest.json` and the QML at the root, launchers in `bin/`), so it can be added with `omarchy plugin add` and listed on the Omarchy marketplace; `omarchy plugin validate .` passes.
- The panel tells you when the `abyss` program is not installed yet.
- README rewritten for marketplace installs (install, what `abyss-setup enable` changes, removal), with animated GIFs and a panel screenshot; `scripts/make-gifs.py` re-renders them; `preview.png` for the listing.

## 0.3.0

- Fish swim in random directions: every school and loner has its own wandering route (periodic over 30 minutes, a pure function of the clock), followers trail their leader, and sprites tilt nose-up or nose-down with the heading.
- A shark patrols the open water (zones 0–3). Its body bends along the route it has swum and schools part around it and close again after it passes.
- A much richer seabed: kelp forests, seagrass, rocks, sea fans, anemones, starfish, urchins, sponges, tube worms, glowing sea pens, crabs; rippled sand with pebbles and caustics; a wavy surface; far schools.
- Landmarks on the sea floor: shipwreck, submarine, plane, ruins, anchor, treasure chest, amphorae, whale skeleton, hydrothermal vents and a broken ocean liner in the abyssal zone. Each is a bestiary discovery (20 entries now).
- `abyss snapshot --ppm FILE [--t SECONDS] [--size WxH] [--season ID] [--landmark ID]` renders a frame without a terminal.
- The stats panel shows the bestiary with 20 entries and picks up `stats.json` even when it did not exist at start-up.
- MIT licence, release PKGBUILD, CI badge.

## 0.2.0

First public release: procedural 2.5D ocean, XP, trophies, bestiary, daily quests, seasonal events, theme-aware palette, shell plugin, `abyss doctor`, `abyss-setup`, socket between monitors, power-aware rendering; a closed terminal no longer leaves `abyss` spinning at 100 % CPU.
