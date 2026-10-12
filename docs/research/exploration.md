# Exploration & Exobiology Research — Elite Nexus

Research date: 2026-10-12. Scope: cartographic (Universal Cartographics) value estimation, "interesting body" alerts,
exobiology prediction and sampling assistance, and FSD-synthesis ("jumponium") material flags.

Companion data files (generated from the same research, see §2 for semantics):

- `data/exobiology/species.json` — 126 entries (91 Odyssey species incl. Radicoida, 27 Horizons-era surface organics, 8 Thargoid xeno samples).
- `data/exobiology/genera.json` — 22 genera + the Thargoid sample group → colony range, codex ids/prefix, colour basis, journal quirks, plus non-sellable codex life groups. Units/conventions are in its `meta` block.

## 0. Method, certainty legend, licensing

**Method.** Source code was read verbatim from raw.githubusercontent.com (no summaries) and numbers were cross-checked
between independent implementations; live data was pulled from the Canonn codex reference API on 2026-10-12. Where two
tools disagree, both values are recorded.

**Certainty legend** (used in this document and in the JSON `certainty` fields):

| Level | Meaning |
|---|---|
| HIGH | Two or more independent sources agree exactly (e.g. tool code + live Canonn data), or the formula reproduces published reference numbers. |
| MEDIUM | Sources agree in substance but differ in detail (thresholds, edge cases), or one strong source plus indirect confirmation. |
| LOW | Single source, inferred, or contested. |

**Sources read** (details and URLs in §6): EDDiscovery `EliteDangerousCore` (Apache-2.0), Observatory Core incl. Explorer
and Botanist plugins (MIT), EDMC-BioScan + EDMC-ExploData + EDMC-Pioneer (GPL-2.0), SrvSurvey (GPL-3.0), EDMC-Canonn (GPL-3.0),
UnixplorationBuddy (MIT, a Rust port of the value formulas), MattG's forum thread "Exploration value formulae" (first post,
last updated Sep 2022, read via the Internet Archive), Canonn codex reference API + Canonn codex pages, Frontier Update 14.01
notes, jixxed `ed-journal-schemas`, Elite Dangerous Fandom wiki, Observatory documentation site, panostrede.de (Exploration Buddy).

**Licensing note.** No third-party source files were copied into the project. GPL code was only read for facts
(constants, thresholds, rule tables). `species.json` is a re-derived factual compilation (numbers/rules) with per-entry
source attribution; Canonn-derived statistics underlie both BioScan and SrvSurvey. Before redistributing the dataset,
keep the attribution (`sources` fields) and do not paste wiki prose (Fandom is CC BY-SA).

---

## 1. Cartographic (exploration data) value

### 1.1 Sources compared

| Source | What it contains | Agreement |
|---|---|---|
| MattG, "Exploration value formulae" (Frontier forum thread 232000, first post updated Sep 2022) | Reference formula + C# snippet + median-value table | Reference |
| EDDiscovery `EstimatedValues.cs` (`ScanEstimatedValues`) | Full 3.3+ implementation, plus 3.2 and pre-2.2 legacy formulas | Same constants; small-body ordering differs (§1.4) |
| EDMC-Pioneer `body_calc.py` / `load.py` | MattG formula + honk, full-scan, full-map bonuses, terraform min/max | Same |
| SrvSurvey `Util.cs` / `SystemData.cs` | MattG formula, honk bonus, 10k full-map bonus | Same (star k lookup has bugs, §1.2) |
| UnixplorationBuddy `valuation.rs` (MIT, Rust) | Port of the same formula | Same constants, but clamps mass to ≥ 1.0 (EM and M☉), applies the 500 minimum before mapping and always applies the Odyssey bonus — over-values bodies below 1 EM; do not copy |

Running the formula below reproduces every number in MattG's own median table exactly (e.g. terraformable water world
4,198,704; class II gas giant 443,997; rocky body 6,064) — HIGH certainty for the core formula.

### 1.2 Stars

```
V_star = k + M_sol * k / 66.25          (M_sol = Scan.StellarMass)
V_star_first_discovered = V_star * 2.6
```

| StarType (journal) | k | Certainty |
|---|---:|---|
| White dwarfs: `D`, `DA`, `DAB`, `DAO`, `DAZ`, `DAV`, `DB`, `DBZ`, `DBV`, `DO`, `DOV`, `DQ`, `DC`, `DCV`, `DX` | 14,057 | HIGH |
| Neutron star `N`, black hole `H` | 22,628 | HIGH |
| Every other star type (O…M, L/T/Y brown dwarfs, `TTS`, `AeBe`, Wolf-Rayet `W*`, carbon `C*`, `MS`, `S`, giants/supergiants) | 1,200 | HIGH |
| `SupermassiveBlackHole` | no agreed value: EDDiscovery k = 33.5678 ("same scaling as 3.2, not confirmed"); Pioneer uses a flat 261,790 Cr and no honk bonus | LOW |

Stars cannot be DSS-mapped, so only the first-discovery multiplier (×2.6) applies. Pitfall: SrvSurvey's `getStarKValue`
tests `"NS"`/`"BH"` and `'W'` (Wolf-Rayet) instead of `N`/`H`/`D*`, so it misprices remnants — do not copy that lookup.

### 1.3 Planets and moons — base value

```
B = k + k * q * M_earth^0.2             q = 0.56591828,  M_earth = Scan.MassEM
```

| PlanetClass | k (base) | + k if terraformable | Notes |
|---|---:|---:|---|
| `Metal rich body` | 21,790 | (MattG: +105,678 "needs verification"; EDDiscovery: +65,631) | Terraformable MR is essentially non-existent; LOW |
| `Ammonia world` | 96,932 | – | HIGH |
| `Sudarsky class I gas giant` | 1,656 | – | HIGH |
| `Sudarsky class II gas giant` | 9,654 | – | HIGH |
| `High metal content body` | 9,654 | +100,677 | HIGH |
| `Water world` | 64,831 | +116,295 | HIGH |
| `Earthlike body` | 64,831 + 116,295 = 181,126 | (always included) | HIGH |
| everything else: `Rocky body`, `Icy body`, `Rocky ice body`, class III/IV/V gas giants, helium / helium-rich gas giants, gas giants with water/ammonia-based life, water giants | 300 | +93,328 | HIGH |

"Terraformable" = `Scan.TerraformState == "Terraformable"` (EDDiscovery/SrvSurvey; EDMC-Canonn's high-value check also
accepts `"Terraforming"`). MattG notes the terraforming component is really a 0–100 % scale: the body always gets 100 %
of the base-k value and between 0 % and 100 % of the terraformable-k value; tools show the maximum (`k = base + bonus`).
Pioneer additionally shows a minimum estimate using factors 0.9 (HMC / other) and 0.75 (water world) on the
terraformable component; for an ELW that is itself flagged `Terraformable` (a terraformed-type ELW) its minimum excludes the
bonus, with the comment that natural ELWs get the full bonus. MEDIUM.

### 1.4 Multipliers and exact order of operations

MattG's reference function (SrvSurvey `Util.GetBodyValue` is a line-for-line copy; Pioneer is equivalent — it multiplies the
efficiency bonus in later, which gives the same result because a mapped value is always ≥ 555):

```
mult = 1
if mapped:                                   # you completed a DSS map (SAAScanComplete)
    if FD and FM: mult = 3.699622554         # first discoverer AND first mapper
    elif FM:      mult = 8.0956              # first mapper only
    else:         mult = 3.3333333333        # body already mapped by someone else
v = B * mult
if mapped:
    if odyssey:   v += max(v * 0.3, 555)     # Odyssey mapping bonus (+30 %, min 555 Cr)
    if efficient: v *= 1.25                  # efficiency bonus
v = max(500, v)                              # minimum value
if FD:            v *= 2.6                   # first discovery
if carrier_sale:  v *= 0.75                  # sold at a fleet-carrier Universal Cartographics
return round(v)
```

| Input | Journal source | Certainty |
|---|---|---|
| `FD` (you are first discoverer) | `Scan.WasDiscovered == false` | HIGH |
| `FM` (you are first mapper) | `Scan.WasMapped == false` | HIGH |
| `mapped` | `SAAScanComplete` for that `BodyID` | HIGH |
| `efficient` | `SAAScanComplete.ProbesUsed <= SAAScanComplete.EfficiencyTarget` (ExploData, SrvSurvey). The target itself is supplied by the game per body; no formula is needed. | HIGH |
| `odyssey` | `LoadGame.Odyssey == true` **or** game version ≥ 4.0 (MattG Sep 2022: Horizons 4.0 Live client also gets it; EDDiscovery `IsOdysseyEstimatedValues`, Pioneer `game_version.major >= 4`) | HIGH |
| Odyssey bonus suppressed | when `WasDiscovered == false && WasMapped == true` (old "bubble" bodies): FD ×2.6 still applies, +30 % does not (MattG "limited testing"; Pioneer implements) | MEDIUM |
| `carrier_sale` | Universal Cartographics on a fleet carrier pays 75 % (MattG); Pioneer shows the 12.5 % carrier-owner share | MEDIUM |

Effective multipliers on `B` for a large Odyssey body mapped efficiently: FD+FM = 3.699622554 × 1.3 × 1.25 × 2.6 ≈ **15.63**;
FM only = 8.0956 × 1.625 ≈ **13.16**; already mapped = 3.3333 × 1.625 ≈ **5.42**; FD without mapping = **2.6**.

**EDDiscovery deviation (MEDIUM, avoid; UnixplorationBuddy does the same):** EDDiscovery clamps `B` to ≥ 500 *before* the mapping multiplier
(`PlanetValue33 = max(k + k*m^0.2*q, 500)`), so small rocks/ices are over-estimated once mapped (rocky body 0.003359 EM,
FD+FM efficient: EDDiscovery 7,815 vs MattG 6,064). MattG's own table matches the "clamp after mapping" order.

Other observations from MattG that tools do **not** model (LOW): re-mapping a body whose unmapped data you sold before 3.3
gave an extra ×1.23524 (not FD) / ×1.89404 (FD).

### 1.5 System-level bonuses (honk, full scan, full map)

| Bonus | Rule | Certainty |
|---|---|---|
| FSS "honk" bonus (credited on the main star) | Σ over all other bodies. Planet: `max(500, B/3) × (2.6 if undiscovered)`. Star: `V_star/3 × (2.6 if undiscovered)` (MattG, Pioneer). SrvSurvey uses `0.3 × V_star` for stars and assumes 500 Cr for bodies not yet scanned. | MEDIUM |
| FSS complete (`FSSAllBodiesFound`) | +1,000 Cr per body (MattG). Pioneer pays it only in a system nobody had discovered and counts `FSSDiscoveryScan.BodyCount` + belt clusters (requires all belt clusters scanned); SrvSurvey has this bonus commented out as unconfirmed. | MEDIUM/LOW |
| All mappable bodies DSS-mapped | +10,000 Cr per mappable (planet) body (MattG). Pioneer: only if no planet was previously mapped; SrvSurvey: when DSS count == number of landable/solid/giant bodies. | MEDIUM |

### 1.6 First footfall

- No tool (EDDiscovery, Pioneer, SrvSurvey, MattG) applies any cartographic multiplier for first footfall; forum
  consensus is that First Footfall gives a tag, not credits. MEDIUM.
- `Scan.WasFootfalled` (bool) tells whether anyone has set foot on the body; EDDiscovery marks it "new 4.2.1" (game
  version), and it is present in current journal schemas. Before it existed, tools had to infer footfall.
- Its real impact is **exobiology**: tools treat a body with `WasFootfalled == false` in an unpopulated system as eligible for
  the Vista Genomics first-logged bonus (§2.2). SrvSurvey notes first footfall "is not possible in systems with any
  population, but WasFootfalled will still be false".

### 1.7 Journal plumbing

| Event | Fields used |
|---|---|
| `Scan` | `StarType`/`PlanetClass`, `StellarMass`/`MassEM`, `TerraformState`, `WasDiscovered`, `WasMapped`, `WasFootfalled`, `Landable`, `DistanceFromArrivalLS`, `Atmosphere`, `AtmosphereType`, `AtmosphereComposition`, `Volcanism`, `SurfaceGravity` (m/s²), `SurfaceTemperature` (K), `SurfacePressure` (Pa), `Materials`, `Rings`, `Parents`, `SemiMajorAxis` (m), `Eccentricity`, `OrbitalInclination`, `OrbitalPeriod` (s), `RotationPeriod` (s), `TidalLock`, `Radius` (m), `AxialTilt` (rad), `Subclass`, `Luminosity` |
| `FSSDiscoveryScan` | `BodyCount`, `NonBodyCount`, `Progress` |
| `FSSAllBodiesFound` | `Count` |
| `FSSBodySignals` | `Signals[].Type` (`$SAA_SignalType_Biological;`, `…Geological;`), `Count` |
| `SAAScanComplete` | `ProbesUsed`, `EfficiencyTarget`, `BodyID` |
| `SAASignalsFound` | `Signals[]`, `Genuses[].Genus` (exact genera after DSS) |
| `SellExplorationData` / `MultiSellExplorationData` | `BaseValue`, `Bonus` (first-discovery and similar), `TotalEarnings` |

Unsold exploration and organic data are lost on death (BioScan computes unsold value from the last sale/death/resurrect).

### 1.8 Worked examples (Odyssey client, values in Cr, computed with §1.4)

ELW, 1.0 EM: `k = 181,126`; `B = 181,126 × (1 + 0.56591828 × 1.0^0.2) = 283,628.5`.
FD+FM+efficient: `283,628.5 × 3.699622554 = 1,049,318.4` → `+30 % = 1,364,113.9` → `×1.25 = 1,705,142.4` → `×2.6 = 4,433,370`.

