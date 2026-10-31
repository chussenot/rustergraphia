//! The day's chapter: one plate the abbey sets each day, the same for every
//! reader, as the chapter house heard one chapter of the Rule a day
//! (web/chapter.rb).
//!
//! Nothing is stored: a day's plate is drawn from a Random seeded by its
//! date, so every server, every restart and every reader draws the same
//! plate — and, the library being byte-identical with the Ruby original, the
//! same plate as the Ruby app.

use std::sync::{Arc, Mutex};

use chrono::{Datelike, NaiveDate, Utc};
use sha2::{Digest, Sha256};

use rustergraphia::layout::{Layout, KEYLESS};
use rustergraphia::{cipher, fragments, grille, lacunae, RubyRandom};

/// Chapter 1, a Monday.
pub fn first() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 26).expect("a fixed date")
}

pub const CACHE: usize = 16; // pages kept in memory
pub const BASE_URL: &str = "https://chapter.invalid"; // where a kept page names its host

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    Intact,
    Worn,
    Pathless,
    Broken,
    Tablet,
    Ruined,
    Expert,
}

impl Rung {
    pub fn name(self) -> &'static str {
        match self {
            Rung::Intact => "intact",
            Rung::Worn => "worn",
            Rung::Pathless => "pathless",
            Rung::Broken => "broken",
            Rung::Tablet => "tablet",
            Rung::Ruined => "ruined",
            Rung::Expert => "expert",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Day {
    pub number: i64,
    pub date: NaiveDate,
    pub rung: Rung,
    pub layout: Layout,
    pub text: String,
    pub source: String,
    pub feast: Option<String>,
}

impl Day {
    pub fn path(&self) -> String {
        format!("/chapter/{}", self.date.format("%Y-%m-%d"))
    }
}

#[derive(Debug, serde::Deserialize)]
struct Reading {
    text: String,
    source: String,
}

#[derive(Debug, serde::Deserialize)]
struct Feast {
    name: String,
    text: String,
    source: String,
}

#[derive(Debug, serde::Deserialize)]
struct Texts {
    readings: Vec<Reading>,
    feasts: std::collections::BTreeMap<String, Feast>,
}

fn texts() -> &'static Texts {
    static TEXTS: std::sync::LazyLock<Texts> = std::sync::LazyLock::new(|| {
        serde_yaml::from_str(include_str!("../chapters.yml")).expect("chapters.yml parses")
    });
    &TEXTS
}

pub fn readings_len() -> usize {
    texts().readings.len()
}

/// Today's date, as the chapter counts it. CHAPTER_DATE (2026-11-01) sets
/// another: a rehearsal of a day to come.
pub fn today() -> NaiveDate {
    std::env::var("CHAPTER_DATE")
        .ok()
        .and_then(|text| parse(&text))
        .unwrap_or_else(|| Utc::now().date_naive())
}

/// The chapter of a date, or None when there is none (before the first, or
/// not yet written).
pub fn of(date: NaiveDate) -> Option<Day> {
    if date < first() || date > today() {
        return None;
    }
    Some(set(date))
}

/// The chapter a date sets, written or not (the checks look ahead).
pub fn set(date: NaiveDate) -> Day {
    let index = (date - first()).num_days();
    let feast = texts().feasts.get(&date.format("%m-%d").to_string());
    let (text, source) = match feast {
        Some(feast) => (feast.text.clone(), feast.source.clone()),
        None => {
            let reading = &texts().readings[(index.rem_euclid(readings_len() as i64)) as usize];
            (reading.text.clone(), reading.source.clone())
        }
    };
    let (rung, layout) = week(date);
    Day {
        number: index + 1,
        date,
        rung,
        layout,
        text,
        source,
        feast: feast.map(|f| f.name.clone()),
    }
}

/// `Date#wday` => [rung, layout]; `:any` draws one of the five keyless ways.
fn week(date: NaiveDate) -> (Rung, Layout) {
    let (rung, layout) = match date.weekday() {
        chrono::Weekday::Mon => (Rung::Intact, Some(Layout::Phaistos)),
        chrono::Weekday::Tue => (Rung::Worn, None),
        chrono::Weekday::Wed => (Rung::Pathless, None),
        chrono::Weekday::Thu => (Rung::Broken, Some(Layout::Phaistos)),
        chrono::Weekday::Fri => (Rung::Tablet, None),
        chrono::Weekday::Sat => (Rung::Ruined, Some(Layout::Phaistos)),
        chrono::Weekday::Sun => (Rung::Expert, None),
    };
    (rung, layout.unwrap_or_else(|| way(date)))
}

/// A date written as the chapter's address, parsed as leniently as Ruby's
/// `Date.strptime(text, '%Y-%m-%d')`: optional sign, unpadded numbers,
/// trailing text ignored.
pub fn parse(text: &str) -> Option<NaiveDate> {
    let (year, rest) = leading_number(text, true)?;
    let rest = rest.strip_prefix('-')?;
    let (month, rest) = leading_number(rest, false)?;
    let rest = rest.strip_prefix('-')?;
    let (day, _) = leading_number(rest, false)?;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

/// The integer a lenient date field starts with, and what follows it.
fn leading_number(text: &str, sign_ok: bool) -> Option<(i32, &str)> {
    let bytes = text.as_bytes();
    let start = usize::from(sign_ok && matches!(bytes.first(), Some(b'+') | Some(b'-')));
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start {
        return None;
    }
    let value: i32 = text[..end].parse().ok()?;
    Some((value, &text[end..]))
}

/// Days of new readings left after today, before the order starts again.
pub fn runway_days(date: NaiveDate) -> i64 {
    readings_len() as i64 - (date - first()).num_days() - 1
}

/// The way `:any` draws for a date (the checks need it by itself).
pub fn way_of(date: NaiveDate) -> Layout {
    way(date)
}

/// Every reading and feast, as `(text, source)`: what the checks read.
pub fn all_texts() -> Vec<(String, String)> {
    let mut texts: Vec<(String, String)> = texts()
        .readings
        .iter()
        .map(|reading| (reading.text.clone(), reading.source.clone()))
        .collect();
    texts.extend(texts_feasts().map(|feast| (feast.text.clone(), feast.source.clone())));
    texts
}

fn texts_feasts() -> impl Iterator<Item = &'static Feast> {
    texts().feasts.values()
}

/// The day's plate, as the page shows it (PlateFile.clean, as a file brought
/// to Read).
pub fn plate(day: &Day) -> Option<String> {
    crate::plate_file::clean(&draw(day).ok()?)
}

/// The plate a chapter is: the day's seed decides the splits, the wear, the
/// breaks and a tablet of its own; nothing else does. The library consumes
/// the seeded draws in the Ruby original's exact order.
pub fn draw(day: &Day) -> rustergraphia::Result<String> {
    let mut random = seeded(day.date, "plate");
    match day.rung {
        Rung::Worn => lacunae::inscribe(&day.text, &day.layout, None, false, false, &mut random)
            .map(|(_, svg, _)| svg),
        Rung::Broken | Rung::Ruined => fragments::inscribe(
            &day.text,
            &Layout::Phaistos,
            day.rung == Rung::Ruined,
            &mut random,
        )
        .map(|(_, svg, _, _)| svg),
        _ => whole(day, &mut random),
    }
}

/// An intact plate: pathless, with a tablet of its own, both (the expert
/// plate) or neither. The tablet's seed is drawn from the day's random
/// before the plate is inscribed, as in the Ruby original.
fn whole(day: &Day, random: &mut RubyRandom) -> rustergraphia::Result<String> {
    let pathless = matches!(day.rung, Rung::Pathless | Rung::Expert);
    let tablet = matches!(day.rung, Rung::Tablet | Rung::Expert).then(|| seed(random));
    cipher::inscribe(
        &day.text,
        None,
        &day.layout,
        false,
        pathless,
        tablet.as_deref(),
        random,
    )
    .map(|(_, svg)| svg)
}

/// The day's layout: one of the five ways, drawn from the date.
fn way(date: NaiveDate) -> Layout {
    let mut random = seeded(date, "way");
    KEYLESS[random.rand_below(KEYLESS.len() as u64) as usize].clone()
}

/// A tablet's seed, drawn from the day's random.
fn seed(random: &mut RubyRandom) -> String {
    let alphabet = grille::ALPHABET.as_bytes();
    (0..grille::LENGTH)
        .map(|_| alphabet[random.rand_below(alphabet.len() as u64) as usize] as char)
        .collect()
}

pub fn seeded(date: NaiveDate, purpose: &str) -> RubyRandom {
    let digest = Sha256::digest(format!(
        "cistergraphia chapter {purpose} {}",
        date.format("%Y-%m-%d")
    ));
    RubyRandom::from_be_bytes(&digest)
}

/// Seconds until the day turns: how long today's chapter may be kept.
pub fn seconds_left() -> i64 {
    let now = Utc::now();
    let tomorrow = now.date_naive() + chrono::Duration::days(1);
    let midnight = tomorrow.and_hms_opt(0, 0, 0).expect("midnight");
    (midnight.and_utc().timestamp() - now.timestamp()).clamp(1, 86_400)
}

/// What a block makes for a key, made once and kept while it is asked for:
/// the oldest kept goes first. The block runs outside the lock.
pub struct Kept {
    pages: Mutex<Vec<(Key, Arc<String>)>>,
}

pub type Key = (String, bool, NaiveDate);

impl Kept {
    pub const fn new() -> Kept {
        Kept {
            pages: Mutex::new(Vec::new()),
        }
    }

