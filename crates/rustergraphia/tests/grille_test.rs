//! The turning grille's behavior, ported from the Ruby suite
//! (test/grille_test.rb); the key-normalization, orbit and abbot/novice
//! basics live in src/grille.rs's module tests.

use rustergraphia::grille::{Grille, Part, EXAMPLE, WRONG_KEY};
use rustergraphia::layout::{self, Layout};
use rustergraphia::{cipher, svg, Error, RubyRandom};

const KEY: &str = "K7Q2M-ZXB4P";

fn grille(key: &str) -> Grille {
    Grille::new(key, Part::Whole).unwrap()
}

#[test]
fn a_malformed_key_says_what_a_key_looks_like() {
    for key in ["", "K7Q2M", "K7Q2M-ZXB4PP", "K7Q2M-ZXB4U", "K7Q2M-ZXB4!"] {
        let error = Grille::new(key, Part::Whole).unwrap_err();
        assert!(error.to_string().contains(EXAMPLE), "{key:?}: {error}");
    }
}

#[test]
fn generated_keys_are_valid_and_different() {
    let keys: Vec<String> = (0..50).map(|_| Grille::generate().to_string()).collect();
    for key in &keys {
        let (half1, half2) = key.split_once('-').unwrap();
        for half in [half1, half2] {
            assert_eq!(half.len(), 5, "{key}");
            assert!(
                half.chars()
                    .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c)),
                "{key}"
            );
        }
    }
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 50);
}

#[test]
fn four_turns_read_every_cell_exactly_once() {
    for size in [2usize, 4, 6, 10, 18, 32] {
        let grille = Grille::generate();
        let mut path = grille.path(size).unwrap();
        assert_eq!(path.len(), size * size);
        path.sort();
        let every: Vec<(i32, i32)> = (0..size as i32)
            .flat_map(|row| (0..size as i32).map(move |col| (row, col)))
            .collect();
        assert_eq!(path, every, "{size} x {size}");
        for turn in 0..4 {
            assert_eq!(grille.holes(size, turn).unwrap().len(), size * size / 4);
        }
    }
}

#[test]
fn turning_the_grille_moves_its_holes_a_quarter_clockwise() {
    let grille = grille(KEY);
    let mut turned: Vec<(i32, i32)> = grille
        .holes(8, 0)
        .unwrap()
        .into_iter()
        .map(|cell| Grille::quarter_turn(cell, 8))
        .collect();
    turned.sort();
    assert_eq!(grille.holes(8, 1).unwrap(), turned);
    // the top-left corner goes to the top-right
    assert_eq!(Grille::quarter_turn((0, 0), 8), (0, 7));
}

#[test]
fn holes_are_read_row_by_row() {
    let holes = grille(KEY).holes(10, 2).unwrap();
    let mut sorted = holes.clone();
    sorted.sort();
    assert_eq!(holes, sorted);
}

#[test]
fn the_key_sets_the_grille_for_every_plate_size() {
    assert_eq!(
        grille(KEY).holes(10, 0).unwrap(),
        grille(KEY).holes(10, 0).unwrap()
    );
    assert_ne!(
        grille(KEY).holes(10, 0).unwrap(),
        grille("K7Q2M-ZXB4Q").holes(10, 0).unwrap()
    );
    // past one digest's 128 holes
    let by_orbit = grille(KEY).holes_by_orbit(32);
    assert_eq!(by_orbit.len(), 256);
    assert!(by_orbit.iter().all(|&hole| hole <= 3));
}

#[test]
fn a_game_plate_reads_back_with_its_grille_and_only_with_it() {
    let layout = Layout::Grille(grille(KEY));
    let (_, drawn) = cipher::inscribe(
        "Meet me at the abbey after vespers",
        None,
        &layout,
        false,
        false,
        None,
        &mut RubyRandom::new_os(),
    )
    .unwrap();
    assert_eq!(
        cipher::decode(&drawn, None, Some(&layout)).unwrap(),
        "meet me at the abbey after vespers"
    );
    assert!(matches!(
        cipher::decode(&drawn, None, None).unwrap_err(),
        Error::GrilleKeyNeeded(_)
    ));
    let wrong = Layout::Grille(grille("00000-00000"));
    let error = cipher::decode(&drawn, None, Some(&wrong)).unwrap_err();
    assert!(matches!(error, Error::WrongGrille(_)));
    assert_eq!(error.to_string(), WRONG_KEY);
}

#[test]
fn a_sealed_plate_under_a_grille_has_an_even_side() {
    let layout = Layout::Grille(grille(KEY));
    for length in [1usize, 30, 60, 120] {
        let message = "x".repeat(length);
        let (sequence, drawn) = cipher::inscribe(
            &message,
            Some("abbey of molesme"),
            &layout,
            false,
            false,
            None,
            &mut RubyRandom::new_os(),
        )
        .unwrap();
        assert_eq!(sequence.len().isqrt() % 2, 0, "{length}");
        assert_eq!(
            cipher::decode(&drawn, Some("abbey of molesme"), Some(&layout)).unwrap(),
            message
        );
    }
}

#[test]
fn a_grille_plate_is_square_and_the_others_are_not() {
    let mut random = RubyRandom::new_os();
    let layout = Layout::Grille(grille(KEY));
    let (_, under_grille) = cipher::inscribe(
        "ora et labora",
        None,
        &layout,
        false,
        false,
        None,
        &mut random,
    )
    .unwrap();
    let (_, snail) = cipher::inscribe(
        "ora et labora",
        None,
        &Layout::Phaistos,
        false,
        false,
        None,
        &mut random,
    )
    .unwrap();
    assert!(svg::is_square(&under_grille));
    assert!(!svg::is_square(&snail));
    let error = cipher::decode(&snail, None, Some(&layout)).unwrap_err();
    assert!(error.is_decode());
    assert_eq!(
        error.to_string(),
        "This plate was not written under a grille"
    );
}

#[test]
fn signs_sit_in_the_middle_of_their_square_cells() {
    let layout = Layout::Grille(grille(KEY));
    // 170 x 170 cells for 100 x 150 signs
    assert_eq!(layout::origin(0, 0, &layout), (50 + 35, 50 + 10));
}

#[test]
fn the_stencil_is_cut_where_the_grille_reads_first() {
    let grille = grille(KEY);
    let layout = Layout::Grille(grille.clone());
    let (sequence, plate) = cipher::inscribe(
        "ora et labora",
        None,
        &layout,
        false,
        false,
        None,
        &mut RubyRandom::new_os(),
    )
    .unwrap();
    let size = sequence.len().isqrt();
    let stencil = svg::stencil(size, &grille).unwrap();

    let width_of = |document: &str| {
        let start = document.find("width=\"").unwrap() + 7;
        document[start..start + document[start..].find('"').unwrap()].to_string()
    };
    assert_eq!(width_of(&plate), width_of(&stencil));

    // The holes in the sheet's path: every M<x> <y>h154 segment
    let holes: Vec<(i32, i32)> = stencil
        .split('M')
        .skip(2) // the document start and the sheet's own outline
        .filter_map(|segment| {
            let (coords, rest) = segment.split_once('h')?;
            if !rest.starts_with("154") {
                return None;
            }
            let (x, y) = coords.split_once(' ')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect();
    let expected: Vec<(i32, i32)> = grille
        .holes(size, 0)
        .unwrap()
        .into_iter()
        .map(|(row, col)| (50 + 170 * col + 8, 50 + 170 * row + 8))
        .collect();
    assert_eq!(holes, expected);
    assert!(stencil.contains(&format!("Grille {KEY}")));
}
