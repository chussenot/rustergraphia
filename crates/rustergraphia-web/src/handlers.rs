//! The routes (app.rb), dispatched with Sinatra's manners: whole-path
//! matches, 404 for any method the path does not take, the Erratum for every
//! refusal, and the library's words everywhere.

use std::sync::LazyLock;

use rustergraphia::grille::Part;
use rustergraphia::layout::Layout;
use rustergraphia::svg::Damage;
use rustergraphia::{cipher, fragments, lacunae, secret, tablet, two_readers};

use crate::chapter::{self, Kept};
use crate::error::AppError;
use crate::esc::h;
use crate::form::{self, Params};
use crate::reader_game;
use crate::views::index::IndexState;
use crate::views::result::ResultState;
use crate::views::simple::{render_decode, render_error, Decoded};
use crate::views::{self, Frame};

/// A routed response before the middleware dresses it.
pub struct Page {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    /// A route-set Cache-Control that overrides the before filter's.
    pub cache_control: Option<String>,
    pub x_cascade: bool,
}

impl Page {
    fn html(status: u16, body: String) -> Page {
        Page {
            status,
            content_type: "text/html;charset=utf-8",
            body: body.into_bytes(),
            cache_control: None,
            x_cascade: false,
        }
    }
}

static CHAPTER_PAGES: LazyLock<Kept> = LazyLock::new(Kept::new);

/// The whole routed app: called with the cleaned path, inside one blocking
/// closure per request; the library's per-thread parse cache is let go at
/// the end of every path.
pub fn route(method: &str, path: &str, params: &mut Params, base_url: &str) -> Page {
    let page = dispatch(method, path, params, base_url);
    rustergraphia::svg::forget();
    page
}

fn dispatch(method: &str, path: &str, params: &mut Params, base_url: &str) -> Page {
    let get = method == "GET" || method == "HEAD";
    match (method, path) {
        _ if get && path == "/up" => Page {
            status: 200,
            content_type: "text/plain",
            body: b"up".to_vec(),
            cache_control: Some("no-store".to_string()),
            x_cascade: false,
        },
        _ if get && path == "/" => index_page(base_url),
        ("POST", "/generate") => generate(params, base_url),
        _ if get && path == "/decode" => decode_form(base_url),
        _ if get && path == "/about" => about_page(base_url),
        ("POST", "/decode") => decode_post(params, base_url),
        _ if get && path == "/chapter" => {
            let mut page = chapter_page(Some(chapter::today()), params, base_url, path);
            page.cache_control = Some(format!("public, max-age={}", chapter::seconds_left()));
            page
        }
        _ if get && chapter_date(path).is_some() => {
            let mut page = chapter_page(
                chapter::parse(&chapter_date(path).expect("checked")),
                params,
                base_url,
                path,
            );
            page.cache_control = Some("public, max-age=3600".to_string());
            page
        }
        ("POST", _) if chapter_date(path).is_some() => chapter_page(
            chapter::parse(&chapter_date(path).expect("checked")),
            params,
            base_url,
            path,
        ),
        ("POST", "/ask") => ask(params, base_url, path),
        _ => erratum(
            &AppError::NotFound,
            Frame {
                base_url: base_url.to_string(),
                path: path.to_string(),
                ..Frame::default()
            },
            true,
        ),
    }
}

/// `/chapter/YYYY-MM-DD`, anchored, ASCII digits.
fn chapter_date(path: &str) -> Option<String> {
    let date = path.strip_prefix("/chapter/")?;
    let bytes = date.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .iter()
            .all(|&i| bytes[i].is_ascii_digit())
    {
        Some(date.to_string())
    } else {
        None
    }
}

fn erratum(error: &AppError, mut frame: Frame, cascade: bool) -> Page {
    let body = render_error(&error.message());
    frame.title = None;
    let mut page = Page::html(error.status(), views::render_layout(&frame, &body));
    page.x_cascade = cascade && matches!(error, AppError::NotFound);
    page
}

fn frame(base_url: &str, path: &str, page: &'static str) -> Frame {
    Frame {
        page: Some(page),
        base_url: base_url.to_string(),
        path: path.to_string(),
        ..Frame::default()
    }
}

fn index_page(base_url: &str) -> Page {
    let body = views::index::render_index(&IndexState::default());
    Page::html(
        200,
        views::render_layout(&frame(base_url, "/", "inscribe"), &body),
    )
}

fn decode_form(base_url: &str) -> Page {
    Page::html(
        200,
        views::render_layout(&frame(base_url, "/decode", "read"), &render_decode()),
    )
}

