//! test/app_test.rb, ported: the app driven in-process as Rack::Test drove
//! the Sinatra one, test by test, with the Ruby names (and their `test_`
//! prefix, so each maps back to its original by a grep) and the Ruby
//! assertions' exact strings.
//!
//! Where a test reached into the Ruby library, the Rust library stands in:
//! it is a byte-identical port, so a plate inscribed from `Random.new(3)`
//! here is the plate the Ruby suite read. Where a test reached into Ruby
//! internals with no Rust counterpart, the observable intent is ported and
//! the rest is said where it stands.

mod common;

use std::collections::BTreeMap;

use rustergraphia::grille::{Grille, Part};
use rustergraphia::layout::{self, Layout};
use rustergraphia::svg::Damage;
use rustergraphia::{
    cipher, fragments, glyphs, grille, lacunae, notebook, pathless, stylus, tablet, tally,
    two_readers, RubyRandom, Value,
};
use rustergraphia_web::{bench, form, middleware, Config};
use serde_json::{json, Value as Json};

use common::*;

const GRILLE_KEY: &str = "K7Q2M-ZXB4P";

// --- The Ruby library's calls, as the suite made them

/// `Random`: the process's own, as the Ruby calls without `random:` drew.
fn random() -> RubyRandom {
    RubyRandom::new_os()
}

fn the_grille() -> Grille {
    Grille::new(GRILLE_KEY, Part::Whole).expect("a key")
}

/// `Cistergraphia.inscribe(message, layout:, pathless:, tablet:)`
fn inscribe(
    message: &str,
    layout: &Layout,
    pathless: bool,
    seed: Option<&str>,
) -> (Vec<Value>, String) {
    cipher::inscribe(message, None, layout, false, pathless, seed, &mut random())
        .expect("inscribed")
}

/// `Cistergraphia.to_svg(Cistergraphia.encode(message, passphrase:))`
fn to_svg_encoded(message: &str, passphrase: Option<&str>) -> String {
    let sequence =
        cipher::encode(message, passphrase, false, None, &mut random()).expect("encoded");
    cipher::to_svg(&sequence, &Layout::Phaistos, false, false, None).expect("drawn")
}

/// `Cistergraphia::Lacunae.inscribe(message, layout:, tally:, pathless:, random:)`
fn worn(
    message: &str,
    layout: &Layout,
    tally: bool,
    pathless: bool,
    random: &mut RubyRandom,
) -> (Vec<Value>, String, Damage) {
    lacunae::inscribe(message, layout, None, tally, pathless, random).expect("weathered")
}

/// `Cistergraphia::Fragments.inscribe(message, weathered:, random:)`
fn broken(
    message: &str,
    weathered: bool,
    random: &mut RubyRandom,
) -> (Vec<Value>, String, Vec<fragments::Piece>, Damage) {
    fragments::inscribe(message, &Layout::Phaistos, weathered, random).expect("broken")
}

fn decode(svg: &str) -> String {
    cipher::decode(svg, None, None).expect("decoded")
}

/// `Cistergraphia::CHARACTERS`: sum => character, the shared tablet.
fn characters() -> BTreeMap<Value, char> {
    rustergraphia::character_sums()
        .map(|sum| (sum, rustergraphia::character_of(sum).expect("a sum")))
        .collect()
}

fn int(value: &Json) -> i64 {
    value
        .as_i64()
        .unwrap_or_else(|| panic!("{value} is an integer"))
}

fn cells_of(body: &str) -> Vec<Json> {
    json_attr(body, "data-plate-cells")
        .as_array()
        .expect("cells are a list")
        .clone()
}

/// `plate_of(body)`: the cells, the layout and the file of a result page.
fn plate_of(body: &str) -> (Vec<Json>, Json, String) {
    (
        cells_of(body),
        json_attr(body, "data-plate-layout"),
        hidden_svg(body),
    )
}

fn row_col(cell: &Json) -> (i32, i32) {
    (int(&cell[0]) as i32, int(&cell[1]) as i32)
}

fn walked(cells: &[Json]) -> Vec<(i32, i32, bool)> {
    cells
        .iter()
        .map(|cell| {
            let (row, col) = row_col(cell);
            (row, col, cell[3].as_bool().unwrap_or(false))
        })
        .collect()
}

fn values_of(cells: &[Json]) -> Vec<Option<Value>> {
    cells
        .iter()
        .map(|cell| cell[2].as_u64().map(|v| v as Value))
        .collect()
}

fn size_of(layout: &Json) -> usize {
    layout["size"].as_u64().expect("a size") as usize
}

fn except_size(layout: &Json) -> Json {
    let mut layout = layout.clone();
    layout.as_object_mut().expect("an object").remove("size");
    layout
}

/// `cells.sort == cells`: grid order, row by row.
fn in_grid_order(cells: &[Json]) -> bool {
    let places: Vec<(i32, i32)> = cells.iter().map(row_col).collect();
    let mut sorted = places.clone();
    sorted.sort();
    places == sorted
}

fn ok(session: &Session, message: &str) {
    assert!(
        session.last().is_ok(),
        "{message}: expected 200, got {}",
        session.status()
    );
}

fn header(session: &Session, name: &str) -> String {
    session.last().header(name).unwrap_or_default()
}

// --- The tests

#[test]
fn test_index_renders_the_form_and_links_to_decode() {
    let mut s = Session::new();
    s.get("/");

    ok(&s, "/");
    assert_match(
        r#"<form [^>]*method="post" action="/generate""#,
        s.body(),
        "the form",
    );
    assert_includes(s.body(), r#"href="/decode""#, "the link");
}

#[test]
fn test_pages_send_a_strict_content_security_policy() {
    let mut s = Session::new();
    s.get("/");

    let csp = header(&s, "content-security-policy");
    assert_includes(&csp, "default-src 'self'", "csp");
    assert_includes(&csp, "frame-ancestors 'none'", "csp");
    assert_eq!("no-referrer", header(&s, "referrer-policy"));
    // nothing inline for the CSP to block
    refute_match(
        r#"<script>|<style>| style="| on[a-z]+=""#,
        s.body(),
        "inline",
    );
}

#[test]
fn test_the_health_check_answers_and_is_never_cached() {
    let mut s = Session::new();
    s.get("/up");

    ok(&s, "/up");
    assert_eq!("up", s.body());
    assert_includes(&header(&s, "cache-control"), "no-store", "cache");
}

/// Ruby counted the plates REXML parsed on the test's thread and checked the
/// two Thread-locals (`:cistergraphia_document`, `:cistergraphia_layers`)
/// were empty after each request. Neither is observable here: the Rust
/// library's parse cache (`rustergraphia::svg`'s thread-locals) exposes no
/// parse counter and no "kept" accessor, and the router runs each request's
/// library work on a pooled blocking thread, not the test's. What the route
/// promises is kept in `handlers::route`: every path, a panic's too, ends
/// with `rustergraphia::svg::forget()`. What is observable is ported: every
/// plate the Ruby test brought, each way it brought it, is answered, and a
/// plate read after another on the same pooled threads reads as itself
/// (nothing of the previous one is taken for it).
#[test]
fn test_every_plate_brought_to_read_is_parsed_once_and_forgotten() {
    let message = "meet me at the old abbey after vespers by the well";
    let mut seeded = RubyRandom::new(4);
    let plates = [
        (
            "worn",
            worn(message, &Layout::Phaistos, false, false, &mut seeded).1,
        ),
        (
            "intact",
            cipher::inscribe(
                message,
                None,
                &Layout::Phaistos,
                false,
                false,
                None,
                &mut seeded,
            )
            .expect("inscribed")
            .1,
        ),
        (
            "expert",
            cipher::inscribe(
                message,
                None,
                &Layout::Unwinding,
                false,
                true,
                Some("K7Q2MZXB4P"),
                &mut seeded,
            )
            .expect("inscribed")
            .1,
        ),
        ("ruined", broken(message, true, &mut seeded).1),
    ];
    let mut s = Session::new();
    for (name, svg) in &plates {
        for extra in [&[][..], &[("decipher", "1")][..]] {
            let mut form = vec![("svg", svg.as_str())];
            form.extend_from_slice(extra);
            s.post("/decode", &form);
            ok(&s, &format!("{name} {extra:?}"));
            if extra.is_empty() {
                continue;
            }
            assert_includes(s.body(), "data-transcription", name);
            // an intact plate reads as its text; a damaged one as its edition
            if matches!(*name, "intact" | "expert") {
                assert_includes(s.body(), message, &format!("{name}: read as itself"));
            } else {
                assert_match(
                    r#"class="leiden-"#,
                    s.body(),
                    &format!("{name}: its edition"),
                );
            }
        }
    }
    let ruined = &plates[3].1;
    s.post("/decode", &[("svg", ruined), ("rejoined", "1")]);
    ok(&s, "ruined rejoined");
    assert_includes(
        s.body(),
        "Whole again: the fragments are rejoined",
        "rejoined",
    );
}

#[test]
fn test_the_leaves_may_be_kept_a_while_and_say_so() {
    let mut s = Session::new();
    for path in ["/", "/decode", "/about"] {
        s.get(path);

        assert_includes(&header(&s, "cache-control"), "max-age=300", path);
        assert_includes(&header(&s, "cache-control"), "public", path);
    }
}

/// Rack::Test set the server's `HTTPS=on`; behind a proxy the app learns it
/// from `X-Forwarded-Proto` (or `X-Forwarded-Ssl: on`), as Rack's `ssl?` does.
#[test]
fn test_over_https_the_browser_is_told_to_stay_there() {
    let mut s = Session::new();
    s.get_with("/", &[("x-forwarded-proto", "https")]);

    assert_eq!(
        "max-age=31536000; includeSubDomains",
        header(&s, "strict-transport-security")
    );
    s.get_with("/", &[("x-forwarded-ssl", "on")]);

    assert_eq!(
        "max-age=31536000; includeSubDomains",
        header(&s, "strict-transport-security")
    );
    s.get("/");

    assert_eq!(None, s.last().header("strict-transport-security"));
}

#[test]
fn test_a_request_larger_than_any_plate_is_refused_before_it_is_read() {
    let mut s = Session::new();
    let length = (form::MAX_SVG_BYTES + 65 * 1024).to_string();
    s.request(
        "POST",
        "/decode",
        &[("content-length", &length)],
        Some(("application/x-www-form-urlencoded".to_string(), Vec::new())),
    );

    assert_eq!(413, s.status());
    assert_includes(s.body(), "larger than any plate", "refused");
}

/// Behind the front door (FRONT_DOOR_KEY set), only a request through it is
/// answered: the key in its header; the probe's /up open
#[test]
fn test_behind_the_front_door_only_a_request_with_the_key_is_answered() {
    let mut session = Session::with(Config {
        front_door_key: Some("abbey-of-molesme".to_string()),
        ..Config::default()
    });
    session.get("/");

    assert_eq!(403, session.status());
    session.get("/up");

    ok(&session, "/up");
    session.get_with("/", &[("x-front-door-key", "abbey-of-molesme")]);

    ok(&session, "the key");
    session.get_with("/", &[("x-front-door-key", "abbey-of-molesmf")]);

    assert_eq!(403, session.status());
}

#[test]
fn test_without_a_key_every_door_is_open() {
    let mut open = Session::with(Config {
        front_door_key: Some(String::new()),
        ..Config::default()
    });
    open.get("/");

    ok(&open, "/");
}

#[test]
fn test_results_carrying_messages_are_never_cached() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "hello"),
            ("mode", "secret"),
            ("passphrase", "abbey of molesme"),
        ],
    );

    assert_includes(&header(&s, "cache-control"), "no-store", "cache");
}

