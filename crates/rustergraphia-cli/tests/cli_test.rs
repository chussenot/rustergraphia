//! The command line, run as a reader would: every kind of plate written and
//! read back through the file, and the refusals with their exit codes
//! (ported from test/cli_test.rb).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const MESSAGE: &str = "ora et labora";

struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rustergraphia-cli-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    fn plate(&self, name: &str) -> String {
        self.0
            .join(format!("{name}.svg"))
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// `(stdout, stderr, exit status)`, run from the test's own directory so
/// default outputs (./tmp) never land in the repository.
fn cli_in(dir: &Path, args: &[&str], env: &[(&str, &str)], stdin: &str) -> (String, String, i32) {
    use std::io::Write;
    let mut command = Command::new(env!("CARGO_BIN_EXE_rustergraphia"));
    command
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let Output {
        status,
        stdout,
        stderr,
    } = child.wait_with_output().unwrap();
    (
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
        status.code().unwrap_or(-1),
    )
}

fn cli(dir: &Dir, args: &[&str]) -> (String, String, i32) {
    cli_in(&dir.0, args, &[], "")
}

/// Whether a message is what a Leiden edition allows it to be: text outside
/// brackets must match, `[r]` was restored, `[b/l]` is either.
fn allowed(edition: &str, message: &str) -> bool {
    let edition = edition.trim();
    let mut rest = message.chars();
    let mut chars = edition.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut set = String::new();
            for inner in chars.by_ref() {
                if inner == ']' {
                    break;
                }
                if inner != '/' {
                    set.push(inner);
                }
            }
            match rest.next() {
                Some(actual) if set.contains(actual) => {}
                _ => return false,
            }
        } else if rest.next() != Some(c) {
            return false;
        }
    }
    rest.next().is_none()
}

fn between(text: &str, prefix: &str, terminator: char) -> Option<String> {
    let start = text.find(prefix)? + prefix.len();
    let end = start + text[start..].find(terminator)?;
    Some(text[start..end].to_string())
}

#[test]
fn a_plate_is_written_and_read_back() {
    let dir = Dir::new();
    let (_, _, status) = cli(&dir, &["-m", MESSAGE, "-o", &dir.plate("intact")]);
    assert_eq!(status, 0);
    assert!(std::fs::read_to_string(dir.plate("intact"))
        .unwrap()
        .contains("<svg"));
    let (out, _, status) = cli(&dir, &["-d", &dir.plate("intact")]);
    assert_eq!((status, out.trim()), (0, MESSAGE));
}

#[test]
fn every_way_is_read_back_without_being_told_its_layout() {
    let dir = Dir::new();
    for way in ["rongorongo", "leap", "furrows", "unwinding"] {
        cli(&dir, &["-m", MESSAGE, "-l", way, "-o", &dir.plate(way)]);
        let (out, _, status) = cli(&dir, &["-d", &dir.plate(way)]);
        assert_eq!((status, out.trim()), (0, MESSAGE), "{way}");
    }
}

#[test]
fn a_pathless_plate_is_announced_read_back_and_refused_where_it_cannot_be() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "-l",
            "unwinding",
            "--pathless",
            "-o",
            &dir.plate("pathless"),
        ],
    );
    assert_eq!(status, 0);
    assert!(out.starts_with("Pathless: the plate says nothing of where it begins"));
    assert!(std::fs::read_to_string(dir.plate("pathless"))
        .unwrap()
        .contains(r#"class="pathless""#));
    let (out, _, status) = cli(&dir, &["-d", &dir.plate("pathless")]);
    assert_eq!((status, out.trim()), (0, MESSAGE));

    let (_, err, status) = cli(
        &dir,
        &["-m", MESSAGE, "--pathless", "-b", "-o", &dir.plate("no")],
    );
    assert_eq!(
        (status, err.trim()),
        (1, rustergraphia::pathless::NOT_BROKEN)
    );
    let (_, err, status) = cli_in(
        &dir.0,
        &["-m", MESSAGE, "--pathless", "-s", "-o", &dir.plate("no")],
        &[("CISTERGRAPHIA_PASSPHRASE", "abbey of molesme")],
        "",
    );
    assert_eq!(
        (status, err.trim()),
        (1, rustergraphia::pathless::NOT_SEALED)
    );
}

#[test]
fn a_plate_with_its_own_tablet_shows_the_scribe_the_tablet_and_reads_back_by_its_seed() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "-l",
            "leap",
            "--own-tablet",
            "-o",
            &dir.plate("own"),
        ],
    );
    assert_eq!(status, 0);
    let seed = between(&out, "Its own tablet (seed ", ')').expect("the seed is shown");
    assert_eq!(seed.len(), 11); // XXXXX-XXXXX
                                // the twenty-seven rows: one 4-digit value per character
    let values = out
        .split_whitespace()
        .filter(|token| token.len() == 4 && token.chars().all(|c| c.is_ascii_digit()))
        .count();
    assert_eq!(values, 27);
    assert_eq!(
        rustergraphia::tablet::seed_of(&std::fs::read_to_string(dir.plate("own")).unwrap()),
        Some(rustergraphia::tablet::normalize(&seed).unwrap())
    );
    let (out, _, status) = cli(&dir, &["-d", &dir.plate("own")]);
    assert_eq!((status, out.trim()), (0, MESSAGE));

    let (_, err, status) = cli(
        &dir,
        &["-m", MESSAGE, "--own-tablet", "-w", "-o", &dir.plate("no")],
    );
    assert_eq!(
        (status, err.trim()),
        (1, rustergraphia::tablet::NOT_DAMAGED)
    );
}

