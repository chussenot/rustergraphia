//! The chapter rake tasks (web/chapter_check.rb, Rakefile): every text under
//! every rung, and the days of new readings left.

use rustergraphia::cipher;
use rustergraphia::layout::Layout;

use crate::chapter::{self, Day, Rung};

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
    let mut troubles = Vec::new();
    let texts = chapter::all_texts();
    let mut i = 0i64;
    for (text, source) in &texts {
        for (rung, layout) in &RUNGS {
            let date = from + chrono::Duration::days(i);
            let day = Day {
                number: 0,
                date,
                rung: *rung,
                layout: layout.clone().unwrap_or_else(|| chapter::way_of(date)),
                text: text.clone(),
                source: source.clone(),
                feast: None,
            };
            if let Some(trouble) = read_trouble(&day, "A trial") {
                troubles.push(trouble);
            }
            i += 1;
        }
    }
    for i in 0..days {
        let day = chapter::set(from + chrono::Duration::days(i));
        let label = format!("Chapter {}, {}", day.number, day.date.format("%Y-%m-%d"));
        if let Some(trouble) = read_trouble(&day, &label) {
            troubles.push(trouble);
        }
    }
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

fn read_trouble(day: &Day, label: &str) -> Option<String> {
    let label = format!(
        "{label} ({}, {}): {}",
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

/// Whether an edition can read as a text: each token's options hold the
/// text's character at its place.
fn holds(edition: &str, text: &str) -> bool {
    let mut tokens: Vec<Vec<String>> = Vec::new();
    let mut chars = edition.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut inner = String::new();
            for next in chars.by_ref() {
                if next == ']' {
                    break;
                }
                inner.push(next);
            }
            tokens.push(inner.split('/').map(str::to_string).collect());
        } else {
            tokens.push(vec![c.to_string()]);
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
}