fn about_page(base_url: &str) -> Page {
    let mut frame = frame(base_url, "/about", "about");
    frame.title = Some("About the ciphers".to_string());
    frame.description = Some(
        "The ciphers of the Cistercian monks: their history, how to read a sign, and the tablet behind Cistergraphia."
            .to_string(),
    );
    Page::html(
        200,
        views::render_layout(&frame, &views::about::render_about()),
    )
}

/// POST /generate, in the Ruby route's exact order.
fn generate(params: &Params, base_url: &str) -> Page {
    let mut frame = frame(base_url, "/generate", "inscribe");
    let message = params.get("message").to_string();
    if message.chars().count() > form::MAX_MESSAGE_LENGTH {
        return erratum(
            &AppError::halt(
                413,
                format!(
                    "Message is limited to {} characters",
                    form::MAX_MESSAGE_LENGTH
                ),
            ),
            frame,
            false,
        );
    }
    let passphrase = form::passphrase(params);
    frame.secret = passphrase.is_some();
    let chosen = form::chosen_layout(params);
    let passphrase_error = passphrase
        .as_deref()
        .filter(|_| frame.secret)
        .and_then(secret::passphrase_problem);

    // The novice's message, under a grille
    let decoy_text = params.get("decoy").trim().to_string();
    let mut decoy_error = None;
    let decoy = if decoy_text.is_empty() || !chosen.layout.is_grille() {
        None
    } else {
        if message.chars().count() > form::MAX_PAIR_LENGTH
            || decoy_text.chars().count() > form::MAX_PAIR_LENGTH
        {
            decoy_error = Some(format!(
                "With two readers, each message is limited to {} characters",
                form::MAX_PAIR_LENGTH
            ));
        }
        Some(decoy_text)
    };

    // read_conditions
    let condition = form::valid_condition(params);
    let (weathered, broken) = form::conditions(&condition);
    let condition_error = form::Refusals::condition_problem(
        &condition,
        passphrase.as_deref(),
        &chosen.layout,
        decoy.is_some(),
    );
    let pathless = params.get("path") == "withheld";
    let path_error = pathless
        .then(|| form::Refusals::path_problem(passphrase.as_deref(), &chosen.layout, broken))
        .flatten();
    let own_tablet = params.get("tablet") == "own";
    let tablet_error = own_tablet
        .then(|| {
            form::Refusals::tablet_problem(passphrase.as_deref(), &chosen.layout, weathered, broken)
        })
        .flatten();

    let state = IndexState {
        secret: frame.secret,
        layout_keyless: match &chosen.layout {
            Layout::Grille(_) => None,
            Layout::Phaistos => None,
            other => Some(other.name().to_string()),
        },
        grille_chosen: chosen.layout.is_grille() || chosen.grille_error.is_some(),
        message: message.clone(),
        grille_key: chosen.grille_key.clone(),
        grille_error: chosen.grille_error.clone(),
        decoy: decoy.clone(),
        decoy_error: decoy_error.clone(),
        passphrase_error: passphrase_error.clone(),
        condition: Some(condition.clone()),
        condition_error: condition_error.map(str::to_string),
        own_tablet,
        tablet_error: tablet_error.map(str::to_string),
        pathless,
        path_error: path_error.map(str::to_string),
    };
    let any_error = state.passphrase_error.is_some()
        || state.grille_error.is_some()
        || state.decoy_error.is_some()
        || state.condition_error.is_some()
        || state.path_error.is_some()
        || state.tablet_error.is_some();
    if any_error {
        let body = views::index::render_index(&state);
        return Page::html(422, views::render_layout(&frame, &body));
    }

    // inscribe_plate
    let inscribed = inscribe_plate(
        &message,
        passphrase.as_deref(),
        &chosen.layout,
        decoy.as_deref(),
        params,
        weathered,
        broken,
        pathless,
        own_tablet,
    );
    let plate = match inscribed {
        Ok(plate) => plate,
        Err(error) => return erratum(&error, frame, false),
    };
    let symbols = plate.sequence.len();
    let result = ResultState {
        secret: frame.secret,
        message: message.clone(),
        decoy,
        pieces: plate.pieces,
        damage: plate.damage,
        tablet: plate.tablet,
        pathless,
        normalized: cipher::normalize(&message),
        symbols,
        layout: plate.layout,
        svg_content: plate.svg,
        fragments: plate.fragments,
        sequence: plate.sequence,
    };
    let body = views::result::render_result(&result);
    Page::html(200, views::render_layout(&frame, &body))
}

