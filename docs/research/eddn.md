# EDDN (Elite Dangerous Data Network) — implementation spec for Elite Nexus

Research date: **2026-10-12**. Applies to: the uploader (HTTP POST to the EDDN Gateway) and the subscriber (ZeroMQ SUB to the EDDN Relay).

Verbatim copies of every schema, every schema README and `docs/Developers.md` are in
[`eddn-schemas/`](./eddn-schemas/) (list in [§11](#11-files-saved-next-to-this-document)).
**The schema JSON files are the authority.** Where a README and its schema disagree, follow the schema and tell EDCD
(EDDN's own rule, stated in every schema README).

How the facts were checked:

* **[DOC]**: EDDN docs and schemas on GitHub (`EDCD/EDDN`, branches `live`, `master` and `develop`).
* **[CODE-EDDN]**: the EDDN server code on GitHub (Python, `live` branch).
* **[EDMC]**: EDMarketConnector `plugins/eddn.py` and `monitor.py` (branches `main` = 6.1.2 and `develop`). EDMC is the reference uploader and sends roughly half of all EDDN traffic (48 % of the last hour, 58 % of yesterday).
* **[EDDI]**: EDDI's `EDDNResponder` source (`develop` branch, 5.0.x). It is the only sender I found with `outfitting/3` and `scanorganic/1` in its source.
* **[LIVE]**: checked against the production service on 2026-10-12, read-only:
  * the public monitor page at `https://eddn.edcd.io/`;
  * harmless `GET` and `OPTIONS` requests to the gateway (nothing was POSTed);
  * ZeroMQ subscriptions to `tcp://eddn.edcd.io:9500`: about 15,000 messages decoded across six sessions totalling about 29 minutes (some sessions overlapped).

> ⚠️ **Most important finding: GitHub is not what runs in production.** In October 2026 the live EDDN service is a
> **new gateway/relay implementation that is not in the EDCD GitHub org** (a search of the org's repositories found nothing). The evidence is all [LIVE]:
>
> * a new monitor page with VALID/WARNING/FAILED/"Signed" counters;
> * the gateway reports 11.8 MB of memory use;
> * `GET /upload/` answers `400 Empty request body`;
> * CORS allows an **`X-Signature`** request header;
> * the relay is a ZMTP 3.1 `XPUB` socket;
> * relayed `uploaderID` values are 64-hex, where the old Python relay produced 40-hex SHA-1 values.
>
> Production also **already accepts `outfitting/3` and `scanorganic/1`**, which are absent from the GitHub `live` branch and its
> `Settings.py`. The protocol and schemas are unchanged for senders. The exact error strings, size limits and
> duplicate-handling of the new gateway could only be inferred, because no POST was made (see [§9](#9-not-verified--open-questions)).

---

## 0. TL;DR — the rules that keep an uploader from being blocked

1. `POST https://eddn.edcd.io:4430/upload/`
   * HTTPS only, on port 4430, with the trailing `/`. Use HTTP/1.1 (the docs say HTTP/2 is not supported).
   * The body is **one** UTF-8 JSON object per request, gzip-compressed, sent with `Content-Encoding: gzip` and `Content-Type: application/json`.
   * Never form-encode the body.
2. Envelope: `{"$schemaRef": "...", "header": {...}, "message": {...}}`. Nothing else is allowed at the top level.
3. Header:
   * `uploaderID`: the CMDR name.
   * `softwareName`: unique and constant. Do **not** use `EDNexus`; another tool already uploads under that name.
   * `softwareVersion`: semver. Bump it whenever the message content changes.
   * `gameversion` and `gamebuild`: **always present** (possibly `""`).
   * Never send `gatewayTimestamp`.
4. `gameversion`/`gamebuild` come from the latest `Fileheader` (then `LoadGame`) of the journal **that produced the event**.
   * Copy `gamebuild` byte-exact. It has a trailing space: `"r332841/r0 "`.
   * CAPI-sourced data uses `"CAPI-Live-market"` or `"CAPI-Live-shipyard"`, or the matching `CAPI-Legacy-…`.
5. `horizons` and `odyssey` come only from `LoadGame.Horizons` and `LoadGame.Odyssey`.
   * If a value is unknown or absent, **omit the key**. Never send `null`, and never send `false` as a stand-in.
   * Never derive these flags from `Fileheader`.
6. Remove every key ending in `_Localised` at **any** depth. Also remove the personal-data keys listed per schema in [§3](#3-per-schema-specification).
7. For schemas with `"additionalProperties": false`, which is every schema except `journal/1`, **build the message from a whitelist** of the
   schema's properties. Never forward a raw event, because new keys Frontier adds will turn into 400s.
8. Adding `StarSystem`, `StarPos` or `SystemAddress` requires a **location cross-check**:
   * Take these values only from the last `Location`, `FSDJump` or `CarrierJump` event.
   * The event's `SystemAddress` (or its system name, if that is all it has) must match the tracked location.
   * If it does not match, **drop the message**.
9. Live game client → live schemas. Beta or alpha game client, **or your own testing or dev builds** → append `/test` to `$schemaRef`.
10. Retries:
    * Never retry a 400, a 403 or a 426. The new gateway can return 403 (rejected); see §1.3.
    * Retry network errors, 408 and 5xx only after **≥ 1 minute**, from a persistent queue that a single bad message cannot block.
    * Send queued messages slowly, about 2 per second at most (EDMC uses 400 ms spacing).
11. Do not upload journal history. Upload only events that happen live.
    * Practice: EDMC and EDDI ignore events read during startup catch-up, and they send nothing while the CMDR is crew in someone else's ship (multicrew).
12. Arrays declared `uniqueItems` must be de-duplicated: `modules`, `ships`, `statusFlags` and `prohibited`.
    * The live gateway currently logs duplicates as "WARNING — accepted temporarily".
    * Earlier in 2026 it answered them with **400** (EDMC issue #2601: CAPI `statusFlags` `["powerplay","powerplay"]`).
13. The relay **no longer drops duplicates** (setting changed 2026-03-22). Do not send the same message twice, for example the same market for the same `MarketID`.
14. Subscriber: `tcp://eddn.edcd.io:9500` with a ZeroMQ `SUB` socket. **You must subscribe to `""`.**
    * Every message is a single zlib (RFC 1950) frame with no topic prefix. A non-empty topic filter is applied by the server to the *compressed* bytes, so it matches nothing.
    * Ignore every `$schemaRef` that ends in `/test`.

---

## 1. Upload (HTTP)

### 1.1 Endpoints [DOC: README.md (live), Developers.md]

| Service | Upload URL | Relay (listeners) | Notes |
|---|---|---|---|
| **Live** | `https://eddn.edcd.io:4430/upload/` | `tcp://eddn.edcd.io:9500` | Always on. Per the README it is auto-restarted every **Thursday 07:06 UTC**, so expect a short outage. |
| Beta | `https://beta.eddn.edcd.io:4431/upload/` | `tcp://beta.eddn.edcd.io:9510/` | Only up while EDCD tests something. Do not rely on it. |
| Dev | `https://dev.eddn.edcd.io:4432/upload/` | `tcp://dev.eddn.edcd.io:9520/` | Same as Beta. |

You MUST use the correct hostname, because TLS terminates on a reverse proxy.

What production looked like [LIVE, 2026-10-12]:

* TLS 1.3 with a Let's Encrypt certificate (`CN=eddn.edcd.io`, issuer `YE2`, expires 2026-12-07). Do **not** pin certificates.
* The server picks `h2` if you offer it through ALPN, but the docs still say "HTTP/1.1. HTTP/2 is not supported at this time". Force HTTP/1.1.
* `GET /` returns `200` with the body `Use a proper endpoint as you are supposed to, you hooligan! ;)`.
* `GET /upload/` returns `400 Empty request body`.
* `OPTIONS /upload/` returns `200` with these headers:

  ```text
  Access-Control-Allow-Methods: POST, OPTIONS
  Access-Control-Allow-Headers: Origin, Accept, Content-Type, X-Requested-With, X-CSRF-Token, X-Signature
  ```
* The old `/health_check/` and `/stats/` paths on 4430 now return 404. Relay port 9090 (`/stats/`) timed out.

### 1.2 Request [DOC: Developers.md “Sending data”; CODE-EDDN Gateway.py; EDMC]

* Method **POST** with no query string. The URL **must** end in `/upload/`. Plain `http://` gets `400 Bad Request`.
* Protocol: **HTTP/1.1**.
* Body: one EDDN message, i.e. a single JSON **object** in UTF-8. Never send an array of messages, and never form-encode.
  Form-encoding (`data=` key) has not been supported since 2022-06-16.
* Headers:
  * Uncompressed body: `Content-Type: application/json`. Developers.md has the typo `applicaton/json`; use the correct spelling.
  * Compressed body (recommended, and EDMC *always* compresses): gzip the body and send **`Content-Encoding: gzip`**.
    Also sending `Content-Type: application/json` is harmless and recommended.
  * The old gateway also accepted `Content-Encoding: deflate`, with either a zlib or a raw deflate stream. It used
    `zlib.decompress(body, 15+32)` and fell back to `-15`. **Use gzip**, the only encoding the docs name.
  * A descriptive `User-Agent` is not required, but EDMC and EDDI both set one (EDDI sends `EDDI/<version>`). Recommended: `EliteNexus/<version>`.
  * Optional: **`X-Signature`**, used only if EDDN has issued you a signing key. See §1.7.
* Timeouts: EDMC uses 10 s. Keep-alive is fine. Error responses may come with `Connection: close` [LIVE].
* Size limit: the old gateway allowed **1 MiB (1,048,576 bytes) of request body, measured *after* compression**, so
  compression lets you send roughly 10× more. Anything bigger gets **413**. The limit of the new gateway is unknown.
  Real sizes are far below it [LIVE averages, compressed/raw]:

  | Message | Compressed | Raw |
  |---|---|---|
  | commodity | 4.5 KB | 25 KB |
  | outfitting/3 | 9.4 KB | 64 KB |
  | outfitting/2 | 2.5 KB | 18 KB |
  | journal | 0.7 KB | 1.3 KB |

### 1.3 Responses [DOC: Developers.md “Server responses”; CODE-EDDN Gateway.py; LIVE]

There are three possible sources of a response: the reverse proxy, the HTTP framework, and the gateway code. Once a message has been
accepted, nothing downstream (for example duplicate detection) can be reported back to you.

| Code | Origin | Meaning | Body | Action |
|---|---|---|---|---|
| **200** | gateway | Accepted | `OK` | Done. Treat any 2xx as success and log the body if it is not `OK`. The new gateway may accept with warnings, see below. |
| **202** | new gateway | Accepted. EDDI's 2026 sender tests treat 202 as success, which implies the new gateway can return it. | ? | Success. |
| **403** | new gateway | **Forbidden: rejected by policy.** EDDI's sender reports it as "EDDN rejected {signed\|unsigned} upload (HTTP 403). Check app signing configuration." and its tests make 403 **terminal** for both signed and unsigned uploads. A bad `X-Signature`, or a sender being refused, would plausibly produce it. | ? | **Never retry.** Alert yourself (the developer), and treat it as a possible block of your `softwareName`/version. |
| **400** | proxy | Plain HTTP used instead of HTTPS | proxy HTML | Fix the code. Never retry. |
| **400** | gateway | `FAIL: zlib.error: <detail>`: the body claimed compression but did not decompress | text | Drop. Never retry. |
| **400** | gateway | `FAIL: Malformed Upload: <detail>`: form-encoding problems. This is a legacy path. | text | Drop. |
| **400** | gateway | `FAIL: JSON parsing: <detail>`, for example `Expecting property name enclosed in double quotes: line 1 column 2 (char 1)` | text | Drop. |
| **400** | gateway | `FAIL: Schema Validation: [<ValidationError: "'StarPos' is a required property">]`. A real 2026 example: `FAIL: Schema Validation: [<ValidationError: "['powerplay', 'powerplay'] has non-unique elements">]` (EDMC #2601, January–March 2026). | text | Drop and fix the code. A disallowed key shows up as `... {'type': ['array', 'boolean', 'integer', 'number', 'null', 'object', 'string']} is not allowed for '<VALUE>'`. The *value* is quoted, not the key name. |
| **400** | gateway | Unknown `$schemaRef`. The old code's text is `Schema <ref> is unknown, unable to validate.`, wrapped in a `FAIL: …` string. | text | Drop. EDMC also drops these, because a new schema may not be live yet. |
| **400** | new gateway [LIVE] | `Empty request body` | text | Bug in your code. |
| **413** | framework | Payload Too Large (more than 1 MiB compressed) | — | You MAY retry later, but "not too quickly or in perpetuity". EDMC drops it. Recommended: drop and log. |
| **426** | gateway | Outdated schema. The exact body from the code is `FAIL: Outdated Schema: The schema you have used is no longer supported. Please check for an updated version of your application.` (Developers.md misspells it "Oudated"). | text | **Never retry.** Update the code. |
| **408** | proxy | Request timed out | — | Retry after ≥ 1 min. |
| **503** (and 502/504) | proxy | Gateway down or not responding | — | Retry after ≥ 1 min. |
| 500 | gateway | The old code produced this when the JSON was not an object or had no `$schemaRef` | — | Treat as a bug and drop. |

Every gateway failure body begins with `FAIL: ` [DOC]. Do **not** parse the detail text for control flow. Key off the status code only.
Production is a new implementation and its wording may differ (for example `Empty request body`).

New-gateway accounting, from the [LIVE] monitor legend:

* "Warnings today = the message didn't conform to the schema (contained duplicate items), but was accepted temporarily."
* "Failed today = … ignored."
* "Signed today = the message was signed and accepted."

The monitor shows **per-software-version WARNING/FAILED counts publicly**. That is where EDCD admins will notice a misbehaving
sender, so watch your own row.

### 1.4 Retry, backoff and queue etiquette

From Developers.md:

* Handle connection refused, timeouts and all the responses above.
* "Carefully consider whether you should queue a 'failed' message for later retry … ensure that one 'bad' message does not
  block other messages from being successfully sent."
* "You **MUST** wait some reasonable time (minimum 1 minute) before retrying any failed message."
* "You **MUST NOT** retry any message that received a HTTP `400` or `426` code", unless you have manually fixed the problem.
* A 413 may be retried, "but should not do so too quickly or in perpetuity".
* "No data is better than bad data. *Delayed* good data is better than degrading the EDDN service for others."

What EDMC does [EDMC `plugins/eddn.py`, `EDDNSender`], a proven model:

* Every message is first written to a **SQLite queue**, `eddn_queue-v1.db`. Each row stores the created timestamp, cmdr, app version, `gameversion`,
  `gamebuild` and the full message JSON. Then EDMC tries to send it immediately.
* Station data (commodity, outfitting, shipyard, fcmaterials_*) is never delayed. Non-station data can optionally be delayed until docked.
* `send` returns "remove from queue" for these outcomes:
  * 200;
  * **400** (dropped);
  * **413** (dropped);
  * the "unknown schema" response.
* Network errors and other HTTP codes leave the message queued and **stop processing the queue** until the next run.
* Queue runs:
  * 10 s after startup;
  * then every **300 s**;
  * immediately on `Docked`.
* Inside a run, messages go **one at a time, oldest first**, with **400 ms** between them (`REPLAY_DELAY`, "roughly two messages per second").
* The request timeout is 10 s.

Recommended for Elite Nexus (Rust):

* One sender task, fed by a persistent queue (SQLite or sled).
* Exponential backoff on transport, 408 and 5xx errors: 60 s → 2 min → 5 min, capped at 15 min, with jitter.
* Permanent drop on 400, 403, 413 or 426, logged with the response body. Raise an internal "EDDN rejected our data" alarm and show it to the user, but do not loop.
  * On repeated 403s, pause all EDDN sending for the session. It probably means the sender is refused.
* Give each message a time-to-live of a few hours. Listeners MAY reject "too old" data and the data loses value fast.
* Keep `gameversion` and `gamebuild` inside each queued message as of creation time, as EDMC does. Never re-stamp at send time.

### 1.5 Batching

* **There is no multi-message batching.** Each HTTP request carries exactly one EDDN message.
* Batching happens *inside* a message only where a schema says so. Today that is only `fsssignaldiscovered/1`, which takes a run of
  `FSSSignalDiscovered` events (see §3.9). The `Route` in `navroute/1` and the `commodities` and `modules` lists are naturally multi-item.
* Do not send messages in parallel bursts.

### 1.6 Rust notes

* `reqwest` with `.http1_only()`, `rustls`, a gzip-compressed body (`flate2::write::GzEncoder`) and explicit headers
  `Content-Encoding: gzip` and `Content-Type: application/json`.
* Serialize integers as JSON integers. `MarketID` and `SystemAddress` exceed 32 bits, so use `u64` or `i64`.
* `StarPos` numbers: emit them as the journal gives them. `f64` is exact for these values, which are multiples of 1/32.
* **Validate locally before sending.** Bundle the schemas from `eddn-schemas/` and validate with the `jsonschema` crate (Draft 4). Note:
  * `develop-branch_scanorganic-v1.0.json` starts with a UTF-8 **BOM**. Strip it before parsing.
  * `fsssignaldiscovered-v1.0.json` puts a `patternProperties` key inside `properties`. That is a schema bug and has no effect.
* Validation catches most mistakes before they become public FAILED counts.

### 1.7 Optional signed uploads (`X-Signature`): new in 2026 and undocumented by EDDN itself

The live gateway allows the request header `X-Signature` (it appears in the CORS `Access-Control-Allow-Headers` list), and the monitor
counts "Signed" messages. Today only EDDI 5.0.6 shows any (118 signed). EDDN has published no specification.
The open-source EDDI sender is the only reference [EDDI `EDDNResponder/Sender/EDDN_Sender.cs`, `BuildSecrets/BuildInjectedSecrets.cs`,
`BuildSecrets/README.md`, `Tests/EddnSigningTests.cs`]:

* `X-Signature: <64 lowercase hex chars>` is `hex(HMAC-SHA256(key, payload))`.
* **`payload` is the exact UTF-8 bytes of the complete, *uncompressed* JSON envelope** (`$schemaRef` + `header` + `message`), as
  serialised for sending.
  * If the request body is then gzipped, the signature does not change: EDDI's tests confirm the same header is sent on the compressed retry.
  * So the gateway presumably verifies against the decompressed body. Serialise once, sign those bytes, and send exactly those bytes, compressed or not.
* `key` is an **application-specific secret "supplied by EDDN"**, the operators. EDDI injects it at build time and never publishes it.
* Unsigned uploads remain valid. Of all traffic, only EDDI's messages show as signed.
* A 403 is terminal. EDDI also drops the message, rather than sending it unsigned, if its signer fails.

**Recommendation for Elite Nexus:**

* Ship unsigned by default.
* Ask the EDDN operators (EDCD Discord `#eddn`) for a key once the uploader is stable.
* Behind a build-time secret, implement HMAC-SHA256 over the exact body bytes.
* Never put the key in the repository.

---

## 2. Message envelope

### 2.1 Top level [DOC: README-EDDN-schemas.md; every schema has `"additionalProperties": false` at the root]

```json
{
  "$schemaRef": "https://eddn.edcd.io/schemas/<name>/<major>[/test]",
  "header": { "uploaderID": "...", "softwareName": "...", "softwareVersion": "...", "gameversion": "...", "gamebuild": "..." },
  "message": { "timestamp": "...", "...": "..." }
}
```

* `$schemaRef` is a string chosen from the accepted list in §3, **including `/test` when appropriate**. The value is an
  identifier, not a URL you need to fetch. Production serves the schema JSON at that URL anyway, through an nginx alias.
* The README of the old docs mentions `$id`. That belongs to the schema file, not to the message. Send only `$schemaRef`, `header` and `message`.

### 2.2 `header` [DOC; all schemas: `"required": ["uploaderID","softwareName","softwareVersion"]`, `additionalProperties: true`]

| Field | Rule |
|---|---|
| `uploaderID` | **Required**, string. "Please DO send a sensible `uploaderID` value, preferably simply the relevant in-game Commander name." The relay replaces it with a hash before anyone sees it. Old code: `sha1("<nonce>-<uploader>")`, with the nonce rotated every **3 min** since 2024-01-23 (it was 12 h before). [LIVE]: listeners now see a 64-hex value. It is never empty in practice. EDMC sends the CMDR name. EDDI sends the CMDR name, or `"Unknown commander"` when it has none. |
| `softwareName` | **Required.** "You MUST set a unique, and self-consistent, value". EDCD identifies misbehaving clients by this name. Today's live names include `E:D Market Connector [Windows]`, `EDDiscovery`, `EDDI`, `EDO Materials Helper` and `EDNexus`. **Pick `Elite Nexus` (exact and constant), not `EDNexus`.** |
| `softwareVersion` | **Required.** Semver is recommended. "you MUST increment your version number if you make any changes to the content of messages your software sends." Listeners filter on it. Dev builds should carry a pre-release tag and send to `/test`. |
| `gameversion` | Optional in the schemas, but **"all Senders MUST make every effort to include them"**. Rules in §2.3. |
| `gamebuild` | Same as above. Rules in §2.3. |
| `gatewayTimestamp` | **Do not send.** The gateway sets or overwrites it in ISO-8601 UTC with microseconds, for example `2026-10-12T09:23:12.361367Z` [LIVE]. |

Other header keys are tolerated by the schema (`additionalProperties: true`) but serve no purpose. Do not add any.
The old gateway also added `uploaderIP`, and the relay stripped it.

### 2.3 `gameversion` / `gamebuild` [DOC: Developers.md “gameversions and gamebuild”]

Where to get them:

* **Journal data:** use `Fileheader.gameversion` and `Fileheader.build` from the journal file the event came from.
  `LoadGame.gameversion` and `LoadGame.build` are also acceptable. Both EDMC and EDDI read Fileheader and then overwrite from
  LoadGame when it has the fields, which it does in 4.x. Prefer Fileheader, because some relevant events arrive before LoadGame.
* **CAPI `/journal` endpoint:** there is no Fileheader.
  * Use `LoadGame.gameversion` if present (4.x).
  * Otherwise use `"CAPI-Legacy-journal"`.
* **Other CAPI endpoints:** use `"CAPI-<Live|Legacy>-<endpoint>"`. **Never** use the value from the journal you are also reading.
  * commodity → `CAPI-Live-market`
  * shipyard → `CAPI-Live-shipyard`
  * outfitting → `CAPI-Live-shipyard` (outfitting data comes from `/shipyard`)
  * fcmaterials_capi → `CAPI-Live-market`
* **`gamebuild`:** use the `build` value under the same rules as above, otherwise `""`.
  * It may instead mirror `gameversion`, but **only** for `CAPI-…` strings.
  * **Do NOT strip whitespace.** Live builds end with a space.
* **Unknown:** if no source value and no `CAPI-…` string applies, you **MUST still send both fields, set to `""`**.

Values seen in production on 2026-10-12 [LIVE]:

* Game clients on `"gameversion":"4.4.1.1"` and `"gamebuild":"r332841/r0 "` dominate.
* CAPI-sourced messages carry `"CAPI-Live-shipyard"` or `"CAPI-Live-market"` with `"gamebuild":""`.

| Data source | Galaxy | gameversion | gamebuild |
|---|---|---|---|
| Journal | Live (4.x) | `"4.4.1.1"` (Fileheader) | `"r332841/r0 "` |
| Journal | Legacy (3.8) | `"3.8.0.1400"` (Developers.md example) | `"r289583/r0 "` |
| CAPI `/journal` | Live | from LoadGame | from LoadGame |
| CAPI `/journal` | Legacy | `"CAPI-Legacy-journal"` | `""` or `"CAPI-Legacy-journal"` |
| CAPI `/market` | Live / Legacy | `"CAPI-Live-market"` / `"CAPI-Legacy-market"` | `""` or the same string |
| CAPI `/shipyard` | Live / Legacy | `"CAPI-Live-shipyard"` / `"CAPI-Legacy-shipyard"` | `""` or the same string |

Live vs Legacy:

* The CAPI hosts are `https://companion.orerve.net` (Live), `https://legacy-companion.orerve.net` (Legacy) and
  `https://pts-companion.orerve.net` (beta/PTS) [EDMC companion.py].
* EDMC treats a journal as Live if `gameversion` ≥ `4.0.0` by semantic comparison (`monitor.is_live_galaxy()`), and as Legacy otherwise.
* There are **no separate Legacy schemas**. Legacy data goes to the same `$schemaRef` values, and listeners tell the two apart by `gameversion`.
* `Fileheader` carries `"Odyssey":true` for any 4.x client. That flag means "4.0 client", **not** Odyssey ownership, so do not use it for the `odyssey` flag (§5).

Beta and test:

* **"You MUST NOT send information from any non-live (e.g. alpha or beta) version of the game to the main Schemas"**.
  Append `/test` instead, for example `https://eddn.edcd.io/schemas/shipyard/2/test`.
* EDMC's rule: a game is beta if `'alpha'` or `'beta'` appears in `gameversion.lower()`. Data from the PTS CAPI host also goes to `/test`.
* "You **MUST** also utilise these test forms of the Schemas whenever you are testing your EDDN-handling code, be that new
  code or changes to existing code." Every live schema has a `/test` twin, and production relays test messages to all listeners.
* An app's own beta *release channel* is not a reason to use `/test` once its EDDN code is final. EDDI changed its behaviour accordingly.

### 2.4 Common `message` rules

* **`timestamp`**: a string in RFC 3339 / ISO 8601 `date-time` format. It is validated with a format checker.
  * Use the event's or file's own timestamp. Journal timestamps are UTC with a `Z` suffix; never claim `Z` for local time.
  * Sub-second precision is allowed.
  * For CAPI data use the time of the CAPI response.
  * Listeners MAY reject data that is "too old".
* **Staleness.** EDDN sets no maximum age. Listeners "MAY make decisions on accepting data based on this time stamp, i.e. 'too old'".
  [LIVE]: the median gap between event and gateway is 2.4 s and the p95 is 19 s. Recommended policy:
  * Upload only events observed live, never during the catch-up read of existing journals.
  * Station data (`commodity`, `outfitting`, `shipyard`, `fcmaterials_*`): give up if it cannot be sent within about 1 h.
    Old market data can overwrite newer data at a careless listener.
  * Other data: give up after about 24 h.
  * Never re-stamp `timestamp`.
  * Companion files (`Market.json`, `Outfitting.json`, `Shipyard.json`, `NavRoute.json`, `FCMaterials.json`) must have the same `timestamp`
    as the journal event that announced them. EDMC allows ±5 s for NavRoute and FCMaterials. Otherwise you are reading a stale file.
* **`event`**: keep the journal `event` key and value for journal-sourced data. Every journal-based schema requires it.
* **`_Localised`**: remove every key whose name ends in `_Localised`, recursively through objects and arrays. Developers.md says "MUST".
  For CAPI data, also remove `locName`. EDMC removes any key matching `^loc[A-Z].+`.
* **Personal data**: messages MUST NOT contain CMDR-specific data other than `uploaderID`, `horizons` and `odyssey`.
  "EDDN is intended to transport generic data … that every player would see in-game in station services or the local map."
* **Key renames**: a schema property with a `"renamed": "X"` attribute means the journal or CAPI key `X` must be emitted under the schema's name.
* **Construction strategy**:
  * `journal/1` is pass-through. Its message has `additionalProperties: true`, so copy the event and delete the blacklist and `_Localised` keys.
  * **Every other schema is whitelist.** `additionalProperties: false` applies at the message level, and inside most array items, so copy only the keys the schema defines.
* **Location augmentation and cross-check** [DOC: Developers.md “Other data Augmentations”; EDMC; EDDI]:
  * Keep a `TrackedLocation {system_name, system_address, star_pos}`.
    * Set it **only** from `Location`, `FSDJump` and `CarrierJump`, which all carry `StarSystem`, `SystemAddress` and `StarPos`.
    * Clear it on `Fileheader`, `LoadGame`, `JoinACrew` and `QuitACrew`.
  * For an event that needs augmenting:
    * If it has `SystemAddress`, it **must equal** `tracked.system_address`, or else **drop the message**.
      This catches delayed `Scan` events after a jump, which EDMC has seen.
    * If it only has a system name, compare names.
    * Only then add the missing `StarSystem`, `StarPos` or `SystemAddress`.
    * "Do not blindly add `SystemAddress` or `StarPos`". The game can skip writing events, so blind augmentation produces wrong data.
  * Optional extra check (EDDI): verify that `StarPos` lies inside the boxel encoded in `SystemAddress`.
    * `mc = addr & 7`, `size = 10 << mc`
    * `x0 = (((addr >> (30 - 2*mc)) & (0x3FFF >> mc)) << mc)*10 - 49985`
    * `y0 = (((addr >> (17 - mc)) & (0x1FFF >> mc)) << mc)*10 - 40985`
    * `z0 = (((addr >> 3) & (0x3FFF >> mc)) << mc)*10 - 24105`
    * Require `0 ≤ pos - origin ≤ size` on each axis.
    * Also reject a `SystemAddress` ≤ 1. Some events, notably old CodexEntry bugs, carried `1` [EDDI].
* **Session gating** (practice, not a written EDDN rule):
  * EDMC sends no journal-derived EDDN message on any schema while `Captain` is set, meaning you are crew aboard another CMDR's ship (multicrew).
  * EDMC reads existing journal lines at startup for state only, and passes only new lines to its EDDN plugin.
  * EDDI does not send while "in telepresence" or in CQC, and neither tool sends events replayed during startup or log loading.
  * Recommended: send only live events, only once the CMDR, `Fileheader`, `LoadGame` and a location event are known, and never from CQC or multicrew-as-crew.

---

## 3. Per-schema specification

These are the `$schemaRef` values production accepts [LIVE monitor and relay, 2026-10-12]. Each also has a `/test` variant.

| Schema | `$schemaRef` | In GitHub `live` branch? | Volume (msgs/h, 60-min window 2026-10-12 09:14 UTC) |
|---|---|---|---|
| journal | `https://eddn.edcd.io/schemas/journal/1` | yes | ~15,200 |
| fsssignaldiscovered | `https://eddn.edcd.io/schemas/fsssignaldiscovered/1` | yes | ~3,600 |
| fssbodysignals | `https://eddn.edcd.io/schemas/fssbodysignals/1` | yes | ~2,200 |
| commodity | `https://eddn.edcd.io/schemas/commodity/3` | yes | ~1,500 |
| fssdiscoveryscan | `https://eddn.edcd.io/schemas/fssdiscoveryscan/1` | yes | ~1,500 |
| dockinggranted | `https://eddn.edcd.io/schemas/dockinggranted/1` | yes (2024-01) | ~1,300 |
| navroute | `https://eddn.edcd.io/schemas/navroute/1` | yes | ~1,300 |
| scanbarycentre | `https://eddn.edcd.io/schemas/scanbarycentre/1` | yes | ~720 |
| fssallbodiesfound | `https://eddn.edcd.io/schemas/fssallbodiesfound/1` | yes | ~710 |
| outfitting v2 | `https://eddn.edcd.io/schemas/outfitting/2` | yes | ~670 |
| shipyard | `https://eddn.edcd.io/schemas/shipyard/2` | yes | ~510 |
| approachsettlement | `https://eddn.edcd.io/schemas/approachsettlement/1` | yes | ~320 |
| codexentry | `https://eddn.edcd.io/schemas/codexentry/1` | yes | ~200 |
| dockingdenied | `https://eddn.edcd.io/schemas/dockingdenied/1` | yes (2024-01) | ~130 |
| **outfitting v3** | `https://eddn.edcd.io/schemas/outfitting/3` | **no**: `master` only (2026-07-03), but live accepts it | ~110 |
| fcmaterials_capi | `https://eddn.edcd.io/schemas/fcmaterials_capi/1` | yes | ~60 |
| navbeaconscan | `https://eddn.edcd.io/schemas/navbeaconscan/1` | yes | ~50 |
| fcmaterials_journal | `https://eddn.edcd.io/schemas/fcmaterials_journal/1` | yes | ~40 |
| **scanorganic** | `https://eddn.edcd.io/schemas/scanorganic/1` | **no**: `develop` only (2026-06-17), but live accepts it | ~26 |
| blackmarket | `https://eddn.edcd.io/schemas/blackmarket/1` | registered in code, **deprecated since 2017** | 0 (not on the monitor) |

**No colonisation or Powerplay schema exists.** The 2024 Powerplay 2.0 changes touched only `fsssignaldiscovered/1`.
Colonisation-era events reach EDDN only through the existing schemas: `Docked`, `Location` and `FSDJump` on `journal/1`, and construction-site
markets on `commodity/3`.

These were proposed and **not merged**:

* `pledged` flag on journal/1 (PR #225, closed 2025-03-23);
* `materialtrade/1` and `technologybroker/1` (PR #226, closed 2025-01);
* `mercmodules` array in outfitting/2 (PR #239, closed 2026-07-01).

**Do not send any of them.**

Notation used below: **REQ** = required by the schema, **OPT** = optional, **DEL** = must be removed, **ADD** = must be added.
"Strict" means `message.additionalProperties: false`, so whitelist.

### 3.1 `journal/1`: generic journal events (not strict)

* Source: these journal events **only**: `Docked`, `FSDJump`, `Scan`, `Location`, `SAASignalsFound`, `CarrierJump`.
  * The enum also contains `CodexEntry`, but CodexEntry has its own schema (§3.11), so send it there.
  * The README calls this schema "historical … will be deprecated". Any new event type needs its own schema.
* **REQ**: `timestamp`, `event`, `StarSystem` (string, at least 1 character), `StarPos` (exactly 3 numbers), `SystemAddress` (integer).
* **OPT**: `horizons`, `odyssey`, and everything else in the event, because `additionalProperties: true`.
* **DEL**, which the schema enforces as disallowed:
  * top level: `ActiveFine`, `CockpitBreach`, `BoostUsed`, `FuelLevel`, `FuelUsed`, `JumpDist`, `Latitude`, `Longitude`, `Wanted`,
    `IsNewEntry`, `NewTraitsDiscovered`, `Traits`, `VoucherAmount`, and any `*_Localised`;
  * inside every `Factions[]` item: `HappiestSystem`, `HomeSystem`, `MyReputation`, `SquadronFaction`, and `*_Localised`;
  * inside `Materials[]`, `StationEconomies[]` and `Signals[]`: `*_Localised`.
  * By event (per the README): Docked loses `Wanted`, `ActiveFine` and `CockpitBreach`. FSDJump loses `Wanted`, `BoostUsed`, `FuelLevel`, `FuelUsed`, `JumpDist` and the Faction keys. Location loses `Wanted`, `Latitude`, `Longitude` and the Faction keys.
  * EDMC simply strips the whole top-level list from every one of these events. EDDI also strips them from `CarrierJump`.
  * Also remove `_Localised` from the nested arrays the schema does not police, for example `SAASignalsFound.Genuses[].Genus_Localised`.
* **ADD**:
  * `horizons` and `odyssey` from LoadGame (§5).
  * `StarSystem` where missing, which is only `SAASignalsFound`.
  * `StarPos` where missing: `Docked`, `Scan` and `SAASignalsFound`.
  * Both additions require the cross-check (§2.4). `SystemAddress` must already be in the event; EDMC aborts if it is not.

  Presence table [EDMC comment]:

  | Event | StarSystem | SystemAddress | StarPos |
  |---|---|---|---|
  | Docked | Y | Y | N |
  | FSDJump | Y | Y | Y |
  | Scan | Y | Y | N |
  | Location | Y | Y | Y |
  | SAASignalsFound | N | Y | N |
  | CarrierJump | Y | Y | Y |
* Optional EDMC extra: on `Docked` at a planetary port, EDMC adds `"Body"` and `"BodyType":"Planet"` from tracked state.
  That is allowed because the schema is non-strict, but you do not need it.
* Real production example (EDMC 6.1.2), abridged:
  ```json
  {"$schemaRef":"https://eddn.edcd.io/schemas/journal/1","header":{"uploaderID":"…","softwareName":"E:D Market Connector [Windows]","softwareVersion":"6.1.2","gameversion":"4.4.1.1","gamebuild":"r332841/r0 "},
   "message":{"timestamp":"2026-10-12T09:23:11Z","event":"Scan","ScanType":"AutoScan","BodyName":"Tucanae Sector JC-U b3-1 A A Belt Cluster 1","BodyID":6,
     "Parents":[{"Ring":5},{"Star":1},{"Null":0}],"StarSystem":"Tucanae Sector JC-U b3-1","SystemAddress":2871319274977,"DistanceFromArrivalLS":5.571921,
     "WasDiscovered":true,"WasMapped":false,"WasFootfalled":false,"horizons":true,"odyssey":true,"StarPos":[110.65625,-201.375,152.84375]}}
  ```

### 3.2 `commodity/3`: station market (strict)

* Source: the journal `Market` event, after which you **read `Market.json`** and treat that file as the event. The alternative source is CAPI `/market`
  (§4, with the CAPI lag checks).
* **REQ**: `systemName` (≥ 1 character), `stationName` (≥ 1 character), `marketId` (integer), `timestamp`, `commodities` (array, **may be empty**,
  which marks a Fleet Carrier with no orders).
* **OPT**:
  * `stationType` (← `StationType`) and `carrierDockingAccess` (← `CarrierDockingAccess`). Both were added 2024-01-23, and EDMC sends them for journal data.
  * `horizons` and `odyssey`.
  * `economies`: an array of `{name, proportion}`, **CAPI only**.
  * `prohibited`: an array of unique strings, **CAPI only**.
* Commodity item (strict):
  * **REQ**: `name` (string, ≥ 1 character), `meanPrice`, `buyPrice`, `stock`, `sellPrice`, `demand` (all integers), and
    `stockBracket` and `demandBracket` (enum `0, 1, 2, 3, ""`).
  * **OPT**: `statusFlags` (array of unique, non-empty strings, `minItems: 1`).
  * **DEL**:
    * explicitly: `Producer`, `Rare` and `id`;
    * implicitly, because items are strict: `Consumer`, `Category`, `Category_Localised`, `Name_Localised` and anything else.
* The README's elision list still says to remove "`StationType`". That has been **stale since 2024-01-23**, when the schema began to allow
  `stationType` and dropped the old disallowed `StationType`. EDMC sends `stationType`.
* From journal data, **do not** send `economies` or `prohibited`: "You **MUST NOT** send empty lists."
* Mapping and filtering: §4.1.
* Dedupe: EDMC does not resend when the commodity list is identical to the last one sent for the same `MarketID`.

### 3.3 `outfitting/2` and `outfitting/3`: station outfitting (strict)

* Source: the journal `Outfitting` event, after which you **read `Outfitting.json`**. The alternative is CAPI `/shipyard` → `lastStarport.modules`.
* **v2** (`https://eddn.edcd.io/schemas/outfitting/2`):
  * **REQ**: `systemName`, `stationName`, `marketId`, `timestamp`, and `modules`. `modules` is an array of **unique** strings,
    `minItems 1`, each matching `(^Hpt_|^hpt_|^Int_|^int_|_Armour_|_armour_)`.
  * **OPT**: `horizons`, `odyssey`.
  * Strict.
* **v3** (`https://eddn.edcd.io/schemas/outfitting/3`), added to master 2026-07-03 for the **Operations** update (30 June 2026, "Merc Coin" currency):
  * Same message keys as v2.
  * `modules` is an array of unique **objects** (`minItems 1`). Each requires `id` (int), `Name` (string matching the same pattern),
    `BuyPrice` (int) and `BuyMercCoinsPrice` (int).
  * The item objects are not strict in the schema, but send only those 4 keys.
  * **ADD**: "You must augment each module with a zero price `BuyMercCoinsPrice` field when it is not already present."
  * CAPI mapping [EDDI]: `id`←`id`, `Name`←`name`, `BuyPrice`←`cost`, `BuyMercCoinsPrice`←`additionalCost.mercCoins` (default 0).
  * Merc-coin items have `BuyPrice: 0` and `BuyMercCoinsPrice > 0` [LIVE sample: 25 of 985 modules].
* **DEL** (both versions):
  * every item that is not `Hpt_*`, `Int_*` or `*_Armour_*`, such as bobbleheads, decals, paintjobs and shipkits;
  * for CAPI data, every item with a non-null `sku` other than `ELITE_HORIZONS_V_PLANETARY_LANDINGS` (PowerPlay and tech-broker items);
  * `Int_PlanetApproachSuite` (any case).
* Do not send an empty list.
* **Which to send?** Production accepts both, and neither is deprecated.
  * EDMC 6.1.x sends **v2 only**. EDDI 5.0.3+ sends **v3 only**. Traffic on 2026-10-11 was about 36k v2 versus 5k v3.
  * EDO Materials Helper 3.15.6 sends **both** v2 and v3 for the same journal Outfitting event [LIVE].
  * Recommendation: send **v2** by default for maximum listener compatibility, and optionally v3 too, behind a feature flag, as a second POST.
  * Whether merc-coin-only modules belong in v2 is not documented. EDMC includes everything that matches the regex.
* ⚠️ **Since the Operations update, `Outfitting.json` can list the same module `Name` more than once.**
  * [LIVE] EDMC 6.1.2 sent `"Hpt_basicmissilerack_fixed_large"` three times in one v2 `modules` list.
  * That violates `uniqueItems` and is why EDMC 6.1.2 has hundreds of WARNINGs a day on the monitor ("contained duplicate items, but was accepted temporarily").
  * **De-duplicate v2 names.** EDMC's `develop` branch now uses a set.
  * For v3, drop only exact-duplicate objects. Entries with the same `Name` but a different `id` or price are distinct objects.
* Mapping: §4.2.

### 3.4 `shipyard/2`: station shipyard (strict)

* Source: the journal `Shipyard` event, after which you **read `Shipyard.json`**. The alternative is CAPI `/shipyard`.
* **REQ**: `systemName`, `stationName`, `marketId`, `timestamp`, and `ships` (← `PriceList`), an array of **unique** non-empty strings with `minItems 1`.
* **OPT**: `horizons`, `odyssey`, and `allowCobraMkIV` (boolean, ← `AllowCobraMkIV` in `Shipyard.json`).
* `ships` holds **only the ship symbol names**:
  * From the journal: `PriceList[].ShipType`.
  * From CAPI: `ships.shipyard_list{}.name` plus `ships.unavailable_list[].name`. Unavailable ships must be **included**.
* Do not send an empty list. Mapping: §4.3.

### 3.5 `navroute/1` (strict)

* Source: the journal `NavRoute` event, after which you **read `NavRoute.json`** and send its `Route` array.
* **REQ**: `timestamp`, `event` (`"NavRoute"`), and `Route`. `Route` is an array of strict items, each with all four of `StarSystem` (≥ 1 character),
  `SystemAddress` (int), `StarPos` (3 numbers) and `StarClass` (≥ 1 character).
* **OPT**: `horizons`, `odyssey`.
* Rules [EDMC]:
  * Re-read the file until its `timestamp` is within **5 s** of the journal event. EDMC retries 11 times.
  * If the file's `event` is `NavRouteClear`, do not send.
  * If there is **no** `Route` key, do not send. Sending without one caused a 400 storm in 2022.
  * An empty `Route` validates, but sending it is pointless. Recommended: skip it.
* No location augmentation.

### 3.6 `fssdiscoveryscan/1` (strict)

* Source: the `FSSDiscoveryScan` event, `{timestamp, event, Progress, BodyCount, NonBodyCount, SystemName, SystemAddress}`.
* **REQ**: `timestamp`, `event`, `SystemName`, `StarPos`, `SystemAddress`, `BodyCount`, `NonBodyCount`.
* **DEL**: `Progress`, which is personal and disallowed.
* **ADD**: `StarPos`, with the cross-check on `SystemAddress`.

### 3.7 `fssallbodiesfound/1` (strict)

* Source: the `FSSAllBodiesFound` event.
* **REQ**: `timestamp`, `event`, `SystemName`, `StarPos`, `SystemAddress`, `Count`.
* **ADD**: `StarPos`, with the cross-check.

### 3.8 `fssbodysignals/1` (strict)

* Source: the `FSSBodySignals` event.
* **REQ**: `timestamp`, `event`, `StarSystem`, `StarPos`, `SystemAddress`, `BodyID`, and `Signals`, whose items are strict with only `{Type, Count}`.
* **OPT**: `BodyName`, `horizons`, `odyssey`.
* **DEL**: `Signals[].Type_Localised`.
* **ADD**: `StarSystem` and `StarPos`, with the cross-check.
* [LIVE] Signal types now include `$PlanetaryMiningLocation_Name;`.

### 3.9 `fsssignaldiscovered/1` (strict, batched)

* Source: `FSSSignalDiscovered` events. This is the **only** source.
* Message:
  * **REQ**: `event` (`"FSSSignalDiscovered"`), `timestamp`, `SystemAddress`, `StarSystem`, `StarPos`, and `signals` (array, `minItems 1`).
  * **OPT**: `horizons`, `odyssey`. The README says SHOULD.
* Signal item:
  * Strict since 2023-10.
  * **REQ**: `timestamp`, `SignalName`.
  * **OPT**: `SignalType`, `IsStation`, `USSType`, `SpawningState`, `SpawningFaction`, `SpawningPower` (added 2024-10-31),
    `OpposingPower` (added 2024-10-31), and `ThreatLevel` (int).
  * **DEL**:
    * `event`, `SystemAddress` (it moves to the top level), `TimeRemaining` (disallowed), and every `*_Localised`
      (`SignalName_Localised`, `USSType_Localised`, `SpawningState_Localised`, `SpawningFaction_Localised` …);
    * anything else not listed. Whitelist.
  * **Drop the whole signal** if `USSType == "$USS_Type_MissionTarget;"`. The schema forbids that value.
  * **Keep** each signal's own `timestamp`.
* Batching algorithm (README, as EDMC implements it):
  1. Append every `FSSSignalDiscovered` to a pending list.
  2. On the first *other* event, if the list is non-empty, choose the location for the batch:
     * If the other event is `Location`, `FSDJump` or `CarrierJump` (the **Odyssey ordering**, where signals arrive *before* the jump event),
       use **that event's** `StarSystem`, `SystemAddress` and `StarPos`.
     * Otherwise (the Horizons ordering) use the tracked location.
       If the tracked location is unknown, drop the batch.
  3. Build the message. If the first signal's `SystemAddress` does not match, drop the entire batch.
     Otherwise drop each signal whose `SystemAddress` does not match, then apply the elisions above.
  4. Set the top-level `timestamp` to the **first signal's** timestamp.
  5. If no signals remain, send nothing.
* Signals scanned one at a time in the FSS, with `Music` and similar events in between, go out as single-signal messages. That is fine.
* [LIVE] example signal from the Operations era: `{"timestamp":"…","SignalName":"$MULTIPLAYER_SCENARIO80_TITLE;","SignalType":"NavBeacon"}`.

### 3.10 `navbeaconscan/1` (strict)

* Source: the `NavBeaconScan` event, `{timestamp, event, SystemAddress, NumBodies}`.
* **REQ**: `timestamp`, `event`, `StarSystem`, `StarPos`, `SystemAddress`, `NumBodies`.
* **ADD**: `StarSystem` and `StarPos`, with the cross-check.

### 3.11 `codexentry/1` (strict)

* Source: the `CodexEntry` event.
* **REQ**: `timestamp`, `event`, `System` (≥ 1 character), `StarPos`, `SystemAddress`, `EntryID`.
* **OPT**:
  * `Name`, `Region`, `Category`, `SubCategory` (all strings of ≥ 1 character);
  * `Latitude`, `Longitude`, `NearestDestination` (may be `""`);
  * `VoucherAmount` (int). It is allowed here even though `journal/1` disallows it.
  * `Traits` (array of non-empty strings);
  * `BodyID`, `BodyName`, `horizons`, `odyssey`.
* **DEL**: `IsNewEntry`, `NewTraitsDiscovered`, and every `*_Localised` (`Name_Localised`, `SubCategory_Localised`, `Category_Localised`, `Region_Localised`, `NearestDestination_Localised`).
* **ADD**: `StarPos`, with the cross-check on `SystemAddress`.
* EDMC drops the message if `System`, `Name`, `Region`, `Category` or `SubCategory` is `""`, or if a `Traits` entry is empty, because `minLength` would fail.
* **BodyName/BodyID rules** (README, written for 4.0.0.801 and later):
  * Track `journal_body_name` and `journal_body_id`:
    * set them from `ApproachBody.BodyName/BodyID` and from `Location` (EDDI reads Location's `Body`/`BodyID`);
    * clear them on `LeaveBody` and `FSDJump`, but **not** on `SupercruiseEntry`.
  * Track `status_body_name` from `Status.json.BodyName`, clearing it when the key is absent. EDMC also clears it on `Music` with `MusicTrack == "MainMenu"`.
  * For a CodexEntry:
    * Only if `status_body_name` is set: set `BodyName = status_body_name`, and set `BodyID = journal_body_id` **only if**
      `status_body_name == journal_body_name`. This avoids binary-pair confusion, for example `Baliscii 7 a` vs `7 b`.
    * In every other case **omit the keys entirely**. Never send `null` or `""`.
  * If the journal event already contains `BodyID` (Update 12 and later) or `BodyName`, keep it. EDMC only adds what is missing.

### 3.12 `scanbarycentre/1` (strict)

* Source: the `ScanBaryCentre` event.
* **REQ**: `timestamp`, `event`, `StarSystem`, `StarPos`, `SystemAddress`, `BodyID`.
* **OPT** (numbers; senders SHOULD include them when present): `SemiMajorAxis`, `Eccentricity`, `OrbitalInclination`, `Periapsis`, `OrbitalPeriod`, `AscendingNode`, `MeanAnomaly`, plus `horizons` and `odyssey`.
* **ADD**: `StarPos`, with the cross-check.

### 3.13 `approachsettlement/1` (strict)

* Source: the `ApproachSettlement` event.
* **REQ**: `timestamp`, `event`, `StarSystem`, `StarPos`, `SystemAddress`, `Name`, `BodyID`, `BodyName`, `Latitude`, `Longitude`.
* **OPT**:
  * `MarketID`: "YOU MUST ABSOLUTELY SEND THIS WHEN IT IS PRESENT";
  * `StationGovernment`, `StationAllegiance`, `StationEconomy`;
  * `StationEconomies[]`, strict `{Name, Proportion}`;
  * `StationFaction`, strict `{Name, FactionState}`;
  * `StationServices[]` (strings);
  * `horizons`, `odyssey`.
  * The Station* fields were added in game Update 17 (2023-10).
* **DEL**: every `*_Localised` (`Name_Localised`, `StationGovernment_Localised`, `StationEconomy_Localised`, `StationEconomies[].Name_Localised`), plus anything else not listed. Whitelist.
* **ADD**: `StarSystem` and `StarPos`, with the cross-check.
* Drop the message when:
  * `Latitude` or `Longitude` is missing. This happens on login at a planetary port, a known game bug [EDMC].
  * No location is tracked yet. `ApproachSettlement` can come **before** `Location` at login [EDMC].

### 3.14 `dockinggranted/1` (strict, added 2024-01-23)

* Source: the `DockingGranted` event, `{timestamp, event, LandingPad, MarketID, StationName, StationType}`.
* **REQ**: `timestamp`, `event`, `MarketID`, `StationName`. **OPT**: `StationType`, `LandingPad` (int), `horizons`, `odyssey`.
* **DEL**: `StationName_Localised`, which some stations have.
* **Do NOT add** `StarSystem`, `StarPos` or `SystemAddress`. The schema is strict and the message would be rejected.

### 3.15 `dockingdenied/1` (strict, added 2024-01-23)

* Source: the `DockingDenied` event, `{timestamp, event, Reason, MarketID, StationName, StationType}`.
* **REQ**: `timestamp`, `event`, `MarketID`, `StationName`, `Reason`. **OPT**: `StationType`, `horizons`, `odyssey`.
* The same DEL and "do not add" rules as dockinggranted apply.

### 3.16 `fcmaterials_journal/1` (strict)

* Source: the journal `FCMaterials` event, after which you **read `FCMaterials.json`**. This is the Fleet Carrier bartender, selling on-foot micro-resources.
* **REQ**: `timestamp`, `event` (`"FCMaterials"`), `MarketID`, `CarrierName` (≥ 1 character), `CarrierID` (≥ 1 character), and `Items`.
  Each `Items[]` entry requires `id`, `Name`, `Price`, `Stock` and `Demand`. Items are not strict, but `*_Localised` is disallowed.
* **OPT**: `horizons`, `odyssey`.
* **DEL**: `Items[].Name_Localised`.
* Rules [EDMC]:
  * Re-read the file until its timestamp is within 5 s of the event, with up to 11 retries.
  * Do not resend when the `MarketID` and `Items` are identical to the last message.
  * Send only if the user enabled sending station data.

### 3.17 `fcmaterials_capi/1` (strict)

* Source: CAPI `/market` → `orders.onfootmicroresources`. EDMC reads it from `lastStarport.orders.onfootmicroresources`.
* Build the message from scratch:
  * `{"timestamp", "event":"FCMaterials", "MarketID": <market "id">, "CarrierID": <market "name">, "Items": <onfootmicroresources minus every "locName">}`.
  * **MUST NOT** set `CarrierName`.
  * **MUST NOT** build the message by stripping the full CAPI document.
* Shape of `Items`:
  * `{ purchases: [ {name, price, outstanding, total} ] }`, strict.
  * `sales` is either `[]` (no orders) **or an object** keyed by commodity id: `{"<id>": {id, name, price, stock}}`, strict.
* `horizons` and `odyssey`: set them only if you are reasonably sure a live game session matches this CAPI query.
* Header: `gameversion` is `"CAPI-Live-market"` or `"CAPI-Legacy-market"`, and `gamebuild` is `""`.

### 3.18 `scanorganic/1` (strict; live but only in the GitHub `develop` branch)

* Status [LIVE]: listed on the production monitor (2,480 messages on 2026-10-11), and 5 messages were relayed during a 6-minute capture.
  EDDI 5.0.3+ is a known sender. The schema comes from PR #230 by GitHub user `Silarn`, opened 2025-04-20 and merged to `develop` on 2026-06-17.
* Source: the `ScanOrganic` event.
* **REQ**: `timestamp`, `event`, `StarSystem`, `StarPos`, `ScanType`, `Genus`, `Species`, `SystemAddress`, `BodyID`.
  `ScanType` is an enum of only `"Log"` and `"Sample"`.
* **OPT**: `Variant` (send it when present), `BodyName`, `Latitude`, `Longitude`, `horizons`, `odyssey`.
* **Rename** the event's `Body` to `BodyID`.
* **Drop** events with `ScanType == "Analyse"`. That scan can fire later, even in another system.
* **DEL**: `*_Localised` (`Genus_Localised`, `Species_Localised`, `Variant_Localised`) and any key not in the schema.
  EDDI also removes `WasLogged`, a newer event field that the strict schema would reject.
* **ADD**: `StarSystem` and `StarPos`, with the cross-check.
* Optional additions:
  * `BodyName`: from Status.json or the journal, cross-checked.
  * `Latitude` and `Longitude`: from `Status.json`. EDDI adds them only when all of these hold:
    * the player is on foot on a planet;
    * the Status.json timestamp is within 60 s of the event;
    * the journal body name equals the Status.json body name.
* Header: the schema file omits `gameversion` and `gamebuild` from `header.properties`, but the header is non-strict, so **still send both**.

### 3.19 `blackmarket/1`: deprecated, do not send

Since September 2017 the same data has been part of `commodity/3` `prohibited`. The schema is still registered in the GitHub
`Settings.py`, but nobody sends it, and issue #150 about removing it entirely was closed in 2025.

### 3.20 Outdated schemaRefs (these return 426)

Any `http://schemas.elite-markets.net/eddn/{commodity/1,2,3, outfitting/1,2, shipyard/1,2, blackmarket/1, journal/1}[/test]` [CODE-EDDN Settings.py].

---

## 4. Market, Outfitting and Shipyard conversion from the companion files

The journal events `Market`, `Outfitting` and `Shipyard` are **signals** that the game has written `Market.json`, `Outfitting.json` or
`Shipyard.json` in the journal folder. Read the file and use its content.

* EDMC resets its "last sent" caches when the `MarketID` changes.
* EDMC does **not** validate the file timestamp for these three files. It does for NavRoute and FCMaterials.
* Recommended for Elite Nexus:
  * Require `file.timestamp == event.timestamp` and `file.MarketID == event.MarketID`.
  * Re-read for up to about 5 s if they do not match yet, then give up.
* The files are written only when the CMDR opens that station service, so they always describe the station the CMDR is at.
* **For CAPI data you MUST verify** [DOC: Developers.md “Detecting CAPI data lag”]:
  1. `/profile` → `commander.docked == true`. EDMC also accepts the case where the CMDR is on foot at a station.
  2. `/market` → `lastStarport.name` and `lastSystem.name` must equal the last journal `Docked` or `Location` station and system. **Abort** on any mismatch.
* Do not send an empty `modules` or `ships` list. An empty `commodities` list **is** sent, and is meaningful for a Fleet Carrier without orders.
* Do not resend identical data for the same `MarketID`. EDMC compares with the last list sent. EDDI suppresses repeat outfitting messages within 2 minutes.

### 4.1 `Market.json` → `commodity/3` [EDMC `export_journal_commodities`; EDDN `scripts/testing/gateway-responses/market.json-to-schema.py`]

`Market.json` structure:

```text
{ "timestamp", "event":"Market", "MarketID", "StationName", "StationType", "CarrierDockingAccess"?, "StarSystem",
  "Items":[ { "id", "Name":"$gold_name;", "Name_Localised", "Category":"$MARKET_category_metals;", "Category_Localised",
              "BuyPrice", "SellPrice", "MeanPrice", "StockBracket", "DemandBracket", "Stock", "Demand",
              "Consumer", "Producer", "Rare" } ] }
```

| EDDN `message` key | From |
|---|---|
| `systemName` | `StarSystem` |
| `stationName` | `StationName` |
| `marketId` | `MarketID` |
| `timestamp` | file `timestamp` |
| `stationType` | `StationType` |
| `carrierDockingAccess` | `CarrierDockingAccess`, only if present and non-empty |
| `horizons`, `odyssey` | LoadGame (§5) |
| `commodities[]` | built from `Items[]` as below |

| Commodity key | From `Items[]` |
|---|---|
| `name` | `Name` with the leading `$` and trailing `_name;` removed: `"$gold_name;"` → **`"gold"`** |
| `meanPrice` | `MeanPrice` |
| `buyPrice` | `BuyPrice` |
| `stock` | `Stock` |
| `stockBracket` | `StockBracket` |
| `sellPrice` | `SellPrice` |
| `demand` | `Demand` |
| `demandBracket` | `DemandBracket` |

* Name canonicalisation (EDMC):
  * The regex is `\$(.+)_name;`, applied with Python `re.match`, so it is anchored at the start. Take group 1.
  * If the pattern does not match, the name is passed through unchanged.
  * EDMC does not lowercase journal names. They are already lowercase in practice.
  * Recommended: canonicalise, then lowercase. That matches the CAPI path, which uses `name.lower()`.
* Do **not** copy `id`, `Category*`, `*_Localised`, `Consumer`, `Producer` or `Rare`.
* **Rare goods are not excluded.** Only the `Rare` and `Producer` *flags* are dropped (the schema disallows them).
  CAPI can express rarity through `statusFlags`. Commodities with zero stock and zero demand are kept too.
* Do **not** send `economies` or `prohibited`. They do not exist in `Market.json`, and empty lists are forbidden.
* `statusFlags`, optional but recommended. The README says to build it from the Market.json booleans and to "do include it if available".
  * EDDI's `CommoditySchema.cs`, which EDDiscovery also follows [LIVE], builds a set:
    `"Producer"` if `Producer == true`, `"Consumer"` if `Consumer == true`, `"Rare"` if `Rare == true`.
  * Set `statusFlags` only when that set is non-empty (`minItems 1`, unique).
  * [LIVE, 150 s sample] journal-sourced flags were `Rare`, `Consumer` and `Producer`, all capitalised, from EDDiscovery, EDDI and EDO Materials Helper.
    One sender in an earlier sample used lowercase `rare`. CAPI-sourced flags (EDMC) were lowercase `powerplay`.
  * EDMC 6.1.x does not add statusFlags for journal data.
* NonMarketable filtering for journal data:
  * `Market.json` has no `categoryname` or `legality` fields. Its `Category` is a symbol such as `$MARKET_category_metals;`.
  * EDDI drops items whose `Category` normalises to `nonmarketable` (limpets) and items named `drones`. EDMC sends every `Items[]` entry.
  * Recommended: drop `NonMarketable`, as the README requires for CAPI data. Keep entries with zero stock and zero demand, as both tools do.
* Empty lists: EDMC sends an empty `commodities` list for a Fleet Carrier with no orders. EDDI does not send journal markets with no items. Both are valid.
* Name canonicalisation as EDDI does it: remove `$`, `_name` and `;` (plain string replaces). The result is the same as EDMC's regex in practice.
* EDMC sorts by `name`. That is not required but is nice for diffs.

**CAPI `/market` → `commodity/3`** [EDMC `export_commodities`]:

* Commodity filter: skip items where `categoryname == "NonMarketable"` (limpets) or where `legality` is non-empty.
* Commodity mapping:
  * `name` = `commodity.name.lower()`;
  * `meanPrice`, `buyPrice`, `stock`, `sellPrice` and `demand` are cast to int;
  * `stockBracket` and `demandBracket` are copied as-is (they may be `""`);
  * `statusFlags` is copied when non-empty and **de-duplicated**, keeping order. That is EDMC's develop branch, 2026-08-15.
    CAPI can return `["powerplay","powerplay"]` when the CMDR is pledged and at a Powerplay station, and the gateway rejected that with a 400 in early 2026 (EDMC issue #2601).
* Station fields:
  * `systemName` = `lastSystem.name`;
  * `stationName` = `lastStarport.name`;
  * `marketId` = `lastStarport.id`;
  * `timestamp` = the CAPI response time.
* `economies` = the values of `lastStarport.economies`, which are `{name, proportion}` objects, sorted by name.
* `prohibited` = the values of `lastStarport.prohibited`, sorted.
* `horizons`: if the game is not known to be running, synthesise it with `capi_is_horizons()` (§5).
* `odyssey`: from LoadGame.
* Header: `gameversion` = `"CAPI-Live-market"`, `gamebuild` = `""`.

### 4.2 `Outfitting.json` → `outfitting/2` (or `/3`) [EDMC `export_journal_outfitting` / `export_outfitting`; EDDI `OutfittingSchema`]

`Outfitting.json` has the shape `{timestamp, event:"Outfitting", MarketID, StationName, StarSystem, Horizons, Items:[{id, Name:"hpt_pulselaser_fixed_small", BuyPrice[, BuyMercCoinsPrice]}]}`.

| EDDN key | From |
|---|---|
| `systemName` | `StarSystem` |
| `stationName` | `StationName` |
| `marketId` | `MarketID` |
| `timestamp` | `timestamp` |
| `horizons` | EDMC: the file's `Horizons`. EDDN docs: LoadGame. They are the same in practice. |
| `odyssey` | LoadGame |
| `modules` | as below |

**Module-name filter and normalisation** (EDMC, verbatim regex):

```python
MODULE_RE = re.compile(r'^Hpt_|^Int_|Armour_', re.IGNORECASE)
# Journal: keep every Items[].Name except 'int_planetapproachsuite', capitalise the matched prefix:
sorted({ MODULE_RE.sub(lambda m: m.group(0).capitalize(), mod['Name'])
         for mod in modules if mod['Name'] != 'int_planetapproachsuite' })
# CAPI: filter, then lowercase, then capitalise the prefix
filter(lambda m: MODULE_RE.search(m['name']) and m.get('sku') in (None, 'ELITE_HORIZONS_V_PLANETARY_LANDINGS')
                 and m['name'] != 'Int_PlanetApproachSuite', modules.values())
sorted({ MODULE_RE.sub(lambda m: m.group(0).capitalize(), mod['name'].lower()) for mod in to_search })
```

What the normalisation does:

* `hpt_pulselaser_fixed_small` → `Hpt_pulselaser_fixed_small`.
* `int_engine_size3_class5` → `Int_engine_size3_class5`.
* `sidewinder_armour_grade1` → `sidewinder_Armour_grade1`.
* The rest of the name keeps the source case. Journal names are lowercase. EDMC lowercases CAPI names first.

The schema pattern is case-sensitive: `(^Hpt_|^hpt_|^Int_|^int_|_Armour_|_armour_)`. Raw journal names already match it, which is
why EDDI sends them unmodified. EDMC's capitalisation is cosmetic, but it guarantees a match.

Recommended for Elite Nexus:

* Apply EDMC's journal rule, and also apply the regex **filter** to journal items. EDMC trusts that Outfitting.json holds only modules; filtering is safer.
* **De-duplicate**: v2 and v3 both declare `uniqueItems: true`. EDMC's develop branch now builds a set.
* Sort the result.
* For **v3**, emit `{id, Name, BuyPrice, BuyMercCoinsPrice (default 0)}` for each kept item.
  * Keep `Name` exactly as in the file. Do not capitalise it.
  * Optionally also exclude PowerPlay modules, as EDDI does.

### 4.3 `Shipyard.json` → `shipyard/2` [EDMC `export_journal_shipyard` / `export_shipyard`]

`Shipyard.json` has the shape `{timestamp, event:"Shipyard", MarketID, StationName, StarSystem, Horizons, AllowCobraMkIV, PriceList:[{id, ShipType:"sidewinder", ShipType_Localised, ShipPrice}]}`.

* `ships` = `sorted(set(PriceList[].ShipType))`.
  * Use the raw symbol: lowercase, for example `sidewinder`, `krait_light`, `type9_military`, `empire_courier`.
  * Never use `ShipType_Localised`. No other renaming is done.
* `allowCobraMkIV` = `AllowCobraMkIV`, optional.
* `horizons`: the file's `Horizons` (EDMC) or LoadGame. `odyssey`: LoadGame.
* CAPI: `ships` = the lowercased `name` of each value in `ships.shipyard_list`, plus each entry of `ships.unavailable_list`.
  * EDDiscovery sends CAPI CamelCase such as `SideWinder` [LIVE]. The schema allows any case, so listeners case-fold.
  * Prefer lowercase.
* EDMC's journal code reads `entry.get('Pricelist')`, with a lower-case "l", but the file key is `PriceList`. This looks like an EDMC bug.
  * Consistent with that, all 39 EDMC `shipyard/2` messages seen in 6 minutes were CAPI-sourced, with `gameversion` `CAPI-Live-shipyard` [LIVE].
  * Use **`PriceList`**, which the schema's `"renamed": "PriceList"` confirms.
* [LIVE] Journal-sourced `ships` values from other tools include `mediumtransport01`, `panthermkii` and `mandalay`. New ship symbols simply pass through.
* Name case differs between senders. EDMC lowercases. EDDiscovery sends `SideWinder` and `Krait_MkII`. EDDI's CAPI commodity names arrive in CamelCase, such as `OccupiedCryoPod`.
  **Listeners must case-fold.** Senders should lowercase.

---

## 5. `horizons` / `odyssey` flags [DOC: Developers.md; EDMC; EDDI]

* **The only source is the `LoadGame` event**: `LoadGame.Horizons` and `LoadGame.Odyssey`. CAPI journals include LoadGame too.
* Include each key **only if** you have a boolean for it. "**DO NOT INCLUDE IT** … No, not with a `false` value" when `LoadGame`
  has no `Odyssey` key. `null` fails validation.
* `Fileheader.Odyssey` has **different semantics**: it means "4.0 client". Never use it.

| Client | Fileheader | LoadGame |
|---|---|---|
| Base 3.8 | `"Odyssey":false` | `"Horizons":false` |
| Horizons 3.8 (Legacy) | `"Odyssey":false` | `"Horizons":true` (no `Odyssey` key → omit `odyssey`) |
| Horizons 4.0 (Live) | `"Odyssey":true` | `"Horizons":true,"Odyssey":false` |
| Odyssey 4.x (Live) | `"Odyssey":true` | `"Horizons":true,"Odyssey":true` |

Production today: nearly every message carries `"horizons":true,"odyssey":true`.

* Every schema except blackmarket allows both keys as optional booleans. Add them to **every** message.
* For `fsssignaldiscovered/1` they are SHOULD rather than MUST.
* For `fcmaterials_capi/1`, add them only when a live game session matches the CAPI query.

CAPI-sourced `horizons` (commodity, outfitting, shipyard):

* If the game is known to be running and logged in, use LoadGame.
* Otherwise synthesise the flag (EDMC `capi_is_horizons`). It is **true** if any of these holds:
  * any `/market` economy has `name == "Colony"`;
  * any `/shipyard` module has `sku == "ELITE_HORIZONS_V_PLANETARY_LANDINGS"`;
  * any `ships.shipyard_list` entry has that same `sku`.
* Do **not** use the Cobra Mk IV or Guardian FSD booster SKUs.
* There is no way to derive `odyssey` from CAPI data. Use LoadGame and trust the system and station name check.

What EDMC actually does when a value is unknown [EDMC `monitor.py` and `plugins/eddn.py`]:

* Initial state: `Horizons: None`, `Odyssey: False`.
* On LoadGame: `Horizons = entry['Horizons']` and `Odyssey = entry.get('Odyssey', False)`.
* Every journal entry passed to EDDN gets `entry['horizons'] = state['Horizons']` and `entry['odyssey'] = state['Odyssey']`.
* So EDMC **always sends `odyssey`, as `false` when unknown**, which contradicts the docs, and it would send `horizons: null` if it ever sent before LoadGame.
* For CAPI messages EDMC uses `capi_is_horizons()` for `horizons` and the journal-tracked `odyssey`, defaulting to false.
* For journal Outfitting and Shipyard messages EDMC uses the companion file's own `Horizons` field.
* EDDI is closer to the docs:
  * it uses nullable flags from LoadGame, with Outfitting/Shipyard updating `Horizons`;
  * it adds the flags only if they are known **and** an `EliteDangerous64` process is running.
* **Follow the docs: omit what you do not know.**

---

## 6. Subscriber (Relay)

Endpoint: **`tcp://eddn.edcd.io:9500`** (live). The README writes it as `tcp://eddn.edcd.io:9500/`; ZeroMQ wants it without the slash.
Beta is `tcp://beta.eddn.edcd.io:9510` and dev is `tcp://dev.eddn.edcd.io:9520`, available only occasionally. There is no TLS and no authentication.

Wire facts, verified [LIVE 2026-10-12] with a hand-written ZMTP client:

| Property | Observed |
|---|---|
| Protocol | ZMTP **3.1**, NULL mechanism |
| Peer socket type | **`XPUB`**, so subscriptions are forwarded and **filtering happens on the server** |
| Your socket type | **`SUB`** |
| Subscription | **must be `""`** (empty). See the note below. |
| Framing | **single-frame** messages; no topic frame and no topic prefix |
| Payload | **zlib (RFC 1950)** compressed UTF-8 JSON. The first bytes were always `0x78 0x5E`. Decompress with `flate2::read::ZlibDecoder`, not a gzip decoder. |
| Payload JSON | `{"$schemaRef", "header", "message"}`. Key order is the **uploader's** order and is not sorted. |
| Header seen by listeners | `uploaderID` (64-hex hash, ephemeral), `softwareName`, `softwareVersion`, `gameversion`, `gamebuild`, `gatewayTimestamp` (UTC, microseconds, `Z`). No `uploaderIP`. |
| Heartbeat | No application-level heartbeat messages. The relay **answers ZMTP PING with PONG**, so libzmq `ZMQ_HEARTBEAT_IVL`/`ZMQ_HEARTBEAT_TIMEOUT` work. |
| Volume | about **8.5 messages/s** on average (about 30,800/h in a 7-minute sample, matching the monitor's 30,108 in the last 60 minutes). About 1.76 M messages/day (Sunday 2026-10-11). Compressed bandwidth is roughly 7–8 KB/s on average, about 90 MB/h decompressed. |
| `/test` traffic | **Expect it to be relayed.** The old relay subscribed to every registered schemaRef, `/test` ones included, and the live monitor counts accepted `journal/1/test`, `commodity/3/test` and similar (a handful to about 100 a day). None arrived during the roughly 12 minutes of captures that tallied every schema. **Drop every `$schemaRef` ending in `/test`.** |
| Duplicates | **Assume they are not removed.** GitHub's relay dedupe has been disabled since 2026-03-22. The new relay's behaviour was not tested. Dedupe yourself (below). |

Note on topic filters:

* Developers.md says the old ZeroMQ "does not support server-side subscription filtering … the dropping of not-matching
  messages will happen at your end". That is **outdated**. Today the XPUB relay *does* filter on the server.
* Because the payload starts with the zlib header and carries no topic, every non-empty schema filter matches nothing.
* Test result: subscribing to `"https://eddn.edcd.io/schemas/journal/1"` delivered **0** messages in 10 s. Subscribing to `""` or to `"x^"`
  (the zlib header bytes) delivered everything.
* Filter by `$schemaRef` **after** decompressing.

Per-schema volume (last 60 minutes on the monitor at 2026-10-12 09:14 UTC, a Monday morning UTC, and the previous day's total):

| Schema | msgs / last 60 min | Yesterday (Sun 2026-10-11) |
|---|---|---|
| journal/1 | 15,218 | 909,121 |
| fsssignaldiscovered/1 | 3,591 | 184,958 |
| fssbodysignals/1 | 2,163 | 126,676 |
| commodity/3 | 1,481 | 92,213 |
| fssdiscoveryscan/1 | 1,478 | 85,451 |
| dockinggranted/1 | 1,328 | 82,211 |
| navroute/1 | 1,294 | 77,400 |
| scanbarycentre/1 | 723 | 45,469 |
| fssallbodiesfound/1 | 714 | 41,337 |
| outfitting/2 | 671 | 36,370 |
| shipyard/2 | 512 | 26,902 |
| approachsettlement/1 | 323 | 13,194 |
| codexentry/1 | 196 | 9,873 |
| dockingdenied/1 | 132 | 14,774 |
| outfitting/3 | 109 | 5,010 |
| fcmaterials_capi/1 | 63 | 3,075 |
| navbeaconscan/1 | 48 | 2,612 |
| fcmaterials_journal/1 | 38 | 696 |
| scanorganic/1 | 26 | 2,480 |
| **All messages** | **30,108** | **1,759,145** (valid 1,750,912 / warning 4,456 / failed 3,777) |

Timing and versions seen by the relay [LIVE, 2,899 messages over 5 minutes]:

* `gatewayTimestamp − message.timestamp` had a median of **2.4 s**, a p95 of **19 s** and a maximum of 38 min, with none negative. Queued retries cause the long tail.
* `gameversion` values:

  | Value | Share |
  |---|---|
  | `4.4.1.1` | 94 % |
  | `CAPI-Live-market` | 3 % |
  | `CAPI-Live-shipyard` | 2.5 % |
  | `4.1.2.100` | a handful, all from old EDMC 5.13.1 |

* `gamebuild` was `"r332841/r0 "` (with the trailing space) or `""`.
* **Every** message carried both `horizons` and `odyssey`.
* journal/1 event mix over 6 minutes:

  | Event | Messages |
  |---|---|
  | Scan | 1,156 |
  | FSDJump | 305 |
  | Docked | 160 |
  | SAASignalsFound | 40 |
  | Location | 36 |
  | CarrierJump | 1 |

Reconnect and robustness:

* The live service restarts every **Thursday at 07:06 UTC** (README), and the operators also redeploy ad hoc. **Expect disconnects.**
* **libzmq** (Rust crate `zmq`) reconnects automatically (`ZMQ_RECONNECT_IVL`, default 100 ms; also set `ZMQ_RECONNECT_IVL_MAX`, for example 30 s) and replays the
  subscription. Turn on `ZMQ_HEARTBEAT_IVL` (for example 10 s) and `ZMQ_HEARTBEAT_TIMEOUT` (30 s) so that half-open TCP connections are detected.
* The **pure-Rust `zeromq` crate** (zmq.rs) is what the official EDDN Rust example uses (`develop` branch, `examples/Rust/minimal-example-tokio`, PR #227).
  Its automatic reconnect and heartbeat support is less mature than libzmq's. That was not verified here, so check the version you use.
  Also run an application **watchdog**: if no message arrives for 60–120 s (normal traffic is several per second), drop the socket and reconnect with backoff,
  starting at 5 s and capping at 60 s.
* The EDDN Python example uses `RCVTIMEO = 600000` ms. On timeout or a ZMQError it disconnects, sleeps 5 s and reconnects.
* The old relay set `SNDHWM = 500` per subscriber, and a slow consumer **loses** messages.
  * Keep the receive loop minimal: receive, then push onto a bounded channel.
  * Decompress and parse in a worker.
  * Never block the socket task on database I/O.
* There is no historical replay. Anything missed while disconnected is gone.
  Archives exist at <https://edgalaxydata.space/>, along with the Spansh and EDSM dumps.

Listener hygiene:

* Validation already happened at the gateway, so there is no need to re-validate. Dispatch on the exact `$schemaRef`.
* Treat everything as **untrusted**:
  * augmented `StarSystem`/`StarPos` can be wrong (a known risk the docs call out);
  * `uploaderID` cannot be used for identity;
  * use `softwareName`/`softwareVersion` for allow and deny decisions, for example ignoring known-bad versions.
* `horizons` and `odyssey` can be missing. Timestamps may carry fractional seconds.
* Dedupe suggestion, the old relay algorithm from `DuplicateMessages.py` (`RELAY_DUPLICATE_MAX_MINUTES` was 15):
  1. Key = SHA-256 of `{"$schemaRef", "message"}` with sorted keys and without `header`.
  2. Before hashing:
     * remove `message.timestamp`;
     * replace `StarPos` with `round(x*32)` integers;
     * replace `DistFromStarLS` with `int(x+0.5)`;
     * remove `ScanType` and `DistanceFromArrivalLS`.
  3. Drop the message if the same key was seen within 15 minutes. Never dedupe `/test`.
* Station data is time-sensitive. Use `message.timestamp`, not `gatewayTimestamp`, for the age of market data, and reject future-dated messages beyond a small tolerance.

Rust skeleton, condensed from the official example (`EDCD/EDDN` `develop:examples/Rust/minimal-example-tokio/src/main.rs`, which uses `zeromq` 0.4.1, `tokio`, `flate2` and `serde_json`):

```rust
let mut socket = zeromq::SubSocket::new();
socket.connect("tcp://eddn.edcd.io:9500").await?;
socket.subscribe("").await?;                     // MUST be empty
loop {
    let msg = socket.recv().await?;              // single frame
    let mut d = flate2::bufread::ZlibDecoder::new(&msg.get(0).unwrap()[..]);
    let mut s = String::new(); d.read_to_string(&mut s)?;
    let v: serde_json::Value = serde_json::from_str(&s)?;
    let schema = v["$schemaRef"].as_str().unwrap_or("");
    if schema.ends_with("/test") { continue; }
    // dispatch on schema ...
}
```

---

## 7. What changed in EDDN, 2024–2026

| Date | Change | Source |
|---|---|---|
| 2024-01-23 | **`dockinggranted/1` and `dockingdenied/1` schemas added** and registered on the live gateway. | PR #219, commit `cd07f7f3c5`, live PR #221/#222 |
| 2024-01-23 | **`commodity/3` gains optional `stationType` and `carrierDockingAccess`.** The old disallowed `StationType` was removed. | PR #220, commit `03729d0f8e` |
| 2024-01-23 | Relay `uploaderID` hash nonce rotation cut from **12 h to 3 min**. | commit `764d14dbe1` |
| 2024-01-24 | `commodity/3` `renamed` annotations fixed (`renaamed` typo, `stationName`→`StationName`). No effect on validation. | commits `0adcc3713e`, `e874815a6e` |
| 2024-10-31 | **`fsssignaldiscovered/1` signals gain optional `SpawningPower` and `OpposingPower`** (Powerplay 2.0). Merged to live the same day. | commits `60f80cfe69`, `a4475a7177`; live `1742a1c96e`, `72905efab3` |
| 2024-12 → 2025-03 | `pledged` flag proposal (PR #225) closed unmerged. | PR #225 |
| 2025-01 | `materialtrade/1` and `technologybroker/1` proposals (PR #226) closed unmerged. A **Rust listener example was merged** into `develop` (PR #227, 2025-01-09) at `examples/Rust/minimal-example-tokio`, using `zeromq` 0.4, tokio and `flate2`. | PRs #226, #227 (state checked via the github.com PR pages) |
| 2025-03-03 | About 20 long-open issues and PRs closed in one sweep. The closer of #210 was `robbyxp1`. Among them: #210 "Investigate verifying EDDN Senders" ("No interest in doing this"), #106 duplicate detection in the gateway, #150 "Truly deprecate the blackmarket schema", #196 making the horizons and odyssey flags mandatory, #180 coping with upload rates, and #168 re-architecting the code. | GitHub issues API listing, issue #210 page |
| 2025-11-16 | Issue #233, "Commodity schema – renamed missing", opened. It is still open and is documentation-only. | issue #233 |
| 2026-03-22 | **Relay de-duplication disabled**: `RELAY_DUPLICATE_MAX_MINUTES = 15` → `False`. Listeners now receive duplicates. | commit `d9204abdd7`, live `5495a79c80`, PR #234 |
| 2026-06-17 | **`scanorganic/1` schema** (PR #230, proposed 2025-04) merged to `develop` and registered in develop's `Settings.py`. **Live accepts it** [LIVE]. | PR #230, commit `02c7c9c960` |
| 2026-06-17 | Live-branch sync. | PRs #236, #237, #238 |
| 2026-06-30 | Game: Elite Dangerous **"Operations"** update, which adds Merc Coin, a currency used to buy ship modules. | elitedangerous.com |
| 2026-07-01 | PR #239 (a `mercmodules` array in outfitting/2) closed unmerged. | PR #239 |
| 2026-07-03/06 | **`outfitting/3` schema** added to `master`, with modules as `{id, Name, BuyPrice, BuyMercCoinsPrice}`. The README requires a `BuyMercCoinsPrice: 0` augmentation. **Live accepts it** [LIVE]. EDDI 5.0.3+ sends it. | PRs #240, #242, commits `f4c1a4963e`, `c6b69c8aca`, `212e803915` |
| 2026-07-06 | `scanorganic-v1.0.json` `$schema` URI fixed (develop). | PR #241 |
| Probably between 2026-03 and 2026-10-12 (exact date unknown) | **Production gateway, relay and monitor replaced by a new implementation that is not on GitHub.** The date range is inferred: users still got old-style 400s for duplicate items in March 2026, and in October duplicates are "accepted temporarily". Evidence from [LIVE]: a new monitor page with VALID/WARNING/FAILED/Signed counters; "duplicate items accepted temporarily" warnings; an `X-Signature` CORS header; a 400 `Empty request body`; an XPUB relay that filters on the server; 64-hex uploader hashes; the old `/stats/` endpoints gone (404); h2 offered through ALPN. | §1.1, §6 |
| 2026-01 → 2026-08 | **Duplicate-item problem.** From January 2026, CAPI `/market` returned `statusFlags` with a duplicate `"powerplay"` entry when the CMDR was pledged and docked at a Powerplay station. The gateway then answered **400** `FAIL: Schema Validation: [<ValidationError: "['powerplay', 'powerplay'] has non-unique elements">]`. EDMC users' queues stalled for weeks behind the rejected message (EDMC issue #2601). EDMC `develop` fixed it on 2026-08-15 by de-duplicating `statusFlags` and outfitting modules (commits `f6eeecf08b`, `0f3bd73161`, `b64e32564f`). By 2026-10 the new gateway accepts duplicates as "WARNING … accepted temporarily". | EDMC issue #2601, EDMC commit history |
| 2026, exact date unknown | **Signed uploads.** An optional `X-Signature` header carrying HMAC-SHA256 of the uncompressed body with an EDDN-issued per-app key. The gateway counts them as "Signed". The gateway can return 403 (rejected) and 202 (accepted). EDDI 5.0.6 is the first known signer. | EDDI `EDDN_Sender.cs`, `BuildSecrets/*`, `Tests/EddnSigningTests.cs`; [LIVE] CORS header and monitor |

---

## 8. Rate limits, blocking and "don't do this"

There are **no documented rate limits and no published blocklist mechanism**. What exists:

* **426** for outdated `$schemaRef` values (old `schemas.elite-markets.net` URLs).
* EDCD maintainers monitor gateway error logs grouped by `softwareName`/`softwareVersion` and contact developers.
  `scripts/eddn-report-log-errors` contains per-tool exception lists, for EDDiscovery, EDDLite, EDDI and EDMC.
* The new public monitor shows each software version's **WARNING and FAILED counts** to everyone.
* The new gateway also counts "Signed" uploads: an `X-Signature` HMAC-SHA256 made with an EDDN-issued key (§1.7).
  EDDN has not documented it. The details come from EDDI's source. Do not send the header unless you hold a key.
* The new gateway can answer **403 Forbidden**. EDDI's sender treats 403 as "EDDN rejected {signed|unsigned} upload … Check app signing
  configuration" and never retries it. This is the most likely shape of a server-side block.

That an uploader gets blocked for non-compliance is plausible: the operators run their own gateway, and 403 handling exists in a current client.
The exact policy is **not documented**. Assume a misbehaving `softwareName`/`softwareVersion` can be refused server-side, and design for a quick off switch:

* A remote **kill switch** that you can flip without a release. EDMC fetches
  `https://raw.githubusercontent.com/EDCD/EDMarketConnector/releases/killswitches_v2.json`, with keys such as `plugins.eddn.send`,
  `plugins.eddn.journal.event.<Event>` and `eddn.capi_export.*`.
* A per-schema enable/disable in config.

Self-imposed limits (good-citizen defaults, based on EDMC):

* At most about **2 requests/s** sustained.
* One request in flight at a time.
* At least 60 s before any retry.
* Persist the queue, and expire entries after a few hours.

**Don't:**

* send non-live (alpha or beta) game data to live schemas, or test your code against live schemas. Use `/test`.
* send `gatewayTimestamp`, `null` flags, or `odyssey:false` when LoadGame has no `Odyssey` key.
* derive `horizons` or `odyssey` from `Fileheader`.
* send any `_Localised` key or any personal data. That includes:
  * Wanted, ActiveFine, CockpitBreach, BoostUsed, FuelLevel, FuelUsed, JumpDist, Latitude and Longitude (in journal/1);
  * Faction reputation and home keys;
  * Progress, IsNewEntry, NewTraitsDiscovered, TimeRemaining and locName.
* add StarSystem, StarPos or SystemAddress without the SystemAddress or name cross-check. Never add them to `dockinggranted` or `dockingdenied`.
* forward raw events to strict schemas. Whitelist.
* send CodexEntry on `journal/1`, `blackmarket/1` at all, ScanOrganic `Analyse`, MissionTarget USS signals, or CQC or multicrew-as-crew data.
* send empty `modules` or `ships`, empty `economies` or `prohibited`, duplicate array items, or a NavRoute without a `Route` or from `NavRouteClear`.
* resend unchanged station data for the same MarketID, or upload journal history during startup catch-up.
* retry 400, 403 or 426, retry faster than once a minute, run parallel bursts, form-encode, use plain HTTP, or drop the trailing slash of `/upload/`.
* strip the trailing space in `gamebuild`, or use the journal's `gameversion` for CAPI-sourced data.
* reuse another tool's `softwareName`, or ship changed message logic without bumping `softwareVersion`.

---

## 9. Not verified / open questions

1. **The new production gateway is not in a public repository.** These are inferred only from docs, the old code, and harmless GETs:
   * exact 400, 413 and 426 bodies;
   * the size limit (1 MiB compressed in the old code);
   * whether `deflate` is still accepted;
   * whether "WARNING — accepted temporarily" turns into FAILED, and when.

   Nothing was POSTed, as instructed. To verify, POST to the `/test` schemas from a dev build.
2. **`X-Signature`**: the format is known from EDDI's source: lowercase hex HMAC-SHA256 of the uncompressed body, with an EDDN-issued key.
   Still unknown:
   * how to obtain a key;
   * what signing buys you (priority? trust flags for listeners? exemption from some checks?);
   * whether unsigned uploads will ever be restricted.

   Signing is not shown in the relayed messages: there are no extra header keys.
3. **403 and 202 semantics** are inferred from EDDI's sender and tests only.
4. **outfitting/2 vs /3 policy**: there is no deprecation notice. It is unclear whether v2 should exclude Merc-Coin-only modules.
5. **Duplicate handling in the new gateway and relay** was not tested. The GitHub relay setting has been disabled since 2026-03-22. Listeners should dedupe, and senders should not rely on anyone else deduping.
6. Whether the **Legacy (3.8) galaxy** still runs in 2026 could not be confirmed, because the web-search budget ran out. The CAPI-Legacy rules are kept for completeness.
7. The exact `gameversion` string of beta or PTS game builds. EDMC's rule is "contains alpha or beta".
8. The `statusFlags` vocabulary is not defined by EDDN. Practice is `Producer`, `Consumer` and `Rare` for journal data (EDDI source and LIVE observation), and lowercase `powerplay` for CAPI data. CAPI may produce other values.
9. **Operations-mode data.** Operations scenario signals appear on EDDN, for example `$MULTIPLAYER_SCENARIO80_TITLE;`, and no EDDN guidance on Operations instances exists yet.
   EDDI raises a "game mode changed" event for Operations. Consider not uploading from inside an Operation until guidance appears.
10. The Thursday 07:06 UTC restart comes from the README of the old infrastructure. It may no longer apply.
11. The per-hour volumes are a single sample (Monday morning UTC). Weekend and evening peaks are probably 1.5–2× higher.

---

## 10. Sources (all fetched 2026-10-12)

EDDN repository (branch heads at research time: `master` 153e62426e 2026-07-12, `live` 4ad669bb7b 2026-06-17, `develop` 15052a271c 2026-07-12):

* Docs and schemas:
  * https://raw.githubusercontent.com/EDCD/EDDN/master/docs/Developers.md (identical on `live`)
  * https://raw.githubusercontent.com/EDCD/EDDN/master/schemas/README-EDDN-schemas.md
  * https://raw.githubusercontent.com/EDCD/EDDN/master/schemas/<schema>-v<N>.0.json and `<schema>-README.md`, for every file listed in §11
  * https://raw.githubusercontent.com/EDCD/EDDN/live/schemas/outfitting-README.md
  * https://raw.githubusercontent.com/EDCD/EDDN/develop/schemas/scanorganic-v1.0.json and `scanorganic-README.md`
  * https://raw.githubusercontent.com/EDCD/EDDN/master/schemas/TEMPLATES/journalevent-README.md
  * https://raw.githubusercontent.com/EDCD/EDDN/live/README.md (endpoints, Thursday restart)
  * https://raw.githubusercontent.com/EDCD/EDDN/live/docs/Contributing.md and `docs/Running-this-software.md`
* Server code (`live` branch):
  * https://raw.githubusercontent.com/EDCD/EDDN/live/src/eddn/conf/Settings.py (also the `master` and `develop` copies, which were diffed)
  * https://raw.githubusercontent.com/EDCD/EDDN/live/src/eddn/Gateway.py, `Relay.py`, `Monitor.py`, `Bouncer.py`
  * https://raw.githubusercontent.com/EDCD/EDDN/live/src/eddn/core/Validator.py, `DuplicateMessages.py`, `StatsCollector.py`
  * https://raw.githubusercontent.com/EDCD/EDDN/live/contrib/monitor/js/eddn.js, `contrib/nginx-eddn.conf`, `docs/Anthor-VPS-nginx.conf`
  * https://raw.githubusercontent.com/EDCD/EDDN/live/scripts/eddn-report-log-errors, `scripts/apache-log-rate`
  * https://raw.githubusercontent.com/EDCD/EDDN/live/scripts/testing/gateway-responses/Market.json, `market.json-to-schema.py`, `commodity-outdated-schema.json`, `README.md`
  * https://raw.githubusercontent.com/EDCD/EDDN/live/examples/Python%203.4/Client_Complete.py
* GitHub API:
  * https://api.github.com/repos/EDCD/EDDN/contents/schemas?ref={master,live,develop}
  * https://api.github.com/repos/EDCD/EDDN/branches
  * https://api.github.com/repos/EDCD/EDDN/commits?sha={master,live,develop}
  * https://api.github.com/repos/EDCD/EDDN/issues?state=all&sort=updated
  * https://api.github.com/orgs/EDCD/repos?sort=pushed
* PR diffs: https://github.com/EDCD/EDDN/pull/{219,220,222,225,226,227,230,234,236,237,238,239,240,241,242}.diff
* Commit diffs: https://github.com/EDCD/EDDN/commit/{60f80cfe69,a4475a7177,cd07f7f3c5,764d14dbe1,e874815a6e,0adcc3713e,d9204abdd7,03729d0f8e,c6b69c8aca,667284c554,212e803915}.diff
* Issues: https://github.com/EDCD/EDDN/issues/210, https://github.com/EDCD/EDDN/issues/228, https://github.com/EDCD/EDDN/issues/233
* PR merged/closed state: the github.com pages https://github.com/EDCD/EDDN/pull/{225,226,227,230,239,240}
* The `examples/` folder listing on `master`, `develop` and `live`: https://github.com/EDCD/EDDN/tree/<branch>/examples

EDMarketConnector:

* https://raw.githubusercontent.com/EDCD/EDMarketConnector/main/plugins/eddn.py (identical to `stable` and `releases`) and https://raw.githubusercontent.com/EDCD/EDMarketConnector/develop/plugins/eddn.py
* https://raw.githubusercontent.com/EDCD/EDMarketConnector/develop/monitor.py, `companion.py`, `edmc_data.py`, `ChangeLog.md`, `killswitch.py`
* https://github.com/EDCD/EDMarketConnector/commits/develop/plugins/eddn.py: commit history
* https://github.com/EDCD/EDMarketConnector/issues/2601: duplicate `statusFlags` and the 400 response

EDDI:

* https://raw.githubusercontent.com/EDCD/EDDI/develop/EDDNResponder/EDDNResponder.cs
* https://raw.githubusercontent.com/EDCD/EDDI/develop/EDDNResponder/Schemas/OutfittingSchema.cs, `Schemas/ScanOrganicSchema.cs` and `Schemas/CommoditySchema.cs` (statusFlags, NonMarketable filter, name canonicalisation, 2-minute repeat suppression)
* https://raw.githubusercontent.com/EDCD/EDDI/develop/EDDNResponder/Toolkit/PersonalDataStripper.cs, `Toolkit/LocationAugmenter.cs`, `Toolkit/GameVersionAugmenter.cs`
* https://raw.githubusercontent.com/EDCD/EDDI/develop/EDDNResponder/Sender/EDDNBody.cs and `Sender/EDDNHeader.cs`
* https://raw.githubusercontent.com/EDCD/EDDI/develop/EDDNResponder/Sender/EDDN_Sender.cs: status handling, gzip on 413 and `X-Signature`
* https://raw.githubusercontent.com/EDCD/EDDI/develop/BuildSecrets/README.md, `BuildSecrets/BuildInjectedSecrets.cs`, `BuildSecrets/BuildInjectedSecrets.local.cs.example`, `BuildSecrets/EddnSigning.targets`
* https://raw.githubusercontent.com/EDCD/EDDI/develop/Tests/EddnSigningTests.cs: 202 is success, 403 is terminal, and the signature covers the uncompressed bytes
* https://github.com/EDCD/EDDI/tree/develop/EDDNResponder/Sender: directory listing, read through github.com HTML because the API was rate-limited
* https://raw.githubusercontent.com/EDCD/EDDI/develop/ChangeLog.md
* https://raw.githubusercontent.com/EDCD/EDMarketConnector/releases/killswitches_v2.json (location only; EDMC `killswitch.py`)

Live service (read-only):

* https://eddn.edcd.io/ (monitor page)
* `GET` and `OPTIONS` on https://eddn.edcd.io:4430/upload/, plus `GET` on https://eddn.edcd.io:4430/, `/health_check/` and `/stats/`, and `GET` on https://eddn.edcd.io:9090/stats/ (it timed out)
* ZeroMQ SUB to tcp://eddn.edcd.io:9500: 6 capture sessions of about 29 minutes in total and about 15,000 messages, plus PING/PONG and topic-filter probes. The scripts are in the research scratchpad and are not part of the project.

Game context:

* https://www.elitedangerous.com/news/update-operations: Operations update and Merc Coin. Seen as a search result and not fetched; the release date of 30 June 2026 is from the search summary.

---

## 11. Files saved next to this document

`docs/research/eddn-schemas/` holds verbatim copies, from `master` unless marked otherwise.

General docs:

* `Developers.md`
* `README-EDDN-schemas.md`

Schemas, each with its README:

* `approachsettlement-v1.0.json` + `approachsettlement-README.md`
* `blackmarket-v1.0.json` + `blackmarket-README.md`
* `codexentry-v1.0.json` + `codexentry-README.md`
* `commodity-v3.0.json` + `commodity-README.md`
* `dockingdenied-v1.0.json` + `dockingdenied-README.md`
* `dockinggranted-v1.0.json` + `dockinggranted-README.md`
* `fcmaterials_capi-v1.0.json` + `fcmaterials_capi-README.md`
* `fcmaterials_journal-v1.0.json` + `fcmaterials_journal-README.md`
* `fssallbodiesfound-v1.0.json` + `fssallbodiesfound-README.md`
* `fssbodysignals-v1.0.json` + `fssbodysignals-README.md`
* `fssdiscoveryscan-v1.0.json` + `fssdiscoveryscan-README.md`
* `fsssignaldiscovered-v1.0.json` + `fsssignaldiscovered-README.md`
* `journal-v1.0.json` + `journal-README.md`
* `navbeaconscan-v1.0.json` + `navbeaconscan-README.md`
* `navroute-v1.0.json` + `navroute-README.md`
* `outfitting-v2.0.json` + `outfitting-v3.0.json` + `outfitting-README.md`
* `scanbarycentre-v1.0.json` + `scanbarycentre-README.md`
* `shipyard-v2.0.json` + `shipyard-README.md`

From other branches:

* `live-branch_outfitting-README.md`: the `live` branch's v2-only README, which differs from `master`.
* `develop-branch_scanorganic-v1.0.json` + `develop-branch_scanorganic-README.md`: these exist only on `develop`. The JSON file starts with a UTF-8 BOM.
