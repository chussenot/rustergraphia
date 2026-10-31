# Web port specification: cistergraphia (Sinatra) → rustergraphia-web (axum)

Single source for porting the web layer of `/home/user/cistergraphia` (Ruby 3.3, Sinatra 4.2.1,
Rack 3.2.7, Erubi 1.13.1, rack-protection 4.2.1, Puma) to `crates/rustergraphia-web` (axum).
The bar: **byte-identical HTML** for the same inputs (so `public/css` and `public/js` work
unchanged), and **the same HTTP behaviour** (status, headers, redirects: none, error pages).

Everything marked *observed* was verified by running the Ruby app under rack-test (gems installed
in a scratch `BUNDLE_PATH`, see §14). The library (`lib/cistergraphia/**`) is being ported
separately (`PORTING.md`); this spec names the library calls the web layer needs (§13.3) but does
not specify them.

Source files covered: `app.rb`, `config.ru`, `config/puma.rb`, `Gemfile`, `Rakefile` (chapter
tasks), `web/*.rb`, `web/chapters.yml`, `views/*.erb`, `public/js/*.js`, `test/app_test.rb`,
`test/chapter_test.rb`, `test/ui/*`. The **literal text of every template is in `views/*.erb`**:
port it byte for byte; this document gives the logic, data and contracts around that text.

Conventions: `MAX_SVG_BYTES = 2_097_152` (2 × 1024 × 1024). "Escaped" = Erubi escaping (§8.1).
Ruby `nil` ↔ Rust `None`. "Raw" = inserted without escaping (`<%== %>`).

---

## 0. Index

1. Process, configuration, environment
2. Middleware stack and its semantics
3. Response headers and caching, per route
4. Request parsing rules
5. Routes
6. Error handling
7. Helpers (every function in `web/*.rb`), with exact strings
8. Templates: rendering semantics, then inventory
9. Markup contracts the JS relies on
10. `/ask` protocol, reader's game and notebook flow
11. The chapter system
12. Static assets
13. Porting checklist (axum)
14. Parity test plan (golden fixtures from the Ruby app)
15. Open questions and risks

---

## 1. Process, configuration, environment

| Ruby | Value | Rust equivalent |
|---|---|---|
| `PORT` | default `9292` (`config/puma.rb`); Docker sets `8080` | bind `0.0.0.0:$PORT`, default 9292 |
| `WEB_CONCURRENCY` | Puma worker processes, default 0 (single process) | ignore, or size the blocking pool |
| `PUMA_THREADS` | threads per process, default 3 | cap concurrent CPU-heavy requests (semaphore of N=3 per core-ish) around `spawn_blocking` |
| `FRONT_DOOR_KEY` | enables FrontDoor (§2.6) when non-empty | same |
| `CHAPTER_DATE` | overrides "today" for the chapter (§11.3) | same |
| `RACK_ENV` | `production` in Docker; development enables reloader and Sinatra host authorization (localhost only) | not needed; behave as production |

Health check: `GET /up` (§5.1). Logging: one CommonLogger line per request (address, time, method,
path+query, status, size, duration); **never log request bodies** (messages, passphrases, keys).
Backtraces of real bugs go to the log only.

---

## 2. Middleware stack and its semantics

