# frozen_string_literal: true

# Generates tests/fixtures/geometry_vectors.json from the Ruby original.
require '/home/user/cistergraphia/lib/cistergraphia'
require 'json'

G = Cistergraphia::Glyphs
L = Cistergraphia::Layout
S = Cistergraphia::Stylus
v = {}
# every stem/digit stroke of every place, plus the stem, pressed by the page hand
strokes = [G::STEM] + G::LINES.values.flatten(1).uniq
v['press'] = strokes.to_h { |s| [s.join(','), S.press(s)] }
# seeded wedges, formatted as outlines for exactness
v['wedges'] = [[[100, 0, 50, 0], 1], [[50, 0, 50, 150], 7], [[50, 50, 100, 0], 42], [[0, 150, 100, 150], -3]].map do |stroke, seed|
  S.wedge(stroke, Random.new(seed)).map { |poly| S.outline(poly) }
end
# layout paths, all five ways, sizes 1..6
v['paths'] = L::KEYLESS.to_h do |way|
  [way, (1..6).map { |n| L.path(n, way) }]
end
# glyph strokes
v['strokes'] = [0, 1, 9, 10, 47, 100, 666, 1000, 1993, 7777, 9000, 9999].to_h { |n| [n.to_s, G.strokes(n, 7, 11)] }
# dimensions and origins
v['dims'] = [[4, false], [4, true], [9, false], [16, true]].map { |len, tally| L.dimensions(Array.new(len), :phaistos, tally: tally) }
grille = Cistergraphia::Grille.new('K7Q2M-ZXB4P')
v['grille_dims'] = L.dimensions(Array.new(16), grille)
v['grille_origin'] = L.origin(1, 2, grille)
v['cell_of'] = [[50, 50, 150, 50], [10, 10, 20, 20], [290, 250, 290, 210]].map { |line| L.cell_of(line) }
File.write('crates/rustergraphia/tests/fixtures/geometry_vectors.json', JSON.pretty_generate(v))
puts 'ok'