| Body | Mass (EM) | Situation | MattG formula | EDDiscovery |
|---|---:|---|---:|---:|
| Earth-like world | 1.0 | discovered by others, not mapped | 283,629 | 283,628 |
| Earth-like world | 1.0 | first discovered, not mapped | 737,434 | 737,434 |
| Earth-like world | 1.0 | FD + FM, efficient | **4,433,370** | 4,433,370 |
| Earth-like world | 1.0 | FD + FM, not efficient | 3,546,696 | 3,546,696 |
| Earth-like world | 1.0 | FM only (discovered by others), efficient | 3,731,232 | 3,731,232 |
| Earth-like world | 1.0 | already discovered & mapped, you map efficiently | 1,536,321 | 1,536,321 |
| Earth-like world | 1.0 | FD + FM, efficient, Legacy/pre-Odyssey client (no +30 %) | 3,410,285 | 3,410,284 |
| Earth-like world | 1.0 | FD + FM, efficient, sold on a fleet carrier | 3,325,028 | – |
| Terraformable water world | 0.453011 | FD + FM, efficient | 4,198,704 | 4,198,704 |
| Water world | 0.780638 | FD + FM, efficient | 1,559,138 | 1,559,138 |
| Ammonia world | 0.43914 | FD + FM, efficient | 2,242,455 | 2,242,455 |
| Terraformable HMC | 0.466929 | FD + FM, efficient | 2,562,654 | 2,562,653 |
| High metal content | 0.344919 | FD + FM, efficient | 219,923 | 219,922 |
| Class II gas giant | 476.2409 | FD + FM, efficient | 443,997 | 443,997 |
| Class I gas giant | 69.55164 | first discovered, not mapped | 9,997 | 9,997 |
| Rocky body | 0.003359 | first discovered, not mapped | 1,300 | 1,300 |
| Rocky body | 0.003359 | FD + FM, efficient | 6,064 | 7,815 |
| Rocky body | 0.003359 | already mapped by others, you map efficiently | 2,170 | 2,777 |
| Icy body | 0.01854 | FD + FM, efficient | 6,330 | 7,815 |

| Star | Mass (M☉) | Base | First discovered |
|---|---:|---:|---:|
| K main sequence | 0.8 | 1,214 | 3,158 |
| G main sequence | 1.0 | 1,218 | 3,167 |
| Neutron star | 1.4 | 23,106 | 60,076 |
| White dwarf | 0.6 | 14,184 | 36,879 |
| Black hole | 8.0 | 25,360 | 65,937 |

Honk example (all undiscovered): main star K 0.8 M☉ + ELW 1.0 EM + HMC 0.3449 EM + rocky 0.003359 EM + icy 0.01854 EM +
secondary M star 0.4 M☉. Honk contributions: ELW 245,811; HMC 12,194; rocky 1,300; icy 1,300; M star 1,046 (SrvSurvey's 0.3×
rule would give 942) → main star shows 3,158 + 261,651 = 264,809. FSS completion adds 6 × 1,000 = 6,000; mapping all four
planets adds 4 × 10,000 = 40,000.

### 1.9 Legacy formulas (only for re-processing old journals; from EDDiscovery)

- **2.2 ≤ date < 3.3** (2017-04-11 → 2018-12-11): stars `k + m·k/66.25` with k = 2,880 (normal), 33,737 (WD), 54,309 (NS/BH),
  80.5654 (SMBH). Planets `k + 3k·M^0.199977/5.3` with k = MR 52,292; HMC/GG2 23,168; ELW/WW 155,581; AW 232,619;
  GG1 3,974; other 720; terraformable bonus computed with the same formula using k = MR 245,306, HMC 241,607,
  ELW/WW 279,088, other 223,971 (ELWs always get it). Values are base only (no FD/mapping logic).
- **before 2.2**: a fixed per-class table (EDDiscovery `EstimatedValueED22`).

### 1.10 Implementation notes for Elite Nexus

- Store per body: `B`, `FD`, `FM`, `mapped`, `efficient`, `odyssey`, `bubble_rule`; compute the eight EDDiscovery-style
  variants (base, FD, FD+FM, FD+FM+eff, FM, FM+eff, mapped, mapped+eff) from the single §1.4 function.
- "Worth mapping" alerts: use the FD/FM-aware potential value (Evaluator-style thresholds, §3.4) rather than planet class only.
- Keep the `max(500, …)` after mapping (MattG order) — it matters only for small rocks/ices but those are the most common bodies.


---

## 2. Exobiology

### 2.1 Scope

| Group | Count | Vista Genomics sellable | Notes |
|---|---:|---|---|
| Odyssey genera (15) | 90 species | yes | Aleoida 5, Bacterium 13, Cactoida 5, Clypeus 3, Concha 4, Electricae 2, Fonticulua 6, Frutexa 7, Fumerola 4, Fungoida 4, Osseus 6, Recepta 3, Stratum 8, Tubus 5, Tussock 15 |
| Radicoida (2025 addition) | 1 | yes (119,037 Cr) | Radicoida Unica, HIP 87621 only; introduced Nov 2025 (in-game Nov 3311) |
| Horizons-era surface organics | 27 | **yes** | Anemone 8, Brain Tree 8, Sinuous Tubers 8, Bark Mounds 1, Amphora Plant 1, Crystalline Shards 1 — all have Vista rewards in the Canonn codex reference and in EDDiscovery's value table; sampled with the Genetic Sampler on foot |
| Thargoid xeno samples | 8 codex ids | yes | Coral Root/Tree, Thargoid Spires (5 ids), Barnacle Matrix/Mega Barnacles — site-only, not predictable |
| Space-borne / orbital life & crystals | 17 Canonn sub-classes | **no** | Lagrange & storm clouds, mineral spheres, ice/silicate/metallic crystals, calcite plates, peduncle/aster/octahedral ("void")/collared/chalice/gyre/rhizome/quadripartite/stolon pods & trees, molluscs. Canonn `reward` is null; codex-only (listed in `genera.json → non_sellable_codex_life`). |

### 2.2 Values, first-logged bonus, history

**Current base values — HIGH.** For every Odyssey and Horizons-era species the value is identical in (a) EDDiscovery's
post-Update-14 table, (b) EDMC-BioScan rule files, (c) the live Canonn codex reference (`reward`, fetched 2026-10-12) and
(d) the Fandom "Vista Genomics" price list (sourced from Canonn, 3 June 2024). Range 1,000,000 – 20,000,000 Cr.
Radicoida Unica (119,037 Cr) is only in BioScan, the Canonn codex reference and Canonn's Radicoida page (which also notes a
952,296 Cr community-goal price at the Exogene Sciences base, 13–27 Nov 3311).

- **Only disagreement:** Concha Biconcavis — BioScan carries 16,777,215 (= 2²⁴−1, unchanged since 2023) while EDDiscovery,
  Canonn's codex ref, Canonn's Concha page and Fandom's Vista Genomics page all give **19,010,800**. Dataset uses 19,010,800
  (MEDIUM) and keeps the alternative in `value_sources`.
- Fandom's "Exobiology Sample Values and Details" page also lists italic values its editors flag as "may be outdated"
  (Amphora Plant / Crystalline Shards 3,626,400; five brain trees 3,565,100; Bacterium Nebulus 9,116,600; Bacterium
  Scopulum 8,633,800; Croceum Anemone 3,399,800; Albidum Sinuous Tubers 3,425,600; 16,777,215 for Biconcavis/Fluctus).
  They are higher than current payouts and are likely Update-14.0 numbers that were reduced in 14.01 — do not use. MEDIUM.