    pub fn fetch(&self, key: &Key, make: impl FnOnce() -> String) -> Arc<String> {
        {
            let mut pages = self.pages.lock().expect("the chapter cache lock");
            if let Some(at) = pages.iter().position(|(kept, _)| kept == key) {
                let entry = pages.remove(at);
                let page = entry.1.clone();
                pages.push(entry); // asked again: kept longest
                return page;
            }
        }
        let made = Arc::new(make());
        let mut pages = self.pages.lock().expect("the chapter cache lock");
        pages.push((key.clone(), made.clone()));
        while pages.len() > CACHE {
            pages.remove(0);
        }
        made
    }
}

impl Default for Kept {
    fn default() -> Self {
        Self::new()
    }
}

/// A chapter's day, as a catalogue dates it: Monday 26 October 2026.
pub fn chapter_day(day: &Day) -> String {
    format!(
        "{} {} {} {}",
        weekday_name(day.date),
        day.date.day(),
        month_name(day.date),
        day.date.year()
    )
}

/// Where the day stands in the week's climb.
pub fn chapter_week(day: &Day) -> String {
    if day.rung == Rung::Expert {
        return "Sunday: the expert plate, pathless with a tablet of its own, the top of the week\u{2019}s climb.".to_string();
    }
    format!(
        "{}: {}. The week climbs from Monday\u{2019}s intact plate to Sunday\u{2019}s expert one.",
        weekday_name(day.date),
        crate::view::rung_name(day.rung.name())
    )
}

pub fn weekday_name(date: NaiveDate) -> &'static str {
    match date.weekday() {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    }
}

pub fn month_name(date: NaiveDate) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][date.month0() as usize]
}
