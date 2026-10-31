//! test/chapter_test.rb, ported: the day's chapter (chapter.rs), its texts,
//! its week, the same plate for every reader, and its pages. Every text
//! under every rung is `chapter-check`'s (a minute and a half); here, the
//! next two weeks.
//!
//! Ruby's `at(date)` set CHAPTER_DATE for the block; here the date is the
//! session's (`Config::chapter_date`) for the pages, and `chapter::on` for
//! the library calls, so no test touches a process-wide variable.

mod common;

use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use rustergraphia::layout::Layout;
use rustergraphia::{cipher, tablet};
use rustergraphia_web::chapter::{self, Day, Rung};
use rustergraphia_web::chapter_check;

use common::*;

fn first() -> NaiveDate {
    chapter::first()
}

fn days(n: i64) -> Duration {
    Duration::days(n)
}

/// A session whose today is `date` (Ruby's `at(date) { ... }` around rack-test).
fn at(date: NaiveDate) -> Session {
    Session::with(Config {
        chapter_date: Some(date),
        ..Config::default()
    })
}

/// `Chapter::WEEK.fetch(date.wday).first`
fn rung_of_the_week(date: NaiveDate) -> Rung {
    match date.weekday() {
        chrono::Weekday::Mon => Rung::Intact,
        chrono::Weekday::Tue => Rung::Worn,
        chrono::Weekday::Wed => Rung::Pathless,
        chrono::Weekday::Thu => Rung::Broken,
        chrono::Weekday::Fri => Rung::Tablet,
        chrono::Weekday::Sat => Rung::Ruined,
        chrono::Weekday::Sun => Rung::Expert,
    }
}

#[test]
fn test_every_text_is_what_its_plate_carries_and_long_enough_for_a_tablet_of_its_own() {
    let readings = chapter::readings();
    let feasts = chapter::feasts();
    let all = readings
        .iter()
        .copied()
        .chain(feasts.iter().map(|(_, _, text, source)| (*text, *source)));
    for (text, source) in all {
        assert_match(
            r"\A[A-Za-z]+( [A-Za-z]+)+\z",
            text,
            "letters and single spaces: the plate shows what it carries",
        );
        assert!(text.split_whitespace().count() >= tablet::SHORT, "{text}");
        assert!(
            text.chars().count() <= 50,
            "{text}: a 10 × 10 plate at most"
        );
        assert!(!source.trim().is_empty());
    }
    let texts: Vec<String> = readings
        .iter()
        .map(|(text, _)| text.to_lowercase())
        .collect();
    let mut unique = texts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        texts.len(),
        "a reading appears once in the order"
    );
}

#[test]
fn test_feasts_are_days_of_the_year_with_a_name() {
    for (day, name, _, _) in chapter::feasts() {
        let (month, date) = day.split_once('-').expect("month-day");
        let month: u32 = month.parse().expect("a month");
        let date: u32 = date.parse().expect("a day");
        // a leap year: 02-29 would be a day
        assert!(
            NaiveDate::from_ymd_opt(2028, month, date).is_some(),
            "{day}"
        );
        assert!(!name.is_empty());
    }
}

#[test]
fn test_the_first_chapter_is_a_monday_and_the_week_climbs_to_the_expert_plate() {
    assert_eq!(chrono::Weekday::Mon, first().weekday());
    let rungs: Vec<Rung> = (0..7)
        .map(|i| chapter::set(first() + days(i)).rung)
        .collect();
    assert_eq!(
        vec![
            Rung::Intact,
            Rung::Worn,
            Rung::Pathless,
            Rung::Broken,
            Rung::Tablet,
            Rung::Ruined,
            Rung::Expert
        ],
        rungs
    );
    assert_eq!(Layout::Phaistos, chapter::set(first()).layout);
    assert_eq!(8, chapter::set(first() + days(7)).number);
}

#[test]
fn test_the_reading_order_runs_from_the_top_and_starts_again_past_the_last() {
    let readings = chapter::readings();
    let feasts: Vec<&str> = chapter::feasts().iter().map(|(day, ..)| *day).collect();
    let day_one = chapter::set(first());
    assert_eq!(readings[0].0, day_one.text);
    let mut again = first() + days(readings.len() as i64);
    while feasts.contains(&again.format("%m-%d").to_string().as_str()) {
        again += days(1);
    }
    let index = ((again - first()).num_days() as usize) % readings.len();
    assert_eq!(readings[index].0, chapter::set(again).text);
}

