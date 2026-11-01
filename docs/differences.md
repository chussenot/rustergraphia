Differences from the Ruby original
==================================

Rustergraphia is built to be the same program as
[cistergraphia](https://github.com/chussenot/cistergraphia). The two read and
write the same plate files, a seeded plate comes out byte-identical from
both, and the web pages render pixel for pixel alike
([ui-proofs.md](ui-proofs.md)). This page lists the few places where they
differ, and says why.

Deliberate differences
----------------------

### Posted pages are never cached

The original sends `POST /decode` responses with
`Cache-Control: public, max-age=300`. A shared cache may therefore keep a
decoded message, plaintext included, for five minutes. The original's own
`docs/serving.md` promises "anything posted: no-store".

The clone sends `no-store` on every POST (`/generate`, `/decode`, `/ask`,
`/chapter/<date>`). GET and HEAD keep the original's cache rules.

*Upstream fix:* `web/leaf_headers.rb`.

### Refusals to a client that accepts gzip

In the original, the 413 (body too large) and 403 (front door) refusals are
frozen constants. When the client accepts gzip, `Rack::Deflater` tries to
replace the frozen body and raises `FrozenError`, so the client gets a 500
backtrace page. Before failing it has already written `content-encoding: gzip`
into the shared headers, so every later uncompressed refusal claims to be
gzipped.

The clone sends a correct 413 or 403, gzipped when the client accepts it.

*Upstream fix:* `.dup` the `REFUSED` constants in `web/body_limit.rb` and
`web/front_door.rb`.

Differences you might notice
----------------------------

| Area | Ruby original | Rust clone |
|---|---|---|
| CLI abbreviations | OptionParser accepts unambiguous prefixes, such as `--weath` for `--weathered` and `-l rongo` for `-l rongorongo`. | Flags and values must be written in full. clap rejects a prefix and lists the accepted values. |
| CLI default output | `tmp/inline.svg` and `tmp/output.png`, next to the `cistergraphia` script. | `tmp/inline.svg` and `tmp/output.png`, under the current directory. |
| Request log | Sinatra logs every request in Common Log Format. | The server logs one line at startup (`listening on …`) and nothing per request. |
| Chapter checks | `rake chapter:check` and `rake chapter:runway`. | Subcommands of the web binary: `rustergraphia-web chapter-check [DAYS]` and `rustergraphia-web chapter-runway`. |
| Environment | `PORT`, `FRONT_DOOR_KEY`, `CHAPTER_DATE`. | The same three, plus `RUSTERGRAPHIA_PUBLIC` to serve the static files from another directory. |
| Concurrency settings | Puma, sized by `WEB_CONCURRENCY` and `PUMA_THREADS`. | One Tokio process, with no worker or thread settings. Plate work runs on Tokio's blocking pool. |
| Deployment | Cloud Run: 1 vCPU, 512 MiB, 3 requests per instance; the `ruby` slim image. | The same design ([going-live.md](going-live.md)): 1 vCPU, 256 MiB (first-generation environment), 10 requests per instance; a 48 MB distroless image. The Terraform names everything after the service, and `zone_settings` lets both sites share one Cloudflare zone. |
| `HEAD` responses | `content-length: 0`. | The length of the matching GET, as hyper reports it. |
| Early 413 | Puma reads the whole body, then refuses it. | The connection closes without reading the rest, so a client that is still uploading may see a broken pipe instead of the 413. |

Not ported
-----------

These are parts of the original's repository rather than features of the
program:

- **Browser suites and assets tasks.** The Playwright suites
  (`rake ui:stability`, `ui:play`, `ui:e2e`) and `rake og:image`. The social
  preview image is served as-is from `public/og.png`.
- **Research.** `experiments/`: the cryptanalysis and weathering studies.

Rack parsing edge cases, out of scope
-------------------------------------

No honest client sends these:

- **Nested parameter keys** such as `message[]=…` or `message[a]=…`. Ruby
  echoes them back as an array or a hash; the clone shows an empty field.
  Both return the same status code.
- **A JSON number beyond the float range** (`1e400`) in the notebook sent to
  `/ask`. Ruby fails with a 500 or ignores it; the clone answers 400.
- **Invalid UTF-8 in form values.** The clone decodes it lossily. This was
  not compared against Ruby.

Everything else
---------------

Everything not listed above is meant to behave exactly as the original. That
includes two quirks that are copied on purpose for byte parity:

- an over-escaped `aria-invalid` attribute on two radio buttons of the form;
- the HTML error page served as `application/json` when `/ask` fails.

The reasoning for each decision is in
[web-port-spec.md](web-port-spec.md) §16. Parity is checked in CI by the
golden and seeded page tests, by the 139 ported HTTP tests, and by
`tools/interop-check.sh`.
