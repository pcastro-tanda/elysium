Gem::Specification.new do |spec|
  spec.name = 'example'
  spec.add_development_dependency 'foo', '>= 1.0'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specify development dependencies in gems.rb.
  spec.add_development_dependency 'allowed', '>= 1.0'
end