#[test]
fn test_a_feast_sets_its_own_line_and_keeps_the_weekdays_rung() {
    let bernard = NaiveDate::from_ymd_opt(2027, 8, 20).expect("a date");
    let day = chapter::set(bernard);
    assert_eq!(Some("Saint Bernard of Clairvaux".to_string()), day.feast);
    let feast_text = chapter::feasts()
        .into_iter()
        .find(|(day, ..)| *day == "08-20")
        .map(|(_, _, text, _)| text)
        .expect("Saint Bernard's feast");
    assert_eq!(feast_text, day.text);
    assert_eq!(rung_of_the_week(bernard), day.rung);
    assert_eq!(None, chapter::set(bernard + days(1)).feast);
}

#[test]
fn test_there_is_no_chapter_before_the_first_nor_ahead_of_today() {
    chapter::on(Some(first() + days(3)), || {
        assert!(chapter::of(first() - days(1)).is_none());
        assert!(chapter::of(first() + days(4)).is_none());
        assert_eq!(4, chapter::of(first() + days(3)).expect("written").number);
    });
}

#[test]
fn test_a_day_draws_the_same_plate_every_time_and_another_day_another() {
    let day = chapter::set(first() + days(1));
    assert_eq!(
        chapter::draw(&day).expect("drawn"),
        chapter::draw(&day).expect("drawn")
    );
    assert_ne!(
        chapter::draw(&day).expect("drawn"),
        chapter::draw(&chapter::set(first() + days(8))).expect("drawn")
    );
}

#[test]
fn test_the_next_two_weeks_each_read_back_as_their_text() {
    let from = Utc::now().date_naive().max(first());
    let troubles: Vec<String> = (0..14)
        .filter_map(|i| chapter_check::trouble(&chapter::set(from + days(i))))
        .collect();
    assert!(troubles.is_empty(), "{troubles:#?}");
}

#[test]
fn test_the_check_tries_a_text_under_every_rung_and_names_what_would_not_read() {
    let (text, source) = chapter::readings()[0];
    assert!(
        chapter_check::troubles(1, first(), &[(text.to_string(), source.to_string())]).is_empty()
    );
    let short = Day {
        number: 3,
        date: first(),
        rung: Rung::Broken,
        layout: Layout::Phaistos,
        text: "ab".to_string(),
        source: String::new(),
        feast: None,
    };
    assert_match(
        r"\AChapter 3, 2026-10-26 \(broken, phaistos\): ab: Too short a message to break",
        &chapter_check::trouble(&short).expect("a trouble"),
        "the trouble",
    );
}

#[test]
fn test_an_edition_holds_a_text_letter_by_letter_or_among_a_brackets() {
    assert!(chapter_check::holds("h[e]ll[o/p]", "hello"));
    assert!(chapter_check::holds("a[ ]b", "a b"));
    assert!(!chapter_check::holds("h[a/i]llo", "hello"));
    assert!(!chapter_check::holds("hell", "hello"));
}

#[test]
fn test_the_runway_counts_the_days_of_new_readings_left() {
    let readings = chapter::readings().len() as i64;
    assert_eq!(readings - 1, chapter::runway_days(first()));
    assert_eq!(-1, chapter::runway_days(first() + days(readings)));
}

#[test]
fn test_the_day_turns_at_midnight_utc() {
    let at = |h, m, s| {
        Utc.with_ymd_and_hms(2026, 10, 26, h, m, s)
            .single()
            .expect("a time")
    };
    assert_eq!(1, chapter::seconds_left_at(at(23, 59, 59)));
    assert_eq!(86_400, chapter::seconds_left_at(at(0, 0, 0)));
}

