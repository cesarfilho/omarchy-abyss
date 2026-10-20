# Changelog

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
