//! The core library's behavior, ported from the Ruby suite
//! (test/cistergraphia_test.rb). Iteration counts are scaled down where the
//! Ruby suite brute-forces probabilities; the property under test is kept.

use rustergraphia::layout::{self, Layout};
use rustergraphia::{cipher, glyphs, pathless, svg, Error, Line, RubyRandom, Value, TABLET};

fn os_random() -> RubyRandom {
    RubyRandom::new_os()
}

fn encode(message: &str) -> Vec<Value> {
    cipher::encode(message, None, false, None, &mut os_random()).unwrap()
}

/// A number's strokes without the stem.
fn strokes_for(num: Value) -> Vec<Line> {
    glyphs::strokes(num, 0, 0)[1..].to_vec()
}

/// A glyph as an order-free set of canonical strokes.
fn glyph(num: Value) -> Vec<Line> {
    let mut strokes: Vec<Line> = glyphs::strokes(num, 0, 0)
        .into_iter()
        .map(glyphs::canonical)
        .collect();
    strokes.sort();
    strokes
}

#[test]
fn split_parts_add_back_up_to_the_original_value() {
    let expected: Vec<Value> = "the quick brown fox"
        .chars()
        .map(|c| {
            TABLET
                .iter()
                .find(|(ch, _)| *ch == c)
                .map(|(_, v)| v * 10)
                .unwrap()
        })
        .collect();
    for _ in 0..100 {
        let seq = cipher::cister("the quick brown fox", None, &mut os_random()).unwrap();
        let sums: Vec<Value> = seq
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| pair[0] + pair[1])
            .collect();
        assert_eq!(sums, expected);
    }
}

#[test]
fn normalize_keeps_only_tablet_characters() {
    assert_eq!(
        cipher::normalize("  Rendez-vous, R2-D2!! "),
        "rendez vous r d"
    );
}

#[test]
fn unsupported_characters_are_separators_instead_of_crashing() {
    let size = |m: &str| cipher::cister(m, None, &mut os_random()).unwrap().len();
    assert_eq!(size("r d"), size("r2d"));
    assert_eq!(size("snake case"), size("snake_case"));
    assert_eq!(2, size("  é!a?? "));
}

#[test]
fn message_without_cipherable_characters_raises_a_domain_error() {
    for message in ["", "   ", "!!!", "1234"] {
        let error = cipher::encode(message, None, false, None, &mut os_random()).unwrap_err();
        assert!(matches!(error, Error::EmptyMessage(_)), "{message:?}");
    }
}

#[test]
fn encode_pads_to_a_perfect_square() {
    for message in ["a", "ab", "abc", "hello", "cistercian"] {
        let size = encode(message).len();
        assert!(
            cipher::perfect_square(size),
            "{size} is not a perfect square"
        );
    }
}

#[test]
fn evil_symbols_never_split_a_character_pair() {
    for _ in 0..50 {
        let sequence = encode("hello world");
        let firsts: Vec<Value> = sequence
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| pair[0])
            .collect();
        assert!(!firsts.contains(&rustergraphia::EVIL_MARKER));
        assert_eq!(cipher::decipher(&sequence, None).unwrap(), "hello world");
    }
}

#[test]
fn perfect_square_is_exact_for_large_numbers() {
    let root = (1usize << 30) + 1;
    assert!(cipher::perfect_square(root * root));
    assert!(!cipher::perfect_square(root * root - 1));
}

#[test]
fn compound_digits_draw_every_stroke() {
    for (num, count) in [(1, 1), (5, 2), (7, 2), (8, 2), (9, 3), (9000, 3), (1993, 8)] {
        assert_eq!(strokes_for(num).len(), count, "strokes for {num}");
    }
}

#[test]
fn every_value_from_0_to_9999_has_a_distinct_glyph() {
    let mut glyphs: Vec<Vec<Line>> = (0..=9999).map(glyph).collect();
    glyphs.sort();
    glyphs.dedup();
    assert_eq!(glyphs.len(), 10_000);
}

#[test]
fn place_values_are_mirror_images_of_the_units() {
    for n in 1..=9 {
        let units = glyphs::lines_of(n).unwrap();
        let tens: Vec<Line> = units
            .iter()
            .map(|&[x1, y1, x2, y2]| [100 - x1, y1, 100 - x2, y2])
            .collect();
        let hundreds: Vec<Line> = units
            .iter()
            .map(|&[x1, y1, x2, y2]| [x1, 150 - y1, x2, 150 - y2])
            .collect();
        assert_eq!(glyphs::lines_of(n * 10).unwrap(), tens.as_slice());
        assert_eq!(glyphs::lines_of(n * 100).unwrap(), hundreds.as_slice());
    }
}