/// The port's one sanctioned deviation (docs/web-port-spec.md §15): the Ruby
/// app lets a shared cache keep `POST /decode` (`public, max-age=300`, the
/// leaf's rule by path); its answer carries the decoded message, so here
/// every POST goes out `no-store`.
#[test]
fn test_a_decoded_message_is_never_cached() {
    let mut s = Session::new();
    s.post(
        "/decode",
        &[
            ("svg", &to_svg_encoded("Meet me at dawn!", None)),
            ("decipher", "1"),
        ],
    );

    ok(&s, "decoded");
    assert_includes(s.body(), "meet me at dawn", "decoded");
    assert_eq!("no-store", header(&s, "cache-control"));
}

#[test]
fn test_static_assets_are_served() {
    let mut s = Session::new();
    for path in [
        "/css/app.css",
        "/js/app.js",
        "/favicon.svg",
        "/fonts/eb-garamond-latin-400-normal.woff2",
    ] {
        s.get(path);

        ok(&s, path);
    }
}

#[test]
fn test_key_illustrations_are_the_real_glyph_geometry_pressed() {
    let mut s = Session::new();
    s.get("/");

    for stroke in glyphs::strokes(1098, 0, 0) {
        assert_includes(
            s.body(),
            &format!(r#"<path d="{}"/>"#, stylus::press(stroke)),
            "pressed",
        );
    }
}

#[test]
fn test_game_mode_ignores_a_stray_passphrase() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Agent 007"),
            ("mode", "game"),
            ("passphrase", "abbey of molesme"),
        ],
    );

    assert_includes(s.body(), "Enciphered as: <code>agent</code>", "game");
}

#[test]
fn test_secret_mode_requires_a_passphrase() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "hello"), ("mode", "secret"), ("passphrase", "")],
    );

    assert_eq!(422, s.status()); // back to the form, not an error page
    assert_match(
        r#"(?s)id="passphrase-error"[^>]*>Secret mode needs a passphrase</p>"#,
        s.body(),
        "the error",
    );
}

/// Ruby checked Sinatra's `show_exceptions` setting (`:after_handler`): its
/// backtrace page never answers a reader's mistake. There is no backtrace
/// page here to switch off; the observable intent is that a domain error is
/// the app's Erratum, in the app's frame, never the error's internals.
#[test]
fn test_domain_errors_never_show_the_backtrace_page() {
    let mut s = Session::new();
    s.post("/decode", &[("svg", "<svg")]);

    assert_eq!(400, s.status());
    assert_includes(
        s.body(),
        r#"<h1 id="erratum-title" class="rubric">Erratum</h1>"#,
        "erratum",
    );
    assert_includes(s.body(), "<!DOCTYPE html>", "in the frame");
    refute_includes(s.body(), "backtrace", "no backtrace");
    refute_includes(s.body(), ".rs:", "no source");
}

#[test]
fn test_result_page_can_read_its_plate_back() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "Ora et labora")]);
    // the scribe knows the answer
    assert_includes(
        s.body(),
        r#"<input type="hidden" name="decipher" value="1">"#,
        "decipher",
    );
    let svg = hidden_svg(s.body());

    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);

    assert_includes(s.body(), "ora et labora", "read back");
}

#[test]
fn test_large_plates_get_optical_weight() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "Ora et labora")]);
    assert_includes(
        s.body(),
        r#"<figure class="plate">"#,
        "6 x 6: the hand as it is",
    );

    s.post(
        "/generate",
        &[(
            "message",
            "Meet me at the old abbey after vespers, under the cloister arches by the well",
        )],
    );
    assert_includes(
        s.body(),
        r#"<figure class="plate plate--large">"#,
        "14 x 14: strokes, not hairlines",
    );
}

#[test]
fn test_live_preview_embeds_the_library_glyph_table() {
    let mut s = Session::new();
    s.get("/");

    let table = json_attr(s.body(), "data-glyph-table");
    assert_eq!(json!(glyphs::STEM), table["stem"]);
    let mut lines = serde_json::Map::new();
    let mut strokes = vec![glyphs::STEM];
    for place in glyphs::PLACES {
        for digit in 1..=9 {
            let value = digit * place;
            let drawn = glyphs::lines_of(value).expect("a place value");
            lines.insert(value.to_string(), json!(drawn));
            strokes.extend_from_slice(drawn);
        }
    }
    assert_eq!(Json::Object(lines), table["lines"]);
    let wedges: serde_json::Map<String, Json> = strokes
        .iter()
        .map(|stroke| {
            let key = stroke
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            (key, json!(stylus::press(*stroke)))
        })
        .collect();
    assert_eq!(Json::Object(wedges), table["wedges"]);
    assert_eq!(9000, int(&table["tablet"][" "]));
    assert_eq!(10, int(&table["tablet"]["a"]));
}

#[test]
fn test_about_page() {
    let mut s = Session::new();
    s.get("/about");

    ok(&s, "/about");
    let body = s.body();
    assert_includes(
        body,
        "<title>About the ciphers · Cistergraphia</title>",
        "title",
    );
    assert_includes(
        body,
        r#"href="/about" class="is-current" aria-current="page""#,
        "nav",
    );
    assert_includes(body, "data-playground", "playground");
    let tablet = capture(body, r#"(?s)<ol class="tablet">(.*?)</ol>"#).expect("the tablet");
    assert_eq!(rustergraphia::TABLET.len(), count(&tablet, "<li>"));
    assert_includes(body, "John of Basingstoke", "history");
}

#[test]
fn test_silence_is_the_default_and_the_colophon_offers_the_hand() {
    let mut s = Session::new();
    s.get("/");
    assert_includes(
        s.body(),
        r#"<script src="/js/sound.js" defer></script>"#,
        "sound.js",
    );
    let control = r#"<button type="button" class="hear awaits-script" data-hear aria-pressed="false">Hear the hand</button>"#;
    assert_includes(s.body(), control, "the control");

    s.get("/about");
    assert_includes(
        s.body(),
        "no sound unless you ask to hear the hand",
        "about",
    );
}

#[test]
fn test_pages_carry_social_preview_metadata() {
    let mut s = Session::new();
    s.get("/about");

    assert_includes(
        s.body(),
        r#"<meta property="og:image" content="http://example.org/og.png">"#,
        "og:image",
    );
    assert_includes(
        s.body(),
        r#"<meta property="og:title" content="About the ciphers">"#,
        "og:title",
    );
    assert_includes(
        s.body(),
        r#"<meta name="twitter:card" content="summary_large_image">"#,
        "twitter",
    );

    s.get("/og.png");

    ok(&s, "/og.png");
    assert_eq!("image/png", s.last().content_type());
}

#[test]
fn test_plate_can_be_walked_in_reading_order() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "Ora et labora")]);
    let (cells, layout, svg) = plate_of(s.body());

    // values, in reading order
    let read: Vec<Option<Value>> = rustergraphia::svg::read(&svg, &Layout::Phaistos, None)
        .expect("read")
        .into_iter()
        .map(Some)
        .collect();
    assert_eq!(read, values_of(&cells));
    assert_eq!(
        layout::spiral_coordinates(size_of(&layout)),
        cells.iter().map(row_col).collect::<Vec<_>>()
    );
    assert!(cells.iter().all(|cell| !cell[3].as_bool().unwrap_or(false)));
    assert_eq!(
        json!({ "padding": 50, "cell": [120, 170], "offset": -10, "grille": false }),
        except_size(&layout)
    );
    assert_includes(s.body(), r#"tabindex="0""#, "focusable");
}

#[test]
fn test_inscribe_puts_the_choices_first_and_reserves_its_dynamic_parts() {
    let mut s = Session::new();
    s.get("/");
    let body = s.body();
    let order = [
        r#"name="mode""#,
        r#"name="layout""#,
        r#"id="message""#,
        "data-preview",
        r#"id="passphrase""#,
        "data-submit",
    ];

    // choices above what you type
    let at: Vec<usize> = order
        .iter()
        .map(|marker| body.find(marker).unwrap_or_else(|| panic!("{marker}")))
        .collect();
    let mut sorted = at.clone();
    sorted.sort();
    assert_eq!(sorted, at);
    // laid out from the first paint
    refute_match(
        r#"<div class="preview[^"]*" data-preview[^>]* hidden[ >]"#,
        body,
        "preview",
    );
    assert_includes(
        body,
        "preview-note--secret",
        "the sealed state shares the strip",
    );
    assert_includes(
        body,
        r#"class="slot secret-slot""#,
        "unfolded by CSS, not by JS",
    );
    assert_includes(body, r#"class="slot grille-slot""#, "grille slot");
    assert_match("anatomy-hint awaits-script", body, "space kept until JS");
}

#[test]
fn test_the_readers_puzzle_is_one_disclosure_shut_on_a_plain_plate_open_when_chosen_or_refused() {
    let mut s = Session::new();
    s.get("/");

    assert_match(
        r#"<details class="puzzle" data-puzzle>"#,
        s.body(),
        "shut on a plain plate",
    );
    assert_includes(s.body(), "Intact, shared tablet, path shown", "summary");
    assert!(
        s.body().find(r#"id="message""#).expect("message")
            < s.body().find(r#"name="condition""#).expect("condition")
    );
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("mode", "secret"),
            ("condition", "worn"),
            ("passphrase", "abbey of molesme"),
        ],
    );

    assert_match(
        r#"<details class="puzzle" data-puzzle open>"#,
        s.body(),
        "open on its refusal",
    );
    assert_includes(s.body(), "Worn, shared tablet, path shown", "summary");
}

#[test]
fn test_a_plate_of_many_signs_can_be_zoomed_a_small_one_need_not() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[(
            "message",
            "the brothers will gather in the chapter house at dawn",
        )],
    );

    assert_includes(s.body(), "data-plate-zoom", "zoom");
    assert_includes(s.body(), r#"aria-label="Zoom in""#, "zoom in");
    s.post("/generate", &[("message", "Ora")]);

    refute_includes(
        s.body(),
        "data-plate-zoom",
        "a 4 × 4 plate: its signs are large enough",
    );
}

