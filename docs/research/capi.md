# Frontier Companion API (CAPI) and Frontier Auth: implementation spec for Elite Nexus

Research date: **2026-10-12**. Status: implementation-ready, with the unverified items marked **[UNVERIFIED]**.

## 0. Sources and verification

- **Frontier's own OAuth2 notes (public):** https://hosting.zaonce.net/docs/oauth2/instructions.html. Updated 2019-01-22. Covers audiences, scopes, `/decode`, `/me` and the T&C FAQ.
- **EDCD canonical CAPI docs:** https://github.com/EDCD/FDevIDs/tree/master/Frontier%20API. Last commit 2025-12-11. Files:
  - `FrontierDevelopments-CAPI-endpoints.md`
  - `FrontierDevelopments-oAuth2-notes.md`
  - `README.md` (User-Agent request)
  - `example-auth-decode.json`
  - `FrontierDevelopments-other-APIs.md`

  These superseded Athanasius' https://github.com/Athanasius/fd-api, whose README carries the 2023-03-21 note about `scope=capi` vs `scope=auth capi` and whose `docs/TODO.md` has the `/communitygoals` shape.
- **EDMarketConnector 6.1.2** (`main` at commit `89f410c9`, 2026-09-02):
  - https://github.com/EDCD/EDMarketConnector/blob/main/companion.py
  - `protocol.py`, `constants.py`, `EDMarketConnector.py`, `common_utils.py`, `ChangeLog.md`
  - (There is no `docs/FrontierDevelopmentsAPI.md` in the current EDMC tree; the CAPI docs moved to FDevIDs.)