#[test]
fn spiral_coordinates_visit_every_cell_once() {
    let coords: Vec<(i32, i32)> = layout::path(6, &Layout::Phaistos)
        .unwrap()
        .into_iter()
        .map(|(row, col, _)| (row, col))
        .collect();
    let mut unique = coords.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 36);
    let top: Vec<(i32, i32)> = (0..6).map(|col| (0, col)).collect();
    assert_eq!(&coords[..6], top.as_slice()); // along the top
    let right: Vec<(i32, i32)> = (1..6).map(|row| (row, 5)).collect();
    assert_eq!(&coords[6..11], right.as_slice()); // down the right side
    let last = *coords.last().unwrap();
    assert!(
        [(2, 2), (2, 3), (3, 2), (3, 3)].contains(&last),
        "ends in the middle"
    );
}

#[test]
fn to_svg_lays_the_sequence_out_as_a_snail() {
    let sequence: Vec<Value> = (1..=9).collect();
    let drawn = cipher::to_svg(&sequence, &Layout::Phaistos, false, false, None).unwrap();
    let stems = svg::lines(&drawn, svg::Layer::Strokes)
        .unwrap()
        .into_iter()
        .filter(|&[x1, y1, x2, y2]| x1 == x2 && y2 - y1 == 150)
        .count();
    assert_eq!(stems, 9);
    assert_eq!(
        svg::read(&drawn, &Layout::Phaistos, None).unwrap(),
        sequence
    );
}

#[test]
fn boustrophedon_coordinates_turn_every_other_row() {
    let coords = layout::path(3, &Layout::Rongorongo).unwrap();
    assert_eq!(
        &coords[..3],
        &[(2, 0, false), (2, 1, false), (2, 2, false)],
        "bottom row, left to right"
    );
    assert_eq!(
        &coords[3..6],
        &[(1, 2, true), (1, 1, true), (1, 0, true)],
        "back, upside down"
    );
    assert_eq!(&coords[6..], &[(0, 0, false), (0, 1, false), (0, 2, false)]);
    let mut cells: Vec<(i32, i32)> = layout::path(4, &Layout::Rongorongo)
        .unwrap()
        .into_iter()
        .map(|(row, col, _)| (row, col))
        .collect();
    cells.sort();
    cells.dedup();
    assert_eq!(cells.len(), 16);
}

#[test]
fn turning_a_sign_swaps_its_places() {
    // Half a turn mirrors both ways: units <-> thousands, tens <-> hundreds
    let turned: Vec<Line> = glyphs::strokes(4127, 0, 0)
        .into_iter()
        .map(glyphs::turn)
        .collect();
    let undirected = |strokes: &[Line]| {
        let mut lines: Vec<Line> = strokes.iter().map(|&s| glyphs::canonical(s)).collect();
        lines.sort();
        lines
    };
    assert_eq!(
        undirected(&glyphs::strokes(7214, 0, 0)),
        undirected(&turned)
    );
    assert_eq!(glyphs::read(&turned).unwrap(), 7214);
}

#[test]
fn rongorongo_plates_draw_turned_rows_upside_down() {
    // 2 x 2: bottom row upright, left to right; top row turned, right to left
    let drawn = cipher::to_svg(&[1, 2, 3, 4], &Layout::Rongorongo, false, false, None).unwrap();
    assert_eq!(
        svg::read(&drawn, &Layout::Rongorongo, None).unwrap(),
        vec![1, 2, 3, 4]
    );
    // read as a snail, the top row is upside down
    assert_eq!(
        svg::read(&drawn, &Layout::Phaistos, None).unwrap(),
        vec![4000, 3000, 2, 1]
    );
}

#[test]
fn inscribe_never_draws_a_plate_that_reads_along_both_layouts() {
    // "jj" as raw rongorongo plates also reads as letters along the snail
    // about 1 time in 100 (the hazard this test guards against)
    let mut random = os_random();
    let ambiguous = (0..1000)
        .filter(|_| {
            let sequence = cipher::encode("jj", None, false, None, &mut random).unwrap();
            let drawn = cipher::to_svg(&sequence, &Layout::Rongorongo, false, false, None).unwrap();
            !cipher::unambiguous(&drawn, &Layout::Rongorongo).unwrap()
        })
        .count();
    assert!(ambiguous > 0, "the hazard this test guards against"); // ~10 expected
    for _ in 0..200 {
        let (_, drawn) = cipher::inscribe(
            "jj",
            None,
            &Layout::Rongorongo,
            false,
            false,
            None,
            &mut random,
        )
        .unwrap();
        assert_eq!(cipher::decode(&drawn, None, None).unwrap(), "jj");
    }
}