struct Inscribed {
    sequence: Vec<rustergraphia::Value>,
    svg: String,
    layout: Layout,
    pieces: Option<usize>,
    damage: Option<Damage>,
    tablet: Option<String>,
    fragments: Option<Vec<fragments::Fragment>>,
}

#[allow(clippy::too_many_arguments)]
fn inscribe_plate(
    message: &str,
    passphrase: Option<&str>,
    layout: &Layout,
    decoy: Option<&str>,
    params: &Params,
    weathered: bool,
    broken: bool,
    pathless: bool,
    own_tablet: bool,
) -> Result<Inscribed, AppError> {
    let mut random = rustergraphia::RubyRandom::new_os();
    if broken {
        let (sequence, svg, pieces, damage) =
            fragments::inscribe(message, layout, weathered, &mut random)?;
        let fragments = fragments::read(&svg)?;
        return Ok(Inscribed {
            sequence,
            layout: layout.clone(),
            pieces: Some(pieces.len()),
            damage: (!damage.is_empty()).then_some(damage),
            tablet: None,
            fragments: Some(fragments),
            svg,
        });
    }
    if weathered {
        let (sequence, svg, damage) =
            lacunae::inscribe(message, layout, passphrase, false, pathless, &mut random)?;
        return Ok(Inscribed {
            sequence,
            layout: layout.clone(),
            pieces: None,
            damage: Some(damage),
            tablet: None,
            fragments: None,
            svg,
        });
    }
    if let Some(decoy) = decoy {
        let given = (!params.get("grille").trim().is_empty())
            .then(|| layout.as_grille().cloned())
            .flatten();
        let (sequence, svg, abbot) =
            two_readers::inscribe(message, decoy, passphrase, given.as_ref())?;
        return Ok(Inscribed {
            sequence,
            svg,
            layout: Layout::Grille(abbot.as_part(Part::Pair)),
            pieces: None,
            damage: None,
            tablet: None,
            fragments: None,
        });
    }
    let seed = own_tablet.then(tablet::generate);
    let (sequence, svg) = cipher::inscribe(
        message,
        passphrase,
        layout,
        false,
        pathless,
        seed.as_deref(),
        &mut random,
    )?;
    Ok(Inscribed {
        sequence,
        svg,
        layout: layout.clone(),
        pieces: None,
        damage: None,
        tablet: seed,
        fragments: None,
    })
}

/// POST /decode: the reader's page, or the transcription, or the Erratum
/// with the Ruby route's exact hints.
fn decode_post(params: &Params, base_url: &str) -> Page {
    let mut frame = frame(base_url, "/decode", "read");
    let svg = match form::submitted_svg(params) {
        Ok(svg) => svg,
        Err(error) => return erratum(&error, frame, false),
    };
    let passphrase = form::passphrase(params);
    match decode_flow(&svg, passphrase.as_deref(), params) {
        Ok(DecodeOutcome::Play(state)) => {
            let body = views::play::render_play(&state);
            Page::html(200, views::render_layout(&frame, &body))
        }
        Ok(DecodeOutcome::Decoded(page)) => {
            frame.secret = passphrase.is_some();
            let body = views::simple::render_decoded(&page.view());
            Page::html(200, views::render_layout(&frame, &body))
        }
        Err(error) => erratum(&error, frame, false),
    }
}

enum DecodeOutcome {
    Play(Box<views::play::PlayState>),
    Decoded(DecodedPage),
}

struct DecodedPage {
    broken: bool,
    weathered: bool,
    secret: bool,
    reader: Option<Part>,
    message: String,
    decoy: Option<String>,
}

impl DecodedPage {
    fn view(&self) -> Decoded<'_> {
        Decoded {
            broken: self.broken,
            weathered: self.weathered,
            secret: self.secret,
            reader: self.reader,
            message: &self.message,
            decoy: self.decoy.as_deref(),
        }
    }
}

fn decode_flow(
    svg: &str,
    passphrase: Option<&str>,
    params: &Params,
) -> Result<DecodeOutcome, AppError> {
    let attempt = (|| -> Result<DecodeOutcome, AppError> {
        let submitted = form::submitted_grille(params)?;
        let playable = reader_game::playable(
            svg,
            passphrase,
            params.get("decipher") == "1",
            params.get("rejoined") == "1",
            submitted.as_ref(),
        )?;
        if let Some(state) = playable {
            return Ok(DecodeOutcome::Play(Box::new(state)));
        }
        // read_plate
        let weathered = lacunae::is_weathered(svg)?;
        let broken = fragments::is_broken(svg);
        let (message, decoy, reader) = match &submitted {
            Some(grille) => {
                let reading = two_readers::read(svg, grille, passphrase)?;
                (reading.message, reading.decoy, Some(reading.reader))
            }
            None => (cipher::decode(svg, passphrase, None)?, None, None),
        };
        Ok(DecodeOutcome::Decoded(DecodedPage {
            broken,
            weathered,
            secret: passphrase.is_some(),
            reader,
            message,
            decoy,
        }))
    })();
    attempt.map_err(|error| match error {
        AppError::Domain(_) => rescue(error, svg, passphrase, params),
        other => other,
    })
}

