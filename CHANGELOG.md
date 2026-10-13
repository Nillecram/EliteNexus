# Changelog

What changed in each release, newest first. Add user-visible changes under "Unreleased" as you make them; at
release time that section becomes the new version and the release workflow publishes it as the release notes.

## Unreleased

### Added
- **HUD & cockpit** (System), tab **HUD colours**: recolour or dim the HUD with the game's own colour setting. Start from the standard
  orange or a community theme (credited to its author), turn the colour wheel, lower saturation or brightness, or set
  the nine matrix numbers; a live cockpit preview shows the result, the swatches show what hostile red, friendly
  green and shield blue become, and the page warns when friend and foe would look alike or the HUD gets too dark.
  Applying edits only the colour lines of `GraphicsConfigurationOverride.xml`, keeps everything else in it, never
  touches a malformed file, and copies your original aside first. Share themes as the `<MatrixRed>` lines forums use,
  and save your own. The page explains what the setting can't do (cockpit lights, single elements) and warns when
  EDHM is installed, since EDHM needs it left at the standard colours.
- **HUD & cockpit › Glare and exposure**: Odyssey's auto-exposure darkens everything outside when the cockpit lights
  fill the view, and it can't be switched off. This tab sets what players change instead, in the same override
  file: glare from lights (`GlareCompensation`), the standard ACES tone curve (the page shows how much brighter darks
  and mid-tones get) and the auto-exposure targets, with presets for the game's values, less glare, and CMDR
  Morbad's 2021 Odyssey settings. The game's own values are read from its install.
- **HUD & cockpit › Cockpit lights**: dim the cockpit's own lights, which the game has no setting for. Elite Nexus
  installs 3Dmigoto (downloaded from its official release and checked against pinned checksums) with a pattern
  that adjusts the game's light shader as it loads, so lights within a reach of your eyes keep only part of their
  light; stations, carriers, planets and the HUD are untouched. Set the light kept, the reach and on/off in the
  tab or with F9, F10 and F11 in the game (the tab warns when your controls use one of those keys too); Remove
  takes out exactly what was installed, and Delete them clears frame captures and exported shaders left over from
  setting 3Dmigoto up. The game's code is never copied, so most game updates don't break it. Leaves EDHM and other
  3Dmigoto mods alone.

## [0.2.2] - 2026-10-12

### Added
- **Exploration › Find valuable bodies**: Spansh's Road to Riches, moved here from Navigation. Spansh skips
  populated systems, so a search from the bubble used to come back empty; it now widens itself (100, 250, 500,
  then 1,000 ly) until it finds something and says so. Pick Earth-likes, ammonia worlds or rocky and metal-rich
  worlds, how far, how many stops, and whether you'll map planets.
- **Exploration › History**: every system you scanned and what became of the data: sold (and when), unsold, not
  sellable, or lost with your ship. Data sales are recorded.
- **Exobiology › Find planets** with two strategies side by side. **Bonus hunt** looks for planets with no record of
  anyone sampling them and spells out what it found: how many planets suit the species, how many already have
  samples on record, were mapped without the genus, are in populated systems or were landed on. Filters for
  evidence, chance, distance from the star and gravity apply instantly, and **Tour the best 15** plans a route
  through them. **Sure value** (moved from Navigation) lists planets where commanders already logged valuable
  species: certain, but expect the base value.

### Changed
- Navigation keeps the travel planners (neutron, exact, fleet carrier) and links to the pages that plan scan
  tours; every tour, and every route the assistant plans, shows up there.
- "Mapped, no samples" planets (formerly "Mapped, not sampled") are hidden by default, with the reason: a sample
  only leaves a record when it adds a codex entry for that commander.
- Limits follow what the services really allow: Spansh planners take jump ranges of 10-100 ly, a radius up to
  1,000 ly and up to 2,000 stops; market searches reach the whole galaxy; trade routes cover up to 1,000 ly.
- A system you haven't scanned, or a populated one, explains itself on the Exploration page instead of showing
  zeros.
- The assistant's money routes widen their search the same way and say that exobiology values are base values.

### Fixed
- A planned route that starts and ends at your current system (every Road to Riches tour) counted as finished the
  moment it was planned; stops now tick off as you arrive.

## [0.2.1] - 2026-10-12

### Added
- **Exobiology › Find untouched planets**: planets where a valuable species should grow (Stratum Tectonicas by
  default) that no commander sharing data has mapped or sampled, so the first-logged bonus (×5) is likely still
  there. The assistant got the matching `untouched_biology` tool (15 tools).

### Changed
- Accounts & sync explains that Inara accepts only apps it has approved by name, with a link to ask for approval.

### Fixed
- Settings › Text and controls size: the slider ran away to 0.75× or 1.75× while dragged; it now comes in fixed
  steps (90-175 %), and Ctrl + / Ctrl − / Ctrl 0 change it too.
- Long values such as folder paths no longer squeeze a settings row's title to one letter per line.
- The release zip includes the Inter font licence.

## [0.2.0] - 2026-10-12

### Added
- **Market › Abnormal prices**: stations and fleet carriers paying far above (or selling far below) the galactic
  average, 1.5× to 10×, across the whole galaxy or a radius.
- A real **galaxy map**: a painted Milky Way with your travels, route, ship and carrier, search, layers and "Plot
  route" to any system.
- Planets: a footfall-aware bodies table, and the assistant's `system_bodies` tool ("has anyone set foot on
  planet X?").
- Inara: the key check shows which commander it belongs to, and the profile is sent as soon as sync starts.

### Changed
- A redesign after macOS 26: tinted cards, grouped settings rows, segmented controls, a floating glass sidebar and
  a toolbar capsule; every page adapts to the window width.
- The interface is set in Inter, bundled with the app.

## [0.1.0] - 2026-10-12

First release: journal-driven dashboard, exploration, exobiology (sampling radar and species predictions), a
live EDDN market database (commodity search, sell my cargo, trade routes, services, hotspots, price alerts),
Spansh route planners, fleet, engineering, missions, mining, fleet carrier, colonisation, BGS, logbook,
statistics, bindings, opt-in EDDN / EDSM / Inara / EDAstro uploads, voice alerts, a compact HUD and the Ask
assistant (Gemini, Claude or a local model).