#[test]
fn test_index_offers_six_layouts() {
    let mut s = Session::new();
    s.get("/");

    assert_match(
        r#"name="layout" value="phaistos" checked"#,
        s.body(),
        "phaistos",
    );
    assert_match(
        r#"name="layout" value="rongorongo""#,
        s.body(),
        "rongorongo",
    );
    assert_match(r#"name="layout" value="grille""#, s.body(), "grille");
    for way in ["leap", "furrows", "unwinding"] {
        assert_match(&format!(r#"name="layout" value="{way}""#), s.body(), way);
    }
    assert_eq!(6, count(s.body(), "data-layout-note=")); // the key follows the choice
    assert_eq!(6, count(s.body(), r#"class="segment-figure""#)); // each way shows its order
}

#[test]
fn test_index_offers_the_tablet_shared_or_its_own() {
    let mut s = Session::new();
    s.get("/");

    assert_match(
        r#"name="tablet" value="shared" checked"#,
        s.body(),
        "shared",
    );
    assert_match(r#"name="tablet" value="own""#, s.body(), "own");
    assert_includes(s.body(), r#"data-note="tablet-short""#, "note");
}

#[test]
fn test_a_plate_with_its_own_tablet_from_the_form_shows_the_scribe_the_tablet() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Meet me at the old abbey after vespers"),
            ("layout", "leap"),
            ("tablet", "own"),
        ],
    );
    let body = s.body().to_string();
    let (_cells, _layout, svg) = plate_of(&body);
    let seed = tablet::seed_of(&svg);

    ok(&s, "own tablet");
    let seed = seed.expect("a seed");
    assert_includes(
        &body,
        &format!(
            r#"<dd>Its own <code class="grille-key">{}</code></dd>"#,
            tablet::show(&seed).expect("shown")
        ),
        "the seed",
    );
    assert_eq!(27, count(&body, r#"<span class="own-tablet-value">"#));
    assert_includes(
        &body,
        "A tablet of its own: the values dealt out again",
        "note",
    );
    assert_eq!("meet me at the old abbey after vespers", decode(&svg));
}

#[test]
fn test_a_plate_cannot_have_its_own_tablet_sealed_under_a_grille_or_damaged() {
    let mut s = Session::new();
    let cases: [(&[(&str, &str)], &str); 4] = [
        (
            &[("mode", "secret"), ("passphrase", "abbey of molesme")],
            tablet::NOT_SEALED,
        ),
        (&[("layout", "grille")], tablet::NOT_UNDER_A_GRILLE),
        (&[("condition", "worn")], tablet::NOT_DAMAGED),
        (&[("condition", "broken")], tablet::NOT_DAMAGED),
    ];
    for (fields, reason) in cases {
        let mut form = vec![("message", "Ora et labora"), ("tablet", "own")];
        form.extend_from_slice(fields);
        s.post("/generate", &form);

        assert_eq!(422, s.status(), "{fields:?}");
        assert_includes(s.body(), reason, &format!("{fields:?}"));
        assert_match(r#"name="tablet" value="own" checked"#, s.body(), "own kept");
    }
}

#[test]
fn test_about_tells_of_the_tablet_of_its_own_and_the_copiale_cipher() {
    let mut s = Session::new();
    s.get("/about");

    assert_includes(s.body(), "a tablet of its own", "tablet");
    assert_includes(s.body(), "aclanthology.org/W11-1202", "copiale");
}

#[test]
fn test_index_offers_the_path_shown_or_withheld() {
    let mut s = Session::new();
    s.get("/");

    assert_match(r#"name="path" value="shown" checked"#, s.body(), "shown");
    assert_match(r#"name="path" value="withheld""#, s.body(), "withheld");
    assert_includes(s.body(), "data-path-withheld", "note");
}

#[test]
fn test_a_pathless_plate_is_written_in_grid_order_and_says_so() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("layout", "leap"),
            ("path", "withheld"),
        ],
    );
    let (_cells, _layout, svg) = plate_of(s.body());

    assert_includes(
        s.body(),
        "<dd>Withheld: the reader finds it</dd>",
        "withheld",
    );
    assert!(pathless::is_pathless(&svg));
    assert_eq!("ora et labora", decode(&svg));
}

#[test]
fn test_a_plate_cannot_be_pathless_sealed_under_a_grille_or_broken() {
    let mut s = Session::new();
    let cases: [(&[(&str, &str)], &str); 4] = [
        (
            &[("mode", "secret"), ("passphrase", "abbey of molesme")],
            pathless::NOT_SEALED,
        ),
        (&[("layout", "grille")], pathless::NOT_UNDER_A_GRILLE),
        (&[("condition", "broken")], pathless::NOT_BROKEN),
        (&[("condition", "ruined")], pathless::NOT_BROKEN),
    ];
    for (fields, reason) in cases {
        let mut form = vec![("message", "Ora et labora"), ("path", "withheld")];
        form.extend_from_slice(fields);
        s.post("/generate", &form);

        assert_eq!(422, s.status(), "{fields:?}");
        assert_includes(s.body(), reason, &format!("{fields:?}"));
        assert_match(
            r#"name="path" value="withheld" checked"#,
            s.body(),
            "withheld kept",
        );
    }
}

#[test]
fn test_a_pathless_plate_brought_to_read_opens_at_a_desk_without_its_way() {
    let (_, svg) = inscribe("Ora et labora", &Layout::Leap, true, None);
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg)]);
    let body = s.body().to_string();
    let cells = cells_of(&body);

    for said in [
        r#"data-play="pathless""#,
        "data-desk-pathless",
        "Which way does it run?",
    ] {
        assert_includes(&body, said, said);
    }
    assert_eq!(5, count(&body, "data-desk-way="));
    assert!(
        in_grid_order(&cells),
        "the cells go to the page in grid order, not along the way"
    );
    assert_pathless_grid(&body, &cells);
    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);

    assert_includes(s.body(), "ora et labora", "the decoder still answers");
}

#[test]
fn test_a_worn_pathless_plate_brought_to_read_opens_at_a_bench_without_its_way() {
    let (sequence, svg, damage) = worn_pathless_plate();
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg)]);
    let body = s.body().to_string();

    for said in [
        r#"data-play="pathless""#,
        "data-bench-pathless",
        "Which way does it run?",
        "Choose a way to restore",
        "data-bench-notebook",
        "<dd>Worn, pathless</dd>",
        "<dd>Withheld: one of the five ways</dd>",
    ] {
        assert_includes(&body, said, said);
    }
    assert_eq!(5, count(&body, "data-bench-way="));
    assert_eq!(
        json!([]),
        json_attr(&body, "data-lacunae"),
        "no lacuna is placed along a way"
    );
    assert_match(r"data-bench-count>0 of \d+ restored<", &body, "count");
    assert_worn_cells_show_only_the_file(&body, &sequence, &damage);
}

#[test]
fn test_a_worn_pathless_plate_is_still_read_by_the_decoder() {
    let (_, svg, _) = worn_pathless_plate();
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);

    assert_match(r"\[[a-z /]+\]", s.body(), "the decoder prints the edition");
}

fn worn_pathless_plate() -> (Vec<Value>, String, Damage) {
    worn(
        "the brothers will gather in the chapter house",
        &Layout::Furrows,
        false,
        true,
        &mut RubyRandom::new(3),
    )
}

/// The worn cells go to the page in grid order, each with what it may hold
/// either way up, never with a place along the way; the cells too
#[track_caller]
fn assert_worn_cells_show_only_the_file(body: &str, sequence: &[Value], damage: &Damage) {
    let worn = json_attr(body, "data-bench-worn");
    let worn = worn.as_array().expect("a list");
    let path = layout::path(sequence.len().isqrt(), &Layout::Furrows).expect("a path");

    let mut expected: Vec<(i32, i32)> = damage
        .keys()
        .map(|&index| (path[index].0, path[index].1))
        .collect();
    expected.sort();
    let shown: Vec<(i32, i32)> = worn
        .iter()
        .map(|cell| (int(&cell["row"]) as i32, int(&cell["col"]) as i32))
        .collect();
    assert_eq!(expected, shown);
    for cell in worn {
        assert_worn_cell_shows_only_the_file(cell);
    }
    let cells = cells_of(body);

    assert!(
        in_grid_order(&cells),
        "the cells go to the page in grid order, not along the way"
    );
    assert_eq!(
        damage.len(),
        cells.iter().filter(|cell| cell[2].is_null()).count(),
        "worn signs are not valued"
    );
}

#[track_caller]
fn assert_worn_cell_shows_only_the_file(cell: &Json) {
    assert!(cell.get("sign").is_none());
    let readings: Vec<&Json> = ["upright", "turned"]
        .iter()
        .filter_map(|side| cell.get(*side).filter(|reading| !reading.is_null()))
        .collect();

    assert!(
        !readings.is_empty(),
        "a worn cell reads as a sign at least one way up"
    );
    for reading in readings {
        for entry in reading.as_array().expect("a list") {
            assert_lacuna_shows_only_the_file(entry);
        }
    }
}

#[test]
fn test_a_plate_with_its_own_tablet_brought_to_read_opens_at_the_desk_with_its_notebook() {
    let (sequence, svg) = inscribe(
        "Meet me at the old abbey",
        &Layout::Furrows,
        false,
        Some(GRILLE_KEY),
    );
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg)]);
    let body = s.body().to_string();

    for said in [
        r#"data-play="tablet""#,
        "data-desk-own",
        r#"data-tablet="{}""#,
        "data-notebook",
        "Ask the scribe",
        "<dd>Its own: to build</dd>",
    ] {
        assert_includes(&body, said, said);
    }
    assert_eq!(
        notebook::entries(&sequence).len(),
        count(&body, "data-entry-row=")
    );
    refute_includes(&body, "tablet-value", "the reader is not shown the tablet");
    assert_eq!(
        Some("K7Q2MZXB4P".to_string()),
        tablet::seed_of(&unescape_html(
            &capture(&body, r#"name="svg" value="([^"]*)""#).expect("the file")
        )),
        "the file goes back to the decoder and the scribe with its seed"
    );
    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);

    assert_includes(
        s.body(),
        "meet me at the old abbey",
        "the decoder reads by the seed",
    );
}

/// `{ sum => tablet[sum] }` of a plate's notebook entries, in their order.
fn right_answers(sequence: &[Value]) -> Vec<(Value, char)> {
    let tablet = tablet::characters(GRILLE_KEY).expect("a tablet");
    notebook::entries(sequence)
        .iter()
        .map(|entry| (entry.sum, tablet[&entry.sum]))
        .collect()
}

fn pencil_json(pencil: &[(Value, char)]) -> Json {
    Json::Object(
        pencil
            .iter()
            .map(|(sum, c)| (sum.to_string(), json!(c.to_string())))
            .collect(),
    )
}

#[test]
fn test_the_scribe_answers_the_notebook_three_at_a_time() {
    let (sequence, svg) = inscribe(
        "Meet me at the old abbey",
        &Layout::Phaistos,
        false,
        Some(GRILLE_KEY),
    );
    let right = right_answers(&sequence);
    let mut s = Session::new();

    assert_eq!(json!({}), ask_the_scribe(&mut s, &svg, &right[..2]));
    assert_eq!(
        pencil_json(&right[..3]),
        ask_the_scribe(&mut s, &svg, &right[..3])
    );
    assert_eq!("application/json", s.last().media_type());
    s.post(
        "/ask",
        &[("svg", &svg), ("pencil", "not json"), ("inked", "[]")],
    );

    assert_eq!(400, s.status());
}

/// What the scribe holds of the pencilled entries ({ sum => character })
fn ask_the_scribe(s: &mut Session, svg: &str, pencil: &[(Value, char)]) -> Json {
    let pencil = pencil_json(pencil).to_string();
    s.post(
        "/ask",
        &[("svg", svg), ("pencil", &pencil), ("inked", "[]")],
    );
    let answer: Json = serde_json::from_str(s.body()).expect("JSON");
    answer["held"].clone()
}

#[test]
fn test_the_expert_plate_is_written_from_the_form_pathless_with_its_own_tablet() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "The relics lie under the third stone"),
            ("path", "withheld"),
            ("tablet", "own"),
        ],
    );
    let body = s.body().to_string();
    let svg = unescape_html(&capture(&body, r#"name="svg" value="([^"]*)""#).expect("the file"));

    assert_eq!(200, s.status());
    assert_includes(
        &body,
        "The expert plate: pathless, with a tablet of its own.",
        "expert",
    );
    assert!(pathless::is_pathless(&svg));
    assert!(tablet::seed_of(&svg).is_some());
    s.get("/");

    assert_includes(s.body(), r#"data-note="tablet-expert""#, "note");
}

const EXPERT_PAGE: [&str; 8] = [
    r#"data-play="expert""#,
    "data-desk-pathless",
    "data-desk-own",
    r#"data-tablet="{}""#,
    "data-notebook-shut",
    r#"data-notebook-sums="[]""#,
    "<dd>Its own: to build</dd>",
    "Which way does it run?",
];

/// The expert plate's page tells neither the way nor the tablet: the desk asks
/// which way, the notebook waits shut, with no sum the way makes in it
#[test]
fn test_the_expert_plate_brought_to_read_opens_at_the_desk_its_notebook_shut() {
    let (sequence, svg) = inscribe(
        "The relics lie under the third stone",
        &Layout::Leap,
        true,
        Some(GRILLE_KEY),
    );
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();

    for said in EXPERT_PAGE {
        assert_includes(body, said, said);
    }
    for entry in notebook::entries(&sequence) {
        refute_includes(
            body,
            &format!(r#"data-entry="{}""#, entry.sum),
            "the notebook lists no sum of the way before it is found",
        );
    }
    let values: Json =
        serde_json::from_str(&capture(body, r#"data-values="([^"]*)""#).expect("the values"))
            .expect("JSON");
    assert_eq!(
        json!(characters().keys().collect::<Vec<_>>()),
        values,
        "the desk holds the values every tablet deals out, and no letter"
    );
}

#[test]
fn test_the_scribe_reads_the_expert_plate_along_its_way() {
    let (sequence, svg) = inscribe(
        "The relics lie under the third stone",
        &Layout::Leap,
        true,
        Some(GRILLE_KEY),
    );
    let right = right_answers(&sequence);
    let mut s = Session::new();

    assert_eq!(
        pencil_json(&right[..3]),
        ask_the_scribe(&mut s, &svg, &right[..3])
    );
}

#[test]
fn test_a_worn_plate_still_keeps_the_shared_tablet() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("condition", "worn"),
            ("path", "withheld"),
            ("tablet", "own"),
        ],
    );

    assert_eq!(422, s.status());
    assert_includes(s.body(), tablet::NOT_DAMAGED, "refused");
}

#[test]
fn test_a_worn_plate_can_be_pathless_from_the_form() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("condition", "worn"),
            ("path", "withheld"),
        ],
    );
    let (_cells, _layout, svg) = plate_of(s.body());

    ok(&s, "worn pathless");
    assert_includes(
        s.body(),
        "<dd>Withheld: the reader finds it</dd>",
        "withheld",
    );
    assert!(pathless::is_pathless(&svg));
    assert!(lacunae::is_weathered(&svg).expect("read"));
}

