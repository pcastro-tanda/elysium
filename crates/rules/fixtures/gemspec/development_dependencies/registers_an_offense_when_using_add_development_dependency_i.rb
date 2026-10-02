Gem::Specification.new do |spec|
  spec.name = 'example'
  spec.add_development_dependency 'foo'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specify development dependencies in Gemfile.
  spec.add_development_dependency 'allowed'
end
