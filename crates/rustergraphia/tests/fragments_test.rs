//! Broken plates, ported from test/fragments_test.rb, with interop fixtures
//! written by the Ruby library itself (fragments_broken_31.svg is Ruby's
//! `Fragments.inscribe('meet me at the old abbey after vespers',
//! random: Random.new(31))`; the ruined set is seed 3 of 'the brothers will
//! gather in the chapter house', weathered).

use std::time::Instant;

use rustergraphia::layout::{self, Layout};
use rustergraphia::{cipher, fragments, glyphs, grille, Error, RubyRandom, Value};

const MESSAGES: [&str; 4] = [
    "the brothers will gather in the chapter house after compline",
    "Meet me at the old abbey after vespers",
    "ora et labora",
    "relics",
];

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_plate_breaks_into_two_to_four_fragments_that_tile_it() {
    for seed in 0..40 {
        let pieces = fragments::breaking::cut(10, &mut RubyRandom::new(seed));
        assert!((2..=fragments::MAX_PIECES).contains(&pieces.len()));
        assert!(pieces
            .iter()
            .all(|piece| piece.rows.min(piece.cols) >= fragments::MIN_SIDE));
        let mut cells: Vec<(i32, i32)> = pieces
            .iter()
            .flat_map(|piece| {
                piece
                    .cells()
                    .into_iter()
                    .map(move |(r, c)| (piece.row + r, piece.col + c))
            })
            .collect();
        cells.sort();
        let whole: Vec<(i32, i32)> = (0..10).flat_map(|r| (0..10).map(move |c| (r, c))).collect();
        assert_eq!(cells, whole, "seed {seed}");
    }
}

#[test]
fn a_broken_plate_rejoins_into_its_message() {
    for (seed, message) in MESSAGES.iter().enumerate() {
        let mut random = RubyRandom::new(seed as i64);
        let (_, svg, pieces, _) =
            fragments::inscribe(message, &Layout::Phaistos, false, &mut random).unwrap();
        assert!(fragments::is_broken(&svg));
        assert_eq!(pieces.len(), fragments::read(&svg).unwrap().len());
        assert_eq!(
            cipher::normalize(message),
            cipher::decode(&svg, None, None).unwrap()
        );
    }
}

#[test]
fn it_rejoins_one_way_only_and_says_where_each_fragment_goes() {
    let mut random = RubyRandom::new(3);
    let (_, svg, pieces, _) =
        fragments::inscribe("ora et labora", &Layout::Phaistos, false, &mut random).unwrap();
    let rejoining = fragments::rejoin(&svg, None, None).unwrap();

    let read = fragments::read(&svg).unwrap();
    assert_eq!(
        1,
        fragments::join::all(&read, &layout::KEYLESS).unwrap().len()
    );
    assert_eq!(Layout::Phaistos, rejoining.layout);
    let mut expected: Vec<(i32, i32, bool)> = pieces
        .iter()
        .map(|piece| (piece.row, piece.col, piece.turned))
        .collect();
    expected.sort();
    let mut places: Vec<(i32, i32, bool)> = rejoining
        .places
        .iter()
        .map(|&(_, row, col, turned)| (row, col, turned))
        .collect();
    places.sort();
    assert_eq!(expected, places);
}

#[test]
fn the_file_never_says_where_a_fragment_goes_or_whether_it_is_turned() {
    let mut random = RubyRandom::new(5);
    let (_, svg, pieces, _) = fragments::inscribe(
        "the brothers will gather in the chapter house",
        &Layout::Phaistos,
        false,
        &mut random,
    )
    .unwrap();

    let transforms: Vec<&str> = svg
        .match_indices("<g class=\"fragment\" transform=\"")
        .map(|(at, matched)| {
            let from = at + matched.len();
            &svg[from..from + svg[from..].find('"').unwrap()]
        })
        .collect();
    assert_eq!(pieces.len(), transforms.len());
    for transform in transforms {
        let tilt: f64 = transform
            .split("rotate(")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(tilt.abs() <= fragments::tray::TILT);
    }
    assert!(!svg.contains("data-"));
    assert!(!svg.contains("rotate(180"));
}

#[test]
fn fragments_laid_where_they_belong_show_the_plate_as_drawn() {
    let mut random = RubyRandom::new(1);
    let (sequence, svg, _, _) =
        fragments::inscribe("ora et labora", &Layout::Phaistos, false, &mut random).unwrap();
    let read = fragments::read(&svg).unwrap();
    let places = fragments::rejoin(&svg, None, None).unwrap().places;

    // Cell => the sign it shows, the fragments laid on the board (a turned
    // one's signs upright again)
    let mut board = std::collections::BTreeMap::new();
    for &(index, row, col, turned) in &places {
        let fragment = &read[index];
        for (&(r, c), reading) in &fragment.readings {
            let (r, c) = if turned {
                fragment.opposite(r, c)
            } else {
                (r, c)
            };
            let values = if turned {
                reading.turned.as_ref()
            } else {
                reading.upright.as_ref()
            };
            board.insert((row + r, col + c), values.unwrap()[0]);
        }
    }
    let laid: Vec<Value> = layout::path(6, &Layout::Phaistos)
        .unwrap()
        .into_iter()
        .map(|(row, col, _)| board[&(row, col)])
        .collect();
    assert_eq!(sequence, laid);
}

