//! Two shortcuts must read exactly what their long way reads: the ambiguity
//! of a new plate from its sequence (not its file), and the fragments of a
//! new broken plate from the signs they were drawn from (not the tray).

use rustergraphia::layout::{Layout, KEYLESS};
use rustergraphia::{along, cipher, fragments, svg, weathering, RubyRandom};

const SHORT: [&str; 8] = ["ab", "to", "me", "we", "ox", "go", "it", "hi"];

#[test]
fn a_new_plate_is_ambiguous_from_its_sequence_exactly_when_its_file_is() {
    let mut ambiguous = 0;
    for seed in 0..1500i64 {
        let layout = &KEYLESS[seed as usize % KEYLESS.len()];
        let pathless = seed % 2 == 1 && *layout != Layout::Phaistos;
        let mut random = RubyRandom::new(seed);
        let sequence = cipher::encode(
            SHORT[seed as usize % SHORT.len()],
            None,
            false,
            None,
            &mut random,
        )
        .unwrap();
        let file = cipher::to_svg(&sequence, layout, false, pathless, None).unwrap();
        let from_file = cipher::unambiguous(&file, layout).unwrap();
        svg::forget();

        assert_eq!(
            from_file,
            along::alone(&sequence, layout).unwrap(),
            "seed {seed} along {layout:?}"
        );
        if !from_file {
            ambiguous += 1;
        }
    }
    assert!(
        ambiguous > 5,
        "the sample holds ambiguous plates to tell apart"
    );
}

#[test]
fn a_broken_plate_is_drawn_from_the_fragments_its_file_gives_back() {
    let messages = [
        "Meet me at the old abbey after vespers",
        "ora et labora et ora",
        "the brothers will gather in the chapter house after compline",
    ];
    for message in messages {
        for seed in 0..30i64 {
            let mut random = RubyRandom::new(seed);
            let sequence = cipher::encode(message, None, false, None, &mut random).unwrap();
            let damage = if seed % 2 == 1 {
                weathering::weather(&sequence, &mut random, None)
            } else {
                svg::Damage::new()
            };
            let pieces = fragments::breaking::cut(sequence.len().isqrt(), &mut random);
            let (file, drawn) =
                fragments::tray::drawn(&sequence, &Layout::Phaistos, &pieces, &mut random, &damage)
                    .unwrap();

            assert_eq!(
                fragments::read(&file).unwrap(),
                drawn,
                "{message:?}, seed {seed}"
            );
            svg::forget();
        }
    }
}
