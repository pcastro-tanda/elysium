::Gem::Specification.new do |spec|
  spec.metadata['key'] = 1
  spec.metadata[:key] = 2
  spec.metadata['key'] = 2
  ^^^^^^^^^^^^^^^^^^^^^^^^ `metadata['key']=` method calls already given on line 2 of the gemspec.
  spec.metadata['key'] = 3
  ^^^^^^^^^^^^^^^^^^^^^^^^ `metadata['key']=` method calls already given on line 2 of the gemspec.
end