#[test]
fn the_same_plate_breaks_the_same_way() {
    let runs: Vec<_> = (0..2)
        .map(|_| {
            let mut random = RubyRandom::new(7);
            fragments::inscribe("ora et labora", &Layout::Phaistos, false, &mut random).unwrap()
        })
        .collect();
    assert_eq!(runs[0], runs[1]);
}

#[test]
fn refusals() {
    let mut random = RubyRandom::new(1);
    assert_eq!(
        fragments::TOO_SMALL,
        fragments::inscribe("ab", &Layout::Phaistos, false, &mut random)
            .unwrap_err()
            .to_string()
    );
    assert_eq!(
        fragments::NOT_ALONG_THE_ROWS,
        fragments::inscribe("ora et labora", &Layout::Rongorongo, false, &mut random)
            .unwrap_err()
            .to_string()
    );
    let under = Layout::Grille(grille::Grille::new("K7Q2M-ZXB4P", grille::Part::Whole).unwrap());
    assert_eq!(
        fragments::NOT_UNDER_A_GRILLE,
        fragments::inscribe("ora", &under, false, &mut random)
            .unwrap_err()
            .to_string()
    );
    let svg = fixture("fragments_broken_31.svg");
    let sealed = cipher::decode(&svg, Some("abbey of molesme"), None).unwrap_err();
    assert_eq!(fragments::SEALED, sealed.to_string());
}

#[test]
fn a_lost_fragment_does_not_rejoin() {
    let svg = fixture("fragments_broken_31.svg");
    let start = svg.find("  <g class=\"fragment\"").unwrap();
    // the group runs to the first dedented closing tag
    let end = start + svg[start..].find("\n  </g>").unwrap() + "\n  </g>".len();
    let without = format!("{}{}", &svg[..start], &svg[end..]);

    assert_ne!(svg, without);
    let error = cipher::decode(&without, None, None).unwrap_err();
    assert!(error.is_decode());
    let message = error.to_string();
    assert!(
        message.contains("square plate") || message.contains("rejoin"),
        "{message}"
    );
}

#[test]
fn too_many_fragments_are_refused_before_any_search() {
    let svg = fixture("fragments_broken_31.svg");
    let start = svg.find("  <g class=\"fragment\"").unwrap();
    let end = start + svg[start..].find("\n  </g>").unwrap() + "\n  </g>".len();
    let group = &svg[start..end];
    let crowded = svg.replace(
        "</svg>",
        &format!("{}\n</svg>", group.repeat(fragments::MAX_PIECES)),
    );

    let error = cipher::decode(&crowded, None, None).unwrap_err();
    assert!(error
        .to_string()
        .contains(&format!("at most {} fragments", fragments::MAX_PIECES)));
}

#[test]
fn a_hostile_plate_where_every_pair_balances_is_still_read_quickly() {
    // four fragments of 16 x 16 signs: a 32 x 32 board, the largest a message makes
    let side = 16;
    let readings: std::collections::BTreeMap<(i32, i32), fragments::CellReading> = (0..side)
        .flat_map(|r| (0..side).map(move |c| (r, c)))
        .map(|cell| {
            (
                cell,
                fragments::CellReading {
                    upright: Some(vec![9999]),
                    turned: Some(vec![9999]),
                    worn: false,
                },
            )
        })
        .collect();
    let null = fragments::Fragment::of(readings, Default::default());
    let four = vec![null.clone(), null.clone(), null.clone(), null];

    let started = Instant::now();
    let error = fragments::join::all(&four, &layout::KEYLESS).unwrap_err();
    assert!(error.to_string().contains("too many ways"), "{error}");
    assert!(started.elapsed().as_secs_f64() < 1.0);
}

#[test]
fn an_intact_plate_is_not_broken() {
    let mut random = RubyRandom::new(2);
    let (_, svg) = cipher::inscribe(
        "ora et labora",
        None,
        &Layout::Phaistos,
        false,
        false,
        None,
        &mut random,
    )
    .unwrap();
    assert!(!fragments::is_broken(&svg));
}

#[test]
fn a_sign_worn_past_one_corner_is_refused() {
    let (x, y) = layout::origin(0, 0, &Layout::Phaistos);
    let stem = layout::relative(glyphs::STEM, -x, -y);
    let positions: Vec<[i32; 4]> = glyphs::PLACES
        .iter()
        .flat_map(|&place| {
            [1, 2, 3, 4, 6]
                .iter()
                .map(move |&digit| glyphs::canonical(glyphs::digit_strokes(place, digit)[0]))
        })
        .map(|stroke| layout::relative(stroke, -x, -y))
        .collect();

    let one_corner = fragments::reading::readings(&[stem], &positions[..5], 0, 0).unwrap();
    assert_eq!(10, one_corner.upright.unwrap().len());
    let error = fragments::reading::readings(&[stem], &positions, 0, 0).unwrap_err();
    assert!(error.to_string().contains("past one corner"), "{error}");
}

