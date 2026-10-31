require File.join(ENV.fetch('CISTERGRAPHIA_DIR'), 'lib/cistergraphia')
C = Cistergraphia
out = ARGV[0]
msg = 'Meet me at the old abbey after vespers'
key = C::Grille.new('K7Q2M-ZXB4P')
w = ->(name, svg) { File.write("#{out}/#{name}.svg", svg) }
w.('intact',     C.inscribe(msg, layout: :phaistos, random: Random.new(1234))[1])
w.('rongorongo', C.inscribe(msg, layout: :rongorongo, random: Random.new(1234))[1])
w.('grille',     C.inscribe('The relics are under the third stone', layout: key, random: Random.new(55))[1])
w.('pathless',   C.inscribe('Vespers at six', layout: :leap, pathless: true, random: Random.new(77))[1])
w.('tablet',     C.inscribe(msg, tablet: 'ABCDEFGH23', random: Random.new(99))[1])
w.('worn',       C::Lacunae.inscribe('Ora et labora et ora', random: Random.new(5))[1])
w.('tallied',    C::Lacunae.inscribe('Ora et labora', tally: true, random: Random.new(7))[1])
w.('broken',     C::Fragments.inscribe(msg, random: Random.new(31))[1])
w.('ruined',     C::Fragments.inscribe('Ora et labora et ora', weathered: true, random: Random.new(4))[1])
w.('sealed',     C.inscribe('In girum imus nocte et consumimur igni', passphrase: 'monastery gate')[1])
_, svg, _ = C::TwoReaders.inscribe('The relics are under the third stone', 'Vespers at six', grille: key)
w.('two_readers', svg)
puts Dir["#{out}/*.svg"].size