#[test]
fn the_leap_steps_as_the_zodiac_did_and_the_unwinding_is_not_the_snail_backwards() {
    let leap = layout::path(6, &Layout::Leap).unwrap();
    assert_eq!(
        &leap[..4],
        &[(0, 0, false), (1, 2, false), (2, 4, false), (3, 0, false)]
    );
    assert_eq!(
        leap[6],
        (0, 1, false),
        "the next orbit starts a column to the right"
    );
    let pairs = |layout: &Layout| -> std::collections::BTreeSet<Vec<(i32, i32)>> {
        layout::path(6, layout)
            .unwrap()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let mut cells = vec![(pair[0].0, pair[0].1), (pair[1].0, pair[1].1)];
                cells.sort();
                cells
            })
            .collect()
    };
    assert!(
        pairs(&Layout::Unwinding)
            .intersection(&pairs(&Layout::Phaistos))
            .next()
            .is_none(),
        "no two signs are read as one pair along both"
    );
    let unwinding = layout::path(6, &Layout::Unwinding).unwrap();
    assert_eq!(unwinding.first().unwrap(), &(2, 2, false));
    assert_eq!(unwinding.last().unwrap(), &(0, 5, false));
}

#[test]
fn at_a_side_of_two_three_of_the_ways_are_one_walk_and_a_one_letter_plate_still_reads() {
    assert_eq!(
        layout::others(&Layout::Leap, 2),
        vec![Layout::Phaistos, Layout::Rongorongo]
    );
    assert_eq!(
        layout::others(&Layout::Unwinding, 4),
        vec![
            Layout::Phaistos,
            Layout::Rongorongo,
            Layout::Leap,
            Layout::Furrows
        ]
    );
    for way in [Layout::Leap, Layout::Furrows, Layout::Unwinding] {
        let (_, drawn) =
            cipher::inscribe("x", None, &way, false, false, None, &mut os_random()).unwrap();
        assert_eq!(cipher::decode(&drawn, None, None).unwrap(), "x", "{way:?}");
    }
}

#[test]
fn a_pathless_plate_lies_in_grid_order_and_still_reads_along_its_way() {
    let (sequence, drawn) = cipher::inscribe(
        "Meet me at the old abbey",
        None,
        &Layout::Leap,
        false,
        true,
        None,
        &mut os_random(),
    )
    .unwrap();
    assert!(pathless::is_pathless(&drawn));
    let (_, plain) = cipher::inscribe(
        "abbey",
        None,
        &Layout::Phaistos,
        false,
        false,
        None,
        &mut os_random(),
    )
    .unwrap();
    assert!(!pathless::is_pathless(&plain));

    let mut cells: Vec<(i32, i32)> = Vec::new();
    for line in svg::lines(&drawn, svg::Layer::Strokes).unwrap() {
        let cell = layout::cell_of(line, &Layout::Phaistos);
        if cells.last() != Some(&cell) && !cells.contains(&cell) {
            cells.push(cell);
        }
    }
    let mut sorted = cells.clone();
    sorted.sort();
    assert_eq!(
        cells, sorted,
        "the file holds the signs row by row, not in reading order"
    );
    assert_eq!(svg::read(&drawn, &Layout::Leap, None).unwrap(), sequence);
    assert_eq!(
        cipher::decode(&drawn, None, None).unwrap(),
        "meet me at the old abbey",
        "the decoder still finds the way"
    );
}

#[test]
fn a_pathless_plate_is_pressed_by_its_cells_not_by_its_way() {
    // The same numbers at the same cells, laid along two ways: the same
    // wedges (seeded by the cell), so the hand's tremor tells nothing of the
    // order
    let numbers: Vec<Value> = (1..=16).collect();
    let grid: std::collections::BTreeMap<(i32, i32), Value> = layout::path(4, &Layout::Leap)
        .unwrap()
        .into_iter()
        .zip(numbers.iter())
        .map(|((row, col, _), &num)| ((row, col), num))
        .collect();
    let along_furrows: Vec<Value> = layout::path(4, &Layout::Furrows)
        .unwrap()
        .into_iter()
        .map(|(row, col, _)| grid[&(row, col)])
        .collect();
    let wedges = |seq: &[Value], way: &Layout, pathless: bool| -> Vec<String> {
        let drawn = svg::render(seq, way, &svg::Damage::new(), false, pathless, None).unwrap();
        drawn
            .lines()
            .filter(|line| line.trim_start().starts_with("<path d=\""))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        wedges(&numbers, &Layout::Leap, true),
        wedges(&along_furrows, &Layout::Furrows, true)
    );
    assert_ne!(
        wedges(&numbers, &Layout::Leap, true),
        wedges(&numbers, &Layout::Leap, false),
        "a plate with its way told is pressed in reading order"
    );
}