#[test]
fn test_the_chapter_opens_as_the_reader_s_game_with_its_day_and_never_its_text() {
    let today = first() + days(6);
    let mut s = at(today);
    s.get("/chapter");

    assert!(s.last().is_ok());
    let body = s.body();
    assert_includes(body, r#"data-play="expert""#, "expert");
    assert_includes(body, r#"data-chapter="2026-11-01""#, "the day");
    assert_includes(body, "Chapter 7", "the number");
    let text = chapter::on(Some(today), || chapter::of(today).expect("written").text);
    refute_includes(&body.to_lowercase(), &text.to_lowercase(), "never its text");
    assert_match(
        r#"<p class="chapter-source" data-chapter-source hidden>From [^<]+\.</p>"#,
        body,
        "the source, hidden",
    );
}

#[test]
fn test_today_s_chapter_may_be_kept_until_the_day_turns_and_links_back_never_ahead() {
    let mut s = at(first() + days(6));
    s.get("/chapter");

    assert_includes(
        &s.last().header("cache-control").unwrap_or_default(),
        "public",
        "kept",
    );
    assert_includes(s.body(), r#"href="/chapter/2026-10-31""#, "the day before");
    refute_includes(s.body(), r#"href="/chapter/2026-11-02""#, "not yet written");
}

#[test]
fn test_each_rung_of_the_week_opens_its_furniture() {
    let mut s = at(first() + days(6));
    for (offset, mark) in [
        (0, "data-desk"),
        (1, "data-bench"),
        (3, "data-fragments"),
        (4, "data-notebook"),
    ] {
        s.get(&format!("/chapter/{}", first() + days(offset)));

        assert!(s.last().is_ok(), "day {}", offset + 1);
        assert_includes(s.body(), mark, &format!("day {}", offset + 1));
    }
}

#[test]
fn test_a_page_kept_for_every_reader_names_each_request_s_host_and_nothing_else_lays_it() {
    let mut s = at(first() + days(6));
    s.get_with("/chapter/2026-10-26", &[("host", "abbey.example")]);
    s.get_with(
        "/chapter/2026-10-26?grille=K7Q2M-ZXB4P",
        &[("host", "scriptorium.example")],
    );

    assert!(s.last().is_ok());
    assert_includes(
        s.body(),
        r#"content="http://scriptorium.example/chapter/2026-10-26""#,
        "this request's host",
    );
    refute_includes(s.body(), "abbey.example", "not the last one's");
    refute_includes(s.body(), chapter::BASE_URL, "nor the kept page's");
    assert_includes(s.body(), "data-desk", "the desk");
}

#[test]
fn test_a_ruined_chapter_s_board_asks_the_chapter_for_its_bench() {
    let mut s = at(first() + days(5));
    s.get("/chapter/2026-10-31");

    assert_includes(
        s.body(),
        r#"action="/chapter/2026-10-31" class="rejoin-restore""#,
        "the board's form",
    );
    s.post("/chapter/2026-10-31", &[("rejoined", "1")]);

    assert!(s.last().is_ok());
    assert_includes(s.body(), "Ruined, rejoined: worn", "the bench");
    assert_includes(s.body(), r#"data-chapter="2026-10-31""#, "the day");
}

#[test]
fn test_a_chapter_not_written_or_not_a_date_is_said_so() {
    let mut s = at(first());
    s.get("/chapter/2026-10-27");
    assert_eq!(404, s.status());
    assert_includes(s.body(), "not yet written", "ahead");

    s.get("/chapter/2026-10-25");
    assert_includes(s.body(), "The chapters begin on 26 October 2026", "before");

    s.get("/chapter/2026-02-30");
    assert_includes(s.body(), "not a date the abbey keeps", "no such day");
}

#[test]
fn test_read_invites_the_reader_with_no_plate_to_the_chapter_and_the_nav_leads_there() {
    let mut s = Session::new();
    s.get("/decode");

    assert_includes(
        s.body(),
        r#"<a href="/chapter">read the day’s chapter</a>"#,
        "the invitation",
    );
    assert_includes(
        s.body(),
        r#"href="/chapter" class="">Chapter</a>"#,
        "the nav",
    );
}

// --- Beyond the Ruby suite

/// Two sessions on two days at once: the date is each one's, not the
/// process's (the Ruby suite's `at` set an environment variable).
#[test]
fn test_two_sessions_count_two_todays_side_by_side() {
    let mut early = at(first());
    let mut late = at(first() + days(6));
    early.get("/chapter");
    late.get("/chapter");
    assert_includes(early.body(), "Chapter 1", "the first day");
    assert_includes(late.body(), "Chapter 7", "the seventh");
    early.get("/chapter/2026-11-01");
    assert_eq!(404, early.status());
    late.get("/chapter/2026-11-01");
    assert!(late.last().is_ok());
}

/// The day's plate reads back as its text, through the page: the file the
/// chapter page carries goes to the decoder like any plate brought to Read.
#[test]
fn test_the_chapter_s_plate_reads_back_through_the_decoder() {
    let today = first() + days(2);
    let mut s = at(today);
    s.get("/chapter");
    let svg = hidden_svg(s.body());
    s.post("/decode", &[("svg", &svg), ("decipher", "1")]);
    assert!(s.last().is_ok());
    let text = cipher::normalize(&chapter::set(today).text);
    assert_includes(s.body(), &text, "the day's text");
}
