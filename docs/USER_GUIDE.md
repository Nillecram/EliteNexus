# Elite Nexus — user guide

Start `Start-EliteNexus.cmd` (or `dist\EliteNexus\EliteNexus.exe`). It finds your journal folder automatically
(Saved Games\Frontier Developments\Elite Dangerous), reads your whole history in well under a second, then follows
the game live. Run it before or after the game — order doesn't matter.

## The window

The **sidebar** on the left lists every page; on narrow windows it shrinks to icons (hover for names). At its foot
is your commander: name, current system and a fuel gauge (hover for ship and situation, click for the Dashboard).

Each page has a large title, and the **toolbar** at the top right shows, left to right: your credits, the live
market feed (**EDDN n/min**: click it for the market database), uploads sent this session (once you enable a service;
amber = a service is paused or rejecting, click for details), **Pause EDDN / Resume EDDN** when you share with EDDN
(market prices are only sent when you undock, so pausing while docked keeps a market you just found to yourself), and
**Live / Offline** (Live = the game is writing events). The round button next to it turns the window into a small
always-on-top **HUD** for a second monitor or the edge of your screen (location, fuel, next star and whether it's
scoopable, exobiology sampling radar, system value, last alerts). "Full view" switches back.

Alerts appear as banners at the top right and are spoken if you enable voice.

## Simple first, everything still there

Each page shows the one thing you usually need, with smart defaults (your current system, your ship's jump range and
landing pad). Every advanced setting the original tools offer, such as Spansh's routing algorithm, neutron/injection
choices or market filters, is one click away under **› Options**.

## Pages

| Page | What it's for | Tips |
|---|---|---|
| Ask | Your AI co-pilot: ask anything in plain words ("Where can I make the most money in the next hour?", "Where should I sell my cargo?", "Plan an exobiology trip near me") | It answers from your ship, cargo, journal, the live market database and Spansh, plus a built-in knowledge base of game mechanics and this guide. One-time setup: click "Get a free Gemini key", paste it, Connect. Routes it plans appear on the Navigation page. Claude or a local model (Ollama, LM Studio) can be chosen under "Other providers and model". |
| Dashboard | Everything at a glance | Session card shows earnings since you logged in; transfers to your own carrier are not counted as spending. |
| Navigation | Your plotted route: scoopable stars, neutron/white-dwarf hazards, longest non-scoopable streak | With "copy next system" on, the next waypoint is on your clipboard after every jump — paste it in the galaxy map. Spansh planners feed the same view. |
| Exploration | The current system as a tree (stars → planets → moons) with value now vs. value if mapped | Click a body for materials, rings/hotspots, biology, discovery & footfall status. "Not sellable" lists data UC refused in earlier sales. To check whether anyone has set foot on a planet in another system, ask the assistant ("has anyone landed on planet X in system Y?"). |
| Exobiology | Sampling radar, bio bodies, unsold Vista Genomics data | The radar's "up" is the way you're facing; each previous sample shows its colony-range circle. Green = far enough. First-logged bonus (×5) is shown when the body had no footfall, the system is uninhabited and the species wasn't logged. |
| Galaxy map | A map of the Milky Way with everywhere you've been, your travel path, the active route, your carrier, parked ships and landmarks (Sol, Colonia, Sag A*, Beagle Point…) | Drag to pan, scroll or pinch to zoom, double-click to zoom in. The buttons top right: zoom, **where you are**, the **whole galaxy**, **all your travels**, and **layers** (what to show). Search finds any system you've visited or a landmark. Click a system for its distance from you, **Copy name**, and **Plot route** (opens Navigation with it as the destination). |
| Market | Local market database fed by EDDN, your journal and Spansh | **Abnormal prices** scans the whole database for stations and fleet carriers paying far more than a commodity's galactic average (sell there) or selling far below it (buy there), e.g. a carrier buy order paying 100× the normal price. Pick the multiple (1.5× to 10×), the whole galaxy or a radius, and how old prices may be; it updates as you change them. Carriers report no average, so they are compared with what stations report for that commodity. **Find commodity** (best price or nearest, pad, age, stock), **Sell my cargo** (best single stop for your whole hold, re-planned after every jump), trade routes for your ship and credits, nearest material traders / brokers / Interstellar Factors / Vista Genomics, mining hotspots + best sell price, **price alerts** ("tell me when Tritium sells for 60,000+ within 50 ly", spoken seconds after any commander docks there), the station you're docked at. **Database & feed** shows the live feed and offers the one-time Spansh import (4.5 GB streamed, nothing saved but the database) so every station is known from day one. |
| Mining | Live session stats | Tonnes/hour, prospect quality, motherlodes. |
| Colonisation | Construction depots | Remaining tonnage per commodity, how much of each you already carry, and **Where to buy**: the nearest stations with enough stock for a full hold of each commodity, around the construction site. |
| Fleet carrier | Tritium, finances, capacity, crew, orders, jumps | Countdown to scheduled jumps. |
| Engineering | Blueprint planner, materials, engineers, suits & weapons, synthesis | Build a wishlist (or quick-add "upgrade to G5" from your ship); the shopping list adds everything up and the trader plan tells you exactly what to trade at which material trader. Engineers are sorted unlocked-first, then by distance. |
| Fleet | All ships, location (incl. in transit), loadouts & engineering | **EDSY / Coriolis** buttons open the exact build. |
| Missions | Active missions, massacre stacks | A kill counts once for every issuing faction — the stack view shows true kills remaining. |
| BGS & Powerplay | What you did for which faction | Pick "since last tick" and set the tick hour you see in your group; carriers are excluded (outside the BGS). |
| Logbook | Activity, every journal event (with JSON + full-text search), travel, docking, credit ledger | Search finds a system/station/CMDR name anywhere in your history. |
| Statistics | Credits over time, income/spend by category, jumps per day, in-game stats | |
| Bindings | Your active presets per category, conflicts, missing essentials, searchable list | "Printable reference card" writes an HTML card and opens it. |
| Accounts & sync | EDDN / EDSM / Inara / EDAstro | All opt-in. With an Inara key, the Inara section confirms who the key belongs to ("Connected as CMDR …", with a link to your profile) and sends your current profile as soon as sync starts. Keys are encrypted with Windows DPAPI. History is never uploaded. Each card shows sent / queued / failed counts and why a service is paused (bad key, rate limit). Unsent items survive restarts (`nexus-data\uplink\`); switching a service off discards its queue. For the first EDDN session you can tick "Test mode" to send to EDDN's test schemas only. |
| Settings | Journal folder, voice, toasts, thresholds, UI scale | "Test voice" checks text-to-speech. |

## Voice

Alerts are spoken through Windows speech. Choose the minimum priority in Settings:
* **Critical** — hull critical, ship destroyed.
* **High** — Earth-like/water/ammonia/terraformable/high-value bodies, motherlodes, analysis complete, massacre
  mission done, fuel low with a non-scoopable star next, interdiction.
* **Normal** — biological signals, sample 1 logged (+ distance to move), codex entries, carrier jump scheduled.
* **Low** — arrival announcements, discovery-scan body counts, system fully scanned.

## Data & privacy

Everything lives in `nexus-data\` next to the executable (settings, galaxy database, logs). Nothing is uploaded
unless you enable it in Accounts & sync; when you do, only live game data is sent (never your history, credits or
cargo), following each service's published rules.
