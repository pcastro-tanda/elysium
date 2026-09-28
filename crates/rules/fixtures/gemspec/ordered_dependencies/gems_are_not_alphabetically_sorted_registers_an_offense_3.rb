Gem::Specification.new do |spec|
  spec.add_development_dependency 'rubocop'
  spec.add_development_dependency 'rspec'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Dependencies should be sorted in an alphabetical order within their section of the gemspec. Dependency `rspec` should appear before `rubocop`.
end