/// The grid beside the cells: the same cells, both readings; the way unnamed
#[track_caller]
fn assert_pathless_grid(body: &str, cells: &[Json]) {
    let grid = json_attr(body, "data-plate-grid");
    let upright = |list: &[Json]| -> Vec<(Json, Json, Json)> {
        list.iter()
            .map(|cell| (cell[0].clone(), cell[1].clone(), cell[2].clone()))
            .collect()
    };

    assert_eq!(upright(cells), upright(grid.as_array().expect("a list")));
    assert_includes(
        body,
        "<dd>Withheld: one of the five ways</dd>",
        "the page names the five, not the one",
    );
}

#[test]
fn test_a_plate_along_each_new_way_is_walked_along_it_and_read_back() {
    let mut s = Session::new();
    for (way, layout) in [
        ("leap", Layout::Leap),
        ("furrows", Layout::Furrows),
        ("unwinding", Layout::Unwinding),
    ] {
        s.post(
            "/generate",
            &[("message", "Ora et labora"), ("layout", way)],
        );
        let (cells, plate_layout, svg) = plate_of(s.body());

        assert_eq!(
            layout::path(size_of(&plate_layout), &layout).expect("a path"),
            walked(&cells)
        );
        assert_eq!("ora et labora", decode(&svg), "{way}");
        let mut name = way.to_string();
        name[..1].make_ascii_uppercase();
        assert_includes(s.body(), &format!("<dd>{name}</dd>"), way);
    }
}

#[test]
fn test_rongorongo_plate_is_walked_with_its_turned_rows() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "Ora et labora"), ("layout", "rongorongo")],
    );
    let (cells, plate_layout, svg) = plate_of(s.body());

    assert_includes(s.body(), "<dd>Rongorongo</dd>", "layout");
    assert_includes(s.body(), r#"data-action="turn-plate""#, "turn");
    let read: Vec<Option<Value>> = rustergraphia::svg::read(&svg, &Layout::Rongorongo, None)
        .expect("read")
        .into_iter()
        .map(Some)
        .collect();
    assert_eq!(read, values_of(&cells));
    assert_eq!(
        layout::boustrophedon_coordinates(size_of(&plate_layout)),
        walked(&cells)
    );

    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);

    assert_includes(s.body(), "ora et labora", "read back");
}

#[test]
fn test_plate_keeps_its_caption_and_controls_in_place() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "abbey"), ("layout", "rongorongo")],
    );
    let body = s.body();

    assert_match(
        "plate-reading-rest",
        body,
        "the resting caption keeps its place...",
    );
    assert_match(
        r#"data-plate-reading aria-live="polite"></span>"#,
        body,
        "...while the live reading fills it",
    );
    assert_match("plate-hint awaits-script", body, "hint");
    assert_match(
        r#"btn awaits-script" data-action="turn-plate""#,
        body,
        "turn",
    );
    refute_match(r"<(?:span|button)[^>]* hidden[ >]", body, "hidden");
}

#[test]
fn test_unknown_layouts_fall_back_to_the_snail() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "abbey"), ("layout", "<script>")]);

    ok(&s, "fallback");
    assert_includes(s.body(), "<dd>Phaistos</dd>", "the snail");
    refute_includes(s.body(), "turn-plate", "no turning");
}

#[test]
fn test_anatomy_corners_are_dials_enabled_by_script() {
    let mut s = Session::new();
    s.get("/");

    let dials = scan_groups(
        s.body(),
        r#"<button type="button" class="quadrant quadrant--(\w+)" data-place="(\d+)" disabled>"#,
    );
    assert_eq!(
        vec![
            vec!["tens", "10"],
            vec!["units", "1"],
            vec!["thousands", "1000"],
            vec!["hundreds", "100"]
        ],
        dials
    );
}

#[test]
fn test_grille_plate_shows_its_key_and_its_stencil() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("layout", "grille"),
            ("grille", "k7q2m zxb4p"),
        ],
    );
    let body = s.body();

    assert_includes(body, "<dd>Grille</dd>", "layout");
    assert_includes(
        body,
        r#"<code class="grille-key">K7Q2M-ZXB4P</code>"#,
        "key",
    );
    assert_includes(body, r#"data-action="lay-grille""#, "lay");
    assert_includes(body, r#"<template data-stencil="whole""#, "stencil");
    // read it back with its grille
    assert_includes(
        body,
        r#"<input type="hidden" name="grille" value="K7Q2M-ZXB4P">"#,
        "carried",
    );
}

#[test]
fn test_grille_plate_is_walked_through_the_grille() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("layout", "grille"),
            ("grille", "k7q2m zxb4p"),
        ],
    );
    let (cells, plate_layout, svg) = plate_of(s.body());
    let grille = the_grille();

    assert_eq!(
        grille.path(size_of(&plate_layout)).expect("a path"),
        cells.iter().map(row_col).collect::<Vec<_>>()
    );
    assert_eq!(
        json!({ "padding": 50, "cell": [170, 170], "offset": 0, "grille": true }),
        except_size(&plate_layout)
    );
    assert_eq!(
        "ora et labora",
        cipher::decode(&svg, None, Some(&Layout::Grille(grille))).expect("read")
    );
}

#[test]
fn test_a_grille_is_drawn_for_each_plate_unless_its_key_is_given() {
    let mut s = Session::new();
    let keys: Vec<String> = (0..2)
        .map(|_| {
            s.post(
                "/generate",
                &[("message", "Ora et labora"), ("layout", "grille")],
            );
            capture(s.body(), r#"<code class="grille-key">([^<]+)</code>"#).expect("a key")
        })
        .collect();

    assert!(keys.iter().all(|key| is_match(key, r"\A\w{5}-\w{5}\z")));
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn test_a_malformed_grille_key_returns_to_the_form_with_what_was_written() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Ora et labora"),
            ("layout", "grille"),
            ("grille", "abbey"),
        ],
    );

    assert_eq!(422, s.status());
    let body = s.body();
    assert_match(
        r#"id="grille-error"[^>]*>A grille key has 10 letters and digits"#,
        body,
        "error",
    );
    assert_match(
        r#"name="layout" value="grille" checked"#,
        body,
        "grille kept",
    );
    refute_match(
        r#"name="layout" value="phaistos" checked"#,
        body,
        "not the snail",
    );
    assert_includes(body, r#"value="abbey""#, "key kept");
    assert_includes(body, "Ora et labora", "message kept");
}

/// A grille plate whose splits are drawn from a fixed seed: under a wrong key,
/// one plate of these in 500 reads as letters by chance (measured), and the key is not refused
fn seeded_grille_plate() -> String {
    cipher::inscribe(
        "Vespers at six",
        None,
        &Layout::Grille(the_grille()),
        false,
        false,
        None,
        &mut RubyRandom::new(3),
    )
    .expect("inscribed")
    .1
}

#[test]
fn test_a_grille_plate_is_read_with_its_key_and_asks_for_it_without() {
    let svg = seeded_grille_plate();
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    assert_eq!(400, s.status());
    assert_includes(
        s.body(),
        "This plate was written under a grille: it needs its key.",
        "asks",
    );

    s.post(
        "/decode",
        &[("svg", &svg), ("grille", "k7q2m-zxb4p"), ("decipher", "1")],
    );
    ok(&s, "with its key");
    assert_includes(s.body(), "vespers at six", "read");

    s.post("/decode", &[("svg", &svg), ("grille", "00000-00000")]);
    assert_includes(s.body(), grille::WRONG_KEY, "wrong key");
    // the key is the problem here, not a passphrase
    refute_includes(s.body(), "secret mode", "no passphrase hint");
}

#[test]
fn test_a_plate_for_two_readers_shows_both_keys_and_both_stencils() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Relics lie under the third pillar"),
            ("decoy", "Vespers at six"),
            ("layout", "grille"),
            ("grille", GRILLE_KEY),
        ],
    );
    let body = s.body();
    let novice = the_grille().novice();

    assert_includes(body, "<dd>Grille, two readers</dd>", "layout");
    assert_includes(
        body,
        r#"<code class="grille-key">K7Q2M-ZXB4P</code>"#,
        "abbot",
    );
    assert_includes(
        body,
        &format!(r#"<code class="grille-key">{novice}</code>"#),
        "novice",
    );
    assert_includes(body, r#"<template data-stencil="abbot""#, "abbot stencil");
    assert_includes(body, r#"<template data-stencil="novice""#, "novice stencil");
    assert_match(r#"(?s)class="decoy-echo".*Vespers at six"#, body, "decoy");
    // the abbot reads it back
    assert_includes(
        body,
        r#"<input type="hidden" name="grille" value="K7Q2M-ZXB4P">"#,
        "carried",
    );
}

#[test]
fn test_the_abbot_reads_both_messages_and_the_novice_one() {
    let (_, svg, abbot) = two_readers::inscribe(
        "Relics lie here",
        "Vespers at six",
        None,
        Some(&the_grille()),
    )
    .expect("inscribed");
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("grille", &abbot.to_string())]);
    assert_includes(s.body(), "relics lie here", "the abbot's");
    assert_match(
        r"(?s)The novice’s grille reads.*vespers at six",
        s.body(),
        "the novice's",
    );

    s.post(
        "/decode",
        &[("svg", &svg), ("grille", &abbot.novice().to_string())],
    );
    assert_includes(s.body(), "vespers at six", "the novice's");
    refute_includes(s.body(), "relics", "not the abbot's");
}

#[test]
fn test_too_long_a_pair_returns_to_the_form() {
    let mut s = Session::new();
    let message = "x".repeat(300);
    s.post(
        "/generate",
        &[
            ("message", &message),
            ("decoy", "Vespers"),
            ("layout", "grille"),
        ],
    );

    assert_eq!(422, s.status());
    assert_match(
        r#"id="decoy-error"[^>]*>With two readers, each message is limited to 250 characters"#,
        s.body(),
        "error",
    );
}

#[test]
fn test_largest_sealed_pair_fits_the_upload_limit() {
    let message = "🙏".repeat(form::MAX_PAIR_LENGTH);
    let (_, svg, _) = two_readers::inscribe(&message, &message, Some("abbey of molesme"), None)
        .expect("inscribed");

    assert!(svg.len() < form::MAX_SVG_BYTES);
}

#[test]
fn test_largest_sealed_grille_plate_fits_the_upload_limit() {
    let message = "🙏".repeat(form::MAX_MESSAGE_LENGTH);
    let layout = Layout::Grille(the_grille());
    let sequence = cipher::encode(
        &message,
        Some("abbey of molesme"),
        true,
        None,
        &mut random(),
    )
    .expect("encoded");
    let svg = cipher::to_svg(&sequence, &layout, false, false, None).expect("drawn");

    assert!(svg.len() < form::MAX_SVG_BYTES);
}

#[test]
fn test_about_page_explains_the_six_layouts_and_their_origins() {
    let mut s = Session::new();
    s.get("/about");

    let body = s.body();
    for said in [
        "Six ways to read a plate",
        "Leap: the Zodiac’s step",
        "https://arxiv.org/abs/2403.17350",
    ] {
        assert_includes(body, said, said);
    }
    assert_includes(body, "reverse boustrophedon", "rongorongo");
    for page in ["Phaistos_Disc", "Rongorongo", "Cardan_grille"] {
        assert_includes(body, &format!("https://en.wikipedia.org/wiki/{page}"), page);
    }
    assert_eq!(3, count(body, r#"<svg class="turning""#)); // the disc, the tablet, Cardano's letter
    assert_eq!(5, count(body, r#"<svg class="snail""#)); // and each keyless way drawn on the grid
    assert_eq!(1, count(body, r#"<svg class="grille-order""#));
    assert_includes(body, r#"id="layouts""#, "linked from the inscribe key");
    refute_match(
        r"(?i)why a snail|began with|the default",
        body,
        "neither layout comes first",
    );
}

#[test]
fn test_about_page_offers_lectio() {
    let mut s = Session::new();
    s.get("/about");

    let body = s.body();
    assert_match(
        r#"<section class="lectio" data-lectio aria-labelledby"#,
        body,
        "laid out from the start",
    );
    assert_match(
        r#"data-lectio-sign data-number="[1-9]"><svg class="glyph""#,
        body,
        "a sign",
    );
    assert_match("data-lectio-answer disabled>", body, "enabled by JS");
    assert_match("data-lectio-submit disabled>", body, "enabled by JS");
}

#[test]
fn test_unknown_pages_render_the_erratum() {
    let mut s = Session::new();
    s.get("/nowhere");

    assert_eq!(404, s.status());
    assert_includes(s.body(), "not in the index", "erratum");
}

#[test]
fn test_generate_renders_an_inline_svg() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "hello world")]);

    ok(&s, "generate");
    assert_includes(
        s.body(),
        r#"<svg xmlns="http://www.w3.org/2000/svg""#,
        "svg",
    );
}

#[test]
fn test_generate_shows_how_the_message_was_normalized() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "Agent 007")]);

    assert_includes(s.body(), "Enciphered as: <code>agent</code>", "normalized");
}

