//! Dev-time cross-check driver: writes seeded plates for the interop diff.
use rustergraphia::layout::Layout;
use rustergraphia::{cipher, RubyRandom};

fn main() {
    let out = std::env::args().nth(1).expect("output dir");
    let layouts = [
        ("phaistos", Layout::Phaistos),
        ("rongorongo", Layout::Rongorongo),
        ("leap", Layout::Leap),
        ("furrows", Layout::Furrows),
        ("unwinding", Layout::Unwinding),
    ];
    for (name, layout) in &layouts {
        let mut random = RubyRandom::new(1234);
        let (_, svg) = cipher::inscribe(
            "meet me at the old abbey after vespers",
            None,
            layout,
            false,
            false,
            None,
            &mut random,
        )
        .unwrap();
        std::fs::write(format!("{out}/rust-{name}.svg"), svg).unwrap();
    }
    // pathless, own tablet, tally would go here as their waves land
    let mut random = RubyRandom::new(77);
    let (_, svg) = cipher::inscribe(
        "vespers at six",
        None,
        &Layout::Phaistos,
        false,
        true,
        None,
        &mut random,
    )
    .unwrap();
    std::fs::write(format!("{out}/rust-pathless.svg"), svg).unwrap();

    let mut random = RubyRandom::new(99);
    let (_, svg) = cipher::inscribe(
        "ora et labora",
        None,
        &Layout::Phaistos,
        false,
        false,
        Some("ABCDEFGH23"),
        &mut random,
    )
    .unwrap();
    std::fs::write(format!("{out}/rust-tablet.svg"), svg).unwrap();

    // a grille plate (seeded splits; the grille itself is keyed, not random)
    let grille =
        rustergraphia::grille::Grille::new("K7Q2M-ZXB4P", rustergraphia::grille::Part::Whole)
            .unwrap();
    let mut random = RubyRandom::new(55);
    let (_, svg) = cipher::inscribe(
        "the relics are under the third stone",
        None,
        &Layout::Grille(grille),
        false,
        false,
        None,
        &mut random,
    )
    .unwrap();
    std::fs::write(format!("{out}/rust-grille.svg"), svg).unwrap();

    // decode back what Ruby wrote, if present
    for entry in std::fs::read_dir(&out).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with("ruby-") || !name.ends_with(".svg") {
            continue;
        }
        let svg = std::fs::read_to_string(&path).unwrap();
        let layout = if name.contains("grille") {
            Some(Layout::Grille(
                rustergraphia::grille::Grille::new(
                    "K7Q2M-ZXB4P",
                    rustergraphia::grille::Part::Whole,
                )
                .unwrap(),
            ))
        } else {
            None
        };
        let passphrase = if name.contains("secret") {
            Some("monastery gate")
        } else {
            None
        };
        match cipher::decode(&svg, passphrase, layout.as_ref()) {
            Ok(message) => println!("rust decodes {name}: {message}"),
            Err(error) => println!("rust FAILS on {name}: {error}"),
        }
    }
}
