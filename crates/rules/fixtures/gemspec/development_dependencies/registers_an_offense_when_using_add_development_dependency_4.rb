Gem::Specification.new do |spec|
  spec.name = 'example'
  spec.add_development_dependency 'foo'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Specify development dependencies in gems.rb.
  spec.add_development_dependency 'allowed'
end