#[test]
fn the_expert_plate_is_pathless_with_its_own_tablet_and_reads_back() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "-l",
            "furrows",
            "--pathless",
            "--own-tablet",
            "-o",
            &dir.plate("expert"),
        ],
    );
    assert_eq!(status, 0);
    assert!(out.contains("Pathless: the plate says nothing of where it begins"));
    assert!(out.contains("Its own tablet (seed "));
    let (out, _, status) = cli(&dir, &["-d", &dir.plate("expert")]);
    assert_eq!((status, out.trim()), (0, MESSAGE));
}

#[test]
fn a_worn_pathless_plate_is_announced_twice_and_reads_back_as_an_edition() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "-l",
            "furrows",
            "-w",
            "--pathless",
            "-o",
            &dir.plate("worn-pathless"),
        ],
    );
    assert_eq!(status, 0);
    assert!(
        out.starts_with("Weathered: ") && out.contains("lacuna"),
        "{out}"
    );
    assert!(out
        .contains("Pathless: the plate says nothing of where it begins; the bench finds the way"));
    let svg = std::fs::read_to_string(dir.plate("worn-pathless")).unwrap();
    assert!(svg.contains(r#"class="pathless""#));
    assert!(svg.contains(r#"class="lacunae""#));
    let (edition, _, status) = cli(&dir, &["-d", &dir.plate("worn-pathless")]);
    assert_eq!(status, 0);
    assert!(
        allowed(&edition, MESSAGE),
        "the message, within what the edition allows: {edition}"
    );
}

#[test]
fn a_worn_plate_says_its_lacunae_and_reads_back_as_an_edition() {
    let dir = Dir::new();
    let (out, _, status) = cli(&dir, &["-m", MESSAGE, "-w", "-o", &dir.plate("worn")]);
    assert_eq!(status, 0);
    assert!(
        out.starts_with("Weathered: ") && out.contains(" lacunae. Restore them"),
        "{out}"
    );
    let (edition, _, status) = cli(&dir, &["-d", &dir.plate("worn")]);
    assert_eq!(status, 0);
    assert!(
        edition.contains('[') && edition.contains(']'),
        "restorations in brackets: {edition}"
    );
    assert!(allowed(&edition, MESSAGE), "{edition}");
}

#[test]
fn a_broken_plate_says_its_fragments_and_is_read_back_rejoined() {
    let dir = Dir::new();
    let (out, _, status) = cli(&dir, &["-m", MESSAGE, "-b", "-o", &dir.plate("broken")]);
    assert_eq!(status, 0);
    assert!(
        out.starts_with("Broken: ") && out.contains(" fragments. Rejoin them"),
        "{out}"
    );
    let (message, _, status) = cli(&dir, &["-d", &dir.plate("broken")]);
    assert_eq!((status, message.trim()), (0, MESSAGE));
}

#[test]
fn a_tallied_worn_plate_reads_back() {
    let dir = Dir::new();
    cli(
        &dir,
        &["-m", MESSAGE, "-t", "-w", "-o", &dir.plate("tallied")],
    );
    let (edition, _, status) = cli(&dir, &["-d", &dir.plate("tallied")]);
    assert_eq!(status, 0);
    assert!(allowed(&edition, MESSAGE), "{edition}");
}

#[test]
fn a_grille_plate_gives_its_key_and_needs_it_to_read() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "-l",
            "grille",
            "-o",
            &dir.plate("grille"),
            "--stencil",
            &dir.plate("stencil"),
        ],
    );
    assert_eq!(status, 0);
    let key = between(&out, "Grille key: ", ' ').expect("the key is shown");
    assert_eq!(key.len(), 11, "{key}");
    assert!(std::fs::read_to_string(dir.plate("stencil"))
        .unwrap()
        .contains("stencil-sheet"));
    let (message, _, status) = cli(&dir, &["-d", &dir.plate("grille"), "-g", &key]);
    assert_eq!((status, message.trim()), (0, MESSAGE));
}

