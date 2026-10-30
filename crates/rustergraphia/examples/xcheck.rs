//! Dev-time cross-check driver for tools/interop-check.sh: writes one plate
//! of every kind (seeded where the Ruby library can draw the same one, for
//! byte-level diffs), and decodes every `ruby-*.svg` the Ruby side wrote.

use rustergraphia::grille::{Grille, Part};
use rustergraphia::layout::Layout;
use rustergraphia::{cipher, fragments, lacunae, two_readers, RubyRandom};

const KEY: &str = "K7Q2M-ZXB4P";
const PASSPHRASE: &str = "monastery gate";

fn grille() -> Grille {
    Grille::new(KEY, Part::Whole).expect("a well-formed key")
}

fn write(out: &str, name: &str, svg: &str) {
    std::fs::write(format!("{out}/rust-{name}.svg"), svg).expect("plate written");
}

fn main() {
    let out = std::env::args().nth(1).expect("output dir");

    // Seeded plates: the Ruby driver draws the same ones, byte for byte
    for (name, layout) in [
        ("phaistos", Layout::Phaistos),
        ("rongorongo", Layout::Rongorongo),
        ("leap", Layout::Leap),
        ("furrows", Layout::Furrows),
        ("unwinding", Layout::Unwinding),
    ] {
        let mut random = RubyRandom::new(1234);
        let (_, svg) = cipher::inscribe(
            "meet me at the old abbey after vespers",
            None,
            &layout,
            false,
            false,
            None,
            &mut random,
        )
        .expect("an intact plate");
        write(&out, name, &svg);
    }

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
    .expect("a pathless plate");
    write(&out, "pathless", &svg);

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
    .expect("a plate with its own tablet");
    write(&out, "tablet", &svg);

    let mut random = RubyRandom::new(55);
    let (_, svg) = cipher::inscribe(
        "the relics are under the third stone",
        None,
        &Layout::Grille(grille()),
        false,
        false,
        None,
        &mut random,
    )
    .expect("a grille plate");
    write(&out, "grille", &svg);

    let mut random = RubyRandom::new(42);
    let (_, svg, _) = lacunae::inscribe(
        "ora et labora et ora",
        &Layout::Rongorongo,
        None,
        false,
        false,
        &mut random,
    )
    .expect("a weathered plate");
    write(&out, "weathered", &svg);

    let mut random = RubyRandom::new(43);
    let (_, svg, _) = lacunae::inscribe(
        "ora et labora et ora",
        &Layout::Phaistos,
        None,
        true,
        false,
        &mut random,
    )
    .expect("a weathered plate with tally signs");
    write(&out, "weathered-tally", &svg);

    let mut random = RubyRandom::new(44);
    let (_, svg, _) = lacunae::inscribe(
        "ora et labora et ora",
        &Layout::Leap,
        None,
        false,
        true,
        &mut random,
    )
    .expect("a weathered pathless plate");
    write(&out, "weathered-pathless", &svg);

    let mut random = RubyRandom::new(31);
    let (_, svg, _, _) = fragments::inscribe(
        "meet me at the old abbey after vespers",
        &Layout::Phaistos,
        false,
        &mut random,
    )
    .expect("a broken plate");
    write(&out, "broken", &svg);

    let mut random = RubyRandom::new(3);
    let (_, svg, _, _) = fragments::inscribe(
        "meet me at the old abbey after vespers",
        &Layout::Phaistos,
        true,
        &mut random,
    )
    .expect("a ruined plate");
    write(&out, "ruined", &svg);

    // Unseeded plates (salts and keys from the operating system): the other
    // side proves it can read them
    let (_, svg, _) = two_readers::inscribe(
        "the relics are under the third stone",
        "vespers at six",
        None,
        Some(&grille()),
    )
    .expect("a plate with two readers");
    write(&out, "two", &svg);

    let (_, svg, _) = two_readers::inscribe(
        "the relics are under the third stone",
        "vespers at six",
        Some(PASSPHRASE),
        Some(&grille()),
    )
    .expect("a sealed plate with two readers");
    write(&out, "two-sealed", &svg);

    let mut random = RubyRandom::new_os();
    let (_, svg) = cipher::inscribe(
        "In girum imus nocte",
        Some(PASSPHRASE),
        &Layout::Phaistos,
        false,
        false,
        None,
        &mut random,
    )
    .expect("a sealed plate");
    write(&out, "sealed", &svg);

    // Decode back what Ruby wrote, if present
    let mut entries: Vec<_> = std::fs::read_dir(&out)
        .expect("output dir readable")
        .map(|entry| entry.expect("entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.starts_with("ruby-") || !name.ends_with(".svg") {
            continue;
        }
        let svg = std::fs::read_to_string(&path).expect("plate readable");
        if name.contains("two") {
            let passphrase = name.contains("sealed").then_some(PASSPHRASE);
            match two_readers::read(&svg, &grille(), passphrase) {
                Ok(reading) => println!(
                    "rust decodes {name}: {} / decoy: {}",
                    reading.message,
                    reading.decoy.as_deref().unwrap_or("-")
                ),
                Err(error) => println!("rust FAILS on {name}: {error}"),
            }
            continue;
        }
        let layout = name.contains("grille").then(|| Layout::Grille(grille()));
        let passphrase = name.contains("sealed").then_some(PASSPHRASE);
        match cipher::decode(&svg, passphrase, layout.as_ref()) {
            Ok(message) => println!("rust decodes {name}: {message}"),
            Err(error) => println!("rust FAILS on {name}: {error}"),
        }
    }
}
