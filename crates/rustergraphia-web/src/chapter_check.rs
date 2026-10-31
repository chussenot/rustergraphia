//! The chapter rake tasks (web/chapter_check.rb, Rakefile): every text under
//! every rung, and the days of new readings left.

use chrono::NaiveDate;
use rustergraphia::cipher;
use rustergraphia::layout::Layout;

use crate::chapter::{self, Day, Rung};

/// `Chapter::WEEK.values`, in its order: Monday's rung to Sunday's.
const RUNGS: [(Rung, Option<Layout>); 7] = [
    (Rung::Intact, Some(Layout::Phaistos)),
    (Rung::Worn, None),
    (Rung::Pathless, None),
    (Rung::Broken, Some(Layout::Phaistos)),
    (Rung::Tablet, None),
    (Rung::Ruined, Some(Layout::Phaistos)),
    (Rung::Expert, None),
];

/// Every reading and feast under every rung, and the days ahead: 0 when all
/// of them read, 1 (with the troubles on stderr) when any would not.
pub fn check(days: i64) -> i32 {
    let from = chapter::today().max(chapter::first());
    let troubles = troubles(days, from, &chapter::all_texts());
    if troubles.is_empty() {
        println!("Every text reads under every rung, and the next {days} days are set.");
        return 0;
    }
    for trouble in &troubles {
        eprintln!("{trouble}");
    }
    eprintln!("{} chapter(s) would not read", troubles.len());
    1
}

/// What would not do (`Chapter::Check.check`), as lines to print: every
/// text under every rung, each on a day of its own (its own seed), then the
/// days ahead as they are set.
pub fn troubles(days: i64, from: NaiveDate, texts: &[(String, String)]) -> Vec<String> {
    let mut trials = Vec::new();
    for (text, source) in texts {
        for (rung, layout) in &RUNGS {
            let date = from + chrono::Duration::days(trials.len() as i64);
            trials.push(Day {
                number: 0,
                date,
                rung: *rung,
                layout: layout.clone().unwrap_or_else(|| chapter::way_of(date)),
                text: text.clone(),
                source: source.clone(),
                feast: None,
            });
        }
    }
    let ahead = (0..days).map(|i| chapter::set(from + chrono::Duration::days(i)));
    trials
        .into_iter()
        .chain(ahead)
        .filter_map(|day| trouble(&day))
        .collect()
}

/// Why a chapter's plate does not read back as its text, or None.
pub fn trouble(day: &Day) -> Option<String> {
    let named = if day.number > 0 {
        format!("Chapter {}, {}", day.number, day.date.format("%Y-%m-%d"))
    } else {
        "A trial".to_string()
    };
    let label = format!(
        "{named} ({}, {}): {}",
        day.rung.name(),
        day.layout.name(),
        day.text
    );
    match chapter::draw(day).and_then(|svg| cipher::decode(&svg, None, None)) {
        Ok(edition) => {
            if holds(&edition, &cipher::normalize(&day.text)) {
                None
            } else {
                Some(format!("{label}: reads {edition}"))
            }
        }
        Err(error) => Some(format!("{label}: {error}")),
    }
}

/// Whether an edition holds a text: each letter as it stands, or among a
/// bracket's (`[r]` restored, `[b/l]` for the words to decide). Read as the
/// Ruby scan `/\[([^\]]*)\]|([^\[])/` reads it: a `[` that no `]` closes is
/// passed over.
pub fn holds(edition: &str, text: &str) -> bool {
    let chars: Vec<char> = edition.chars().collect();
    let mut tokens: Vec<Vec<String>> = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if chars[at] == '[' {
            match chars[at + 1..].iter().position(|&c| c == ']') {
                Some(end) => {
                    let inner: String = chars[at + 1..at + 1 + end].iter().collect();
                    // Ruby's "".split('/', -1) is [], not [""]
                    tokens.push(if inner.is_empty() {
                        Vec::new()
                    } else {
                        inner.split('/').map(str::to_string).collect()
                    });
                    at += end + 2;
                }
                None => at += 1,
            }
        } else {
            tokens.push(vec![chars[at].to_string()]);
            at += 1;
        }
    }
    let text_chars: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    tokens.len() == text_chars.len()
        && tokens
            .iter()
            .zip(&text_chars)
            .all(|(options, c)| options.contains(c))
}

/// Days of new readings left after today.
pub fn runway() -> i32 {
    let size = chapter::readings_len();
    let left = chapter::runway_days(chapter::today());
    println!("{size} readings; {left} days of new readings left after today.");
    let floor: i64 = std::env::var("RUNWAY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14);
    if left < floor {
        eprintln!("Under {floor} days: append readings to web/chapters.yml (docs/operations.md)");
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::holds;

    #[test]
    fn editions_hold_their_texts() {
        assert!(holds("h[e]ll[o/p]", "hello"));
        assert!(holds("h[e]ll[o/p]", "hellp"));
        assert!(holds("a[ ]b", "a b"));
        assert!(!holds("ab", "abc"));
        assert!(!holds("[x/y]", "z"));
    }

    #[test]
    fn an_unclosed_bracket_is_passed_over_as_the_ruby_scan_does() {
        assert!(holds("ab[c", "abc"));
        assert!(!holds("a[]b", "ab"));
    }
}
