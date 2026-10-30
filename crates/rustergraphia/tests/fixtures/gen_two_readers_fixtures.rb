# frozen_string_literal: true

# Generates the two-reader and notebook interop fixtures from the Ruby library.
require '/home/user/cistergraphia/lib/cistergraphia'
require 'json'

KEY = 'K7Q2M-ZXB4P'
dir = __dir__

# A game plate with two readers, under a known abbot's key
_s, svg, abbot = Cistergraphia::TwoReaders.inscribe(
  'Relics lie under the third pillar', 'Vespers at six in the choir', grille: Cistergraphia::Grille.new(KEY)
)
File.write(File.join(dir, 'two_readers_game.svg'), svg)
raise unless abbot.code == 'K7Q2MZXB4P'

# A sealed plate with two readers
_s, svg, = Cistergraphia::TwoReaders.inscribe(
  'Les reliques sont à Cîteaux', 'Vêpres à six heures',
  passphrase: 'monastery gate', grille: Cistergraphia::Grille.new(KEY)
)
File.write(File.join(dir, 'two_readers_sealed.svg'), svg)

# The notebook plate: seeded, so Rust regenerates it byte for byte
sequence, svg = Cistergraphia.inscribe('Meet me at the old abbey after vespers',
                                       layout: :furrows, tablet: KEY, random: Random.new(2024))
File.write(File.join(dir, 'notebook_plate.svg'), svg)
entries = Cistergraphia::Notebook.entries(sequence)
tablet = Cistergraphia::Tablet.characters(KEY)
sums = entries.map { |e| e[:sum] }
expect = {
  sequence: sequence,
  entries: entries.map { |e| { sum: e[:sum], seen: e[:seen], spellings: e[:spellings] } },
  tablet: tablet.to_a.sort,
  held_first3: Cistergraphia::Notebook.held(svg, sums.first(3).to_h { |s| [s, tablet[s]] }).to_a.sort,
  held_first2: Cistergraphia::Notebook.held(svg, sums.first(2).to_h { |s| [s, tablet[s]] }).to_a.sort,
  held_last2_inked: Cistergraphia::Notebook.held(svg, sums.last(2).to_h { |s| [s, tablet[s]] },
                                                 sums.first(sums.size - 2)).to_a.sort
}
File.write(File.join(dir, 'notebook_expect.json'), JSON.pretty_generate(expect))
puts 'ok'
