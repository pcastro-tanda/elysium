Gem::Specification.new do |spec|
  spec.name = 'your_cool_gem_name'
  spec.test_files = Dir.glob('test/**/*')
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not set `test_files` in gemspec.
  spec.bindir = 'exe'
end