#[test]
fn test_generate_escapes_the_reflected_message() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "hi <script>alert(document.domain)</script>")],
    );

    ok(&s, "generate");
    refute_includes(s.body(), "<script>alert(document.domain)</script>", "raw");
    assert_includes(s.body(), "&lt;script&gt;", "escaped");
}

#[test]
fn test_generate_accepts_digits_and_underscores() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "r2d2_and_c3po")]);

    ok(&s, "generate");
}

#[test]
fn test_generate_rejects_messages_without_cipherable_characters() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "!!!")]);

    assert_eq!(400, s.status());
    assert_includes(s.body(), "no characters that can be enciphered", "refused");
}

#[test]
fn test_generate_rejects_missing_message() {
    let mut s = Session::new();
    s.post("/generate", &[]);

    assert_eq!(400, s.status());
}

#[test]
fn test_generate_rejects_oversized_messages() {
    let mut s = Session::new();
    let message = "a".repeat(form::MAX_MESSAGE_LENGTH + 1);
    s.post("/generate", &[("message", &message)]);

    assert_eq!(413, s.status());
}

#[test]
fn test_decode_form_renders() {
    let mut s = Session::new();
    s.get("/decode");

    ok(&s, "/decode");
    assert_includes(s.body(), r#"enctype="multipart/form-data""#, "multipart");
}

#[test]
fn test_decode_pasted_svg() {
    let mut s = Session::new();
    s.post(
        "/decode",
        &[
            ("svg", &to_svg_encoded("Meet me at dawn!", None)),
            ("decipher", "1"),
        ],
    );

    ok(&s, "decode");
    assert_includes(s.body(), "meet me at dawn", "read");
}

#[test]
fn test_decode_uploaded_svg() {
    let svg = to_svg_encoded("scriptorium", None);
    let mut s = Session::new();
    s.post_multipart(
        "/decode",
        &[
            FormPart::File {
                name: "file",
                filename: "cipher.svg",
                content_type: "image/svg+xml",
                content: svg.as_bytes(),
            },
            FormPart::Field("decipher", "1"),
        ],
    );

    ok(&s, "upload");
    assert_includes(s.body(), "scriptorium", "read");
}

#[test]
fn test_decode_falls_back_to_pasted_svg_when_the_upload_is_empty() {
    let svg = to_svg_encoded("abbey", None);
    let mut s = Session::new();
    s.post_multipart(
        "/decode",
        &[
            FormPart::File {
                name: "file",
                filename: "empty.svg",
                content_type: "image/svg+xml",
                content: b"",
            },
            FormPart::Field("svg", &svg),
            FormPart::Field("decipher", "1"),
        ],
    );

    ok(&s, "fallback");
    assert_includes(s.body(), "abbey", "read");
}

#[test]
fn test_decode_rejects_foreign_svg() {
    let mut s = Session::new();
    s.post(
        "/decode",
        &[(
            "svg",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="4"/></svg>"#,
        )],
    );

    assert_eq!(400, s.status());
    assert_includes(s.body(), "No signs found on this plate", "refused");
}

#[test]
fn test_secret_mode_round_trip_through_the_web() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Cîteaux, 1098!"),
            ("passphrase", "abbey of molesme"),
        ],
    );

    ok(&s, "generate");
    assert_includes(s.body(), "Secret mode", "secret");
    let svg = matched(s.body(), r"(?s)<svg xmlns.*?</svg>").expect("the plate");

    s.post(
        "/decode",
        &[("svg", &svg), ("passphrase", "abbey of molesme")],
    );

    ok(&s, "decode");
    assert_includes(s.body(), "Cîteaux, 1098!", "read");
}

#[test]
fn test_blank_passphrase_means_game_mode() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "Agent 007"), ("passphrase", "")]);

    assert_includes(s.body(), "Enciphered as: <code>agent</code>", "game");
}

#[test]
fn test_short_passphrase_comes_back_to_the_form_with_what_was_written() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Rendez-vous at dawn"),
            ("mode", "secret"),
            ("passphrase", "short"),
            ("layout", "rongorongo"),
        ],
    );
    let body = s.body();

    assert_eq!(422, s.status());
    assert_match(
        r#"(?s)id="passphrase-error"[^>]*>A passphrase needs at least 8 characters</p>"#,
        body,
        "error",
    );
    assert_match(r#"aria-invalid="true" autofocus"#, body, "focus");
    assert_includes(body, ">Rendez-vous at dawn</textarea>", "message kept");
    assert_match(r#"value="secret" checked"#, body, "mode kept");
    assert_match(r#"value="rongorongo" checked"#, body, "layout kept");
    assert_includes(body, r#"<body data-mode="secret">"#, "body mode");
    refute_includes(
        body,
        r#"value="short""#,
        "the passphrase is never sent back",
    );
}

#[test]
fn test_decode_with_a_short_passphrase_is_a_wrong_passphrase() {
    let svg = to_svg_encoded("hidden", Some("right passphrase"));
    let mut s = Session::new();
    s.post("/decode", &[("svg", &svg), ("passphrase", "short")]);

    assert_eq!(400, s.status());
    assert_includes(s.body(), "Wrong passphrase", "refused");
}

#[test]
fn test_decode_secret_image_with_wrong_passphrase() {
    let svg = to_svg_encoded("hidden", Some("right passphrase"));
    let mut s = Session::new();
    s.post(
        "/decode",
        &[("svg", &svg), ("passphrase", "wrong passphrase")],
    );

    assert_eq!(400, s.status());
    assert_includes(s.body(), "Wrong passphrase", "refused");
}

#[test]
fn test_secret_rongorongo_round_trip_through_the_web() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "Hanga Roa"),
            ("passphrase", "orongo village"),
            ("layout", "rongorongo"),
        ],
    );
    let svg = matched(s.body(), r"(?s)<svg xmlns.*?</svg>").expect("the plate");

    s.post(
        "/decode",
        &[("svg", &svg), ("passphrase", "orongo village")],
    );

    assert_includes(s.body(), "Hanga Roa", "read");
}

#[test]
fn test_decode_secret_image_without_passphrase_hints_at_secret_mode() {
    let mut s = Session::new();
    s.post(
        "/decode",
        &[("svg", &to_svg_encoded("hidden", Some("right passphrase")))],
    );

    assert_eq!(400, s.status());
    assert_includes(s.body(), "enter its passphrase", "hint");
}

#[test]
fn test_largest_secret_message_fits_the_upload_limit() {
    let message = "🙏".repeat(form::MAX_MESSAGE_LENGTH);
    let svg = to_svg_encoded(&message, Some("abbey of molesme"));

    assert!(svg.len() < form::MAX_SVG_BYTES);
}

#[test]
fn test_decode_rejects_oversized_svg() {
    let mut s = Session::new();
    let svg = " ".repeat(form::MAX_SVG_BYTES + 1);
    s.post("/decode", &[("svg", &svg)]);

    assert_eq!(413, s.status());
}

#[test]
fn test_the_form_offers_the_ladder() {
    let mut s = Session::new();
    s.get("/");

    let body = s.body();
    for rung in ["intact", "worn", "broken", "ruined"] {
        assert_match(
            &format!(r#"<input type="radio" name="condition" value="{rung}""#),
            body,
            rung,
        );
    }
    assert_match(r#"value="intact" data-rung checked"#, body, "intact");
    assert_includes(body, "A sealed plate cannot weather", "note");
    assert_includes(body, "Broken plates follow the snail", "note");
    // the library's and the CLI's, not the form's
    refute_includes(body, r#"name="tally""#, "tally");
    refute_includes(body, r#"name="weathered""#, "weathered");
}

#[test]
fn test_a_weathered_plate_shows_its_lacunae_and_never_its_lost_values() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "the brothers will gather in the chapter house"),
            ("condition", "worn"),
        ],
    );

    ok(&s, "worn");
    let body = s.body();
    assert_includes(body, r#"class="weathering""#, "weathering");
    assert_match("<dt>Condition</dt><dd>Worn</dd>", body, "condition");
    assert_match(
        r"Worn: \d+ lacunae\. Download it and send it",
        body,
        "caption",
    );
    let cells = cells_of(body);
    assert!(
        cells.iter().any(|cell| cell[2].is_null()),
        "a worn sign has no value on the page"
    );
    refute_match(r#" style=""#, body, "no inline style");
}

#[test]
fn test_a_weathered_plate_has_a_bench_that_never_holds_a_lost_value() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "the brothers will gather in the chapter house"),
            ("condition", "worn"),
        ],
    );

    let body = s.body();
    let lacunae = json_attr(body, "data-lacunae");
    let lacunae = lacunae.as_array().expect("a list");
    assert!(!lacunae.is_empty());
    for lacuna in lacunae {
        assert_lacuna_shows_only_the_file(lacuna);
    }
    assert_match(r"data-bench-sum>[\d?]{4} \+ [\d?]{4} = \?<", body, "sum");
    assert_match(r"data-bench-edition>[^<]*\[\.\]", body, "edition");
    assert_eq!(10, count(body, r#"class="bench-digit""#));
}

/// A lacuna holds only what the file shows: its place, the digits its strokes allow, and
/// its sign's surviving digits (none in its own place)
#[track_caller]
fn assert_lacuna_shows_only_the_file(lacuna: &Json) {
    let place = int(&lacuna["place"]);
    assert!([1, 10, 100, 1000].contains(&place), "{lacuna}");
    assert!(!lacuna["digits"].as_array().expect("digits").is_empty());
    let open: Vec<i64> = lacuna["open"]
        .as_array()
        .expect("open")
        .iter()
        .map(int)
        .collect();
    assert!(open.contains(&place));
    let known = int(&lacuna["known"]);
    assert!(open.iter().all(|open| (known / open % 10) == 0));
}

#[test]
fn test_the_form_does_not_draw_tally_signs() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "ora et labora"), ("tally", "1")]);

    ok(&s, "generate");
    refute_includes(s.body(), r#"class="tally-lines""#, "no tally");
    refute_match("<dt>Condition</dt>", s.body(), "no condition");
}