#[test]
fn ruby_broken_plates_decode_and_regenerate_byte_identically() {
    // The fixture is Ruby's own output for this message and seed: decoding
    // it and re-inscribing it must both agree with Ruby exactly
    let svg = fixture("fragments_broken_31.svg");
    assert_eq!(
        "meet me at the old abbey after vespers",
        cipher::decode(&svg, None, None).unwrap()
    );
    let mut random = RubyRandom::new(31);
    let (_, ours, _, _) = fragments::inscribe(
        "meet me at the old abbey after vespers",
        &Layout::Phaistos,
        false,
        &mut random,
    )
    .unwrap();
    assert_eq!(svg, ours);
}

#[test]
fn a_ruined_ruby_plate_rejoins_into_its_edition_and_whole_drawing() {
    let svg = fixture("fragments_ruined_3.svg");
    let edition = fixture("fragments_ruined_3_edition.txt");
    assert_eq!(edition, cipher::decode(&svg, None, None).unwrap());

    let whole = fragments::rejoined(&svg).unwrap();
    assert_eq!(Layout::Phaistos, whole.layout);
    assert!(!whole.damage.is_empty());
    assert!(whole
        .candidates
        .iter()
        .all(|(index, candidates)| whole.signs[*index].is_none() && !candidates.is_empty()));
    assert_eq!(fixture("fragments_ruined_3_whole.svg"), whole.svg);

    // the drawing is a plate, not a board and a tray
    assert!(!whole.svg.contains("class=\"board\""));
}

#[test]
fn a_weathered_broken_plate_rejoins_into_its_edition() {
    for (seed, message) in MESSAGES.iter().take(3).enumerate() {
        let mut random = RubyRandom::new(seed as i64);
        let (_, svg, pieces, damage) =
            fragments::inscribe(message, &Layout::Phaistos, true, &mut random).unwrap();
        let edition = cipher::decode(&svg, None, None).unwrap();

        assert!(!damage.is_empty());
        let read = fragments::read(&svg).unwrap();
        assert_eq!(
            1,
            fragments::join::all(&read, &layout::KEYLESS).unwrap().len()
        );
        let layers = svg.matches("class=\"lacunae\"").count();
        assert!((1..=pieces.len()).contains(&layers));
        assert!(edition.contains('['), "{edition}");
        assert!(
            faithful(&edition, message),
            "{edition:?} is not {message:?}"
        );
    }
}

#[test]
fn a_weathered_fragment_never_carries_what_was_lost() {
    let mut random = RubyRandom::new(4);
    let (sequence, svg, _, damage) = fragments::inscribe(
        "the brothers will gather",
        &Layout::Phaistos,
        true,
        &mut random,
    )
    .unwrap();
    let lost: usize = svg
        .split("<g class=\"lacunae\"")
        .skip(1)
        .map(|layer| {
            layer[..layer.find("</g>").unwrap()]
                .matches("<line")
                .count()
        })
        .sum();
    let strokes: usize = sequence
        .iter()
        .map(|&num| glyphs::strokes(num, 0, 0).len())
        .sum();
    let taken: usize = damage
        .iter()
        .map(|(index, gone)| {
            glyphs::strokes(sequence[*index], 0, 0)
                .iter()
                .filter(|&&stroke| gone.contains(&glyphs::canonical(stroke)))
                .count()
        })
        .sum();

    assert_eq!(damage.values().map(Vec::len).sum::<usize>(), lost);
    assert_eq!(strokes - taken + lost, svg.matches("<line").count());
}

/// Whether an edition can be the message: every letter read is right, and
/// every restoration allows the true letter.
fn faithful(edition: &str, message: &str) -> bool {
    let mut expected: Vec<char> = cipher::normalize(message).chars().collect();
    expected.reverse();
    let mut rest = edition;
    while !rest.is_empty() {
        let Some(truth) = expected.pop() else {
            return false;
        };
        if let Some(stripped) = rest.strip_prefix('[') {
            let end = stripped.find(']').unwrap();
            let allowed: Vec<&str> = stripped[..end].split('/').collect();
            if !allowed.contains(&truth.to_string().as_str()) {
                return false;
            }
            rest = &stripped[end + 1..];
        } else {
            let mut chars = rest.chars();
            if chars.next() != Some(truth) {
                return false;
            }
            rest = chars.as_str();
        }
    }
    expected.is_empty()
}

#[test]
fn only_a_plate_that_rejoins_one_way_is_rejoined() {
    let svg = fixture("fragments_broken_31.svg");
    let whole = fragments::rejoined(&svg).unwrap();
    assert_eq!(Layout::Phaistos, whole.layout); // intact fragments: no lacunae
    assert!(whole.damage.is_empty());

    let start = svg.find("  <g class=\"fragment\"").unwrap();
    let end = start + svg[start..].find("\n  </g>").unwrap() + "\n  </g>".len();
    let lost = format!("{}{}", &svg[..start], &svg[end..]);
    assert!(matches!(
        fragments::rejoined(&lost).unwrap_err(),
        Error::Decode(_)
    ));
}
