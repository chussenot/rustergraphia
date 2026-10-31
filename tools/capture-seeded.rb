# frozen_string_literal: true

# Captures the Ruby app's result pages (POST /generate) for
# crates/rustergraphia-web/tests/seeded.rs: each case's plate is drawn from
# Random.new(seed) (the splits, the wear, the breaks), so the Rust port,
# given the same seed (Config::plate_seed), must lay the same page to the byte.
#
# The cases are crates/rustergraphia-web/tests/fixtures/seeded/cases.json;
# only plates whose every draw comes from that Random can be seeded (not a
# new grille key, a tablet of its own, a sealed plate or two readers, which
# draw from SecureRandom or OpenSSL).
#
# Usage, from the Ruby original's checkout (its gems installed):
#   bundle exec ruby /path/to/rustergraphia/tools/capture-seeded.rb
ENV['APP_ENV'] = 'test'
require 'json'
require 'zlib'
require 'rack/test'
require File.expand_path('app', Dir.pwd)

FIXTURES = File.expand_path('../crates/rustergraphia-web/tests/fixtures/seeded', __dir__)

$plate_seed = nil
# Random.new without a seed, as the library's defaults call it, draws from the case's
class << Random
  alias_method :unseeded_new, :new
  def new(*args)
    args.empty? && $plate_seed ? unseeded_new($plate_seed) : unseeded_new(*args)
  end
end

# Drives the app as the suites do
class Capture
  include Rack::Test::Methods

  def app = CistergraphiaApp

  def run
    JSON.parse(File.read(File.join(FIXTURES, 'cases.json'))).each do |c|
      $plate_seed = c['seed']
      srand(c['seed']) # Cistergraphia.inscribe draws from Random itself
      post '/generate', c['form']
      $plate_seed = nil
      raise "#{c['name']}: #{last_response.status}" unless last_response.ok?

      Zlib::GzipWriter.open(File.join(FIXTURES, "#{c['name']}.body.gz")) { |gz| gz.write(last_response.body) }
      puts "#{c['name']}: #{last_response.body.bytesize} bytes"
    end
  end
end

Capture.new.run
