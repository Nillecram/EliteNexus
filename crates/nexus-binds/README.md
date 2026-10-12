# nexus-binds

Elite Dangerous key-bindings parser and analyser for Elite Nexus — a native replacement for
[EDRefCard](https://edrefcard.info).

* **Parse** `.binds` files (`Bindings::load` / `Bindings::parse`): preset name, version, keyboard
  layout, every action with primary / secondary / axis inputs (modifiers, hold, `DeviceIndex`,
  inversion, dead zone, toggle), plain settings, and the set of devices used.
* **Discover presets** (`find_presets`): newest file per preset, the active preset per category
  from `StartPreset.4.start` (General, Ship, SRV, On foot), and `load_effective` to merge the four
  category presets into the bindings the game actually uses (built-in presets are resolved from
  the game's `ControlSchemes` folder).
* **Catalogue** (`action_info`): friendly name, context (Ship, SRV, On foot, Galaxy map, …) and
  category (Flight rotation, Targeting, …) for all 440 known actions; unknown actions get a
  generated name and a context guessed from their prefix/suffix.
* **Analyse** (`analyze`): conflicts, unbound essentials, keys used both as modifier and alone,
  and per-device counts.
* **Render** (`render_html`): a standalone, printable, dark reference card (inline CSS, no
  external resources) grouped by context/category and by device, with conflicts highlighted.

```rust
use nexus_binds::{analyze, default_bindings_dir, find_presets, render_html};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let presets = find_presets(default_bindings_dir().ok_or("LOCALAPPDATA not set")?)?;
    let bindings = presets.primary().ok_or("no presets")?.load()?;
    let report = analyze(&bindings);
    println!("{} conflicts", report.conflicts.len());
    std::fs::write("refcard.html", render_html(&bindings, &report))?;
    Ok(())
}
```

## Conflict rules

Two actions conflict when the same input — device, device index, key (including the `Pos_`/`Neg_`
half of an axis), the *set* of modifiers and the hold flag — is bound to both **and** both are live
at the same time:

1. Each action has a *scope* (the mode in which it is live). Only actions with the same scope are
   compared, so UI navigation can share WASD with flight, and galaxy map, FSS, DSS, camera,
   Holo-Me, store and settlement-placement controls can reuse flight keys; SRV and on-foot
   bindings never clash with ship bindings.
2. Overlays that *replace* the controls they share inputs with get their own scope: landing
   overrides, alternate flight axes, headlook, SRV turret, on-foot item wheel.
3. General actions (game menu, push-to-talk, HMD reset, Galnet audio) are live everywhere and are
   compared with ship, SRV, on-foot and multicrew actions.
4. Pairs that Frontier's own presets deliberately share are accepted (SRV steering vs. roll,
   SRV secondary fire vs. target ahead, subsystem cycling vs. colonisation module, on-foot
   interact / reload / tool mode / mission help / conflict-zone menu, crouch vs. secondary
   interact, item wheel vs. emote wheel, multicrew utility fire vs. UI cycling).
5. Legacy actions the game no longer reads, and an action sharing an input with itself, are
   ignored. Repeated elements in a file (some built-in presets have them) collapse to the last.

The rules were calibrated on the 30 presets shipped with the game: the only conflicts left are
genuine double bindings Frontier shipped (e.g. `T16000M Btn 16` → Deploy Heat Sink *and* Open
System Map).

Keys used both as a modifier and on their own in the same scope are listed as informational
*modifier notes* (Frontier's gamepad presets do this a lot).

## Essentials

`analysis::REQUIREMENTS` is a curated checklist; each item is satisfied by any one alternative
(e.g. *Frame Shift Drive*: `HyperSuperCombination`, or both `Supercruise` and `Hyperspace`;
*Pitch*: the axis, both buttons, or a mouse axis set to pitch). **Essential** items cover flight
(pitch/roll/yaw, throttle, vertical thrust, boost), FSD, next route system, landing gear, cargo
scoop, hardpoints, primary/secondary fire, target ahead, left/right panels (or UI focus), galaxy
and system map, FSS enter/leave, HUD mode, UI up/down/left/right/select/back and panel tabs, SRV
steering/speed/recall ship/systems panel, and on-foot movement/look/interact/fire. Everything else
on the list is **Recommended**.

## Smoke test

```text
cargo run -p nexus-binds --example inspect [-- [BINDINGS_DIR] [--game CONTROL_SCHEMES_DIR] [--out FILE.html] [--builtin]]
```

Prints the presets, the analysis of the active Ship preset and of the effective per-category
merge, and writes `refcard.html` / `refcard-effective.html` to `$CARGO_TARGET_DIR` (or
`target-agents/binds`). `--builtin` also analyses every preset shipped with the game.

## NOTICE

The action catalogue's groupings and several friendly names were cross-checked against and partly
derived from EDRefCard's `www/scripts/bindingsData.py`
(<https://github.com/richardbuckle/EDRefCard>), which is distributed under the MIT licence:

```text
MIT License

Copyright (c) 2017 Richard Buckle

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Action identifiers, the current action list and the key/device names come from the game's own
binding files and `ControlSchemes\Help.txt` / `DeviceMappings.xml`.
