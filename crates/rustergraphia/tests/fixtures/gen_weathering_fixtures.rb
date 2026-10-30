# frozen_string_literal: true

# Generates tests/fixtures/weathering_vectors.json from the Ruby original:
#   ruby gen_weathering_fixtures.rb /path/to/cistergraphia
require File.join(ARGV.fetch(0, '/home/user/cistergraphia'), 'lib/cistergraphia')
require 'json'

C = Cistergraphia
cases = []
[['ora et labora et ora', :phaistos, false, false, 42],
 ['the brothers will gather in the chapter house after compline', :rongorongo, false, false, 7],
 ['the brothers will gather in the chapter house after compline', :phaistos, true, false, 7],
 ['Meet me at the old abbey after vespers', :leap, false, true, 5]].each do |message, layout, tally, pathless, seed|
  sequence, svg, damage = C::Lacunae.inscribe(message, layout: layout, tally: tally, pathless: pathless,
                                                       random: Random.new(seed))
  cases << { message: message, layout: layout, tally: tally, pathless: pathless, seed: seed,
             sequence: sequence, svg: svg, damage: damage.map { |i, lost| [i, lost] },
             edition: C.decode(svg),
             survey_layout: (C::Lacunae::Survey.layout(svg) unless pathless) }
end

g = C::Grille.new('K7Q2M-ZXB4P')
sequence, svg, damage = C::Lacunae.inscribe('ora et labora', layout: g, random: Random.new(11))
cases << { message: 'ora et labora', layout: 'grille:K7Q2MZXB4P', tally: false, pathless: false, seed: 11,
           sequence: sequence, svg: svg, damage: damage.map { |i, l| [i, l] },
           edition: C.decode(svg, layout: g), survey_layout: nil }

seqref = C.encode('ora et labora', random: Random.new(3))
tally = C::Tally.signs(seqref, :phaistos)
File.write(File.join(__dir__, 'weathering_vectors.json'), JSON.pretty_generate({
  cases: cases,
  tally_ref: { sequence: seqref, rows: tally[:rows], cols: tally[:cols], corner: tally[:corner],
               adds: [[[], 0], [[1234, 4321], 5555], [[9999, 2469], 1358]] }
}))
puts cases.map { |c| [c[:layout].to_s, c[:svg].bytesize, c[:edition]].join(' | ') }