/// The route's two rescues: the grille-key hint, then the secret-mode hint.
fn rescue(error: AppError, _svg: &str, passphrase: Option<&str>, params: &Params) -> AppError {
    let AppError::Domain(message) = &error else {
        return error;
    };
    // GrilleKeyNeeded carries the library's fixed message
    if message.starts_with("This plate was written under a grille: it needs its key") {
        return AppError::Domain(format!(
            "{message}. Enter it under “Grille key”, with the passphrase if it was sealed."
        ));
    }
    if passphrase.is_some() || !params.get("grille").trim().is_empty() {
        return error;
    }
    AppError::Domain(format!(
        "{message}. If this image was made in secret mode, enter its passphrase."
    ))
}

/// GET/POST /chapter…: the day's page, kept once laid.
fn chapter_page(
    date: Option<chrono::NaiveDate>,
    params: &mut Params,
    base_url: &str,
    path: &str,
) -> Page {
    let mut frame = frame(base_url, path, "chapter");
    frame.page = None; // only set once the day is laid (404s carry no current nav)
    let day = date.and_then(chapter::of);
    let Some(day) = day else {
        let message = not_a_chapter(date);
        return erratum(&AppError::halt(404, message), frame, false);
    };
    params.keep_only("rejoined");
    let rejoined = params.get("rejoined") == "1";
    let key = (path.to_string(), rejoined, chapter::today());
    let page = CHAPTER_PAGES.fetch(&key, || {
        // a drawing failure leaves the page empty: a bug, said below
        lay_chapter(&day, rejoined, path).unwrap_or_default()
    });
    if page.is_empty() {
        return erratum(&AppError::Bug, frame, false);
    }
    let written = page.replace(chapter::BASE_URL, &h(base_url));
    Page::html(200, written)
}

fn lay_chapter(day: &chapter::Day, rejoined: bool, path: &str) -> Result<String, AppError> {
    let svg = chapter::plate(day).ok_or(AppError::Bug)?;
    let marks = reader_game::read_marks(&svg)?;
    let state = reader_game::lay(&svg, svg.clone(), marks, rejoined, None)?;
    let mut state = state.ok_or(AppError::Bug)?;
    state.chapter = Some(day.clone());
    let frame = Frame {
        title: Some(format!("Chapter {}", day.number)),
        secret: false,
        page: Some("chapter"),
        description: Some(
            "The day's chapter: one plate the abbey sets each day, the same for every reader."
                .to_string(),
        ),
        base_url: chapter::BASE_URL.to_string(),
        path: path.to_string(),
    };
    let body = views::play::render_play(&state);
    Ok(views::render_layout(&frame, &body))
}

fn not_a_chapter(date: Option<chrono::NaiveDate>) -> String {
    let Some(date) = date else {
        return "This is not a date the abbey keeps. A chapter’s address is its day: /chapter/2026-10-26."
            .to_string();
    };
    if date > chapter::today() {
        return "This chapter is not yet written: it is set on its day, at midnight UTC."
            .to_string();
    }
    format!(
        "The chapters begin on {} {} {}.",
        chapter::first().format("%-d"),
        chapter::month_name(chapter::first()),
        chapter::first().format("%Y")
    )
}

/// POST /ask: the scribe's JSON, or the Erratum page with the JSON content
/// type (the Ruby quirk, kept).
fn ask(params: &Params, base_url: &str, path: &str) -> Page {
    let outcome =
        form::submitted_svg(params).and_then(|svg| reader_game::scribe_answer(&svg, params));
    match outcome {
        Ok(held) => Page {
            status: 200,
            content_type: "application/json",
            body: format!("{{\"held\":{held}}}").into_bytes(),
            cache_control: None,
            x_cascade: false,
        },
        Err(error) => {
            let frame = Frame {
                base_url: base_url.to_string(),
                path: path.to_string(),
                ..Frame::default()
            };
            let mut page = erratum(&error, frame, false);
            page.content_type = "application/json";
            page
        }
    }
}