#[test]
fn a_grille_plate_without_its_key_is_refused_with_the_way_in() {
    let dir = Dir::new();
    cli(
        &dir,
        &["-m", MESSAGE, "-l", "grille", "-o", &dir.plate("grille")],
    );
    let (_, err, status) = cli(&dir, &["-d", &dir.plate("grille")]);
    assert_eq!(status, 1);
    assert!(
        err.contains("written under a grille: it needs its key. Add --grille KEY"),
        "{err}"
    );
}

#[test]
fn two_readers_the_abbot_reads_both_messages_and_the_novice_the_decoy() {
    let dir = Dir::new();
    let (out, _, status) = cli(
        &dir,
        &[
            "-m",
            MESSAGE,
            "--decoy",
            "vespers at six",
            "-o",
            &dir.plate("two"),
        ],
    );
    assert_eq!(status, 0);
    let abbot = between(&out, "Abbot's key: ", ' ').expect("the abbot's key");
    let novice = between(&out, "Novice's key: ", ' ').expect("the novice's key");
    let (both, _, _) = cli(&dir, &["-d", &dir.plate("two"), "-g", &abbot]);
    assert!(both.contains(MESSAGE), "{both}");
    assert!(both.contains("vespers at six"), "{both}");
    let (only, _, _) = cli(&dir, &["-d", &dir.plate("two"), "-g", &novice]);
    assert!(only.contains("vespers at six"), "{only}");
    assert!(!only.contains(MESSAGE), "{only}");
}

#[test]
fn a_sealed_plate_opens_with_its_passphrase_and_refuses_another() {
    let dir = Dir::new();
    let env = [("CISTERGRAPHIA_PASSPHRASE", "abbey of molesme")];
    let (_, _, status) = cli_in(
        &dir.0,
        &["-m", MESSAGE, "-s", "-o", &dir.plate("sealed")],
        &env,
        "",
    );
    assert_eq!(status, 0);
    let (message, _, status) = cli_in(&dir.0, &["-d", &dir.plate("sealed"), "-s"], &env, "");
    assert_eq!((status, message.trim()), (0, MESSAGE));
    let (_, err, status) = cli_in(
        &dir.0,
        &["-d", &dir.plate("sealed"), "-s"],
        &[("CISTERGRAPHIA_PASSPHRASE", "wrong passphrase")],
        "",
    );
    assert_eq!(status, 1);
    assert!(err.contains("Wrong passphrase"), "{err}");
}

#[test]
fn a_file_that_is_not_a_plate_is_refused_cleanly() {
    let dir = Dir::new();
    std::fs::write(dir.plate("not"), "hello").unwrap();
    let (out, err, status) = cli(&dir, &["-d", &dir.plate("not")]);
    assert_eq!(status, 1);
    assert!(err.contains("Unreadable SVG"), "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(!err.contains("panicked"), "no backtrace: {err}");
}

#[test]
fn a_missing_message_is_refused() {
    let dir = Dir::new();
    let (_, err, status) = cli_in(&dir.0, &[], &[], "");
    assert_eq!(status, 1);
    assert!(
        err.contains("Message has no characters that can be enciphered"),
        "{err}"
    );
}

#[test]
fn help_lists_every_option() {
    let dir = Dir::new();
    let (out, _, status) = cli(&dir, &["--help"]);
    assert_eq!(status, 0);
    for option in [
        "--message",
        "--output",
        "--png",
        "--decode",
        "--layout",
        "--grille",
        "--stencil",
        "--decoy",
        "--weathered",
        "--broken",
        "--tally",
        "--secret",
        "--pathless",
        "--own-tablet",
    ] {
        assert!(out.contains(option), "{option} missing from help");
    }
}