#[test]
fn test_a_tallied_plate_from_the_library_checks_its_tallies_on_the_readers_bench() {
    let (_, svg, _) = worn(
        "the brothers will gather in the chapter house",
        &Layout::Phaistos,
        true,
        false,
        &mut random(),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    assert_match(
        "<dt>Condition</dt><dd>Worn, with tally signs</dd>",
        body,
        "condition",
    );
    assert_includes(body, r#"data-tally=""#, "tally");
    assert_match(
        r"data-bench-tally>Tallies, \w+: row \?, column \?<",
        body,
        "bench tally",
    );
}

#[test]
fn test_scripts_and_styles_go_compressed_to_a_browser_that_accepts_it() {
    let mut s = Session::new();
    s.get("/js/app.js");
    let raw = s.last().raw.len();
    s.get_with("/js/app.js", &[("accept-encoding", "gzip")]);

    assert_eq!("gzip", header(&s, "content-encoding"));
    assert!(s.last().raw.len() < raw / 2);
}

#[test]
fn test_an_intact_plate_has_no_bench() {
    let mut s = Session::new();
    s.post("/generate", &[("message", "ora et labora")]);

    refute_includes(s.body(), "data-bench", "no bench");
}

#[test]
fn test_a_sealed_plate_cannot_weather() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "relics"),
            ("mode", "secret"),
            ("passphrase", "abbey of molesme"),
            ("condition", "worn"),
        ],
    );

    assert_eq!(422, s.status());
    assert_includes(s.body(), lacunae::NOT_SEALED, "refused");
    assert_match(
        r#"name="condition" value="worn" data-rung checked"#,
        s.body(),
        "kept",
    );
}

#[test]
fn test_two_readers_cannot_weather_yet() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "relics"),
            ("layout", "grille"),
            ("decoy", "vespers"),
            ("condition", "worn"),
        ],
    );

    assert_eq!(422, s.status());
    assert_includes(s.body(), lacunae::NOT_TWO_READERS, "refused");
}

#[test]
fn test_a_weathered_plate_is_read_back_as_an_edition() {
    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);
    ok(&s, "decode");
    assert_includes(s.body(), "Restored from a worn plate", "restored");
    assert_match(
        r#"<span class="leiden-(restored|doubt)">\[[^<]*\]</span>"#,
        s.body(),
        "edition",
    );
}

#[test]
fn test_a_weathered_grille_plate_is_read_with_its_key() {
    let grille = the_grille();
    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Grille(grille.clone()),
        false,
        false,
        &mut random(),
    );
    let mut s = Session::new();

    s.post(
        "/decode",
        &[
            ("svg", &svg),
            ("grille", &grille.to_string()),
            ("decipher", "1"),
        ],
    );
    ok(&s, "decode");
    assert_includes(s.body(), "leiden-", "edition");
}

#[test]
fn test_a_broken_plate_lays_its_fragments_in_a_tray() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "the brothers will gather in the chapter house"),
            ("condition", "broken"),
        ],
    );

    let body = s.body();
    let data = json_attr(body, "data-fragments");
    let count = count(body, r#"<g class="fragment""#);
    assert!((2..=fragments::MAX_PIECES).contains(&count), "{count}");
    assert_match(
        &format!("<dt>Condition</dt><dd>Broken, in {count} fragments</dd>"),
        body,
        "condition",
    );
    assert_match(
        &format!(r"Broken: {count} fragments\. Download it and send it"),
        body,
        "caption",
    );
    assert_eq!(count, data["fragments"].as_array().expect("a list").len());
    assert_includes(body, "data-rejoin-status", "status");
    refute_includes(body, "data-plate-cells", "no walking a plate in pieces");
}

/// The page holds what each fragment shows, read both ways, never where it goes
#[test]
fn test_a_broken_plate_page_never_says_where_a_fragment_goes() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "ora et labora"), ("condition", "broken")],
    );

    let data = json_attr(s.body(), "data-fragments");
    assert_eq!(6, int(&data["size"]));
    for fragment in data["fragments"].as_array().expect("a list") {
        let mut keys: Vec<&String> = fragment.as_object().expect("an object").keys().collect();
        keys.sort();
        assert_eq!(vec!["cols", "rows", "signs"], keys);
        assert_eq!(
            int(&fragment["rows"]) * int(&fragment["cols"]),
            fragment["signs"].as_array().expect("signs").len() as i64
        );
    }
}

#[test]
fn test_a_broken_plate_goes_back_to_the_form_when_it_cannot_break() {
    let mut s = Session::new();
    let cases: [(&[(&str, &str)], &str); 4] = [
        (&[("layout", "rongorongo")], fragments::NOT_ALONG_THE_ROWS),
        (&[("layout", "grille")], fragments::NOT_UNDER_A_GRILLE),
        (
            &[("condition", "ruined"), ("layout", "rongorongo")],
            fragments::NOT_ALONG_THE_ROWS,
        ),
        (
            &[("mode", "secret"), ("passphrase", "abbey of molesme")],
            fragments::NOT_SEALED,
        ),
    ];
    for (fields, error) in cases {
        let mut form = vec![("message", "ora et labora"), ("condition", "broken")];
        // the later field wins, as Ruby's Hash#merge
        for (name, value) in fields {
            form.retain(|(kept, _)| kept != name);
            form.push((name, value));
        }
        s.post("/generate", &form);

        assert_eq!(422, s.status(), "{fields:?}");
        assert_includes(s.body(), &escape_html(error), &format!("{fields:?}"));
        let rung = fields
            .iter()
            .find(|(name, _)| *name == "condition")
            .map_or("broken", |(_, value)| *value);
        assert_match(
            &format!(r#"name="condition" value="{rung}" data-rung checked"#),
            s.body(),
            "rung kept",
        );
        // never missed
        assert_match(
            &format!(r#"(?s)value="{rung}"[^>]*aria-invalid="true" autofocus>"#),
            s.body(),
            "focused",
        );
        let said = matched(s.body(), r#"id="condition-error"[^>]*>[^<]*</p>"#).expect("the error");
        assert_includes(&said, &escape_html(error), "said by the rung");
    }
}

#[test]
fn test_a_broken_plate_can_be_weathered_too() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[
            ("message", "the brothers will gather in the chapter house"),
            ("condition", "ruined"),
        ],
    );

    ok(&s, "ruined");
    let body = s.body();
    assert_match(
        r"<dt>Condition</dt><dd>Ruined: \d fragments, worn</dd>",
        body,
        "condition",
    );
    assert_match(
        r"Ruined: \d fragments, \d+ lacunae\. Download it and send it",
        body,
        "caption",
    );
    assert_includes(body, r#"class="lacunae""#, "lacunae");
    refute_includes(body, "data-bench", "no bench on a plate in pieces");
    let signs = fragment_signs(body);
    assert!(
        signs.iter().any(|sign| truthy(&sign[2])),
        "worn signs are marked"
    );
    assert!(signs.iter().all(|sign| {
        [&sign[0], &sign[1]]
            .iter()
            .filter(|side| !side.is_null())
            .all(|side| side.is_array())
    }));
}

fn fragment_signs(body: &str) -> Vec<Json> {
    json_attr(body, "data-fragments")["fragments"]
        .as_array()
        .expect("a list")
        .iter()
        .flat_map(|fragment| fragment["signs"].as_array().expect("signs").clone())
        .collect()
}

/// Ruby's truth: anything but nil and false.
fn truthy(value: &Json) -> bool {
    !(value.is_null() || value == &Json::Bool(false))
}

#[test]
fn test_a_weathered_broken_plate_is_read_back_as_an_edition() {
    let (_, svg, _, _) = broken("ora et labora", true, &mut RubyRandom::new(3));
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);
    ok(&s, "decode");
    assert_includes(s.body(), "Rejoined from a ruined plate", "rejoined");
    assert_match(
        r#"<span class="leiden-(restored|doubt)">\[[^<]*\]</span>"#,
        s.body(),
        "edition",
    );
}

#[test]
fn test_a_broken_plate_is_read_back_rejoined() {
    let (_, svg, _, _) = broken("ora et labora", false, &mut RubyRandom::new(3));
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);
    ok(&s, "decode");
    assert_includes(s.body(), "Rejoined from a broken plate", "rejoined");
    assert_includes(s.body(), "ora et labora", "read");
}

// --- The reader's game: a damaged plate brought to Read opens as its puzzle