**When did values change? (MEDIUM).** The large exobiology revaluation shipped with **Update 14 (29 Nov 2022 — the
Live/Legacy galaxy split)**; Frontier's **Update 14.01 notes (7 Dec 2022)** say "Exobiology rank requirements have been
adjusted to correctly account for their adjusted balance". Community reports describe 14.01 also trimming 14.0 base
payouts. No evidence was found that **Update 15 (9 May 2023)** changed exobiology values — the "Update 15 rebalance"
premise is not supported; EDDiscovery names its tables `valuelistpre14` / `valuelistU14`. For journals before
2022-11-29 use `value_pre_update14` (EDDiscovery's pre-U14 table, single source → LOW/MEDIUM).

**First-logged bonus.** `SellOrganicData.BioData[]` has `Value` and `Bonus`. A "first logged" sample pays
`Bonus = 4 × Value`, i.e. **5 × base in total** (Fandom; BioScan and SrvSurvey both multiply by 5). An issue-tracker
quotation of the Update 14 notes says the first-time multiplier went "from 2 to 5" (LOW for the pre-U14 factor).

**Eligibility rule as modelled by current tools (MEDIUM):**

1. The body must not have been footfalled before: `Scan.WasFootfalled == false` (newer journals) — i.e. you make first footfall.
2. The system must be unpopulated (`Population == 0` from `FSDJump`/`Location`). SrvSurvey: first footfall "is not possible in
   systems with any population, but WasFootfalled will still be false". BioScan applies the same two tests.
3. If `ScanOrganic.WasLogged == true` the species was already logged on that body → no bonus (BioScan shows a "no entry" icon).
4. The bonus is per species per body and goes to the first commander to *sell* it (forum consensus); unsold data is lost on death.

Older tools without `WasFootfalled` had to infer this (SrvSurvey even samples the blue "first footfall" banner colour from the
screen). Treat bonus predictions as "possible" until the sale confirms (`Bonus > 0`).

**Fleet carrier sale:** Vista Genomics on a fleet carrier pays 75 % to the seller (12.5 % to the carrier owner, 12.5 % lost) —
forum statement about the update that added carrier Vista Genomics; same 75 % rule as carrier cartographics. MEDIUM.

### 2.3 Clonal colony range (minimum distance between samples)

| Genus | Range (m) | ExploData/BioScan | Observatory Botanist | SrvSurvey wiki | Fandom | Certainty |
|---|---:|:---:|:---:|:---:|:---:|---|
| Aleoida | 150 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Bacterium | 500 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Cactoida | 300 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Clypeus | 150 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Concha | 150 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Electricae | 1000 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Fonticulua | 500 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Frutexa | 150 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Fumerola | 100 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Fungoida | 300 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Osseus | 800 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Recepta | 150 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Stratum | 500 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Tubus | 800 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Tussock | 200 | ✓ | ✓ | ✓ | ✓ | HIGH |
| Anemone, Brain Tree, Sinuous Tubers, Bark Mounds, Amphora Plant, Crystalline Shards | 100 | ✓ | ✓ (default 100) | ✓ | ✓ | HIGH |
| Radicoida | 15 | ✓ | – | – | "?" | LOW |
| Thargoid samples | 100 | – | – | – | ✓ | LOW |

### 2.4 Codex ids and journal events

| Event | Relevant fields |
|---|---|
| `FSSBodySignals` (after FSS) | `Signals[]` with `Type = "$SAA_SignalType_Biological;"` and `Count` → number of bio signals (genera) on the body |
| `SAASignalsFound` (after DSS) | `Genuses[].Genus` (+`Genus_Localised`) → the exact genera present |
| `ScanOrganic` | `ScanType` = `Log` (1st) / `Sample` (2nd) / `Analyse` (3rd, completes), `Genus`, `Species`, `Variant` (e.g. `$Codex_Ent_Tubus_01_A_Name;`, localised "Tubus Conifer - Indigo"), `WasLogged`, `SystemAddress`, `Body` |
| `CodexEntry` | `Name` = **variant** id for Odyssey bios (e.g. `$Codex_Ent_Aleoids_01_A_Name;`) and the species id for Horizons-era organics; `Category "$Codex_Category_Biology;"`, `SubCategory "$Codex_SubCategory_Organic_Structures;"`, `Region "$Codex_RegionName_N;"`, `IsNewEntry`, `NewTraitsDiscovered`, `VoucherAmount`, `Latitude`/`Longitude`, `BodyID` |
| `SellOrganicData` | `BioData[]` = `Genus`, `Species`, `Variant`, `Value`, `Bonus` |

Id patterns (HIGH — from journal schemas, BioScan/ExploData tables and 816 Canonn codex biology entries):

- Odyssey genus: `$Codex_Ent_<G>_Genus_Name;` with `<G>` ∈ Aleoids, Bacterial, Cactoid, Clypeus, Conchas, Electricae,
  Fonticulus (Fonticulua), Shrubs (Frutexa), Fumerolas, Fungoids, Osseus, Recepta, Stratum, Tubus, Tussocks,
  Ingensradices (Radicoida).
- Odyssey species: `$Codex_Ent_<G>_<NN>_Name;` (NN = 01…15); Radicoida: `$Codex_Ent_Ingensradices_Unicus_Name;`.
- Variant: `$Codex_Ent_<G>_<NN>_<Key>_Name;`, `<Key>` = star class code `O B A F G K M L T Y TTS Ae W D N` for star-coloured
  species, or a capitalised element name (`Antimony Polonium Ruthenium Technetium Tellurium Yttrium` or
  `Cadmium Mercury Molybdenum Niobium Tin Tungsten`) for material-coloured species.
- Horizons-era ids (exact match required — `$Codex_Ent_Tube` is also a prefix of `$Codex_Ent_Tubus_`):
  Brain Trees genus `$Codex_Ent_Brancae_Name;`, species `$Codex_Ent_Seed_Name;`, `$Codex_Ent_SeedABCD_01/02/03_Name;`,
  `$Codex_Ent_SeedEFGH_01/02/03_Name;`, `$Codex_Ent_SeedEFGH_Name;`; Anemone genus `$Codex_Ent_Sphere_Name;`, species
  `$Codex_Ent_Sphere_Name;`, `…SphereABCD_01-03…`, `…SphereEFGH_01-03…`, `$Codex_Ent_SphereEFGH_Name;`; Sinuous Tubers genus
  `$Codex_Ent_Tube_Name;` with `Tube`, `TubeABCD_01-03`, `TubeEFGH_01-03`, `TubeEFGH`; Bark Mounds `$Codex_Ent_Cone_Name;`;
  Amphora Plant `$Codex_Ent_Vents_Name;`; Crystalline Shards `$Codex_Ent_Ground_Struct_Ice_Name;`.

Journal quirks: Clypeus genus seen as `$Codex_Ent_Clepeus_Genus_Name;;` (Observatory Botanist maps it); Stratum Araneamus may
report genus `$Codex_Ent_Stratum_04_Name;` (ExploData); Brain Trees and Sinuous Tubers can have several species on one body
(ExploData `multiple: True`), whereas for Odyssey genera only one species per genus exists per planet (BioInsights docs).

### 2.5 Prediction rules — semantics

The dataset keeps two layers per species:

- `rulesets` — EDMC-BioScan rule sets (OR of rulesets; each ruleset is an AND of constraints). Use these for prediction.
- `conditions` — a merged envelope (union) of all BioScan rulesets and the SrvSurvey criteria tree, in the field layout
  requested (`atmospheres`, `body_types`, `max_gravity_g`, `min_temp_k`, …). Good for filtering/UI and quick rejection;
  too permissive to be a final predictor on its own. `srvsurvey_crosscheck` and `crosscheck_differences` show the second
  source; `color_variants` lists every known variant with its codex id.

**Units (HIGH for BioScan, MEDIUM for the interpretation).** BioScan converts `g = SurfaceGravity / 9.797759` and
`atm = SurfacePressure / 101231.65625` (these look like Earth's journal values). SrvSurvey uses `/10` and `/100000` (bar);
the dataset converts SrvSurvey thresholds to BioScan units, and also gives raw journal-unit limits
(`min/max_gravity_ms2`, `min/max_pressure_pa`) so the app can compare without conversions. In BioScan `max_pressure` is
exclusive (eliminated if `>=`); missing temperature/pressure skips that test.

**Atmosphere.** Compare `Scan.AtmosphereType`: `CarbonDioxide`, `CarbonDioxideRich`, `Ammonia`, `Argon`, `ArgonRich`, `Helium`,
`Methane`, `MethaneRich`, `Neon`, `NeonRich`, `Nitrogen`, `Oxygen`, `SulphurDioxide`, `Water`, `WaterRich`; empty/`None` = no
atmosphere. "Thinness": every landable atmosphere in Odyssey is thin (SrvSurvey: landable pressure caps at 0.1 atm, minimum
≈0.00098 atm); "-rich" is a separate type; the `Atmosphere` text adds "thin"/"hot thin" but is inconsistent — use
`AtmosphereType` + `SurfacePressure` + `SurfaceTemperature`. Some species also test `AtmosphereComposition` (Recepta:
SulphurDioxide ≥ 1.05 % even in CO₂/O₂ atmospheres; SrvSurvey adds e.g. Aleoida CO₂ = 100 % or SO₂ ≥ 0.99 %).
Horizons-era organics are on airless bodies (SrvSurvey requires `None`); Crystalline Shards are also accepted by BioScan on some
thin atmospheres.

**Body class.** `PlanetClass` ∈ `Rocky body`, `High metal content body`, `Rocky ice body`, `Icy body`, `Metal rich body`.

**Gravity.** Most Odyssey genera top out at ≈0.27–0.28 g (2.70–2.76 m/s²): Aleoida, Cactoida, Clypeus, Concha, Electricae,
Fonticulua, Frutexa, Fumerola, Fungoida, Osseus, Recepta, Tussock. Tubus ≈0.15–0.17 g; Bacterium up to ≈0.61–0.7 g; Stratum up
to ≈0.6 g; Horizons-era organics can be on high-g bodies (Bark Mounds up to ≈3.5 g). BioScan's own UI warns ≥1 g "high" and
≥2.7 g "extreme — impossible on foot".

**Volcanism** (`Scan.Volcanism`, e.g. `"major silicate vapour geysers volcanism"`, empty = none). BioScan semantics: list entries
are lower-case substrings (`"silicate"`, `"major rocky magma"`, `"water"` …), a leading `=` means exact match, `None` = must be
empty, `Any` = must be non-empty, `!x` = non-empty and not containing `x`. In `conditions.volcanism` the dataset uses
`"none"`, `"required"`, `"any"` or a list (`"none"` inside a list means "no volcanism is also fine").

**Star rules (BioScan).** `star` = some star *anywhere in the system* matches; `parent_star` = the primary star or any
parent star of the body matches. Luminosity tuples like `("B","IV")` match `Luminosity` `IV`, `IVa`, `IVb`, `IVab`, `IVz`.
Class matching: `A` also matches `A_BlueWhiteSuperGiant`; `B` → `B_BlueWhiteSuperGiant`; `F` → `F_WhiteSuperGiant`;
`G` → `G_WhiteSuperGiant`; `K` → `K_OrangeGiant`; `M` → `M_RedGiant`, `M_RedSuperGiant`; `D`, `C`, `W` are prefix matches;
everything else exact (`AeBe`, `TTS`, `N`, `H`, `MS`, `S`).

**Galactic region.** Region id from galactic coordinates (`StarPos`) via klightspeed's EliteDangerousRegionMap run-length
map (used by ExploData): `px = floor((x + 49985) * 83 / 4096)`, `pz = floor((z + 24105) * 83 / 4096)`, then walk the RLE row
`pz`. Region ids also appear in `CodexEntry.Region` (`$Codex_RegionName_N;`). Tables of the 42 regions and the groups used by
BioScan follow below; SrvSurvey uses equivalent arm groups (Orion-CygnusArm {7,8,16,17,18,35}, Sagittarius-CarinaArm
{9,18,19,20,21,22,23,40}, Scutum-CentaurusArm {9,10,11,12,24,25,26,42,28}, PerseusArm {15,30,32,33,34,36,38,39},
OuterArm {5,6,13,14,27,29,31,41,37}, CentreLeft {1,4}, CentreTop {1,3,7}, CentreRight {1,2}, and per-organic "batches").

**Nebula proximity.** Electricae Radialem (BioScan `nebula: all` = within 150 ly of a large nebula reference star or within
100 ly of a planetary nebula) and Bark Mounds (BioScan `large` = 150 ly; SrvSurvey ≤ 160 ly; BioScan also accepts systems whose
name starts with a named nebula sector such as "California Sector"). Data: EDAstro `nebulae-coordinates.csv` (SrvSurvey) or
BioScan's reference-star list ("Catalog of Galactic Nebulae").

**Guardian bubbles.** All Brain Trees: inside one of the Guardian-ruins bubbles below (BioScan; SrvSurvey uses the same
750/100 ly radii with slightly different reference systems).

**Sinuous-tuber zones.** Sinuous Tubers occur in distance *bands* around zone centres (table below; BioScan).

**Other system / orbit tests.** Amphora Plant: system must contain an Earth-like world, gas giant with water-based life or a
water giant, plus an A-class star, and the body is a hot (1000–1800 K) airless metal-rich body. Crystalline Shards: system must
contain an ELW, AW, WW, water giant or gas giant with life; body ≥ 12,000 ls from arrival (BioScan; SrvSurvey ≥ 10,000 ls);
star A/F/G/K/MS/S somewhere in the system; "exterior" regions only. Clypeus Speculumi: body ≥ 2,000 ls (BioScan; Fandom says
>2,500 ls). Some Sinuous-tuber rulesets need `OrbitalPeriod < 86,400 s`.

**Colour (variant) determination.**

- *Star-coloured species* (Aleoida, Cactoida, Clypeus, Fonticulua, Frutexa, Stratum, Tubus, Tussock, plus Bacterium Aurasus/
  Alcyoneum/Cerbrus, Concha Aureolas/Labiata, Osseus Fractus/Spiralis/Cornibus/Pellebantus, Recepta Umbrux): the colour key is the
  class of the body's parent star. BioScan walks the body's `Parents` and takes the first parent star whose class has a colour,
  and also adds the primary star (DistanceFromArrivalLS = 0) and stars orbiting a black-hole parent — so several candidate
  colours can remain. SrvSurvey picks the brightest parent star by flux ∝ R²·T⁴/d² (R = radius, T = surface temperature, d from
  semi-major axes). Black holes (`H`) have no colour of their own.
- *Material-coloured species* (most Bacterium, Electricae, Fumerola, Fungoida, Concha Renibus/Biconcavis, Osseus Discus/Pumice,
  Recepta Deltahedronix/Conditivus): the colour key is whichever element of the species' group appears in `Scan.Materials`
  (group A: Antimony, Polonium, Ruthenium, Technetium, Tellurium, Yttrium; group B: Cadmium, Mercury, Molybdenum, Niobium, Tin,
  Tungsten). Several present → ambiguous (BioScan lists all). SrvSurvey requires the material above 0.25 %.
- *Horizons-era organics*: the colour is part of the species name; no variants.
- Cross-check: BioScan's colour tables and the Canonn codex variants agree on **every** overlapping key (0 conflicts). Canonn
  has two variants missing from BioScan (Recepta Umbrux `O` = Indigo, Tussock Stigmasis `Ae` = Amethyst); BioScan predicts some
  keys that Canonn's codex list has never seen (marked `?` in the colour table). HIGH for overlapping keys.

### 2.6 Region, Guardian-bubble and tuber-zone reference tables

| ID | Region | ID | Region | ID | Region |
|---:|---|---:|---|---:|---|
| 1 | Galactic Centre | 15 | Outer Orion-Perseus Conflux | 29 | Errant Marches |
| 2 | Empyrean Straits | 16 | Orion-Cygnus Arm | 30 | Perseus Arm |
| 3 | Ryker's Hope | 17 | Temple | 31 | Formidine Rift |
| 4 | Odin's Hold | 18 | Inner Orion Spur | 32 | Vulcan Gate |
| 5 | Norma Arm | 19 | Hawking's Gap | 33 | Elysian Shore |
| 6 | Arcadian Stream | 20 | Dryman's Point | 34 | Sanguineous Rim |
| 7 | Izanami | 21 | Sagittarius-Carina Arm | 35 | Outer Orion Spur |
| 8 | Inner Orion-Perseus Conflux | 22 | Mare Somnia | 36 | Achilles's Altar |
| 9 | Inner Scutum-Centaurus Arm | 23 | Acheron | 37 | Xibalba |
| 10 | Norma Expanse | 24 | Formorian Frontier | 38 | Lyra's Song |
| 11 | Trojan Belt | 25 | Hieronymus Delta | 39 | Tenebrae |
| 12 | The Veils | 26 | Outer Scutum-Centaurus Arm | 40 | The Abyss |
| 13 | Newton's Vault | 27 | Outer Arm | 41 | Kepler's Crest |
| 14 | The Conduit | 28 | Aquila's Halo | 42 | The Void |

| BioScan group | Region IDs |
|---|---|
| `orion-cygnus` | 1, 4, 7, 8, 16, 17, 18, 35 |
| `orion-cygnus-1` | 4, 7, 8, 16, 17, 18, 35 |
| `orion-cygnus-core` | 7, 8, 16, 17, 18, 35 |
| `sagittarius-carina` | 1, 4, 9, 18, 19, 20, 21, 22, 23, 40 |
| `sagittarius-carina-core` | 9, 18, 19, 20, 21, 22, 23, 40 |
| `sagittarius-carina-core-9` | 18, 19, 20, 21, 22, 23, 40 |
| `scutum-centaurus` | 1, 4, 9, 10, 11, 12, 24, 25, 26, 42, 28 |
| `scutum-centaurus-core` | 9, 10, 11, 12, 24, 25, 26, 42, 28 |
| `outer` | 1, 2, 5, 6, 13, 14, 27, 29, 31, 41, 37 |
| `perseus` | 1, 3, 7, 15, 30, 32, 33, 34, 36, 38, 39 |
| `perseus-core` | 3, 7, 15, 30, 32, 33, 34, 36, 38, 39 |
| `exterior` | 14, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 34, 36, 37, 38, 39, 40, 41, 42 |
| `anemone-a` | 7, 8, 13, 14, 15, 16, 17, 18, 27, 30, 32 |
| `amphora` | 10, 19, 20, 21, 22 |
| `brain-tree` | 2, 9, 10, 17, 18, 35 |
| `empyrean-straits` | 2 |
| `center` | 1, 2, 3 |

| Guardian bubble (BioScan) | Radius ly | x, y, z |
|---|---:|---|
| Hen 2-333 | 750 | -840.65625, -561.15625, 13361.8125 |
| Gamma Velorum | 750 | 1099.21875, -146.6875, -133.59375 |
| Skaudai AA-A h71 | 100 | -5493.09375, -589.28125, 10424.4375 |
| Blaa Hypai AA-A h68 | 100 | 1220.40625, -694.625, 12312.8125 |
| Eorl Auwsy AA-A h72 | 100 | 4949.9375, 164, 20640.125 |
| Prai Hypoo AA-A h60 | 100 | -9294.875, -458.40625, 7905.71875 |
| Eta Carina Nebula | 100 | 8579.96875, -138.96875, 2701.375 |
| NGC 3199 | 100 | 14574.15625, -259.625, 3511.90625 |

| Tuber zone (BioScan) | Distance band from centre (ly) | Centre x, y, z |
|---|---|---|
| Arcadian Stream | 200-600 | 8885, -20, 20535 |
| Empyrean Straits | 200-400 | 4325, 400, 21185 |
| Galactic Center | 500-1000 | 44.5, 492.7, 25916 |
| Hawking A | 150-600 | 5788, 150, 6335 |
| Hawking B | 200-600 | 9990, -40, 8335 |
| Inner Orion Spur | 200-600 | -3485, 39, 7320 |
| Inner O-P Conflux | 350-750 | -13245, -80, 30285 |
| Inner S-C Arm A | 200-600 | -1600, -37, 10720 |
| Inner S-C Arm B 1 | 150-300 | -6645, 0, 12590 |
| Inner S-C Arm B 2 | 200-600 | -6645, 0, 12590 |
| Inner S-C Arm C | 200-600 | -9355, -50, 17175 |
| Inner S-C Arm D | 300-400 | -12000, 232, 22670 |
| Izanami | 200-750 | -4610, 370, 37225 |
| Norma Arm A | 500-1000 | 3722.6, 200, 16441 |
| Norma Arm B | 200-500 | 3740, 175, 16460 |
| Norma Expanse A | 200-600 | 4245, -42, 12071 |
| Norma Expanse B | 150-250 | 5580, 40, 11727 |
| Odin A | 750-1000 | -7945, 230, 28025 |
| Odin B | 200-600 | -5329, -68, 18647 |
| Ryker A | 250-750 | 1715, 766, 34070 |
| Ryker B | 750-1500 | -1445, 345, 30345 |
| Trojan Belt | 250-500 | 18600, 65, 31750 |


### 2.7 Species table

Generated from `species.json`. Legend: atmospheres CO2 / CO2r (rich) / NH3 / Ar / Arr / He / CH4 / CH4r / Ne / Ner / N2 / O2 /
SO2 / H2O / H2Or / none; bodies R = rocky, HMC, I = icy, RI = rocky-ice, MR = metal-rich; `g max` in BioScan g (×9.797759 for
m/s²); `reg` = allowed galactic region ids, `not reg` = excluded ids; `*` after a value = sources disagree (see §2.2).
`Cert` is the overall certainty = minimum of value, conditions and colony-range certainty.

| Species | Value (Cr) | x5 first-logged | Range m | Species codex id | Atmospheres | Bodies | g max | T min-max K | Volcanism | Colour by | Other | Cert |
|---|---:|---:|---:|---|---|---|---:|---|---|---|---|---|
| Aleoida Arcus | 7,252,500 | 36,262,500 | 150 | `$Codex_Ent_Aleoids_01_Name;` | CO2 | HMC,R | 0.2817 | 175-180 | none | star |  | high |
| Aleoida Coronamus | 6,284,600 | 31,423,000 | 150 | `$Codex_Ent_Aleoids_02_Name;` | CO2 | HMC,R | 0.2817 | 180-190 | none | star |  | high |
| Aleoida Gravis | 12,934,900 | 64,674,500 | 150 | `$Codex_Ent_Aleoids_05_Name;` | CO2 | HMC,R | 0.2817 | 190-197 | none | star |  | high |
| Aleoida Laminiae | 3,385,200 | 16,926,000 | 150 | `$Codex_Ent_Aleoids_04_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star | reg 1,4,7,8,9,16,17,18,19,20,21,22,23,35,40 | high |
| Aleoida Spica | 3,385,200 | 16,926,000 | 150 | `$Codex_Ent_Aleoids_03_Name;` | NH3 | HMC,R | 0.2817 | 170-177 | any | star | not reg 7,8,9,16,17,18,19,20,21,22,23,35,40 | high |
| Bacterium Acies | 1,000,000 | 5,000,000 | 500 | `$Codex_Ent_Bacterial_04_Name;` | Ne | I,RI | 0.6226 | 20-61 | any | element |  | high |
| Bacterium Alcyoneum | 1,658,500 | 8,292,500 | 500 | `$Codex_Ent_Bacterial_06_Name;` | NH3 | HMC,R,RI | 0.6226 | 152-177 | any | star |  | medium |
| Bacterium Aurasus | 1,000,000 | 5,000,000 | 500 | `$Codex_Ent_Bacterial_01_Name;` | CO2 | HMC,R,RI | 0.6226 | 145-400 | any | star |  | medium |
| Bacterium Bullaris | 1,152,500 | 5,762,500 | 500 | `$Codex_Ent_Bacterial_10_Name;` | CH4,CH4r | HMC,I,R,RI | 0.6226 | 67-141 | any | element |  | medium |
| Bacterium Cerbrus | 1,689,800 | 8,449,000 | 500 | `$Codex_Ent_Bacterial_12_Name;` | SO2,H2O,H2Or | HMC,R,RI | 0.6226 | any | any | star |  | high |
| Bacterium Informem | 8,418,000 | 42,090,000 | 500 | `$Codex_Ent_Bacterial_08_Name;` | N2 | HMC,I,R,RI | 0.63 | 42.5-151 | any | element |  | high |
| Bacterium Nebulus | 5,289,900 | 26,449,500 | 500 | `$Codex_Ent_Bacterial_02_Name;` | He | I,RI | 0.7 | 20-21 | any | element |  | medium |
| Bacterium Omentum | 4,638,900 | 23,194,500 | 500 | `$Codex_Ent_Bacterial_11_Name;` | Ar,Arr,He,CH4,Ne,Ner,N2,H2Or | I | 0.6226 | >=20 | ammonia/nitrogen | element |  | medium |
| Bacterium Scopulum | 4,934,500 | 24,672,500 | 500 | `$Codex_Ent_Bacterial_03_Name;` | Ar,He,CH4,Ne,Ner,N2,O2 | I,RI | 0.6226 | 20-220 | carbon dioxide/methane | element |  | high |
| Bacterium Tela | 1,949,000 | 9,745,000 | 500 | `$Codex_Ent_Bacterial_07_Name;` | NH3,Ar,Arr,CO2,CO2r,He,CH4,Ne,Ner,N2,O2,SO2,H2O,H2Or | HMC,I,R,RI | 0.6226 | any | any | element |  | high |
| Bacterium Verrata | 3,897,000 | 19,485,000 | 500 | `$Codex_Ent_Bacterial_13_Name;` | NH3,Ar,Arr,CO2,CO2r,He,Ne,Ner,N2,O2,H2O,H2Or | I,R,RI | 0.6226 | any | water | element |  | medium |
| Bacterium Vesicula | 1,000,000 | 5,000,000 | 500 | `$Codex_Ent_Bacterial_05_Name;` | Ar | HMC,I,R,RI | 0.5205 | 50-267 | any | element |  | medium |
| Bacterium Volu | 7,774,700 | 38,873,500 | 500 | `$Codex_Ent_Bacterial_09_Name;` | O2 | HMC,I,R,RI | 0.6226 | 143.5-246 | any | element |  | high |
| Cactoida Cortexum | 3,667,600 | 18,338,000 | 300 | `$Codex_Ent_Cactoid_01_Name;` | CO2 | HMC,R | 0.2817 | 180-197 | none | star | reg 1,4,7,8,16,17,18,35 | high |
| Cactoida Lapis | 2,483,600 | 12,418,000 | 300 | `$Codex_Ent_Cactoid_02_Name;` | NH3 | HMC,R | 0.2817 | 160-177 | any | star | reg 1,4,9,18,19,20,21,22,23,40 | high |
| Cactoida Peperatis | 2,483,600 | 12,418,000 | 300 | `$Codex_Ent_Cactoid_05_Name;` | NH3 | HMC,R | 0.2817 | 160-177 | any | star | reg 1,4,9,10,11,12,24,25,26,28,42 | high |
| Cactoida Pullulanta | 3,667,600 | 18,338,000 | 300 | `$Codex_Ent_Cactoid_04_Name;` | CO2 | HMC,R | 0.2817 | 180-197 | none | star | reg 1,3,7,15,30,32,33,34,36,38,39 | high |
| Cactoida Vermis | 16,202,800 | 81,014,000 | 300 | `$Codex_Ent_Cactoid_03_Name;` | SO2,H2O | HMC,R | 0.2817 | >=160 | none/water | star |  | high |
| Clypeus Lacrimam | 8,418,000 | 42,090,000 | 150 | `$Codex_Ent_Clypeus_01_Name;` | CO2,H2O | HMC,R | 0.2817 | >=190 | none/water | star |  | medium |
| Clypeus Margaritus | 11,873,200 | 59,366,000 | 150 | `$Codex_Ent_Clypeus_02_Name;` | CO2,H2O | HMC,R | 0.2817 | >=190 | none | star |  | medium |
| Clypeus Speculumi | 16,202,800 | 81,014,000 | 150 | `$Codex_Ent_Clypeus_03_Name;` | CO2,H2O | HMC,R | 0.276 | >=190 | none/water | star | dist>=2000ls | medium |
| Concha Aureolas | 7,774,700 | 38,873,500 | 150 | `$Codex_Ent_Conchas_02_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star |  | high |
| Concha Biconcavis | 19,010,800* | 95,054,000 | 150 | `$Codex_Ent_Conchas_04_Name;` | N2 | HMC,R | 0.2817 | 42-52 | none | element |  | medium |
| Concha Labiata | 2,352,400 | 11,762,000 | 150 | `$Codex_Ent_Conchas_03_Name;` | CO2 | HMC,R | 0.2817 | 150-200 | none | star |  | high |
| Concha Renibus | 4,572,400 | 22,862,000 | 150 | `$Codex_Ent_Conchas_01_Name;` | NH3,CO2,CH4,H2O | HMC,R | 0.65 | >=79 | none/metallic/silicate/water | element |  | medium |
| Electricae Pluma | 6,284,600 | 31,423,000 | 1000 | `$Codex_Ent_Electricae_01_Name;` | Ar,Arr,Ne,Ner | I | 0.2817 | 20-150 | any | element | star A,AeBe,D,H,N | high |
| Electricae Radialem | 6,284,600 | 31,423,000 | 1000 | `$Codex_Ent_Electricae_02_Name;` | Ar,Arr,CH4,Ne,Ner | I | 0.2817 | 20-150 | any | element | nebula | medium |
| Fonticulua Campestris | 1,000,000 | 5,000,000 | 500 | `$Codex_Ent_Fonticulus_02_Name;` | Ar | I,RI | 0.2817 | 50-151 | any | star |  | high |
| Fonticulua Digitos | 1,804,100 | 9,020,500 | 500 | `$Codex_Ent_Fonticulus_06_Name;` | CH4 | I,RI | 0.2817 | 83-109 | any | star |  | medium |
| Fonticulua Fluctus | 20,000,000 | 100,000,000 | 500 | `$Codex_Ent_Fonticulus_05_Name;` | CO2r,O2 | I,RI | 0.2817 | 143-200 | any | star |  | medium |
| Fonticulua Lapida | 3,111,000 | 15,555,000 | 500 | `$Codex_Ent_Fonticulus_04_Name;` | N2 | I,RI | 0.2817 | 50-81 | any | star |  | high |
| Fonticulua Segmentatus | 19,010,800 | 95,054,000 | 500 | `$Codex_Ent_Fonticulus_01_Name;` | Ne,Ner | I,RI | 0.2817 | 50-75 | none/minor nitrogen magma | star |  | medium |
| Fonticulua Upupam | 5,727,600 | 28,638,000 | 500 | `$Codex_Ent_Fonticulus_03_Name;` | Arr | I,RI | 0.2817 | 61-125 | any | star |  | high |
| Frutexa Acus | 7,774,700 | 38,873,500 | 150 | `$Codex_Ent_Shrubs_02_Name;` | CO2 | HMC,R | 0.2429 | 146-197 | none | star | reg 1,4,7,8,16,17,18,35 | medium |
| Frutexa Collum | 1,639,800 | 8,199,000 | 150 | `$Codex_Ent_Shrubs_07_Name;` | SO2 | HMC,R | 0.2817 | 132-215 | any | star |  | high |
| Frutexa Fera | 1,632,500 | 8,162,500 | 150 | `$Codex_Ent_Shrubs_05_Name;` | CO2 | HMC,R | 0.276 | 146-197 | none | star | reg 1,2,5,6,13,14,27,29,31,37,41 | medium |
| Frutexa Flabellum | 1,808,900 | 9,044,500 | 150 | `$Codex_Ent_Shrubs_01_Name;` | NH3,CO2 | HMC,R | 0.2817 | 152-177 | any | star | not reg 1,4,9,10,11,12,24,25,26,28,42 | medium |
| Frutexa Flammasis | 10,326,000 | 51,630,000 | 150 | `$Codex_Ent_Shrubs_04_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star | reg 1,4,9,10,11,12,24,25,26,28,42 | medium |
| Frutexa Metallicum | 1,632,500 | 8,162,500 | 150 | `$Codex_Ent_Shrubs_03_Name;` | NH3,CO2,CH4,H2O | HMC,R | 0.2817 | <=400 | any | star |  | medium |
| Frutexa Sponsae | 5,988,000 | 29,940,000 | 150 | `$Codex_Ent_Shrubs_06_Name;` | H2O | HMC,R | 0.2817 | >=390 | none/water | star |  | medium |
| Fumerola Aquatis | 6,284,600 | 31,423,000 | 100 | `$Codex_Ent_Fumerolas_04_Name;` | NH3,Ar,Arr,CO2,CO2r,CH4,Ne,N2,O2,SO2,H2O | I,R,RI | 0.2817 | any | water | element |  | medium |
| Fumerola Carbosis | 6,284,600 | 31,423,000 | 100 | `$Codex_Ent_Fumerolas_01_Name;` | NH3,Ar,Arr,CO2r,CH4,Ne,N2,O2,SO2 | I,RI | 0.276 | any | carbon/methane/methane magma | element |  | medium |
| Fumerola Extremus | 16,202,800 | 81,014,000 | 100 | `$Codex_Ent_Fumerolas_02_Name;` | NH3,Ar,CO2,CH4,SO2 | HMC,R,RI | 0.276 | >=50 | metallic/rocky/silicate | element |  | medium |
| Fumerola Nitris | 7,500,900 | 37,504,500 | 100 | `$Codex_Ent_Fumerolas_03_Name;` | Ar,Arr,CH4,Ne,Ner,N2,O2,SO2 | I | 0.276 | >=30 | ammonia/nitrogen | element |  | high |
| Fungoida Bullarum | 3,703,200 | 18,516,000 | 300 | `$Codex_Ent_Fungoids_03_Name;` | Ar,N2 | HMC,R,RI | 0.2817 | 50-133 | none | element |  | high |
| Fungoida Gelata | 3,330,300 | 16,651,500 | 300 | `$Codex_Ent_Fungoids_04_Name;` | NH3,Ar,CO2,CH4,H2O | HMC,R,RI | 0.2817 | >=77 | none/major metallic/major rocky/major silicate | element | not reg 7,8,16,17,18,35 | high |
| Fungoida Setisis | 1,670,100 | 8,350,500 | 300 | `$Codex_Ent_Fungoids_01_Name;` | NH3,CO2,CH4 | HMC,R,RI | 0.2817 | 67-177 | any | element |  | medium |
| Fungoida Stabitis | 2,680,300 | 13,401,500 | 300 | `$Codex_Ent_Fungoids_02_Name;` | NH3,Ar,Arr,CO2,CH4,H2O | HMC,I,R,RI | 0.5 | any | any | element | reg 1,4,7,8,16,17,18,35 | medium |
| Osseus Cornibus | 1,483,000 | 7,415,000 | 800 | `$Codex_Ent_Osseus_05_Name;` | CO2 | HMC,R | 0.2817 | 180-195.41 | none | star | reg 1,3,7,15,30,32,33,34,36,38,39 | high |
| Osseus Discus | 12,934,900 | 64,674,500 | 800 | `$Codex_Ent_Osseus_02_Name;` | NH3,Ar,CO2,CH4,H2O | HMC,R,RI | 0.2817 | any | any | element |  | medium |
| Osseus Fractus | 4,027,800 | 20,139,000 | 800 | `$Codex_Ent_Osseus_01_Name;` | CO2 | HMC,R | 0.2817 | 180-190 | none | star | not reg 1,3,7,15,30,32,33,34,36,38,39 | high |
| Osseus Pellebantus | 9,739,000 | 48,695,000 | 800 | `$Codex_Ent_Osseus_06_Name;` | CO2 | HMC,R | 0.2817 | 191-195.41 | none | star | not reg 1,3,7,15,30,32,33,34,36,38,39 | high |
| Osseus Pumice | 3,156,300 | 15,781,500 | 800 | `$Codex_Ent_Osseus_04_Name;` | Ar,Arr,CH4,N2 | HMC,R,RI | 0.2817 | 42-135 | any | element |  | high |
| Osseus Spiralis | 2,404,700 | 12,023,500 | 800 | `$Codex_Ent_Osseus_03_Name;` | NH3 | HMC,R,RI | 0.2817 | 160-177 | any | star |  | high |
| Radicoida Unica | 119,037 | n/a | 15 | `$Codex_Ent_Ingensradices_Unicus_Name;` | - | - | - | any | any | none | HIP 87621 only | low |
| Recepta Conditivus | 14,313,700 | 71,568,500 | 150 | `$Codex_Ent_Recepta_03_Name;` | CO2,CO2r,O2,SO2 | HMC,I,R,RI | 0.2817 | 132-275 | any | element | SO2>=1.05% | medium |
| Recepta Deltahedronix | 16,202,800 | 81,014,000 | 150 | `$Codex_Ent_Recepta_02_Name;` | CO2,CO2r,O2,SO2 | HMC,I,R,RI | 0.2817 | 132-272 | any | element | SO2>=1.05% | medium |
| Recepta Umbrux | 12,934,900 | 64,674,500 | 150 | `$Codex_Ent_Recepta_01_Name;` | NH3,CO2,CO2r,O2,SO2 | HMC,I,R,RI | 0.2817 | 132-273 | any | star | SO2>=1.05% | medium |
| Stratum Araneamus | 2,448,900 | 12,244,500 | 500 | `$Codex_Ent_Stratum_04_Name;` | SO2 | R | 0.5818 | 165-373 | any | star |  | high |
| Stratum Cucumisis | 16,202,800 | 81,014,000 | 500 | `$Codex_Ent_Stratum_06_Name;` | CO2,CO2r,O2,SO2 | R | 0.6022 | 191-373 | any | star | reg 1,4,9,18,19,20,21,22,23,40 | high |
| Stratum Excutitus | 2,448,900 | 12,244,500 | 500 | `$Codex_Ent_Stratum_01_Name;` | CO2,O2,SO2 | R | 0.5001 | 165-190 | any | star | reg 1,4,7,8,16,17,18,35 | medium |
| Stratum Frigus | 2,637,500 | 13,187,500 | 500 | `$Codex_Ent_Stratum_08_Name;` | CO2,CO2r,O2,SO2 | R | 0.5716 | 191-369 | any | star | reg 3,7,15,30,32,33,34,36,38,39 | medium |
| Stratum Laminamus | 2,788,300 | 13,941,500 | 500 | `$Codex_Ent_Stratum_03_Name;` | NH3 | R | 0.34 | 161-177 | any | star | reg 1,4,7,8,16,17,18,35 | high |
| Stratum Limaxus | 1,362,000 | 6,810,000 | 500 | `$Codex_Ent_Stratum_05_Name;` | CO2,O2,SO2 | R | 0.4899 | 165-190 | any | star | reg 9,10,11,12,24,25,26,28,42 | medium |
| Stratum Paleas | 1,362,000 | 6,810,000 | 500 | `$Codex_Ent_Stratum_02_Name;` | NH3,CO2,CO2r,O2,H2O | R | 0.59 | >=165 | any | star |  | high |
| Stratum Tectonicas | 19,010,800 | 95,054,000 | 500 | `$Codex_Ent_Stratum_07_Name;` | NH3,Ar,Arr,CO2,CO2r,O2,SO2,H2O | HMC | 0.62 | >=165 | any | star |  | medium |
| Tubus Cavas | 11,873,200 | 59,366,000 | 800 | `$Codex_Ent_Tubus_03_Name;` | CO2 | R | 0.1562 | 160-197 | none | star | reg 1,4,9,10,11,12,24,25,26,28,42 | high |
| Tubus Compagibus | 7,774,700 | 38,873,500 | 800 | `$Codex_Ent_Tubus_05_Name;` | CO2 | R | 0.1633 | 160-197 | none | star | reg 1,4,9,18,19,20,21,22,23,40 | high |
| Tubus Conifer | 2,415,500 | 12,077,500 | 800 | `$Codex_Ent_Tubus_01_Name;` | CO2 | R | 0.1551 | 160-197 | none | star | reg 1,3,7,15,30,32,33,34,36,38,39 | high |
| Tubus Rosarium | 2,637,500 | 13,187,500 | 800 | `$Codex_Ent_Tubus_04_Name;` | NH3 | R | 0.1674 | 160-177 | any | star |  | high |
| Tubus Sororibus | 5,727,600 | 28,638,000 | 800 | `$Codex_Ent_Tubus_02_Name;` | NH3,CO2 | HMC | 0.1562 | 160-195 | any | star |  | high |
| Tussock Albata | 3,252,500 | 16,262,500 | 200 | `$Codex_Ent_Tussocks_08_Name;` | CO2 | HMC,R | 0.2817 | 175-180 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | high |
| Tussock Capillum | 7,025,800 | 35,129,000 | 200 | `$Codex_Ent_Tussocks_15_Name;` | Ar,CH4 | R,RI | 0.2817 | 80-129 | any | star |  | high |
| Tussock Caputus | 3,472,400 | 17,362,000 | 200 | `$Codex_Ent_Tussocks_11_Name;` | CO2 | HMC,R | 0.2817 | 181-190 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | high |
| Tussock Catena | 1,766,600 | 8,833,000 | 200 | `$Codex_Ent_Tussocks_05_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star | reg 9,10,11,12,24,25,26,28,42 | high |
| Tussock Cultro | 1,766,600 | 8,833,000 | 200 | `$Codex_Ent_Tussocks_04_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star | reg 1,4,7,8,16,17,18,35 | high |
| Tussock Divisa | 1,766,600 | 8,833,000 | 200 | `$Codex_Ent_Tussocks_10_Name;` | NH3 | HMC,R | 0.2817 | 152-177 | any | star | reg 3,7,15,30,32,33,34,36,38,39 | high |
| Tussock Ignis | 1,849,000 | 9,245,000 | 200 | `$Codex_Ent_Tussocks_03_Name;` | CO2 | HMC,R | 0.2001 | 161-170 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | high |
| Tussock Pennata | 5,853,800 | 29,269,000 | 200 | `$Codex_Ent_Tussocks_01_Name;` | CO2 | HMC,R | 0.2817 | 145-154 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | medium |
| Tussock Pennatis | 1,000,000 | 5,000,000 | 200 | `$Codex_Ent_Tussocks_06_Name;` | CO2 | HMC,R | 0.2817 | 146-197 | none | star | reg 1,2,5,6,13,14,27,29,31,37,41 | high |
| Tussock Propagito | 1,000,000 | 5,000,000 | 200 | `$Codex_Ent_Tussocks_09_Name;` | CO2 | HMC,R | 0.2817 | 145-197 | none | star | reg 1,4,9,10,11,12,24,25,26,28,42 | high |
| Tussock Serrati | 4,447,100 | 22,235,500 | 200 | `$Codex_Ent_Tussocks_07_Name;` | CO2 | HMC,R | 0.2317 | 171-174 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | high |
| Tussock Stigmasis | 19,010,800 | 95,054,000 | 200 | `$Codex_Ent_Tussocks_13_Name;` | SO2 | HMC,R | 0.2817 | 132-180 | any | star |  | medium |
| Tussock Triticum | 7,774,700 | 38,873,500 | 200 | `$Codex_Ent_Tussocks_12_Name;` | CO2 | HMC,R | 0.2817 | 191-197 | none | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | high |
| Tussock Ventusa | 3,227,700 | 16,138,500 | 200 | `$Codex_Ent_Tussocks_02_Name;` | CO2 | HMC,R | 0.1633 | 155-160 | none/minor carbon dioxide geysers | star | reg 3,7,8,15,16,17,18,19,20,21,22,23,30,32,33,34,35,36,38,39,40 | medium |
| Tussock Virgam | 14,313,700 | 71,568,500 | 200 | `$Codex_Ent_Tussocks_14_Name;` | H2O | HMC,R | 0.2817 | >=390 | none/water | star |  | medium |
| Amphora Plant | 1,628,800 | 8,144,000 | 100 | `$Codex_Ent_Vents_Name;` | none | MR | - | 1000-1800 | metallic/rocky/silicate | none | reg 10,19,20,21,22; system has ELW, GG-WL, WG; star A | medium |
| Blatteum Bioluminescent Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereEFGH_Name;` | none | HMC,MR | - | 220-6000 | required | none | reg 7,8,13,14,15,16,17,18,27,30,32; star B IV,B V | high |
| Croceum Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereABCD_01_Name;` | none | R | 0.4287 | 200-440 | metallic/rocky/silicate | none | reg 7,8,13,14,15,16,17,18,27,30,32; star A III,B V,B VI | medium |
| Luteolum Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_Sphere_Name;` | none | R | 1.633 | 200-440 | metallic/rocky/silicate/water | none | reg 7,8,13,14,15,16,17,18,27,30,32; star B IV,B V | medium |
| Prasinum Bioluminescent Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereEFGH_02_Name;` | none | HMC,MR,R | - | 110-4000 | any | none | star O | medium |
| Puniceum Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereABCD_02_Name;` | none | I,RI | 2.6537 | 65-800 | none/carbon dioxide geysers | none | reg 7,8,13,14,15,16,17,18,27,30,32; star O | medium |
| Roseum Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereABCD_03_Name;` | none | R | 0.394 | 200-440 | metallic/rocky/silicate | none | reg 7,8,13,14,15,16,17,18,27,30,32; star B I,B II,B III,B IV | medium |
| Roseum Bioluminescent Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereEFGH_03_Name;` | none | HMC,MR | 4.61 | 400-5000 | required | none | star B I,B II,B III | high |
| Rubeum Bioluminescent Anemone | 1,499,900 | 7,499,500 | 100 | `$Codex_Ent_SphereEFGH_01_Name;` | none | HMC,MR | 4.61 | 160-1800 | required | none | star A I,A II,A III,B VI,N | high |
| Bark Mounds | 1,471,900 | 7,359,500 | 100 | `$Codex_Ent_Cone_Name;` | none | HMC,I,MR,R,RI | 3.5722 | 200-440 | any | none | not reg 1,2,3; nebula | medium |
| Aureum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedEFGH_01_Name;` | none | HMC,MR,R | 2.9599 | 300-500 | metallic/rocky/silicate | none | reg 2,9,10,17,18,35; guardian bubble | medium |
| Gypseeum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedABCD_01_Name;` | none | R | 0.42 | 200-400 | metallic/rocky/silicate/water | none | reg 2,9,10,17,18,35; guardian bubble; system has ELW, GG-WL, WG | medium |
| Lindigoticum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedEFGH_03_Name;` | none | HMC,R | 2.7 | 300-500 | metallic/rocky/silicate | none | reg 2,9,10,17,18,35; guardian bubble; system has ELW, GG-WL, WG | medium |
| Lividum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedEFGH_Name;` | none | R | 0.5 | 300-500 | none/metallic/rocky/silicate/water | none | reg 2,9,10,17,18,35; guardian bubble | medium |
| Ostrinum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedABCD_02_Name;` | none | HMC,MR,R | 2.3903 | 20-1800 | metallic/rocky/silicate | none | reg 2,9,10,17,18,35; guardian bubble | medium |
| Puniceum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedEFGH_02_Name;` | none | HMC,MR | 4.8991 | 20-1800 | required | none | reg 2,9,10,17,18,35; guardian bubble; system has ELW, GG-WL, WG | high |
| Roseum Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_Seed_Name;` | none | HMC,I,R,RI | 4.2887 | 200-500 | required | none | reg 2,9,10,17,18,35; guardian bubble | high |
| Viride Brain Tree | 1,593,700 | 7,968,500 | 100 | `$Codex_Ent_SeedABCD_03_Name;` | none | RI | 0.4 | 100-270 | required | none | reg 2,9,10,17,18,35; guardian bubble; system has ELW, GG-WL, WG | medium |
| Crystalline Shards | 1,628,800 | 8,144,000 | 100 | `$Codex_Ent_Ground_Struct_Ice_Name;` | Ar,Arr,CO2,CO2r,He,CH4,Ne,Ner,none | HMC,I,R,RI | 2 | 20-273 | any | none | reg 14,21,22,23,24,25,26,27,28,29,31,34,36,37,38,39,40,41,42; system has AW, ELW, GG-AL, GG-WL, WG, WW; dist>=12000ls; star A,F,G,K,MS,S | medium |
| Albidum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeABCD_02_Name;` | none | HMC,R | 3.0619 | 200-500 | major metallic magma/major silicate vapour | none | tuber zone; some: orbit<1d | medium |
| Blatteum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeEFGH_Name;` | none | HMC,MR,R | 3.0619 | 200-500 | =metallic magma volcanism/=rocky magma volcanism/major silic | none | tuber zone | medium |
| Caeruleum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeABCD_03_Name;` | none | HMC,R | 3.0619 | 200-500 | major silicate vapour | none | tuber zone; some: orbit<1d | medium |
| Lindigoticum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeEFGH_01_Name;` | none | HMC,R | 2.0413 | 200-500 | major silicate vapour | none | tuber zone; some: orbit<1d | medium |
| Prasinum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeABCD_01_Name;` | none | HMC,MR,R | 3.0619 | 200-500 | required | none | tuber zone | medium |
| Roseum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_Tube_Name;` | none | HMC,R | 4.0826 | 200-500 | rocky magma | none | tuber zone | medium |
| Violaceum Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeEFGH_02_Name;` | none | HMC,MR,R | 3.0619 | 200-500 | major rocky magma/major silicate vapour | none | tuber zone | medium |
| Viride Sinuous Tubers | 1,514,500 | 7,572,500 | 100 | `$Codex_Ent_TubeEFGH_03_Name;` | none | HMC,R | 2.0413 | 200-500 | major rocky magma/major silicate vapour | none | tuber zone; some: orbit<1d | high |

| Thargoid sample | Value (Cr) | Codex id |
|---|---:|---|
| Coral Root | 1,924,600 | `$Codex_Ent_Thargoid_Coral_Root_Name;` |
| Coral Tree | 1,896,800 | `$Codex_Ent_Thargoid_Coral_Tree_Name;` |
| Major Thargoid Spire | 2,247,100 | `$Codex_Ent_Thargoid_Tower_High_Name;` |
| Minor Thargoid Spire | 2,247,100 | `$Codex_Ent_Thargoid_Tower_Low_Name;` |
| Primary Thargoid Spire | 2,247,100 | `$Codex_Ent_Thargoid_Tower_ExtraHigh_Name;` |
| Thargoid Barnacle Matrix | 2,313,500 | `$Codex_Ent_Thargoid_Barnacle_Matrix_Name;` |
| Thargoid Spire | 2,247,100 | `$Codex_Ent_Thargoid_Tower_Med_Name;` |
| Thargoid Spires | 2,247,100 | `$Codex_Ent_Thargoid_Tower_Name;` |


### 2.8 Colour variants

Star keys: O B A F G K M L T Y, TTS (T Tauri), Ae (Herbig Ae/Be), W (Wolf-Rayet), D (white dwarf), N (neutron star).
Element keys: the element present in `Scan.Materials`. Variant codex id = species id with `_<key>_Name;`
(e.g. `$Codex_Ent_Aleoids_01_B_Name;`, `$Codex_Ent_Bacterial_04_Antimony_Name;`). Canonn entry ids are in `species.json`.

| Species | Basis | Variants (key: colour; `?` = predicted by BioScan but no such codex entry in Canonn ref; `!` = in codex ref but missing from BioScan map) |
|---|---|---|
| Aleoida Arcus | star | B: Yellow, A: Green, F: Teal, K: Turquoise, M: Emerald, L: Lime, T: Sage, Y: Amethyst, TTS: Mauve, W: Grey, D: Indigo, N: Ocher |
| Aleoida Coronamus | star | B: Yellow, A: Green, F: Teal, K: Turquoise, M: Emerald, L: Lime, T: Sage, Y: Amethyst, TTS: Mauve, W: Grey?, D: Indigo, N: Ocher |
| Aleoida Gravis | star | B: Yellow, A: Green, F: Teal, K: Turquoise, M: Emerald, L: Lime, T: Sage, Y: Amethyst, TTS: Mauve, W: Grey?, D: Indigo, N: Ocher |
| Aleoida Laminiae | star | B: Yellow, A: Green, F: Teal, K: Turquoise, M: Emerald, L: Lime, T: Sage, Y: Amethyst, TTS: Mauve, W: Grey, D: Indigo, N: Ocher |
| Aleoida Spica | star | B: Yellow, A: Green, F: Teal, K: Turquoise, M: Emerald, L: Lime, T: Sage, Y: Amethyst, TTS: Mauve, W: Grey?, D: Indigo, N: Ocher |
| Bacterium Acies | element | Antimony: Cyan, Polonium: Magenta, Ruthenium: Cobalt, Technetium: Lime, Tellurium: White, Yttrium: Aquamarine |
| Bacterium Alcyoneum | star | O: Turquoise, B: Grey, A: Yellow, F: Lime, G: Emerald, K: Green, M: Teal, L: Sage, T: Red, Y: Mauve, TTS: Maroon, Ae: Orange, W: Amethyst, D: Ocher, N: Indigo |
| Bacterium Aurasus | star | O: Turquoise, B: Grey, A: Yellow, F: Lime, G: Emerald, K: Green, M: Teal, L: Sage, T: Red, Y: Mauve, TTS: Maroon, Ae: Orange, W: Amethyst, D: Ocher, N: Indigo |
| Bacterium Bullaris | element | Antimony: Cobalt, Polonium: Yellow, Ruthenium: Aquamarine, Technetium: Gold, Tellurium: Lime, Yttrium: Red |
| Bacterium Cerbrus | star | O: Turquoise, B: Grey, A: Yellow, F: Lime, G: Emerald, K: Green, M: Teal, L: Sage, T: Red, Y: Mauve, TTS: Maroon, Ae: Orange, W: Amethyst, D: Ocher, N: Indigo |
| Bacterium Informem | element | Antimony: Red, Polonium: Lime, Ruthenium: Gold, Technetium: Aquamarine, Tellurium: Yellow, Yttrium: Cobalt |
| Bacterium Nebulus | element | Antimony: Magenta, Polonium: Gold, Ruthenium: Orange, Technetium: Cyan, Tellurium: Green, Yttrium: Cobalt |
| Bacterium Omentum | element | Cadmium: Lime, Mercury: White, Molybdenum: Aquamarine, Niobium: Peach, Tungsten: Blue, Tin: Red |
| Bacterium Scopulum | element | Cadmium: White, Mercury: Peach, Molybdenum: Lime, Niobium: Red, Tungsten: Aquamarine, Tin: Mulberry |
| Bacterium Tela | element | Cadmium: Gold, Mercury: Orange, Molybdenum: Yellow, Niobium: Magenta, Tungsten: Green, Tin: Cobalt |
| Bacterium Verrata | element | Cadmium: Peach, Mercury: Red, Molybdenum: White, Niobium: Mulberry, Tungsten: Lime, Tin: Blue |
| Bacterium Vesicula | element | Antimony: Cyan, Polonium: Orange, Ruthenium: Mulberry, Technetium: Gold, Tellurium: Red, Yttrium: Lime |
| Bacterium Volu | element | Antimony: Red, Polonium: Aquamarine, Ruthenium: Cobalt, Technetium: Lime, Tellurium: Cyan, Yttrium: Gold |
| Cactoida Cortexum | star | O: Grey?, A: Green, F: Yellow, G: Teal, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, W: Indigo?, D: Turquoise, N: Sage |
| Cactoida Lapis | star | O: Grey, A: Green, F: Yellow, G: Teal, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, W: Indigo, D: Turquoise, N: Sage |
| Cactoida Peperatis | star | O: Grey?, A: Green, F: Yellow, G: Teal, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, W: Indigo?, D: Turquoise, N: Sage |
| Cactoida Pullulanta | star | O: Grey?, A: Green, F: Yellow, G: Teal, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, W: Indigo?, D: Turquoise, N: Sage |
| Cactoida Vermis | star | O: Grey, A: Green, F: Yellow, G: Teal, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, W: Indigo?, D: Turquoise, N: Sage |
| Clypeus Lacrimam | star | B: Maroon, A: Orange, F: Mauve, G: Amethyst, K: Grey, M: Turquoise, L: Teal, Y: Green, D: Lime, N: Yellow |
| Clypeus Margaritus | star | B: Maroon, A: Orange, F: Mauve, G: Amethyst, K: Grey, M: Turquoise, L: Teal, Y: Green, D: Lime, N: Yellow |
| Clypeus Speculumi | star | B: Maroon, A: Orange, F: Mauve, G: Amethyst, K: Grey, M: Turquoise, L: Teal?, Y: Green?, D: Lime?, N: Yellow |
| Concha Aureolas | star | B: Indigo, A: Teal, F: Grey, G: Turquoise, K: Red, L: Orange, Y: Yellow, W: Lime?, D: Green, N: Emerald |
| Concha Biconcavis | element | Antimony: Peach, Polonium: Red, Ruthenium: Orange, Technetium: White, Tellurium: Yellow, Yttrium: Gold |
| Concha Labiata | star | B: Indigo, A: Teal, F: Grey, G: Turquoise, K: Red, L: Orange, Y: Yellow, W: Lime, D: Green, N: Emerald |
| Concha Renibus | element | Cadmium: Red, Mercury: Mulberry, Molybdenum: Peach, Niobium: Blue, Tungsten: White, Tin: Aquamarine |
| Electricae Pluma | element | Antimony: Cobalt, Polonium: Cyan, Ruthenium: Blue, Technetium: Magenta, Tellurium: Red, Yttrium: Mulberry |
| Electricae Radialem | element | Antimony: Cyan, Polonium: Cobalt, Ruthenium: Blue, Technetium: Aquamarine, Tellurium: Magenta, Yttrium: Green |
| Fonticulua Campestris | star | O: Grey, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, Ae: Maroon, W: Indigo?, D: Turquoise, N: Sage |
| Fonticulua Digitos | star | O: Grey?, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, Ae: Maroon?, W: Indigo?, D: Turquoise, N: Sage |
| Fonticulua Fluctus | star | O: Grey?, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher?, TTS: Red, Ae: Maroon?, W: Indigo?, D: Turquoise?, N: Sage |
| Fonticulua Lapida | star | O: Grey, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, Ae: Maroon, W: Indigo?, D: Turquoise, N: Sage |
| Fonticulua Segmentatus | star | O: Grey?, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, Ae: Maroon, W: Indigo?, D: Turquoise, N: Sage |
| Fonticulua Upupam | star | O: Grey?, B: Lime, A: Green, F: Yellow, G: Teal, K: Emerald, M: Amethyst, L: Mauve, T: Orange, Y: Ocher, TTS: Red, Ae: Maroon, W: Indigo, D: Turquoise, N: Sage |
| Frutexa Acus | star | O: Yellow?, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange, D: Indigo, N: Red |
| Frutexa Collum | star | O: Yellow, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange?, D: Indigo, N: Red |
| Frutexa Fera | star | O: Yellow?, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange?, D: Indigo, N: Red |
| Frutexa Flabellum | star | O: Yellow, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange, D: Indigo, N: Red |
| Frutexa Flammasis | star | O: Yellow?, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange?, D: Indigo, N: Red |
| Frutexa Metallicum | star | O: Yellow?, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange?, D: Indigo, N: Red |
| Frutexa Sponsae | star | O: Yellow?, B: Lime, F: Green, G: Emerald, M: Grey, L: Teal, TTS: Mauve, W: Orange?, D: Indigo, N: Red |
| Fumerola Aquatis | element | Cadmium: Green, Mercury: Yellow, Molybdenum: Cyan, Niobium: Gold, Tungsten: Cobalt, Tin: Orange |
| Fumerola Carbosis | element | Cadmium: Orange, Mercury: Magenta, Molybdenum: Gold, Niobium: Cobalt, Tungsten: Yellow, Tin: Cyan |
| Fumerola Extremus | element | Cadmium: Aquamarine, Mercury: Lime, Molybdenum: Blue, Niobium: White, Tungsten: Mulberry, Tin: Peach |
| Fumerola Nitris | element | Cadmium: White, Mercury: Peach, Molybdenum: Lime, Niobium: Red, Tungsten: Aquamarine, Tin: Mulberry |
| Fungoida Bullarum | element | Antimony: Red, Polonium: Mulberry, Ruthenium: Magenta, Technetium: Peach, Tellurium: Gold, Yttrium: Orange |
| Fungoida Gelata | element | Cadmium: Cyan, Mercury: Lime, Molybdenum: Mulberry, Niobium: Green, Tungsten: Orange, Tin: Red |
| Fungoida Setisis | element | Antimony: Peach, Polonium: White, Ruthenium: Gold, Technetium: Lime, Tellurium: Yellow, Yttrium: Orange |
| Fungoida Stabitis | element | Cadmium: Blue, Mercury: Green, Molybdenum: Magenta, Niobium: White, Tungsten: Peach, Tin: Orange |
| Osseus Cornibus | star | O: Yellow?, A: Lime, F: Turquoise, G: Grey, K: Indigo, T: Emerald, Y: Maroon, TTS: Green |
| Osseus Discus | element | Cadmium: White, Mercury: Lime, Molybdenum: Peach, Niobium: Aquamarine, Tungsten: Red, Tin: Blue |
| Osseus Fractus | star | O: Yellow?, A: Lime, F: Turquoise, G: Grey, K: Indigo, T: Emerald, Y: Maroon, TTS: Green |
| Osseus Pellebantus | star | O: Yellow?, A: Lime, F: Turquoise, G: Grey, K: Indigo, T: Emerald, Y: Maroon, TTS: Green |
| Osseus Pumice | element | Antimony: White, Polonium: Peach, Ruthenium: Gold, Technetium: Lime, Tellurium: Green, Yttrium: Yellow |
| Osseus Spiralis | star | O: Yellow, A: Lime, F: Turquoise, G: Grey, K: Indigo, T: Emerald, Y: Maroon, TTS: Green |
| Recepta Conditivus | element | Antimony: Lime, Polonium: White, Ruthenium: Yellow, Technetium: Aquamarine, Tellurium: Cyan, Yttrium: Green |
| Recepta Deltahedronix | element | Cadmium: Lime, Mercury: Cyan, Molybdenum: Gold, Niobium: Mulberry, Tungsten: Red, Tin: Orange |
| Recepta Umbrux | star | O: Indigo!, B: Turquoise, A: Amethyst, F: Mauve, G: Orange, K: Red, M: Maroon, L: Ocher, T: Teal, Y: Lime, TTS: Sage, Ae: Grey, D: Yellow, N: Emerald |
| Stratum Araneamus | star | B: Emerald?, A: Emerald?, F: Emerald, N: Emerald? |
| Stratum Cucumisis | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red, D: Mauve |
| Stratum Excutitus | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red, D: Mauve |
| Stratum Frigus | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red?, D: Mauve |
| Stratum Laminamus | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal?, W: Red, D: Mauve |
| Stratum Limaxus | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red?, D: Mauve |
| Stratum Paleas | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red, D: Mauve |
| Stratum Tectonicas | star | F: Emerald, K: Lime, M: Green, L: Turquoise, T: Grey, Y: Indigo, TTS: Amethyst, Ae: Teal, W: Red, D: Mauve |
| Tubus Cavas | star | O: Green?, B: Emerald, A: Indigo, F: Grey, G: Red, K: Maroon, M: Teal, L: Turquoise, T: Mauve, TTS: Ocher, W: Lime?, D: Yellow, N: Amethyst |
| Tubus Compagibus | star | O: Green?, B: Emerald, A: Indigo, F: Grey, G: Red, K: Maroon, M: Teal, L: Turquoise, T: Mauve, TTS: Ocher, W: Lime, D: Yellow, N: Amethyst |
| Tubus Conifer | star | O: Green?, B: Emerald, A: Indigo, F: Grey, G: Red, K: Maroon, M: Teal, L: Turquoise, T: Mauve, TTS: Ocher, W: Lime?, D: Yellow, N: Amethyst |
| Tubus Rosarium | star | O: Green, B: Emerald, A: Indigo, F: Grey, G: Red, K: Maroon, M: Teal, L: Turquoise, T: Mauve, TTS: Ocher, W: Lime?, D: Yellow, N: Amethyst |
| Tubus Sororibus | star | O: Green?, B: Emerald, A: Indigo, F: Grey, G: Red, K: Maroon, M: Teal, L: Turquoise, T: Mauve, TTS: Ocher, W: Lime?, D: Yellow, N: Amethyst |
| Tussock Albata | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange, D: Maroon, N: Yellow? |
| Tussock Capillum | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Caputus | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Catena | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Cultro | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange, D: Maroon, N: Yellow? |
| Tussock Divisa | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Ignis | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange, D: Maroon, N: Yellow? |
| Tussock Pennata | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange, D: Maroon, N: Yellow? |
| Tussock Pennatis | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Propagito | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Serrati | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Stigmasis | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, Ae: Amethyst!, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Triticum | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |
| Tussock Ventusa | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange, D: Maroon, N: Yellow? |
| Tussock Virgam | star | F: Yellow, G: Lime, K: Green, M: Emerald, L: Sage, T: Teal, Y: Red, W: Orange?, D: Maroon, N: Yellow? |


### 2.9 Notable disagreements between BioScan and SrvSurvey

Most conditions agree (58 species HIGH on conditions). The material differences found by the automated cross-check
(`crosscheck_differences` in `species.json`) are:

| Species | Difference | Handling in `conditions` |
|---|---|---|
| Clypeus Lacrimam / Margaritus / Speculumi | BioScan splits by body (Lacrimam & Speculumi rocky, Margaritus HMC); SrvSurvey allows both for all three; SrvSurvey adds T ≥ 190 K | union (R + HMC) |
| Concha Renibus | BioScan water ruleset allows 0.65 g; SrvSurvey caps 0.28 g | union; treat >0.28 g as low confidence |
| Frutexa Acus / Fera / Flabellum / Flammasis / Sponsae / Metallicum | SrvSurvey also lists the other of Rocky/HMC; Flabellum also CO2 | union |
| Fonticulua Fluctus / Segmentatus | SrvSurvey adds rocky-ice bodies (Fluctus also CO2-rich) | union |
| Fonticulua Digitos, Tussock Pennata / Virgam, Frutexa Sponsae | BioScan gravity caps much lower (0.07 / 0.09 / 0.065 / 0.056 g) than SrvSurvey (0.28 g) | union (max) |
| Bacterium Alcyoneum / Nebulus / Aurasus / Omentum / Verrata / Vesicula | body/atmosphere/gravity lists differ in the extra cases (rocky-ice, argon-rich, nitrogen, CO2 for Verrata …) | union |
| Recepta (all three) | atmospheres differ (SrvSurvey: Ammonia for Umbrux, CO2-rich/O2 for Deltahedronix); all need SO2 in the mix | union + `special` |
| Stratum Excutitus / Frigus / Limaxus / Tectonicas | SrvSurvey adds Oxygen (Tectonicas: BioScan adds Argon); Limaxus/Excutitus gravity caps 0.40/0.48 vs 0.49/0.50 | union |
| Bark Mounds | BioScan requires volcanism; SrvSurvey data includes bodies without volcanism | `volcanism: any` |
| Fonticulua Segmentatus, Tussock Ventusa | BioScan "no volcanism"; SrvSurvey also saw minor N2 magma / minor CO2 geysers | list incl. `none` |
| Lividum Brain Tree | BioScan requires volcanism; SrvSurvey includes none | list incl. `none` |
| Crystalline Shards | BioScan: no or thin (Ar/CO2/He/CH4/Ne) atmospheres, ≥12,000 ls, gravity ≤2 g; SrvSurvey: airless only, ≥10,000 ls, gravity ≤1.48 | union |
| Horizons-era organics | BioScan does not constrain atmosphere; SrvSurvey requires none; tubers/anemones gravity & temperature caps differ | constrained side taken |

Use `rulesets` (BioScan) as the primary predictor and SrvSurvey's envelope as a "possible" widening; keep a per-species
`certainty` so the UI can show "likely" vs "possible".

### 2.10 Prediction pipeline (recommended)

1. **On `Scan`** of a landable body: evaluate every species' `rulesets` (atmosphere type, body class, g, T, P, volcanism,
   composition), then system-level tests (stars present, primary/parent star classes and luminosity, region id, nebula
   distance, Guardian bubble, tuber zone, other body types in system, distance from arrival). Use `conditions` envelopes
   first as a cheap filter.
2. **On `FSSBodySignals`**: keep bodies with biological signals; show genus groups with min–max value (×5 if first-logged is
   possible per §2.2).
3. **On `SAASignalsFound.Genuses`**: discard genera not listed; within a genus keep species that passed step 1
   (Odyssey: exactly one species per genus per body).
4. **Colour**: star-coloured → parent-star class (§2.5); material-coloured → element in `Scan.Materials`. Mark variants not yet in
   the commander's codex for the current region (`CodexEntry.IsNewEntry` history) like BioScan/SrvSurvey.
5. **On `ScanOrganic`** (Log): the species/variant is now known; lock value; track sampling (§2.11).
6. **On `SellOrganicData`**: reconcile `Value`/`Bonus` against predictions (feedback loop for rule accuracy).

### 2.11 Sampling assistance

- Position: `Status.json` (`Latitude`, `Longitude`, `Altitude`, `Heading`, `PlanetRadius`, `BodyName`) — written continuously
  while near/on a body.
- Distance between two points: haversine with the body radius (BioScan):
  `a = sin²(Δφ/2) + cos φ1 · cos φ2 · sin²(Δλ/2)`, `d = 2·R·atan2(√a, √(1−a))`, `R = PlanetRadius` (m).
- Record the lat/long at each `ScanOrganic` `Log` and `Sample`. The next sample is valid when the distance to **every** previous
  sample of that species is ≥ the genus colony range (Log → Sample → Analyse; 3 samples total).
- Starting a different species before finishing the current one discards the incomplete samples (BioScan README).
- Composition-scanner (SRV) `CodexEntry` events give positions of other plants → waypoints; SrvSurvey notes recorded positions
  are the commander's position (inside the ship there is an offset for large ships).
- Useful warnings: gravity ≥ 1 g ("high") and ≥ 2.7 g (cannot disembark on foot), BioScan conventions.


---

## 3. Existing tools — feature inventory

### 3.1 Observatory Core — Explorer plugin built-in criteria (MIT, `ObservatoryExplorer/DefaultCriteria.cs`, `Worker.cs`)

All thresholds are exact from the source. "Default" = enabled in `Worker.cs` default settings. HIGH.

| Criterion (setting) | Default | Exact rule (Scan event fields) |
|---|:---:|---|
| Landable & Terraformable | on | `Landable` and `TerraformState` non-empty → "Landable and {TerraformState}" |
| Landable w/ Ring | on | `Landable` and `Rings.Count > 0` |
| Landable w/ Atmosphere | on | `Landable` and `Atmosphere` non-empty |
| Landable High-g | on | `Landable` and `SurfaceGravity > 29.4` m/s² (≈3 g) |
| Landable Large | on | `Landable` and `Radius > 18,000,000` m |
| Close Orbit | on | not a ring, and `parent.Radius × 3 > SemiMajorAxis` |
| Shepherd Moon | on | not a ring, parent has rings, last (outermost) ring `OuterRad > SemiMajorAxis`, and that ring is not a belt |
| Close Ring Proximity | on | for each parent ring: `min(abs(SMA − OuterRad), abs(InnerRad − SMA)) − Radius < 10 × Radius` (reports ring or belt) |
| Wide Ring | on | non-belt ring with `OuterRad − InnerRad > 5 × body Radius` |
| Small Object | on | planet (not star/barycentre) with `Radius < 300,000` m |
| High Eccentricity | on | `Eccentricity > 0.9` (prefixed "Landable with" if landable) |
| Nested Moon | on | `Parents[0]` and `Parents[1]` are both planets (moon of a moon) |
| Fast Rotation | on | not tidally locked, not a star/ring, `abs(RotationPeriod) < 28,800` s (8 h) |
| Close Binary | on | `Parents[0]` is a barycentre (`Null`), exactly one sibling, and both have `Radius / SemiMajorAxis > 0.4` |
| Colliding Binary | on | close-binary pair with `R1 + R2 ≥ a1(1 − e1) + a2(1 − e2)` |
| Fast Orbit | off | `abs(OrbitalPeriod) < 28,800` s (8 h), not a ring |
| High-Value Body | off | `PlanetClass` ∈ {Earthlike body, Ammonia world, Water world} or `TerraformState` non-empty; detail "Undiscovered"/"Unmapped"/"Terraformable", distance |
| Diverse Life | off | `FSSBodySignals` biological count `> 7` |
| Good FSD Injection | off | `Landable` and the body has 5 of the 6 premium-injection materials {Carbon, Germanium, Arsenic, Niobium, Yttrium, Polonium} (only 5 can occur on one body) |
| All FSD Mats In System ("green") | off | the 6 premium materials are all present across the system's bodies (system-wide notification, once) |
| All Surface Mats In System ("gold") | off | all 25 surface raw materials (Sb, As, Cd, C, Cr, Ge, Fe, Mn, Hg, Mo, Ni, Nb, P, Po, Ru, Se, S, Tc, Te, Sn, W, V, Y, Zn, Zr) present across the system |
| Uncommon Secondary Star | off | `BodyID > 0`, is a star, `DistanceFromArrivalLS > 10`, and `StarType` not in {O, B, A, F, G, K, M, L, T, Y, TTS} (names giants, Herbig, WR, carbon, S/MS, WD, NS, BH, exotic) |
| Codex Discoveries | off | `CodexEntry` with `IsNewEntry` → "New {Category} Codex Entry" |
| Custom Criteria | off | user Lua file `ObservatoryCriteria.lua` (`CustomCriteriaManager`), with hooks for Scan, FSSDiscoveryScan, FSSAllBodiesFound, SAASignalsFound and FSDJump |

### 3.2 Observatory Botanist (core plugin, MIT)

Lists biological signals per body after DSS, tracks sampling status, optional sampler-status overlay ("sticky until sampling
is complete"), colony-distance table (§2.3), journal-locale-agnostic genus mapping including the `Clepeus` misspelling.

### 3.3 BioInsights (MattG, third-party Observatory plugin; closed source — features from observatory.xjph.net docs and the forum thread)

- Detects biological signals during FSS and **predicts** likely genera/species/variants from the body's and its stars'
  characteristics (docs); users describe it as checking atmosphere and surface composition as planets are scanned. Exact
  rule set not public (closed source); the BioScan README says the two tools are of similar accuracy.
- Grid columns: Body, Sigs (bio count, corrected after DSS), Flags (high biological value, new codex entry), Genus/Species
  (genus fixed on DSS, species once scanned), Seen (composition scanner or genetic sampler; a cross = predicted but not present),
  Samples, Type (variant depends on star class vs. planetary material), Possible Variants (with the responsible star/material in
  brackets), Base Value (without first-discovery bonus; body total shown as a narrowing range), Distance (clonal colony range).
- Marking of unscanned biologicals (blue diamond by default), "Read All" historic journal import for accurate
  undiscovered-signal reporting, valuable-body settings (a "Minimum Base Value" setting is mentioned on the forum).

### 3.4 Evaluator (MattG, third-party Observatory plugin; closed source)

- Columns: System, Body, Type, Distance (Ls), Gravity, Terraformable, Current Value, Potential Max Value (if DSS'd), Map
  (worth mapping), Order (efficient mapping order), Done, Total Value.
- Settings: Hide Bodies Below [Cr]; Minimum Mapping Value [Cr]; Maximum Distance per class (ELW / WW / Atmospheric / Other);
  Maximum systems after Read All; toggles: atmospherics worth mapping, atmospherics *with bios* worth mapping (they ignore the
  minimum-value rule and have their own distance limit), notifications for bodies worth mapping, efficient-route calculation
  and notifications, persistent "bodies still to map" notification.

### 3.5 Exploration Buddy (EDEB, panostrede.de, Windows, v2.7.9; closed source)

- Route view per system: primary class, distance, scoopable stars, exploration status, EDSM value, counts of valuable /
  terraformable / landable planets, EDSM discoverer; route locking/export/import, Spansh plotter import, next system copied to
  clipboard; "Surroundings" (25 closest systems).
- Body view: body count, type, distance, bio/geo signal counts, terraform state, value, DSS status (colour-coded who mapped
  first), atmosphere/temperature/gravity for landables, first-discovery flags, touchdown tracking, ring info and inclination.
- Biologicals: genus/species, scan progress, clonal colony range with live inside/outside indicator, Vista value (base + bonus),
  variants read from the journal, species prediction from Canonn statistics (atmosphere, body class, gravity, temperature,
  volcanism, distance to primary, star class) narrowed after DSS, species seen < 3 times ignored.
- Planets of Interest: user criteria sets (radius, gravity, ring width/total width/density/type/reserve, inclination, parent
  planet section, …) with icon + speech; voice announcements with templates; HUD overlay; trip history with estimated
  cartographic + Vista value (since 1.5.9 own cartographic formula, "first discovery bonus as a range" since 2.3.0).

### 3.6 SrvSurvey (njthomson, GPL-3.0)

- Exobiology: per-system bio overlay (genus blocks coloured by predicted value bands <3M / <7M / <12M / >12M, min/max bars),
  per-body overlay after DSS (signals, analysed count, never-scanned flags for galaxy/region, Canonn first-discovery starburst),
  scan-progress overlay with required distance, radar with sample circles that change colour once far enough, composition
  scanner trackers (`+name`/`-name` chat commands, genus prefixes with default radii), Canonn prior-scan locations and a
  navigation table to them, stock images, predictions via the `bio-criteria` JSON trees (Canonn hit counts), first-footfall
  tracking (incl. screen-colour inference).
- Exploration: real-time reward estimate (MattG formula, honk bonus, 10k full-DSS bonus), Extended FSS info list (value, DSS
  value incl. efficiency, `*` first discovered, `T` terraformable, `L` landable; hides < 10k unless bios), body info panel,
  Galaxy-map preview (EDSM/Spansh/Canonn data), FSD jump info (route progress, scoopable / neutron highlighting, traffic).
- Also: ground-target guidance (lat/long), Guardian ruins/structure mapping, Ram Tah helpers, human settlement maps, spherical
  search, screenshot processing, colonisation helpers, Codex bingo.

### 3.7 EDMC-BioScan / EDMC-Pioneer (Silarn, GPL-2.0)

- BioScan: predictions for Odyssey + Horizons organics with min/max value per genus and full species/variant breakdown, codex
  "never seen in region / galaxy" markers, scan progress with live distance to previous samples, composition-scanner waypoints
  with bearing, radar overlay, unsold-value tracking, EDSM fetch, journal import.
- Pioneer: per-body and system exploration value (current/max, min–max for terraformables), honk value on the main star,
  "Fully Scanned" (1k/body) and "Fully Mapped" (10k/planet) bonuses, efficiency flags, carrier-sale values (75 % / 12.5 %),
  map counter.

### 3.8 EDMC-Canonn POI checks (GPL-3.0, `canonn/codex.py`) — extra "interesting" ideas

Hot landable (> 1500 K), synchronous orbit (rotation = orbital period, tidally locked; ELW-around-star special case),
helium-rich gas giant (He ≥ 30 %), predicted green gas giant, shepherd moon, close bodies / close binaries, close rings,
trojans, ringed star, odd biology, high value (MattG k-table), satellite star (star named like a planet/moon), deeply nested
(≥ 3 planet parents), green system (all 8 jumponium elements in system), per-body jumponium (with `+v` volcanism / `+b` bio
modifiers, suppressed if the commander already holds ≥ 150 of that material), large / fast / racing / "Taylor" rings.
UnixplorationBuddy (MIT Rust port of Canonn logic) uses: close orbit = apoapsis < 2 × (body radius + parent radius);
close flypast = sibling orbits within 100 km and synodic period < 40 days (collision if orbits overlap); trojan;
satellite star; ≥3-level nesting; fast rotator < 1 h; retrograde orbit (inclination > 90°); eccentricity > 0.9; axial tilt > 90°.

### 3.9 Suggested Elite Nexus default alert set (synthesis)

Start from Observatory's default-on list (§3.1), add: Diverse Life (> 7 bio signals), high-value / worth-mapping by
*potential* value (FD/FM-aware, §1.4, with per-class distance limits like Evaluator), landable atmospheric with bios,
first-footfall-eligible bios (§2.2), new codex variant for the region, jumponium (per body and system, §4), uncommon
secondary star, and Canonn's hot-landable / helium-rich / trojan / flypast checks.


---

## 4. Surface materials — "jumponium" (FSD injection synthesis)

### 4.1 Recipes (HIGH — EDDiscovery `RecipesSynthesis.cs`, EDMC-Canonn `codex.py`, UnixplorationBuddy)

| Synthesis | Jump-range boost | Materials (1 each) |
|---|---:|---|
| FSD Injection — Basic | +25 % | Carbon, Vanadium, Germanium |
| FSD Injection — Standard | +50 % | Carbon, Vanadium, Germanium, Cadmium, Niobium |
| FSD Injection — Premium | +100 % | Carbon, Germanium, Niobium, Arsenic, Polonium, Yttrium |

The boost percentages are from UnixplorationBuddy's module docs and long-standing community knowledge (MEDIUM-HIGH); the
recipes themselves are HIGH.

The jumponium set used by EDDiscovery (`IsJumponium`) and EDMC-Canonn: **Arsenic, Cadmium, Carbon, Germanium, Niobium,
Polonium, Vanadium, Yttrium** (the user list plus Carbon, which is grade 1 and nearly ubiquitous).

Raw-material grades (EDDiscovery `MCMRType.cs`, HIGH): grade 1 (very common) C, P, S, Fe, Ni, Re, Pb; grade 2 (common) V, Cr, Mn,
Zn, Ge, As, Zr; grade 3 (standard) Nb, Mo, Cd, Sn, W, Hg, B; grade 4 (rare) Y, Tc, Ru, Se, Te, Po, Sb. Jumponium by grade:
C (1), V/Ge/As (2), Cd/Nb (3), Y/Po (4). Storage caps per grade are commonly 300/250/200/150 (MEDIUM); EDMC-Canonn stops
flagging a material once the commander holds ≥ 150.

### 4.2 Reading `Scan.Materials`

`"Materials":[{"Name":"iron","Percent":19.6},{"Name":"sulphur","Percent":18.3},…,{"Name":"polonium","Percent":0.9}]` —
lower-case names (an optional `Name_Localised` may appear), percentages of the surface composition; present for landable
bodies. Match case-insensitively; tools differ on minimum percent (SrvSurvey treats < 0.25 % as absent for bio colour
predictions; Observatory and Canonn use simple presence).

### 4.3 Flags to raise

| Flag | Rule | Source |
|---|---|---|
| Per-body jumponium list | which of As, Cd, C, Ge, Nb, Po, V, Y are present, with % (sort rare first: Y, Po, then Cd, Nb, then As, Ge, V) | EDMC-Canonn, EDDiscovery |
| Basic / Standard / Premium on one body | body has all materials of the recipe | EDMC-Canonn |
| "Good FSD Injection" | landable body with 5 of the 6 premium materials (6 cannot coexist on one body) | Observatory |
| Green system | all premium materials (Observatory: 6) or all 8 jumponium elements (Canonn) present across the system's bodies | Observatory, Canonn |
| Gold system | all 25 surface raw materials present across the system | Observatory |
| Modifiers | `+v` body has volcanism (geological sites → richer material collection), `+b` airless body with bio signals | EDMC-Canonn |
| Suppression | skip a material the commander already holds at/near the cap (Canonn threshold 150) | EDMC-Canonn |

Note the overlap with exobiology: the element-coloured species (§2.5) use the same `Materials` list (Cd, Hg, Mo, Nb, Sn, W or
Sb, Po, Ru, Tc, Te, Y), so one material parse feeds both features.

---

## 5. Open questions and confidence summary

| Topic | Confidence | Note |
|---|---|---|
| Star/planet base formula, k table, q, mapping multipliers, 2.6 FD, 1.25 efficiency, +30 %/555 Odyssey, min 500 | HIGH | four implementations share the constants; MattG's reference table is reproduced exactly |
| Order of the 500-minimum (after mapping) | MEDIUM-HIGH | MattG code and table, SrvSurvey, Pioneer; EDDiscovery and UnixplorationBuddy clamp before mapping |
| Odyssey bonus not applied to `WasDiscovered=false, WasMapped=true` bodies | MEDIUM | MattG limited testing; Pioneer implements |
| Honk bonus (1/3 vs 0.3 for stars), FSS-complete 1k/body, full-map 10k/body | MEDIUM / LOW | tools disagree on details; SrvSurvey leaves 1k unconfirmed |
| Supermassive black hole value | LOW | 261,790 (Pioneer) vs formula with k = 33.5678 (EDDiscovery) |
| Terraformable partial bonus | MEDIUM | no journal field exposes the fraction |
| Exobiology base values | HIGH | 4 sources identical; Biconcavis 19,010,800 MEDIUM |
| First-logged ×5 | HIGH (multiplier) / MEDIUM (eligibility) | eligibility = not footfalled + unpopulated + not WasLogged, per BioScan/SrvSurvey |
| "Update 15 rebalance" | not supported | values date from Update 14 (2022-11-29) / 14.01 (2022-12-07) |
| Colony ranges | HIGH | 4 sources (Radicoida 15 m LOW) |
| Species conditions | per species | 58 HIGH, 60 MEDIUM, 8 LOW (Thargoid entries) on conditions; overall 57 HIGH / 68 MEDIUM / 1 LOW |
| Colour variant mapping | HIGH for observed keys | 0 conflicts BioScan vs Canonn codex; "which star" choice is heuristic (parent star vs brightest star) |
| Region map | HIGH | klightspeed EliteDangerousRegionMap data (used by ExploData) |
| Horizons-era organic sellability | HIGH | Canonn rewards + EDDiscovery values for all 27 |
| Space-borne life sellability | HIGH (not sellable) | Canonn reward null for all 17 groups |

Recommended follow-ups before shipping: (1) validate predictions against the commander's own `SellOrganicData` history;
(2) source nebula coordinates (EDAstro CSV) and the region RLE map, checking their licences; (3) re-pull the Canonn codex
reference periodically (new species such as Radicoida appear without tool updates).

---

## 6. Sources

Code (read verbatim):

- EDDiscovery EliteDangerousCore — `EliteDangerous/FrontierData/Enumerations/EstimatedValues.cs`, `EliteReleaseDates.cs`,
  `JournalStatus/Events/JournalScan.cs`, `JournalScan_Helpers.cs`, `Entry/JournalEntry.cs`, `FrontierData/Items/RecipesSynthesis.cs`,
  `FrontierData/Items/MCMRType.cs` — https://github.com/EDDiscovery/EliteDangerousCore (Apache-2.0)
- Observatory Core — `ObservatoryExplorer/DefaultCriteria.cs`, `ExplorerSettings.cs`, `Worker.cs`, `Explorer.cs`,
  `ObservatoryBotanist/Botanist.cs` — https://github.com/Xjph/ObservatoryCore (MIT)
- EDMC-BioScan — `src/bio_scan/bio_data/rulesets/*.py`, `species.py`, `regions.py`, `codex.py`, `body_data/util.py`, `load.py`,
  `README.md` — https://github.com/Silarn/EDMC-BioScan (GPL-2.0)
- EDMC-ExploData — `bio_data/genus.py`, `species.py`, `codex.py`, `journal_parse.py`, `RegionMap.py`, `RegionMapData.py` —
  https://github.com/Silarn/EDMC-ExploData (GPL-2.0)
- EDMC-Pioneer — `src/pioneer/body_calc.py`, `src/load.py` — https://github.com/Silarn/EDMC-Pioneer (GPL-2.0)
- SrvSurvey — `SrvSurvey/bio-criteria/*.json` + `readme.md`, `BioPredictor.cs`, `Util.cs`, `game/SystemData.cs`, `game/Game.cs`,
  `net/CodexRef.cs` — https://github.com/njthomson/SrvSurvey (GPL-3.0)
- EDMC-Canonn — `canonn/codex.py` — https://github.com/canonn-science/EDMC-Canonn (GPL-3.0)
- UnixplorationBuddy — `src/model/valuation.rs`, `anomaly*.rs` — https://github.com/ArnarValur/UnixplorationBuddy (MIT)
- Journal schemas (Scan `WasFootfalled`, ScanOrganic `WasLogged`, SellOrganicData, SAAScanComplete, …) —
  https://github.com/jixxed/ed-journal-schemas

Data and documentation:

- Canonn codex reference API (live, 2026-10-12): https://us-central1-canonn-api-236217.cloudfunctions.net/query/codex/ref
- Canonn codex pages: https://canonn.science/codex/concha/ , https://canonn.science/codex/radicoida/
- MattG, "Exploration value formulae" (first post, updated Sep 2022): https://forums.frontier.co.uk/threads/exploration-value-formulae.232000/
- Frontier, Elite Dangerous Update 14.01 notes (7 Dec 2022): https://www.elitedangerous.com/news/elite-dangerous-update-1401
- Fandom: https://elite-dangerous.fandom.com/wiki/Vista_Genomics , https://elite-dangerous.fandom.com/wiki/Exobiology_Sample_Values_and_Details ,
  https://elite-dangerous.fandom.com/wiki/Exobiologist , https://elite-dangerous.fandom.com/wiki/Radicoida
- Observatory docs: https://observatory.xjph.net/usage/plugins/explorer , https://observatory.xjph.net/usage/plugins/thirdparty/bioinsights ,
  https://observatory.xjph.net/usage/plugins/thirdparty/evaluator
- MattG's Observatory plugins thread (BioInsights/Evaluator): https://forums.frontier.co.uk/threads/mattgs-observatory-plugins.589466/
- Exploration Buddy: https://www.panostrede.de/EDEB/ and https://www.panostrede.de/EDEB/changelog.html
- SrvSurvey wiki: https://github.com/njthomson/SrvSurvey/wiki , `/wiki/Organic-Scanning`, `/wiki/Exploration`
- Nebula coordinates used by SrvSurvey: https://edastro.com/mapcharts/files/nebulae-coordinates.csv
