Gem::Specification.new do |spec|
  spec.name = 'example'
  spec.add_development_dependency 'foo', '>= 1.0', '< 2.0'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specify development dependencies in Gemfile.
  spec.add_development_dependency 'allowed', '>= 1.0', '< 2.0'
end