#[test]
fn test_a_weathered_plate_brought_to_read_opens_as_a_bench() {
    let (_, svg, _) = worn(
        "the brothers will gather in the chapter house",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    ok(&s, "decode");
    let body = s.body();
    assert_includes(body, "A plate to restore", "title");
    assert_includes(body, "data-bench", "bench");
    assert_match("<dt>Condition</dt><dd>Worn</dd>", body, "condition");
    refute_includes(body, "data-transcription", "the answer is not on the page");
    refute_includes(body, "leiden-", "no edition");
}

#[test]
fn test_the_readers_bench_holds_only_what_the_file_shows() {
    let (sequence, svg, damage) = worn(
        "the brothers will gather in the chapter house",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let entries = json_attr(s.body(), "data-lacunae");
    let entries = entries.as_array().expect("a list");
    assert_eq!(
        damage.keys().copied().collect::<Vec<_>>(),
        signs_of(entries)
    );
    for entry in entries {
        assert_lacuna_shows_only_the_file(entry);
    }
    let cells = cells_of(s.body());
    assert!(
        damage.keys().all(|&index| cells[index][2].is_null()),
        "worn signs are not valued"
    );
    assert_eq!(sequence.len(), cells.len());
}

/// `entries.map { |entry| entry['sign'] }.uniq.sort`
fn signs_of(entries: &[Json]) -> Vec<usize> {
    let mut signs: Vec<usize> = entries
        .iter()
        .map(|entry| int(&entry["sign"]) as usize)
        .collect();
    signs.sort();
    signs.dedup();
    signs
}

#[test]
fn test_the_readers_bench_is_the_scribes_bench() {
    let (sequence, svg, damage) = worn(
        "the brothers will gather in the chapter house",
        &Layout::Rongorongo,
        true,
        false,
        &mut RubyRandom::new(5),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    let entries = json_attr(body, "data-lacunae");
    let scribes: Json = serde_json::from_str(&bench::entries_json(&bench::lacunae_entries(
        &sequence,
        &Layout::Rongorongo,
        &damage,
    )))
    .expect("JSON");
    assert_eq!(scribes, entries);
    assert_match(
        "<dt>Condition</dt><dd>Worn, with tally signs</dd>",
        body,
        "condition",
    );
    assert_match("<dt>Layout</dt><dd>Rongorongo</dd>", body, "layout");
    let signs = tally::signs(&sequence, &Layout::Rongorongo).expect("tallied");
    assert_eq!(
        json!({ "rows": signs.rows, "cols": signs.cols, "corner": signs.corner }),
        json_attr(body, "data-tally")
    );
}

#[test]
fn test_a_broken_plate_brought_to_read_opens_as_a_board() {
    let (_, svg, pieces, _) = broken(
        "the brothers will gather in the chapter house",
        false,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    assert_includes(body, "A plate to rejoin", "title");
    assert_match(
        &format!(
            "<dt>Condition</dt><dd>Broken, in {} fragments</dd>",
            pieces.len()
        ),
        body,
        "condition",
    );
    assert_eq!(pieces.len(), count(body, r#"<g class="fragment""#));
    assert_eq!(
        pieces.len(),
        json_attr(body, "data-fragments")["fragments"]
            .as_array()
            .expect("a list")
            .len()
    );
    assert_includes(body, "data-rejoin-status", "status");
    // no answer, no walking a plate in pieces
    refute_match("data-transcription|data-plate-cells", body, "no answer");
}

#[test]
fn test_a_weathered_broken_plate_brought_to_read_opens_as_a_board_with_worn_signs() {
    let (_, svg, _, _) = broken(
        "the brothers will gather in the chapter house",
        true,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    ok(&s, "decode");
    let body = s.body();
    assert_includes(body, "A plate to rejoin and restore", "title");
    assert_match(
        r"<dt>Condition</dt><dd>Ruined: \d fragments, worn</dd>",
        body,
        "condition",
    );
    let signs = fragment_signs(body);
    assert!(
        signs.iter().any(|sign| truthy(&sign[2])),
        "worn signs are marked"
    );
    refute_includes(body, "leiden-", "no edition");
}

#[test]
fn test_the_readers_page_asks_the_decoder_with_the_same_file() {
    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    assert_match(
        r#"(?s)<form class="readback" method="post" action="/decode">.*name="decipher" value="1".*Decipher it"#,
        body,
        "the readback form",
    );
    let carried = hidden_svg(body);

    s.post("/decode", &[("svg", &carried), ("decipher", "1")]);
    ok(&s, "decode");
    assert_includes(s.body(), "Restored from a worn plate", "restored");
    // the file shown is the file brought
    assert_eq!(decode(&svg), decode(&carried));
}

#[test]
fn test_a_weathered_grille_plate_plays_with_its_key() {
    let grille = the_grille();
    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Grille(grille.clone()),
        false,
        false,
        &mut random(),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("grille", &grille.to_string())]);
    ok(&s, "decode");
    assert_includes(s.body(), "A plate to restore", "title");
    assert_match("<dt>Layout</dt><dd>Grille</dd>", s.body(), "layout");
    assert_includes(
        s.body(),
        &format!(r#"<input type="hidden" name="grille" value="{grille}">"#),
        "carried",
    );
}

#[test]
fn test_a_weathered_grille_plate_needs_its_key_to_play() {
    let grille = the_grille();
    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Grille(grille),
        false,
        false,
        &mut random(),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    assert_eq!(400, s.status());
    assert_includes(s.body(), "Enter it under “Grille key”", "asks");

    s.post("/decode", &[("svg", &svg), ("grille", "00000-00000")]);
    assert_eq!(400, s.status());
    assert_includes(s.body(), grille::WRONG_KEY, "wrong key");
}

#[test]
fn test_the_readers_page_redraws_the_file_it_was_given() {
    let (_, svg, _, _) = broken("ora et labora", false, &mut RubyRandom::new(3));
    let hostile = svg.replacen(
        "<defs>",
        r#"<script>alert(1)</script><a href="javascript:alert(2)"><rect onclick="alert(3)"/></a><defs>"#,
        1,
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &hostile)]);
    ok(&s, "decode");
    let body = s.body();
    refute_includes(body, "alert(", "no script");
    refute_includes(body, "onclick", "no handler");
    assert_includes(body, r#"<g class="fragment""#, "the fragments");
}

#[test]
fn test_a_sealed_plate_is_read_not_played() {
    let mut s = Session::new();
    s.post(
        "/decode",
        &[(
            "svg",
            &to_svg_encoded("ora et labora", Some("abbey of molesme")),
        )],
    );
    assert_eq!(400, s.status()); // not a game: the decoder asks for the passphrase
    assert_includes(s.body(), "secret mode", "hint");
    refute_includes(s.body(), "data-desk", "no desk");

    let (_, svg, _) = worn(
        "ora et labora",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    s.post(
        "/decode",
        &[("svg", &svg), ("passphrase", "abbey of molesme")],
    );
    assert_eq!(400, s.status()); // a weathered plate is never sealed: the decoder says so
    refute_includes(s.body(), "data-bench", "no bench");
}

// --- The reading desk: an intact game plate, read pair by pair

#[test]
fn test_an_intact_plate_brought_to_read_opens_on_the_desk() {
    let (sequence, svg) = inscribe("Meet me at dawn", &Layout::Phaistos, false, None);
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    ok(&s, "decode");
    let body = s.body();
    assert_includes(body, "A plate to read", "title");
    assert_match("<dt>Condition</dt><dd>Intact</dd>", body, "condition");
    assert_includes(
        body,
        &format!("Pair 1 of {} · signs 1 and 2", sequence.len() / 2),
        "the first pair",
    );
    assert_eq!(2, count(body, r#"class="glyph desk-glyph""#));
    // the text is the reader's to read
    refute_match("data-transcription|meet me at dawn", body, "no answer");
}

#[test]
fn test_the_desk_holds_the_tablet_and_the_signs_values() {
    let (sequence, svg) = inscribe("Meet me at dawn", &Layout::Phaistos, false, None);
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    let tablet: serde_json::Map<String, Json> = characters()
        .into_iter()
        .map(|(sum, c)| (sum.to_string(), json!(c.to_string())))
        .collect();
    assert_eq!(Json::Object(tablet), json_attr(body, "data-tablet"));
    let values = values_of(&cells_of(body));
    assert_eq!(sequence.into_iter().map(Some).collect::<Vec<_>>(), values);
    // the script draws the pairs from it
    assert_includes(body, r#"data-glyph-table=""#, "glyph table");
}

#[test]
fn test_the_desk_follows_the_layout_that_reads_the_plate() {
    let (_, svg) = inscribe("Meet me at dawn", &Layout::Rongorongo, false, None);
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let body = s.body();
    assert_includes(body, "data-desk", "desk");
    assert_match("<dt>Layout</dt><dd>Rongorongo</dd>", body, "layout");
    assert_includes(body, r#"data-action="turn-plate""#, "turn");
}

#[test]
fn test_a_grille_plate_opens_on_the_desk_with_its_key() {
    let grille = the_grille();
    let (_, svg) = inscribe(
        "Vespers at six",
        &Layout::Grille(grille.clone()),
        false,
        None,
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("grille", &grille.to_string())]);
    ok(&s, "decode");
    assert_includes(s.body(), "data-desk", "desk");
    assert_match("<dt>Layout</dt><dd>Grille</dd>", s.body(), "layout");
    assert_includes(
        s.body(),
        &format!(r#"<input type="hidden" name="grille" value="{grille}">"#),
        "carried",
    );
}

#[test]
fn test_the_desk_asks_the_decoder_with_the_same_file() {
    let (_, svg) = inscribe("Meet me at dawn", &Layout::Phaistos, false, None);
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg)]);
    let carried = hidden_svg(s.body());

    s.post("/decode", &[("svg", &carried), ("decipher", "1")]);
    assert_includes(s.body(), "data-transcription", "transcription");
    assert_includes(s.body(), "meet me at dawn", "read");
}

// --- The scriptorium record: a reserved line on Read, and the rung a won plate counts under

#[test]
fn test_the_read_page_reserves_a_line_for_the_scriptorium_record() {
    let mut s = Session::new();
    s.get("/decode");

    assert_includes(
        s.body(),
        r#"<p class="record" data-record="read" aria-live="polite"></p>"#,
        "record",
    );
}

#[test]
fn test_a_readers_page_names_its_rung_for_the_record() {
    let plates = [
        (
            inscribe("Meet me at dawn", &Layout::Phaistos, false, None).1,
            "intact",
        ),
        (
            worn(
                "ora et labora",
                &Layout::Phaistos,
                false,
                false,
                &mut RubyRandom::new(3),
            )
            .1,
            "worn",
        ),
        (
            broken("ora et labora", false, &mut RubyRandom::new(3)).1,
            "broken",
        ),
        (
            broken("ora et labora", true, &mut RubyRandom::new(3)).1,
            "ruined",
        ),
    ];
    let mut s = Session::new();
    for (svg, rung) in &plates {
        s.post("/decode", &[("svg", svg)]);

        assert_includes(s.body(), &format!(r#"data-play="{rung}""#), rung);
        assert_includes(
            s.body(),
            r#"<p class="record" data-record="play" aria-live="polite"></p>"#,
            rung,
        );
    }
}

#[test]
fn test_the_scribes_preview_is_not_recorded() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "ora et labora"), ("condition", "worn")],
    );

    refute_includes(s.body(), "data-play=", "not played");
    refute_includes(s.body(), "data-record=", "not recorded");
}

// --- A ruined plate: the board, then the bench on the plate rejoined

#[test]
fn test_a_ruined_board_offers_the_bench_once_whole_and_a_broken_one_does_not() {
    let (_, ruined, _, _) = broken("ora et labora", true, &mut RubyRandom::new(3));
    let mut s = Session::new();
    s.post("/decode", &[("svg", &ruined)]);
    let form = matched(
        s.body(),
        r#"(?s)<form method="post" action="/decode" class="rejoin-restore">.*?</form>"#,
    )
    .expect("the form");
    assert_includes(
        &form,
        r#"<input type="hidden" name="rejoined" value="1">"#,
        "rejoined",
    );
    assert_includes(
        &form,
        "data-rejoin-restore disabled>Restore it</button>",
        "button",
    );

    let (_, broken_svg, _, _) = broken("ora et labora", false, &mut RubyRandom::new(3));
    s.post("/decode", &[("svg", &broken_svg)]);
    refute_includes(s.body(), "data-rejoin-restore", "no bench to offer");
}

#[test]
fn test_a_ruined_plate_rejoined_opens_on_the_bench() {
    let (sequence, svg, pieces, damage) = broken(
        "the brothers will gather in the chapter house",
        true,
        &mut RubyRandom::new(3),
    );
    let mut s = Session::new();

    s.post("/decode", &[("svg", &svg), ("rejoined", "1")]);
    let body = s.body();
    assert_includes(body, "A plate to restore", "title");
    assert_includes(body, "Whole again: the fragments are rejoined", "whole");
    assert_match(
        "<dt>Condition</dt><dd>Ruined, rejoined: worn</dd>",
        body,
        "condition",
    );
    assert_match(r#"data-play="ruined"|data-bench"#, body, "played");
    assert_eq!(pieces.len(), count(body, r#"<g class="fragment""#));
    assert_rejoined_bench_holds_only_the_file(body, &sequence, &damage);
}

#[track_caller]
fn assert_rejoined_bench_holds_only_the_file(body: &str, sequence: &[Value], damage: &Damage) {
    let entries = json_attr(body, "data-lacunae");
    let entries = entries.as_array().expect("a list");
    assert_eq!(
        damage.keys().copied().collect::<Vec<_>>(),
        signs_of(entries)
    );
    for entry in entries {
        assert_lacuna_shows_only_the_file(entry);
    }
    let values = values_of(&cells_of(body));
    let expected: Vec<Option<Value>> = sequence
        .iter()
        .enumerate()
        .map(|(i, &num)| (!damage.contains_key(&i)).then_some(num))
        .collect();
    assert_eq!(expected, values);
    let carried = hidden_svg(body);
    assert!(
        fragments::is_broken(&carried),
        "the decoder gets the broken file, not the drawing"
    );
}

#[test]
fn test_rejoined_is_ignored_on_a_plate_that_is_not_ruined() {
    let (_, broken_svg, _, _) = broken("ora et labora", false, &mut RubyRandom::new(3));
    let mut s = Session::new();
    s.post("/decode", &[("svg", &broken_svg), ("rejoined", "1")]);
    assert_includes(s.body(), "data-rejoin-status", "the board");
    refute_includes(s.body(), "data-bench", "no bench");

    let (_, worn_svg, _) = worn(
        "ora et labora",
        &Layout::Phaistos,
        false,
        false,
        &mut RubyRandom::new(3),
    );
    s.post("/decode", &[("svg", &worn_svg), ("rejoined", "1")]);
    assert_includes(s.body(), "data-bench", "the bench");
    assert_match("<dt>Condition</dt><dd>Worn</dd>", s.body(), "condition");
}

// --- Beyond the Ruby suite: what its rack-test session could not reach

/// The Ruby suite never sent the middleware's refusals through the deflater;
/// a browser always asks for gzip, and a page goes compressed to it.
#[test]
fn test_pages_go_compressed_to_a_browser_that_accepts_it() {
    let mut s = Session::new();
    s.get_with("/", &[("accept-encoding", "gzip, deflate, br")]);

    ok(&s, "/");
    assert_eq!("gzip", header(&s, "content-encoding"));
    assert_includes(&header(&s, "vary"), "Accept-Encoding", "vary");
    let mut page = String::new();
    std::io::Read::read_to_string(
        &mut flate2::read::GzDecoder::new(s.last().raw.as_slice()),
        &mut page,
    )
    .expect("gzip");
    assert_includes(&page, r#"action="/generate""#, "the index");
}

/// A generated plate's file, uploaded as a file rather than pasted, opens the
/// same reader's page as the pasted one (the multipart path end to end).
#[test]
fn test_a_generated_plate_uploaded_as_a_file_reads_back() {
    let mut s = Session::new();
    s.post(
        "/generate",
        &[("message", "Ora et labora"), ("layout", "furrows")],
    );
    let svg = hidden_svg(s.body());
    s.post_multipart(
        "/decode",
        &[
            FormPart::File {
                name: "file",
                filename: "plate.svg",
                content_type: "image/svg+xml",
                content: svg.as_bytes(),
            },
            FormPart::Field("svg", "ignored: the file wins"),
        ],
    );

    ok(&s, "upload");
    assert_includes(s.body(), r#"data-play="intact""#, "the desk");
    assert_match("<dt>Layout</dt><dd>Furrows</dd>", s.body(), "layout");
    assert_eq!("no-store", header(&s, "cache-control"));
}

/// The leaf headers go on every routed page, refusals and errors too, but
/// never on a static file (Sinatra serves those before its filters).
#[test]
fn test_the_leaf_headers_go_on_routed_pages_not_on_static_files() {
    let mut s = Session::new();
    s.get("/nowhere");
    assert_eq!(404, s.status());
    assert_eq!(
        middleware::CONTENT_SECURITY_POLICY,
        header(&s, "content-security-policy")
    );

    s.get("/css/app.css");
    ok(&s, "static");
    assert_eq!(None, s.last().header("content-security-policy"));
    assert_eq!("public, max-age=3600", header(&s, "cache-control"));
    assert_eq!("nosniff", header(&s, "x-content-type-options"));
}

// --- Divergences found against the running Ruby app, held fixed

fn gunzip(raw: &[u8]) -> String {
    let mut text = String::new();
    std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(raw), &mut text).expect("gzip");
    text
}

fn form_body(body: &str) -> Option<(String, Vec<u8>)> {
    Some((
        "application/x-www-form-urlencoded".to_string(),
        body.as_bytes().to_vec(),
    ))
}

/// The Ruby app's refusals are frozen constants Rack::Deflater cannot write
/// its gzip stream into: a FrozenError (a 500 backtrace page), after which
/// the shared headers say gzip over every plain refusal. That bug is not
/// kept: a browser that accepts gzip is refused in gzip, and nothing of one
/// refusal is left on the next.
#[test]
fn test_a_refusal_goes_compressed_to_a_browser_that_accepts_it() {
    let mut s = Session::new();
    let length = (form::MAX_SVG_BYTES + 65 * 1024).to_string();
    let too_large = |gzip: bool| {
        let mut headers = vec![("content-length", length.as_str())];
        if gzip {
            headers.push(("accept-encoding", "gzip"));
        }
        headers
    };
    s.request("POST", "/decode", &too_large(true), form_body(""));
    assert_eq!(413, s.status());
    assert_eq!("gzip", header(&s, "content-encoding"));
    assert_eq!(
        "The request is larger than any plate.\n",
        gunzip(&s.last().raw)
    );
    s.request("POST", "/decode", &too_large(false), form_body(""));
    assert_eq!(None, s.last().header("content-encoding"));
    assert_eq!("The request is larger than any plate.\n", s.body());

    let mut door = Session::with(Config {
        front_door_key: Some("abbey-of-molesme".to_string()),
        ..Config::default()
    });
    door.get_with("/", &[("accept-encoding", "gzip")]);
    assert_eq!(403, door.status());
    assert_eq!("Not through the front door.\n", gunzip(&door.last().raw));
    assert_eq!("no-store", header(&door, "cache-control"));
}

/// Rack refuses a `%` that is not an escape, in the query or a form body,
/// a static file's query too (Sinatra reads the params first): Sinatra's
/// BadRequest, its message escaped twice, before the leaf headers are set.
#[test]
fn test_a_malformed_escape_is_a_bad_request() {
    let mut s = Session::new();
    s.request(
        "POST",
        "/generate",
        &[],
        form_body("message=ab%Zc&mode=secret"),
    );
    assert_eq!(400, s.status());
    assert_eq!(
        "Invalid query parameters: invalid %-encoding (ab%Zc)",
        s.body()
    );
    assert_eq!("text/html;charset=utf-8", s.last().content_type());
    assert_eq!(None, s.last().header("content-security-policy"));
    assert_eq!(None, s.last().header("cache-control"));

    s.request("POST", "/generate", &[], form_body("message=%<b>"));
    assert_eq!(
        "Invalid query parameters: invalid %-encoding (%&amp;lt;b&amp;gt;)",
        s.body()
    );

    for path in [
        "/?x=%zz",
        "/css/app.css?v=%",
        "/up?v=%",
        "/chapter/2026-10-26?v=%",
    ] {
        s.get(path);
        assert_eq!(400, s.status(), "{path}");
        assert_includes(
            s.body(),
            "Invalid query parameters: invalid %-encoding (%",
            path,
        );
    }
}

/// Rack's form_data?: a POST without a content type is read as a form, and
/// the media type is matched in any case, whatever its parameters.
#[test]
fn test_a_form_is_read_whatever_its_content_type_is_written_like() {
    let mut s = Session::new();
    let body = b"message=Ora&mode=secret&passphrase=short".to_vec();
    for content_type in [
        "",
        "Application/X-WWW-Form-Urlencoded",
        "application/x-www-form-urlencoded; charset=UTF-8",
    ] {
        s.request(
            "POST",
            "/generate",
            &[],
            Some((content_type.to_string(), body.clone())),
        );
        assert_eq!(422, s.status(), "{content_type:?}");
        assert_includes(
            s.body(),
            "A passphrase needs at least 8 characters",
            content_type,
        );
    }
}

/// Rack reads a multipart body under any multipart type with a boundary,
/// and refuses one that ends before its closing boundary (EmptyContentError,
/// which Sinatra answers 400); an empty one is no form at all.
#[test]
fn test_a_multipart_body_is_read_as_rack_reads_it() {
    let mut s = Session::new();
    let part = "--XyZ\r\nContent-Disposition: form-data; name=\"svg\"\r\n\r\n<svg/>";
    let closed = format!("{part}\r\n--XyZ--\r\n");
    let send = |s: &mut Session, content_type: &str, body: &str| {
        s.request(
            "POST",
            "/decode",
            &[],
            Some((content_type.to_string(), body.as_bytes().to_vec())),
        );
    };

    send(&mut s, "multipart/mixed; boundary=XyZ", &closed);
    assert_includes(s.body(), "No signs found on this plate", "multipart/mixed");
    send(&mut s, "multipart/form-data; BOUNDARY=\"XyZ\"", &closed);
    assert_includes(
        s.body(),
        "No signs found on this plate",
        "a quoted boundary",
    );

    for (content_type, body) in [
        ("multipart/form-data; boundary=XyZ", part.to_string()),
        (
            "multipart/form-data; boundary=XyZ",
            closed.replace("XyZ", "abc"),
        ),
    ] {
        send(&mut s, content_type, &body);
        assert_eq!(400, s.status());
        assert_eq!(
            "Invalid multipart/form-data: Rack::Multipart::EmptyContentError",
            s.body()
        );
        assert_eq!(None, s.last().header("content-security-policy"));
    }

    send(&mut s, "multipart/form-data; boundary=XyZ", "");
    assert_eq!(400, s.status());
    assert_includes(s.body(), "Not an SVG document", "no form at all");
}

/// REXML reads an empty file as a document with no root ("Not an SVG
/// document"), and a second root as malformed; the library now reads them
/// alike (a core fix: rustergraphia::svg::parse).
#[test]
fn test_an_empty_or_doubled_file_is_refused_in_the_ruby_words() {
    let mut s = Session::new();
    for form in [
        &[][..],
        &[("svg", "")][..],
        &[("svg", ""), ("decipher", "1")][..],
    ] {
        s.post("/decode", form);
        assert_eq!(400, s.status());
        assert_includes(
            s.body(),
            "Not an SVG document. If this image was made in secret mode, enter its passphrase.",
            &format!("{form:?}"),
        );
    }
    s.post("/decode", &[("svg", "<svg/><svg/>")]);
    assert_includes(s.body(), "Unreadable SVG: not well-formed XML", "two roots");
}

/// The scribe compares what was pencilled with the tablet's character as
/// Ruby does, the whole string (`char.to_s`): two letters, or null, are not
/// a letter; keys are read by Ruby's `Integer()`, pairs may come as a list.
#[test]
fn test_the_scribe_holds_only_what_is_written_whole() {
    let (sequence, svg) = inscribe(
        "Meet me at the old abbey",
        &Layout::Phaistos,
        false,
        Some(GRILLE_KEY),
    );
    let right = right_answers(&sequence);
    let mut s = Session::new();
    let ask = |s: &mut Session, pencil: &str| {
        s.post(
            "/ask",
            &[("svg", &svg), ("pencil", pencil), ("inked", "[]")],
        );
        assert_eq!(200, s.status(), "{pencil}");
        serde_json::from_str::<Json>(s.body()).expect("JSON")["held"].clone()
    };
    let entry = |(sum, c): (Value, char)| format!(r#""{sum}":"{c}""#);
    let [a, b, c] = [right[0], right[1], right[2]];

    let third_spelled =
        |spelling: &str| format!(r#"{{{},{},"{}":{spelling}}}"#, entry(a), entry(b), c.0);
    assert_eq!(
        json!({}),
        ask(&mut s, &third_spelled(&format!(r#""{}x""#, c.1)))
    );
    assert_eq!(json!({}), ask(&mut s, &third_spelled("null")));
    assert_eq!(
        pencil_json(&right[..3]),
        ask(&mut s, &third_spelled(&format!(r#""{}""#, c.1)))
    );

    let hex = format!(r#"{{"0x{:x}":"{}",{},{}}}"#, a.0, a.1, entry(b), entry(c));
    assert_eq!(pencil_json(&right[..3]), ask(&mut s, &hex));
    let listed = format!(
        r#"[[{},"{}"],["{}","{}"],[{}.5,"{}"]]"#,
        a.0, a.1, b.0, b.1, c.0, c.1
    );
    assert_eq!(pencil_json(&right[..3]), ask(&mut s, &listed));

    s.post(
        "/ask",
        &[
            ("svg", &svg),
            ("pencil", &format!(r#"{{{},{}}}"#, entry(a), entry(a))),
            ("inked", "[]"),
        ],
    );
    assert_eq!(400, s.status(), "a key given twice");
    assert_includes(
        s.body(),
        "The notebook could not be read: ask again",
        "refused",
    );
}