Order, outermost first (Sinatra builds it so; the three `use` lines of `app.rb` come after
Sinatra's defaults):

1. `Rack::Head` — HEAD on any GET route answers with the GET's headers and no body.
2. `Rack::CommonLogger` (`enable :logging`).
3. `Rack::Protection` (Sinatra defaults, `reaction: :drop_session`, no session): FrameOptions,
   HttpOrigin, IPSpoofing, JsonCsrf, PathTraversal, XSSHeader. See §2.1.
4. `Rack::Protection::HostAuthorization` — production: every host allowed (no-op).
5. `Rack::Deflater` (§2.3).
6. `BodyLimit` with limit `MAX_SVG_BYTES + 64*1024 = 2_162_688` (§2.4).
7. `FrontDoor` (§2.5).
8. The app: static files first (§12), then `before` filter (§3), route, `after` filter.

### 2.1 Rack::Protection, as it actually behaves (observed)

Because Sinatra passes `reaction: :drop_session` and there is no session, the *denying*
protections **never deny**: a POST with a foreign `Origin` → 200; `X-Forwarded-For` vs
`X-Real-IP` mismatch → 200; foreign `Host` → 200. They only log a warning. Port: **do not**
implement Origin/IP checks (doing so would change behaviour).

What it does do:

- **XSSHeader**, on *every* response that passes through (pages, static, `/up`, `/ask`, the
  BodyLimit 413, the FrontDoor 403): `x-content-type-options: nosniff` (if not already set).
  Additionally `x-xss-protection: 1; mode=block` **only when** the Content-Type's `type/subtype`
  is `text/html` (rack-protection `html_types`: `text/html` and friends; in practice: HTML pages).
- **FrameOptions**: `x-frame-options: SAMEORIGIN` only on `text/html` responses.
- **PathTraversal**: before routing, `PATH_INFO` is rewritten: `%2e`→`.`, `%2f`→`/`, `%5c`→`\`
  (case-insensitive), `\`→`/`, split on `/`, drop empty and `.` segments, `..` pops; rejoin with a
  leading `/`; keep a trailing `/` if there were segments and the unescaped path ends in `/`,
  `/.` or `/..`. Observed: `GET /a/../about` → the About page (title `About the ciphers`,
  `og:url` …`/about`); `GET /css/app%2Ecss` → the stylesheet. Everything below (routing, static
  lookup, `request.path` in `og:url`, the leaf-header path test, FrontDoor's `/up` test, the
  chapter cache key) sees the **cleaned** path.

So JSON (`/ask`) and `text/plain` (`/up`) responses carry `nosniff` but not
`x-xss-protection`/`x-frame-options` (observed).

### 2.2 Host authorization

Production: allow all. (Development only allows localhost, `.localhost`, `.test`, IPs.) Port: none.

### 2.3 Rack::Deflater (gzip only)

Applies to every response from the layers inside it (pages, static files, `/up`, `/ask`, the
413/403 refusals). Decision per response:

- Skip (return as is, **no Vary added**) if: status in 100–199/204/304; `Cache-Control` contains
  `no-transform`; `Content-Encoding` present and not `identity`; `Content-Length` is `"0"`.
  No content-type filter: PNG, woff2, SVG, text/plain are all compressed (observed: og.png and
  woff2 come back `content-encoding: gzip`). No minimum size (`/up`'s `up` is gzipped).
- Otherwise add `vary: Accept-Encoding` (appended to an existing Vary unless it has `*` or
  already `accept-encoding`), then negotiate between `gzip` and `identity` from
  `Accept-Encoding` (Rack `select_best_encoding`: q-values honoured; `identity` acceptable unless
  `identity;q=0` or `*;q=0` without identity):
  - `gzip` → `content-encoding: gzip`, remove `Content-Length` (streamed), gzip mtime = the
    response's `Last-Modified` if any.
  - `identity` → unchanged (but Vary added).
  - none acceptable → **406**, `content-type: text/plain`, body
    `An acceptable encoding for the requested resource <fullpath> could not be found.`
    (observed for `Accept-Encoding: identity;q=0`).
- `br`, `deflate`, `zstd` are **never** used (`Accept-Encoding: br` → identity, observed).

The UI budget test (`test/ui/play.js`) requires scripts+styles ≤ 60 000 bytes *transferred*
(≈37 KB gzipped today): compression of `.js`/`.css` is mandatory.

### 2.4 BodyLimit (`web/body_limit.rb`)

- Limit: `2_162_688` bytes (`MAX_SVG_BYTES + 65_536`).
- Test: `CONTENT_LENGTH.to_i > limit` (a non-numeric or absent Content-Length counts as 0;
  chunked bodies pass, the app's own 2 MB check on the SVG still applies).
- Refusal, before anything is parsed, before FrontDoor (so even without the door key):
  status **413**, headers exactly `content-type: text/plain; charset=utf-8`,
  `cache-control: no-store`, body `The request is larger than any plate.\n` (38 bytes, trailing
  newline). Outer layers then add `vary: Accept-Encoding` (and gzip if accepted) and
  `x-content-type-options: nosniff` (observed). No CSP header (the before filter never runs).

### 2.5 FrontDoor (`web/front_door.rb`)

- Key: env `FRONT_DOOR_KEY`; empty or unset → door open (every request passes).
- Open path: `PATH_INFO == "/up"` (cleaned path) always passes.
- Otherwise header `X-Front-Door-Key` (Rack `HTTP_X_FRONT_DOOR_KEY`) must equal the key:
  same byte length **and** constant-time compare (`Rack::Utils.secure_compare`).
- Refusal: status **403**, `content-type: text/plain; charset=utf-8`, `cache-control: no-store`,
  body `Not through the front door.\n`. Plus `vary`/`nosniff` from outer layers.
- Tests: no key → 403 on `/`; `/up` → 200; right key → 200; `abbey-of-molesmf` vs
  `abbey-of-molesme` → 403; `FrontDoor.new(app, '')` → open.

### 2.6 `after` filter

`Cistergraphia::SVG::Parsed.forget` after every request (success, halt or error): the per-thread
parsed-document cache (one SVG parsed once per request; `test_every_plate_brought_to_read_is_
parsed_once_and_forgotten` asserts 1 parse per page, 2 for the ruined `rejoined=1` page, and that
nothing is kept after). In Rust the library keeps a `thread_local!` cache: **run all library
work of one request inside one `spawn_blocking` closure and call `svg::forget()` at its end on
every path** (an `async` handler can hop threads at `.await`).

---

## 3. Response headers and caching, per route

### 3.1 The `before` filter (`web/leaf_headers.rb`), on every routed request (not static files)

Runs for every request that reaches routing, **including 404 route-misses and error pages**:

```
Content-Security-Policy: default-src 'self'; img-src 'self' data: blob:; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'
Referrer-Policy: no-referrer
X-Content-Type-Options: nosniff
Permissions-Policy: camera=(), microphone=(), geolocation=()
Strict-Transport-Security: max-age=31536000; includeSubDomains     (only when request.ssl?)
```

then, in this order (the second overwrites the first):

1. if the method is POST: `Cache-Control: no-store`
2. if the cleaned `PATH_INFO` is exactly `/`, `/decode` or `/about` (any method):
   `Cache-Control: public, max-age=300`

`request.ssl?` ⇔ scheme is `https` (or `wss`), where scheme = (§4.4) `HTTPS=on` (TLS at the
server) → `X-Forwarded-Ssl: on` → `Forwarded: proto=` (last) → `X-Forwarded-Proto` (comma list,
last allowed value) → `X-Forwarded-Scheme` → `http`. Observed: `X-Forwarded-Proto: https` → HSTS
sent.

### 3.2 Final Cache-Control matrix (observed)

| Request | Status | Cache-Control |
|---|---|---|
| `GET /`, `GET /decode`, `GET /about` (and HEAD) | 200 | `public, max-age=300` |
| `POST /generate` (200, 413, 422, 400, 500) | — | `no-store` |
| **`POST /decode`** (200 transcription *and* reader's page, 400, 413) | — | **`public, max-age=300`** (path is a "leaf": overrides `no-store`; a parity quirk, see §15) |
| `POST /ask` (200 and 400) | — | `no-store` |
| `GET /up` | 200 | `no-store` (set by the route's own header hash) |
| `POST /up`, `PUT /generate`, any unknown POST | 404 | `no-store` |
| unknown GET (`/nope`, `/decode/`, `/chapter/`) | 404 | *(none)* |
| `GET /chapter` (200 or 404) | — | `public, max-age=<Chapter.seconds_left>` (1..86400) |
| `GET /chapter/YYYY-MM-DD` (200 or 404) | — | `public, max-age=3600` |
| `POST /chapter/YYYY-MM-DD` (200 or 404) | — | `no-store` |
| static file (200 / 304 / 206) | — | `public, max-age=3600` |

Formatting: Sinatra joins directives with `", "`: `public, max-age=300`.

### 3.3 Other headers

| Response kind | Content-Type (exact) | Extra |
|---|---|---|
| HTML page (any status) | `text/html;charset=utf-8` (no space) | `x-xss-protection: 1; mode=block`, `x-frame-options: SAMEORIGIN` |
| `/up` | `text/plain` | — |
| `/ask` (200 **and** its 400 error page) | `application/json` (no charset) | — (the 400 body is the HTML Erratum page with this JSON content type: quirk, §15) |
| route-miss 404 | `text/html;charset=utf-8` | `x-cascade: pass` |
| BodyLimit / FrontDoor | `text/plain; charset=utf-8` (with space) | `cache-control: no-store` |
| static | see §12 | `last-modified`, `cache-control: public, max-age=3600` |

All responses passing the deflater get `vary: Accept-Encoding`. No ETag anywhere. HEAD: same
headers, empty body (Sinatra reports `content-length: 0`; hyper's HEAD semantics are fine, §15).

---

## 4. Request parsing rules

### 4.1 Params

- Sinatra `params` = query-string params merged with body params; **body wins** on the same key.
- Bodies: `application/x-www-form-urlencoded` (Inscribe form, readback forms, rejoin form,
  `/ask` via `URLSearchParams`) and `multipart/form-data` (the Read form). Every POST route must
  accept both.
- Same key twice: **last value wins** (Rack). Keys like `a[]`/`a[b]` are Rack nested params
  (`params[:message]` would become an Array/Hash and `.to_s` its `inspect`): crafted input only;
  treat keys literally in Rust (§15).
- Missing param → `params[:x]` is nil → `.to_s` is `""`.
- No method override: `Sinatra::Base` leaves `method_override` off, so a `_method` field has no
  effect.
- Values are UTF-8 strings. Lengths are **character (code point) counts** (`String#length`), not
  bytes, except SVG size (bytes).

### 4.2 Multipart file (`params[:file]`)

A part counts as an upload only if Rack sees a filename (`params[:file]` is then a Hash with a
`:tempfile`). Read at most `MAX_SVG_BYTES + 1` bytes of it. A part `file` without filename is a
plain string param and is ignored as an upload. An upload with empty content → falls back to the
`svg` text field (`test_decode_falls_back_to_pasted_svg_when_the_upload_is_empty`).

### 4.3 Form semantics helpers (`web/form_params.rb`, private)

**`passphrase`** → `Option<String>`:
```
if params[:mode] == "game"            -> None
value = params[:passphrase].to_s
if value.empty? && params[:mode] != "secret" -> None
else Some(value)        # may be Some("") when mode=secret and blank
```
Everywhere Ruby tests `if passphrase`, `Some("")` is **truthy** (secret mode with a blank
passphrase still counts as sealed for refusals). `@secret = passphrase.is_some()`.

**`chosen_layout`**:
- `params[:layout]` ∈ {`rongorongo`,`leap`,`furrows`,`unwinding`} → that keyless layout.
- `grille` → `grille_from(params[:grille])`, or `Grille.generate` (a fresh random key; OS
  randomness) when that returns nil.
- anything else (absent, `phaistos`, `<script>`) → `:phaistos` (test: `layout=<script>` → 200,
  `<dd>Phaistos</dd>`, no turn button).

**`grille_from(key)`**: nil if `key.to_s.strip` is empty; else `Grille.new(key)` (library
normalises case, spaces, dashes, look-alikes: `k7q2m zxb4p` → `K7Q2M-ZXB4P`). On
`Cistergraphia::Error`: `@grille_key = key` (raw, unstripped), `@grille_error = e.message`
(`A grille key has 10 letters and digits, like K7Q2M-ZXB4P`), return `:phaistos`.

**`submitted_grille`**: `Grille.new(params[:grille].to_s)` unless it strips empty; invalid key
**raises** `Cistergraphia::Error` (→ 400, §6) wherever it is evaluated.

**`decoy`**: `text = params[:decoy].to_s.strip`; nil if empty **or** `@layout` is not a Grille
(so a refused grille key also drops the decoy). If `@message.length > 250 || text.length > 250`
→ `@decoy_error = "With two readers, each message is limited to 250 characters"`. Returns `text`.

**`read_conditions`** (sets ivars):
```
@condition = params[:condition] if in RUNGS keys {intact, worn, broken, ruined}, else "intact"
@weathered, @broken = CONDITIONS[@condition]
    intact=[false,false] worn=[true,false] broken=[false,true] ruined=[true,true]
@condition_error = condition_problem
@pathless = params[:path] == "withheld"
@path_error = path_problem if @pathless
@own_tablet = params[:tablet] == "own"
@tablet_error = tablet_problem if @own_tablet
```
(`tally`, `weathered` params are ignored: tally signs are never drawn from the form.)

**`form_errors`** = `[@passphrase_error, @grille_error, @decoy_error, @condition_error,
@path_error, @tablet_error]` — any non-nil sends the form back (422).

### 4.4 Request URL pieces (Rack semantics; used for `og:url`, `og:image`, chapter host)

- `request.path` = cleaned `PATH_INFO` (script name empty). No query string.
- `request.base_url` = `"#{scheme}://#{host_with_port}"`:
  - scheme as in §3.1 (allowed: `https`, `http`, `wss`, `ws`).
  - authority = `Forwarded: host=` (last) → `X-Forwarded-Host` (comma list, **last** entry) →
    `Host` header → server name/port.
  - port omitted when equal to the scheme's default (80/443).
  - Observed: `X-Forwarded-Host: a.example, b.example` + `X-Forwarded-Proto: https` →
    `https://b.example`; `Forwarded: host=f.example;proto=https` → `https://f.example`;
    `Host: h.example:8080` → `http://h.example:8080`; rack-test default `http://example.org`.
- The Cloudflare Worker (`infra/worker.js`) sets `X-Forwarded-Host`, `X-Forwarded-Proto: https`,
  `X-Front-Door-Key`; drops query strings except under `/chapter`; refuses methods other than
  GET/HEAD/POST (405) and bodies > 3 MB (413). Not part of the port.

---

## 5. Routes

Constants: `MAX_MESSAGE_LENGTH = 500`, `MAX_PAIR_LENGTH = 250`, `MAX_SVG_BYTES = 2_097_152`.
Sinatra matches the whole cleaned path; no trailing-slash tolerance (`/decode/` → 404). Only
GET (and HEAD) and POST exist; any other method → route-miss 404.

| Method | Path | Handler |
|---|---|---|
| GET | `/up` | §5.1 |
| GET | `/` | §5.2 |
| POST | `/generate` | §5.3 |
| GET | `/decode` | §5.4 |
| GET | `/about` | §5.5 |
| POST | `/decode` | §5.6 |
| GET | `/chapter` | §5.7 |
| GET | `/chapter/(\d{4}-\d{2}-\d{2})` (anchored, ASCII digits) | §5.8 |
| POST | `/chapter/(\d{4}-\d{2}-\d{2})` | §5.9 |
| POST | `/ask` | §5.10 |
| any | anything else | 404 Erratum `This page is not in the index.` + `x-cascade: pass` |

(`/chapter/2026-10-26x`, `/chapter/x2026-10-26`, `/chapter/` → route-miss 404, observed.)

### 5.1 `GET /up`
Status 200, `content-type: text/plain`, `cache-control: no-store`, body `up`. Still gets the
before-filter headers (CSP etc., observed) and FrontDoor exemption.

### 5.2 `GET /`
`@page = :inscribe`; render `index` in `layout`. Every form ivar nil (defaults: game mode,
Phaistos checked, Intact, Shared, Shown, puzzle shut, summary `Intact, shared tablet, path shown`).

### 5.3 `POST /generate`

Fields: `message`, `mode` (`game`|`secret`), `passphrase`, `layout`
(`phaistos`|`rongorongo`|`leap`|`furrows`|`unwinding`|`grille`), `grille` (key), `decoy`,
`condition` (`intact`|`worn`|`broken`|`ruined`), `tablet` (`shared`|`own`), `path`
(`shown`|`withheld`).

Order (exact):
1. `@page = :inscribe`; `@message = params[:message].to_s`.
2. `@message.length > 500` → **413** Erratum `Message is limited to 500 characters`.
3. `@secret = passphrase.is_some()`.
4. `@layout = chosen_layout` (may set `@grille_key`, `@grille_error`).
5. `@passphrase_error = Secret.passphrase_problem(passphrase)` if `@secret`:
   `""` → `Secret mode needs a passphrase`; fewer than 8 characters →
   `A passphrase needs at least 8 characters`; else nil.
6. `@decoy = decoy` (may set `@decoy_error`).
7. `read_conditions` (refusals, §7.5).
8. any `form_errors` → **422**, render `index` with everything kept (message, mode, layout or the
   refused grille key, decoy, rung, tablet, path; never the passphrase).
9. `@sequence, @svg_content, grille = inscribe_plate` (§7.4). Library `Cistergraphia::Error`
   (e.g. `Message has no characters that can be enciphered` for `""`, `!!!`, `123`, a missing
   message) → **400** Erratum (§6), `@page`/`@secret` already set so the nav shows Inscribe
   current and `<body data-mode>` follows `@secret`.
10. `@layout = grille.as(:pair)` if `@decoy`.
11. `@symbols = @sequence.size`; `@normalized = Cistergraphia.normalize(@message)`.
12. Render `result` (200).

`@svg_content` here is the **library's raw SVG** (not cleaned).

### 5.4 `GET /decode`
`@page = :read`; render `decode`.

### 5.5 `GET /about`
`@page = :about`; `@title = 'About the ciphers'`; `@description = 'The ciphers of the
Cistercian monks: their history, how to read a sign, and the tablet behind Cistergraphia.'`;
render `about`. The page draws `rand(1..9)` per request (Lectio's first sign): not deterministic.

### 5.6 `POST /decode`

Fields: `file` (multipart upload), `svg` (text), `passphrase`, `grille`, `decipher` (`1`),
`rejoined` (`1`). (`mode` is honoured by `passphrase` if sent, though no Read form sends it.)

```
@page = :read
svg = submitted_svg                 # upload (if non-empty) else params[:svg];
                                    # bytesize > 2_097_152 -> 413 "SVG is limited to 2048 KB"
if playable?(svg) -> render play (200)          # §10.2
(@message, @decoy, @reader) = read_plate(svg)    # §7.4
@secret = passphrase.is_some()
render decoded (200)
rescue GrilleKeyNeededError e:
  raise DecodeError "#{e.message}. Enter it under “Grille key”, with the passphrase if it was sealed."
rescue DecodeError e:
  re-raise e unchanged if passphrase.is_some() || submitted_grille   # (may itself raise for a bad key)
  raise DecodeError "#{e.message}. If this image was made in secret mode, enter its passphrase."
```
Both rescues cover `playable?` and `read_plate` (errors from laying the reader's page too). An
exception raised inside a rescue clause is not caught by the sibling clause. All end as **400**
Erratum. A non-DecodeError `Cistergraphia::Error` (e.g. a malformed grille key: `A grille key has
10 letters and digits, like K7Q2M-ZXB4P`) passes straight to 400 unchanged.

Observed messages:
- `<svg/>` → `No signs found on this plate. If this image was made in secret mode, enter its passphrase.`
- `not xml` → `Unreadable SVG: not well-formed XML. If this image was made in secret mode, enter its passphrase.`
- sealed plate, no passphrase → e.g. `0 + 29 is not a known character. If this image was made in secret mode, enter its passphrase.`
- grille plate, no key → `This plate was written under a grille: it needs its key. Enter it under “Grille key”, with the passphrase if it was sealed.`
- wrong key → `This grille does not read this plate: check its key` (no suffix)
- wrong passphrase → contains `Wrong passphrase` (library `WRONG_PASSPHRASE`).
- 2 MiB + 1 byte → 413 `SVG is limited to 2048 KB`.

### 5.7 `GET /chapter`
`cache_control :public, max_age: Chapter.seconds_left` **first**, then
`chapter_page(Chapter.today)` (§11.6). 404s keep that Cache-Control.

### 5.8 `GET /chapter/:date`
`cache_control :public, max_age: 3600`, then `chapter_page(Chapter.parse(date))`.

### 5.9 `POST /chapter/:date`
`chapter_page(Chapter.parse(date))` (Cache-Control stays `no-store`). Used by the ruined board's
"Restore it" form (`rejoined=1`); its `svg` field is ignored (only `rejoined` survives).

### 5.10 `POST /ask`
`content_type :json` first; then `JSON.generate(held: scribe_answer(submitted_svg))`. See §10.1.

---

## 6. Error handling

Template `error` in `layout`, message escaped in `<p class="erratum-text">`.

| Trigger | Status | `@error_message` |
|---|---|---|
| `halt_with(status, msg)` (413 message/SVG size, 404 chapter) | as given | as given |
| `Cistergraphia::Error` raised (incl. DecodeError, EmptyMessageError, Wrong*Error) | **400** | `e.message` |
| any other exception (a bug) | **500** | `Something unexpected happened, please try again.` (never the exception text; backtrace to log) |
| route miss | **404** | `This page is not in the index.` (`@error_message ||=`, so a `halt_with 404` keeps its own) |

The page keeps whatever ivars were set before the failure: `@page` (nav `aria-current`),
`@secret` (`<body data-mode="secret">` after a sealed `/generate` failure), `@title` (none of the
erroring routes set one). The Erratum offers `/` ("Inscribe a message") and `/decode`
("Read a plate").

`show_exceptions :after_handler`: the app's handlers always render (no debug backtrace page).

Rust: one `AppError { status, message }` → render error page; map library `Error` kinds to 400;
`catch_unwind`/panic handler and unexpected errors → 500 with the fixed message.

---

## 7. Helpers (`web/*.rb`)

Callers abbreviated: L=layout, I=index, D=decode, T=decoded (transcription), R=result,
A=about, P=play, E=error, `_plate`, `_board`, `_bench`, `_desk`, `_notebook`, `_notebook_row`,
`_ways`.

### 7.1 `ViewHelpers` (`web/view_helpers.rb`)

Constants:
- `GLYPH_VIEWBOX = '-8 -8 116 166'`
- `WEDGES`: ordered map `"x1,y1,x2,y2"` → `Stylus.press(stroke)` for `[STEM, *LINES.values.flatten(1)].uniq` (insertion order: stem, then LINES in order).
- `DESCRIPTION = 'Write a message as Cistercian numerals coiled like a snail: a cipher game, or a passphrase-sealed secret.'`
- `LAYOUT_NAMES` (ordered): `phaistos: 'Phaistos', rongorongo: 'Rongorongo', leap: 'Leap', furrows: 'Furrows', unwinding: 'Unwinding'`
- `ZOOM_FROM = 7`
- `STENCIL_LABELS = { whole: 'the grille', abbot: 'the abbot’s grille', novice: 'the novice’s grille' }`
- `RUNG_NAMES = { intact: 'intact', worn: 'worn', pathless: 'pathless', broken: 'broken', tablet: 'with a tablet of its own', ruined: 'ruined' }`

| Helper | In → Out | Callers |
|---|---|---|
| `glyph_table_json` | → JSON string, memoised: `{"viewBox":"-8 -8 116 166","stem":[50,0,50,150],"lines":{"1":[[50,0,100,0]],"2":…,…,"9000":…},"wedges":{"50,0,50,150":"M43.3 -0.3L…Z",…},"tablet":{"a":10,…,"z":8000," ":9000}}`. Key order as listed; `lines` keys are the place values 1..9, 10..90, 100..900, 1000..9000 (LINES order); `tablet` in TABLET order a..z then space, value×10 | I (`data-glyph-table` on preview), A (hidden div), `_bench`, `_desk` |
| `glyph_svg(num, css: 'glyph', title: nil)` | → raw HTML `<svg class="CSS" viewBox="-8 -8 116 166" role="img" aria-label="LABEL"><title>LABEL</title><g>WEDGES</g></svg>`; LABEL = title or `num.to_s`, **not escaped**; WEDGES = `<path d="…"/>` per stroke of `Glyphs.strokes(num)` (stem first), looked up in WEDGES | L (1098 brand), I (1098 anatomy, units 1..9 tabula), D (9999), A (timeline years, playground 1098, Lectio, tablet), `_bench`, `_desk` |
| `plate_cells_json(sequence, layout = :phaistos, damage = {})` | → JSON `[[row,col,value\|null,turned],…]` along `Layout.path(isqrt(len), layout)`; value null where `damage` has the index | R, P |
| `pathless_cells_json(grid)` | → JSON `[[row,col,upright,false],…]` grid order (upright may be null) | P |
| `leiden_html(text)` | split on `/(\[[^\]]*\])/` keeping delimiters, drop empties; plain parts escaped (`Rack::Utils.escape_html` = §8.1 escaping); bracket parts → `<span class="leiden-doubt">ESC</span>` if they contain `/`, else `<span class="leiden-restored">ESC</span>` | T |
| `plate_layout_json(sequence, layout = :phaistos)` | → JSON `{"padding":50,"cell":[W,H],"offset":O,"size":N,"grille":B}`; keyless: cell `[120,170]`, offset `-10`; grille: cell `[170,170]`, offset `0`; N = isqrt(len) (observed exact strings) | R, P |
| `page_description` | `@description \|\| DESCRIPTION` | L |
| `plate_size_class(size)` | `' plate--large'` if size ≥ 10; `' plate--medium'` if ≥ 7; else `''` | `_plate` |
| `layout_name(layout)` | keyless → LAYOUT_NAMES; Grille with part `:pair` → `Grille, two readers`; other Grille → `Grille` | R, P |
| `plate_grilles` | uses `@layout`, `@symbols`: grilles = pair ? `[as(:abbot), novice]` : `[@layout]`; each → `{part, label: STENCIL_LABELS[part], key: grille.to_s, stencil: SVG.stencil(size, grille), path: JSON(grille.path(size))}` (path = `[[row,col],…]`) | R |
| `grille?` | `@layout` is a Grille | R, P |
| `grille_chosen?` | `@layout` is a Grille **or** `@grille_error` set | I |
| `page_base_url` | `@base_url \|\| request.base_url` | L |
| `nav_class(page)` | `'is-current'` if `@page == page`, else nil (renders `class=""`) | L |
| `chapter_day(day)` | `strftime('%A %-d %B %Y')`, e.g. `Monday 26 October 2026` | P |
| `chapter_week(day)` | expert → `Sunday: the expert plate, pathless with a tablet of its own, the top of the week’s climb.`; else `"#{weekday}: #{RUNG_NAMES[rung]}. The week climbs from Monday’s intact plate to Sunday’s expert one."` (e.g. `Monday: intact. The week climbs …`) | P |

### 7.2 `BenchHelpers` (`web/bench_helpers.rb`)

`PLACE_NAMES = {1=>'units', 10=>'tens', 100=>'hundreds', 1000=>'thousands'}`.

| Helper | Behaviour | Callers |
|---|---|---|
| `place_name(place)` | PLACE_NAMES lookup | `_bench` |
| `lacunae(sequence, layout, damage)` | scribe's entries: for `damage.sort` (by index) → `sign_entries(path, layout, index, lost, options(sequence[index], lost))` flattened | R |
| `lacunae_in(svg, layout, values)` | reader's entries: `Lacunae::Survey.damaged(svg, layout)` each `{index, lost, candidates}` → `sign_entries(…, candidates)` | ReaderGame |
| `lacunae_from(values, layout, damage, candidates)` | rejoined ruin: `damage.sort` → `sign_entries(…, candidates.fetch(index))` | ReaderGame |
| `worn_cells(svg)` | `Survey.worn_cells(svg)` `[row,col,upright,turned]` → `{row:, col:, upright: cell_entries(upright), turned: cell_entries(turned)}` | ReaderGame |
| `cell_entries(reading)` | nil if reading nil, else `by_place(reading[:candidates], reading[:lost])` | — |
| `pathless_lacunae(worn)` | `Σ (cell[:upright] \|\| cell[:turned]).size` | P, `_bench` |
| `sign_entries(path, layout, index, lost, options)` | `row,col,turned = path[index]`; `x,y = Layout.origin(row,col,layout)`; each `by_place` entry merged after `{sign: index, x:, y:, turned:}` | — |
| `by_place(options, lost)` | group lost strokes by `Glyphs::STROKE_PLACES[stroke]`; `shared = {known: without(options.first, places), open: places.sort}`; for places sorted **descending**: `{place:, lost: strokes.map(join(',')), digits: digits_at(options, place)}.merge(shared)` | — |
| `options(value, lost)` | `Weathering.candidates(Weathering.surviving(value, lost), lost)` | — |
| `without(value, places)` | `value - Σ (value / place % 10) * place` | — |
| `digits_at(values, place)` | `values.map { v / place % 10 }.uniq.sort` | — |
| `bench_sum(values, entries, lacuna)` | `first = sign - sign % 2`; for `[first, first+1]`: worn = first entry with that `sign` → `open_digits(worn)`, else `format('%04d', values.fetch(i))`; → `"A + B = ?"` | `_bench` |
| `open_digits(lacuna)` | `format('%04d', known)` with `?` at position `3 - round(log10(place))` for each open place (e.g. known 1034, open [100] → `1?34`) | — |
| `bench_verdict(values, lacuna)` | `sign.even? && values[sign+1] == 9999` → `A null pair: any digit.` else `Press a digit to balance it.` | `_bench` |
| `bench_tallies(lacuna)` | `"Tallies, #{place_name(place)}: row ?, column ?"` | `_bench` |
| `bench_edition(values, damage)` | pairs `(first, second)` index p: skip if `second == 9999 && !damage.key?(2p+1)`; `[.]` if damage has `2p` or `2p+1`; else `CHARACTERS[first+second]` (nil dropped); joined | `_bench` |

Lacuna entry JSON key order: `sign, x, y, turned, place, lost, digits, known, open`.
Worn-cell JSON: `{"row","col","upright":[{place,lost,digits,known,open}…]|null,"turned":…|null}`.

### 7.3 `TrayHelpers` (`web/tray_helpers.rb`)

| Helper | Behaviour | Callers |
|---|---|---|
| `fragments_json(fragments, size)` | `{"cell":[120,170],"board":40,"gap":40,"size":N,"paths":{"phaistos":[[r,c,t],…],"rongorongo":…,"leap":…,"furrows":…,"unwinding":…},"tablet":{"10":"a",…,"9000":" "},"null":9999,"fragments":[{"rows":R,"cols":C,"signs":[[upright,turned,worn],…]}]}`; signs in `fragment.cells` order, each `fragment.readings[cell]` (upright/turned = candidate-value arrays or null, worn bool). `board = PADDING - SPACE/2 = 40`, `gap = Fragments::Tray::GAP = 40` | `_board` |
| `lacunae_count(n)` | `"#{n} lacuna"` if 1 else `"#{n} lacunae"` | R |
| `fragments_count(n)` | `1 fragment` / `"#{n} fragments"` | R, P, `_board` |

### 7.4 `FormParams` (rest) and plate making/reading

- `RUNGS = {'intact'=>'Intact','worn'=>'Worn','broken'=>'Broken','ruined'=>'Ruined'}` (ordered),
  `CONDITIONS` see §4.3. Used by `index` to draw the ladder.
- `puzzle_state` → `[RUNGS[@condition||'intact'], @own_tablet ? 'its own tablet' : 'shared tablet', @pathless ? 'path withheld' : 'path shown'].join(', ')` (I; JS mirrors it).
- `puzzle_open?` → `@pathless || @own_tablet || (@condition||'intact') != 'intact' || any of [@condition_error,@path_error,@tablet_error]` (I).
- `condition_invalid(rung)` → `''` unless `@condition_error && rung == @condition`; then
  `' aria-invalid="true" autofocus'` if `@condition_error` is the first non-nil of `form_errors`,
  else `' aria-invalid="true"'` (I, raw).
- `inscribe_plate` → `[sequence, svg, grille]`:
  1. `@broken || @weathered` → `damaged_plate`:
     - broken: `sequence, svg, @pieces, damage = Fragments.inscribe(@message, layout: @layout, weathered: @weathered)`; `@damage = damage` unless empty; `@fragments = Fragments.read(svg)` (fragments re-read from the file).
     - else: `sequence, svg, @damage = Lacunae.inscribe(@message, layout: @layout, pathless: @pathless)` (layout may be a Grille).
     - returns `[sequence, svg, @layout]`.
  2. no decoy: `@tablet = Tablet.generate if @own_tablet`; `Cistergraphia.inscribe(@message, passphrase:, layout: @layout, pathless: @pathless, tablet: @tablet)` + `[@layout]`.
  3. decoy: `given = @layout` unless `params[:grille]` strips empty (a shared key is kept; a fresh one may be redrawn); `TwoReaders.inscribe(@message, @decoy, passphrase:, grille: given)` → `[sequence, svg, abbot_grille]`.
- `read_plate(svg)`: `@weathered = Lacunae.weathered?(svg)`, `@broken = Fragments.broken?(svg)`;
  `grille = submitted_grille`; with grille → `TwoReaders.read(svg, grille, passphrase:)` →
  `Reading(message, decoy, reader)` with reader `:whole` (plain grille plate), `:abbot` (both
  messages), `:novice`; else `Reading(Cistergraphia.decode(svg, passphrase:), nil, nil)`.

### 7.5 `Refusals` (`web/refusals.rb`) — the library's exact words

```
tablet_problem   (only if @own_tablet):
  passphrase            -> Tablet::NOT_SEALED          "A sealed plate has no tablet to find: its passphrase is the whole key"
  @layout is Grille     -> Tablet::NOT_UNDER_A_GRILLE  "A grille plate has a key already: one key at a time"
  @weathered || @broken -> Tablet::NOT_DAMAGED         "A worn or broken plate keeps the shared tablet: the bench and the board speak in letters"
path_problem     (only if @pathless):
  passphrase            -> Pathless::NOT_SEALED          "A sealed plate cannot be pathless: it reads as no letters along any way"
  @layout is Grille     -> Pathless::NOT_UNDER_A_GRILLE  "A grille plate hides its way by its key already"
  @broken               -> Pathless::NOT_BROKEN          "A broken plate cannot be pathless: the board rejoins its fragments along the snail"
condition_problem:
  @condition == intact  -> nil
  @broken               -> broken_problem:
        passphrase          -> Fragments::NOT_SEALED          "A sealed plate cannot be broken: its signs carry no sums to rejoin it by"
        @layout is Grille   -> Fragments::NOT_UNDER_A_GRILLE  "A plate under a grille cannot be broken yet"
        @layout != phaistos -> Fragments::NOT_ALONG_THE_ROWS  "Only a Phaistos plate can be broken: the board rejoins the fragments along the snail"
  passphrase            -> Lacunae::NOT_SEALED      "A sealed plate cannot wear: with one stroke lost, it would never open again"
  @decoy                -> Lacunae::NOT_TWO_READERS "A plate for two readers cannot wear yet"
```
Allowed combinations worth noting: worn + pathless (any keyless layout); worn under a grille;
pathless + own tablet (the expert plate); own + worn/broken → refused.

Other web-layer user strings:
- `Message is limited to 500 characters` (413), `SVG is limited to 2048 KB` (413; `limit / 1024`).
- `Secret mode needs a passphrase`, `A passphrase needs at least 8 characters` (library; also in
  `index` as `data-missing` / `data-too-short`).
- `With two readers, each message is limited to 250 characters`.
- `A grille key has 10 letters and digits, like K7Q2M-ZXB4P` (library).

### 7.6 `PlateInput` (`web/plate_input.rb`)

- `submitted_svg`: `uploaded_svg.to_s`, if empty `params[:svg].to_s`; `bytesize > 2_097_152` →
  `halt_with 413, "SVG is limited to 2048 KB"`.
- `uploaded_svg`: the upload's first `2_097_153` bytes (§4.2).
- `halt_with(status, message)`: `@error_message = message`; halt with that status and the
  `error` page.

### 7.7 `PlateFile.clean(svg)` (`web/plate_file.rb`) — the reader's-page sanitiser

Returns the plate re-serialised, or nil when it is not an SVG document. **Port exactly**: its
output is inserted raw into the reader's page and posted back by the readback forms.

```
root = SVG::Parsed.of(svg).root            # the request's one parse (library cache)
nil unless root && root.name == "svg"      # local name
rescue DecodeError -> nil                  # unreadable XML
write(root, xmlns: true)
```
- Elements kept (by local name): `svg defs pattern g rect polygon path line`. Any other element
  is dropped **with its whole subtree**. Text, comments, CDATA, PIs are never written.
- Attributes kept: `version width height viewBox class id fill stroke stroke-width
  stroke-dasharray stroke-linecap stroke-linejoin x y x1 y1 x2 y2 points d transform display
  opacity patternUnits patternTransform data-tablet` (by expanded name, case-sensitive; so
  `xlink:href`, `style`, `on*` are dropped).
- A value containing `url(` is kept only if it matches `\Aurl\(#[\w-]+\)\z`; otherwise only if
  it matches `\A[\w\s#().,%-]*\z`. Ruby `\w` = `[A-Za-z0-9_]`, `\s` = `[ \t\n\v\f\r]` (ASCII
  only, verified): in Rust use `(?-u)` or explicit classes.
- Root gets an extra `xmlns="http://www.w3.org/2000/svg"` (the original xmlns attribute is not in
  the list, so it is re-added exactly once).
- Attributes sorted by name (bytewise), written ` name="value"` with value escaped
  `&`→`&amp;`, `<`→`&lt;`, `"`→`&quot;` (in that order; `>` and `'` untouched). Values are the
  parser's decoded values (entities resolved).
- Element with no kept child elements → `<name attrs/>`; else `<name attrs>` + children +
  `</name>`. No whitespace, no newline anywhere (the output is one line).

---

## 8. Templates

### 8.1 Rendering semantics to reproduce (Erubi via Tilt, `escape_html: true`, trim mode)

Verified with Tilt/Erubi 1.13.1:

- `<%= expr %>` → escaped: `&`→`&amp;`, `<`→`&lt;`, `>`→`&gt;`, `"`→`&quot;`, `'`→`&#39;`.
  Nothing else (`/` is **not** escaped). `Rack::Utils.escape_html` (Rack 3.2) is identical.
- `<%== expr %>` → raw.
- Value rendering: `nil` → `""`; `true`/`false` → `true`/`false`; Integer → decimal; Symbol →
  its name; Grille → `to_s` (`K7Q2M-ZXB4P`); String as is.
- **Trim rule**: a line consisting only of optional indentation + one `<% code %>` tag +
  newline emits **nothing** (indent and newline removed). `<%# comment %>` blocks alone on their
  lines likewise vanish with their newline. A `<% %>` tag that shares its line with anything
  else is not trimmed: the text before it and the newline after it stay (observed: `play.erb`
  chapter nav emits an 8-space line `        \n` when `before` is nil). `<%= %>`/`<%== %>`
  lines are never trimmed.
- Partials (`erb :_x, layout: false, locals: {...}`) render to a string inserted raw.
  `defined?(local)` defaults a missing local (`grid`, `pathless`, `own`, `shut`, `worn`).
- Pages render inside `layout.erb` at `<%== yield %>`.
- Output encoding UTF-8; the templates contain non-ASCII literals (`’`, `·`, `×`, `␣`, `Î`,
  `“”`, `…`, `−`, `‹ ›`). Copy them byte for byte.

**minijinja differences you must neutralise** (risk §15): default HTML auto-escape escapes `/`
(`&#x2f;`) and `'` (`&#x27;`); `none` renders as `none`; `trim_blocks`/`lstrip_blocks` do not
equal Erubi's rule (they also strip around tags that share a line). Recommended: §13.2.

### 8.2 Ruby quirks to reproduce in the HTML

1. **Double-escaped attributes** in `index.erb` (observed): the `own` tablet radio and the
   `withheld` path radio use `<%= ' aria-invalid="true"' if … %>` (escaped), producing literally
   ` aria-invalid=&quot;true&quot;>` when `@tablet_error` / `@path_error` is set. The condition
   rung, grille key, decoy and passphrase fields use raw output (correct ` aria-invalid="true"`).
2. Several `autofocus` attributes can appear at once (each refused field puts one).
3. `nav_class` nil → `class=""` on non-current nav links (test asserts
   `href="/chapter" class="">Chapter</a>`).
4. The chapter description contains an ASCII `'` → `The day&#39;s chapter: …` in meta tags.
5. `_plate.erb` caption: only `rongorongo`, grille, and pathless have their own sentence; Leap,
   Furrows, Unwinding and Phaistos all get `Read clockwise from the top-left corner, coiling inward.`
6. `decoded.erb` lede: `<p class="lede">…<% end %>\n    <% if @reader … %>…<% end %></p>` →
   the lede text, newline, 4 spaces, then the reader sentence or nothing, then `</p>`.
7. `about.erb` folio mark says `fol. 4<sup>r</sup>` (its header comment says 3r): copy the markup.

### 8.3 `layout.erb` (frame of every page)

Inputs: `@title`, `@secret`, `@page`, `page_description`, `page_base_url`, `request.path`,
`glyph_svg`, `nav_class`, `yield`.

- `<title>`: `"#{@title} · Cistergraphia"` or `Cistergraphia`.
- `<meta name="description" content=DESC>`; `og:type website`; `og:site_name Cistergraphia`;
  `og:title` = `@title || 'Cistergraphia'`; `og:description` = DESC; `og:url` =
  `page_base_url + request.path`; `og:image` = `page_base_url + "/og.png"`; width 1200, height
  630; fixed `og:image:alt`; `twitter:card summary_large_image`; `color-scheme light dark`; two
  `theme-color` metas (`#f3ede1` light, `#191714` dark).
- Assets: `<link rel="icon" href="/favicon.svg" type="image/svg+xml">`,
  `<link rel="preload" href="/fonts/eb-garamond-latin-400-normal.woff2" as="font" type="font/woff2" crossorigin>`,
  `<link rel="stylesheet" href="/css/app.css">`, then
  `<script src="/js/ink.js" defer></script>`, `ways.js`, `app.js`, `sound.js` **in this order**
  (app.js reads `window.Ink` at load; ways.js defines `window.Ways`).
- `<body data-mode="secret|game">` (`@secret`).
- Skip link `<a class="skip-link" href="#main">`; masthead brand link `/` with
  `glyph_svg(1098, css: 'glyph brand-mark', title: '1098, the founding of Cîteaux')`.
- Nav `aria-label="Main"`: `/` Inscribe (`:inscribe`), `/decode` Read (`:read`), `/chapter`
  Chapter (`:chapter`), `/about` About (`:about`); each `class="<%= nav_class %>"` then raw
  ` aria-current="page"` when current.
- `<main id="main" class="folio">` + page; colophon with two GitHub links and
  `<button type="button" class="hear awaits-script" data-hear aria-pressed="false">Hear the hand</button>`;
  `<div class="toast" id="toast" role="status" aria-live="polite"></div>`.
- No inline `<script>`, `<style>`, `style="`, or `on*=` anywhere (CSP; tested).

### 8.4 `error.erb`
Input `@error_message` (escaped). Section `leaf leaf--narrow`, `h1#erratum-title` "Erratum",
`<p class="erratum-text">MSG</p>`, links `/` (btn--primary) and `/decode`.

### 8.5 `index.erb` (Inscribe, fol. 1r)

Inputs: `@secret, @layout, @message, @grille_key, @grille_error, @decoy, @decoy_error,
@passphrase_error, @condition, @condition_error, @own_tablet, @tablet_error, @pathless,
@path_error`; helpers `snail_svg`, `grille_order_svg`, `glyph_table_json`, `glyph_svg`,
`grille_chosen?`, `puzzle_open?`, `puzzle_state`, `condition_invalid`; constants
`FormParams::RUNGS/CONDITIONS`, `Grille::EXAMPLE` (`K7Q2M-ZXB4P`), `MAX_PAIR_LENGTH` (250),
`Secret::MIN_PASSPHRASE_LENGTH` (8), `Secret.passphrase_problem('')`/`('-')`, `Tablet::SHORT` (6).

Structure / contracts:
- `<form class="entry" method="post" action="/generate" data-entry>`.
- Mode radios `name="mode"` `game` (checked unless `@secret`), `secret` (checked if `@secret`);
  notes `#note-game`/`#note-secret` with `data-note`.
- Layout radios `name="layout"`: `phaistos` checked unless `@layout` ∈ {rongorongo, leap,
  furrows, unwinding} or `grille_chosen?`; each other checked when equal; `grille` checked when
  `grille_chosen?`. Each label has `<span class="segment-figure" aria-hidden="true">` with
  `snail_svg(4[, layout:])` or `grille_order_svg` (6 figures; tested). Notes `data-note=<layout>`.
- `<textarea id="message" name="message" maxlength="500" rows="4" required placeholder="Meet me at the old abbey after vespers" data-counted>MSG</textarea>`;
  counter `[data-counter]` with `[data-count]` `0` and ` / 500`.
- Preview `<div class="preview is-empty" data-preview data-glyph-table="JSON">` with
  `ol[data-preview-signs]`, three `.preview-note` (`--empty`, `--secret`, `--own`). Never `hidden`.
- `.slot.grille-slot`: `input#grille name="grille" value="@grille_key"` placeholder EXAMPLE,
  raw ` aria-invalid="true" autofocus` if `@grille_error`; `p#grille-error` = `@grille_error`;
  `textarea#decoy name="decoy" maxlength="250"` with raw invalid/autofocus if `@decoy_error`,
  content `@decoy`; `p#decoy-error` = `@decoy_error`.
- `.slot.secret-slot`: `input#passphrase type="password" name="passphrase" minlength="8"` (never
  a value), raw invalid/autofocus if `@passphrase_error`; reveal button `data-reveal="passphrase"`;
  `p#passphrase-error … data-passphrase-error data-missing="Secret mode needs a passphrase" data-too-short="A passphrase needs at least 8 characters"` containing `@passphrase_error`.
- `<details class="puzzle" data-puzzle[ open]>` (`' open'` via escaped output when
  `puzzle_open?`); summary `[data-puzzle-state]` = `puzzle_state`.
  - Condition: for each RUNG `label.segment.segment--rung.segment--<rung>[ segment--snail]`
    (snail for broken/ruined) with `<input type="radio" name="condition" value="<rung>" data-rung[ checked]` (checked when `(@condition||'intact') == rung`), newline+indent,
    `aria-describedby="note-<rung> note-unweathered note-snail condition-error"` + raw
    `condition_invalid(rung)`. Notes `#note-intact|worn|broken|ruined|snail|unweathered`,
    `p#condition-error` = `@condition_error`.
  - Tablet radios `name="tablet"` `shared` (checked unless own) / `own` (checked if own; quirk
    §8.2.1); notes incl. `data-note="tablet-short"` (mentions 6 words), `tablet-sealed`,
    `tablet-grille`, `tablet-damaged`, `tablet-expert`; `p#tablet-error`.
  - Path radios `name="path"` `shown` / `withheld` (quirk §8.2.1); notes `shown`, `withheld`,
    `path-sealed`, `path-grille`, `path-broken`; `p#path-error`.
- Submit `<button class="btn btn--primary" type="submit" data-submit>Inscribe</button>`.
- Aside: anatomy `figure[data-anatomy][data-number="1098"]` with 4 buttons
  `quadrant quadrant--tens|units|thousands|hundreds` `data-place="10|1|1000|100"` **disabled**
  (in that order; tested), `[data-digit]`, `[data-anatomy-caption]`, `.anatomy-hint.awaits-script[data-anatomy-hint]`;
  tabula `glyph_svg(1..9)`; orders `figure.order[data-layout-note="<layout>"]` ×6 (phaistos 4,
  rongorongo 4, **leap 6**, furrows 4, unwinding 4, grille); `<p class="note" data-path-withheld>`.
- Element order (tested): `name="mode"` < `name="layout"` < `id="message"` < `data-preview` <
  `id="passphrase"` < `data-submit`; `id="message"` before `name="condition"`.

### 8.6 `decode.erb` (Read, fol. 2r)
No ivars. `glyph_svg(9999, title: 'A Cistercian sign')`; `<p class="record" data-record="read" aria-live="polite"></p>`
(exact; tested); chapter invite `<a href="/chapter">read the day’s chapter</a>`;
`<form class="entry" method="post" action="/decode" enctype="multipart/form-data">`;
`label.dropzone[for=file][data-dropzone]` with `[data-file-name]` and
`input#file type="file" name="file" accept=".svg,image/svg+xml"`; `details.paste` with
`textarea#svg name="svg"` (placeholder already entity-encoded in source: copy literally);
`input#passphrase name="passphrase" autocomplete="current-password"` + reveal button;
`input#grille name="grille"` placeholder `K7Q2M-ZXB4P`; submit "Read".

### 8.7 `decoded.erb` (Transcription, fol. 3r)
Inputs `@broken, @weathered, @secret, @reader, @message, @decoy`; helper `leiden_html`.
- Lede: ruined (`@broken && @weathered`) / broken / weathered / else (`@secret ? 'Unsealed with its passphrase: authentic and unaltered.' : 'Read from an open plate: letters and spaces, as enciphered.'`), then `\n    ` and `Read with the abbot’s key: the novice’s message follows.` (`:abbot`) or `Read with the novice’s key.` (`:novice`) or nothing (`:whole`/nil).
- `<p class="transcription" data-transcription>` + (`@weathered` ? raw `leiden_html(@message)` : escaped `@message`) + `</p>` — no whitespace inside (JS reads `textContent`).
- If `@decoy`: `h2` "The novice’s grille reads" + `p.transcription.transcription--second`.
- Actions: `button[data-action="copy-text"]`, `/decode`, `/`.

### 8.8 `result.erb` (the plate, fol. 1v)

Inputs: `@secret, @message, @decoy, @pieces, @damage, @tablet, @pathless, @normalized,
@symbols, @layout, @svg_content, @fragments, @sequence`. Helpers: `fragments_count`,
`lacunae_count`, `layout_name`, `grille?`, `plate_grilles`, `plate_cells_json`,
`plate_layout_json`, `lacunae`, `Tablet.show`, `Tablet.rows`, partials.

- `h1`: `Your sealed plate` / `Your plate`. `<q>` echo of `@message` (escaped); `p.decoy-echo`
  if `@decoy`.
- Lede priority: `@pieces && @damage` (Ruined: N fragments, M lacunae…) → `@pieces` (Broken) →
  `@damage` (Worn) → `@tablet && @pathless` (expert) → `@tablet` → `@secret` →
  `@normalized != @message` (`Enciphered as: <code>NORMALIZED</code>`) → `A game: anyone patient
  enough can read it back.`
- `dl.specs`: Mode; Signs `@symbols`; Grid `N × N` (isqrt); Layout `layout_name(@layout)`; Path
  `Withheld: the reader finds it` if `@pathless`; Tablet `Its own <code class="grille-key">SHOW</code>`
  (`Tablet.show(@tablet)` = `XXXXX-XXXXX`); Condition (`Ruined: N fragments, worn` / `Broken, in
  N fragments` / `Worn`); keys (`Abbot’s key` + `Novice’s key` when grille+decoy, else `Grille key`).
- Actions: `data-action="download-svg"`, `download-png`, `copy-svg`; rongorongo only:
  `<button type="button" class="btn awaits-script" data-action="turn-plate" aria-pressed="false">Turn the plate</button>`.
- `@pieces` → `_board(svg: @svg_content, fragments: @fragments, size: isqrt(@symbols), worn: !@damage.nil?)`;
  else `_plate(svg: @svg_content, cells: plate_cells_json(@sequence, @layout, @damage||{}), geometry: plate_layout_json(@sequence, @layout), layout: @layout, decoy: @decoy, size: isqrt(@symbols))`.
- `@damage && !@pieces` → `_bench(entries: lacunae(@sequence, @layout, @damage), values: @sequence, damage: @damage, tally: nil)`.
- `@tablet` → `section.own-tablet` with `ol.own-tablet-rows[data-tablet-rows]`, one `li` per
  `Tablet.rows(@tablet)` (27): `<span class="own-tablet-value">%04d</span> <span class="own-tablet-char">C|␣</span>`.
- `grille?` → `div.actions.grille-actions`: if 2 grilles, `div.segments.grille-choice.awaits-script[role=group]`
  with buttons `data-grille-choice="abbot|novice"` `aria-pressed="true|false"` (first true),
  titles `Abbot`/`Novice`; `data-action="lay-grille"` button; `data-action="download-stencil"`
  (`Download the stencils`/`Download the stencil`); per grille `data-action="copy-key" data-key="KEY"`
  (`Copy the abbot’s key`/`Copy the novice’s key` via `label.sub('grille','key')`, or `Copy the key`).
  Then per grille `<template data-stencil="PART" data-label="LABEL" data-path="JSON">` + raw stencil SVG + `</template>`.
- Readback form (no `data-play`, no `data-record`: the scribe's preview is never recorded, tested):
  `<form class="readback" method="post" action="/decode">`, `<input type="hidden" name="svg" value="ESCAPED @svg_content">`,
  `<input type="hidden" name="decipher" value="1">`, grille → `<input type="hidden" name="grille" value="KEY">`
  (for a pair: the abbot's key); secret → `input#readback-passphrase name="passphrase" required`;
  submit `Decipher it`; link `/` "Inscribe another".

### 8.9 `about.erb` (fol. 4r)
Static prose plus: timeline `glyph_svg(1098|1533|2001, title: 'YEAR')`; figures
`disc_spiral_svg`, `snail_svg(4)`, `boustrophedon_svg`, `snail_svg(4, layout: :rongorongo)`,
`snail_svg(6, layout: :leap)`, `snail_svg(4, layout: :furrows)`, `snail_svg(4, layout: :unwinding)`,
`cardano_svg`, `grille_order_svg` (tests: 3 `<svg class="turning"`, 5 `<svg class="snail"`, 1
`<svg class="grille-order"`, `id="layouts"`); playground (`[data-playground-glyph]` with
`glyph_svg(1098, title: 'The Cistercian sign for 1098')`, `input#number[data-playground]`,
`[data-playground-reading]` `1 thousand · 0 hundreds · 9 tens · 8 units`, presets
`data-playground-set` 1098 1533 2001 2026 9999); Lectio `section.lectio[data-lectio]` with
`first = rand(1..9)` → `div.lectio-sign[data-lectio-sign][data-number=first]` + `glyph_svg(first,
title: 'A Cistercian sign to read')`, disabled `[data-lectio-answer]`, `[data-lectio-submit]`;
tablet `ol.tablet` one `li` per TABLET entry: `glyph_svg(v*10, title: "#{char|'space'}: #{v*10}")`,
`<b>char|␣</b>`, `<span>v*10</span>`; hidden `div[data-glyph-table]`; Sources list.

### 8.10 `play.erb` (the reader's page, fol. 2v)

Inputs: `@broken, @rejoined, @weathered, @expert, @grid, @notebook, @desk, @chapter, @entries,
@values, @worn, @fragments, @size, @tally, @layout, @svg_content, @plate_svg, @damage`.

- `rung` (for `data-play`): `@broken || @rejoined` → (`@weathered` ? `ruined` : `broken`);
  elsif `@expert` → `expert`; elsif `@grid` → `pathless`; elsif `@notebook` → `tablet`; elsif
  `@desk` → `intact`; else `worn`.
- `<section class="leaf" aria-labelledby="play-title" data-play="RUNG"[ data-chapter="YYYY-MM-DD"]>`.
- Chapter mark (if `@chapter`): `<p class="chapter-mark"><span class="chapter-number">Chapter N</span> · DAY[ · <span class="chapter-feast">FEAST</span>]</p>`.
- `h1`: `@broken` → (`@weathered` ? `A plate to rejoin and restore` : `A plate to rejoin`);
  elsif `@desk` → `A plate to read`; else `A plate to restore`.
- Lede priority: `@rejoined` → `@grid && !@desk` (worn pathless; uses `@values.size / 2`,
  `pathless_lacunae(@worn)`) → `@expert` → `@grid` → `@notebook` (`@notebook.size` sums) →
  `@desk` → `@broken && @weathered` → `@broken` → worn (`@entries.size` `stroke position(s)`).
- `p.chapter-week` = `chapter_week(@chapter)` if chapter.
- `dl.specs`: Signs `@size*@size`; Grid; Layout (`Phaistos` if `@broken`; `Withheld: one of the
  five ways` if `@grid`; else `layout_name(@layout)`); Condition: broken → `Ruined: N fragments,
  worn` / `Broken, in N fragments`; rejoined → `Ruined, rejoined: worn`; worn pathless →
  `Worn, pathless, with tally signs`/`Worn, pathless`; intact pathless → `Intact, pathless` (+
  `Tablet: Its own: to build` if expert); notebook → `Intact` + `Its own: to build`; desk →
  `Intact`; else `Worn, with tally signs`/`Worn`. `Grille key` row when `grille?`.
- Body: `@broken` → `_board(svg: @svg_content, fragments: @fragments, size: @size, worn: @weathered)`.
  Else `div.room > div.room-plate` + `div.room-work`:
  - plate: `@grid` → `_plate(svg: @svg_content, cells: pathless_cells_json(@grid), geometry: plate_layout_json(@values, @layout), layout: @layout, decoy: nil, size: @size, grid: @grid)`;
    `@desk` → `_plate(…, cells: plate_cells_json(@values, @layout), …)`;
    else `_plate(svg: @plate_svg || @svg_content, cells: plate_cells_json(@values, @layout, @damage), …)`.
  - work: `@grid && !@desk` → `_bench(entries: @entries, values: @values, damage: @damage, tally: @tally, pathless: true, worn: @worn)`;
    `@grid` → `_desk(values: @values, tablet: @expert ? {} : CHARACTERS, pathless: true, own: @expert)` + (expert) `_notebook(entries: [], values: @values, shut: true)`;
    `@notebook` → `_desk(values: @values, tablet: {}, own: true)` + `_notebook(entries: @notebook, values: @values)`;
    `@desk` → `_desk(values: @values, tablet: CHARACTERS)`;
    else `_bench(entries: @entries, values: @values, damage: @damage, tally: @tally)`.
- `<p class="record" data-record="play" aria-live="polite"></p>` (exact; tested).
- Chapter: `<p class="chapter-source" data-chapter-source hidden>From SOURCE.</p>`.
- Readback form: hidden `svg` = **`@svg_content`** (the cleaned file brought; for a rejoined ruin
  still the broken file), `decipher=1`, `grille` if `grille?`; `Decipher it`; rongorongo →
  turn-plate button; link `/decode` "Read another".
- Chapter nav (if `@chapter`): `before = Chapter.of(date-1)`, `after = Chapter.of(date+1)`; if
  either: `<nav class="chapter-nav" aria-label="Chapters">` with
  `<a href="/chapter/D">Chapter N, the day before</a>` and
  `<a class="chapter-nav-after" href="/chapter/D">Chapter N, the day after</a>` (each on its own
  non-trimmed line, §8.1).

### 8.11 `_plate.erb`
Locals `svg, cells, geometry, layout, decoy, size, grid=nil`. `zoomable = size >= 7`.
- `<figure class="plate#{plate_size_class(size)}">` (tested: `<figure class="plate">` for 6×6,
  `plate plate--large` for 14×14).
- `div.plate-view > div.plate-frame[ plate-frame--zoomable]#svg-display[data-plate][tabindex=0][role=group]`
  with `aria-label="The plate. Use the arrow keys to walk its signs in reading order[, and plus, minus and zero to zoom]."`,
  `data-plate-cells="CELLS"`, `data-plate-layout="GEOMETRY"`, and `data-plate-grid="JSON(grid)"`
  when grid; inner raw `svg`.
- zoomable → `div.plate-zoom.awaits-script[data-plate-zoom][role=group]` with buttons
  `data-zoom-step="-1"` (disabled, `−`), `data-zoom-fit data-zoom-level` (disabled, `×1`),
  `data-zoom-step="1"` (`+`, `aria-label="Zoom in"`), `span[data-zoom-said]`. Not present below 7
  (tested with `Ora`, 4×4).
- Caption `span.plate-reading > span.plate-reading-rest` (sentence by grid / rongorongo / grille
  pair / grille / else) + `<span class="plate-reading-live" data-plate-reading aria-live="polite"></span>`;
  `span.plate-hint.awaits-script[data-plate-hint]` (+ `; + and − zoom in and out` when zoomable).

### 8.12 `_board.erb`
Locals `svg, fragments, size, worn`; uses `@chapter`.
`figure.plate.plate--broken > div.plate-frame#svg-display[data-plate][role=group][data-fragments=JSON]` + raw svg;
caption (+ worn sentence); `section.rejoin.awaits-script[data-rejoin]` with `[data-rejoin-turn]`
(disabled), `[data-rejoin-reset]`, and when worn
`<form method="post" action="(@chapter ? @chapter.path : '/decode')" class="rejoin-restore">`
+ hidden `svg` (escaped local svg) + `<input type="hidden" name="rejoined" value="1">` +
`<button type="submit" class="btn btn--small btn--primary" data-rejoin-restore disabled>Restore it</button>`;
`p.rejoin-status[data-rejoin-status]` `On the board: 0 of N fragments.`; `p.rejoin-reading[data-rejoin-reading]`.
No `data-plate-cells` on a broken plate (tested).

### 8.13 `_bench.erb`
Locals `entries, values, damage, tally, pathless=false, worn`. `first = entries.first`;
`count = pathless ? pathless_lacunae(worn) : entries.size`.
- `section.bench.awaits-script[data-bench][ data-bench-pathless]` + `data-lacunae="JSON(entries)"`
  + `data-glyph-table` + (tally) `data-tally="JSON(tally)"` ({rows:[…],cols:[…],corner:v}) +
  (pathless) `data-bench-worn="JSON(worn)"` (attributes on continuation lines: copy layout).
- Nav: `data-bench-step="-1"`/`"1"`, `span.bench-which[data-bench-which]` = first ?
  `Lacuna 1 of COUNT: the PLACE` : `Choose a way to restore`.
- pathless → `_ways(at: 'bench')`.
- Palette: 10 `button.bench-digit[data-digit=d][aria-pressed=false]` (disabled unless
  `first && first[:digits].include?(d)`), each `glyph_svg(d * (first ? first[:place] : 1), css: 'glyph bench-glyph', title: "#{d} in the #{place_name(first ? first[:place] : 1)}")` + `<span>d</span>`.
- Balance: `[data-bench-sum]` (`bench_sum` or `? + ? = ?`), `[data-bench-verdict]`
  (`bench_verdict` or `Choose a way: the pairs form along it.`), tally →
  `[data-bench-tally]` (`bench_tallies` or `Tallies: row ?, column ?`), `[data-bench-count]`
  `0 of COUNT restored`.
- `[data-bench-edition]`: first ? `bench_edition(values, damage)` : `'[.]' * (values.size/2)`.
- pathless → `p.bench-notebook[data-bench-notebook]` `The notebook: what each way gave.`

### 8.14 `_desk.erb`
Locals `values, tablet, pathless=false, own=false`; `expert = pathless && own`.
`section.desk.awaits-script[data-desk][data-desk-mode="guided"][ data-desk-pathless][ data-desk-own]`
with `data-tablet="JSON(tablet)"` (`{}` or CHARACTERS `{"10":"a",…,"9000":" "}`), own →
`data-values="[10,20,…,9000]"` (CHARACTERS keys sorted), `data-null="9999"`, `data-glyph-table`.
Mode buttons `data-desk-set-mode="guided"` (pressed) / `unaided`; pathless → `_ways(at: 'desk')`;
nav `data-desk-step` ±1, `[data-desk-which]` = pathless ? `Choose a way to read` :
`Pair 1 of N · signs 1 and 2`; `form.desk-pair[data-desk-form]` with `[data-desk-sign="0|1"]`
(glyph of `values[0]`/`values[1]` with `css: 'glyph desk-glyph'` unless pathless),
`input#desk-first|#desk-second[data-desk-value="0|1"]`, `output[data-desk-letter]` `?`,
`[data-desk-read]`; `[data-desk-feedback]` (4 texts by expert/pathless/own/else); unless own:
`<p class="desk-text" data-desk-text aria-live="polite">` + `'·' * (values.size/2)`; pathless →
`[data-desk-notebook]`; standing `[data-desk-rank]` Novice, `[data-desk-streak]` 0, `[data-desk-best]` 0.

### 8.15 `_notebook.erb` / `_notebook_row.erb`
`_notebook` locals `entries, values, shut=false`; note text constant `Each sum is one letter,
however its pair is spelled. Write a letter in pencil against a sum; the scribe confirms entries
three at a time.`
- `section.notebook.awaits-script[ is-shut][data-notebook][data-notebook-sums="JSON(sums)"][ data-notebook-shut data-notebook-open-note="NOTE"]`.
- `[data-notebook-count]`: shut → `Shut until the way is found`; else `0 of N in ink · asked 0 times`.
- `p.desk-text.notebook-line[ desk-text--notebook][data-desk-text]`: shut → `'·' * (values.size/2)`;
  else raw `'<span>·</span>' * Notebook.pairs(values).size` (null pairs excluded).
- `[data-notebook-note]`; table `tbody[data-notebook-rows]` one row per entry; shut →
  `<template data-notebook-row>` + row for `{sum: 0, seen: 0, spellings: []}`.
- `button[data-notebook-ask]` (disabled when shut) `Ask the scribe`; `[data-notebook-verdict]`.
- Row: `<tr data-entry-row="SUM">`, `td.notebook-sum` `%04d`, `td.notebook-seen`,
  `td.notebook-spellings` first 3 spellings `<span class="spelling">%04d + %04d</span>` then
  `<span class="spelling-more[ spelling-more--wide]">…</span>` if >1 (wide if >3);
  `label[for=entry-SUM]` `What SUM reads as`; `select#entry-SUM.input.notebook-entry[data-entry=SUM]`
  options `value=""` `·`, `a`…`z`, `value=" "` `␣`.

### 8.16 `_ways.erb`
Local `at` (`desk`|`bench`): `p.ways-question#AT-question` `Which way does it run?`; 5 buttons
`button.segment.segment--way[data-AT-way=WAY][aria-pressed=false]` in LAYOUT_NAMES order with
`snail_svg(4, layout: WAY)` and title.

### 8.17 `Figures` helpers (`web/figures.rb`) — exact SVG

`ORDER_LABELS` (aria-labels): phaistos `Reading order: a clockwise spiral from the top-left
corner`; rongorongo `Reading order: rows from the bottom left, every other row running back upside
down`; leap `Reading order: one row down and two columns across at every sign, wrapping at the
edge; when the orbit closes, a new start one column to the right`; furrows `Reading order: down
the first column, up the next, from the left`; unwinding `Reading order: from the centre outward,
counter-clockwise, to the top-right corner`; grille `Reading order: the signs under the grille’s
holes, row by row, then the grille turned a quarter clockwise, four times`.

- `snail_svg(size = 4, layout:)`: cells = `Layout.path(size, layout)` → `[col*20+10, row*20+10, turned]`;
  `<svg class="snail" viewBox="0 0 S S" role="img" aria-label="LABEL">` (S = size×20) + polylines
  + dots + start + `</svg>`. Polylines: split the cell list where `hypot(dx, dy) > 50`; each run →
  `<polyline points="x,y x,y …" pathLength="1"/>` (a one-point run is still emitted). Dots:
  `<circle cx="X" cy="Y" r="2.2"/>` or `<circle cx="X" cy="Y" r="2.2" class="turned"/>`.
  Start: `<circle class="snail-start" cx="X0" cy="Y0" r="4"/>`. Example (observed, size 4):
  `<svg class="snail" viewBox="0 0 80 80" role="img" aria-label="Reading order: a clockwise spiral from the top-left corner"><polyline points="10,10 30,10 50,10 70,10 70,30 70,50 70,70 50,70 30,70 10,70 10,50 10,30 30,30 50,30 50,50 30,50" pathLength="1"/><circle cx="10" cy="10" r="2.2"/>…<circle class="snail-start" cx="10" cy="10" r="4"/></svg>`.
- `cardano_svg`: `<svg class="turning" viewBox="0 0 100 80" role="img" aria-label="Cardano’s grille: an ordinary letter, with openings over the words that carry the secret"><rect x="4" y="4" width="92" height="72" rx="6"/><g class="turning-glyphs">` + per word `<path d="M{x} {16+row*12}h{len}"/>` + `</g>` + per opening `<rect class="aperture" x="{x-2}" y="{11+row*12}" width="{len+4}" height="10" rx="1.5"/>` + `</svg>`.
  Letter `[[[12,16],[32,10],[46,22],[72,16]],[[12,24],[40,14],[58,30]],[[12,10],[26,20],[50,14],[68,20]],[[12,30],[46,18],[68,20]],[[12,18],[34,26],[64,14]]]`, openings `[[0,2],[2,1],[3,2],[4,0]]`.
- `boustrophedon_svg`: rows y `[64,48,32,16]`; `<svg class="turning" viewBox="0 0 100 80" role="img" aria-label="Reverse boustrophedon: read left to right from the bottom, turning the tablet half round between lines"><rect x="4" y="4" width="92" height="72" rx="6"/><g class="turning-glyphs">TICKS</g><polyline points="PATH" pathLength="1"/><circle class="snail-start" cx="14" cy="64" r="3"/></svg>`;
  ticks for x in 20,32,…,80: even row `<path d="M{x} {y-2}v-7h4"/>`, odd `<path d="M{x} {y-9}v7h-4"/>`;
  path: even row `14,y 86,y`, odd `86,y 14,y`.
- `disc_spiral_svg`: `<svg class="turning" viewBox="0 0 100 100" role="img" aria-label="A spiral read clockwise from the rim to the centre"><circle cx="50" cy="50" r="47"/><polyline points="…" pathLength="1"/><circle class="snail-start" cx="50" cy="8" r="3"/></svg>`;
  points for i in 0..=108: `radius = 42 - (34.0 * i / 108)`, `degrees = -90 + i*10`,
  `angle = degrees * PI / 180` (multiply then divide, in that order), point =
  `[(50 + r*cos(angle)).round(1), (50 + r*sin(angle)).round(1)]`, printed as Ruby Floats
  (`50.0`, `8.0`, `57.2`). Use Ruby's `Float#round(1)` (round-half-up with its correction step:
  same helper the stylus port uses) and print integral values with `.0`. Starts
  `50.0,8.0 57.2,8.9 64.1,11.1 70.5,14.4 …`.
- `grille_order_svg`: `GRILLE_EXAMPLE = Grille.new('K7Q2M-ZXB4P')`; for turn 0..3, each hole
  `(row,col)` of `holes(4, turn)`: `<g class="grille-cell[ grille-hole]"><rect x="{col*20+1.5}" y="{row*20+1.5}" width="17" height="17" rx="2"/><text x="{col*20+10}" y="{row*20+13.5}">{N|E|S|W}</text></g>`
  (hole class on turn 0; floats print `1.5`, `41.5`, `13.5`); wrapped in
  `<svg class="grille-order" viewBox="0 0 80 80" role="img" aria-label="ORDER_LABELS[:grille]">…</svg>`.
  Observed first cell: `<g class="grille-cell grille-hole"><rect x="41.5" y="1.5" width="17" height="17" rx="2"/><text x="50" y="13.5">N</text></g>`.

---

## 9. Markup contracts the JS relies on (`public/js`)

Scripts load deferred in order ink → ways → app → sound; `app.js` runs its inits on
`DOMContentLoaded`: `initModes, initLayouts, initCounter, initPuzzle, initReveal, initActions,
initDropzone, initPreview, initAnatomy, initPlayground, initLectio, initRecord, inkPlate,
initPlateWalk, initPlateZoom, initDesk, initGrille, initBench, initFragments, initTranscription`.

### 9.1 Server endpoints referenced by JS
- `fetch('/ask', { method: 'POST', body: URLSearchParams{svg, pencil, inked}, headers: { Accept: 'application/json' } })` → expects `response.ok` and JSON `{ held: { "<sum>": "<char>" } }` (§10.1). Non-OK → `The scribe could not read the notebook: ask again.`
- Forms (no JS needed): `/generate`, `/decode` (Read: multipart; readback: urlencoded with
  `svg`, `decipher=1`[, `grille`][, `passphrase`]), rejoin `/decode` or `/chapter/DATE` with
  `svg`, `rejoined=1`. Links `/`, `/decode`, `/chapter`, `/chapter/DATE`, `/about`, `/about#layouts`.
- Downloads are client-side: SVG source = `.readback input[name="svg"]` **value** (so that hidden
  field must hold the exact file); PNG from `[data-plate] svg` width/height; stencils from
  `template[data-stencil]` innerHTML (`cistergraphia-grille[-abbot|-novice].svg`).
- The notebook seed = `/data-tablet="([^"]+)"/` on that hidden value.

### 9.2 Selectors and attributes used (complete list, from app.js + sound.js + ink.js)
IDs: `#toast`, `#passphrase`, `getElementById(button.dataset.reveal)`.
Classes read: `.plate-reading`, `.plate-focus`, `.segment-title`, `.notebook-sum`,
`.notebook-seen`, `.notebook-spellings`, `.grille-actions .awaits-script`,
`.readback input[name="svg"]`, `g.fragment`, `svg.glyph`.
Data attributes: `data-action` (`copy-text`, `copy-key`, `download-stencil`, `download-svg`,
`download-png`, `copy-svg`, `lay-grille`, `turn-plate`), `data-anatomy`, `data-anatomy-caption`,
`data-anatomy-hint`, `data-number`, `data-place`, `data-digit`, `data-bench`,
`data-bench-pathless`, `data-bench-step`, `data-bench-way`, `data-bench-which|sum|verdict|tally|count|edition|notebook`,
`data-lacunae`, `data-tally`, `data-bench-worn`, `data-chapter`, `data-chapter-source`,
`data-count`, `data-counted`, `data-counter`, `data-desk`, `data-desk-mode`,
`data-desk-pathless`, `data-desk-own`, `data-desk-set-mode`, `data-desk-step`, `data-desk-way`,
`data-desk-which|feedback|text|notebook|letter|read|rank|streak|best`, `data-desk-form`,
`data-desk-sign`, `data-desk-value`, `data-tablet`, `data-values`, `data-null`, `data-dropzone`,
`data-file-name`, `data-entry` (Inscribe form **and** notebook selects), `data-entry-row`,
`data-grille-choice`, `data-stencil`, `data-label`, `data-path`, `data-key`, `data-hear`,
`data-layout-note`, `data-lectio`, `data-lectio-answer|best|feedback|form|rank|sign|streak|submit`,
`data-notebook`, `data-notebook-ask|count|note|row|rows|verdict|sums|shut|open-note`,
`data-passphrase-error`, `data-missing`, `data-too-short`, `data-plate`, `data-plate-cells`,
`data-plate-layout`, `data-plate-grid`, `data-fragments`, `data-plate-hint`,
`data-plate-reading`, `data-plate-zoom`, `data-zoom-step`, `data-zoom-fit`, `data-zoom-level`,
`data-zoom-said`, `data-play`, `data-playground`, `data-playground-glyph|reading|set`,
`data-preview`, `data-preview-signs`, `data-glyph-table`, `data-puzzle`, `data-puzzle-state`,
`data-record` (`read`|`play`), `data-rejoin`, `data-rejoin-turn|reset|restore|status|reading`,
`data-reveal`, `data-rung`, `data-submit`, `data-transcription`.
Inputs by name: `mode`, `layout`, `condition`, `tablet`, `path` (`:checked`), `input[type="file"]`.
State classes JS toggles (CSS contract, not server): `awaits-script` (JS removes), `is-*`.

### 9.3 JSON shapes the server must produce (exact key order)

| Attribute | Shape |
|---|---|
| `data-glyph-table` | §7.1 |
| `data-plate-cells` | `[[row,col,value\|null,turned],…]` reading order; pathless: grid order, `turned` false |
| `data-plate-layout` | `{"padding":50,"cell":[W,H],"offset":O,"size":N,"grille":B}` (sound.js reads it too) |
| `data-plate-grid` | `[[row,col,upright\|null,turned\|null],…]` (`Pathless.grid`) |
| `data-fragments` | §7.3 |
| `data-lacunae` | `[{"sign","x","y","turned","place","lost":["x1,y1,x2,y2"],"digits":[…],"known","open":[…]}]` |
| `data-tally` | `{"rows":[…],"cols":[…],"corner":V}` |
| `data-bench-worn` | `[{"row","col","upright":[…]\|null,"turned":[…]\|null}]` |
| `data-tablet` (desk) | `{}` or `{"10":"a","20":"b",…,"180":"q","190":"r","1000":"s",…,"9000":" "}` |
| `data-values` | `[10,20,…,9000]` (27 ints, ascending) |
| `data-notebook-sums` | `[sum,…]` in entries order |
| `data-path` (stencil) | `[[row,col],…]` |

All produced by Ruby `JSON.generate`: compact, UTF-8 unescaped, `/` unescaped, integers as
integers. Then attribute-escaped (`"` → `&quot;`, etc.).

### 9.4 Client storage (unchanged by the port; same origin required)
`localStorage`: `cistergraphia.lectio.best`, `cistergraphia.record`, `cistergraphia.chapters`,
`cistergraphia.desk.mode`, `cistergraphia.notebook.<seed>`, `cistergraphia.sound`. `data-play`
must be one of `intact worn broken ruined pathless tablet expert` (the record's rungs).

---

## 10. `/ask` protocol and the reader's game

### 10.1 `POST /ask`

Request (urlencoded or multipart): `svg` (the plate file, as the readback field holds it),
`pencil` (JSON object `{"<sum>": "<char>", …}`), `inked` (JSON array of sums already in ink).

```
content_type json
svg = submitted_svg                         # >2 MiB -> 413 (HTML body, JSON content type)
pencil, inked = notebook_sent:
   pencil = JSON.parse(params[:pencil].to_s).to_h { |sum, char| [Integer(sum), char.to_s] }
   inked  = JSON.parse(params[:inked].to_s).map { |sum| Integer(sum) }
   rescue ParserError/ArgumentError/TypeError/NoMethodError ->
      Cistergraphia::Error "The notebook could not be read: ask again"   (400)
held = Notebook.held(svg, pencil, inked)    # Error NO_TABLET "This plate has no tablet of its own: the shared one reads it" (400)
                                            # DecodeError "No way reads this plate" (400)
200: {"held":{"<sum>":"<char>",…}}           # keys are strings
```
Order: size check → JSON parsing → library (observed: `svg=""` with valid JSON → NO_TABLET 400).

Parsing details (Ruby, observed): `pencil` `"not json"` or missing → 400; `[]` (array) → `{}` OK;
`null` → `{}`; a non-integer key (`{"x":"a"}`) → 400; `inked` `{}` → `[]` OK; `inked` `null` → 400.
`Integer()` accepts surrounding whitespace, `_`, `0x`/`0b`/leading-0 octal, and truncates floats;
a non-string `char` is stringified (can never equal a tablet letter). Rust: accept JSON object
(string keys) or array of 2-element arrays or `null` for `pencil`; array (or empty object) for
`inked`; parse integer keys as decimal (± whitespace); anything else → 400 with the message.

`Notebook.held(svg, pencil, inked)`: `tablet, sums = plate(svg)` (tablet from the file's seed;
sums = entries order); `open = sums - inked`; `right = pencil.slice(*open)` keeping only
`tablet[sum] == char` (order of `open`); return `right` if `right.size >= 3` or
(`open.size < 3` and every open sum is in `right`), else `{}`. JSON key order = `open` order.

Tests: 2 right → `{}`; 3 right → those 3; `media_type == application/json`; bad JSON → 400; the
expert plate (pathless + own tablet) is read along its way by the scribe.

### 10.2 `playable?` and laying the reader's page (`web/reader_game.rb`)

```
playable?(svg):
  false if passphrase.is_some() || params[:decipher] == "1"
  read_marks(svg): @weathered = Lacunae.weathered?(svg); @broken = Fragments.broken?(svg);
                   @pathless = Pathless.pathless?(svg); @own = Tablet.seed_of(svg).is_some()
  false if @own && (@weathered || @broken)                 # two_to_find?
  @svg_content = PlateFile.clean(svg); false if nil
  lay(svg) is non-nil                                       # truthy result
lay(svg):
  @broken && @weathered && params[:rejoined]=="1" -> lay_rejoined
  @broken                                          -> lay_board
  @weathered ? lay_bench : lay_desk
lay_rejoined: whole = Fragments.rejoined(svg); @layout = whole.layout; @values = whole.signs;
  @damage = whole.damage; @entries = lacunae_from(whole.signs, @layout, @damage, whole.candidates);
  @plate_svg = PlateFile.clean(whole.svg); @rejoined = true; @broken = false; @size = isqrt(len)
lay_desk: Grille.check(svg, submitted_grille)   # raises GrilleKeyNeeded / "not written under a grille"
  @layout = submitted_grille || first of KEYLESS where letters(svg, way)
  return nil unless @layout && (@values = letters(svg, @layout))
  @size = isqrt; lay_desk_variant; @desk = true
lay_desk_variant: lay_pathless if @pathless && !submitted_grille
  @expert = @own && @pathless
  @notebook = (@expert ? [] : Notebook.entries(@values)) if @own
letters(svg, layout): seq = SVG.read(svg, layout:); Cistergraphia.decipher(seq); seq   (DecodeError -> nil)
lay_pathless: @grid = Pathless.grid(svg); @layout = :phaistos; @values = @grid.map { upright || 0 }
lay_board: @fragments = Fragments.read(svg); @size = Fragments::Join.side(@fragments)
lay_bench: @layout = submitted_grille || Survey.layout(svg)
  return nil unless @layout && (!submitted_grille || Survey.edition(svg, @layout))
  @values = Survey.values(svg, @layout); @size = isqrt; @tally = Tally::Plate.read(svg, @layout)
  @pathless ? lay_pathless_bench : lay_lacunae
lay_lacunae: @damage = Survey.damaged(svg, @layout).to_h { index => lost }; @entries = lacunae_in(svg, @layout, @values)
lay_pathless_bench: lay_pathless; @worn = worn_cells(svg); @entries = []; @damage = {}
```
Note `@desk` is nil on bench/board pages; `@notebook == []` (truthy in Ruby!) for the expert
plate — but `play.erb` tests `@expert` before `@notebook`, and `_notebook` gets `entries: []`.
In Rust model these as explicit enums/bools, not truthiness.

`rejoined=1` on a plate that is not ruined is ignored (broken → board; worn → bench; tested).
Sealed plates never play (passphrase given → decoder; no passphrase → no layout reads letters →
decoder error with the secret-mode hint).

### 10.3 The notebook flow (client, for context)
Guided/unaided desk; the reader pencils letters per sum; "Ask the scribe" requires ≥3 pencilled
open sums (or all remaining when <3 left) else a local message; POSTs `/ask`; confirmed sums go
into ink; all in ink → `Every sum is in ink: the tablet is yours.` and `recordWin()` (record line,
chapter date stored, `[data-chapter-source]` unhidden). Expert: notebook shut until the desk finds
the way; then built client-side from the row `<template data-notebook-row>`.

---

## 11. The chapter system (`web/chapter.rb`, `web/chapter_page.rb`, `web/chapter_check.rb`, `web/chapters.yml`)

### 11.1 `chapters.yml`

```yaml
readings:                 # ordered list; chapter n reads line (n-1) % len; append only
  - text: Listen my son to the precepts of the master
    source: The Rule of Saint Benedict, prologue
  - text: Let no day pass without a line
    source: After Pliny the Elder, on the painter Apelles
  … (55 readings today; last: "Never despair of the mercy of God")
feasts:                   # "MM-DD" -> {name, text, source}; fixed every year
  "01-26": {name: The Founders of Cîteaux, text: One charity and one rule and like customs, source: "Stephen Harding, the Charter of Charity"}
  "03-21", "07-03", "07-11", "08-20" (Saint Bernard of Clairvaux), "12-11"
```
Rules (`test/chapter_test.rb`): each text `\A[A-Za-z]+( [A-Za-z]+)+\z`, ≥ 6 words, ≤ 50 chars,
unique; feast keys are valid month-days (checked against leap year 2028) with a name. Strings are
UTF-8 (`’`, `î`, `à`). Load with a YAML parser at startup (`YAML.safe_load_file`); keep the file
verbatim (embed with `include_str!`).

### 11.2 Constants
- `FIRST = 2026-10-26` (a Monday, chapter 1).
- `WEEK` (by weekday, Sunday = 0): `1 → [intact, phaistos]`, `2 → [worn, any]`,
  `3 → [pathless, any]`, `4 → [broken, phaistos]`, `5 → [tablet, any]`, `6 → [ruined, phaistos]`,
  `0 → [expert, any]`. Insertion order (used by Check): 1,2,3,4,5,6,0.
- `CACHE = 16`; `BASE_URL = 'https://chapter.invalid'`.
- `Day { number, date, rung, layout, text, source, feast }`, `path = "/chapter/#{date.iso8601}"`.

### 11.3 Functions
- `today`: `parse(ENV['CHAPTER_DATE'])` or the current **UTC** date. (`Date.strptime(s,
  '%Y-%m-%d')` is lenient: `2026-1-5`, `+2026-01-05`, `2026-01-05x` parse; invalid → fallback.)
- `parse(text)`: date or nil (`2026-02-30` → nil). Route text is already `\d{4}-\d{2}-\d{2}`.
- `of(date)`: nil if `date < FIRST || date > today`, else `set(date)`.
- `set(date)`: `index = date - FIRST` (days); `feast = FEASTS[date.strftime('%m-%d')]`;
  `reading = feast || READINGS[index % READINGS.size]`; `rung, layout = WEEK[date.wday]`;
  `Day(number: index+1, date, rung, layout: way(layout, date), text: reading.text, source:
  reading.source, feast: feast&.name)`. A feast keeps the weekday's rung; the displaced reading
  is simply not read that cycle.
- `runway(date = today)` = `READINGS.size - (date - FIRST) - 1` (negative once the order repeats).
- `seconds_left(now = Time.now.utc)` = `(midnight UTC of tomorrow) - now.to_i`, clamped to
  `1..86400` (23:59:59 → 1; 00:00:00 → 86400). Independent of `CHAPTER_DATE`.
- `seeded(date, purpose = 'plate')` =
  `Random.new(SHA256.hexdigest("cistergraphia chapter #{purpose} #{date.iso8601}").to_i(16))`
  → Rust: `RubyRandom::from_be_bytes(&sha256(format!("cistergraphia chapter {purpose} {date}")))`
  (`from_be_bytes` already reproduces `hexdigest.to_i(16)` seeding, incl. MRI's
  drop-a-top-word-of-1 guard). Purposes: `plate`, `way`.
- `way(layout, date)`: `layout` unless `:any`; else `KEYLESS[seeded(date,'way').rand(5)]`
  (`rand_below(5)`, one draw) with KEYLESS = `[phaistos, rongorongo, leap, furrows, unwinding]`.
- `draw(day)` — one `random = seeded(day.date)` ('plate'), passed to the library, **draw order
  matters**:
  - `worn` → `Lacunae.inscribe(text, layout: day.layout, random:)[1]`
  - `broken`/`ruined` → `Fragments.inscribe(text, weathered: rung == ruined, random:)[1]` (layout default phaistos)
  - else (`intact`, `pathless`, `tablet`, `expert`) → `whole`: `pathless = rung ∈ {pathless, expert}`;
    `tablet = seed(random)` **first** if `rung ∈ {tablet, expert}`; then
    `Cistergraphia.inscribe(text, layout:, pathless:, tablet:, random:)[1]`.
  - `seed(random)` = 10 × `ALPHABET[random.rand(32)]` joined, ALPHABET
    `0123456789ABCDEFGHJKMNPQRSTVWXYZ`.
  - The text is passed as written (capitals); the library normalises.
- `plate(day)` = `PlateFile.clean(draw(day))`.
- `kept(key) { block }`: LRU of 16 entries; lookup under a mutex (a hit moves the key to the
  newest position); on a miss the block runs **outside** the lock (concurrent misses may each
  render; last write wins), then insert and evict oldest while size > 16.

### 11.4 Seeding parity
Nothing is stored: every server draws the same plate from the date. The Rust port reproduces
plates only if (a) `RubyRandom` matches MT19937/MRI (done, vector-tested), (b) the library calls
consume draws in exactly Ruby's order and kinds, (c) SVG rendering is byte-identical. Verify
with goldens of `/chapter/2026-10-26` … `/chapter/2026-11-01` (every rung) under
`CHAPTER_DATE=2026-11-01` (§14). Day → layout for the first week can be read from those pages
(`<dt>Layout</dt>`).

### 11.5 Rake tasks to mirror (CLI subcommands)
- `chapter:check[days=90]`: `Check.check(days:, from: max(today, FIRST), texts: READINGS +
  FEASTS.values)`: (1) **trials**: `texts.product(WEEK.values).each_with_index` → `Day(number 0,
  date: from + i, rung, layout: way(layout, from + i), text)`; (2) **ahead**: `set(from + i)` for
  i in 0...days. For each day: `edition = Cistergraphia.decode(Chapter.draw(day))` (raw SVG, not
  cleaned); trouble `"#{label}: reads #{edition}"` unless `holds?(edition, normalize(text))`;
  library `Error` → `"#{label}: #{e.message}"`. Label: `"Chapter #{n}, #{date}"` or `A trial`,
  then ` (#{rung}, #{layout}): #{text}`. Prints troubles to stderr, aborts with
  `N chapter(s) would not read`, else `Every text reads under every rung, and the next D days are set.`
  `holds?(edition, text)`: tokens of `/\[([^\]]*)\]|([^\[])/` → plain `[c]` or bracket
  `split('/', -1)`; same count as `text` chars and each char among its token's options
  (`h[e]ll[o/p]` holds `hello`; `a[ ]b` holds `a b`).
- `chapter:runway`: prints `"#{READINGS.size} readings; #{left} days of new readings left after
  today."`; aborts `"Under #{floor} days: append readings to web/chapters.yml (docs/operations.md)"`
  if `left < RUNWAY` (env, default 14).

### 11.6 `chapter_page(date)` (routes §5.7–5.9)
```
day = date && Chapter.of(date)
unless day: halt_with 404, not_a_chapter(date):
   date nil          -> "This is not a date the abbey keeps. A chapter’s address is its day: /chapter/2026-10-26."
   date > today      -> "This chapter is not yet written: it is set on its day, at midnight UTC."
   else (< FIRST)    -> "The chapters begin on 26 October 2026."
params.keep only "rejoined"       # nothing else a request carries lays it (grille, passphrase, svg…)
page = Chapter.kept([request.path, params[:rejoined] == "1", Chapter.today]) {
   lay_chapter(day); @base_url = BASE_URL; render play (with layout) }
page.gsub("https://chapter.invalid", escape_html(request.base_url))
lay_chapter: @page = :chapter; @chapter = day; @title = "Chapter #{n}";
   @description = "The day's chapter: one plate the abbey sets each day, the same for every reader.";
   @svg_content = Chapter.plate(day); read_marks(@svg_content); lay(@svg_content)
```
- The 404s happen before the cache and after the route's Cache-Control; `@page` is not set (no
  nav item current), title is `Cistergraphia`.
- The cached string holds the placeholder host in `og:url`/`og:image`; each response writes the
  requester's escaped base URL in (tested: no `abbey.example` leak to `scriptorium.example`, no
  `chapter.invalid` in output, `?grille=` ignored).
- `lay` runs with filtered params: `rejoined=1` on a ruined day lays the rejoined bench (GET or
  POST), on other days it is ignored (but is still a distinct cache key).
- `_board`'s restore form posts to `@chapter.path`. `data-chapter` = ISO date. The text is never
  in the page (tested); the source is (hidden).
- Rungs → furniture (tested): Mon desk, Tue bench, Thu `data-fragments`, Fri `data-notebook`,
  Sun `data-play="expert"`; Sat ruined: board with `action="/chapter/2026-10-31" class="rejoin-restore"`,
  `POST rejoined=1` → `Ruined, rejoined: worn`.
- A rung whose plate cannot be laid (lay → nil) or a drawing failure → exception → 500 (ops doc:
  "`/chapter` answers 500 … run `rake chapter:check`").

---

## 12. Static assets (copy verbatim from `public/`)

```
public/css/app.css
public/js/app.js  public/js/ink.js  public/js/ways.js  public/js/sound.js
public/favicon.svg
public/og.png
public/fonts/OFL.txt
public/fonts/eb-garamond-latin-400-italic.woff2
public/fonts/eb-garamond-latin-400-normal.woff2
public/fonts/eb-garamond-latin-600-normal.woff2
```
Serving (Sinatra `static!`, before routes and **before the before-filter**, so no CSP/Referrer/
Permissions headers on assets; observed):
- GET/HEAD only (POST to a static path → routed → 404 Erratum).
- Path = cleaned `PATH_INFO`, URL-decoded, must be a regular file under `public/`; directories
  (`/css/`) → route-miss 404. Query strings ignored (`/js/app.js?v=2` → 200).
- Headers: `cache-control: public, max-age=3600`, `last-modified` (file mtime, HTTP date),
  `content-type`: `.css` `text/css;charset=utf-8`, `.js` `text/javascript;charset=utf-8`,
  `.svg` `image/svg+xml`, `.png` `image/png`, `.woff2` `font/woff2`, `.txt`
  `text/plain;charset=utf-8`; plus `x-content-type-options: nosniff`, `vary`, gzip (§2.3).
- `If-Modified-Since` ≥ mtime → **304** with only `cache-control` + `nosniff` (observed);
  `Range: bytes=0-9` → **206** `content-range: bytes 0-9/69678`.
- Only server endpoint referenced from JS: `POST /ask` (§9.1). No JS fetches assets dynamically
  (fonts via CSS `url()` — check `app.css` `@font-face` paths stay `/fonts/...`).

---

## 13. Porting checklist (axum)

### 13.1 Crate layout and dependencies (`crates/rustergraphia-web`)
- `axum` (0.7/0.8), `tokio` (multi-thread), `tower`, `minijinja` (+ `minijinja-embed` or
  `include_str!` loader), `serde`/`serde_json` (**struct field order** or `preserve_order`;
  never a `BTreeMap` for ordered objects: `{"10":…,"100":…}` would sort lexicographically),
  `flate2` (gzip), `sha2`, `chrono` or `time` (UTC dates, `%A %-d %B %Y`), a YAML crate
  (`serde_yaml`/`yaml-rust2`/`serde_norway`), `subtle` (constant-time compare),
  `form_urlencoded`, `multer` (or axum `Multipart`), `httpdate`, `regex` (ASCII classes).
- `main.rs`: read env (§1), build router, bind `0.0.0.0:$PORT`.

### 13.2 Mapping

| Ruby piece | axum equivalent |
|---|---|
| Sinatra routes | `Router::new().route("/up", get(up)).route("/", get(index)).route("/generate", post(generate)).route("/decode", get(decode_form).post(decode)).route("/about", get(about)).route("/chapter", get(chapter_today)).route("/chapter/:date", get(chapter_get).post(chapter_post)).route("/ask", post(ask)).fallback(not_found)`; validate `:date` against `^[0-9]{4}-[0-9]{2}-[0-9]{2}$` in the handler and fall back to the route-miss 404 (with `x-cascade: pass`) when it does not match. Non-GET/POST methods on known paths must be route-miss **404**, not axum's 405: use a custom `method_not_allowed_fallback` or route `any()` → 404. |
| Middleware order | one outer custom layer (or `tower` stack) applying, outside → in: nosniff + (html) xss/frame headers; PathTraversal path rewrite (do it in a `map_request` *before* routing: `Router::layer` runs after routing, so wrap the Router as a service inside the layer); gzip (§2.3, custom, gzip-only, all content types); BodyLimit (Content-Length check); FrontDoor; then static/route. Raise axum's `DefaultBodyLimit` (default 2 MB **< 2_162_688 + multipart overhead**) to at least the BodyLimit value or disable it and enforce yourself. |
| `before` filter | a per-route response finaliser (`leaf_headers(req_path, method, ssl, &mut HeaderMap)`) applied to every routed response incl. errors/404, not to static files |
| `after` filter | `svg::forget()` at the end of the `spawn_blocking` closure, every path |
| helpers modules | `web/view.rs`, `bench.rs`, `tray.rs`, `figures.rs`, `form.rs`, `refusals.rs`, `plate_file.rs`, `reader_game.rs`, `chapter.rs`, `chapter_page.rs`, `headers.rs` |
| ivars | a `PageState` struct (explicit `Option`s and enums: `Rung`, `Furniture::{Desk, Notebook, Expert, PathlessDesk, Bench, PathlessBench, Board, RejoinedBench}`) serialised into the template context |
| ERB templates | minijinja templates `templates/*.html` ported line by line from `views/*.erb` |
| `erb :_x, locals:` | render partial to a `String` in Rust and pass as `Value::from_safe_string`, or `{% include %}` within `{% with %}` |
| `<%== yield %>` | render page, then `layout.html` with `content` (safe) |
| `Chapter::CACHE` | `Mutex<LruLike>` (a `VecDeque`/`IndexMap` of 16), render outside the lock |
| `Rack::Deflater` | custom gzip layer (tower-http `CompressionLayer` would add br/zstd/deflate, skip images and bodies < 32 B, and lower-case the Vary value) |
| static | custom handler over embedded or on-disk `public/` with the exact headers/304/206 (§12); `tower-http::ServeDir` needs header overrides (its `.js` type and missing `;charset=utf-8` differ) |
| Puma threads | `spawn_blocking` for every request that touches the library (`/generate`, `POST /decode`, `/ask`, chapter renders, and `GET /`/`/about` if glyph tables are not precomputed) + a semaphore |
| errors | `enum AppError { Domain(String) /*400*/, Halt(u16, String), NotFound, Bug }` → error page; map library `Error` variants: all user-input errors → 400 |

**Templates (recommended approach): minijinja with a custom formatter, no trim flags.**
1. `env.set_formatter(|out, state, value| …)`: `undefined`/`none` → nothing; safe strings → raw;
   everything else → `value.to_string()` escaped with the Erubi table (`& < > " '` →
   `&amp; &lt; &gt; &quot; &#39;`). Booleans print `true`/`false`; integers decimal. Never emit
   minijinja's own `&#x2f;`/`&#x27;`.
2. Leave `trim_blocks`/`lstrip_blocks` **off**; port each tag-alone line by deleting its
   indentation and its newline (put `{% … %}` flush against the start of the next line);
   leave tags that share a line untouched. `{# #}` header comments: same treatment. This is
   mechanical; a small script can do it while converting.
3. Keep template logic thin: compute strings (`puzzle_state`, `condition_invalid`, ledes'
   counts, JSON blobs, SVG figures) in Rust; templates choose branches and print.
4. Golden-test every page against Ruby output (§14). Whitespace is part of the contract (JS
   reads `textContent` of `data-transcription`; the stability suite measures layout).

### 13.3 Library API the web layer needs (Ruby names; snake_case in Rust)
`Cistergraphia.inscribe/decode/decipher/normalize`, `CHARACTERS`, `TABLET`, `EVIL_MARKER`;
`Glyphs::STEM/LINES/STROKE_PLACES`, `Glyphs.strokes`; `Stylus.press`; `Layout.path/cell/origin`,
`Layout::KEYLESS/PADDING/SPACE`; `SVG.read/stencil`, `SVG::Parsed.of/forget`; `Grille.new/
generate/check`, `Grille#to_s/as/novice/path/holes/part`, `Grille::EXAMPLE/TURNS`;
`TwoReaders.inscribe/read`, `Reading{message,decoy,reader}`; `Lacunae.inscribe/weathered?`,
`Lacunae::Survey.layout/edition/values/damaged/worn_cells`; `Fragments.inscribe/read/broken?/
rejoined`, `Fragments::Join.side`, `Fragments::BOARD`, `Fragments::Tray::GAP`, fragment
`rows/cols/cells/readings`; `Pathless.pathless?/grid`; `Tablet.generate/seed_of/show/rows/
characters`, `Tablet::ALPHABET/LENGTH/SHORT`; `Notebook.entries/pairs/held`,
`Notebook::SPELLINGS`; `Tally::Plate.read`; `Weathering.candidates/surviving`;
`Secret.passphrase_problem`, `Secret::MIN_PASSPHRASE_LENGTH`; every `NOT_*` refusal string and
error message verbatim; `random:` parameters accepting `RubyRandom`.

### 13.4 Step-by-step
1. Static serving + headers + gzip + BodyLimit + FrontDoor + `/up` (+ unit tests from §2).
2. Error page + layout + `GET /`, `/decode`, `/about` (goldens; mask Lectio's random digit).
3. Helpers (§7) with unit tests against Ruby-produced strings.
4. `POST /generate` (all refusals → 422 goldens; plate pages: structural tests).
5. `POST /decode` (decoded + every reader's page variant; goldens from fixed SVG inputs).
6. `/ask`.
7. Chapter (goldens for the first week under `CHAPTER_DATE=2026-11-01`), CLI `chapter-check`,
   `chapter-runway`.
8. Port `test/app_test.rb` and `test/chapter_test.rb` assertions; run `test/ui/*` against the
   Rust server via `BASE_URL`.

---

## 14. Parity test plan

### 14.1 Running the Ruby original
```sh
cd /home/user/cistergraphia
export BUNDLE_PATH=/tmp/<scratch>/gems BUNDLE_WITHOUT=development
BUNDLE_FROZEN=true bundle install            # does not modify the repo
RACK_ENV=production bundle exec puma -C config/puma.rb config.ru   # :9292
# or in-process: ruby -e 'require "rack/test"; require "./app"; include Rack::Test::Methods; def app = CistergraphiaApp; …'
```
(`bundle exec` without these gems fails: they are not installed system-wide.)

### 14.2 Golden fixtures (bake into `crates/rustergraphia-web/tests/fixtures/`)
Deterministic responses to capture (status, headers minus Date/Content-Length, body):
- `GET /`, `GET /decode`, `GET /about` (normalise `data-number="[1-9]"` and the Lectio glyph),
  `GET /nope`, `GET /up`, BodyLimit 413, FrontDoor 403 (key set).
- 422 refusals of `/generate` (no random draws before the refusal): each refusal of §7.5, short
  passphrase, bad grille key, too-long pair.
- 400/413 errors of §5.6 and §10.1.
- `POST /decode` with fixed SVGs produced by Ruby with seeds (as the suites do:
  `Lacunae.inscribe(MSG, random: Random.new(5))`, `Fragments.inscribe(MSG, random: Random.new(3))`,
  ruined with `weathered: true`, `srand(11); inscribe(MSG, layout: :leap, pathless: true)`,
  own tablet `K7Q2M-ZXB4P`, expert `Q4M8T-RJ2WX`, tallied `random: Random.new(7)`, grille
  `K7Q2M-ZXB4P`, two readers, sealed): with and without `decipher=1`, ruined with `rejoined=1`.
  Save the SVG inputs as fixtures too.
- Chapters `/chapter/2026-10-26` … `/chapter/2026-11-01`, `/chapter` and `POST
  /chapter/2026-10-31 rejoined=1`, all with `CHAPTER_DATE=2026-11-01`, Host `example.org`.
- `/generate` success pages draw at random (`Tablet.generate`, `Grille.generate`,
  `Random.new` in Lacunae/Fragments, SecureRandom in Secret): assert structure (port the
  app_test assertions), or render the `result` template in-process with fixed library outputs.
  (Ruby intact game plates become deterministic under `srand(n)`; Rust could mirror that with a
  test-only seed hook if desired.)

### 14.3 Browser suites against the Rust server
`test/ui/*.rb` accept `BASE_URL=http://127.0.0.1:PORT` (they then skip booting Puma; need Node,
Playwright, Chromium, axe-core). `play.rb` expects the server's today to be `2026-11-01`: start
the Rust server with `CHAPTER_DATE=2026-11-01`. Suites: `rake ui:stability` (nothing moves, JS
on/off), `rake ui:play` (behaviour, a11y, 60 KB transfer budget), `rake ui:e2e` (every form
choice round-trips).

---

## 15. Open questions and risks

Risks (parity traps), most severe first:
1. **Template whitespace/escaping**: Erubi trim rule ≠ minijinja trim flags; minijinja's default
   HTML escaping differs (`/`, `'`), and `none` prints `none`. Byte parity needs the custom
   formatter + manual whitespace port + goldens (§13.2).
2. **Chapter plates** depend on the library port consuming `RubyRandom` draws in Ruby's exact
   order (tablet seed before inscribe; Lacunae/Fragments internal draws) and on byte-identical
   SVG; a mismatch silently changes every reader's daily plate (and goldens only cover the dates
   you capture).
3. **JSON key order and number types** in `data-*` blobs: insertion order (`padding, cell,
   offset, size, grille`; lacuna `sign, x, y, turned, place, lost, digits, known, open`;
   CHARACTERS/LINES numeric-string keys in numeric order). `serde_json` without
   `preserve_order` sorts map keys.
4. **`POST /decode` is sent `Cache-Control: public, max-age=300`** (leaf rule overrides
   `no-store`; observed), although docs say "anything posted: no-store". The decoded message is
   in that response. Parity says replicate; privacy says `no-store`. **Decide** (recommend asking
   the maintainer; the Worker never caches POSTs, browsers may).
5. **Double-escaped `aria-invalid=&quot;true&quot;`** on the tablet/path radios (§8.2.1):
   replicate for byte parity, or fix in both codebases.
6. `PlateFile.clean` depends on REXML specifics: local vs prefixed names, attribute value
   decoding/normalisation (newlines/tabs in values, entity expansion), DOCTYPE handling. `xmltree`
   may differ on crafted input; regex classes must be ASCII (§7.7).
7. axum defaults that differ: 405 for wrong method (Ruby: 404 + `x-cascade: pass`), 2 MB
   `DefaultBodyLimit` (Ruby accepts bodies up to 2_162_688 and multipart overhead), routing
   before middleware (PathTraversal must run before routing), HEAD `content-length` (Ruby
   reports 0; hyper reports the GET length — acceptable).
8. Thread-local parse cache vs async: run each request's library work in one blocking closure and
   always `forget()`.
9. `/ask` errors return the HTML Erratum with `content-type: application/json` (quirk; JS only
   checks `response.ok`). Replicate or return JSON? Parity: replicate.
10. Rack param edge cases not worth replicating unless required: nested keys (`a[]`), Ruby
    `Integer()` quirks (octal `012`, `0x`, `_`), invalid UTF-8 in form values (Ruby may raise →
    500), percent-encoded route paths. Decide "out of scope".
11. Protections that never deny (`reaction: :drop_session`): do not add Origin/IP checks.
12. Deflater compresses everything incl. PNG/woff2 and 2-byte bodies, gzip only; 406 on
    `identity;q=0`. A stock compression layer differs (br, size/content-type predicates).
13. `Date.strptime` leniency for `CHAPTER_DATE` (accepts `2026-1-5`, trailing garbage).
14. `about` Lectio digit uses the process RNG (non-deterministic by design).
15. Float formatting in figures (`disc_spiral_svg`, `grille_order_svg`): Ruby `Float#round(1)`
    and `Float#to_s` (`50.0`); Rust `{}` prints `50`. Reuse the stylus port's rounding helper.

Open questions:
- Keep `public, max-age=300` on `POST /decode` (risk 4) and the escaped `aria-invalid` (risk 5)?
- Should the Rust server also serve the Ruby CLI-only features (`tally` on the form)? (Ruby: no.)
- Production bind address/port (Ruby Docker uses `PORT=8080`) and whether `WEB_CONCURRENCY`/
  `PUMA_THREADS` map to a concurrency limit.

## 16. Decisions taken in the port

| # | Question (§15) | Decision | Why |
|---|---|---|---|
| 4 | `POST /decode` cache-control | **Deviate: `no-store`** on every POST (`/decode`, `/ask`, `/generate`, `/chapter/<date>`); GET/HEAD keep Ruby's cache rules. | The response carries the decoded message. A shared cache keeping it for five minutes is a privacy leak, and `docs/serving.md` in the original already promises "anything posted: no-store". The golden suite masks this one header on those responses and nothing else. Worth fixing upstream in `web/leaf_headers.rb`. |
| 5 | Escaped `aria-invalid=&quot;true&quot;` | **Replicate** for byte parity (commented at the template). | Harmless to assistive tech in practice (the attribute is malformed, so ignored either way); fix in both codebases together. |
| 9 | `/ask` error content-type | **Replicate** (HTML Erratum under `application/json`). | The page script only checks `response.ok`. |
| 10 | Rack param edge cases | **Mostly replicated now**, found by diffing the running Ruby app (tests/app_test.rs, last section): a `%` that is not an escape is Sinatra's 400 `Invalid query parameters: …` (query, form body, static files too, before the leaf headers); a multipart body cut short is the 400 `Invalid multipart/form-data: Rack::Multipart::EmptyContentError`; any `multipart/*` with a boundary is read; a POST without a content type is read as a form; the notebook's JSON is read with Ruby's `Integer()`, `to_h` and `to_s` (src/notebook_sent.rs). **Still out of scope**: nested keys (`a[]`, `a[b]`, which Ruby echoes as `Array#to_s`/`Hash#inspect`), invalid UTF-8 in form values, a JSON number past a Float (`1e400`). | No honest client sends them; what was cheap to match is matched. |
| — | Ruby's refusals under gzip | **Not replicated** (a Ruby bug). | `BodyLimit::REFUSED` and `FrontDoor::REFUSED` are frozen arrays: Rack::Deflater's `response[2] = GzipStream…` raises FrozenError for a client that accepts gzip (a 500 backtrace page), after writing `content-encoding: gzip` into the shared headers hash, so every later plain refusal claims gzip. The port answers 413/403 in gzip. Fix upstream: `.dup` the constants (or `[*REFUSED]`). |
| — | Empty or doubled SVG file | **Fixed in the library** (`rustergraphia::svg::parse`). | REXML reads `""` as a document with no root (`Not an SVG document`) and a second root element as malformed; xmltree refused the first and silently dropped the second root. |
| — | Configuration | `rustergraphia_web::app(Config)`; the binary reads `Config::from_env()`. | The suites set the front door key and the chapter date per test, never through the process environment. `Config::plate_seed` (never read from the environment) draws every plate from `Random.new(seed)`, so tests/seeded.rs holds result pages to the Ruby app's bytes (captured by tools/capture-seeded.rb). |
| 15 | Float formatting in figures | Ruby `Float#round(1)`/`to_s` reproduced (`50.0`). | Byte parity; the stylus rounding rules are shared. |
| — | Bind address/port | `PORT` env (default 9292), all interfaces. | Matches `rackup` locally and the Docker image's `PORT` convention. |
