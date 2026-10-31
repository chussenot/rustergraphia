//! Cistergraphia: About the ciphers (fol. 4r) — views/about.erb.

use crate::esc::h;
use crate::figures::{
    boustrophedon_svg, cardano_svg, disc_spiral_svg, grille_order_svg, snail_svg,
};
use crate::view::{glyph_svg, GLYPH_TABLE_JSON};

/// Lectio's first sign: the page draws one at random per request, by design.
fn first_sign() -> u32 {
    let mut byte = [0u8; 1];
    getrandom(&mut byte);
    1 + u32::from(byte[0]) % 9
}

fn getrandom(buf: &mut [u8]) {
    // The web crate avoids a direct getrandom dependency: a timing fallback
    // would be wrong here, so reuse the core library's source through Grille
    // generation? No — read /dev/urandom portably via std.
    use std::io::Read;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(buf))
        .expect("operating system randomness");
}

pub fn render_about() -> String {
    let first = first_sign();
    let tablet: String = rustergraphia::TABLET
        .iter()
        .map(|&(c, value)| {
            let shown = value * 10;
            let name = if c == ' ' { "space".to_string() } else { c.to_string() };
            let letter = if c == ' ' { '␣' } else { c };
            format!(
                "        <li>\n          {}\n          <b>{letter}</b>\n          <span>{shown}</span>\n        </li>\n",
                glyph_svg(shown, "glyph", Some(&format!("{name}: {shown}")))
            )
        })
        .collect();
    format!(
        r#"<div class="spread spread--about">
  <article class="leaf" aria-labelledby="about-title">
    <p class="folio-mark" aria-hidden="true">fol. 4<sup>r</sup></p>
    <h1 id="about-title" class="rubric">About the ciphers</h1>
    <p class="lede">A forgotten number notation of the Middle Ages, and the game built on it.</p>

    <section class="prose" aria-labelledby="monks-title">
      <h2 id="monks-title" class="index-heading">The ciphers of the monks</h2>
      <p>In the early thirteenth century, about when Arabic numerals reached northwestern Europe, the
        Cistercian order began writing numbers with a single stroke and four corners. The idea apparently came from
        a two-place notation (1 to 99) introduced by John of Basingstoke, archdeacon of Leicester. Its
        earliest known form, in the monasteries of Hainaut, stopped at 99; it soon grew to four places:
        any number up to 9,999 in one sign.</p>
      <p>The monks used the signs to <em>number</em>, never to <em>count</em>: for years, the leaves
        of a book, text divisions, indexes and concordances, Easter tables, even the lines of musical
        staves, but not for arithmetic or accounts. About two dozen manuscripts survive, from the 13th to
        the 15th century, from England to Italy and from Normandy to Sweden.</p>
      <p>Beyond the cloister, a 14th-century astrolabe from Picardy carries them, and the wine-gaugers
        of Bruges still used them in the 18th century. Later the signs were taken up by an occultist
        (Agrippa), by a Parisian Rosicrucian order, and Nazi ideologues considered them as supposed "Aryan"
        symbolism. Their modern rediscovery is largely the work of David A. King.</p>
      <p class="note">The Cistercians most often drew the stave horizontally. The upright stave used here
        is attested in northern France in the 14th and 15th centuries, and in the later revivals.</p>
    </section>

    <section aria-labelledby="timeline-title">
      <h2 id="timeline-title" class="index-heading">Index of dates</h2>
      <ol class="timeline">
        <li>{glyph_1098}<p><b>1098</b> Robert of Molesme founds the abbey of Cîteaux, the cradle of the Cistercian order.</p></li>
        <li><span class="timeline-mark" aria-hidden="true">—</span><p><b>13th century</b> The ciphers spread through Cistercian houses, first up to 99, then up to 9,999.</p></li>
        <li><span class="timeline-mark" aria-hidden="true">—</span><p><b>14th century</b> The Berselius astrolabe, made in Picardy, bears the signs.</p></li>
        <li>{glyph_1533}<p><b>1533</b> Agrippa describes them in his <em>Three Books of Occult Philosophy</em>.</p></li>
        <li><span class="timeline-mark" aria-hidden="true">—</span><p><b>18th century</b> Wine-gaugers around Bruges still use them, at least into the early 1700s.</p></li>
        <li>{glyph_2001}<p><b>2001</b> David A. King publishes <em>The Ciphers of the Monks</em>, the major study of the signs.</p></li>
      </ol>
    </section>

    <section class="layouts-origin" id="layouts" aria-labelledby="layouts-title">
      <h2 id="layouts-title" class="index-heading">Six ways to read a plate</h2>
      <p>A plate is laid out along one of six reading paths. Two take their movement from an
        undeciphered script that turns as it is read: a clay disc from Crete, and the wooden tablets of
        Rapa Nui. Three are ways a text can run over a grid: a cipher’s diagonal step that took fifty-one
        years to find, the ploughman’s turn, and a spiral read from its centre. The last comes from the
        cipher-makers themselves: a pierced sheet that turns over the page. The decoder tries the first
        five and keeps the one that reads; the grille needs its key.</p>

      <h3 class="layout-title">Phaistos: the spiral</h3>
      <div class="turnings">
        <figure>
          {disc_spiral}
          <figcaption><b>The Phaistos Disc, Crete.</b> A spiral on each face, conventionally read clockwise
            from the rim to the centre.</figcaption>
        </figure>
        <figure>
          {snail_phaistos}
          <figcaption><b>The Phaistos layout.</b> The same movement, clockwise and inward, set on the square
            grid of an index: from the top-left corner, coiling to the centre.</figcaption>
        </figure>
      </div>
      <p>The Phaistos Disc is a clay disc of the Minoan Bronze Age, found by Luigi Pernier on 3 July 1908,
        while excavating the Minoan palace site of Phaistos. Its signs were stamped, one by one, in a
        spiral on both faces. Arthur Evans first thought it should be read from the centre outward, then
        from the rim inward, clockwise, which is the convention now used to number its signs. It has not
        been deciphered. An Etruscan lead disc from Magliano also writes its text in a spiral, from the
        edge toward the centre.</p>

      <h3 class="layout-title">Rongorongo: the turning tablet</h3>
      <div class="turnings">
        <figure>
          {boustrophedon}
          <figcaption><b>Rongorongo, Rapa Nui.</b> Read from the bottom left, line by line, left to right.
            Between lines the reader turns the tablet half round, so every other line stands upside down:
            <em>reverse boustrophedon</em>.</figcaption>
        </figure>
        <figure>
          {snail_rongorongo}
          <figcaption><b>The Rongorongo layout.</b> Rows from the bottom left; every other row is written
            upside down (the hollow points), so you turn the plate half round to read it, like a tablet.</figcaption>
        </figure>
      </div>
      <p>About two dozen wooden objects carry rongorongo, first reported by Eugène Eyraud in 1864. The
        script is undeciphered, and many scholars think it is proto-writing rather than true writing. Its
        reader turns the text at every line, and so does the reader of a Rongorongo plate.</p>

      <h3 class="layout-title">Leap: the Zodiac’s step</h3>
      <div class="turnings">
        <figure>
          {snail_leap}
          <figcaption><b>The Leap layout.</b> One row down and two columns across at every sign, wrapping at
            the edge; when the orbit closes, after one sign in every row, the next starts a column to the right.</figcaption>
        </figure>
      </div>
      <p>In November 1969 the Zodiac killer mailed a cipher of 340 characters to the <em>San Francisco
        Chronicle</em>. It was read in December 2020, by David Oranchak, Sam Blake and Jarl Van Eycke, and
        the FBI confirmed their solution: a homophonic substitution, one letter under several symbols,
        laid along a diagonal, one row down and two columns across, wrapping at the edge, in three
        sections. They found the order by machine, trying some 650,000 candidate ways of reading the
        grid. A plate’s pairs are homophones too, since a letter is any two signs that add to its value;
        the leap lends them the Zodiac’s step.</p>

      <h3 class="layout-title">Furrows: the ploughman’s turn</h3>
      <div class="turnings">
        <figure>
          {snail_furrows}
          <figcaption><b>The Furrows layout.</b> Down the first column, up the next, from the left; no sign
            turned.</figcaption>
        </figure>
      </div>
      <p><em>Boustrophedon</em> is Greek for “as the ox turns”: at the end of a line the next runs back the
        other way, as a ploughman turns at the end of a furrow. Archaic Greek inscriptions were often cut
        so; the law code of Gortyn, on Crete, in the fifth century BC, runs boustrophedon down twelve
        columns of stone. The furrows take the turn but not the line: where the turning tablet climbs the
        rows, the furrows walk the columns.</p>

      <h3 class="layout-title">Unwinding: from the centre</h3>
      <div class="turnings">
        <figure>
          {snail_unwinding}
          <figcaption><b>The Unwinding layout.</b> From the centre outward, counter-clockwise, to the
            top-right corner.</figcaption>
        </figure>
      </div>
      <p>Arthur Evans first proposed reading the Phaistos Disc from the centre outward, before settling
        on the rim inward. The unwinding keeps his first thought. It is not the snail backwards: the
        snail read in reverse pairs the same signs and gives the message in reverse. The unwinding is
        the snail turned a quarter and read backwards, so it pairs none of them.</p>

      <h3 class="layout-title">Grille: the turning parchment</h3>
      <div class="turnings">
        <figure>
          {cardano}
          <figcaption><b>Cardano’s grille, 1550.</b> A pierced sheet laid on the paper: the secret is written
            through its openings, then an ordinary letter around it.</figcaption>
        </figure>
        <figure>
          {grille_order}
          <figcaption><b>The Grille layout.</b> A turning grille: read the signs under its holes row by row,
            then turn it a quarter clockwise. At N, E, S and W its holes uncover every sign once.</figcaption>
        </figure>
      </div>
      <p>In 1550 Girolamo Cardano proposed a pierced sheet for private letters: write the secret through its
        openings, lift it, and write an ordinary letter around the words, so that the whole does not look like
        a cipher at all. A square grille that turns came later. The mathematician C. F. Hindenburg studied it in
        1796, and Eduard Fleissner von Wostrowitz’s <em>Handbuch der Kryptographie</em> (1881) gave it the name
        it still carries. Laid on the page and turned a quarter at a time, its holes uncover every cell exactly
        once. Jules Verne built <em>Mathias Sandorf</em> (1885) around one, six squares a side. The German army
        issued them in 1916 and withdrew them after four months: they were too easy to break.</p>
      <p>A Grille plate is written row by row, like a page, in the order its grille reads it: without the grille
        it is an anagram of its own message. Each plate gets a grille from a key of ten letters and digits, shown
        with the plate, and a stencil to print and cut. Lay it on the printed plate with N at the top, read
        the holes, and turn it. It is a puzzle with a key, not a seal: given only the images, a model read ten
        short Grille plates in about twenty-six minutes, ten times longer than without a grille, and a script
        that knows the rules but not the key reads a short one in seconds, and long ones once a key is used twice (<a href="https://github.com/chussenot/cistergraphia/blob/master/docs/security-study.md#experiment-3-the-grille">the experiment</a>).
        Secrecy is the Secret mode’s.</p>
      <p>Cardano’s letter had two readers: whoever saw only the ordinary letter, and whoever held the sheet. A
        Grille plate can have two as well. Give it a second message and its cells are shared out: the abbot’s
        grille reads the real message, the novice’s, pierced where the abbot’s is not, reads the other. The
        novice’s key is drawn from the abbot’s, so the abbot reads both and the novice only one. It is no
        hiding place: the novice’s grille leaves half the plate unread, so its holder knows there is more.</p>

      <p>You choose the layout when you inscribe. The decoder recognises the first two on its own, and asks
        for a grille’s key.</p>
    </section>

    <section aria-labelledby="modes-title">
      <h2 id="modes-title" class="index-heading">Cistergraphia, in two modes</h2>
      <div class="mode-options">
        <div class="mode mode--game mode--static">
          <span class="mode-title">Game</span>
          <span class="mode-desc">Letters become numbers from the tablet, each split into two random signs, padded and
            coiled into a snail. A puzzle: a model given only the images read ten messages out of ten in minutes.</span>
        </div>
        <div class="mode mode--secret mode--static">
          <span class="mode-title">Secret</span>
          <span class="mode-desc">The message is encrypted (scrypt + AES-256-GCM) and the bytes are written as the same
            signs. Without the passphrase, the image reveals only its length.</span>
        </div>
      </div>
      <p>A game plate may carry <b>a tablet of its own</b>: the same values dealt out again, so that a
        pair still adds to a number on the tablet and the number means a letter only on this plate. Its
        reader builds the tablet in the notebook, sum by sum, as the Copiale cipher was read in 2011:
        a 105-page manuscript of some 75,000 characters in about ninety symbols, whose readers counted,
        grouped the symbols by the company they kept, tried the Roman letters as the text and found them
        to be the spaces. The split has done the grouping here: every pair that adds to one sum is one
        symbol, however it is spelled, and a reader who sees two spellings of one sum has seen the
        homophony that hid the Copiale for two centuries. The scribe confirms entries three at a time,
        never one: a rule borrowed from <i>Return of the Obra Dinn</i>, so that a guess cannot be cycled
        until it sticks.</p>
      <p class="note">The experiments, the reasoning and the limits are in the
        <a href="https://github.com/chussenot/cistergraphia/blob/master/docs/security-study.md">security study</a>.</p>
    </section>

    <section aria-labelledby="damaged-title">
      <h2 id="damaged-title" class="index-heading">Damaged plates: worn, broken, ruined</h2>
      <p>Most old records reach us damaged. The Inka kept accounts on knotted cords, khipus, where
        the values of some cords reappear as their sums on others: in 2005 Gary Urton and Carrie Brezine
        read seven khipus from Puruchuco as linked this way, across three levels of administration. Such
        arithmetic outlives damage. In 2024 Manuel Medrano and Ashok Khosla rejoined two fragments of a
        broken khipu from arithmetic relations between its cords.</p>
      <p>A Cistergraphia plate carries the same kind of check. The two signs of a pair must add up to a
        value on the tablet. A worn plate has lost some strokes, hatched where its surface is
        broken. Restore them so that every pair balances. Often only one digit fits. Sometimes two or three
        letters fit, and the words decide: <em>la[b/l]ora</em> can only be <em>labora</em>. Ask the
        decoder and it prints its edition the way epigraphers have since the Leiden conventions
        of 1931: <em>[r]</em> for a restored letter, <em>[b/l]</em> for a doubt left to the reader. Under a
        worn plate, the bench lets you press the lost digits back in and weighs each pair as you go.</p>
      <p>A plate made at the command line (<code>--tally</code>) can also carry tally signs, as the khipus carried their sums: one at the end of every row,
        one under every column, in rubric past a ruled cord. Each place is added on its own, without carrying,
        the way a monk could check a column by eye; the khipus kept true sums, and this is a departure from
        them. With tally signs a worn plate can lose whole signs: a row with one sign missing gives it
        back.</p>
      <p>A plate can also come broken, in two to four fragments laid in a tray, some turned half round,
        under an empty board the size of the plate. The breaks run straight along the grid and every edge is
        worn, so the shapes alone fit several ways (32, typically). The sums tell them apart: of all the ways
        the fragments cover the board, only one balances every pair, as the arithmetic of Medrano and Khosla
        showed which fragments of a khipu belonged together. Only a Phaistos plate breaks: the snail crosses
        every break, while the rows of a Rongorongo plate keep their pairs to themselves, and the whole plate
        turned half round would balance just as well. A ruined plate is broken and worn: then a worn sign
        counts as every digit it may have held, the join still holds only one way, and the plate, whole again,
        opens on the bench to be restored.</p>
      <p>Only a game plate can wear or break. A sealed plate that loses one stroke would never open again.</p>
    </section>

    <section aria-labelledby="scriptorium-title">
      <h2 id="scriptorium-title" class="index-heading">The scriptorium</h2>
      <p>A plate is written to be sent. The scribe inscribes it, chooses its path and its condition, and
        downloads it; the reader brings the file to Read, and the page recognises what it is from the
        file alone. A sealed plate asks for its passphrase. A game plate opens as its puzzle, in the
        scriptorium, where the reader works at one of three pieces of furniture.</p>
      <p><b>Intact</b> plates are read at the desk, pair by pair: the two signs shown large, their numbers
        typed, their sum found on the tablet, the text growing below. Guided, a tap on a sign shows its
        value; unaided, a wrong reading is marked and never corrected, and three right pairs in a row earn
        the ranks of Lectio. A <b>pathless</b> plate, its way withheld by the scribe, asks first which of the
        five ways it runs: lay one, and the pairs form along it and read as letters, or break; the notebook
        under the desk keeps what each way gave and where it broke. <b>Worn</b> plates are restored at the bench: each lacuna takes a digit, each
        pair is weighed, and the words settle what the balance cannot; worn and pathless, the bench asks the
        way first, and the edition along a wrong way shows pairs no digit can balance. A plate with
        <b>a tablet of its own</b> is read at the desk with a notebook under it: one entry per sum, a letter
        in pencil against each, and the scribe confirms them three at a time, in ink. Pathless as well, it
        is <b>the expert plate</b>: the way first, on sums alone (every tablet deals out the same values, so a
        sum that is none of them breaks the way), and three pairs that land open the notebook. <b>Broken</b> plates are rejoined on
        the board, where only the sums say where each fragment goes. <b>Ruined</b> plates are rejoined
        first, then restored.</p>
      <p>The decoder is one click below each puzzle, for a reader who wants the answer or has one to
        check. Plates read are counted in your scriptorium, by condition, in this browser only: a record
        of an exchange, not a score. There are no timers and no scores on the plate, and no sound unless you ask to hear the hand. The reward
        is the message.</p>
    </section>
  </article>

  <aside class="leaf" aria-labelledby="playground-title">
    <h2 id="playground-title" class="rubric rubric--small">Write a number</h2>
    <div class="playground">
      <div class="playground-glyph" data-playground-glyph>{playground_glyph}</div>
      <div class="field">
        <label for="number">Any number from 0 to 9,999</label>
        <input type="number" id="number" class="input" min="0" max="9999" step="1" value="1098" inputmode="numeric" data-playground>
      </div>
      <p class="playground-reading" data-playground-reading aria-live="polite">1 thousand · 0 hundreds · 9 tens · 8 units</p>
      <div class="playground-presets" aria-label="Examples">
          <button type="button" class="btn btn--small" data-playground-set="1098">1098</button>
          <button type="button" class="btn btn--small" data-playground-set="1533">1533</button>
          <button type="button" class="btn btn--small" data-playground-set="2001">2001</button>
          <button type="button" class="btn btn--small" data-playground-set="2026">2026</button>
          <button type="button" class="btn btn--small" data-playground-set="9999">9999</button>
      </div>
    </div>

    <section class="lectio" data-lectio aria-labelledby="lectio-title">
      <h3 id="lectio-title" class="index-heading">Lectio · read the sign</h3>
      <div class="lectio-sign" data-lectio-sign data-number="{first}">{lectio_glyph}</div>
      <form class="lectio-form" data-lectio-form>
        <label for="lectio-answer" class="visually-hidden">The number this sign reads</label>
        <input type="number" id="lectio-answer" class="input" min="0" max="9999" inputmode="numeric"
               autocomplete="off" placeholder="What does it read?" data-lectio-answer disabled>
        <button type="submit" class="btn btn--primary" data-lectio-submit disabled>Read</button>
      </form>
      <p class="lectio-feedback" data-lectio-feedback aria-live="polite">Signs start with the units. Read three in a row to take your vows.</p>
      <p class="lectio-standing"><span data-lectio-rank>Novice</span> · <span data-lectio-streak>0</span> in a row · best <span data-lectio-best>0</span></p>
    </section>

    <h3 class="index-heading">The tablet</h3>
    <p class="note">Each letter's number (times ten) in the game. The original tablet skips 17.</p>
    <ol class="tablet">
{tablet}    </ol>
    <div data-glyph-table="{glyph_table}" hidden></div>
  </aside>
</div>

<section class="leaf sources" aria-labelledby="sources-title">
  <h2 id="sources-title" class="index-heading">Sources</h2>
  <ul>
    <li>David A. King, <em>The Ciphers of the Monks: A Forgotten Number-notation of the Middle Ages</em>, Stuttgart: Franz Steiner, 2001.</li>
    <li><a href="https://en.wikipedia.org/wiki/Cistercian_numerals">Cistercian numerals</a>, Wikipedia, and the sources it cites.</li>
    <li><a href="https://www.dcode.fr/cistercian-numbers">dCode: Cistercian numbers</a>, a converter for the signs.</li>
    <li><a href="https://glossographia.wordpress.com/2013/07/09/cistercian-number-magic-of-the-boy-scouts/">Glossographia: Cistercian number magic of the boy scouts</a>.</li>
    <li><a href="https://en.wikipedia.org/wiki/Rongorongo">Rongorongo</a> and <a href="https://en.wikipedia.org/wiki/Boustrophedon">Boustrophedon</a>, Wikipedia, and the sources they cite.</li>
    <li><a href="https://en.wikipedia.org/wiki/Phaistos_Disc">Phaistos Disc</a> and <a href="https://en.wikipedia.org/wiki/Lead_Plaque_of_Magliano">Lead Plaque of Magliano</a>, Wikipedia, and the sources they cite.</li>
    <li>David Oranchak, Sam Blake and Jarl Van Eycke, <a href="https://arxiv.org/abs/2403.17350">The Solution of the Zodiac Killer’s 340-Character Cipher</a>, 2024; <a href="https://www.theregister.com/2020/12/12/zodiac_killers_cipher_solved/">the FBI’s confirmation</a>, December 2020.</li>
    <li><a href="https://en.wikipedia.org/wiki/Gortyn_code">Gortyn code</a>, Wikipedia, and the sources it cites.</li>
    <li>Gary Urton and Carrie J. Brezine, <a href="https://www.sciencenews.org/article/knotted-strings-and-inca-accounts">Khipu accounting in ancient Peru</a>, <em>Science</em> 309, 2005.</li>
    <li>Manuel Medrano and Ashok Khosla, <a href="https://www.cambridge.org/core/journals/latin-american-antiquity/article/abs/how-can-data-science-contribute-to-understanding-the-khipu-code/E2D0E68A0515F3F9582A23C63693F4CE">How Can Data Science Contribute to Understanding the Khipu Code?</a>, <em>Latin American Antiquity</em> 36, 2025.</li>
    <li><a href="https://saxa-loquuntur.nl/tools/epigraphic-sigla.html">Epigraphic sigla</a>: the Leiden conventions.</li>
    <li><a href="https://en.wikipedia.org/wiki/Cardan_grille">Cardan grille</a> and <a href="https://en.wikipedia.org/wiki/Grille_(cryptography)">Grille (cryptography)</a>, Wikipedia, and the sources they cite.</li>
    <li>Kevin Knight, Beáta Megyesi and Christiane Schaefer, <a href="https://aclanthology.org/W11-1202/">“The Copiale Cipher”</a>, Proceedings of the 4th Workshop on Building and Using Comparable Corpora, ACL, 2011; <a href="https://www.su.se/english/research/research-projects/decipherment-of-historical-manuscripts/the-copiale-cipher-1.688288">the project’s page</a> at Stockholm University.</li>
    <li><a href="https://en.wikipedia.org/wiki/Return_of_the_Obra_Dinn">Return of the Obra Dinn</a> (Lucas Pope, 2018), Wikipedia: fates confirmed in threes.</li>
  </ul>
</section>
"#,
        glyph_1098 = glyph_svg(1098, "glyph", Some("1098")),
        glyph_1533 = glyph_svg(1533, "glyph", Some("1533")),
        glyph_2001 = glyph_svg(2001, "glyph", Some("2001")),
        disc_spiral = disc_spiral_svg(),
        snail_phaistos = snail_svg(4, "phaistos"),
        boustrophedon = boustrophedon_svg(),
        snail_rongorongo = snail_svg(4, "rongorongo"),
        snail_leap = snail_svg(6, "leap"),
        snail_furrows = snail_svg(4, "furrows"),
        snail_unwinding = snail_svg(4, "unwinding"),
        cardano = cardano_svg(),
        grille_order = grille_order_svg(),
        playground_glyph = glyph_svg(1098, "glyph", Some("The Cistercian sign for 1098")),
        first = first,
        lectio_glyph = glyph_svg(first, "glyph", Some("A Cistercian sign to read")),
        tablet = tablet,
        glyph_table = h(&GLYPH_TABLE_JSON),
    )
}
