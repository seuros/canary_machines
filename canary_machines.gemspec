# frozen_string_literal: true
require_relative 'lib/canary_machines/version'

Gem::Specification.new do |spec|
  spec.name = 'canary_machines'
  spec.version = CanaryMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']
  spec.summary = 'Sequential canary analysis with stated error bounds for Ruby'
  spec.description = 'Sequential probability ratio tests over Bernoulli trials and matched pairs, with the ' \
                     'false-rollback and false-promote rates fixed before the first sample, bound to Ruby ' \
                     'from the no_std canary-machines crate. A canary of several tests recommends promote, ' \
                     'roll back, continue or hold; it never touches the deployment.'
  spec.homepage = 'https://github.com/seuros/canary_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'canary_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/canary_machines_native/ffi/Cargo.toml'
  spec.files = Dir.glob(%w[
    lib/**/*.rb sig/**/*.rbs ext/**/*.{rs,rb,toml,md} Cargo.toml
    Cargo.lock README.md CHANGELOG.md LICENSE.txt
  ]).select { |p| File.file?(p) }
    .reject { |p| p.end_with?('/tests.rs') || p.include?('/tests/') }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/canary_machines_native/extconf.rb']
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