- **EDDiscovery CAPI library:** https://github.com/EDDiscovery/CAPI, specifically `CAPI/CAPI.cs`, `CAPI/Squadrons.cs` and `CAPI/Credentials.cs`.
- **Live checks on 2026-10-12** (no tokens and no client ID used):
  - `GET https://companion.orerve.net/` returned **401** `{"status":401,"message":"OAuth Authentication Required","tag":"…"}`.
  - `GET https://user.frontierstore.net/developer/docs` returned 200 but shows only a login prompt; the developer docs require login.
  - `GET https://auth.frontierstore.net/client/signup` (the URL in EDMC's source comment) returned **404**.

---

## 1. Obtaining a Client ID (action required by you, the developer)

1. **Apply.** Log in to the Frontier User Portal at https://user.frontierstore.net/ with a Frontier account and **apply for developer access**. The application form's exact fields are not documented anywhere public **[UNVERIFIED]**. The EDCD doc author could not document it either.
2. **Wait for approval.** Frontier approves manually, and the docs stress that it is **not instant**.
3. **Collect the credentials.** After approval, the portal's **Developer Zone** lists your client with **AUTH** and **CAPI** scopes. **View** reveals the **Client ID** and a **Shared Key** (client secret), and you can regenerate the key there.
   - A desktop app uses **PKCE** and needs only the Client ID.
   - The Shared Key is only for confidential (server-side) clients.
4. **Read the developer docs.** They are at https://user.frontierstore.net/developer/docs, after login.
5. **Note the dead link.** EDMC's source comment says "Obtain from https://auth.frontierstore.net/client/signup", but that URL is now 404. Use the User Portal or Developer Zone route instead.

**Terms you accept** (from Frontier's public FAQ at hosting.zaonce.net, paraphrased):

- The **person who obtains the key** accepts Frontier's T&Cs.
- For open-source projects, anybody else running an instance needs their own key.
- **Your API key should not be included in your open-source code.**
- Because API access exposes PII (real names, emails, in-game identities), you must **notify players what information you use and why**.

How existing desktop tools handle the Client ID:

- **EDMC** hard-codes its own Client ID in `companion.py`. It can be overridden with the `CLIENT_ID` environment variable. Do **not** reuse another app's ID.
- **EDDiscovery** keeps the ID out of version control (comment: "we are not allowed to check the client ID into version control or publish it to 3rd parties") and injects it at build time from a `CAPIID` environment variable.

**Recommendation:**

- Inject the ID at build time, e.g. `option_env!("ELITE_NEXUS_CAPI_CLIENT_ID")`.
- Ship it only in official release builds.
- Make CAPI a feature that is disabled when the ID is absent, so forks and dev builds work without it.
- Ask Frontier, when you apply, whether embedding the ID in distributed binaries is acceptable for a desktop app **[UNVERIFIED policy detail]**.

**User-Agent:** Frontier asked that all CAPI and Auth traffic use a custom UA matching `EDCD-[A-Za-z]+-[.0-9]+`, e.g. `EDCD-YourApp-0.0.1`, so they can find misbehaving clients (FDevIDs `README.md`). The fd-api sample config says to use the `EDCD-` prefix "if at all associated with EDCD".

- EDMC sends `EDCD-EDMarketConnector-<semver>`.
- **Decide** between `EDCD-EliteNexus-<ver>` and a non-EDCD `EliteNexus-<ver>`. The safest course is to ask Frontier in your application.

---

## 2. OAuth2 Authorization Code + PKCE flow

**Auth server:** `https://auth.frontierstore.net`.

| Purpose | Method and path |
|---|---|
| Authorization (user's browser) | `GET /auth` |
| Token exchange and refresh | `POST /token` |
| Decode the access token (claims) | `GET /decode` |
| Account details | `GET /me` |

### 2.1 Generate PKCE values and state

- **`code_verifier`:** 32 cryptographically random bytes, base64url-encoded (`-` for `+`, `_` for `/`), **without `=` padding**. That gives a 43-character string.
  - EDMC and EDDiscovery both strip the padding, and it works.
  - The 2019 EDCD note says the verifier must *keep* its `=`; that conflicts with RFC 7636 and with what working clients do. Use no padding.
- **`code_challenge`:** `base64url_nopad( SHA-256( ascii_bytes(code_verifier) ) )`.
  - Use the **binary** digest, not hex.
  - The `=` **must** be stripped, or Frontier answers `{"message":"An error occured.","logref":"<hex>"}`.
  - `code_challenge_method=S256`.
- **`state`:** 32 random bytes, base64url without padding. Keep it in memory and compare it on callback (CSRF protection).

### 2.2 Authorization request (open in the user's default browser; never fetch it yourself)

```
https://auth.frontierstore.net/auth
  ?response_type=code
  &audience=frontier,steam,epic        (EDMC)   | all (default; EDDiscovery)
  &scope=auth%20capi                    (space-separated; BOTH scopes required)
  &client_id=<YOUR_CLIENT_ID>
  &code_challenge=<challenge>
  &code_challenge_method=S256
  &state=<state>
  &redirect_uri=<url-encoded redirect URI>
```

- **`audience`** is a comma-separated list. Frontier's doc lists `frontier` (Frontier store login), `steam`, `xbox`, `psn`, and `all` (the default).
  - EDMC also passes `epic` for Epic Games accounts; that value is newer than the 2019 doc.
  - Use `all`, or `frontier,steam,epic` for PC-only.
- **`scope`:**
  - `auth` (the default) gives email, first name, last name, `thirdpartyuserID` and platform.
  - `capi` gives Companion API access.
  - **Always request `auth capi`.** Since around March 2023, a token minted with `scope=capi` alone lacks the PII claims, and CAPI rejects it with `401 {"status":401,"message":"Authentication token incomplete","missing":["email","firstname","lastname"],…}` (fd-api README).
- **URL-encode the redirect URI.** EDDiscovery uses `Uri.EscapeDataString`; EDMC concatenates the raw value. Encoding is correct.

### 2.3 Callback

The browser is redirected to `<redirect_uri>?code=<code>&state=<state>`. On failure it gets `?error=…&error_description=…` instead.

1. Verify that the URL starts with your redirect URI.
2. Verify that `state` matches, then discard the stored state (single use).
3. Read `code`.
4. Treat the callback URL as **untrusted input**. Any local process can invoke a custom URL scheme.

### 2.4 Token request

```
POST https://auth.frontierstore.net/token
Content-Type: application/x-www-form-urlencoded

grant_type=authorization_code&client_id=<id>&code_verifier=<verifier>&code=<code>&redirect_uri=<same redirect_uri, url-encoded>
```

- **Success (200):** `{"access_token":"…","token_type":"Bearer","expires_in":14400,"refresh_token":"…"}`.
- **Failure:** a bad, expired or reused code, or a bad verifier, returns 401 or the `{"message":"An error occured.","logref":…}` body.
- **Header:** use `Authorization: <token_type> <access_token>`. Store `token_type` rather than hard-coding "Bearer".

### 2.5 Identity check (recommended; EDMC does this)

```
GET https://auth.frontierstore.net/decode
Authorization: Bearer <access_token>
```

Response (claims of the JWE; see `example-auth-decode.json`):

```json
{ "iss": "https://auth.frontierstore.net", "iat": 1566922699, "exp": 1566937099, "scope": "auth capi",
  "usr": { "customer_id": "1234567", "firstname": "…", "lastname": "…", "email": "…", "platform": "frontier",
           "developer": null, "allowedDownloads": [ { "product_sku": "…", "product_name": "…", "product_qty": "1" } ] } }
```

- **EDMC check:** `"F" + usr.customer_id` must equal the journal `FID` from the `Commander`/`LoadGame` events of the commander you are authorising. Otherwise error with "customer_id doesn't match!". This prevents binding the wrong Frontier account to a commander.
- **`/me`** returns flat account info:
  - PC: `{"customer_id","firstname","lastname","email","platform":"frontier","thirdPartyUserId":null,"parent_id":null}`
  - PSN and Xbox also carry `parent_id` and `thirdPartyUserId`.
- **Token format:** the tokens are **JWE** (encrypted). You cannot decode them locally; use `/decode`. Both servers check `exp`.
- **PII handling:** do not persist the PII from these endpoints. EDMC stores and transmits none of it.

### 2.6 Refresh

```
POST https://auth.frontierstore.net/token
Content-Type: application/x-www-form-urlencoded

grant_type=refresh_token&client_id=<id>&refresh_token=<refresh_token>
```

- `client_secret` is needed only for non-PKCE clients.
- **Success (200):** a new `access_token`, a new `expires_in` (**14400 s = 4 h**), and a **new `refresh_token`**.
  - The refresh token **rotates on every refresh**, so always persist the new one atomically.
  - "Refreshed tokens may omit any third party details" (Frontier).
- **Failure:** 401 means the refresh token is expired or revoked, so start a new authorization (§2.2).

### 2.7 Lifetimes and storage

| Item | Value |
|---|---|
| Access token | 4 h (`expires_in` 14400; `exp - iat` = 14400 in both the Frontier and EDCD examples). |
| Refresh token | Rotates on use. **Usable for at most 25 days after the initial authorization**, after which the user must re-authorize. Re-authorizing usually needs only a login, not re-approval (EDCD notes). Exact server policy **[UNVERIFIED]**. |
| Revocation | Revoking the app in the User Portal ("Authorized Applications") stops refreshes (401), but **already-issued access tokens keep working until `exp`**. |

How clients store and refresh:

- **EDMC** stores only the refresh token, per commander (config key `fdev_apikeys`, indexed like `cmdrs`), and keeps the access token in memory. It refreshes on start, and on a CAPI 401 (`CredentialsRequireRefresh`). EDMC 5.13 added cycling through all known CMDRs to keep each one's auth alive.
- **EDDiscovery** stores access token, refresh token and expiry per commander, and refreshes 60 s before expiry.

**Recommendation for Rust:**

- Store `{refresh_token, access_token, expires_at, token_type, customer_id}` per commander FID in the OS credential store (`keyring` crate).
- Refresh proactively at T-60 s and reactively on 401.
- Serialise refreshes per commander with a mutex, because a rotating token plus concurrent refreshes produces a lockout.

---

## 3. Redirect URI options

| Option | Used by | Notes |
|---|---|---|
| **Custom URL scheme** `edmc://auth` | EDMC frozen Windows builds | Needs the `edmc:` protocol registered in the registry by the installer, plus a running-instance hand-off. EDMC uses Windows **DDE** (`WM_DDE_EXECUTE` with `open("edmc://auth?code=…")`) and checks that the URL starts with the redirect. Breaks with multiple instances. |
| **Custom scheme** `<scheme>://auth/` | EDDiscovery (`URI` + `://auth/`) | Same idea. EDD also hands the URL to the running instance via DDE. |
| **Loopback** `http://localhost:<ephemeral-port>/auth` | EDMC on Linux, source runs, Wine, and Windows with `--force-localserver-for-auth` | `HTTPServer(("localhost", 0))`. The redirect is `http://localhost:{port}/auth`; any `GET /auth…` is accepted, a "close this tab" page is returned, and the query is parsed. |

- EDMC uses the **same Client ID** for both `edmc://auth` and `http://localhost:<random port>/auth`.
- That implies Frontier's auth server either does not strictly pin `redirect_uri` to one registered value, or allows any localhost port **[UNVERIFIED; confirm what the Developer Zone asks for when you register]**.
- Whatever you use, the token request must repeat the *same* `redirect_uri`.

**Recommendation for Elite Nexus (Rust):**

- Primary: an RFC 8252 loopback redirect `http://127.0.0.1:<random port>/auth`, or `http://localhost:<port>/auth`, the exact form EDMC proves works.
  - A one-shot `tiny_http`/`hyper` listener with a 5-minute timeout.
  - No registry writes and no single-instance IPC.
  - Works on Windows, Linux and Proton.
- Optional fallback: a custom scheme `elitenexus://auth`, registered under `HKCU\Software\Classes\elitenexus` (`URL Protocol` value, `shell\open\command`). It needs single-instance forwarding (named pipe) to deliver the URL to the running app.
- Register both redirect URIs with Frontier if the Developer Zone allows a list.

---

## 4. CAPI hosts

| Host | Use |
|---|---|
| `https://companion.orerve.net` | **Live** galaxy (4.0+ clients). Since Update 14 (2022-11-29) it returns only Live data. |
| `https://legacy-companion.orerve.net` | **Legacy** galaxy (Horizons 3.8 clients). |
| `https://pts-companion.orerve.net` | Historical beta/PTS host. Used before Update 14 and not currently documented as active **[UNVERIFIED]**. |

Choosing the host (EDMC `capi_host_for_galaxy`):

- Beta → PTS.
- Otherwise `semver(gameversion) >= 4.0.0` → Live.
- Else → Legacy.
- If the beta/live status is unknown, don't query.

Every request carries `Authorization: Bearer <access_token>` and your custom `User-Agent`. Unauthenticated requests get `401 {"status":401,"message":"OAuth Authentication Required","tag":"…"}`, which was observed live.

## 5. Endpoints

`GET /` lists the available endpoints (now auth-only; the EDCD doc shows the listing). Several endpoints accept `?language=en|fr|de|ru|es|pt` to localise `locName`/descriptions: `/profile`, `/market`, `/communitygoals` and `/fleetcarrier`.

### 5.1 `GET /profile`: commander, current ship, fleet, last system and station

| Key | Contents |
|---|---|
| `commander` | `name`, `id`, `docked`, `alive`, `credits`, `debt`, `currentShipId`, `rank` {`combat`, `trade`, `explore`, `cqc`, `empire`, `federation`, `power`, `service`, `crime`}, `capabilities` (account flags such as Horizons ownership). Any newer keys (e.g. on-foot state, Odyssey ranks) are **[UNVERIFIED]**: parse leniently. |
| `lastSystem` | `id` (sometimes the id64, sometimes another id), `name`, `faction`. |
| `lastStarport` | `id` (MarketID), `name`, `faction`, `minorfaction`, `services` {name: `ok`/`unavailable`/`private`}. |
| `ship` | `id` (ship index), `name` (FDev symbol), `shipName`, `shipID`, `value` {`hull`, `modules`, `cargo`, `total`, `unloaned`}, `free`, `station` {`id`, `name`}, `starsystem` {`id`, `name`, `systemaddress`}, `alive`, `health` {`hull`, `shield`, `shieldup`, `integrity`, `paintwork`}, `cockpitBreached`, `oxygenRemaining`, `modules` {slot: {`module` {`id`, `name`, `locName`, `value`, `health`, `on`, `priority`, …}, `engineer` {…}, `WorkInProgress_modifications`, `specialModifications`}}, `launchBays` {slot: {`name`, `locName`, `rebuilds`, `loadout`, `loadoutName`}}. |
| `ships` | All owned ships in the same format as `ship`. **This is either an array or an object keyed by stringified index when there are gaps.** Handle both (EDMC `listify`). |
| Odyssey extras | `suit`, `suits`, `loadout`, `loadouts` (suit loadouts; again array or sparse dict). |

### 5.2 `GET /market`: last docked station, fleet carrier or settlement market

- **Top level:** `id` (MarketID), `name`, `outpostType` (e.g. `starport`, `fleetcarrier`; may differ from the journal's `StationType`), `imported`/`exported`/`prohibited` {commodityId: symbol}, `services` {…}, `economies` {economyId: {`name`, `proportion`}}, and `commodities`.
- **Each `commodities[]` entry:** `id`, `name` (symbol), `legality` ("" = legal), `buyPrice`, `sellPrice`, `meanPrice`, `demandBracket` 0–3, `stockBracket` 0–3, `stock`, `demand`, `statusFlags`, `categoryname`, `locName`.
- **Odyssey extra:** at fleet carriers the market data also carries `orders.onfootmicroresources`, the bartender orders. EDMC reads it from the merged `lastStarport.orders.onfootmicroresources` and sends it to EDDN as `fcmaterials_capi` (`plugins/eddn.py`, `export_capi_fcmaterials`). The inner item shape is **[UNVERIFIED]** here.

### 5.3 `GET /shipyard`: last docked shipyard and outfitting

- **Top level:** `id`, `name`, `outpostType`, `imported`, `exported`, `services`, `economies` and `prohibited`.
- **`modules`:** {moduleId: {`id`, `category`, `name`, `cost`, `sku`, `stock` (-1 = unlimited)}}. It is **`[]` or absent when there are none**, e.g. at a damaged station or fleet carrier.
- **`ships`:** {`shipyard_list`: {name: {`id`, `name`, `basevalue`, `sku`, `stock`}}, `unavailable_list`: […]}.

### 5.4 `GET /fleetcarrier`: the commander's own fleet carrier

| Status | Meaning |
|---|---|
| 200 | Data returned. |
| **204** | The commander owns no carrier. |

Keys:

- `name` {`callsign`, `vanityName` (hex-encoded), `filteredVanityName` (hex-encoded)}
- `currentStarSystem`, `balance`, `fuel` (0–1000 tritium)
- `state` (`normalOperation`, `debtState`, `pendingDecommission`)
- `theme`, `dockingAccess` (`all`, `squadronfriends`, `friends`, `none`), `notoriousAccess`
- `capacity` {`shipPacks`, `modulePacks`, `cargoForSale`, `cargoNotForSale`, `cargoSpaceReserved`, `crew`, `freeSpace`, `microresourceCapacityTotal`, `…Free`, `…Used`, `…Reserved`}
- `itinerary` {`completed` [{`departureTime`, `arrivalTime`, `state`, `visitDurationSeconds`, `starsystem`}], `totalDistanceJumpedLY`, `currentJump`}
- `marketFinances` and `blackmarketFinances` {`cargoTotalValue`, `allTimeProfit`, `numCommodsForSale`, `numCommodsPurchaseOrders`, `balanceAllocForPurchaseOrders`}
- `finance` {`bankBalance`, `bankReservedBalance`, `taxation`, `service_taxation` {…}, `numServices`, `numOptionalServices`, `debtThreshold`, `maintenance`, `maintenanceToDate`, `coreCost`, `servicesCost`, `servicesCostToDate`, `jumpsCost`, `numJumps`, `bartender` {…}}
- `servicesCrew` {service: {`crewMember` {`name`, `gender`, `enabled`, `faction`, `salary`, `avatarStr`, `lastEdit`}, `invoicesWeekToDate`, `status`}}
- `cargo` [{`commodity`, `mission`, `qty`, `value`, `stolen`, `locName`}]
- `orders` {`commodities` {`sales`, `purchases`}, `onfootmicroresources` {`sales` (dict, or `[]` when empty), `purchases`}}
- `carrierLocker` {`assets`, `goods`, `data`: [{`id`, `quantity`, `name`, `locName`}]}
- `reputation` [{`majorFaction`, `score`}]
- `market` (same shape as `/market`), `ships` {`shipyard_list`}, `modules`

**Slow endpoint:** EDMC uses a **60 s timeout** and a **15-minute cooldown**, because large carriers make this query slow.

### 5.5 `GET /journal` and `GET /journal/YYYY/MM/DD`

- **Content:** the authenticated player's journal lines for that day; all sessions are concatenated, one JSON event per line. With no date, today is returned.

  | Status | Meaning |
  |---|---|
  | 200 | Complete. |
  | **204** | No play that day. |
  | **206** | Partial; retry later until you get 200. |
  | 401 | Token problem. |

- **Edge case:** EDDiscovery has seen a 200 with the plain-text body `Journal unavailable`, and a `{}` body. Treat both as no data.
- **Not real-time:** query it after a play session, not live.
- **Game version for EDSM/EDDN:** use `LoadGame.gameversion` from the CAPI journal (Live), because CAPI journals have no `Fileheader`. Use `"CAPI-Legacy-journal"` for Legacy.

### 5.6 `GET /communitygoals`

Active CGs plus the commander's progress. The shape below comes from community notes in Athanasius/fd-api `docs/TODO.md` **[UNVERIFIED, may be stale]**:

```json
{ "activeCommunityGoals": [ { "id": "…", "title": "…", "expiry": "…", "market_name": "…", "starsystem_name": "…",
    "activityType": "…", "target_commodity_list": "…", "target_qty": "…", "qty": "…", "objective": "…",
    "news": "…", "bulletin": "…",
    "commander_progress": { "currentQty": 0, "numContributors": 0, "commanderContributionQty": null,
                            "commanderPercentile": 0, "commanderSubscribed": false, "unixTimeStamp": 0, "complete": false } } ] }
```

### 5.7 `GET /visitedstars`

- **Content:** a **zip** containing the player's `VisitedStarsCache.dat`.
- **Status 102** means the file is being generated in the background; try again in a couple of minutes.
- The `.dat` format is binary and not documented here **[UNVERIFIED]**.

### 5.8 `GET /squadron` (newer, 2025)

This endpoint is in EDDiscovery's CAPI library (`Squadrons.cs`) but not in the EDCD docs **[UNVERIFIED against Frontier docs]**.

- **Top level:** `id`, `name`, `acceptingNewMembers`, `superpowerName`, `factionName`, `factionHomeSystemName`, `factionHomeSystemId`, `memberCount`, `description`, `motto`, `active7Days`, `active30Days`, `perks` {`primary`, `secondary`}.
- **`squadronCarrier`:** same shape as `/fleetcarrier`.
- **`bank`:** {`credits` {`All`: [{`qty`}], `Carrier Balance`: [{`qty`}]}, `commodities` {…}, `microresources` {…}}.
- **`members`:** [{`member_id`, `name` (hex), `joined`, `lastOnline`, `status`, `shipName`, `shipModel`, `rank_id`, `rankCombat`, `rankExplore`, `rankTrade`, `rankCqc`, `soldierRank`, `exobiologistRank`, `empireRank`, `federationRank`, `powerRank`, `userPlayTime`, `lastLocation` (hex), `requestletter` (hex), `biography` (hex)}].

### 5.9 HTTP status semantics

| Code | Meaning | Action |
|---|---|---|
| 200 | OK | – |
| 102 | `/visitedstars` still generating | Retry in minutes. |
| 204 | No content (no FC; no journal for the day) | Not an error. |
| 206 | Partial journal | Retry later. |
| 401 | Token invalid or expired, or not authenticated. Bodies seen: `"JWT has incorrect/unexpected fields"`, `"Authentication token incomplete"`, `"OAuth Authentication Required"`. | Refresh once, then re-auth. |
| 418 | "I'm a teapot", meaning **maintenance** | Back off (minutes); show "Frontier CAPI down for maintenance". |
| 422 | Pre-2019 signal for an expired token | Treat like 401. |
| 429, 5xx | Throttled or server error | EDMC's `urllib3.Retry` uses total=5 with `backoff_factor=0.5` on 429/500/502/503/504. |

- **Empty or non-JSON 200 bodies:** these happen. Treat them as an error and retry later; don't crash.
- **Missing `timestamp`:** if the data has none, derive it from the response `Date` header (EDMC).

## 6. Rate limits and query etiquette

- **Not real-time.** The CAPI is not designed to be polled.
- **Guidance:** issue **no more than about 1 query per minute**. Frontier has said rate limiting **may apply above 2 queries per second**. Burst-querying `/profile` + `/market` + `/shipyard` on docking is accepted practice.
- **No published numeric quota [UNVERIFIED].**

EDMC behaviour to mirror:

- **Auto query on docking:** triggered on `StartUp`/`Location`/`Docked` when docked at a station (not in an Apex taxi), or on `Disembark` from a taxi `OnStation`. The query is delayed by **5 s** (`SERVER_RETRY`).
  1. `/profile`.
  2. If `commander.docked` (or on foot), then `/market` if `services.commodities`, and `/shipyard` if `services.outfitting` or `services.shipyard`.
  3. Merge the results into `lastStarport`.
- **Consistency checks:**
  - Raise "server lagging" if `lastStarport.id` ≠ `market.id`/`shipyard.id`.
  - **Strip a trailing `" +"`** from station names (Frontier bug, issue 40706).
- **Cooldowns and timeouts:**
  - **60 s** between station query sets.
  - Fleet carrier: **15 min** cooldown, **60 s** timeout. Triggered by `CarrierBuy`, `StartUp`, `CarrierLocation`, `CarrierStats`, `CargoTransfer` and `MarketBuy` when enabled.
  - Default request timeout **10 s**.
- **When not to query:** in CQC, in multicrew as crew, or while authentication is pending.
- **Data lag:** CAPI data lags the journal. In an Odyssey taxi, `Docked` precedes the CAPI update, which only happens after `Disembark`.

## 7. Other Frontier endpoints (no auth)

These are from FDevIDs `FrontierDevelopments-other-APIs.md`, dated 2021, **[UNVERIFIED current]**:

- **Community Goals list:** `https://api.orerve.net/2.0/website/initiatives/list?lang=en` (`de`, `en`, `fr`, `es`, `pt`, `ru`). Returns JSON with `Accept: */*`, or XML with `Accept: text/xml`.
- **GalNet:**
  - RSS: `https://community.elitedangerous.com/galnet-rss`
  - JSON:API: `https://cms.zaonce.net/en-GB/jsonapi/node/galnet_article`. Filter by slug with `?filter%5Bfield_slug%5D=<slug>`.

## 8. Rust implementation sketch

- **Crates:**
  - `oauth2` (PKCE with `AuthUrl`/`TokenUrl`; set `AuthType::RequestBody`), or a hand-rolled flow with `sha2` + `base64::URL_SAFE_NO_PAD` + `rand`.
  - `reqwest` (rustls, gzip), `open` (launch the browser), `tiny_http` (loopback).
  - `keyring` (token storage), `serde_json` with lenient `Option` fields, `semver` (galaxy detection).
- **Flow:**
  1. Build the auth URL from §2.2 with `audience=all`, or `frontier,steam,epic`.
  2. Start the loopback listener, then `open::that(url)`.
  3. Await the callback with a timeout and validate `state`.
  4. `POST /token`.
  5. `GET /decode` and check that `"F"+usr.customer_id == journal FID`.
  6. Store the tokens.
  7. Schedule a refresh at `expires_at - 60s`.
- **Query client:**
  - Add the `Authorization` and `User-Agent` headers.
  - Select the host from `gameversion`.
  - Map status codes per §5.9.
  - Normalise `ships`, `modules` and `suits` from arrays or sparse dicts.
  - Hex-decode carrier and squadron vanity names.
- **Persistence:** cache the last raw response per endpoint with its timestamp (EDMC `CAPIDataRaw`) for debugging and for offline UI.

## 9. [UNVERIFIED] / open questions for the developer

- **Registration details:** the Developer Zone application form fields, the approval criteria and time, and whether multiple redirect URIs (custom scheme and loopback) can be registered.
- **Client ID in binaries:** whether embedding the Client ID in a distributed, open-source desktop binary is acceptable under Frontier's current T&Cs. Frontier's FAQ says not to put it in open-source code; EDMC ships its own anyway.
- **Endpoint shapes:** the `/squadron` and `/communitygoals` response shapes, and whether `pts-companion` is still operated.
- **Refresh-token lifetime:** the exact policy (EDCD notes say 25 days from initial auth).