#[test]
fn decode_detects_the_layout() {
    let alphabet: Vec<char> = ('a'..='z').chain([' ']).collect();
    let mut random = os_random();
    for way in &layout::KEYLESS {
        for _ in 0..10 {
            let mut message = String::new();
            message.push(alphabet[random.rand_below(26) as usize]);
            let extra = random.rand_below(41) as usize;
            for _ in 0..extra {
                message.push(alphabet[random.rand_below(alphabet.len() as u64) as usize]);
            }
            let (_, drawn) =
                cipher::inscribe(&message, None, way, false, false, None, &mut random).unwrap();
            let expected = cipher::normalize(&message);
            assert_eq!(
                cipher::decode(&drawn, None, None).unwrap(),
                expected,
                "{way:?}"
            );
            assert_eq!(
                cipher::decode(&drawn, None, Some(way)).unwrap(),
                expected,
                "{way:?}"
            );
        }
    }
}

#[test]
fn round_trip_for_random_messages() {
    let alphabet: Vec<char> = ('a'..='z').chain([' ', ' ', ' ']).collect();
    let mut random = os_random();
    for _ in 0..30 {
        let mut message = String::new();
        message.push((b'a' + random.rand_below(26) as u8) as char);
        let extra = random.rand_below(81) as usize;
        for _ in 0..extra {
            message.push(alphabet[random.rand_below(alphabet.len() as u64) as usize]);
        }
        let sequence = cipher::encode(&message, None, false, None, &mut random).unwrap();
        let drawn = cipher::to_svg(&sequence, &Layout::Phaistos, false, false, None).unwrap();
        assert_eq!(
            cipher::decode(&drawn, None, None).unwrap(),
            cipher::normalize(&message)
        );
    }
}

#[test]
fn decode_tolerates_reversed_strokes_and_float_coordinates() {
    let sequence = encode("monk");
    let drawn = cipher::to_svg(&sequence, &Layout::Phaistos, false, false, None).unwrap();
    // Reverse every line's endpoints and write the first coordinate as a float
    let reversed = regex_lite_reverse(&drawn);
    assert_eq!(cipher::decode(&reversed, None, None).unwrap(), "monk");
}

/// Rewrites every `<line x1=".." y1=".." x2=".." y2=".."/>` with its
/// endpoints swapped and a trailing `.0` on the first coordinate, without a
/// regex dependency.
fn regex_lite_reverse(svg_text: &str) -> String {
    svg_text
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("<line ") {
                return line.to_string();
            }
            let attr = |name: &str| -> &str {
                let start = line.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
                let end = start + line[start..].find('"').unwrap();
                &line[start..end]
            };
            let (x1, y1, x2, y2) = (attr("x1"), attr("y1"), attr("x2"), attr("y2"));
            let indent = &line[..line.len() - trimmed.len()];
            format!(r#"{indent}<line x1="{x2}.0" y1="{y2}" x2="{x1}" y2="{y1}"/>"#)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn decode_rejects_input_it_did_not_produce() {
    let tampered = cipher::to_svg(&[10, 9999, 5, 42], &Layout::Phaistos, false, false, None)
        .unwrap()
        .replacen(
            "<line",
            "<line x1=\"1\" y1=\"1\" x2=\"2\" y2=\"2\"/><line",
            1,
        );
    let bad = [
        "".to_string(),
        "hello".to_string(),
        "<html/>".to_string(),
        "<svg".to_string(),
        r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_string(),
        r#"<svg><line x1="a" y1="0" x2="0" y2="0"/></svg>"#.to_string(),
        r#"<svg><line x1="100" y1="50" x2="100" y2="200"/><line x1="100" y1="50" x2="140" y2="90"/></svg>"#
            .to_string(),
        tampered,
    ];
    for svg_text in bad {
        let error = cipher::decode(&svg_text, None, None).unwrap_err();
        assert!(error.is_decode(), "{svg_text:.60}: {error}");
    }
}

#[test]
fn decipher_rejects_sequences_that_are_not_characters() {
    for sequence in [vec![9999], vec![1, 2], vec![10], vec![5, 9999]] {
        let error = cipher::decipher(&sequence, None).unwrap_err();
        assert!(error.is_decode(), "{sequence:?}");
    }
}
